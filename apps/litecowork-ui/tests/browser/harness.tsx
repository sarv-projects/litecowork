import { useRef, useState } from "react";
import ReactDOM from "react-dom/client";
import { emit } from "@tauri-apps/api/event";
import { mockIPC } from "@tauri-apps/api/mocks";
import { ConversationsPage } from "../../src/conversations/ConversationsPage";
import { ResourceRevisionEditor } from "../../src/resources/ResourceRevisionEditor";
import { LibraryNavigationDialog, libraryNavigationMainIsInert, useLibraryNavigation } from "../../src/resources/library-navigation";
import "../../src/styles.css";

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
  frameId: 0,
};

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

mockIPC((rawCommand, rawArgs) => {
  const args = record(rawCommand, rawArgs);
  const command = rawCommand;
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
  };

  if (showLibraryNavigationGuard) return <LibraryNavigationFixture />;

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
