import { useEffect, useMemo, useRef, useState } from "react";
import {
  newDelegationProfileRevision,
  type DelegationProfile,
} from "../coworkers/delegation-profile-api";
import { desktopDelegationProfileCatalogApi } from "../coworkers/desktop-delegation-profile-api";
import "./agent-catalog-settings.css";

export type AgentInstallation = {
  agentId: string;
  displayName: string;
  protocolCandidate: string;
  installation: "MISSING" | "INSTALLED" | "VERSION_UNAVAILABLE" | string;
  version: string | null;
  authentication: "UNKNOWN" | string;
  sessionReadiness: "NOT_PROBED" | string;
};

export type AgentProfileObservation = {
  endpointId: string;
  runtimeId: string;
  runtimeIncarnationId: string;
  compatible: boolean;
  readiness: string;
  observedAt: string;
  offerExpiresAt: string;
  constraints?: Record<string, unknown>;
};

export type AgentEndpoint = {
  endpointId: string;
  agentProfileId: string;
  protocol: string;
  topology: string;
  protocolVersion: string | null;
  capabilities: Record<string, unknown>;
};

export type AgentProfile = {
  agentProfileId: string;
  providerKey: string;
  displayName: string;
  endpoints: AgentEndpoint[];
  discoveredAt: string;
  observations: AgentProfileObservation[];
};

export type AgentBinding = {
  agentBindingId: string;
  workspaceId: string;
  agentProfileId: string;
  runtimeId: string | null;
  endpointSelectionPolicy: Record<string, unknown>;
  authRef: Record<string, unknown> | null;
  configuration: Record<string, unknown>;
  enabled: boolean;
  leadEligible: boolean;
  createdAt: string;
  version: number;
};

export type AgentCatalogSettingsProps = {
  workspaceId: string | null;
  workspaceName?: string;
  workspaceActive: boolean;
  runtimeReady: boolean;
  runtimeEnrolled: boolean;
  selectedDefaultLeadBindingId: string | null;
  installations: AgentInstallation[];
  profiles: AgentProfile[];
  bindings: AgentBinding[];
  loading?: boolean;
  error?: string | null;
  actionMessage?: string | null;
  actionError?: string | null;
  busyAction?: string | null;
  defaultAgentBusy?: boolean;
  onRefresh: () => void;
  onProbeCodex: () => void;
  onProbeOpenCode: () => void;
  onCreateBinding: (profile: AgentProfile, leadEligible: boolean) => void;
  onEnableBinding: (binding: AgentBinding) => void;
  onSetDefaultLead: (agentBindingId: string | null) => void;
};

type WorkerProfileForm = { bindingId: string; profileId: string | null; baseVersion: number | null; name: string; routing: string; instructions: string };
type WorkerProfileDuplicateForm = { profileId: string; baseVersion: number; name: string };
type WorkerProfileArchiveConfirmation = { profileId: string; baseVersion: number };

type Readiness = { label: string; tone: "good" | "caution" | "quiet"; detail: string };

type ProbeDiagnostic = {
  readiness: string;
  failure?: string;
  protocolInitialize?: string;
  accountRead?: string;
  authentication?: string;
  modelList?: string;
  modelCatalog?: string;
  modelCount?: number;
  modelCatalogTruncated?: boolean;
  sessionStart?: string;
  hostProcessStopped?: boolean;
  writerQuiescenceProven?: boolean;
  conversationEligibility?: string;
  conversationDiagnostic?: string;
  conversationBlockers?: string[];
};

type OpenCodeProviderDisplay = {
  id: string;
  displayName: string | null;
  models: Array<{ id: string; displayName: string | null }>;
};

type OpenCodeProbeDiagnostic = {
  readiness: string;
  failure?: string;
  providerStatus: string;
  connectedProviderIds: string[];
  connectedProviderIdsTruncated: boolean;
  providers: OpenCodeProviderDisplay[];
  catalogObserved: boolean;
  catalogTruncated: boolean;
};

function recordValue(value: unknown): Record<string, unknown> | null {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : null;
}

function safeOpenCodeId(value: unknown): string | null {
  return typeof value === "string"
    && value.length > 0
    && value.length <= 128
    && /^[a-zA-Z0-9._:/-]+$/.test(value)
    ? value
    : null;
}

function safeOpenCodeDisplayName(value: unknown): string | null {
  return typeof value === "string"
    && value.length > 0
    && value.length <= 128
    && !/[\u0000-\u001f\u007f]/.test(value)
    && !value.includes("://")
    && !value.toLowerCase().startsWith("www.")
    ? value
    : null;
}

function openCodeProbeDiagnostic(observation: AgentProfileObservation | undefined): OpenCodeProbeDiagnostic | null {
  const constraints = recordValue(observation?.constraints);
  if (!constraints || constraints.protocol !== "OPENCODE_SERVER") return null;
  const readiness = enumValue(constraints.readiness, ["COMPLETE", "PARTIAL", "FAILED"]);
  if (!readiness) return null;

  const connectedIdsValue = Array.isArray(constraints.reported_connected_provider_ids)
    ? constraints.reported_connected_provider_ids
    : [];
  const rawConnectedIds = connectedIdsValue.slice(0, 32);
  const connectedProviderIds = rawConnectedIds
    .map(safeOpenCodeId)
    .filter((id): id is string => id !== null);
  const providerValue = Array.isArray(constraints.configured_providers)
    ? constraints.configured_providers
    : [];
  const rawProviders = providerValue.slice(0, 32);
  let localCatalogTruncated = providerValue.length > 32;
  const providers = rawProviders.flatMap((rawProvider): OpenCodeProviderDisplay[] => {
    const provider = recordValue(rawProvider);
    const id = safeOpenCodeId(provider?.id);
    if (!provider || !id) return [];
    const modelValue = Array.isArray(provider.models) ? provider.models : [];
    if (modelValue.length > 64) localCatalogTruncated = true;
    const rawModels = modelValue.slice(0, 64);
    const models = rawModels.flatMap((rawModel) => {
      const model = recordValue(rawModel);
      const modelId = safeOpenCodeId(model?.id);
      return model && modelId
        ? [{ id: modelId, displayName: safeOpenCodeDisplayName(model.display_name) }]
        : [];
    });
    return [{ id, displayName: safeOpenCodeDisplayName(provider.display_name), models }];
  });

  return {
    readiness,
    failure: enumValue(constraints.failure, ["INVALID_ADMISSION", "HOST_UNAVAILABLE", "PROVIDER_STATUS_UNAVAILABLE", "CATALOG_UNAVAILABLE", "HOST_STOP_UNOBSERVED"]),
    providerStatus: enumValue(constraints.reported_provider_status, ["OBSERVED", "UNKNOWN", "NOT_PROBED"]) ?? "UNKNOWN",
    connectedProviderIds,
    connectedProviderIdsTruncated: constraints.reported_connected_provider_ids_truncated === true || connectedIdsValue.length > 32,
    providers,
    catalogObserved: constraints.reported_model_catalog === "OBSERVED",
    catalogTruncated: constraints.catalog_truncated === true || localCatalogTruncated,
  };
}

function enumValue(value: unknown, allowed: readonly string[]): string | undefined {
  return typeof value === "string" && allowed.includes(value) ? value : undefined;
}

