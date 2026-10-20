import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { after, test } from "node:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { createServer as createHttpServer } from "node:http";
import react from "@vitejs/plugin-react";
import { createServer } from "vite";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const chrome = process.env.LITECOWORK_CHROME ?? "/usr/bin/google-chrome";
const wait = ms => new Promise(resolveWait => setTimeout(resolveWait, ms));
async function until(check, label, timeout = 8000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const value = await check();
    if (value) return value;
    await wait(40);
  }
  throw new Error(`Timed out waiting for ${label}`);
}

const html = `<!doctype html><html><head><meta charset="utf-8"></head><body><div id="root"></div><div id="direct-detail-root"></div>
<script type="module">
window.__calls = []; window.__pending = []; window.__openedTasks = []; window.__keyboardEvents = [];
document.addEventListener('keydown', event => window.__keyboardEvents.push('down:' + event.key));
document.addEventListener('keyup', event => window.__keyboardEvents.push('up:' + event.key));
document.addEventListener('click', event => { if (event.target instanceof HTMLButtonElement) window.__keyboardEvents.push('click:' + event.target.textContent.trim()); }, true);
window.__TAURI_INTERNALS__ = { invoke(command, args) { window.__calls.push({ command, args }); return new Promise((resolve, reject) => window.__pending.push({ resolve, reject })); } };
import React from 'react';
import { createRoot } from 'react-dom/client';
const { TaskAttentionPage } = await import('/src/needs-you/TaskAttentionPage.tsx');
const { isUserRequestDetailFor, UserRequestDetail } = await import('/src/needs-you/UserRequestDetail.tsx');
let workspace = 'workspace-a';
let operatorReady = true;
const root = createRoot(document.getElementById('root'));
const directDetailRoot = createRoot(document.getElementById('direct-detail-root'));
window.setWorkspace = value => { workspace = value; render(); };
window.setOperatorReady = value => { operatorReady = value; render(); };
window.validateRequestDetail = (workspaceId, requestId, value) => isUserRequestDetailFor(workspaceId, requestId, value);
window.setDirectDetailIdentity = (workspaceId, requestId) => directDetailRoot.render(React.createElement(UserRequestDetail, { workspaceId, requestId, operatorReady, onBack: () => {} }));
window.unmountDirectDetail = () => directDetailRoot.unmount();
window.checkChoiceBudget = targetBytes => {
  const choices = Array.from({ length: 32 }, (_, index) => ({ choiceId: 'choice-' + index, label: 'x'.repeat(15000), description: null }));
  let remaining = targetBytes - new TextEncoder().encode(JSON.stringify(choices)).length;
  for (const choice of choices) {
    const addition = Math.min(remaining, 16384 - choice.label.length);
    choice.label += 'x'.repeat(addition);
    remaining -= addition;
  }
  if (remaining !== 0) return { bytes: -1, valid: false };
  const value = { requestId: 'request-a', workspaceId: 'workspace-a', scope: { kind: 'TASK_PLANNING', taskId: 'task-a' }, kind: 'QUESTION', interactionMode: 'FORM', prompt: 'Choose an option', choices, status: 'PENDING', version: 1, createdAt: '2026-10-15T10:00:00Z', expiresAt: null, resolvedAt: null };
  return { bytes: new TextEncoder().encode(JSON.stringify(choices)).length, valid: isUserRequestDetailFor('workspace-a', 'request-a', value) };
};
window.resolveCall = (index, value) => window.__pending[index].resolve(value);
window.rejectCall = (index, message) => window.__pending[index].reject(new Error(message));
function render() { root.render(React.createElement(TaskAttentionPage, { workspaceId: workspace, operatorReady, onOpenTask: id => window.__openedTasks.push(id) })); }
render(); window.__mounted = true;
</script></body></html>`;

