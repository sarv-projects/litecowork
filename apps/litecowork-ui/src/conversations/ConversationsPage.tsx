import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { RichResponseView } from "../presentation/RichResponseView";
import { parseRichPresentationResponse, type ValidatedRichPresentation } from "../presentation/rich-presentation";
import { appendConversationListPage, appendConversationPage, ConversationCreateRequestCache, ConversationRequestEpochs, mergeConversationStreamSnapshot, parseConversationListPage, parseConversationList, parseConversationSnapshot, richPresentationCacheKey, type ConversationMessageView, type ConversationSnapshotView, type ConversationSummary, type PendingConversationCreate } from "./conversation-view";
import { ConversationStreamController, type ConversationStreamDriver, type StreamEnvelope } from "./conversation-stream";
import "./conversations-page.css";

// Keep ambiguous create receipts across keyed Workspace page remounts for this app session.
const pendingCreates = new ConversationCreateRequestCache();

function messageText(message: ConversationMessageView): string {
  return message.content.map(block => block.kind === "TEXT" ? block.text : "Attachment").join("\n");
}

function semanticText(message: ConversationMessageView): string {
  return message.content.filter((block): block is { kind: "TEXT"; text: string } => block.kind === "TEXT").map(block => block.text).join("\n");
}

function ConversationMessageContent({ workspaceId, conversationId, message, autoLoadRich }: {
  workspaceId: string;
  conversationId: string;
  message: ConversationMessageView;
  autoLoadRich: boolean;
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
  const cacheKey = richPresentationCacheKey(workspaceId, conversationId, message.message_id, reference);
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

export function ConversationsPage({ workspaceId, workspaceName, operatorReady }: { workspaceId: string; workspaceName: string; operatorReady: boolean }) {
  const [items, setItems] = useState<ConversationSummary[]>([]);
  const [listCursor, setListCursor] = useState<string | null>(null);
  const [loadingMoreConversations, setLoadingMoreConversations] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [snapshot, setSnapshot] = useState<ConversationSnapshotView | null>(null);
  const [title, setTitle] = useState("");
  const [busy, setBusy] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [snapshotLoadedKey, setSnapshotLoadedKey] = useState<string | null>(null);
  const [streamStatus, setStreamStatus] = useState<"CONNECTING" | "LIVE" | "RECONNECTING" | null>(null);
  const requests = useRef(new ConversationRequestEpochs());
  const activeListEpoch = useRef<number | null>(null);
  const activeSnapshotEpoch = useRef<number | null>(null);
  const mounted = useRef(false);
  const activeWorkspace = useRef(workspaceId);
  activeWorkspace.current = workspaceId;
  const selectionKey = workspaceId && selectedId ? `${workspaceId}\u0000${selectedId}` : null;

  const refresh = useCallback(async () => {
    const current = requests.current.beginList();
    activeListEpoch.current = current;
    setListCursor(null);
    setLoadingMoreConversations(false);
    setSnapshot(null);
    setSnapshotLoadedKey(null);
    setSelectedId(null);
    if (!operatorReady || !workspaceId) { setItems([]); setBusy(false); return; }
    setBusy(true); setError(null);
    try {
      const response = await invoke<unknown>("list_conversations", { workspaceId, cursor: null });
      if (activeWorkspace.current !== workspaceId || !requests.current.isCurrentList(current)) return;
      const page = parseConversationListPage(response, workspaceId);
      setItems(page.items);
      setListCursor(page.next_cursor);
      if (page.items[0]) setSelectedId(page.items[0].conversation_id);
    } catch (cause) {
      if (activeWorkspace.current === workspaceId && requests.current.isCurrentList(current)) setError(cause instanceof Error ? cause.message : String(cause));
    } finally { if (activeWorkspace.current === workspaceId && requests.current.isCurrentList(current)) setBusy(false); }
  }, [operatorReady, workspaceId]);

  useEffect(() => { void refresh(); return () => { requests.current.invalidateList(); }; }, [refresh]);

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
    setSnapshot(null);
    setSnapshotLoadedKey(null);
    setLoadingMore(false);
    if (!operatorReady || !workspaceId || !selectedId) return () => { requests.current.invalidateSnapshot(); };
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
      setStreamStatus(null);
      return;
    }
    const driver: ConversationStreamDriver = {
      listen: async callback => listen<unknown>("conversation-presentation-stream", event => callback(event.payload as StreamEnvelope)),
      start: input => invoke<void>("subscribe_conversation_stream", input),
      acknowledge: (sessionId, frameId) => invoke<void>("acknowledge_conversation_stream_frame", { sessionId, frameId }),
      stop: sessionId => invoke<void>("unsubscribe_conversation_stream", { sessionId }),
      newSessionId: () => crypto.randomUUID(),
    };
    const controller = new ConversationStreamController(workspaceId, selectedId!, driver, state => {
      const live = state.conversationSnapshot;
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

  const create = async () => {
    if (!operatorReady || busy) return;
    const requestWorkspaceId = workspaceId;
    const normalizedTitle = title.trim() || null;
    let request: PendingConversationCreate;
    try {
      request = pendingCreates.get(requestWorkspaceId, normalizedTitle, () => crypto.randomUUID());
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      return;
    }
    setBusy(true); setError(null);
    try {
      const response = await invoke<unknown>("create_conversation", {
        workspaceId: requestWorkspaceId, title: normalizedTitle, requestId: request.requestId,
      });
      if (!mounted.current || activeWorkspace.current !== requestWorkspaceId) return;
      const created = parseConversationList({ items: [response] }, requestWorkspaceId)[0];
      if (!created) throw new Error("Local Runtime returned an invalid Conversation.");
      pendingCreates.complete(request);
      setTitle("");
      setItems(current => [created, ...current.filter(item => item.conversation_id !== created.conversation_id)]);
      setSelectedId(created.conversation_id);
      setSnapshot({ workspace_id: requestWorkspaceId, conversation_id: created.conversation_id, messages: [], next_cursor: null, active_turn: null });
      setSnapshotLoadedKey(`${requestWorkspaceId}\u0000${created.conversation_id}`);
    } catch (cause) {
      if (mounted.current && activeWorkspace.current === requestWorkspaceId) setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      if (mounted.current && activeWorkspace.current === requestWorkspaceId) setBusy(false);
    }
  };

  const selected = items.find(item => item.conversation_id === selectedId) ?? null;
  const automaticRichIds = new Set((snapshot?.messages ?? []).filter(message => message.role === "AGENT" && message.rich_presentation?.availability === "AVAILABLE").slice(-6).map(message => message.message_id));
  const activeStatus = snapshot?.active_turn?.status;
  const statusLabel = activeStatus === "RUNNING" ? "A conversation turn is running"
    : activeStatus === "WAITING_USER" ? "Waiting for your response"
      : activeStatus === "WAITING_DEPENDENCY" ? "Waiting for a service"
        : activeStatus === "CANCEL_REQUESTED" ? "Stopping"
          : activeStatus === "OPEN" ? "Preparing"
            : null;
  const streamStatusLabel = streamStatus === "LIVE" ? "Saved history updates live"
    : streamStatus === "RECONNECTING" ? "Reconnecting saved-history updates…"
      : streamStatus === "CONNECTING" ? "Connecting saved-history updates…"
        : null;

  return <div className="page-content conversations-page">
    <div className="eyebrow">SAVED WORKSPACE CONVERSATIONS</div>
    <h1>Conversations</h1>
    <p className="conversation-intro">Review messages saved in {workspaceName}. Messages are stored locally in this Workspace.</p>
    {!workspaceId ? <section className="conversation-empty"><h2>Select a Workspace</h2><p>Choose a Workspace in Settings before opening its saved conversations.</p></section>
      : !operatorReady ? <section className="conversation-empty"><h2>Local Runtime unavailable</h2><p>Start the local Runtime to load saved conversations.</p></section> : <div className="conversation-layout">
      <aside className="conversation-list" aria-label="Saved conversations">
        <form className="conversation-create" onSubmit={event => { event.preventDefault(); void create(); }}>
          <label htmlFor="conversation-title">Create a conversation</label>
          <input id="conversation-title" value={title} onChange={event => setTitle(event.target.value)} maxLength={160} placeholder="Add a title (optional)" disabled={busy} />
          <button className="primary-button" type="submit" disabled={busy}>{busy ? "Saving…" : "Create"}</button>
        </form>
        <div className="conversation-list-heading"><strong>Recent</strong><button type="button" className="text-button" onClick={() => void refresh()} disabled={busy}>Refresh</button></div>
        {items.length === 0 && !busy ? <p className="conversation-list-empty">No saved conversations yet.</p> : <ul>
          {items.map(item => <li key={item.conversation_id}><button className={`conversation-list-item ${selectedId === item.conversation_id ? "selected" : ""}`} type="button" onClick={() => setSelectedId(item.conversation_id)}>
            <strong>{item.title ?? "New conversation"}</strong><small>{timeLabel(item.created_at)}</small>
          </button></li>)}
        </ul>}
        {listCursor && <button type="button" className="conversation-load-more" onClick={() => void loadMoreConversations()} disabled={loadingMoreConversations}>{loadingMoreConversations ? "Loading conversations…" : "Load more conversations"}</button>}
      </aside>
      <section className="conversation-reader" aria-label="Conversation history">
        {error && <div className="conversation-error" role="alert">{error}</div>}
        {selected && <div className="conversation-reader-heading"><div><div className="eyebrow">SAVED CONVERSATION</div><h2>{selected.title ?? "New conversation"}</h2></div><div className="conversation-statuses">{statusLabel && <span className="conversation-status">{statusLabel}</span>}{streamStatusLabel && <span className="conversation-status" role="status" aria-live="polite">{streamStatusLabel}</span>}</div></div>}
        {!selected ? <div className="conversation-empty"><h2>Choose a conversation</h2><p>Create one or select a saved conversation to read its messages.</p></div>
          : busy && !snapshot ? <div className="conversation-empty" role="status">Loading saved messages…</div>
            : snapshot?.messages.length ? <div className="conversation-messages">
              {snapshot.next_cursor && <button type="button" className="conversation-load-more" onClick={() => void loadMoreMessages()} disabled={loadingMore}>{loadingMore ? "Loading messages…" : "Load more messages"}</button>}
              {snapshot.messages.map(message => <article className={`conversation-message ${message.role.toLowerCase()}`} key={message.message_id}>
              <div className="conversation-message-author">{message.role === "USER" ? "You" : message.role === "AGENT" ? "Assistant" : message.role === "SYSTEM_NOTICE" ? "LiteCowork" : "Connected channel"}</div>
              <ConversationMessageContent workspaceId={workspaceId} conversationId={selected.conversation_id} message={message} autoLoadRich={automaticRichIds.has(message.message_id)} />
              <time>{timeLabel(message.created_at)}</time>
            </article>)}</div>
              : <div className="conversation-empty"><h2>This conversation is ready</h2><p>Conversation records and messages are durable. Sending messages is not available yet because native agent turn execution is not connected.</p></div>}
        {selected && <div className="conversation-composer"><textarea aria-label="Message" placeholder="Messaging will be available when agent execution is connected" disabled value="" readOnly /><button className="primary-button" type="button" disabled>Send</button><small>Read-only until a real agent session can be admitted safely.</small></div>}
      </section>
    </div>}
  </div>;
}
