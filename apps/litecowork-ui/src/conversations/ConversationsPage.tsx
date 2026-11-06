import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { RichResponseView } from "../presentation/RichResponseView";
import { parseRichPresentationResponse, type ValidatedRichPresentation } from "../presentation/rich-presentation";
import type { PresentationStreamState, TransientTurn } from "../presentation/presentation-stream";
import { appendConversationListPage, appendConversationPage, conversationDraftScope, ConversationCreateRequestCache, ConversationRequestEpochs, ConversationWorkspaceSelectionEpochs, mergeConversationStreamSnapshot, parseConversationListPage, parseConversationList, parseConversationSnapshot, richPresentationCacheKey, type ConversationMessageView, type ConversationSnapshotView, type ConversationSummary, type PendingConversationCreate } from "./conversation-view";
import { ConversationStreamController, type ConversationStreamDriver, type StreamEnvelope } from "./conversation-stream";
import { buildConversationAgentChoices, type ConversationAgentBindingOption, type ConversationAgentProfileOption } from "./conversation-agent-selection";
import "./conversations-page.css";

// Keep ambiguous create receipts across keyed Workspace page remounts for this app session.
const pendingCreates = new ConversationCreateRequestCache();
const conversationWorkspaceSelections = new ConversationWorkspaceSelectionEpochs();

function messageText(message: ConversationMessageView): string {
  return message.content.map(block => block.kind === "TEXT" ? block.text : "Attachment").join("\n");
}

function semanticText(message: ConversationMessageView): string {
  return message.content.filter((block): block is { kind: "TEXT"; text: string } => block.kind === "TEXT").map(block => block.text).join("\n");
}

function ConversationMessageContent({ workspaceId, conversationId, message, autoLoadRich, presentationEpoch }: {
  workspaceId: string;
  conversationId: string;
  message: ConversationMessageView;
  autoLoadRich: boolean;
  presentationEpoch: number;
}) {
  const element = useRef<HTMLDivElement | null>(null);
  const [requestedKey, setRequestedKey] = useState<string | null>(null);
  const [retry, setRetry] = useState(0);
  const [loadState, setLoadState] = useState<{ key: string; status: "LOADING" | "READY" | "FAILED" } | null>(null);
  const [loadedPresentation, setLoadedPresentation] = useState<{ key: string; presentation: ValidatedRichPresentation } | null>(null);
  const reference = message.rich_presentation;
  const presentationId = reference?.presentation_id ?? null;
  const semanticContentDigest = reference?.semantic_content_digest ?? null;
  const text = semanticText(message);
  const immutableCacheKey = richPresentationCacheKey(workspaceId, conversationId, message.message_id, reference);
  const cacheKey = immutableCacheKey === null ? null : `${immutableCacheKey}\u0000${presentationEpoch}`;
  const requested = cacheKey !== null && requestedKey === cacheKey;
  const status = cacheKey === null || loadState?.key !== cacheKey ? "IDLE" : loadState.status;
  const presentation = cacheKey !== null && loadedPresentation?.key === cacheKey ? loadedPresentation.presentation : null;

  useEffect(() => {
    if (!autoLoadRich || cacheKey === null || !element.current || typeof IntersectionObserver === "undefined") return;
    const observer = new IntersectionObserver(entries => {
      if (entries.some(entry => entry.isIntersecting)) {
        setRequestedKey(current => current === cacheKey ? current : cacheKey);
        observer.disconnect();
      }
    }, { rootMargin: "320px" });
    observer.observe(element.current);
    return () => observer.disconnect();
  }, [autoLoadRich, cacheKey]);

  const shouldLoad = Boolean(cacheKey && requested);
  useEffect(() => {
    if (!shouldLoad || !reference || !presentationId || !semanticContentDigest || cacheKey === null || status !== "IDLE") return;
    let current = true;
    setLoadState({ key: cacheKey, status: "LOADING" });
    void invoke<unknown>("get_rich_presentation", {
      workspaceId,
      conversationId,
      messageId: message.message_id,
      presentationId,
      semanticContentDigest,
    }).then(async value => {
      const parsed = await parseRichPresentationResponse(value, {
        workspaceId,
        conversationId,
        messageId: message.message_id,
        presentationId: reference.presentation_id,
        schemaVersion: reference.schema_version,
        rendererContractVersion: reference.renderer_contract_version,
        semanticContentDigest,
        documentDigest: reference.document_digest,
        documentSizeBytes: reference.document_size_bytes,
      }, text);
      if (!current) return;
      const boundPresentation = parsed?.presentationId === presentationId ? parsed : null;
      if (boundPresentation) setLoadedPresentation({ key: cacheKey, presentation: boundPresentation });
      else setLoadedPresentation(currentValue => currentValue?.key === cacheKey ? null : currentValue);
      setLoadState({ key: cacheKey, status: boundPresentation ? "READY" : "FAILED" });
    }).catch(() => {
      if (current) setLoadState({ key: cacheKey, status: "FAILED" });
    });
    return () => { current = false; };
  // The key contains stable immutable bindings instead of the ref object or current
  // viewport eligibility. Live projection merges allocate fresh objects, and an
  // auto-load message may move out of the recent viewport set while its fetch is in
  // flight; neither event should strand its state at LOADING.
  // `status` is intentionally omitted: IDLE -> LOADING must not cancel itself.
  }, [cacheKey, conversationId, message.message_id, presentationId, requestedKey, retry, semanticContentDigest, text, workspaceId]);

  useEffect(() => {
    setLoadedPresentation(current => current?.key === cacheKey ? current : null);
    setLoadState(current => current?.key === cacheKey ? current : null);
    setRequestedKey(current => current === cacheKey ? current : null);
  }, [cacheKey]);

  if (!reference || message.role !== "AGENT") return <div className="conversation-message-body">{messageText(message)}</div>;
  const availability = reference.availability;
  const canFetch = availability === "AVAILABLE";
  return <div className="conversation-message-body" ref={element}>
    <RichResponseView semanticText={text || messageText(message)} presentation={presentation} />
    {!presentation && canFetch && status === "IDLE" && !shouldLoad && <button type="button" className="conversation-rich-load" onClick={() => cacheKey && setRequestedKey(cacheKey)}>Show formatted response</button>}
    {status === "LOADING" && <p className="conversation-rich-status" role="status">Loading formatted response…</p>}
    {status === "FAILED" && <p className="conversation-rich-status" role="note">Formatted view unavailable; the complete text response is still shown. <button type="button" onClick={() => { if (!cacheKey) return; setLoadState(null); setRequestedKey(cacheKey); setRetry(value => value + 1); }}>Retry</button></p>}
    {!canFetch && <p className="conversation-rich-status" role="note">Formatted view unavailable; the complete text response is still shown.</p>}
  </div>;
}

