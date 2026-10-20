import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { after, test } from "node:test";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createServer as createHttpServer } from "node:http";
import react from "@vitejs/plugin-react";
import { createServer } from "vite";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const chrome = process.env.LITECOWORK_CHROME ?? "/usr/bin/google-chrome";
const CHROME_STDERR_LIMIT = 4096;
const CHROME_STARTUP_TIMEOUT_MS = 30_000;
const PAGE_MOUNT_TIMEOUT_MS = 30_000;
const wait = ms => new Promise(resolveWait => setTimeout(resolveWait, ms));
async function until(check, label, timeout = 8000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const value = await check();
    if (value) return value;
    await wait(50);
  }
  throw new Error(`Timed out waiting for ${label}`);
}

function appendBoundedStderr(current, chunk) {
  let bounded = Buffer.concat([Buffer.from(current), Buffer.from(chunk)])
    .subarray(-CHROME_STDERR_LIMIT)
    .toString("utf8");
  while (Buffer.byteLength(bounded) > CHROME_STDERR_LIMIT) bounded = bounded.slice(1);
  return bounded;
}

function formatChromeStartupFailure(reason, { spawnError, exit, stderr }) {
  const details = [];
  if (spawnError) details.push(`spawn error: ${spawnError.message}`);
  if (exit) details.push(exit.signal ? `child exited on ${exit.signal}` : `child exited with exit code ${exit.code}`);
  if (stderr.trim()) details.push(`stderr: ${stderr.trim()}`);
  return `Chrome startup failed: ${reason}${details.length ? ` (${details.join("; ")})` : ""}`;
}

function dependentBrowserSkipReason(setupFailure, sessionSocket) {
  if (setupFailure) return `The preceding browser interaction test failed: ${setupFailure}`;
  if (!sessionSocket) return "The preceding browser interaction test did not initialize its shared browser session";
  return null;
}

async function waitForChromePage(port, getChildState, getStderr, timeout = CHROME_STARTUP_TIMEOUT_MS) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const { spawnError, exit } = getChildState();
    if (spawnError || exit) {
      throw new Error(formatChromeStartupFailure("the DevTools page did not become ready", {
        spawnError, exit, stderr: getStderr(),
      }));
    }
    try {
      const response = await fetch(`http://127.0.0.1:${port}/json`, { signal: AbortSignal.timeout(1000) });
      if (response.ok) {
        const page = (await response.json()).find(entry => entry.type === "page");
        if (page) return page;
      }
    } catch {}
    await wait(100);
  }
  const { spawnError, exit } = getChildState();
  throw new Error(formatChromeStartupFailure(`no DevTools page after ${timeout}ms`, {
    spawnError, exit, stderr: getStderr(),
  }));
}

async function waitForChromePageFromProfile(profile, getChildState, getStderr, timeout = CHROME_STARTUP_TIMEOUT_MS) {
  const deadline = Date.now() + timeout;
  const portFile = join(profile, "DevToolsActivePort");
  while (Date.now() < deadline) {
    const { spawnError, exit } = getChildState();
    if (spawnError || exit) {
      throw new Error(formatChromeStartupFailure("the DevTools endpoint did not become ready", {
        spawnError, exit, stderr: getStderr(),
      }));
    }
    try {
      const port = Number((await readFile(portFile, "utf8")).split(/\r?\n/, 1)[0]);
      if (Number.isInteger(port) && port > 0 && port <= 65_535) {
        return await waitForChromePage(port, getChildState, getStderr, Math.max(1, deadline - Date.now()));
      }
    } catch {}
    await wait(100);
  }
  const { spawnError, exit } = getChildState();
  throw new Error(formatChromeStartupFailure(`no DevTools page after ${timeout}ms`, {
    spawnError, exit, stderr: getStderr(),
  }));
}