function codexProbeDiagnostic(observation: AgentProfileObservation | undefined): ProbeDiagnostic | null {
  const constraints = observation?.constraints;
  if (!constraints || constraints.protocol !== "CODEX_APP_SERVER") return null;
  const readiness = enumValue(constraints.readiness, ["COMPLETE", "PARTIAL", "FAILED"]);
  if (!readiness) return null;
  const listed = Array.isArray(constraints.listed_model_options)
    ? constraints.listed_model_options.slice(0, 32)
    : [];
  const safety = recordValue(constraints.conversation_eligibility);
  const blockersValue = Array.isArray(safety?.blockers) ? safety.blockers.slice(0, 8) : [];
  return {
    readiness,
    failure: enumValue(constraints.failure, ["INVALID_ADMISSION", "HOST_UNAVAILABLE", "PROTOCOL_TIMEOUT", "PROTOCOL_REJECTED", "UNSAFE_SERVER_REQUEST", "HOST_STOP_UNOBSERVED"]),
    protocolInitialize: enumValue(constraints.protocol_initialize, ["SUPPORTED", "REJECTED", "UNKNOWN", "NOT_PROBED"]),
    accountRead: enumValue(constraints.account_read, ["SUPPORTED", "REJECTED", "UNKNOWN", "NOT_PROBED"]),
    authentication: enumValue(constraints.authentication, ["NEEDS_AUTH", "CONFIGURED", "UNKNOWN"]),
    modelList: enumValue(constraints.model_list, ["SUPPORTED", "REJECTED", "UNKNOWN", "NOT_PROBED"]),
    modelCatalog: enumValue(constraints.model_catalog, ["OBSERVED", "UNKNOWN"]),
    modelCount: listed.length,
    modelCatalogTruncated: constraints.model_catalog_truncated === true,
    sessionStart: enumValue(constraints.session_start, ["SUPPORTED", "REJECTED", "UNKNOWN", "NOT_PROBED"]),
    hostProcessStopped: typeof constraints.host_process_stopped === "boolean" ? constraints.host_process_stopped : undefined,
    writerQuiescenceProven: typeof constraints.writer_quiescence_proven === "boolean" ? constraints.writer_quiescence_proven : undefined,
    conversationEligibility: enumValue(safety?.status, ["ELIGIBLE", "NOT_ELIGIBLE"]),
    conversationDiagnostic: enumValue(safety?.diagnostic_code, ["CODEX_CONVERSATION_SAFETY_UNQUALIFIED"]),
    conversationBlockers: blockersValue.flatMap((value) => typeof value === "string" && [
      "RESTRICTED_READ_ROOTS_NOT_QUALIFIED",
      "READ_ONLY_WRITE_BOUNDARY_NOT_QUALIFIED",
      "SHELL_NETWORK_BOUNDARY_NOT_QUALIFIED",
      "NATIVE_MCP_TOOLS_NOT_DISABLED",
      "NATIVE_APP_TOOLS_NOT_DISABLED",
      "NATIVE_PLUGINS_AND_HOOKS_NOT_DISABLED",
    ].includes(value) ? [value] : []),
  };
}

function conversationBlockerLabel(value: string): string {
  switch (value) {
    case "RESTRICTED_READ_ROOTS_NOT_QUALIFIED": return "File reads are not proven to stay within an explicit allowed root.";
    case "READ_ONLY_WRITE_BOUNDARY_NOT_QUALIFIED": return "The read-only write boundary is not qualified.";
    case "SHELL_NETWORK_BOUNDARY_NOT_QUALIFIED": return "Shell network access is not proven disabled.";
    case "NATIVE_MCP_TOOLS_NOT_DISABLED": return "Native MCP tools are not proven disabled or mediated.";
    case "NATIVE_APP_TOOLS_NOT_DISABLED": return "Native app tools are not proven disabled or mediated.";
    case "NATIVE_PLUGINS_AND_HOOKS_NOT_DISABLED": return "Native plugins and hooks are not proven disabled or mediated.";
    default: return "A required Conversation safety control is not qualified.";
  }
}

function observationLabel(value: string | undefined): string {
  if (!value) return "Not reported";
  switch (value) {
    case "SUPPORTED": return "Supported by this probe";
    case "REJECTED": return "Rejected by the endpoint";
    case "UNKNOWN": return "Unknown";
    case "NOT_PROBED": return "Not tested";
    case "NEEDS_AUTH": return "Sign-in required";
    case "CONFIGURED": return "Account configuration observed";
    case "OBSERVED": return "Catalog observed";
    case "COMPLETE": return "Probe completed";
    case "PARTIAL": return "Probe partially completed";
    case "FAILED": return "Probe failed";
    default: return "Not reported";
  }
}

function failureLabel(value: string | undefined): string | undefined {
  switch (value) {
    case "INVALID_ADMISSION": return "Probe preconditions were not valid";
    case "HOST_UNAVAILABLE": return "Codex App Server could not start";
    case "PROTOCOL_TIMEOUT": return "The App Server did not respond in time";
    case "PROTOCOL_REJECTED": return "The App Server rejected a read-only request";
    case "UNSAFE_SERVER_REQUEST": return "The App Server requested an unsupported operation";
    case "HOST_STOP_UNOBSERVED": return "The probe could not confirm its host stopped";
    default: return undefined;
  }
}

function openCodeFailureLabel(value: string | undefined): string | undefined {
  switch (value) {
    case "INVALID_ADMISSION": return "Probe preconditions were not valid";
    case "HOST_UNAVAILABLE": return "OpenCode Server could not start";
    case "PROVIDER_STATUS_UNAVAILABLE": return "Provider connection status could not be read";
    case "CATALOG_UNAVAILABLE": return "Provider/model catalog could not be read";
    case "HOST_STOP_UNOBSERVED": return "The probe could not confirm its server stopped";
    default: return undefined;
  }
}

function latestObservation(profile: AgentProfile): AgentProfileObservation | undefined {
  return [...profile.observations].sort((a, b) => Date.parse(b.observedAt) - Date.parse(a.observedAt))[0];
}

function readiness(observation: AgentProfileObservation | undefined): Readiness {
  if (!observation) return { label: "No current offer", tone: "quiet", detail: "Refresh the profile through its supported adapter to request a current Runtime observation." };
  const expiry = Date.parse(observation.offerExpiresAt);
  if (!Number.isFinite(expiry) || expiry <= Date.now()) {
    return { label: "Offer expired", tone: "quiet", detail: "This observation is historical. Refresh it before using it to configure a binding." };
  }
  if (!observation.compatible) return { label: "Not compatible", tone: "caution", detail: "The Runtime did not mark this endpoint compatible for current admission." };
  switch (observation.readiness) {
    case "STARTABLE": return { label: "Can start when needed", tone: "good", detail: "The adapter reported a startable endpoint. Listing or probing did not start a work session." };
    case "AVAILABLE": return { label: "Available", tone: "good", detail: "The endpoint reported availability; this is not proof of model entitlement or Task execution support." };
    case "READY": return { label: "Ready", tone: "good", detail: "Readiness was observed, but entitlement and production-safe execution are not implied." };
    case "NEEDS_AUTH": return { label: "Sign-in needed", tone: "caution", detail: "The adapter reported that authentication is needed. LiteCowork does not read or display credentials here." };
    case "BUSY": return { label: "Busy", tone: "caution", detail: "The endpoint reported that it is busy; this screen does not queue or launch work." };
    case "DEGRADED": return { label: "Degraded", tone: "caution", detail: "The Runtime reported degraded readiness." };
    case "OFFLINE": return { label: "Offline", tone: "quiet", detail: "The Runtime or endpoint is offline." };
    default: return { label: "Unavailable", tone: "quiet", detail: "The adapter did not report an endpoint that can currently be used." };
  }
}

