export type DelegationProfileStatus = "ENABLED" | "DISABLED" | "ARCHIVED";

export type DelegationProfileRevisionInput = {
  name: string;
  routing_description: string;
  instructions: string | null;
  // The UI deliberately exposes no model/session selector until the adapter has
  // negotiated and validated an option schema.
  session_options: Record<string, never>;
  session_options_descriptor_digest: null;
  required_features: string[];
  preferred_features: string[];
  enforced_policy: {
    capability_allowlist: unknown[];
    maximum_effect_risk: "SAFE";
    filesystem_write_scope: "WORKTREE_ONLY";
    external_effects: "DENY";
    secret_access: "NONE";
  };
  optimization_preference: "BALANCED";
  quality_floor: null;
  max_concurrency: 1;
  max_host_delegation_depth: 0;
  budget_ceiling: null;
  latency_class: "STANDARD";
  environment_policy: {
    placement_preference: "AUTO";
    isolation: "REQUIRED";
    sharing_scope: "ATTEMPT_PRIVATE";
  };
  native_delegation_policy: "INHERIT";
  warm_policy: {
    host: "COLD";
    native_session: "CLOSE_ON_SETTLE";
    capability_hosts: "COLD";
    browser_environment: "COLD";
    local_model: "PROVIDER_DEFAULT";
    ttl_ms: null;
    max_memory_bytes: null;
    max_idle_cost: null;
    triggers: string[];
  };
};

export type DelegationProfile = {
  delegation_profile_id: string;
  workspace_id: string;
  agent_binding_id: string;
  name: string;
  current_revision: number;
  revision: DelegationProfileRevisionInput & {
    delegation_profile_id: string;
    revision: number;
    workspace_id: string;
    authored_by: unknown;
    created_at: string;
  };
  status: DelegationProfileStatus;
  editable: boolean;
  edit_block_reason: string | null;
  created_at: string;
  updated_at: string;
  version: number;
};
export type DelegationProfileCatalogItem = DelegationProfile;

export type DelegationProfileRevisionView = {
  delegation_profile_id: string;
  revision: number;
  workspace_id: string;
  name: string;
  routing_description: string;
  instructions: string | null;
  authored_by: unknown;
  created_at: string;
};

export type WorkerTaskCategory = "SOFTWARE_ENGINEERING" | "RESEARCH" | "WRITING" | "DATA_ANALYSIS" | "OFFICE" | "BROWSER" | "PERSONAL_ADMIN" | "OTHER";
export type WorkerPerformanceProjection = {
  delegation_profile_id: string;
  task_category: WorkerTaskCategory;
  sample_count: number;
  verifier_pass_rate: number | null;
  median_latency_ms: number | null;
  median_observed_cost: { quantity: number; unit: string; currency: string | null; confidence: "EXACT" | "ESTIMATED" } | null;
  retry_rate: number | null;
  human_intervention_rate: number | null;
  failure_categories: string[];
  confidence: "LOW" | "MEDIUM" | "HIGH";
};

export type DelegationProfilePage = { items: DelegationProfile[]; next_cursor: string | null };

export type DelegationProfileOperation = {
  path: string;
  method?: "GET" | "POST";
  requestId?: string;
  expectedVersion?: number;
  body?: unknown;
};

export type DelegationProfileTransport = (operation: DelegationProfileOperation, signal?: AbortSignal) => Promise<Response>;

