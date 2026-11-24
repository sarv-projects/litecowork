import assert from "node:assert/strict";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import react from "@vitejs/plugin-react";
import { createServer } from "vite";

const vite = await createServer({
  configFile: false,
  root: fileURLToPath(new URL("..", import.meta.url)),
  plugins: [react()],
  server: { middlewareMode: true, hmr: false },
  optimizeDeps: { noDiscovery: true, include: [] },
  appType: "custom",
});
const { RecentChatsSidebar } = await vite.ssrLoadModule("/src/shell/RecentChatsSidebar.tsx");
after(async () => vite.close());

const conversation = {
  conversation_id: "conversation-1",
  title: "Research notes",
  workspace_id: "workspace-1",
  status: "ACTIVE",
  created_at: "2026-11-23T00:00:00Z",
  updated_at: "2026-11-23T00:00:00Z",
};
const base = {
  items: [],
  status: "READY",
  runtimeReady: true,
  selectedConversationId: null,
  showPagination: false,
  hasMore: false,
  loadingMore: false,
  workspaceSelected: true,
  onOpenConversation: () => {},
  onRetry: () => {},
  onLoadMore: () => {},
};

test("Recent Chats distinguishes loading, unavailable, failure and ready-empty", () => {
  const loading = renderToStaticMarkup(createElement(RecentChatsSidebar, { ...base, status: "LOADING" }));
  assert.match(loading, /Loading recent chats/);
  assert.doesNotMatch(loading, /No saved chats yet/);

  const unavailable = renderToStaticMarkup(createElement(RecentChatsSidebar, { ...base, status: "UNAVAILABLE", runtimeReady: false }));
  assert.match(unavailable, /Start the local Runtime/);
  assert.doesNotMatch(unavailable, /No saved chats yet/);

  const unavailableSource = renderToStaticMarkup(createElement(RecentChatsSidebar, { ...base, status: "UNAVAILABLE" }));
  assert.match(unavailableSource, /Recent chats are unavailable right now/);
  assert.doesNotMatch(unavailableSource, /No saved chats yet/);

  const idle = renderToStaticMarkup(createElement(RecentChatsSidebar, { ...base, status: "IDLE" }));
  assert.match(idle, /Open a chat to load recent chats/);
  assert.doesNotMatch(idle, /No saved chats yet/);

  const failed = renderToStaticMarkup(createElement(RecentChatsSidebar, { ...base, status: "FAILED" }));
  assert.match(failed, /role="alert"/);
  assert.match(failed, /Retry/);
  assert.doesNotMatch(failed, /No saved chats yet/);

  const empty = renderToStaticMarkup(createElement(RecentChatsSidebar, base));
  assert.match(empty, /No saved chats yet/);
});

test("Recent Chats never shows cached chats as available without a selected Workspace", () => {
  const html = renderToStaticMarkup(createElement(RecentChatsSidebar, {
    ...base,
    items: [conversation],
    workspaceSelected: false,
  }));
  assert.match(html, /Select a Workspace to view its chats/);
  assert.doesNotMatch(html, /Research notes/);
});

test("Recent Chats marks only the selected conversation and exposes real pagination", () => {
  const html = renderToStaticMarkup(createElement(RecentChatsSidebar, {
    ...base,
    items: [conversation],
    selectedConversationId: "conversation-1",
    showPagination: true,
    hasMore: true,
  }));
  assert.match(html, /aria-current="true"/);
  assert.match(html, /Load more chats/);
});
