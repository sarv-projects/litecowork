export type AutomationSetupStatus = "ENABLED" | "PAUSED" | "DISABLED";
export type AutomationSetupStepState = "DONE" | "TODO" | "BLOCKED";

export type AutomationSetupStep = {
  id: "REVIEW" | "TEST" | "ENABLE";
  label: string;
  state: AutomationSetupStepState;
  detail: string;
};

export type AutomationTriggerKind = "SCHEDULE" | "ONE_SHOT" | "MANUAL";

export const AUTOMATION_TRIGGER_OPTIONS = [
  { kind: "SCHEDULE", label: "Recurring schedule", supported: true, reason: "" },
  { kind: "ONE_SHOT", label: "One time", supported: false, reason: "One-time triggers are not supported by the current local trigger host." },
  { kind: "MANUAL", label: "Manual", supported: true, reason: "" },
] as const;

export function isSupportedAutomationTriggerKind(kind: string): boolean {
  return AUTOMATION_TRIGGER_OPTIONS.some(option => option.kind === kind && option.supported);
}

const MONTH_NAMES = ["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"];
const DAY_NAMES = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];

function cronValue(value: string, minimum: number, maximum: number, names: readonly string[] = [], nameOffset = 0): number | null {
  const trimmed = value.trim();
  if (/^\d+$/.test(trimmed)) {
    const parsed = Number(trimmed);
    return Number.isSafeInteger(parsed) && parsed >= minimum && parsed <= maximum ? parsed : null;
  }
  if (names.length === 0) return null;

  const upper = trimmed.toUpperCase();
  const exactIndex = names.findIndex(name => name === upper);
  if (exactIndex >= 0) return exactIndex + nameOffset;
  if (upper.length !== 2) return null;
  const matches = names.flatMap((name, index) => name.startsWith(upper) ? [index] : []);
  return matches.length === 1 ? matches[0] + nameOffset : null;
}

function validCronField(field: string, minimum: number, maximum: number, names: readonly string[] = [], nameOffset = 0): boolean {
  const values = new Set<number>();
  for (const rawItem of field.split(",")) {
    const item = rawItem.trim();
    if (!item) return false;
    const parts = item.split("/");
    if (parts.length > 2) return false;
    const [rangePart, rawStep] = parts;
    const step = rawStep === undefined ? 1 : cronValue(rawStep, 1, maximum);
    if (step === null) return false;

    let start: number | null;
    let end: number | null;
    if (rangePart === "*") {
      start = minimum;
      end = maximum;
    } else if (rangePart.includes("-")) {
      const range = rangePart.split("-");
      if (range.length !== 2) return false;
      start = cronValue(range[0], minimum, maximum, names, nameOffset);
      end = cronValue(range[1], minimum, maximum, names, nameOffset);
    } else {
      start = cronValue(rangePart, minimum, maximum, names, nameOffset);
      end = start === null ? null : step === 1 ? start : maximum;
    }
    if (start === null || end === null || start > end) return false;
    for (let value = start; value <= end; value += step) {
      values.add(value);
      if (values.size > 128) return false;
    }
  }
  return values.size > 0;
}

/** Mirrors the local trigger host's five-field Cron5 parser for early UI feedback. */
export function isValidLocalCron5(expression: string): boolean {
  if (typeof expression !== "string") return false;
  const fields = expression.trim().split(/\s+/);
  if (fields.length !== 5) return false;
  const bounds = [
    { minimum: 0, maximum: 59, names: [] as readonly string[], nameOffset: 0 },
    { minimum: 0, maximum: 23, names: [] as readonly string[], nameOffset: 0 },
    { minimum: 1, maximum: 31, names: [] as readonly string[], nameOffset: 0 },
    { minimum: 1, maximum: 12, names: MONTH_NAMES, nameOffset: 1 },
    { minimum: 0, maximum: 7, names: DAY_NAMES, nameOffset: 0 },
  ];
  return fields.every((field, index) => {
    const bound = bounds[index];
    return validCronField(field, bound.minimum, bound.maximum, bound.names, bound.nameOffset);
  });
}

