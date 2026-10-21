export type AutomationSetupStatus = "ENABLED" | "PAUSED" | "DISABLED";
export type AutomationSetupStepState = "DONE" | "TODO" | "BLOCKED";

export type AutomationSetupStep = {
  id: "REVIEW" | "TEST" | "ENABLE";
  label: string;
  state: AutomationSetupStepState;
  detail: string;
};

type Input = {
  status: AutomationSetupStatus;
  hasDefinition: boolean;
  hasTestTask: boolean;
  routineActive: boolean;
  hostReady: boolean | null;
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
    || input.hostReady === false;
  const enableState: AutomationSetupStepState = input.status === "ENABLED"
    ? "DONE"
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
        ? "Enabled. It runs only while this Runtime holds the trigger-host binding."
        : disabled
          ? "Disabled is terminal."
          : !input.hasDefinition || !input.routineActive
            ? "A saved definition with an active pinned Routine is required."
            : input.hostReady === false
              ? "This Runtime does not currently hold an active trigger-host binding."
              : input.hostReady === null
                ? "Host readiness is not observed yet; enable will recheck it."
                : "Enabling registers trigger-host ownership atomically and is separate from testing.",
    },
  ];
}
