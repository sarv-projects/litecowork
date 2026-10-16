import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { createServer as createHttpServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, test } from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import react from "@vitejs/plugin-react";
import { createServer } from "vite";

const viteCacheDir = await mkdtemp(join(tmpdir(), "litecowork-task-attention-vite-"));
const vite = await createServer({
  configFile: false,
  root: new URL("..", import.meta.url).pathname,
  cacheDir: viteCacheDir,
  plugins: [react()],
  optimizeDeps: { noDiscovery: true, include: ["react", "react-dom/client"] },
  server: { middlewareMode: true, hmr: false, ws: false },
  appType: "custom",
});
const { mergeTaskAttentionPages, TaskAttentionLoadMoreButton, TaskAttentionPaginationGate } = await vite.ssrLoadModule(
  "/src/needs-you/TaskAttentionPage.tsx",
);
after(async () => {
  await vite.close();
  await rm(viteCacheDir, { recursive: true, force: true });
});

const chrome = process.env.LITECOWORK_CHROME
  ?? ["/usr/bin/google-chrome", "/usr/bin/google-chrome-stable", "/usr/bin/chromium"]
    .find(candidate => existsSync(candidate));

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

test("mounted Needs You pagination survives refresh and Workspace invalidation while requests are pending", async context => {
  if (!chrome) {
    context.skip("Set LITECOWORK_CHROME or install Chrome/Chromium to run the mounted-page regression.");
    return;
  }

  const html = `<!doctype html><html><head><meta charset="utf-8"></head><body><div id="root"></div>
<script type="module">
window.__calls = [];
window.__pending = [];
window.__TAURI_INTERNALS__ = { invoke(command, args) {
  window.__calls.push({ command, args });
  return new Promise((resolve, reject) => window.__pending.push({ resolve, reject }));
} };
import React from "react";
import { createRoot } from "react-dom/client";
const { TaskAttentionPage } = await import("/src/needs-you/TaskAttentionPage.tsx");
let workspaceId = "workspace-a";
let operatorReady = true;
const root = createRoot(document.querySelector("#root"));
function render() {
  root.render(React.createElement(TaskAttentionPage, {
    workspaceId,
    operatorReady,
    onOpenTask() {},
  }));
}
window.setWorkspace = value => { workspaceId = value; render(); };
window.setOperatorReady = value => { operatorReady = value; render(); };
window.resolveCall = (index, value) => window.__pending[index].resolve(value);
window.rejectCall = (index, message) => window.__pending[index].reject(new Error(message));
render();
window.__mounted = true;
</script></body></html>`;

  let httpServer;
  let chromeProcess;
  let socket;
  let profile;
  let stderr = "";
  let spawnError = null;
  let exit = null;
  let nextCommandId = 0;
  const pendingCommands = new Map();
  const diagnostics = [];

  function cdp(method, params = {}) {
    const id = ++nextCommandId;
    const response = new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        pendingCommands.delete(id);
        reject(new Error(`Chrome DevTools timed out on ${method}`));
      }, 8000);
      pendingCommands.set(id, message => {
        clearTimeout(timer);
        message.error ? reject(new Error(message.error.message)) : resolve(message.result);
      });
    });
    socket.send(JSON.stringify({ id, method, params }));
    return response;
  }

  async function evaluate(expression) {
    const response = await cdp("Runtime.evaluate", {
      expression,
      returnByValue: true,
      awaitPromise: true,
      userGesture: true,
    });
    if (response.exceptionDetails) {
      throw new Error(response.exceptionDetails.exception?.description ?? response.exceptionDetails.text);
    }
    return response.result.value;
  }

  async function until(check, label, timeout = 8000) {
    const deadline = Date.now() + timeout;
    while (Date.now() < deadline) {
      const value = await check();
      if (value) return value;
      await new Promise(resolve => setTimeout(resolve, 40));
    }
    throw new Error(`Timed out waiting for ${label}`);
  }

  async function waitForChromePage() {
    const deadline = Date.now() + 30000;
    const portFile = join(profile, "DevToolsActivePort");
    while (Date.now() < deadline) {
      if (spawnError || exit) throw new Error(`Chrome exited before DevTools was ready: ${stderr}`);
      try {
        const port = Number((await readFile(portFile, "utf8")).split(/\r?\n/, 1)[0]);
        if (Number.isInteger(port) && port > 0 && port <= 65535) {
          const response = await fetch(`http://127.0.0.1:${port}/json`, { signal: AbortSignal.timeout(1000) });
          if (response.ok) {
            const page = (await response.json()).find(entry => entry.type === "page");
            if (page) return page;
          }
        }
      } catch {}
      await new Promise(resolve => setTimeout(resolve, 80));
    }
    throw new Error(`Chrome DevTools did not become ready: ${stderr}`);
  }

  async function click(label) {
    const clicked = await evaluate(`(() => {
      const button = [...document.querySelectorAll("button")].find(candidate => candidate.textContent.trim() === ${JSON.stringify(label)});
      if (!button || button.disabled) return false;
      button.click();
      return true;
    })()`);
    assert.equal(clicked, true, `${label} is rendered and enabled`);
  }

  async function callCount() {
    return evaluate("window.__calls.length");
  }

  async function settle(index, value, rejected = false) {
    await evaluate(`window.${rejected ? "rejectCall" : "resolveCall"}(${index}, ${JSON.stringify(value)})`);
  }

  const item = id => ({
    itemId: id,
    kind: "TASK_BLOCKER",
    status: "OPEN",
    approvalId: null,
    userRequestId: null,
    taskId: "task-a",
    blockerId: `blocker-${id}`,
    title: id,
    safeSummary: "A persisted item needs attention.",
    createdAt: "2026-10-14T10:00:00Z",
    resolvedAt: null,
  });

  try {
    vite.middlewares.use("/__task-attention-pagination", async (_request, response, next) => {
      try {
        response.setHeader("content-type", "text/html; charset=utf-8");
        response.end(await vite.transformIndexHtml("/__task-attention-pagination", html));
      } catch (error) {
        next(error);
      }
    });
    httpServer = createHttpServer(vite.middlewares);
    await new Promise((resolve, reject) => {
      httpServer.once("error", reject);
      httpServer.listen(0, "127.0.0.1", resolve);
    });
    const address = httpServer.address();
    assert.ok(address && typeof address === "object");
    profile = await mkdtemp(join(tmpdir(), "litecowork-needs-you-pagination-"));
    chromeProcess = spawn(chrome, [
      "--headless=new", "--no-sandbox", "--disable-gpu", "--disable-dev-shm-usage",
      "--remote-debugging-port=0", "--remote-allow-origins=*", `--user-data-dir=${profile}`, "about:blank",
    ], { detached: process.platform !== "win32", stdio: ["ignore", "ignore", "pipe"] });
    chromeProcess.stderr?.setEncoding("utf8");
    chromeProcess.stderr?.on("data", chunk => { stderr = `${stderr}${chunk}`.slice(-4096); });
    chromeProcess.once("error", error => { spawnError = error; });
    chromeProcess.once("exit", (code, signal) => { exit = { code, signal }; });
    const page = await waitForChromePage();
    socket = new WebSocket(page.webSocketDebuggerUrl);
    await new Promise((resolve, reject) => {
      socket.addEventListener("open", resolve, { once: true });
      socket.addEventListener("error", reject, { once: true });
    });
    socket.addEventListener("message", event => {
      const message = JSON.parse(event.data);
      if (message.method === "Runtime.exceptionThrown") {
        diagnostics.push(message.params.exceptionDetails.exception?.description ?? message.params.exceptionDetails.text);
      }
      if (message.method === "Log.entryAdded") diagnostics.push(message.params.entry.text);
      if (message.id) pendingCommands.get(message.id)?.(message);
    });
    await cdp("Page.enable");
    await cdp("Runtime.enable");
    await cdp("Log.enable");
    await cdp("Page.navigate", { url: `http://127.0.0.1:${address.port}/__task-attention-pagination` });
    try {
      await until(() => evaluate("window.__mounted === true"), "mounted Needs You page");
    } catch (error) {
      const pageState = await evaluate("document.readyState + ': ' + document.body.innerText + ': ' + document.body.innerHTML.slice(0, 1000)").catch(failure => failure.message);
      throw new Error(`${error.message}; diagnostics=${diagnostics.join(" | ")}; page=${pageState}`);
    }
    await until(async () => (await callCount()) === 1, "initial Workspace request");

    await settle(0, { items: [item("a-initial")], nextCursor: "a-initial-cursor" });
    await until(() => evaluate("document.body.innerText.includes('a-initial')"), "initial page");

    // A pending page must visibly own the busy state; a failed page releases it so
    // the same cursor can be retried.
    await click("Load more");
    await until(async () => (await callCount()) === 2, "first pagination request");
    assert.equal(await evaluate("document.querySelector('.needs-you-load-more button')?.getAttribute('aria-busy')"), "true");
    assert.equal(await evaluate("document.body.innerText.includes('Loading more Needs You items…')"), true);
    assert.equal(await evaluate("window.__calls[1].args.cursor"), "a-initial-cursor");
    await settle(1, "offline", true);
    await until(() => evaluate("document.body.innerText.includes('More Needs You items could not be loaded') && document.querySelector('.needs-you-load-more button')?.disabled === false"), "failed pagination retry state");
    assert.equal(await evaluate("document.querySelector('.needs-you-load-more button')?.getAttribute('aria-busy')"), "false");
    await click("Load more");
    await until(async () => (await callCount()) === 3, "retry request");
    assert.equal(await evaluate("window.__calls[2].args.cursor"), "a-initial-cursor", "failure retains the last usable cursor");
    await settle(2, { items: [item("a-retried")], nextCursor: null });
    await until(() => evaluate("document.body.innerText.includes('a-retried') && !document.querySelector('.needs-you-load-more button')"), "successful retry page");

    // Refresh invalidates the old request. Its late response must neither append
    // rows nor unlock a newer page request.
    await click("Refresh");
    await until(async () => (await callCount()) === 4, "refresh seed request");
    await settle(3, { items: [item("a-refresh-seed")], nextCursor: "a-refresh-cursor" });
    await until(() => evaluate("document.body.innerText.includes('a-refresh-seed')"), "refresh seed page");
    await click("Load more");
    await until(async () => (await callCount()) === 5, "old refresh-generation page");
    assert.equal(await evaluate("window.__calls[4].args.cursor"), "a-refresh-cursor");
    await click("Refresh");
    await until(async () => (await callCount()) === 6, "new refresh-generation first page");
    assert.equal(await evaluate("document.body.innerText.includes('Checking the latest Needs You items')"), true, "refresh truthfully marks the retained page while pending");
    await settle(5, { items: [item("a-refreshed")], nextCursor: "a-refreshed-cursor" });
    await until(() => evaluate("document.body.innerText.includes('a-refreshed') && !document.body.innerText.includes('a-refresh-seed')"), "new refresh-generation page");
    await click("Load more");
    await until(async () => (await callCount()) === 7, "new refresh-generation page request");
    assert.equal(await evaluate("window.__calls[6].args.cursor"), "a-refreshed-cursor");
    assert.equal(await evaluate("document.querySelector('.needs-you-load-more button')?.getAttribute('aria-busy')"), "true");
    await settle(4, { items: [item("a-stale-refresh-page")], nextCursor: "stale-cursor" });
    await new Promise(resolve => setTimeout(resolve, 100));
    assert.equal(await evaluate("document.body.innerText.includes('a-stale-refresh-page')"), false);
    assert.equal(await evaluate("document.querySelector('.needs-you-load-more button')?.getAttribute('aria-busy')"), "true", "old completion cannot unlock the refreshed generation");
    await settle(6, "offline", true);
    await until(() => evaluate("document.body.innerText.includes('More Needs You items could not be loaded') && document.querySelector('.needs-you-load-more button')?.disabled === false"), "refreshed page releases busy state after its own failure");
    await click("Load more");
    await until(async () => (await callCount()) === 8, "retry after refresh race");
    assert.equal(await evaluate("window.__calls[7].args.cursor"), "a-refreshed-cursor", "stale cursor cannot replace the refreshed cursor");
    await settle(7, { items: [item("a-current-retry")], nextCursor: "a-after-retry-cursor" });
    await until(() => evaluate("document.body.innerText.includes('a-current-retry')"), "refreshed page retry result");

    // A Workspace switch also releases the prior generation. Resolve a new page,
    // start its pagination, then complete the old Workspace request late.
    await click("Load more");
    await until(async () => (await callCount()) === 9, "old Workspace pending page");
    await evaluate("window.setWorkspace('workspace-b')");
    await until(async () => (await callCount()) === 10, "new Workspace first page");
    assert.equal(await evaluate("document.body.innerText.includes('a-current-retry')"), false, "prior Workspace items are hidden immediately");
    await settle(9, { items: [item("b-initial")], nextCursor: "b-cursor" });
    await until(() => evaluate("document.body.innerText.includes('b-initial')"), "new Workspace page");
    await click("Load more");
    await until(async () => (await callCount()) === 11, "new Workspace pending page");
    assert.equal(await evaluate("window.__calls[10].args.workspaceId"), "workspace-b");
    assert.equal(await evaluate("document.querySelector('.needs-you-load-more button')?.getAttribute('aria-busy')"), "true");
    await settle(8, { items: [item("a-stale-workspace-page")], nextCursor: "a-stale-workspace-cursor" });
    await new Promise(resolve => setTimeout(resolve, 100));
    assert.equal(await evaluate("document.body.innerText.includes('a-stale-workspace-page')"), false);
    assert.equal(await evaluate("document.body.innerText.includes('b-initial')"), true);
    assert.equal(await evaluate("document.querySelector('.needs-you-load-more button')?.getAttribute('aria-busy')"), "true", "old Workspace completion cannot unlock the new Workspace page");

    // A Runtime disconnect invalidates a pending page. Reconnect starts a new
    // first-page read, and the old page's late completion must not strand or
    // release a subsequent pagination request.
    await evaluate("window.setOperatorReady(false)");
    await until(() => evaluate("document.body.innerText.includes('Showing the last saved open items') && document.querySelector('.needs-you-load-more button')?.getAttribute('aria-busy') === 'false'"), "Runtime disconnect releases pending pagination busy state");
    await evaluate("window.setOperatorReady(true)");
    await until(async () => (await callCount()) === 12, "first-page request after Runtime reconnect");
    assert.equal(await evaluate("document.body.innerText.includes('Checking the latest Needs You items')"), true, "reconnect refresh labels the retained snapshot");
    assert.equal(await evaluate("document.querySelector('.needs-you-load-more button')?.getAttribute('aria-busy')"), "false", "first-page refresh does not claim pagination is busy");
    await settle(11, { items: [item("b-reconnected")], nextCursor: "b-reconnected-cursor" });
    await until(() => evaluate("document.body.innerText.includes('b-reconnected')"), "fresh first page after Runtime reconnect");
    await click("Load more");
    await until(async () => (await callCount()) === 13, "pagination after Runtime reconnect");
    assert.equal(await evaluate("window.__calls[12].args.cursor"), "b-reconnected-cursor");
    assert.equal(await evaluate("document.querySelector('.needs-you-load-more button')?.getAttribute('aria-busy')"), "true");
    await settle(10, { items: [item("b-stale-disconnect-page")], nextCursor: "stale-disconnect-cursor" });
    await new Promise(resolve => setTimeout(resolve, 100));
    assert.equal(await evaluate("document.body.innerText.includes('b-stale-disconnect-page')"), false, "late pre-disconnect page is ignored");
    assert.equal(await evaluate("document.querySelector('.needs-you-load-more button')?.getAttribute('aria-busy')"), "true", "pre-disconnect completion cannot release pagination after reconnect");
    await settle(12, { items: [item("b-later")], nextCursor: null });
    await until(() => evaluate("document.body.innerText.includes('b-later') && !document.querySelector('.needs-you-load-more button')"), "new Workspace pagination completion after Runtime reconnect");
  } finally {
    try { socket?.close(); } catch {}
    if (chromeProcess?.pid) {
      const target = process.platform === "win32" ? chromeProcess.pid : -chromeProcess.pid;
      try { process.kill(target, "SIGTERM"); } catch {}
      await Promise.race([
        new Promise(resolve => {
          if (chromeProcess.exitCode !== null || chromeProcess.signalCode !== null) resolve();
          else chromeProcess.once("exit", resolve);
        }),
        new Promise(resolve => setTimeout(resolve, 3000)),
      ]);
      if (chromeProcess.exitCode === null && chromeProcess.signalCode === null) {
        try { process.kill(target, "SIGKILL"); } catch {}
      }
    }
    if (httpServer?.listening) await new Promise(resolve => httpServer.close(resolve));
    if (profile) await rm(profile, { recursive: true, force: true });
  }
});
