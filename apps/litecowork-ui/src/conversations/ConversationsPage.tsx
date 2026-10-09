import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RichResponseView } from "../presentation/RichResponseView";
import { parseRichPresentationResponse, type ValidatedRichPresentation } from "../presentation/rich-presentation";
import { appendConversationPage, ConversationRequestEpochs, parseConversationList, parseConversationSnapshot, type ConversationMessageView, type ConversationSnapshotView, type ConversationSummary } from "./conversation-view";
import "./conversations-page.css";

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
  const [visible, setVisible] = useState(false);
  const [requested, setRequested] = useState(false);
  const [retry, setRetry] = useState(0);
  const [status, setStatus] = useState<"IDLE" | "LOADING" | "READY" | "FAILED">("IDLE");
  const [presentation, setPresentation] = useState<ValidatedRichPresentation | null>(null);
  const reference = message.rich_presentation;
  const text = semanticText(message);

  useEffect(() => {
    if (!autoLoadRich || !reference || reference.availability !== "AVAILABLE" || !element.current || typeof IntersectionObserver === "undefined") return;
    const observer = new IntersectionObserver(entries => {
      if (entries.some(entry => entry.isIntersecting)) {
        setVisible(true);
        observer.disconnect();
      }
    }, { rootMargin: "320px" });
    observer.observe(element.current);
    return () => observer.disconnect();
  }, [autoLoadRich, reference]);

  const shouldLoad = Boolean(reference && reference.availability === "AVAILABLE" && (requested || (autoLoadRich && visible)));
  useEffect(() => {
    if (!shouldLoad || !reference || status !== "IDLE") return;
    let current = true;
    setStatus("LOADING");
    void invoke<unknown>("get_rich_presentation", {
      workspaceId,
      conversationId,
      messageId: message.message_id,
      presentationId: reference.presentation_id,
      semanticContentDigest: reference.semantic_content_digest,
    }).then(async value => {
      const parsed = await parseRichPresentationResponse(value, {
        workspaceId,
        conversationId,
        messageId: message.message_id,
        semanticContentDigest: reference.semantic_content_digest,
      }, text);
      if (!current) return;
      setPresentation(parsed);
      setStatus(parsed ? "READY" : "FAILED");
    }).catch(() => {
      if (current) setStatus("FAILED");
    });
    return () => { current = false; };
  // `status` is intentionally read to gate a new request but omitted from the
  // dependency list: changing IDLE -> LOADING must not clean up and cancel the
  // request that this effect just started. `retry` is the explicit re-fetch key.
  }, [conversationId, message.message_id, reference, retry, shouldLoad, text, workspaceId]);

  if (!reference || message.role !== "AGENT") return <div className="conversation-message-body">{messageText(message)}</div>;
  const availability = reference.availability;
  const canFetch = availability === "AVAILABLE";
  return <div className="conversation-message-body" ref={element}>
    <RichResponseView semanticText={text || messageText(message)} presentation={presentation} />
    {!presentation && canFetch && status === "IDLE" && !shouldLoad && <button type="button" className="conversation-rich-load" onClick={() => setRequested(true)}>Show formatted response</button>}
    {status === "LOADING" && <p className="conversation-rich-status" role="status">Loading formatted response…</p>}
    {status === "FAILED" && <p className="conversation-rich-status" role="note">Formatted view unavailable; the complete text response is still shown. <button type="button" onClick={() => { setStatus("IDLE"); setRequested(true); setRetry(value => value + 1); }}>Retry</button></p>}
    {!canFetch && <p className="conversation-rich-status" role="note">Formatted view unavailable; the complete text response is still shown.</p>}
  </div>;
}

function timeLabel(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? "Saved conversation" : date.toLocaleString();
}

