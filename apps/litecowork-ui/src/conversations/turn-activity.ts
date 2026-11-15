/**
 * Pure model for REPORTED turn activity (001-T25, AC-029).
 *
 * Entries come from transient `turn.activity` frames and from the stored REPORTED records
 * read back after a restart. They describe what the native agent said it did. They are
 * never LiteCowork Effects, receipts or grants, and the copy always says "reported by the
 * agent". Labels are bounded display words; no path, argument, output or reasoning text.
 */

export const ACTIVITY_KINDS = ["TOOL", "FILE", "PERMISSION", "TODO", "STEP", "CONTEXT"] as const;
export const ACTIVITY_STATUSES = ["STARTED", "PROGRESS", "SUCCEEDED", "FAILED", "ASKED", "UPDATED", "ENDED", "REDUCED"] as const;
export const MAX_ACTIVITY_ENTRIES = 200;

export type ActivityKind = (typeof ACTIVITY_KINDS)[number];
export type ActivityStatus = (typeof ACTIVITY_STATUSES)[number];

export type ActivityEntry = {
  sequence: number;
  kind: ActivityKind;
  status: ActivityStatus;
  callRef: string | null;
  label: string | null;
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function boundedOrNull(value: unknown, max: number): string | null | undefined {
  if (value === null) return null;
  if (typeof value !== "string" || value.length > max || /[\u0000-\u001f]/.test(value)) return undefined;
  return value;
}

/** Validates the shared fields of a frame or stored record; returns null on any violation. */
export function parseActivityEntry(value: unknown): ActivityEntry | null {
  if (!isRecord(value)) return null;
  const { sequence, kind, status } = value;
  if (!Number.isSafeInteger(sequence) || (sequence as number) < 1 || (sequence as number) > MAX_ACTIVITY_ENTRIES) return null;
  if (!(ACTIVITY_KINDS as readonly unknown[]).includes(kind) || !(ACTIVITY_STATUSES as readonly unknown[]).includes(status)) return null;
  const callRef = boundedOrNull(value.call_ref ?? null, 128);
  const label = boundedOrNull(value.label ?? null, 128);
  if (callRef === undefined || label === undefined) return null;
  return { sequence: sequence as number, kind: kind as ActivityKind, status: status as ActivityStatus, callRef, label };
}

/** Parses the served `listConversationTurnActivity` page; any malformed item rejects the page. */
export function parseActivityPage(value: unknown, turnId: string, retryOrdinal: number): ActivityEntry[] | null {
  if (!isRecord(value) || value.turn_id !== turnId || value.retry_ordinal !== retryOrdinal
    || value.attribution !== "REPORTED" || !Array.isArray(value.items) || value.items.length > MAX_ACTIVITY_ENTRIES) return null;
  const entries: ActivityEntry[] = [];
  for (const item of value.items) {
    const entry = parseActivityEntry(item);
    if (!entry) return null;
    entries.push(entry);
  }
  return mergeActivity([], entries);
}

/** Merges entries by sequence (ascending, de-duplicated, bounded). Existing entries win. */
export function mergeActivity(current: readonly ActivityEntry[], incoming: readonly ActivityEntry[]): ActivityEntry[] {
  const bySequence = new Map<number, ActivityEntry>();
  for (const entry of incoming) bySequence.set(entry.sequence, entry);
  for (const entry of current) bySequence.set(entry.sequence, entry);
  return [...bySequence.values()].sort((a, b) => a.sequence - b.sequence).slice(0, MAX_ACTIVITY_ENTRIES);
}

export type ActivityCardState = "RUNNING" | "SUCCEEDED" | "FAILED" | "ASKED" | "RECORDED";

export type ActivityCard = {
  key: string;
  title: string;
  state: ActivityCardState;
  /** Visible status words, always attributed to the agent. */
  summary: string;
};

function titleFor(entry: ActivityEntry): string {
  switch (entry.kind) {
    case "TOOL": return entry.label ? `Tool ${entry.label}` : "Tool";
    case "FILE": return "File change";
    case "PERMISSION": return "Permission question";
    case "TODO": return "Plan";
    case "STEP": return "Step";
    case "CONTEXT": return "Context";
  }
}

/**
 * Collapses TOOL start/settle pairs sharing a call reference into one card. Every other
 * observation is its own card. Copy says "reported by the agent", never "done" or "verified".
 */
export function buildActivityCards(entries: readonly ActivityEntry[]): ActivityCard[] {
  const cards: ActivityCard[] = [];
  const toolCards = new Map<string, number>();
  for (const entry of entries) {
    if (entry.kind === "TODO") continue; // shown by the plan checklist, not as a card
    if (entry.kind === "TOOL" && entry.callRef) {
      const existing = toolCards.get(entry.callRef);
      if (existing !== undefined) {
        const card = cards[existing]!;
        if (entry.status === "SUCCEEDED") cards[existing] = { ...card, state: "SUCCEEDED", summary: "Reported by the agent: finished" };
        else if (entry.status === "FAILED") cards[existing] = { ...card, state: "FAILED", summary: "Reported by the agent: failed" };
        continue;
      }
      toolCards.set(entry.callRef, cards.length);
    }
    const state: ActivityCardState = entry.status === "FAILED" ? "FAILED"
      : entry.status === "SUCCEEDED" ? "SUCCEEDED"
      : entry.status === "ASKED" ? "ASKED"
      : entry.status === "STARTED" || entry.status === "PROGRESS" ? "RUNNING"
      : "RECORDED";
    const summary = state === "RUNNING" ? "Reported by the agent: running"
      : state === "SUCCEEDED" ? "Reported by the agent: finished"
      : state === "FAILED" ? "Reported by the agent: failed"
      : state === "ASKED" ? "Reported by the agent: the agent requested permission. LiteCowork is not asking or relaying an answer, and this activity record does not show whether the request is still open."
      : "Reported by the agent";
    cards.push({ key: `${entry.sequence}`, title: titleFor(entry), state, summary });
  }
  return cards;
}

export type PlanChecklist = { itemCount: number; label: string };

/**
 * The latest plan update. The normalizer keeps only the item count (item text is not
 * captured), so the checklist is a count line, not a list of items.
 */
export function planChecklist(entries: readonly ActivityEntry[]): PlanChecklist | null {
  for (let index = entries.length - 1; index >= 0; index -= 1) {
    const entry = entries[index]!;
    if (entry.kind !== "TODO") continue;
    const count = Number.parseInt(entry.label ?? "", 10);
    if (!Number.isSafeInteger(count) || count < 0) return null;
    return { itemCount: count, label: `Plan reported by the agent: ${count} ${count === 1 ? "item" : "items"}` };
  }
  return null;
}