let vite, httpServer, chromeProcess, socket, tempProfile, tempViteCache;
const pending = new Map();
const diagnostics = [];
let nextId = 0;
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
  assert.equal(await evaluate(`(() => { const b = [...document.querySelectorAll('button')].find(x => x.textContent.trim() === ${JSON.stringify(label)}); if (!b || b.disabled) return false; b.click(); return true; })()`), true, `enabled button ${label} exists`);
}
async function activateWithEnter(label) {
  assert.equal(await evaluate(`(() => { const b = [...document.querySelectorAll('button')].find(x => x.textContent.trim() === ${JSON.stringify(label)}); if (!b || b.disabled) return false; b.focus(); return document.activeElement === b; })()`), true, `button ${label} can receive keyboard focus`);
  await cdp("Page.bringToFront");
  await cdp("Input.dispatchKeyEvent", { type: "keyDown", key: "Enter", code: "Enter", text: "\r", unmodifiedText: "\r", windowsVirtualKeyCode: 13 });
  await cdp("Input.dispatchKeyEvent", { type: "keyUp", key: "Enter", code: "Enter", windowsVirtualKeyCode: 13 });
  const keys = await evaluate("window.__keyboardEvents.slice(-3)");
  assert.deepEqual(keys, ["down:Enter", `click:${label}`, "up:Enter"], "Enter activates the focused native button");
}
async function callCount() { return evaluate("window.__calls.length"); }
async function settle(index, value, error = false) { await evaluate(`window.${error ? "rejectCall" : "resolveCall"}(${index}, ${JSON.stringify(value)})`); }
function inboxItem(workspaceId = "workspace-a") {
  return { itemId: `sha256:${"a".repeat(64)}`, kind: "USER_REQUEST", status: "OPEN", approvalId: null, userRequestId: "request-a", taskId: null, blockerId: null, title: "Response needed", safeSummary: "A response is waiting.", createdAt: "2026-10-15T10:00:00Z", resolvedAt: null, workspaceId };
}
function detail(status = "DISMISSED", resolvedAt = status === "PENDING" ? null : "2026-10-15T11:00:00Z", requestId = "request-a", interactionMode = "FORM", kindOverride = null) {
  return {
    requestId, workspaceId: "workspace-a",
    scope: { kind: "TASK_PLANNING", taskId: "task-a" },
    kind: kindOverride ?? (interactionMode === "EXTERNAL_URL" ? "EXTERNAL_AUTHORIZATION" : "QUESTION"), interactionMode,
    prompt: requestId === "request-a" ? '<img src=x onerror="window.__xss=1"> Choose a path A' : "Private prompt B",
    choices: interactionMode === "EXTERNAL_URL" ? [] : [{ choiceId: "choice-a", label: '<script>window.__xss=2</script>Option A', description: "Safe text" }],
    status, version: 3, createdAt: "2026-10-15T10:00:00Z", expiresAt: null,
    resolvedAt,
  };
}

after(async () => {
  try { socket?.close(); } catch {}
  if (chromeProcess?.pid) {
    try { process.kill(-chromeProcess.pid, "SIGTERM"); } catch {}
    await new Promise(resolveExit => {
      if (chromeProcess.exitCode !== null || chromeProcess.signalCode !== null) resolveExit();
      else chromeProcess.once("exit", resolveExit);
    });
  }
  if (httpServer?.listening) await new Promise(resolveClose => httpServer.close(resolveClose));
  await vite?.close();
  if (tempProfile) await rm(tempProfile, { recursive: true, force: true });
  if (tempViteCache) await rm(tempViteCache, { recursive: true, force: true });
});

