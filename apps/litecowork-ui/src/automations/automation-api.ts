export type AutomationStatus = "ENABLED" | "PAUSED" | "DISABLED";

export type Automation = {
  automation_id: string;
  workspace_id: string;
  name: string;
  current_revision: number;
  status: AutomationStatus;
  version: number;
  created_at: string;
  updated_at: string;
};

export type AutomationPage = { items: Automation[]; next_cursor: string | null };
export type RoutineStatus = "ACTIVE" | "ARCHIVED";
export type Routine = {
  routine_id: string; workspace_id: string; name: string; current_revision: number;
  status: RoutineStatus; created_at: string; updated_at: string; version: number;
};
export type RoutinePage = { items: Routine[]; next_cursor: string | null };
export type RoutineRevision = Record<string, unknown> & { routine_id: string; revision: number; objective_template: string };
export type RevisionPage<T> = { items: T[]; next_cursor: string | null };
export type MaterializedAutomationTask = {
  task_id: string;
  workspace_id: string;
  automation_id: string;
  automation_revision: number;
  routine_revision: number;
  status: "READY";
  current_spec_revision: number;
  occurrence_id: string;
};
export type CoworkerRevisionRef = { coworker_id: string; revision: number };
export type AutomationCoworker = {
  coworker_id: string; workspace_id: string; current_revision: number; name: string;
  status: "ACTIVE" | "PAUSED" | "ARCHIVED";
};
export type TriggerSpec = Record<string, unknown>;
export type AutomationExecutionPolicy = {
  placement_preference: "AUTO" | "LOCAL_ONLY" | "CLOUD_PREFERRED" | "CLOUD_ONLY" | { runtime_id: string };
  max_concurrent_occurrences: number;
  overlap_policy: "SKIP" | "QUEUE" | "CANCEL_OLD" | "ALLOW";
  retry_policy: { max_attempts: number; initial_backoff_ms: number; max_backoff_ms: number; multiplier: number; jitter: boolean; retryable_error_codes: string[] };
  budget_ceiling: Record<string, unknown> | null;
  notification_policy: "ALWAYS" | "ON_SUCCESS" | "ON_FAILURE" | "ON_CONDITION" | "SILENT";
  wake_policy: "NEVER" | "TRY_WAKE" | "REQUIRE_RUNTIME_AWAKE";
};
export type AutomationRevision = {
  automation_id: string; revision: number; routine_id: string; routine_revision: number;
  triggers: TriggerSpec[]; execution_policy: AutomationExecutionPolicy;
  coworker_ref: CoworkerRevisionRef | null;
  authored_by: Record<string, unknown>; created_at: string;
};
export type AutomationDefinitionInput = {
  name: string; routine_id: string; routine_revision: number;
  triggers: TriggerSpec[]; execution_policy: AutomationExecutionPolicy;
  coworker_ref: CoworkerRevisionRef | null;
};
export type AutomationTransport = (path: string, init: RequestInit) => Promise<Response>;

export interface AutomationApi {
  list(cursor?: string, signal?: AbortSignal): Promise<AutomationPage>;
  get(automationId: string, signal?: AbortSignal): Promise<Automation>;
  getCurrentRevision(automation: Automation, signal?: AbortSignal): Promise<AutomationRevision>;
  listRoutines(cursor?: string, signal?: AbortSignal): Promise<RoutinePage>;
  getRoutine(routineId: string, signal?: AbortSignal): Promise<Routine>;
  listRoutineRevisions(routineId: string, cursor?: string, signal?: AbortSignal): Promise<RevisionPage<RoutineRevision>>;
  getRoutineRevision(routineId: string, revision: number, signal?: AbortSignal): Promise<RoutineRevision>;
  listCoworkers(cursor?: string, signal?: AbortSignal): Promise<{ items: AutomationCoworker[]; next_cursor: string | null }>;
  getCoworker(coworkerId: string, signal?: AbortSignal): Promise<AutomationCoworker>;
  listRevisions(automationId: string, cursor?: string, signal?: AbortSignal): Promise<RevisionPage<AutomationRevision>>;
  createDefinition(input: AutomationDefinitionInput, requestId: string): Promise<Automation>;
  reviseDefinition(automationId: string, expectedVersion: number, input: AutomationDefinitionInput, requestId: string): Promise<Automation>;
  pause(automationId: string, expectedVersion: number, requestId: string): Promise<Automation>;
  disable(automationId: string, expectedVersion: number, requestId: string): Promise<Automation>;
  run(automation: Automation, revision: AutomationRevision, inputs: Record<string, unknown>, requestId: string): Promise<MaterializedAutomationTask>;
}

