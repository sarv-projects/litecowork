import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ArtifactLibrary } from "./artifacts/ArtifactLibrary";
import { TaskArtifactOutputs } from "./artifacts/TaskArtifactOutputs";
import { desktopArtifactApi } from "./artifacts/desktop-artifact-api";
import type { PinnedResourceRef } from "./artifacts/artifact-api";
import { CoworkerSettings, type CoworkerLeadBindingOption } from "./coworkers/CoworkerSettings";
import { HomeCoworkerOnboarding } from "./coworkers/HomeCoworkerOnboarding";
import { desktopCoworkerApi } from "./coworkers/desktop-coworker-api";
import { desktopDelegationProfileCatalogApi } from "./coworkers/desktop-delegation-profile-api";
import type { DelegationProfileCatalogItem } from "./coworkers/delegation-profile-api";
import type { Coworker, WorkspacePrimaryReceipt } from "./coworkers/coworker-api";
import { GoalsPage } from "./goals/GoalsPage";
import { desktopGoalApi } from "./goals/desktop-goal-api";
import { SuggestionsPage } from "./suggestions/SuggestionsPage";
import { TaskPresentationPanel } from "./presentation/TaskPresentationPanel";
import { TaskSpecRevisionHistory } from "./tasks/TaskSpecRevisionHistory";
import { AutomationsPage } from "./automations/AutomationsPage";
import { desktopAutomationApi } from "./automations/desktop-automation-api";
import { RoutinesPage } from "./routines/RoutinesPage";
import { desktopRoutineApi } from "./routines/desktop-routine-api";
import { AgentCatalogSettings } from "./agents/AgentCatalogSettings";
import type { AgentProviderKey } from "./agents/agent-catalog-api";
import { desktopAgentCatalogApi } from "./agents/desktop-agent-catalog-api";
import { ZipIntakeNotice } from "./resources/ZipIntakeNotice";
import { isSensitiveOrGenerated, resourceDisplayName, resourceFolderRelativePath, resourceIndexAction, validateResourceFileSelection } from "./resources/resource-intake";
import { ResourceRevisionEditor } from "./resources/ResourceRevisionEditor";
import { TaskResourceTextPreview } from "./resources/TaskResourceTextPreview";
import { validateResourceSourceHighlights, type ResourceSourceMatch, type SourceHighlightResult } from "./resources/source-span-highlights";
import { TaskAttentionPage } from "./needs-you/TaskAttentionPage";
import { maybeDesktopSuggestionApi } from "./suggestions/desktop-suggestion-api";
import {
  isTaskPlanningReadinessFor,
  planningReadinessNoBlockersMessage,
  type PlanningBlocker,
  type TaskPlanningReadinessView,
} from "./tasks/planning-readiness";
import { canOfferLocalRuntimeStart } from "./runtime/runtime-start-readiness";
import { ConversationsPage } from "./conversations/ConversationsPage";

type Page = "Home" | "Conversations" | "Work" | "Library" | "Needs You" | "Ideas" | "Coworkers" | "Goals" | "Routines" | "Automations" | "Settings";
type RuntimeStatus = {
  state: string;
  runtimeId: string | null;
  localIncarnationId: string | null;
  blockers: string[];
  lastShutdownClean: boolean | null;
  processRunning: boolean;
  operatorReady: boolean;
  daemonAvailable: boolean;
  detail: string | null;
};
type WorkspaceView = {
  workspaceId: string;
  name: string;
  replicationPolicy: string;
  defaultAgentBindingId: string | null;
  primaryCoworkerId?: string | null;
  status: string;
  version: number;
};
type AgentInstallationView = {
  agentId: string;
  displayName: string;
  protocolCandidate: string;
  installation: string;
  version: string | null;
  authentication: string;
  sessionReadiness: string;
};
type AgentProfileObservationView = {
  endpointId: string;
  runtimeId: string;
  runtimeIncarnationId: string;
  compatible: boolean;
  readiness: string;
  observedAt: string;
  offerExpiresAt: string;
  constraints?: Record<string, unknown>;
};
type AgentEndpointView = { endpointId: string; agentProfileId: string; protocol: string; topology: string; protocolVersion: string | null; capabilities: Record<string, unknown> };
type AgentProfileView = {
  agentProfileId: string;
  providerKey: string;
  displayName: string;
  endpoints: AgentEndpointView[];
  discoveredAt: string;
  observations: AgentProfileObservationView[];
};
type AgentProfilePageView = { items: AgentProfileView[]; nextCursor: string | null };
type AgentBindingView = {
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
type AgentBindingPageView = { items: AgentBindingView[]; nextCursor: string | null };
type RuntimeWorkspaceBindingView = {
  runtimeWorkspaceBindingId: string;
  runtimeId: string;
  workspaceId: string;
  enrollmentMode: "LOCAL_ENROLLMENT" | "MESH_PAIRING";
  status: "PENDING" | "ACTIVE" | "REVOKED";
  roles: string[];
  createdAt: string;
  activatedAt: string | null;
  revokedAt: string | null;
  version: number;
};
type ResourceView = {
  resourceId: string;
  workspaceId: string;
  resourceRevisionId: string;
  displayName: string;
  mediaType: string;
  contentDigest: string;
  sizeBytes: number;
};
type ResourceTextIndexRebuildView = {
  requestId: string;
  correlationId: string;
  workspaceId: string;
  resourceId: string;
  resourceRevisionId: string;
  contentDigest: string;
  outcome: "INDEXED" | "NOT_INDEXABLE";
  reason: "UNSUPPORTED_TYPE" | "OVER_SIZE_LIMIT" | "INVALID_UTF8" | "CONTROL_CHARACTERS" | "TERM_LIMIT_EXCEEDED" | null;
};
type WorkspaceRootView = {
  workspaceId: string;
  workspaceRootId: string;
  resourceId: string;
  displayName: string;
  watchPolicy: string;
  replicationPolicy: string;
  status: "ACTIVE" | "PAUSED" | "REVOKED" | "UNAVAILABLE";
  locationAvailability: "AVAILABLE" | "OFFLINE" | "PLACEHOLDER" | "REVOKED" | "UNKNOWN" | "UNAVAILABLE";
  version: number;
};
type WorkspaceRootPageView = { items: WorkspaceRootView[]; nextCursor: string | null };
type PinnedResourceRefView = { workspaceId: string; resourceId: string; revisionId: string };
type TaskInputSelection = PinnedResourceRefView & { displayName: string };
type ResourcePageView = { items: ResourceView[]; nextCursor: string | null };
type ResourceSearchResultView = { resourceId: string; resourceRevisionId: string; sourceContentDigest: string; sourceMatches: ResourceSourceMatch[]; displayName: string; freshness: string; matchReasons: string[]; snippet: string | null };
type ResourceSearchPreviewView = { text: string; resourceRevisionId: string; contentDigest: string };
type PreviewSearchPin = { contentDigest: string; matches: ResourceSourceMatch[] };
type ResourceContentScanView = { candidatesScanned: number; textResourcesChecked: number; skippedUnsupportedType: number; skippedOverFileLimit: number; skippedRevisionChanged: number; byteBudgetExhausted: boolean; candidateBudgetExhausted: boolean; maxCandidates: number; maxFileBytes: number; maxTotalBytes: number };
type ResourceSearchPageView = { items: ResourceSearchResultView[]; nextCursor: string | null; mode: string; contentScan: ResourceContentScanView | null };
type TaskStatus = "READY" | "RUNNING" | "WAITING_USER" | "BLOCKED" | "VERIFYING" | "NEEDS_USER" | "INCOMPLETE" | "PAUSE_REQUESTED" | "PAUSED" | "COMPLETED" | "FAILED" | "CANCEL_REQUESTED" | "CANCELLED";
type TaskSummaryView = { taskId: string; status: TaskStatus; objective: string; createdAt: string; updatedAt: string };
type TaskPageView = { items: TaskSummaryView[]; nextCursor: string | null };
type PlannedStepView = { stepId: string; logicalKey: string; title: string; objective: string; status: string };
type TaskDetailView = { taskId: string; workspaceId: string; originCoworkerId: string | null; originCoworkerRevision: number | null; currentSpecRevision: number; currentPlanRevision: number | null; status: TaskStatus; taskVersion: number; objective: string; inputRefs: PinnedResourceRefView[]; planSpecRevision: number | null; planIsStale: boolean; plannedSteps: PlannedStepView[]; createdAt: string; updatedAt: string };
type TaskSpecRevisionReceiptView = { taskId: string; revision: number; objective: string };
type UploadRangeView = { startOffset: number; endOffsetInclusive: number; sha256: string };
type ResourceUploadView = {
  uploadId: string;
  workspaceId: string;
  displayName: string;
  mediaType: string;
  expectedSizeBytes: number;
  expectedDigest: string | null;
  contextDocument: ContextDocumentCreateMetadata | null;
  folderRelativePath: string | null;
  committedResourceId: string | null;
  chunkSizeBytes: number;
  receivedRanges: UploadRangeView[];
  nextMissingOffset: number;
  state: string;
  expiresAt: string;
};
type ContextDocumentCreateMetadata = {
  kind: "WORKSPACE_NOTES";
  owner_ref: { kind: "WORKSPACE"; workspace_id: string };
};
type LocalUploadResume = {
  workspaceId: string;
  fileKey: string;
  displayName: string;
  folderRelativePath: string | null;
  mediaType: string;
  sizeBytes: number;
  lastModified: number;
  createRequestId: string;
  uploadId?: string;
  commitRequestId?: string;
  chunkRequestIds?: Record<string, string>;
};
type PendingTaskSave = {
  workspaceId: string;
  objective: string;
  leadAgentBindingId: string;
  coworkerId: string | null;
  expectedCoworkerVersion: number | null;
  inputRefs: PinnedResourceRefView[];
  requestId: string;
};
type HomeCoworkerSelection = {
  workspaceId: string;
  coworkerId: string | null;
  expectedVersion: number | null;
  name: string;
  status: "ACTIVE" | "PAUSED" | "ARCHIVED" | "WORKSPACE_DEFAULTS";
  source: "PRIMARY" | "USER" | "DEFAULTS";
};
type WorkspaceInstructionView = {
  workspaceId: string;
  revision: number;
  parentRevisions: number[];
  contentRef: { workspace_id?: string; resource_id?: string; revision_id?: string | null };
  contentDigest: string;
  authoredBy: Record<string, unknown>;
  createdAt: string;
};

const navigation: { label: Page; icon: string; group?: string }[] = [
  { label: "Conversations", icon: "▤" },
  { label: "Coworkers", icon: "◉" },
  { label: "Home", icon: "⌂" },
  { label: "Needs You", icon: "◇" },
  { label: "Work", icon: "▤" },
  { label: "Library", icon: "▧" },
  { label: "Automations", icon: "◷" },
  { label: "Routines", icon: "↻" },
  { label: "Goals", icon: "◎" },
  { label: "Ideas", icon: "✦" },
  { label: "Settings", icon: "⚙", group: "Manage" },
];

const UPLOAD_RESUME_KEY = "litecowork.resourceUploadResume.v1";
const TASK_STATUSES: TaskStatus[] = [
  "READY", "RUNNING", "WAITING_USER", "BLOCKED", "VERIFYING", "NEEDS_USER", "INCOMPLETE",
  "PAUSE_REQUESTED", "PAUSED", "COMPLETED", "FAILED", "CANCEL_REQUESTED", "CANCELLED",
];

function taskStatusLabel(status: TaskStatus): string {
  const labels: Record<TaskStatus, string> = {
    READY: "Ready", RUNNING: "Working", WAITING_USER: "Waiting for you", BLOCKED: "Blocked",
    VERIFYING: "Checking the result", NEEDS_USER: "Needs your decision", INCOMPLETE: "Not finished",
    PAUSE_REQUESTED: "Pausing safely", PAUSED: "Paused", COMPLETED: "Completed", FAILED: "Failed",
    CANCEL_REQUESTED: "Stopping", CANCELLED: "Cancelled",
  };
  return labels[status];
}

function resourceRefKey(resource: PinnedResourceRefView): string {
  return JSON.stringify([resource.workspaceId, resource.resourceId, resource.revisionId]);
}

function stepStatusLabel(status: string): string {
  const labels: Record<string, string> = {
    PENDING: "Not started", READY: "Ready", RUNNING: "Working", WAITING_USER: "Waiting for you",
    BLOCKED: "Blocked", VERIFYING: "Checking result", COMPLETED: "Completed", FAILED: "Failed",
    CANCEL_REQUESTED: "Stopping", CANCELLED: "Cancelled", SUPERSEDED: "Replaced",
  };
  return labels[status] ?? "Status unavailable";
}

function planningBlockerLabel(blocker: PlanningBlocker): string {
  const labels: Record<PlanningBlocker, string> = {
    TASK_STATE_NOT_ELIGIBLE: "The Task state is not eligible for planning.",
    PLAN_ALREADY_ACCEPTED: "A plan is already saved for this Task.",
    CURRENT_LEAD_OR_ENDPOINT_UNAVAILABLE: "The selected lead or a fresh compatible local endpoint is unavailable.",
    TASK_ISOLATION_UNAVAILABLE: "Task-specific execution isolation is not available.",
    NATIVE_CAPABILITIES_UNMEDIATED: "The agent's native capabilities are not yet mediated by LiteCowork.",
    PROTOCOL_UNQUALIFIED: "The planning protocol has not been qualified for this setup.",
    PROCESS_CONTAINMENT_UNQUALIFIED: "Process containment has not been qualified for this setup.",
    PLANNING_CONTEXT_RESOURCE_UNAVAILABLE: "Required planning context is unavailable.",
    SESSION_SETTLEMENT_UNAVAILABLE: "Safe planning-session settlement is not available.",
    PROVIDER_UNSUPPORTED: "This provider is not supported for local planning.",
  };
  return labels[blocker];
}

function workspaceRootStatusLabel(status: WorkspaceRootView["status"]): string {
  switch (status) {
    case "ACTIVE": return "Active · folder watching and indexing are not active in this build.";
    case "PAUSED": return "Paused · watching is off in this build. Identity is checked at startup; restart LiteCowork before resuming after moving or replacing this folder.";
    case "UNAVAILABLE": return "Unavailable · folder identity could not be verified at startup. Check the original folder and restart LiteCowork to retry.";
    case "REVOKED": return "Access removed.";
  }
}

function formatTaskTime(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? "Time unavailable" : date.toLocaleString();
}

function readUploadResumes(): Record<string, LocalUploadResume> {
  try {
    const parsed: unknown = JSON.parse(localStorage.getItem(UPLOAD_RESUME_KEY) ?? "{}");
    return parsed && typeof parsed === "object" && !Array.isArray(parsed) ? parsed as Record<string, LocalUploadResume> : {};
  } catch {
    return {};
  }
}

function writeUploadResume(key: string, resume: LocalUploadResume | null): void {
  try {
    const records = readUploadResumes();
    if (resume) records[key] = resume;
    else delete records[key];
    localStorage.setItem(UPLOAD_RESUME_KEY, JSON.stringify(records));
  } catch {
    // Resume metadata is a convenience only; never fail or persist file content here.
  }
}

function fileResumeKey(workspaceId: string, file: File): string {
  const relative = (file as File & { webkitRelativePath?: string }).webkitRelativePath || file.name;
  return JSON.stringify([workspaceId, relative, file.size, file.lastModified]);
}

function resourceContextDocumentMatches(
  actual: ResourceUploadView["contextDocument"] | undefined,
  expected: ContextDocumentCreateMetadata | null,
): boolean {
  if (!expected) return actual == null;
  return actual != null
    && Object.keys(actual).length === 2
    && Object.prototype.hasOwnProperty.call(actual, "kind")
    && Object.prototype.hasOwnProperty.call(actual, "owner_ref")
    && actual.kind === expected.kind
    && actual.owner_ref.kind === expected.owner_ref.kind
    && actual.owner_ref.workspace_id === expected.owner_ref.workspace_id;
}

async function digestHex(bytes: ArrayBuffer): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("");
}

function bytesToBase64(bytes: Uint8Array): string {
  let binary = "";
  for (let offset = 0; offset < bytes.length; offset += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + 0x8000));
  }
  return btoa(binary);
}