test("Needs You opens the exact request with safe, inert, current detail and explicit responses", async () => {
  assert.ok(chrome, "Chrome is configured");
  tempViteCache = await mkdtemp(join(tmpdir(), "litecowork-request-detail-vite-"));
  vite = await createServer({
    configFile: false, root: packageRoot, cacheDir: tempViteCache,
    server: { middlewareMode: true, hmr: false, ws: false }, appType: "custom",
    optimizeDeps: { noDiscovery: true, include: ["react", "react-dom/client"] },
    plugins: [react(), { name: "request-detail-document", configureServer(server) {
      server.middlewares.use("/__request-detail-test", async (_req, res, next) => {
        try { res.setHeader("content-type", "text/html"); res.end(await server.transformIndexHtml("/__request-detail-test", html)); }
        catch (error) { next(error); }
      });
    } }],
  });
  httpServer = createHttpServer(vite.middlewares);
  await new Promise((resolveListen, rejectListen) => { httpServer.once("error", rejectListen); httpServer.listen(0, "127.0.0.1", resolveListen); });
  const address = httpServer.address();
  assert.ok(address && typeof address === "object");
  tempProfile = await mkdtemp(join(tmpdir(), "litecowork-request-detail-chrome-"));
  chromeProcess = spawn(chrome, ["--headless=new", "--no-sandbox", "--disable-gpu", "--disable-dev-shm-usage", "--remote-debugging-port=0", "--remote-allow-origins=*", `--user-data-dir=${tempProfile}`, "about:blank"], { detached: process.platform !== "win32", stdio: ["ignore", "ignore", "ignore"] });
  const portFile = join(tempProfile, "DevToolsActivePort");
  let target;
  for (let i = 0; i < 300 && !target; i++) {
    try {
      const { readFile } = await import("node:fs/promises");
      const port = Number((await readFile(portFile, "utf8")).split(/\r?\n/, 1)[0]);
      if (port > 0) target = (await (await fetch(`http://127.0.0.1:${port}/json`)).json()).find(entry => entry.type === "page");
    } catch {}
    if (!target) await wait(100);
  }
  assert.ok(target, "headless Chrome exposes a page");
  socket = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolveSocket, reject) => { socket.addEventListener("open", resolveSocket, { once: true }); socket.addEventListener("error", reject, { once: true }); });
  socket.addEventListener("message", event => {
    const message = JSON.parse(event.data);
    if (message.method === "Runtime.exceptionThrown") {
      const error = message.params.exceptionDetails.exception?.description ?? message.params.exceptionDetails.text;
      if (!error.includes("WebSocket closed without opened") || !error.includes("/@vite/client")) diagnostics.push(error);
    }
    if (message.id) pending.get(message.id)?.(message);
  });
  await cdp("Page.enable"); await cdp("Runtime.enable");
  await cdp("Page.navigate", { url: `http://127.0.0.1:${address.port}/__request-detail-test` });
  await until(() => evaluate("window.__mounted === true"), "mounted Needs You view");
  await until(async () => (await callCount()) === 1, "initial Needs You projection");
  await settle(0, { items: [inboxItem()], nextCursor: null });
  await until(() => evaluate("document.body.innerText.includes('Response needed')"), "request row");

  await activateWithEnter("View request details");
  await until(async () => (await callCount()) === 2, "exact UserRequest detail read");
  const detailCall = await evaluate("window.__calls[1]");
  assert.equal(detailCall.command, "get_user_request_detail");
  assert.deepEqual(detailCall.args, { workspaceId: "workspace-a", requestId: "request-a" });
  await until(() => evaluate("document.body.innerText.includes('Loading request details')"), "loading state");
  await settle(1, detail("PENDING"));
  await until(() => evaluate("document.body.innerText.includes('Waiting for your response')"), "current pending status");
  assert.equal(await evaluate('document.activeElement?.id === "user-request-detail-title"'), true, "opening the detail moves keyboard focus to its heading");

  await evaluate("window.setOperatorReady(false)");
  await until(() => evaluate("document.body.innerText.includes('last loaded request status')"), "offline last-loaded status notice");
  await evaluate("window.setOperatorReady(true)");
  await until(async () => (await callCount()) >= 4, "automatic same-request status refresh after reconnect");
  const reconnectCalls = await evaluate("window.__calls.map(call => call.command)");
  assert.deepEqual(reconnectCalls.slice(-2), ["get_user_request_detail", "list_needs_you"]);
  await until(() => evaluate("document.body.innerText.includes('Checking the latest request status')"), "status refresh labels the visible detail as last loaded");
  await settle(2, "Runtime offline", true);
  await settle(3, { items: [inboxItem()], nextCursor: null });
  await until(() => evaluate("document.body.innerText.includes('latest status could not be checked')"), "retryable stale-detail error");
  await click("Retry request details");
  await until(async () => (await callCount()) === 5, "retry of same detail read");
  const retryCall = await evaluate("window.__calls[4]");
  assert.deepEqual(retryCall.args, { workspaceId: "workspace-a", requestId: "request-a" });
  await until(() => evaluate("document.body.innerText.includes('Checking the latest request status')"), "retry keeps the prior status marked as last loaded");
  assert.equal(await evaluate("document.body.innerText.includes('Waiting for your response')"), true, "old status remains visible only with the checking label");
  assert.equal(await evaluate('document.activeElement?.id === "user-request-detail-title"'), true, "same-request refresh leaves keyboard focus where it was");
  await settle(4, detail("DISMISSED", null));
  await until(() => evaluate("document.body.innerText.includes('Request dismissed')"), "current resolved status");

  const state = await evaluate(`(() => ({
    prompt: document.querySelector('.user-request-detail-prompt')?.textContent,
    option: document.querySelector('.user-request-choice-label')?.textContent,
    scope: document.querySelector('.user-request-detail-scope')?.textContent,
    body: document.body.innerText,
    imageCount: document.querySelectorAll('img').length,
    scriptCount: document.querySelectorAll('script:not([type=module])').length,
    links: [...document.querySelectorAll('.user-request-detail a')].map(link => link.href),
    controls: [...document.querySelectorAll('.user-request-detail button')].map(button => button.textContent.trim()),
    xss: window.__xss ?? 0,
  }))()`);
  assert.match(state.prompt, /<img src=x onerror=/, "prompt is displayed as inert text");
  assert.match(state.option, /<script>window\.__xss=2<\/script>/, "choice label is displayed as inert text");
  assert.equal(state.scope, "Task planning", "opaque scope IDs are kept out of user-facing prose");
  assert.equal(state.body.includes("request-a"), false, "opaque request ID stays out of the visible view");
  assert.equal(state.body.includes("task-a"), false, "opaque Task ID stays out of the visible view");
  assert.equal(state.body.includes("Version"), false, "storage version stays out of ordinary user-facing details");
  assert.match(state.body, /Resolved\s+Not recorded/, "terminal status stays visible when the source recorded no resolved timestamp");
  assert.equal(state.imageCount, 0);
  assert.equal(state.scriptCount, 0);
  assert.deepEqual(state.links, [], "choice values never become active links");
  assert.deepEqual(state.controls, ["Back to Needs You"], "terminal detail has no further mutation actions");
  assert.equal(state.xss, 0);
  assert.equal(await evaluate(`window.validateRequestDetail("workspace-a", "request-a", ${JSON.stringify(detail("DISMISSED", null))})`), true);
  const detailWithRawChoiceValue = { ...detail("DISMISSED", null), choices: [{ ...detail("DISMISSED", null).choices[0], value: "secret raw value" }] };
  assert.equal(await evaluate(`window.validateRequestDetail("workspace-a", "request-a", ${JSON.stringify(detailWithRawChoiceValue)})`), false, "the read-only detail decoder rejects choice values entirely");
  assert.equal(await evaluate(`window.validateRequestDetail("workspace-a", "request-a", ${JSON.stringify({ ...detail("DISMISSED", null), providerInputBinding: { secret: "never" } })})`), false, "unknown private provider fields are rejected");
  const promptAtLimit = { ...detail("DISMISSED", null), prompt: "é".repeat((256 * 1024) / 2) };
  assert.equal(await evaluate(`window.validateRequestDetail("workspace-a", "request-a", ${JSON.stringify(promptAtLimit)})`), true, "prompt bound uses UTF-8 bytes");
  const promptOverLimit = { ...promptAtLimit, prompt: `${promptAtLimit.prompt}é` };
  assert.equal(await evaluate(`window.validateRequestDetail("workspace-a", "request-a", ${JSON.stringify(promptOverLimit)})`), false, "prompt over the byte budget is rejected");
  assert.deepEqual(await evaluate("window.checkChoiceBudget(512 * 1024)"), { bytes: 512 * 1024, valid: true }, "choices at the serialized byte limit are accepted");
  assert.deepEqual(await evaluate("window.checkChoiceBudget(512 * 1024 + 1)"), { bytes: 512 * 1024 + 1, valid: false }, "choices over the serialized byte limit are rejected");
  const responseOverLimit = { ...detail("DISMISSED", null), prompt: "\0".repeat(160_000) };
  assert.equal(await evaluate(`window.validateRequestDetail("workspace-a", "request-a", ${JSON.stringify(responseOverLimit)})`), false, "detail over the total response byte limit is rejected");
  const calls = await evaluate("window.__calls.map(call => call.command)");
  assert.deepEqual(calls, ["list_needs_you", "get_user_request_detail", "get_user_request_detail", "list_needs_you", "get_user_request_detail"], "opening, reconnecting, and retrying details issue only reads");
  await evaluate('window.setDirectDetailIdentity("workspace-a", "request-a")');
  await until(async () => (await callCount()) === 6, "isolated rendered detail A read");
  await settle(5, detail("EXPIRED", "2026-10-15T11:00:00Z", "request-a", "EXTERNAL_URL"));
  await until(() => evaluate('document.querySelector("#direct-detail-root .user-request-detail-prompt")?.textContent.includes("path A")'), "detail A rendered");
  const handoffCopy = await evaluate('document.querySelector("#direct-detail-root .user-request-detail")?.innerText ?? ""');
  assert.match(handoffCopy, /configured for an external sign-in handoff/i, "terminal requests describe the handoff as historical");
  assert.doesNotMatch(handoffCopy, /needs an external sign-in handoff/i, "terminal requests do not imply a pending sign-in action");
  await evaluate('window.setDirectDetailIdentity("workspace-a", "request-b")');
  await until(async () => (await callCount()) === 7, "detail B read");
  assert.equal(await evaluate('document.querySelector("#direct-detail-root .user-request-detail-prompt")?.textContent ?? null'), null, "request A prompt is hidden immediately while request B loads");
  await settle(6, detail("PENDING", null, "request-b", "EXTERNAL_URL"));
  await until(() => evaluate('document.querySelector("#direct-detail-root .user-request-detail-prompt")?.textContent === "Private prompt B"'), "detail B rendered");
  const pendingHandoffCopy = await evaluate('document.querySelector("#direct-detail-root .user-request-detail")?.innerText ?? ""');
  assert.match(pendingHandoffCopy, /needs an external sign-in handoff/i, "pending requests explain the handoff requirement");
  assert.equal(await evaluate('document.querySelector("#direct-detail-root .user-request-detail-prompt")?.textContent.includes("path A")'), false, "request A detail never appears under request B");
  await activateWithEnter("Decline sign-in");
  await until(async () => (await callCount()) === 8, "authenticated UserRequest response bridge call");
  const declineCall = await evaluate("window.__calls[7]");
  assert.equal(declineCall.command, "respond_user_request");
  assert.equal(declineCall.args.workspaceId, "workspace-a");
  assert.equal(declineCall.args.requestId, "request-b");
  assert.equal(declineCall.args.expectedVersion, 3);
  assert.deepEqual(declineCall.args.response, { action: "decline" });
  assert.equal(declineCall.args.choiceId, null);
  assert.match(declineCall.args.idempotencyKey, /^[0-9a-f-]{36}$/i);
  await settle(7, { requestId: "request-b", workspaceId: "workspace-a", status: "ANSWERED", version: 4, resolvedAt: "2026-10-15T11:10:00Z" });
  await until(async () => (await callCount()) === 9, "post-response request status refresh");
  await settle(8, detail("ANSWERED", "2026-10-15T11:10:00Z", "request-b", "EXTERNAL_URL"));
  await until(() => evaluate('document.querySelector("#direct-detail-root .user-request-detail")?.innerText.includes("Response saved")'), "saved response status shown without claiming provider continuation");
  await evaluate('window.setDirectDetailIdentity("workspace-a", "request-choice")');
  await until(async () => (await callCount()) === 10, "choice request detail read");
  await settle(9, detail("PENDING", null, "request-choice", "FORM", "DECISION"));
  await until(() => evaluate('document.querySelector("#direct-detail-root input[type=radio][value=choice-a]") !== null'), "safe choice handle rendered as a native radio control");
  await evaluate('document.querySelector("#direct-detail-root input[type=radio][value=choice-a]").click()');
  await click("Save response");
  await until(async () => (await callCount()) === 11, "choice ID sent to the response bridge");
  const choiceCall = await evaluate("window.__calls[10]");
  assert.equal(choiceCall.command, "respond_user_request");
  assert.equal(choiceCall.args.choiceId, "choice-a");
  assert.equal(choiceCall.args.response, null);
  await settle(10, { requestId: "request-choice", workspaceId: "workspace-a", status: "ANSWERED", version: 4, resolvedAt: "2026-10-15T11:20:00Z" });
  await until(async () => (await callCount()) === 12, "choice response status refresh");
  await settle(11, detail("ANSWERED", "2026-10-15T11:20:00Z", "request-choice", "FORM", "DECISION"));
  await until(() => evaluate('document.querySelector("#direct-detail-root .user-request-detail")?.innerText.includes("Response saved")'), "choice response status rendered");
  await evaluate("window.unmountDirectDetail()");
  assert.deepEqual(diagnostics, [], "the browser reports no runtime exceptions");

  await activateWithEnter("Back to Needs You");
  await until(() => evaluate("document.activeElement?.textContent?.trim() === 'View request details'"), "keyboard focus returns to the originating request row");
  const pageCallBase = await callCount();
  await click("View request details");
  await until(async () => (await callCount()) === pageCallBase + 1, "next request detail read");
  await evaluate("window.setWorkspace('workspace-b')");
  await until(async () => (await callCount()) === pageCallBase + 2, "new Workspace inbox read");
  await settle(pageCallBase + 1, { items: [], nextCursor: null });
  await settle(pageCallBase, detail("PENDING"));
  await wait(100);
  assert.equal(await evaluate("document.body.innerText.includes('Choose a path')"), false, "a late prior-Workspace detail cannot populate the new Workspace");
  assert.equal(await evaluate("document.body.innerText.includes('request-a')"), false);
});