export class AutomationApiError extends Error {
  constructor(readonly code: string, readonly status: number) {
    super(status === 409
      ? code === "AUTOMATION_NOT_PAUSED"
        ? "Pause this Automation before changing its definition."
        : "This Automation changed elsewhere. Refresh it before trying again."
      : status === 401 || status === 403
        ? "You do not have access to Automations in this Workspace."
        : status === 404
          ? "The Automation service or selected definition is not available yet."
          : status === 503 || status === 502
            ? "The local Automation service is unavailable. Start or reconnect the Runtime, then retry."
            : "The Automation request could not be completed.");
    this.name = "AutomationApiError";
  }
}

type JsonObject = Record<string, unknown>;
function object(value: unknown): JsonObject {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Invalid Automation API response.");
  return value as JsonObject;
}
function text(value: unknown): string {
  if (typeof value !== "string" || value.trim().length === 0) throw new Error("Invalid Automation API response.");
  return value;
}
function positiveInteger(value: unknown): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 1) throw new Error("Invalid Automation API response.");
  return value;
}
function decodePage<T>(bodyValue: unknown, decode: (value: unknown) => T): RevisionPage<T> {
  const body = object(bodyValue);
  if (!Array.isArray(body.items)) throw new Error("Invalid paginated API response.");
  const cursor = body.next_cursor;
  if (cursor !== null && cursor !== undefined && typeof cursor !== "string") throw new Error("Invalid API cursor.");
  return { items: body.items.map(decode), next_cursor: (cursor as string | null | undefined) ?? null };
}
function decodeAutomation(value: unknown, workspaceId: string): Automation {
  const row = object(value);
  if (text(row.workspace_id) !== workspaceId) throw new Error("Automation response belongs to a different Workspace.");
  const status = text(row.status) as AutomationStatus;
  if (!(status === "ENABLED" || status === "PAUSED" || status === "DISABLED")) throw new Error("Invalid Automation status.");
  return {
    automation_id: text(row.automation_id), workspace_id: workspaceId, name: text(row.name),
    current_revision: positiveInteger(row.current_revision), status,
    version: positiveInteger(row.version), created_at: text(row.created_at), updated_at: text(row.updated_at),
  };
}
function decodeRoutine(value: unknown, workspaceId: string): Routine {
  const row = object(value);
  if (text(row.workspace_id) !== workspaceId) throw new Error("Routine response belongs to a different Workspace.");
  const status = text(row.status) as RoutineStatus;
  if (status !== "ACTIVE" && status !== "ARCHIVED") throw new Error("Invalid Routine status.");
  return {
    routine_id: text(row.routine_id), workspace_id: workspaceId, name: text(row.name),
    current_revision: positiveInteger(row.current_revision), status,
    created_at: text(row.created_at), updated_at: text(row.updated_at), version: positiveInteger(row.version),
  };
}
function decodeRoutineRevision(value: unknown): RoutineRevision {
  const row = object(value);
  return { ...row, routine_id: text(row.routine_id), revision: positiveInteger(row.revision), objective_template: text(row.objective_template) };
}
function decodeManualRun(value: unknown, workspaceId: string, automation: Automation, revision: AutomationRevision): MaterializedAutomationTask {
  const response = object(value);
  if (text(response.automation_id) !== automation.automation_id
    || positiveInteger(response.automation_revision) !== revision.revision
    || positiveInteger(response.occurrence_version) !== 3
    || text(response.occurrence_status) !== "STARTED") throw new Error("The Manual Automation receipt does not match the selected definition.");
  const occurrenceId = text(response.occurrence_id);
  text(response.trigger_id);
  const view = object(response.task);
  const task = object(view.task);
  const spec = object(view.current_spec_revision);
  const taskId = text(task.task_id);
  const specRevision = positiveInteger(spec.revision);
  if (text(task.workspace_id) !== workspaceId || text(task.status) !== "READY"
    || text(task.automation_id) !== automation.automation_id
    || text(task.automation_occurrence_id) !== occurrenceId
    || text(task.routine_id) !== revision.routine_id
    || positiveInteger(task.routine_revision) !== revision.routine_revision
    || positiveInteger(task.current_spec_revision) !== specRevision
    || text(spec.task_id) !== taskId || text(spec.workspace_id) !== workspaceId) {
    throw new Error("The saved Task response does not match this Workspace and Automation revision.");
  }
  return {
    task_id: taskId,
    workspace_id: workspaceId,
    automation_id: automation.automation_id,
    automation_revision: revision.revision,
    routine_revision: revision.routine_revision,
    status: "READY",
    current_spec_revision: specRevision,
    occurrence_id: occurrenceId,
  };
}
function decodeCoworker(value: unknown, workspaceId: string): AutomationCoworker {
  const row = object(value);
  if (text(row.workspace_id) !== workspaceId) throw new Error("Coworker response belongs to a different Workspace.");
  const revision = object(row.revision);
  const status = text(row.status);
  if (status !== "ACTIVE" && status !== "PAUSED" && status !== "ARCHIVED") throw new Error("Invalid Coworker status.");
  return {
    coworker_id: text(row.coworker_id), workspace_id: workspaceId,
    current_revision: positiveInteger(row.current_revision), name: text(revision.name), status,
  };
}
function decodeAutomationRevision(value: unknown, automationId: string): AutomationRevision {
  const row = object(value);
  if (text(row.automation_id) !== automationId || !Array.isArray(row.triggers)) throw new Error("Automation revision identity is invalid.");
  const execution = object(row.execution_policy);
  const retry = object(execution.retry_policy);
  const overlap = text(execution.overlap_policy) as AutomationExecutionPolicy["overlap_policy"];
  const notification = text(execution.notification_policy) as AutomationExecutionPolicy["notification_policy"];
  const wake = text(execution.wake_policy) as AutomationExecutionPolicy["wake_policy"];
  const placement = execution.placement_preference;
  if (!(placement === "AUTO" || placement === "LOCAL_ONLY" || placement === "CLOUD_PREFERRED" || placement === "CLOUD_ONLY"
    || (placement && typeof placement === "object" && typeof (placement as JsonObject).runtime_id === "string"))) throw new Error("Automation placement preference is invalid.");
  if (!(["SKIP", "QUEUE", "CANCEL_OLD", "ALLOW"] as string[]).includes(overlap)
    || !(["ALWAYS", "ON_SUCCESS", "ON_FAILURE", "ON_CONDITION", "SILENT"] as string[]).includes(notification)
    || !(["NEVER", "TRY_WAKE", "REQUIRE_RUNTIME_AWAKE"] as string[]).includes(wake)
    || !Array.isArray(retry.retryable_error_codes) || retry.retryable_error_codes.some(code => typeof code !== "string")) throw new Error("Automation execution policy is invalid.");
  const numberField = (value: unknown) => { if (typeof value !== "number" || !Number.isFinite(value)) throw new Error("Automation retry policy is invalid."); return value; };
  const coworker = row.coworker_ref == null ? null : object(row.coworker_ref);
  return {
    automation_id: automationId, revision: positiveInteger(row.revision), routine_id: text(row.routine_id),
    routine_revision: positiveInteger(row.routine_revision), triggers: row.triggers.map(value => object(value)),
    execution_policy: {
      placement_preference: placement as AutomationExecutionPolicy["placement_preference"],
      max_concurrent_occurrences: positiveInteger(execution.max_concurrent_occurrences), overlap_policy: overlap,
      retry_policy: {
        max_attempts: numberField(retry.max_attempts), initial_backoff_ms: numberField(retry.initial_backoff_ms),
        max_backoff_ms: numberField(retry.max_backoff_ms), multiplier: numberField(retry.multiplier),
        jitter: retry.jitter === true, retryable_error_codes: [...retry.retryable_error_codes] as string[],
      },
      budget_ceiling: execution.budget_ceiling == null ? null : object(execution.budget_ceiling),
      notification_policy: notification, wake_policy: wake,
    },
    coworker_ref: coworker ? { coworker_id: text(coworker.coworker_id), revision: positiveInteger(coworker.revision) } : null,
    authored_by: object(row.authored_by), created_at: text(row.created_at),
  };
}

