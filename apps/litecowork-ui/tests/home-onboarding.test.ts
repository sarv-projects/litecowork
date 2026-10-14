import test from "node:test";
import assert from "node:assert/strict";
import {
  canAutoSelectCreatedWorkspace,
  canRestoreInitialWorkspaceSelection,
  nextWorkspaceCreateRequest,
  WorkspaceListRequestFence,
  WorkspaceCreateGuard,
} from "../src/workspaces/workspace-onboarding.ts";

test("Workspace creation trims the first request and reuses its idempotency key after an ambiguous failure", () => {
  const first = nextWorkspaceCreateRequest("  Home  ", null, () => "request-1");
  assert.deepEqual(first, { name: "Home", requestId: "request-1" });
  assert.deepEqual(nextWorkspaceCreateRequest("Home", first, () => "request-2"), first);
  assert.equal(nextWorkspaceCreateRequest("Different", first, () => "request-3"), null);
  assert.equal(nextWorkspaceCreateRequest("   ", null, () => "request-4"), null);
});

test("Workspace creation guard rejects a synchronous duplicate before React can rerender", () => {
  const guard = new WorkspaceCreateGuard();
  assert.equal(guard.tryAcquire(), true);
  assert.equal(guard.tryAcquire(), false);
  guard.release();
  assert.equal(guard.tryAcquire(), true);
  guard.release();
});

test("late Workspace creation does not take selection from a newer user choice", () => {
  assert.equal(canAutoSelectCreatedWorkspace(4, 4), true);
  assert.equal(canAutoSelectCreatedWorkspace(4, 5), false);
  assert.equal(canAutoSelectCreatedWorkspace(4, 6), false);
});

test("initial Workspace restoration stops after the user has changed selection", () => {
  assert.equal(canRestoreInitialWorkspaceSelection(0), true);
  assert.equal(canRestoreInitialWorkspaceSelection(1), false);
  assert.equal(canRestoreInitialWorkspaceSelection(2), false);
});

test("a Workspace list response started before creation cannot erase the committed Workspace", () => {
  const fence = new WorkspaceListRequestFence();
  const staleListGeneration = fence.begin();
  const created = { workspaceId: "workspace-created", name: "Created Workspace" };
  let workspaces = [{ workspaceId: "workspace-old", name: "Old Workspace" }];

  fence.commitCreated(created, update => { workspaces = update(workspaces); });
  if (fence.isCurrent(staleListGeneration)) {
    workspaces = [{ workspaceId: "workspace-old", name: "Old Workspace" }];
  }

  assert.deepEqual(workspaces, [
    { workspaceId: "workspace-old", name: "Old Workspace" },
    { workspaceId: "workspace-created", name: "Created Workspace" },
  ]);
  assert.equal(fence.isCurrent(staleListGeneration), false);
});