function freshCompatibleOffer(profile: AgentProfile): boolean {
  const offer = latestObservation(profile);
  return !!offer
    && offer.compatible
    && Number.isFinite(Date.parse(offer.offerExpiresAt))
    && Date.parse(offer.offerExpiresAt) > Date.now()
    && ["AVAILABLE", "STARTABLE", "READY"].includes(offer.readiness);
}

function formatTime(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? "Unknown" : date.toLocaleString();
}

function humanize(value: string): string {
  return value.replaceAll("_", " ").toLowerCase();
}

function capabilitySummary(profile: AgentProfile): string[] {
  const values = new Set<string>();
  for (const endpoint of profile.endpoints) {
    const session = endpoint.capabilities.session as Record<string, unknown> | undefined;
    const input = endpoint.capabilities.input as Record<string, unknown> | undefined;
    const reporting = endpoint.capabilities.reporting as Record<string, unknown> | undefined;
    if (session?.resume === true) values.add("Resume reported");
    if (session?.steer === true) values.add("Steering reported");
    if (input?.text === true) values.add("Text input reported");
    if (input?.image === true) values.add("Image input reported");
    if (reporting?.tool_calls === true) values.add("Tool-call reporting");
    if (reporting?.usage === true) values.add("Usage reporting");
  }
  return [...values];
}

