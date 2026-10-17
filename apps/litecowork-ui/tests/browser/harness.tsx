import { useRef, useState, type ComponentProps } from "react";
import ReactDOM from "react-dom/client";
import { emit } from "@tauri-apps/api/event";
import { mockIPC } from "@tauri-apps/api/mocks";
import App from "../../src/App";
import { ConversationsPage } from "../../src/conversations/ConversationsPage";
import { ResourceRevisionEditor } from "../../src/resources/ResourceRevisionEditor";
import { LibraryNavigationDialog, libraryNavigationMainIsInert, useLibraryNavigation } from "../../src/resources/library-navigation";
import { WorkspaceFirstRun } from "../../src/workspaces/WorkspaceFirstRun";
import { AgentCatalogSettings } from "../../src/agents/AgentCatalogSettings";
import "../../src/styles.css";

type WorkspaceFirstRunProps = ComponentProps<typeof WorkspaceFirstRun>;
type AgentCatalogSettingsProps = ComponentProps<typeof AgentCatalogSettings>;
type WorkspaceFirstRunScenario = "loading" | "error" | "offline" | "runtime-error" | "existing" | "ready" | "pending" | "busy";
type WorkspaceRuntimeStatus = Exclude<WorkspaceFirstRunProps["runtime"], null>;
type WorkspaceAppScenario = "workspace-list-race" | "workspace-list-failure" | "runtime-refresh-failure" | "workspace-policy-lost-ack" | "workspace-policy-lost-ack-confirmed" | "workspace-policy-list-race" | "first-use-conversation" | "workspace-root-scan-complete" | "workspace-root-scan-partial" | "workspace-root-scan-stale" | "workspace-root-scan-unsupported" | "workspace-root-scan-rejected" | "workspace-root-scan-ambiguous" | "resource-upload-ambiguous-create" | "resource-upload-ambiguous-chunk" | "resource-upload-ambiguous-chunk-unaccepted" | "resource-upload-ambiguous-chunk-bad-range" | "resource-upload-progress-read-failure" | "resource-upload-malformed-progress" | "resource-upload-mismatched-progress" | "resource-upload-progress-read-deferred" | "resource-upload-terminal-explicit-forget" | "resource-upload-commit-progress-read-failure" | "resource-upload-pause-before-session" | "resource-upload-pause-chunk-pending" | "resource-upload-pause-between-files" | "resource-upload-pause-before-commit" | "resource-upload-pause-commit-pending" | "resource-upload-pause-progress-pending" | "resource-upload-pause-create-pending";
type WorkspaceAppWorkspace = {
  workspaceId: string;
  name: string;
  replicationPolicy: string;
  defaultAgentBindingId: string | null;
  primaryCoworkerId: string | null;
  status: string;
  version: number;
};

type InvokeArgs = Record<string, unknown>;
type Call = { command: string; args: InvokeArgs };
type Deferred<T> = { promise: Promise<T>; resolve: (value: T) => void; reject: (reason: Error) => void };
type PendingConversationCreateRecord = { workspaceId: string; title: string | null; requestId: string };
type DeferredConversationAcknowledgement = { key: string; request: Deferred<void> };
type RenderedUploadSession = {
  uploadId: string;
  workspaceId: unknown;
  resourceId: unknown;
  displayName: string;
  mediaType: unknown;
  expectedSizeBytes: unknown;
  expectedDigest: unknown;
  expectedResourceVersion: unknown;
  parentRevisionIds: unknown;
  chunkSizeBytes: number;
  receivedRanges: { startOffset: number; endOffsetInclusive: number; sha256: string }[];
  nextMissingOffset: number;
  state: "OPEN" | "CONTENT_RECEIVED" | "COMMITTED";
  committedResourceId: string | null;
  expiresAt: string;
};
type RenderedResourceUploadSession = {
  uploadId: string;
  workspaceId: string;
  displayName: string;
  mediaType: string;
  expectedSizeBytes: number;
  expectedDigest: string;
  contextDocument: null;
  folderRelativePath: string | null;
  committedResourceId: string | null;
  chunkSizeBytes: number;
  receivedRanges: { startOffset: number; endOffsetInclusive: number; sha256: string }[];
  nextMissingOffset: number;
  state: "OPEN" | "CONTENT_RECEIVED" | "COMMITTED" | "FAILED" | "EXPIRED";
  expiresAt: string;
};
type RenderedCommittedResource = {
  resourceId: string;
  workspaceId: string;
  resourceRevisionId: string;
  displayName: string;
  mediaType: string;
  contentDigest: string;
  sizeBytes: number;
};
type DeferredResourceFileDigest = {
  request: Deferred<ArrayBuffer>;
  read: () => Promise<ArrayBuffer>;
};
type DeferredResourceUploadChunk = {
  request: Deferred<null>;
  chunkIndex: number;
};
type DeferredResourceUploadCommit = {
  request: Deferred<RenderedCommittedResource>;
  resource: RenderedCommittedResource;
};
type DeferredResourceUploadCreate = {
  request: Deferred<RenderedResourceUploadSession>;
  session: RenderedResourceUploadSession;
};

const semanticText = "Semantic answer remains available.";
const semanticDigest = `sha256:${"a".repeat(64)}`;
const now = "2026-10-13T12:00:00.000Z";
const bytesToDigest = async (bytes: Uint8Array): Promise<string> => {
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return `sha256:${Array.from(new Uint8Array(digest), byte => byte.toString(16).padStart(2, "0")).join("")}`;
};

function canonicalJson(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  if (value !== null && typeof value === "object") {
    const record = value as Record<string, unknown>;
    return `{${Object.keys(record).sort().map(key => `${JSON.stringify(key)}:${canonicalJson(record[key])}`).join(",")}}`;
  }
  return JSON.stringify(value);
}

const richDocument = async (messageId = "message-a") => {
  const semanticBytes = new TextEncoder().encode(semanticText);
  const document = {
    schema_version: 1,
    renderer_contract_version: 1,
    presentation_id: "presentation-a",
    message_id: messageId,
    semantic_content_digest: semanticDigest,
    root_blocks: [{
      kind: "TEXT_SLICE",
      source: {
        start_utf8_byte: 0,
        end_utf8_byte_exclusive: semanticBytes.byteLength,
        slice_digest: await bytesToDigest(semanticBytes),
      },
      width: "READABLE",
    }],
    citations: [],
    actions: [],
    block_provenance: [{
      block_path: "/0",
      origin: "SEMANTIC_MESSAGE",
      resource_refs: [],
      artifact_refs: [],
      evidence_refs: [],
      verification_refs: [],
    }],
  };
  const canonical = new TextEncoder().encode(canonicalJson(document));
  const documentDigest = await bytesToDigest(canonical);
  return {
    reference: {
      presentation_id: "presentation-a",
      message_id: messageId,
      schema_version: 1,
      renderer_contract_version: 1,
      semantic_content_digest: semanticDigest,
      document_digest: documentDigest,
      document_size_bytes: canonical.byteLength,
      availability: "AVAILABLE" as const,
    },
    response: {
      workspace_id: "workspace-a",
      conversation_id: "conversation-a",
      message_id: messageId,
      presentation_id: "presentation-a",
      schema_version: 1,
      renderer_contract_version: 1,
      semantic_content_digest: semanticDigest,
      document_digest: documentDigest,
      document_size_bytes: canonical.byteLength,
      document,
    },
  };
};

const resourceA = {
  resourceId: "resource-a",
  workspaceId: "workspace-a",
  resourceRevisionId: "revision-a1",
  displayName: "A.txt",
  mediaType: "text/plain",
  contentDigest: `sha256:${"1".repeat(64)}`,
  sizeBytes: 19,
};
function contextDocumentDeletionStatus(
  status: "ACTIVE" | "DELETION_PENDING" | "DELETED",
  acknowledgedReplicaCount = 0,
) {
  return {
    resourceId: "resource-a",
    status,
    manifestDigest: status === "ACTIVE" ? null : `sha256:${"a".repeat(64)}`,
    requiredReplicaCount: status === "ACTIVE" ? 0 : 2,
    acknowledgedReplicaCount,
    updatedAt: now,
  };
}
const resourceB = {
  resourceId: "resource-b",
  workspaceId: "workspace-a",
  resourceRevisionId: "revision-b1",
  displayName: "B.txt",
  mediaType: "text/plain",
  contentDigest: `sha256:${"2".repeat(64)}`,
  sizeBytes: 19,
};
const resourceDetails: Record<string, Record<string, unknown>> = {
  "resource-a": {
    resourceId: "resource-a", workspaceId: "workspace-a", kind: "TEXT", displayName: "A.txt",
    currentRevisionId: "revision-a1", version: 1,
    contextDocument: { kind: "WORKSPACE_NOTES", status: "ACTIVE", owner_ref: { kind: "WORKSPACE", workspace_id: "workspace-a" } },
  },
  "resource-b": {
    resourceId: "resource-b", workspaceId: "workspace-a", kind: "TEXT", displayName: "B.txt",
    currentRevisionId: "revision-b1", version: 1,
    contextDocument: { kind: "WORKSPACE_NOTES", status: "ACTIVE", owner_ref: { kind: "WORKSPACE", workspace_id: "workspace-a" } },
  },
};
const resourceRevisionPages: Record<string, Record<string, unknown>> = {
  "resource-a": {
    items: [{
      resourceRevisionId: "revision-a1", resourceId: "resource-a", parentRevisionIds: [],
      contentDigest: `sha256:${"1".repeat(64)}`, sizeBytes: 19, mediaType: "text/plain",
      observedAt: "2026-10-14T00:00:00Z", isHead: true,
    }],
    nextCursor: null,
  },
  "resource-b": {
    items: [{
      resourceRevisionId: "revision-b1", resourceId: "resource-b", parentRevisionIds: [],
      contentDigest: `sha256:${"2".repeat(64)}`, sizeBytes: 19, mediaType: "text/plain",
      observedAt: "2026-10-14T00:00:00Z", isHead: true,
    }],
    nextCursor: null,
  },
};

type ResourceReadCommand = "detail" | "history";

function resourceReadKey(command: ResourceReadCommand, workspaceId: string, resourceId: string): string {
  return JSON.stringify([command, workspaceId, resourceId]);
}

function resourcePreviewKey(workspaceId: string, resourceId: string): string {
  return JSON.stringify([workspaceId, resourceId]);
}

function conversationSummary(workspaceId: string, conversationId: string, title: string) {
  return {
    conversation_id: conversationId,
    workspace_id: workspaceId,
    title,
    active_agent_binding_id: null,
    version: 1,
    created_at: now,
  };
}

