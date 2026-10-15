import assert from "node:assert/strict";
import test from "node:test";
import {
  clearWorkspacePolicyUpdateRequest,
  isWorkspacePolicyUpdateResponse,
  mergeWorkspaceListProjection,
  mergeWorkspacePolicyProjection,
  reconcileWorkspacePolicyUpdateRequests,
  workspacePolicyRefreshOutcome,
  workspacePolicyUpdateRequest,
  type PendingWorkspacePolicyUpdate,
  type WorkspacePolicyProjection,
} from "../src/workspaces/workspace-policy.ts";

function projection(workspaceId: string, version: number, replicationPolicy: string): WorkspacePolicyProjection {
  return { workspaceId, version, replicationPolicy };
}

test("ambiguous retry reuses the original Workspace, version, policy, and request ID", () => {
  const pending = new Map<string, PendingWorkspacePolicyUpdate>();
  let created = 0;
  const intent = { workspaceId: "workspace-a", expectedVersion: 4, replicationPolicy: "LOCAL_ONLY" };
  const first = workspacePolicyUpdateRequest(pending, intent, () => `request-${++created}`);
  const retry = workspacePolicyUpdateRequest(pending, intent, () => `request-${++created}`);

  assert.deepEqual(retry, first);
  assert.equal(created, 1);
  assert.equal(pending.size, 1);
});

test("another Workspace remains isolated and a changed version or policy starts a new intent", () => {
  const pending = new Map<string, PendingWorkspacePolicyUpdate>();
  let created = 0;
  const makeId = () => `request-${++created}`;
  const first = workspacePolicyUpdateRequest(pending, {
    workspaceId: "workspace-a", expectedVersion: 4, replicationPolicy: "LOCAL_ONLY",
  }, makeId);
  const otherWorkspace = workspacePolicyUpdateRequest(pending, {
    workspaceId: "workspace-b", expectedVersion: 9, replicationPolicy: "LOCAL_ONLY",
  }, makeId);
  const newVersion = workspacePolicyUpdateRequest(pending, {
    workspaceId: "workspace-a", expectedVersion: 5, replicationPolicy: "LOCAL_ONLY",
  }, makeId);

  assert.equal(first.requestId, "request-1");
  assert.equal(otherWorkspace.requestId, "request-2");
  assert.equal(newVersion.requestId, "request-3");
  assert.deepEqual([...pending.values()], [otherWorkspace, newVersion]);
});

test("a changed payload at the same version gets a new request identity", () => {
  const pending = new Map<string, PendingWorkspacePolicyUpdate>();
  let created = 0;
  const first = workspacePolicyUpdateRequest(pending, {
    workspaceId: "workspace-a", expectedVersion: 4, replicationPolicy: "LOCAL_ONLY",
  }, () => `request-${++created}`);
  const changed = workspacePolicyUpdateRequest(pending, {
    workspaceId: "workspace-a", expectedVersion: 4, replicationPolicy: "METADATA_ONLY",
  }, () => `request-${++created}`);

  assert.notEqual(changed.requestId, first.requestId);
  assert.equal(pending.size, 1);
});

test("list refresh retires only requests whose Workspace version has advanced", () => {
  const pending = new Map<string, PendingWorkspacePolicyUpdate>();
  const makeId = (() => { let id = 0; return () => `request-${++id}`; })();
  const stale = workspacePolicyUpdateRequest(pending, {
    workspaceId: "workspace-a", expectedVersion: 4, replicationPolicy: "LOCAL_ONLY",
  }, makeId);
  const stillCurrent = workspacePolicyUpdateRequest(pending, {
    workspaceId: "workspace-b", expectedVersion: 9, replicationPolicy: "LOCAL_ONLY",
  }, makeId);

  reconcileWorkspacePolicyUpdateRequests(pending, [projection("workspace-a", 5, "LOCAL_ONLY")]);
  assert.equal(pending.has(JSON.stringify([stale.workspaceId, stale.expectedVersion, stale.replicationPolicy])), false);
  assert.equal(pending.has(JSON.stringify([stillCurrent.workspaceId, stillCurrent.expectedVersion, stillCurrent.replicationPolicy])), true);
});

test("only a matching next-version policy response confirms the pending request", () => {
  const request: PendingWorkspacePolicyUpdate = {
    workspaceId: "workspace-a", expectedVersion: 4, replicationPolicy: "LOCAL_ONLY", requestId: "request-1",
  };
  assert.equal(isWorkspacePolicyUpdateResponse(projection("workspace-a", 5, "LOCAL_ONLY"), request), true);
  assert.equal(isWorkspacePolicyUpdateResponse(projection("workspace-b", 5, "LOCAL_ONLY"), request), false);
  assert.equal(isWorkspacePolicyUpdateResponse(projection("workspace-a", 6, "LOCAL_ONLY"), request), false);
  assert.equal(isWorkspacePolicyUpdateResponse(projection("workspace-a", 5, "METADATA_ONLY"), request), false);

  const pending = new Map<string, PendingWorkspacePolicyUpdate>();
  const stored = workspacePolicyUpdateRequest(pending, request, () => "unused");
  clearWorkspacePolicyUpdateRequest(pending, { ...stored, requestId: "different-request" });
  assert.equal(pending.size, 1);
  clearWorkspacePolicyUpdateRequest(pending, stored);
  assert.equal(pending.size, 0);
});

test("a late policy response cannot replace a newer Workspace projection", () => {
  const v2 = { workspaceId: "workspace-a", version: 2, replicationPolicy: "LOCAL_ONLY", name: "A" };
  const v3 = { ...v2, version: 3, replicationPolicy: "METADATA_ONLY" };
  assert.deepEqual(mergeWorkspacePolicyProjection([v3], v2), [v3]);
  assert.deepEqual(mergeWorkspacePolicyProjection([{ ...v2, version: 1 }], v2), [v2]);
});

test("a stale Workspace list cannot roll back a newer version and still reflects membership changes", () => {
  const current = [
    { workspaceId: "workspace-a", version: 5, replicationPolicy: "LOCAL_ONLY", name: "A" },
    { workspaceId: "workspace-b", version: 1, replicationPolicy: "METADATA_ONLY", name: "B" },
  ];
  const refreshed = [
    { workspaceId: "workspace-a", version: 4, replicationPolicy: "METADATA_ONLY", name: "A" },
    { workspaceId: "workspace-c", version: 1, replicationPolicy: "LOCAL_ONLY", name: "C" },
  ];

  assert.deepEqual(mergeWorkspaceListProjection(current, refreshed), [current[0], refreshed[1]]);
});

test("lost acknowledgement resolution uses the effective versioned Workspace projection", () => {
  const request = {
    workspaceId: "workspace-a", expectedVersion: 4, replicationPolicy: "LOCAL_ONLY", requestId: "request-1",
  };
  const committed = projection("workspace-a", 5, "LOCAL_ONLY");
  const staleList = projection("workspace-a", 4, "METADATA_ONLY");
  const effective = mergeWorkspaceListProjection([committed], [staleList])[0];

  assert.equal(workspacePolicyRefreshOutcome(effective, request), "CONFIRMED");
  assert.equal(workspacePolicyRefreshOutcome(projection("workspace-a", 6, "METADATA_ONLY"), request), "SUPERSEDED");
  assert.equal(workspacePolicyRefreshOutcome(staleList, request), "UNCHANGED");
  assert.equal(workspacePolicyRefreshOutcome(undefined, request), "MISSING");
});
