import { useRef, useState, type ComponentProps } from "react";
import ReactDOM from "react-dom/client";
import { emit } from "@tauri-apps/api/event";
import { mockIPC } from "@tauri-apps/api/mocks";
import App from "../../src/App";
import { ConversationsPage } from "../../src/conversations/ConversationsPage";
import { ResourceRevisionEditor } from "../../src/resources/ResourceRevisionEditor";
import { LibraryNavigationDialog, libraryNavigationMainIsInert, useLibraryNavigation } from "../../src/resources/library-navigation";
import { WorkspaceFirstRun } from "../../src/workspaces/WorkspaceFirstRun";
import "../../src/styles.css";

type WorkspaceFirstRunProps = ComponentProps<typeof WorkspaceFirstRun>;
type WorkspaceFirstRunScenario = "loading" | "error" | "offline" | "runtime-error" | "existing" | "ready" | "pending" | "busy";
type WorkspaceRuntimeStatus = Exclude<WorkspaceFirstRunProps["runtime"], null>;
type WorkspaceAppScenario = "workspace-list-race" | "workspace-list-failure" | "runtime-refresh-failure" | "resource-upload-ambiguous-chunk" | "resource-upload-ambiguous-chunk-bad-range";
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
  state: "OPEN" | "CONTENT_RECEIVED" | "COMMITTED";
  expiresAt: string;
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

const richDocument = async () => {
  const semanticBytes = new TextEncoder().encode(semanticText);
  const document = {
    schema_version: 1,
    renderer_contract_version: 1,
    presentation_id: "presentation-a",
    message_id: "message-a",
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
      message_id: "message-a",
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
      message_id: "message-a",
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
    contextDocument: { kind: "WORKSPACE_RESOURCE", status: "ACTIVE", owner_ref: { kind: "WORKSPACE", workspace_id: "workspace-a" } },
  },
  "resource-b": {
    resourceId: "resource-b", workspaceId: "workspace-a", kind: "TEXT", displayName: "B.txt",
    currentRevisionId: "revision-b1", version: 1,
    contextDocument: { kind: "WORKSPACE_RESOURCE", status: "ACTIVE", owner_ref: { kind: "WORKSPACE", workspace_id: "workspace-a" } },
  },
};

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

