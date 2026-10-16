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
  optimizeDeps: { noDiscovery: true, include: [] },
  server: { middlewareMode: true, hmr: false },
  appType: "custom",
});
const { isValidNeedsYouPage, mergeNeedsYouPage } = await vite.ssrLoadModule(
  "/src/needs-you/task-attention-state.ts",
);
const { NeedsYouFilter, NeedsYouItemCard } = await vite.ssrLoadModule(
  "/src/needs-you/TaskAttentionPage.tsx",
);
after(async () => vite.close());

function item(overrides = {}) {
  return {
    itemId: "needs-1",
    kind: "TASK_BLOCKER",
    status: "OPEN",
    approvalId: null,
    userRequestId: null,
    taskId: "task-1",
    blockerId: "blocker-1",
    title: "Review the draft",
    safeSummary: "A Task is waiting for your review.",
    createdAt: "2026-10-14T10:00:00Z",
    resolvedAt: null,
    ...overrides,
  };
}

test("Needs You pages validate the requested status and strictly advancing cursor", () => {
  assert.equal(isValidNeedsYouPage("OPEN", { items: [item()], nextCursor: "next-1" }, null), true);
  assert.equal(isValidNeedsYouPage("OPEN", { items: [item()], nextCursor: "next-1" }, "prior"), true);
  assert.equal(isValidNeedsYouPage("OPEN", { items: [item()], nextCursor: "prior" }, "prior"), false);
  assert.equal(isValidNeedsYouPage("OPEN", { items: [item({ status: "RESOLVED" })], nextCursor: null }, null), false);
  assert.equal(isValidNeedsYouPage("RESOLVED", { items: [item()], nextCursor: null }, null), false);
  assert.equal(isValidNeedsYouPage("OPEN", { items: [item({ kind: "SECRET" })], nextCursor: null }, null), false);
  assert.equal(isValidNeedsYouPage("OPEN", { items: [item({ itemId: "" })], nextCursor: null }, null), false);
  assert.equal(isValidNeedsYouPage("OPEN", { items: [item()], nextCursor: "" }, null), false);
  assert.equal(isValidNeedsYouPage("OPEN", { items: [item(), item()], nextCursor: null }, null), false);
});

test("Needs You pagination retains stable item identity and appends only unseen rows", () => {
  const result = mergeNeedsYouPage(
    [item(), item({ itemId: "needs-2", taskId: "task-2" })],
    { items: [item({ title: "Updated duplicate" }), item({ itemId: "needs-3", taskId: "task-3" })], nextCursor: null },
  );

  assert.deepEqual(result.items.map(entry => entry.itemId), ["needs-1", "needs-2", "needs-3"]);
  assert.equal(result.items[0].title, "Review the draft", "a duplicate page item cannot rewrite an already rendered record");
  assert.equal(result.nextCursor, null);
});

test("the Open and Resolved history filter exposes its selected state to assistive technology", () => {
  const html = renderToStaticMarkup(createElement(NeedsYouFilter, {
    status: "RESOLVED",
    onChange: () => {},
  }));

  assert.match(html, /<fieldset[^>]*>/);
  assert.match(html, /<legend>Show Needs You items<\/legend>/);
  assert.match(html, /value="OPEN"/);
  assert.match(html, /checked="" value="RESOLVED"/);
});

test("Approval and UserRequest entries are display-only even when linked to a Task", () => {
  for (const kind of ["APPROVAL", "USER_REQUEST"]) {
    const html = renderToStaticMarkup(createElement(NeedsYouItemCard, {
      item: item({ kind, approvalId: kind === "APPROVAL" ? "approval-1" : null, userRequestId: kind === "USER_REQUEST" ? "request-1" : null }),
      onOpenTask: () => {},
    }));

    assert.match(html, /<article/);
    if (kind === "APPROVAL") {
      assert.match(html, /Approval details remain on their owning review surface\./);
      assert.doesNotMatch(html, /<button/);
    } else {
      assert.match(html, /<button[^>]*>View request details<\/button>/);
      assert.match(html, /Opening this request does not answer or dismiss it\./);
    }
  }
});

test("only a Task blocker offers a non-mutating link to its current Task detail", () => {
  const html = renderToStaticMarkup(createElement(NeedsYouItemCard, {
    item: item(),
    onOpenTask: () => {},
  }));

  assert.match(html, /<button[^>]*type="button"/);
  assert.match(html, /Open current Task/);
  assert.match(html, /Opening the Task does not resolve this blocker\./);
  assert.match(html, /Task blocker/);
  assert.match(html, />Open</);
});

test("a Task blocker without a Task identity has no misleading navigation action", () => {
  const html = renderToStaticMarkup(createElement(NeedsYouItemCard, {
    item: item({ taskId: null }),
    onOpenTask: () => {},
  }));

  assert.doesNotMatch(html, /<button/);
  assert.match(html, /Task details are unavailable for this item\./);
});
