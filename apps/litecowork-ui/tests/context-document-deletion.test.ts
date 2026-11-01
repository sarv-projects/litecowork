import assert from "node:assert/strict";
import test from "node:test";
import {
  contextDocumentDeletionNeedsPolling,
  contextDocumentDeletionProgress,
  contextDocumentDeletionRejectionCode,
  contextDocumentDeletionReducer,
  restoreContextDocumentDeletionFocus,
  isWorkspaceNoteOwnedBySelectedWorkspace,
  decodeContextDocumentDeletionStatus,
  isDefinitiveContextDocumentDeletionRejection,
  newContextDocumentDeletionState,
  type ContextDocumentDeletionStatus,
} from "../src/resources/context-document-deletion.ts";

const digest = `sha256:${"a".repeat(64)}`;
const pendingStatus = {
  resourceId: "resource-a",
  status: "DELETION_PENDING",
  manifestDigest: digest,
  requiredReplicaCount: 2,
  acknowledgedReplicaCount: 1,
  updatedAt: "2026-10-15T12:00:00.000Z",
};
const scope = { workspaceId: "workspace-a", resourceId: "resource-a", generation: 3 };

test("local deletion is offered only for Workspace Notes owned by the selected Workspace", () => {
  assert.equal(isWorkspaceNoteOwnedBySelectedWorkspace({
    kind: "WORKSPACE_NOTES",
    owner_ref: { kind: "WORKSPACE", workspace_id: "workspace-a" },
  }, "workspace-a"), true);
  assert.equal(isWorkspaceNoteOwnedBySelectedWorkspace({
    kind: "WORKSPACE_NOTES",
    owner_ref: { kind: "WORKSPACE", workspace_id: "workspace-b" },
  }, "workspace-a"), false);
  assert.equal(isWorkspaceNoteOwnedBySelectedWorkspace({
    kind: "COWORKER_NOTES",
    owner_ref: { kind: "COWORKER", workspace_id: "workspace-a", coworker_id: "coworker-a" },
  }, "workspace-a"), false);
  assert.equal(isWorkspaceNoteOwnedBySelectedWorkspace(null, "workspace-a"), false);
});

test("deletion status accepts the exact safe projection and rejects mismatched, private, or impossible values", () => {
  assert.deepEqual(decodeContextDocumentDeletionStatus(pendingStatus, "resource-a"), pendingStatus);
  assert.throws(() => decodeContextDocumentDeletionStatus({ ...pendingStatus, resourceId: "resource-b" }, "resource-a"));
  assert.throws(() => decodeContextDocumentDeletionStatus({ ...pendingStatus, storageLocator: "/private/path" }, "resource-a"));
  assert.throws(() => decodeContextDocumentDeletionStatus({ ...pendingStatus, acknowledgedReplicaCount: 3 }, "resource-a"));
  assert.throws(() => decodeContextDocumentDeletionStatus({ ...pendingStatus, manifestDigest: "private-object-key" }, "resource-a"));
});

test("an ambiguous deletion admission retains the exact idempotency request for retry until status confirms admission", () => {
  const request = { ...scope, expectedVersion: 7, requestId: "request-stable-1" };
  let state = newContextDocumentDeletionState(scope);
  state = contextDocumentDeletionReducer(state, { type: "begin", request });
  state = contextDocumentDeletionReducer(state, { type: "postStarted", request });
  state = contextDocumentDeletionReducer(state, { type: "postFailed", request, error: "Response was lost." });

  assert.equal(state.request?.requestId, "request-stable-1");
  assert.equal(state.request?.expectedVersion, 7);
  assert.equal(state.busy, false);
  assert.equal(state.error, "Response was lost.");

  state = contextDocumentDeletionReducer(state, { type: "postStarted", request });
  assert.equal(state.request?.requestId, "request-stable-1");
  state = contextDocumentDeletionReducer(state, { type: "postAccepted", request, status: pendingStatus });
  assert.equal(state.request, null);
  assert.equal(state.status?.status, "DELETION_PENDING");
  assert.equal(state.error, null);
});

