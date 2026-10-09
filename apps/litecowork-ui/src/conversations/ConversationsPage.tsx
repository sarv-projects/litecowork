import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { parseConversationList, parseConversationSnapshot, type ConversationMessageView, type ConversationSnapshotView, type ConversationSummary } from "./conversation-view";
import "./conversations-page.css";

function messageText(message: ConversationMessageView): string {
  return message.content.map(block => block.kind === "TEXT" ? block.text : "Attachment").join("\n");
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
  const [error, setError] = useState<string | null>(null);
  const generation = useRef(0);

  const refresh = useCallback(async () => {
    const current = ++generation.current;
    setSnapshot(null);
    setSelectedId(null);
    if (!operatorReady || !workspaceId) { setItems([]); return; }
    setBusy(true); setError(null);
    try {
      const response = await invoke<unknown>("list_conversations", { workspaceId });
      if (current !== generation.current) return;
      const rows = parseConversationList(response, workspaceId);
      setItems(rows);
      if (rows[0]) setSelectedId(rows[0].conversation_id);
    } catch (cause) {
      if (current === generation.current) setError(cause instanceof Error ? cause.message : String(cause));
    } finally { if (current === generation.current) setBusy(false); }
  }, [operatorReady, workspaceId]);

  useEffect(() => { void refresh(); return () => { generation.current += 1; }; }, [refresh]);

  useEffect(() => {
    const current = ++generation.current;
    setSnapshot(null);
    if (!operatorReady || !workspaceId || !selectedId) return () => { generation.current += 1; };
    setBusy(true); setError(null);
    void invoke<unknown>("get_conversation_presentation", { workspaceId, conversationId: selectedId })
      .then(value => { if (generation.current === current) setSnapshot(parseConversationSnapshot(value, workspaceId, selectedId)); })
      .catch(cause => { if (generation.current === current) setError(cause instanceof Error ? cause.message : String(cause)); })
      .finally(() => { if (generation.current === current) setBusy(false); });
    return () => { generation.current += 1; };
  }, [operatorReady, selectedId, workspaceId]);

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
      setSnapshot({ workspace_id: workspaceId, conversation_id: created.conversation_id, messages: [], active_turn: null });
    } catch (cause) { setError(cause instanceof Error ? cause.message : String(cause)); }
    finally { setBusy(false); }
  };

  const selected = items.find(item => item.conversation_id === selectedId) ?? null;
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
            : snapshot?.messages.length ? <div className="conversation-messages">{snapshot.messages.map(message => <article className={`conversation-message ${message.role.toLowerCase()}`} key={message.message_id}>
              <div className="conversation-message-author">{message.role === "USER" ? "You" : message.role === "AGENT" ? "Assistant" : message.role === "SYSTEM_NOTICE" ? "LiteCowork" : "Connected channel"}</div>
              <div className="conversation-message-body">{messageText(message)}</div>
              <time>{timeLabel(message.created_at)}</time>
            </article>)}</div>
              : <div className="conversation-empty"><h2>This conversation is ready</h2><p>Conversation records and messages are durable. Sending messages is not available yet because native agent turn execution is not connected.</p></div>}
        {selected && <div className="conversation-composer"><textarea aria-label="Message" placeholder="Messaging will be available when agent execution is connected" disabled value="" readOnly /><button className="primary-button" type="button" disabled>Send</button><small>Read-only until a real agent session can be admitted safely.</small></div>}
      </section>
    </div>}
  </div>;
}