function App() {
  const [page, setPage] = useState<Page>("Home");
  const [draft, setDraft] = useState("");
  const [taskSaving, setTaskSaving] = useState(false);
  const taskSavingRef = useRef(false);
  const [taskSaveError, setTaskSaveError] = useState<string | null>(null);
  const [pendingTaskSave, setPendingTaskSave] = useState<PendingTaskSave | null>(null);
  const [homeCoworkerSelections, setHomeCoworkerSelections] = useState<Record<string, HomeCoworkerSelection>>({});
  const [taskInputSelections, setTaskInputSelections] = useState<Record<string, TaskInputSelection[]>>({});
  const [resourceNameCache, setResourceNameCache] = useState<Record<string, string>>({});
  const [runtime, setRuntime] = useState<RuntimeStatus | null>(null);
  const [runtimeBusy, setRuntimeBusy] = useState(false);
  const [runtimeError, setRuntimeError] = useState<string | null>(null);
  const [workspaces, setWorkspaces] = useState<WorkspaceView[]>([]);
  const [workspaceError, setWorkspaceError] = useState<string | null>(null);
  const [agentInstallations, setAgentInstallations] = useState<AgentInstallationView[]>([]);
  const [agentInstallationError, setAgentInstallationError] = useState<string | null>(null);
  const [agentProfiles, setAgentProfiles] = useState<AgentProfileView[]>([]);
  const [agentProfileError, setAgentProfileError] = useState<string | null>(null);
  const [agentBindings, setAgentBindings] = useState<AgentBindingView[]>([]);
  const [agentBindingError, setAgentBindingError] = useState<string | null>(null);
  const [delegationProfiles, setDelegationProfiles] = useState<DelegationProfileCatalogItem[]>([]);
  const [delegationProfileError, setDelegationProfileError] = useState<string | null>(null);
  const [delegationProfilesReady, setDelegationProfilesReady] = useState(false);
  const [runtimeBindings, setRuntimeBindings] = useState<RuntimeWorkspaceBindingView[]>([]);
  const [runtimeBindingError, setRuntimeBindingError] = useState<string | null>(null);
  const [runtimeBindingsLoading, setRuntimeBindingsLoading] = useState(false);
  const [runtimeEnrollmentBusy, setRuntimeEnrollmentBusy] = useState(false);
  const [runtimeEnrollmentMessage, setRuntimeEnrollmentMessage] = useState<string | null>(null);
  const [runtimeEnrollmentError, setRuntimeEnrollmentError] = useState<string | null>(null);
  const [agentActionMessage, setAgentActionMessage] = useState<string | null>(null);
  const [agentActionError, setAgentActionError] = useState<string | null>(null);
  const [agentActionBusy, setAgentActionBusy] = useState<string | null>(null);
  const [workspaceName, setWorkspaceName] = useState("");
  const [workspaceCreateBusy, setWorkspaceCreateBusy] = useState(false);
  const [workspacePolicyBusy, setWorkspacePolicyBusy] = useState(false);
  const [workspaceDefaultAgentBusy, setWorkspaceDefaultAgentBusy] = useState(false);
  const [workspaceInstructions, setWorkspaceInstructions] = useState<WorkspaceInstructionView[]>([]);
  const [instructionText, setInstructionText] = useState("");
  const [instructionBusy, setInstructionBusy] = useState(false);
  const [instructionMessage, setInstructionMessage] = useState<string | null>(null);
  const [latestInstructionsAfterConflict, setLatestInstructionsAfterConflict] = useState<string | null>(null);
  const [pendingInstructions, setPendingInstructions] = useState<{
    workspaceId: string;
    text: string;
    requestId: string;
    resourceRequestId: string;
    expectedVersion: number;
    parentRevisions: number[];
  } | null>(null);
  const instructionGeneration = useRef(0);
  const [pendingWorkspaceRequest, setPendingWorkspaceRequest] = useState<{ name: string; requestId: string } | null>(null);
  const [resources, setResources] = useState<ResourceView[]>([]);
  const [resourceCatalogRefreshNonce, setResourceCatalogRefreshNonce] = useState(0);
  const [resourceNextCursor, setResourceNextCursor] = useState<string | null>(null);
  const [resourcePageBusy, setResourcePageBusy] = useState(false);
  const [resourceBusy, setResourceBusy] = useState(false);
  const resourcePauseRequested = useRef(false);
  const [resourcePausePending, setResourcePausePending] = useState(false);
  const [resourceMessage, setResourceMessage] = useState<string | null>(null);
  const [previewResourceId, setPreviewResourceId] = useState<string | null>(null);
  const [previewResourceName, setPreviewResourceName] = useState<string | null>(null);
  const [resourcePreview, setResourcePreview] = useState<string | null>(null);
  const [resourcePreviewHighlights, setResourcePreviewHighlights] = useState<SourceHighlightResult | null>(null);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const previewGeneration = useRef(0);
  const resourceListGeneration = useRef(0);
  const resourceIndexRequestKeys = useRef(new Map<string, string>());
  const delegationProfileGeneration = useRef(0);
  const [openedTask, setOpenedTask] = useState<{ workspaceId: string; taskId: string } | null>(null);
  const [selectedWorkspaceId, setSelectedWorkspaceId] = useState(() => {
    try { return localStorage.getItem("litecowork.selectedWorkspaceId") ?? ""; }
    catch { return ""; }
  });
  const artifactApi = useMemo(() => desktopArtifactApi(selectedWorkspaceId), [selectedWorkspaceId]);
  const automationApi = useMemo(() => desktopAutomationApi(selectedWorkspaceId), [selectedWorkspaceId]);
  const routineApi = useMemo(() => desktopRoutineApi(selectedWorkspaceId), [selectedWorkspaceId]);
  const coworkerApi = useMemo(() => desktopCoworkerApi(selectedWorkspaceId), [selectedWorkspaceId]);
  const delegationProfileApi = useMemo(() => desktopDelegationProfileCatalogApi(selectedWorkspaceId), [selectedWorkspaceId]);
  const goalApi = useMemo(() => desktopGoalApi(selectedWorkspaceId), [selectedWorkspaceId]);
  const suggestionApi = useMemo(() => maybeDesktopSuggestionApi(selectedWorkspaceId), [selectedWorkspaceId]);
  const rootStatusRequestKeys = useRef(new Map<string, string>());
  const selectedWorkspaceIdRef = useRef(selectedWorkspaceId);
  selectedWorkspaceIdRef.current = selectedWorkspaceId;
  useEffect(() => {
    rootStatusRequestKeys.current.clear();
  }, [selectedWorkspaceId]);

  const refreshRuntime = async () => {
    try {
      const status = await invoke<RuntimeStatus>("get_runtime_status");
      setRuntime(status);
      setRuntimeError(status.detail);
    } catch {
      setRuntimeError("Desktop Runtime status is unavailable.");
    }
  };

  const refreshWorkspaces = async () => {
    try {
      const items = await invoke<WorkspaceView[]>("list_workspaces");
      setWorkspaces(items);
      setWorkspaceError(null);
    } catch {
      setWorkspaceError("Workspace list is unavailable until the local Operator API is ready.");
    }
  };

  const refreshAgentInstallations = async () => {
    try {
      setAgentInstallations(await invoke<AgentInstallationView[]>("list_agent_installations"));
      setAgentInstallationError(null);
    } catch {
      setAgentInstallationError("Installed agent inventory is unavailable until the local Operator API is ready.");
    }
  };

  const refreshAgentProfiles = async (workspaceId: string) => {
    if (!workspaceId) { setAgentProfiles([]); setAgentProfileError(null); return; }
    try {
      const page = await invoke<AgentProfilePageView>("list_agent_profiles", { workspaceId });
      setAgentProfiles(page.items);
      setAgentProfileError(null);
    } catch {
      setAgentProfiles([]);
      setAgentProfileError("Agent profiles and current Runtime offers are unavailable. Start the local Runtime and try again.");
    }
  };

  const refreshAgentBindings = async (workspaceId: string) => {
    if (!workspaceId) { setAgentBindings([]); setAgentBindingError(null); return; }
    try {
      const page = await invoke<AgentBindingPageView>("list_agent_bindings", { workspaceId });
      setAgentBindings(page.items);
      setAgentBindingError(null);
    } catch {
      setAgentBindings([]);
      setAgentBindingError("Workspace agent bindings are unavailable. Start the local Runtime and try again.");
    }
  };

  const refreshRuntimeBindings = async (workspaceId: string) => {
    if (!workspaceId) { setRuntimeBindings([]); setRuntimeBindingError(null); setRuntimeBindingsLoading(false); return; }
    setRuntimeBindingsLoading(true);
    try {
      const bindings = await invoke<RuntimeWorkspaceBindingView[]>("list_workspace_runtime_bindings", { workspaceId });
      if (selectedWorkspaceIdRef.current === workspaceId) {
        setRuntimeBindings(bindings);
        setRuntimeBindingError(null);
        setRuntimeBindingsLoading(false);
      }
    } catch {
      if (selectedWorkspaceIdRef.current === workspaceId) {
        setRuntimeBindings([]);
        setRuntimeBindingError("This computer’s Workspace enrollment could not be checked. Start the local Runtime and refresh.");
        setRuntimeBindingsLoading(false);
      }
    }
  };

  const enrollLocalRuntime = async () => {
    const workspace = workspaces.find((item) => item.workspaceId === selectedWorkspaceId);
    if (!workspace || !runtime?.runtimeId) return;
    setRuntimeEnrollmentBusy(true);
    setRuntimeEnrollmentMessage(null);
    setRuntimeEnrollmentError(null);
    try {
      await invoke<RuntimeWorkspaceBindingView>("enroll_local_runtime", {
        workspaceId: workspace.workspaceId,
        expectedWorkspaceVersion: workspace.version,
        requestId: crypto.randomUUID(),
      });
      setRuntimeEnrollmentMessage(`This computer is now enrolled in ${workspace.name}. This authorizes its local execution role but does not start an agent or execute a Task.`);
      await refreshRuntimeBindings(workspace.workspaceId);
    } catch {
      setRuntimeEnrollmentError("This computer could not be enrolled. Refresh the Workspace and Runtime status, then try again.");
    } finally {
      setRuntimeEnrollmentBusy(false);
    }
  };

  const probeAgentProfile = async (providerKey: AgentProviderKey) => {
    const action = providerKey === "CODEX" ? "probe" : "probe:OPENCODE";
    const displayName = providerKey === "CODEX" ? "Codex" : "OpenCode";
    setAgentActionBusy(action);
    setAgentActionMessage(null);
    setAgentActionError(null);
    try {
      const workspaceId = selectedWorkspaceIdRef.current;
      if (!workspaceId || !runtimeBindings.some((binding) => binding.workspaceId === workspaceId && binding.runtimeId === runtime?.runtimeId && binding.status === "ACTIVE")) {
        setAgentActionError(`Enroll this computer in the selected Workspace before probing ${displayName}.`);
        return;
      }
      const profile = providerKey === "CODEX"
        ? await desktopAgentCatalogApi.probeCodex(workspaceId)
        : await desktopAgentCatalogApi.probeOpenCode(workspaceId);
      if (profile.providerKey !== providerKey) {
        throw new Error("The local Runtime returned a different agent profile than requested.");
      }
      if (selectedWorkspaceIdRef.current !== workspaceId) return;
      setAgentProfiles((profiles) => [profile, ...profiles.filter((item) => item.agentProfileId !== profile.agentProfileId)]);
      setAgentActionMessage(providerKey === "OPENCODE"
        ? "OpenCode profile probe finished. The refreshed catalog is display-only; authentication, inference, model selection, and Task execution are not qualified."
        : "Codex profile probe completed. Review the current offer below; this does not validate model entitlement or run a Task.");
      await refreshAgentInstallations();
      if (selectedWorkspaceIdRef.current === workspaceId) await refreshAgentProfiles(workspaceId);
    } catch {
      setAgentActionError(`${displayName} profile probe could not complete. The local Runtime or probe route may be unavailable; refresh Runtime status and try again.`);
    } finally {
      setAgentActionBusy(null);
    }
  };

  const probeCodexProfile = () => probeAgentProfile("CODEX");
  const probeOpenCodeProfile = () => probeAgentProfile("OPENCODE");

  const createAgentBinding = async (profile: AgentProfileView, leadEligible: boolean) => {
    if (!selectedWorkspaceId) return;
    if (!profileHasFreshAdmissionOffer(profile)) {
      setAgentActionError("Refresh the profile and confirm a fresh compatible Runtime offer before creating a binding.");
      return;
    }
    if (!runtimeBindings.some((binding) => binding.workspaceId === selectedWorkspaceId && binding.runtimeId === runtime?.runtimeId && binding.status === "ACTIVE")) {
      setAgentActionError("Enroll this computer in the selected Workspace before creating an agent binding.");
      return;
    }
    setAgentActionBusy(`create:${profile.agentProfileId}`);
    setAgentActionMessage(null);
    setAgentActionError(null);
    try {
      await invoke<AgentBindingView>("create_agent_binding", {
        workspaceId: selectedWorkspaceId,
        agentProfileId: profile.agentProfileId,
        leadEligible,
        runtimeId: runtime?.runtimeId ?? null,
        requestId: crypto.randomUUID(),
      });
      setAgentActionMessage(`${profile.displayName} was added to this Workspace in the disabled state. Enable it separately when you are ready.`);
      await refreshAgentBindings(selectedWorkspaceId);
    } catch {
      setAgentActionError("The Workspace binding could not be created. Refresh the profile and selected Workspace, then try again.");
    } finally {
      setAgentActionBusy(null);
    }
  };

  const enableAgentBinding = async (binding: AgentBindingView) => {
    if (!runtimeBindings.some((item) => item.workspaceId === binding.workspaceId && item.runtimeId === runtime?.runtimeId && item.status === "ACTIVE")) {
      setAgentActionError("Enroll this computer in the selected Workspace before enabling an agent binding.");
      return;
    }
    setAgentActionBusy(`enable:${binding.agentBindingId}`);
    setAgentActionMessage(null);
    setAgentActionError(null);
    try {
      await invoke<AgentBindingView>("enable_agent_binding", {
        workspaceId: binding.workspaceId,
        agentBindingId: binding.agentBindingId,
        expectedVersion: binding.version,
        requestId: crypto.randomUUID(),
      });
      setAgentActionMessage("Binding enabled for future admission. This does not select a Workspace default, start an agent, or execute Tasks.");
      await refreshAgentBindings(binding.workspaceId);
    } catch {
      setAgentActionError("The binding could not be enabled. Refresh the Workspace and review its current state before retrying.");
    } finally {
      setAgentActionBusy(null);
    }
  };

  useEffect(() => {
    void refreshRuntime();
    void refreshWorkspaces();
  }, []);

  useEffect(() => {
    if ((page !== "Settings" && page !== "Coworkers") || runtime?.operatorReady !== true) {
      setAgentProfiles([]);
      setAgentBindings([]);
      setRuntimeBindings([]);
      setRuntimeBindingsLoading(false);
      return;
    }
    if (page === "Settings") void refreshAgentInstallations();
    if (!selectedWorkspaceId) {
      setAgentProfiles([]);
      setAgentBindings([]);
      setRuntimeBindings([]);
      setRuntimeBindingsLoading(false);
      setAgentProfileError(null);
      setAgentBindingError(null);
      setRuntimeBindingError(null);
      return;
    }
    void refreshAgentProfiles(selectedWorkspaceId);
    void refreshAgentBindings(selectedWorkspaceId);
    void refreshRuntimeBindings(selectedWorkspaceId);
  }, [page, runtime?.operatorReady, selectedWorkspaceId]);

  useEffect(() => {
    const generation = ++delegationProfileGeneration.current;
    setDelegationProfiles([]);
    setDelegationProfileError(null);
    setDelegationProfilesReady(false);
    if (page !== "Coworkers" || runtime?.operatorReady !== true || !selectedWorkspaceId) return;
    let active = true;
    void delegationProfileApi.listAll().then(profiles => {
      if (active && generation === delegationProfileGeneration.current && selectedWorkspaceIdRef.current === selectedWorkspaceId) {
        setDelegationProfiles(profiles);
        setDelegationProfileError(null);
        setDelegationProfilesReady(true);
      }
    }).catch(() => {
      if (active && generation === delegationProfileGeneration.current && selectedWorkspaceIdRef.current === selectedWorkspaceId) {
        setDelegationProfiles([]);
        setDelegationProfileError("Worker profiles are unavailable. Existing Coworker assignments are preserved.");
        setDelegationProfilesReady(false);
      }
    });
    return () => { active = false; };
  }, [page, runtime?.operatorReady, selectedWorkspaceId, delegationProfileApi]);

  useEffect(() => {
    if (workspaces.length > 0 && !workspaces.some((workspace) => workspace.workspaceId === selectedWorkspaceId)) {
      setSelectedWorkspaceId(workspaces[0].workspaceId);
    }
  }, [workspaces, selectedWorkspaceId]);

  useEffect(() => {
    previewGeneration.current += 1;
    const generation = ++resourceListGeneration.current;
    setPreviewResourceId(null);
    setPreviewResourceName(null);
    setResourcePreview(null);
    setResourcePreviewHighlights(null);
    setPreviewError(null);
    setResourceNextCursor(null);
    if (!selectedWorkspaceId) { setResources([]); return; }
    setResources([]);
    setResourceMessage(null);
    let active = true;
    void invoke<ResourcePageView>("list_resources", { workspaceId: selectedWorkspaceId, cursor: null }).then((page) => {
      if (active && generation === resourceListGeneration.current) {
        setResources(page.items);
        setResourceNextCursor(page.nextCursor);
      }
    }).catch(() => {
      if (active) setResourceMessage("Saved Resources are unavailable until the local Runtime is ready.");
    });
    return () => { active = false; };
  }, [selectedWorkspaceId, runtime?.operatorReady, resourceCatalogRefreshNonce]);

  useEffect(() => {
    if (resources.length === 0) return;
    setResourceNameCache((current) => {
      let changed = false;
      const next = { ...current };
      for (const resource of resources) {
        const key = resourceRefKey({ workspaceId: resource.workspaceId, resourceId: resource.resourceId, revisionId: resource.resourceRevisionId });
        if (next[key] !== resource.displayName) {
          next[key] = resource.displayName;
          changed = true;
        }
      }
      return changed ? next : current;
    });
  }, [resources]);

  useEffect(() => {
    const generation = ++instructionGeneration.current;
    setWorkspaceInstructions([]);
    setInstructionText("");
    setInstructionMessage(null);
    setLatestInstructionsAfterConflict(null);
    if (!selectedWorkspaceId) return;
    let active = true;
    void invoke<WorkspaceInstructionView[]>("list_workspace_instructions", { workspaceId: selectedWorkspaceId })
      .then(async (revisions) => {
        if (!active || generation !== instructionGeneration.current) return;
        setWorkspaceInstructions(revisions);
        const current = revisions.at(-1);
        const resourceId = current?.contentRef.resource_id;
        const revisionId = current?.contentRef.revision_id;
        if (!resourceId || !revisionId) return;
        const content = await invoke<string>("preview_resource_text", {
          workspaceId: selectedWorkspaceId,
          resourceId,
          revisionId,
        });
        if (active && generation === instructionGeneration.current) setInstructionText(content);
      })
      .catch((error) => {
        if (active && generation === instructionGeneration.current) {
          const detail = typeof error === "string" ? error : error instanceof Error ? error.message : "";
          setInstructionMessage(detail.includes("revision changed after selection")
            ? "The Resource linked to these instructions has changed. LiteCowork kept the pinned revision and did not show newer content."
            : "Workspace instructions could not be loaded. The local Runtime may still be starting.");
        }
      });
    return () => { active = false; };
  }, [selectedWorkspaceId, runtime?.operatorReady]);

  useEffect(() => {
    if (!selectedWorkspaceId) return;
    try { localStorage.setItem("litecowork.selectedWorkspaceId", selectedWorkspaceId); }
    catch { /* Workspace selection remains available for this window. */ }
  }, [selectedWorkspaceId]);

  const startRuntime = async () => {
    setRuntimeBusy(true);
    setRuntimeError(null);
    try {
      const status = await invoke<RuntimeStatus>("start_local_runtime");
      setRuntime(status);
      setRuntimeError(status.detail);
      await refreshWorkspaces();
    } catch (error) {
      setRuntimeError(typeof error === "string" ? error : "Runtime startup failed.");
      await refreshRuntime();
    } finally {
      setRuntimeBusy(false);
    }
  };

  const createWorkspace = async () => {
    const normalizedName = workspaceName.trim();
    const requestId = pendingWorkspaceRequest?.name === normalizedName
      ? pendingWorkspaceRequest.requestId
      : crypto.randomUUID();
    setPendingWorkspaceRequest({ name: normalizedName, requestId });
    setWorkspaceCreateBusy(true);
    setWorkspaceError(null);
    try {
      const created = await invoke<WorkspaceView>("create_workspace", {
        name: normalizedName,
        requestId,
      });
      setWorkspaces((current) => current.some((item) => item.workspaceId === created.workspaceId)
        ? current
        : [...current, created]);
      setSelectedWorkspaceId(created.workspaceId);
      setWorkspaceName("");
      setPendingWorkspaceRequest(null);
    } catch (error) {
      setWorkspaceError(typeof error === "string" ? error : "Workspace could not be created.");
    } finally {
      setWorkspaceCreateBusy(false);
    }
  };

  const updateHomeCoworkerSelection = useCallback((selection: HomeCoworkerSelection) => {
    setHomeCoworkerSelections((current) => ({ ...current, [selection.workspaceId]: selection }));
    setTaskSaveError(null);
  }, []);

  const saveTask = async (selection: HomeCoworkerSelection) => {
    if (taskSavingRef.current) return;
    const workspace = workspaces.find((item) => item.workspaceId === selectedWorkspaceId);
    const objective = draft.trim();
    if (!workspace || workspace.status !== "ACTIVE") {
      setTaskSaveError("Choose an active Workspace before saving this Task.");
      return;
    }
    if (runtime?.operatorReady !== true) {
      setTaskSaveError("Start the local Runtime before saving a Task.");
      return;
    }
    if (!objective || new TextEncoder().encode(objective).byteLength > 32 * 1024) {
      setTaskSaveError("Enter a task description of up to 32 KiB.");
      return;
    }
    const requestedInputs = (taskInputSelections[workspace.workspaceId] ?? [])
      .map(({ workspaceId, resourceId, revisionId }) => ({ workspaceId, resourceId, revisionId }));
    if (selection.workspaceId !== workspace.workspaceId) {
      setTaskSaveError("The Coworker selection belongs to another Workspace. Select it again before saving.");
      return;
    }
    const selectedCoworkerId = selection.coworkerId;
    const selectedCoworkerVersion = selection.expectedVersion;
    if ((selectedCoworkerId === null) !== (selectedCoworkerVersion === null)
      || (selectedCoworkerId !== null && selection.status !== "ACTIVE")) {
      setTaskSaveError("Select an active Coworker, or explicitly choose Workspace defaults, before saving this Task.");
      return;
    }
    const canReplayPending = pendingTaskSave?.workspaceId === workspace.workspaceId
      && pendingTaskSave.objective === objective
      && pendingTaskSave.coworkerId === selectedCoworkerId
      && pendingTaskSave.expectedCoworkerVersion === selectedCoworkerVersion
      && JSON.stringify(pendingTaskSave.inputRefs) === JSON.stringify(requestedInputs);
    if (pendingTaskSave && !canReplayPending) {
      setTaskSaveError("A previous save may have reached the Runtime. Open Work in its original Workspace and check before creating a separate Task.");
      return;
    }
    taskSavingRef.current = true;
    setTaskSaving(true);
    setTaskSaveError(null);
    try {
      // Preserve the original RequestId and Coworker version for a same-input
      // retry after a lost response. A changed Coworker head must not silently
      // rebase that in-flight creation onto different defaults.
      let pending: PendingTaskSave;
      if (canReplayPending && pendingTaskSave) {
        pending = pendingTaskSave;
      } else {
        const coworker = selectedCoworkerId ? await coworkerApi.get(selectedCoworkerId) : null;
        if (coworker && (coworker.workspace_id !== workspace.workspaceId || coworker.coworker_id !== selectedCoworkerId)) {
          throw new Error("The selected Coworker belongs to a different Workspace. Your draft is preserved.");
        }
        if (coworker?.status !== undefined && coworker.status !== "ACTIVE") {
          throw new Error(`The selected Coworker is ${coworker.status.toLowerCase()}. Choose an active Coworker before saving work.`);
        }
        if (coworker && coworker.version !== selectedCoworkerVersion) {
          throw new Error("The selected Coworker changed since you chose it. Refresh the selection and review its current settings before saving.");
        }
        const leadAgentBindingId = coworker?.revision.default_lead_agent_binding_id ?? workspace.defaultAgentBindingId;
        if (!leadAgentBindingId) {
          throw new Error("Choose an enabled lead agent in Settings or set one on the selected Coworker before saving a Task.");
        }
        pending = {
          workspaceId: workspace.workspaceId,
          objective,
          leadAgentBindingId,
          coworkerId: coworker?.coworker_id ?? null,
          expectedCoworkerVersion: selectedCoworkerVersion,
          inputRefs: requestedInputs,
          requestId: crypto.randomUUID(),
        };
      }
      setPendingTaskSave(pending);
      const saved = await invoke<TaskDetailView>("create_task", {
        workspaceId: pending.workspaceId,
        objective: pending.objective,
        preferredLeadAgentBindingId: pending.leadAgentBindingId,
        coworkerId: pending.coworkerId,
        expectedCoworkerVersion: pending.expectedCoworkerVersion,
        inputRefs: pending.inputRefs,
        requestId: pending.requestId,
      });
      if (saved.workspaceId !== pending.workspaceId || saved.originCoworkerId !== pending.coworkerId
        || (saved.originCoworkerId !== null) !== (saved.originCoworkerRevision !== null)
        || saved.objective !== pending.objective
        || JSON.stringify(saved.inputRefs) !== JSON.stringify(pending.inputRefs)) {
        throw new Error("The Local Runtime returned a Task that does not match this save. Your draft is preserved; open Work to inspect the committed state.");
      }
      setPendingTaskSave(null);
      setTaskInputSelections((current) => ({ ...current, [pending.workspaceId]: [] }));
      if (selectedWorkspaceIdRef.current === pending.workspaceId) {
        if (draft.trim() === pending.objective) setDraft("");
        setOpenedTask({ workspaceId: pending.workspaceId, taskId: saved.taskId });
        setPage("Work");
      }
    } catch (error) {
      setTaskSaveError(typeof error === "string" ? error : error instanceof Error ? error.message : "Task could not be saved. Retry to reconcile the same request.");
    } finally {
      taskSavingRef.current = false;
      setTaskSaving(false);
    }
  };

  const toggleTaskInput = (resource: { resourceId: string; resourceRevisionId: string; displayName: string }) => {
    if (!selectedWorkspaceId || taskSavingRef.current) return;
    setTaskSaveError(null);
    const selection: TaskInputSelection = {
      workspaceId: selectedWorkspaceId,
      resourceId: resource.resourceId,
      revisionId: resource.resourceRevisionId,
      displayName: resource.displayName,
    };
    setResourceNameCache((current) => ({ ...current, [resourceRefKey(selection)]: resource.displayName }));
    setTaskInputSelections((current) => {
      const items = current[selectedWorkspaceId] ?? [];
      const existing = items.find((item) => item.resourceId === resource.resourceId);
      const nextItems = !existing
        ? [...items, selection]
        : existing.revisionId === resource.resourceRevisionId
          ? items.filter((item) => item.resourceId !== resource.resourceId)
          : items.map((item) => item.resourceId === resource.resourceId ? selection : item);
      return { ...current, [selectedWorkspaceId]: nextItems };
    });
  };

  const removeTaskInput = (resourceId: string) => {
    setTaskInputSelections((current) => ({
      ...current,
      [selectedWorkspaceId]: (current[selectedWorkspaceId] ?? []).filter((item) => item.resourceId !== resourceId),
    }));
  };

  const clearTaskInputs = () => {
    setTaskInputSelections((current) => ({ ...current, [selectedWorkspaceId]: [] }));
    setTaskSaveError(null);
  };

  const updateWorkspacePolicy = async (replicationPolicy: string) => {
    const workspace = workspaces.find((item) => item.workspaceId === selectedWorkspaceId);
    if (!workspace || workspacePolicyBusy) return;
    setWorkspacePolicyBusy(true);
    setWorkspaceError(null);
    try {
      const updated = await invoke<WorkspaceView>("update_workspace_policy", {
        workspaceId: workspace.workspaceId,
        replicationPolicy,
        expectedVersion: workspace.version,
        requestId: crypto.randomUUID(),
      });
      setWorkspaces((items) => items.map((item) => item.workspaceId === updated.workspaceId ? updated : item));
    } catch (error) {
      setWorkspaceError(typeof error === "string" ? error : "Workspace policy could not be updated.");
      await refreshWorkspaces();
    } finally {
      setWorkspacePolicyBusy(false);
    }
  };

  const setWorkspaceDefaultAgent = async (agentBindingId: string | null) => {
    const workspace = workspaces.find((item) => item.workspaceId === selectedWorkspaceId);
    if (!workspace || workspaceDefaultAgentBusy) return;
    setWorkspaceDefaultAgentBusy(true);
    setWorkspaceError(null);
    try {
      const updated = await invoke<WorkspaceView>("set_workspace_default_agent_binding", {
        workspaceId: workspace.workspaceId,
        agentBindingId,
        expectedVersion: workspace.version,
        requestId: crypto.randomUUID(),
      });
      setWorkspaces((items) => items.map((item) => item.workspaceId === updated.workspaceId ? updated : item));
    } catch {
      setWorkspaceError("Workspace lead agent could not be updated. Refresh the Workspace and try again.");
      await refreshWorkspaces();
    } finally {
      setWorkspaceDefaultAgentBusy(false);
    }
  };

  const saveWorkspaceInstructions = async () => {
    const workspace = workspaces.find((item) => item.workspaceId === selectedWorkspaceId);
    if (!workspace || instructionBusy || workspace.status !== "ACTIVE") return;
    const generation = instructionGeneration.current;
    const size = new TextEncoder().encode(instructionText).byteLength;
    if (size > 64 * 1024) {
      setInstructionMessage("Workspace instructions are limited to 64 KiB of UTF-8 text.");
      return;
    }
    const pending = pendingInstructions?.workspaceId === workspace.workspaceId && pendingInstructions.text === instructionText
      ? pendingInstructions
      : {
          workspaceId: workspace.workspaceId,
          text: instructionText,
          requestId: crypto.randomUUID(),
          resourceRequestId: crypto.randomUUID(),
          expectedVersion: workspace.version,
          parentRevisions: workspaceInstructions.length > 0 ? [workspaceInstructions[workspaceInstructions.length - 1].revision] : [],
        };
    setPendingInstructions(pending);
    setInstructionBusy(true);
    setInstructionMessage("Saving a versioned Workspace instruction revision…");
    try {
      const bytes = new TextEncoder().encode(pending.text);
      let binary = "";
      for (let offset = 0; offset < bytes.length; offset += 0x8000) {
        binary += String.fromCharCode(...bytes.subarray(offset, offset + 0x8000));
      }
      const resource = await invoke<ResourceView>("import_resource", {
        workspaceId: workspace.workspaceId,
        displayName: "workspace-instructions.txt",
        mediaType: "text/plain; charset=utf-8",
        contentBase64: btoa(binary),
        requestId: `${pending.resourceRequestId}-instructions`,
      });
      if (generation === instructionGeneration.current && selectedWorkspaceIdRef.current === workspace.workspaceId) {
        setResources((current) => [resource, ...current.filter((item) => item.resourceId !== resource.resourceId)]);
      }
      await invoke<WorkspaceInstructionView>("create_workspace_instruction_revision", {
        workspaceId: workspace.workspaceId,
        expectedVersion: pending.expectedVersion,
        parentRevisions: pending.parentRevisions,
        resourceId: resource.resourceId,
        resourceRevisionId: resource.resourceRevisionId,
        contentDigest: resource.contentDigest,
        requestId: pending.requestId,
      });
      if (generation === instructionGeneration.current && selectedWorkspaceIdRef.current === workspace.workspaceId) {
        setPendingInstructions(null);
        setInstructionMessage("Workspace instructions saved. New Tasks can pin this revision; existing Tasks remain unchanged.");
        setLatestInstructionsAfterConflict(null);
      }
      await refreshWorkspaces();
      const revisions = await invoke<WorkspaceInstructionView[]>("list_workspace_instructions", { workspaceId: workspace.workspaceId });
      if (generation === instructionGeneration.current && selectedWorkspaceIdRef.current === workspace.workspaceId) {
        setWorkspaceInstructions(revisions);
      }
    } catch (error) {
      if (typeof error === "string" && error.includes("STALE_WORKSPACE_VERSION")) {
        if (generation === instructionGeneration.current && selectedWorkspaceIdRef.current === workspace.workspaceId) {
          setPendingInstructions(null);
          setInstructionMessage("This Workspace changed while you were editing. Your text is still here; review the latest revision, then save again.");
        }
        await refreshWorkspaces();
        try {
          const revisions = await invoke<WorkspaceInstructionView[]>("list_workspace_instructions", { workspaceId: workspace.workspaceId });
          if (generation === instructionGeneration.current && selectedWorkspaceIdRef.current === workspace.workspaceId) {
            setWorkspaceInstructions(revisions);
            const latestResourceId = revisions[revisions.length - 1]?.contentRef.resource_id;
            const latestRevisionId = revisions[revisions.length - 1]?.contentRef.revision_id;
            if (latestResourceId && latestRevisionId) {
              const latestText = await invoke<string>("preview_resource_text", {
                workspaceId: workspace.workspaceId,
                resourceId: latestResourceId,
                revisionId: latestRevisionId,
              });
              if (generation === instructionGeneration.current && selectedWorkspaceIdRef.current === workspace.workspaceId) {
                setLatestInstructionsAfterConflict(latestText);
              }
            }
          }
        } catch (reloadError) {
          const detail = typeof reloadError === "string" ? reloadError : reloadError instanceof Error ? reloadError.message : "";
          if (detail.includes("revision changed after selection")
            && generation === instructionGeneration.current
            && selectedWorkspaceIdRef.current === workspace.workspaceId) {
            setInstructionMessage("The latest saved instruction points to a Resource revision that has advanced. Newer bytes were not substituted; your draft is still here.");
          }
        }
      } else if (generation === instructionGeneration.current && selectedWorkspaceIdRef.current === workspace.workspaceId) {
        setInstructionMessage(typeof error === "string" ? error : "Workspace instructions could not be saved. Retry to safely resume the same request.");
      }
    } finally {
      setInstructionBusy(false);
    }
  };

  const importFiles = async (files: FileList | File[] | null, contextDocument: ContextDocumentCreateMetadata | null = null) => {
    const workspaceId = selectedWorkspaceIdRef.current;
    if (!files || !workspaceId) return;
    const requested = Array.from(files);
    const selected = requested.filter((file) => !isSensitiveOrGenerated(file));
    const skipped = requested.length - selected.length;
    if (selected.length === 0) {
      setResourceMessage(skipped > 0 ? "Selection contained only excluded secret or generated files." : "No files selected.");
      return;
    }
    const selectionError = validateResourceFileSelection(selected);
    if (selectionError) {
      setResourceMessage(selectionError);
      return;
    }
    setResourceBusy(true);
    resourcePauseRequested.current = false;
    setResourcePausePending(false);
    setResourceMessage(null);
    let importedCount = 0;
    let recoveredCommitCount = 0;
    let activeFileName = "";
    let pausedFileName: string | null = null;
    try {
      for (const [index, file] of selected.entries()) {
        const displayName = resourceDisplayName(file);
        activeFileName = displayName;
        if (resourcePauseRequested.current) {
          pausedFileName = displayName;
          break;
        }
        const folderRelativePath = resourceFolderRelativePath(file);
        if (displayName.length > 240) throw new Error("A selected file path is too long to save.");
        const mediaType = file.type || "application/octet-stream";
        const baseResumeKey = fileResumeKey(workspaceId, file);
        const resumeKey = contextDocument ? JSON.stringify([baseResumeKey, contextDocument]) : baseResumeKey;
        const saved = readUploadResumes()[resumeKey];
        let resume: LocalUploadResume = saved && saved.workspaceId === workspaceId && saved.displayName === displayName && (saved.folderRelativePath ?? null) === folderRelativePath && saved.mediaType === mediaType && saved.sizeBytes === file.size && saved.lastModified === file.lastModified
          ? saved
          : {
              workspaceId,
              fileKey: resumeKey,
              displayName,
              folderRelativePath,
              mediaType,
              sizeBytes: file.size,
              lastModified: file.lastModified,
              createRequestId: crypto.randomUUID(),
            };
        // Name, size, modification time, and hashes of received chunks cannot
        // identify the bytes in the chunks that have not been uploaded yet.
        setResourceMessage(`Checking “${displayName}” (${formatBytes(file.size)}) before starting or resuming its upload…`);
        const expectedDigest = `sha256:${await digestHex(await file.arrayBuffer())}`;
        if (resourcePauseRequested.current) {
          pausedFileName = displayName;
          break;
        }
        let session: ResourceUploadView | null = null;
        if (resume.uploadId) {
          try {
            session = await invoke<ResourceUploadView>("get_resource_upload", { workspaceId, uploadId: resume.uploadId });
          } catch {
            // A missing/expired server session is replaced below using a fresh idempotency key.
          }
        }
        if (resourcePauseRequested.current) {
          pausedFileName = displayName;
          break;
        }
        const sessionIdentityMatches = session
          && session.workspaceId === workspaceId
          && session.displayName === displayName
          && session.mediaType === mediaType
          && session.folderRelativePath === folderRelativePath
          && session.expectedSizeBytes === file.size
          && resourceContextDocumentMatches(session.contextDocument, contextDocument);
        const sessionMatches = sessionIdentityMatches
          && session !== null
          && session.expectedDigest === expectedDigest
          && session.chunkSizeBytes > 0
          && session.chunkSizeBytes <= 4_194_304
          && session.chunkSizeBytes <= 4 * 1024 * 1024
          && ["OPEN", "CONTENT_RECEIVED"].includes(session.state)
          && Date.parse(session.expiresAt) > Date.now();
        if (session?.state === "COMMITTED") {
          if (sessionIdentityMatches && session.expectedDigest === null) {
            const recordedResource = session.committedResourceId ? ` Recorded Resource ID: ${session.committedResourceId}.` : " Its Resource mapping is unavailable.";
            throw new Error(`An older upload is already committed, but its stored digest cannot verify these selected bytes.${recordedResource} Review the Library before uploading again.`);
          }
          // Upload TTL applies to unfinished transfer state. A committed Resource is
          // recovered through the durable commit receipt even after that TTL elapses.
          const committedIdentityMatches = sessionIdentityMatches
            && session.expectedDigest === expectedDigest;
          if (committedIdentityMatches) {
            if (resourcePauseRequested.current) {
              pausedFileName = displayName;
              break;
            }
            if (!session.committedResourceId) {
              throw new Error("This upload is marked committed, but its Resource mapping is unavailable. No duplicate upload was started; review the Workspace before retrying.");
            }
            const recovered = await invoke<ResourceView>("commit_resource_upload", {
              workspaceId,
              uploadId: session.uploadId,
              requestId: resume.commitRequestId ?? crypto.randomUUID(),
            });
            if (recovered.resourceId !== session.committedResourceId) {
              throw new Error("The recovered commit does not match the Resource recorded by the upload session.");
            }
            if (selectedWorkspaceIdRef.current === workspaceId) {
              setResources((current) => [recovered, ...current.filter((item) => item.resourceId !== recovered.resourceId)]);
            }
            writeUploadResume(resumeKey, null);
            recoveredCommitCount += 1;
            continue;
          }
        }
        let canResume = Boolean(sessionMatches);
        if (session && canResume && file.size > 0) {
          for (const range of session.receivedRanges) {
            const chunkIndex = Math.floor(range.startOffset / session.chunkSizeBytes);
            const start = chunkIndex * session.chunkSizeBytes;
            const endExclusive = Math.min(start + session.chunkSizeBytes, file.size);
            if (range.startOffset !== start || range.endOffsetInclusive + 1 !== endExclusive) {
              canResume = false;
              break;
            }
            const existingBytes = await file.slice(start, endExclusive).arrayBuffer();
            if (`sha256:${await digestHex(existingBytes)}` !== range.sha256) {
              canResume = false;
              break;
            }
          }
        }
        if (!canResume) {
          if (resourcePauseRequested.current) {
            pausedFileName = displayName;
            break;
          }
          const createRequestId = resume.uploadId ? crypto.randomUUID() : resume.createRequestId;
          resume = {
            workspaceId,
            fileKey: resumeKey,
            displayName,
            folderRelativePath,
            mediaType,
            sizeBytes: file.size,
            lastModified: file.lastModified,
            createRequestId,
          };
          writeUploadResume(resumeKey, resume);
          session = await invoke<ResourceUploadView>("create_resource_upload", {
            workspaceId,
            displayName,
            mediaType,
            sizeBytes: file.size,
            expectedDigest,
            folderRelativePath,
            contextDocument,
            requestId: resume.createRequestId,
          });
          resume.uploadId = session.uploadId;
          resume.commitRequestId = crypto.randomUUID();
          writeUploadResume(resumeKey, resume);
        } else if (session) {
          resume.uploadId = session.uploadId;
          resume.commitRequestId ||= crypto.randomUUID();
          resume.chunkRequestIds ||= {};
          writeUploadResume(resumeKey, resume);
        }
        if (!session) throw new Error("Upload session could not be created.");
        const uploaded = session.receivedRanges.reduce((sum, range) => sum + range.endOffsetInclusive - range.startOffset + 1, 0);
        setResourceMessage(`Adding file ${index + 1} of ${selected.length}: ${displayName} · ${formatBytes(uploaded)} of ${formatBytes(file.size)} uploaded`);
        const receivedIndexes = new Set(session.receivedRanges.map((range) => Math.floor(range.startOffset / session!.chunkSizeBytes)));
        const chunkSize = session.chunkSizeBytes;
        for (let offset = 0, chunkIndex = 0; offset < file.size; offset += chunkSize, chunkIndex += 1) {
          if (resourcePauseRequested.current) {
            pausedFileName = displayName;
            break;
          }
          const endExclusive = Math.min(offset + chunkSize, file.size);
          if (receivedIndexes.has(chunkIndex)) continue;
          const chunk = new Uint8Array(await file.slice(offset, endExclusive).arrayBuffer());
          const sha256 = await digestHex(chunk.buffer as ArrayBuffer);
          if (resourcePauseRequested.current) {
            pausedFileName = displayName;
            break;
          }
          resume.chunkRequestIds ||= {};
          resume.chunkRequestIds[String(chunkIndex)] ||= crypto.randomUUID();
          writeUploadResume(resumeKey, resume);
          await invoke("upload_resource_chunk", {
            workspaceId,
            uploadId: session.uploadId,
            chunkIndex,
            contentRange: `bytes ${offset}-${endExclusive - 1}/${file.size}`,
            chunkSha256: sha256,
            contentBase64: bytesToBase64(chunk),
            requestId: resume.chunkRequestIds[String(chunkIndex)],
          });
          setResourceMessage(`Adding file ${index + 1} of ${selected.length}: ${displayName} · ${formatBytes(endExclusive)} of ${formatBytes(file.size)} uploaded`);
          if (resourcePauseRequested.current) {
            pausedFileName = displayName;
            break;
          }
        }
        if (pausedFileName) break;
        if (resourcePauseRequested.current) {
          pausedFileName = displayName;
          break;
        }
        const commitRequestId = resume.commitRequestId ?? crypto.randomUUID();
        resume.commitRequestId = commitRequestId;
        writeUploadResume(resumeKey, resume);
        const imported = await invoke<ResourceView>("commit_resource_upload", {
          workspaceId,
          uploadId: session.uploadId,
          requestId: commitRequestId,
        });
        if (selectedWorkspaceIdRef.current === workspaceId) {
          setResources((current) => [imported, ...current.filter((item) => item.resourceId !== imported.resourceId)]);
        }
        writeUploadResume(resumeKey, null);
        importedCount += 1;
      }
      const completed = importedCount + recoveredCommitCount;
      if (pausedFileName) {
        const completedSummary = completed > 0 ? `${completed} ${completed === 1 ? "file was" : "files were"} saved. ` : "";
        setResourceMessage(`${completedSummary}Upload paused before finishing “${pausedFileName}”. Any accepted chunks remain resumable until their session expires. Choose the same unchanged file again to continue.${skipped > 0 ? ` Excluded ${skipped} likely secret or generated ${skipped === 1 ? "file" : "files"}.` : ""}`);
      } else {
        setResourceMessage(`${completed} ${completed === 1 ? "file" : "files"} added to this Workspace.${recoveredCommitCount > 0 ? ` Recovered ${recoveredCommitCount} earlier commit${recoveredCommitCount === 1 ? "" : "s"}.` : ""}${skipped > 0 ? ` Excluded ${skipped} likely secret or generated ${skipped === 1 ? "file" : "files"}.` : ""}`);
      }
    } catch (error) {
      const detail = typeof error === "string" ? error : error instanceof Error ? error.message : "Resource intake failed.";
      const completedCount = importedCount + recoveredCommitCount;
      const completedNoun = completedCount === 1 ? "file was" : "files were";
      const completedSummary = completedCount > 0
        ? `${completedCount} of ${selected.length} selected ${completedNoun} saved before import stopped${recoveredCommitCount > 0 ? ` (including ${recoveredCommitCount} earlier committed upload${recoveredCommitCount === 1 ? "" : "s"} recovered)` : ""}. `
        : "No selected files were confirmed saved. ";
      const resumeHint = activeFileName
        ? ` If an upload session was already created for “${activeFileName}”, choose the same unchanged file again to resume it while the session remains valid.`
        : "";
      const excludedSummary = skipped > 0 ? ` ${skipped} likely secret or generated ${skipped === 1 ? "file was" : "files were"} excluded.` : "";
      setResourceMessage(`${completedSummary}${detail}${resumeHint}${excludedSummary}`);
    } finally {
      resourcePauseRequested.current = false;
      setResourcePausePending(false);
      setResourceBusy(false);
    }
  };

  const pauseResourceImport = () => {
    if (!resourceBusy || resourcePauseRequested.current) return;
    resourcePauseRequested.current = true;
    setResourcePausePending(true);
    setResourceMessage("Pause requested. Any in-flight request will finish; no later upload request or file will start.");
  };

  const loadMoreResources = async () => {
    const workspaceId = selectedWorkspaceIdRef.current;
    const cursor = resourceNextCursor;
    const generation = resourceListGeneration.current;
    if (!workspaceId || !cursor || resourcePageBusy) return;
    setResourcePageBusy(true);
    setResourceMessage(null);
    try {
      const page = await invoke<ResourcePageView>("list_resources", { workspaceId, cursor });
      if (generation !== resourceListGeneration.current || workspaceId !== selectedWorkspaceIdRef.current) return;
      const seen = new Set(resources.map((item) => item.resourceId));
      if (page.items.some((item) => seen.has(item.resourceId)) || page.nextCursor === cursor) {
        throw new Error("Resource catalog returned a repeated page or cursor.");
      }
      setResources((current) => {
        const existing = new Set(current.map((item) => item.resourceId));
        return [...current, ...page.items.filter((item) => !existing.has(item.resourceId))];
      });
      setResourceNextCursor(page.nextCursor);
    } catch (error) {
      if (generation === resourceListGeneration.current && workspaceId === selectedWorkspaceIdRef.current) {
        setResourceMessage(typeof error === "string" ? error : error instanceof Error ? error.message : "More Resources could not be loaded.");
      }
    } finally {
      setResourcePageBusy(false);
    }
  };

  const previewText = async (resourceId: string, displayName: string, revisionId: string, searchPin?: PreviewSearchPin) => {
    if (!selectedWorkspaceId) return;
    const generation = ++previewGeneration.current;
    setPreviewResourceId(resourceId);
    setPreviewResourceName(displayName);
    setResourcePreview(null);
    setResourcePreviewHighlights(null);
    setPreviewError(null);
    try {
      if (searchPin && searchPin.matches.length > 0) {
        try {
          const preview = await invoke<ResourceSearchPreviewView>("preview_resource_text_with_provenance", {
            workspaceId: selectedWorkspaceId,
            resourceId,
            revisionId,
            expectedContentDigest: searchPin.contentDigest,
          });
          const validation = await validateResourceSourceHighlights({
            preview,
            expectedRevisionId: revisionId,
            expectedContentDigest: searchPin.contentDigest,
            matches: searchPin.matches,
          });
          if (generation === previewGeneration.current) {
            setResourcePreview(preview.text);
            setResourcePreviewHighlights(validation);
          }
          return;
        } catch {
          // A provenance mismatch may still permit a plain exact-revision preview.
          // The ordinary path never applies search spans or claims a match.
          const plainText = await invoke<string>("preview_resource_text", {
            workspaceId: selectedWorkspaceId,
            resourceId,
            revisionId,
          });
          if (generation === previewGeneration.current) {
            setResourcePreview(plainText);
            setResourcePreviewHighlights({ kind: "plain", reason: "Search matches could not be verified against this exact preview." });
          }
          return;
        }
      }
      const content = await invoke<string>("preview_resource_text", {
        workspaceId: selectedWorkspaceId,
        resourceId,
        revisionId,
      });
      if (generation === previewGeneration.current) setResourcePreview(content);
    } catch (error) {
      if (generation === previewGeneration.current) {
        setPreviewError(typeof error === "string" ? error : "Text preview is unavailable.");
        setResourcePreviewHighlights(null);
      }
    }
  };

  const openArtifactSource = (ref: PinnedResourceRef) => {
    if (ref.workspace_id !== selectedWorkspaceIdRef.current) {
      setResourceMessage("This Artifact source belongs to another Workspace. Select that Workspace to open it.");
      return;
    }
    const key = resourceRefKey({ workspaceId: ref.workspace_id, resourceId: ref.resource_id, revisionId: ref.revision_id });
    const displayName = resources.find((resource) => resource.resourceId === ref.resource_id && resource.resourceRevisionId === ref.revision_id)?.displayName
      ?? resourceNameCache[key]
      ?? `Resource ${ref.resource_id}`;
    void previewText(ref.resource_id, displayName, ref.revision_id);
  };

  const pendingTaskMatchesComposer = Boolean(pendingTaskSave)
    && pendingTaskSave?.workspaceId === selectedWorkspaceId
    && pendingTaskSave.objective === draft.trim()
    && pendingTaskSave.coworkerId === homeCoworkerSelections[selectedWorkspaceId]?.coworkerId
    && pendingTaskSave.expectedCoworkerVersion === homeCoworkerSelections[selectedWorkspaceId]?.expectedVersion
    && JSON.stringify(pendingTaskSave.inputRefs) === JSON.stringify((taskInputSelections[selectedWorkspaceId] ?? [])
      .map(({ workspaceId, resourceId, revisionId }) => ({ workspaceId, resourceId, revisionId })));
  const openPendingTaskWorkspace = () => {
    if (!pendingTaskSave) return;
    setSelectedWorkspaceId(pendingTaskSave.workspaceId);
    setOpenedTask(null);
    setPage("Work");
  };
  const clearPendingTaskAfterReview = () => {
    if (!pendingTaskSave) return;
    const accepted = window.confirm("If the earlier save committed, starting another Task can create a duplicate. Clear its retry identity only after checking Work in the original Workspace. Continue?");
    if (!accepted) return;
    setPendingTaskSave(null);
    setTaskSaveError("Original retry cleared after your review. The next save uses the current Coworker and lead settings.");
  };

  return (
    <div className="app-shell">
      <aside className="sidebar" aria-label="Main navigation">
        <a className="brand" href="#home" onClick={(event) => { event.preventDefault(); setPage("Home"); }}>
          <span className="brand-mark" aria-hidden="true">L</span>
          <span>LiteCowork</span>
        </a>

        <div className="workspace-switcher" aria-label={`Local Runtime ${runtime?.state ?? "status unknown"}; Operator API ${runtime?.operatorReady ? "serving" : "not ready"}`}>
          <span className={`workspace-dot ${runtime?.operatorReady ? "online" : ""}`} aria-hidden="true" />
          <span className="workspace-label">
            <strong>Desktop</strong>
            {workspaces.length > 1 ? (
              <select aria-label="Selected Workspace" value={selectedWorkspaceId} disabled={taskSaving} onChange={(event) => setSelectedWorkspaceId(event.target.value)}>
                {workspaces.map((workspace) => <option key={workspace.workspaceId} value={workspace.workspaceId}>{workspace.name}</option>)}
              </select>
            ) : <small>{workspaces[0]?.name ?? `No Workspace · ${runtime?.state ?? "Checking"}`}</small>}
          </span>
          {workspaces.length > 1 && <span className="chevron" aria-hidden="true">⌄</span>}
        </div>

        <nav className="nav-list">
          {navigation.map((item) => (
            <div key={item.label}>
              {item.group && <div className="nav-group-label">{item.group}</div>}
              <button
                className={`nav-item ${page === item.label ? "selected" : ""}`}
                type="button"
                disabled={taskSaving}
                aria-current={page === item.label ? "page" : undefined}
                onClick={() => { if (item.label === "Work") setOpenedTask(null); setPage(item.label); }}
              >
                <span className="nav-icon" aria-hidden="true">{item.icon}</span>
                <span>{item.label}</span>
              </button>
            </div>
          ))}
        </nav>

        <div className="sidebar-bottom">
          <div className="connection-note" role="status">
            <span className={`connection-dot ${runtime?.state === "READY" ? "online" : ""}`} aria-hidden="true" />
            <span>Runtime {runtime?.state?.toLowerCase().replaceAll("_", " ") ?? "status unknown"}</span>
          </div>
          <div className="user-row">
            <span className="user-avatar" aria-hidden="true">S</span>
            <span className="user-meta"><strong>Local desktop</strong><small>Personal Workspace</small></span>
            <button className="icon-button more-button" aria-label="More account options" type="button">···</button>
          </div>
        </div>
      </aside>

      <main className="main-area">
        <header className="topbar">
          <div className="breadcrumbs"><span>LiteCowork</span><span className="crumb-separator">/</span><strong>{page}</strong></div>
          <div className="topbar-actions">
            <button className="icon-button help-button" type="button" aria-label="Help">?</button>
          </div>
        </header>

        {page === "Home" ? (
          <HomePage draft={draft} onDraftChange={(value) => { setDraft(value); setTaskSaveError(null); }} runtime={runtime} runtimeError={runtimeError} runtimeBusy={runtimeBusy} startRuntime={startRuntime} workspace={workspaces.find((workspace) => workspace.workspaceId === selectedWorkspaceId) ?? null} coworkerApi={coworkerApi} coworkerSelection={homeCoworkerSelections[selectedWorkspaceId] ?? null} onCoworkerSelectionChange={updateHomeCoworkerSelection} taskInputs={taskInputSelections[selectedWorkspaceId] ?? []} onRemoveTaskInput={removeTaskInput} onOpenLibrary={() => setPage("Library")} operatorReady={runtime?.operatorReady === true} taskSaving={taskSaving} taskSaveError={taskSaveError} hasPendingTaskSave={pendingTaskSave !== null} pendingTaskRetryMatches={pendingTaskMatchesComposer} onOpenPendingTask={openPendingTaskWorkspace} onClearPendingTaskAfterReview={clearPendingTaskAfterReview} onSaveTask={saveTask} onSettings={() => setPage("Settings")} onOpenCoworkers={() => setPage("Coworkers")} onWorkspaceUpdated={(updated) => setWorkspaces(current => current.map(item => item.workspaceId === updated.workspace_id && item.version <= updated.version ? { ...item, primaryCoworkerId: updated.primary_coworker_id, version: updated.version } : item))} onOpenWork={() => { setOpenedTask(null); setPage("Work"); }} onOpenTask={(taskId) => { setOpenedTask({ workspaceId: selectedWorkspaceId, taskId }); setPage("Work"); }} />
        ) : page === "Conversations" ? (
          <ConversationsPage key={selectedWorkspaceId} workspaceId={selectedWorkspaceId} workspaceName={workspaces.find((workspace) => workspace.workspaceId === selectedWorkspaceId)?.name ?? "Workspace"} operatorReady={runtime?.operatorReady === true && Boolean(selectedWorkspaceId)} />
        ) : page === "Work" ? (
          <WorkPage key={selectedWorkspaceId} selectedWorkspaceId={selectedWorkspaceId} workspaceName={workspaces.find((workspace) => workspace.workspaceId === selectedWorkspaceId)?.name ?? "Workspace"} resourceNameCache={resourceNameCache} artifactApi={artifactApi} coworkerApi={coworkerApi} operatorReady={runtime?.operatorReady === true} initialTaskId={openedTask?.workspaceId === selectedWorkspaceId ? openedTask.taskId : null} />
        ) : page === "Library" ? (
          <LibraryPage selectedWorkspaceId={selectedWorkspaceId} artifactApi={artifactApi} onOpenArtifactSource={openArtifactSource} rootStatusRequestKeys={rootStatusRequestKeys} resourceIndexRequestKeys={resourceIndexRequestKeys} workspaceVersion={workspaces.find((workspace) => workspace.workspaceId === selectedWorkspaceId)?.version ?? null} operatorReady={runtime?.operatorReady === true} resources={resources} onRevisionCommitted={(updated) => setResources((current) => current.map((item) => item.resourceId === updated.resourceId ? updated : item))} onRefreshResources={() => setResourceCatalogRefreshNonce((current) => current + 1)} selectedTaskInputs={taskInputSelections[selectedWorkspaceId] ?? []} onToggleTaskInput={toggleTaskInput} onClearTaskInputs={clearTaskInputs} onUseSelectedInTask={() => setPage("Home")} nextCursor={resourceNextCursor} pageBusy={resourcePageBusy} onLoadMore={loadMoreResources} busy={resourceBusy} pausePending={resourcePausePending} onPauseImport={pauseResourceImport} message={resourceMessage} onFiles={importFiles} onPreview={previewText} onClosePreview={() => { previewGeneration.current += 1; setPreviewResourceId(null); setPreviewResourceName(null); setResourcePreview(null); setResourcePreviewHighlights(null); setPreviewError(null); }} previewResourceId={previewResourceId} previewResourceName={previewResourceName} preview={resourcePreview} previewHighlights={resourcePreviewHighlights} previewError={previewError} />
        ) : page === "Needs You" ? (
          <TaskAttentionPage key={selectedWorkspaceId} workspaceId={selectedWorkspaceId} operatorReady={runtime?.operatorReady === true} onOpenTask={(taskId) => { setOpenedTask({ workspaceId: selectedWorkspaceId, taskId }); setPage("Work"); }} />
        ) : page === "Coworkers" && runtime?.operatorReady !== true && selectedWorkspaceId ? (
          <div className="page-content subpage-content"><div className="eyebrow">WORKSPACE</div><h1>Coworkers</h1><section className="subpage-panel empty-panel"><div className="empty-icon" aria-hidden="true">◉</div><h2>Local Operator unavailable</h2><p>Start the local Runtime before loading or changing Coworker settings. No Coworker state is cached in this view.</p><button className="text-button" type="button" onClick={() => setPage("Settings")}>Open Runtime settings <span aria-hidden="true">→</span></button></section></div>
        ) : page === "Coworkers" ? (
          <CoworkerSettings
            api={coworkerApi}
            workspaceId={selectedWorkspaceId}
            workspaceVersion={workspaces.find((workspace) => workspace.workspaceId === selectedWorkspaceId)?.version ?? null}
            leadBindings={runtime?.operatorReady === true && agentBindingError === null ? agentBindings.filter(binding => binding.workspaceId === selectedWorkspaceId).map(binding => ({
              agent_binding_id: binding.agentBindingId,
              display_name: agentProfiles.find(profile => profile.agentProfileId === binding.agentProfileId)?.displayName ?? `Agent binding ${binding.agentBindingId}`,
              enabled: binding.enabled,
              lead_eligible: binding.leadEligible,
            } satisfies CoworkerLeadBindingOption)) : undefined}
            workerProfiles={runtime?.operatorReady === true && delegationProfilesReady && delegationProfileError === null
              ? delegationProfiles.filter(profile => profile.status !== "ARCHIVED").map(profile => ({
                  delegation_profile_id: profile.delegation_profile_id,
                  name: profile.name,
                  agent_binding_id: profile.agent_binding_id,
                  enabled: profile.status === "ENABLED",
                }))
              : undefined}
            workerProfilesError={delegationProfileError ?? (runtime?.operatorReady === true && !delegationProfilesReady ? "Loading worker profiles…" : null)}
            onWorkspaceUpdated={(updated: WorkspacePrimaryReceipt) => {
              setWorkspaces(current => current.map(workspace => workspace.workspaceId === updated.workspace_id
                ? { ...workspace, primaryCoworkerId: updated.primary_coworker_id, version: updated.version }
                : workspace));
            }}
          />
        ) : page === "Goals" ? (
          <GoalsPage key={selectedWorkspaceId} api={goalApi} workspaceId={selectedWorkspaceId} onOpenTask={(taskId) => {
            setOpenedTask({ workspaceId: selectedWorkspaceId, taskId });
            setPage("Work");
          }} />
        ) : page === "Ideas" && !selectedWorkspaceId ? (
          <div className="page-content subpage-content"><div className="eyebrow">IDEAS</div><h1>Ideas</h1><section className="subpage-panel empty-panel"><div className="empty-icon" aria-hidden="true">✦</div><h2>Select a Workspace</h2><p>Create or select a Workspace in Settings to view saved Suggestions.</p><button className="text-button" type="button" onClick={() => setPage("Settings")}>Open Workspace settings <span aria-hidden="true">→</span></button></section></div>
        ) : page === "Ideas" && runtime?.operatorReady !== true && selectedWorkspaceId ? (
          <div className="page-content subpage-content"><div className="eyebrow">IDEAS</div><h1>Ideas</h1><section className="subpage-panel empty-panel"><div className="empty-icon" aria-hidden="true">✦</div><h2>Local Operator unavailable</h2><p>Start the local Runtime to load saved Suggestions. This view never starts work.</p><button className="text-button" type="button" onClick={() => setPage("Settings")}>Open Runtime settings <span aria-hidden="true">→</span></button></section></div>
        ) : page === "Ideas" && suggestionApi ? (
          <SuggestionsPage key={selectedWorkspaceId} api={suggestionApi} onTaskAccepted={(taskId) => {
            setOpenedTask({ workspaceId: selectedWorkspaceId, taskId });
            setPage("Work");
          }} />
        ) : page === "Routines" && runtime?.operatorReady !== true && selectedWorkspaceId ? (
          <div className="page-content subpage-content"><div className="eyebrow">WORKSPACE RESPONSIBILITIES</div><h1>Routines</h1><section className="subpage-panel empty-panel"><div className="empty-icon" aria-hidden="true">↻</div><h2>Local Operator unavailable</h2><p>Start the local Runtime to load or change Routine definitions. Run now can save a Task in READY state; planning and agent execution are not available yet.</p><button className="text-button" type="button" onClick={() => setPage("Settings")}>Open Runtime settings <span aria-hidden="true">→</span></button></section></div>
        ) : page === "Routines" ? (
          <RoutinesPage key={selectedWorkspaceId} api={routineApi} workspaceId={selectedWorkspaceId} resources={resources} resourcesNextCursor={resourceNextCursor} resourcesPageBusy={resourcePageBusy} onLoadMoreResources={loadMoreResources} onOpenTask={(task) => {
            if (task.workspace_id !== selectedWorkspaceId || task.status !== "READY" || !task.task_id) return;
            setOpenedTask({ workspaceId: task.workspace_id, taskId: task.task_id });
            setPage("Work");
          }} />
        ) : page === "Automations" && runtime?.operatorReady !== true && selectedWorkspaceId ? (
          <div className="page-content subpage-content"><div className="eyebrow">WORKSPACE RESPONSIBILITIES</div><h1>Automations</h1><section className="subpage-panel empty-panel"><div className="empty-icon" aria-hidden="true">◷</div><h2>Local Operator unavailable</h2><p>Start the local Runtime before loading Automation definitions. No Automation state is cached in this view.</p><button className="text-button" type="button" onClick={() => setPage("Settings")}>Open Runtime settings <span aria-hidden="true">→</span></button></section></div>
        ) : page === "Automations" ? (
          <AutomationsPage key={selectedWorkspaceId} api={automationApi} workspaceId={selectedWorkspaceId} resources={resources} resourcesNextCursor={resourceNextCursor} resourcesPageBusy={resourcePageBusy} onLoadMoreResources={loadMoreResources} onOpenTask={(taskId) => {
            setOpenedTask({ workspaceId: selectedWorkspaceId, taskId });
            setPage("Work");
          }} />
        ) : page === "Settings" ? (
          <SettingsPage runtime={runtime} runtimeError={runtimeError} runtimeBusy={runtimeBusy} startRuntime={startRuntime} refreshRuntime={refreshRuntime} agents={agentInstallations} agentError={agentInstallationError} refreshAgents={refreshAgentInstallations} agentProfiles={agentProfiles} agentProfileError={agentProfileError} agentBindings={agentBindings} agentBindingError={agentBindingError} agentActionMessage={agentActionMessage} agentActionError={agentActionError} agentActionBusy={agentActionBusy} onProbeCodex={probeCodexProfile} onProbeOpenCode={probeOpenCodeProfile} onRefreshAgentProfiles={() => refreshAgentProfiles(selectedWorkspaceId)} onRefreshAgentBindings={() => refreshAgentBindings(selectedWorkspaceId)} onCreateAgentBinding={createAgentBinding} onEnableAgentBinding={enableAgentBinding} runtimeBindings={runtimeBindings} runtimeBindingError={runtimeBindingError} runtimeBindingsLoading={runtimeBindingsLoading} runtimeEnrollmentBusy={runtimeEnrollmentBusy} runtimeEnrollmentMessage={runtimeEnrollmentMessage} runtimeEnrollmentError={runtimeEnrollmentError} onEnrollLocalRuntime={enrollLocalRuntime} onRefreshRuntimeBindings={() => refreshRuntimeBindings(selectedWorkspaceId)} workspaces={workspaces} workspaceError={workspaceError} refreshWorkspaces={refreshWorkspaces} workspaceName={workspaceName} setWorkspaceName={setWorkspaceName} createWorkspace={createWorkspace} workspaceCreateBusy={workspaceCreateBusy} selectedWorkspaceId={selectedWorkspaceId} onSelectWorkspace={setSelectedWorkspaceId} onUpdatePolicy={updateWorkspacePolicy} policyBusy={workspacePolicyBusy} onSetDefaultAgentBinding={setWorkspaceDefaultAgent} defaultAgentBusy={workspaceDefaultAgentBusy} instructions={instructionText} onInstructionsChange={setInstructionText} instructionHistory={workspaceInstructions} instructionMessage={instructionMessage} instructionBusy={instructionBusy} onSaveInstructions={saveWorkspaceInstructions} latestInstructionsAfterConflict={latestInstructionsAfterConflict} onUseLatestInstructions={() => { setInstructionText(latestInstructionsAfterConflict ?? ""); setLatestInstructionsAfterConflict(null); }} />
        ) : (
          <PlaceholderPage page={page} onSettings={() => setPage("Settings")} />
        )}
      </main>
    </div>
  );
}