export function isValidEditorCron5(expression: string): boolean {
  const normalized = expression.trim();
  if (!normalized || normalized.length > 128) return false;
  const fields = normalized.split(/\s+/);
  return fields.length === 5
    && fields.every(field => /^[0-9*/,-]+$/.test(field))
    && isValidLocalCron5(normalized);
}

export function isValidIanaTimezone(timezone: unknown): timezone is string {
  if (typeof timezone !== "string" || !timezone.trim() || timezone.length > 80) return false;
  try {
    new Intl.DateTimeFormat("en", { timeZone: timezone.trim() }).format();
    return true;
  } catch {
    return false;
  }
}

export function unsupportedAutomationTriggerKinds(kinds: readonly string[]): string[] {
  return [...new Set(kinds.filter(kind => !isSupportedAutomationTriggerKind(kind)))];
}

type Input = {
  status: AutomationSetupStatus;
  hasDefinition: boolean;
  hasTestTask: boolean;
  routineActive: boolean;
  hostReady: boolean | null;
  triggerSupported: boolean;
};

/** A truthful, non-authoritative checklist. The server rechecks every gate. */
export function automationSetupSteps(input: Input): AutomationSetupStep[] {
  const disabled = input.status === "DISABLED";
  const reviewState: AutomationSetupStepState = !input.hasDefinition
    ? "BLOCKED"
    : input.routineActive ? "DONE" : "BLOCKED";
  const testState: AutomationSetupStepState = input.hasTestTask
    ? "DONE"
    : disabled ? "BLOCKED" : "TODO";
  const enableBlocked = disabled
    || !input.hasDefinition
    || !input.routineActive
    || !input.triggerSupported
    || input.hostReady === false;
  const enabledBlocker = !input.hasDefinition
    ? "Stored status is Enabled, but the pinned Automation definition is unavailable."
    : !input.routineActive
      ? "Stored status is Enabled, but the pinned Routine is unavailable or archived."
      : !input.triggerSupported
        ? "Stored status is Enabled, but this Runtime cannot host one or more trigger types in this revision."
        : input.hostReady === false
          ? "Stored status is Enabled, but this Runtime does not currently hold an active trigger-host binding."
          : input.hostReady === null
            ? "Stored status is Enabled, but host readiness has not been observed; scheduling is not confirmed."
            : null;
  const enableState: AutomationSetupStepState = input.status === "ENABLED"
    ? enabledBlocker === null ? "DONE" : "BLOCKED"
    : enableBlocked ? "BLOCKED" : "TODO";
  return [
    {
      id: "REVIEW",
      label: "Review the saved definition",
      state: reviewState,
      detail: !input.hasDefinition
        ? "Open a saved Automation revision before testing."
        : input.routineActive
          ? "The exact Automation and Routine revisions are pinned."
          : "The pinned Routine is archived. Revise the definition before proceeding.",
    },
    {
      id: "TEST",
      label: "Create a test Task",
      state: testState,
      detail: input.hasTestTask
        ? "A real READY test Task was created with no occurrence or cursor."
        : disabled
          ? "A disabled Automation cannot create a test Task."
          : "Test the exact revision with the same bounded inputs. This is real work, not a dry run.",
    },
    {
      id: "ENABLE",
      label: "Enable the definition",
      state: enableState,
      detail: input.status === "ENABLED"
        ? enabledBlocker ?? "Enabled. It runs only while this Runtime holds the trigger-host binding."
        : disabled
          ? "Disabled is terminal."
          : !input.hasDefinition || !input.routineActive
            ? "A saved definition with an active pinned Routine is required."
            : !input.triggerSupported
              ? "This Runtime cannot host one or more trigger types in this revision."
              : input.hostReady === false
                ? "This Runtime does not currently hold an active trigger-host binding."
                : input.hostReady === null
                  ? "Host readiness is not observed yet; enable will recheck it."
                  : "Enabling registers trigger-host ownership atomically and is separate from testing.",
    },
  ];
}
