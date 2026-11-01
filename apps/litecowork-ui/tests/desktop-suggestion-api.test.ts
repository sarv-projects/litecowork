import assert from "node:assert/strict";
import test from "node:test";
import { maybeDesktopSuggestionApi } from "../src/suggestions/desktop-suggestion-api.ts";

test("an unselected Workspace leaves the Ideas API unavailable without crashing app startup", () => {
  assert.equal(maybeDesktopSuggestionApi(""), null);
  assert.equal(maybeDesktopSuggestionApi("   "), null);
});

test("a selected Workspace receives a scoped Suggestions API", () => {
  assert.equal(maybeDesktopSuggestionApi("workspace-1")?.workspaceId, "workspace-1");
});