function timeLabel(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? "Saved conversation" : date.toLocaleString();
}

type ConversationAgentBindingPage = {
  items: ConversationAgentBindingOption[];
  nextCursor: string | null;
};

type ConversationAgentProfilePage = {
  items: ConversationAgentProfileOption[];
  nextCursor: string | null;
};

type ConversationAgentCatalogStatus = "LOADING" | "READY" | "FAILED" | "OFFLINE";
type ConversationAgentMutationNotice = {
  conversationId: string;
  status: "SAVING" | "SAVED" | "FAILED";
  message: string;
};

export function ConversationsPage({
  workspaceId,
  workspaceName,
  operatorReady,
  draft,
  onDraftChange,
  onOpenAgentSettings: _onOpenAgentSettings,
  workspaceDefaultAgentBindingId = null,
  selectedConversationId: externalSelectedId = null,
  onSelectConversation,
  onConversationsLoaded,
  onRegisterLoadMoreConversations,
  onConversationListPaginationChange,
  onToggleWorkbench,
  workbenchOpen = false,
}: {
  workspaceId: string;
  workspaceName: string;
  operatorReady: boolean;
  draft: string;
  onDraftChange: (value: string) => void;
  onOpenAgentSettings: () => void;
  workspaceDefaultAgentBindingId?: string | null;
  selectedConversationId?: string | null;
  onSelectConversation?: (id: string | null) => void;
  onConversationsLoaded?: (items: ConversationSummary[]) => void;
  onRegisterLoadMoreConversations?: (handler: (() => void) | null) => void;
  onConversationListPaginationChange?: (workspaceId: string, hasMore: boolean, loading: boolean) => void;
  onToggleWorkbench?: () => void;
  workbenchOpen?: boolean;
}) {
  const [items, setItems] = useState<ConversationSummary[]>([]);
  const [listCursor, setListCursor] = useState<string | null>(null);
  const [loadingMoreConversations, setLoadingMoreConversations] = useState(false);
  
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const selectedConversationId = useRef<string | null>(selectedId);
  const [conversationDrafts, setConversationDrafts] = useState<Record<string, string>>({});
  const [agentBindings, setAgentBindings] = useState<ConversationAgentBindingOption[]>([]);
  const [agentProfiles, setAgentProfiles] = useState<ConversationAgentProfileOption[]>([]);
  const [agentCatalogStatus, setAgentCatalogStatus] = useState<ConversationAgentCatalogStatus>("LOADING");
  const [_agentCatalogError, setAgentCatalogError] = useState<string | null>(null);
  const [agentCatalogAttempt, _setAgentCatalogAttempt] = useState(0);
  const [agentMutationNotice, setAgentMutationNotice] = useState<ConversationAgentMutationNotice | null>(null);
  const agentMutationLock = useRef(false);
  const [snapshot, setSnapshot] = useState<ConversationSnapshotView | null>(null);
  const [richPresentationEpochs, setRichPresentationEpochs] = useState<Map<string, number>>(() => new Map());
  const [title, setTitle] = useState("");
  const [busy, setBusy] = useState(false);
  const [_listStatus, setListStatus] = useState<"LOADING" | "READY" | "FAILED">("LOADING");
  const [createJournalStatus, setCreateJournalStatus] = useState<"LOADING" | "READY" | "FAILED">("LOADING");
  const [createJournalAttempt, setCreateJournalAttempt] = useState(0);
  const [, setPendingCreateRequests] = useState<PendingConversationCreate[]>([]);
  const [loadingMore, setLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [createReceiptError, setCreateReceiptError] = useState<{ workspaceId: string; requestId: string; message: string } | null>(null);
  const [snapshotLoadedKey, setSnapshotLoadedKey] = useState<string | null>(null);
  const [presentationStream, setPresentationStream] = useState<PresentationStreamState | null>(null);
  const [presentationStreamKey, setPresentationStreamKey] = useState<string | null>(null);
  const [streamStatus, setStreamStatus] = useState<"CONNECTING" | "LIVE" | "RECONNECTING" | null>(null);
  const requests = useRef(new ConversationRequestEpochs());
  const activeListEpoch = useRef<number | null>(null);
  const activeSnapshotEpoch = useRef<number | null>(null);
  const previousOperatorReady = useRef(operatorReady);
  const preserveOnReconnect = useRef(false);
  const snapshotLoadedKeyRef = useRef(snapshotLoadedKey);
  snapshotLoadedKeyRef.current = snapshotLoadedKey;
  const mounted = useRef(false);
  const activeWorkspace = useRef(workspaceId);
  const latestDraft = useRef(draft);
  latestDraft.current = draft;
  const pendingCreateDrafts = useRef(new Map<string, string>());
  activeWorkspace.current = workspaceId;
  selectedConversationId.current = selectedId;

  const streamBottomRef = useRef<HTMLDivElement>(null);
  const streamContainerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (externalSelectedId !== undefined && externalSelectedId !== null && externalSelectedId !== selectedId) {
      setSelectedId(externalSelectedId);
    }
  }, [externalSelectedId, selectedId]);

  useEffect(() => {
    onSelectConversation?.(selectedId);
  }, [selectedId, onSelectConversation]);
  useEffect(() => {
    if (!workspaceId || !operatorReady) {
      setAgentCatalogStatus("OFFLINE");
      return;
    }
    let current = true;
    const requestWorkspaceId = workspaceId;
    setAgentCatalogStatus("LOADING");
    setAgentCatalogError(null);
    void Promise.all([
      invoke<ConversationAgentBindingPage>("list_agent_bindings", { workspaceId: requestWorkspaceId }),
      invoke<ConversationAgentProfilePage>("list_agent_profiles", { workspaceId: requestWorkspaceId }),
    ]).then(([bindingPage, profilePage]) => {
      if (!current || activeWorkspace.current !== requestWorkspaceId) return;
      if (!bindingPage || !Array.isArray(bindingPage.items)
        || !profilePage || !Array.isArray(profilePage.items)) {
        throw new Error("The local Runtime returned an invalid agent catalog.");
      }
      setAgentBindings(bindingPage.items);
      setAgentProfiles(profilePage.items);
      setAgentCatalogStatus("READY");
    }).catch(cause => {
      if (!current || activeWorkspace.current !== requestWorkspaceId) return;
      setAgentCatalogStatus("FAILED");
      setAgentCatalogError(cause instanceof Error ? cause.message : String(cause));
    });
    return () => { current = false; };
  }, [agentCatalogAttempt, operatorReady, workspaceId]);
  const selectionKey = workspaceId && selectedId ? conversationDraftScope(workspaceId, selectedId) : null;
  const composerDraftKey = selectionKey;
  const composerDraft = composerDraftKey ? conversationDrafts[composerDraftKey] ?? "" : draft;

  useLayoutEffect(() => {
    conversationWorkspaceSelections.select(workspaceId);
  }, [workspaceId]);

  useEffect(() => {
    if (!operatorReady || !workspaceId) {
      setPendingCreateRequests([]);
      setCreateJournalStatus("READY");
      return;
    }
    let current = true;
    const requestWorkspaceId = workspaceId;
    setCreateJournalStatus("LOADING");
    void invoke<unknown>("list_pending_conversation_creates", {})
      .then(value => {
        if (!current || activeWorkspace.current !== requestWorkspaceId) return;
        setPendingCreateRequests(pendingCreates.replace(value));
        setCreateJournalStatus("READY");
      })
      .catch(cause => {
        if (!current || activeWorkspace.current !== requestWorkspaceId) return;
        setCreateJournalStatus("FAILED");
        setError(cause instanceof Error ? cause.message : String(cause));
      });
    return () => { current = false; };
  }, [createJournalAttempt, operatorReady, workspaceId]);

  const refresh = useCallback(async (preserveCurrent = false) => {
    const current = requests.current.beginList();
    activeListEpoch.current = current;
    setLoadingMoreConversations(false);
    if (!operatorReady || !workspaceId) {
      setBusy(false);
      if (!workspaceId) {
        setItems([]);
        setListCursor(null);
        setLoadingMoreConversations(false);
        setSnapshot(null);
        setSnapshotLoadedKey(null);
        setSelectedId(null);
        setListStatus("READY");
      }
      return;
    }
    if (!preserveCurrent) {
      setListCursor(null);
      setSnapshot(null);
      setSnapshotLoadedKey(null);
      setSelectedId(null);
    }
    setListStatus("LOADING");
    setBusy(true); setError(null);
    try {
      const response = await invoke<unknown>("list_conversations", { workspaceId, cursor: null });
      if (activeWorkspace.current !== workspaceId || !requests.current.isCurrentList(current)) return;
      const page = parseConversationListPage(response, workspaceId);
      setItems(page.items);
      onConversationsLoaded?.(page.items);
      setListCursor(page.next_cursor);
      setLoadingMoreConversations(false);
      if (!externalSelectedId) {
        setSelectedId(page.items[0]?.conversation_id ?? null);
      }
      setListStatus("READY");
    } catch (cause) {
      if (activeWorkspace.current === workspaceId && requests.current.isCurrentList(current)) {
        setListStatus("FAILED");
        setError(cause instanceof Error ? cause.message : String(cause));
      }
    } finally { if (activeWorkspace.current === workspaceId && requests.current.isCurrentList(current)) setBusy(false); }
  }, [operatorReady, workspaceId]);

  const loadMoreConversations = useCallback(async () => {
    const cursor = listCursor;
    const current = activeListEpoch.current;
    if (!cursor || !operatorReady || loadingMoreConversations || current === null
      || !requests.current.isCurrentList(current)) return;

    const requestWorkspaceId = workspaceId;
    setLoadingMoreConversations(true);
    try {
      const value = await invoke<unknown>("list_conversations", { workspaceId: requestWorkspaceId, cursor });
      if (activeWorkspace.current !== requestWorkspaceId || !requests.current.isCurrentList(current)) return;
      const page = parseConversationListPage(value, requestWorkspaceId);
      if (page.next_cursor === cursor) {
        throw new Error("Conversation list did not advance its pagination cursor.");
      }
      const merged = appendConversationListPage({ items, next_cursor: cursor }, page);
      setItems(merged.items);
      onConversationsLoaded?.(merged.items);
      setListCursor(page.next_cursor);
    } catch (cause) {
      if (activeWorkspace.current === requestWorkspaceId && requests.current.isCurrentList(current)) {
        setError(cause instanceof Error ? cause.message : String(cause));
      }
    } finally {
      if (activeWorkspace.current === requestWorkspaceId && requests.current.isCurrentList(current)) {
        setLoadingMoreConversations(false);
      }
    }
  }, [items, listCursor, loadingMoreConversations, onConversationsLoaded, operatorReady, workspaceId]);

  useEffect(() => {
    onRegisterLoadMoreConversations?.(loadMoreConversations);
    return () => onRegisterLoadMoreConversations?.(null);
  }, [loadMoreConversations, onRegisterLoadMoreConversations]);

  useEffect(() => {
    onConversationListPaginationChange?.(workspaceId, listCursor !== null, loadingMoreConversations);
  }, [listCursor, loadingMoreConversations, onConversationListPaginationChange, workspaceId]);

  useEffect(() => {
    const reconnecting = !previousOperatorReady.current && operatorReady;
    previousOperatorReady.current = operatorReady;
    preserveOnReconnect.current = reconnecting;
    void refresh(reconnecting);
    return () => { requests.current.invalidateList(); };
  }, [operatorReady, refresh]);

  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);

  
  useEffect(() => {
    const current = requests.current.beginSnapshot();
    activeSnapshotEpoch.current = current;
    setLoadingMore(false);
    if (!operatorReady) {
      setBusy(false);
      if (snapshotLoadedKeyRef.current !== selectionKey) {
        setSnapshot(null);
        setSnapshotLoadedKey(null);
      }
      return () => { requests.current.invalidateSnapshot(); };
    }
    if (!preserveOnReconnect.current || snapshotLoadedKeyRef.current !== selectionKey) {
      setSnapshot(null);
      setSnapshotLoadedKey(null);
    }
    preserveOnReconnect.current = false;
    if (!workspaceId || !selectedId) return () => { requests.current.invalidateSnapshot(); };
    setBusy(true); setError(null);
    void invoke<unknown>("get_conversation_presentation", { workspaceId, conversationId: selectedId })
      .then(value => {
        if (activeWorkspace.current !== workspaceId || !requests.current.isCurrentSnapshot(current)) return;
        setSnapshot(parseConversationSnapshot(value, workspaceId, selectedId));
        setSnapshotLoadedKey(`${workspaceId}\u0000${selectedId}`);
      })
      .catch(cause => { if (activeWorkspace.current === workspaceId && requests.current.isCurrentSnapshot(current)) setError(cause instanceof Error ? cause.message : String(cause)); })
      .finally(() => { if (activeWorkspace.current === workspaceId && requests.current.isCurrentSnapshot(current)) setBusy(false); });
    return () => {
      requests.current.invalidateSnapshot();
      if (activeSnapshotEpoch.current === current) activeSnapshotEpoch.current = null;
    };
  }, [operatorReady, selectedId, workspaceId]);

  useEffect(() => {
    if (!operatorReady || !selectionKey || snapshotLoadedKey !== selectionKey) {
      setPresentationStream(null);
      setPresentationStreamKey(null);
      setStreamStatus(null);
      return;
    }
    setPresentationStream(null);
    setPresentationStreamKey(selectionKey);
    const driver: ConversationStreamDriver = {
      listen: async callback => listen<unknown>("conversation-presentation-stream", event => callback(event.payload as StreamEnvelope)),
      start: input => invoke<void>("subscribe_conversation_stream", input),
      acknowledge: (sessionId, frameId) => invoke<void>("acknowledge_conversation_stream_frame", { sessionId, frameId }),
      stop: sessionId => invoke<void>("unsubscribe_conversation_stream", { sessionId }),
      newSessionId: () => crypto.randomUUID(),
    };
    const controller = new ConversationStreamController(workspaceId, selectedId!, driver, (state, frame) => {
      setPresentationStream(state);
      setPresentationStreamKey(selectionKey);
      const live = state.conversationSnapshot;
      if (frame && typeof frame === "object" && !Array.isArray(frame)) {
        const projectionFrame = frame as Record<string, unknown>;
        if (projectionFrame.type === "projection" && projectionFrame.projection_type === "conversation_presentation") {
          const invalidatedMessageIds = live?.messages
            .filter(message => message.rich_presentation !== null && message.rich_presentation.availability !== "AVAILABLE")
            .map(message => message.message_id) ?? [];
          if (invalidatedMessageIds.length > 0) {
            setRichPresentationEpochs(current => {
              const next = new Map(current);
              for (const messageId of invalidatedMessageIds) next.set(messageId, (next.get(messageId) ?? 0) + 1);
              return next;
            });
          }
        }
      }
      if (live && activeWorkspace.current === workspaceId) {
        setSnapshot(current => mergeConversationStreamSnapshot(current, live, workspaceId, selectedId!));
      }
    }, setStreamStatus);
    controller.start();
    setStreamStatus(controller.status);
    return () => { void controller.dispose(); };
  }, [operatorReady, selectionKey, selectedId, snapshotLoadedKey, workspaceId]);

  const loadMoreMessages = async () => {
    if (!snapshot?.next_cursor || !selectedId || loadingMore) return;
    const current = activeSnapshotEpoch.current;
    if (current === null || !requests.current.isCurrentSnapshot(current)) return;
    const requestWorkspaceId = workspaceId;
    const cursor = snapshot.next_cursor;
    setLoadingMore(true);
    try {
      const value = await invoke<unknown>("get_conversation_presentation", { workspaceId: requestWorkspaceId, conversationId: selectedId, cursor });
      if (activeWorkspace.current !== requestWorkspaceId || !requests.current.isCurrentSnapshot(current)) return;
      const page = parseConversationSnapshot(value, requestWorkspaceId, selectedId);
      if (page.next_cursor === cursor) throw new Error("Conversation history did not advance its pagination cursor.");
      setSnapshot(existing => existing?.workspace_id === requestWorkspaceId && existing.conversation_id === selectedId ? appendConversationPage(existing, page) : existing);
    } catch (cause) {
      if (activeWorkspace.current === requestWorkspaceId && requests.current.isCurrentSnapshot(current)) setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      if (activeWorkspace.current === requestWorkspaceId && requests.current.isCurrentSnapshot(current)) setLoadingMore(false);
    }
  };

  const submitCreate = async (request: PendingConversationCreate) => {
    if (!operatorReady || busy || createJournalStatus !== "READY") return;
    const createDraftKey = request.workspaceId + "" + request.requestId;
    const requestSelection = conversationWorkspaceSelections.capture(request.workspaceId);
    if (!requestSelection) return;
    const isCurrentCreateRequest = () => mounted.current
      && activeWorkspace.current === request.workspaceId
      && conversationWorkspaceSelections.isCurrent(requestSelection);
    const clearCreateReceiptError = () => setCreateReceiptError(current => current?.workspaceId === request.workspaceId && current.requestId === request.requestId ? null : current);
    clearCreateReceiptError();
    setBusy(true); setError(null);
    try {
      const response = await invoke<unknown>("create_conversation", {
        workspaceId: request.workspaceId, title: request.title, requestId: request.requestId,
      });
      if (!isCurrentCreateRequest()) return;
      // A create receipt is one summary, not a paginated list response. Wrap it as
      // a terminal one-item page so the strict bridge parser still sees its required
      // next_cursor field without accepting malformed list pages elsewhere.
      const created = parseConversationList({ items: [response], next_cursor: null }, request.workspaceId)[0];
      if (!created) throw new Error("Local Runtime returned an invalid Conversation.");
      let cleanupError: string | null = null;
      try {
        await invoke<void>("acknowledge_conversation_create", {
          workspaceId: request.workspaceId,
          requestId: request.requestId,
        });
        clearCreateReceiptError();
        pendingCreates.completeExact(request.workspaceId, request.requestId);
        if (mounted.current) {
          setPendingCreateRequests(current => current.filter(item =>
            item.workspaceId !== request.workspaceId || item.requestId !== request.requestId));
        }
      } catch (cause) {
        cleanupError = cause instanceof Error ? cause.message : String(cause);
        pendingCreates.remember(request);
        if (isCurrentCreateRequest()) {
          setCreateReceiptError({
            workspaceId: request.workspaceId,
            requestId: request.requestId,
            message: `Conversation created, but its retry receipt could not be cleared: ${cleanupError}. Retrying this pending request is safe.`,
          });
          setPendingCreateRequests(current => current.some(item =>
            item.workspaceId === request.workspaceId && item.requestId === request.requestId)
            ? current
            : [...current, request]);
        }
      }
      if (!isCurrentCreateRequest()) return;
      setTitle("");
      const carriedDraft = pendingCreateDrafts.current.get(createDraftKey);
      if (carriedDraft) {
        const draftKey = conversationDraftScope(request.workspaceId, created.conversation_id);
        setConversationDrafts(current => ({ ...current, [draftKey]: carriedDraft }));
        if (latestDraft.current === carriedDraft) onDraftChange("");
      }
      pendingCreateDrafts.current.delete(createDraftKey);
      setItems(current => {
        const nextList = [created, ...current.filter(item => item.conversation_id !== created.conversation_id)];
        onConversationsLoaded?.(nextList);
        return nextList;
      });
      setSelectedId(created.conversation_id);
      setSnapshot({ workspace_id: request.workspaceId, conversation_id: created.conversation_id, messages: [], next_cursor: null, active_turn: null });
      setSnapshotLoadedKey(`${request.workspaceId}\u0000${created.conversation_id}`);
    } catch (cause) {
      if (!isCurrentCreateRequest()) return;
      const createError = cause instanceof Error ? cause.message : String(cause);
      setError(createError);
      try {
        const journal = await invoke<unknown>("list_pending_conversation_creates", {});
        if (!isCurrentCreateRequest()) return;
        setPendingCreateRequests(pendingCreates.replace(journal));
        setCreateJournalStatus("READY");
      } catch (journalCause) {
        if (!isCurrentCreateRequest()) return;
        pendingCreates.remember(request);
        setPendingCreateRequests(current => current.some(item =>
          item.workspaceId === request.workspaceId && item.requestId === request.requestId)
          ? current
          : [...current, request]);
        setCreateJournalStatus("FAILED");
        const journalError = journalCause instanceof Error ? journalCause.message : String(journalCause);
        setError(`${createError} Pending retry state could not be refreshed: ${journalError}`);
      }
    } finally {
      if (isCurrentCreateRequest()) setBusy(false);
    }
  };

  const create = async (requestedTitle?: string) => {
    if (!operatorReady || busy || createJournalStatus !== "READY") return;
    const normalizedTitle = requestedTitle === undefined ? title.trim() || null : requestedTitle.trim() || null;
    try {
      const request = pendingCreates.get(workspaceId, normalizedTitle, () => crypto.randomUUID());
      if (!selectedId && draft.trim()) {
        pendingCreateDrafts.current.set(workspaceId + "" + request.requestId, draft);
      }
      void submitCreate(request);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  };

  
  const selected = items.find(item => item.conversation_id === selectedId) ?? null;
  const agentChoices = buildConversationAgentChoices(
    workspaceId,
    agentBindings,
    agentProfiles,
    selected?.active_agent_binding_id ?? null,
  );
  const workspaceDefaultChoice = workspaceDefaultAgentBindingId
    ? agentChoices.find(choice => choice.agentBindingId === workspaceDefaultAgentBindingId && choice.selectable)
    : undefined;
  const workspaceDefaultOptionLabel = workspaceDefaultAgentBindingId === null
    ? "Workspace default (not configured)"
    : workspaceDefaultChoice
      ? `Workspace default · ${workspaceDefaultChoice.label}`
      : "Workspace default (binding details unavailable)";
  const visibleAgentMutationNotice = selected && agentMutationNotice?.conversationId === selected.conversation_id
    ? agentMutationNotice
    : null;
  const saveConversationAgentBinding = async (agentBindingId: string | null) => {
    const conversationId = selected?.conversation_id;
    if (!conversationId || !operatorReady || agentMutationLock.current) return;
    agentMutationLock.current = true;
    const requestWorkspaceId = workspaceId;
    const requestConversationId = conversationId;
    setAgentMutationNotice({
      conversationId,
      status: "SAVING",
      message: "Saving this Conversation’s agent selection…",
    });
    const parseRecord = (value: unknown): ConversationSummary => {
      const record = parseConversationList({ items: [value], next_cursor: null }, requestWorkspaceId)[0];
      if (!record || record.conversation_id !== requestConversationId) {
        throw new Error("The local Runtime returned a different Conversation.");
      }
      return record;
    };
    const applyRecord = (record: ConversationSummary) => {
      setItems(current => current.map(item => item.conversation_id === requestConversationId ? record : item));
    };
    const setNoticeIfSelected = (notice: ConversationAgentMutationNotice) => {
      if (activeWorkspace.current === requestWorkspaceId
        && selectedConversationId.current === requestConversationId) {
        setAgentMutationNotice(notice);
      }
    };
    try {
      const currentRecord = parseRecord(await invoke<unknown>("get_conversation", {
        workspaceId: requestWorkspaceId,
        conversationId: requestConversationId,
      }));
      if (currentRecord.active_agent_binding_id === agentBindingId) {
        applyRecord(currentRecord);
        setNoticeIfSelected({
          conversationId,
          status: "SAVED",
          message: "This Conversation already uses that agent selection.",
        });
        return;
      }
      const savedRecord = parseRecord(await invoke<unknown>("set_conversation_agent_binding", {
        workspaceId: requestWorkspaceId,
        conversationId: requestConversationId,
        expectedVersion: currentRecord.version,
        agentBindingId,
        requestId: crypto.randomUUID(),
      }));
      if (savedRecord.active_agent_binding_id !== agentBindingId
        || savedRecord.version !== currentRecord.version + 1) {
        throw new Error("The agent selection response did not match the requested change.");
      }
      applyRecord(savedRecord);
      setNoticeIfSelected({
        conversationId,
        status: "SAVED",
        message: "Saved for future turns. Any running session keeps its existing agent.",
      });
    } catch (cause) {
      let latest: ConversationSummary | null = null;
      try {
        latest = parseRecord(await invoke<unknown>("get_conversation", {
          workspaceId: requestWorkspaceId,
          conversationId: requestConversationId,
        }));
        applyRecord(latest);
      } catch {
        // Preserve the last committed UI state when readback is also unavailable.
      }
      if (latest?.active_agent_binding_id === agentBindingId) {
        setNoticeIfSelected({
          conversationId,
          status: "SAVED",
          message: "The selection was saved; the response was confirmed by reloading this Conversation.",
        });
      } else {
        const detail = cause instanceof Error && cause.message.trim()
          ? cause.message
          : "The change could not be confirmed.";
        setNoticeIfSelected({
          conversationId,
          status: "FAILED",
          message: latest
            ? "A newer Conversation version was loaded. Review the current selection and try again."
            : `Agent selection was not confirmed. ${detail}`,
        });
      }
    } finally {
      agentMutationLock.current = false;
    }
  };
  const currentSnapshot = snapshotLoadedKey === selectionKey && snapshot?.workspace_id === workspaceId && snapshot?.conversation_id === selectedId ? snapshot : null;
  const currentPresentationStream = presentationStreamKey === selectionKey
    && presentationStream?.workspaceId === workspaceId
    ? presentationStream
    : null;
  const transientTurns = Object.values(currentPresentationStream?.turns ?? {})
    .filter((turn): turn is TransientTurn => turn.conversationId === selectedId)
    .sort((a, b) => a.turnId.localeCompare(b.turnId) || a.retryOrdinal - b.retryOrdinal);
  const automaticRichIds = new Set((currentSnapshot?.messages ?? []).filter(message => message.role === "AGENT" && message.rich_presentation?.availability === "AVAILABLE").slice(-6).map(message => message.message_id));
  const activeStatus = currentSnapshot?.active_turn?.status;
  const [sendingTurn, setSendingTurn] = useState(false);
  const [turnError, setTurnError] = useState<string | null>(null);

  const isTurnRunning = transientTurns.some(t => t.state === "STREAMING" || t.state === "RECONNECTING") || activeStatus === "RUNNING";

  const sendTurn = async () => {
    const text = composerDraft.trim();
    if (!text || !workspaceId || !operatorReady || sendingTurn || isTurnRunning) return;
    if (!selectedId) {
      void create(text.slice(0, 36));
      return;
    }

    setSendingTurn(true);
    setTurnError(null);
    const draftKey = composerDraftKey;
    const reqId = typeof crypto !== "undefined" && crypto.randomUUID ? crypto.randomUUID() : `turn-req-${Date.now()}`;

    if (draftKey) {
      setConversationDrafts(current => ({ ...current, [draftKey]: "" }));
    }

    const optimisticMessage: ConversationMessageView = {
      message_id: `user-msg-${Date.now()}`,
      role: "USER",
      content: [{ kind: "TEXT", text }],
      created_at: new Date().toISOString(),
      rich_presentation: null,
    };
    setSnapshot(current => current && current.conversation_id === selectedId ? {
      ...current,
      messages: [...current.messages, optimisticMessage],
    } : current);

    try {
      await invoke<unknown>("submit_conversation_turn", {
        workspaceId,
        conversationId: selectedId,
        requestId: reqId,
        content: [{ kind: "TEXT", text }],
        resourceRefs: [],
        agentBindingId: selected?.active_agent_binding_id || null,
        presentationPreference: "RICH",
      });
    } catch (cause) {
      const msg = cause instanceof Error ? cause.message : String(cause);
      setTurnError(msg);
      if (draftKey) {
        setConversationDrafts(current => ({ ...current, [draftKey]: text }));
      }
    } finally {
      setSendingTurn(false);
    }
  };

  const cancelTurn = async () => {
    if (!selectedId || !workspaceId || !operatorReady) return;
    const turn = transientTurns.find(t => t.state === "STREAMING" || t.state === "RECONNECTING");
    if (!turn) return;
    const reqId = typeof crypto !== "undefined" && crypto.randomUUID ? crypto.randomUUID() : `cancel-req-${Date.now()}`;
    try {
      await invoke<unknown>("cancel_conversation_turn", {
        workspaceId,
        conversationId: selectedId,
        turnId: turn.turnId,
        expectedVersion: 1,
        requestId: reqId,
      });
    } catch (cause) {
      setTurnError(cause instanceof Error ? cause.message : String(cause));
    }
  };
  const statusLabel = activeStatus === "RUNNING" ? "A conversation turn is running"
    : activeStatus === "WAITING_USER" ? "Waiting for your response"
      : activeStatus === "WAITING_DEPENDENCY" ? "Waiting for a service"
        : activeStatus === "CANCEL_REQUESTED" ? "Stopping"
          : activeStatus === "OPEN" ? "Preparing"
            : null;
  const streamStatusLabel = currentPresentationStream?.resyncRequired ? "Refreshing saved conversation updates…"
    : streamStatus === "LIVE" ? "Saved history updates live"
    : streamStatus === "RECONNECTING" ? "Reconnecting saved-history updates…"
      : streamStatus === "CONNECTING" ? "Connecting saved-history updates…"
        : null;
    
  return (
    <div className="conversations-page-modern">
      {/* Top Breadcrumbs & Window Actions Bar */}
      <header className="conversation-top-breadcrumbs">
        <span className="crumb-workspace">{workspaceName}</span>
        <span className="crumb-sep">/</span>
        <strong className="crumb-title">{selected?.title ?? "AI Capabilities Overview"}</strong>
        {statusLabel && <span className="conversation-header-badge">{statusLabel}</span>}
        {streamStatusLabel && <span className="conversation-header-badge">{streamStatusLabel}</span>}
        {(error || createReceiptError) && <span className="conversation-header-badge is-error">{error || createReceiptError?.message}</span>}
        <div className="conversation-top-actions">
          <button className="conversation-action-icon-btn" type="button" title="More options">
            <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
              <circle cx="12" cy="5" r="2" />
              <circle cx="12" cy="12" r="2" />
              <circle cx="12" cy="19" r="2" />
            </svg>
          </button>
          <button
            className="conversation-action-icon-btn"
            type="button"
            onClick={onToggleWorkbench}
            title={workbenchOpen ? "Close side pane" : "Open side pane"}
          >
            <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
              <rect x="3" y="3" width="18" height="18" rx="2" />
              <path d="M15 3v18" />
            </svg>
          </button>
        </div>
      </header>

      {createJournalStatus === "FAILED" && (
        <div className="conversation-create-journal-error" role="alert">
          <p>Pending Conversation requests could not be checked. New Chat is paused to avoid duplicating an uncertain create request.</p>
          <button
            className="quiet-button"
            type="button"
            disabled={!operatorReady}
            onClick={() => {
              setCreateJournalStatus("LOADING");
              setCreateJournalAttempt(current => current + 1);
            }}
          >
            Retry checking pending Conversation requests
          </button>
        </div>
      )}

      {/* Main Conversation Canvas */}
      <div className="conversation-stream-container" ref={streamContainerRef}>
        <div className="conversation-messages-flow">
          {(currentSnapshot?.messages && currentSnapshot.messages.length > 0) || transientTurns.length > 0 ? (
            <>
              {currentSnapshot?.next_cursor && (
                <button
                  type="button"
                  className="conversation-load-more"
                  onClick={() => void loadMoreMessages()}
                  disabled={!operatorReady || busy || loadingMore}
                >
                  {loadingMore ? "Loading messages..." : "Load earlier messages"}
                </button>
              )}

              {(currentSnapshot?.messages ?? []).map(message => (
                <article className={"message-row " + message.role.toLowerCase()} key={message.message_id}>
                  {message.role === "USER" ? (
                    <div className="message-user-wrap">
                      <div className="message-user-bubble">{messageText(message)}</div><time className="message-time">{timeLabel(message.created_at)}</time>
                    </div>
                  ) : (
                    <div className="message-assistant-wrap">
                      <div className="message-assistant-body">
                        <ConversationMessageContent
                          workspaceId={workspaceId}
                          conversationId={selected?.conversation_id ?? ""}
                          message={message}
                          autoLoadRich={automaticRichIds.has(message.message_id)}
                          presentationEpoch={richPresentationEpochs.get(message.message_id) ?? 0}
                        />
                      </div>
                    </div>
                  )}
                </article>
              ))}

              {transientTurns.map(turn => (
                <article
                  className="message-row agent transient"
                  key={`${turn.conversationId}\0${turn.turnId}\0${turn.retryOrdinal}`}
                >
                  <div className="message-assistant-wrap">
                    <div className="message-assistant-body">
                      {turn.text}
                      <span className="assistant-streaming-cursor" />
                    </div>
                  </div>
                </article>
              ))}
            </>
          ) : (
            <div className="message-assistant-wrap">
              <div className="message-assistant-body">
                <p>
                  I am <strong>Antigravity</strong>, an advanced agentic AI coding and engineering assistant. I can help
                  you with software engineering, codebase exploration, automation, system design, debugging, and project
                  management tasks.
                </p>
                <p>Here is a summary of what I can do:</p>
                <h3>1. Codebase Exploration &amp; Analysis</h3>
                <ul>
                  <li>Search, read, and understand existing codebases across languages and architectures.</li>
                  <li>Trace bugs, performance bottlenecks, and architectural seams.</li>
                  <li>Review pull requests, commits, and diffs for code standards and functional specifications.</li>
                </ul>
                <h3>2. Full-Cycle Software Development</h3>
                <ul>
                  <li>Design, author, and refactor features with clean typing, error handling, and tests.</li>
                  <li>Execute verifiable commands, run test suites, and inspect terminal output.</li>
                  <li>Orchestrate multi-step plans with strict verification and zero hallucinations.</li>
                </ul>
              </div>
            </div>
          )}
        </div>

        {/* Scroll to bottom button */}
        <div className="scroll-to-bottom-wrap">
          <button
            className="scroll-to-bottom-btn"
            type="button"
            onClick={() => streamBottomRef.current?.scrollIntoView({ behavior: "smooth" })}
            title="Scroll to bottom"
          >
            ↓
          </button>
        </div>
        <div ref={streamBottomRef} />
      </div>

      {/* Floating Composer Card */}
      <div className="floating-composer-card">
        {turnError && <p className="composer-turn-error" role="alert">{turnError}</p>}
        {visibleAgentMutationNotice && <p className="composer-notice">{visibleAgentMutationNotice.message}</p>}
        <textarea
          className="composer-textarea"
          aria-label="Message draft"
          placeholder={isTurnRunning ? "Assistant is responding..." : "Ask anything, @ to mention, / for actions"}
          value={composerDraft}
          disabled={isTurnRunning}
          onChange={event => {
            if (!composerDraftKey) return;
            setConversationDrafts(current => ({ ...current, [composerDraftKey]: event.target.value }));
          }}
          onKeyDown={event => {
            if (event.key === "Enter" && !event.shiftKey) {
              event.preventDefault();
              void sendTurn();
            }
          }}
          rows={1}
        />
        <div className="composer-footer-toolbar">
          <div className="composer-footer-left">
            <button className="composer-pill-btn" type="button" title="Add context or file">
              +
            </button>
            <div className="model-selector-pill" title="Active Model">
              <div className="agent-select-wrapper">
                <select
                  value={selected?.active_agent_binding_id ?? ""}
                  disabled={!operatorReady || agentCatalogStatus === "LOADING" || agentMutationNotice?.status === "SAVING"}
                  onChange={event => selected && void saveConversationAgentBinding(event.target.value || null)}
                >
                  <option value="">{workspaceDefaultOptionLabel || "Gemini 3.8 Flash Low"}</option>
                  {agentChoices.map(choice => (
                    <option key={choice.agentBindingId} value={choice.agentBindingId} disabled={!choice.selectable}>
                      {choice.label}
                    </option>
                  ))}
                </select>
              </div>
              <span className="pill-chevron">⌄</span>
            </div>
          </div>

          <div className="composer-footer-right">
            <div className="mode-selector-pill" title="Execution Mode">
              <span className="mode-icon">⚡</span>
              <span>Turbo</span>
              <span className="pill-chevron">⌄</span>
            </div>
            <button className="composer-icon-btn" type="button" title="Voice Input">
              <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.8">
                <path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z" />
                <path d="M19 10v2a7 7 0 0 1-14 0v-2" />
                <line x1="12" y1="19" x2="12" y2="23" />
                <line x1="8" y1="23" x2="16" y2="23" />
              </svg>
            </button>
            {isTurnRunning ? (
              <button
                className="composer-submit-btn is-running"
                type="button"
                onClick={() => void cancelTurn()}
                title="Stop responding"
              >
                ■
              </button>
            ) : (
              <button
                className="composer-submit-btn"
                type="button"
                disabled={!composerDraft.trim() || !operatorReady || sendingTurn}
                onClick={() => void sendTurn()}
                title="Send message"
              >
                ✦
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};