function HomePage({ draft, onDraftChange, runtime, runtimeError, runtimeBusy, startRuntime, workspace, coworkerApi, coworkerSelection, onCoworkerSelectionChange, taskInputs, onRemoveTaskInput, onOpenLibrary, operatorReady, taskSaving, taskSaveError, hasPendingTaskSave, pendingTaskRetryMatches, onOpenPendingTask, onClearPendingTaskAfterReview, onSaveTask, onSettings, onOpenCoworkers, onWorkspaceUpdated, onOpenWork, onOpenTask }: { draft: string; onDraftChange: (value: string) => void; runtime: RuntimeStatus | null; runtimeError: string | null; runtimeBusy: boolean; startRuntime: () => Promise<void>; workspace: WorkspaceView | null; coworkerApi: ReturnType<typeof desktopCoworkerApi>; coworkerSelection: HomeCoworkerSelection | null; onCoworkerSelectionChange: (selection: HomeCoworkerSelection) => void; taskInputs: TaskInputSelection[]; onRemoveTaskInput: (resourceId: string) => void; onOpenLibrary: () => void; operatorReady: boolean; taskSaving: boolean; taskSaveError: string | null; hasPendingTaskSave: boolean; pendingTaskRetryMatches: boolean; onOpenPendingTask: () => void; onClearPendingTaskAfterReview: () => void; onSaveTask: (selection: HomeCoworkerSelection) => Promise<void>; onSettings: () => void; onOpenCoworkers: () => void; onWorkspaceUpdated: (updated: WorkspacePrimaryReceipt) => void; onOpenWork: () => void; onOpenTask: (taskId: string) => void }) {
  const primaryCoworkerId = workspace?.primaryCoworkerId ?? null;
  const workspaceId = workspace?.workspaceId ?? "";
  const primaryCoworkerKey = workspace && primaryCoworkerId ? `${workspace.workspaceId}:${primaryCoworkerId}` : null;
  const [coworkerRoster, setCoworkerRoster] = useState<{
    key: string;
    status: "LOADING" | "READY" | "UNAVAILABLE";
    items: Coworker[];
    nextCursor: string | null;
    error?: string;
  } | null>(null);
  const [rosterMoreBusy, setRosterMoreBusy] = useState(false);
  const [primaryResolution, setPrimaryResolution] = useState<{ key: string; status: "LOADING" | "UNAVAILABLE" } | null>(null);
  const [selectionFreshness, setSelectionFreshness] = useState<{ key: string; status: "CHECKING" | "CURRENT" | "STALE" | "UNAVAILABLE" } | null>(null);
  const [refreshSelectionBusy, setRefreshSelectionBusy] = useState(false);
  const [selectionActionMessage, setSelectionActionMessage] = useState<string | null>(null);

  useEffect(() => {
    if (!workspaceId) return;
    const key = workspaceId;
    setCoworkerRoster({ key, status: "LOADING", items: [], nextCursor: null });
    if (!operatorReady) {
      setCoworkerRoster({ key, status: "UNAVAILABLE", items: [], nextCursor: null, error: "Coworkers are unavailable while the local Runtime is offline." });
      return;
    }
    const controller = new AbortController();
    void coworkerApi.list(undefined, controller.signal).then((page) => {
      if (controller.signal.aborted) return;
      if (page.items.some(item => item.workspace_id !== workspaceId)) throw new Error("Coworker list scope mismatch.");
      setCoworkerRoster({ key, status: "READY", items: page.items, nextCursor: page.next_cursor });
    }).catch(() => {
      if (!controller.signal.aborted) setCoworkerRoster({ key, status: "UNAVAILABLE", items: [], nextCursor: null, error: "Coworkers could not be loaded. Your draft is preserved." });
    });
    return () => controller.abort();
  }, [coworkerApi, operatorReady, workspaceId]);

  useEffect(() => {
    if (!workspace) return;
    if (coworkerSelection?.workspaceId === workspace.workspaceId && coworkerSelection.source === "USER") return;
    if (coworkerSelection?.workspaceId === workspace.workspaceId
      && coworkerSelection.source === "PRIMARY"
      && coworkerSelection.coworkerId === primaryCoworkerId) return;
    if (!primaryCoworkerId) {
      onCoworkerSelectionChange({ workspaceId: workspace.workspaceId, coworkerId: null, expectedVersion: null, name: "Workspace defaults", status: "WORKSPACE_DEFAULTS", source: "DEFAULTS" });
      setPrimaryResolution(null);
      return;
    }
    const key = `${workspace.workspaceId}:${primaryCoworkerId}`;
    setPrimaryResolution({ key, status: "LOADING" });
    if (!operatorReady) {
      setPrimaryResolution({ key, status: "UNAVAILABLE" });
      return;
    }
    const controller = new AbortController();
    void coworkerApi.get(primaryCoworkerId, controller.signal).then((coworker) => {
      if (controller.signal.aborted) return;
      if (coworker.workspace_id !== workspace.workspaceId || coworker.coworker_id !== primaryCoworkerId) throw new Error("Coworker response scope mismatch.");
      onCoworkerSelectionChange({
        workspaceId: workspace.workspaceId,
        coworkerId: coworker.coworker_id,
        expectedVersion: coworker.version,
        name: coworker.revision.name,
        status: coworker.status,
        source: "PRIMARY",
      });
      setPrimaryResolution(null);
    }).catch(() => {
      if (!controller.signal.aborted) setPrimaryResolution({ key, status: "UNAVAILABLE" });
    });
    return () => controller.abort();
  }, [coworkerApi, coworkerSelection?.coworkerId, coworkerSelection?.source, coworkerSelection?.workspaceId, onCoworkerSelectionChange, operatorReady, primaryCoworkerId, workspace]);

  const currentSelection = coworkerSelection?.workspaceId === workspaceId ? coworkerSelection : null;
  const selectionKey = currentSelection?.coworkerId && currentSelection.expectedVersion !== null
    ? `${workspaceId}:${currentSelection.coworkerId}:${currentSelection.expectedVersion}` : null;

  useEffect(() => {
    if (!currentSelection || !selectionKey) {
      setSelectionFreshness(currentSelection ? { key: `${workspaceId}:defaults`, status: "CURRENT" } : null);
      return;
    }
    if (!operatorReady) {
      setSelectionFreshness({ key: selectionKey, status: "UNAVAILABLE" });
      return;
    }
    const controller = new AbortController();
    setSelectionFreshness({ key: selectionKey, status: "CHECKING" });
    void coworkerApi.get(currentSelection.coworkerId!, controller.signal).then((latest) => {
      if (controller.signal.aborted) return;
      if (latest.workspace_id !== workspaceId || latest.coworker_id !== currentSelection.coworkerId) throw new Error("Coworker response scope mismatch.");
      const status = latest.version === currentSelection.expectedVersion && latest.status === currentSelection.status ? "CURRENT" : "STALE";
      setSelectionFreshness({ key: selectionKey, status });
    }).catch(() => {
      if (!controller.signal.aborted) setSelectionFreshness({ key: selectionKey, status: "UNAVAILABLE" });
    });
    return () => controller.abort();
  }, [coworkerApi, currentSelection?.coworkerId, currentSelection?.expectedVersion, currentSelection?.status, currentSelection?.workspaceId, operatorReady, selectionKey, workspaceId]);

  const activeCoworkers = coworkerRoster?.key === workspaceId ? coworkerRoster.items.filter(item => item.status === "ACTIVE") : [];
  const selectionCheck = selectionKey && selectionFreshness?.key === selectionKey ? selectionFreshness.status : currentSelection?.coworkerId ? "CHECKING" : currentSelection ? "CURRENT" : "UNAVAILABLE";
  const primarySelectionSettled = !primaryCoworkerId
    ? currentSelection?.source === "USER" || currentSelection?.source === "DEFAULTS"
    : currentSelection?.source === "USER" || (currentSelection?.source === "PRIMARY" && currentSelection.coworkerId === primaryCoworkerId);
  const primaryResolutionUnavailable = Boolean(primaryCoworkerId && primaryResolution?.key === primaryCoworkerKey && primaryResolution.status === "UNAVAILABLE");
  const selectionReady = Boolean(currentSelection && primarySelectionSettled && !primaryResolutionUnavailable)
    && (currentSelection?.coworkerId === null || (currentSelection?.status === "ACTIVE" && selectionCheck === "CURRENT"));

  const loadMoreCoworkers = async () => {
    const roster = coworkerRoster;
    if (!workspaceId || roster?.key !== workspaceId || !roster.nextCursor || rosterMoreBusy || !operatorReady) return;
    setRosterMoreBusy(true);
    try {
      const page = await coworkerApi.list(roster.nextCursor);
      if (page.items.some(item => item.workspace_id !== workspaceId)) throw new Error("Coworker list scope mismatch.");
      setCoworkerRoster(current => current?.key === workspaceId ? {
        ...current,
        items: [...current.items, ...page.items.filter(item => !current.items.some(existing => existing.coworker_id === item.coworker_id))],
        nextCursor: page.next_cursor,
      } : current);
    } catch {
      setCoworkerRoster(current => current?.key === workspaceId ? { ...current, error: "More Coworkers could not be loaded. Existing selection is unchanged." } : current);
    } finally {
      setRosterMoreBusy(false);
    }
  };

  const chooseCoworker = (coworkerId: string) => {
    if (!workspace || taskSaving) return;
    if (coworkerId === "") {
      setSelectionActionMessage(null);
      onCoworkerSelectionChange({ workspaceId: workspace.workspaceId, coworkerId: null, expectedVersion: null, name: "Workspace defaults", status: "WORKSPACE_DEFAULTS", source: "USER" });
      return;
    }
    const selected = activeCoworkers.find(item => item.coworker_id === coworkerId);
    if (!selected) return;
    setSelectionActionMessage(null);
    onCoworkerSelectionChange({ workspaceId: workspace.workspaceId, coworkerId: selected.coworker_id, expectedVersion: selected.version, name: selected.revision.name, status: selected.status, source: "USER" });
  };

  const refreshCoworkerSelection = async () => {
    if (!currentSelection?.coworkerId || !workspace || !operatorReady || refreshSelectionBusy) return;
    setRefreshSelectionBusy(true);
    try {
      const latest = await coworkerApi.get(currentSelection.coworkerId);
      if (latest.workspace_id !== workspace.workspaceId || latest.coworker_id !== currentSelection.coworkerId) throw new Error("Coworker response scope mismatch.");
      onCoworkerSelectionChange({
        ...currentSelection,
        expectedVersion: latest.version,
        name: latest.revision.name,
        status: latest.status,
      });
      setSelectionActionMessage(latest.status === "ACTIVE"
        ? "Latest Coworker settings loaded. Review the selection before saving."
        : `This Coworker is ${latest.status.toLowerCase()}; choose an active Coworker to continue.`);
    } catch {
      setSelectionActionMessage("The selected Coworker is unavailable in this Workspace. Your draft is preserved.");
    } finally {
      setRefreshSelectionBusy(false);
    }
  };

  const recipientSelection = currentSelection && primarySelectionSettled ? currentSelection : null;
  const recipientLabel = recipientSelection?.name
    ?? (primaryCoworkerId ? (primaryResolution?.key === primaryCoworkerKey && primaryResolution.status === "UNAVAILABLE" ? "Coworker unavailable" : "Loading Coworker…") : "Loading Workspace defaults…");
  const showCoworkerSelector = activeCoworkers.length > 1
    || (coworkerRoster?.key === workspaceId && coworkerRoster.nextCursor !== null)
    || Boolean(recipientSelection?.coworkerId && (recipientSelection.status !== "ACTIVE" || selectionCheck === "STALE" || selectionCheck === "UNAVAILABLE"))
    || Boolean(recipientSelection?.source === "USER" && coworkerRoster?.key === workspaceId && coworkerRoster.status === "UNAVAILABLE")
    || (primaryResolutionUnavailable && recipientSelection?.source !== "USER");
  const selectedOptionAvailable = recipientSelection?.coworkerId === null
    || activeCoworkers.some(item => item.coworker_id === recipientSelection?.coworkerId);
  const freshnessMessage = recipientSelection?.coworkerId && selectionCheck === "CHECKING"
    ? "Checking this Coworker’s current status…"
    : recipientSelection?.coworkerId && selectionCheck === "STALE"
      ? "This Coworker changed since selection. Review its latest settings before saving."
      : recipientSelection?.coworkerId && selectionCheck === "UNAVAILABLE"
        ? "This Coworker could not be checked in the selected Workspace. Your draft is preserved."
        : recipientSelection?.status === "ARCHIVED"
          ? "This Coworker is archived. Choose an active Coworker; LiteCowork will not switch automatically."
          : recipientSelection?.status === "PAUSED"
            ? "This Coworker is paused. Choose an active Coworker to save work from Home."
            : null;

  return (
    <div className="page-content home-content">
      <div className="home-heading">
        <div className="eyebrow">YOUR WORKSPACE</div>
        <h1>What should we work on?</h1>
        <p>Save work in this Workspace. Saved Tasks are not sent to an agent yet.</p>
      </div>

      {workspace?.status === "ACTIVE" && !primaryCoworkerId && operatorReady && coworkerRoster?.key === workspaceId && coworkerRoster.status === "READY" && <HomeCoworkerOnboarding
        key={workspaceId}
        api={coworkerApi}
        workspaceId={workspaceId}
        workspaceVersion={workspace.version}
        items={coworkerRoster.items}
        hasMore={coworkerRoster.nextCursor !== null}
        disabled={taskSaving || hasPendingTaskSave}
        onCreated={coworker => setCoworkerRoster(current => current?.key === workspaceId ? { ...current, items: [...current.items.filter(item => item.coworker_id !== coworker.coworker_id), coworker] } : current)}
        onPrimary={onWorkspaceUpdated}
        onManage={onOpenCoworkers}
      />}

      <form className="composer-card" aria-label="Save a Task" onSubmit={(event) => { event.preventDefault(); if (currentSelection && selectionReady) void onSaveTask(currentSelection); }}>
        <div className="composer-recipient">
          <span>For</span>
          {showCoworkerSelector ? (
            <select
              aria-label="Coworker for this Task"
              value={recipientSelection?.coworkerId ?? ""}
              disabled={taskSaving}
              onChange={(event) => chooseCoworker(event.target.value)}
            >
              <option value="">Workspace defaults</option>
              {recipientSelection?.coworkerId && !selectedOptionAvailable && (
                <option value={recipientSelection.coworkerId} disabled>
                  {recipientSelection.name} · {recipientSelection.status === "ARCHIVED" ? "Archived" : recipientSelection.status === "PAUSED" ? "Paused" : selectionCheck === "UNAVAILABLE" ? "Unavailable" : "Not loaded"}
                </option>
              )}
              {activeCoworkers.map(item => <option key={item.coworker_id} value={item.coworker_id}>{item.revision.name}{item.is_primary ? " · Primary" : ""}</option>)}
            </select>
          ) : <strong>{recipientLabel}</strong>}
          {(primaryCoworkerId || activeCoworkers.length > 0) && <button type="button" className="text-button" onClick={onOpenCoworkers}>Manage</button>}
        </div>
        {coworkerRoster?.key === workspaceId && coworkerRoster.status === "LOADING" && <p className="composer-selection-note" role="status">Loading Coworkers in this Workspace…</p>}
        {coworkerRoster?.key === workspaceId && coworkerRoster.status === "UNAVAILABLE" && <p className="composer-selection-note" role="status">{coworkerRoster.error}</p>}
        {primaryResolutionUnavailable && recipientSelection?.source !== "USER" && <p className="composer-selection-note is-warning" role="alert">The Workspace’s primary Coworker could not be confirmed. Choose another Coworker or Workspace defaults; the Task will not be reassigned automatically.</p>}
        {coworkerRoster?.key === workspaceId && coworkerRoster.nextCursor && <button className="text-button composer-load-more" type="button" disabled={rosterMoreBusy || taskSaving} onClick={() => void loadMoreCoworkers()}>{rosterMoreBusy ? "Loading Coworkers…" : "Load more Coworkers"}</button>}
        {coworkerRoster?.key === workspaceId && coworkerRoster.error && coworkerRoster.status === "READY" && <p className="composer-selection-note" role="status">{coworkerRoster.error}</p>}
        {freshnessMessage && <div className={`composer-selection-note${selectionCheck === "STALE" || recipientSelection?.status !== "ACTIVE" ? " is-warning" : ""}`} role="status">
          <span>{freshnessMessage}</span>
          {recipientSelection?.coworkerId && (selectionCheck === "STALE" || selectionCheck === "UNAVAILABLE") && <button type="button" className="text-button" disabled={!operatorReady || refreshSelectionBusy || taskSaving} onClick={() => void refreshCoworkerSelection()}>{refreshSelectionBusy ? "Checking…" : "Refresh selection"}</button>}
        </div>}
        {selectionActionMessage && <p className="composer-selection-note" role="status">{selectionActionMessage}</p>}
        <label className="sr-only" htmlFor="task-draft">Describe what you want to work on</label>
        <textarea
          id="task-draft"
          value={draft}
          onChange={(event) => onDraftChange(event.target.value)}
          placeholder="Describe something you want to get done…"
          rows={3}
          maxLength={32768}
          disabled={taskSaving}
          aria-describedby="composer-status"
        />
        {taskInputs.length > 0 && <ul className="task-input-chips" aria-label="Pinned Task inputs">
          {taskInputs.map((input) => <li key={input.resourceId}>
            <span aria-hidden="true">▤</span><span>{input.displayName}<small>Revision pinned</small></span>
            <button type="button" className="icon-button" aria-label={`Remove ${input.displayName} from Task inputs`} disabled={taskSaving} onClick={() => onRemoveTaskInput(input.resourceId)}>×</button>
          </li>)}
        </ul>}
        <div className="composer-footer">
          <span id="composer-status" className="composer-hint">Saves a durable Task in {workspace?.name ?? "the selected Workspace"}{recipientSelection?.coworkerId ? ` with ${recipientSelection.name}’s selected settings` : " using Workspace defaults"}. The selected Coworker identity and revision are checked at save time. Selected Resources are pinned as inputs; no agent is started yet.</span>
          <div className="composer-actions">
            <button className="quiet-button" type="button" onClick={onOpenLibrary} disabled={taskSaving}>Add inputs</button>
            <button className="primary-button" type="submit" disabled={taskSaving || !operatorReady || workspace?.status !== "ACTIVE" || !draft.trim() || !selectionReady || (hasPendingTaskSave && !pendingTaskRetryMatches)}>
              <span>{taskSaving ? "Saving…" : pendingTaskRetryMatches ? "Retry original save" : "Save Task"}</span><span aria-hidden="true">→</span>
            </button>
          </div>
        </div>
        {hasPendingTaskSave && pendingTaskRetryMatches && <div className="composer-hint" role="status">A prior response was interrupted. Retrying uses the original Coworker and lead settings so LiteCowork can return the same saved Task. <button type="button" className="text-button" onClick={onOpenPendingTask} disabled={taskSaving}>Check Work first</button></div>}
        {hasPendingTaskSave && !pendingTaskRetryMatches && <div className="composer-hint" role="alert">The current draft differs from an unresolved save. Check the original Workspace before starting separate work. <button type="button" className="text-button" onClick={onOpenPendingTask} disabled={taskSaving}>Open original Workspace Work</button> <button type="button" className="text-button" onClick={onClearPendingTaskAfterReview} disabled={taskSaving}>I checked Work; clear retry</button></div>}
        {!workspace?.defaultAgentBindingId && !recipientSelection?.coworkerId && <p className="composer-hint">Choose and enable a lead agent in Settings, or set one on the selected Coworker, before saving a Task. <button type="button" className="text-button" onClick={onSettings}>Set up a lead agent</button></p>}
        {!operatorReady && <p className="composer-hint">Start the local Runtime before saving work.</p>}
        {taskSaveError && <p className="inline-error" role="alert">{taskSaveError}</p>}
      </form>

      <div className="setup-callout" role="status">
        <span className="callout-icon" aria-hidden="true">i</span>
        <div>
          <strong>{runtime?.processRunning && !runtime.operatorReady ? "Local Runtime process is starting" : runtime?.processRunning && runtime.state === "DEGRADED" ? "Local Runtime is available; work execution is not" : canOfferLocalRuntimeStart(runtime) ? "Local Runtime is not running" : "Local Runtime setup"}</strong>
          <p>{runtimeError ?? (runtime?.processRunning && !runtime.operatorReady ? "The daemon process exists, but the authenticated Operator API has not confirmed this Runtime incarnation yet." : runtime?.state === "DEGRADED" ? `Work remains blocked: ${runtime.blockers.join(", ") || "startup recovery is incomplete"}.` : "Start the local Runtime to initialize encrypted local storage. It will report any services that are not ready.")}</p>
          {canOfferLocalRuntimeStart(runtime) ? (
            <button className="text-button" type="button" onClick={() => void startRuntime()} disabled={runtimeBusy}>
              {runtimeBusy ? "Starting Runtime…" : "Start local Runtime"} <span aria-hidden="true">→</span>
            </button>
          ) : <button className="text-button" type="button" onClick={onSettings}>View Runtime details <span aria-hidden="true">→</span></button>}
        </div>
      </div>

      <div className="section-heading">
        <div><h2>Recent work</h2><p>Persisted Tasks in the selected Workspace.</p></div>
        <button className="quiet-button" type="button" onClick={onOpenWork}>View all</button>
      </div>
      <TaskListPanel workspaceId={workspaceId} operatorReady={operatorReady} status={null} limit={5} compact onOpenTask={onOpenTask} />
    </div>
  );
}