test("a definitive conflict releases the obsolete retry so the refreshed Resource can start a new request", () => {
  const request = { ...scope, expectedVersion: 7, requestId: "request-conflict" };
  let state = newContextDocumentDeletionState(scope);
  state = contextDocumentDeletionReducer(state, { type: "begin", request });
  state = contextDocumentDeletionReducer(state, { type: "postRejected", request, error: "RESOURCE_CONFLICT: Resource version changed." });

  assert.equal(isDefinitiveContextDocumentDeletionRejection(state.error ?? ""), true);
  assert.equal(state.request, null);
  assert.equal(state.error, "RESOURCE_CONFLICT: Resource version changed.");
  assert.equal(isDefinitiveContextDocumentDeletionRejection("DEPENDENCY_UNAVAILABLE: Runtime is offline."), false);
});

test("classifies the stable Operator error suffix while leaving retryable outcomes ambiguous", () => {
  const conflict = "Resource changed [RESOURCE_CONFLICT; retryable=false; reference=cor_123]";
  assert.equal(contextDocumentDeletionRejectionCode(conflict), "RESOURCE_CONFLICT");
  assert.equal(contextDocumentDeletionRejectionCode("Archived [WORKSPACE_ARCHIVED; retryable=false; reference=cor_124]"), "WORKSPACE_ARCHIVED");
  assert.equal(contextDocumentDeletionRejectionCode("Resource changed [RESOURCE_CONFLICT; retryable=true; reference=cor_125]"), null);
  assert.equal(contextDocumentDeletionRejectionCode("Runtime is unavailable [DEPENDENCY_UNAVAILABLE; retryable=true; reference=cor_126]"), null);
  assert.equal(isDefinitiveContextDocumentDeletionRejection(conflict), true);
});

test("returns focus to the deletion panel when the prior trigger is disabled or cannot receive focus", () => {
  const calls: string[] = [];
  const panel = { focus: () => calls.push("panel") };
  const disabledTrigger = { isConnected: true, disabled: true, focus: () => calls.push("trigger") };

  restoreContextDocumentDeletionFocus(disabledTrigger, panel, () => false);
  assert.deepEqual(calls, ["panel"]);

  calls.length = 0;
  const enabledTrigger = { isConnected: true, disabled: false, focus: () => calls.push("trigger") };
  restoreContextDocumentDeletionFocus(enabledTrigger, panel, (target) => target === enabledTrigger);
  assert.deepEqual(calls, ["trigger"]);

  calls.length = 0;
  restoreContextDocumentDeletionFocus(enabledTrigger, panel, () => false);
  assert.deepEqual(calls, ["trigger", "panel"]);
});

test("status recovery clears an ambiguous retry only when the durable projection says pending or deleted", () => {
  const request = { ...scope, expectedVersion: 7, requestId: "request-stable-2" };
  let state = newContextDocumentDeletionState(scope);
  state = contextDocumentDeletionReducer(state, { type: "begin", request });
  state = contextDocumentDeletionReducer(state, { type: "postFailed", request, error: "Response was lost." });
  state = contextDocumentDeletionReducer(state, { type: "statusStarted", scope, sequence: 1 });
  state = contextDocumentDeletionReducer(state, { type: "statusReceived", scope, sequence: 1, status: pendingStatus });

  assert.equal(state.request, null);
  assert.equal(state.status?.status, "DELETION_PENDING");
  assert.equal(state.checkingStatus, false);
  assert.equal(state.error, null);

  const deleted = { ...pendingStatus, status: "DELETED", acknowledgedReplicaCount: 2 } as const;
  state = contextDocumentDeletionReducer(state, { type: "statusStarted", scope, sequence: 2 });
  state = contextDocumentDeletionReducer(state, { type: "statusReceived", scope, sequence: 2, status: deleted });
  assert.equal(state.status?.status, "DELETED");
  assert.equal(contextDocumentDeletionNeedsPolling(state.status), false);
});

test("an older overlapping status read cannot overwrite a newer terminal response", () => {
  const deleted = { ...pendingStatus, status: "DELETED", acknowledgedReplicaCount: 2 } as const;
  let state = newContextDocumentDeletionState(scope);
  state = contextDocumentDeletionReducer(state, { type: "statusStarted", scope, sequence: 1 });
  state = contextDocumentDeletionReducer(state, { type: "statusStarted", scope, sequence: 2 });
  state = contextDocumentDeletionReducer(state, { type: "statusReceived", scope, sequence: 2, status: deleted });
  state = contextDocumentDeletionReducer(state, { type: "statusReceived", scope, sequence: 1, status: pendingStatus });
  state = contextDocumentDeletionReducer(state, { type: "statusStarted", scope, sequence: 3 });
  state = contextDocumentDeletionReducer(state, { type: "statusReceived", scope, sequence: 3, status: pendingStatus });

  assert.equal(state.status?.status, "DELETED");
  assert.equal(state.checkingStatus, false);
});

