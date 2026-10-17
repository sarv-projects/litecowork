import assert from "node:assert/strict";
import { test } from "node:test";
import { isGoalRequestCurrent } from "../src/goals/goal-request-scope.ts";

test("a late Goal result cannot replace a different Workspace view", () => {
  assert.equal(isGoalRequestCurrent("workspace-a", "workspace-a"), true);
  assert.equal(isGoalRequestCurrent("workspace-a", "workspace-b"), false);
  assert.equal(isGoalRequestCurrent("workspace-a", ""), false);
});

test("a catalog response is accepted only for the still-selected Workspace and its returned owner", async () => {
  const { isGoalWorkspaceResponseCurrent } = await import("../src/goals/goal-request-scope.ts");
  assert.equal(isGoalWorkspaceResponseCurrent("workspace-a", "workspace-a", ["workspace-a"]), true);
  assert.equal(isGoalWorkspaceResponseCurrent("workspace-a", "workspace-b", ["workspace-a"]), false);
  assert.equal(isGoalWorkspaceResponseCurrent("workspace-a", "workspace-a", ["workspace-b"]), false);
});
