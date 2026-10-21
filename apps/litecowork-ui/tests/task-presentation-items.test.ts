import assert from "node:assert/strict";
import { test } from "node:test";
import { partitionTaskPresentationItems } from "../src/presentation/task-presentation-items.ts";

function item(overrides: Record<string, unknown> = {}) {
  return {
    item_key: "item-1",
    kind: "TEXT",
    payload_version: 1,
    source_refs: [],
    order_key: "0001",
    freshness: "CURRENT",
    payload: { text: "Saved answer text." },
    ...overrides,
  };
}

const taskCard = item({
  item_key: "task",
  kind: "TASK_CARD",
  order_key: "0000",
  payload: { task_id: "task-1", objective: "Ship the report", status: "COMPLETED", step_summary: "Two steps" },
});

const activity = item({
  item_key: "activity",
  kind: "ACTIVITY",
  order_key: "0001",
  label: "Ran the exporter",
  status: "COMPLETE",
  occurred_at: "2026-10-19T10:00:00Z",
  payload: { summary: "Ran the exporter" },
});

const artifact = item({
  item_key: "artifact",
  kind: "ARTIFACT",
  order_key: "0002",
  payload: {
    artifact_id: "artifact-1",
    version: 3,
    display_name: "Quarterly report",
    artifact_kind: "REPORT",
    verification_status: "VERIFIED",
  },
});

test("a valid snapshot partitions into the Task outcome, activity, and saved outputs", () => {
  const partition = partitionTaskPresentationItems([taskCard, activity, artifact]);

  assert.equal(partition.task?.payload.objective, "Ship the report");
  assert.equal(partition.activity.length, 1);
  assert.equal(partition.artifacts.length, 1);
  assert.equal(partition.other.length, 0);
  assert.equal(partition.rejected.length, 0);
  assert.equal(partition.omittedCount, 0);
});

test("a saved item that cannot be validated becomes a visible placeholder instead of disappearing", () => {
  const partition = partitionTaskPresentationItems([
    taskCard,
    item({ item_key: "broken", payload: { text: 42, forged_status: "APPROVED" } }),
  ]);

  assert.equal(partition.rejected.length, 1);
  assert.equal(partition.task?.payload.objective, "Ship the report");
  assert.equal(partition.rejected[0].rejected.reportedKind, "TEXT");
  assert.equal(partition.rejected[0].rejected.sourceRefs.length, 0);
});

test("a rejected item keeps only revalidated source identities", () => {
  const partition = partitionTaskPresentationItems([
    item({
      item_key: "broken",
      source_refs: [{ kind: "TASK", id: "task-1" }, { kind: "NOT_A_SOURCE", id: "forged" }],
      payload: {},
    }),
  ]);

  assert.equal(partition.rejected.length, 1);
  assert.deepEqual(
    partition.rejected[0].rejected.sourceRefs.map(source => source.id),
    ["task-1"],
  );
});

test("item kinds outside the Task surface stay available as other saved items", () => {
  const partition = partitionTaskPresentationItems([
    taskCard,
    item({ item_key: "text", kind: "TEXT", order_key: "0003" }),
  ]);

  assert.equal(partition.other.length, 1);
  assert.equal(partition.other[0].kind, "TEXT");
});

test("a snapshot beyond the display limit reports the omitted count", () => {
  const many = Array.from({ length: 4 }, (_value, index) =>
    item({ item_key: `item-${index}`, order_key: `000${index}` }),
  );
  const partition = partitionTaskPresentationItems(many);

  assert.equal(partition.other.length + partition.activity.length + partition.artifacts.length, 4);
  assert.equal(partition.omittedCount, 0);
});