function conversationSnapshot(workspaceId: string, availability: string, rich: Awaited<ReturnType<typeof richDocument>>["reference"]) {
  const isA = workspaceId === "workspace-a";
  return {
    workspace_id: workspaceId,
    conversation_id: isA ? "conversation-a" : "conversation-b",
    projection_revision: 1,
    stream_cursor: `conversation-history-${isA ? "a" : "b"}-1`,
    origin_sequence: "1",
    items: [{
      message: isA ? conversationMessage : conversationBMessage,
      rich_presentation: isA ? { ...rich, availability } : null,
      linked_items: [],
    }],
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
const state = {
  calls: [] as Call[],
  activeWorkspace: "workspace-a",
  richAvailability: "AVAILABLE",
  richMode: "success" as "success" | "defer" | "reject",
  richPending: [] as Deferred<unknown>[],
  createMode: "success" as "success" | "defer" | "lose-response",
  createPending: [] as Deferred<unknown>[],
  createReceipts: new Map<string, ReturnType<typeof conversationSummary>>(),
  createCommits: 0,
  conversationListMode: "single" as "single" | "51",
  activeStream: null as { sessionId: string; workspaceId: string; conversationId: string } | null,
  resourceReadsDeferred: false,
  resourcePending: new Map<string, Deferred<unknown>[]>(),
  resourceCommitMode: "success" as "success" | "defer" | "reject",
  resourceCommitPending: [] as Deferred<unknown>[],
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
  f90UploadSession: null as RenderedResourceUploadSession | null,
  f90UploadCreates: 0,
  f90LoseNextChunkAck: false,
  f90CorruptNextRangeRead: false,
  f90CommittedResource: null as {
    resourceId: string;
    workspaceId: string;
    resourceRevisionId: string;
    displayName: string;
    mediaType: string;
    contentDigest: string;
    sizeBytes: number;
  } | null,
  frameId: 0,
};

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
  const requestId = String(args.requestId);
  const existing = state.createReceipts.get(requestId);
  if (existing) return existing;
  const committed = conversationSummary(String(args.workspaceId), `conversation-created-${state.createCommits + 1}`, String(args.title ?? "New conversation"));
  state.createReceipts.set(requestId, committed);
  state.createCommits += 1;
  return committed;
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
  return state.appCreatedWorkspace ? [state.appCreatedWorkspace] : [];
}

function isF90UploadScenario(scenario: WorkspaceAppScenario | null): boolean {
  return scenario?.startsWith("resource-upload-ambiguous-chunk") === true;
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
  if (isF90UploadScenario(state.appScenario) && command === "create_resource_upload") {
    state.f90UploadCreates += 1;
    return structuredClone(createRenderedResourceUpload(args));
  }
  if (isF90UploadScenario(state.appScenario) && command === "get_resource_upload") {
    if (!state.f90UploadSession || state.f90UploadSession.uploadId !== args.uploadId) {
      return Promise.reject(new Error("Rendered harness has no matching Resource upload session."));
    }
    const view = structuredClone(state.f90UploadSession);
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
    if (state.f90LoseNextChunkAck) {
      state.f90LoseNextChunkAck = false;
      return Promise.reject(new Error("Rendered harness lost the chunk acknowledgement."));
    }
    return null;
  }
  if (isF90UploadScenario(state.appScenario) && command === "commit_resource_upload") {
    const session = state.f90UploadSession;
    if (!session || session.uploadId !== args.uploadId || session.state !== "CONTENT_RECEIVED") {
      return Promise.reject(new Error("Rendered harness refused to commit before every chunk was accepted."));
    }
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
    return structuredClone(state.f90CommittedResource);
  }
  if (command === "list_agent_installations") return [];
  if (command === "list_agent_profiles" || command === "list_agent_bindings") return { items: [], nextCursor: null };
  if (command === "list_workspace_runtime_bindings" || command === "list_workspace_instructions") return [];
  if (command === "list_resources") return { items: state.f90CommittedResource ? [state.f90CommittedResource] : [], nextCursor: null };
  if (command === "list_workspace_roots") return { items: [], nextCursor: null };
  if (command === "list_delegation_profiles") {
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
  if (state.appScenario !== null) return handleWorkspaceAppInvoke(command, args);
  if (command === "get_resource_detail") {
    const resourceId = String(args.resourceId);
    if (!state.resourceReadsDeferred) return resourceDetails[resourceId];
    const request = deferred<unknown>();
    state.resourcePending.set(resourceId, [...(state.resourcePending.get(resourceId) ?? []), request]);
    return request.promise;
  }
  if (command === "list_resource_revisions") {
    const resourceId = String(args.resourceId);
    return { items: [], nextCursor: null };
  }
  if (command === "preview_resource_text") {
    return args.resourceId === "resource-a" ? "Saved A source text" : "Saved B source text";
  }
  if (command === "list_resource_locations") return [];
  if (command === "create_resource_revision_upload") {
    const session: RenderedUploadSession = {
      uploadId: "upload-a",
      workspaceId: args.workspaceId,
      resourceId: args.resourceId,
      displayName: "A.txt",
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
    if (state.resourceCommitMode === "defer") {
      const request = deferred<unknown>();
      state.resourceCommitPending.push(request);
      return request.promise.then(committed);
    }
    return committed();
  }
  if (command === "list_conversations") {
    const workspaceId = String(args.workspaceId);
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
    return conversationSnapshot(String(args.workspaceId), state.richAvailability, rich.reference);
  }
  if (command === "get_rich_presentation") {
    if (state.richMode === "reject") return Promise.reject(new Error("test provider unavailable"));
    if (state.richMode === "defer") {
      const request = deferred<unknown>();
      state.richPending.push(request);
      return request.promise;
    }
    return rich.response;
  }
  if (command === "create_conversation") {
    const committed = commitConversationCreate(args);
    if (state.createMode === "lose-response") return Promise.reject(new Error("test lost response after commit"));
    if (state.createMode === "defer") {
      const request = deferred<unknown>();
      state.createPending.push(request);
      return request.promise;
    }
    return committed;
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
  resetResource: (deferredReads?: boolean) => true;
  selectResourceB: () => true;
  switchWorkspaceB: () => true;
  activeWorkspace: () => string;
  clearCalls: () => true;
  calls: () => Call[];
  configureConversation: (workspaceId: string, richMode: "success" | "defer" | "reject", availability: string) => true;
  setRichMode: (mode: "success" | "defer" | "reject") => true;
  resolveRich: () => true;
  setCreateMode: (mode: "success" | "defer" | "lose-response") => true;
  resolveCreate: () => true;
  switchConversationWorkspaceB: () => true;
  switchConversationWorkspaceA: () => true;
  configureConversationList: (mode: "single" | "51") => true;
  createCommitCount: () => number;
  resourcePendingCount: (resourceId: string) => number;
  resolveResource: (resourceId: string) => true;
  activateLibraryNavigationGuard: () => true;
  setResourceCommitMode: (mode: "success" | "defer" | "reject") => true;
  resolveResourceCommit: () => true;
  emitReady: () => Promise<true>;
  emitSnapshot: (availability: string, revision: number) => Promise<true>;
  configureWorkspaceFirstRun: (scenario: WorkspaceFirstRunScenario) => true;
  workspaceActionCounts: () => { startRuntime: number; refreshRuntime: number; refreshWorkspaces: number; createWorkspace: number };
  mountProductionApp: (scenario: WorkspaceAppScenario) => true;
  resolveInitialAppWorkspaceList: () => Promise<true>;
  rejectInitialAppWorkspaceList: () => Promise<true>;
  deferNextAppWorkspaceListRefresh: () => true;
  resolveStaleAppWorkspaceList: () => Promise<true>;
  resolveAppWorkspaceCreateReceipt: () => true;
  productionAppWorkspaceListCount: () => number;
  productionAppWorkspaceCreateCount: () => number;
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
  const [conversationMount, setConversationMount] = useState(0);
  const [showLibraryNavigationGuard, setShowLibraryNavigationGuard] = useState(false);
  const [workspaceFirstRunProps, setWorkspaceFirstRunProps] = useState<WorkspaceFirstRunProps | null>(null);
  const [showProductionApp, setShowProductionApp] = useState(false);
  const [productionAppMount, setProductionAppMount] = useState(0);

  window.litecoworkHarness = {
    resetResource(deferredReads = false) {
      state.resourceReadsDeferred = deferredReads;
      state.resourcePending.clear();
      setActiveWorkspace("workspace-a");
      setRequestedResource(resourceA);
      setResourceVisible(true);
      setResourceMount(value => value + 1);
      return true;
    },
    selectResourceB() { setRequestedResource(resourceB); return true; },
    setResourceReadsDeferred(value) { state.resourceReadsDeferred = value; state.resourcePending.clear(); return true; },
    resourcePendingCount(resourceId) { return state.resourcePending.get(resourceId)?.length ?? 0; },
    resolveResource(resourceId) {
      const pending = state.resourcePending.get(resourceId)?.shift();
      if (!pending) throw new Error(`No deferred Resource response exists for ${resourceId}.`);
      pending.resolve(resourceDetails[resourceId]);
      return true;
    },
    activateLibraryNavigationGuard() {
      state.resourceReadsDeferred = false;
      state.resourcePending.clear();
      resourceDetails["resource-a"] = {
        ...resourceDetails["resource-a"], currentRevisionId: "revision-a1", version: 1,
      };
      state.resourceCommitMode = "success";
      state.resourceCommitPending = [];
      setShowLibraryNavigationGuard(true);
      return true;
    },
    setResourceCommitMode(mode) { state.resourceCommitMode = mode; return true; },
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
    configureConversation(workspaceId, richMode, availability) {
      state.richMode = richMode;
      state.richAvailability = availability;
      state.richPending = [];
      state.createMode = "success";
      state.createPending = [];
      state.activeStream = null;
      setConversationWorkspace(workspaceId);
      setConversationMount(value => value + 1);
      return true;
    },
    configureConversationList(mode) { state.conversationListMode = mode; return true; },
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
    switchConversationWorkspaceB() { setConversationWorkspace("workspace-b"); return true; },
    switchConversationWorkspaceA() { setConversationWorkspace("workspace-a"); return true; },
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
            rich_presentation: { ...rich.reference, availability },
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
      state.appCreatedWorkspace = isF90UploadScenario(scenario) ? {
        workspaceId: "workspace-a",
        name: "Workspace A",
        replicationPolicy: "LOCAL_ONLY",
        defaultAgentBindingId: null,
        primaryCoworkerId: null,
        status: "ACTIVE",
        version: 1,
      } : null;
      state.appWorkspaceCreateReceipt = scenario === "workspace-list-race" ? deferred<WorkspaceAppWorkspace>() : null;
      state.appWorkspaceCreateCalls = 0;
      state.f90UploadSession = null;
      state.f90UploadCreates = 0;
      state.f90LoseNextChunkAck = isF90UploadScenario(scenario);
      state.f90CorruptNextRangeRead = scenario === "resource-upload-ambiguous-chunk-bad-range";
      state.f90CommittedResource = null;
      state.calls.length = 0;
      localStorage.removeItem("litecowork.selectedWorkspaceId");
      setShowLibraryNavigationGuard(false);
      setWorkspaceFirstRunProps(null);
      setShowProductionApp(true);
      setProductionAppMount(value => value + 1);
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
    async resolveStaleAppWorkspaceList() {
      const pending = state.appStaleWorkspaceList;
      if (!pending) throw new Error("There is no deferred stale Workspace list response.");
      state.appStaleWorkspaceList = null;
      pending.resolve([]);
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
    workspaceActionCounts: () => ({ ...state.workspaceActions }),
  };

  if (showLibraryNavigationGuard) return <LibraryNavigationFixture />;
  if (showProductionApp) return <App key={productionAppMount} />;
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
        operatorReady
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