export function ConversationsPage({ workspaceId, workspaceName, operatorReady }: { workspaceId: string; workspaceName: string; operatorReady: boolean }) {
  const [items, setItems] = useState<ConversationSummary[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [snapshot, setSnapshot] = useState<ConversationSnapshotView | null>(null);
  const [title, setTitle] = useState("");
  const [busy, setBusy] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const requests = useRef(new ConversationRequestEpochs());
  const activeSnapshotEpoch = useRef<number | null>(null);

  const refresh = useCallback(async () => {
    const current = requests.current.beginList();
    setSnapshot(null);
    setSelectedId(null);
    if (!operatorReady || !workspaceId) { setItems([]); return; }
    setBusy(true); setError(null);
    try {
      const response = await invoke<unknown>("list_conversations", { workspaceId });
      if (!requests.current.isCurrentList(current)) return;
      const rows = parseConversationList(response, workspaceId);
      setItems(rows);
      if (rows[0]) setSelectedId(rows[0].conversation_id);
    } catch (cause) {
      if (requests.current.isCurrentList(current)) setError(cause instanceof Error ? cause.message : String(cause));
    } finally { if (requests.current.isCurrentList(current)) setBusy(false); }
  }, [operatorReady, workspaceId]);

  useEffect(() => { void refresh(); return () => { requests.current.invalidateList(); }; }, [refresh]);

  useEffect(() => {
    const current = requests.current.beginSnapshot();
    activeSnapshotEpoch.current = current;
    setSnapshot(null);
    setLoadingMore(false);
    if (!operatorReady || !workspaceId || !selectedId) return () => { requests.current.invalidateSnapshot(); };
    setBusy(true); setError(null);
    void invoke<unknown>("get_conversation_presentation", { workspaceId, conversationId: selectedId })
      .then(value => { if (requests.current.isCurrentSnapshot(current)) setSnapshot(parseConversationSnapshot(value, workspaceId, selectedId)); })
      .catch(cause => { if (requests.current.isCurrentSnapshot(current)) setError(cause instanceof Error ? cause.message : String(cause)); })
      .finally(() => { if (requests.current.isCurrentSnapshot(current)) setBusy(false); });
    return () => {
      requests.current.invalidateSnapshot();
      if (activeSnapshotEpoch.current === current) activeSnapshotEpoch.current = null;
    };
  }, [operatorReady, selectedId, workspaceId]);

  const loadMoreMessages = async () => {
    if (!snapshot?.next_cursor || !selectedId || loadingMore) return;
    const current = activeSnapshotEpoch.current;
    if (current === null || !requests.current.isCurrentSnapshot(current)) return;
    const cursor = snapshot.next_cursor;
    setLoadingMore(true);
    try {
      const value = await invoke<unknown>("get_conversation_presentation", { workspaceId, conversationId: selectedId, cursor });
      if (!requests.current.isCurrentSnapshot(current)) return;
      const page = parseConversationSnapshot(value, workspaceId, selectedId);
      if (page.next_cursor === cursor) throw new Error("Conversation history did not advance its pagination cursor.");
      setSnapshot(existing => existing?.conversation_id === selectedId ? appendConversationPage(existing, page) : existing);
    } catch (cause) {
      if (requests.current.isCurrentSnapshot(current)) setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      if (requests.current.isCurrentSnapshot(current)) setLoadingMore(false);
    }
  };

  const create = async () => {
    if (!operatorReady || busy) return;
    setBusy(true); setError(null);
    try {
      const response = await invoke<unknown>("create_conversation", {
        workspaceId, title: title.trim() || null, requestId: crypto.randomUUID(),
      });
      const created = parseConversationList({ items: [response] }, workspaceId)[0];
      setTitle("");
      setItems(current => [created, ...current.filter(item => item.conversation_id !== created.conversation_id)].slice(0, 100));
      setSelectedId(created.conversation_id);
      setSnapshot({ workspace_id: workspaceId, conversation_id: created.conversation_id, messages: [], next_cursor: null, active_turn: null });
    } catch (cause) { setError(cause instanceof Error ? cause.message : String(cause)); }
    finally { setBusy(false); }
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
      </aside>
      <section className="conversation-reader" aria-label="Conversation history">
        {error && <div className="conversation-error" role="alert">{error}</div>}
        {selected && <div className="conversation-reader-heading"><div><div className="eyebrow">SAVED CONVERSATION</div><h2>{selected.title ?? "New conversation"}</h2></div>{statusLabel && <span className="conversation-status">{statusLabel}</span>}</div>}
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
