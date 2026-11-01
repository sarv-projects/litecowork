import assert from "node:assert/strict";
import test from "node:test";
import { isCurrentWorkspaceSearchRequest } from "../src/resources/resource-search-request.ts";

test("a search response is current only for the current request and selected Workspace", () => {
  assert.equal(isCurrentWorkspaceSearchRequest(4, 4, "workspace-a", "workspace-a"), true);
  assert.equal(isCurrentWorkspaceSearchRequest(3, 4, "workspace-a", "workspace-a"), false);
  assert.equal(isCurrentWorkspaceSearchRequest(4, 4, "workspace-a", "workspace-b"), false);
});
