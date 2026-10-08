export type SuggestionStatus = "PROPOSED" | "ACCEPTED" | "DISMISSED" | "EXPIRED";
export type SuggestionVisibility = "VISIBLE" | "SNOOZED" | "ALL";
export type SuggestionKind = "TASK_OPPORTUNITY" | "ROUTINE_OPPORTUNITY" | "AUTOMATION_OPPORTUNITY";
export type SuggestionPreference = { workspace_id: string; kind: SuggestionKind; muted: boolean; updated_at: string | null; version: number };
export type Suggestion = {
  suggestion_id: string;
  workspace_id: string;
  coworker_id: string | null;
  dedupe_key: string;
  kind: "TASK_OPPORTUNITY" | "ROUTINE_OPPORTUNITY" | "AUTOMATION_OPPORTUNITY";
  reason: string;
  source_refs: { workspace_id: string; resource_id: string; revision_id: string }[];
  goal_refs: { goal_id: string; revision: number }[];
  proposed_action: "TASK" | "OPEN_ROUTINE_EDITOR" | "OPEN_AUTOMATION_EDITOR";
  proposed_by: { service_id: string };
  proposed_task_spec: Record<string, unknown> | null;
  estimated_cost: Record<string, unknown> | null;
  latency_class_hint: "STANDARD" | "INTERACTIVE" | "DEADLINE_SENSITIVE" | null;
  status: SuggestionStatus;
  created_at: string;
  expires_at: string;
  snoozed_until: string | null;
  resolved_at: string | null;
  resolved_by: { principal_id: string; kind: string } | null;
  resolution_reason: string | null;
  result_task_id: string | null;
  version: number;
};
export type SuggestionPage = { items: Suggestion[]; next_cursor: string | null };
export type SuggestionTransport = (workspaceId: string, visibility: SuggestionVisibility, cursor?: string, signal?: AbortSignal) => Promise<unknown>;
export type SuggestionOwnerAction = "dismiss" | "snooze" | "unsnooze";
export type SuggestionActionTransport = (workspaceId: string, suggestionId: string, operation: SuggestionOwnerAction, expectedVersion: number, requestId: string, snoozedUntil?: string) => Promise<unknown>;
export type SuggestionAcceptTaskTransport = (workspaceId: string, suggestionId: string, expectedVersion: number, requestId: string) => Promise<unknown>;
export type SuggestionPreferenceTransport = (workspaceId: string) => Promise<unknown>;
export type SuggestionPreferenceUpdateTransport = (workspaceId: string, kind: SuggestionKind, muted: boolean, expectedVersion: number, requestId: string) => Promise<unknown>;
export interface SuggestionApi {
  list(visibility?: SuggestionVisibility, cursor?: string, signal?: AbortSignal): Promise<SuggestionPage>;
  dismiss(suggestionId: string, expectedVersion: number, requestId: string): Promise<Suggestion>;
  snooze(suggestionId: string, expectedVersion: number, requestId: string, until: string): Promise<Suggestion>;
  unsnooze(suggestionId: string, expectedVersion: number, requestId: string): Promise<Suggestion>;
  acceptTask(suggestionId: string, expectedVersion: number, requestId: string): Promise<string>;
  preferences(): Promise<SuggestionPreference[]>;
  setPreference(kind: SuggestionKind, muted: boolean, expectedVersion: number, requestId: string): Promise<SuggestionPreference>;
}

