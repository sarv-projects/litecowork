export type CoworkerStatus = "ACTIVE" | "PAUSED" | "ARCHIVED";
export type CoworkerDelegationStrategy = "NATIVE_DEFAULT" | "BALANCED" | "COST_SAVER" | "HOST_DELEGATION_ONLY";
export type CoworkerInteractionDefault = "STANDARD_TRUST_POLICY" | "REQUIRE_OWNER_APPROVAL" | "HANDOFF_TO_OWNER";
export type CoworkerContextKind = "PERSONAL_PROFILE" | "COWORKER_NOTES" | "WORKSPACE_NOTES" | "GOAL_NOTES";
export type PinnedResourceRef = { workspace_id: string; resource_id: string; revision_id: string };
export type LeadFailoverPolicy = {
  mode: "DISABLED" | "ASK" | "ALLOW_LISTED";
  triggers: ("AGENT_UNAVAILABLE" | "QUOTA_EXHAUSTED" | "RUNTIME_UNAVAILABLE")[];
  fallback_agent_binding_ids: string[];
  max_lead_changes: number;
};
export type CoworkerRevisionInput = {
  name: string;
  avatar_ref: PinnedResourceRef | null;
  role_description: string;
  default_lead_agent_binding_id: string | null;
  delegation_strategy: CoworkerDelegationStrategy;
  enabled_delegation_profile_ids: string[];
  delegation_budget_policy: Record<string, unknown> | null;
  lead_failover_policy: LeadFailoverPolicy | null;
  interaction_policy: {
    read_only_work: CoworkerInteractionDefault;
    draft_creation: CoworkerInteractionDefault;
    external_mutation: CoworkerInteractionDefault;
    destructive_action: CoworkerInteractionDefault;
    financial_commitment: CoworkerInteractionDefault;
  };
  context_policy: {
    allowed_context_kinds: CoworkerContextKind[];
    max_retrieved_items: number;
    retain_task_summaries: boolean;
    require_user_confirmation_for_memory: true;
  };
  notification_policy: {
    blockers: "ALWAYS" | "SILENT";
    completion: "ALWAYS" | "ON_SUCCESS" | "SILENT";
    failures: "ALWAYS" | "SILENT";
  };
};
export type Coworker = {
  coworker_id: string;
  workspace_id: string;
  current_revision: number;
  revision: CoworkerRevisionInput;
  status: CoworkerStatus;
  is_primary: boolean;
  created_at: string;
  updated_at: string;
  version: number;
};
export type CoworkerRevisionRecord = {
  coworker_id: string;
  revision: number;
  definition: CoworkerRevisionInput;
  authored_by: { principal_id: string; kind: "USER" | "SERVICE" | "RUNTIME" | "AGENT" | "CHANNEL_IDENTITY" };
  created_at: string;
};
export type CoworkerPage = { items: Coworker[]; next_cursor: string | null };
export type CoworkerPresence = {
  coworker_id: string;
  computed_at: string;
  proactive_status: CoworkerStatus;
  activity_status: "AVAILABLE" | "PLANNING" | "WORKING" | "WAITING" | "NEEDS_YOU";
  runtime_status: "AVAILABLE" | "DEGRADED" | "OFFLINE" | "UNKNOWN";
  active_task_count: number;
  waiting_task_count: number;
  needs_you_count: number;
  last_task_activity_at: string | null;
};
export type WorkspacePrimaryReceipt = {
  workspace_id: string;
  primary_coworker_id: string | null;
  version: number;
};

export type CoworkerTransport = (path: string, init: RequestInit) => Promise<Response>;

/** Native/desktop hosts inject the authenticated Operator transport here.
 * The adapter adds the exact Workspace context to every request. */
export interface CoworkerSettingsApi {
  list(cursor?: string, signal?: AbortSignal): Promise<CoworkerPage>;
  get(coworkerId: string, signal?: AbortSignal): Promise<Coworker>;
  getRevision(coworkerId: string, revision: number, signal?: AbortSignal): Promise<CoworkerRevisionRecord>;
  getPresence(coworkerId: string, signal?: AbortSignal): Promise<CoworkerPresence>;
  create(revision: CoworkerRevisionInput, requestId: string): Promise<Coworker>;
  revise(coworkerId: string, expectedVersion: number, revision: CoworkerRevisionInput, requestId: string): Promise<Coworker>;
  changeStatus(coworkerId: string, expectedVersion: number, status: CoworkerStatus, requestId: string): Promise<Coworker>;
  setPrimary(coworkerId: string | null, expectedWorkspaceVersion: number, requestId: string): Promise<WorkspacePrimaryReceipt>;
}

