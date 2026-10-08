export type GoalStatus = "ACTIVE" | "PAUSED" | "COMPLETED" | "ARCHIVED";
export type RoutineRevisionRef = { routine_id: string; revision: number };
export type ArtifactVersionRef = { workspace_id: string; artifact_id: string; version: number };
export type GoalTaskOption = { task_id: string; status: string; objective: string };
export type GoalArtifactOption = { workspace_id: string; artifact_id: string; display_name: string; kind: string; current_version: number; library_status: string };
export type GoalRevisionInput = {
  objective: string;
  success_criteria: string[];
  constraints: string[];
  horizon: string | null;
  related_task_ids: string[];
  related_routine_refs: RoutineRevisionRef[];
  related_artifact_refs: ArtifactVersionRef[];
};
export type GoalTaskContribution = {
  task_id: string;
  task_status: string;
  outcome_state: "VERIFIED" | "INCOMPLETE" | "UNVERIFIED" | "STALE" | "CONFLICTED";
  evidence_refs: string[];
};
/** A read-side Task/Evidence projection. Never inferred from Goal text or status. */
export type GoalProgressProjection = {
  computed_at: string;
  availability: "COMPLETE" | "PARTIAL";
  limitations: ("VERIFICATION_RUN_READ_MODEL_UNAVAILABLE" | "TASK_DEPENDENCY_FRESHNESS_UNAVAILABLE" | "ARTIFACT_DEPENDENCY_FRESHNESS_UNAVAILABLE" | "ARTIFACT_EVIDENCE_REFERENCE_UNRESOLVED" | "EVIDENCE_LIST_TRUNCATED")[];
  verified_task_count: number | null;
  linked_task_count: number;
  stale_source_count: number | null;
  conflicted_source_count: number | null;
  contributions: GoalTaskContribution[];
  artifact_evidence_refs: { artifact_id: string; version: number; evidence_refs: string[] }[];
  summary: string;
};
export type Goal = {
  goal_id: string;
  workspace_id: string;
  coworker_id: string | null;
  current_revision: number;
  revision: GoalRevisionInput;
  status: GoalStatus;
  created_at: string;
  updated_at: string;
  version: number;
  /** Omitted when this server has no Task/Evidence projector. */
  progress?: GoalProgressProjection;
};
export type GoalPage = { items: Goal[]; next_cursor: string | null };
export type GoalOptionPage<T> = { items: T[]; next_cursor: string | null };
export type GoalTransport = (path: string, init: RequestInit) => Promise<Response>;

export interface GoalApi {
  list(cursor?: string, signal?: AbortSignal): Promise<GoalPage>;
  get(goalId: string, signal?: AbortSignal): Promise<Goal>;
  create(coworkerId: string | null, revision: GoalRevisionInput, requestId: string): Promise<Goal>;
  revise(goalId: string, expectedVersion: number, revision: GoalRevisionInput, requestId: string): Promise<Goal>;
  changeStatus(goalId: string, expectedVersion: number, status: GoalStatus, requestId: string): Promise<Goal>;
  listRelatedTasks(cursor?: string, signal?: AbortSignal): Promise<GoalOptionPage<GoalTaskOption>>;
  listRelatedArtifacts(cursor?: string, signal?: AbortSignal): Promise<GoalOptionPage<GoalArtifactOption>>;
}

export class GoalApiError extends Error {
  constructor(readonly code: string, readonly status: number) {
    super(status === 409
      ? "This Goal changed elsewhere. Reload the latest version before trying again."
      : status === 401 || status === 403
        ? "You do not have access to Goals in the selected Workspace."
        : status === 404
          ? "This Goal is no longer available in the selected Workspace."
          : status === 503 || status === 502
            ? "The local Goal service is unavailable. Start or reconnect the Runtime, then retry."
            : "The Goal change could not be completed.");
    this.name = "GoalApiError";
  }
}