export function AgentCatalogSettings(props: AgentCatalogSettingsProps) {
  const {
    workspaceId, workspaceName, workspaceActive, runtimeReady, runtimeEnrolled,
    selectedDefaultLeadBindingId, installations, profiles, bindings, loading = false,
    error, actionMessage, actionError, busyAction, defaultAgentBusy = false, onRefresh, onProbeCodex,
    onProbeOpenCode, onCreateBinding, onEnableBinding, onSetDefaultLead,
  } = props;
  const [leadEligibleByProfile, setLeadEligibleByProfile] = useState<Record<string, boolean>>({});
  const [clearDefaultPrompt, setClearDefaultPrompt] = useState<{ workspaceId: string; bindingId: string } | null>(null);
  const profileApi = useMemo(() => workspaceId ? desktopDelegationProfileCatalogApi(workspaceId) : null, [workspaceId]);
  const [workerProfiles, setWorkerProfiles] = useState<DelegationProfile[]>([]);
  const [workerProfilesError, setWorkerProfilesError] = useState<string | null>(null);
  const [workerProfilesLoading, setWorkerProfilesLoading] = useState(false);
  const [workerProfileBusy, setWorkerProfileBusy] = useState(false);
  const [workerProfileMessage, setWorkerProfileMessage] = useState<string | null>(null);
  const [workerProfileForm, setWorkerProfileForm] = useState<WorkerProfileForm | null>(null);
  const [workerProfileDuplicateForm, setWorkerProfileDuplicateForm] = useState<WorkerProfileDuplicateForm | null>(null);
  const [workerProfileArchiveConfirmation, setWorkerProfileArchiveConfirmation] = useState<WorkerProfileArchiveConfirmation | null>(null);
  const [workerProfileLifecycleBusy, setWorkerProfileLifecycleBusy] = useState<string | null>(null);
  const workerProfileGeneration = useRef(0);
  const workerProfileRequestIds = useRef(new Map<string, string>());
  useEffect(() => {
    const generation = ++workerProfileGeneration.current;
    setWorkerProfiles([]);
    setWorkerProfilesError(null);
    setWorkerProfilesLoading(false);
    setWorkerProfileBusy(false);
    setWorkerProfileMessage(null);
    setWorkerProfileForm(null);
    setWorkerProfileDuplicateForm(null);
    setWorkerProfileArchiveConfirmation(null);
    setWorkerProfileLifecycleBusy(null);
    workerProfileRequestIds.current.clear();
    if (!workspaceId || !profileApi) return;
    const controller = new AbortController();
    setWorkerProfilesLoading(true);
    void profileApi.listAll(controller.signal).then(items => {
      if (controller.signal.aborted || workerProfileGeneration.current !== generation) return;
      setWorkerProfiles(items);
      setWorkerProfilesError(null);
    }).catch(() => {
      if (controller.signal.aborted || workerProfileGeneration.current !== generation) return;
      setWorkerProfiles([]);
      setWorkerProfilesError("Worker profiles could not be loaded. Existing Coworker assignments have not been changed.");
    }).finally(() => {
      if (!controller.signal.aborted && workerProfileGeneration.current === generation) setWorkerProfilesLoading(false);
    });
    return () => controller.abort();
  }, [workspaceId, profileApi]);

  const refreshWorkerProfiles = () => {
    if (!profileApi) return;
    const generation = workerProfileGeneration.current;
    setWorkerProfilesLoading(true);
    setWorkerProfilesError(null);
    void profileApi.listAll().then(items => {
      if (generation === workerProfileGeneration.current) setWorkerProfiles(items);
    }).catch(() => {
      if (generation === workerProfileGeneration.current) setWorkerProfilesError("Worker profiles could not be refreshed. Check the Runtime and retry.");
    }).finally(() => {
      if (generation === workerProfileGeneration.current) setWorkerProfilesLoading(false);
    });
  };

  const requestIdFor = (signature: string) => {
    let requestId = workerProfileRequestIds.current.get(signature);
    if (!requestId) {
      requestId = crypto.randomUUID();
      workerProfileRequestIds.current.set(signature, requestId);
    }
    return requestId;
  };

  const updateSavedProfile = (saved: DelegationProfile) => {
    setWorkerProfiles(current => {
      const next = current.filter(item => item.delegation_profile_id !== saved.delegation_profile_id);
      return [...next, saved].sort((a, b) => a.name.localeCompare(b.name));
    });
  };

  const changeProfileStatus = (profile: DelegationProfile, status: "DISABLED" | "ARCHIVED") => {
    if (!profileApi || !workspaceId || status === profile.status) return;
    const signature = JSON.stringify({ workspaceId, operation: status, profileId: profile.delegation_profile_id, version: profile.version });
    const requestId = requestIdFor(signature);
    const generation = workerProfileGeneration.current;
    setWorkerProfilesError(null);
    setWorkerProfileMessage(null);
    setWorkerProfileLifecycleBusy(profile.delegation_profile_id);
    void profileApi.changeSafeStatus(profile, status, requestId).then(saved => {
      if (generation !== workerProfileGeneration.current) return;
      updateSavedProfile(saved);
      workerProfileRequestIds.current.delete(signature);
      setWorkerProfileArchiveConfirmation(null);
      setWorkerProfileMessage(status === "DISABLED"
        ? `${saved.name} is disabled. New worker admission is stopped; existing Attempts remain pinned.`
        : `${saved.name} is archived. Its saved history is retained.`);
    }).catch((error: unknown) => {
      if (generation !== workerProfileGeneration.current) return;
      const message = error instanceof Error ? error.message : "Worker profile status could not be changed. Refresh before retrying.";
      setWorkerProfilesError(message.slice(0, 400).replace(/[\u0000-\u001f\u007f]/g, " "));
    }).finally(() => {
      if (generation === workerProfileGeneration.current) setWorkerProfileLifecycleBusy(null);
    });
  };

  const duplicateProfile = (form: WorkerProfileDuplicateForm) => {
    if (!profileApi || !workspaceId) return;
    const source = workerProfiles.find(item => item.delegation_profile_id === form.profileId);
    if (!source || source.status === "ARCHIVED" || source.version !== form.baseVersion) {
      setWorkerProfilesError("The source profile changed. Refresh and review its latest revision before duplicating.");
      return;
    }
    const signature = JSON.stringify({ workspaceId, operation: "duplicate", profileId: source.delegation_profile_id, version: source.version, name: form.name.trim() });
    const requestId = requestIdFor(signature);
    const generation = workerProfileGeneration.current;
    setWorkerProfilesError(null);
    setWorkerProfileMessage(null);
    setWorkerProfileLifecycleBusy(source.delegation_profile_id);
    void profileApi.duplicate(source, form.name, requestId).then(saved => {
      if (generation !== workerProfileGeneration.current) return;
      updateSavedProfile(saved);
      workerProfileRequestIds.current.delete(signature);
      setWorkerProfileDuplicateForm(null);
      setWorkerProfileMessage(`Created ${saved.name} as a disabled profile. No execution history or authority was copied.`);
    }).catch((error: unknown) => {
      if (generation !== workerProfileGeneration.current) return;
      const message = error instanceof Error ? error.message : "Worker profile could not be duplicated. Refresh before retrying.";
      setWorkerProfilesError(message.slice(0, 400).replace(/[\u0000-\u001f\u007f]/g, " "));
    }).finally(() => {
      if (generation === workerProfileGeneration.current) setWorkerProfileLifecycleBusy(null);
    });
  };
  const profilesById = new Map(profiles.map((profile) => [profile.agentProfileId, profile]));
  const workspaceBindings = bindings.filter((binding) => binding.workspaceId === workspaceId);
  const profileIdsBound = new Set(workspaceBindings.map((binding) => binding.agentProfileId));
  const codexInstallation = installations.find((installation) => `${installation.agentId} ${installation.displayName} ${installation.protocolCandidate}`.toLowerCase().includes("codex"));
  const openCodeInstallation = installations.find((installation) => `${installation.agentId} ${installation.displayName} ${installation.protocolCandidate}`.toLowerCase().includes("opencode"));
  const codexProbeAvailable = codexInstallation?.installation === "INSTALLED";
  const openCodeProbeAvailable = openCodeInstallation?.installation === "INSTALLED";
  const hasWorkspace = workspaceId !== null;
  const canProbe = hasWorkspace && workspaceActive && runtimeReady && runtimeEnrolled && busyAction == null;
  const canProbeCodex = canProbe && codexProbeAvailable;
  const canProbeOpenCode = canProbe && openCodeProbeAvailable;
  const canConfigureBinding = hasWorkspace && workspaceActive && runtimeReady && runtimeEnrolled && busyAction == null;
  const defaultBinding = workspaceBindings.find((binding) => binding.agentBindingId === selectedDefaultLeadBindingId);
  const confirmingDefaultClear = clearDefaultPrompt?.workspaceId === workspaceId
    && clearDefaultPrompt.bindingId === selectedDefaultLeadBindingId;

  return (
    <section className="agent-catalog" aria-labelledby="agent-catalog-title">
      <header className="agent-catalog__header">
        <div>
          <span className="agent-catalog__eyebrow">LOCAL AGENT CATALOG</span>
          <h2 id="agent-catalog-title">Coding agents</h2>
          <p>Check what is installed, what the adapter has observed, and which agents this Workspace is configured to use.</p>
        </div>
        <button type="button" className="agent-catalog__secondary" onClick={onRefresh} disabled={!runtimeReady || loading}>
          {loading ? "Refreshing…" : "Refresh catalog"}
        </button>
      </header>

      <div className="agent-catalog__explanation" role="note">
        <strong>Three separate states</strong>
        <ol>
          <li><b>Installed</b> means a local executable returned a recognized version.</li>
          <li><b>Profile observed</b> means an adapter reported protocol and expiring Runtime readiness.</li>
          <li><b>Workspace binding</b> records whether LiteCowork may use it and whether it is lead-eligible.</li>
        </ol>
        <p>None of these starts a Task. Lead eligibility is not worker-profile enablement, and enabling a binding only permits future admission.</p>
        <p className="agent-catalog__authoring-status"><strong>Worker profiles can be saved, but cannot be enabled for execution.</strong> Create a disabled profile for an enabled Workspace binding and revise its instructions. Model/session options stay empty until an adapter negotiates and validates them. Trust admission, isolated execution, Task delegation, and worker runs remain unavailable.</p>
      </div>

      {!hasWorkspace && <p className="agent-catalog__notice">Select a Workspace to inspect its profiles and bindings.</p>}
      {hasWorkspace && (!runtimeEnrolled || !runtimeReady) && <p className="agent-catalog__notice" role="status">
        {runtimeReady
          ? `Enroll this computer in ${workspaceName ?? "the selected Workspace"} before requesting a Codex profile probe or adding a binding.`
          : "The local Operator is unavailable. Discovery, profile probes, and Workspace configuration are disabled."}
      </p>}
      {error && <p className="agent-catalog__error" role="status">{error}</p>}
      {actionError && <p className="agent-catalog__error" role="alert">{actionError}</p>}
      {actionMessage && <p className="agent-catalog__success" role="status">{actionMessage}</p>}
      {workerProfileMessage && <p className="agent-catalog__success" role="status">{workerProfileMessage}</p>}
      {workerProfilesError && <p className="agent-catalog__error" role="status">{workerProfilesError}</p>}

      <section className="agent-catalog__section" aria-labelledby="agent-installed-title">
        <div className="agent-catalog__section-heading">
          <div><h3 id="agent-installed-title">Installed on this computer</h3><p>Inventory is local and does not prove sign-in, model access, or a usable session.</p></div>
          <span className="agent-catalog__count">{installations.length}</span>
        </div>
        {installations.length === 0 ? <p className="agent-catalog__empty">No supported coding-agent executable was detected by the current inventory provider.</p> : (
          <ul className="agent-catalog__installations">
            {installations.map((installation) => (
              <li key={installation.agentId}>
                <div><strong>{installation.displayName}</strong><span>{installation.version ? `${installation.version} · ${humanize(installation.protocolCandidate)}` : humanize(installation.protocolCandidate)}</span></div>
                <div className="agent-catalog__inventory-state">
                  <span className={`agent-catalog__badge ${installation.installation === "INSTALLED" ? "is-good" : "is-muted"}`}>{humanize(installation.installation)}</span>
                  <small>Auth: unknown · session: not probed</small>
                </div>
              </li>
            ))}
          </ul>
        )}
        <p className="agent-catalog__footnote">Codex and OpenCode have explicit read-only profile probes. Other detected installations remain inventory entries until their adapters are implemented and qualified.</p>
      </section>

      <section className="agent-catalog__section" aria-labelledby="agent-probe-title">
        <div className="agent-catalog__section-heading">
          <div><h3 id="agent-probe-title">Adapter observations</h3><p>Each probe is an explicit, bounded catalog/configuration check. It is not an execution test.</p></div>
        </div>
        <div className="agent-catalog__probe-row">
          <div>
            <strong>Codex App Server profile</strong>
            <span>{codexInstallation?.installation === "INSTALLED" ? `CLI detected${codexInstallation.version ? ` · ${codexInstallation.version}` : ""}` : codexInstallation ? humanize(codexInstallation.installation) : "No Codex executable detected"}</span>
          </div>
          <button type="button" className="agent-catalog__primary" onClick={onProbeCodex} disabled={!canProbeCodex}>
            {busyAction === "probe" ? "Probing…" : "Probe Codex profile"}
          </button>
        </div>
        {!codexProbeAvailable && <p className="agent-catalog__footnote">Probe is unavailable because the local inventory has not confirmed an installed Codex executable.</p>}
        <p className="agent-catalog__footnote">A successful probe may report authentication and model-catalog metadata, but model discovery does not prove entitlement. Probe results do not establish process containment or safe Task switching.</p>
        <div className="agent-catalog__probe-row">
          <div>
            <strong>OpenCode profile</strong>
            <span>{openCodeInstallation?.installation === "INSTALLED" ? `CLI detected${openCodeInstallation.version ? ` · ${openCodeInstallation.version}` : ""}` : openCodeInstallation ? humanize(openCodeInstallation.installation) : "No OpenCode executable detected"}</span>
          </div>
          <button type="button" className="agent-catalog__primary" onClick={onProbeOpenCode} disabled={!canProbeOpenCode}>
            {busyAction === "probe:OPENCODE" ? "Probing…" : "Probe OpenCode profile"}
          </button>
        </div>
        {!openCodeProbeAvailable && <p className="agent-catalog__footnote">Probe is unavailable because the local inventory has not confirmed an installed OpenCode executable.</p>}
        <p className="agent-catalog__footnote">OpenCode probing is owner-triggered and reads only sanitized provider/model IDs and display names plus OpenCode’s reported connected-provider status. Authentication, inference access, and session model selection are unqualified. This probe starts no Task or work session; OpenCode remains incompatible for Task execution and cannot be bound for use.</p>
      </section>

      <section className="agent-catalog__section" aria-labelledby="agent-profiles-title">
        <div className="agent-catalog__section-heading">
          <div><h3 id="agent-profiles-title">Observed profiles</h3><p>Profiles describe adapter identity and advertised capabilities. They are not enabled Workspace access.</p></div>
          <span className="agent-catalog__count">{hasWorkspace ? profiles.length : 0}</span>
        </div>
        {!hasWorkspace ? null : profiles.length === 0 ? <p className="agent-catalog__empty">No adapter profile has been observed in this Workspace. Use an available explicit Codex or OpenCode probe above to request one.</p> : (
          <div className="agent-catalog__profiles">
            {profiles.map((profile) => {
              const observation = latestObservation(profile);
              const state = readiness(observation);
              const probe = profile.providerKey === "CODEX" ? codexProbeDiagnostic(observation) : null;
              const openCodeProbe = profile.providerKey === "OPENCODE" ? openCodeProbeDiagnostic(observation) : null;
              const isBound = profileIdsBound.has(profile.agentProfileId);
              const offerReady = freshCompatibleOffer(profile);
              const protocols = [...new Set(profile.endpoints.map((endpoint) => humanize(endpoint.protocol)))];
              const features = capabilitySummary(profile);
              return <article className="agent-catalog__profile" key={profile.agentProfileId}>
                <div className="agent-catalog__profile-top">
                  <div><span className="agent-catalog__eyebrow">{profile.providerKey} · ADAPTER PROFILE</span><h4>{profile.displayName}</h4><p>{protocols.join(" · ") || "Protocol not reported"}</p></div>
                  <span className={`agent-catalog__badge is-${state.tone}`}>{state.label}</span>
                </div>
                <p className="agent-catalog__readiness">{state.detail}</p>
                {observation && <dl className="agent-catalog__facts">
                  <div><dt>Observed</dt><dd>{formatTime(observation.observedAt)}</dd></div>
                  <div><dt>Offer expires</dt><dd>{formatTime(observation.offerExpiresAt)}</dd></div>
                  <div><dt>Runtime</dt><dd>{observation.runtimeId}</dd></div>
                </dl>}
                <details className="agent-catalog__capabilities">
                  <summary>Reported configuration and protocol support</summary>
                  {features.length ? <ul>{features.map((feature) => <li key={feature}>{feature}</li>)}</ul> : <p>No detailed session-option catalog is currently available for this profile.</p>}
                  <p>Model/reasoning selectors and live-session controls are not offered unless the adapter exposes and validates them. Native configuration is not rewritten.</p>
                </details>
                {probe && <details className="agent-catalog__capabilities agent-catalog__probe-diagnostics">
                  <summary>Codex probe details</summary>
                  <p>{observationLabel(probe.readiness)}{failureLabel(probe.failure) ? ` · ${failureLabel(probe.failure)}` : ""}</p>
                  <dl className="agent-catalog__probe-facts">
                    <div><dt>Protocol initialization</dt><dd>{observationLabel(probe.protocolInitialize)}</dd></div>
                    <div><dt>Account read</dt><dd>{observationLabel(probe.accountRead)}</dd></div>
                    <div><dt>Authentication</dt><dd>{observationLabel(probe.authentication)}</dd></div>
                    <div><dt>Model-list request</dt><dd>{observationLabel(probe.modelList)}</dd></div>
                    <div><dt>Model catalog</dt><dd>{probe.modelCatalog === "OBSERVED" ? `Observed · ${probe.modelCount ?? 0}${probe.modelCatalogTruncated ? "+" : ""} options` : "Unknown"}</dd></div>
                    <div><dt>Inference access</dt><dd>Not tested</dd></div>
                    <div><dt>Session start</dt><dd>{observationLabel(probe.sessionStart)}</dd></div>
                    <div><dt>Conversation turns</dt><dd>{probe.conversationEligibility === "ELIGIBLE" ? "Eligible under qualified safety controls" : "Not eligible · safety controls not qualified"}</dd></div>
                    <div><dt>Probe host</dt><dd>{probe.hostProcessStopped === true ? "Direct process stopped" : probe.hostProcessStopped === false ? "Stop not confirmed" : "Not reported"}</dd></div>
                    <div><dt>Writer quiescence</dt><dd>{probe.writerQuiescenceProven === true ? "Reported proven" : "Not proven"}</dd></div>
                  </dl>
                  {probe.conversationEligibility !== "ELIGIBLE" && <div className="agent-catalog__safety-blocker" role="note">
                    <strong>Codex Conversation turns are unavailable on this profile.</strong>
                    <ul>{(probe.conversationBlockers ?? []).map((blocker) => <li key={blocker}>{conversationBlockerLabel(blocker)}</li>)}</ul>
                    {probe.conversationDiagnostic && <small>Diagnostic: {probe.conversationDiagnostic}</small>}
                  </div>}
                  <p>These are bounded read-only observations. A listed model is not proof of entitlement or successful inference; this probe does not test a work session or safe switching.</p>
                </details>}
                {openCodeProbe && <details className="agent-catalog__capabilities agent-catalog__probe-diagnostics" open>
                  <summary>OpenCode provider and model catalog</summary>
                  <p>{observationLabel(openCodeProbe.readiness)}{openCodeFailureLabel(openCodeProbe.failure) ? ` · ${openCodeFailureLabel(openCodeProbe.failure)}` : ""}</p>
                  <div className="agent-catalog__opencode-status">
                    <strong>Reported connected-provider status</strong>
                    <span>{openCodeProbe.providerStatus === "OBSERVED" ? "OpenCode returned a connected-provider list" : openCodeProbe.providerStatus === "NOT_PROBED" ? "Not probed" : "Status unknown"}</span>
                    {openCodeProbe.connectedProviderIds.length > 0
                      ? <ul aria-label="Provider IDs reported connected by OpenCode">{openCodeProbe.connectedProviderIds.map((id) => <li key={id}><code>{id}</code></li>)}</ul>
                      : <span>No connected provider IDs were reported{openCodeProbe.connectedProviderIdsTruncated ? " in the displayed portion" : ""}.</span>}
                    {openCodeProbe.connectedProviderIdsTruncated && <small>Provider list was truncated.</small>}
                  </div>
                  <div className="agent-catalog__opencode-catalog">
                    <strong>{openCodeProbe.catalogObserved ? "Configured provider/model catalog" : "Provider/model catalog not observed"}</strong>
                    {openCodeProbe.providers.length > 0 ? <ul>{openCodeProbe.providers.map((provider) => <li key={provider.id}>
                      <strong>{provider.displayName ?? provider.id}</strong>{provider.displayName && <code>{provider.id}</code>}
                      {provider.models.length > 0 && <ul>{provider.models.map((model) => <li key={model.id}><span>{model.displayName ?? model.id}</span>{model.displayName && <code>{model.id}</code>}</li>)}</ul>}
                    </li>)}</ul> : <p>No sanitized provider/model entries were returned.</p>}
                    {openCodeProbe.catalogTruncated && <small>Catalog was truncated to the bounded display limit.</small>}
                  </div>
                  <p>“Connected” is only OpenCode’s reported status; it does not prove authentication, entitlement, or inference. Authentication, inference, and session model selection are unqualified. No Task or AgentSession is started, and this profile is incompatible with Task execution.</p>
                </details>}
                {isBound ? <p className="agent-catalog__bound">A Workspace binding exists below. A detected profile alone does not make it a lead or a worker.</p> : (
                  <div className="agent-catalog__configure">
                    <label><input type="checkbox" checked={leadEligibleByProfile[profile.agentProfileId] ?? true} onChange={(event) => setLeadEligibleByProfile((current) => ({ ...current, [profile.agentProfileId]: event.target.checked }))} /> Allow this binding to be selected as a lead</label>
                    <button type="button" className="agent-catalog__secondary" disabled={!canConfigureBinding || !offerReady} onClick={() => onCreateBinding(profile, leadEligibleByProfile[profile.agentProfileId] ?? true)}>
                      {busyAction === `create:${profile.agentProfileId}` ? "Adding…" : !offerReady ? "Refresh the offer first" : "Add disabled Workspace binding"}
                    </button>
                    <small>Adding creates a disabled binding. It does not select a default lead, create a DelegationProfile, start an agent, or run a Task.</small>
                  </div>
                )}
              </article>;
            })}
          </div>
        )}
      </section>

      {hasWorkspace && <section className="agent-catalog__section" aria-labelledby="agent-bindings-title">
        <div className="agent-catalog__section-heading"><div><h3 id="agent-bindings-title">Workspace bindings</h3><p>Bindings authorize future interaction with an agent profile. Worker profiles are configured separately below.</p></div><span className="agent-catalog__count">{workspaceBindings.length}</span></div>
        {workspaceBindings.length === 0 ? <p className="agent-catalog__empty">No profiles are configured for this Workspace.</p> : <ul className="agent-catalog__bindings">
          {workspaceBindings.map((binding) => {
            const profile = profilesById.get(binding.agentProfileId);
            const isDefaultLead = binding.agentBindingId === selectedDefaultLeadBindingId;
            const eligibleLead = binding.enabled && binding.leadEligible;
            return <li key={binding.agentBindingId}>
              <div className="agent-catalog__binding-copy">
                <strong>{profile?.displayName ?? "Profile unavailable"}</strong>
                <span>{binding.enabled ? "Enabled for future admission" : "Disabled"} · {binding.leadEligible ? "lead eligible" : "worker only"}</span>
                {isDefaultLead && <span className="agent-catalog__default">Current Workspace default lead</span>}
                <small>Configuration: {Object.keys(binding.configuration).length === 0 ? "no adapter overrides" : "adapter overrides present"} · auth reference: {binding.authRef ? "configured (value hidden)" : "none reported"}</small>
              </div>
              <div className="agent-catalog__binding-action">
                <span className={`agent-catalog__badge ${binding.enabled ? "is-good" : "is-muted"}`}>{binding.enabled ? "Enabled" : "Disabled"}</span>
                {!binding.enabled && <button type="button" className="agent-catalog__secondary" disabled={!canConfigureBinding || busyAction != null} onClick={() => onEnableBinding(binding)}>{busyAction === `enable:${binding.agentBindingId}` ? "Enabling…" : "Enable"}</button>}
                {eligibleLead && !isDefaultLead && <button type="button" className="agent-catalog__secondary" disabled={!canConfigureBinding || defaultAgentBusy} onClick={() => onSetDefaultLead(binding.agentBindingId)}>{defaultAgentBusy ? "Saving lead…" : "Set as default lead"}</button>}
                {binding.enabled && !binding.leadEligible && <span className="agent-catalog__hint">Cannot be selected as a lead</span>}
              </div>
            </li>;
          })}
        </ul>}
        <div className="agent-catalog__default-note">
          <div className="agent-catalog__default-copy">
            <strong>Workspace default lead</strong>
            <span>{defaultBinding
              ? `${profilesById.get(defaultBinding.agentProfileId)?.displayName ?? "Selected binding"} · ${defaultBinding.enabled && defaultBinding.leadEligible ? "eligible for new work" : "currently unavailable"}`
              : selectedDefaultLeadBindingId ? "Saved lead selection is unavailable in the current binding list" : "No Workspace default lead selected"}</span>
            <small>Changing or clearing this default affects future Task admission only. Active Tasks and Attempts stay pinned to their current lead. A Coworker's explicit lead remains higher priority; without any configured lead, new Task creation is rejected and the composer draft is preserved.</small>
          </div>
          {selectedDefaultLeadBindingId && !confirmingDefaultClear && <button
            type="button"
            className="agent-catalog__secondary"
            disabled={!canConfigureBinding || defaultAgentBusy}
            onClick={() => setClearDefaultPrompt({ workspaceId: workspaceId!, bindingId: selectedDefaultLeadBindingId })}
          >Clear Workspace default…</button>}
          {confirmingDefaultClear && <div className="agent-catalog__clear-default-confirm" role="group" aria-label="Confirm clearing Workspace default lead">
            <span>Clear this default for future work? Tasks without an explicit or Coworker lead will be rejected as unavailable.</span>
            <button type="button" className="agent-catalog__secondary" disabled={defaultAgentBusy} onClick={() => setClearDefaultPrompt(null)}>Keep default</button>
            <button type="button" className="agent-catalog__primary" disabled={!canConfigureBinding || defaultAgentBusy} onClick={() => { setClearDefaultPrompt(null); onSetDefaultLead(null); }}>{defaultAgentBusy ? "Clearing…" : "Confirm clear"}</button>
          </div>}
        </div>
      </section>}

      {hasWorkspace && <section className="agent-catalog__section" aria-labelledby="worker-profiles-title">
        <div className="agent-catalog__section-heading">
          <div><h3 id="worker-profiles-title">Subagent profiles</h3><p>One installed agent can have several profiles. Creating or revising a profile never starts an agent. Profiles remain disabled until adapter option validation and Trust/Environment admission are available.</p></div>
          <button type="button" className="agent-catalog__secondary" onClick={refreshWorkerProfiles} disabled={workerProfilesLoading || !runtimeReady}>{workerProfilesLoading ? "Refreshing…" : "Refresh profiles"}</button>
        </div>
        {!runtimeReady && <p className="agent-catalog__notice">Start the local Operator to manage worker profiles.</p>}
        {workerProfilesLoading && workerProfiles.length === 0 ? <p className="agent-catalog__empty">Loading saved worker profiles…</p> : workerProfiles.length === 0 ? <p className="agent-catalog__empty">No worker profiles have been created for this Workspace.</p> : <ul className="agent-catalog__worker-profiles">
          {workerProfiles.map(profile => {
            const binding = workspaceBindings.find(item => item.agentBindingId === profile.agent_binding_id);
            const agent = binding ? profilesById.get(binding.agentProfileId) : undefined;
            const archived = profile.status === "ARCHIVED";
            const duplicateForm = workerProfileDuplicateForm?.profileId === profile.delegation_profile_id ? workerProfileDuplicateForm : null;
            const archiveConfirmation = !archived && workerProfileArchiveConfirmation?.profileId === profile.delegation_profile_id ? workerProfileArchiveConfirmation : null;
            const changedSinceDuplicateForm = duplicateForm !== null && duplicateForm.baseVersion !== profile.version;
            const sourceArchived = duplicateForm !== null && archived;
            const changedSinceArchivePrompt = archiveConfirmation !== null && archiveConfirmation.baseVersion !== profile.version;
            return <li key={profile.delegation_profile_id}>
              <div className="agent-catalog__worker-profile-copy">
                <strong>{profile.name}</strong>
                <span>{agent?.displayName ?? "Agent binding unavailable"} · revision {profile.current_revision}</span>
                <span className={`agent-catalog__badge ${profile.status === "DISABLED" ? "is-muted" : profile.status === "ARCHIVED" ? "is-caution" : "is-good"}`}>
                  {profile.status === "DISABLED" ? "Disabled" : profile.status === "ARCHIVED" ? "Archived" : "Enabled for policy selection"}
                </span>
                <small>{profile.revision.routing_description}</small>
                {profile.status === "ENABLED" && <small>Execution is unavailable in this Runtime; this status does not mean a worker can run.</small>}
              </div>
              <div className="agent-catalog__worker-profile-actions">
                <button type="button" className="agent-catalog__secondary" disabled={workerProfileBusy || workerProfileLifecycleBusy !== null || !!workerProfileForm || archived || profile.status !== "DISABLED" || !profile.editable || !binding?.enabled || !workspaceActive || !runtimeEnrolled || !runtimeReady} onClick={() => setWorkerProfileForm({
                  bindingId: profile.agent_binding_id,
                  profileId: profile.delegation_profile_id,
                  baseVersion: profile.version,
                  name: profile.name,
                  routing: profile.revision.routing_description,
                  instructions: profile.revision.instructions ?? "",
                })}>{!profile.editable ? "Settings need adapter review" : profile.status !== "DISABLED" ? "Disable before editing" : "Edit future revision"}</button>
                {!archived && !duplicateForm && <button type="button" className="agent-catalog__secondary" disabled={!workspaceActive || !runtimeReady || !runtimeEnrolled || workerProfileLifecycleBusy !== null || !!workerProfileForm} onClick={() => setWorkerProfileDuplicateForm({ profileId: profile.delegation_profile_id, baseVersion: profile.version, name: `${profile.name} Copy` })}>Duplicate</button>}
                {profile.status === "ENABLED" && <button type="button" className="agent-catalog__secondary" disabled={!workspaceActive || !runtimeReady || !runtimeEnrolled || workerProfileLifecycleBusy !== null} onClick={() => changeProfileStatus(profile, "DISABLED")}>{workerProfileLifecycleBusy === profile.delegation_profile_id ? "Disabling…" : "Disable"}</button>}
                {!archived && !archiveConfirmation && <button type="button" className="agent-catalog__secondary" disabled={!workspaceActive || !runtimeReady || !runtimeEnrolled || workerProfileLifecycleBusy !== null} onClick={() => setWorkerProfileArchiveConfirmation({ profileId: profile.delegation_profile_id, baseVersion: profile.version })}>Archive…</button>}
                {!profile.editable && profile.edit_block_reason && <small>{profile.edit_block_reason}</small>}
              </div>
              {duplicateForm && <form className="agent-catalog__lifecycle-form" onSubmit={event => { event.preventDefault(); if (!changedSinceDuplicateForm && duplicateForm.name.trim()) duplicateProfile(duplicateForm); }}>
                <strong>Duplicate as a disabled profile</strong>
                <span>Copies the current non-secret settings into revision 1. Attempts, credentials, grants, and environments are never copied.</span>
                {sourceArchived ? <div className="agent-catalog__conflict" role="alert"><strong>The source profile is archived.</strong><span>Choose another active profile to duplicate.</span></div> : changedSinceDuplicateForm && <div className="agent-catalog__conflict" role="alert"><strong>The source profile changed.</strong><span>Review its latest routing description: {profile.revision.routing_description}</span><button type="button" className="agent-catalog__secondary" onClick={() => setWorkerProfileDuplicateForm({ ...duplicateForm, baseVersion: profile.version })}>Use this latest source revision</button></div>}
                <label>New profile name<input maxLength={120} value={duplicateForm.name} onChange={event => setWorkerProfileDuplicateForm({ ...duplicateForm, name: event.target.value })} /></label>
                {workerProfilesError && <p className="agent-catalog__error" role="alert">{workerProfilesError}</p>}
                <div className="agent-catalog__worker-editor-actions"><button type="button" className="agent-catalog__secondary" disabled={workerProfileLifecycleBusy === profile.delegation_profile_id} onClick={() => setWorkerProfileDuplicateForm(null)}>Cancel</button><button type="submit" className="agent-catalog__primary" disabled={!duplicateForm.name.trim() || duplicateForm.name.length > 120 || changedSinceDuplicateForm || sourceArchived || workerProfileLifecycleBusy !== null}>{workerProfileLifecycleBusy === profile.delegation_profile_id ? "Duplicating…" : "Create disabled copy"}</button></div>
              </form>}
              {archiveConfirmation && <div className="agent-catalog__lifecycle-form" role="group" aria-label={`Archive ${profile.name}`}>
                <strong>Archive this profile?</strong><span>It will stop being available for future selection. Existing Attempts retain their pinned history. The profile can no longer be revised; a duplicate can be created before archiving if you need a new working copy.</span>
                {changedSinceArchivePrompt && <div className="agent-catalog__conflict" role="alert"><strong>The profile changed since this confirmation opened.</strong><span>Current revision {profile.current_revision}: {profile.revision.routing_description}</span><button type="button" className="agent-catalog__secondary" onClick={() => setWorkerProfileArchiveConfirmation({ profileId: profile.delegation_profile_id, baseVersion: profile.version })}>Review and confirm latest revision</button></div>}
                <div className="agent-catalog__worker-editor-actions"><button type="button" className="agent-catalog__secondary" disabled={workerProfileLifecycleBusy === profile.delegation_profile_id} onClick={() => setWorkerProfileArchiveConfirmation(null)}>Cancel</button><button type="button" className="agent-catalog__primary" disabled={changedSinceArchivePrompt || workerProfileLifecycleBusy !== null} onClick={() => changeProfileStatus(profile, "ARCHIVED")}>{workerProfileLifecycleBusy === profile.delegation_profile_id ? "Archiving…" : "Confirm archive"}</button></div>
              </div>}
            </li>;
          })}
        </ul>}
        {workspaceBindings.filter(binding => binding.enabled).map(binding => {
          const agent = profilesById.get(binding.agentProfileId);
          const hasForm = workerProfileForm?.bindingId === binding.agentBindingId;
          return <div className="agent-catalog__worker-create" key={binding.agentBindingId}>
            <div><strong>{agent?.displayName ?? "Configured agent"}</strong><span>Enabled Workspace binding</span></div>
            {!hasForm && !workerProfileForm && <button type="button" className="agent-catalog__secondary" disabled={workerProfileBusy || !workspaceActive || !runtimeReady || !runtimeEnrolled} onClick={() => setWorkerProfileForm({ bindingId: binding.agentBindingId, profileId: null, baseVersion: null, name: `${agent?.displayName ?? "Agent"} Worker`, routing: "Bounded coding work for this agent.", instructions: "" })}>Create disabled worker profile</button>}
            {hasForm && <WorkerProfileEditor
              value={workerProfileForm}
              busy={workerProfileBusy}
              error={workerProfilesError}
              conflict={workerProfileForm.profileId !== null && workerProfiles.find(item => item.delegation_profile_id === workerProfileForm.profileId)?.version !== workerProfileForm.baseVersion}
              onChange={setWorkerProfileForm}
              onCancel={() => { setWorkerProfileForm(null); setWorkerProfilesError(null); }}
              onLoadLatest={() => {
                if (!workerProfileForm.profileId) return;
                const latest = workerProfiles.find(item => item.delegation_profile_id === workerProfileForm.profileId);
                if (!latest) return;
                setWorkerProfileForm({ bindingId: latest.agent_binding_id, profileId: latest.delegation_profile_id, baseVersion: latest.version, name: latest.name, routing: latest.revision.routing_description, instructions: latest.revision.instructions ?? "" });
                setWorkerProfilesError(null);
              }}
              onSave={() => {
                if (!profileApi || !workspaceId || !workerProfileForm) return;
                const form = workerProfileForm;
                const existing = form.profileId ? workerProfiles.find(item => item.delegation_profile_id === form.profileId) : undefined;
                if (form.profileId && (!existing || !existing.editable || existing.status !== "DISABLED" || existing.version !== form.baseVersion)) {
                  setWorkerProfilesError("This profile changed or is no longer editable. Refresh and review its current state.");
                  return;
                }
                const revision = newDelegationProfileRevision(form.name, form.routing, form.instructions);
                const signature = JSON.stringify({
                  workspaceId,
                  operation: form.profileId ? "revise" : "create",
                  profileId: form.profileId,
                  version: existing?.version ?? null,
                  bindingId: form.bindingId,
                  revision,
                });
                let requestId = workerProfileRequestIds.current.get(signature);
                if (!requestId) {
                  requestId = crypto.randomUUID();
                  workerProfileRequestIds.current.set(signature, requestId);
                }
                const generation = workerProfileGeneration.current;
                setWorkerProfilesError(null);
                setWorkerProfileMessage(null);
                setWorkerProfileBusy(true);
                const operation = existing
                  ? profileApi.revise(existing, revision, requestId)
                  : profileApi.create(form.bindingId, revision, requestId);
                void operation.then(saved => {
                  if (generation !== workerProfileGeneration.current) return;
                  setWorkerProfiles(current => {
                    const next = current.filter(item => item.delegation_profile_id !== saved.delegation_profile_id);
                    return [...next, saved].sort((a, b) => a.name.localeCompare(b.name));
                  });
                  workerProfileRequestIds.current.delete(signature);
                  setWorkerProfileForm(null);
                  setWorkerProfileMessage(`Saved ${saved.name} revision ${saved.current_revision}. The profile remains disabled.`);
                }).catch((error: unknown) => {
                  if (generation !== workerProfileGeneration.current) return;
                  const message = error instanceof Error ? error.message : "Worker profile could not be saved. Refresh before trying again.";
                  setWorkerProfilesError(message.slice(0, 400).replace(/[\u0000-\u001f\u007f]/g, " "));
                }).finally(() => {
                  if (generation === workerProfileGeneration.current) setWorkerProfileBusy(false);
                });
              }}
            />}
          </div>;
        })}
      </section>}

      <footer className="agent-catalog__safety">
        <strong>Execution is not integrated here.</strong>
        <span>This catalog does not start agents, create AgentSessions or Attempts, run Tasks, resume a native session, switch an active Task, or perform delegation. Those actions remain unavailable until Task, lease, Environment, Trust, process-containment, and Effect-recovery gates are qualified.</span>
      </footer>
    </section>
  );
}