function object(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Invalid worker profile response.");
  return value as Record<string, unknown>;
}
function text(value: unknown, label = "worker profile response", maxLength = 12000): string {
  if (typeof value !== "string" || value.trim().length === 0 || value.length > maxLength || /[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/.test(value)) throw new Error(`Invalid ${label}.`);
  return value;
}
function positiveInteger(value: unknown, label: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 1) throw new Error(`Invalid ${label}.`);
  return value;
}
function decodeProfile(value: unknown, workspaceId: string, expectedBindingId?: string): DelegationProfile {
  // Only explicitly supported, non-secret fields are retained. The server response
  // remains untrusted even though React escapes rendered strings.
  const row = object(value);
  const profileId = text(row.delegation_profile_id, "worker profile identifier", 256);
  if (text(row.workspace_id) !== workspaceId) throw new Error("Worker profile belongs to a different Workspace.");
  const bindingId = text(row.agent_binding_id, "agent binding identifier", 256);
  if (expectedBindingId && bindingId !== expectedBindingId) throw new Error("Worker profile belongs to a different agent binding.");
  const currentRevision = positiveInteger(row.current_revision, "worker profile revision");
  const version = positiveInteger(row.version, "worker profile version");
  const status = row.status;
  if (status !== "ENABLED" && status !== "DISABLED" && status !== "ARCHIVED") throw new Error("Invalid worker profile status.");
  const revision = object(row.revision);
  if (text(revision.delegation_profile_id, "worker profile identifier", 256) !== profileId || revision.revision !== currentRevision || text(revision.workspace_id) !== workspaceId) {
    throw new Error("Worker profile current revision does not match its head.");
  }
  const name = text(row.name, "worker profile name", 120);
  if (text(revision.name, "worker profile name", 120) !== name) throw new Error("Worker profile name does not match its current revision.");
  const routing = text(revision.routing_description, "worker routing description", 240);
  const instructions = revision.instructions === null || revision.instructions === undefined ? null : text(revision.instructions, "worker instructions", 12000);
  const sessionOptions = object(revision.session_options);
  const policy = object(revision.enforced_policy);
  const environment = object(revision.environment_policy);
  const warm = object(revision.warm_policy);
  const emptyFeatures = Array.isArray(revision.required_features) && revision.required_features.length === 0
    && Array.isArray(revision.preferred_features) && revision.preferred_features.length === 0;
  const editable = Object.keys(sessionOptions).length === 0
    && revision.session_options_descriptor_digest === null
    && emptyFeatures
    && Array.isArray(policy.capability_allowlist) && policy.capability_allowlist.length === 0
    && policy.maximum_effect_risk === "SAFE"
    && policy.filesystem_write_scope === "WORKTREE_ONLY"
    && policy.external_effects === "DENY"
    && policy.secret_access === "NONE"
    && revision.optimization_preference === "BALANCED"
    && (revision.quality_floor === null || revision.quality_floor === undefined)
    && revision.max_concurrency === 1
    && revision.max_host_delegation_depth === 0
    && (revision.budget_ceiling === null || revision.budget_ceiling === undefined)
    && revision.latency_class === "STANDARD"
    && environment.placement_preference === "AUTO"
    && environment.isolation === "REQUIRED"
    && environment.sharing_scope === "ATTEMPT_PRIVATE"
    && revision.native_delegation_policy === "INHERIT"
    && warm.host === "COLD"
    && warm.native_session === "CLOSE_ON_SETTLE"
    && warm.capability_hosts === "COLD"
    && warm.browser_environment === "COLD"
    && warm.local_model === "PROVIDER_DEFAULT"
    && (warm.ttl_ms === null || warm.ttl_ms === undefined)
    && (warm.max_memory_bytes === null || warm.max_memory_bytes === undefined)
    && (warm.max_idle_cost === null || warm.max_idle_cost === undefined)
    && Array.isArray(warm.triggers) && warm.triggers.length === 0;
  const defaultRevision = newDelegationProfileRevision(name, routing, instructions ?? "");
  const decodedRevision: DelegationProfile["revision"] = {
    ...defaultRevision,
    delegation_profile_id: profileId,
    revision: currentRevision,
    workspace_id: workspaceId,
    authored_by: revision.authored_by,
    created_at: text(revision.created_at, "worker profile timestamp", 64),
  };
  return {
    delegation_profile_id: profileId,
    workspace_id: workspaceId,
    agent_binding_id: bindingId,
    name,
    current_revision: currentRevision,
    revision: decodedRevision,
    status,
    editable,
    edit_block_reason: editable ? null : "This revision contains settings not supported by the current adapter or editor. Refresh adapter options before changing it.",
    created_at: text(row.created_at, "worker profile timestamp", 64),
    updated_at: text(row.updated_at, "worker profile timestamp", 64),
    version,
  };
}

