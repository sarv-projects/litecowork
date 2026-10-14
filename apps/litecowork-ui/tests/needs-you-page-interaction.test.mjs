import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { after, test } from "node:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import net from "node:net";
import react from "@vitejs/plugin-react";
import { createServer } from "vite";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const chrome = process.env.LITECOWORK_CHROME ?? "/usr/bin/google-chrome";
const wait = ms => new Promise(resolveWait => setTimeout(resolveWait, ms));
async function freePort() {
  const server = net.createServer();
  await new Promise((resolveListen, reject) => { server.once("error", reject); server.listen(0, "127.0.0.1", resolveListen); });
  const { port } = server.address();
  await new Promise(resolveClose => server.close(resolveClose));
  return port;
}
async function until(check, label, timeout = 8000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const value = await check();
    if (value) return value;
    await wait(50);
  }
  throw new Error(`Timed out waiting for ${label}`);
}

const html = `<!doctype html><html><head><meta charset="utf-8"></head><body><div id="root"></div>
<script type="module">
window.__pending = []; window.__calls = []; window.__openedTasks = [];
window.__TAURI_INTERNALS__ = { invoke(command, args) { window.__calls.push({command,args}); return new Promise((resolve,reject) => window.__pending.push({resolve,reject})); } };
import React from 'react';
import { createRoot } from 'react-dom/client';
const { TaskAttentionPage } = await import('/src/needs-you/TaskAttentionPage.tsx');
let workspace = 'workspace-a';
const root = createRoot(document.getElementById('root'));
window.setWorkspace = value => { workspace = value; render(); };
window.resolveCall = (index, value) => window.__pending[index].resolve(value);
window.rejectCall = (index, message) => window.__pending[index].reject(new Error(message));
function render() { root.render(React.createElement(TaskAttentionPage, {workspaceId: workspace, operatorReady: true, onOpenTask: id => window.__openedTasks.push(id)})); }
render(); window.__mounted = true;
</script></body></html>`;

let vite, chromeProcess, socket, tempProfile, tempViteCache;
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
async function callCount() { return evaluate("window.__calls.length"); }
async function settle(index, result, error = false) { await evaluate(`window.${error ? "rejectCall" : "resolveCall"}(${index}, ${JSON.stringify(result)})`); }

after(async () => {
  try { socket?.close(); } catch {}
  if (chromeProcess?.pid) {
    try { process.kill(-chromeProcess.pid, "SIGTERM"); } catch {}
    await new Promise(resolveExit => { if (chromeProcess.exitCode !== null || chromeProcess.signalCode !== null) resolveExit(); else chromeProcess.once("exit", resolveExit); });
  }
  await vite?.close();
  if (tempProfile) await rm(tempProfile, { recursive: true, force: true });
  if (tempViteCache) await rm(tempViteCache, { recursive: true, force: true });
});

test("mounted Needs You page fences Workspace switches and renders selected status", async () => {
  if (!chrome) throw new Error("Set LITECOWORK_CHROME to a local Chrome/Chromium executable.");
  const port = await freePort();
  const cdpPort = await freePort();
  tempViteCache = await mkdtemp(join(tmpdir(), "litecowork-needs-you-vite-"));
  vite = await createServer({
    configFile: false, root: packageRoot,
    cacheDir: tempViteCache,
    server: { host: "127.0.0.1", port, strictPort: true },
    appType: "custom",
    plugins: [react(), { name: "needs-you-test-document", configureServer(server) { server.middlewares.use("/__needs-you-test", async (req, res, next) => { try { res.setHeader("content-type", "text/html"); res.end(await server.transformIndexHtml(req.url ?? "/__needs-you-test", html)); } catch (error) { next(error); } }); } }],
  });
  await vite.listen();
  tempProfile = await mkdtemp(join(tmpdir(), "litecowork-needs-you-ui-"));
  chromeProcess = spawn(chrome, ["--headless=new", "--no-sandbox", "--disable-gpu", "--disable-dev-shm-usage", "--remote-debugging-port=" + cdpPort, "--remote-allow-origins=*", `--user-data-dir=${tempProfile}`, "about:blank"], { detached: process.platform !== "win32", stdio: "ignore" });
  const target = await until(async () => {
    try { const response = await fetch(`http://127.0.0.1:${cdpPort}/json`); return (await response.json()).find(entry => entry.type === "page"); } catch { return null; }
  }, "Chrome DevTools page");
  socket = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolveSocket, reject) => { socket.addEventListener("open", resolveSocket, { once: true }); socket.addEventListener("error", reject, { once: true }); });
  socket.addEventListener("message", event => { const message = JSON.parse(event.data); if (message.method === "Runtime.exceptionThrown") diagnostics.push(message.params.exceptionDetails.exception?.description ?? message.params.exceptionDetails.text); if (message.method === "Log.entryAdded") diagnostics.push(message.params.entry.text); if (message.id) pending.get(message.id)?.(message); });
  await cdp("Page.enable"); await cdp("Runtime.enable"); await cdp("Log.enable");
  await cdp("Page.navigate", { url: `http://127.0.0.1:${port}/__needs-you-test` });
  try {
    await until(async () => { try { return await evaluate("window.__mounted === true"); } catch { return false; } }, "mounted React page", 15000);
  } catch (error) {
    const pageState = await evaluate("document.readyState + ': ' + document.body.innerText + ': ' + document.body.innerHTML.slice(0, 400)").catch(failure => failure.message);
    throw new Error(`${error.message}; diagnostics=${diagnostics.join(" | ")}; page=${pageState}`);
  }
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
});

test("mounted page reports refresh and pagination failures truthfully, releases busy state, and only opens Task blockers", async () => {
  // Reuse the browser/session initialized by the preceding test.
  assert.ok(socket, "browser setup completed");
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

  assert.equal(await evaluate("[...document.querySelectorAll('article')].find(x=>x.innerText.includes('approval-row')).querySelector('button') === null"), true, "Approval is display-only");
  await evaluate("(() => { const article=[...document.querySelectorAll('article')].find(x=>x.innerText.includes('blocker-row')); article.querySelector('button').click(); })()");
  assert.deepEqual(await evaluate("window.__openedTasks"), ["task-a"], "only blocker navigation opens the Task");
});