/** Finite Workspace-scoped contract for the currently available Automation routes. */
export function createAutomationApi(workspaceId: string, transport: AutomationTransport): AutomationApi {
  const request = async (path: string, init: RequestInit = {}) => {
    if (!workspaceId) throw new Error("Select a Workspace before loading Automations.");
    const headers = new Headers(init.headers);
    headers.set("X-Workspace-ID", workspaceId);
    const response = await transport(path, { ...init, headers, cache: "no-store" });
    if (!response.ok) {
      let code = "AUTOMATION_REQUEST_FAILED";
      try { const body = object(await response.json()); if (typeof body.code === "string") code = body.code; } catch { /* Do not surface untrusted server text. */ }
      throw new AutomationApiError(code, response.status);
    }
    return response;
  };
  const statusRequest = async (automationId: string, action: "pause" | "disable", expectedVersion: number, requestId: string) => {
    if (!Number.isSafeInteger(expectedVersion) || expectedVersion < 1) throw new Error("Automation version is invalid.");
    const body = object(await (await request(`/v1/automations/${encodeURIComponent(automationId)}/${action}`, {
      method: "POST",
      headers: { "Idempotency-Key": requestId, "If-Match": `\"${expectedVersion}\"` },
    })).json());
    return decodeAutomation(body, workspaceId);
  };
  const revisionRequest = (automationId: string, cursor?: string, signal?: AbortSignal) => {
    const query = new URLSearchParams(); if (cursor) query.set("cursor", cursor);
    const suffix = query.size ? `?${query}` : "";
    return request(`/v1/automations/${encodeURIComponent(automationId)}/revisions${suffix}`, { signal });
  };
  const definitionBody = (input: AutomationDefinitionInput) => ({
    name: input.name, routine_id: input.routine_id, routine_revision: input.routine_revision,
    triggers: input.triggers, execution_policy: input.execution_policy, coworker_ref: input.coworker_ref,
  });
  return {
    async list(cursor, signal) {
      const query = new URLSearchParams({ limit: "50" });
      if (cursor) query.set("cursor", cursor);
      const body = object(await (await request(`/v1/automations?${query}`, { signal })).json());
      if (!Array.isArray(body.items)) throw new Error("Invalid Automation page.");
      const next = body.next_cursor;
      if (next !== null && next !== undefined && typeof next !== "string") throw new Error("Invalid Automation cursor.");
      return { items: body.items.map(item => decodeAutomation(item, workspaceId)), next_cursor: (next as string | null | undefined) ?? null };
    },
    async get(automationId, signal) {
      return decodeAutomation(await (await request(`/v1/automations/${encodeURIComponent(automationId)}`, { signal })).json(), workspaceId);
    },
    async getCurrentRevision(automation, signal) {
      const page = decodePage(await (await revisionRequest(automation.automation_id, undefined, signal)).json(), value => decodeAutomationRevision(value, automation.automation_id));
      const revision = page.items[0];
      if (!revision || revision.revision !== automation.current_revision || revision.automation_id !== automation.automation_id) {
        throw new Error("The current immutable Automation revision could not be verified.");
      }
      return revision;
    },
    async listRoutines(cursor, signal) {
      const query = new URLSearchParams({ limit: "50" }); if (cursor) query.set("cursor", cursor);
      return decodePage(await (await request(`/v1/routines?${query}`, { signal })).json(), value => decodeRoutine(value, workspaceId));
    },
    async getRoutine(routineId, signal) {
      return decodeRoutine(await (await request(`/v1/routines/${encodeURIComponent(routineId)}`, { signal })).json(), workspaceId);
    },
    async listRoutineRevisions(routineId, cursor, signal) {
      const query = new URLSearchParams(); if (cursor) query.set("cursor", cursor);
      const suffix = query.size ? `?${query}` : "";
      return decodePage(await (await request(`/v1/routines/${encodeURIComponent(routineId)}/revisions${suffix}`, { signal })).json(), decodeRoutineRevision);
    },
    async getRoutineRevision(routineId, revision, signal) {
      if (!Number.isSafeInteger(revision) || revision < 1) throw new Error("Routine revision is invalid.");
      let cursor: string | undefined;
      for (let pageNumber = 0; pageNumber < 100; pageNumber += 1) {
        const query = new URLSearchParams(); if (cursor) query.set("cursor", cursor);
        const suffix = query.size ? `?${query}` : "";
        const page = decodePage(await (await request(`/v1/routines/${encodeURIComponent(routineId)}/revisions${suffix}`, { signal })).json(), decodeRoutineRevision);
        if (page.items.some(item => item.routine_id !== routineId)) throw new Error("Routine revision page identity mismatch.");
        const found = page.items.find(item => item.revision === revision);
        if (found) return found;
        if (!page.next_cursor) break;
        cursor = page.next_cursor;
      }
      throw new Error("The exact pinned Routine revision could not be loaded.");
    },
    async listCoworkers(cursor, signal) {
      const query = new URLSearchParams({ limit: "50" }); if (cursor) query.set("cursor", cursor);
      const body = object(await (await request(`/v1/coworkers?${query}`, { signal })).json());
      if (!Array.isArray(body.items)) throw new Error("Invalid Coworker page.");
      const next = body.next_cursor;
      if (next !== null && next !== undefined && typeof next !== "string") throw new Error("Invalid Coworker cursor.");
      return { items: body.items.map(item => decodeCoworker(item, workspaceId)), next_cursor: (next as string | null | undefined) ?? null };
    },
    async getCoworker(coworkerId, signal) {
      return decodeCoworker(await (await request(`/v1/coworkers/${encodeURIComponent(coworkerId)}`, { signal })).json(), workspaceId);
    },
    async listRevisions(automationId, cursor, signal) {
      const page = decodePage(await (await revisionRequest(automationId, cursor, signal)).json(), value => decodeAutomationRevision(value, automationId));
      if (page.items.some(item => item.automation_id !== automationId)) throw new Error("Automation revision page identity mismatch.");
      return page;
    },
    async createDefinition(input, requestId) {
      if (input.triggers.length < 1 || input.triggers.length > 10) throw new Error("Automation trigger count is out of bounds.");
      const body = object(await (await request("/v1/automations", {
        method: "POST", headers: { "Content-Type": "application/json", "Idempotency-Key": requestId },
        body: JSON.stringify({ workspace_id: workspaceId, ...definitionBody(input) }),
      })).json());
      return decodeAutomation(body, workspaceId);
    },
    async reviseDefinition(automationId, expectedVersion, input, requestId) {
      if (!Number.isSafeInteger(expectedVersion) || expectedVersion < 1) throw new Error("Automation version is invalid.");
      if (input.triggers.length < 1 || input.triggers.length > 10) throw new Error("Automation trigger count is out of bounds.");
      const body = object(await (await request(`/v1/automations/${encodeURIComponent(automationId)}`, {
        method: "PATCH", headers: { "Content-Type": "application/json", "Idempotency-Key": requestId, "If-Match": `\"${expectedVersion}\"` },
        body: JSON.stringify(definitionBody(input)),
      })).json());
      return decodeAutomation(body, workspaceId);
    },
    pause: (automationId, expectedVersion, requestId) => statusRequest(automationId, "pause", expectedVersion, requestId),
    disable: (automationId, expectedVersion, requestId) => statusRequest(automationId, "disable", expectedVersion, requestId),
    async run(automation, revision, inputs, requestId) {
      // Do not pre-reject a disabled/stale snapshot here: a retry after an ambiguous
      // response must reach the server's receipt-first exact replay path. New commands
      // are still checked against current status/version by the Operator transaction.
      if (revision.automation_id !== automation.automation_id) throw new Error("Automation revision identity is invalid.");
      if (!Number.isSafeInteger(automation.version) || automation.version < 1 || !requestId || requestId.length > 128) {
        throw new Error("Manual Automation request identity or version is invalid.");
      }
      const response = await request(`/v1/automations/${encodeURIComponent(automation.automation_id)}/run`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "Idempotency-Key": requestId,
          "If-Match": `\"${automation.version}\"`,
        },
        body: JSON.stringify({ automation_revision: revision.revision, inputs }),
      });
      if (response.status !== 200 && response.status !== 201) throw new Error("The Manual Automation response status is invalid.");
      return decodeManualRun(await response.json(), workspaceId, automation, revision);
    },
  };
}
