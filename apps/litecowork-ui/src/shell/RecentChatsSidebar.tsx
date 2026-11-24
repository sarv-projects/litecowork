import type { ConversationSummary } from "../conversations/conversation-view";

export type ConversationListDisplayStatus = "IDLE" | "LOADING" | "READY" | "FAILED" | "UNAVAILABLE";

type Props = {
  items: readonly ConversationSummary[];
  status: ConversationListDisplayStatus;
  runtimeReady: boolean;
  selectedConversationId: string | null;
  showPagination: boolean;
  hasMore: boolean;
  loadingMore: boolean;
  workspaceSelected: boolean;
  onOpenConversation: (conversationId: string) => void;
  onRetry: () => void;
  onLoadMore: () => void;
};

export function RecentChatsSidebar({
  items,
  status,
  runtimeReady,
  selectedConversationId,
  showPagination,
  hasMore,
  loadingMore,
  workspaceSelected,
  onOpenConversation,
  onRetry,
  onLoadMore,
}: Props) {
  return (
    <section className="sidebar-recent-chats-wrap" aria-label="Recent Chats">
      <div className="sidebar-section-header">
        <span className="sidebar-section-label">Recent Chats</span>
      </div>
      <div className="sidebar-recent-chat-list" aria-live="polite">
        {!runtimeReady ? (
          <p className="sidebar-chat-item-empty" role="status">Start the local Runtime to load recent chats.</p>
        ) : !workspaceSelected ? (
          <p className="sidebar-chat-item-empty" role="status">Select a Workspace to view its chats.</p>
        ) : status === "LOADING" ? (
          <p className="sidebar-chat-item-empty" role="status">Loading recent chats…</p>
        ) : status === "UNAVAILABLE" ? (
          <p className="sidebar-chat-item-empty" role="status">Recent chats are unavailable right now.</p>
        ) : status === "IDLE" ? (
          <p className="sidebar-chat-item-empty" role="status">Open a chat to load recent chats.</p>
        ) : status === "FAILED" ? (
          <div className="sidebar-chat-item-empty" role="alert">
            <p>Recent chats could not be loaded.</p>
            <button className="sidebar-chat-retry" type="button" onClick={onRetry}>Retry</button>
          </div>
        ) : items.length === 0 ? (
          <p className="sidebar-chat-item-empty" role="status">No saved chats yet</p>
        ) : (
          items.map((conversation) => {
            const selected = selectedConversationId === conversation.conversation_id;
            return (
              <button
                key={conversation.conversation_id}
                className={`sidebar-chat-item-row${selected ? " selected" : ""}`}
                type="button"
                aria-current={selected ? "true" : undefined}
                onClick={() => onOpenConversation(conversation.conversation_id)}
              >
                <span className="chat-row-title" title={conversation.title || "Untitled"}>
                  {conversation.title || "Untitled"}
                </span>
                {selected ? <span className="chat-row-now">now</span> : <span className="chat-row-dot" />}
              </button>
            );
          })
        )}
      </div>
      {showPagination && status === "READY" && hasMore && (
        <button className="sidebar-chat-load-more" type="button" disabled={loadingMore} onClick={onLoadMore}>
          {loadingMore ? "Loading chats…" : "Load more chats"}
        </button>
      )}
    </section>
  );
}