test("Chrome startup failure diagnostics bound stderr and include child exit details", () => {
  const stderr = appendBoundedStderr("x".repeat(CHROME_STDERR_LIMIT), "Chrome failed to bind");
  const message = formatChromeStartupFailure("DevTools did not become ready", {
    spawnError: null,
    exit: { code: 1, signal: null },
    stderr,
  });

  assert.ok(Buffer.byteLength(stderr) <= CHROME_STDERR_LIMIT);
  assert.match(stderr, /Chrome failed to bind$/);
  assert.match(message, /DevTools did not become ready/);
  assert.match(message, /exit code 1/);
  assert.match(message, /Chrome failed to bind/);
});

test("dependent browser interaction reports the preceding setup failure instead of cascading", () => {
  assert.equal(dependentBrowserSkipReason(null, {}), null);
  assert.match(dependentBrowserSkipReason("Chrome did not start", null), /preceding browser interaction test failed: Chrome did not start/);
  assert.match(dependentBrowserSkipReason(null, null), /did not initialize its shared browser session/);
});

const html = `<!doctype html><html><head><meta charset="utf-8"></head><body><div id="root"></div>
<script type="module">
window.__pending = []; window.__calls = []; window.__openedTasks = [];
window.__TAURI_INTERNALS__ = { invoke(command, args) { window.__calls.push({command,args}); return new Promise((resolve,reject) => window.__pending.push({resolve,reject})); } };
import React from 'react';
import { createRoot } from 'react-dom/client';
const { TaskAttentionPage } = await import('/src/needs-you/TaskAttentionPage.tsx');
let workspace = 'workspace-a';
let operatorReady = true;
const root = createRoot(document.getElementById('root'));
window.setWorkspace = value => { workspace = value; render(); };
window.setOperatorReady = value => { operatorReady = value; render(); };
window.resolveCall = (index, value) => window.__pending[index].resolve(value);
window.rejectCall = (index, message) => window.__pending[index].reject(new Error(message));
function render() { root.render(React.createElement(TaskAttentionPage, {workspaceId: workspace, operatorReady, onOpenTask: id => window.__openedTasks.push(id)})); }
render(); window.__mounted = true;
</script></body></html>`;

