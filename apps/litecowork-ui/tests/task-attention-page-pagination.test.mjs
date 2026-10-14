import assert from "node:assert/strict";
import { after, test } from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import react from "@vitejs/plugin-react";
import { createServer } from "vite";

const vite = await createServer({
  configFile: false,
  root: new URL("..", import.meta.url).pathname,
  plugins: [react()],
  server: { middlewareMode: true },
  appType: "custom",
});
const { mergeTaskAttentionPages, TaskAttentionLoadMoreButton, TaskAttentionPaginationGate } = await vite.ssrLoadModule(
  "/src/needs-you/TaskAttentionPage.tsx",
);
after(async () => vite.close());

test("one synchronous pagination claim wins and an old completion cannot unlock a refreshed generation", () => {
  const gate = new TaskAttentionPaginationGate();
  gate.invalidate(4);

  assert.equal(gate.begin(4), true);
  assert.equal(gate.begin(4), false, "same-render duplicate clicks cannot start another request");

  gate.invalidate(5);
  assert.equal(gate.begin(5), true, "refresh permits the new generation to paginate");
  assert.equal(gate.finish(4), false, "the old request cannot release the new generation's lock");
  assert.equal(gate.begin(5), false);
  assert.equal(gate.finish(5), true);
  assert.equal(gate.begin(5), true, "completion releases the current generation");
});

test("successful status pages advance and append even when a sibling status page fails", () => {
  const existing = {
    taskId: "waiting-1",
    status: "WAITING_USER",
    objective: "Existing waiting task",
    createdAt: "2026-10-14T10:00:00Z",
    updatedAt: "2026-10-14T10:00:00Z",
  };
  const waitingNext = {
    taskId: "waiting-2",
    status: "WAITING_USER",
    objective: "Next waiting task",
    createdAt: "2026-10-14T10:01:00Z",
    updatedAt: "2026-10-14T10:01:00Z",
  };
  const blockedNext = {
    taskId: "blocked-2",
    status: "BLOCKED",
    objective: "Next blocked task",
    createdAt: "2026-10-14T10:02:00Z",
    updatedAt: "2026-10-14T10:02:00Z",
  };
  const result = mergeTaskAttentionPages(
    [existing],
    [
      { status: "WAITING_USER", nextCursor: "waiting-cursor-1" },
      { status: "NEEDS_USER", nextCursor: "needs-cursor-1" },
      { status: "BLOCKED", nextCursor: "blocked-cursor-1" },
    ],
    [
      { status: "WAITING_USER", ok: true, page: { items: [existing, waitingNext], nextCursor: null } },
      { status: "NEEDS_USER", ok: false },
      { status: "BLOCKED", ok: true, page: { items: [blockedNext], nextCursor: "blocked-cursor-2" } },
    ],
  );

  assert.deepEqual(result.items.map(task => task.taskId), ["waiting-1", "waiting-2", "blocked-2"]);
  assert.deepEqual(result.pages, [
    { status: "WAITING_USER", nextCursor: null },
    { status: "NEEDS_USER", nextCursor: "needs-cursor-1" },
    { status: "BLOCKED", nextCursor: "blocked-cursor-2" },
  ]);
  assert.equal(result.failed, true);
});

test("a malformed status page does not append data or advance that status cursor", () => {
  const result = mergeTaskAttentionPages(
    [],
    [{ status: "BLOCKED", nextCursor: "blocked-cursor-1" }],
    [{
      status: "BLOCKED",
      ok: true,
      page: {
        items: [{
          taskId: "wrong-status",
          status: "NEEDS_USER",
          objective: "Mismatched status",
          createdAt: "2026-10-14T10:00:00Z",
          updatedAt: "2026-10-14T10:00:00Z",
        }],
        nextCursor: null,
      },
    }],
  );

  assert.deepEqual(result.items, []);
  assert.deepEqual(result.pages, [{ status: "BLOCKED", nextCursor: "blocked-cursor-1" }]);
  assert.equal(result.failed, true);
});

test("empty or repeated next cursors fail safely and keep the last usable cursor", () => {
  const retained = [
    { status: "BLOCKED", nextCursor: "blocked-cursor-1" },
    { status: "WAITING_USER", nextCursor: "waiting-cursor-1" },
  ];
  const result = mergeTaskAttentionPages(
    [],
    retained,
    [
      {
        status: "BLOCKED",
        ok: true,
        page: {
          items: [{ taskId: "blocked-2", status: "BLOCKED", objective: "Blocked task" }],
          nextCursor: "blocked-cursor-1",
        },
      },
      {
        status: "WAITING_USER",
        ok: true,
        page: {
          items: [{ taskId: "waiting-2", status: "WAITING_USER", objective: "Waiting task" }],
          nextCursor: "",
        },
      },
    ],
  );

  assert.deepEqual(result.items, []);
  assert.deepEqual(result.pages, retained);
  assert.equal(result.failed, true);
});

test("pagination loading keeps its accessible button label and announces busy state", () => {
  const html = renderToStaticMarkup(createElement(TaskAttentionLoadMoreButton, {
    loading: true,
    disabled: true,
    onLoadMore: () => {},
  }));

  assert.match(html, /<button[^>]*aria-busy="true"[^>]*>Load more<\/button>/);
  assert.match(html, /<p role="status">Loading more Tasks…<\/p>/);
});