export class CoworkerApiError extends Error {
  constructor(readonly code: string, readonly status: number) {
    super(code === "COWORKER_HAS_ACTIVE_WORK"
      ? "This Coworker still has active Tasks or Automations. Finish the work and disable its Automations before archiving."
      : code === "STALE_VERSION" || status === 409
        ? "This Coworker or Workspace changed elsewhere. Reload the latest state before retrying."
        : code === "COWORKER_ARCHIVED"
          ? "Archived Coworkers cannot be changed."
          : status === 401 || status === 403
            ? "You do not have access to this Coworker in the selected Workspace."
            : status === 404
              ? "This Coworker is no longer available in the selected Workspace."
              : "The Coworker change could not be completed.");
    this.name = "CoworkerApiError";
  }
}

function object(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Invalid Coworker API response.");
  return value as Record<string, unknown>;
}
function text(value: unknown): string {
  if (typeof value !== "string" || value.length === 0) throw new Error("Invalid Coworker API response.");
  return value;
}
function version(value: unknown): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 1) throw new Error("Invalid Coworker API response.");
  return value;
}
function enumValue<T extends string>(value: unknown, values: readonly T[]): T {
  if (typeof value !== "string" || !values.includes(value as T)) throw new Error("Invalid Coworker API response.");
  return value as T;
}
function stringList(value: unknown): string[] {
  if (!Array.isArray(value) || value.some(item => typeof item !== "string" || item.length === 0) || new Set(value).size !== value.length) {
    throw new Error("Invalid Coworker API response.");
  }
  return [...value] as string[];
}
function nullableText(value: unknown): string | null {
  if (value === null || value === undefined) return null;
  return text(value);
}
function decodeResourceRef(value: unknown, workspaceId: string): PinnedResourceRef | null {
  if (value == null) return null;
  const ref = object(value);
  if (text(ref.workspace_id) !== workspaceId) throw new Error("Coworker avatar belongs to a different Workspace.");
  return { workspace_id: workspaceId, resource_id: text(ref.resource_id), revision_id: text(ref.revision_id) };
}
function decodeRevision(value: unknown, workspaceId: string): CoworkerRevisionInput {
  const row = object(value);
  const strategy = enumValue(row.delegation_strategy, ["NATIVE_DEFAULT", "BALANCED", "COST_SAVER", "HOST_DELEGATION_ONLY"] as const);
  const interaction = object(row.interaction_policy);
  const interactionValues = ["STANDARD_TRUST_POLICY", "REQUIRE_OWNER_APPROVAL", "HANDOFF_TO_OWNER"] as const;
  const context = object(row.context_policy);
  const contextKinds = ["PERSONAL_PROFILE", "COWORKER_NOTES", "WORKSPACE_NOTES", "GOAL_NOTES"] as const;
  if (!Array.isArray(context.allowed_context_kinds) || typeof context.max_retrieved_items !== "number" || !Number.isInteger(context.max_retrieved_items) || context.max_retrieved_items < 0 || context.max_retrieved_items > 100 || typeof context.retain_task_summaries !== "boolean" || context.require_user_confirmation_for_memory !== true) {
    throw new Error("Invalid Coworker context policy.");
  }
  const notifications = object(row.notification_policy);
  const budget = row.delegation_budget_policy == null ? null : object(row.delegation_budget_policy);
  const failover = row.lead_failover_policy == null ? null : object(row.lead_failover_policy);
  return {
    name: text(row.name),
    avatar_ref: decodeResourceRef(row.avatar_ref, workspaceId),
    role_description: typeof row.role_description === "string" ? row.role_description : "",
    default_lead_agent_binding_id: nullableText(row.default_lead_agent_binding_id),
    delegation_strategy: strategy,
    enabled_delegation_profile_ids: stringList(row.enabled_delegation_profile_ids),
    delegation_budget_policy: budget ? { ...budget } : null,
    lead_failover_policy: failover ? {
      mode: enumValue(failover.mode, ["DISABLED", "ASK", "ALLOW_LISTED"] as const),
      triggers: Array.isArray(failover.triggers) ? failover.triggers.map(item => enumValue(item, ["AGENT_UNAVAILABLE", "QUOTA_EXHAUSTED", "RUNTIME_UNAVAILABLE"] as const)) : [],
      fallback_agent_binding_ids: stringList(failover.fallback_agent_binding_ids),
      max_lead_changes: typeof failover.max_lead_changes === "number" && Number.isInteger(failover.max_lead_changes) ? failover.max_lead_changes : 0,
    } : null,
    interaction_policy: {
      read_only_work: enumValue(interaction.read_only_work, interactionValues),
      draft_creation: enumValue(interaction.draft_creation, interactionValues),
      external_mutation: enumValue(interaction.external_mutation, interactionValues),
      destructive_action: enumValue(interaction.destructive_action, interactionValues),
      financial_commitment: enumValue(interaction.financial_commitment, interactionValues),
    },
    context_policy: {
      allowed_context_kinds: context.allowed_context_kinds.map(item => enumValue(item, contextKinds)),
      max_retrieved_items: context.max_retrieved_items,
      retain_task_summaries: context.retain_task_summaries,
      require_user_confirmation_for_memory: true,
    },
    notification_policy: {
      blockers: enumValue(notifications.blockers, ["ALWAYS", "SILENT"] as const),
      completion: enumValue(notifications.completion, ["ALWAYS", "ON_SUCCESS", "SILENT"] as const),
      failures: enumValue(notifications.failures, ["ALWAYS", "SILENT"] as const),
    },
  };
}
function decodeCoworker(value: unknown, workspaceId: string): Coworker {
  const row = object(value);
  if (text(row.workspace_id) !== workspaceId) throw new Error("Coworker response belongs to a different Workspace.");
  if (typeof row.is_primary !== "boolean") throw new Error("Invalid Coworker primary marker.");
  return {
    coworker_id: text(row.coworker_id),
    workspace_id: workspaceId,
    current_revision: version(row.current_revision),
    revision: decodeRevision(row.revision, workspaceId),
    status: enumValue(row.status, ["ACTIVE", "PAUSED", "ARCHIVED"] as const),
    is_primary: row.is_primary,
    created_at: text(row.created_at),
    updated_at: text(row.updated_at),
    version: version(row.version),
  };
}
function decodeCoworkerRevision(value: unknown, workspaceId: string, expectedCoworkerId: string, expectedRevision: number): CoworkerRevisionRecord {
  const row = object(value);
  const coworkerId = text(row.coworker_id);
  const revision = version(row.revision);
  if (coworkerId !== expectedCoworkerId || revision !== expectedRevision) {
    throw new Error("Coworker revision response identity mismatch.");
  }
  const author = object(row.authored_by);
  return {
    coworker_id: coworkerId,
    revision,
    definition: decodeRevision(row.definition, workspaceId),
    authored_by: {
      principal_id: text(author.principal_id),
      kind: enumValue(author.kind, ["USER", "SERVICE", "RUNTIME", "AGENT", "CHANNEL_IDENTITY"] as const),
    },
    created_at: text(row.created_at),
  };
}
function decodePresence(value: unknown, coworkerId: string): CoworkerPresence {
  const row = object(value);
  if (text(row.coworker_id) !== coworkerId) throw new Error("Coworker presence identity mismatch.");
  const count = (item: unknown) => typeof item === "number" && Number.isSafeInteger(item) && item >= 0 ? item : (() => { throw new Error("Invalid Coworker presence count."); })();
  return {
    coworker_id: coworkerId,
    computed_at: text(row.computed_at),
    proactive_status: enumValue(row.proactive_status, ["ACTIVE", "PAUSED", "ARCHIVED"] as const),
    activity_status: enumValue(row.activity_status, ["AVAILABLE", "PLANNING", "WORKING", "WAITING", "NEEDS_YOU"] as const),
    runtime_status: enumValue(row.runtime_status, ["AVAILABLE", "DEGRADED", "OFFLINE", "UNKNOWN"] as const),
    active_task_count: count(row.active_task_count), waiting_task_count: count(row.waiting_task_count), needs_you_count: count(row.needs_you_count),
    last_task_activity_at: row.last_task_activity_at === null ? null : text(row.last_task_activity_at),
  };
}
function decodeWorkspacePrimary(value: unknown, workspaceId: string): WorkspacePrimaryReceipt {
  const row = object(value);
  if (text(row.workspace_id) !== workspaceId) throw new Error("Workspace primary response scope mismatch.");
  return { workspace_id: workspaceId, primary_coworker_id: nullableText(row.primary_coworker_id), version: version(row.version) };
}