test("deletion status cannot regress or change its sealed manifest after admission", () => {
  let state = newContextDocumentDeletionState(scope);
  const receive = (sequence: number, status: ContextDocumentDeletionStatus) => {
    state = contextDocumentDeletionReducer(state, { type: "statusStarted", scope, sequence });
    state = contextDocumentDeletionReducer(state, { type: "statusReceived", scope, sequence, status });
  };

  const revoked: ContextDocumentDeletionStatus = {
    ...pendingStatus,
    status: "REVOKED",
    manifestDigest: null,
    requiredReplicaCount: 0,
    acknowledgedReplicaCount: 0,
  };
  receive(1, revoked);
  receive(2, { ...revoked, status: "ACTIVE" });
  assert.equal(state.status?.status, "REVOKED");

  receive(3, pendingStatus as ContextDocumentDeletionStatus);
  receive(4, { ...pendingStatus, manifestDigest: `sha256:${"b".repeat(64)}`, acknowledgedReplicaCount: 2 } as ContextDocumentDeletionStatus);
  assert.deepEqual(state.status, pendingStatus);
  receive(5, { ...pendingStatus, requiredReplicaCount: 3, acknowledgedReplicaCount: 2 } as ContextDocumentDeletionStatus);
  assert.deepEqual(state.status, pendingStatus);

  const deleted: ContextDocumentDeletionStatus = { ...pendingStatus, status: "DELETED", acknowledgedReplicaCount: 2 };
  receive(6, deleted);
  assert.deepEqual(state.status, deleted);
  receive(7, pendingStatus as ContextDocumentDeletionStatus);
  assert.deepEqual(state.status, deleted);
});

test("a late response from an earlier A selection cannot overwrite the current A after an A to B to A switch", () => {
  const oldRequest = { ...scope, expectedVersion: 7, requestId: "request-stale" };
  let state = newContextDocumentDeletionState(scope);
  state = contextDocumentDeletionReducer(state, {
    type: "select",
    scope: { workspaceId: "workspace-b", resourceId: "resource-b", generation: 4 },
  });
  state = contextDocumentDeletionReducer(state, { type: "statusReceived", scope, sequence: 1, status: pendingStatus });

  assert.equal(state.scope.workspaceId, "workspace-b");
  assert.equal(state.status, null);
  assert.equal(state.request, null);

  state = contextDocumentDeletionReducer(state, {
    type: "select",
    scope: { workspaceId: "workspace-a", resourceId: "resource-a", generation: 5 },
  });
  const currentRequest = { ...state.scope, expectedVersion: 8, requestId: "request-current" };
  state = contextDocumentDeletionReducer(state, { type: "begin", request: currentRequest });
  state = contextDocumentDeletionReducer(state, { type: "postAccepted", request: oldRequest, status: pendingStatus });
  assert.equal(state.status, null);
  assert.equal(state.request?.requestId, "request-current");
});

test("pending purge progress remains pollable, including a verified empty plan", () => {
  assert.equal(contextDocumentDeletionNeedsPolling(pendingStatus), true);
  assert.equal(contextDocumentDeletionProgress(pendingStatus), "Deletion is pending. 1 of 2 deletion targets are acknowledged.");
  assert.equal(contextDocumentDeletionNeedsPolling({ ...pendingStatus, requiredReplicaCount: 0, acknowledgedReplicaCount: 0 }), true);
  assert.equal(contextDocumentDeletionProgress({ ...pendingStatus, requiredReplicaCount: 0, acknowledgedReplicaCount: 0 }), "Deletion is pending while LiteCowork verifies that no local content remains.");
  assert.equal(contextDocumentDeletionNeedsPolling({ ...pendingStatus, status: "DELETED", acknowledgedReplicaCount: 2 }), false);
});