const conversationMessage = {
  message_id: "message-a",
  conversation_id: "conversation-a",
  role: "AGENT",
  created_at: now,
  content: [{ kind: "TEXT", text: semanticText }],
};
const conversationBMessage = {
  message_id: "message-b",
  conversation_id: "conversation-b",
  role: "AGENT",
  created_at: now,
  content: [{ kind: "TEXT", text: "Workspace B answer." }],
};

function conversationSnapshot(workspaceId: string, availability: string, rich: Awaited<ReturnType<typeof richDocument>>["reference"], conversationId?: string) {
  const isA = workspaceId === "workspace-a";
  const resolvedConversationId = conversationId ?? (isA ? "conversation-a" : "conversation-b");
  const knownConversation = resolvedConversationId === "conversation-a" || resolvedConversationId === "conversation-b";
  return {
    workspace_id: workspaceId,
    conversation_id: resolvedConversationId,
    projection_revision: 1,
    stream_cursor: `conversation-history-${resolvedConversationId}-1`,
    origin_sequence: "1",
    items: knownConversation ? [{
      message: isA ? conversationMessage : conversationBMessage,
      rich_presentation: isA ? { ...rich, availability } : null,
      linked_items: [],
    }] : [],
    next_cursor: null,
    active_turn: null,
  };
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (reason: Error) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

const rich = await richDocument();
const conversationCommands = new Set([
  "list_conversations",
  "get_conversation_presentation",
  "get_rich_presentation",
  "list_pending_conversation_creates",
  "create_conversation",
  "acknowledge_conversation_create",
  "subscribe_conversation_stream",
  "acknowledge_conversation_stream_frame",
  "unsubscribe_conversation_stream",
]);
const state = {
  calls: [] as Call[],
  activeWorkspace: "workspace-a",
  richAvailability: "AVAILABLE",
  richMode: "success" as "success" | "defer" | "reject",
  richPending: [] as Deferred<unknown>[],
  createMode: "success" as "success" | "defer" | "lose-response",
  createPending: [] as Deferred<unknown>[],
  acknowledgeMode: "success" as "success" | "defer" | "reject",
  acknowledgePending: [] as DeferredConversationAcknowledgement[],
  pendingConversationCreates: new Map<string, PendingConversationCreateRecord>(),
  createReceipts: new Map<string, ReturnType<typeof conversationSummary>>(),
  createCommits: 0,
  conversationListMode: "single" as "single" | "51" | "empty" | "defer" | "reject",
  conversationListPending: [] as Deferred<unknown>[],
  emptyConversationSnapshot: false,
  activeStream: null as { sessionId: string; workspaceId: string; conversationId: string } | null,
  resourceReadDeferral: null as ResourceReadCommand | null,
  resourcePending: new Map<string, Deferred<unknown>[]>(),
  resourcePreviewDeferred: new Set<string>(),
  resourcePreviewPending: new Map<string, Deferred<string>[]>(),
  resourceCommitMode: "success" as "success" | "defer" | "reject" | "conflict",
  resourceCommitPending: [] as Deferred<unknown>[],
  contextDocumentDeletionBeginCalls: 0,
  contextDocumentDeletionStatusReads: 0,
  contextDocumentDeletionFirstStatus: "ACTIVE" as "ACTIVE" | "DELETION_PENDING",
  contextDocumentDeletionDetailReadFailures: 0,
  contextDocumentDeletionStaleDetailStatusReads: 0,
  resourceUploadSession: null as RenderedUploadSession | null,
  workspaceActions: { startRuntime: 0, refreshRuntime: 0, refreshWorkspaces: 0, createWorkspace: 0 },
  appScenario: null as WorkspaceAppScenario | null,
  appRuntimeStatusCalls: 0,
  appWorkspaceListCalls: 0,
  appInitialWorkspaceList: null as Deferred<WorkspaceAppWorkspace[]> | null,
  appStaleWorkspaceList: null as Deferred<WorkspaceAppWorkspace[]> | null,
  appCreatedWorkspace: null as WorkspaceAppWorkspace | null,
  appWorkspaceCreateReceipt: null as Deferred<WorkspaceAppWorkspace> | null,
  appWorkspaceCreateCalls: 0,
  appWorkspacePolicyRequests: [] as InvokeArgs[],
  appWorkspacePolicyCommitted: null as WorkspaceAppWorkspace | null,
  appWorkspacePolicyListRefreshFails: false,
  workspaceRootScanPending: [] as Deferred<unknown>[],
  f90UploadSession: null as RenderedResourceUploadSession | null,
  f90UploadCreates: 0,
  f90UploadCreateReceipts: new Map<string, { payload: string; session: RenderedResourceUploadSession }>(),
  f90DeferNextCreateResponse: false,
  f90CreatePending: [] as DeferredResourceUploadCreate[],
  f90LoseNextCreateAck: false,
  f90DeferFileDigestName: null as string | null,
  f90FileDigestPending: [] as DeferredResourceFileDigest[],
  f90DeferredChunkIndexes: new Set<number>(),
  f90ChunkPending: [] as DeferredResourceUploadChunk[],
  f90ChunkRequestReceipts: new Map<string, string>(),
  f90LoseNextChunkAckBeforeAccept: false,
  f90CommitReceipts: new Map<string, { payload: string; resource: RenderedCommittedResource }>(),
  f90DeferNextCommitResponse: false,
  f90CommitPending: [] as DeferredResourceUploadCommit[],
  f90OmitCommittedResourceFromList: false,
  f90LoseNextChunkAck: false,
  f90LoseNextCommitAck: false,
  f90FailNextProgressRead: false,
  f90MalformedNextProgressRead: false,
  f90MismatchNextProgressWorkspace: false,
  f90DeferNextProgressRead: false,
  f90ProgressReadPending: [] as Deferred<RenderedResourceUploadSession>[],
  f90CorruptNextRangeRead: false,
  f90CommittedResource: null as RenderedCommittedResource | null,
  frameId: 0,
};

const workspaceRootScanFixture = {
  workspaceId: "workspace-a",
  workspaceRootId: "workspace-root-a",
  resourceId: "resource-folder-a",
  displayName: "Project folder",
  watchPolicy: "MANUAL",
  replicationPolicy: "LOCAL_ONLY",
  status: "ACTIVE",
  locationAvailability: "AVAILABLE",
  version: 7,
};

function isWorkspaceRootScanScenario(scenario: WorkspaceAppScenario | null): boolean {
  return scenario === "workspace-root-scan-complete"
    || scenario === "workspace-root-scan-partial"
    || scenario === "workspace-root-scan-stale"
    || scenario === "workspace-root-scan-unsupported"
    || scenario === "workspace-root-scan-rejected"
    || scenario === "workspace-root-scan-ambiguous";
}

function workspaceFirstRunFixtureProps(scenario: WorkspaceFirstRunScenario, onWorkspaceNameChange: (name: string) => void): WorkspaceFirstRunProps {
  const stoppedRuntime: WorkspaceRuntimeStatus = {
    state: "STOPPED",
    blockers: [],
    processRunning: false,
    operatorReady: false,
    daemonAvailable: true,
  };
  const readyRuntime: WorkspaceRuntimeStatus = {
    state: "READY",
    blockers: [],
    processRunning: true,
    operatorReady: true,
    daemonAvailable: true,
  };
  const pending = scenario === "pending";
  const busy = scenario === "busy";
  return {
    runtime: scenario === "offline" ? stoppedRuntime : scenario === "existing" || scenario === "ready" || pending || busy ? readyRuntime : null,
    runtimeError: scenario === "offline"
      ? "The local Runtime is not responding."
      : scenario === "runtime-error" ? "Desktop Runtime status is unavailable." : null,
    runtimeBusy: false,
    onStartRuntime: async () => { state.workspaceActions.startRuntime += 1; },
    onRefreshRuntime: async () => { state.workspaceActions.refreshRuntime += 1; },
    workspaceListState: scenario === "loading" ? "LOADING" : scenario === "error" ? "ERROR" : "READY",
    workspacesCount: scenario === "existing" ? 1 : 0,
    workspaceError: scenario === "error"
      ? "The Workspace list could not be loaded."
      : pending ? "The previous Workspace request may have completed. Retry to check its receipt." : null,
    onRefreshWorkspaces: async () => { state.workspaceActions.refreshWorkspaces += 1; return true; },
    workspaceName: pending ? "Local Workspace" : busy ? "Starting Workspace" : "",
    onWorkspaceNameChange,
    workspaceCreateBusy: busy,
    workspaceCreatePending: pending ? { name: "Local Workspace", requestId: "request-1" } : null,
    onCreateWorkspace: async () => { state.workspaceActions.createWorkspace += 1; },
    onOpenSettings: () => {},
  };
}

function record(command: string, args: unknown): InvokeArgs {
  const value = args && typeof args === "object" ? args as InvokeArgs : {};
  state.calls.push({ command, args: structuredClone(value) });
  return value;
}

function commitConversationCreate(args: InvokeArgs) {
  const workspaceId = String(args.workspaceId);
  const requestId = String(args.requestId);
  const key = JSON.stringify([workspaceId, requestId]);
  const existing = state.createReceipts.get(key);
  if (existing) return existing;
  const committed = conversationSummary(workspaceId, `conversation-created-${state.createCommits + 1}`, String(args.title ?? "New conversation"));
  state.createReceipts.set(key, committed);
  state.createCommits += 1;
  return committed;
}

function conversationCreateJournalKey(workspaceId: string, requestId: string): string {
  return JSON.stringify([workspaceId, requestId]);
}

function recordPendingConversationCreate(args: InvokeArgs): string {
  const workspaceId = String(args.workspaceId);
  const requestId = String(args.requestId);
  const title = args.title === null || args.title === undefined ? null : String(args.title);
  const key = conversationCreateJournalKey(workspaceId, requestId);
  const existing = state.pendingConversationCreates.get(key);
  if (existing && existing.title !== title) throw new Error("The pending Conversation request key was reused with a different title.");
  state.pendingConversationCreates.set(key, { workspaceId, title, requestId });
  return key;
}

function acknowledgeConversationCreate(args: InvokeArgs): void {
  state.pendingConversationCreates.delete(conversationCreateJournalKey(String(args.workspaceId), String(args.requestId)));
}

function appRuntimeStatus() {
  return {
    state: "READY",
    runtimeId: "runtime-local-test",
    localIncarnationId: "incarnation-local-test",
    blockers: [],
    lastShutdownClean: true,
    processRunning: true,
    operatorReady: true,
    daemonAvailable: true,
    detail: null,
  };
}

function appRuntimeLifecycle() {
  return {
    runtimeId: "runtime-local-test",
    startupPolicy: "MANUAL",
    incarnation: {
      runtimeIncarnationId: "incarnation-local-test",
      runtimeId: "runtime-local-test",
      processStartedAt: "2026-10-14T12:00:00.000Z",
      litecoworkVersion: "test",
      recoveredFromUncleanShutdown: false,
      recoveryState: "READY",
      readyAt: "2026-10-14T12:00:01.000Z",
      stoppedAt: null,
      version: 1,
    },
    availability: "ONLINE",
    blockers: [],
  };
}

function appWorkspaceListResponse(): WorkspaceAppWorkspace[] {
  const workspace = state.appWorkspacePolicyCommitted ?? state.appCreatedWorkspace;
  if (!workspace) return [];
  if (state.appScenario === "resource-upload-progress-read-deferred") {
    return [workspace, {
      ...workspace,
      workspaceId: "workspace-b",
      name: "Workspace B",
    }];
  }
  return [workspace];
}

function isF90UploadScenario(scenario: WorkspaceAppScenario | null): boolean {
  return scenario?.startsWith("resource-upload-") === true;
}

function createRenderedResourceUpload(args: InvokeArgs): RenderedResourceUploadSession {
  const session: RenderedResourceUploadSession = {
    uploadId: `upload-f90-${state.f90UploadCreates}`,
    workspaceId: String(args.workspaceId),
    displayName: String(args.displayName),
    mediaType: String(args.mediaType),
    expectedSizeBytes: Number(args.sizeBytes),
    expectedDigest: String(args.expectedDigest),
    contextDocument: null,
    folderRelativePath: typeof args.folderRelativePath === "string" ? args.folderRelativePath : null,
    committedResourceId: null,
    chunkSizeBytes: 4 * 1024 * 1024,
    receivedRanges: [],
    nextMissingOffset: 0,
    state: "OPEN",
    expiresAt: new Date(Date.now() + 60 * 60 * 1000).toISOString(),
  };
  state.f90UploadSession = session;
  return session;
}

async function flushRenderedTurn(): Promise<void> {
  await new Promise<void>(resolve => window.setTimeout(resolve, 0));
  await new Promise<void>(resolve => window.requestAnimationFrame(() => resolve()));
}

function resourceUploadReceiptKey(args: InvokeArgs): string {
  return JSON.stringify([args.workspaceId, args.requestId]);
}

function handleWorkspaceAppInvoke(command: string, args: InvokeArgs): unknown {
  if (command === "get_runtime_status") {
    state.appRuntimeStatusCalls += 1;
    if (state.appScenario === "runtime-refresh-failure" && state.appRuntimeStatusCalls > 1) {
      return Promise.reject(new Error("Rendered harness Runtime status probe failed."));
    }
    return appRuntimeStatus();
  }
  if (command === "get_runtime_lifecycle") return appRuntimeLifecycle();
  if (command === "list_workspaces") {
    state.appWorkspaceListCalls += 1;
    if (state.appScenario === "workspace-policy-lost-ack" && state.appWorkspacePolicyListRefreshFails) {
      state.appWorkspacePolicyListRefreshFails = false;
      return Promise.reject(new Error("Rendered harness Workspace refresh failed after ambiguous policy response."));
    }
    if ((state.appScenario === "workspace-list-race" || state.appScenario === "workspace-list-failure") && state.appWorkspaceListCalls === 1) {
      if (!state.appInitialWorkspaceList) throw new Error("Initial Workspace list response was not configured.");
      return state.appInitialWorkspaceList.promise;
    }
    if (state.appWorkspaceListCalls === 2 && state.appStaleWorkspaceList) return state.appStaleWorkspaceList.promise;
    return appWorkspaceListResponse();
  }
  if (command === "create_workspace") {
    state.appWorkspaceCreateCalls += 1;
    const created: WorkspaceAppWorkspace = {
      workspaceId: "workspace-created-in-app-test",
      name: String(args.name),
      replicationPolicy: "LOCAL_ONLY",
      defaultAgentBindingId: null,
      primaryCoworkerId: null,
      status: "ACTIVE",
      version: 1,
    };
    state.appCreatedWorkspace = created;
    return state.appWorkspaceCreateReceipt?.promise ?? created;
  }
  if (command === "update_workspace_policy") {
    state.appWorkspacePolicyRequests.push(structuredClone(args));
    if (![
      "workspace-policy-lost-ack",
      "workspace-policy-lost-ack-confirmed",
      "workspace-policy-list-race",
    ].includes(state.appScenario ?? "")) {
      return Promise.reject(new Error("Unexpected Workspace policy update in this rendered scenario."));
    }
    if (state.appWorkspacePolicyRequests.length === 1) {
      const current = state.appCreatedWorkspace;
      if (!current) throw new Error("The policy retry fixture has no Workspace.");
      state.appWorkspacePolicyCommitted = {
        ...current,
        replicationPolicy: String(args.replicationPolicy),
        version: Number(args.expectedVersion) + 1,
      };
      state.appWorkspacePolicyListRefreshFails = state.appScenario === "workspace-policy-lost-ack";
      if (state.appScenario === "workspace-policy-list-race") {
        return structuredClone(state.appWorkspacePolicyCommitted);
      }
      return Promise.reject(new Error("Rendered harness lost the committed Workspace policy response."));
    }
    const first = state.appWorkspacePolicyRequests[0];
    if (canonicalJson(first) !== canonicalJson(args)) {
      return Promise.reject(new Error("Rendered harness received a non-identical Workspace policy retry."));
    }
    if (!state.appWorkspacePolicyCommitted) throw new Error("No committed Workspace policy receipt is available.");
    return structuredClone(state.appWorkspacePolicyCommitted);
  }
  if (isF90UploadScenario(state.appScenario) && command === "create_resource_upload") {
    const receiptKey = resourceUploadReceiptKey(args);
    const payload = canonicalJson(args);
    const priorReceipt = state.f90UploadCreateReceipts.get(receiptKey);
    if (priorReceipt) {
      if (priorReceipt.payload !== payload) return Promise.reject(new Error("Rendered harness rejected a changed payload for a reused upload create RequestId."));
      return structuredClone(priorReceipt.session);
    }
    state.f90UploadCreates += 1;
    const session = createRenderedResourceUpload(args);
    state.f90UploadCreateReceipts.set(receiptKey, { payload, session: structuredClone(session) });
    if (state.f90LoseNextCreateAck) {
      state.f90LoseNextCreateAck = false;
      return Promise.reject(new Error("Rendered harness lost the create acknowledgement after session creation."));
    }
    if (state.f90DeferNextCreateResponse) {
      state.f90DeferNextCreateResponse = false;
      const request = deferred<RenderedResourceUploadSession>();
      state.f90CreatePending.push({ request, session: structuredClone(session) });
      return request.promise.then(view => structuredClone(view));
    }
    return structuredClone(session);
  }
  if (isF90UploadScenario(state.appScenario) && command === "get_resource_upload") {
    if (state.f90FailNextProgressRead) {
      state.f90FailNextProgressRead = false;
      return Promise.reject(new Error("Rendered harness could not read upload progress."));
    }
    if (state.f90MalformedNextProgressRead) {
      state.f90MalformedNextProgressRead = false;
      return null;
    }
    if (state.f90DeferNextProgressRead) {
      state.f90DeferNextProgressRead = false;
      const request = deferred<RenderedResourceUploadSession>();
      state.f90ProgressReadPending.push(request);
      return request.promise.then(view => structuredClone(view));
    }
    if (!state.f90UploadSession || state.f90UploadSession.uploadId !== args.uploadId) {
      return Promise.reject(new Error("Rendered harness has no matching Resource upload session."));
    }
    const view = structuredClone(state.f90UploadSession);
    if (state.f90MismatchNextProgressWorkspace) {
      state.f90MismatchNextProgressWorkspace = false;
      view.workspaceId = "workspace-other";
    }
    if (state.f90CorruptNextRangeRead && view.receivedRanges.length > 0) {
      state.f90CorruptNextRangeRead = false;
      view.receivedRanges[0].sha256 = `sha256:${"0".repeat(64)}`;
    }
    return view;
  }
  if (isF90UploadScenario(state.appScenario) && command === "upload_resource_chunk") {
    const session = state.f90UploadSession;
    if (!session || session.uploadId !== args.uploadId) return Promise.reject(new Error("Rendered harness has no matching Resource upload session."));
    const match = /^bytes (\d+)-(\d+)\/(\d+)$/.exec(String(args.contentRange));
    if (!match || Number(match[3]) !== session.expectedSizeBytes) {
      return Promise.reject(new Error("Rendered harness received an invalid chunk range."));
    }
    const chunkIndex = Number(args.chunkIndex);
    const receiptKey = resourceUploadReceiptKey(args);
    const payload = canonicalJson({
      workspaceId: args.workspaceId,
      uploadId: args.uploadId,
      chunkIndex,
      contentRange: args.contentRange,
      chunkSha256: args.chunkSha256,
    });
    const priorReceipt = state.f90ChunkRequestReceipts.get(receiptKey);
    if (priorReceipt !== undefined) {
      if (priorReceipt !== payload) return Promise.reject(new Error("Rendered harness rejected a changed payload for a reused chunk RequestId."));
      return null;
    }
    if (state.f90LoseNextChunkAckBeforeAccept) {
      state.f90LoseNextChunkAckBeforeAccept = false;
      return Promise.reject(new Error("Rendered harness lost the chunk response before accepting the chunk."));
    }
    const startOffset = Number(match[1]);
    const endOffsetInclusive = Number(match[2]);
    const acceptedRange = {
      startOffset,
      endOffsetInclusive,
      sha256: `sha256:${String(args.chunkSha256)}`,
    };
    const existing = session.receivedRanges.find(range => range.startOffset === startOffset);
    if (existing) {
      if (existing.endOffsetInclusive !== endOffsetInclusive || existing.sha256 !== acceptedRange.sha256) {
        return Promise.reject(new Error("Rendered harness rejected a changed chunk replay."));
      }
    } else {
      session.receivedRanges.push(acceptedRange);
      session.receivedRanges.sort((left, right) => left.startOffset - right.startOffset);
    }
    session.nextMissingOffset = 0;
    for (const range of session.receivedRanges) {
      if (range.startOffset !== session.nextMissingOffset) break;
      session.nextMissingOffset = range.endOffsetInclusive + 1;
    }
    if (session.nextMissingOffset === session.expectedSizeBytes) session.state = "CONTENT_RECEIVED";
    state.f90ChunkRequestReceipts.set(receiptKey, payload);
    if (state.f90LoseNextChunkAck) {
      state.f90LoseNextChunkAck = false;
      return Promise.reject(new Error("Rendered harness lost the chunk acknowledgement."));
    }
    if (state.f90DeferredChunkIndexes.delete(chunkIndex)) {
      const request = deferred<null>();
      state.f90ChunkPending.push({ request, chunkIndex });
      return request.promise;
    }
    return null;
  }
  if (isF90UploadScenario(state.appScenario) && command === "commit_resource_upload") {
    const session = state.f90UploadSession;
    if (!session || session.uploadId !== args.uploadId) {
      return Promise.reject(new Error("Rendered harness refused to commit before every chunk was accepted."));
    }
    const receiptKey = resourceUploadReceiptKey(args);
    const payload = canonicalJson({ workspaceId: args.workspaceId, uploadId: args.uploadId });
    const priorReceipt = state.f90CommitReceipts.get(receiptKey);
    if (priorReceipt) {
      if (priorReceipt.payload !== payload) return Promise.reject(new Error("Rendered harness rejected a changed payload for a reused commit RequestId."));
      return structuredClone(priorReceipt.resource);
    }
    if (session.state === "COMMITTED" && state.f90CommittedResource) return structuredClone(state.f90CommittedResource);
    if (session.state !== "CONTENT_RECEIVED") return Promise.reject(new Error("Rendered harness refused to commit before every chunk was accepted."));
    session.state = "COMMITTED";
    session.committedResourceId = "resource-f90";
    state.f90CommittedResource = {
      resourceId: "resource-f90",
      workspaceId: session.workspaceId,
      resourceRevisionId: "resource-f90-revision-1",
      displayName: session.displayName,
      mediaType: session.mediaType,
      contentDigest: session.expectedDigest,
      sizeBytes: session.expectedSizeBytes,
    };
    state.f90CommitReceipts.set(receiptKey, { payload, resource: structuredClone(state.f90CommittedResource) });
    if (state.f90LoseNextCommitAck) {
      state.f90LoseNextCommitAck = false;
      return Promise.reject(new Error("Rendered harness lost the commit acknowledgement after commit."));
    }
    if (state.f90DeferNextCommitResponse) {
      state.f90DeferNextCommitResponse = false;
      const request = deferred<RenderedCommittedResource>();
      state.f90CommitPending.push({ request, resource: structuredClone(state.f90CommittedResource) });
      return request.promise.then(resource => structuredClone(resource));
    }
    return structuredClone(state.f90CommittedResource);
  }
  if (command === "list_agent_installations") return [];
  if (command === "list_agent_profiles" || command === "list_agent_bindings") return { items: [], nextCursor: null };
  if (command === "list_workspace_runtime_bindings" || command === "list_workspace_instructions") return [];
  if (command === "list_resources") return {
    items: !state.f90OmitCommittedResourceFromList && state.f90CommittedResource?.workspaceId === args.workspaceId ? [state.f90CommittedResource] : [],
    nextCursor: null,
  };
  if (command === "list_workspace_roots") return {
    items: isWorkspaceRootScanScenario(state.appScenario) && args.workspaceId === workspaceRootScanFixture.workspaceId
      ? [structuredClone(workspaceRootScanFixture)]
      : [],
    nextCursor: null,
  };
  if (command === "scan_workspace_root") {
    if (!isWorkspaceRootScanScenario(state.appScenario)) {
      return Promise.reject(new Error("Unexpected folder scan in this rendered scenario."));
    }
    const request = deferred<unknown>();
    state.workspaceRootScanPending.push(request);
    return request.promise;
  }
  if (command === "list_delegation_profiles") {
    return {
      status: 200,
      contentType: "application/json",
      bodyBase64: btoa(JSON.stringify({ items: [], next_cursor: null })),
    };
  }
  if (command === "artifact_read") {
    return {
      status: 200,
      contentType: "application/json",
      bodyBase64: btoa(JSON.stringify({ items: [], next_cursor: null })),
    };
  }
  throw new Error(`Production App rendered test received an unexpected Tauri command: ${command}`);
}

mockIPC((rawCommand, rawArgs) => {
  const args = record(rawCommand, rawArgs);
  const command = rawCommand;
  if (state.appScenario !== null && !conversationCommands.has(command)) return handleWorkspaceAppInvoke(command, args);
  if (command === "get_resource_detail") {
    const workspaceId = String(args.workspaceId);
    const resourceId = String(args.resourceId);
    if (state.contextDocumentDeletionDetailReadFailures > 0) {
      state.contextDocumentDeletionDetailReadFailures -= 1;
      return Promise.reject(new Error("Rendered Resource detail refresh failed after deletion status confirmation."));
    }
    if (state.resourceReadDeferral !== "detail") {
      const detail = structuredClone(resourceDetails[resourceId]);
      const contextDocument = detail.contextDocument as Record<string, unknown> | undefined;
      if (resourceId === "resource-a" && contextDocument?.status === "DELETED"
        && state.contextDocumentDeletionStaleDetailStatusReads > 0) {
        state.contextDocumentDeletionStaleDetailStatusReads -= 1;
        detail.contextDocument = { ...contextDocument, status: "DELETION_PENDING" };
      }
      return detail;
    }
    const request = deferred<unknown>();
    const key = resourceReadKey("detail", workspaceId, resourceId);
    state.resourcePending.set(key, [...(state.resourcePending.get(key) ?? []), request]);
    return request.promise;
  }
  if (command === "begin_context_document_deletion") {
    state.contextDocumentDeletionBeginCalls += 1;
    if (state.contextDocumentDeletionBeginCalls === 1) {
      return Promise.reject(new Error("Rendered harness lost the deletion acknowledgement."));
    }
    resourceDetails["resource-a"] = {
      ...resourceDetails["resource-a"],
      version: 2,
      contextDocument: {
        ...(resourceDetails["resource-a"].contextDocument as Record<string, unknown>),
        status: "DELETION_PENDING",
      },
    };
    return contextDocumentDeletionStatus("DELETION_PENDING");
  }
  if (command === "get_context_document_deletion_status") {
    const read = state.contextDocumentDeletionStatusReads++;
    if (read === 0 && state.contextDocumentDeletionFirstStatus === "DELETION_PENDING") {
      resourceDetails["resource-a"] = {
        ...resourceDetails["resource-a"],
        version: 2,
        contextDocument: {
          ...(resourceDetails["resource-a"].contextDocument as Record<string, unknown>),
          status: "DELETION_PENDING",
        },
      };
      return contextDocumentDeletionStatus("DELETION_PENDING");
    }
    if (read === 0) return contextDocumentDeletionStatus("ACTIVE");
    if (read === 1) return contextDocumentDeletionStatus("DELETION_PENDING", 1);
    resourceDetails["resource-a"] = {
      ...resourceDetails["resource-a"],
      version: 2,
      contextDocument: {
        ...(resourceDetails["resource-a"].contextDocument as Record<string, unknown>),
        status: "DELETED",
      },
    };
    return contextDocumentDeletionStatus("DELETED", 2);
  }
  if (command === "list_resource_revisions") {
    const workspaceId = String(args.workspaceId);
    const resourceId = String(args.resourceId);
    if (state.resourceReadDeferral !== "history") return structuredClone(resourceRevisionPages[resourceId]);
    const request = deferred<unknown>();
    const key = resourceReadKey("history", workspaceId, resourceId);
    state.resourcePending.set(key, [...(state.resourcePending.get(key) ?? []), request]);
    return request.promise;
  }
  if (command === "preview_resource_text") {
    const workspaceId = String(args.workspaceId);
    const resourceId = String(args.resourceId);
    const key = resourcePreviewKey(workspaceId, resourceId);
    if (state.resourcePreviewDeferred.delete(key)) {
      const request = deferred<string>();
      state.resourcePreviewPending.set(key, [...(state.resourcePreviewPending.get(key) ?? []), request]);
      return request.promise;
    }
    return resourceId === "resource-a" ? "Saved A source text" : "Saved B source text";
  }
  if (command === "list_resource_locations") return [];
  if (command === "create_resource_revision_upload") {
    const session: RenderedUploadSession = {
      uploadId: "upload-a",
      workspaceId: args.workspaceId,
      resourceId: args.resourceId,
      displayName: args.resourceId === "resource-b" ? "B.txt" : "A.txt",
      mediaType: args.mediaType,
      expectedSizeBytes: args.sizeBytes,
      expectedDigest: args.expectedDigest,
      expectedResourceVersion: args.expectedResourceVersion,
      parentRevisionIds: args.parentRevisionIds,
      chunkSizeBytes: 64 * 1024,
      receivedRanges: [],
      nextMissingOffset: 0,
      state: "OPEN",
      committedResourceId: null,
      expiresAt: new Date(Date.now() + 60 * 60 * 1000).toISOString(),
    };
    state.resourceUploadSession = session;
    return structuredClone(session);
  }
  if (command === "get_resource_upload") {
    if (!state.resourceUploadSession || state.resourceUploadSession.uploadId !== args.uploadId) {
      return Promise.reject(new Error("Rendered harness has no matching Resource upload session."));
    }
    return structuredClone(state.resourceUploadSession);
  }
  if (command === "upload_resource_chunk") {
    const session = state.resourceUploadSession;
    if (!session || session.uploadId !== args.uploadId) return Promise.reject(new Error("Rendered harness has no matching Resource upload session."));
    const match = /^bytes (\d+)-(\d+)\/(\d+)$/.exec(String(args.contentRange));
    if (!match) return Promise.reject(new Error("Rendered harness received an invalid chunk range."));
    const startOffset = Number(match[1]);
    const endOffsetInclusive = Number(match[2]);
    session.receivedRanges = [{ startOffset, endOffsetInclusive, sha256: `sha256:${String(args.chunkSha256)}` }];
    session.nextMissingOffset = endOffsetInclusive + 1;
    if (session.nextMissingOffset === Number(session.expectedSizeBytes)) session.state = "CONTENT_RECEIVED";
    return null;
  }
  if (command === "commit_resource_revision_upload") {
    const committed = () => {
      const nextVersion = Number(resourceDetails["resource-a"].version) + 1;
      const nextRevisionId = `revision-a${nextVersion}`;
      resourceDetails["resource-a"] = { ...resourceDetails["resource-a"], currentRevisionId: nextRevisionId, version: nextVersion };
      if (state.resourceUploadSession?.uploadId === args.uploadId) {
        state.resourceUploadSession.state = "COMMITTED";
        state.resourceUploadSession.committedResourceId = "resource-a";
      }
      return { ...resourceA, resourceRevisionId: nextRevisionId, contentDigest: state.resourceUploadSession?.expectedDigest ?? resourceA.contentDigest };
    };
    if (state.resourceCommitMode === "reject") return Promise.reject(new Error("Rendered Resource save failed; the draft remains local."));
    if (state.resourceCommitMode === "conflict") return Promise.reject(new Error("RESOURCE_CONFLICT: Rendered Resource changed before commit."));
    if (state.resourceCommitMode === "defer") {
      const request = deferred<unknown>();
      state.resourceCommitPending.push(request);
      return request.promise.then(committed);
    }
    return committed();
  }
  if (command === "list_conversations") {
    const workspaceId = String(args.workspaceId);
    if (state.conversationListMode === "empty") return { items: [], next_cursor: null };
    if (state.conversationListMode === "reject") return Promise.reject(new Error("Rendered harness could not load Conversations."));
    if (state.conversationListMode === "defer") {
      const request = deferred<unknown>();
      state.conversationListPending.push(request);
      return request.promise;
    }
    if (state.conversationListMode === "51") {
      if (args.cursor === "older-page") {
        return { items: [conversationSummary(workspaceId, "conversation-51", "Conversation 51")], next_cursor: null };
      }
      return {
        items: Array.from({ length: 50 }, (_, index) => conversationSummary(workspaceId, `conversation-${index + 1}`, `Conversation ${index + 1}`)),
        next_cursor: "older-page",
      };
    }
    const item = workspaceId === "workspace-a"
      ? conversationSummary(workspaceId, "conversation-a", "Conversation A")
      : conversationSummary(workspaceId, "conversation-b", "Workspace B conversation");
    return { items: [item], next_cursor: null };
  }
  if (command === "get_conversation_presentation") {
    return richDocument(conversationMessage.message_id).then(value => {
      const snapshot = conversationSnapshot(String(args.workspaceId), state.richAvailability, value.reference, String(args.conversationId));
      return state.emptyConversationSnapshot ? { ...snapshot, items: [] } : snapshot;
    });
  }
  if (command === "get_rich_presentation") {
    if (state.richMode === "reject") return Promise.reject(new Error("test provider unavailable"));
    if (state.richMode === "defer") {
      const request = deferred<unknown>();
      state.richPending.push(request);
      return request.promise;
    }
    return richDocument(String(args.messageId)).then(value => value.response);
  }
  if (command === "list_pending_conversation_creates") {
    return structuredClone([...state.pendingConversationCreates.values()]);
  }
  if (command === "create_conversation") {
    recordPendingConversationCreate(args);
    const committed = commitConversationCreate(args);
    if (state.createMode === "lose-response") return Promise.reject(new Error("test lost response after commit"));
    if (state.createMode === "defer") {
      const request = deferred<unknown>();
      state.createPending.push(request);
      return request.promise;
    }
    return committed;
  }
  if (command === "acknowledge_conversation_create") {
    if (state.acknowledgeMode === "reject") return Promise.reject(new Error("test durable acknowledgement unavailable"));
    if (state.acknowledgeMode === "defer") {
      const request = deferred<void>();
      state.acknowledgePending.push({
        key: conversationCreateJournalKey(String(args.workspaceId), String(args.requestId)),
        request,
      });
      return request.promise.then(() => acknowledgeConversationCreate(args));
    }
    acknowledgeConversationCreate(args);
    return null;
  }
  if (command === "subscribe_conversation_stream") {
    state.activeStream = {
      sessionId: String(args.sessionId),
      workspaceId: String(args.workspaceId),
      conversationId: String(args.conversationId),
    };
    return null;
  }
  if (command === "acknowledge_conversation_stream_frame" || command === "unsubscribe_conversation_stream") return null;
  throw new Error(`Rendered harness received an unexpected Tauri command: ${command}`);
}, { shouldMockEvents: true });

async function emitStreamFrame(frame: Record<string, unknown>) {
  const stream = state.activeStream;
  if (!stream) throw new Error("Conversation stream is not subscribed yet.");
  state.frameId += 1;
  await emit("conversation-presentation-stream", {
    ...stream,
    kind: "frame",
    frameId: `rendered-frame-${state.frameId}`,
    frame,
  });
}

type HarnessApi = {
  resetResource: (deferredRead?: ResourceReadCommand | false) => true;
  selectResourceB: () => true;
  switchWorkspaceB: () => true;
  activeWorkspace: () => string;
  clearCalls: () => true;
  calls: () => Call[];
  configureConversation: (workspaceId: string, richMode: "success" | "defer" | "reject", availability: string, operatorReady?: boolean, messageId?: string, emptySnapshot?: boolean) => true;
  setConversationOperatorReady: (operatorReady: boolean) => true;
  setRichMode: (mode: "success" | "defer" | "reject") => true;
  resolveRich: () => true;
  setCreateMode: (mode: "success" | "defer" | "lose-response") => true;
  resolveCreate: () => true;
  setAcknowledgeMode: (mode: "success" | "defer" | "reject") => true;
  acknowledgePendingCount: () => number;
  pendingConversationCreateCount: (workspaceId: string) => number;
  resolveAcknowledge: () => true;
  switchConversationWorkspaceB: () => true;
  switchConversationWorkspaceA: () => true;
  configureConversationList: (mode: "single" | "51" | "empty" | "defer" | "reject") => true;
  conversationListPendingCount: () => number;
  resolveConversationList: (withSavedConversation?: boolean) => true;
  agentSettingsOpenCount: () => number;
  createCommitCount: () => number;
  resourcePendingCount: (command: ResourceReadCommand, resourceId: string) => number;
  resolveResource: (command: ResourceReadCommand, resourceId: string) => true;
  rejectResource: (command: ResourceReadCommand, resourceId: string) => true;
  deferNextResourcePreview: (resourceId: string) => true;
  resourcePreviewPendingCount: (resourceId: string) => number;
  resolveResourcePreview: (resourceId: string) => true;
  rejectResourcePreview: (resourceId: string) => true;
  activateLibraryNavigationGuard: () => true;
  setResourceCommitMode: (mode: "success" | "defer" | "reject" | "conflict") => true;
  setContextDocumentDeletionFirstStatus: (status: "ACTIVE" | "DELETION_PENDING") => true;
  setResourceContextDocumentOwner: (contextKind: "WORKSPACE_NOTES" | "COWORKER_NOTES", ownerWorkspaceId: string) => true;
  failNextContextDocumentDeletionDetailRefresh: (count?: number) => true;
  returnStaleContextDocumentDeletionDetailStatus: (count?: number) => true;
  resolveResourceCommit: () => true;
  emitReady: () => Promise<true>;
  hasActiveConversationStream: () => boolean;
  emitSnapshot: (availability: string, revision: number) => Promise<true>;
  emitEmptyConversationSnapshot: (revision: number) => Promise<true>;
  emitTurnDelta: (turnId: string, sequence: number, textDelta: string) => Promise<true>;
  emitTurnSettled: (turnId: string, retryOrdinal: number, committedMessageId?: string) => Promise<true>;
  emitConversationResync: () => Promise<true>;
  failConversationStream: () => Promise<true>;
  configureWorkspaceFirstRun: (scenario: WorkspaceFirstRunScenario) => true;
  workspaceActionCounts: () => { startRuntime: number; refreshRuntime: number; refreshWorkspaces: number; createWorkspace: number };
  mountProductionApp: (scenario: WorkspaceAppScenario) => true;
  mountAgentCatalogDefaultPrompt: () => true;
  selectAgentCatalogWorkspace: (workspaceId: string) => true;
  setResourceUploadState: (state: "FAILED" | "EXPIRED" | "ELAPSED_OPEN") => true;
  mismatchNextResourceUploadProgressWorkspace: () => true;
  resourceProgressPendingCount: () => number;
  rejectResourceProgressRead: () => true;
  resolveResourceProgressRead: () => Promise<true>;
  resolveInitialAppWorkspaceList: () => Promise<true>;
  rejectInitialAppWorkspaceList: () => Promise<true>;
  deferNextAppWorkspaceListRefresh: () => true;
  resolveStaleAppWorkspaceList: () => Promise<true>;
  resolveAppWorkspaceCreateReceipt: () => true;
  productionAppWorkspaceListCount: () => number;
  productionAppWorkspaceCreateCount: () => number;
  workspaceRootScanPendingCount: () => number;
  resolveWorkspaceRootScan: (status: "COMPLETE" | "PARTIAL") => Promise<true>;
  rejectWorkspaceRootScanAsStale: () => Promise<true>;
  rejectWorkspaceRootScan: (code: "SCAN_UNSUPPORTED_PLATFORM" | "RESOURCE_IDENTITY_UNAVAILABLE" | "SCAN_UNAVAILABLE") => Promise<true>;
  deferNextResourceFileDigest: (fileName: string) => true;
  gateResourceUploadFileDigest: (file: File) => true;
  resourceFileDigestPendingCount: () => number;
  resolveResourceFileDigest: () => Promise<true>;
  deferNextResourceUploadChunk: (chunkIndex: number) => true;
  resourceUploadChunkPendingCount: () => number;
  resolveResourceUploadChunk: () => Promise<true>;
  deferNextResourceUploadCommit: () => true;
  resourceUploadCommitPendingCount: () => number;
  resourceUploadCommittedReceipt: () => RenderedCommittedResource | null;
  resourceUploadListOmitsCommittedResource: () => boolean;
  resolveResourceUploadCommit: () => Promise<true>;
  deferNextResourceUploadCreate: () => true;
  resourceUploadCreatePendingCount: () => number;
  resolveResourceUploadCreate: () => Promise<true>;
  resourceUploadServerSessionCount: () => number;
};

declare global {
  interface Window { litecoworkHarness: HarnessApi }
}

function Harness() {
  const [activeWorkspace, setActiveWorkspace] = useState("workspace-a");
  const [requestedResource, setRequestedResource] = useState(resourceA);
  const [resourceMount, setResourceMount] = useState(0);
  const [resourceVisible, setResourceVisible] = useState(true);
  const [conversationWorkspace, setConversationWorkspace] = useState("workspace-a");
  const [conversationOperatorReady, setConversationOperatorReady] = useState(true);
  const [conversationMount, setConversationMount] = useState(0);
  const [conversationDrafts, setConversationDrafts] = useState<Record<string, string>>({});
  const [agentSettingsOpenCount, setAgentSettingsOpenCount] = useState(0);
  const [showLibraryNavigationGuard, setShowLibraryNavigationGuard] = useState(false);
  const [workspaceFirstRunProps, setWorkspaceFirstRunProps] = useState<WorkspaceFirstRunProps | null>(null);
  const [showProductionApp, setShowProductionApp] = useState(false);
  const [productionAppMount, setProductionAppMount] = useState(0);
  const [agentCatalogFixture, setAgentCatalogFixture] = useState<AgentCatalogSettingsProps | null>(null);
  const [lastDisabledAgentBindingId, setLastDisabledAgentBindingId] = useState<string | null>(null);

  window.litecoworkHarness = {
    resetResource(deferredRead = false) {
      state.resourceReadDeferral = deferredRead || null;
      state.resourcePending.clear();
      state.resourcePreviewDeferred.clear();
      state.resourcePreviewPending.clear();
      state.resourceCommitMode = "success";
      state.resourceUploadSession = null;
      state.contextDocumentDeletionBeginCalls = 0;
      state.contextDocumentDeletionStatusReads = 0;
      state.contextDocumentDeletionFirstStatus = "ACTIVE";
      state.contextDocumentDeletionDetailReadFailures = 0;
      state.contextDocumentDeletionStaleDetailStatusReads = 0;
      resourceDetails["resource-a"] = {
        ...resourceDetails["resource-a"], currentRevisionId: "revision-a1", version: 1,
        contextDocument: { kind: "WORKSPACE_NOTES", status: "ACTIVE", owner_ref: { kind: "WORKSPACE", workspace_id: "workspace-a" } },
      };
      resourceDetails["resource-b"] = {
        ...resourceDetails["resource-b"], currentRevisionId: "revision-b1", version: 1,
      };
      setActiveWorkspace("workspace-a");
      setRequestedResource(resourceA);
      setResourceVisible(true);
      setResourceMount(value => value + 1);
      return true;
    },
    selectResourceB() { setRequestedResource(resourceB); return true; },
    resourcePendingCount(command, resourceId) {
      const key = resourceReadKey(command, "workspace-a", resourceId);
      return state.resourcePending.get(key)?.length ?? 0;
    },
    resolveResource(command, resourceId) {
      const key = resourceReadKey(command, "workspace-a", resourceId);
      const pending = state.resourcePending.get(key)?.shift();
      if (!pending) throw new Error(`No deferred Resource ${command} response exists for ${resourceId}.`);
      pending.resolve(command === "detail" ? structuredClone(resourceDetails[resourceId]) : structuredClone(resourceRevisionPages[resourceId]));
      return true;
    },
    rejectResource(command, resourceId) {
      const key = resourceReadKey(command, "workspace-a", resourceId);
      const pending = state.resourcePending.get(key)?.shift();
      if (!pending) throw new Error(`No deferred Resource ${command} response exists for ${resourceId}.`);
      pending.reject(new Error(`Rendered stale Resource ${command} request failed for ${resourceId}.`));
      return true;
    },
    deferNextResourcePreview(resourceId) {
      state.resourcePreviewDeferred.add(resourcePreviewKey("workspace-a", resourceId));
      return true;
    },
    resourcePreviewPendingCount(resourceId) {
      return state.resourcePreviewPending.get(resourcePreviewKey("workspace-a", resourceId))?.length ?? 0;
    },
    resolveResourcePreview(resourceId) {
      const pending = state.resourcePreviewPending.get(resourcePreviewKey("workspace-a", resourceId))?.shift();
      if (!pending) throw new Error(`No deferred Resource preview exists for ${resourceId}.`);
      pending.resolve(resourceId === "resource-a" ? "Saved A source text" : "Saved B source text");
      return true;
    },
    rejectResourcePreview(resourceId) {
      const pending = state.resourcePreviewPending.get(resourcePreviewKey("workspace-a", resourceId))?.shift();
      if (!pending) throw new Error(`No deferred Resource preview exists for ${resourceId}.`);
      pending.reject(new Error(`Rendered stale Resource preview failed for ${resourceId}.`));
      return true;
    },
    activateLibraryNavigationGuard() {
      state.resourceReadDeferral = null;
      state.resourcePending.clear();
      state.resourcePreviewDeferred.clear();
      state.resourcePreviewPending.clear();
      resourceDetails["resource-a"] = {
        ...resourceDetails["resource-a"], currentRevisionId: "revision-a1", version: 1,
        contextDocument: { kind: "WORKSPACE_NOTES", status: "ACTIVE", owner_ref: { kind: "WORKSPACE", workspace_id: "workspace-a" } },
      };
      state.resourceCommitMode = "success";
      state.resourceCommitPending = [];
      setShowLibraryNavigationGuard(true);
      return true;
    },
    setResourceCommitMode(mode) { state.resourceCommitMode = mode; return true; },
    setContextDocumentDeletionFirstStatus(status) { state.contextDocumentDeletionFirstStatus = status; return true; },
    setResourceContextDocumentOwner(contextKind, ownerWorkspaceId) {
      const ownerRef = contextKind === "WORKSPACE_NOTES"
        ? { kind: "WORKSPACE", workspace_id: ownerWorkspaceId }
        : { kind: "COWORKER", workspace_id: ownerWorkspaceId, coworker_id: "coworker-a" };
      resourceDetails["resource-a"] = {
        ...resourceDetails["resource-a"],
        contextDocument: { kind: contextKind, status: "ACTIVE", owner_ref: ownerRef },
      };
      setResourceMount(value => value + 1);
      return true;
    },
    failNextContextDocumentDeletionDetailRefresh(count = 1) {
      state.contextDocumentDeletionDetailReadFailures = Math.max(0, Math.min(10, Math.floor(count)));
      return true;
    },
    returnStaleContextDocumentDeletionDetailStatus(count = 1) {
      state.contextDocumentDeletionStaleDetailStatusReads = Math.max(0, Math.min(10, Math.floor(count)));
      return true;
    },
    resolveResourceCommit() {
      const pending = state.resourceCommitPending.shift();
      if (!pending) throw new Error("No deferred Resource commit exists.");
      pending.resolve(null);
      return true;
    },
    switchWorkspaceB() { setActiveWorkspace("workspace-b"); return true; },
    activeWorkspace: () => activeWorkspace,
    clearCalls() { state.calls.length = 0; return true; },
    calls: () => structuredClone(state.calls),
    configureConversation(workspaceId, richMode, availability, operatorReady = true, messageId = "message-a", emptySnapshot = false) {
      state.richMode = richMode;
      state.richAvailability = availability;
      conversationMessage.message_id = messageId;
      state.richPending = [];
      state.createMode = "success";
      state.createPending = [];
      state.acknowledgeMode = "success";
      state.activeStream = null;
      state.emptyConversationSnapshot = emptySnapshot;
      state.appScenario = null;
      setShowProductionApp(false);
      setConversationWorkspace(workspaceId);
      setConversationOperatorReady(operatorReady);
      setConversationMount(value => value + 1);
      return true;
    },
    configureConversationList(mode) { state.conversationListMode = mode; state.conversationListPending = []; return true; },
    conversationListPendingCount: () => state.conversationListPending.length,
    setConversationOperatorReady(operatorReady) { setConversationOperatorReady(operatorReady); return true; },
    resolveConversationList(withSavedConversation = false) {
      const pending = state.conversationListPending.shift();
      if (!pending) throw new Error("There is no deferred Conversation list request.");
      const items = withSavedConversation
        ? [conversationSummary(conversationWorkspace, conversationWorkspace === "workspace-a" ? "conversation-a" : "conversation-b", "Reconnected conversation")]
        : [];
      pending.resolve({ items, next_cursor: null });
      return true;
    },
    agentSettingsOpenCount: () => agentSettingsOpenCount,
    createCommitCount: () => state.createCommits,
    setRichMode(mode) { state.richMode = mode; return true; },
    resolveRich() {
      const pending = state.richPending.shift();
      if (!pending) throw new Error("There is no deferred RichPresentation fetch to resolve.");
      pending.resolve(rich.response);
      return true;
    },
    setCreateMode(mode) { state.createMode = mode; return true; },
    resolveCreate() {
      const pending = state.createPending.shift();
      if (!pending) throw new Error("There is no deferred Conversation create to resolve.");
      const call = state.calls.findLast(entry => entry.command === "create_conversation");
      if (!call) throw new Error("No Conversation create request was recorded.");
      pending.resolve(commitConversationCreate(call.args));
      return true;
    },
    setAcknowledgeMode(mode) { state.acknowledgeMode = mode; return true; },
    acknowledgePendingCount: () => state.acknowledgePending.length,
    pendingConversationCreateCount(workspaceId) {
      return [...state.pendingConversationCreates.values()].filter(request => request.workspaceId === workspaceId).length;
    },
    resolveAcknowledge() {
      const pending = state.acknowledgePending.shift();
      if (!pending) throw new Error("There is no deferred Conversation acknowledgement to resolve.");
      pending.request.resolve();
      return true;
    },
    switchConversationWorkspaceB() { setConversationWorkspace("workspace-b"); return true; },
    switchConversationWorkspaceA() { setConversationWorkspace("workspace-a"); return true; },
    hasActiveConversationStream: () => state.activeStream !== null,
    async emitReady() {
      await emitStreamFrame({
        type: "stream.ready",
        workspace_id: "workspace-a",
        projection_version: 1,
        cursor: "rendered-cursor-1",
        max_frame_bytes: 1024 * 1024,
      });
      return true;
    },
    async emitSnapshot(availability, revision) {
      state.richAvailability = availability;
      const messageRich = await richDocument(conversationMessage.message_id);
      await emitStreamFrame({
        type: "projection",
        workspace_id: "workspace-a",
        projection_type: "conversation_presentation",
        projection_revision: revision,
        cursor: `rendered-cursor-${revision}`,
        payload: {
          conversation_id: "conversation-a",
          stream_cursor: `conversation-history-a-${revision}`,
          origin_sequence: String(revision),
          items: [{
            message: conversationMessage,
            rich_presentation: { ...messageRich.reference, availability },
            linked_items: [],
          }, ...Array.from({ length: 6 }, (_, index) => ({
            message: {
              message_id: `message-new-${index + 1}`,
              conversation_id: "conversation-a",
              role: "AGENT",
              created_at: `2026-10-13T12:00:${String(index + 1).padStart(2, "0")}.000Z`,
              content: [{ kind: "TEXT", text: `Newer answer ${index + 1}.` }],
            },
            rich_presentation: null,
            linked_items: [],
          }))],
          next_cursor: null,
          active_turn: null,
        },
      });
      return true;
    },
    async emitEmptyConversationSnapshot(revision) {
      await emitStreamFrame({
        type: "projection",
        workspace_id: "workspace-a",
        projection_type: "conversation_presentation",
        projection_revision: revision,
        cursor: `rendered-cursor-${revision}`,
        payload: {
          conversation_id: "conversation-a",
          stream_cursor: `conversation-history-a-${revision}`,
          origin_sequence: String(revision),
          items: [],
          next_cursor: null,
          active_turn: null,
        },
      });
      return true;
    },
    async emitTurnDelta(turnId, sequence, textDelta) {
      await emitStreamFrame({
        type: "turn.delta", workspace_id: "workspace-a", conversation_id: "conversation-a",
        turn_id: turnId, retry_ordinal: 0, sequence, text_delta: textDelta,
      });
      return true;
    },
    async emitTurnSettled(turnId, retryOrdinal, committedMessageId) {
      await emitStreamFrame({
        type: "turn.settled", workspace_id: "workspace-a", conversation_id: "conversation-a",
        turn_id: turnId, retry_ordinal: retryOrdinal,
        ...(committedMessageId ? { committed_message_id: committedMessageId } : {}),
      });
      return true;
    },
    async emitConversationResync() {
      await emitStreamFrame({ type: "stream.resync_required", workspace_id: "workspace-a", reason: "CURSOR_EXPIRED" });
      return true;
    },
    async failConversationStream() {
      const stream = state.activeStream;
      if (!stream) throw new Error("Conversation stream is not subscribed yet.");
      await emit("conversation-presentation-stream", { ...stream, kind: "error", message: "private transport details stay hidden" });
      return true;
    },
    configureWorkspaceFirstRun(scenario) {
      state.workspaceActions = { startRuntime: 0, refreshRuntime: 0, refreshWorkspaces: 0, createWorkspace: 0 };
      setShowLibraryNavigationGuard(false);
      setShowProductionApp(false);
      const onWorkspaceNameChange = (name: string) => setWorkspaceFirstRunProps(current => current ? { ...current, workspaceName: name } : current);
      setWorkspaceFirstRunProps(workspaceFirstRunFixtureProps(scenario, onWorkspaceNameChange));
      return true;
    },
    mountProductionApp(scenario) {
      state.appScenario = scenario;
      state.appRuntimeStatusCalls = 0;
      state.appWorkspaceListCalls = 0;
      state.appInitialWorkspaceList = scenario === "workspace-list-race" || scenario === "workspace-list-failure"
        ? deferred<WorkspaceAppWorkspace[]>()
        : null;
      state.appStaleWorkspaceList = null;
      state.appCreatedWorkspace = isF90UploadScenario(scenario) || scenario === "first-use-conversation" || isWorkspaceRootScanScenario(scenario) ? {
        workspaceId: "workspace-a",
        name: "Workspace A",
        replicationPolicy: "LOCAL_ONLY",
        defaultAgentBindingId: null,
        primaryCoworkerId: null,
        status: "ACTIVE",
        version: 1,
      } : null;
      if ([
        "workspace-policy-lost-ack",
        "workspace-policy-lost-ack-confirmed",
        "workspace-policy-list-race",
      ].includes(scenario ?? "")) {
        state.appCreatedWorkspace = {
          workspaceId: "workspace-policy-a",
          name: "Policy Workspace",
          replicationPolicy: "METADATA_ONLY",
          defaultAgentBindingId: null,
          primaryCoworkerId: null,
          status: "ACTIVE",
          version: 4,
        };
      }
      state.appWorkspacePolicyRequests = [];
      state.appWorkspacePolicyCommitted = null;
      state.appWorkspacePolicyListRefreshFails = false;
      state.workspaceRootScanPending = [];
      state.appWorkspaceCreateReceipt = scenario === "workspace-list-race" ? deferred<WorkspaceAppWorkspace>() : null;
      state.appWorkspaceCreateCalls = 0;
      state.f90UploadSession = null;
      state.f90UploadCreates = 0;
      state.f90UploadCreateReceipts.clear();
      state.f90DeferNextCreateResponse = false;
      state.f90CreatePending = [];
      state.f90LoseNextCreateAck = scenario === "resource-upload-ambiguous-create";
      state.f90DeferFileDigestName = null;
      state.f90FileDigestPending = [];
      state.f90DeferredChunkIndexes.clear();
      state.f90ChunkPending = [];
      state.f90ChunkRequestReceipts.clear();
      state.f90LoseNextChunkAckBeforeAccept = scenario === "resource-upload-ambiguous-chunk-unaccepted";
      state.f90CommitReceipts.clear();
      state.f90DeferNextCommitResponse = false;
      state.f90CommitPending = [];
      state.f90OmitCommittedResourceFromList = scenario === "resource-upload-pause-commit-pending";
      state.f90LoseNextChunkAck = [
        "resource-upload-ambiguous-chunk",
        "resource-upload-ambiguous-chunk-bad-range",
        "resource-upload-progress-read-failure",
        "resource-upload-malformed-progress",
        "resource-upload-mismatched-progress",
        "resource-upload-progress-read-deferred",
        "resource-upload-pause-progress-pending",
        "resource-upload-terminal-explicit-forget",
      ].includes(scenario);
      state.f90LoseNextCommitAck = scenario === "resource-upload-commit-progress-read-failure";
      state.f90FailNextProgressRead = scenario === "resource-upload-progress-read-failure" || scenario === "resource-upload-commit-progress-read-failure";
      state.f90MalformedNextProgressRead = scenario === "resource-upload-malformed-progress";
      state.f90MismatchNextProgressWorkspace = scenario === "resource-upload-mismatched-progress";
      state.f90DeferNextProgressRead = scenario === "resource-upload-progress-read-deferred" || scenario === "resource-upload-pause-progress-pending";
      state.f90ProgressReadPending = [];
      state.f90CorruptNextRangeRead = scenario === "resource-upload-ambiguous-chunk-bad-range";
      state.f90CommittedResource = null;
      state.calls.length = 0;
      localStorage.removeItem("litecowork.selectedWorkspaceId");
      localStorage.removeItem("litecowork.resourceUploadResume.v1");
      setShowLibraryNavigationGuard(false);
      setWorkspaceFirstRunProps(null);
      setShowProductionApp(true);
      setProductionAppMount(value => value + 1);
      return true;
    },
    mountAgentCatalogDefaultPrompt() {
      setShowLibraryNavigationGuard(false);
      setShowProductionApp(false);
      setWorkspaceFirstRunProps(null);
      setLastDisabledAgentBindingId(null);
      setAgentCatalogFixture({
        workspaceId: "workspace-a",
        workspaceName: "Workspace A",
        workspaceActive: true,
        runtimeReady: true,
        runtimeEnrolled: false,
        selectedDefaultLeadBindingId: "binding-a",
        installations: [],
        profiles: [
          { agentProfileId: "profile-a", providerKey: "TEST_A", displayName: "Primary Agent", endpoints: [], discoveredAt: "2026-10-15T00:00:00Z", observations: [] },
          { agentProfileId: "profile-b", providerKey: "TEST_B", displayName: "Secondary Agent", endpoints: [], discoveredAt: "2026-10-15T00:00:00Z", observations: [] },
        ],
        bindings: [
          {
            agentBindingId: "binding-a",
            workspaceId: "workspace-a",
            agentProfileId: "profile-a",
            runtimeId: null,
            endpointSelectionPolicy: {},
            authRef: null,
            configuration: {},
            enabled: true,
            leadEligible: true,
            createdAt: "2026-10-15T00:00:00Z",
            version: 1,
          },
          {
            agentBindingId: "binding-secondary",
            workspaceId: "workspace-a",
            agentProfileId: "profile-b",
            runtimeId: null,
            endpointSelectionPolicy: {},
            authRef: null,
            configuration: {},
            enabled: true,
            leadEligible: true,
            createdAt: "2026-10-15T00:00:00Z",
            version: 1,
          },
        ],
        onRefresh() {},
        onProbeCodex() {},
        onProbeOpenCode() {},
        onCreateBinding() {},
        onEnableBinding() {},
        onDisableBinding(binding) { setLastDisabledAgentBindingId(binding.agentBindingId); },
        onSetDefaultLead() {},
      });
      return true;
    },
    selectAgentCatalogWorkspace(workspaceId) {
      setAgentCatalogFixture(current => current ? {
        ...current,
        workspaceId,
        workspaceName: workspaceId === "workspace-a" ? "Workspace A" : "Workspace B",
      } : current);
      return true;
    },
    setResourceUploadState(value) {
      if (!state.f90UploadSession) throw new Error("There is no Resource upload session to update.");
      if (value === "ELAPSED_OPEN") {
        state.f90UploadSession.state = "OPEN";
        state.f90UploadSession.expiresAt = "2000-01-01T00:00:00.000Z";
      } else {
        state.f90UploadSession.state = value;
      }
      return true;
    },
    mismatchNextResourceUploadProgressWorkspace() {
      state.f90MismatchNextProgressWorkspace = true;
      return true;
    },
    resourceProgressPendingCount() { return state.f90ProgressReadPending.length; },
    rejectResourceProgressRead() {
      const pending = state.f90ProgressReadPending.shift();
      if (!pending) throw new Error("There is no deferred Resource upload progress read.");
      pending.reject(new Error("Rendered harness deferred progress read failed."));
      return true;
    },
    async resolveResourceProgressRead() {
      const pending = state.f90ProgressReadPending.shift();
      if (!pending || !state.f90UploadSession) throw new Error("There is no deferred Resource upload progress response.");
      pending.resolve(structuredClone(state.f90UploadSession));
      await flushRenderedTurn();
      return true;
    },
    async resolveInitialAppWorkspaceList() {
      const pending = state.appInitialWorkspaceList;
      if (!pending) throw new Error("There is no deferred initial production App Workspace list.");
      state.appInitialWorkspaceList = null;
      pending.resolve([]);
      await new Promise<void>(resolve => window.setTimeout(resolve, 0));
      await new Promise<void>(resolve => window.requestAnimationFrame(() => resolve()));
      return true;
    },
    async rejectInitialAppWorkspaceList() {
      const pending = state.appInitialWorkspaceList;
      if (!pending) throw new Error("There is no deferred initial production App Workspace list.");
      state.appInitialWorkspaceList = null;
      pending.reject(new Error("Rendered harness initial Workspace list failed."));
      await new Promise<void>(resolve => window.setTimeout(resolve, 0));
      await new Promise<void>(resolve => window.requestAnimationFrame(() => resolve()));
      return true;
    },
    deferNextAppWorkspaceListRefresh() {
      if (state.appStaleWorkspaceList) throw new Error("A stale Workspace list response is already pending.");
      state.appStaleWorkspaceList = deferred<WorkspaceAppWorkspace[]>();
      return true;
    },
    async resolveStaleAppWorkspaceList(workspaces: WorkspaceAppWorkspace[] = []) {
      const pending = state.appStaleWorkspaceList;
      if (!pending) throw new Error("There is no deferred stale Workspace list response.");
      state.appStaleWorkspaceList = null;
      pending.resolve(structuredClone(workspaces));
      await new Promise<void>(resolve => window.setTimeout(resolve, 0));
      await new Promise<void>(resolve => window.requestAnimationFrame(() => resolve()));
      return true;
    },
    resolveAppWorkspaceCreateReceipt() {
      const pending = state.appWorkspaceCreateReceipt;
      if (!pending || !state.appCreatedWorkspace) throw new Error("There is no pending production App Workspace create receipt.");
      state.appWorkspaceCreateReceipt = null;
      pending.resolve(state.appCreatedWorkspace);
      return true;
    },
    productionAppWorkspaceListCount: () => state.appWorkspaceListCalls,
    productionAppWorkspaceCreateCount: () => state.appWorkspaceCreateCalls,
    workspaceRootScanPendingCount: () => state.workspaceRootScanPending.length,
    async resolveWorkspaceRootScan(status) {
      const pending = state.workspaceRootScanPending.shift();
      const call = [...state.calls].reverse().find(item => item.command === "scan_workspace_root");
      if (!pending || !call) throw new Error("There is no pending WorkspaceRoot scan request.");
      pending.resolve({
        workspaceId: call.args.workspaceId,
        workspaceRootId: call.args.workspaceRootId,
        requestId: call.args.requestId,
        rootVersion: call.args.expectedVersion,
        status,
        entriesExamined: status === "COMPLETE" ? 12 : 7,
        filesObserved: status === "COMPLETE" ? 8 : 3,
        partialReason: status === "COMPLETE" ? null : "SCAN_BUDGET_REACHED",
        completedAt: now,
      });
      await flushRenderedTurn();
      return true;
    },
    async rejectWorkspaceRootScanAsStale() {
      const pending = state.workspaceRootScanPending.shift();
      if (!pending) throw new Error("There is no pending WorkspaceRoot scan request.");
      pending.reject(new Error("STALE_WORKSPACE_VERSION"));
      await flushRenderedTurn();
      return true;
    },
    async rejectWorkspaceRootScan(code) {
      if (state.appScenario === "workspace-root-scan-unsupported" && code !== "SCAN_UNSUPPORTED_PLATFORM") {
        throw new Error("The unsupported-platform rendered scenario received the wrong scan failure.");
      }
      if (state.appScenario === "workspace-root-scan-rejected" && code !== "RESOURCE_IDENTITY_UNAVAILABLE") {
        throw new Error("The deterministic-rejection rendered scenario received the wrong scan failure.");
      }
      if (state.appScenario === "workspace-root-scan-ambiguous" && code !== "SCAN_UNAVAILABLE") {
        throw new Error("The ambiguous scan rendered scenario received the wrong scan failure.");
      }
      const pending = state.workspaceRootScanPending.shift();
      if (!pending) throw new Error("There is no pending WorkspaceRoot scan request.");
      pending.reject(new Error(code));
      await flushRenderedTurn();
      return true;
    },
    productionAppWorkspacePolicyRequests: () => structuredClone(state.appWorkspacePolicyRequests),
    deferNextResourceFileDigest(fileName) {
      state.f90DeferFileDigestName = fileName;
      return true;
    },
    gateResourceUploadFileDigest(file) {
      if (file.name !== state.f90DeferFileDigestName) return true;
      state.f90DeferFileDigestName = null;
      const read = file.arrayBuffer.bind(file);
      let pending: DeferredResourceFileDigest | null = null;
      Object.defineProperty(file, "arrayBuffer", {
        configurable: true,
        value: () => {
          if (!pending) {
            pending = { request: deferred<ArrayBuffer>(), read };
            state.f90FileDigestPending.push(pending);
          }
          return pending.request.promise;
        },
      });
      return true;
    },
    resourceFileDigestPendingCount: () => state.f90FileDigestPending.length,
    async resolveResourceFileDigest() {
      const pending = state.f90FileDigestPending.shift();
      if (!pending) throw new Error("There is no deferred Resource file digest.");
      pending.request.resolve(await pending.read());
      await flushRenderedTurn();
      return true;
    },
    deferNextResourceUploadChunk(chunkIndex) {
      state.f90DeferredChunkIndexes.add(chunkIndex);
      return true;
    },
    resourceUploadChunkPendingCount: () => state.f90ChunkPending.length,
    async resolveResourceUploadChunk() {
      const pending = state.f90ChunkPending.shift();
      if (!pending) throw new Error("There is no deferred Resource upload chunk response.");
      pending.request.resolve(null);
      await flushRenderedTurn();
      return true;
    },
    deferNextResourceUploadCommit() {
      state.f90DeferNextCommitResponse = true;
      return true;
    },
    resourceUploadCommitPendingCount: () => state.f90CommitPending.length,
    resourceUploadCommittedReceipt: () => state.f90CommittedResource ? structuredClone(state.f90CommittedResource) : null,
    resourceUploadListOmitsCommittedResource: () => state.f90OmitCommittedResourceFromList,
    async resolveResourceUploadCommit() {
      const pending = state.f90CommitPending.shift();
      if (!pending) throw new Error("There is no deferred Resource upload commit response.");
      pending.request.resolve(structuredClone(pending.resource));
      await flushRenderedTurn();
      return true;
    },
    deferNextResourceUploadCreate() {
      state.f90DeferNextCreateResponse = true;
      return true;
    },
    resourceUploadCreatePendingCount: () => state.f90CreatePending.length,
    async resolveResourceUploadCreate() {
      const pending = state.f90CreatePending.shift();
      if (!pending) throw new Error("There is no deferred Resource upload create response.");
      pending.request.resolve(structuredClone(pending.session));
      await flushRenderedTurn();
      return true;
    },
    resourceUploadServerSessionCount: () => state.f90UploadCreateReceipts.size,
    workspaceActionCounts: () => ({ ...state.workspaceActions }),
  };

  if (showLibraryNavigationGuard) return <LibraryNavigationFixture />;
  if (showProductionApp) return <App key={productionAppMount} />;
  if (agentCatalogFixture) return <main className="harness-ready" data-agent-catalog-workspace={agentCatalogFixture.workspaceId}>
    <output data-last-disabled-agent-binding={lastDisabledAgentBindingId ?? ""}>{lastDisabledAgentBindingId ?? ""}</output>
    <AgentCatalogSettings {...agentCatalogFixture} />
  </main>;
  if (workspaceFirstRunProps) return <main className="harness-ready">
    <h1>Workspace first-run rendered fixture</h1>
    <section aria-label="First-run Workspace setup">
      <WorkspaceFirstRun {...workspaceFirstRunProps} />
    </section>
  </main>;

  return <main className="harness-ready">
    <header>
      <h1>LiteCowork rendered UI test harness</h1>
      <nav aria-label="Test fixture controls">
        <button type="button" onClick={() => window.litecoworkHarness.selectResourceB()}>Select Resource B</button>
        <button type="button" onClick={() => window.litecoworkHarness.switchWorkspaceB()}>Switch Resource Workspace to B</button>
        <button type="button" onClick={() => window.litecoworkHarness.switchConversationWorkspaceB()}>Switch Conversation Workspace to B</button>
      </nav>
    </header>
    <section aria-label="Resource revision editor">
      {resourceVisible && <ResourceRevisionEditor
        key={resourceMount}
        resource={requestedResource}
        activeWorkspaceId={activeWorkspace}
        onClose={() => setResourceVisible(false)}
        onCommitted={() => {}}
        onSelectResource={setRequestedResource}
        onSelectWorkspace={setActiveWorkspace}
      />}
    </section>
    <section aria-label="Conversation history fixture">
      <ConversationsPage
        key={`${conversationMount}-${conversationWorkspace}`}
        workspaceId={conversationWorkspace}
        workspaceName={conversationWorkspace === "workspace-a" ? "Workspace A" : "Workspace B"}
        operatorReady={conversationOperatorReady && Boolean(conversationWorkspace)}
        draft={conversationDrafts[conversationWorkspace] ?? ""}
        onDraftChange={value => setConversationDrafts(current => ({ ...current, [conversationWorkspace]: value }))}
        onOpenAgentSettings={() => setAgentSettingsOpenCount(value => value + 1)}
      />
    </section>
  </main>;
}

function LibraryNavigationFixture() {
  const navigation = useLibraryNavigation<"Home" | "Library">("Library");
  const [drawerOpen, setDrawerOpen] = useState(false);
  const mobileMenuRef = useRef<HTMLButtonElement>(null);
  const leaveLibrary = () => {
    const focusTarget = drawerOpen && window.innerWidth <= 900 ? mobileMenuRef.current : null;
    navigation.requestNavigation("Home", focusTarget);
    if (drawerOpen) setDrawerOpen(false);
  };

  return <div className="harness-ready">
    <h1>Library navigation rendered fixture</h1>
    <button id="test-mobile-menu" ref={mobileMenuRef} type="button" onClick={() => setDrawerOpen(true)}>Open mobile menu</button>
    <button type="button" onClick={leaveLibrary}>Leave Library for Home</button>
    <nav className="library-test-navigation" aria-label="Test navigation" hidden={!drawerOpen}>
      <button type="button" onClick={leaveLibrary}>Home</button>
    </nav>
    <section className="library-test-main" inert={libraryNavigationMainIsInert(false, navigation.pendingNavigation !== null)} aria-hidden={libraryNavigationMainIsInert(false, navigation.pendingNavigation !== null) || undefined}>
      {navigation.page === "Library" ? <ResourceRevisionEditor
        resource={resourceA}
        activeWorkspaceId="workspace-a"
        onClose={() => {}}
        onCommitted={navigation.draftCommitted}
        onSelectResource={() => {}}
        onSelectWorkspace={() => {}}
        onNavigationSaveFailure={navigation.draftSaveFailed}
      /> : <div><h2 id="library-navigation-home" data-testid="library-navigation-home">Home destination</h2><button type="button" onClick={() => navigation.requestNavigation("Library")}>Return to Library</button></div>}
    </section>
    <LibraryNavigationDialog
      pending={navigation.pendingNavigation}
      onStay={navigation.stayInLibrary}
      onSave={navigation.saveLibraryDraftBeforeNavigation}
      onDiscard={navigation.discardLibraryDraftAndNavigate}
    />
  </div>;
}

ReactDOM.createRoot(document.getElementById("root")!).render(<Harness />);