type WorkerProfileEditorValue = { bindingId: string; profileId: string | null; baseVersion: number | null; name: string; routing: string; instructions: string };
function WorkerProfileEditor({ value, busy, error, conflict, onChange, onCancel, onLoadLatest, onSave }: {
  value: WorkerProfileEditorValue;
  busy: boolean;
  error: string | null;
  conflict: boolean;
  onChange: (value: WorkerProfileEditorValue | null) => void;
  onCancel: () => void;
  onLoadLatest: () => void;
  onSave: () => void;
}) {
  const canSave = value.name.trim().length > 0 && value.name.trim().length <= 120
    && value.routing.trim().length > 0 && value.routing.trim().length <= 240
    && value.instructions.length <= 12000;
  return <form className="agent-catalog__worker-editor" onSubmit={event => { event.preventDefault(); if (canSave && !busy) onSave(); }}>
    <h4>{value.profileId ? "Create an immutable revision" : "New worker profile"}</h4>
    <label>Profile name<input maxLength={120} value={value.name} disabled={busy} onChange={event => onChange({ ...value, name: event.target.value })} required /></label>
    <label>When to use<textarea maxLength={240} rows={2} value={value.routing} disabled={busy} onChange={event => onChange({ ...value, routing: event.target.value })} required /></label>
    <label>Worker instructions <span>(guidance, not a security control)</span><textarea maxLength={12000} rows={4} value={value.instructions} disabled={busy} onChange={event => onChange({ ...value, instructions: event.target.value })} /></label>
    <div className="agent-catalog__worker-editor-note"><strong>Fixed safety defaults</strong><span>No session/model overrides; one concurrent child; no recursive delegation; isolated Attempt environment and worktree writes; no external effects or secret access.</span></div>
    {conflict && <div className="agent-catalog__conflict" role="alert"><strong>A newer revision exists.</strong><span>Your current draft is preserved. Review the latest revision before you continue.</span><button type="button" className="agent-catalog__secondary" onClick={onLoadLatest} disabled={busy}>Discard draft and load latest revision</button></div>}
    {error && <p className="agent-catalog__error" role="alert">{error}</p>}
    <div className="agent-catalog__worker-editor-actions"><button type="button" className="agent-catalog__secondary" onClick={onCancel} disabled={busy}>Cancel</button><button type="submit" className="agent-catalog__primary" disabled={!canSave || busy || conflict}>{busy ? "Saving…" : value.profileId ? "Save new revision" : "Create disabled profile"}</button></div>
  </form>;
}
