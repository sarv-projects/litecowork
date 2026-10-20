import assert from "node:assert/strict";
import test from "node:test";
import {
  clearPendingApprovalDecision,
  getOrCreatePendingApprovalDecision,
  isApprovalDetailFor,
  type ApprovalDetailView,
} from "../src/needs-you/approval-state.ts";

const digestA = `sha256:${"a".repeat(64)}`;
const digestB = `sha256:${"b".repeat(64)}`;

function detail(overrides: Partial<ApprovalDetailView> = {}): ApprovalDetailView {
  return {
    approvalId: "approval-1",
    taskId: "task-1",
    requestedByAttempt: null,
    kind: "EXTERNAL_MUTATION",
    actionSummary: "Publish the reviewed document",
    targetRef: { workspaceId: "workspace-1", resourceId: "resource-1", revisionId: null },
    scopeDigest: digestA,
    actionDigest: digestB,
    risk: "SENSITIVE",
    requiredAssurance: "APPROVE_SENSITIVE",
    status: "PENDING",
    requestedAt: "2026-10-16T10:00:00Z",
    expiresAt: null,
    resolvedBy: null,
    resolvedAt: null,
    version: 2,
    ...overrides,
  };
}

class MemoryStorage {
  values = new Map<string, string>();
  getItem(key: string): string | null { return this.values.get(key) ?? null; }
  setItem(key: string, value: string): void { this.values.set(key, value); }
  removeItem(key: string): void { this.values.delete(key); }
}

test("Approval detail validation binds exact Workspace, ID, target, status, and bounded digests", () => {
  assert.equal(isApprovalDetailFor("workspace-1", "approval-1", detail()), true);
  assert.equal(isApprovalDetailFor("workspace-other", "approval-1", detail()), false);
  assert.equal(isApprovalDetailFor("workspace-1", "approval-other", detail()), false);
  assert.equal(isApprovalDetailFor("workspace-1", "approval-1", { ...detail(), actionDigest: "sha256:bad" }), false);
  assert.equal(isApprovalDetailFor("workspace-1", "approval-1", { ...detail(), status: "WAITING" }), false);
  assert.equal(isApprovalDetailFor("workspace-1", "approval-1", { ...detail(), targetRef: { ...detail().targetRef, revisionId: "" } }), false);
  assert.equal(isApprovalDetailFor("workspace-1", "approval-1", { ...detail(), extra: "untrusted" }), false);
});

test("an ambiguous retry reuses its original idempotency request and cannot switch decisions", () => {
  const storage = new MemoryStorage();
  const approval = detail();
  const requestId = "approval-ui-00000000-0000-4000-8000-000000000000";
  const first = getOrCreatePendingApprovalDecision(storage, "workspace-1", approval, "APPROVE", () => requestId);
  const retry = getOrCreatePendingApprovalDecision(storage, "workspace-1", approval, "APPROVE", () => "approval-ui-11111111-1111-4111-8111-111111111111");
  assert.deepEqual(retry, first);
  assert.throws(() => getOrCreatePendingApprovalDecision(storage, "workspace-1", approval, "DENY"), /previous approval decision/);
  assert.deepEqual(getOrCreatePendingApprovalDecision(storage, "workspace-1", detail({ version: 3 }), "DENY", () => "approval-ui-22222222-2222-4222-8222-222222222222"), {
    workspaceId: "workspace-1",
    approvalId: "approval-1",
    expectedVersion: 3,
    decision: "DENY",
    requestId: "approval-ui-22222222-2222-4222-8222-222222222222",
  });
});

test("terminal approval clears the scoped retry record", () => {
  const storage = new MemoryStorage();
  const approval = detail();
  getOrCreatePendingApprovalDecision(storage, "workspace-1", approval, "DENY", () => "approval-ui-33333333-3333-4333-8333-333333333333");
  clearPendingApprovalDecision(storage, "workspace-1", "approval-1");
  assert.equal(storage.values.size, 0);
});
