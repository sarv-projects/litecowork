import assert from "node:assert/strict";
import { test } from "node:test";
import { CoworkerRosterRequestEpoch } from "../src/coworkers/coworker-roster-request-epoch.ts";
import { appendCoworkerRosterPage } from "../src/coworkers/coworker-roster-request-epoch.ts";
import { validateQuickCoworkerIdentity } from "../src/coworkers/coworker-roster-request-epoch.ts";

test("ignores a Coworker page response after switching Workspace", () => {
  const epochs = new CoworkerRosterRequestEpoch();
  const oldWorkspaceRequest = epochs.begin("workspace-a");
  const newWorkspaceRequest = epochs.begin("workspace-b");

  assert.equal(epochs.isCurrent(oldWorkspaceRequest), false);
  assert.equal(epochs.isCurrent(newWorkspaceRequest), true);
});

test("invalidates in-flight Coworker pages after a roster refresh", () => {
  const epochs = new CoworkerRosterRequestEpoch();
  const pageRequest = epochs.begin("workspace-a");

  epochs.begin("workspace-a");

  assert.equal(epochs.isCurrent(pageRequest), false);
});

test("an old request finally cannot clear a newer Workspace operation", () => {
  const epochs = new CoworkerRosterRequestEpoch();
  const oldOperation = epochs.begin("workspace-a");
  const newOperation = epochs.begin("workspace-b");

  // Component finally handlers use this check before clearing the shared
  // operation indicator. A response from A must not clear B's pending state.
  assert.equal(epochs.isCurrent(oldOperation), false);
  assert.equal(epochs.isCurrent(newOperation), true);
});

test("does not accept a response for a different Workspace even if a token is reused", () => {
  const epochs = new CoworkerRosterRequestEpoch();
  const request = epochs.begin("workspace-a");

  assert.equal(epochs.isCurrent({ ...request, workspaceId: "workspace-b" }), false);
});

test("appends only same-Workspace Coworkers and removes duplicate IDs", () => {
  const first = { coworker_id: "cw-1", workspace_id: "workspace-a" } as never;
  const result = appendCoworkerRosterPage([first], {
    items: [first, { coworker_id: "cw-2", workspace_id: "workspace-a" } as never],
    next_cursor: "cursor-2",
  }, "workspace-a", "cursor-1");

  assert.deepEqual(result.items.map(item => item.coworker_id), ["cw-1", "cw-2"]);
  assert.equal(result.nextCursor, "cursor-2");
});

test("rejects a Coworker page with a foreign Workspace or a non-advancing cursor", () => {
  assert.throws(() => appendCoworkerRosterPage([], {
    items: [{ coworker_id: "cw-foreign", workspace_id: "workspace-b" } as never], next_cursor: null,
  }, "workspace-a", "cursor-1"), /different Workspace/);
  assert.throws(() => appendCoworkerRosterPage([], {
    items: [], next_cursor: "cursor-1",
  }, "workspace-a", "cursor-1"), /did not advance/);
});

test("Quick Create requires both a name and a plain-language purpose", () => {
  assert.equal(validateQuickCoworkerIdentity("Research Assistant", "Help compare reliable sources."), null);
  assert.equal(validateQuickCoworkerIdentity("  ", "Help with research."), "Enter a name between 1 and 120 characters.");
  assert.equal(validateQuickCoworkerIdentity("Research Assistant", "  "), "Add a short description of what this Coworker should help with.");
  assert.equal(validateQuickCoworkerIdentity("a".repeat(121), "Help with research."), "Enter a name between 1 and 120 characters.");
});

test("ignores a late detail or presence response after selecting another Coworker", async () => {
  const { isCoworkerDetailRequestCurrent } = await import("../src/coworkers/coworker-roster-request-epoch.ts");
  assert.equal(isCoworkerDetailRequestCurrent("workspace-a", "workspace-a", "cw-1", "cw-1"), true);
  assert.equal(isCoworkerDetailRequestCurrent("workspace-a", "workspace-b", "cw-1", "cw-1"), false);
  assert.equal(isCoworkerDetailRequestCurrent("workspace-a", "workspace-a", "cw-1", "cw-2"), false);
});
