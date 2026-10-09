export type RoutineStatus = "ACTIVE" | "ARCHIVED";

export type Routine = {
  routine_id: string;
  workspace_id: string;
  name: string;
  current_revision: number;
  status: RoutineStatus;
  version: number;
  created_at: string;
  updated_at: string;
};

/** All definition fields are retained as JSON so editing metadata never drops a
 * field the current client does not render. The Operator validates the contract. */
export type RoutineRevision = Record<string, unknown> & {
  routine_id: string;
  revision: number;
  objective_template: string;
  instructions: string;
  authored_by: Record<string, unknown>;
  created_at: string;
};

export type RoutinePage = { items: Routine[]; next_cursor: string | null };
export type RoutineRevisionPage = { items: RoutineRevision[]; next_cursor: string | null };
export type MaterializedRoutineTask = {
  task_id: string;
  workspace_id: string;
  routine_id: string;
  routine_revision: number;
  status: "READY";
  current_spec_revision: number;
};
export type RoutineTransport = (path: string, init: RequestInit) => Promise<Response>;

export interface RoutineApi {
  list(cursor?: string, signal?: AbortSignal): Promise<RoutinePage>;
  get(routineId: string, signal?: AbortSignal): Promise<Routine>;
  revisions(routineId: string, cursor?: string, signal?: AbortSignal): Promise<RoutineRevisionPage>;
  create(name: string, revision: Record<string, unknown>, requestId: string): Promise<Routine>;
  revise(routineId: string, expectedVersion: number, revision: Record<string, unknown>, requestId: string): Promise<RoutineRevision>;
  archive(routineId: string, expectedVersion: number, requestId: string): Promise<Routine>;
  run(routineId: string, routineRevision: number, inputs: Record<string, unknown>, requestId: string): Promise<MaterializedRoutineTask>;
}

export class RoutineApiError extends Error {
  constructor(readonly code: string, readonly status: number) {
    super(status === 409
      ? code === "ROUTINE_ARCHIVE_BLOCKED"
        ? "This Routine is still used by an enabled Automation. Pause, disable, or rebind that Automation first."
        : "This Routine changed elsewhere. Refresh it before trying again."
      : status === 401 || status === 403
        ? "You do not have access to Routines in this Workspace."
        : status === 404
          ? "The Routine service or selected definition is unavailable."
          : status === 503 || status === 502
            ? "The local Routine service is unavailable. Start or reconnect the Runtime, then retry."
            : "The Routine request could not be completed.");
    this.name = "RoutineApiError";
  }
}

type JsonObject = Record<string, unknown>;
function object(value: unknown): JsonObject {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Invalid Routine API response.");
  return value as JsonObject;
}
function text(value: unknown): string {
  if (typeof value !== "string" || value.trim().length === 0) throw new Error("Invalid Routine API response.");
  return value;
}
function positiveInteger(value: unknown): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 1) throw new Error("Invalid Routine API response.");
  return value;
}
function decodeRoutine(value: unknown, workspaceId: string): Routine {
  const row = object(value);
  if (text(row.workspace_id) !== workspaceId) throw new Error("Routine response belongs to another Workspace.");
  const status = text(row.status) as RoutineStatus;
  if (status !== "ACTIVE" && status !== "ARCHIVED") throw new Error("Invalid Routine status.");
  return {
    routine_id: text(row.routine_id), workspace_id: workspaceId, name: text(row.name),
    current_revision: positiveInteger(row.current_revision), status,
    version: positiveInteger(row.version), created_at: text(row.created_at), updated_at: text(row.updated_at),
  };
}
function decodeRevision(value: unknown, routineId: string): RoutineRevision {
  const row = object(value);
  if (text(row.routine_id) !== routineId) throw new Error("Routine revision belongs to another definition.");
  const revision = positiveInteger(row.revision);
  const objectiveTemplate = text(row.objective_template);
  if (typeof row.instructions !== "string") throw new Error("Invalid Routine revision.");
  const authoredBy = object(row.authored_by);
  const createdAt = text(row.created_at);
  return {
    ...row,
    routine_id: routineId,
    revision,
    objective_template: objectiveTemplate,
    instructions: row.instructions,
    authored_by: authoredBy,
    created_at: createdAt,
  };
}
function decodeMaterializedTask(value: unknown, workspaceId: string, routineId: string, routineRevision: number): MaterializedRoutineTask {
  const view = object(value);
  const row = object(view.task);
  const spec = object(view.current_spec_revision);
  if (text(row.workspace_id) !== workspaceId || text(row.routine_id) !== routineId
    || positiveInteger(row.routine_revision) !== routineRevision || text(row.status) !== "READY"
    || text(spec.task_id) !== text(row.task_id) || positiveInteger(spec.revision) !== positiveInteger(row.current_spec_revision)) {
    throw new Error("The saved Task response did not match this Workspace and Routine revision.");
  }
  return {
    task_id: text(row.task_id), workspace_id: workspaceId, routine_id: routineId,
    routine_revision: routineRevision, status: "READY", current_spec_revision: positiveInteger(spec.revision),
  };
}