function WorkPage({ selectedWorkspaceId, workspaceName, resourceNameCache, artifactApi, coworkerApi, operatorReady, initialTaskId }: { selectedWorkspaceId: string; workspaceName: string; resourceNameCache: Record<string, string>; artifactApi: ReturnType<typeof desktopArtifactApi>; coworkerApi: ReturnType<typeof desktopCoworkerApi>; operatorReady: boolean; initialTaskId: string | null }) {
  const [status, setStatus] = useState<TaskStatus | "">("");
  const [openTaskId, setOpenTaskId] = useState<string | null>(initialTaskId);
  const [refreshToken, setRefreshToken] = useState(0);
  return (
    <div className="page-content subpage-content work-content">
      <div className="eyebrow">{workspaceName.toUpperCase()}</div>
      <div className="work-page-heading">
        <div><h1>Work</h1><p>Browse saved Tasks. Open a Task to see its plan and execution state.</p></div>
        {!openTaskId && <div className="work-list-controls">
          <label className="task-status-filter">Status
            <select value={status} onChange={(event) => setStatus(event.target.value as TaskStatus | "")} aria-label="Filter Tasks by exact status">
              <option value="">All statuses</option>
              {TASK_STATUSES.map((item) => <option key={item} value={item}>{taskStatusLabel(item)}</option>)}
            </select>
          </label>
          <button type="button" className="quiet-button" onClick={() => setRefreshToken((value) => value + 1)} disabled={!operatorReady}>Refresh</button>
        </div>}
      </div>
      {openTaskId ? (
          <TaskDetailPanel key={`${selectedWorkspaceId}:${openTaskId}`} workspaceId={selectedWorkspaceId} taskId={openTaskId} resourceNameCache={resourceNameCache} artifactApi={artifactApi} coworkerApi={coworkerApi} operatorReady={operatorReady} onBack={() => setOpenTaskId(null)} />
      ) : (
        <TaskListPanel workspaceId={selectedWorkspaceId} operatorReady={operatorReady} status={status || null} limit={50} refreshToken={refreshToken} onOpenTask={setOpenTaskId} />
      )}
    </div>
  );
}

