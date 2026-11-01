export type ApprovalStatus = "PENDING" | "APPROVED" | "DENIED" | "EXPIRED" | "CANCELLED";
export type ApprovalDecision = "APPROVE" | "DENY";
export type ApprovalTargetRef = {
  workspaceId: string;
  resourceId: string;
  revisionId: string | null;
};
export type ApprovalDetailView = {
  approvalId: string;
  taskId: string;
  requestedByAttempt: string | null;
  kind: string;
  actionSummary: string;
  targetRef: ApprovalTargetRef;
  scopeDigest: string;
  actionDigest: string;
  risk: "SAFE" | "SENSITIVE" | "HIGH_IMPACT";
  requiredAssurance: string;
  status: ApprovalStatus;
  requestedAt: string;
  expiresAt: string | null;
  resolvedBy: { principalId: string; kind: string } | null;
  resolvedAt: string | null;
  version: number;
};

export type PendingApprovalDecision = {
  workspaceId: string;
  approvalId: string;
  expectedVersion: number;
  decision: ApprovalDecision;
  requestId: string;
};

const DIGEST = /^sha256:[a-f0-9]{64}$/u;
const STATUSES = new Set<ApprovalStatus>(["PENDING", "APPROVED", "DENIED", "EXPIRED", "CANCELLED"]);
const RISKS = new Set(["SAFE", "SENSITIVE", "HIGH_IMPACT"]);

function validId(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0 && value.length <= 512 && !/[\u0000-\u001f\u007f]/u.test(value);
}

function validTimestamp(value: unknown): value is string {
  return typeof value === "string" && Number.isFinite(Date.parse(value));
}

function validNullableTimestamp(value: unknown): value is string | null {
  return value === null || validTimestamp(value);
}

function validNullableId(value: unknown): value is string | null {
  return value === null || validId(value);
}

function exactKeys(value: Record<string, unknown>, expected: readonly string[]): boolean {
  const keys = Object.keys(value);
  return keys.length === expected.length && expected.every(key => Object.hasOwn(value, key));
}

export function isApprovalDetailFor(workspaceId: string, approvalId: string, value: unknown): value is ApprovalDetailView {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const item = value as Record<string, unknown>;
  const expected = ["approvalId", "taskId", "requestedByAttempt", "kind", "actionSummary", "targetRef", "scopeDigest", "actionDigest", "risk", "requiredAssurance", "status", "requestedAt", "expiresAt", "resolvedBy", "resolvedAt", "version"];
  if (!exactKeys(item, expected)) return false;
  const target = item.targetRef;
  if (!target || typeof target !== "object" || Array.isArray(target)) return false;
  const targetRef = target as Record<string, unknown>;
  if (!exactKeys(targetRef, ["workspaceId", "resourceId", "revisionId"])) return false;
  const resolvedBy = item.resolvedBy;
  const validResolvedBy = resolvedBy === null || (typeof resolvedBy === "object" && !Array.isArray(resolvedBy)
    && exactKeys(resolvedBy as Record<string, unknown>, ["principalId", "kind"])
    && validId((resolvedBy as Record<string, unknown>).principalId)
    && validId((resolvedBy as Record<string, unknown>).kind));
  return item.approvalId === approvalId
    && item.targetRef !== null
    && targetRef.workspaceId === workspaceId
    && validId(item.approvalId)
    && validId(item.taskId)
    && validNullableId(item.requestedByAttempt)
    && validId(item.kind)
    && typeof item.actionSummary === "string" && item.actionSummary.trim().length > 0 && item.actionSummary.length <= 16_384
    && validId(targetRef.workspaceId)
    && validId(targetRef.resourceId)
    && validNullableId(targetRef.revisionId)
    && typeof item.scopeDigest === "string" && DIGEST.test(item.scopeDigest)
    && typeof item.actionDigest === "string" && DIGEST.test(item.actionDigest)
    && typeof item.risk === "string" && RISKS.has(item.risk)
    && validId(item.requiredAssurance)
    && typeof item.status === "string" && STATUSES.has(item.status as ApprovalStatus)
    && validTimestamp(item.requestedAt)
    && validNullableTimestamp(item.expiresAt)
    && validResolvedBy
    && validNullableTimestamp(item.resolvedAt)
    && Number.isSafeInteger(item.version) && (item.version as number) > 0;
}

function pendingDecisionStorageKey(workspaceId: string, approvalId: string): string {
  return `litecowork:approval-decision:v1:${encodeURIComponent(workspaceId)}:${encodeURIComponent(approvalId)}`;
}

function validPendingDecision(value: unknown, workspaceId: string, approval: ApprovalDetailView): value is PendingApprovalDecision {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const pending = value as Record<string, unknown>;
  return exactKeys(pending, ["workspaceId", "approvalId", "expectedVersion", "decision", "requestId"])
    && pending.workspaceId === workspaceId
    && pending.approvalId === approval.approvalId
    && pending.expectedVersion === approval.version
    && (pending.decision === "APPROVE" || pending.decision === "DENY")
    && typeof pending.requestId === "string"
    && /^approval-ui-[0-9a-f-]{36}$/u.test(pending.requestId);
}

/**
 * Reuses the same idempotency key after an ambiguous response. A pending decision
 * for another version or opposite decision is never silently overwritten.
 */
export function getOrCreatePendingApprovalDecision(
  storage: Pick<Storage, "getItem" | "setItem" | "removeItem">,
  workspaceId: string,
  approval: ApprovalDetailView,
  decision: ApprovalDecision,
  createRequestId: () => string = () => `approval-ui-${crypto.randomUUID()}`,
): PendingApprovalDecision {
  const key = pendingDecisionStorageKey(workspaceId, approval.approvalId);
  const raw = storage.getItem(key);
  if (raw !== null) {
    let parsed: unknown;
    try { parsed = JSON.parse(raw); } catch { parsed = null; }
    if (validPendingDecision(parsed, workspaceId, approval)) {
      if (parsed.decision !== decision) throw new Error("A previous approval decision may still be awaiting confirmation. Refresh its status before choosing another decision.");
      return parsed;
    }
    storage.removeItem(key);
  }
  const requestId = createRequestId();
  if (!/^approval-ui-[0-9a-f-]{36}$/u.test(requestId)) throw new Error("A safe approval request identifier could not be created.");
  const pending = { workspaceId, approvalId: approval.approvalId, expectedVersion: approval.version, decision, requestId };
  storage.setItem(key, JSON.stringify(pending));
  return pending;
}

export function clearPendingApprovalDecision(
  storage: Pick<Storage, "removeItem">,
  workspaceId: string,
  approvalId: string,
): void {
  storage.removeItem(pendingDecisionStorageKey(workspaceId, approvalId));
}
