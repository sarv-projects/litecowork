import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./user-request-detail.css";

type UserRequestKind = "QUESTION" | "DECISION" | "RESOURCE_SELECTION" | "EXTERNAL_AUTHORIZATION";
type UserRequestMode = "FORM" | "EXTERNAL_URL";
type UserRequestStatus = "PENDING" | "ANSWERED" | "DISMISSED" | "EXPIRED" | "CANCELLED";
type UserRequestScope =
  | { kind: "CONVERSATION"; conversationId: string; conversationTurnId: string }
  | { kind: "TASK_PLANNING"; taskId: string }
  | { kind: "ATTEMPT_EXECUTION"; taskId: string; attemptId: string };
type UserRequestChoice = { choiceId: string; label: string; description: string | null };
export type UserRequestDetailView = {
  requestId: string;
  workspaceId: string;
  scope: UserRequestScope;
  kind: UserRequestKind;
  interactionMode: UserRequestMode;
  prompt: string;
  choices: UserRequestChoice[];
  status: UserRequestStatus;
  version: number;
  createdAt: string;
  expiresAt: string | null;
  resolvedAt: string | null;
};

const REQUEST_KINDS = new Set<UserRequestKind>(["QUESTION", "DECISION", "RESOURCE_SELECTION", "EXTERNAL_AUTHORIZATION"]);
const REQUEST_STATUSES = new Set<UserRequestStatus>(["PENDING", "ANSWERED", "DISMISSED", "EXPIRED", "CANCELLED"]);
const MAX_USER_REQUEST_PROMPT_BYTES = 256 * 1024;
const MAX_USER_REQUEST_CHOICES_BYTES = 512 * 1024;
const MAX_USER_REQUEST_DETAIL_BYTES = 900 * 1024;

function nonEmptyId(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0 && byteLength(value) <= 512 && !/[\u0000-\u001f\u007f]/u.test(value);
}

function byteLength(value: string): number {
  return new TextEncoder().encode(value).length;
}

function exactKeys(value: Record<string, unknown>, expected: readonly string[]): boolean {
  const actual = Object.keys(value);
  return actual.length === expected.length && expected.every(key => Object.hasOwn(value, key));
}

function serializedByteLength(value: unknown): number | null {
  try {
    const serialized = JSON.stringify(value);
    return typeof serialized === "string" ? byteLength(serialized) : null;
  } catch {
    return null;
  }
}

function isTimestamp(value: unknown): value is string {
  return typeof value === "string" && Number.isFinite(Date.parse(value));
}

function isNullableTimestamp(value: unknown): value is string | null {
  return value === null || isTimestamp(value);
}

function isScope(value: unknown): value is UserRequestScope {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const scope = value as Record<string, unknown>;
  if (scope.kind === "CONVERSATION") return exactKeys(scope, ["kind", "conversationId", "conversationTurnId"]) && nonEmptyId(scope.conversationId) && nonEmptyId(scope.conversationTurnId);
  if (scope.kind === "TASK_PLANNING") return exactKeys(scope, ["kind", "taskId"]) && nonEmptyId(scope.taskId);
  if (scope.kind === "ATTEMPT_EXECUTION") return exactKeys(scope, ["kind", "taskId", "attemptId"]) && nonEmptyId(scope.taskId) && nonEmptyId(scope.attemptId);
  return false;
}