let vite, httpServer, chromeProcess, socket, tempProfile, tempViteCache;
let chromeSpawnError = null;
let chromeExit = null;
let chromeStderr = "";
let browserSetupFailure = null;
let nextId = 0;
const pending = new Map();
const diagnostics = [];
function cdp(method, params = {}) {
  const id = ++nextId;
  const result = new Promise((resolveResult, reject) => {
    const timer = setTimeout(() => { pending.delete(id); reject(new Error(`CDP timeout: ${method}`)); }, 7000);
    pending.set(id, message => { clearTimeout(timer); message.error ? reject(new Error(message.error.message)) : resolveResult(message.result); });
  });
  socket.send(JSON.stringify({ id, method, params }));
  return result;
}
async function evaluate(expression) {
  const result = await cdp("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true, userGesture: true });
  if (result.exceptionDetails) throw new Error(result.exceptionDetails.exception?.description ?? result.exceptionDetails.text);
  return result.result.value;
}
async function click(label) {
  assert.equal(await evaluate(`(() => { const b=[...document.querySelectorAll('button')].find(x=>x.textContent.trim()===${JSON.stringify(label)}); if(!b)return false;b.click();return true;})()`), true, `button ${label} exists`);
}
function item(id, kind = "TASK_BLOCKER", status = "OPEN", taskId = "task-a") {
  return { itemId: id, kind, status, approvalId: kind === "APPROVAL" ? "approval-a" : null, userRequestId: kind === "USER_REQUEST" ? "request-a" : null, taskId: kind === "TASK_BLOCKER" ? taskId : null, blockerId: kind === "TASK_BLOCKER" ? "blocker-a" : null, title: id, safeSummary: "A persisted item needs attention.", createdAt: "2026-10-14T10:00:00Z", resolvedAt: status === "RESOLVED" ? "2026-10-14T11:00:00Z" : null };
}
function approval(status = "PENDING", version = 2) {
  return { approvalId: "approval-a", taskId: "task-a", requestedByAttempt: null, kind: "EXTERNAL_MUTATION", actionSummary: "Publish the approved document", targetRef: { workspaceId: "workspace-b", resourceId: "resource-a", revisionId: "revision-a" }, scopeDigest: `sha256:${"a".repeat(64)}`, actionDigest: `sha256:${"b".repeat(64)}`, risk: "SENSITIVE", requiredAssurance: "APPROVE_SENSITIVE", status, requestedAt: "2026-10-14T10:00:00Z", expiresAt: null, resolvedBy: status === "PENDING" ? null : { principalId: "owner-a", kind: "USER" }, resolvedAt: status === "PENDING" ? null : "2026-10-14T11:00:00Z", version };
}
async function callCount() { return evaluate("window.__calls.length"); }
async function settle(index, result, error = false) { await evaluate(`window.${error ? "rejectCall" : "resolveCall"}(${index}, ${JSON.stringify(result)})`); }

after(async () => {
  try { socket?.close(); } catch {}
  if (chromeProcess?.pid) {
    try { process.kill(-chromeProcess.pid, "SIGTERM"); } catch {}
    await new Promise(resolveExit => { if (chromeProcess.exitCode !== null || chromeProcess.signalCode !== null) resolveExit(); else chromeProcess.once("exit", resolveExit); });
  }
  if (httpServer?.listening) await new Promise(resolveClose => httpServer.close(resolveClose));
  await vite?.close();
  if (tempProfile) await rm(tempProfile, { recursive: true, force: true });
  if (tempViteCache) await rm(tempViteCache, { recursive: true, force: true });
});

test("mounted Needs You page fences Workspace switches and renders selected status", async () => {
  try {
    if (!chrome) throw new Error("Set LITECOWORK_CHROME to a local Chrome/Chromium executable.");
    tempViteCache = await mkdtemp(join(tmpdir(), "litecowork-needs-you-vite-"));
    vite = await createServer({
      configFile: false, root: packageRoot,
      cacheDir: tempViteCache,
      server: { middlewareMode: true, hmr: false, ws: false },
      appType: "custom",
      optimizeDeps: { noDiscovery: true, include: ["react", "react-dom/client"] },
      plugins: [react(), { name: "needs-you-test-document", configureServer(server) { server.middlewares.use("/__needs-you-test", async (_req, res, next) => { try { res.setHeader("content-type", "text/html"); res.end(await server.transformIndexHtml("/__needs-you-test", html)); } catch (error) { next(error); } }); } }],
    });
    httpServer = createHttpServer(vite.middlewares);
    await new Promise((resolveListen, rejectListen) => {
      httpServer.once("error", rejectListen);
      httpServer.listen(0, "127.0.0.1", resolveListen);
    });
    const httpAddress = httpServer.address();
    assert.ok(httpAddress && typeof httpAddress === "object", "the harness HTTP server listens on an OS-assigned port");
    const port = httpAddress.port;
    tempProfile = await mkdtemp(join(tmpdir(), "litecowork-needs-you-ui-"));
    chromeProcess = spawn(chrome, ["--headless=new", "--no-sandbox", "--disable-gpu", "--disable-dev-shm-usage", "--remote-debugging-port=0", "--remote-allow-origins=*", `--user-data-dir=${tempProfile}`, "about:blank"], { detached: process.platform !== "win32", stdio: ["ignore", "ignore", "pipe"] });
    chromeProcess.stderr?.setEncoding("utf8");
    chromeProcess.stderr?.on("data", chunk => { chromeStderr = appendBoundedStderr(chromeStderr, chunk); });
    chromeProcess.once("error", error => { chromeSpawnError = error; });
    chromeProcess.once("exit", (code, signal) => { chromeExit = { code, signal }; });
    const target = await waitForChromePageFromProfile(tempProfile, () => ({ spawnError: chromeSpawnError, exit: chromeExit }), () => chromeStderr);
    socket = new WebSocket(target.webSocketDebuggerUrl);
    await new Promise((resolveSocket, reject) => { socket.addEventListener("open", resolveSocket, { once: true }); socket.addEventListener("error", reject, { once: true }); });
    socket.addEventListener("message", event => { const message = JSON.parse(event.data); if (message.method === "Runtime.exceptionThrown") diagnostics.push(message.params.exceptionDetails.exception?.description ?? message.params.exceptionDetails.text); if (message.method === "Log.entryAdded") diagnostics.push(message.params.entry.text); if (message.id) pending.get(message.id)?.(message); });
    await cdp("Page.enable"); await cdp("Runtime.enable"); await cdp("Log.enable");
    await cdp("Page.navigate", { url: `http://127.0.0.1:${port}/__needs-you-test` });
    try {
      await until(async () => { try { return await evaluate("window.__mounted === true"); } catch { return false; } }, "mounted React page", PAGE_MOUNT_TIMEOUT_MS);
    } catch (error) {
      const pageState = await evaluate("document.readyState + ': ' + document.body.innerText + ': ' + document.body.innerHTML.slice(0, 400)").catch(failure => failure.message);
      throw new Error(`${error.message}; diagnostics=${diagnostics.join(" | ")}; page=${pageState}`);
    }
    const entryModule = await evaluate("[...document.querySelectorAll('script[type=module][src]')].map(script => script.src).find(src => src.includes('html-proxy')) ?? null");
    assert.ok(entryModule, "Vite serves the inline module through an external entry URL");
    assert.match(entryModule, /__needs-you-test/, "the test route keeps its own Vite module identity");
    assert.doesNotMatch(entryModule, /index\.html\?html-proxy/, "the mounted middleware route does not alias its module to index.html");
    await until(async () => (await callCount()) >= 1, "initial Workspace request");
    await evaluate("window.setWorkspace('workspace-b')");
    await until(async () => (await callCount()) >= 2, "new Workspace request");
    await settle(1, { items: [item("workspace-b-row")], nextCursor: null });
    await until(() => evaluate("document.body.innerText.includes('workspace-b-row')"), "new Workspace item");
    await settle(0, { items: [item("stale-workspace-a-row")], nextCursor: null });
    await wait(150);
    assert.equal(await evaluate("document.body.innerText.includes('stale-workspace-a-row')"), false, "late prior-Workspace response is ignored");

    await evaluate("(() => { const input=document.querySelector('input[value=RESOLVED]'); input.click(); })()");
    await until(async () => (await callCount()) >= 3, "resolved status request");
    assert.equal(await evaluate("document.querySelector('input[value=RESOLVED]').checked"), true);
    await settle(2, { items: [item("resolved-row", "TASK_BLOCKER", "RESOLVED")], nextCursor: null });
    await until(() => evaluate("document.body.innerText.includes('resolved-row')"), "resolved history row");
    assert.equal(await evaluate("document.body.innerText.includes('workspace-b-row')"), false, "status switch shows its own snapshot");
  } catch (error) {
    browserSetupFailure = error instanceof Error ? error.message : String(error);
    throw error;
  }
});

test("mounted page reports inbox failures and checks exact approvals before resolving them", async context => {
  // Reuse the browser/session initialized by the preceding test.
  const skipReason = dependentBrowserSkipReason(browserSetupFailure, socket);
  if (skipReason) {
    context.skip(skipReason);
    return;
  }
  await evaluate("(() => document.querySelector('input[value=OPEN]').click())()");
  await until(async () => (await callCount()) >= 4, "open status request");
  await settle(3, { items: [item("blocker-row"), item("approval-row", "APPROVAL")], nextCursor: "cursor-1" });
  await until(() => evaluate("document.body.innerText.includes('blocker-row') && document.body.innerText.includes('approval-row')"), "open inbox rows");
  await click("Refresh");
  await until(async () => (await callCount()) >= 5, "refresh request");
  await settle(4, "offline", true);
  await until(() => evaluate("document.body.innerText.includes('Showing the last saved open items')"), "truthful stale snapshot notice");
  assert.equal(await evaluate("document.body.innerText.includes('blocker-row')"), true, "refresh error retains same-Workspace items");

  await click("Load more");
  await until(async () => (await callCount()) >= 6, "pagination request");
  assert.equal(await evaluate("document.querySelector('.needs-you-load-more button').getAttribute('aria-busy')"), "true");
  await settle(5, "offline", true);
  await until(() => evaluate("document.body.innerText.includes('More Needs You items could not be loaded') && document.querySelector('.needs-you-load-more button')?.disabled === false && document.querySelector('.needs-you-load-more button')?.getAttribute('aria-busy') === 'false'"), "pagination failure and released busy state");
  await click("Load more");
  await until(async () => (await callCount()) >= 7, "pagination retry request");
  await settle(6, { items: [item("later-row")], nextCursor: null });
  await until(() => evaluate("document.body.innerText.includes('later-row')"), "pagination retry result");

  await click("Review decision");
  const approvalLoadIndex = await callCount() - 1;
  await until(() => evaluate("window.__calls[" + approvalLoadIndex + "]?.command === 'get_approval_detail'"), "approval detail request");
  assert.equal(await evaluate("window.__calls[" + approvalLoadIndex + "].args.workspaceId"), "workspace-b");
  await settle(approvalLoadIndex, approval());
  await until(() => evaluate("document.body.innerText.includes('Publish the approved document') && document.body.innerText.includes('revision-a')"), "exact approval action and target");
  assert.equal(await evaluate("document.body.innerText.includes('does not grant a reusable capability')"), true, "approval boundary is explained plainly");
  await click("Back to Needs You");
  await until(() => evaluate("!document.body.innerText.includes('Publish the approved document')"), "approval detail closes");
  assert.match(await evaluate("document.activeElement?.textContent.trim() ?? ''"), /Review decision/, "Back restores focus to the opening Approval row");
  const approvalRetryStart = await callCount();
  await click("Review decision");
  await until(() => evaluate("window.__calls.slice(" + approvalRetryStart + ").some(call => call.command === 'get_approval_detail')"), "approval detail reopened");
  const reopenedDetailIndex = await evaluate("window.__calls.findIndex((call, index) => index >= " + approvalRetryStart + " && call.command === 'get_approval_detail')");
  await settle(reopenedDetailIndex, approval());
  await until(() => evaluate("document.body.innerText.includes('Publish the approved document')"), "approval detail after reopen");

  await evaluate("window.setOperatorReady(false)");
  assert.equal(await evaluate("[...document.querySelectorAll('button')].find(button => button.textContent.includes('Approve this action'))?.disabled"), true, "approval action is disabled while local Runtime is offline");
  const reconnectCallStart = await callCount();
  await evaluate("window.setOperatorReady(true)");
  await until(() => evaluate("window.__calls.slice(" + reconnectCallStart + ").some(call => call.command === 'get_approval_detail')"), "approval refresh after reconnect");
  const approvalReloadIndex = await evaluate("window.__calls.findIndex((call, index) => index >= " + reconnectCallStart + " && call.command === 'get_approval_detail')");
  const inboxReconnectIndex = await evaluate("window.__calls.findIndex((call, index) => index >= " + reconnectCallStart + " && call.command === 'list_needs_you')");
  await settle(approvalReloadIndex, approval());
  if (inboxReconnectIndex >= 0) await settle(inboxReconnectIndex, { items: [item("blocker-row"), item("approval-row", "APPROVAL")], nextCursor: null });
  const inboxCountBeforeApproval = await evaluate("window.__calls.filter(call => call.command === 'list_needs_you').length");
  await click("Approve this action");
  await until(() => evaluate("window.__calls.some(call => call.command === 'resolve_approval')"), "approval decision request");
  const resolveIndex = await evaluate("window.__calls.findIndex(call => call.command === 'resolve_approval')");
  const resolveArgs = await evaluate("window.__calls[" + resolveIndex + "].args");
  assert.equal(resolveArgs.workspaceId, "workspace-b");
  assert.equal(resolveArgs.approvalId, "approval-a");
  assert.equal(resolveArgs.expectedVersion, 2);
  assert.equal(resolveArgs.decision, "APPROVE");
  assert.equal(resolveArgs.confirmationDigest, approval().actionDigest);
  assert.match(resolveArgs.requestId, /^approval-ui-[0-9a-f-]{36}$/u);
  await settle(resolveIndex, approval("APPROVED", 3));
  await until(() => evaluate("window.__calls.filter(call => call.command === 'list_needs_you').length > " + inboxCountBeforeApproval), "Needs You refresh after approval");
  const approvalRefreshIndex = await evaluate("window.__calls.map((call, index) => ({call,index})).filter(entry => entry.call.command === 'list_needs_you').at(-1).index");
  await settle(approvalRefreshIndex, { items: [item("blocker-row")], nextCursor: null });
  await until(() => evaluate("!document.body.innerText.includes('Publish the approved document')"), "approval closes after successful decision");

  await evaluate("(() => { const article=[...document.querySelectorAll('article')].find(x=>x.innerText.includes('blocker-row')); article.querySelector('button').click(); })()");
  assert.deepEqual(await evaluate("window.__openedTasks"), ["task-a"], "only blocker navigation opens the Task");
});

test("mounted page ignores a stale pagination completion after refresh without unlocking the newer page", async context => {
  const skipReason = dependentBrowserSkipReason(browserSetupFailure, socket);
  if (skipReason) {
    context.skip(skipReason);
    return;
  }

  const firstRefreshIndex = await callCount();
  await click("Refresh");
  await until(async () => (await callCount()) >= firstRefreshIndex + 1, "initial page before pagination race");
  await settle(firstRefreshIndex, { items: [item("race-before-refresh")], nextCursor: "cursor-before-refresh" });
  await until(() => evaluate("document.body.innerText.includes('race-before-refresh') && document.querySelector('.needs-you-load-more button')?.disabled === false"), "initial race fixture page");

  await click("Load more");
  const stalePageIndex = firstRefreshIndex + 1;
  await until(async () => (await callCount()) >= stalePageIndex + 1, "held old pagination request");
  assert.equal(await evaluate("window.__calls[" + stalePageIndex + "].args.cursor"), "cursor-before-refresh", "old page starts from the first cursor");
  assert.equal(await evaluate("document.querySelector('.needs-you-load-more button')?.getAttribute('aria-busy')"), "true");

  const refreshedPageIndex = stalePageIndex + 1;
  await click("Refresh");
  await until(async () => (await callCount()) >= refreshedPageIndex + 1, "refreshed first-page request while old pagination is pending");
  await settle(refreshedPageIndex, { items: [item("race-after-refresh")], nextCursor: "cursor-after-refresh" });
  await until(() => evaluate("document.body.innerText.includes('race-after-refresh') && document.querySelector('button.quiet-button:not(.task-attention-more)')?.disabled === false"), "refreshed first page");
  assert.equal(await evaluate("document.body.innerText.includes('race-before-refresh')"), false, "refresh replaces the first-page snapshot");

  const currentPageIndex = refreshedPageIndex + 1;
  await click("Load more");
  await until(async () => (await callCount()) >= currentPageIndex + 1, "new-generation pagination request");
  assert.equal(await evaluate("window.__calls[" + currentPageIndex + "].args.cursor"), "cursor-after-refresh", "new page starts from the refreshed cursor");
  assert.equal(await evaluate("document.querySelector('.needs-you-load-more button')?.getAttribute('aria-busy')"), "true");

  await settle(stalePageIndex, { items: [item("stale-pagination-row")], nextCursor: "cursor-stale" });
  await wait(100);
  assert.equal(await evaluate("document.body.innerText.includes('stale-pagination-row')"), false, "late pagination rows are ignored");
  assert.equal(await evaluate("document.body.innerText.includes('race-after-refresh')"), true, "refreshed rows remain visible");
  assert.equal(await evaluate("document.querySelector('.needs-you-load-more button')?.getAttribute('aria-busy')"), "true", "old completion cannot clear the newer page busy state");
  assert.equal(await evaluate("document.querySelector('.needs-you-load-more button')?.disabled"), true, "the newer page remains non-retryable while its request is pending");

  await settle(currentPageIndex, "offline", true);
  await until(() => evaluate("document.body.innerText.includes('More Needs You items could not be loaded') && document.querySelector('.needs-you-load-more button')?.disabled === false && document.querySelector('.needs-you-load-more button')?.getAttribute('aria-busy') === 'false'"), "newer page failure releases only its own busy state");

  const retryPageIndex = currentPageIndex + 1;
  await click("Load more");
  await until(async () => (await callCount()) >= retryPageIndex + 1, "retry after the current page failure");
  assert.equal(await evaluate("window.__calls[" + retryPageIndex + "].args.cursor"), "cursor-after-refresh", "stale cursor cannot replace the refreshed cursor used for retry");
  await settle(retryPageIndex, { items: [item("race-retry-row")], nextCursor: null });
  await until(() => evaluate("document.body.innerText.includes('race-retry-row')"), "retry page after the race");
});

test("mounted Needs You labels its saved page stale while Runtime is offline and reconnecting", async context => {
  const skipReason = dependentBrowserSkipReason(browserSetupFailure, socket);
  if (skipReason) {
    context.skip(skipReason);
    return;
  }

  const seedIndex = await callCount();
  await click("Refresh");
  await until(async () => (await callCount()) >= seedIndex + 1, "saved page before Runtime disconnect");
  await settle(seedIndex, { items: [item("runtime-last-known")], nextCursor: "cursor-offline" });
  await until(() => evaluate("document.body.innerText.includes('runtime-last-known') && document.querySelector('.needs-you-load-more button')?.disabled === false"), "saved page with another cursor");

  await evaluate("window.setOperatorReady(false)");
  await until(() => evaluate("document.body.innerText.includes('Showing the last saved open items')"), "offline stale-list notice");
  assert.equal(await evaluate("document.body.innerText.includes('runtime-last-known')"), true, "offline preserves the same-Workspace saved page");
  assert.equal(await evaluate("document.body.innerText.includes('No open Needs You items')"), false, "offline never infers an empty inbox");
  assert.equal(await evaluate("document.querySelector('.needs-you-load-more button')?.disabled"), true, "pagination stays disabled without Runtime authority");
  assert.equal(await evaluate("document.querySelector('.needs-you-load-more button')?.getAttribute('aria-busy')"), "false", "disconnect releases any pagination busy state");

  const reconnectIndex = await callCount();
  await evaluate("window.setOperatorReady(true)");
  await until(async () => (await callCount()) >= reconnectIndex + 1, "first-page refresh after Runtime reconnect");
  assert.equal(await evaluate("document.body.innerText.includes('runtime-last-known')"), true, "the saved page stays visible while reconnect refresh is pending");
  assert.equal(await evaluate("document.body.innerText.includes('Showing the last saved open items')"), true, "the saved page remains marked stale until the refreshed response arrives");
  assert.equal(await evaluate("document.querySelector('.needs-you-load-more button')?.disabled"), true, "pagination remains disabled while the reconnect refresh is pending");

  await settle(reconnectIndex, { items: [item("runtime-refreshed")], nextCursor: null });
  await until(() => evaluate("document.body.innerText.includes('runtime-refreshed')"), "fresh page after Runtime reconnect");
  assert.equal(await evaluate("document.body.innerText.includes('runtime-last-known')"), false, "the successful refresh replaces the prior snapshot");
  assert.equal(await evaluate("document.body.innerText.includes('Showing the last saved open items')"), false, "fresh data clears the stale label");
});