test("Back focuses the Needs You heading when a filter removes the invoking row", async () => {
  const inboxIndex = await callCount();
  await evaluate("window.setWorkspace('workspace-a')");
  await until(async () => (await callCount()) === inboxIndex + 1, "return to original Workspace");
  await settle(inboxIndex, { items: [inboxItem()], nextCursor: null });
  await until(() => evaluate("document.body.innerText.includes('Response needed')"), "request row before filter");
  await activateWithEnter("View request details");
  await until(async () => (await callCount()) === inboxIndex + 2, "request before filter");
  await settle(inboxIndex + 1, detail("PENDING"));
  await until(() => evaluate("document.body.innerText.includes('Waiting for your response')"), "loaded request before filter");
  await evaluate("document.querySelector('input[value=RESOLVED]').click()");
  await until(async () => (await callCount()) === inboxIndex + 3, "resolved filter read");
  await settle(inboxIndex + 2, { items: [], nextCursor: null });
  await until(() => evaluate("document.body.innerText.includes('No resolved history')"), "filter removes invoking row");
  await activateWithEnter("Back to Needs You");
  await until(() => evaluate("document.activeElement === document.querySelector('.task-attention-heading h1')"), "Back restores focus to stable Needs You heading");
  assert.equal(await evaluate("document.activeElement.tabIndex"), -1, "fallback heading accepts programmatic keyboard focus");
});