function TaskListPanel({ workspaceId, operatorReady, status, limit, compact = false, refreshToken = 0, onOpenTask }: { workspaceId: string; operatorReady: boolean; status: TaskStatus | null; limit: number; compact?: boolean; refreshToken?: number; onOpenTask: (taskId: string) => void }) {
  const [items, setItems] = useState<TaskSummaryView[]>([]);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [pageBusy, setPageBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [refreshFailed, setRefreshFailed] = useState(false);
  const [loadedKey, setLoadedKey] = useState<string | null>(null);
  const [lastLoadedAt, setLastLoadedAt] = useState<string | null>(null);
  const generation = useRef(0);
  const queryKey = JSON.stringify([workspaceId, status, limit]);
  const visibleItems = loadedKey === queryKey ? items : [];
  const stale = loadedKey === queryKey && visibleItems.length > 0 && (!operatorReady || refreshFailed);

  useEffect(() => {
    const currentGeneration = ++generation.current;
    let active = true;
    setPageBusy(false);
    setNextCursor(null);
    setError(null);
    setRefreshFailed(false);
    if (!workspaceId) {
      setLoadedKey(null);
      setItems([]);
      setLastLoadedAt(null);
      setBusy(false);
      return () => { active = false; };
    }
    if (!operatorReady) {
      setBusy(false);
      setPageBusy(false);
      setError(loadedKey === queryKey && items.length > 0
        ? "Local Runtime is offline. Showing the last Task list loaded in this window."
        : "Task browsing is unavailable while the local Runtime is offline.");
      if (loadedKey !== queryKey) {
        setItems([]);
        setLoadedKey(null);
        setLastLoadedAt(null);
      }
      return () => { active = false; };
    }
    if (loadedKey !== queryKey) {
      setItems([]);
      setLoadedKey(null);
      setLastLoadedAt(null);
    }
    setBusy(true);
    void invoke<TaskPageView>("list_tasks", { workspaceId, status, cursor: null, limit }).then((page) => {
      if (!active || generation.current !== currentGeneration) return;
      setItems(page.items);
      setNextCursor(page.nextCursor);
      setLoadedKey(queryKey);
      setLastLoadedAt(new Date().toISOString());
      setRefreshFailed(false);
      setError(null);
    }).catch((failure) => {
      if (!active || generation.current !== currentGeneration) return;
      setRefreshFailed(loadedKey === queryKey && items.length > 0);
      setError(typeof failure === "string" ? failure : "Task list could not be loaded from the local Runtime.");
    }).finally(() => {
      if (active && generation.current === currentGeneration) setBusy(false);
    });
    return () => { active = false; };
  }, [workspaceId, operatorReady, status, limit, queryKey, refreshToken]);

  const loadMore = async () => {
    if (!workspaceId || !operatorReady || !nextCursor || pageBusy || loadedKey !== queryKey) return;
    const currentGeneration = generation.current;
    setPageBusy(true);
    setError(null);
    try {
      const page = await invoke<TaskPageView>("list_tasks", { workspaceId, status, cursor: nextCursor, limit });
      if (generation.current !== currentGeneration) return;
      setItems((current) => [...current, ...page.items]);
      setNextCursor(page.nextCursor);
      setLastLoadedAt(new Date().toISOString());
    } catch (failure) {
      if (generation.current === currentGeneration) setError(typeof failure === "string" ? failure : "More Tasks could not be loaded.");
    } finally {
      if (generation.current === currentGeneration) setPageBusy(false);
    }
  };

  if (!workspaceId) return <div className="task-empty-state"><h2>Select a Workspace</h2><p>Choose or create a Workspace in Settings to browse its saved Tasks.</p></div>;
  return (
    <section className={`task-list-panel ${compact ? "compact" : ""}`} aria-label={compact ? "Recent Tasks" : "Task list"}>
      {error && <p className={stale ? "task-stale-note" : "inline-error"} role="status">{stale ? `Showing the last Task list loaded ${lastLoadedAt ? formatTaskTime(lastLoadedAt) : "earlier"}. ${error}` : error}</p>}
      {busy && visibleItems.length === 0 ? <div className="task-loading" role="status">Loading saved Tasks…</div> : visibleItems.length > 0 ? (
        <>
          <ul className="task-list">
            {visibleItems.map((task) => <li key={task.taskId}>
              <button type="button" className="task-row" onClick={() => onOpenTask(task.taskId)}>
                <span className={`task-status-mark status-${task.status.toLowerCase()}`} aria-hidden="true" />
                <span className="task-row-content"><strong>{task.objective || "Untitled Task"}</strong><small>{taskStatusLabel(task.status)} · Task updated {formatTaskTime(task.updatedAt)}</small></span>
                <span className="task-row-arrow" aria-hidden="true">›</span>
              </button>
            </li>)}
          </ul>
          {!compact && nextCursor && <div className="load-more-row"><button type="button" className="quiet-button" onClick={() => void loadMore()} disabled={pageBusy || !operatorReady}>{pageBusy ? "Loading…" : "Load more"}</button></div>}
          {compact && nextCursor && <p className="task-more-hint">Showing the 5 most recent Tasks.</p>}
        </>
      ) : busy ? null : error ? <div className="task-empty-state"><h2>Tasks could not be loaded</h2><p>Check the local Runtime connection, then return to Work to refresh.</p></div> : (
        <div className="task-empty-state"><h2>{status ? `No ${taskStatusLabel(status).toLowerCase()} Tasks` : "No saved Tasks yet"}</h2><p>{status ? "Try another status filter to find saved work." : "Use the Home composer to save a Task in this Workspace."}</p></div>
      )}
    </section>
  );
}

function TaskDetailPanel({ workspaceId, taskId, resourceNameCache, artifactApi, coworkerApi, operatorReady, onBack }: { workspaceId: string; taskId: string; resourceNameCache: Record<string, string>; artifactApi: ReturnType<typeof desktopArtifactApi>; coworkerApi: ReturnType<typeof desktopCoworkerApi>; operatorReady: boolean; onBack: () => void }) {
  const [task, setTask] = useState<TaskDetailView | null>(null);
  const [originCoworkerName, setOriginCoworkerName] = useState<string | null>(null);
  const [originCoworkerLoading, setOriginCoworkerLoading] = useState(false);
  const [originCoworkerUnavailable, setOriginCoworkerUnavailable] = useState(false);
  const [planningReadiness, setPlanningReadiness] = useState<TaskPlanningReadinessView | null>(null);
  const [planningReadinessBusy, setPlanningReadinessBusy] = useState(false);
  const [planningReadinessError, setPlanningReadinessError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [editingObjective, setEditingObjective] = useState(false);
  const [objectiveDraft, setObjectiveDraft] = useState("");
  const [objectiveSaving, setObjectiveSaving] = useState(false);
  const [objectiveError, setObjectiveError] = useState<string | null>(null);
  const [objectiveSaved, setObjectiveSaved] = useState<string | null>(null);
  const pendingEdit = useRef<{ key: string; requestId: string } | null>(null);
  const objectiveEditTrigger = useRef<HTMLButtonElement | null>(null);
  const wasEditingObjective = useRef(false);
  const [lastLoadedAt, setLastLoadedAt] = useState<string | null>(null);
  const [detailReload, setDetailReload] = useState(0);
  const generation = useRef(0);
  const readinessGeneration = useRef(0);
  const visibleTask = task?.taskId === taskId && task.workspaceId === workspaceId ? task : null;
  const originCoworkerId = visibleTask?.originCoworkerId ?? null;
  const originCoworkerRevision = visibleTask?.originCoworkerRevision ?? null;
  useEffect(() => {
    const currentGeneration = ++generation.current;
    readinessGeneration.current += 1;
    setPlanningReadiness(null);
    setPlanningReadinessError(null);
    setPlanningReadinessBusy(false);
    let active = true;
    setError(null);
    if (!workspaceId || !taskId) { setTask(null); setBusy(false); return () => { active = false; }; }
    if (!operatorReady) {
      setBusy(false);
      setError(visibleTask ? `Local Runtime is offline. Showing the Task details last loaded ${lastLoadedAt ? formatTaskTime(lastLoadedAt) : "earlier"}.` : "Task details are unavailable while the local Runtime is offline.");
      return () => { active = false; };
    }
    setBusy(true);
    void invoke<TaskDetailView>("get_task", { workspaceId, taskId }).then((view) => {
      if (!active || generation.current !== currentGeneration) return;
      if (view.workspaceId !== workspaceId || view.taskId !== taskId) {
        throw new Error("Task details do not match the selected Workspace and Task.");
      }
      setTask(view);
      setLastLoadedAt(new Date().toISOString());
      setError(null);
    }).catch((failure) => {
      if (!active || generation.current !== currentGeneration) return;
      setTask(null);
      setError(typeof failure === "string" ? failure : "Task details could not be loaded from the local Runtime.");
    }).finally(() => {
      if (active && generation.current === currentGeneration) setBusy(false);
    });
    return () => { active = false; };
  }, [workspaceId, taskId, operatorReady, detailReload]);

  useEffect(() => {
    const controller = new AbortController();
    let active = true;
    setOriginCoworkerName(null);
    setOriginCoworkerUnavailable(false);
    if (!originCoworkerId || originCoworkerRevision === null) {
      setOriginCoworkerLoading(false);
      return () => { active = false; };
    }
    if (!operatorReady) return () => { active = false; };
    setOriginCoworkerLoading(true);
    void coworkerApi.getRevision(originCoworkerId, originCoworkerRevision, controller.signal).then((revision) => {
      if (!active || controller.signal.aborted) return;
      if (revision.coworker_id !== originCoworkerId || revision.revision !== originCoworkerRevision) {
        throw new Error("Coworker revision identity mismatch.");
      }
      setOriginCoworkerName(revision.definition.name);
    }).catch(() => {
      if (active && !controller.signal.aborted) setOriginCoworkerUnavailable(true);
    }).finally(() => {
      if (active && !controller.signal.aborted) setOriginCoworkerLoading(false);
    });
    return () => { active = false; controller.abort(); };
  }, [coworkerApi, operatorReady, originCoworkerId, originCoworkerRevision]);

  useEffect(() => {
    if (wasEditingObjective.current && !editingObjective) objectiveEditTrigger.current?.focus();
    wasEditingObjective.current = editingObjective;
  }, [editingObjective]);

  const reloadTask = async () => {
    if (!operatorReady) return;
    readinessGeneration.current += 1;
    setPlanningReadiness(null);
    setPlanningReadinessError(null);
    setPlanningReadinessBusy(false);
    setBusy(true);
    setObjectiveError(null);
    try {
      const latest = await invoke<TaskDetailView>("get_task", { workspaceId, taskId });
      if (latest.workspaceId !== workspaceId || latest.taskId !== taskId) throw new Error("Task details belong to a different Workspace.");
      setTask(latest);
      setObjectiveDraft(latest.objective);
      setLastLoadedAt(new Date().toISOString());
      setEditingObjective(false);
      setObjectiveSaved(null);
      pendingEdit.current = null;
    } catch (failure) {
      setObjectiveError(typeof failure === "string" ? failure : failure instanceof Error ? failure.message : "Task details could not be refreshed.");
    } finally {
      setBusy(false);
    }
  };

  const checkPlanningReadiness = async () => {
    if (!visibleTask || !operatorReady || planningReadinessBusy) return;
    const checkedTask = visibleTask;
    const currentGeneration = ++readinessGeneration.current;
    setPlanningReadiness(null);
    setPlanningReadinessError(null);
    setPlanningReadinessBusy(true);
    try {
      const result: unknown = await invoke("get_task_planning_readiness", {
        workspaceId,
        taskId,
        expectedTaskVersion: checkedTask.taskVersion,
      });
      if (readinessGeneration.current !== currentGeneration) return;
      if (!isTaskPlanningReadinessFor(result, {
        taskId,
        taskVersion: checkedTask.taskVersion,
        taskSpecRevision: checkedTask.currentSpecRevision,
        taskStatus: checkedTask.status,
      })) {
        throw new Error("Task changed or the Runtime returned an unsupported readiness result. Reload the Task and try again.");
      }
      setPlanningReadiness(result);
    } catch (failure) {
      if (readinessGeneration.current === currentGeneration) {
        setPlanningReadinessError(typeof failure === "string" ? failure : failure instanceof Error ? failure.message : "Planning readiness could not be checked.");
      }
    } finally {
      if (readinessGeneration.current === currentGeneration) setPlanningReadinessBusy(false);
    }
  };

  const saveObjective = async () => {
    if (!visibleTask || objectiveSaving || !operatorReady) return;
    const objective = objectiveDraft.trim();
    if (!objective || new TextEncoder().encode(objective).length > 32 * 1024) {
      setObjectiveError("Enter an objective within the 32 KB limit.");
      return;
    }
    if (objective === visibleTask.objective) {
      setObjectiveError("Make a change before saving a new specification revision.");
      return;
    }
    const key = JSON.stringify([workspaceId, taskId, visibleTask.taskVersion, visibleTask.currentSpecRevision, objective]);
    let request = pendingEdit.current;
    if (!request || request.key !== key) {
      request = { key, requestId: crypto.randomUUID() };
      pendingEdit.current = request;
    }
    setObjectiveSaving(true);
    setObjectiveError(null);
    setObjectiveSaved(null);
    try {
      const receipt = await invoke<TaskSpecRevisionReceiptView>("revise_task_spec", {
        workspaceId,
        taskId,
        expectedTaskVersion: visibleTask.taskVersion,
        parentRevision: visibleTask.currentSpecRevision,
        objective,
        requestId: request.requestId,
      });
      if (receipt.taskId !== taskId || receipt.revision !== visibleTask.currentSpecRevision + 1 || receipt.objective !== objective) {
        throw new Error("Local Runtime returned a revision that does not match this edit.");
      }
      const latest = await invoke<TaskDetailView>("get_task", { workspaceId, taskId });
      if (latest.workspaceId !== workspaceId || latest.taskId !== taskId || latest.currentSpecRevision < receipt.revision) {
        throw new Error("The edit was saved, but the latest Task state could not be confirmed. Reload the Task before continuing.");
      }
      readinessGeneration.current += 1;
      setPlanningReadiness(null);
      setPlanningReadinessError(null);
      setTask(latest);
      setObjectiveDraft(latest.objective);
      setLastLoadedAt(new Date().toISOString());
      setEditingObjective(false);
      setObjectiveSaved(latest.currentSpecRevision === receipt.revision
        ? `Saved as specification revision ${receipt.revision}.`
        : `Saved revision ${receipt.revision}; a newer revision is now current.`);
      pendingEdit.current = null;
    } catch (failure) {
      setObjectiveError(typeof failure === "string" ? failure : failure instanceof Error ? failure.message : "Task objective could not be saved. Retry the unchanged edit to reconcile the request.");
    } finally {
      setObjectiveSaving(false);
    }
  };

  return (
    <section className="task-detail-panel" aria-label="Task details">
      <button type="button" className="text-button task-back-button" onClick={onBack}>← Back to Work</button>
      {error && <p className="task-stale-note" role="status">{error}</p>}
      {objectiveError && <p className="task-stale-note" role="alert">{objectiveError} <button type="button" className="text-button" onClick={() => void reloadTask()} disabled={!operatorReady || busy}>Reload latest Task</button></p>}
      {objectiveSaved && <p className="task-edit-success" role="status">{objectiveSaved}</p>}
      {busy && !visibleTask ? <div className="task-loading" role="status">Loading Task details…</div> : visibleTask ? (
        <>
          <div className="task-detail-heading"><span className={`status-pill ${visibleTask.status === "COMPLETED" ? "success" : ""}`}>{taskStatusLabel(visibleTask.status)}</span><small>Task {visibleTask.taskId}</small></div>
          {editingObjective ? <form className="task-objective-editor" onSubmit={(event) => { event.preventDefault(); void saveObjective(); }}>
            <label htmlFor="task-objective-edit">Objective</label>
            <textarea id="task-objective-edit" value={objectiveDraft} onChange={(event) => { setObjectiveDraft(event.target.value); setObjectiveError(null); setObjectiveSaved(null); }} maxLength={32_768} autoFocus />
            <div className="task-objective-actions"><button type="button" className="quiet-button" onClick={() => { setObjectiveDraft(visibleTask.objective); setEditingObjective(false); setObjectiveError(null); }} disabled={objectiveSaving}>Cancel</button><button type="submit" className="primary-button" disabled={objectiveSaving || !operatorReady || !objectiveDraft.trim() || objectiveDraft.trim() === visibleTask.objective}>{objectiveSaving ? "Saving…" : "Save revision"}</button></div>
          </form> : <div className="task-objective-heading"><h2>{visibleTask.objective || "Untitled Task"}</h2>{visibleTask.status === "READY" && visibleTask.currentPlanRevision === null && <button ref={objectiveEditTrigger} type="button" className="quiet-button" onClick={() => { setObjectiveDraft(visibleTask.objective); setEditingObjective(true); setObjectiveError(null); setObjectiveSaved(null); }}>Edit objective</button>}</div>}
          {visibleTask.originCoworkerId && visibleTask.originCoworkerRevision !== null && <p className="task-detail-note" aria-live="polite">
            {originCoworkerName
              ? `For ${originCoworkerName} · Coworker settings revision ${visibleTask.originCoworkerRevision} pinned when this Task was created.`
              : originCoworkerLoading
                ? `Loading the Coworker settings pinned to this Task…`
                : originCoworkerUnavailable
                  ? `Coworker settings at creation are unavailable · revision ${visibleTask.originCoworkerRevision}.`
                  : `Coworker settings revision ${visibleTask.originCoworkerRevision} pinned when this Task was created.`}
          </p>}
          <p className="task-detail-note">{visibleTask.currentPlanRevision === null ? "No accepted plan is currently saved for this Task." : "A plan is saved for this Task. Open details to review its Steps and pinned inputs."}</p>
          {visibleTask.currentPlanRevision === null && <section className="task-planning-readiness" aria-label="Local planning readiness">
            <div className="task-planning-readiness-heading">
              <div><h3>Planning availability</h3><p>This check is read-only. It does not start planning or an agent.</p></div>
              <button type="button" className="quiet-button" onClick={() => void checkPlanningReadiness()} disabled={!operatorReady || planningReadinessBusy}>
                {planningReadinessBusy ? "Checking…" : "Check readiness"}
              </button>
            </div>
            {planningReadinessError && <p className="task-stale-note" role="alert">{planningReadinessError} <button type="button" className="text-button" onClick={() => void reloadTask()} disabled={!operatorReady || busy}>Reload Task</button></p>}
            {planningReadiness && <div className="task-planning-readiness-result" role="status" aria-live="polite">
              <p>Checked {formatTaskTime(planningReadiness.observedAt)} for Task version {planningReadiness.taskVersion}.</p>
              {planningReadiness.blockers.length === 0
                ? <p>{planningReadinessNoBlockersMessage(planningReadiness.blockers)}</p>
                : <ul>{planningReadiness.blockers.map((blocker) => <li key={blocker}>{planningBlockerLabel(blocker)}</li>)}</ul>}
              {planningReadiness.blockers.length > 0 && <p>Planning dispatch is unavailable; this result does not authorize or start work.</p>}
            </div>}
          </section>}
          <details className="task-work-details">
            <summary>
              <span>Work details</span>
              <small>{visibleTask.plannedSteps.length} {visibleTask.plannedSteps.length === 1 ? "step" : "steps"} · {visibleTask.inputRefs.length} {visibleTask.inputRefs.length === 1 ? "input" : "inputs"}</small>
            </summary>
            <dl className="task-detail-meta"><div><dt>Created</dt><dd>{formatTaskTime(visibleTask.createdAt)}</dd></div><div><dt>Last updated</dt><dd>{formatTaskTime(visibleTask.updatedAt)}</dd></div><div><dt>Task specification</dt><dd>Revision {visibleTask.currentSpecRevision}</dd></div><div><dt>Current plan</dt><dd>{visibleTask.currentPlanRevision ? `Revision ${visibleTask.currentPlanRevision}` : "None accepted"}</dd></div></dl>
            <section className="task-input-list" aria-label="Pinned Task inputs">
              <h3>Inputs <span>{visibleTask.inputRefs.length}</span></h3>
              {visibleTask.inputRefs.length === 0 ? <p>No inputs are pinned to this Task.</p> : <ul>{visibleTask.inputRefs.map((input) => <li key={resourceRefKey(input)}>
                <strong>{resourceNameCache[resourceRefKey(input)] ?? `Resource ${input.resourceId}`}</strong>
                <small>Exact revision pinned · {input.revisionId}</small>
                <TaskResourceTextPreview workspaceId={workspaceId} input={{
                  workspaceId: input.workspaceId,
                  resourceId: input.resourceId,
                  revisionId: input.revisionId,
                  displayName: resourceNameCache[resourceRefKey(input)] ?? `Resource ${input.resourceId}`,
                }} />
              </li>)}</ul>}
            </section>
            {visibleTask.currentPlanRevision !== null && <section className="task-plan-list" aria-label="Task plan steps">
              <div className="task-plan-heading"><h3>Plan steps</h3><span>Revision {visibleTask.currentPlanRevision}</span></div>
              {visibleTask.planIsStale && <p className="task-plan-stale" role="status">This plan uses Task specification revision {visibleTask.planSpecRevision}; the current specification is revision {visibleTask.currentSpecRevision}.</p>}
              <ol>{visibleTask.plannedSteps.map((step) => <li key={step.stepId}>
                <span className={`task-step-mark step-${step.status.toLowerCase()}`} aria-hidden="true" />
                <div><strong>{step.title}</strong><small>{stepStatusLabel(step.status)}</small><p>{step.objective}</p></div>
              </li>)}</ol>
              <p className="task-detail-note">These are persisted plan Steps. Attempt details, Evidence, and verification are not shown in this Task view yet.</p>
            </section>}
          </details>
          <TaskSpecRevisionHistory workspaceId={workspaceId} taskId={taskId} currentRevision={visibleTask.currentSpecRevision} operatorReady={operatorReady} taskBusy={busy} taskEditing={editingObjective || objectiveSaving || pendingEdit.current !== null} onRefreshTask={() => void reloadTask()} />
          <TaskArtifactOutputs key={`${workspaceId}:${taskId}`} api={artifactApi} workspaceId={workspaceId} taskId={taskId} operatorReady={operatorReady} />
          <TaskPresentationPanel key={`${workspaceId}:${taskId}`} workspaceId={workspaceId} taskId={taskId} taskVersion={visibleTask.taskVersion} operatorReady={operatorReady} />
        </>
      ) : busy ? null : <div className="task-empty-state"><h2>Task details unavailable</h2><p>{error ?? "The Task could not be loaded from this Workspace."}</p><button className="quiet-button" type="button" onClick={() => setDetailReload(value => value + 1)} disabled={!operatorReady}>Retry Task details</button></div>}
    </section>
  );
}

type SettingsPageProps = {
  runtime: RuntimeStatus | null;
  runtimeError: string | null;
  runtimeBusy: boolean;
  startRuntime: () => Promise<void>;
  refreshRuntime: () => Promise<void>;
  agents: AgentInstallationView[];
  agentError: string | null;
  refreshAgents: () => Promise<void>;
  agentProfiles: AgentProfileView[];
  agentProfileError: string | null;
  agentBindings: AgentBindingView[];
  agentBindingError: string | null;
  agentActionMessage: string | null;
  agentActionError: string | null;
  agentActionBusy: string | null;
  runtimeBindings: RuntimeWorkspaceBindingView[];
  runtimeBindingError: string | null;
  runtimeBindingsLoading: boolean;
  runtimeEnrollmentBusy: boolean;
  runtimeEnrollmentMessage: string | null;
  runtimeEnrollmentError: string | null;
  onEnrollLocalRuntime: () => Promise<void>;
  onRefreshRuntimeBindings: () => Promise<void>;
  onProbeCodex: () => Promise<void>;
  onProbeOpenCode: () => Promise<void>;
  onRefreshAgentProfiles: () => Promise<void>;
  onRefreshAgentBindings: () => Promise<void>;
  onCreateAgentBinding: (profile: AgentProfileView, leadEligible: boolean) => Promise<void>;
  onEnableAgentBinding: (binding: AgentBindingView) => Promise<void>;
  workspaces: WorkspaceView[];
  workspaceError: string | null;
  refreshWorkspaces: () => Promise<void>;
  workspaceName: string;
  setWorkspaceName: (name: string) => void;
  createWorkspace: () => Promise<void>;
  workspaceCreateBusy: boolean;
  selectedWorkspaceId: string;
  onSelectWorkspace: (id: string) => void;
  onUpdatePolicy: (policy: string) => Promise<void>;
  policyBusy: boolean;
  onSetDefaultAgentBinding: (agentBindingId: string | null) => Promise<void>;
  defaultAgentBusy: boolean;
  instructions: string;
  onInstructionsChange: (value: string) => void;
  instructionHistory: WorkspaceInstructionView[];
  instructionMessage: string | null;
  instructionBusy: boolean;
  onSaveInstructions: () => Promise<void>;
  latestInstructionsAfterConflict: string | null;
  onUseLatestInstructions: () => void;
};

function profileHasFreshAdmissionOffer(profile: AgentProfileView): boolean {
  const latest = [...profile.observations].sort((left, right) => Date.parse(right.observedAt) - Date.parse(left.observedAt))[0];
  return !!latest
    && latest.compatible
    && Number.isFinite(Date.parse(latest.offerExpiresAt))
    && Date.parse(latest.offerExpiresAt) > Date.now()
    && (latest.readiness === "AVAILABLE" || latest.readiness === "STARTABLE" || latest.readiness === "READY");
}

function SettingsPage({ runtime, runtimeError, runtimeBusy, startRuntime, refreshRuntime, agents, agentError, refreshAgents, agentProfiles, agentProfileError, agentBindings, agentBindingError, agentActionMessage, agentActionError, agentActionBusy, onProbeCodex, onProbeOpenCode, onRefreshAgentProfiles, onRefreshAgentBindings, onCreateAgentBinding, onEnableAgentBinding, runtimeBindings, runtimeBindingError, runtimeBindingsLoading, runtimeEnrollmentBusy, runtimeEnrollmentMessage, runtimeEnrollmentError, onEnrollLocalRuntime, onRefreshRuntimeBindings, workspaces, workspaceError, refreshWorkspaces, workspaceName, setWorkspaceName, createWorkspace, workspaceCreateBusy, selectedWorkspaceId, onSelectWorkspace, onUpdatePolicy, policyBusy, onSetDefaultAgentBinding, defaultAgentBusy, instructions, onInstructionsChange, instructionHistory, instructionMessage, instructionBusy, onSaveInstructions, latestInstructionsAfterConflict, onUseLatestInstructions }: SettingsPageProps) {
  const selectedWorkspace = workspaces.find((workspace) => workspace.workspaceId === selectedWorkspaceId);
  const currentRuntimeBinding = runtimeBindings.find((binding) => binding.workspaceId === selectedWorkspaceId && binding.runtimeId === runtime?.runtimeId && binding.status === "ACTIVE")
    ?? runtimeBindings.find((binding) => binding.workspaceId === selectedWorkspaceId && binding.runtimeId === runtime?.runtimeId);
  const currentRuntimeEnrolled = currentRuntimeBinding?.status === "ACTIVE";
  const runtimeEnrollmentStatus = runtimeBindingsLoading ? "Checking…" : runtimeBindingError ? "Could not check" : !runtime?.runtimeId ? "Runtime unavailable" : currentRuntimeEnrolled ? "Active on this computer" : currentRuntimeBinding?.status === "PENDING" ? "Enrollment pending" : "Not enrolled";
  return (
    <div className="page-content subpage-content">
      <div className="eyebrow">LOCAL DESKTOP</div>
      <h1>Settings</h1>
      <section className="runtime-card" aria-labelledby="runtime-title">
        <div className="section-heading">
          <div><h2 id="runtime-title">LiteCowork Runtime</h2><p>Local daemon lifecycle and readiness.</p></div>
          <span className={`status-pill ${runtime?.state === "READY" ? "success" : "muted"}`}>{runtime?.state ?? "Checking"}</span>
        </div>
        {runtimeError && <p className="inline-error" role="alert">{runtimeError}</p>}
        {runtime?.runtimeId && <dl className="runtime-details"><div><dt>Runtime</dt><dd>{runtime.runtimeId}</dd></div><div><dt>Incarnation</dt><dd>{runtime.localIncarnationId}</dd></div></dl>}
        {!!runtime?.blockers.length && <div className="blocker-list"><strong>Unavailable services</strong><ul>{runtime.blockers.map((blocker) => <li key={blocker}>{blocker.replaceAll("_", " ").toLowerCase()}</li>)}</ul></div>}
        <div className="button-row">
          {canOfferLocalRuntimeStart(runtime) && <button className="primary-button" type="button" onClick={() => void startRuntime()} disabled={runtimeBusy}>{runtimeBusy ? "Starting…" : "Start local Runtime"}</button>}
          <button className="quiet-button" type="button" onClick={() => void refreshRuntime()}>Refresh status</button>
        </div>
      </section>
      <section className="workspace-list-card" aria-labelledby="workspace-title">
        <div className="section-heading">
          <div><h2 id="workspace-title">Workspaces</h2><p>Local Workspaces returned by the authenticated Operator API.</p></div>
          <button className="quiet-button" type="button" onClick={() => void refreshWorkspaces()}>Refresh</button>
        </div>
        <form className="workspace-create-form" onSubmit={(event) => { event.preventDefault(); void createWorkspace(); }}>
          <label className="sr-only" htmlFor="workspace-name">Workspace name</label>
          <input id="workspace-name" type="text" value={workspaceName} maxLength={160} onChange={(event) => setWorkspaceName(event.target.value)} placeholder="Name a Workspace" />
          <button className="primary-button" type="submit" disabled={workspaceCreateBusy || !workspaceName.trim() || runtime?.operatorReady !== true}>
            {workspaceCreateBusy ? "Creating…" : "Create Workspace"}
          </button>
        </form>
        {workspaceError && <p className="inline-error" role="status">{workspaceError}</p>}
        {workspaces.length === 0 ? <p className="empty-inline">Create a local Workspace to organize resources and future work.</p> : (
          <ul className="workspace-list">{workspaces.map((workspace) => <li key={workspace.workspaceId} className={selectedWorkspaceId === workspace.workspaceId ? "selected" : ""}><button type="button" onClick={() => onSelectWorkspace(workspace.workspaceId)} aria-pressed={selectedWorkspaceId === workspace.workspaceId}><strong>{workspace.name}</strong><span>{workspace.status} · {workspace.replicationPolicy === "LOCAL_ONLY" ? "this computer only" : "saved sync policy · inactive"}</span></button>{selectedWorkspaceId === workspace.workspaceId && <small>Selected</small>}</li>)}</ul>
        )}
        {selectedWorkspace && <div className="workspace-policy-form">
          <strong>Workspace storage</strong>
          {selectedWorkspace.replicationPolicy === "LOCAL_ONLY"
            ? <><span className="status-pill muted">This computer only</span><small>V1 keeps this Workspace on this computer. Cloud continuation and remote Runtimes are not active.</small></>
            : <><span className="status-pill muted">Saved policy · {selectedWorkspace.replicationPolicy.replaceAll("_", " ")} · inactive</span><small>This local V1 build does not transfer Workspace data to cloud or remote Runtimes. The saved policy is not being applied.</small><button className="quiet-button" type="button" disabled={policyBusy || selectedWorkspace.status !== "ACTIVE"} onClick={() => void onUpdatePolicy("LOCAL_ONLY")}>{policyBusy ? "Updating…" : "Set this computer only"}</button></>}
        </div>}
        {selectedWorkspace && <div className="workspace-instructions-form">
          <div className="section-heading">
            <div><h3>Workspace instructions</h3><p>Shared guidance for future work in this Workspace.</p></div>
            <span className="status-pill muted">{instructionHistory.length > 0 ? `Revision ${instructionHistory[instructionHistory.length - 1].revision}` : "Not set"}</span>
          </div>
          <label htmlFor="workspace-instructions">Instructions</label>
          <textarea
            id="workspace-instructions"
            rows={7}
            value={instructions}
            maxLength={65536}
            disabled={instructionBusy || selectedWorkspace.status !== "ACTIVE"}
            onChange={(event) => onInstructionsChange(event.target.value)}
            placeholder="Add project conventions, preferred workflows, and context that should guide future Tasks."
            aria-describedby="workspace-instructions-help"
          />
          <small id="workspace-instructions-help">Saved as an immutable revision from a local text Resource. Limit: 64 KiB UTF-8. Instructions do not grant permissions and do not change existing Tasks.</small>
          <div className="button-row">
            <button className="primary-button" type="button" onClick={() => void onSaveInstructions()} disabled={instructionBusy || selectedWorkspace.status !== "ACTIVE"}>
              {instructionBusy ? "Saving…" : "Save new revision"}
            </button>
            <span className="resource-search-hint">{new TextEncoder().encode(instructions).byteLength.toLocaleString()} / 65,536 bytes</span>
          </div>
          {instructionMessage && <p className={instructionMessage.includes("could not") || instructionMessage.includes("unavailable") || instructionMessage.includes("limited") ? "inline-error" : "inline-status"} role="status">{instructionMessage}</p>}
          {latestInstructionsAfterConflict !== null && <section className="instruction-conflict" aria-label="Latest saved Workspace instructions">
            <div className="section-heading"><div><h4>Latest saved version</h4><p>Your unsaved draft remains in the editor above.</p></div><button className="quiet-button" type="button" onClick={onUseLatestInstructions}>Use this version</button></div>
            <pre>{latestInstructionsAfterConflict || "(This revision is empty.)"}</pre>
          </section>}
          {instructionHistory.length > 0 && <details className="instruction-history">
            <summary>Revision history and source provenance ({instructionHistory.length})</summary>
            <p className="resource-search-hint">These are the immutable Workspace instruction sources returned by the authenticated Operator. They describe saved Workspace guidance; they do not claim which sources a Task or agent actually consumed.</p>
            <ol>{[...instructionHistory].reverse().map((revision) => <li key={`${revision.workspaceId}-${revision.revision}`}>
              <strong>Revision {revision.revision}</strong>
              <small>{new Date(revision.createdAt).toLocaleString()} · {revision.contentDigest.slice(0, 19)}…</small>
              <small>Source Resource: {revision.contentRef.resource_id ?? "Unavailable"} · revision: {revision.contentRef.revision_id ?? "Unavailable"}</small>
              <small>Parent instruction revisions: {revision.parentRevisions.length ? revision.parentRevisions.join(", ") : "None (initial revision)"}</small>
            </li>)}</ol>
          </details>}
        </div>}
      </section>
      <section className="workspace-list-card runtime-enrollment-card" aria-labelledby="runtime-enrollment-title">
        <div className="section-heading">
          <div><h2 id="runtime-enrollment-title">This computer’s Workspace access</h2><p>Enrollment authorizes this local Runtime installation for the selected Workspace.</p></div>
          <button className="quiet-button" type="button" onClick={() => void onRefreshRuntimeBindings()} disabled={runtime?.operatorReady !== true || !selectedWorkspaceId}>Refresh</button>
        </div>
        {!selectedWorkspace ? <p className="agent-empty-note">Select a Workspace above to check this computer’s enrollment.</p> : <>
          <div className="runtime-enrollment-status">
            <span className={`status-pill ${currentRuntimeEnrolled ? "success" : currentRuntimeBinding?.status === "PENDING" ? "warning" : "muted"}`}>{runtimeEnrollmentStatus}</span>
            <span>{selectedWorkspace.name}</span>
          </div>
          {currentRuntimeBinding && <dl className="agent-observation-details"><div><dt>Runtime</dt><dd>{currentRuntimeBinding.runtimeId}</dd></div><div><dt>Enrollment</dt><dd>{currentRuntimeBinding.enrollmentMode.replaceAll("_", " ").toLowerCase()}</dd></div><div><dt>Roles</dt><dd>{currentRuntimeBinding.roles.map((role) => role.replaceAll("_", " ").toLowerCase()).join(", ") || "None reported"}</dd></div></dl>}
          <p className="runtime-enrollment-copy">This grants this installation its authorized local execution role in the Workspace. It does not pair another computer, start an agent, or execute a Task.</p>
          {runtimeBindingError && <p className="inline-error" role="status">{runtimeBindingError}</p>}
          {runtimeEnrollmentError && <p className="inline-error" role="alert">{runtimeEnrollmentError}</p>}
          {runtimeEnrollmentMessage && <p className="inline-status" role="status">{runtimeEnrollmentMessage}</p>}
          {!currentRuntimeEnrolled && <div className="button-row"><button className="primary-button" type="button" onClick={() => void onEnrollLocalRuntime()} disabled={runtimeEnrollmentBusy || runtimeBindingsLoading || runtimeBindingError !== null || runtime?.operatorReady !== true || !runtime?.runtimeId || selectedWorkspace.status !== "ACTIVE"}>{runtimeEnrollmentBusy ? "Enrolling this computer…" : "Enroll this computer"}</button></div>}
        </>}
      </section>
      <AgentCatalogSettings
        workspaceId={selectedWorkspaceId || null}
        workspaceName={selectedWorkspace?.name}
        workspaceActive={selectedWorkspace?.status === "ACTIVE"}
        runtimeReady={runtime?.operatorReady === true}
        runtimeEnrolled={currentRuntimeEnrolled === true}
        selectedDefaultLeadBindingId={selectedWorkspace?.defaultAgentBindingId ?? null}
        installations={agents}
        profiles={agentProfiles}
        bindings={agentBindings}
        error={[agentError, agentProfileError, agentBindingError].filter(Boolean).join(" ") || null}
        actionMessage={agentActionMessage}
        actionError={agentActionError}
        busyAction={agentActionBusy}
        defaultAgentBusy={defaultAgentBusy}
        onRefresh={() => { void refreshAgents(); void onRefreshAgentProfiles(); void onRefreshAgentBindings(); }}
        onProbeCodex={() => { void onProbeCodex(); }}
        onProbeOpenCode={() => { void onProbeOpenCode(); }}
        onCreateBinding={(profile, leadEligible) => { void onCreateAgentBinding(profile, leadEligible); }}
        onEnableBinding={(binding) => { void onEnableAgentBinding(binding); }}
        onSetDefaultLead={(agentBindingId) => { void onSetDefaultAgentBinding(agentBindingId); }}
      />
      <p className="settings-note">Operator API serving confirms the authenticated desktop connection only. The Runtime remains degraded until Task recovery and execution services are implemented; it does not yet accept work or start coding agents.</p>
    </div>
  );
}

function PlaceholderPage({ page, onSettings }: { page: Page; onSettings: () => void }) {
  const content: Record<Page, { title: string; description: string }> = {
    Home: { title: "Home", description: "Your workspace overview." },
    Conversations: { title: "Conversations", description: "Saved local conversation history." },
    Work: { title: "Work", description: "Durable Tasks and their real progress will appear here after Task execution and the Operator API are integrated." },
    Library: { title: "Library", description: "Saved files and committed Artifacts for this Workspace." },
    "Needs You": { title: "Needs You", description: "Approvals and blockers will appear here when the authenticated Operator API is integrated." },
    Ideas: { title: "Ideas", description: "Read-only proposals from approved Suggestion producers. Viewing an idea never starts work." },
    Coworkers: { title: "Coworkers", description: "Workspace assistants and their saved settings." },
    Goals: { title: "Goals", description: "Workspace outcomes and their linked, evidence-based progress." },
    Routines: { title: "Routines", description: "Reusable work definitions. Saving a Routine does not schedule or run it." },
    Automations: { title: "Automations", description: "Saved trigger definitions. Trigger hosting and Task creation are not available yet." },
    Settings: { title: "Settings", description: "Local Runtime status and startup are available in the desktop shell." },
  };
  const item = content[page];
  return (
    <div className="page-content subpage-content">
      <div className="eyebrow">LITE COWORK</div>
      <h1>{item.title}</h1>
      <div className="empty-panel subpage-panel">
        <div className="empty-icon" aria-hidden="true">{page === "Library" ? "▧" : page === "Needs You" ? "◇" : "▤"}</div>
        <h2>Nothing in {page.toLowerCase()} yet</h2>
        <p>{item.description}</p>
        <button className="text-button" type="button" onClick={onSettings}>View Runtime status <span aria-hidden="true">→</span></button>
      </div>
    </div>
  );
}

function LibraryPage({ selectedWorkspaceId, artifactApi, onOpenArtifactSource, rootStatusRequestKeys, resourceIndexRequestKeys, workspaceVersion, operatorReady, resources, onRevisionCommitted, onRefreshResources, selectedTaskInputs, onToggleTaskInput, onClearTaskInputs, onUseSelectedInTask, nextCursor, pageBusy, onLoadMore, busy, pausePending, onPauseImport, message, onFiles, onPreview, onClosePreview, previewResourceId, previewResourceName, preview, previewHighlights, previewError }: { selectedWorkspaceId: string; artifactApi: ReturnType<typeof desktopArtifactApi>; onOpenArtifactSource: (ref: PinnedResourceRef) => void; rootStatusRequestKeys: { current: Map<string, string> }; resourceIndexRequestKeys: { current: Map<string, string> }; workspaceVersion: number | null; operatorReady: boolean; resources: ResourceView[]; onRevisionCommitted: (resource: ResourceView) => void; onRefreshResources: () => void; selectedTaskInputs: TaskInputSelection[]; onToggleTaskInput: (resource: { resourceId: string; resourceRevisionId: string; displayName: string }) => void; onClearTaskInputs: () => void; onUseSelectedInTask: () => void; nextCursor: string | null; pageBusy: boolean; onLoadMore: () => Promise<void>; busy: boolean; pausePending: boolean; onPauseImport: () => void; message: string | null; onFiles: (files: FileList | File[] | null, contextDocument?: ContextDocumentCreateMetadata | null) => Promise<void>; onPreview: (resourceId: string, displayName: string, revisionId: string, searchPin?: PreviewSearchPin) => Promise<void>; onClosePreview: () => void; previewResourceId: string | null; previewResourceName: string | null; preview: string | null; previewHighlights: SourceHighlightResult | null; previewError: string | null }) {
  const folderInput = useRef<HTMLInputElement>(null);
  const rootListGeneration = useRef(0);
  const [workspaceRoots, setWorkspaceRoots] = useState<WorkspaceRootView[]>([]);
  const [rootNextCursor, setRootNextCursor] = useState<string | null>(null);
  const [rootBusy, setRootBusy] = useState(false);
  const [rootAdding, setRootAdding] = useState(false);
  const [rootRevokingId, setRootRevokingId] = useState<string | null>(null);
  const [rootStatusAction, setRootStatusAction] = useState<{ workspaceRootId: string; action: "pause" | "resume" } | null>(null);
  const [rootError, setRootError] = useState<string | null>(null);
  const [rootMessage, setRootMessage] = useState<string | null>(null);
  const searchRequest = useRef(0);
  const [resourceIndexRefreshNonce, setResourceIndexRefreshNonce] = useState(0);
  const [query, setQuery] = useState("");
  const [searchResults, setSearchResults] = useState<ResourceSearchResultView[]>([]);
  const [searchCursor, setSearchCursor] = useState<string | null>(null);
  const [searchBusy, setSearchBusy] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [searchMode, setSearchMode] = useState<"METADATA" | "ON_DEMAND_CONTENT" | "INDEXED_CONTENT">("METADATA");
  const [searchKind, setSearchKind] = useState("");
  const [searchFreshness, setSearchFreshness] = useState("");
  const [contentScanInfo, setContentScanInfo] = useState<ResourceContentScanView | null>(null);
  const [rebuildBusyResourceId, setRebuildBusyResourceId] = useState<string | null>(null);
  const [saveAsBusyResourceId, setSaveAsBusyResourceId] = useState<string | null>(null);
  const [saveAsMessage, setSaveAsMessage] = useState<string | null>(null);
  const [saveAsError, setSaveAsError] = useState<string | null>(null);
  const [indexActionMessage, setIndexActionMessage] = useState<string | null>(null);
  const [indexActionError, setIndexActionError] = useState<string | null>(null);
  const [indexRetryPin, setIndexRetryPin] = useState<{ workspaceId: string; resourceId: string; revisionId: string; contentDigest: string } | null>(null);
  const [dropActive, setDropActive] = useState(false);
  const [contextNoteTitle, setContextNoteTitle] = useState("");
  const [contextNoteBody, setContextNoteBody] = useState("");
  const [contextNoteError, setContextNoteError] = useState<string | null>(null);
  const [revisionResource, setRevisionResource] = useState<ResourceView | null>(null);
  const createWorkspaceNote = () => {
    const title = contextNoteTitle.trim();
    const body = contextNoteBody;
    if (!selectedWorkspaceId || !title || !body.trim() || busy) return;
    if (new TextEncoder().encode(body).byteLength > 64 * 1024) {
      setContextNoteError("Workspace notes are limited to 64 KiB of UTF-8 text.");
      return;
    }
    setContextNoteError(null);
    const safeTitle = title.normalize("NFKC").replace(/[^\p{L}\p{N} _-]/gu, "-").replace(/\s+/gu, " ").replace(/^[-. ]+|[-. ]+$/gu, "").slice(0, 80) || "Workspace note";
    const file = new File([body], `${safeTitle}.md`, { type: "text/markdown", lastModified: 0 });
    void onFiles([file], {
      kind: "WORKSPACE_NOTES",
      owner_ref: { kind: "WORKSPACE", workspace_id: selectedWorkspaceId },
    });
  };
  useEffect(() => { folderInput.current?.setAttribute("webkitdirectory", ""); }, []);
  useEffect(() => {
    setQuery("");
    setSearchResults([]);
    setSearchCursor(null);
    setSearchError(null);
    setContentScanInfo(null);
    setSearchMode("METADATA");
    setSearchKind("");
    setSearchFreshness("");
    setRevisionResource(null);
    setIndexActionMessage(null);
    setIndexActionError(null);
  }, [selectedWorkspaceId]);
  useEffect(() => {
    const generation = ++rootListGeneration.current;
    setWorkspaceRoots([]);
    setRootNextCursor(null);
    setRootError(null);
    setRootMessage(null);
    setRootStatusAction(null);
    setRootBusy(false);
    if (!selectedWorkspaceId || !operatorReady) return;
    setRootBusy(true);
    void invoke<WorkspaceRootPageView>("list_workspace_roots", { workspaceId: selectedWorkspaceId, cursor: null })
      .then((page) => {
        if (generation !== rootListGeneration.current) return;
        setWorkspaceRoots(page.items);
        setRootNextCursor(page.nextCursor);
      })
      .catch((error) => {
        if (generation === rootListGeneration.current) {
          setRootError(typeof error === "string" ? error : error instanceof Error ? error.message : "Persistent folders could not be loaded.");
        }
      })
      .finally(() => { if (generation === rootListGeneration.current) setRootBusy(false); });
    return () => { rootListGeneration.current += 1; };
  }, [selectedWorkspaceId, operatorReady]);
  useEffect(() => {
    const normalizedQuery = query.trim();
    const request = ++searchRequest.current;
    const hasSearchFilters = Boolean(searchKind || searchFreshness);
    const metadataFilterOnly = searchMode === "METADATA" && hasSearchFilters;
    if ((!normalizedQuery && !metadataFilterOnly) || !selectedWorkspaceId) {
      setSearchResults([]);
      setSearchCursor(null);
      setSearchBusy(false);
      setSearchError(null);
      setContentScanInfo(null);
      return;
    }
    setSearchResults([]);
    setSearchCursor(null);
    setContentScanInfo(null);
    if (busy) {
      setSearchBusy(false);
      return;
    }
    const timer = window.setTimeout(() => {
      setSearchBusy(true);
      setSearchError(null);
      void invoke<ResourceSearchPageView>("search_resources", {
        workspaceId: selectedWorkspaceId,
        query: normalizedQuery,
        mode: searchMode,
        kind: searchKind || null,
        freshness: searchFreshness || null,
        cursor: null,
      }).then((page) => {
        if (request === searchRequest.current) {
          setSearchResults(page.items);
          setSearchCursor(page.nextCursor);
          setContentScanInfo(page.contentScan);
        }
      }).catch((error) => {
        if (request === searchRequest.current) {
          setSearchError(typeof error === "string" ? error : error instanceof Error ? error.message : "Workspace search is unavailable.");
          setSearchResults([]);
          setSearchCursor(null);
          setContentScanInfo(null);
        }
      }).finally(() => {
        if (request === searchRequest.current) setSearchBusy(false);
      });
    }, 220);
    return () => window.clearTimeout(timer);
  }, [query, selectedWorkspaceId, resources, busy, searchMode, searchKind, searchFreshness, resourceIndexRefreshNonce]);
  const rebuildResourceTextIndex = async (resource: ResourceView) => {
    if (!selectedWorkspaceId || !operatorReady || rebuildBusyResourceId) return;
    const pendingKey = `${selectedWorkspaceId}:${resource.resourceId}:${resource.resourceRevisionId}:${resource.contentDigest}`;
    const requestId = resourceIndexRequestKeys.current.get(pendingKey) ?? crypto.randomUUID();
    resourceIndexRequestKeys.current.set(pendingKey, requestId);
    setRebuildBusyResourceId(resource.resourceId);
    setIndexActionMessage(null);
    setIndexActionError(null);
    try {
      const result = await invoke<ResourceTextIndexRebuildView>("rebuild_resource_text_index", {
        workspaceId: selectedWorkspaceId,
        resourceId: resource.resourceId,
        resourceRevisionId: resource.resourceRevisionId,
        contentDigest: resource.contentDigest,
        requestId,
      });
      if (result.workspaceId !== selectedWorkspaceId
        || result.resourceId !== resource.resourceId
        || result.resourceRevisionId !== resource.resourceRevisionId
        || result.contentDigest !== resource.contentDigest
        || result.requestId !== requestId) {
        throw new Error("The local Runtime returned a result for a different Resource revision.");
      }
      resourceIndexRequestKeys.current.delete(pendingKey);
      setIndexRetryPin((current) => current?.workspaceId === selectedWorkspaceId
        && current.resourceId === resource.resourceId
        && current.revisionId === resource.resourceRevisionId
        && current.contentDigest === resource.contentDigest ? null : current);
      setResourceIndexRefreshNonce((current) => current + 1);
      if (result.outcome === "INDEXED") {
        setIndexActionMessage(`Encrypted local text index rebuilt for “${resource.displayName}”.`);
      } else {
        const reason = result.reason === "OVER_SIZE_LIMIT" ? "it is over the 1 MiB limit"
          : result.reason === "INVALID_UTF8" ? "it is not valid UTF-8 text"
            : result.reason === "CONTROL_CHARACTERS" ? "it contains unsupported control characters"
              : result.reason === "TERM_LIMIT_EXCEEDED" ? "it exceeds the local term limit"
                : "this file type is not supported";
        setIndexActionMessage(`“${resource.displayName}” was not indexed because ${reason}.`);
      }
    } catch (error) {
      const message = typeof error === "string" ? error : error instanceof Error ? error.message : "The local text index could not be rebuilt.";
      if (message.includes("Reload the Library")) {
        setIndexRetryPin(null);
      } else {
        setIndexRetryPin({
          workspaceId: selectedWorkspaceId,
          resourceId: resource.resourceId,
          revisionId: resource.resourceRevisionId,
          contentDigest: resource.contentDigest,
        });
      }
      setIndexActionError(message);
    } finally {
      setRebuildBusyResourceId(null);
    }
  };
  const saveResourceAs = async (resource: ResourceView) => {
    if (!selectedWorkspaceId || !operatorReady || saveAsBusyResourceId) return;
    setSaveAsBusyResourceId(resource.resourceId);
    setSaveAsMessage(null);
    setSaveAsError(null);
    try {
      const result = await invoke<{ status: string }>("resource_save_as", {
        workspaceId: selectedWorkspaceId,
        resourceId: resource.resourceId,
        expectedResourceRevisionId: resource.resourceRevisionId,
        expectedContentDigest: resource.contentDigest,
        expectedSizeBytes: resource.sizeBytes,
        expectedMediaType: resource.mediaType,
      });
      if (!result || Object.keys(result).join(",") !== "status" || !["SAVED", "CANCELLED"].includes(result.status)) {
        throw new Error("Resource Save As response is invalid.");
      }
      if (result.status === "SAVED") setSaveAsMessage(`${resource.displayName} was saved to the selected location.`);
    } catch (error) {
      setSaveAsError(typeof error === "string" ? error : error instanceof Error ? error.message : "Resource could not be saved.");
    } finally {
      setSaveAsBusyResourceId(null);
    }
  };
  const loadMoreSearchResults = async () => {
    const normalizedQuery = query.trim();
    const cursor = searchCursor;
    const request = searchRequest.current;
    const metadataFilterOnly = searchMode === "METADATA" && Boolean(searchKind || searchFreshness);
    if ((!normalizedQuery && !metadataFilterOnly) || !selectedWorkspaceId || !cursor || searchBusy) return;
    setSearchBusy(true);
    setSearchError(null);
    try {
      const page = await invoke<ResourceSearchPageView>("search_resources", {
        workspaceId: selectedWorkspaceId,
        query: normalizedQuery,
        mode: searchMode,
        kind: searchKind || null,
        freshness: searchFreshness || null,
        cursor,
      });
      if (request !== searchRequest.current) return;
      const seen = new Set(searchResults.map((item) => item.resourceId));
      if (page.items.some((item) => seen.has(item.resourceId)) || page.nextCursor === cursor) throw new Error("Workspace search returned a repeated page or cursor.");
      setSearchResults((current) => [...current, ...page.items.filter((item) => !current.some((existing) => existing.resourceId === item.resourceId))]);
      setSearchCursor(page.nextCursor);
      setContentScanInfo(page.contentScan);
    } catch (error) {
      if (request === searchRequest.current) setSearchError(typeof error === "string" ? error : error instanceof Error ? error.message : "More search results could not be loaded.");
    } finally {
      if (request === searchRequest.current) setSearchBusy(false);
    }
  };
  const addPersistentFolder = async () => {
    if (!selectedWorkspaceId || workspaceVersion === null || rootAdding || busy) return;
    const generation = rootListGeneration.current;
    setRootAdding(true);
    setRootError(null);
    setRootMessage(null);
    try {
      const root = await invoke<WorkspaceRootView | null>("add_workspace_root", {
        workspaceId: selectedWorkspaceId,
        expectedWorkspaceVersion: workspaceVersion,
        watchPolicy: "METADATA",
        replicationPolicy: "NONE",
        requestId: crypto.randomUUID(),
      });
      if (!root) return;
      if (generation !== rootListGeneration.current) return;
      if (root.workspaceId !== selectedWorkspaceId || root.status !== "ACTIVE") {
        throw new Error("The Runtime returned a folder outside this Workspace.");
      }
      setWorkspaceRoots((current) => [root, ...current.filter((item) => item.workspaceRootId !== root.workspaceRootId)]);
      setRootMessage(`“${root.displayName}” was saved as a persistent folder scope. Folder watching and indexing are not active in this build.`);
    } catch (error) {
      if (generation === rootListGeneration.current) setRootError(typeof error === "string" ? error : error instanceof Error ? error.message : "The persistent folder could not be added.");
    } finally {
      setRootAdding(false);
    }
  };
  const loadMoreRoots = async () => {
    const cursor = rootNextCursor;
    const generation = rootListGeneration.current;
    if (!selectedWorkspaceId || !cursor || rootBusy) return;
    setRootBusy(true);
    setRootError(null);
    try {
      const page = await invoke<WorkspaceRootPageView>("list_workspace_roots", { workspaceId: selectedWorkspaceId, cursor });
      if (generation !== rootListGeneration.current) return;
      setWorkspaceRoots((current) => [...current, ...page.items.filter((item) => !current.some((existing) => existing.workspaceRootId === item.workspaceRootId))]);
      setRootNextCursor(page.nextCursor);
    } catch (error) {
      if (generation === rootListGeneration.current) setRootError(typeof error === "string" ? error : error instanceof Error ? error.message : "More persistent folders could not be loaded.");
    } finally {
      if (generation === rootListGeneration.current) setRootBusy(false);
    }
  };
  const revokePersistentFolder = async (root: WorkspaceRootView) => {
    if (!selectedWorkspaceId || root.workspaceId !== selectedWorkspaceId || rootRevokingId) return;
    const confirmed = window.confirm(`Remove LiteCowork's saved access to “${root.displayName}”? Existing replicated copies are not deleted.`);
    if (!confirmed) return;
    const generation = rootListGeneration.current;
    setRootRevokingId(root.workspaceRootId);
    setRootError(null);
    setRootMessage(null);
    try {
      const revoked = await invoke<WorkspaceRootView>("revoke_workspace_root", {
        workspaceId: selectedWorkspaceId,
        workspaceRootId: root.workspaceRootId,
        expectedVersion: root.version,
        requestId: crypto.randomUUID(),
      });
      if (generation !== rootListGeneration.current) return;
      setWorkspaceRoots((current) => current.map((item) => item.workspaceRootId === revoked.workspaceRootId ? revoked : item));
      setRootMessage(`Access to “${revoked.displayName}” was revoked. Existing replicated copies are unchanged.`);
    } catch (error) {
      if (generation === rootListGeneration.current) setRootError(typeof error === "string" ? error : error instanceof Error ? error.message : "Persistent folder access could not be removed.");
    } finally {
      setRootRevokingId(null);
    }
  };
  const updatePersistentFolderStatus = async (root: WorkspaceRootView, action: "pause" | "resume") => {
    const expectedStatus = action === "pause" ? "ACTIVE" : "PAUSED";
    if (!selectedWorkspaceId || root.workspaceId !== selectedWorkspaceId || root.status !== expectedStatus
      || rootStatusAction || rootRevokingId) return;
    if (!Number.isSafeInteger(root.version) || root.version < 1 || root.version >= Number.MAX_SAFE_INTEGER) {
      setRootError("This folder version cannot be safely updated. Reload the Library before trying again.");
      return;
    }
    const generation = rootListGeneration.current;
    const requestKey = JSON.stringify([selectedWorkspaceId, root.workspaceRootId, action, root.version]);
    const requestId = rootStatusRequestKeys.current.get(requestKey) ?? crypto.randomUUID();
    rootStatusRequestKeys.current.set(requestKey, requestId);
    setRootStatusAction({ workspaceRootId: root.workspaceRootId, action });
    setRootError(null);
    setRootMessage(null);
    try {
      const updated = await invoke<WorkspaceRootView>(action === "pause" ? "pause_workspace_root" : "resume_workspace_root", {
        workspaceId: selectedWorkspaceId,
        workspaceRootId: root.workspaceRootId,
        expectedVersion: root.version,
        requestId,
      });
      const committedStatus = action === "pause" ? "PAUSED" : "ACTIVE";
      if (updated.workspaceId !== selectedWorkspaceId
        || updated.workspaceRootId !== root.workspaceRootId
        || updated.status !== committedStatus
        || updated.version !== root.version + 1
        || (action === "resume" && updated.locationAvailability !== "AVAILABLE")) {
        throw new Error("The Runtime did not confirm this folder status change.");
      }
      rootStatusRequestKeys.current.delete(requestKey);
      if (generation !== rootListGeneration.current) return;
      setWorkspaceRoots((current) => current.map((item) => item.workspaceRootId === updated.workspaceRootId ? updated : item));
      setRootMessage(action === "pause"
        ? `“${updated.displayName}” is paused. Its saved permission remains; folder watching and indexing are not active in this build.`
        : `“${updated.displayName}” is active again. Folder watching and indexing are not active in this build.`);
    } catch (error) {
      if (generation === rootListGeneration.current) setRootError(typeof error === "string" ? error : error instanceof Error ? error.message : "Persistent folder status could not be updated.");
    } finally {
      if (generation === rootListGeneration.current) setRootStatusAction(null);
    }
  };
  const hasSearchFilters = Boolean(searchKind || searchFreshness);
  const searching = query.trim().length > 0 || hasSearchFilters;
  const searchPageBusy = searchBusy;
  return (
    <div className="page-content subpage-content">
      <div className="eyebrow">WORKSPACE FILES</div>
      <div className="section-heading library-heading"><div><h1>Library</h1><p>Store and find files in this Workspace. Adding a file does not attach it to a Task.</p></div></div>
      <ZipIntakeNotice workspaceId={selectedWorkspaceId} operatorReady={operatorReady} />
      {selectedTaskInputs.length > 0 && <section className="task-input-selection" aria-label="Files selected for a Task">
        <div><strong>{selectedTaskInputs.length} file{selectedTaskInputs.length === 1 ? "" : "s"} selected for a Task</strong><p>Each selection pins the exact Resource revision. The files are not read by an agent until Task execution is available.</p></div>
        <div className="task-input-selection-actions"><button type="button" className="quiet-button" onClick={onClearTaskInputs}>Clear</button><button type="button" className="primary-button" onClick={onUseSelectedInTask}>Use in Task</button></div>
      </section>}
      {!selectedWorkspaceId && <p className="inline-error" role="status">Select or create a Workspace before adding files.</p>}
      <section className="persistent-folders" aria-label="Persistent folder scopes">
        <div className="section-heading"><div><h2>Persistent folders</h2><p>Register a selected folder as a persistent scope. LiteCowork does not yet watch or index its contents in this build.</p></div><button className="quiet-button" type="button" disabled={!selectedWorkspaceId || !operatorReady || workspaceVersion === null || rootAdding || busy} onClick={() => void addPersistentFolder()}>{rootAdding ? "Opening folder picker…" : "Add persistent folder"}</button></div>
        {!operatorReady && selectedWorkspaceId && <p className="inline-status" role="status">Start the local Runtime to manage persistent folders.</p>}
        {rootBusy && workspaceRoots.length === 0 && <p className="inline-status" role="status">Loading persistent folders…</p>}
        {rootError && <p className="inline-error" role="status">{rootError}</p>}
        {rootMessage && <p className="inline-status" role="status" aria-live="polite">{rootMessage}</p>}
        {workspaceRoots.length > 0 && <ul className="resource-list persistent-folder-list">{workspaceRoots.map((root) => {
          const actionPending = rootStatusAction?.workspaceRootId === root.workspaceRootId;
          const controlsDisabled = !operatorReady || rootBusy || rootStatusAction !== null || rootRevokingId !== null;
          return <li key={root.workspaceRootId}>
            <span className="resource-file-icon" aria-hidden="true">▱</span>
            <span><strong>{root.displayName}</strong><small>{root.status === "PAUSED" && root.locationAvailability === "UNAVAILABLE"
              ? "Paused · folder identity could not be verified. Remove this saved scope and add the intended folder again."
              : workspaceRootStatusLabel(root.status)}</small></span>
            <span className="resource-preview-unavailable">Metadata policy · inactive</span>
            <span className="persistent-folder-actions">
              {root.status === "ACTIVE" && <button className="quiet-button" type="button" aria-label={`Pause ${root.displayName}`} disabled={controlsDisabled} onClick={() => void updatePersistentFolderStatus(root, "pause")}>
                {actionPending && rootStatusAction?.action === "pause" ? "Pausing…" : "Pause"}
              </button>}
              {root.status === "PAUSED" && <button className="quiet-button" type="button" aria-label={`Resume ${root.displayName}`} disabled={controlsDisabled} onClick={() => void updatePersistentFolderStatus(root, "resume")}>
                {actionPending && rootStatusAction?.action === "resume" ? "Checking folder…" : "Revalidate & resume"}
              </button>}
              {root.status !== "REVOKED" && <button className="quiet-button" type="button" disabled={controlsDisabled} onClick={() => void revokePersistentFolder(root)}>
                {rootRevokingId === root.workspaceRootId ? "Removing…" : "Remove access"}
              </button>}
            </span>
          </li>;
        })}</ul>}
        {rootNextCursor && <div className="load-more-row"><button className="quiet-button" type="button" disabled={rootBusy} onClick={() => void loadMoreRoots()}>{rootBusy ? "Loading…" : "Load more folders"}</button></div>}
      </section>
      <section
        className={`resource-intake${dropActive ? " drop-active" : ""}${!selectedWorkspaceId ? " intake-disabled" : ""}`}
        aria-label="Add files to this Workspace"
        onDragEnter={(event) => { event.preventDefault(); if (selectedWorkspaceId && !busy && Array.from(event.dataTransfer.types).includes("Files")) setDropActive(true); }}
        onDragOver={(event) => { event.preventDefault(); if (selectedWorkspaceId && !busy && Array.from(event.dataTransfer.types).includes("Files")) event.dataTransfer.dropEffect = "copy"; }}
        onDragLeave={(event) => { if (!event.currentTarget.contains(event.relatedTarget as Node | null)) setDropActive(false); }}
        onDrop={(event) => { event.preventDefault(); setDropActive(false); if (selectedWorkspaceId && !busy && event.dataTransfer.files.length > 0) void onFiles(event.dataTransfer.files); }}
      >
        <div className="resource-intake-copy">
          <span className="resource-intake-icon" aria-hidden="true">＋</span>
          <div>
            <h2>{dropActive ? "Drop files to add them" : busy ? "Adding files to this Workspace…" : "Add files to this Workspace"}</h2>
            <p>Choose files, a folder, or a ZIP archive. You can also drag files here.</p>
          </div>
        </div>
        <div className="resource-intake-actions">
          {busy && <button className="quiet-button" type="button" disabled={pausePending} onClick={onPauseImport}>{pausePending ? "Pausing…" : "Pause upload"}</button>}
          <label className="quiet-button file-picker-button">
            Add folder
            <input ref={folderInput} type="file" multiple disabled={busy || !selectedWorkspaceId} onChange={(event) => { void onFiles(event.currentTarget.files); event.currentTarget.value = ""; }} />
          </label>
          <label className="primary-button file-picker-button">
            {busy ? "Adding…" : "Choose files"}
            <input type="file" multiple disabled={busy || !selectedWorkspaceId} accept=".zip,.pdf,.txt,.md,.csv,.json,.docx,.xlsx,.pptx,image/*" onChange={(event) => { void onFiles(event.currentTarget.files); event.currentTarget.value = ""; }} />
          </label>
        </div>
        <ul className="resource-intake-facts">
          <li>One-time copy of the selected files; a folder is not watched for future changes.</li>
          <li>ZIP files are saved intact. Their contents are not unpacked.</li>
          <li>Up to 100 files and 100 MiB per selection; likely secret and generated files are skipped.</li>
        </ul>
        <div className="context-note-create" aria-label="Create a Workspace note">
          <h3>Write a Workspace note</h3>
          <p>This creates a Workspace-scoped text Resource with revision history. It is not automatically sent to agents and does not enable semantic RAG.</p>
          <label htmlFor="workspace-note-title">Title</label>
          <input id="workspace-note-title" type="text" maxLength={80} value={contextNoteTitle} onChange={(event) => { setContextNoteTitle(event.currentTarget.value); setContextNoteError(null); }} placeholder="Project conventions" disabled={busy || !selectedWorkspaceId} />
          <label htmlFor="workspace-note-body">Note</label>
          <textarea id="workspace-note-body" value={contextNoteBody} onChange={(event) => { setContextNoteBody(event.currentTarget.value); setContextNoteError(null); }} maxLength={64 * 1024} rows={4} placeholder="Write a short note for this Workspace…" disabled={busy || !selectedWorkspaceId} />
          <div className="context-note-footer"><small>Stored only in this Workspace. Maximum 64 KiB of UTF-8 text.</small><button className="quiet-button" type="button" disabled={!operatorReady || busy || !contextNoteTitle.trim() || !contextNoteBody.trim()} onClick={createWorkspaceNote}>Save Workspace note</button></div>
          {contextNoteError && <p className="inline-error" role="alert">{contextNoteError}</p>}
        </div>
      </section>
      {message && <p className="inline-status resource-intake-status" role="status" aria-live="polite">{message}</p>}
      {indexActionMessage && <p className="inline-status" role="status" aria-live="polite">{indexActionMessage}</p>}
      {indexActionError && <p className="inline-error" role="alert">{indexActionError}{indexRetryPin?.workspaceId === selectedWorkspaceId ? " Retry stays pinned to the same Resource revision in this desktop session." : ""}</p>}
      {saveAsMessage && <p className="inline-status" role="status" aria-live="polite">{saveAsMessage}</p>}
      {saveAsError && <p className="inline-error" role="alert">{saveAsError}</p>}
      {indexActionError?.includes("Reload the Library") && <button className="quiet-button" type="button" disabled={!operatorReady} onClick={() => { setIndexActionError(null); onRefreshResources(); }}>Reload files</button>}
      {resources.length === 0 ? <div className="empty-panel subpage-panel"><div className="empty-icon" aria-hidden="true">▧</div><h2>No files in this Workspace yet</h2><p>After adding files, find them by name or type. Small supported text files can also be scanned on demand; ZIP contents are not extracted.</p></div> : (
        <>
          <div className="library-search-block">
            <label htmlFor="resource-search">Find a saved file</label>
            <input id="resource-search" className="resource-search" type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search by file name, type, or text" />
            <label className="resource-content-search-toggle" htmlFor="resource-search-mode">Search in</label>
            <select id="resource-search-mode" className="resource-search-mode" value={searchMode} onChange={(event) => setSearchMode(event.currentTarget.value as typeof searchMode)}>
              <option value="METADATA">Names and file types</option>
              <option value="INDEXED_CONTENT">Indexed local text</option>
              <option value="ON_DEMAND_CONTENT">Scan small text files now</option>
            </select>
            <div className="resource-search-filters" aria-label="Resource filters">
              <label htmlFor="resource-kind-filter">File type</label>
              <select id="resource-kind-filter" className="resource-search-mode" value={searchKind} onChange={(event) => setSearchKind(event.currentTarget.value)}>
                <option value="">All types</option>
                <option value="FILE">Files</option>
                <option value="FOLDER">Folders</option>
                <option value="ARTIFACT">Artifacts</option>
                <option value="CONNECTOR_OBJECT">Connected items</option>
                <option value="WEB_RESOURCE">Web resources</option>
                <option value="OTHER">Other</option>
              </select>
              <label htmlFor="resource-freshness-filter">Freshness</label>
              <select id="resource-freshness-filter" className="resource-search-mode" value={searchFreshness} onChange={(event) => setSearchFreshness(event.currentTarget.value)}>
                <option value="">Any freshness</option>
                <option value="CURRENT">Current</option>
                <option value="STALE">Stale</option>
                <option value="UNKNOWN">Unknown</option>
                <option value="UNAVAILABLE">Unavailable</option>
              </select>
            </div>
            <p className="resource-search-hint">{searchMode === "INDEXED_CONTENT"
              ? "Searches the encrypted local index for current plain-text Resources up to 1 MiB. All words must match. ZIP, PDF, Office files, folders, OCR, and semantic search are not included."
              : searchMode === "ON_DEMAND_CONTENT"
                ? "Reads up to 20 current managed Resources per request, with a 1 MiB per-file and 8 MiB total limit. Extracted text is not saved. ZIP, PDF, Office files, and folders are not scanned."
                : "Search checks saved names and media types. Text contents are not read."}</p>
            {searchMode === "ON_DEMAND_CONTENT" && contentScanInfo && <p className="resource-search-hint" role="status">Checked {contentScanInfo.textResourcesChecked} text files ({contentScanInfo.candidatesScanned} Resources considered). {contentScanInfo.skippedUnsupportedType > 0 ? `${contentScanInfo.skippedUnsupportedType} unsupported type(s); ` : ""}{contentScanInfo.skippedOverFileLimit > 0 ? `${contentScanInfo.skippedOverFileLimit} over the 1 MiB limit; ` : ""}{contentScanInfo.skippedRevisionChanged > 0 ? `${contentScanInfo.skippedRevisionChanged} changed during search; ` : ""}{contentScanInfo.byteBudgetExhausted || contentScanInfo.candidateBudgetExhausted ? "Load more to continue." : "No persistent text index is used."}</p>}
          </div>
          {searching ? <>
            {searchError && <p className="inline-error" role="status">{searchError}</p>}
            {searchPageBusy && searchResults.length === 0 ? <p className="inline-status" role="status">{searchMode === "ON_DEMAND_CONTENT" ? "Scanning bounded local text…" : searchMode === "INDEXED_CONTENT" ? "Searching indexed local text…" : "Searching this Workspace…"}</p> : searchResults.length === 0 && !searchError ? <p className="empty-inline">{query.trim() ? `No saved files match “${query.trim()}” in the checked results.` : searchMode === "METADATA" ? "No saved files match the selected filters." : "Enter search terms to apply filters to content search."}</p> : <ul className="resource-list">{searchResults.map((result) => {
              const attached = selectedTaskInputs.find((input) => input.resourceId === result.resourceId);
              const exactRevisionAttached = attached?.revisionId === result.resourceRevisionId;
              return <li key={result.resourceId}><span className="resource-file-icon" aria-hidden="true">▤</span><span><strong>{result.displayName}</strong><small>{result.matchReasons.map((reason) => reason === "MEDIA_TYPE" ? "Type match" : reason === "CONTENT_ON_DEMAND" ? "Text match · scanned now" : reason === "CONTENT_INDEXED" ? "Text match · local index" : "Name match").join(" · ") || "Saved Resource"} · {result.freshness.toLocaleLowerCase()}</small></span><span className={result.snippet ? "resource-search-snippet" : "resource-digest"}>{result.snippet ?? "Metadata match"}</span><button className="quiet-button" type="button" onClick={() => void onPreview(result.resourceId, result.displayName, result.resourceRevisionId, result.sourceMatches.length > 0 ? { contentDigest: result.sourceContentDigest, matches: result.sourceMatches } : undefined)} aria-expanded={previewResourceId === result.resourceId}>Preview</button><button className="quiet-button" type="button" aria-pressed={exactRevisionAttached} onClick={() => onToggleTaskInput({ resourceId: result.resourceId, resourceRevisionId: result.resourceRevisionId, displayName: result.displayName })}>{exactRevisionAttached ? "Remove input" : attached ? "Use this revision" : "Add to Task"}</button></li>;
            })}</ul>}
            {searchCursor && <div className="load-more-row"><button className="quiet-button" type="button" disabled={searchBusy} onClick={() => void loadMoreSearchResults()}>{searchBusy ? "Searching…" : "Load more results"}</button></div>}
          </> : <>
            {resources.length === 0 ? null : <ul className="resource-list">{resources.map((resource) => {
              const previewable = supportsTextPreview(resource.mediaType, resource.sizeBytes);
              const indexAction = resourceIndexAction(resource);
              const isZip = indexAction.kind === "UNAVAILABLE";
              const attached = selectedTaskInputs.find((input) => input.resourceId === resource.resourceId);
              const exactRevisionAttached = attached?.revisionId === resource.resourceRevisionId;
              const canRetryIndex = indexRetryPin?.workspaceId === selectedWorkspaceId
                && indexRetryPin.resourceId === resource.resourceId
                && indexRetryPin.revisionId === resource.resourceRevisionId
                && indexRetryPin.contentDigest === resource.contentDigest;
              return <li key={resource.resourceId}><span className="resource-file-icon" aria-hidden="true">▤</span><span><strong>{resource.displayName}</strong><small>{formatBytes(resource.sizeBytes)} · {isZip ? "ZIP archive stored intact" : resource.mediaType}</small></span><span className="resource-digest">{resource.contentDigest.slice(0, 19)}…</span>{previewable ? <button className="quiet-button" type="button" onClick={() => void onPreview(resource.resourceId, resource.displayName, resource.resourceRevisionId)} aria-expanded={previewResourceId === resource.resourceId}>Preview text</button> : <span className="resource-preview-unavailable" title={resource.sizeBytes > 1024 * 1024 ? "Text preview is limited to 1 MiB." : isZip ? "ZIP files are stored intact and are not extracted." : "Preview is available for text files only."}>{resource.sizeBytes > 1024 * 1024 ? "Over preview limit" : isZip ? "Not extracted" : "No text preview"}</span>}{resource.sizeBytes <= 10 * 1024 * 1024 ? <button className="quiet-button" type="button" disabled={!operatorReady || saveAsBusyResourceId !== null || busy} onClick={() => void saveResourceAs(resource)}>{saveAsBusyResourceId === resource.resourceId ? "Saving…" : "Save original…"}</button> : <span className="resource-preview-unavailable" title="Desktop Save As is limited to 10 MiB.">Over Save As limit</span>}{indexAction.kind === "REBUILD" ? <button className="quiet-button" type="button" disabled={!operatorReady || rebuildBusyResourceId !== null || busy} onClick={() => void rebuildResourceTextIndex(resource)}>{rebuildBusyResourceId === resource.resourceId ? "Rebuilding index…" : canRetryIndex ? "Retry index rebuild" : indexAction.label}</button> : <span className="resource-preview-unavailable" title={indexAction.reason}>{indexAction.label}</span>}<button className="quiet-button" type="button" onClick={() => setRevisionResource(resource)} aria-expanded={revisionResource?.resourceId === resource.resourceId}>History &amp; edit</button><button className="quiet-button" type="button" aria-pressed={exactRevisionAttached} onClick={() => onToggleTaskInput({ resourceId: resource.resourceId, resourceRevisionId: resource.resourceRevisionId, displayName: resource.displayName })}>{exactRevisionAttached ? "Remove input" : attached ? "Use this revision" : "Add to Task"}</button></li>;
            })}</ul>}
            {nextCursor && <div className="load-more-row"><button className="quiet-button" type="button" disabled={pageBusy} onClick={() => void onLoadMore()}>{pageBusy ? "Loading…" : "Load older files"}</button></div>}
          </>}
          {previewResourceId && <section className="resource-preview" aria-label="Resource text preview">
            <div className="section-heading"><h2>{previewResourceName ?? "Preview"}</h2><button className="quiet-button" type="button" onClick={onClosePreview}>Close preview</button></div>
            {previewError ? <p className="inline-error" role="status">{previewError}</p> : preview === null ? <p className="inline-status" role="status">Loading preview…</p> : <>
              {previewHighlights?.kind === "plain" && <p className="inline-status" role="status">Search matches could not be verified, so this exact revision is shown without highlights.</p>}
              <pre className="resource-preview-content">{previewHighlights?.kind === "highlighted"
                ? previewHighlights.segments.map((segment, index) => segment.terms.length > 0
                  ? <mark key={`${segment.terms.join("-")}-${index}`} title={`Indexed term: ${segment.terms.join(", ")}`}>{segment.text}</mark>
                  : <span key={`plain-${index}`}>{segment.text}</span>)
                : preview}</pre>
            </>}
          </section>}
          {revisionResource && <ResourceRevisionEditor resource={revisionResource} onClose={() => setRevisionResource(null)} onCommitted={(updated) => { onRevisionCommitted(updated); setRevisionResource(updated); }} />}
        </>
      )}
      <section className="workspace-list-card" aria-labelledby="finished-artifacts-heading">
        <div className="section-heading"><div><h2 id="finished-artifacts-heading">Finished Artifacts</h2><p>Committed outputs from work in this Workspace, with version history and provenance.</p></div></div>
        {!selectedWorkspaceId
          ? <p className="inline-status" role="status">Select a Workspace to view its Artifacts.</p>
          : !operatorReady
            ? <p className="inline-status" role="status">Start the local Runtime to load committed Artifacts.</p>
            : <ArtifactLibrary api={artifactApi} workspaceId={selectedWorkspaceId} onOpenSource={onOpenArtifactSource} />}
      </section>
    </div>
  );
}

function formatBytes(size: number): string {
  if (size === 0) return "0 B";
  if (size < 1024) return `${size} B`;
  return size < 1024 * 1024 ? `${Math.round(size / 1024)} KB` : `${(size / (1024 * 1024)).toFixed(1)} MB`;
}

function supportsTextPreview(mediaType: string, sizeBytes: number): boolean {
  const normalized = mediaType.toLowerCase().split(";", 1)[0].trim();
  const textLike = normalized.startsWith("text/") || [
    "application/json",
    "application/xml",
    "application/yaml",
    "application/x-yaml",
    "application/javascript",
  ].includes(normalized);
  return textLike && sizeBytes <= 1024 * 1024;
}

export default App;
