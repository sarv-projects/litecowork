import assert from "node:assert/strict";
import test from "node:test";
import { buildActivityCards, mergeActivity, parseActivityEntry, parseActivityPage, planChecklist } from "../src/conversations/turn-activity.ts";
import { emptyPresentationStream, reducePresentationFrame } from "../src/presentation/presentation-stream.ts";

const entry = (sequence: number, kind: string, status: string, callRef: string | null = null, label: string | null = null) =>
  ({ sequence, kind, status, call_ref: callRef, label });

test("turn activity entries are validated against the closed kinds, statuses and bounds", () => {
  assert.ok(parseActivityEntry(entry(1, "TOOL", "STARTED", "c1", "bash")));
  assert.equal(parseActivityEntry(entry(1, "EFFECT", "STARTED")), null, "no Effect kind");
  assert.equal(parseActivityEntry(entry(0, "TOOL", "STARTED")), null);
  assert.equal(parseActivityEntry(entry(1, "TOOL", "STARTED", null, "x".repeat(129))), null);
});

test("tool start and settle pairs collapse into one card worded as reported by the agent", () => {
  const entries = [1, 2, 3, 4].map((n) => parseActivityEntry(
    [entry(1, "TOOL", "STARTED", "c1", "bash"), entry(2, "TOOL", "FAILED", "c1"), entry(3, "FILE", "UPDATED"), entry(4, "PERMISSION", "ASKED", "p1")][n - 1])!);
  const cards = buildActivityCards(entries);
  assert.equal(cards.length, 3);
  assert.equal(cards[0]!.state, "FAILED");
  assert.equal(cards[0]!.title, "Tool bash");
  assert.ok(cards.every((card) => card.summary.startsWith("Reported by the agent")));
  const asked = cards.find((card) => card.state === "ASKED")!;
  assert.match(asked.summary, /the agent requested permission/);
  assert.match(asked.summary, /LiteCowork is not asking/);
  assert.match(asked.summary, /this activity record does not show whether the request is still open/i);
  assert.doesNotMatch(asked.summary, /remains pending|default applies/i, "a stored report is not live permission state");
  assert.doesNotMatch(asked.summary, /approve|allow|deny/i, "no answer control is implied");
});

test("the plan checklist is the latest reported todo count and never invents items", () => {
  const entries = [entry(1, "TODO", "UPDATED", null, "2"), entry(2, "TODO", "UPDATED", null, "1")].map((e) => parseActivityEntry(e)!);
  assert.equal(planChecklist(entries)?.label, "Plan reported by the agent: 1 item");
  assert.equal(planChecklist([]), null);
  assert.equal(buildActivityCards(entries).length, 0, "todo is shown by the checklist, not as a card");
});

test("stored activity pages must match the requested turn and be REPORTED", () => {
  const page = { turn_id: "t1", retry_ordinal: 0, attribution: "REPORTED", items: [entry(2, "STEP", "ENDED"), entry(1, "STEP", "STARTED")] };
  assert.deepEqual(parseActivityPage(page, "t1", 0)?.map((e) => e.sequence), [1, 2]);
  assert.equal(parseActivityPage({ ...page, attribution: "EFFECT" }, "t1", 0), null);
  assert.equal(parseActivityPage(page, "other", 0), null);
  assert.equal(parseActivityPage({ ...page, items: [{ nope: true }] }, "t1", 0), null);
  assert.equal(mergeActivity(parseActivityPage(page, "t1", 0)!, [parseActivityEntry(entry(2, "STEP", "ENDED"))!]).length, 2);
});

test("the stream reducer accumulates turn.activity frames in order and rejects foreign scope", () => {
  let state = reducePresentationFrame(emptyPresentationStream, { type: "stream.ready", workspace_id: "w", projection_version: 1, cursor: "c", max_frame_bytes: 65536 });
  state = { ...state, snapshotReady: true };
  const frame = (sequence: number, extra: Record<string, unknown> = {}) => ({
    type: "turn.activity", workspace_id: "w", conversation_id: "cv", turn_id: "t", retry_ordinal: 0, sequence,
    kind: "TOOL", status: "STARTED", call_ref: "c1", label: "bash", ...extra,
  });
  const scope = { workspaceId: "w", conversationId: "cv" };
  state = reducePresentationFrame(state, frame(1), scope);
  state = reducePresentationFrame(state, frame(2, { status: "SUCCEEDED" }), scope);
  const afterDuplicate = reducePresentationFrame(state, frame(2), scope);
  assert.equal(afterDuplicate, state, "a repeated sequence is ignored");
  assert.equal(Object.values(state.activities)[0]!.entries.length, 2);
  assert.equal(reducePresentationFrame(state, frame(3, { workspace_id: "other" }), scope), state);
  assert.equal(reducePresentationFrame(state, frame(3, { kind: "EFFECT" }), scope), state);
  assert.equal(reducePresentationFrame(state, frame(3, { extra: 1 }), scope), state, "unknown keys are refused");
});