type JsonObject = Record<string, unknown>;
function object(value: unknown): JsonObject {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Invalid Goal API response.");
  return value as JsonObject;
}
function text(value: unknown, allowEmpty = false): string {
  if (typeof value !== "string" || (!allowEmpty && value.trim().length === 0)) throw new Error("Invalid Goal API response.");
  return value;
}
function number(value: unknown): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) throw new Error("Invalid Goal API response.");
  return value;
}
function optionalNumber(value: unknown): number | null {
  return value === null ? null : number(value);
}
function strings(value: unknown): string[] {
  if (!Array.isArray(value) || value.some(item => typeof item !== "string" || item.trim().length === 0) || new Set(value).size !== value.length) {
    throw new Error("Invalid Goal API response.");
  }
  return [...value] as string[];
}
function decodeRevision(value: unknown): GoalRevisionInput {
  const row = object(value);
  if (row.horizon !== null && row.horizon !== undefined && typeof row.horizon !== "string") throw new Error("Invalid Goal horizon.");
  if (!Array.isArray(row.related_routine_refs)) throw new Error("Invalid Goal routine references.");
  const relatedRoutineRefs = row.related_routine_refs.map(raw => {
    const ref = object(raw);
    const revision = number(ref.revision);
    if (revision < 1) throw new Error("Invalid Goal routine revision.");
    return { routine_id: text(ref.routine_id), revision };
  });
  const relatedArtifactRefs = row.related_artifact_refs === undefined ? [] : row.related_artifact_refs;
  if (!Array.isArray(relatedArtifactRefs)) throw new Error("Invalid Goal Artifact references.");
  const artifactRefs = relatedArtifactRefs.map(raw => {
    const ref = object(raw);
    const version = number(ref.version);
    if (version < 1) throw new Error("Invalid Goal Artifact version.");
    return { workspace_id: text(ref.workspace_id), artifact_id: text(ref.artifact_id), version };
  });
  return {
    objective: text(row.objective),
    success_criteria: strings(row.success_criteria),
    constraints: strings(row.constraints),
    horizon: row.horizon == null ? null : text(row.horizon),
    related_task_ids: strings(row.related_task_ids),
    related_routine_refs: relatedRoutineRefs,
    related_artifact_refs: artifactRefs,
  };
}
function decodeProgress(value: unknown): GoalProgressProjection {
  const row = object(value);
  if (!Array.isArray(row.contributions)) throw new Error("Invalid Goal progress projection.");
  const availability = text(row.availability) as GoalProgressProjection["availability"];
  if (!(availability === "COMPLETE" || availability === "PARTIAL")) throw new Error("Invalid Goal progress availability.");
  const validLimitations = ["VERIFICATION_RUN_READ_MODEL_UNAVAILABLE", "TASK_DEPENDENCY_FRESHNESS_UNAVAILABLE", "ARTIFACT_DEPENDENCY_FRESHNESS_UNAVAILABLE", "ARTIFACT_EVIDENCE_REFERENCE_UNRESOLVED", "EVIDENCE_LIST_TRUNCATED"] as const;
  if (!Array.isArray(row.limitations) || row.limitations.some(item => !(validLimitations as readonly unknown[]).includes(item))) throw new Error("Invalid Goal progress limitations.");
  const validOutcome = ["VERIFIED", "INCOMPLETE", "UNVERIFIED", "STALE", "CONFLICTED"] as const;
  const contributions = row.contributions.map(raw => {
    const contribution = object(raw);
    const state = text(contribution.outcome_state) as GoalTaskContribution["outcome_state"];
    if (!(validOutcome as readonly string[]).includes(state)) throw new Error("Invalid Goal contribution status.");
    return {
      task_id: text(contribution.task_id), task_status: text(contribution.task_status), outcome_state: state,
      evidence_refs: strings(contribution.evidence_refs),
    };
  });
  if (!Array.isArray(row.artifact_evidence_refs)) throw new Error("Invalid Goal Artifact evidence references.");
  const artifactEvidenceRefs = row.artifact_evidence_refs.map(raw => {
    const artifact = object(raw);
    return { artifact_id: text(artifact.artifact_id), version: number(artifact.version), evidence_refs: strings(artifact.evidence_refs) };
  });
  return {
    computed_at: text(row.computed_at),
    availability, limitations: [...row.limitations] as GoalProgressProjection["limitations"],
    verified_task_count: optionalNumber(row.verified_task_count),
    linked_task_count: number(row.linked_task_count),
    stale_source_count: optionalNumber(row.stale_source_count),
    conflicted_source_count: optionalNumber(row.conflicted_source_count),
    contributions,
    artifact_evidence_refs: artifactEvidenceRefs,
    summary: text(row.summary),
  };
}
function decodeGoal(value: unknown, workspaceId: string): Goal {
  const row = object(value);
  if (text(row.workspace_id) !== workspaceId) throw new Error("Goal response belongs to a different Workspace.");
  const statuses = ["ACTIVE", "PAUSED", "COMPLETED", "ARCHIVED"] as const;
  const status = text(row.status) as GoalStatus;
  if (!(statuses as readonly string[]).includes(status)) throw new Error("Invalid Goal status.");
  const revisionValue = row.revision ?? row.definition;
  // Some adapters serialize GoalRevision as a flattened aggregate. Keep one canonical UI shape.
  const revision = decodeRevision(revisionValue);
  if (revision.related_artifact_refs.some(reference => reference.workspace_id !== workspaceId)) {
    throw new Error("Goal Artifact reference belongs to a different Workspace.");
  }
  const progress = row.progress == null ? undefined : decodeProgress(row.progress);
  return {
    goal_id: text(row.goal_id), workspace_id: workspaceId,
    coworker_id: row.coworker_id == null ? null : text(row.coworker_id),
    current_revision: number(row.current_revision), revision,
    status, created_at: text(row.created_at), updated_at: text(row.updated_at), version: number(row.version),
    ...(progress ? { progress } : {}),
  };
}

