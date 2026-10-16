export type ContextDocumentDeletionStatusName = "ACTIVE" | "REVOKED" | "DELETION_PENDING" | "DELETED";

export type ContextDocumentDeletionStatus = {
  resourceId: string;
  status: ContextDocumentDeletionStatusName;
  manifestDigest: string | null;
  requiredReplicaCount: number;
  acknowledgedReplicaCount: number;
  updatedAt: string;
};

export function isWorkspaceNoteOwnedBySelectedWorkspace(
  contextDocument: { kind: string; owner_ref?: { kind?: string; workspace_id?: string } } | null,
  selectedWorkspaceId: string,
): boolean {
  return contextDocument?.kind === "WORKSPACE_NOTES"
    && contextDocument.owner_ref?.kind === "WORKSPACE"
    && contextDocument.owner_ref.workspace_id === selectedWorkspaceId;
}

export type ContextDocumentDeletionScope = {
  workspaceId: string;
  resourceId: string;
  generation: number;
};

export type ContextDocumentDeletionRequest = ContextDocumentDeletionScope & {
  expectedVersion: number;
  requestId: string;
};

export type ContextDocumentDeletionState = {
  scope: ContextDocumentDeletionScope;
  status: ContextDocumentDeletionStatus | null;
  request: ContextDocumentDeletionRequest | null;
  busy: boolean;
  checkingStatus: boolean;
  statusReadSequence: number;
  error: string | null;
};

export type ContextDocumentDeletionAction =
  | { type: "select"; scope: ContextDocumentDeletionScope }
  | { type: "begin"; request: ContextDocumentDeletionRequest }
  | { type: "postStarted"; request: ContextDocumentDeletionRequest }
  | { type: "postAccepted"; request: ContextDocumentDeletionRequest; status: ContextDocumentDeletionStatus }
  | { type: "postFailed"; request: ContextDocumentDeletionRequest; error: string }
  | { type: "postRejected"; request: ContextDocumentDeletionRequest; error: string }
  | { type: "statusStarted"; scope: ContextDocumentDeletionScope; sequence: number }
  | { type: "statusReceived"; scope: ContextDocumentDeletionScope; sequence: number; status: ContextDocumentDeletionStatus }
  | { type: "statusFailed"; scope: ContextDocumentDeletionScope; sequence: number; error: string };

const DIGEST_PATTERN = /^sha256:[0-9a-f]{64}$/;
const DATE_TIME_PATTERN = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$/;
const WIRE_FIELDS = [
  "resourceId",
  "status",
  "manifestDigest",
  "requiredReplicaCount",
  "acknowledgedReplicaCount",
  "updatedAt",
] as const;

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

export function decodeContextDocumentDeletionStatus(
  value: unknown,
  expectedResourceId: string,
): ContextDocumentDeletionStatus {
  if (!isRecord(value) || Object.keys(value).length !== WIRE_FIELDS.length
    || WIRE_FIELDS.some((field) => !Object.hasOwn(value, field))) {
    throw new Error("Local Runtime returned an unsupported ContextDocument deletion status.");
  }
  const status = value.status;
  const requiredReplicaCount = value.requiredReplicaCount;
  const acknowledgedReplicaCount = value.acknowledgedReplicaCount;
  const manifestDigest = value.manifestDigest;
  const updatedAt = value.updatedAt;
  if (typeof value.resourceId !== "string" || value.resourceId !== expectedResourceId
    || !["ACTIVE", "REVOKED", "DELETION_PENDING", "DELETED"].includes(String(status))
    || (manifestDigest !== null && (typeof manifestDigest !== "string" || !DIGEST_PATTERN.test(manifestDigest)))
    || !Number.isSafeInteger(requiredReplicaCount) || Number(requiredReplicaCount) < 0 || Number(requiredReplicaCount) > 0xffff_ffff
    || !Number.isSafeInteger(acknowledgedReplicaCount) || Number(acknowledgedReplicaCount) < 0
    || Number(acknowledgedReplicaCount) > Number(requiredReplicaCount)
    || typeof updatedAt !== "string" || updatedAt.length > 64 || !DATE_TIME_PATTERN.test(updatedAt)
    || !Number.isFinite(Date.parse(updatedAt))) {
    throw new Error("Local Runtime returned an invalid ContextDocument deletion status.");
  }
  if ((status === "ACTIVE" || status === "REVOKED")
    && (manifestDigest !== null || requiredReplicaCount !== 0 || acknowledgedReplicaCount !== 0)) {
    throw new Error("Local Runtime returned inconsistent ContextDocument deletion metadata.");
  }
  if ((status === "DELETION_PENDING" || status === "DELETED") && typeof manifestDigest !== "string") {
    throw new Error("Local Runtime returned a deletion status without its sealed manifest.");
  }
  if (status === "DELETED" && acknowledgedReplicaCount !== requiredReplicaCount) {
    throw new Error("Local Runtime marked ContextDocument deletion complete before all targets were acknowledged.");
  }
  return {
    resourceId: value.resourceId,
    status: status as ContextDocumentDeletionStatusName,
    manifestDigest,
    requiredReplicaCount: Number(requiredReplicaCount),
    acknowledgedReplicaCount: Number(acknowledgedReplicaCount),
    updatedAt,
  };
}

