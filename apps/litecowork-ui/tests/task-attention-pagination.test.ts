import test from "node:test";
import assert from "node:assert/strict";
import { taskAttentionLoadReducer, type TaskAttentionLoadState } from "../src/needs-you/task-attention-state.ts";

const initial: TaskAttentionLoadState = { generation: 0, loadingMore: false };

test("refresh or Workspace invalidation releases the old pagination busy state", () => {
  const loading = taskAttentionLoadReducer(initial, { type: "start-more", generation: 0 });
  assert.deepEqual(loading, { generation: 0, loadingMore: true });

  const refreshed = taskAttentionLoadReducer(loading, { type: "invalidate", generation: 1 });
  assert.deepEqual(refreshed, { generation: 1, loadingMore: false });
});

test("a stale page completion cannot clear the busy state of a newer request", () => {
  const refreshed = taskAttentionLoadReducer(
    { generation: 4, loadingMore: true },
    { type: "invalidate", generation: 5 },
  );
  const newerRequest = taskAttentionLoadReducer(refreshed, { type: "start-more", generation: 5 });

  assert.deepEqual(
    taskAttentionLoadReducer(newerRequest, { type: "finish-more", generation: 4 }),
    { generation: 5, loadingMore: true },
  );
  assert.deepEqual(
    taskAttentionLoadReducer(newerRequest, { type: "finish-more", generation: 5 }),
    { generation: 5, loadingMore: false },
  );
});