export function isUserRequestDetailFor(
  expectedWorkspaceId: string,
  expectedRequestId: string,
  value: unknown,
): value is UserRequestDetailView {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const detail = value as Record<string, unknown>;
  if (!exactKeys(detail, ["requestId", "workspaceId", "scope", "kind", "interactionMode", "prompt", "choices", "status", "version", "createdAt", "expiresAt", "resolvedAt"])) return false;
  const choices = detail.choices;
  const validChoices = Array.isArray(choices) && choices.length <= 500 && choices.every(choice => {
    if (!choice || typeof choice !== "object" || Array.isArray(choice)) return false;
    const candidate = choice as Record<string, unknown>;
    return exactKeys(candidate, ["choiceId", "label", "description"])
      && nonEmptyId(candidate.choiceId)
      && typeof candidate.label === "string" && candidate.label.trim().length > 0 && byteLength(candidate.label) <= 16_384
      && (candidate.description === null || (typeof candidate.description === "string" && byteLength(candidate.description) <= 16_384));
  });
  const validKind = typeof detail.kind === "string" && REQUEST_KINDS.has(detail.kind as UserRequestKind);
  const validStatus = typeof detail.status === "string" && REQUEST_STATUSES.has(detail.status as UserRequestStatus);
  return detail.workspaceId === expectedWorkspaceId
    && detail.requestId === expectedRequestId
    && nonEmptyId(detail.workspaceId)
    && nonEmptyId(detail.requestId)
    && isScope(detail.scope)
    && validKind
    && (detail.interactionMode === "FORM" || detail.interactionMode === "EXTERNAL_URL")
    && ((detail.interactionMode === "EXTERNAL_URL" && detail.kind === "EXTERNAL_AUTHORIZATION" && Array.isArray(choices) && choices.length === 0)
      || (detail.interactionMode === "FORM" && detail.kind !== "EXTERNAL_AUTHORIZATION" && validChoices))
    && typeof detail.prompt === "string" && detail.prompt.trim().length > 0 && byteLength(detail.prompt) <= MAX_USER_REQUEST_PROMPT_BYTES
    && validStatus
    && Number.isSafeInteger(detail.version) && (detail.version as number) >= 1
    && isTimestamp(detail.createdAt)
    && isNullableTimestamp(detail.expiresAt)
    && isNullableTimestamp(detail.resolvedAt)
    && serializedByteLength(choices) !== null && serializedByteLength(choices)! <= MAX_USER_REQUEST_CHOICES_BYTES
    && serializedByteLength(value) !== null && serializedByteLength(value)! <= MAX_USER_REQUEST_DETAIL_BYTES;
}

function statusLabel(status: UserRequestStatus): string {
  switch (status) {
    case "PENDING": return "Waiting for your response";
    case "ANSWERED": return "Response received";
    case "DISMISSED": return "Request dismissed";
    case "EXPIRED": return "Request expired";
    case "CANCELLED": return "Request cancelled";
  }
}

function scopeLabel(scope: UserRequestScope): string {
  switch (scope.kind) {
    case "CONVERSATION": return "Conversation";
    case "TASK_PLANNING": return "Task planning";
    case "ATTEMPT_EXECUTION": return "Task execution";
  }
}

function kindLabel(kind: UserRequestKind): string {
  switch (kind) {
    case "QUESTION": return "Question";
    case "DECISION": return "Decision";
    case "RESOURCE_SELECTION": return "Resource selection";
    case "EXTERNAL_AUTHORIZATION": return "Sign-in request";
  }
}

function displayTime(value: string | null): string {
  if (value === null) return "Not recorded";
  const parsed = Date.parse(value);
  return Number.isFinite(parsed) ? new Date(parsed).toLocaleString() : "Time unavailable";
}

type UserRequestActionReceipt = {
  requestId: string;
  workspaceId: string;
  status: "ANSWERED" | "DISMISSED";
  version: number;
  resolvedAt: string;
};

function isActionReceipt(value: unknown, workspaceId: string, requestId: string, expectedVersion: number): value is UserRequestActionReceipt {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const receipt = value as Record<string, unknown>;
  return receipt.requestId === requestId && receipt.workspaceId === workspaceId
    && (receipt.status === "ANSWERED" || receipt.status === "DISMISSED")
    && receipt.version === expectedVersion + 1 && isTimestamp(receipt.resolvedAt);
}

function createIdempotencyKey(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") return crypto.randomUUID();
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  return [...bytes].map(byte => byte.toString(16).padStart(2, "0")).join("");
}