test("page Refresh rechecks the exact open request and labels retained status through failure and retry", async () => {
  const inboxIndex = await callCount();
  await evaluate("document.querySelector('input[value=OPEN]').click()");
  await until(async () => (await callCount()) === inboxIndex + 1, "open filter read");
  await settle(inboxIndex, { items: [inboxItem()], nextCursor: null });
  await until(() => evaluate("document.body.innerText.includes('Response needed')"), "request row before refresh");
  await activateWithEnter("View request details");
  await until(async () => (await callCount()) === inboxIndex + 2, "request before refresh");
  await settle(inboxIndex + 1, detail("PENDING"));
  await until(() => evaluate("document.body.innerText.includes('Waiting for your response')"), "loaded request before refresh");
  const refreshIndex = await callCount();
  await click("Refresh");
  await until(async () => (await callCount()) === refreshIndex + 2, "Refresh reads both inbox and exact request");
  const calls = await evaluate(`window.__calls.slice(${refreshIndex})`);
  assert.deepEqual(calls.map(call => call.command).sort(), ["get_user_request_detail", "list_needs_you"]);
  const detailIndex = refreshIndex + calls.findIndex(call => call.command === "get_user_request_detail");
  const listIndex = refreshIndex + calls.findIndex(call => call.command === "list_needs_you");
  assert.deepEqual(calls.find(call => call.command === "get_user_request_detail").args, { workspaceId: "workspace-a", requestId: "request-a" });
  await until(() => evaluate("document.body.innerText.includes('Checking the latest request status') && document.body.innerText.includes('last loaded details')"), "retained request explicitly marked last loaded");
  await settle(listIndex, { items: [], nextCursor: null });
  await settle(detailIndex, "offline", true);
  await until(() => evaluate("document.body.innerText.includes('latest status could not be checked') && document.body.innerText.includes('last loaded request status')"), "request refresh failure stays visible independently of inbox success");
  assert.equal(await evaluate("document.body.innerText.includes('Waiting for your response')"), true);
  const retryIndex = await callCount();
  await click("Retry request details");
  await until(async () => (await callCount()) === retryIndex + 1, "exact request retry");
  await settle(retryIndex, detail("ANSWERED"));
  await until(() => evaluate("document.body.innerText.includes('Response received')"), "refreshed request status");
  await activateWithEnter("Back to Needs You");
  await until(() => evaluate("document.activeElement === document.querySelector('.task-attention-heading h1')"), "Back falls back after refresh removes invoking row");
  assert.equal(await evaluate(`window.__calls.slice(${refreshIndex}).every(call => ['list_needs_you', 'get_user_request_detail'].includes(call.command))`), true, "refresh and retry remain read-only");
  assert.deepEqual(diagnostics, [], "the browser reports no runtime exceptions");
});
