import assert from "node:assert/strict";
import test from "node:test";
import {
  WorkspaceInstructionDraftCache,
  canApplyWorkspaceInstructionResponse,
  getOrCreateInstructionSave,
  instructionSaveFailureKind,
  instructionSaveRefreshKind,
  workspaceInstructionByteLength,
} from "../src/workspaces/workspace-instructions.ts";

test("Workspace switches preserve each unsaved instruction draft independently", () => {
  const drafts = new WorkspaceInstructionDraftCache();
  drafts.set("workspace-a", "A's unsaved instructions");
  drafts.set("workspace-b", "B's unsaved instructions");

  assert.equal(drafts.get("workspace-a"), "A's unsaved instructions");
  assert.equal(drafts.get("workspace-b"), "B's unsaved instructions");
  drafts.delete("workspace-a");
  assert.equal(drafts.get("workspace-a"), undefined);
  assert.equal(drafts.get("workspace-b"), "B's unsaved instructions");
});

test("late Workspace instruction responses apply only to the selected Workspace generation", () => {
  assert.equal(canApplyWorkspaceInstructionResponse(
    { workspaceId: "workspace-a", generation: 4 },
    { workspaceId: "workspace-a", generation: 4 },
  ), true);
  assert.equal(canApplyWorkspaceInstructionResponse(
    { workspaceId: "workspace-a", generation: 4 },
    { workspaceId: "workspace-b", generation: 5 },
  ), false);
  assert.equal(canApplyWorkspaceInstructionResponse(
    { workspaceId: "workspace-a", generation: 4 },
    { workspaceId: "workspace-a", generation: 5 },
  ), false);
});

test("unchanged instruction retries retain both idempotency keys and pinned version", () => {
  const first = getOrCreateInstructionSave(null, {
    workspaceId: "workspace-a",
    text: "Draft text",
    expectedVersion: 7,
    parentRevisions: [3],
  }, (() => {
    let index = 0;
    return () => ["request-1", "resource-1"][index++];
  })());
  const retry = getOrCreateInstructionSave(first, {
    workspaceId: "workspace-a",
    text: "Draft text",
    expectedVersion: 8,
    parentRevisions: [4],
  }, () => "unexpected-new-id");

  assert.equal(retry.requestId, "request-1");
  assert.equal(retry.resourceRequestId, "resource-1");
  assert.equal(retry.expectedVersion, 7);
  assert.deepEqual(retry.parentRevisions, [3]);
});

test("changed instruction drafts receive fresh identities and current concurrency pins", () => {
  const first = getOrCreateInstructionSave(null, {
    workspaceId: "workspace-a",
    text: "Draft text",
    expectedVersion: 7,
    parentRevisions: [3],
  }, () => "first-id");
  const changed = getOrCreateInstructionSave(first, {
    workspaceId: "workspace-a",
    text: "Changed draft",
    expectedVersion: 9,
    parentRevisions: [5],
  }, (() => {
    let index = 0;
    return () => ["request-2", "resource-2"][index++];
  })());

  assert.equal(changed.requestId, "request-2");
  assert.equal(changed.resourceRequestId, "resource-2");
  assert.equal(changed.expectedVersion, 9);
  assert.deepEqual(changed.parentRevisions, [5]);
});

test("failed instruction commit after Resource import is classified as a disclosed partial save", () => {
  assert.equal(instructionSaveFailureKind({ stale: false, resourceImported: true }), "RESOURCE_IMPORTED");
  assert.equal(instructionSaveFailureKind({ stale: true, resourceImported: true }), "STALE_AFTER_RESOURCE_IMPORT");
  assert.equal(instructionSaveFailureKind({ stale: false, resourceImported: false }), "RETRY_SAME_REQUEST");
});

test("history refresh failure after commit remains a committed save with refresh pending", () => {
  assert.equal(instructionSaveRefreshKind({ workspace: true, history: true }), "CURRENT");
  assert.equal(instructionSaveRefreshKind({ workspace: false, history: true }), "REFRESH_PENDING");
  assert.equal(instructionSaveRefreshKind({ workspace: true, history: false }), "REFRESH_PENDING");
});

test("Workspace instruction length uses UTF-8 bytes at the exact 64 KiB boundary", () => {
  assert.equal(workspaceInstructionByteLength("😀".repeat(16_384)), 65_536);
  assert.equal(workspaceInstructionByteLength("😀".repeat(16_385)), 65_540);
});
