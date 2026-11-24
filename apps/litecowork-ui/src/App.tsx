import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { WorkbenchPanel } from "./workbench/WorkbenchPanel";
import { invoke } from "@tauri-apps/api/core";
import type { components } from "./api/operator-api";
import { TaskArtifactOutputs } from "./artifacts/TaskArtifactOutputs";
import { desktopArtifactApi } from "./artifacts/desktop-artifact-api";
import type { PinnedResourceRef } from "./artifacts/artifact-api";
import { CoworkerSettings, type CoworkerLeadBindingOption } from "./coworkers/CoworkerSettings";
import { HomeCoworkerOnboarding } from "./coworkers/HomeCoworkerOnboarding";
import { desktopCoworkerApi } from "./coworkers/desktop-coworker-api";
import { WorkspaceFirstRun, type WorkspaceListState } from "./workspaces/WorkspaceFirstRun";
import { desktopDelegationProfileCatalogApi } from "./coworkers/desktop-delegation-profile-api";
import type { DelegationProfileCatalogItem } from "./coworkers/delegation-profile-api";
import type { Coworker, WorkspacePrimaryReceipt } from "./coworkers/coworker-api";
import { GoalsPage } from "./goals/GoalsPage";
import { desktopGoalApi } from "./goals/desktop-goal-api";
import { NotificationsPage } from "./notifications/NotificationsPage";
import { maybeDesktopNotificationApi } from "./notifications/desktop-notification-api";
import { SuggestionsPage } from "./suggestions/SuggestionsPage";
import { TaskPresentationPanel } from "./presentation/TaskPresentationPanel";
import { TaskSpecRevisionHistory } from "./tasks/TaskSpecRevisionHistory";
import { TaskDelegationPolicySnapshot } from "./tasks/TaskDelegationPolicySnapshot";
import { AutomationsPage } from "./automations/AutomationsPage";
import { desktopAutomationApi } from "./automations/desktop-automation-api";
import type { AutomationStarterId, AutomationTemplateFlow } from "./automations/automation-starters";
import { RoutinesPage } from "./routines/RoutinesPage";
import { desktopRoutineApi } from "./routines/desktop-routine-api";
import type { AgentProviderKey } from "./agents/agent-catalog-api";
import { desktopAgentCatalogApi } from "./agents/desktop-agent-catalog-api";
import { desktopAgentControlDescriptorApi } from "./agents/desktop-agent-control-api";
import {
  AgentBindingRefreshFence,
  agentActionFeedbackForWorkspace,
  agentBindingMutationFailureMessage,
  agentBindingMutationSuccessMessage,
  clearAgentBindingMutationRequest,
  clearAgentActionBusyIfMatches,
  getOrCreateAgentBindingMutationRequest,
  mergeAgentBindingProjection,
  mergeAgentBindingProjections,
  reconcileAgentBindingMutationRequests,
  updateAgentActionFeedback as applyAgentActionFeedback,
  type AgentActionFeedbackByWorkspace,
  type AgentActionFeedback,
  type PendingAgentBindingMutation,
} from "./agents/agent-binding-mutation.ts";
import { isSensitiveOrGenerated, resourceDisplayName, resourceFolderRelativePath, validateResourceFileSelection } from "./resources/resource-intake";
import {
  matchesResourceUploadIdentity,
  prepareResourceUploadCommitRequestId,
  receivedUploadRangesMatchFile,
  resumableResourceUploadSession,
  type ResourceUploadContextDocument,
  type ResourceUploadIdentity,
} from "./resources/resource-upload-resume.ts";
import { LibraryPage } from "./library/LibraryPage";
import { formatBytes } from "./library/library-format";
import { OperatorUnavailablePanel } from "./shell/OperatorUnavailablePanel";
import { RecentChatsSidebar, type ConversationListDisplayStatus } from "./shell/RecentChatsSidebar";
import { WorkspaceProjectsPage } from "./projects/WorkspaceProjectsPage";
import { WorkspaceDashboard, type WidgetSource } from "./dashboard/WorkspaceDashboard";
import { summarizeByStatus, summarizeTotal } from "./dashboard/dashboard-state.ts";
import { LibraryNavigationDialog, libraryNavigationMainIsInert, useLibraryNavigation } from "./resources/library-navigation";
import { TaskResourceTextPreview } from "./resources/TaskResourceTextPreview";
import { validateResourceSourceHighlights, type ResourceSourceMatch, type SourceHighlightResult } from "./resources/source-span-highlights";
import { TaskAttentionPage } from "./needs-you/TaskAttentionPage";
import { maybeDesktopSuggestionApi } from "./suggestions/desktop-suggestion-api";
import {
  isTaskPlanningReadinessFor,
  planningBlockerLabel,
  planningReadinessNoBlockersMessage,
  type TaskPlanningReadinessView,
} from "./tasks/planning-readiness";
import { canOfferLocalRuntimeStart } from "./runtime/runtime-start-readiness";
import { isRuntimeExecutionReady } from "./runtime/runtime-health";
import type { RuntimeStopPreviewView, StopPreviewState } from "./runtime/runtime-stop-state.ts";
import { RuntimeInventoryStreamController, type RuntimeInventoryStreamStatus } from "./runtime/runtime-inventory-stream.ts";
import { desktopRuntimeInventoryStreamDriver } from "./runtime/desktop-runtime-inventory-stream.ts";
import { ConversationsPage } from "./conversations/ConversationsPage";
import { SettingsView } from "./settings/SettingsView";
import type { ConversationSummary } from "./conversations/conversation-view";
import { DiscoverPage } from "./discover/DiscoverPage";
import { desktopDiscoverApi } from "./discover/desktop-discover-api";
import {
  WorkspaceInstructionDraftCache,
  canApplyWorkspaceInstructionResponse,
  clearWorkspaceInstructionSave,
  getOrCreateWorkspaceInstructionSave,
  instructionSaveFailureKind,
  instructionSaveRefreshKind,
  workspaceInstructionByteLength,
  type PendingWorkspaceInstructionSave,
} from "./workspaces/workspace-instructions.ts";
import {
  clearWorkspacePolicyUpdateRequest,
  isWorkspacePolicyUpdateResponse,
  mergeWorkspaceListProjection,
  mergeWorkspacePolicyProjection,
  reconcileWorkspacePolicyUpdateRequests,
  workspacePolicyRefreshOutcome,
  workspacePolicyUpdateRequest,
  type PendingWorkspacePolicyUpdate,
} from "./workspaces/workspace-policy.ts";
import {
  canAutoSelectCreatedWorkspace,
  canRestoreInitialWorkspaceSelection,
  nextWorkspaceCreateRequest,
  WorkspaceListRequestFence,
  WorkspaceCreateGuard,
  type PendingWorkspaceCreate,
} from "./workspaces/workspace-onboarding.ts";
import {
  applyLocalPreferences,
  browserPreferenceStorage,
  loadLocalPreferences,
  saveLocalPreferences,
  type LocalPreferences,
} from "./settings/local-preferences.ts";
import { matchShortcut, shortcutKeys, type ShortcutId } from "./shell/shortcut-registry.ts";
import {
  classifyRenameFailure,
  nextRenameRequest,
  renameFailureMessage,
  validateWorkspaceName,
  type PendingRename,
} from "./settings/workspace-rename.ts";

type Page = "Home" | "Conversations" | "Projects" | "Work" | "Library" | "Discover" | "Needs You" | "Ideas" | "Coworkers" | "Goals" | "Notifications" | "Routines" | "Automations" | "Dashboard" | "Settings";
type ConversationNavigationIntent =
  | { kind: "NEW_CHAT" }
  | { kind: "OPEN_CONVERSATION"; conversationId: string }
  | { kind: "OPEN_WORKSPACE"; workspaceId: string };