function decodeRevision(value: unknown, workspaceId: string, profileId: string): DelegationProfileRevisionView {
  const row = object(value);
  if (text(row.workspace_id) !== workspaceId
    || text(row.delegation_profile_id, "worker profile identifier", 256) !== profileId) {
    throw new Error("Worker profile revision belongs to a different scope.");
  }
  const instructions = row.instructions === null || row.instructions === undefined
    ? null : text(row.instructions, "worker instructions", 12000);
  if (!Number.isSafeInteger(row.revision) || (row.revision as number) < 1) {
    throw new Error("Invalid worker profile revision number.");
  }
  return {
    delegation_profile_id: profileId,
    revision: row.revision as number,
    workspace_id: workspaceId,
    name: text(row.name, "worker profile name", 120),
    routing_description: text(row.routing_description, "worker routing description", 240),
    instructions,
    authored_by: row.authored_by,
    created_at: text(row.created_at, "worker profile revision timestamp", 64),
  };
}

const WORKER_TASK_CATEGORIES: readonly WorkerTaskCategory[] = [
  "SOFTWARE_ENGINEERING", "RESEARCH", "WRITING", "DATA_ANALYSIS", "OFFICE", "BROWSER", "PERSONAL_ADMIN", "OTHER",
];

function nullableRate(value: unknown, label: string): number | null {
  if (value === null || value === undefined) return null;
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > 1) {
    throw new Error(`Invalid worker performance ${label}.`);
  }
  return value;
}

function nullableNonNegativeInteger(value: unknown, label: string): number | null {
  if (value === null || value === undefined) return null;
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw new Error(`Invalid worker performance ${label}.`);
  }
  return value;
}

function decodePerformance(value: unknown, profileId: string, requestedCategory?: WorkerTaskCategory): WorkerPerformanceProjection {
  const row = object(value);
  if (text(row.delegation_profile_id, "worker profile identifier", 256) !== profileId) {
    throw new Error("Worker performance projection belongs to a different profile scope.");
  }
  if (typeof row.task_category !== "string" || !WORKER_TASK_CATEGORIES.includes(row.task_category as WorkerTaskCategory)
    || (requestedCategory !== undefined && row.task_category !== requestedCategory)) {
    throw new Error("Worker performance projection has a different or invalid task category.");
  }
  if (typeof row.sample_count !== "number" || !Number.isSafeInteger(row.sample_count) || row.sample_count < 0) {
    throw new Error("Invalid worker performance sample count.");
  }
  const confidence = row.confidence;
  if (confidence !== "LOW" && confidence !== "MEDIUM" && confidence !== "HIGH") {
    throw new Error("Invalid worker performance confidence.");
  }
  const rawFailures = row.failure_categories;
  if (!Array.isArray(rawFailures) || rawFailures.length > 64) throw new Error("Invalid worker performance failure categories.");
  const failureCategories = rawFailures.map(item => text(item, "worker performance failure category", 128));
  if (new Set(failureCategories).size !== failureCategories.length) throw new Error("Duplicate worker performance failure category.");
  let medianObservedCost: WorkerPerformanceProjection["median_observed_cost"] = null;
  if (row.median_observed_cost !== null && row.median_observed_cost !== undefined) {
    const cost = object(row.median_observed_cost);
    if (typeof cost.quantity !== "number" || !Number.isFinite(cost.quantity) || cost.quantity < 0) {
      throw new Error("Invalid worker performance cost quantity.");
    }
    const currency = cost.currency;
    if (currency !== null && currency !== undefined && (typeof currency !== "string" || !/^[A-Z]{3}$/.test(currency))) {
      throw new Error("Invalid worker performance cost currency.");
    }
    if (cost.confidence !== "EXACT" && cost.confidence !== "ESTIMATED") throw new Error("Invalid worker performance cost confidence.");
    medianObservedCost = {
      quantity: cost.quantity,
      unit: text(cost.unit, "worker performance cost unit", 64),
      currency: currency ?? null,
      confidence: cost.confidence,
    };
  }
  return {
    delegation_profile_id: profileId,
    task_category: row.task_category as WorkerTaskCategory,
    sample_count: row.sample_count,
    verifier_pass_rate: nullableRate(row.verifier_pass_rate, "verification rate"),
    median_latency_ms: nullableNonNegativeInteger(row.median_latency_ms, "median latency"),
    median_observed_cost: medianObservedCost,
    retry_rate: nullableRate(row.retry_rate, "retry rate"),
    human_intervention_rate: nullableRate(row.human_intervention_rate, "human intervention rate"),
    failure_categories: failureCategories,
    confidence,
  };
}

