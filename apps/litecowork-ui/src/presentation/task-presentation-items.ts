import {
  projectPresentationEntries,
  type PresentationEntry,
  type PresentationItem,
} from "./presentation-types.ts";

export type TaskPresentationPartition = {
  /** The committed Task outcome row, when the snapshot carries one. */
  readonly task: Extract<PresentationItem, { kind: "TASK_CARD" }> | null;
  readonly activity: Extract<PresentationItem, { kind: "ACTIVITY" }>[];
  readonly artifacts: Extract<PresentationItem, { kind: "ARTIFACT" }>[];
  /** Other validated item kinds that the Task surface still renders. */
  readonly other: PresentationItem[];
  /** Saved items that could not be validated. They are never silently dropped. */
  readonly rejected: Extract<PresentationEntry, { entry: "REJECTED" }>[];
  /** Saved items beyond the display limit. */
  readonly omittedCount: number;
};

/**
 * Partition an untrusted Task presentation snapshot into the rows the Task surface
 * renders.
 *
 * A saved item that fails validation becomes a bounded placeholder instead of
 * disappearing, so the Task stays honestly described. Nothing here interprets a
 * payload, fabricates status, or starts work.
 */
export function partitionTaskPresentationItems(
  items: readonly unknown[],
): TaskPresentationPartition {
  const projection = projectPresentationEntries(items);
  const validated = projection.entries
    .filter((entry): entry is Extract<PresentationEntry, { entry: "ITEM" }> => entry.entry === "ITEM")
    .map(entry => entry.item);
  const rejected = projection.entries.filter(
    (entry): entry is Extract<PresentationEntry, { entry: "REJECTED" }> => entry.entry === "REJECTED",
  );
  const task = validated.find((item): item is Extract<PresentationItem, { kind: "TASK_CARD" }> =>
    item.kind === "TASK_CARD",
  ) ?? null;
  const activity = validated.filter(
    (item): item is Extract<PresentationItem, { kind: "ACTIVITY" }> => item.kind === "ACTIVITY",
  );
  const artifacts = validated.filter(
    (item): item is Extract<PresentationItem, { kind: "ARTIFACT" }> => item.kind === "ARTIFACT",
  );
  const other = validated.filter(
    item => item.kind !== "TASK_CARD" && item.kind !== "ACTIVITY" && item.kind !== "ARTIFACT",
  );
  return { task, activity, artifacts, other, rejected, omittedCount: projection.omittedCount };
}