export type RuntimeStatus = {
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
export type RuntimeLifecycleView = {
  runtimeId: string;
  startupPolicy: "MANUAL" | "LOGIN_BACKGROUND" | "ALWAYS_ON_SERVICE";
  incarnation: {
    runtimeIncarnationId: string;
    runtimeId: string;
    processStartedAt: string;
    litecoworkVersion: string;
    recoveredFromUncleanShutdown: boolean;
    recoveryState: "STARTING" | "RECOVERING" | "READY" | "DEGRADED" | "DRAINING" | "STOPPING" | "STOPPED";
    readyAt: string | null;
    stoppedAt: string | null;
    version: number;
  };
  availability: "PAIRING" | "STARTING" | "RECOVERING" | "ONLINE" | "DEGRADED" | "DRAINING" | "OFFLINE" | "REVOKED";
  blockers: Array<{
    blockerId: string;
    code: "DEPENDENCY_UNAVAILABLE" | "RUNTIME_UNAVAILABLE";
    safeMessage: string;
    resolutionHint: string;
    createdAt: string;
  }>;
};
export type WorkspaceView = {
  workspaceId: string;
  name: string;
  replicationPolicy: string;
  defaultAgentBindingId: string | null;
  primaryCoworkerId?: string | null;
  status: string;
  version: number;
};
export type AgentInstallationView = {
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
export type AgentProfileView = {
  agentProfileId: string;
  providerKey: string;
  displayName: string;
  endpoints: AgentEndpointView[];
  discoveredAt: string;
  observations: AgentProfileObservationView[];
};
type AgentProfilePageView = { items: AgentProfileView[]; nextCursor: string | null };
export type AgentBindingView = {
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
export type RuntimeWorkspaceBindingView = {
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
export type ResourceView = {
  resourceId: string;
  workspaceId: string;
  resourceRevisionId: string;
  displayName: string;
  mediaType: string;
  contentDigest: string;
  sizeBytes: number;
};
export type ResourceTextIndexRebuildView = {
  requestId: string;
  correlationId: string;
  workspaceId: string;
  resourceId: string;
  resourceRevisionId: string;
  contentDigest: string;
  outcome: "INDEXED" | "NOT_INDEXABLE";
  reason: "UNSUPPORTED_TYPE" | "OVER_SIZE_LIMIT" | "INVALID_UTF8" | "CONTROL_CHARACTERS" | "TERM_LIMIT_EXCEEDED" | null;
};
export type WorkspaceRootView = {
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
export type WorkspaceRootPageView = { items: WorkspaceRootView[]; nextCursor: string | null };
export type WorkspaceRootScanFeedback = { kind: "loading" | "result" | "error" | "stale" | "unsupported" | "rejected"; message: string; rootVersion: number };
type PinnedResourceRefView = { workspaceId: string; resourceId: string; revisionId: string };
export type TaskInputSelection = PinnedResourceRefView & { displayName: string };
type ResourcePageView = { items: ResourceView[]; nextCursor: string | null };
export type ResourceSearchResultView = { resourceId: string; resourceRevisionId: string; sourceContentDigest: string; sourceMatches: ResourceSourceMatch[]; displayName: string; freshness: string; matchReasons: string[]; snippet: string | null };
type ResourceSearchPreviewView = { text: string; resourceRevisionId: string; contentDigest: string };
export type PreviewSearchPin = { contentDigest: string; matches: ResourceSourceMatch[] };
export type ResourceContentScanView = { candidatesScanned: number; textResourcesChecked: number; skippedUnsupportedType: number; skippedOverFileLimit: number; skippedRevisionChanged: number; byteBudgetExhausted: boolean; candidateBudgetExhausted: boolean; maxCandidates: number; maxFileBytes: number; maxTotalBytes: number };
export type ResourceSearchPageView = { items: ResourceSearchResultView[]; nextCursor: string | null; mode: string; contentScan: ResourceContentScanView | null };
type TaskStatus = "READY" | "RUNNING" | "WAITING_USER" | "BLOCKED" | "VERIFYING" | "NEEDS_USER" | "INCOMPLETE" | "PAUSE_REQUESTED" | "PAUSED" | "COMPLETED" | "FAILED" | "CANCEL_REQUESTED" | "CANCELLED";
type TaskSummaryView = { taskId: string; status: TaskStatus; objective: string; createdAt: string; updatedAt: string };
type TaskPageView = { items: TaskSummaryView[]; nextCursor: string | null };
type PlannedStepView = { stepId: string; logicalKey: string; title: string; objective: string; status: string };
type DelegationPolicySnapshot = components["schemas"]["ResolvedDelegationPolicySnapshot"];
type TaskDetailView = { taskId: string; workspaceId: string; conversationId: string | null; sourceMessageRefs: string[]; originCoworkerId: string | null; originCoworkerRevision: number | null; currentSpecRevision: number; currentPlanRevision: number | null; status: TaskStatus; taskVersion: number; objective: string; inputRefs: PinnedResourceRefView[]; delegationPolicySnapshot: DelegationPolicySnapshot | null; planSpecRevision: number | null; planIsStale: boolean; plannedSteps: PlannedStepView[]; createdAt: string; updatedAt: string };
type TaskSpecRevisionReceiptView = { taskId: string; revision: number; objective: string; delegationPolicySnapshot: DelegationPolicySnapshot | null };
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
export type ContextDocumentCreateMetadata = ResourceUploadContextDocument;
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
export type ResourceUploadResumeForget = {
  workspaceId: string;
  resumeKey: string;
  uploadId: string;
  displayName: string;
  terminalState: "FAILED" | "EXPIRED";
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
export type WorkspaceInstructionView = {
  workspaceId: string;
  revision: number;
  parentRevisions: number[];
  contentRef: { workspace_id?: string; resource_id?: string; revision_id?: string | null };
  contentDigest: string;
  authoredBy: Record<string, unknown>;
  createdAt: string;
};

export function SidebarNavIcon({ page }: { page: Page }) {
  switch (page) {
    case "Conversations":
      return (
        <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
        </svg>
      );
    case "Work":
      return (
        <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <polyline points="9 11 12 14 22 4" />
          <path d="M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11" />
        </svg>
      );
    case "Needs You":
      return (
        <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="12" cy="12" r="10" />
          <line x1="12" y1="8" x2="12" y2="12" />
          <line x1="12" y1="16" x2="12.01" y2="16" />
        </svg>
      );
    case "Home":
      return (
        <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <path d="m3 9 9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
          <polyline points="9 22 9 12 15 12 15 22" />
        </svg>
      );
    case "Coworkers":
      return (
        <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2" />
          <circle cx="9" cy="7" r="4" />
          <path d="M23 21v-2a4 4 0 0 0-3-3.87" />
          <path d="M16 3.13a4 4 0 0 1 0 7.75" />
        </svg>
      );
    case "Library":
      return (
        <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20" />
          <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z" />
        </svg>
      );
    case "Automations":
      return (
        <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="12" cy="12" r="10" />
          <polyline points="12 6 12 12 16 14" />
        </svg>
      );
    case "Routines":
      return (
        <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <polyline points="23 4 23 10 17 10" />
          <polyline points="1 20 1 14 7 14" />
          <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15" />
        </svg>
      );
    case "Discover":
      return (
        <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
        </svg>
      );
    case "Goals":
      return (
        <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="12" cy="12" r="10" />
          <circle cx="12" cy="12" r="6" />
          <circle cx="12" cy="12" r="2" />
        </svg>
      );
    case "Notifications":
      return (
        <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9" />
          <path d="M13.73 21a2 2 0 0 1-3.46 0" />
        </svg>
      );
    case "Ideas":
      return (
        <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <path d="M9 18h6" />
          <path d="M10 22h4" />
          <path d="M15.09 14c.18-.98.65-1.74 1.41-2.5A4.65 4.65 0 0 0 18 8 6 6 0 0 0 6 8c0 1 .23 2.23 1.5 3.5A4.61 4.61 0 0 1 8.91 14" />
        </svg>
      );
    case "Settings":
      return (
        <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="12" cy="12" r="3" />
          <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z" />
        </svg>
      );
    default:
      return null;
  }
}

export const navigation: { label: Page; displayLabel?: string; group?: string }[] = [
  { label: "Conversations", displayLabel: "Recent chats", group: "WORKSPACE" },
  { label: "Projects", displayLabel: "Projects", group: "WORKSPACE" },
  { label: "Needs You", displayLabel: "Needs You" },
  { label: "Home", displayLabel: "New task" },
  { label: "Work", displayLabel: "Tasks & Work" },
  { label: "Library", displayLabel: "Library & Files", group: "RESOURCES" },
  { label: "Coworkers", displayLabel: "Coworkers" },
  { label: "Discover", displayLabel: "Discover Skills" },
  { label: "Automations", displayLabel: "Automations", group: "AUTOMATION" },
  { label: "Routines", displayLabel: "Routines" },
  { label: "Goals", displayLabel: "Goals", group: "MONITORING" },
  { label: "Notifications", displayLabel: "Notifications" },
  { label: "Ideas", displayLabel: "Ideas" },
  { label: "Dashboard", displayLabel: "Dashboard" },
  { label: "Settings", displayLabel: "Settings", group: "SYSTEM" },
];

/** Less-frequent destinations remain reachable without taking permanent sidebar space. */
const SIDEBAR_MORE_PAGES: Page[] = ["Home", "Work", "Library", "Coworkers", "Discover", "Automations", "Routines", "Dashboard", "Goals", "Notifications", "Ideas"];

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

function writeUploadResume(key: string, resume: LocalUploadResume | null): boolean {
  try {
    const records = readUploadResumes();
    if (resume) records[key] = resume;
    else delete records[key];
    localStorage.setItem(UPLOAD_RESUME_KEY, JSON.stringify(records));
    return true;
  } catch {
    // Do not start the next transfer/commit request without its retry identity saved.
    // Clearing after a confirmed commit is best-effort because replay remains safe.
    if (resume) {
      throw new Error("Could not persist resource upload recovery metadata; the next upload request was not sent.");
    }
    return false;
  }
}

function fileResumeKey(workspaceId: string, file: File): string {
  const relative = (file as File & { webkitRelativePath?: string }).webkitRelativePath || file.name;
  return JSON.stringify([workspaceId, relative, file.size, file.lastModified]);
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
  const libraryNavigation = useLibraryNavigation<Page>("Conversations");
  const {
    page,
    pendingNavigation: pendingLibraryNavigation,
    requestNavigation: requestPageNavigation,
    stayInLibrary,
    saveLibraryDraftBeforeNavigation,
    discardLibraryDraftAndNavigate,
    draftSaveFailed: onLibraryDraftSaveFailed,
    draftCommitted: onLibraryDraftCommitted,
  } = libraryNavigation;
  const [navigationOpen, setNavigationOpen] = useState(false);
  const [sidebarMoreOpen, setSidebarMoreOpen] = useState(false);
  const navigationTriggerRef = useRef<HTMLButtonElement>(null);
  const navigationDrawerRef = useRef<HTMLElement>(null);
  const workbenchTriggerRef = useRef<HTMLButtonElement>(null);
  const workbenchPanelRef = useRef<HTMLElement>(null);
  const [workbenchOpen, setWorkbenchOpen] = useState(false);
  const [workbenchNarrow, setWorkbenchNarrow] = useState(() => typeof window !== "undefined" && window.matchMedia("(max-width: 1200px)").matches);
  const [workbenchLayoutByWorkspace, setWorkbenchLayoutByWorkspace] = useState<Record<string, boolean>>(() => {
    try {
      const saved: unknown = JSON.parse(window.localStorage.getItem("litecowork.workbench.layout.v1") ?? "{}");
      if (saved && typeof saved === "object" && !Array.isArray(saved)) {
        return Object.fromEntries(Object.entries(saved).filter(([, visible]) => typeof visible === "boolean")) as Record<string, boolean>;
      }
    } catch {
      // The default layout remains available when WebView storage is disabled.
    }
    return {};
  });
  const [workbenchWidth, setWorkbenchWidth] = useState<number>(() => {
    try {
      const saved = window.localStorage.getItem("litecowork.workbenchWidth");
      return saved ? Math.max(340, Math.min(parseInt(saved, 10), 1200)) : 480;
    } catch {
      return 480;
    }
  });
  const [workbenchFullView, setWorkbenchFullView] = useState(false);
  const [localPreferences, setLocalPreferences] = useState<LocalPreferences>(() => loadLocalPreferences(browserPreferenceStorage()));
  useEffect(() => {
    applyLocalPreferences(document.documentElement, localPreferences);
  }, [localPreferences]);
  const changeLocalPreferences = useCallback((next: LocalPreferences) => {
    setLocalPreferences(next);
    saveLocalPreferences(browserPreferenceStorage(), next);
  }, []);
  const [isDraggingWorkbench, setIsDraggingWorkbench] = useState(false);

  useEffect(() => {
    if (!isDraggingWorkbench) return;
    const handleMouseMove = (e: MouseEvent) => {
      const newWidth = Math.max(320, Math.min(window.innerWidth - e.clientX, Math.floor(window.innerWidth * 0.75)));
      setWorkbenchWidth(newWidth);
    };
    const handleMouseUp = () => {
      setIsDraggingWorkbench(false);
      try {
        window.localStorage.setItem("litecowork.workbenchWidth", String(workbenchWidth));
      } catch {}
    };
    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);
    return () => {
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
    };
  }, [isDraggingWorkbench, workbenchWidth]);
  useEffect(() => {
    try {
      window.localStorage.setItem("litecowork.workbench.layout.v1", JSON.stringify(workbenchLayoutByWorkspace));
    } catch {
      // Panel visibility is a convenience preference, not a prerequisite for the app.
    }
  }, [workbenchLayoutByWorkspace]);
  useEffect(() => {
    const media = window.matchMedia("(max-width: 1200px)");
    const updateLayout = () => setWorkbenchNarrow(media.matches);
    updateLayout();
    media.addEventListener("change", updateLayout);
    return () => media.removeEventListener("change", updateLayout);
  }, []);
  const closeNavigation = useCallback((restoreFocus = true) => {
    setNavigationOpen(false);
    if (restoreFocus) window.requestAnimationFrame(() => {
      if (window.innerWidth <= 900) navigationTriggerRef.current?.focus();
    });
  }, []);
  const closeWorkbench = useCallback((restoreFocus = true) => {
    setWorkbenchOpen(false);
    if (restoreFocus) window.requestAnimationFrame(() => workbenchTriggerRef.current?.focus());
  }, []);
  const navigationGuardFocusTarget = () =>
    navigationOpen && window.innerWidth <= 900 ? navigationTriggerRef.current : null;
  useEffect(() => {
    if (!navigationOpen) return;
    const focusableElements = () => Array.from(
      navigationDrawerRef.current?.querySelectorAll<HTMLElement>(
        'a[href], button:not(:disabled), select:not(:disabled), [tabindex]:not([tabindex="-1"])',
      ) ?? [],
    );
    let focusFrame = 0;
    let focusWaitFrames = 0;
    const focusDrawerCloseWhenVisible = () => {
      const close = navigationDrawerRef.current?.querySelector<HTMLElement>(".sidebar-close");
      if (!close) return;
      if (getComputedStyle(close).visibility !== "visible" || close.getClientRects().length === 0) {
        if (focusWaitFrames++ < 30) focusFrame = window.requestAnimationFrame(focusDrawerCloseWhenVisible);
        return;
      }
      close.focus();
    };
    focusFrame = window.requestAnimationFrame(focusDrawerCloseWhenVisible);
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        closeNavigation();
        return;
      }
      if (event.key !== "Tab") return;
      const elements = focusableElements();
      const first = elements[0];
      const last = elements.at(-1);
      if (!first || !last) return;
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    const closeOnDesktopResize = () => {
      if (window.innerWidth > 900) {
        setNavigationOpen(false);
        window.requestAnimationFrame(() => {
          const activeNavigationItem = navigationDrawerRef.current?.querySelector<HTMLElement>('[aria-current="page"]');
          (activeNavigationItem ?? navigationDrawerRef.current?.querySelector<HTMLElement>(".brand"))?.focus();
        });
      }
    };
    document.addEventListener("keydown", handleKeyDown);
    window.addEventListener("resize", closeOnDesktopResize);
    return () => {
      window.cancelAnimationFrame(focusFrame);
      document.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("resize", closeOnDesktopResize);
    };
  }, [navigationOpen, closeNavigation]);
  useEffect(() => {
    if (SIDEBAR_MORE_PAGES.includes(page)) setSidebarMoreOpen(true);
  }, [page]);
  useEffect(() => {
    if (!workbenchNarrow || !workbenchOpen) return;
    const focusableElements = () => Array.from(
      workbenchPanelRef.current?.querySelectorAll<HTMLElement>(
        'button:not(:disabled), a[href], select:not(:disabled), [tabindex]:not([tabindex="-1"])',
      ) ?? [],
    );
    workbenchPanelRef.current?.querySelector<HTMLElement>(".workbench-close")?.focus();
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        closeWorkbench();
        return;
      }
      if (event.key !== "Tab") return;
      const elements = focusableElements();
      const first = elements[0];
      const last = elements.at(-1);
      if (!first || !last) return;
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [workbenchNarrow, workbenchOpen, closeWorkbench]);
  const [draft, setDraft] = useState("");
  const [taskSaving, setTaskSaving] = useState(false);
  const taskSavingRef = useRef(false);
  const [taskSaveError, setTaskSaveError] = useState<string | null>(null);
  const [pendingTaskSave, setPendingTaskSave] = useState<PendingTaskSave | null>(null);
  const [homeCoworkerSelections, setHomeCoworkerSelections] = useState<Record<string, HomeCoworkerSelection>>({});
  const [taskInputSelections, setTaskInputSelections] = useState<Record<string, TaskInputSelection[]>>({});
  const [resourceNameCache, setResourceNameCache] = useState<Record<string, string>>({});
  const [runtime, setRuntime] = useState<RuntimeStatus | null>(null);
  const [runtimeLifecycle, setRuntimeLifecycle] = useState<{ identity: string; view: RuntimeLifecycleView } | null>(null);
  const [runtimeLifecycleLoading, setRuntimeLifecycleLoading] = useState(false);
  const [runtimeLifecycleError, setRuntimeLifecycleError] = useState<string | null>(null);
  const [stopPreview, setStopPreview] = useState<StopPreviewState>({ status: "IDLE" });
  const runtimeRefreshGeneration = useRef(0);
  const [runtimeBusy, setRuntimeBusy] = useState(false);
  const [runtimeError, setRuntimeError] = useState<string | null>(null);
  const [workspaces, setWorkspaces] = useState<WorkspaceView[]>([]);
  const [selectedConversationId, setSelectedConversationId] = useState<string | null>(null);
  const [newChatRequestId, setNewChatRequestId] = useState(0);
  const [conversationsByWorkspace, setConversationsByWorkspace] = useState<Record<string, ConversationSummary[]>>({});
  const [conversationListStatusByWorkspace, setConversationListStatusByWorkspace] = useState<Record<string, ConversationListDisplayStatus>>({});
  const [conversationPaginationByWorkspace, setConversationPaginationByWorkspace] = useState<Record<string, { hasMore: boolean; loading: boolean }>>({});
  const loadMoreConversationChatsRef = useRef<(() => void) | null>(null);
  const refreshConversationListRef = useRef<(() => void) | null>(null);
  const registerLoadMoreConversationChats = useCallback((handler: (() => void) | null) => {
    loadMoreConversationChatsRef.current = handler;
  }, []);
  const registerRefreshConversationList = useCallback((handler: (() => void) | null) => {
    refreshConversationListRef.current = handler;
  }, []);
  const updateConversationListStatus = useCallback((workspaceId: string, status: ConversationListDisplayStatus) => {
    if (!workspaceId) return;
    setConversationListStatusByWorkspace(current => current[workspaceId] === status
      ? current
      : { ...current, [workspaceId]: status });
  }, []);
  const updateConversationListPagination = useCallback((workspaceId: string, hasMore: boolean, loading: boolean) => {
    setConversationPaginationByWorkspace(current => {
      const previous = current[workspaceId];
      if (previous?.hasMore === hasMore && previous.loading === loading) return current;
      return { ...current, [workspaceId]: { hasMore, loading } };
    });
  }, []);
  const workspacesRef = useRef<WorkspaceView[]>([]);
  const updateWorkspaceProjection = (update: WorkspaceView[] | ((current: WorkspaceView[]) => WorkspaceView[])) => {
    const next = typeof update === "function" ? update(workspacesRef.current) : update;
    workspacesRef.current = next;
    setWorkspaces(next);
    return next;
  };
  const [workspaceListState, setWorkspaceListState] = useState<WorkspaceListState>("LOADING");
  const workspaceListGeneration = useRef(new WorkspaceListRequestFence());
  const [workspaceError, setWorkspaceError] = useState<string | null>(null);
  const pendingWorkspacePolicyRequests = useRef(new Map<string, PendingWorkspacePolicyUpdate>());
  const [workspacePolicyRequests, setWorkspacePolicyRequests] = useState<PendingWorkspacePolicyUpdate[]>([]);
  const [agentInstallations, setAgentInstallations] = useState<AgentInstallationView[]>([]);
  const [agentInstallationError, setAgentInstallationError] = useState<string | null>(null);
  const [agentProfiles, setAgentProfiles] = useState<AgentProfileView[]>([]);
  const [agentProfileError, setAgentProfileError] = useState<string | null>(null);
  const [runtimeInventoryStreamStatus, setRuntimeInventoryStreamStatus] = useState<RuntimeInventoryStreamStatus | "UNAVAILABLE">("UNAVAILABLE");
  const agentProfilesGeneration = useRef(new AgentBindingRefreshFence());
  const agentBindingsGeneration = useRef(new AgentBindingRefreshFence());
  const pendingAgentBindingMutations = useRef(new Map<string, PendingAgentBindingMutation>());
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
  const [agentActionFeedback, setAgentActionFeedback] = useState<AgentActionFeedbackByWorkspace>({});
  const updateAgentActionFeedback = (workspaceId: string, update: Partial<AgentActionFeedback>) => {
    setAgentActionFeedback(current => applyAgentActionFeedback(current, workspaceId, update));
  };
  const [workspaceName, setWorkspaceName] = useState("");
  const [workspaceCreateBusy, setWorkspaceCreateBusy] = useState(false);
  const [workspacePolicyBusy, setWorkspacePolicyBusy] = useState(false);
  const workspacePolicyBusyRef = useRef(false);
  const [workspaceArchiveBusy, setWorkspaceArchiveBusy] = useState(false);
  const [workspaceRenameBusy, setWorkspaceRenameBusy] = useState(false);
  const [workspaceRenameError, setWorkspaceRenameError] = useState<string | null>(null);
  const pendingWorkspaceRename = useRef<PendingRename | null>(null);
  const pendingWorkspaceArchive = useRef<{ workspaceId: string; version: number; requestId: string } | null>(null);
  const [workspaceDefaultAgentBusy, setWorkspaceDefaultAgentBusy] = useState(false);
  const [workspaceInstructions, setWorkspaceInstructions] = useState<WorkspaceInstructionView[]>([]);
  const [instructionText, setInstructionText] = useState("");
  const instructionDrafts = useRef(new WorkspaceInstructionDraftCache());
  const [instructionBusy, setInstructionBusy] = useState(false);
  const [instructionMessage, setInstructionMessage] = useState<string | null>(null);
  const [latestInstructionsAfterConflict, setLatestInstructionsAfterConflict] = useState<string | null>(null);
  // Keep ambiguous save identities per Workspace. A failed A request must retain
  // its original Resource and revision request IDs while the user saves B.
  const pendingInstructions = useRef(new Map<string, PendingWorkspaceInstructionSave>());
  const instructionGeneration = useRef(0);
  const [pendingWorkspaceRequest, setPendingWorkspaceRequest] = useState<PendingWorkspaceCreate | null>(null);
  const pendingWorkspaceRequestRef = useRef<PendingWorkspaceCreate | null>(null);
  const workspaceCreateGuard = useRef(new WorkspaceCreateGuard());
  const [resources, setResources] = useState<ResourceView[]>([]);
  const [resourceCatalogRefreshNonce, setResourceCatalogRefreshNonce] = useState(0);
  const [resourceNextCursor, setResourceNextCursor] = useState<string | null>(null);
  const [resourcePageBusy, setResourcePageBusy] = useState(false);
  const [resourceBusy, setResourceBusy] = useState(false);
  const resourcePauseRequested = useRef(false);
  const [resourcePausePending, setResourcePausePending] = useState(false);
  const [resourceMessage, setResourceMessage] = useState<string | null>(null);
  const [resourceListFailed, setResourceListFailed] = useState(false);
  const [resourceUploadResumeForget, setResourceUploadResumeForget] = useState<ResourceUploadResumeForget | null>(null);
  const [previewResourceId, setPreviewResourceId] = useState<string | null>(null);
  const [previewResourceRevisionId, setPreviewResourceRevisionId] = useState<string | null>(null);
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
  const [conversationDrafts, setConversationDrafts] = useState<Record<string, string>>({});
  const [pendingConversationNavigationIntent, setPendingConversationNavigationIntent] = useState<ConversationNavigationIntent | null>(null);
  const [automationTemplateFlow, setAutomationTemplateFlow] = useState<AutomationTemplateFlow | null>(null);
  const artifactApi = useMemo(() => desktopArtifactApi(selectedWorkspaceId), [selectedWorkspaceId]);
  const discoverApi = useMemo(() => selectedWorkspaceId ? desktopDiscoverApi(selectedWorkspaceId) : null, [selectedWorkspaceId]);
  const automationApi = useMemo(() => desktopAutomationApi(selectedWorkspaceId), [selectedWorkspaceId]);
  const routineApi = useMemo(() => desktopRoutineApi(selectedWorkspaceId), [selectedWorkspaceId]);
  const coworkerApi = useMemo(() => desktopCoworkerApi(selectedWorkspaceId), [selectedWorkspaceId]);
  const dashboardSources = useMemo<Record<"tasks" | "automations" | "coworkers" | "artifacts", WidgetSource>>(() => ({
    tasks: {
      load: async () => {
        const page = await invoke<TaskPageView>("list_tasks", { workspaceId: selectedWorkspaceId, status: null, cursor: null, limit: 50 });
        return summarizeByStatus(page.items, Object.fromEntries(TASK_STATUSES.map((status) => [status, taskStatusLabel(status)])), page.nextCursor !== null);
      },
    },
    automations: {
      load: async () => {
        const page = await automationApi.list(undefined);
        return summarizeByStatus(page.items, {}, page.next_cursor !== null && page.next_cursor !== undefined);
      },
    },
    coworkers: {
      load: async () => {
        const page = await coworkerApi.list(undefined);
        return summarizeTotal("Coworkers", page.items.length, page.next_cursor !== null);
      },
    },
    artifacts: { unavailable: "No Workspace-wide recent-artifacts projection is served yet; artifacts are only listed per Task." },
  }), [selectedWorkspaceId, automationApi, coworkerApi]);
  const delegationProfileApi = useMemo(() => desktopDelegationProfileCatalogApi(selectedWorkspaceId), [selectedWorkspaceId]);
  const goalApi = useMemo(() => desktopGoalApi(selectedWorkspaceId), [selectedWorkspaceId]);
  const notificationApi = useMemo(() => maybeDesktopNotificationApi(selectedWorkspaceId), [selectedWorkspaceId]);
  const suggestionApi = useMemo(() => maybeDesktopSuggestionApi(selectedWorkspaceId), [selectedWorkspaceId]);
  const rootStatusRequestKeys = useRef(new Map<string, string>());
  const selectedWorkspaceIdRef = useRef(selectedWorkspaceId);
  selectedWorkspaceIdRef.current = selectedWorkspaceId;
  const workspaceSelectionEpoch = useRef(0);
  const selectWorkspace = useCallback((workspaceId: string) => {
    const previousWorkspaceId = selectedWorkspaceIdRef.current;
    if (previousWorkspaceId !== workspaceId) {
      workspaceSelectionEpoch.current += 1;
      setResourceMessage(null);
      setAutomationTemplateFlow(null);
    }
    if (!previousWorkspaceId && workspaceId) {
      setConversationDrafts(current => {
        if (!Object.hasOwn(current, "")) return current;
        const { "": unscopedDraft, ...workspaceDrafts } = current;
        return { ...workspaceDrafts, [workspaceId]: unscopedDraft };
      });
    }
    selectedWorkspaceIdRef.current = workspaceId;
    setSelectedWorkspaceId(workspaceId);
  }, []);
  const applyConversationNavigationIntent = useCallback((intent: ConversationNavigationIntent) => {
    if (intent.kind === "OPEN_WORKSPACE") selectWorkspace(intent.workspaceId);
    if (intent.kind !== "OPEN_CONVERSATION") setNewChatRequestId(current => current + 1);
    setSelectedConversationId(intent.kind === "OPEN_CONVERSATION" ? intent.conversationId : null);
  }, [selectWorkspace]);
  const requestConversationNavigation = useCallback((intent: ConversationNavigationIntent, blockedFocusTarget?: HTMLElement | null) => {
    const navigated = requestPageNavigation("Conversations", blockedFocusTarget);
    if (navigated) applyConversationNavigationIntent(intent);
    else setPendingConversationNavigationIntent(intent);
    return navigated;
  }, [applyConversationNavigationIntent, requestPageNavigation]);
  useEffect(() => {
    if (!pendingConversationNavigationIntent) return;
    if (page === "Conversations") {
      applyConversationNavigationIntent(pendingConversationNavigationIntent);
      setPendingConversationNavigationIntent(null);
    } else if (page === "Library" && pendingLibraryNavigation === null) {
      // The user chose to stay in Library; discard the deferred selection intent with the route.
      setPendingConversationNavigationIntent(null);
    }
  }, [applyConversationNavigationIntent, page, pendingConversationNavigationIntent, pendingLibraryNavigation]);
  useEffect(() => {
    rootStatusRequestKeys.current.clear();
  }, [selectedWorkspaceId]);

  const refreshRuntime = async () => {
    const generation = ++runtimeRefreshGeneration.current;
    setRuntime(null);
    setRuntimeError(null);
    setRuntimeLifecycle(null);
    setRuntimeLifecycleLoading(false);
    setRuntimeLifecycleError(null);
    try {
      const status = await invoke<RuntimeStatus>("get_runtime_status");
      if (generation !== runtimeRefreshGeneration.current) return;
      setRuntime(status);
      setRuntimeError(status.detail);
      const runtimeId = status.runtimeId;
      const incarnationId = status.localIncarnationId;
      if (!status.operatorReady || !runtimeId || !incarnationId) {
        setRuntimeLifecycle(null);
        setRuntimeLifecycleLoading(false);
        setRuntimeLifecycleError(null);
        return;
      }
      const identity = `${runtimeId}:${incarnationId}`;
      setRuntimeLifecycle(null);
      setRuntimeLifecycleLoading(true);
      setRuntimeLifecycleError(null);
      try {
        const view = await invoke<RuntimeLifecycleView>("get_runtime_lifecycle", { runtimeId, incarnationId });
        if (generation !== runtimeRefreshGeneration.current) return;
        if (view.runtimeId !== runtimeId || view.incarnation.runtimeId !== runtimeId || view.incarnation.runtimeIncarnationId !== incarnationId) {
          throw new Error("Lifecycle identity changed");
        }
        setRuntimeLifecycle({ identity, view });
        setRuntimeLifecycleLoading(false);
      } catch {
        if (generation === runtimeRefreshGeneration.current) {
          setRuntimeLifecycleLoading(false);
          setRuntimeLifecycleError("Persisted Runtime lifecycle details are unavailable for this incarnation.");
        }
      }
    } catch {
      if (generation !== runtimeRefreshGeneration.current) return;
      setRuntime(null);
      setRuntimeLifecycle(null);
      setRuntimeLifecycleLoading(false);
      setRuntimeLifecycleError(null);
      setRuntimeError("Desktop Runtime status is unavailable.");
    }
  };

  const previewRuntimeStop = async () => {
    const runtimeId = runtime?.runtimeId;
    const incarnationId = runtime?.localIncarnationId;
    if (!runtime?.operatorReady || !runtimeId || !incarnationId) {
      setStopPreview({ status: "ERROR", message: "The local Runtime is not connected." });
      return;
    }
    setStopPreview({ status: "LOADING" });
    try {
      const preview = await invoke<RuntimeStopPreviewView>("get_runtime_stop_preview", { runtimeId, incarnationId });
      setStopPreview({ status: "READY", preview });
    } catch {
      setStopPreview({ status: "ERROR", message: "The Runtime did not return a stop preview." });
    }
  };

  const refreshWorkspaces = async (onAcceptedProjection?: (items: WorkspaceView[]) => void) => {
    const generation = workspaceListGeneration.current.begin();
    setWorkspaceListState("LOADING");
    setWorkspaceError(null);
    try {
      const items = await invoke<WorkspaceView[]>("list_workspaces");
      if (!workspaceListGeneration.current.isCurrent(generation)) return false;
      reconcileWorkspacePolicyUpdateRequests(pendingWorkspacePolicyRequests.current, items);
      setWorkspacePolicyRequests([...pendingWorkspacePolicyRequests.current.values()]);
      const projection = updateWorkspaceProjection((current) => mergeWorkspaceListProjection(current, items));
      onAcceptedProjection?.(projection);
      setWorkspaceError(null);
      setWorkspaceListState("READY");
      return true;
    } catch {
      if (workspaceListGeneration.current.isCurrent(generation)) {
        setWorkspaceError("Workspace list is unavailable until the authenticated local Operator API is ready.");
        setWorkspaceListState("ERROR");
      }
      return false;
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
    const generation = agentProfilesGeneration.current.begin(workspaceId);
    try {
      const page = await invoke<AgentProfilePageView>("list_agent_profiles", { workspaceId });
      if (!agentProfilesGeneration.current.isCurrent(workspaceId, generation, selectedWorkspaceIdRef.current)) return;
      setAgentProfiles(page.items);
      setAgentProfileError(null);
    } catch {
      if (!agentProfilesGeneration.current.isCurrent(workspaceId, generation, selectedWorkspaceIdRef.current)) return;
      setAgentProfiles([]);
      setAgentProfileError("Agent profiles and current Runtime offers are unavailable. Start the local Runtime and try again.");
    }
  };

  const refreshAgentBindings = async (workspaceId: string) => {
    if (!workspaceId) { setAgentBindings([]); setAgentBindingError(null); return []; }
    const generation = agentBindingsGeneration.current.begin(workspaceId);
    try {
      const page = await invoke<AgentBindingPageView>("list_agent_bindings", { workspaceId });
      if (!agentBindingsGeneration.current.isCurrent(workspaceId, generation, selectedWorkspaceIdRef.current)) return null;
      setAgentBindings(current => mergeAgentBindingProjections(current, page.items));
      reconcileAgentBindingMutationRequests(pendingAgentBindingMutations.current, workspaceId, page.items);
      setAgentBindingError(null);
      return page.items;
    } catch {
      if (agentBindingsGeneration.current.isCurrent(workspaceId, generation, selectedWorkspaceIdRef.current)) {
        setAgentBindingError("Workspace agent bindings are unavailable. Start the local Runtime and try again.");
      }
      return null;
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
    const workspaceId = selectedWorkspaceIdRef.current;
    if (!workspaceId) return;
    const action = providerKey === "CODEX" ? "probe" : "probe:OPENCODE";
    const displayName = providerKey === "CODEX" ? "Codex" : "OpenCode";
    const isCurrentWorkspace = () => selectedWorkspaceIdRef.current === workspaceId;
    updateAgentActionFeedback(workspaceId, { busy: action, message: null, error: null });
    try {
      if (!workspaceId || !runtimeBindings.some((binding) => binding.workspaceId === workspaceId && binding.runtimeId === runtime?.runtimeId && binding.status === "ACTIVE")) {
        if (isCurrentWorkspace()) updateAgentActionFeedback(workspaceId, { error: `Enroll this computer in the selected Workspace before probing ${displayName}.` });
        return;
      }
      const profile = providerKey === "CODEX"
        ? await desktopAgentCatalogApi.probeCodex(workspaceId)
        : await desktopAgentCatalogApi.probeOpenCode(workspaceId);
      if (profile.providerKey !== providerKey) {
        throw new Error("The local Runtime returned a different agent profile than requested.");
      }
      if (!isCurrentWorkspace()) return;
      setAgentProfiles((profiles) => [profile, ...profiles.filter((item) => item.agentProfileId !== profile.agentProfileId)]);
      updateAgentActionFeedback(workspaceId, { message: providerKey === "OPENCODE"
        ? "OpenCode profile probe finished. The refreshed catalog is display-only; authentication, inference, model selection, and Task execution are not qualified."
        : "Codex profile probe completed. Review the current offer below; this does not validate model entitlement or run a Task." });
      await refreshAgentInstallations();
      if (isCurrentWorkspace()) await refreshAgentProfiles(workspaceId);
    } catch {
      if (isCurrentWorkspace()) updateAgentActionFeedback(workspaceId, { error: `${displayName} profile probe could not complete. The local Runtime or probe route may be unavailable; refresh Runtime status and try again.` });
    } finally {
      setAgentActionFeedback(current => clearAgentActionBusyIfMatches(current, workspaceId, action));
    }
  };

  const probeCodexProfile = () => probeAgentProfile("CODEX");
  const probeOpenCodeProfile = () => probeAgentProfile("OPENCODE");

  const createAgentBinding = async (profile: AgentProfileView, leadEligible: boolean) => {
    const workspaceId = selectedWorkspaceIdRef.current;
    if (!workspaceId) return;
    const isCurrentWorkspace = () => selectedWorkspaceIdRef.current === workspaceId;
    if (!profileHasFreshAdmissionOffer(profile)) {
      if (isCurrentWorkspace()) updateAgentActionFeedback(workspaceId, { error: "Refresh the profile and confirm a fresh compatible Runtime offer before creating a binding." });
      return;
    }
    if (!runtimeBindings.some((binding) => binding.workspaceId === workspaceId && binding.runtimeId === runtime?.runtimeId && binding.status === "ACTIVE")) {
      if (isCurrentWorkspace()) updateAgentActionFeedback(workspaceId, { error: "Enroll this computer in the selected Workspace before creating an agent binding." });
      return;
    }
    const action = `create:${profile.agentProfileId}`;
    updateAgentActionFeedback(workspaceId, { busy: action, message: null, error: null });
    try {
      await invoke<AgentBindingView>("create_agent_binding", {
        workspaceId,
        agentProfileId: profile.agentProfileId,
        leadEligible,
        runtimeId: runtime?.runtimeId ?? null,
        requestId: crypto.randomUUID(),
      });
      if (isCurrentWorkspace()) updateAgentActionFeedback(workspaceId, { message: `${profile.displayName} was added to this Workspace in the disabled state. Enable it separately when you are ready.` });
      await refreshAgentBindings(workspaceId);
    } catch {
      if (isCurrentWorkspace()) updateAgentActionFeedback(workspaceId, { error: "The Workspace binding could not be created. Refresh the profile and selected Workspace, then try again." });
    } finally {
      setAgentActionFeedback(current => clearAgentActionBusyIfMatches(current, workspaceId, action));
    }
  };

  const changeAgentBindingState = async (binding: AgentBindingView, operation: "enable" | "disable") => {
    if (selectedWorkspaceIdRef.current !== binding.workspaceId) return;
    const currentWorkspace = workspacesRef.current.find((item) => item.workspaceId === binding.workspaceId);
    if (operation === "enable" && !runtimeBindings.some((item) => item.workspaceId === binding.workspaceId && item.runtimeId === runtime?.runtimeId && item.status === "ACTIVE")) {
      updateAgentActionFeedback(binding.workspaceId, { error: "Enroll this computer in the selected Workspace before enabling an agent." });
      return;
    }
    if (operation === "disable" && currentWorkspace?.defaultAgentBindingId === binding.agentBindingId) {
      updateAgentActionFeedback(binding.workspaceId, { error: "Clear or change the Workspace default before disabling this agent." });
      return;
    }
    const pending = getOrCreateAgentBindingMutationRequest(pendingAgentBindingMutations.current, {
      operation,
      workspaceId: binding.workspaceId,
      agentBindingId: binding.agentBindingId,
      expectedVersion: binding.version,
    }, () => crypto.randomUUID());
    const command = operation === "enable" ? "enable_agent_binding" : "disable_agent_binding";
    const desiredEnabled = operation === "enable";
    const busyAction = `${operation}:${binding.agentBindingId}`;
    const isCurrentWorkspace = () => selectedWorkspaceIdRef.current === binding.workspaceId;
    updateAgentActionFeedback(binding.workspaceId, { busy: busyAction });
    if (isCurrentWorkspace()) {
      updateAgentActionFeedback(binding.workspaceId, { message: null, error: null });
    }
    try {
      // A module-backed agent (OpenCode) is enabled only against the latest fresh
      // descriptor revision: read the current one and refresh it when none is fresh.
      let descriptorRevision: string | null = null;
      if (operation === "enable"
        && agentProfiles.find(profile => profile.agentProfileId === binding.agentProfileId)?.providerKey === "OPENCODE") {
        const descriptor = await desktopAgentControlDescriptorApi.get(binding.workspaceId, binding.agentProfileId)
          .catch(() => desktopAgentControlDescriptorApi.refresh(binding.workspaceId, binding.agentProfileId));
        descriptorRevision = descriptor.descriptor_digest;
      }
      const updated = await invoke<AgentBindingView>(command, {
        workspaceId: binding.workspaceId,
        agentBindingId: binding.agentBindingId,
        expectedVersion: binding.version,
        requestId: pending.requestId,
        ...(operation === "enable" ? { descriptorRevision } : {}),
      });
      if (updated.workspaceId !== binding.workspaceId
        || updated.agentBindingId !== binding.agentBindingId
        || updated.version <= binding.version
        || updated.enabled !== desiredEnabled) {
        throw new Error("The AgentBinding mutation response did not match its request.");
      }
      clearAgentBindingMutationRequest(pendingAgentBindingMutations.current, pending);
      if (isCurrentWorkspace()) {
        setAgentBindings(current => mergeAgentBindingProjection(current, updated));
        updateAgentActionFeedback(binding.workspaceId, { message: operation === "enable"
          ? "This agent is enabled for future work. It will not become the Workspace default or start anything on its own."
          : "New Conversation and Task sessions cannot use this agent. Sessions already admitted stay attached; later Task steps may need another enabled agent." });
      }
      const refreshed = await refreshAgentBindings(binding.workspaceId);
      if (isCurrentWorkspace()) {
        const latestBinding = refreshed === null
          ? updated
          : mergeAgentBindingProjections([updated], refreshed).find(item => item.workspaceId === binding.workspaceId && item.agentBindingId === binding.agentBindingId);
        updateAgentActionFeedback(binding.workspaceId, {
          message: agentBindingMutationSuccessMessage(operation, latestBinding, refreshed !== null),
        });
      }
    } catch {
      const [refreshed, workspaceRefreshed] = await Promise.all([
        refreshAgentBindings(binding.workspaceId),
        operation === "disable" ? refreshWorkspaces() : Promise.resolve(true),
      ]);
      const current = refreshed?.find(item => item.workspaceId === binding.workspaceId && item.agentBindingId === binding.agentBindingId);
      if (isCurrentWorkspace()) {
        const latestWorkspace = workspaceRefreshed
          ? workspacesRef.current.find(item => item.workspaceId === binding.workspaceId)
          : undefined;
        updateAgentActionFeedback(binding.workspaceId, { error: agentBindingMutationFailureMessage(
          operation,
          binding.agentBindingId,
          binding.version,
          current,
          latestWorkspace?.defaultAgentBindingId,
        ) });
      }
    } finally {
      setAgentActionFeedback(current => clearAgentActionBusyIfMatches(current, binding.workspaceId, busyAction));
    }
  };

  const enableAgentBinding = (binding: AgentBindingView) => changeAgentBindingState(binding, "enable");
  const disableAgentBinding = (binding: AgentBindingView) => changeAgentBindingState(binding, "disable");

  useEffect(() => {
    const bootstrap = async () => {
      try {
        const initialStatus = await invoke<RuntimeStatus>("get_runtime_status");
        if (!initialStatus.processRunning && initialStatus.daemonAvailable) {
          try {
            await invoke<RuntimeStatus>("start_local_runtime");
          } catch {
            // Manual fallback if auto-start doesn't complete
          }
        }
      } catch {
        // Desktop runtime query failure handled by refreshRuntime
      }
      await refreshRuntime();
      await refreshWorkspaces();
    };
    void bootstrap();
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
    const executorBinding = runtimeBindings.some(binding => binding.workspaceId === selectedWorkspaceId
      && binding.runtimeId === runtime?.runtimeId && binding.status === "ACTIVE" && binding.roles.includes("EXECUTOR"));
    if (page !== "Settings" || runtime?.operatorReady !== true || !runtime?.runtimeId
      || !runtime.localIncarnationId || !selectedWorkspaceId || !executorBinding) {
      setRuntimeInventoryStreamStatus("UNAVAILABLE");
      return;
    }
    const controller = new RuntimeInventoryStreamController(
      selectedWorkspaceId,
      runtime.runtimeId,
      desktopRuntimeInventoryStreamDriver,
      () => {
        if (selectedWorkspaceIdRef.current === selectedWorkspaceId) void refreshAgentProfiles(selectedWorkspaceId);
      },
      setRuntimeInventoryStreamStatus,
    );
    controller.start();
    return () => { void controller.dispose(); };
  }, [page, runtime?.operatorReady, runtime?.runtimeId, runtime?.localIncarnationId, selectedWorkspaceId, runtimeBindings]);

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
    if (workspaceListState === "READY" && workspaces.length > 0
      && !workspaces.some((workspace) => workspace.workspaceId === selectedWorkspaceId)
      && canRestoreInitialWorkspaceSelection(workspaceSelectionEpoch.current)) {
      selectWorkspace(workspaces[0].workspaceId);
    }
  }, [workspaces, selectedWorkspaceId, workspaceListState, selectWorkspace]);

  useEffect(() => {
    previewGeneration.current += 1;
    const generation = ++resourceListGeneration.current;
    setPreviewResourceId(null);
    setPreviewResourceRevisionId(null);
    setPreviewResourceName(null);
    setResourcePreview(null);
    setResourcePreviewHighlights(null);
    setPreviewError(null);
    setResourceNextCursor(null);
    if (!selectedWorkspaceId) { setResources([]); return; }
    setResources([]);
    setResourceMessage(null);
    setResourceListFailed(false);
    let active = true;
    void invoke<ResourcePageView>("list_resources", { workspaceId: selectedWorkspaceId, cursor: null }).then((page) => {
      if (active && generation === resourceListGeneration.current) {
        setResources(page.items);
        setResourceNextCursor(page.nextCursor);
      }
    }).catch(() => {
      // A failed read proves nothing about the Workspace, so the Library must not
      // claim that the Workspace has no files.
      if (active) {
        setResourceListFailed(true);
        setResourceMessage(null);
      }
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
    const savedDraft = instructionDrafts.current.get(selectedWorkspaceId);
    setInstructionText(savedDraft ?? "");
    setInstructionMessage(null);
    setLatestInstructionsAfterConflict(null);
    if (!selectedWorkspaceId) return;
    const requestContext = { workspaceId: selectedWorkspaceId, generation };
    const isCurrent = () => canApplyWorkspaceInstructionResponse(
      requestContext,
      { workspaceId: selectedWorkspaceIdRef.current, generation: instructionGeneration.current },
    );
    let active = true;
    void invoke<WorkspaceInstructionView[]>("list_workspace_instructions", { workspaceId: selectedWorkspaceId })
      .then(async (revisions) => {
        if (!active || !isCurrent()) return;
        setWorkspaceInstructions(revisions);
        if (savedDraft !== undefined) return;
        const current = revisions.at(-1);
        const resourceId = current?.contentRef.resource_id;
        const revisionId = current?.contentRef.revision_id;
        if (!resourceId || !revisionId) return;
        const content = await invoke<string>("preview_resource_text", {
          workspaceId: selectedWorkspaceId,
          resourceId,
          revisionId,
        });
        if (active && isCurrent() && instructionDrafts.current.get(selectedWorkspaceId) === undefined) {
          setInstructionText(content);
        }
      })
      .catch((error) => {
        if (active && isCurrent()) {
          const detail = typeof error === "string" ? error : error instanceof Error ? error.message : "";
          setInstructionMessage(detail.includes("revision changed after selection")
            ? "The Resource linked to these instructions has changed. LiteCowork kept the pinned revision and did not show newer content."
            : "Workspace instructions could not be loaded. The local Runtime may still be starting.");
        }
      });
    return () => { active = false; };
  }, [selectedWorkspaceId, runtime?.operatorReady]);

  const changeWorkspaceInstructions = (text: string) => {
    if (selectedWorkspaceId) instructionDrafts.current.set(selectedWorkspaceId, text);
    setInstructionText(text);
  };

  const useLatestWorkspaceInstructions = () => {
    if (selectedWorkspaceId) instructionDrafts.current.delete(selectedWorkspaceId);
    setInstructionText(latestInstructionsAfterConflict ?? "");
    setLatestInstructionsAfterConflict(null);
  };

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
      runtimeRefreshGeneration.current += 1;
      setRuntime(status);
      setRuntimeError(status.detail);
      await refreshRuntime();
      await refreshWorkspaces();
    } catch (error) {
      setRuntimeError(typeof error === "string" ? error : "Runtime startup failed.");
      await refreshRuntime();
    } finally {
      setRuntimeBusy(false);
    }
  };

  const createWorkspace = async () => {
    if (runtime?.operatorReady !== true) {
      setWorkspaceError("Confirm authenticated local Runtime readiness before creating a Workspace.");
      return;
    }
    if (!workspaceCreateGuard.current.tryAcquire()) return;
    const pending = nextWorkspaceCreateRequest(workspaceName, pendingWorkspaceRequestRef.current, () => crypto.randomUUID());
    if (!pending) {
      setWorkspaceError(pendingWorkspaceRequestRef.current
        ? "Retry the existing Workspace request with its original name before starting another creation."
        : "Enter a Workspace name before creating it.");
      workspaceCreateGuard.current.release();
      return;
    }
    pendingWorkspaceRequestRef.current = pending;
    setPendingWorkspaceRequest(pending);
    const selectionEpochAtRequest = workspaceSelectionEpoch.current;
    setWorkspaceCreateBusy(true);
    setWorkspaceError(null);
    try {
      const created = await invoke<WorkspaceView>("create_workspace", {
        name: pending.name,
        requestId: pending.requestId,
      });
      workspaceListGeneration.current.commitCreated(created, (update) => updateWorkspaceProjection(update));
      if (canAutoSelectCreatedWorkspace(selectionEpochAtRequest, workspaceSelectionEpoch.current)) {
        selectWorkspace(created.workspaceId);
      }
      setWorkspaceName("");
      pendingWorkspaceRequestRef.current = null;
      setPendingWorkspaceRequest(null);
      setWorkspaceListState("READY");
    } catch (error) {
      setWorkspaceError(typeof error === "string" ? error : "Workspace could not be created.");
    } finally {
      setWorkspaceCreateBusy(false);
      workspaceCreateGuard.current.release();
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
        conversationId: null,
        sourceMessageRefs: [],
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
        requestPageNavigation("Work");
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
    if (!workspace || workspacePolicyBusyRef.current) return;
    workspacePolicyBusyRef.current = true;
    setWorkspacePolicyBusy(true);
    setWorkspaceError(null);
    const pending = workspacePolicyUpdateRequest(pendingWorkspacePolicyRequests.current, {
      workspaceId: workspace.workspaceId,
      expectedVersion: workspace.version,
      replicationPolicy,
    }, () => crypto.randomUUID());
    setWorkspacePolicyRequests([...pendingWorkspacePolicyRequests.current.values()]);
    try {
      const updated = await invoke<WorkspaceView>("update_workspace_policy", {
        workspaceId: workspace.workspaceId,
        replicationPolicy,
        expectedVersion: pending.expectedVersion,
        requestId: pending.requestId,
      });
      if (!isWorkspacePolicyUpdateResponse(updated, pending)) {
        throw new Error("The local Runtime did not confirm the requested Workspace policy version.");
      }
      clearWorkspacePolicyUpdateRequest(pendingWorkspacePolicyRequests.current, pending);
      setWorkspacePolicyRequests([...pendingWorkspacePolicyRequests.current.values()]);
      updateWorkspaceProjection((items) => mergeWorkspacePolicyProjection(items, updated));
    } catch (error) {
      const stale = error === "STALE_WORKSPACE_VERSION";
      if (stale) {
        clearWorkspacePolicyUpdateRequest(pendingWorkspacePolicyRequests.current, pending);
        setWorkspacePolicyRequests([...pendingWorkspacePolicyRequests.current.values()]);
      }
      let refreshedWorkspace: WorkspaceView | undefined;
      const refreshed = await refreshWorkspaces((items) => {
        refreshedWorkspace = items.find((item) => item.workspaceId === pending.workspaceId);
      });
      if (selectedWorkspaceIdRef.current === pending.workspaceId) {
        const latestWorkspace = refreshedWorkspace
          ?? workspacesRef.current.find((item) => item.workspaceId === pending.workspaceId);
        const refreshOutcome = workspacePolicyRefreshOutcome(latestWorkspace, pending);
        if (refreshOutcome === "CONFIRMED") {
          setWorkspaceError(null);
        } else {
          setWorkspaceError(stale
            ? refreshed
              ? "This Workspace changed before the setting was saved. Review its latest state before trying again."
              : "This Workspace changed before the setting was saved, and its latest state could not be checked. Refresh Workspaces before trying again."
            : refreshOutcome === "SUPERSEDED"
              ? "This Workspace changed while the update was in flight. Review its latest saved policy before trying again."
              : refreshed
                ? "The Workspace policy update could not be confirmed. Retry the exact update or refresh the Workspace status."
                : "The Workspace policy update could not be confirmed, and the Workspace list could not be refreshed. The exact request is retained for retry.");
        }
      }
    } finally {
      workspacePolicyBusyRef.current = false;
      setWorkspacePolicyBusy(false);
    }
  };

  const archiveWorkspace = async () => {
    const workspace = workspaces.find((item) => item.workspaceId === selectedWorkspaceId);
    if (!workspace || workspace.status !== "ACTIVE" || workspaceArchiveBusy) return;
    setWorkspaceArchiveBusy(true);
    setWorkspaceError(null);
    let pending = pendingWorkspaceArchive.current;
    if (!pending || pending.workspaceId !== workspace.workspaceId || pending.version !== workspace.version) {
      pending = { workspaceId: workspace.workspaceId, version: workspace.version, requestId: crypto.randomUUID() };
      pendingWorkspaceArchive.current = pending;
    }
    try {
      const updated = await invoke<WorkspaceView>("archive_workspace", {
        workspaceId: workspace.workspaceId,
        expectedVersion: workspace.version,
        requestId: pending.requestId,
      });
      pendingWorkspaceArchive.current = null;
      updateWorkspaceProjection((items) => items.map((item) => item.workspaceId === updated.workspaceId ? updated : item));
    } catch (error) {
      const message = typeof error === "string" ? error : "Workspace could not be archived.";
      await refreshWorkspaces();
      setWorkspaceError(message);
    } finally {
      setWorkspaceArchiveBusy(false);
    }
  };

  const shortcutHandlers: Record<ShortcutId, () => void> = {
    "new-chat": () => {
      if (taskSaving) return;
      setSelectedConversationId(null);
      requestPageNavigation("Conversations");
    },
    "open-library": () => {
      requestPageNavigation("Library");
    },
  };
  const shortcutHandlersRef = useRef(shortcutHandlers);
  shortcutHandlersRef.current = shortcutHandlers;
  useEffect(() => {
    const onShortcut = (event: KeyboardEvent) => {
      const id = matchShortcut(event);
      if (!id) return;
      event.preventDefault();
      shortcutHandlersRef.current[id]();
    };
    document.addEventListener("keydown", onShortcut);
    return () => document.removeEventListener("keydown", onShortcut);
  }, []);

  /** Resolves true when the rename committed; false keeps the caller's draft. */
  const renameWorkspace = async (draftName: string): Promise<boolean> => {
    const workspace = workspaces.find((item) => item.workspaceId === selectedWorkspaceId);
    if (!workspace || workspaceRenameBusy) return false;
    if (workspace.status !== "ACTIVE") {
      setWorkspaceRenameError(renameFailureMessage("ARCHIVED", null));
      return false;
    }
    const checked = validateWorkspaceName(draftName);
    if (!checked.ok) {
      setWorkspaceRenameError(checked.reason);
      return false;
    }
    setWorkspaceRenameBusy(true);
    setWorkspaceRenameError(null);
    const pending = nextRenameRequest(pendingWorkspaceRename.current, workspace.workspaceId, workspace.version, checked.name, () => crypto.randomUUID());
    pendingWorkspaceRename.current = pending;
    try {
      const updated = await invoke<WorkspaceView>("rename_workspace", {
        workspaceId: workspace.workspaceId,
        name: checked.name,
        expectedVersion: workspace.version,
        requestId: pending.requestId,
      });
      pendingWorkspaceRename.current = null;
      updateWorkspaceProjection((items) => items.map((item) => item.workspaceId === updated.workspaceId ? updated : item));
      return true;
    } catch (error) {
      const failure = classifyRenameFailure(typeof error === "string" ? error : "");
      if (failure !== "OTHER") pendingWorkspaceRename.current = null;
      let latestName: string | null = null;
      if (failure === "STALE" || failure === "ARCHIVED") {
        await refreshWorkspaces((items) => {
          latestName = items.find((item) => item.workspaceId === workspace.workspaceId)?.name ?? null;
        });
      }
      setWorkspaceRenameError(renameFailureMessage(failure, latestName));
      return false;
    } finally {
      setWorkspaceRenameBusy(false);
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
      updateWorkspaceProjection((items) => items.map((item) => item.workspaceId === updated.workspaceId ? updated : item));
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
    const size = workspaceInstructionByteLength(instructionText);
    if (size > 64 * 1024) {
      setInstructionMessage("Workspace instructions are limited to 64 KiB of UTF-8 text.");
      return;
    }
    const pending = getOrCreateWorkspaceInstructionSave(pendingInstructions.current, {
      workspaceId: workspace.workspaceId,
      text: instructionText,
      expectedVersion: workspace.version,
      parentRevisions: workspaceInstructions.length > 0 ? [workspaceInstructions[workspaceInstructions.length - 1].revision] : [],
    }, () => crypto.randomUUID());
    pendingInstructions.current.set(workspace.workspaceId, pending);
    setInstructionBusy(true);
    setInstructionMessage("Saving a versioned Workspace instruction revision…");
    const requestContext = { workspaceId: workspace.workspaceId, generation };
    const isCurrent = () => canApplyWorkspaceInstructionResponse(
      requestContext,
      { workspaceId: selectedWorkspaceIdRef.current, generation: instructionGeneration.current },
    );
    let resourceImported = false;
    try {
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
        resourceImported = true;
        if (isCurrent()) {
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
      } catch (error) {
        const stale = typeof error === "string" && error.includes("STALE_WORKSPACE_VERSION");
        const failureKind = instructionSaveFailureKind({ stale, resourceImported });
        if (failureKind === "STALE" || failureKind === "STALE_AFTER_RESOURCE_IMPORT") {
          clearWorkspaceInstructionSave(pendingInstructions.current, workspace.workspaceId, pending.requestId);
          if (isCurrent()) {
            setInstructionMessage(failureKind === "STALE_AFTER_RESOURCE_IMPORT"
              ? "This Workspace changed while you were editing. No instruction revision was saved; the text Resource was imported and remains in the Library. Your draft is still here. Review the latest revision before saving again."
              : "This Workspace changed while you were editing. Your text is still here; review the latest revision, then save again.");
          }
          await refreshWorkspaces();
          try {
            const revisions = await invoke<WorkspaceInstructionView[]>("list_workspace_instructions", { workspaceId: workspace.workspaceId });
            if (isCurrent()) {
              setWorkspaceInstructions(revisions);
              const latestResourceId = revisions[revisions.length - 1]?.contentRef.resource_id;
              const latestRevisionId = revisions[revisions.length - 1]?.contentRef.revision_id;
              if (latestResourceId && latestRevisionId) {
                const latestText = await invoke<string>("preview_resource_text", {
                  workspaceId: workspace.workspaceId,
                  resourceId: latestResourceId,
                  revisionId: latestRevisionId,
                });
                if (isCurrent()) {
                  setLatestInstructionsAfterConflict(latestText);
                }
              }
            }
          } catch (reloadError) {
            const detail = typeof reloadError === "string" ? reloadError : reloadError instanceof Error ? reloadError.message : "";
            if (detail.includes("revision changed after selection") && isCurrent()) {
              setInstructionMessage("The latest saved instruction points to a Resource revision that has advanced. Newer bytes were not substituted; your draft is still here.");
            }
          }
        } else if (isCurrent()) {
          setInstructionMessage(failureKind === "RESOURCE_IMPORTED"
            ? "The text Resource was imported, but its instruction revision was not confirmed. It may remain in the Library. Retry this unchanged draft to reconcile with the same request IDs."
            : typeof error === "string"
              ? error
              : "Workspace instructions could not be saved. Retry to safely resume the same request.");
        }
        return;
      }

      clearWorkspaceInstructionSave(pendingInstructions.current, workspace.workspaceId, pending.requestId);
      instructionDrafts.current.delete(workspace.workspaceId);
      if (isCurrent()) {
        setInstructionMessage("Workspace instructions saved. New Tasks can pin this revision; existing Tasks remain unchanged.");
        setLatestInstructionsAfterConflict(null);
      }
      const workspaceRefreshed = await refreshWorkspaces();
      let historyRefreshed = false;
      try {
        const revisions = await invoke<WorkspaceInstructionView[]>("list_workspace_instructions", { workspaceId: workspace.workspaceId });
        historyRefreshed = true;
        if (isCurrent()) setWorkspaceInstructions(revisions);
      } catch {
        historyRefreshed = false;
      }
      if (isCurrent() && instructionSaveRefreshKind({ workspace: workspaceRefreshed, history: historyRefreshed }) === "REFRESH_PENDING") {
        setInstructionMessage("Workspace instructions were saved, but current status or history could not refresh. The revision is committed; reopen Settings or switch back to refresh it.");
      }
    } finally {
      setInstructionBusy(false);
    }
  };

  const importFiles = async (files: FileList | File[] | null, contextDocument: ContextDocumentCreateMetadata | null = null) => {
    const workspaceId = selectedWorkspaceIdRef.current;
    if (!files || !workspaceId) return;
    const selectionEpoch = workspaceSelectionEpoch.current;
    const isCurrentImport = () => selectedWorkspaceIdRef.current === workspaceId
      && workspaceSelectionEpoch.current === selectionEpoch;
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
    setResourceUploadResumeForget(null);
    let importedCount = 0;
    let recoveredCommitCount = 0;
    let activeFileName = "";
    let pausedFileName: string | null = null;
    let resumeRequiresForget = false;
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
        if (isCurrentImport()) setResourceMessage(`Checking “${displayName}” (${formatBytes(file.size)}) before starting or resuming its upload…`);
        const expectedDigest = `sha256:${await digestHex(await file.arrayBuffer())}`;
        if (resourcePauseRequested.current) {
          pausedFileName = displayName;
          break;
        }
        let session: ResourceUploadView | null = null;
        if (resume.uploadId) {
          try {
            const status: unknown = await invoke<unknown>("get_resource_upload", { workspaceId, uploadId: resume.uploadId });
            if (!status || typeof status !== "object" || Array.isArray(status)) {
              throw new Error("Saved upload progress response was invalid.");
            }
            session = status as ResourceUploadView;
          } catch {
            throw new Error("Saved upload progress could not be verified. Recovery details were kept, and no replacement upload session was started. Retry the same unchanged file when the local Runtime is available.");
          }
        }
        if (resourcePauseRequested.current) {
          pausedFileName = displayName;
          break;
        }
        if (session && (session.workspaceId !== workspaceId || session.uploadId !== resume.uploadId)) {
          throw new Error("Saved upload progress did not match this Workspace and upload. Recovery details were kept, and no replacement upload session was started.");
        }
        const expectedUploadIdentity: ResourceUploadIdentity = {
          workspaceId,
          displayName,
          mediaType,
          folderRelativePath,
          sizeBytes: file.size,
          contextDocument,
        };
        if (session && !["OPEN", "CONTENT_RECEIVED", "COMMITTED", "FAILED", "EXPIRED"].includes(session.state)) {
          throw new Error("The saved upload returned an unsupported status. Recovery details were kept, and no replacement upload session was started.");
        }
        if (session && (session.state === "FAILED" || session.state === "EXPIRED")) {
          resumeRequiresForget = true;
          if (isCurrentImport()) setResourceUploadResumeForget({
            workspaceId,
            resumeKey,
            uploadId: session.uploadId,
            displayName,
            terminalState: session.state,
          });
          throw new Error(`This upload is ${session.state}. No replacement session was started. Review the Library, then use “Forget upload resume details” if you want to start another upload.`);
        }
        if (session && (session.state === "OPEN" || session.state === "CONTENT_RECEIVED")) {
          const expiresAt = Date.parse(session.expiresAt);
          if (!Number.isFinite(expiresAt)) {
            throw new Error("The saved upload has an invalid expiry. Recovery details were kept, and no replacement upload session was started.");
          }
          if (expiresAt <= Date.now()) {
            resumeRequiresForget = true;
            if (isCurrentImport()) setResourceUploadResumeForget({
              workspaceId,
              resumeKey,
              uploadId: session.uploadId,
              displayName,
              terminalState: "EXPIRED",
            });
            throw new Error("This upload has expired. No replacement session was started. Review the Library, then use “Forget upload resume details” if you want to start another upload.");
          }
        }
        const sessionIdentityMatches = Boolean(
          session && matchesResourceUploadIdentity(session, expectedUploadIdentity),
        );
        const sessionMatches = Boolean(
          session
          && resumableResourceUploadSession(
            session,
            expectedUploadIdentity,
            expectedDigest,
            Date.now(),
          ),
        );
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
            const commitRequestId = prepareResourceUploadCommitRequestId(
              resume,
              (nextResume) => writeUploadResume(resumeKey, nextResume),
              () => crypto.randomUUID(),
            );
            const recovered = await invoke<ResourceView>("commit_resource_upload", {
              workspaceId,
              uploadId: session.uploadId,
              requestId: commitRequestId,
            });
            if (recovered.resourceId !== session.committedResourceId) {
              throw new Error("The recovered commit does not match the Resource recorded by the upload session.");
            }
            if (isCurrentImport()) {
              setResources((current) => [recovered, ...current.filter((item) => item.resourceId !== recovered.resourceId)]);
            }
            writeUploadResume(resumeKey, null);
            recoveredCommitCount += 1;
            continue;
          }
        }
        let canResume = Boolean(sessionMatches);
        if (session && canResume) {
          canResume = await receivedUploadRangesMatchFile(file, session);
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
        if (isCurrentImport()) setResourceMessage(`Adding file ${index + 1} of ${selected.length}: ${displayName} · ${formatBytes(uploaded)} of ${formatBytes(file.size)} uploaded`);
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
          if (isCurrentImport()) setResourceMessage(`Adding file ${index + 1} of ${selected.length}: ${displayName} · ${formatBytes(endExclusive)} of ${formatBytes(file.size)} uploaded`);
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
        const commitRequestId = prepareResourceUploadCommitRequestId(
          resume,
          (nextResume) => writeUploadResume(resumeKey, nextResume),
          () => crypto.randomUUID(),
        );
        const imported = await invoke<ResourceView>("commit_resource_upload", {
          workspaceId,
          uploadId: session.uploadId,
          requestId: commitRequestId,
        });
        if (isCurrentImport()) {
          setResources((current) => [imported, ...current.filter((item) => item.resourceId !== imported.resourceId)]);
        }
        writeUploadResume(resumeKey, null);
        importedCount += 1;
      }
      const completed = importedCount + recoveredCommitCount;
      if (pausedFileName) {
        const completedSummary = completed > 0 ? `${completed} ${completed === 1 ? "file was" : "files were"} saved. ` : "";
        if (isCurrentImport()) setResourceMessage(`${completedSummary}Upload paused before finishing “${pausedFileName}”. Any accepted chunks remain resumable until their session expires. Choose the same unchanged file again to continue.${skipped > 0 ? ` Excluded ${skipped} likely secret or generated ${skipped === 1 ? "file" : "files"}.` : ""}`);
      } else {
        if (isCurrentImport()) setResourceMessage(`${completed} ${completed === 1 ? "file" : "files"} added to this Workspace.${recoveredCommitCount > 0 ? ` Recovered ${recoveredCommitCount} earlier commit${recoveredCommitCount === 1 ? "" : "s"}.` : ""}${skipped > 0 ? ` Excluded ${skipped} likely secret or generated ${skipped === 1 ? "file" : "files"}.` : ""}`);
      }
    } catch (error) {
      const detail = typeof error === "string" ? error : error instanceof Error ? error.message : "Resource intake failed.";
      const completedCount = importedCount + recoveredCommitCount;
      const completedNoun = completedCount === 1 ? "file was" : "files were";
      const completedSummary = completedCount > 0
        ? `${completedCount} of ${selected.length} selected ${completedNoun} saved before import stopped${recoveredCommitCount > 0 ? ` (including ${recoveredCommitCount} earlier committed upload${recoveredCommitCount === 1 ? "" : "s"} recovered)` : ""}. `
        : "No selected files were confirmed saved. ";
      const resumeHint = activeFileName
        ? resumeRequiresForget
          ? " The saved upload identity remains until you explicitly forget it."
          : ` If an upload session was already created for “${activeFileName}”, choose the same unchanged file again to resume it while the session remains valid.`
        : "";
      const excludedSummary = skipped > 0 ? ` ${skipped} likely secret or generated ${skipped === 1 ? "file was" : "files were"} excluded.` : "";
      if (isCurrentImport()) setResourceMessage(`${completedSummary}${detail}${resumeHint}${excludedSummary}`);
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

  const forgetResourceUploadResume = () => {
    const pending = resourceUploadResumeForget;
    if (!pending || selectedWorkspaceIdRef.current !== pending.workspaceId) return;
    const saved = readUploadResumes()[pending.resumeKey];
    if (!saved || saved.uploadId !== pending.uploadId) {
      setResourceUploadResumeForget(null);
      setResourceMessage("The saved upload recovery record changed, so no local record was cleared. Review the Library and retry the same file to read its current status.");
      return;
    }
    if (!writeUploadResume(pending.resumeKey, null)) {
      setResourceMessage("LiteCowork could not clear the saved upload recovery record. The server session was not changed.");
      return;
    }
    setResourceUploadResumeForget(null);
    setResourceMessage(`Local upload recovery details for “${pending.displayName}” were forgotten. The server-side upload session was not deleted. Review the Library before selecting this file again.`);
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
    setPreviewResourceRevisionId(revisionId);
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
    selectWorkspace(pendingTaskSave.workspaceId);
    setOpenedTask(null);
    requestPageNavigation("Work");
  };
  const clearPendingTaskAfterReview = () => {
    if (!pendingTaskSave) return;
    const accepted = window.confirm("If the earlier save committed, starting another Task can create a duplicate. Clear its retry identity only after checking Work in the original Workspace. Continue?");
    if (!accepted) return;
    setPendingTaskSave(null);
    setTaskSaveError("Original retry cleared after your review. The next save uses the current Coworker and lead settings.");
  };

  const mainAreaInert = libraryNavigationMainIsInert(navigationOpen, pendingLibraryNavigation !== null);
  const routeContentInert = mainAreaInert || (workbenchNarrow && workbenchOpen);
  const workbenchLayoutKey = selectedWorkspaceId || "__no_workspace__";
  const workbenchDesktopVisible = workbenchLayoutByWorkspace[workbenchLayoutKey] !== false;
  const workbenchShown = workbenchNarrow ? workbenchOpen : workbenchDesktopVisible;

  return (
    <div className={`app-shell ${navigationOpen ? "navigation-open" : ""}`}>
      <a
        className="skip-link"
        href="#main-content"
        onClick={(event) => {
          event.preventDefault();
          document.getElementById("main-content")?.focus();
        }}
      >Skip to main content</a>
      {navigationOpen && <button className="sidebar-backdrop" type="button" tabIndex={-1} aria-label="Close main navigation" onClick={() => closeNavigation()} />}
      <aside
        id="main-navigation"
        ref={navigationDrawerRef}
        className={`sidebar ${navigationOpen ? "is-open" : ""}`}
        aria-label="Main navigation"
        role={navigationOpen ? "dialog" : undefined}
        aria-modal={navigationOpen ? true : undefined}
        inert={pendingLibraryNavigation !== null}
      >
        <div className="sidebar-top-bar">
          <div className="sidebar-brand-wrap" title="LiteCowork Workspace">
            <span className="sidebar-brand-icon" aria-hidden="true">◩</span>
            <span className="sidebar-brand-name">LiteCowork</span>
          </div>
          <div className="sidebar-top-nav-group">
            <button
              className="sidebar-icon-btn sidebar-close"
              type="button"
              aria-label="Close navigation"
              title="Collapse sidebar"
              onClick={() => closeNavigation()}
            >
              <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <rect x="3" y="3" width="18" height="18" rx="2" />
                <path d="M9 3v18" />
              </svg>
            </button>
          </div>
        </div>

        {/* 1. Workspace Switcher: Clear dropdown/selector for current local workspace roots */}
        <div className="sidebar-workspace-switcher">
          <span className="sidebar-workspace-folder-icon" aria-hidden="true">📂</span>
          <select
            className="sidebar-workspace-select"
            aria-label="Current Workspace Root"
            value={selectedWorkspaceId}
            disabled={taskSaving}
            onChange={(event) => {
              selectWorkspace(event.target.value);
              setSelectedConversationId(null);
            }}
          >
            {workspaces.map((ws) => (
              <option key={ws.workspaceId} value={ws.workspaceId}>
                {ws.name}
              </option>
            ))}
          </select>
          <span className="sidebar-workspace-chevron" aria-hidden="true">⌄</span>
        </div>

        {/* 2. New Chat pill button */}
        <div className="sidebar-primary-action">
          <button
            className="sidebar-new-chat-pill"
            type="button"
            aria-current={page === "Conversations" ? "page" : undefined}
            disabled={taskSaving}
            onClick={() => {
              const focusTarget = navigationOpen && window.innerWidth <= 900 ? navigationTriggerRef.current : null;
              const navigated = requestConversationNavigation({ kind: "NEW_CHAT" }, focusTarget);
              if (navigationOpen) closeNavigation(navigated);
            }}
          >
            <span className="sidebar-plus-glyph" aria-hidden="true">+</span>
            <span>New Chat</span>
            <span className="sidebar-chat-shortcut">{shortcutKeys("new-chat")}</span>
          </button>
        </div>

        {/* 3. Recent Chats: source-backed loading, unavailable and failure states; no invented count. */}
        <RecentChatsSidebar
          items={conversationsByWorkspace[selectedWorkspaceId] ?? []}
          status={!selectedWorkspaceId || runtime?.operatorReady !== true
            ? "UNAVAILABLE"
            : conversationListStatusByWorkspace[selectedWorkspaceId] ?? (page === "Conversations" ? "LOADING" : "IDLE")}
          runtimeReady={runtime?.operatorReady === true}
          selectedConversationId={page === "Conversations" ? selectedConversationId : null}
          showPagination={page === "Conversations"}
          hasMore={conversationPaginationByWorkspace[selectedWorkspaceId]?.hasMore ?? false}
          loadingMore={conversationPaginationByWorkspace[selectedWorkspaceId]?.loading ?? false}
          workspaceSelected={Boolean(selectedWorkspaceId)}
          onOpenConversation={(conversationId) => {
            const navigated = requestConversationNavigation({ kind: "OPEN_CONVERSATION", conversationId }, navigationGuardFocusTarget());
            if (navigationOpen) closeNavigation(navigated);
          }}
          onRetry={() => {
            if (page === "Conversations" && refreshConversationListRef.current) {
              refreshConversationListRef.current();
              return;
            }
            const navigated = requestPageNavigation("Conversations", navigationGuardFocusTarget());
            if (navigationOpen) closeNavigation(navigated);
          }}
          onLoadMore={() => loadMoreConversationChatsRef.current?.()}
        />

        {/* 4. Core Objects Nav (Clean & Uncluttered) */}
        <nav className="sidebar-nav-list" aria-label="Workspace Navigation">
          <button
            className={`sidebar-nav-btn ${page === "Projects" ? "selected" : ""}`}
            type="button"
            aria-current={page === "Projects" ? "page" : undefined}
            onClick={() => {
              const navigated = requestPageNavigation("Projects", navigationGuardFocusTarget());
              if (navigationOpen) closeNavigation(navigated);
            }}
          >
            <span className="sidebar-nav-icon" aria-hidden="true">▦</span>
            <span className="sidebar-nav-text">Projects</span>
          </button>

          {/* Needs You: Decision inbox for pending user approvals, credential prompts, blockers */}
          <button
            className={`sidebar-nav-btn ${page === "Needs You" ? "selected" : ""}`}
            type="button"
            aria-current={page === "Needs You" ? "page" : undefined}
            onClick={() => {
              const navigated = requestPageNavigation("Needs You", navigationGuardFocusTarget());
              if (navigationOpen) closeNavigation(navigated);
            }}
          >
            <span className="sidebar-nav-icon">◇</span>
            <span className="sidebar-nav-text">Needs You</span>
          </button>

          <details
            className="sidebar-more"
            open={sidebarMoreOpen}
            onToggle={event => setSidebarMoreOpen(event.currentTarget.open)}
          >
            <summary className="sidebar-more-summary">
              More{SIDEBAR_MORE_PAGES.includes(page) ? ` · ${navigation.find(item => item.label === page)?.displayLabel ?? page}` : ""}
            </summary>
          {navigation.filter((item) => SIDEBAR_MORE_PAGES.includes(item.label)).map((item) => (
            <button
              key={item.label}
              className={`sidebar-nav-btn ${page === item.label ? "selected" : ""}`}
              type="button"
              aria-current={page === item.label ? "page" : undefined}
              onClick={() => {
                const navigated = requestPageNavigation(item.label, navigationGuardFocusTarget());
                if (navigated) {
                  if (item.label === "Home" || item.label === "Work") setOpenedTask(null);
                  setSidebarMoreOpen(true);
                }
                if (navigationOpen) closeNavigation(navigated);
              }}
            >
              <span className="sidebar-nav-icon" aria-hidden="true">•</span>
              <span className="sidebar-nav-text">{item.displayLabel ?? item.label}</span>
            </button>
          ))}
          </details>
        </nav>

        {/* 5. Settings: Local runtime, agent harnesses, diagnostics */}
        <div className="sidebar-bottom-bar">
          <button
            className={`sidebar-settings-btn ${page === "Settings" ? "selected" : ""}`}
            type="button"
            onClick={() => {
              const navigated = requestPageNavigation("Settings", navigationGuardFocusTarget());
              if (navigationOpen) closeNavigation(navigated);
            }}
          >
            <span className="sidebar-nav-icon">⚙</span>
            <span className="sidebar-nav-text">Settings</span>
            <span
              className={`sidebar-runtime-indicator ${runtime?.state === "READY" ? "online" : runtime?.state === "DEGRADED" ? "warning" : ""}`}
              title={`Local Runtime: ${runtime?.state ?? "Checking"}`}
            />
          </button>
        </div>
      </aside>

      <main id="main-content" tabIndex={-1} className="main-area" inert={mainAreaInert} aria-hidden={mainAreaInert || undefined}>
        {page !== "Conversations" && (
          <header className="topbar">
          <button
            ref={navigationTriggerRef}
            className="icon-button mobile-navigation-toggle"
            type="button"
            aria-label="Open main navigation"
            aria-controls="main-navigation"
            aria-expanded={navigationOpen}
            onClick={() => { setWorkbenchOpen(false); setNavigationOpen(true); }}
          >
            <span aria-hidden="true">☰</span>
            <span>Menu</span>
          </button>
          <div className="breadcrumbs"><span>LiteCowork</span><span className="crumb-separator">/</span><strong>{page === "Home" ? "New task" : page}</strong></div>
          <div className="topbar-actions">
            <button
              ref={workbenchTriggerRef}
              className="workbench-toggle"
              type="button"
              aria-label={workbenchShown ? "Hide Workbench" : "Show Workbench"}
              aria-controls="desktop-workbench"
              aria-expanded={workbenchShown}
              onClick={() => {
                if (workbenchNarrow) {
                  if (workbenchOpen) closeWorkbench();
                  else {
                    closeNavigation(false);
                    setWorkbenchOpen(true);
                  }
                } else {
                  setWorkbenchLayoutByWorkspace(current => ({ ...current, [workbenchLayoutKey]: !workbenchDesktopVisible }));
                }
              }}
            >
              <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="3" y="3" width="18" height="18" rx="2" /><path d="M15 3v18" /></svg>
              <span>Workbench</span>
            </button>
          </div>
        </header>
        )}

        <div
          className={`workspace-shell ${workbenchShown ? "has-workbench" : ""}`}
          style={{ "--workbench-width": `${workbenchWidth}px` } as React.CSSProperties}
        >
          <div className="workspace-route-content" inert={routeContentInert} aria-hidden={routeContentInert || undefined}>
        {page === "Home" ? (
          <HomePage draft={draft} onDraftChange={(value) => { setDraft(value); setTaskSaveError(null); }} runtime={runtime} runtimeError={runtimeError} runtimeBusy={runtimeBusy} startRuntime={startRuntime} refreshRuntime={refreshRuntime} workspace={workspaces.find((workspace) => workspace.workspaceId === selectedWorkspaceId) ?? null} workspaceListState={workspaceListState} workspaceError={workspaceError} refreshWorkspaces={refreshWorkspaces} workspaceName={workspaceName} onWorkspaceNameChange={setWorkspaceName} workspaceCreateBusy={workspaceCreateBusy} pendingWorkspaceRequest={pendingWorkspaceRequest} createWorkspace={createWorkspace} workspacesCount={workspaces.length} coworkerApi={coworkerApi} coworkerSelection={homeCoworkerSelections[selectedWorkspaceId] ?? null} onCoworkerSelectionChange={updateHomeCoworkerSelection} taskInputs={taskInputSelections[selectedWorkspaceId] ?? []} onRemoveTaskInput={removeTaskInput} onOpenLibrary={() => requestPageNavigation("Library")} operatorReady={runtime?.operatorReady === true} taskSaving={taskSaving} taskSaveError={taskSaveError} hasPendingTaskSave={pendingTaskSave !== null} pendingTaskRetryMatches={pendingTaskMatchesComposer} onOpenPendingTask={openPendingTaskWorkspace} onClearPendingTaskAfterReview={clearPendingTaskAfterReview} onSaveTask={saveTask} onSettings={() => requestPageNavigation("Settings")} onOpenCoworkers={() => requestPageNavigation("Coworkers")} onWorkspaceUpdated={(updated) => updateWorkspaceProjection(current => current.map(item => item.workspaceId === updated.workspace_id && item.version <= updated.version ? { ...item, primaryCoworkerId: updated.primary_coworker_id, version: updated.version } : item))} onOpenWork={() => { setOpenedTask(null); requestPageNavigation("Work"); }} onOpenTask={(taskId) => { setOpenedTask({ workspaceId: selectedWorkspaceId, taskId }); requestPageNavigation("Work"); }} />
        ) : page === "Projects" ? (
          <WorkspaceProjectsPage
            workspaces={workspaces}
            selectedWorkspaceId={selectedWorkspaceId}
            listState={workspaceListState}
            error={workspaceError}
            runtimeReady={runtime?.operatorReady === true}
            busy={taskSaving}
            onRetry={() => void refreshWorkspaces()}
            onOpenSettings={() => requestPageNavigation("Settings")}
            onOpenWorkspace={(workspaceId) => {
              const navigated = requestConversationNavigation({ kind: "OPEN_WORKSPACE", workspaceId });
              if (navigationOpen) closeNavigation(navigated);
            }}
          />
        ) : page === "Conversations" ? (
          <ConversationsPage
            key={selectedWorkspaceId}
            workspaceId={selectedWorkspaceId}
            workspaceName={workspaces.find((workspace) => workspace.workspaceId === selectedWorkspaceId)?.name ?? "Workspace"}
            workspaceDefaultAgentBindingId={workspaces.find((workspace) => workspace.workspaceId === selectedWorkspaceId)?.defaultAgentBindingId ?? null}
            operatorReady={runtime?.operatorReady === true && Boolean(selectedWorkspaceId)}
            executionReady={isRuntimeExecutionReady({
              operatorReady: runtime?.operatorReady === true && Boolean(selectedWorkspaceId),
              runtimeId: runtime?.runtimeId,
              incarnationId: runtime?.localIncarnationId,
              lifecycleIdentity: runtimeLifecycle?.identity,
              lifecycleRuntimeId: runtimeLifecycle?.view.runtimeId,
              lifecycleIncarnationId: runtimeLifecycle?.view.incarnation.runtimeIncarnationId,
              availability: runtimeLifecycle?.view.availability,
              recoveryState: runtimeLifecycle?.view.incarnation.recoveryState,
            })}
            runtimeLifecycleLoading={runtimeLifecycleLoading}
            onRefreshRuntimeStatus={refreshRuntime}
            draft={conversationDrafts[selectedWorkspaceId] ?? ""}
            onDraftChange={value => setConversationDrafts(current => ({ ...current, [selectedWorkspaceId]: value }))}
            onOpenAgentSettings={() => requestPageNavigation("Settings")}
            selectedConversationId={selectedConversationId}
            newChatRequestId={newChatRequestId}
            onSelectConversation={setSelectedConversationId}
            onTaskCreated={(taskId) => {
              setOpenedTask({ workspaceId: selectedWorkspaceId, taskId });
              requestPageNavigation("Work");
            }}
            onConversationsLoaded={(loaded) => {
              setConversationsByWorkspace(current => ({ ...current, [selectedWorkspaceId]: loaded }));
            }}
            onConversationListStatusChange={updateConversationListStatus}
            onRegisterRefreshConversationList={registerRefreshConversationList}
            onRegisterLoadMoreConversations={registerLoadMoreConversationChats}
            onConversationListPaginationChange={updateConversationListPagination}
            onOpenNavigation={() => { setWorkbenchOpen(false); setNavigationOpen(true); }}
            navigationTriggerRef={navigationTriggerRef}
            navigationOpen={navigationOpen}
            workbenchOpen={workbenchShown}
            onToggleWorkbench={() => {
              if (workbenchNarrow) {
                if (workbenchOpen) closeWorkbench();
                else {
                  closeNavigation(false);
                  setWorkbenchOpen(true);
                }
              } else {
                setWorkbenchLayoutByWorkspace(current => ({ ...current, [workbenchLayoutKey]: !workbenchDesktopVisible }));
              }
            }}
          />
        ) : page === "Work" ? (
          <WorkPage key={selectedWorkspaceId} selectedWorkspaceId={selectedWorkspaceId} workspaceName={workspaces.find((workspace) => workspace.workspaceId === selectedWorkspaceId)?.name ?? "Workspace"} resourceNameCache={resourceNameCache} artifactApi={artifactApi} coworkerApi={coworkerApi} operatorReady={runtime?.operatorReady === true} initialTaskId={openedTask?.workspaceId === selectedWorkspaceId ? openedTask.taskId : null} onOpenSourceConversation={(conversationId) => { setSelectedConversationId(conversationId); requestPageNavigation("Conversations"); }} />
        ) : page === "Library" ? (
          <LibraryPage selectedWorkspaceId={selectedWorkspaceId} onSelectWorkspace={selectWorkspace} selectedWorkspaceIdRef={selectedWorkspaceIdRef} artifactApi={artifactApi} onOpenArtifactSource={openArtifactSource} rootStatusRequestKeys={rootStatusRequestKeys} resourceIndexRequestKeys={resourceIndexRequestKeys} workspaceVersion={workspaces.find((workspace) => workspace.workspaceId === selectedWorkspaceId)?.version ?? null} operatorReady={runtime?.operatorReady === true} resources={resources} onRevisionCommitted={(updated) => {
            setResources((current) => current.map((item) => item.resourceId === updated.resourceId ? updated : item));
            onLibraryDraftCommitted();
          }} onDraftSaveFailure={onLibraryDraftSaveFailed} onRefreshResources={() => setResourceCatalogRefreshNonce((current) => current + 1)} selectedTaskInputs={taskInputSelections[selectedWorkspaceId] ?? []} onToggleTaskInput={toggleTaskInput} onClearTaskInputs={clearTaskInputs} onUseSelectedInTask={() => requestPageNavigation("Home")} nextCursor={resourceNextCursor} pageBusy={resourcePageBusy} onLoadMore={loadMoreResources} busy={resourceBusy} pausePending={resourcePausePending} onPauseImport={pauseResourceImport} message={resourceMessage} listFailed={resourceListFailed} uploadResumeForget={resourceUploadResumeForget} onForgetUploadResume={forgetResourceUploadResume} onFiles={importFiles} onPreview={previewText} onClosePreview={() => { previewGeneration.current += 1; setPreviewResourceId(null); setPreviewResourceRevisionId(null); setPreviewResourceName(null); setResourcePreview(null); setResourcePreviewHighlights(null); setPreviewError(null); }} previewResourceId={previewResourceId} previewResourceName={previewResourceName} preview={resourcePreview} previewHighlights={resourcePreviewHighlights} previewError={previewError} />
        ) : page === "Needs You" ? (
          <TaskAttentionPage key={selectedWorkspaceId} workspaceId={selectedWorkspaceId} operatorReady={runtime?.operatorReady === true} onOpenTask={(taskId) => { setOpenedTask({ workspaceId: selectedWorkspaceId, taskId }); requestPageNavigation("Work"); }} />
        ) : page === "Coworkers" && runtime?.operatorReady !== true && selectedWorkspaceId ? (
          <OperatorUnavailablePanel eyebrow="WORKSPACE" title="Coworkers" icon="◉" message="Start the local Runtime before loading or changing Coworker settings. No Coworker state is cached in this view." onOpenSettings={() => requestPageNavigation("Settings")} />
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
              updateWorkspaceProjection(current => current.map(workspace => workspace.workspaceId === updated.workspace_id
                ? { ...workspace, primaryCoworkerId: updated.primary_coworker_id, version: updated.version }
                : workspace));
            }}
          />
        ) : page === "Dashboard" && (!selectedWorkspaceId || runtime?.operatorReady !== true) ? (
          <OperatorUnavailablePanel eyebrow="WORKSPACE" title="Dashboard" icon="▦" message="Start the local Runtime and select a Workspace to read dashboard projections. No value is cached or invented in this view." onOpenSettings={() => requestPageNavigation("Settings")} />
        ) : page === "Dashboard" ? (
          <WorkspaceDashboard key={selectedWorkspaceId} workspaceId={selectedWorkspaceId} storage={window.localStorage} sources={dashboardSources} />
        ) : page === "Goals" ? (
          <GoalsPage key={selectedWorkspaceId} api={goalApi} workspaceId={selectedWorkspaceId} onOpenTask={(taskId) => {
            setOpenedTask({ workspaceId: selectedWorkspaceId, taskId });
            requestPageNavigation("Work");
          }} />
        ) : page === "Notifications" && !selectedWorkspaceId ? (
          <div className="page-content subpage-content"><div className="eyebrow">WORKSPACE SIGNALS</div><h1>Notifications</h1><section className="subpage-panel empty-panel"><div className="empty-icon" aria-hidden="true">◔</div><h2>Select a Workspace</h2><p>Choose a Workspace to read its notification delivery history and policies.</p><button className="text-button" type="button" onClick={() => requestPageNavigation("Settings")}>Open Workspace settings <span aria-hidden="true">→</span></button></section></div>
        ) : page === "Notifications" && runtime?.operatorReady !== true ? (
          <OperatorUnavailablePanel eyebrow="WORKSPACE SIGNALS" title="Notifications" icon="◔" message="Start the local Runtime to read notification deliveries and policies. This view never sends a notification." onOpenSettings={() => requestPageNavigation("Settings")} />
        ) : page === "Notifications" && notificationApi ? (
          <NotificationsPage key={selectedWorkspaceId} api={notificationApi} workspaceId={selectedWorkspaceId} />
        ) : page === "Ideas" && !selectedWorkspaceId ? (
          <div className="page-content subpage-content"><div className="eyebrow">IDEAS</div><h1>Ideas</h1><section className="subpage-panel empty-panel"><div className="empty-icon" aria-hidden="true">✦</div><h2>Select a Workspace</h2><p>Create or select a Workspace in Settings to view saved Suggestions.</p><button className="text-button" type="button" onClick={() => requestPageNavigation("Settings")}>Open Workspace settings <span aria-hidden="true">→</span></button></section></div>
        ) : page === "Ideas" && runtime?.operatorReady !== true && selectedWorkspaceId ? (
          <OperatorUnavailablePanel eyebrow="IDEAS" title="Ideas" icon="✦" message="Start the local Runtime to load saved Suggestions. This view never starts work." onOpenSettings={() => requestPageNavigation("Settings")} />
        ) : page === "Ideas" && suggestionApi ? (
          <SuggestionsPage key={selectedWorkspaceId} api={suggestionApi} onTaskAccepted={(taskId) => {
            setOpenedTask({ workspaceId: selectedWorkspaceId, taskId });
            requestPageNavigation("Work");
          }} />
        ) : page === "Routines" && runtime?.operatorReady !== true && selectedWorkspaceId ? (
          <OperatorUnavailablePanel eyebrow="WORKSPACE RESPONSIBILITIES" title="Routines" icon="↻" message="Start the local Runtime to load or change Routine definitions. Run now can save a Task in READY state; planning and agent execution are not available yet." onOpenSettings={() => requestPageNavigation("Settings")} />
        ) : page === "Routines" ? (
          <RoutinesPage key={selectedWorkspaceId} api={routineApi} workspaceId={selectedWorkspaceId} resources={resources} resourcesNextCursor={resourceNextCursor} resourcesPageBusy={resourcePageBusy} onLoadMoreResources={loadMoreResources}
            automationTemplateFlow={automationTemplateFlow?.workspaceId === selectedWorkspaceId ? automationTemplateFlow : null}
            onAutomationTemplateRoutineSaved={(routineId, revision) => setAutomationTemplateFlow(current => current?.workspaceId === selectedWorkspaceId ? { ...current, routineId, routineRevision: revision } : current)}
            onContinueAutomationTemplate={() => requestPageNavigation("Automations")}
            onCancelAutomationTemplate={() => setAutomationTemplateFlow(null)}
            onOpenTask={(task) => {
            if (task.workspace_id !== selectedWorkspaceId || task.status !== "READY" || !task.task_id) return;
            setOpenedTask({ workspaceId: task.workspace_id, taskId: task.task_id });
            requestPageNavigation("Work");
          }} />
        ) : page === "Discover" && !selectedWorkspaceId ? (
          <div className="page-content subpage-content"><div className="eyebrow">CAPABILITIES</div><h1>Discover</h1><section className="subpage-panel empty-panel"><div className="empty-icon" aria-hidden="true">⌕</div><h2>Select a Workspace</h2><p>Choose a Workspace to view local skills and observed capability hosts.</p><button className="text-button" type="button" onClick={() => requestPageNavigation("Settings")}>Open Workspace settings <span aria-hidden="true">→</span></button></section></div>
        ) : page === "Discover" && runtime?.operatorReady !== true ? (
          <OperatorUnavailablePanel eyebrow="CAPABILITIES" title="Discover" icon="⌕" message="Start the local Runtime to read workspace-scoped capabilities. Discover never installs or authorizes a catalog entry." onOpenSettings={() => requestPageNavigation("Settings")} />
        ) : page === "Discover" && discoverApi ? (
          <DiscoverPage key={selectedWorkspaceId} api={discoverApi} />
        ) : page === "Automations" && runtime?.operatorReady !== true && selectedWorkspaceId ? (
          <OperatorUnavailablePanel eyebrow="WORKSPACE RESPONSIBILITIES" title="Automations" icon="◷" message="Start the local Runtime before loading Automation definitions. No Automation state is cached in this view." onOpenSettings={() => requestPageNavigation("Settings")} />
        ) : page === "Automations" ? (
          <AutomationsPage key={selectedWorkspaceId} api={automationApi} workspaceId={selectedWorkspaceId} resources={resources} resourcesNextCursor={resourceNextCursor} resourcesPageBusy={resourcePageBusy} onLoadMoreResources={loadMoreResources}
            templateFlow={automationTemplateFlow?.workspaceId === selectedWorkspaceId ? automationTemplateFlow : null}
            onStartTemplate={(starterId: AutomationStarterId) => {
              setAutomationTemplateFlow({ workspaceId: selectedWorkspaceId, starterId, routineId: null, routineRevision: null });
              requestPageNavigation("Routines");
            }}
            onResumeTemplateRoutine={() => requestPageNavigation("Routines")}
            onCompleteTemplateFlow={() => setAutomationTemplateFlow(null)}
            onOpenTask={(taskId) => {
            setOpenedTask({ workspaceId: selectedWorkspaceId, taskId });
            requestPageNavigation("Work");
          }} />
        ) : page === "Settings" ? (
          <SettingsView stopPreview={stopPreview} onPreviewStop={previewRuntimeStop} localPreferences={localPreferences} onChangeLocalPreferences={changeLocalPreferences} runtime={runtime} runtimeLifecycle={runtimeLifecycle?.identity === `${runtime?.runtimeId}:${runtime?.localIncarnationId}` ? runtimeLifecycle.view : null} runtimeLifecycleLoading={runtimeLifecycleLoading} runtimeLifecycleError={runtimeLifecycleError} runtimeError={runtimeError} runtimeBusy={runtimeBusy} startRuntime={startRuntime} refreshRuntime={refreshRuntime} agents={agentInstallations} agentError={agentInstallationError} refreshAgents={refreshAgentInstallations} agentProfiles={agentProfiles} agentProfileError={agentProfileError} runtimeInventoryStreamStatus={runtimeInventoryStreamStatus} agentBindings={agentBindings} agentBindingError={agentBindingError} agentActionMessage={agentActionFeedbackForWorkspace(agentActionFeedback, selectedWorkspaceId).message} agentActionError={agentActionFeedbackForWorkspace(agentActionFeedback, selectedWorkspaceId).error} agentActionBusy={agentActionFeedbackForWorkspace(agentActionFeedback, selectedWorkspaceId).busy} onProbeCodex={probeCodexProfile} onProbeOpenCode={probeOpenCodeProfile} onRefreshAgentProfiles={() => refreshAgentProfiles(selectedWorkspaceId)} onRefreshAgentBindings={async () => { await refreshAgentBindings(selectedWorkspaceId); }} onCreateAgentBinding={createAgentBinding} onEnableAgentBinding={enableAgentBinding} onDisableAgentBinding={disableAgentBinding} runtimeBindings={runtimeBindings} runtimeBindingError={runtimeBindingError} runtimeBindingsLoading={runtimeBindingsLoading} runtimeEnrollmentBusy={runtimeEnrollmentBusy} runtimeEnrollmentMessage={runtimeEnrollmentMessage} runtimeEnrollmentError={runtimeEnrollmentError} onEnrollLocalRuntime={enrollLocalRuntime} onRefreshRuntimeBindings={() => refreshRuntimeBindings(selectedWorkspaceId)} workspaces={workspaces} workspaceListState={workspaceListState} workspaceError={workspaceError} refreshWorkspaces={refreshWorkspaces} workspaceName={workspaceName} setWorkspaceName={setWorkspaceName} createWorkspace={createWorkspace} workspaceCreateBusy={workspaceCreateBusy} workspaceCreatePending={pendingWorkspaceRequest !== null} selectedWorkspaceId={selectedWorkspaceId} onSelectWorkspace={selectWorkspace} onUpdatePolicy={updateWorkspacePolicy} policyBusy={workspacePolicyBusy} policyRetryPending={workspacePolicyRequests.some(request => request.workspaceId === selectedWorkspaceId && request.expectedVersion === workspaces.find(item => item.workspaceId === selectedWorkspaceId)?.version && request.replicationPolicy === "LOCAL_ONLY")} onArchiveWorkspace={archiveWorkspace} archiveBusy={workspaceArchiveBusy} onRenameWorkspace={renameWorkspace} renameBusy={workspaceRenameBusy} renameError={workspaceRenameError} onClearRenameError={() => setWorkspaceRenameError(null)} onOpenNotifications={() => requestPageNavigation("Notifications")} onSetDefaultAgentBinding={setWorkspaceDefaultAgent} defaultAgentBusy={workspaceDefaultAgentBusy} instructions={instructionText} onInstructionsChange={changeWorkspaceInstructions} instructionHistory={workspaceInstructions} instructionMessage={instructionMessage} instructionBusy={instructionBusy} onSaveInstructions={saveWorkspaceInstructions} latestInstructionsAfterConflict={latestInstructionsAfterConflict} onUseLatestInstructions={useLatestWorkspaceInstructions} />
        ) : (
          <PlaceholderPage page={page} onSettings={() => requestPageNavigation("Settings")} />
        )}
          </div>
          {workbenchShown && !workbenchNarrow && !workbenchFullView && (
            <div
              className={`workbench-resizer ${isDraggingWorkbench ? "is-dragging" : ""}`}
              onMouseDown={(e) => {
                e.preventDefault();
                setIsDraggingWorkbench(true);
              }}
              onDoubleClick={() => {
                setWorkbenchWidth(480);
                try { window.localStorage.setItem("litecowork.workbenchWidth", "480"); } catch {}
              }}
              title="Drag to resize Workbench panel (Double-click to reset)"
              role="separator"
              aria-orientation="vertical"
            />
          )}
          {workbenchNarrow && workbenchOpen && (
            <button className="workbench-backdrop" type="button" tabIndex={-1} aria-label="Close Workbench" onClick={() => closeWorkbench()} />
          )}
          {workbenchShown && (
            <WorkbenchPanel
              key={selectedWorkspaceId}
              panelRef={workbenchPanelRef}
              modal={workbenchNarrow}
              workspaceId={selectedWorkspaceId}
              workspaceName={workspaces.find(workspace => workspace.workspaceId === selectedWorkspaceId)?.name ?? ""}
              runtimeState={runtime?.state ?? null}
              operatorReady={runtime?.operatorReady === true}
              resources={resources}
              resourceNextCursor={resourceNextCursor}
              resourcePageBusy={resourcePageBusy}
              resourceBusy={resourceBusy}
              resourceListFailed={resourceListFailed}
              previewResourceId={previewResourceId}
              previewResourceRevisionId={previewResourceRevisionId}
              previewResourceName={previewResourceName}
              preview={resourcePreview}
              previewError={previewError}
              isFullView={workbenchFullView}
              onToggleFullView={() => setWorkbenchFullView(prev => !prev)}
              onClose={() => {
                if (workbenchNarrow) {
                  closeWorkbench();
                } else {
                  setWorkbenchLayoutByWorkspace(current => ({ ...current, [workbenchLayoutKey]: false }));
                }
              }}
              onOpenLibrary={() => requestPageNavigation("Library")}
              onOpenWork={() => { setOpenedTask(null); requestPageNavigation("Work"); }}
              onOpenSettings={() => requestPageNavigation("Settings")}
              onLoadMoreResources={() => { void loadMoreResources(); }}
              onRefreshResources={() => setResourceCatalogRefreshNonce(current => current + 1)}
              onPreviewResource={resource => { void previewText(resource.resourceId, resource.displayName, resource.resourceRevisionId); }}
              onClosePreview={() => {
                previewGeneration.current += 1;
                setPreviewResourceId(null);
                setPreviewResourceRevisionId(null);
                setPreviewResourceName(null);
                setResourcePreview(null);
                setResourcePreviewHighlights(null);
                setPreviewError(null);
              }}
            />
          )}
        </div>
      </main>
      <LibraryNavigationDialog
        pending={pendingLibraryNavigation}
        onStay={stayInLibrary}
        onSave={saveLibraryDraftBeforeNavigation}
        onDiscard={discardLibraryDraftAndNavigate}
      />
    </div>
  );
}

function HomePage({ draft, onDraftChange, runtime, runtimeError, runtimeBusy, startRuntime, refreshRuntime, workspace, workspaceListState, workspaceError, refreshWorkspaces, workspaceName, onWorkspaceNameChange, workspaceCreateBusy, pendingWorkspaceRequest, createWorkspace, workspacesCount, coworkerApi, coworkerSelection, onCoworkerSelectionChange, taskInputs, onRemoveTaskInput, onOpenLibrary, operatorReady, taskSaving, taskSaveError, hasPendingTaskSave, pendingTaskRetryMatches, onOpenPendingTask, onClearPendingTaskAfterReview, onSaveTask, onSettings, onOpenCoworkers, onWorkspaceUpdated, onOpenWork, onOpenTask }: { draft: string; onDraftChange: (value: string) => void; runtime: RuntimeStatus | null; runtimeError: string | null; runtimeBusy: boolean; startRuntime: () => Promise<void>; refreshRuntime: () => Promise<void>; workspace: WorkspaceView | null; workspaceListState: WorkspaceListState; workspaceError: string | null; refreshWorkspaces: () => Promise<boolean>; workspaceName: string; onWorkspaceNameChange: (name: string) => void; workspaceCreateBusy: boolean; pendingWorkspaceRequest: PendingWorkspaceCreate | null; createWorkspace: () => Promise<void>; workspacesCount: number; coworkerApi: ReturnType<typeof desktopCoworkerApi>; coworkerSelection: HomeCoworkerSelection | null; onCoworkerSelectionChange: (selection: HomeCoworkerSelection) => void; taskInputs: TaskInputSelection[]; onRemoveTaskInput: (resourceId: string) => void; onOpenLibrary: () => void; operatorReady: boolean; taskSaving: boolean; taskSaveError: string | null; hasPendingTaskSave: boolean; pendingTaskRetryMatches: boolean; onOpenPendingTask: () => void; onClearPendingTaskAfterReview: () => void; onSaveTask: (selection: HomeCoworkerSelection) => Promise<void>; onSettings: () => void; onOpenCoworkers: () => void; onWorkspaceUpdated: (updated: WorkspacePrimaryReceipt) => void; onOpenWork: () => void; onOpenTask: (taskId: string) => void }) {
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

      {!workspace && <WorkspaceFirstRun
        runtime={runtime}
        runtimeError={runtimeError}
        runtimeBusy={runtimeBusy}
        onStartRuntime={startRuntime}
        onRefreshRuntime={refreshRuntime}
        workspaceListState={workspaceListState}
        workspaceError={workspaceError}
        onRefreshWorkspaces={refreshWorkspaces}
        workspaceName={workspaceName}
        onWorkspaceNameChange={onWorkspaceNameChange}
        workspaceCreateBusy={workspaceCreateBusy}
        workspaceCreatePending={pendingWorkspaceRequest}
        onCreateWorkspace={createWorkspace}
        onOpenSettings={onSettings}
        workspacesCount={workspacesCount}
      />}

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

      {workspace && <form className="composer-card" aria-label="Save a Task" onSubmit={(event) => { event.preventDefault(); if (currentSelection && selectionReady) void onSaveTask(currentSelection); }}>
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
      </form>}

      {workspace && <div className="setup-callout" role="status">
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
      </div>}

      {workspace && <div className="section-heading">
        <div><h2>Recent work</h2><p>Persisted Tasks in the selected Workspace.</p></div>
        <button className="quiet-button" type="button" onClick={onOpenWork}>View all</button>
      </div>}
      {workspace && <TaskListPanel workspaceId={workspaceId} operatorReady={operatorReady} status={null} limit={5} compact onOpenTask={onOpenTask} />}
    </div>
  );
}

function WorkPage({ selectedWorkspaceId, workspaceName, resourceNameCache, artifactApi, coworkerApi, operatorReady, initialTaskId, onOpenSourceConversation }: { selectedWorkspaceId: string; workspaceName: string; resourceNameCache: Record<string, string>; artifactApi: ReturnType<typeof desktopArtifactApi>; coworkerApi: ReturnType<typeof desktopCoworkerApi>; operatorReady: boolean; initialTaskId: string | null; onOpenSourceConversation: (conversationId: string) => void }) {
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
          <TaskDetailPanel key={`${selectedWorkspaceId}:${openTaskId}`} workspaceId={selectedWorkspaceId} taskId={openTaskId} resourceNameCache={resourceNameCache} artifactApi={artifactApi} coworkerApi={coworkerApi} operatorReady={operatorReady} onBack={() => setOpenTaskId(null)} onOpenSourceConversation={onOpenSourceConversation} />
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

function TaskDetailPanel({ workspaceId, taskId, resourceNameCache, artifactApi, coworkerApi, operatorReady, onBack, onOpenSourceConversation }: { workspaceId: string; taskId: string; resourceNameCache: Record<string, string>; artifactApi: ReturnType<typeof desktopArtifactApi>; coworkerApi: ReturnType<typeof desktopCoworkerApi>; operatorReady: boolean; onBack: () => void; onOpenSourceConversation: (conversationId: string) => void }) {
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
      if (receipt.taskId !== taskId || receipt.revision !== visibleTask.currentSpecRevision + 1 || receipt.objective !== objective || receipt.delegationPolicySnapshot === null) {
        throw new Error("Local Runtime returned a revision that does not match this edit.");
      }
      const latest = await invoke<TaskDetailView>("get_task", { workspaceId, taskId });
      if (latest.workspaceId !== workspaceId || latest.taskId !== taskId || latest.currentSpecRevision < receipt.revision
        || latest.currentSpecRevision === receipt.revision
          && JSON.stringify(latest.delegationPolicySnapshot) !== JSON.stringify(receipt.delegationPolicySnapshot)) {
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
          <div className="task-detail-heading"><span className="status-pill">{taskStatusLabel(visibleTask.status)}</span><small>Task {visibleTask.taskId}</small></div>
          {visibleTask.status === "COMPLETED" && <p className="task-verification-note">Completed means the Task reached a terminal successful state. Outputs are not verified in this view.</p>}
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
          {visibleTask.conversationId && <p className="task-detail-note task-conversation-origin">
            Created from a completed Conversation reply.
            <button type="button" className="text-button" onClick={() => onOpenSourceConversation(visibleTask.conversationId!)}>Open source Conversation</button>
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
          <details className="task-work-details task-delegation-policy-disclosure">
            <summary>
              <span>Delegation policy</span>
              <small>{visibleTask.delegationPolicySnapshot
                ? `Workspace settings revision ${visibleTask.delegationPolicySnapshot.workspace_settings_revision} · ${visibleTask.delegationPolicySnapshot.source === "WORKSPACE_DEFAULT" ? "Workspace defaults" : "Coworker override"}`
                : "Unavailable for this older Task"}</small>
            </summary>
            <TaskDelegationPolicySnapshot snapshot={visibleTask.delegationPolicySnapshot} />
          </details>
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

function profileHasFreshAdmissionOffer(profile: AgentProfileView): boolean {
  const latest = [...profile.observations].sort((left, right) => Date.parse(right.observedAt) - Date.parse(left.observedAt))[0];
  return !!latest
    && latest.compatible
    && Number.isFinite(Date.parse(latest.offerExpiresAt))
    && Date.parse(latest.offerExpiresAt) > Date.now()
    && (latest.readiness === "AVAILABLE" || latest.readiness === "STARTABLE" || latest.readiness === "READY");
}

function PlaceholderPage({ page, onSettings }: { page: Page; onSettings: () => void }) {
  const content: Record<Page, { title: string; description: string }> = {
    Home: { title: "Home", description: "Your workspace overview." },
    Conversations: { title: "Conversations", description: "Saved local conversation history." },
    Projects: { title: "Projects", description: "Browse the existing Workspaces on this device." },
    Work: { title: "Work", description: "Durable Tasks and their real progress will appear here after Task execution and the Operator API are integrated." },
    Library: { title: "Library", description: "Saved files and committed Artifacts for this Workspace." },
    Discover: { title: "Discover", description: "Browse workspace skills and observed capability hosts. Catalog entries are not installation, authentication, or authorization." },
    "Needs You": { title: "Needs You", description: "Review Workspace-scoped open items and their resolved source history." },
    Ideas: { title: "Ideas", description: "Read-only proposals from approved Suggestion producers. Viewing an idea never starts work." },
    Coworkers: { title: "Coworkers", description: "Workspace assistants and their saved settings." },
    Goals: { title: "Goals", description: "Workspace outcomes and their linked, evidence-based progress." },
    Notifications: { title: "Notifications", description: "Local delivery history and the policies that decide whether a settled outcome is announced." },
    Routines: { title: "Routines", description: "Reusable work definitions. Saving a Routine does not schedule or run it." },
    Automations: { title: "Automations", description: "Saved trigger definitions. Trigger hosting and Task creation are not available yet." },
    Dashboard: { title: "Dashboard", description: "Pinnable Workspace status widgets read from served Runtime projections." },
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

export default App;