/** Finite Workspace-scoped contract for the saved Routine routes currently implemented. */
export function createRoutineApi(workspaceId: string, transport: RoutineTransport): RoutineApi {
  const request = async (path: string, init: RequestInit = {}) => {
    if (!workspaceId) throw new Error("Select a Workspace before loading Routines.");
    const headers = new Headers(init.headers);
    headers.set("X-Workspace-ID", workspaceId);
    const response = await transport(path, { ...init, headers, cache: "no-store" });
    if (!response.ok) {
      let code = "ROUTINE_REQUEST_FAILED";
      try { const body = object(await response.json()); if (typeof body.code === "string") code = body.code; } catch { /* Ignore untrusted error payload text. */ }
      throw new RoutineApiError(code, response.status);
    }
    return response;
  };
  const mutationHeaders = (requestId: string, expectedVersion?: number) => {
    if (!requestId || requestId.length > 128) throw new Error("Routine request identity is invalid.");
    const headers = new Headers({ "Idempotency-Key": requestId });
    if (expectedVersion !== undefined) {
      if (!Number.isSafeInteger(expectedVersion) || expectedVersion < 1) throw new Error("Routine version is invalid.");
      headers.set("If-Match", `"${expectedVersion}"`);
    }
    return headers;
  };
  return {
    async list(cursor, signal) {
      const query = new URLSearchParams({ limit: "50" });
      if (cursor) query.set("cursor", cursor);
      const body = object(await (await request(`/v1/routines?${query}`, { signal })).json());
      if (!Array.isArray(body.items)) throw new Error("Invalid Routine page.");
      const next = body.next_cursor;
      if (next !== null && next !== undefined && typeof next !== "string") throw new Error("Invalid Routine cursor.");
      return { items: body.items.map(item => decodeRoutine(item, workspaceId)), next_cursor: (next as string | null | undefined) ?? null };
    },
    async get(routineId, signal) {
      return decodeRoutine(await (await request(`/v1/routines/${encodeURIComponent(routineId)}`, { signal })).json(), workspaceId);
    },
    async revisions(routineId, cursor, signal) {
      const query = new URLSearchParams();
      if (cursor) query.set("cursor", cursor);
      const suffix = query.size ? `?${query}` : "";
      const body = object(await (await request(`/v1/routines/${encodeURIComponent(routineId)}/revisions${suffix}`, { signal })).json());
      if (!Array.isArray(body.items)) throw new Error("Invalid Routine revision page.");
      const next = body.next_cursor;
      if (next !== null && next !== undefined && typeof next !== "string") throw new Error("Invalid Routine revision cursor.");
      return { items: body.items.map(item => decodeRevision(item, routineId)), next_cursor: (next as string | null | undefined) ?? null };
    },
    async create(name, revision, requestId) {
      const value = await (await request("/v1/routines", {
        method: "POST", headers: mutationHeaders(requestId),
        body: JSON.stringify({ workspace_id: workspaceId, name, revision }),
      })).json();
      return decodeRoutine(value, workspaceId);
    },
    async revise(routineId, expectedVersion, revision, requestId) {
      const value = await (await request(`/v1/routines/${encodeURIComponent(routineId)}/revisions`, {
        method: "POST", headers: mutationHeaders(requestId, expectedVersion), body: JSON.stringify({ revision }),
      })).json();
      return decodeRevision(value, routineId);
    },
    async archive(routineId, expectedVersion, requestId) {
      const value = await (await request(`/v1/routines/${encodeURIComponent(routineId)}/archive`, {
        method: "POST", headers: mutationHeaders(requestId, expectedVersion),
      })).json();
      return decodeRoutine(value, workspaceId);
    },
    async run(routineId, routineRevision, inputs, requestId) {
      const value = await (await request(`/v1/routines/${encodeURIComponent(routineId)}/run`, {
        method: "POST", headers: mutationHeaders(requestId),
        body: JSON.stringify({ routine_revision: routineRevision, inputs }),
      })).json();
      return decodeMaterializedTask(value, workspaceId, routineId, routineRevision);
    },
  };
}