export function createCoworkerApi(workspaceId: string, transport: CoworkerTransport): CoworkerSettingsApi {
  const request = async (path: string, init: RequestInit = {}) => {
    if (!workspaceId) throw new Error("Select a Workspace before loading Coworkers.");
    const headers = new Headers(init.headers);
    headers.set("X-Workspace-ID", workspaceId);
    const response = await transport(path, { ...init, headers, cache: "no-store" });
    if (!response.ok) {
      let code = "COWORKER_REQUEST_FAILED";
      try {
        const body = object(await response.json());
        if (typeof body.code === "string") code = body.code;
      } catch { /* Never surface untrusted Operator response text in UI. */ }
      throw new CoworkerApiError(code, response.status);
    }
    return response;
  };
  const jsonRequest = (method: "POST", body: unknown, requestId: string, expectedVersion?: number) => ({
    method,
    headers: {
      "Content-Type": "application/json",
      "Idempotency-Key": requestId,
      ...(expectedVersion === undefined ? {} : { "If-Match": `"${expectedVersion}"` }),
    },
    body: JSON.stringify(body),
  } satisfies RequestInit);

  return {
    async list(cursor, signal) {
      const query = new URLSearchParams({ limit: "50" });
      if (cursor) query.set("cursor", cursor);
      const body = object(await (await request(`/v1/coworkers?${query}`, { signal })).json());
      if (!Array.isArray(body.items) || (body.next_cursor !== null && body.next_cursor !== undefined && typeof body.next_cursor !== "string")) throw new Error("Invalid Coworker page.");
      const items = body.items.map(item => decodeCoworker(item, workspaceId));
      return { items, next_cursor: body.next_cursor as string | null ?? null };
    },
    async get(coworkerId, signal) {
      const id = encodeURIComponent(text(coworkerId));
      return decodeCoworker(await (await request(`/v1/coworkers/${id}`, { signal })).json(), workspaceId);
    },
    async getRevision(coworkerId, revision, signal) {
      const id = encodeURIComponent(text(coworkerId));
      const exactRevision = version(revision);
      return decodeCoworkerRevision(await (await request(`/v1/coworkers/${id}/revisions/${exactRevision}`, { signal })).json(), workspaceId, coworkerId, exactRevision);
    },
    async getPresence(coworkerId, signal) {
      const id = encodeURIComponent(text(coworkerId));
      return decodePresence(await (await request(`/v1/coworkers/${id}/presence`, { signal })).json(), coworkerId);
    },
    async create(revision, requestId) {
      const body = { workspace_id: workspaceId, revision };
      return decodeCoworker(await (await request("/v1/coworkers", jsonRequest("POST", body, text(requestId)))).json(), workspaceId);
    },
    async revise(coworkerId, expectedVersion, revision, requestId) {
      const id = encodeURIComponent(text(coworkerId));
      const init = jsonRequest("POST", revision, text(requestId), version(expectedVersion));
      return decodeCoworker(await (await request(`/v1/coworkers/${id}/revisions`, init)).json(), workspaceId);
    },
    async changeStatus(coworkerId, expectedVersion, status, requestId) {
      const id = encodeURIComponent(text(coworkerId));
      const nextStatus = enumValue(status, ["ACTIVE", "PAUSED", "ARCHIVED"] as const);
      const init = jsonRequest("POST", { status: nextStatus }, text(requestId), version(expectedVersion));
      return decodeCoworker(await (await request(`/v1/coworkers/${id}/status`, init)).json(), workspaceId);
    },
    async setPrimary(coworkerId, expectedWorkspaceVersion, requestId) {
      const id = encodeURIComponent(workspaceId);
      const target = coworkerId === null ? null : text(coworkerId);
      const init = jsonRequest("POST", { coworker_id: target }, text(requestId), version(expectedWorkspaceVersion));
      return decodeWorkspacePrimary(await (await request(`/v1/workspaces/${id}/primary-coworker`, init)).json(), workspaceId);
    },
  };
}