export function newContextDocumentDeletionState(scope: ContextDocumentDeletionScope): ContextDocumentDeletionState {
  return { scope, status: null, request: null, busy: false, checkingStatus: false, statusReadSequence: 0, error: null };
}

function sameScope(left: ContextDocumentDeletionScope, right: ContextDocumentDeletionScope): boolean {
  return left.workspaceId === right.workspaceId
    && left.resourceId === right.resourceId
    && left.generation === right.generation;
}

function sameRequest(left: ContextDocumentDeletionRequest | null, right: ContextDocumentDeletionRequest): boolean {
  return left !== null && sameScope(left, right)
    && left.expectedVersion === right.expectedVersion
    && left.requestId === right.requestId;
}

function statusCanAdvance(
  current: ContextDocumentDeletionStatus | null,
  next: ContextDocumentDeletionStatus,
): boolean {
  if (current === null) return true;
  switch (current.status) {
    case "ACTIVE":
      return next.status === "ACTIVE" || next.status === "REVOKED"
        || next.status === "DELETION_PENDING" || next.status === "DELETED";
    case "REVOKED":
      return next.status === "REVOKED" || next.status === "DELETION_PENDING"
        || next.status === "DELETED";
    case "DELETION_PENDING":
      return (next.status === "DELETION_PENDING" || next.status === "DELETED")
        && next.manifestDigest === current.manifestDigest
        && next.requiredReplicaCount === current.requiredReplicaCount
        && next.acknowledgedReplicaCount >= current.acknowledgedReplicaCount;
    case "DELETED":
      return next.status === "DELETED"
        && next.manifestDigest === current.manifestDigest
        && next.requiredReplicaCount === current.requiredReplicaCount
        && next.acknowledgedReplicaCount === current.acknowledgedReplicaCount;
  }
}

