import assert from "node:assert/strict";
import test from "node:test";
import {
  EMPTY_NEEDS_YOU_MOTION_BASELINE,
  nextNeedsYouMotionBaseline,
  type NeedsYouMotionBaseline,
} from "../src/needs-you/task-attention-state.ts";

const SCOPE = "workspace-1|OPEN";

function baseline(scopeKey: string | null, itemIds: string[]): NeedsYouMotionBaseline {
  return { scopeKey, itemIds: new Set(itemIds) };
}

test("the first read of a scope never replays a transition", () => {
  const result = nextNeedsYouMotionBaseline(
    EMPTY_NEEDS_YOU_MOTION_BASELINE,
    SCOPE,
    ["item-1", "item-2"],
  );
  assert.deepEqual(result.entering, []);
  assert.equal(result.baseline.scopeKey, SCOPE);
  assert.deepEqual([...result.baseline.itemIds].sort(), ["item-1", "item-2"]);
});

test("a newly committed item is emphasised once and then stays static", () => {
  const first = nextNeedsYouMotionBaseline(EMPTY_NEEDS_YOU_MOTION_BASELINE, SCOPE, ["item-1"]);
  assert.deepEqual(first.entering, []);

  const second = nextNeedsYouMotionBaseline(first.baseline, SCOPE, ["item-1", "item-2"]);
  assert.deepEqual(second.entering, ["item-2"], "only the new card enters");

  const third = nextNeedsYouMotionBaseline(second.baseline, SCOPE, ["item-1", "item-2"]);
  assert.deepEqual(third.entering, [], "an unchanged refresh never re-enters");
});

test("an item the owner already saw is not emphasised again", () => {
  const first = nextNeedsYouMotionBaseline(EMPTY_NEEDS_YOU_MOTION_BASELINE, SCOPE, ["item-1"]);
  const second = nextNeedsYouMotionBaseline(first.baseline, SCOPE, ["item-1"]);
  assert.deepEqual(second.entering, []);

  // The card left the open list and came back inside the same scope.
  const third = nextNeedsYouMotionBaseline(second.baseline, SCOPE, ["item-1"]);
  assert.deepEqual(third.entering, []);
});

test("changing Workspace or status filter is a fresh projection, not new work", () => {
  const first = nextNeedsYouMotionBaseline(EMPTY_NEEDS_YOU_MOTION_BASELINE, SCOPE, ["item-1"]);
  const resolved = nextNeedsYouMotionBaseline(first.baseline, "workspace-1|RESOLVED", ["item-9"]);
  assert.deepEqual(resolved.entering, [], "historical hydration never replays a transition");

  // Returning to the previous scope is still a scope change, not a new item.
  const reopened = nextNeedsYouMotionBaseline(resolved.baseline, SCOPE, ["item-1"]);
  assert.deepEqual(reopened.entering, []);
});

test("an appended page emphasises only the rows it actually adds", () => {
  const first = nextNeedsYouMotionBaseline(EMPTY_NEEDS_YOU_MOTION_BASELINE, SCOPE, ["item-1"]);
  const appended = nextNeedsYouMotionBaseline(first.baseline, SCOPE, ["item-1", "item-2"]);
  assert.deepEqual(appended.entering, ["item-2"]);
  assert.deepEqual([...appended.baseline.itemIds].sort(), ["item-1", "item-2"]);
});

test("an empty scope change keeps the baseline usable for the next scope", () => {
  const first = nextNeedsYouMotionBaseline(EMPTY_NEEDS_YOU_MOTION_BASELINE, SCOPE, []);
  assert.deepEqual(first.entering, []);
  const second = nextNeedsYouMotionBaseline(first.baseline, SCOPE, ["item-1"]);
  assert.deepEqual(second.entering, ["item-1"]);
});