async function checkedJson(response: Response, expectedStatus: number): Promise<unknown> {
  if (response.status !== expectedStatus) {
    if (response.status === 409 || response.status === 412) throw new Error("The worker profile changed elsewhere. Refresh it and review the latest revision before saving again.");
    if (response.status === 401 || response.status === 403) throw new Error("This Workspace does not authorize worker profile changes.");
    if (response.status === 404) throw new Error("The selected Workspace or worker profile no longer exists.");
    if (response.status === 422) throw new Error("The adapter or policy rejected these worker profile settings.");
    throw new Error(`Worker profile request failed (${response.status}).`);
  }
  return response.json();
}

export function createDelegationProfileCatalogApi(workspaceId: string, transport: DelegationProfileTransport) {
  return {
    async listAll(signal?: AbortSignal): Promise<DelegationProfile[]> {
      if (!workspaceId) return [];
      const items: DelegationProfile[] = [];
      const seen = new Set<string>();
      let cursor: string | null = null;
      for (let pageNumber = 0; pageNumber < 100; pageNumber += 1) {
        const url = new URL("/v1/delegation-profiles", "https://operator.invalid");
        url.searchParams.set("limit", "200");
        if (cursor) url.searchParams.set("cursor", cursor);
        const response = await transport({ path: `${url.pathname}${url.search}` }, signal);
        if (!response.ok) throw new Error(`Worker profiles could not be loaded (${response.status}).`);
        const row = object(await response.json());
        if (!Array.isArray(row.items)) throw new Error("Invalid worker profile catalog page.");
        for (const item of row.items) items.push(decodeProfile(item, workspaceId));
        const next = row.next_cursor;
        if (next === null || next === undefined || next === "") return items;
        if (typeof next !== "string" || next.length > 2048 || seen.has(next)) throw new Error("Worker profile pagination did not advance.");
        seen.add(next);
        cursor = next;
      }
      throw new Error("Worker profile catalog exceeds the safe paging limit.");
    },
    async listRevisions(profileId: string, signal?: AbortSignal): Promise<DelegationProfileRevisionView[]> {
      if (!workspaceId) return [];
      if (!profileId || profileId === "." || profileId === ".." || profileId.includes("/") || profileId.includes("\\")
        || profileId.length > 256 || /[\u0000-\u001f\u007f]/.test(profileId)) {
        throw new Error("Worker profile identifier is invalid.");
      }
      const revisions: DelegationProfileRevisionView[] = [];
      const seen = new Set<string>();
      let cursor: string | null = null;
      let previousRevision = Number.POSITIVE_INFINITY;
      for (let pageNumber = 0; pageNumber < 100; pageNumber += 1) {
        const url = new URL(`/v1/delegation-profiles/${encodeURIComponent(profileId)}/revisions`, "https://operator.invalid");
        url.searchParams.set("limit", "200");
        if (cursor) url.searchParams.set("cursor", cursor);
        const response = await transport({ path: `${url.pathname}${url.search}` }, signal);
        if (!response.ok) throw new Error(`Worker profile history could not be loaded (${response.status}).`);
        const row = object(await response.json());
        if (!Array.isArray(row.items)) throw new Error("Invalid worker profile revision page.");
        for (const item of row.items) {
          const decoded = decodeRevision(item, workspaceId, profileId);
          if (decoded.revision >= previousRevision) throw new Error("Worker profile revision history is out of order.");
          previousRevision = decoded.revision;
          revisions.push(decoded);
        }
        const next = row.next_cursor;
        if (next === null || next === undefined || next === "") return revisions;
        if (typeof next !== "string" || next.length > 2048 || seen.has(next)) {
          throw new Error("Worker profile revision pagination did not advance.");
        }
        seen.add(next);
        cursor = next;
      }
      throw new Error("Worker profile history exceeds the safe paging limit.");
    },
    async getPerformance(profileId: string, taskCategory?: WorkerTaskCategory, signal?: AbortSignal): Promise<WorkerPerformanceProjection> {
      if (!workspaceId) throw new Error("A Workspace is required to load worker performance.");
      if (!profileId || profileId === "." || profileId === ".." || profileId.includes("/") || profileId.includes("\\")
        || profileId.length > 256 || /[\u0000-\u001f\u007f]/.test(profileId)) {
        throw new Error("Worker profile identifier is invalid.");
      }
      if (taskCategory !== undefined && !WORKER_TASK_CATEGORIES.includes(taskCategory)) throw new Error("Worker task category is invalid.");
      const url = new URL(`/v1/delegation-profiles/${encodeURIComponent(profileId)}/performance`, "https://operator.invalid");
      if (taskCategory) url.searchParams.set("task_category", taskCategory);
      const response = await transport({ path: `${url.pathname}${url.search}` }, signal);
      return decodePerformance(await checkedJson(response, 200), profileId, taskCategory);
    },
    async create(agentBindingId: string, revision: DelegationProfileRevisionInput, requestId: string, signal?: AbortSignal) {
      const response = await transport({
        path: "/v1/delegation-profiles",
        method: "POST",
        requestId,
        body: { workspace_id: workspaceId, agent_binding_id: agentBindingId, revision },
      }, signal);
      const saved = decodeProfile(await checkedJson(response, 201), workspaceId, agentBindingId);
      if (saved.status !== "DISABLED") throw new Error("The server did not keep the new worker profile disabled.");
      return saved;
    },
    async revise(profile: DelegationProfile, revision: DelegationProfileRevisionInput, requestId: string, signal?: AbortSignal) {
      const response = await transport({
        path: `/v1/delegation-profiles/${encodeURIComponent(profile.delegation_profile_id)}/revisions`,
        method: "POST",
        requestId,
        expectedVersion: profile.version,
        body: revision,
      }, signal);
      const saved = decodeProfile(await checkedJson(response, 201), workspaceId, profile.agent_binding_id);
      if (saved.status !== profile.status) throw new Error("Saving a revision unexpectedly changed the worker profile status.");
      return saved;
    },
    async duplicate(profile: DelegationProfile, name: string, requestId: string, signal?: AbortSignal) {
      const response = await transport({
        path: `/v1/delegation-profiles/${encodeURIComponent(profile.delegation_profile_id)}/duplicate`,
        method: "POST",
        requestId,
        expectedVersion: profile.version,
        body: { name: name.trim() },
      }, signal);
      const saved = decodeProfile(await checkedJson(response, 201), workspaceId, profile.agent_binding_id);
      if (saved.status !== "DISABLED" || saved.current_revision !== 1 || saved.delegation_profile_id === profile.delegation_profile_id) {
        throw new Error("The server did not create a new disabled worker profile.");
      }
      return saved;
    },
    async changeSafeStatus(profile: DelegationProfile, status: "DISABLED" | "ARCHIVED", requestId: string, signal?: AbortSignal) {
      const response = await transport({
        path: `/v1/delegation-profiles/${encodeURIComponent(profile.delegation_profile_id)}/status`,
        method: "POST",
        requestId,
        expectedVersion: profile.version,
        body: { status },
      }, signal);
      const saved = decodeProfile(await checkedJson(response, 200), workspaceId, profile.agent_binding_id);
      if (saved.status !== status) throw new Error("The server returned a different worker profile status than requested.");
      return saved;
    },
  };
}