export function contextDocumentDeletionReducer(
  state: ContextDocumentDeletionState,
  action: ContextDocumentDeletionAction,
): ContextDocumentDeletionState {
  if (action.type === "select") return newContextDocumentDeletionState(action.scope);
  if (!sameScope(state.scope, action.type === "begin" || action.type === "postStarted"
    || action.type === "postAccepted" || action.type === "postFailed" || action.type === "postRejected" ? action.request : action.scope)) {
    return state;
  }

  switch (action.type) {
    case "begin":
      if (state.request !== null || state.status?.status === "DELETION_PENDING" || state.status?.status === "DELETED") return state;
      return { ...state, request: action.request, busy: false, error: null };
    case "postStarted":
      return sameRequest(state.request, action.request)
        ? { ...state, busy: true, error: null }
        : state;
    case "postAccepted":
      if (!sameRequest(state.request, action.request) || action.status.resourceId !== state.scope.resourceId
        || (action.status.status !== "DELETION_PENDING" && action.status.status !== "DELETED")) return state;
      return {
        ...state,
        status: statusCanAdvance(state.status, action.status) ? action.status : state.status,
        request: null,
        busy: false,
        checkingStatus: false,
        error: null,
      };
    case "postFailed":
      return sameRequest(state.request, action.request)
        ? { ...state, busy: false, error: action.error }
        : state;
    case "postRejected":
      return sameRequest(state.request, action.request)
        ? { ...state, request: null, busy: false, error: action.error }
        : state;
    case "statusStarted":
      return action.sequence > state.statusReadSequence
        ? { ...state, checkingStatus: true, statusReadSequence: action.sequence, error: state.request ? state.error : null }
        : state;
    case "statusReceived":
      if (action.sequence !== state.statusReadSequence || action.status.resourceId !== state.scope.resourceId) return state;
      return {
        ...state,
        status: statusCanAdvance(state.status, action.status) ? action.status : state.status,
        request: action.status.status === "DELETION_PENDING" || action.status.status === "DELETED" ? null : state.request,
        busy: false,
        checkingStatus: false,
        error: action.status.status === "DELETION_PENDING" || action.status.status === "DELETED" ? null : state.error,
      };
    case "statusFailed":
      return action.sequence === state.statusReadSequence
        ? { ...state, checkingStatus: false, error: action.error }
        : state;
  }
}

export function contextDocumentDeletionNeedsPolling(
  status: ContextDocumentDeletionStatus | null,
): boolean {
  return status?.status === "DELETION_PENDING";
}

export type ContextDocumentDeletionRejectionCode =
  | "RESOURCE_CONFLICT"
  | "WORKSPACE_ARCHIVED"
  | "NOT_FOUND"
  | "INVALID_ARGUMENT";

export function contextDocumentDeletionRejectionCode(error: string): ContextDocumentDeletionRejectionCode | null {
  const direct = /^(RESOURCE_CONFLICT|WORKSPACE_ARCHIVED|NOT_FOUND|INVALID_ARGUMENT):/.exec(error);
  if (direct) return direct[1] as ContextDocumentDeletionRejectionCode;

  // read_bounded_response formats stable Operator errors as
  // "message [CODE; retryable=false; reference=correlation-id]".
  const operator = /\[(RESOURCE_CONFLICT|WORKSPACE_ARCHIVED|NOT_FOUND|INVALID_ARGUMENT); retryable=false; reference=[^\]\r\n]+\]$/.exec(error);
  return operator ? operator[1] as ContextDocumentDeletionRejectionCode : null;
}

export function isDefinitiveContextDocumentDeletionRejection(error: string): boolean {
  return contextDocumentDeletionRejectionCode(error) !== null;
}

export type ContextDocumentDeletionFocusTarget = { focus(): void };
export type ContextDocumentDeletionFocusTrigger = ContextDocumentDeletionFocusTarget & {
  isConnected: boolean;
  disabled: boolean;
};

export function restoreContextDocumentDeletionFocus(
  trigger: ContextDocumentDeletionFocusTrigger | null,
  panel: ContextDocumentDeletionFocusTarget | null,
  isFocused: (target: ContextDocumentDeletionFocusTarget) => boolean,
): void {
  if (trigger?.isConnected && !trigger.disabled) {
    trigger.focus();
    if (isFocused(trigger)) return;
  }
  panel?.focus();
}

export function contextDocumentDeletionProgress(status: ContextDocumentDeletionStatus): string {
  if (status.status === "DELETED") {
    return `Deletion complete. ${status.acknowledgedReplicaCount} of ${status.requiredReplicaCount} deletion targets are acknowledged.`;
  }
  if (status.status === "DELETION_PENDING") {
    if (status.requiredReplicaCount === 0) {
      return "Deletion is pending while LiteCowork verifies that no local content remains.";
    }
    return `Deletion is pending. ${status.acknowledgedReplicaCount} of ${status.requiredReplicaCount} deletion targets are acknowledged.`;
  }
  return status.status === "REVOKED" ? "Future LiteCowork reads are blocked; stored bytes remain." : "Deletion has not been requested.";
}
