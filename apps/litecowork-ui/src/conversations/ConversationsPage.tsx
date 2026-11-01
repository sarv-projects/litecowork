import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { RichResponseView } from "../presentation/RichResponseView";
import { parseRichPresentationResponse, type ValidatedRichPresentation } from "../presentation/rich-presentation";
import type { PresentationStreamState, TransientTurn } from "../presentation/presentation-stream";
import { appendConversationListPage, appendConversationPage, conversationDraftScope, ConversationCreateRequestCache, ConversationRequestEpochs, ConversationWorkspaceSelectionEpochs, mergeConversationStreamSnapshot, parseConversationListPage, parseConversationList, parseConversationSnapshot, richPresentationCacheKey, type ConversationMessageView, type ConversationSnapshotView, type ConversationSummary, type PendingConversationCreate } from "./conversation-view";
import { ConversationStreamController, type ConversationStreamDriver, type StreamEnvelope } from "./conversation-stream";
import { deriveConversationAgentSurface } from "./conversation-agent-surface";
import { buildConversationAgentChoices, type ConversationAgentBindingOption, type ConversationAgentProfileOption } from "./conversation-agent-selection";
import { firstUseConversationGuidance } from "./first-use-guidance";
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

export function ConversationsPage({ workspaceId, workspaceName, operatorReady, draft, onDraftChange, onOpenAgentSettings, workspaceDefaultAgentBindingId = null }: {
  workspaceId: string;
  workspaceName: string;
  operatorReady: boolean;
  draft: string;
  onDraftChange: (value: string) => void;
  onOpenAgentSettings: () => void;
  workspaceDefaultAgentBindingId?: string | null;
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
  const [agentCatalogError, setAgentCatalogError] = useState<string | null>(null);
  const [agentCatalogAttempt, setAgentCatalogAttempt] = useState(0);
  const [agentMutationNotice, setAgentMutationNotice] = useState<ConversationAgentMutationNotice | null>(null);
  const agentMutationLock = useRef(false);
  const [snapshot, setSnapshot] = useState<ConversationSnapshotView | null>(null);
  const [richPresentationEpochs, setRichPresentationEpochs] = useState<Map<string, number>>(() => new Map());
  const [title, setTitle] = useState("");
  const [busy, setBusy] = useState(false);
  const [listStatus, setListStatus] = useState<"LOADING" | "READY" | "FAILED">("LOADING");
  const [createJournalStatus, setCreateJournalStatus] = useState<"LOADING" | "READY" | "FAILED">("LOADING");
  const [createJournalAttempt, setCreateJournalAttempt] = useState(0);
  const [pendingCreateRequests, setPendingCreateRequests] = useState<PendingConversationCreate[]>([]);
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
  const composerSurface = deriveConversationAgentSurface({
    workspaceId,
    conversationId: selectedId ?? "",
    agentBindingId: "",
    descriptor: null,
    descriptorObservedAt: "",
    enabled: true,
    leadEligible: true,
    draft: composerDraft,
    attachments: [],
    nativeTokens: [],
    selectedOptions: {},
  }, Date.now());

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
      setListCursor(page.next_cursor);
      setSelectedId(page.items[0]?.conversation_id ?? null);
      setListStatus("READY");
    } catch (cause) {
      if (activeWorkspace.current === workspaceId && requests.current.isCurrentList(current)) {
        setListStatus("FAILED");
        setError(cause instanceof Error ? cause.message : String(cause));
      }
    } finally { if (activeWorkspace.current === workspaceId && requests.current.isCurrentList(current)) setBusy(false); }
  }, [operatorReady, workspaceId]);

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

  const loadMoreConversations = async () => {
    if (!listCursor || loadingMoreConversations || !workspaceId) return;
    const current = activeListEpoch.current;
    if (current === null || !requests.current.isCurrentList(current)) return;
    const requestWorkspaceId = workspaceId;
    const cursor = listCursor;
    setLoadingMoreConversations(true);
    try {
      const response = await invoke<unknown>("list_conversations", { workspaceId: requestWorkspaceId, cursor });
      if (activeWorkspace.current !== requestWorkspaceId || !requests.current.isCurrentList(current)) return;
      const page = parseConversationListPage(response, requestWorkspaceId);
      if (page.next_cursor === cursor) throw new Error("Conversation catalog did not advance its pagination cursor.");
      setItems(existing => appendConversationListPage({ items: existing, next_cursor: cursor }, page).items);
      setListCursor(page.next_cursor);
    } catch (cause) {
      if (activeWorkspace.current === requestWorkspaceId && requests.current.isCurrentList(current)) setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      if (activeWorkspace.current === requestWorkspaceId && requests.current.isCurrentList(current)) setLoadingMoreConversations(false);
    }
  };

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
    const createDraftKey = request.workspaceId + " " + request.requestId;
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
      setItems(current => [created, ...current.filter(item => item.conversation_id !== created.conversation_id)]);
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
        pendingCreateDrafts.current.set(workspaceId + " " + request.requestId, draft);
      }
      void submitCreate(request);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  };

  const workspacePendingCreates = pendingCreateRequests.filter(request => request.workspaceId === workspaceId);

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
  const firstUseGuidance = firstUseConversationGuidance({ workspaceAvailable: Boolean(workspaceId), operatorReady });
  const firstUseUnavailable = firstUseGuidance.state !== "SENDING_NOT_AVAILABLE";
  const firstUseView = <section className={firstUseUnavailable ? "conversation-unavailable" : "conversation-first-use"} aria-labelledby="conversation-first-use-title">
    <h2 id="conversation-first-use-title">{firstUseGuidance.heading}</h2>
    <p>{firstUseGuidance.message}</p>
    <label htmlFor="conversation-first-use-draft">Question draft</label>
    <textarea
      id="conversation-first-use-draft"
      aria-label="Question draft"
      placeholder="Write a question to keep in this window…"
      value={draft}
      onChange={event => onDraftChange(event.target.value)}
      rows={4}
    />
    <p className="conversation-first-use-status" role="status" aria-live="polite">
      {draft.trim() ? "Draft only. It has not been sent or saved. Opening Settings will not send it." : "No message has been sent. Any text you add stays only in this window."}
    </p>
    {firstUseGuidance.actionLabel && <button className="primary-button" type="button" onClick={onOpenAgentSettings}>{firstUseGuidance.actionLabel}</button>}
  </section>;

  return <div className="page-content conversations-page">
    <div className="conversation-page-heading">
      <div><div className="eyebrow">WORKSPACE CHATS</div><h1>Chats</h1></div>
      <button className="primary-button conversation-new-chat" type="button" onClick={() => void create("")} disabled={!workspaceId || !operatorReady || busy || createJournalStatus !== "READY"} aria-busy={busy} title={!workspaceId ? "Select a Workspace before creating a chat." : !operatorReady ? "Start the local Runtime before creating a chat." : createJournalStatus !== "READY" ? "Wait until pending conversation requests are checked." : undefined}>
        <span aria-hidden="true">＋</span> New chat
      </button>
    </div>
    <p className="conversation-intro">{workspaceId ? `Conversations are saved locally in ${workspaceName}.` : "Select a Workspace to view its locally saved conversations."}</p>
    {!workspaceId || (!operatorReady && items.length === 0) ? firstUseView : <div className="conversation-layout">
      <aside className="conversation-list" aria-label="Saved conversations">
        {createJournalStatus === "LOADING" && <p className="conversation-create-recovery" role="status">Checking pending Conversation requests…</p>}
        {createJournalStatus === "FAILED" && <section className="conversation-create-recovery" role="alert">
          <p>Pending Conversation retry state is unavailable. Creating a Conversation is disabled until the local journal can be read.</p>
          <button type="button" disabled={busy} onClick={() => {
            setError(null);
            setCreateJournalStatus("LOADING");
            setCreateJournalAttempt(value => value + 1);
          }}>Retry checking pending Conversation requests</button>
        </section>}
        {workspacePendingCreates.length > 0 && <section className="conversation-create-recovery" aria-label="Pending Conversation creations">
          <p>A previous create may have completed. Retry uses the same request identity and will not create a duplicate.</p>
          <ul>{workspacePendingCreates.map(request => <li key={request.requestId}>
            <span>{request.title ?? "Untitled conversation"}</span>
            <button type="button" disabled={!operatorReady || busy} onClick={() => void submitCreate(request)}>Retry pending create</button>
          </li>)}</ul>
        </section>}
        <details id="conversation-create-details" className="conversation-create-details">
          <summary>Create a blank conversation</summary>
          <form className="conversation-create" onSubmit={event => { event.preventDefault(); void create(); }}>
            <label htmlFor="conversation-title">Optional title</label>
            <input id="conversation-title" value={title} onChange={event => setTitle(event.target.value)} maxLength={160} placeholder="Name this conversation" disabled={!operatorReady || busy || createJournalStatus !== "READY"} />
            <p>This saves an empty conversation. It does not start an agent.</p>
            <button className="primary-button" type="submit" disabled={!operatorReady || busy || createJournalStatus !== "READY"}>{busy ? "Saving…" : "Create blank conversation"}</button>
          </form>
        </details>
        <div className="conversation-list-heading"><strong>Recent</strong><button type="button" className="text-button" onClick={() => void refresh()} disabled={!operatorReady || busy}>Refresh</button></div>
        {items.length === 0 && !busy ? <p className="conversation-list-empty">No saved conversations yet.</p> : <ul>
          {items.map(item => <li key={item.conversation_id}><button className={`conversation-list-item ${selectedId === item.conversation_id ? "selected" : ""}`} type="button" disabled={!operatorReady && item.conversation_id !== selectedId} onClick={() => setSelectedId(item.conversation_id)}>
            <strong>{item.title ?? "New conversation"}</strong><small>{timeLabel(item.created_at)}</small>
          </button></li>)}
        </ul>}
        {listCursor && <button type="button" className="conversation-load-more" onClick={() => void loadMoreConversations()} disabled={!operatorReady || loadingMoreConversations}>{loadingMoreConversations ? "Loading conversations…" : "Load more conversations"}</button>}
      </aside>
      <section className="conversation-reader" aria-label="Conversation history">
        {!operatorReady && items.length > 0 && <p className="conversation-offline-notice" role="status">The local Runtime is unavailable. Showing the last loaded conversation list and messages already loaded in this window. Read-only until the Runtime reconnects.</p>}
        {operatorReady && listStatus === "LOADING" && items.length > 0 && <p className="conversation-refresh-notice" role="status">Reconnecting. Showing the last loaded conversations and messages until refresh finishes.</p>}
        {(error || createReceiptError?.workspaceId === workspaceId) && <div className="conversation-error" role="alert">{[error, createReceiptError?.workspaceId === workspaceId ? createReceiptError.message : null].filter(Boolean).join(" ")}</div>}
        {selected && <div className="conversation-reader-heading"><div><div className="eyebrow">SAVED CONVERSATION</div><h2>{selected.title ?? "New conversation"}</h2></div><div className="conversation-statuses">{statusLabel && <span className="conversation-status">{statusLabel}</span>}{streamStatusLabel && <span className="conversation-status" role="status" aria-live="polite">{streamStatusLabel}</span>}</div></div>}
        {!selected ? listStatus === "LOADING" ? <div className="conversation-empty" role="status">Loading saved conversations…</div>
          : listStatus === "FAILED" ? <div className="conversation-empty conversation-list-error" role="alert"><h2>Could not load saved conversations</h2><p>Try again to check this Workspace’s history.</p><button type="button" className="conversation-load-more" onClick={() => void refresh()} disabled={!operatorReady || busy}>Retry loading conversations</button></div>
            : items.length === 0 ? firstUseView
              : <div className="conversation-empty"><h2>Choose a conversation</h2><p>Create one or select a saved conversation to read its messages.</p></div>
          : busy && !currentSnapshot ? <div className="conversation-empty" role="status">Loading saved messages…</div>
            : !operatorReady && !currentSnapshot ? <div className="conversation-empty"><h2>Saved messages are unavailable</h2><p>These messages were not loaded before the Runtime disconnected. Reconnect to read them.</p></div>
              : currentSnapshot?.messages.length || transientTurns.length ? <div className="conversation-messages">
              {currentSnapshot?.next_cursor && <button type="button" className="conversation-load-more" onClick={() => void loadMoreMessages()} disabled={!operatorReady || busy || loadingMore}>{loadingMore ? "Loading messages…" : "Load more messages"}</button>}
              {(currentSnapshot?.messages ?? []).map(message => <article className={`conversation-message ${message.role.toLowerCase()}`} key={message.message_id}>
              <div className="conversation-message-author">{message.role === "USER" ? "You" : message.role === "AGENT" ? "Assistant" : message.role === "SYSTEM_NOTICE" ? "LiteCowork" : "Connected channel"}</div>
              <ConversationMessageContent workspaceId={workspaceId} conversationId={selected.conversation_id} message={message} autoLoadRich={automaticRichIds.has(message.message_id)} presentationEpoch={richPresentationEpochs.get(message.message_id) ?? 0} />
              <time>{timeLabel(message.created_at)}</time>
            </article>)}
              {transientTurns.map(turn => <article className={`conversation-message agent conversation-transient-turn ${turn.state.toLowerCase()}`} key={`${turn.conversationId}\u0000${turn.turnId}\u0000${turn.retryOrdinal}`} aria-label={turn.state === "STREAMING" ? "Assistant response in progress" : turn.state === "RECONNECTING" ? "Assistant response reconnecting" : "Incomplete assistant response"}>
                <div className="conversation-message-author" role="status" aria-live="polite" aria-atomic="true">Assistant · {turn.state === "STREAMING" ? "response in progress" : turn.state === "RECONNECTING" ? "reconnecting response" : "response did not finish"}</div>
                {turn.text && <div className="conversation-message-body conversation-transient-text">{turn.text}</div>}
                {turn.state === "RECONNECTING" && <p className="conversation-rich-status" role="note">Reconnecting this response. The text remains temporary until it is saved.</p>}
                {turn.state === "INCOMPLETE" && <p className="conversation-rich-status" role="note">This response did not finish and is not part of saved history. Its partial text is shown above.</p>}
              </article>)}
            </div>
              : <div className="conversation-empty"><h2>This conversation is ready</h2><p>Conversation records and messages are durable. Sending messages is not available yet because native agent turn execution is not connected.</p></div>}
        {selected && <div className="conversation-composer">
          <div className="conversation-agent-control">
            <label htmlFor={`conversation-agent-${selected.conversation_id}`}>Agent for this conversation</label>
            <select
              id={`conversation-agent-${selected.conversation_id}`}
              value={selected.active_agent_binding_id ?? ""}
              disabled={!operatorReady || agentCatalogStatus === "LOADING" || agentMutationNotice?.status === "SAVING"}
              onChange={event => void saveConversationAgentBinding(event.target.value || null)}
            >
              <option value="">{workspaceDefaultOptionLabel}</option>
              {agentChoices.map(choice => <option key={choice.agentBindingId} value={choice.agentBindingId} disabled={!choice.selectable}>{choice.label}</option>)}
            </select>
            <small>New turns use this selection. Changing it does not switch a running session.</small>
            {agentCatalogStatus === "LOADING" && <p role="status">Loading available agents…</p>}
            {agentCatalogStatus === "FAILED" && <p className="conversation-agent-error" role="alert">{agentCatalogError ?? "Available agents could not be loaded."} <button type="button" className="text-button" onClick={() => setAgentCatalogAttempt(value => value + 1)}>Retry</button></p>}
            {agentCatalogStatus === "OFFLINE" && <p role="status">Reconnect the local Runtime to change this selection.</p>}
            {visibleAgentMutationNotice && <p className={`conversation-agent-notice ${visibleAgentMutationNotice.status.toLowerCase()}`} role={visibleAgentMutationNotice.status === "FAILED" ? "alert" : "status"} aria-live="polite">{visibleAgentMutationNotice.message}</p>}
            {agentCatalogStatus === "READY" && agentChoices.filter(choice => choice.selectable).length === 0 && <p>No enabled, lead-eligible agent is available. <button type="button" className="text-button" onClick={onOpenAgentSettings}>Manage agents</button></p>}
          </div>
          <textarea
            aria-label="Message draft"
            placeholder="Write a message to keep as a draft…"
            value={composerDraft}
            onChange={event => {
              if (!composerDraftKey) return;
              setConversationDrafts(current => ({ ...current, [composerDraftKey]: event.target.value }));
            }}
            rows={4}
          />
          <button className="primary-button" type="button" disabled>Send</button>
          <small role="status" aria-live="polite">
            {composerDraft.trim()
              ? `Draft only. ${composerSurface.blockers[0] ?? "An admitted native session is required before sending."}`
              : `Sending is unavailable until a native session is admitted safely. ${composerSurface.blockers[0] ?? ""}`}
          </small>
        </div>}
      </section>
    </div>}
  </div>;
}