export function UserRequestDetail({
  workspaceId,
  requestId,
  operatorReady,
  refreshToken = 0,
  onBack,
  onResolved,
}: {
  workspaceId: string;
  requestId: string;
  operatorReady: boolean;
  refreshToken?: number;
  onBack: () => void;
  onResolved?: (status: "ANSWERED" | "DISMISSED") => void;
}) {
  const [loadedDetail, setLoadedDetail] = useState<{ workspaceId: string; requestId: string; value: UserRequestDetailView } | null>(null);
  const detail = loadedDetail?.workspaceId === workspaceId && loadedDetail.requestId === requestId ? loadedDetail.value : null;
  const [loading, setLoading] = useState(false);
  const [failure, setFailure] = useState<"offline" | "load" | null>(null);
  const [retryVersion, setRetryVersion] = useState(0);
  const generation = useRef(0);
  const title = useRef<HTMLHeadingElement>(null);
  const [choiceId, setChoiceId] = useState("");
  const [textResponse, setTextResponse] = useState("");
  const [actionBusy, setActionBusy] = useState(false);
  const [actionFailure, setActionFailure] = useState<string | null>(null);
  const actionKey = useRef<{ fingerprint: string; key: string } | null>(null);

  useEffect(() => {
    setChoiceId("");
    setTextResponse("");
    setActionFailure(null);
    actionKey.current = null;
  }, [workspaceId, requestId]);

  useLayoutEffect(() => {
    title.current?.focus();
  }, [workspaceId, requestId]);

  useLayoutEffect(() => {
    generation.current += 1;
    setLoadedDetail(current => current?.workspaceId === workspaceId && current.requestId === requestId ? current : null);
  }, [workspaceId, requestId, operatorReady, retryVersion, refreshToken]);

  useEffect(() => {
    const current = generation.current;
    let active = true;
    if (!workspaceId || !requestId) {
      setLoadedDetail(null);
      setLoading(false);
      setFailure("load");
      return () => { active = false; };
    }
    if (!operatorReady) {
      setLoading(false);
      setFailure("offline");
      return () => { active = false; };
    }

    setLoading(true);
    setFailure(null);
    void invoke<unknown>("get_user_request_detail", { workspaceId, requestId }).then(value => {
      if (!active || generation.current !== current) return;
      if (!isUserRequestDetailFor(workspaceId, requestId, value)) throw new Error("The request detail did not match this Workspace and request.");
      setLoadedDetail({ workspaceId, requestId, value: value as UserRequestDetailView });
      setFailure(null);
    }).catch(() => {
      if (!active || generation.current !== current) return;
      setFailure("load");
    }).finally(() => {
      if (active && generation.current === current) setLoading(false);
    });
    return () => { active = false; };
  }, [workspaceId, requestId, operatorReady, retryVersion, refreshToken]);

  const staleOffline = failure === "offline" && detail !== null;
  const retry = () => setRetryVersion(value => value + 1);
  const canSubmitChoice = detail?.interactionMode === "FORM"
    && (detail.kind === "DECISION" || detail.kind === "RESOURCE_SELECTION")
    && detail.choices.length > 0;

  const submitAction = async (action: "respond" | "dismiss", input?: { response?: unknown; choiceId?: string }) => {
    if (!detail || detail.status !== "PENDING" || !operatorReady || actionBusy) return;
    const fingerprint = JSON.stringify({ action, workspaceId, requestId, version: detail.version, input });
    if (actionKey.current?.fingerprint !== fingerprint) actionKey.current = { fingerprint, key: createIdempotencyKey() };
    const key = actionKey.current.key;
    setActionBusy(true);
    setActionFailure(null);
    try {
      const receipt = action === "dismiss"
        ? await invoke<unknown>("dismiss_user_request", {
            workspaceId, requestId, expectedVersion: detail.version, idempotencyKey: key,
          })
        : await invoke<unknown>("respond_user_request", {
            workspaceId, requestId, expectedVersion: detail.version, idempotencyKey: key,
            response: input?.response ?? null, choiceId: input?.choiceId ?? null,
          });
      if (!isActionReceipt(receipt, workspaceId, requestId, detail.version)) throw new Error("The saved response did not match this request.");
      setActionFailure(null);
      setTextResponse("");
      setChoiceId("");
      onResolved?.(receipt.status);
      setRetryVersion(value => value + 1);
    } catch {
      setActionFailure("The response could not be confirmed. Refresh the request before trying again.");
    } finally {
      setActionBusy(false);
    }
  };

  return <section className="user-request-detail" aria-labelledby="user-request-detail-title" aria-busy={loading}>
    <div className="user-request-detail-heading">
      <div>
        <div className="eyebrow">USER REQUEST</div>
        <h2 id="user-request-detail-title" ref={title} tabIndex={-1}>Request details</h2>
      </div>
      <button type="button" className="quiet-button" onClick={onBack}>Back to Needs You</button>
    </div>
    {loading && <p role="status" className="user-request-detail-notice">
      {detail ? "Checking the latest request status. Showing the last loaded details until this check completes." : "Loading request details…"}
    </p>}
    {failure === "offline" && <p role="status" className="user-request-detail-notice">
      {staleOffline
        ? "Local Runtime is offline. This is the last loaded request status and may be out of date."
        : "Local Runtime is offline. Request details cannot be checked until it reconnects."}
    </p>}
    {failure === "load" && <div className="user-request-detail-error" role="alert">
      <p>{detail
        ? "The latest status could not be checked. Showing the last loaded request status; retry to check again."
        : "Request details could not be loaded. The request status has not been inferred."}</p>
      <button type="button" className="quiet-button" onClick={retry} disabled={!operatorReady || loading}>Retry request details</button>
    </div>}
    {failure === "offline" && !detail && <button type="button" className="quiet-button" onClick={retry} disabled={!operatorReady || loading}>Retry request details</button>}
    {detail && <>
      <div className="user-request-detail-meta">
        <span className={`needs-you-status ${detail.status === "PENDING" ? "status-open" : "status-resolved"}`}>{statusLabel(detail.status)}</span>
        <span className="user-request-detail-kind">{kindLabel(detail.kind)}</span>
      </div>
      <p className="user-request-detail-scope">{scopeLabel(detail.scope)}</p>
      <div className="user-request-detail-prompt" aria-label="Request prompt">{detail.prompt}</div>
      {detail.choices.length > 0 && <section aria-labelledby="user-request-choices-title">
        <h3 id="user-request-choices-title">Choices</h3>
        {canSubmitChoice && detail.status === "PENDING" ? <fieldset className="user-request-choice-list">
          <legend className="sr-only">Choose one response</legend>
          {detail.choices.map(choice => <label className="user-request-choice-option" key={choice.choiceId}>
            <input type="radio" name={`user-request-choice-${requestId}`} value={choice.choiceId} checked={choiceId === choice.choiceId} onChange={() => setChoiceId(choice.choiceId)} disabled={!operatorReady || actionBusy} />
            <span><strong className="user-request-choice-label">{choice.label}</strong>{choice.description && <span className="user-request-choice-description">{choice.description}</span>}</span>
          </label>)}
        </fieldset> : <ul className="user-request-choice-list">
          {detail.choices.map(choice => <li key={choice.choiceId}>
            <strong className="user-request-choice-label">{choice.label}</strong>
            {choice.description && <p>{choice.description}</p>}
          </li>)}
        </ul>}
      </section>}
      {detail.interactionMode === "EXTERNAL_URL" && <p className="user-request-detail-notice">{detail.status === "PENDING"
        ? "This request needs an external sign-in handoff. The provider address is kept private and is not shown in this read-only view."
        : "This request was configured for an external sign-in handoff. The provider address is kept private and is not shown in this read-only view."}</p>}
      <dl className="user-request-detail-facts">
        <div><dt>Created</dt><dd>{displayTime(detail.createdAt)}</dd></div>
        <div><dt>Expires</dt><dd>{displayTime(detail.expiresAt)}</dd></div>
        <div><dt>Resolved</dt><dd>{displayTime(detail.resolvedAt)}</dd></div>
      </dl>
      {detail.interactionMode === "FORM" && detail.status === "PENDING" && !canSubmitChoice && <label className="user-request-response-field">
        <span>Your response</span>
        <textarea value={textResponse} onChange={event => setTextResponse(event.currentTarget.value)} rows={4} maxLength={65536} disabled={!operatorReady || actionBusy} />
        <small>Your answer is checked against the request before it is saved. Do not enter passwords, keys, or other credentials.</small>
      </label>}
      {detail.status === "PENDING" && actionFailure && <p className="user-request-detail-error" role="alert">{actionFailure}</p>}
      {detail.status === "PENDING" && <div className="user-request-actions">
        {detail.interactionMode === "FORM" && <button type="button" className="primary-button" disabled={!operatorReady || actionBusy || (canSubmitChoice ? !choiceId : !textResponse.trim())} onClick={() => void submitAction("respond", canSubmitChoice ? { choiceId } : { response: textResponse })}>{actionBusy ? "Saving…" : "Save response"}</button>}
        {detail.interactionMode === "EXTERNAL_URL" && <>
          <p className="user-request-detail-notice">The sign-in handoff is not available from this screen yet. You can decline or cancel this request; the provider address remains private.</p>
          <button type="button" className="quiet-button" disabled={!operatorReady || actionBusy} onClick={() => void submitAction("respond", { response: { action: "decline" } })}>Decline sign-in</button>
          <button type="button" className="quiet-button" disabled={!operatorReady || actionBusy} onClick={() => void submitAction("respond", { response: { action: "cancel" } })}>Cancel request</button>
        </>}
        <button type="button" className="quiet-button" disabled={!operatorReady || actionBusy} onClick={() => void submitAction("dismiss")}>{actionBusy ? "Saving…" : "Dismiss request"}</button>
      </div>}
      {detail.status === "ANSWERED" && <p className="user-request-detail-notice" role="status">Response saved. Provider continuation has not been confirmed.</p>}
    </>}
  </section>;
}