function decodePage<T>(value: unknown, decodeItem: (value: unknown) => T): GoalOptionPage<T> {
  const body = object(value);
  if (!Array.isArray(body.items)) throw new Error("Invalid Workspace item page.");
  const cursor = body.next_cursor;
  if (cursor !== null && cursor !== undefined && typeof cursor !== "string") throw new Error("Invalid Workspace item cursor.");
  return { items: body.items.map(decodeItem), next_cursor: (cursor as string | null | undefined) ?? null };
}

export function createGoalApi(workspaceId: string, transport: GoalTransport): GoalApi {
  const request = async (path: string, init: RequestInit = {}) => {
    if (!workspaceId) throw new Error("Select a Workspace before loading Goals.");
    const headers = new Headers(init.headers);
    headers.set("X-Workspace-ID", workspaceId);
    const response = await transport(path, { ...init, headers, cache: "no-store" });
    if (!response.ok) {
      let code = "GOAL_REQUEST_FAILED";
      try { const body = object(await response.json()); if (typeof body.code === "string") code = body.code; } catch { /* Do not display untrusted server text. */ }
      throw new GoalApiError(code, response.status);
    }
    return response;
  };
  const mutate = (body: unknown, requestId: string, expectedVersion?: number): RequestInit => ({
    method: "POST",
    headers: {
      "Content-Type": "application/json", "Idempotency-Key": requestId,
      ...(expectedVersion === undefined ? {} : { "If-Match": `"${expectedVersion}"` }),
    },
    body: JSON.stringify(body),
  });
  return {
    async list(cursor, signal) {
      const query = new URLSearchParams({ limit: "50" });
      if (cursor) query.set("cursor", cursor);
      const body = object(await (await request(`/v1/goals?${query}`, { signal })).json());
      if (!Array.isArray(body.items)) throw new Error("Invalid Goal page.");
      const items = body.items.map(item => decodeGoal(item, workspaceId));
      const next = body.next_cursor;
      if (next !== null && next !== undefined && typeof next !== "string") throw new Error("Invalid Goal cursor.");
      return { items, next_cursor: (next as string | null | undefined) ?? null };
    },
    async get(goalId, signal) {
      return decodeGoal(await (await request(`/v1/goals/${encodeURIComponent(goalId)}`, { signal })).json(), workspaceId);
    },
    async create(coworkerId, revision, requestId) {
      const body = object(await (await request("/v1/goals", mutate({ workspace_id: workspaceId, coworker_id: coworkerId, revision }, requestId))).json());
      return decodeGoal(body, workspaceId);
    },
    async revise(goalId, expectedVersion, revision, requestId) {
      const body = object(await (await request(`/v1/goals/${encodeURIComponent(goalId)}/revisions`, mutate(revision, requestId, expectedVersion))).json());
      return decodeGoal(body, workspaceId);
    },
    async changeStatus(goalId, expectedVersion, status, requestId) {
      const body = object(await (await request(`/v1/goals/${encodeURIComponent(goalId)}/status`, mutate({ status }, requestId, expectedVersion))).json());
      return decodeGoal(body, workspaceId);
    },
    async listRelatedTasks(cursor, signal) {
      const query = new URLSearchParams({ limit: "100" });
      if (cursor) query.set("cursor", cursor);
      return decodePage(await (await request(`/v1/tasks?${query}`, { signal })).json(), raw => {
        const row = object(raw);
        return { task_id: text(row.task_id), status: text(row.status), objective: text(row.objective) };
      });
    },
    async listRelatedArtifacts(cursor, signal) {
      const query = new URLSearchParams({ limit: "100" });
      if (cursor) query.set("cursor", cursor);
      return decodePage(await (await request(`/v1/artifacts?${query}`, { signal })).json(), raw => {
        const row = object(raw);
        if (text(row.workspace_id) !== workspaceId) throw new Error("Artifact catalog belongs to a different Workspace.");
        return {
          workspace_id: workspaceId, artifact_id: text(row.artifact_id), display_name: text(row.display_name), kind: text(row.kind),
          current_version: number(row.current_version), library_status: text(row.library_status),
        };
      });
    },
  };
}
