import assert from "node:assert/strict";
import { test } from "node:test";
import { isSuggestionRequestCurrent } from "../src/suggestions/suggestion-request-scope.ts";

test("a late suggestion result cannot navigate or announce in a different Workspace", () => {
  assert.equal(isSuggestionRequestCurrent("workspace-a", "workspace-a"), true);
  assert.equal(isSuggestionRequestCurrent("workspace-a", "workspace-b"), false);
  assert.equal(isSuggestionRequestCurrent("workspace-a", ""), false);
});

test("a page response is accepted only for the still-selected Workspace and its returned owner", async () => {
  const { isSuggestionWorkspaceResponseCurrent } = await import("../src/suggestions/suggestion-request-scope.ts");
  assert.equal(isSuggestionWorkspaceResponseCurrent("workspace-a", "workspace-a", ["workspace-a"]), true);
  assert.equal(isSuggestionWorkspaceResponseCurrent("workspace-a", "workspace-b", ["workspace-a"]), false);
  assert.equal(isSuggestionWorkspaceResponseCurrent("workspace-a", "workspace-a", ["workspace-b"]), false);
});