export function newDelegationProfileRevision(name: string, routingDescription: string, instructions: string): DelegationProfileRevisionInput {
  return {
    name: name.trim(),
    routing_description: routingDescription.trim(),
    instructions: instructions.trim() || null,
    session_options: {},
    session_options_descriptor_digest: null,
    required_features: [],
    preferred_features: [],
    enforced_policy: {
      capability_allowlist: [],
      maximum_effect_risk: "SAFE",
      filesystem_write_scope: "WORKTREE_ONLY",
      external_effects: "DENY",
      secret_access: "NONE",
    },
    optimization_preference: "BALANCED",
    quality_floor: null,
    max_concurrency: 1,
    max_host_delegation_depth: 0,
    budget_ceiling: null,
    latency_class: "STANDARD",
    environment_policy: {
      placement_preference: "AUTO",
      isolation: "REQUIRED",
      sharing_scope: "ATTEMPT_PRIVATE",
    },
    native_delegation_policy: "INHERIT",
    warm_policy: {
      host: "COLD",
      native_session: "CLOSE_ON_SETTLE",
      capability_hosts: "COLD",
      browser_environment: "COLD",
      local_model: "PROVIDER_DEFAULT",
      ttl_ms: null,
      max_memory_bytes: null,
      max_idle_cost: null,
      triggers: [],
    },
  };
}