export class SuggestionApiError extends Error {
  constructor(message: string) { super(message); this.name = "SuggestionApiError"; }
}
type Obj = Record<string, unknown>;
function object(value: unknown): Obj {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new SuggestionApiError("Local Runtime returned an invalid suggestion.");
  return value as Obj;
}
function text(value: unknown, empty = false): string {
  if (typeof value !== "string" || (!empty && value.trim().length === 0)) throw new SuggestionApiError("Local Runtime returned an invalid suggestion.");
  return value;
}
function nullableText(value: unknown): string | null { return value == null ? null : text(value); }
function objOrNull(value: unknown): Obj | null { return value == null ? null : object(value); }
function decodeSuggestion(value: unknown, workspaceId: string): Suggestion {
  const row = object(value);
  if (text(row.workspace_id) !== workspaceId) throw new SuggestionApiError("A suggestion belongs to another Workspace.");
  const kinds = ["TASK_OPPORTUNITY", "ROUTINE_OPPORTUNITY", "AUTOMATION_OPPORTUNITY"] as const;
  const actions = ["TASK", "OPEN_ROUTINE_EDITOR", "OPEN_AUTOMATION_EDITOR"] as const;
  const statuses = ["PROPOSED", "ACCEPTED", "DISMISSED", "EXPIRED"] as const;
  const latency = ["STANDARD", "INTERACTIVE", "DEADLINE_SENSITIVE"] as const;
  const kind = text(row.kind) as Suggestion["kind"];
  const action = text(row.proposed_action) as Suggestion["proposed_action"];
  const status = text(row.status) as SuggestionStatus;
  const latencyHint = row.latency_class_hint == null ? null : text(row.latency_class_hint) as Suggestion["latency_class_hint"];
  if (!(kinds as readonly string[]).includes(kind) || !(actions as readonly string[]).includes(action)
      || !(statuses as readonly string[]).includes(status) || (latencyHint && !(latency as readonly string[]).includes(latencyHint))) {
    throw new SuggestionApiError("Local Runtime returned an unsupported suggestion type.");
  }
  if (!Array.isArray(row.source_refs) || !Array.isArray(row.goal_refs)) throw new SuggestionApiError("Local Runtime returned invalid suggestion sources.");
  const sources = row.source_refs.map(raw => {
    const ref = object(raw);
    if (text(ref.workspace_id) !== workspaceId) throw new SuggestionApiError("A suggestion source belongs to another Workspace.");
    return { workspace_id: workspaceId, resource_id: text(ref.resource_id), revision_id: text(ref.revision_id) };
  });
  const goals = row.goal_refs.map(raw => {
    const ref = object(raw);
    if (typeof ref.revision !== "number" || !Number.isSafeInteger(ref.revision) || ref.revision < 1) throw new SuggestionApiError("A suggestion Goal reference is invalid.");
    return { goal_id: text(ref.goal_id), revision: ref.revision };
  });
  const service = object(row.proposed_by);
  if (typeof row.version !== "number" || !Number.isSafeInteger(row.version) || row.version < 1) throw new SuggestionApiError("A suggestion version is invalid.");
  if (typeof row.dedupe_key !== "string" || !/^sha256:[a-f0-9]{64}$/.test(row.dedupe_key)) throw new SuggestionApiError("A suggestion identity is invalid.");
  return {
    suggestion_id: text(row.suggestion_id), workspace_id: workspaceId, coworker_id: nullableText(row.coworker_id),
    dedupe_key: row.dedupe_key, kind, reason: text(row.reason, true), source_refs: sources, goal_refs: goals,
    proposed_action: action, proposed_by: { service_id: text(service.service_id) },
    proposed_task_spec: objOrNull(row.proposed_task_spec), estimated_cost: objOrNull(row.estimated_cost),
    latency_class_hint: latencyHint, status, created_at: text(row.created_at), expires_at: text(row.expires_at),
    snoozed_until: nullableText(row.snoozed_until), resolved_at: nullableText(row.resolved_at),
    resolved_by: row.resolved_by == null ? null : (() => { const ref = object(row.resolved_by); return { principal_id: text(ref.principal_id), kind: text(ref.kind) }; })(),
    resolution_reason: nullableText(row.resolution_reason), result_task_id: nullableText(row.result_task_id), version: row.version,
  };
}
function decodeActionResponse(value: unknown, workspaceId: string): Suggestion {
  const response = object(value);
  if (typeof response.status !== "number" || !Number.isInteger(response.status)
      || typeof response.bodyBase64 !== "string") throw new SuggestionApiError("Local Runtime returned an invalid Suggestion action response.");
  let body: unknown;
  try {
    const bytes = Uint8Array.from(atob(response.bodyBase64), character => character.charCodeAt(0));
    body = JSON.parse(new TextDecoder().decode(bytes));
  } catch { throw new SuggestionApiError("Local Runtime returned an unreadable Suggestion action response."); }
  if (response.status < 200 || response.status >= 300) {
    const error = body && typeof body === "object" ? body as Obj : {};
    throw new SuggestionApiError(typeof error.message === "string" ? error.message : "The Suggestion could not be updated.");
  }
  return decodeSuggestion(body, workspaceId);
}

function decodePreference(value: unknown, workspaceId: string): SuggestionPreference {
  const row = object(value);
  if (text(row.workspace_id) !== workspaceId) throw new SuggestionApiError("A suggestion preference belongs to another Workspace.");
  const kind = text(row.kind) as SuggestionKind;
  if (!["TASK_OPPORTUNITY", "ROUTINE_OPPORTUNITY", "AUTOMATION_OPPORTUNITY"].includes(kind)
      || typeof row.muted !== "boolean"
      || typeof row.version !== "number" || !Number.isSafeInteger(row.version) || row.version < 0
      || !(row.updated_at === null || (typeof row.updated_at === "string" && Number.isFinite(Date.parse(row.updated_at))))
      || (row.version === 0 && (row.muted || row.updated_at !== null))
      || (row.version > 0 && row.updated_at === null)) {
    throw new SuggestionApiError("Local Runtime returned an invalid suggestion preference.");
  }
  return { workspace_id: workspaceId, kind, muted: row.muted, updated_at: row.updated_at as string | null, version: row.version };
}

function decodePreferenceActionResponse(value: unknown, workspaceId: string): SuggestionPreference {
  const response = object(value);
  if (typeof response.status !== "number" || !Number.isInteger(response.status)
      || typeof response.bodyBase64 !== "string") throw new SuggestionApiError("Local Runtime returned an invalid preference response.");
  let body: unknown;
  try {
    const bytes = Uint8Array.from(atob(response.bodyBase64), character => character.charCodeAt(0));
    body = JSON.parse(new TextDecoder().decode(bytes));
  } catch { throw new SuggestionApiError("Local Runtime returned an unreadable preference response."); }
  if (response.status < 200 || response.status >= 300) {
    const error = body && typeof body === "object" ? body as Obj : {};
    throw new SuggestionApiError(typeof error.message === "string" ? error.message : "The suggestion preference could not be updated.");
  }
  return decodePreference(body, workspaceId);
}

export function createSuggestionApi(workspaceId: string, transport: SuggestionTransport, actionTransport: SuggestionActionTransport, acceptTaskTransport: SuggestionAcceptTaskTransport, preferenceTransport: SuggestionPreferenceTransport, preferenceUpdateTransport: SuggestionPreferenceUpdateTransport): SuggestionApi {
  if (!workspaceId) throw new Error("Select a Workspace before viewing suggestions.");
  const mutate = async (id: string, version: number, requestId: string, operation: SuggestionOwnerAction, until?: string) => {
    if (!id || !Number.isSafeInteger(version) || version < 1 || !requestId) throw new SuggestionApiError("Suggestion action is invalid.");
    return decodeActionResponse(await actionTransport(workspaceId, id, operation, version, requestId, until), workspaceId);
  };
  return {
    async list(visibility = "VISIBLE", cursor, signal) {
      const raw = object(await transport(workspaceId, visibility, cursor, signal));
      if (!Array.isArray(raw.items) || (raw.next_cursor != null && typeof raw.next_cursor !== "string")) throw new SuggestionApiError("Local Runtime returned an invalid suggestion page.");
      return { items: raw.items.map(item => decodeSuggestion(item, workspaceId)), next_cursor: raw.next_cursor as string | null ?? null };
    },
    dismiss: (id, version, requestId) => mutate(id, version, requestId, "dismiss"),
    snooze: (id, version, requestId, until) => mutate(id, version, requestId, "snooze", until),
    unsnooze: (id, version, requestId) => mutate(id, version, requestId, "unsnooze"),
    async acceptTask(id, version, requestId) {
      if (!id || !Number.isSafeInteger(version) || version < 1 || !requestId) throw new SuggestionApiError("Suggestion acceptance is invalid.");
      const taskId = await acceptTaskTransport(workspaceId, id, version, requestId);
      if (typeof taskId !== "string" || taskId.trim().length === 0) throw new SuggestionApiError("Local Runtime returned an invalid accepted Task.");
      return taskId;
    },
    async preferences() {
      const page = object(await preferenceTransport(workspaceId));
      if (!Array.isArray(page.items)) throw new SuggestionApiError("Local Runtime returned invalid suggestion settings.");
      const items = page.items.map(item => decodePreference(item, workspaceId));
      const kinds = new Set(items.map(item => item.kind));
      if (items.length !== 3 || kinds.size !== 3) throw new SuggestionApiError("Local Runtime returned incomplete suggestion settings.");
      return items;
    },
    async setPreference(kind, muted, version, requestId) {
      if (!["TASK_OPPORTUNITY", "ROUTINE_OPPORTUNITY", "AUTOMATION_OPPORTUNITY"].includes(kind)
          || !Number.isSafeInteger(version) || version < 0 || !requestId) {
        throw new SuggestionApiError("Suggestion preference update is invalid.");
      }
      return decodePreferenceActionResponse(await preferenceUpdateTransport(workspaceId, kind, muted, version, requestId), workspaceId);
    },
  };
}
