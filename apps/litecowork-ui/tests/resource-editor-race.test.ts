import assert from "node:assert/strict";
import { spawn, type ChildProcess } from "node:child_process";
import { createServer as createHttpServer, type Server } from "node:http";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { after, test } from "node:test";
import react from "@vitejs/plugin-react";
import { createServer as createViteServer, type ViteDevServer } from "vite";

const editorSource = await readFile(new URL("../src/resources/ResourceRevisionEditor.tsx", import.meta.url), "utf8");

function productionFunction(name: string) {
  const declaration = editorSource.match(new RegExp(`^export function ${name}\\(([\\s\\S]*?)\\)(?:\\s*:[^\\{\\n]+)?\\s*\\{([\\s\\S]*?)^\\}`, "m"));
  assert.ok(declaration, `ResourceRevisionEditor must export ${name} as a directly testable navigation guard.`);
  const parameters = declaration[1].split(",").map((parameter) => parameter.trim().split(":", 1)[0].trim()).join(",");
  return new Function(`return function ${name}(${parameters}) {${declaration[2]}}`)() as (...args: unknown[]) => unknown;
}

test("Resource editor requires a deliberate choice before switching away from a dirty Resource", () => {
  const decide = productionFunction("resourceNavigationDecision");
  const current = { workspaceId: "workspace-a", resourceId: "resource-a" };
  const sameResource = { workspaceId: "workspace-a", resourceId: "resource-a" };
  const otherResource = { workspaceId: "workspace-a", resourceId: "resource-b" };

  assert.equal(decide(current, sameResource, "workspace-a", true, false), "SAME");
  assert.equal(decide(current, otherResource, "workspace-a", true, false), "CONFIRM");
  assert.equal(decide(current, otherResource, "workspace-a", false, false), "SWITCH_RESOURCE");
});

test("Resource editor waits for ambiguous status and upload requests before changing targets", () => {
  const decide = productionFunction("resourceNavigationDecision");
  const current = { workspaceId: "workspace-a", resourceId: "resource-a" };
  const otherResource = { workspaceId: "workspace-a", resourceId: "resource-b" };

  assert.equal(decide(current, otherResource, "workspace-a", true, true), "WAIT");
  assert.equal(decide(current, otherResource, "workspace-a", false, true), "WAIT");
});

test("Workspace changes preserve dirty Resource drafts until Save, Discard, or Stay is chosen", () => {
  const decide = productionFunction("resourceNavigationDecision");
  const current = { workspaceId: "workspace-a", resourceId: "resource-a" };

  assert.equal(decide(current, current, "workspace-b", true, false), "CONFIRM");
  assert.equal(decide(current, current, "workspace-b", false, false), "CLOSE_WORKSPACE");
  assert.equal(decide(current, current, "workspace-b", true, true), "WAIT");
});

test("late Resource reads are accepted only for the current Workspace, Resource, and request generation", () => {
  const accepts = productionFunction("isCurrentResourceSelection");
  const selected = { workspaceId: "workspace-b", resourceId: "resource-b" };
  assert.equal(accepts({ workspaceId: "workspace-a", resourceId: "resource-a" }, selected, 3, 4), false);
  assert.equal(accepts(selected, selected, 3, 4), false);
  assert.equal(accepts(selected, selected, 4, 4), true);
});

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const chrome = process.env.LITECOWORK_CHROME ?? "/usr/bin/google-chrome";
const wait = (milliseconds: number) => new Promise((resolveWait) => setTimeout(resolveWait, milliseconds));
async function until(check: () => Promise<unknown>, label: string, timeout = 10_000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const result = await check();
    if (result) return result;
    await wait(50);
  }
  throw new Error(`Timed out waiting for ${label}`);
}

const html = `<!doctype html><html><head><meta charset="utf-8"></head><body><div id="root"></div>
<script type="module">
window.__calls = []; window.__deferredKeys = new Set(); window.__pendingCalls = new Map();
const resources = {
  "resource-a": { resourceId: "resource-a", workspaceId: "workspace-a", resourceRevisionId: "revision-a1", displayName: "A.txt", mediaType: "text/plain", contentDigest: "sha256:${"1".repeat(64)}", sizeBytes: 19 },
  "resource-b": { resourceId: "resource-b", workspaceId: "workspace-a", resourceRevisionId: "revision-b1", displayName: "B.txt", mediaType: "text/plain", contentDigest: "sha256:${"2".repeat(64)}", sizeBytes: 19 }
};
function key(command, resourceId) { return JSON.stringify([command, resourceId]); }
function detail(resourceId) { return { resourceId, workspaceId: "workspace-a", kind: "TEXT", displayName: resourceId === "resource-a" ? "A.txt" : "B.txt", currentRevisionId: resourceId === "resource-a" ? "revision-a1" : "revision-b1", version: 1, contextDocument: { kind: "WORKSPACE_RESOURCE", status: "ACTIVE", owner_ref: { kind: "WORKSPACE", workspace_id: "workspace-a" } } }; }
function response(command, resourceId) {
  if (command === "get_resource_detail") return detail(resourceId);
  if (command === "list_resource_revisions") return { items: [{ resourceRevisionId: resourceId === "resource-a" ? "revision-a1" : "revision-b1", resourceId, parentRevisionIds: [], contentDigest: "sha256:${"1".repeat(64)}", sizeBytes: 19, mediaType: "text/plain", observedAt: "2026-10-16T00:00:00Z", isHead: true }], nextCursor: null };
  if (command === "preview_resource_text") return resourceId === "resource-a" ? "Saved A source text" : "Saved B source text";
  return null;
}
window.deferResourceCall = (command, resourceId) => { window.__deferredKeys.add(key(command, resourceId)); };
window.resourcePendingCount = (command, resourceId) => window.__pendingCalls.get(key(command, resourceId))?.length ?? 0;
window.resolveResourceCall = (command, resourceId, reject = false) => {
  const pending = window.__pendingCalls.get(key(command, resourceId))?.shift();
  if (!pending) throw new Error("No pending " + command + " response for " + resourceId);
  if (reject) pending.reject(new Error("Deferred " + command + " failed for " + resourceId));
  else pending.resolve(response(command, resourceId));
};
window.__TAURI_INTERNALS__ = { invoke(command, args = {}) {
  window.__calls.push({ command, args });
  const resourceId = String(args.resourceId ?? "resource-a");
  if (["get_resource_detail", "list_resource_revisions", "preview_resource_text"].includes(command)) {
    const deferredKey = key(command, resourceId);
    if (window.__deferredKeys.delete(deferredKey)) return new Promise((resolve, reject) => {
      const pendingKey = key(command, resourceId);
      window.__pendingCalls.set(pendingKey, [...(window.__pendingCalls.get(pendingKey) ?? []), { resolve, reject }]);
    });
    return Promise.resolve(response(command, resourceId));
  }
  if (command === "list_resource_locations") return Promise.resolve([]);
  if (command === "create_resource_revision_upload") {
    if (window.__conflictNextSave) { window.__conflictNextSave = false; return Promise.reject(new Error("RESOURCE_CONFLICT: controlled stale Resource head")); }
    return Promise.reject(new Error("controlled upload failure after recording target"));
  }
  if (command === "get_context_document_deletion_status") return Promise.resolve({ resourceId, status: "ACTIVE", manifestDigest: null, requiredReplicaCount: 0, acknowledgedReplicaCount: 0, updatedAt: "2026-10-16T00:00:00Z" });
  return Promise.resolve(null);
} };
import React from 'react';
import { createRoot } from 'react-dom/client';
const { ResourceRevisionEditor } = await import('/src/resources/ResourceRevisionEditor.tsx');
const root = createRoot(document.getElementById('root'));
let selected = resources['resource-a'];
let mount = 0;
function render() { root.render(React.createElement(ResourceRevisionEditor, { key: mount, resource: selected, activeWorkspaceId: 'workspace-a', onClose() {}, onCommitted() {}, onSelectResource() {}, onSelectWorkspace() {} })); }
window.selectResourceB = () => { selected = resources['resource-b']; render(); };
window.resetResource = resourceId => { selected = resources[resourceId]; mount += 1; render(); };
window.conflictNextSave = () => { window.__conflictNextSave = true; };
render(); window.__mounted = true;
</script></body></html>`;

const searchHtml = `<!doctype html><html><head><meta charset="utf-8"></head><body><div id="root"></div>
<script type="module">
await import('/tests/browser/harness.tsx');
await new Promise(resolve => setTimeout(resolve, 0));
window.__searchPending = new Map(); window.__searchCalls = [];
const internal = window.__TAURI_INTERNALS__;
const originalInvoke = internal.invoke;
function searchResult(workspaceId) {
  return { resourceId: workspaceId === "workspace-a" ? "resource-search-a" : "resource-search-b", resourceRevisionId: workspaceId === "workspace-a" ? "revision-search-a" : "revision-search-b", sourceContentDigest: "sha256:${"a".repeat(64)}", sourceMatches: [], displayName: workspaceId === "workspace-a" ? "A search result.txt" : "B search result.txt", freshness: "CURRENT", matchReasons: ["NAME"], snippet: null };
}
internal.invoke = (command, args = {}) => {
  if (command === "list_resources") {
    const workspaceId = String(args.workspaceId);
    return Promise.resolve({ items: [{ resourceId: workspaceId === "workspace-a" ? "resource-search-a" : "resource-search-b", workspaceId, resourceRevisionId: workspaceId === "workspace-a" ? "revision-search-a" : "revision-search-b", displayName: workspaceId === "workspace-a" ? "A source.txt" : "B source.txt", mediaType: "text/plain", contentDigest: "sha256:${"b".repeat(64)}", sizeBytes: 12 }], nextCursor: null });
  }
  if (command === "search_resources") {
    const workspaceId = String(args.workspaceId);
    window.__searchCalls.push({ workspaceId, query: args.query });
    return new Promise((resolve, reject) => {
      window.__searchPending.set(workspaceId, { resolve: () => resolve({ items: [searchResult(workspaceId)], nextCursor: null, mode: "METADATA", contentScan: null }), reject });
    });
  }
  return originalInvoke.call(internal, command, args);
};
window.resolveSearch = workspaceId => { const pending=window.__searchPending.get(workspaceId); if(!pending) throw new Error("No pending search for " + workspaceId); window.__searchPending.delete(workspaceId); pending.resolve(); };
window.searchPendingCount = workspaceId => window.__searchPending.has(workspaceId) ? 1 : 0;
window.mountSearchApp = () => window.litecoworkHarness.mountProductionApp("resource-upload-progress-read-deferred");
window.__searchHarnessReady = true;
</script></body></html>`;

let vite: ViteDevServer | undefined;
let httpServer: Server | undefined;
let chromeProcess: ChildProcess | undefined;
let socket: WebSocket | undefined;
let tempProfile: string | undefined;
let tempViteCache: string | undefined;
let chromeStartupError: Error | null = null;
let chromeExit: { code: number | null; signal: NodeJS.Signals | null } | null = null;
let browserSetupFailure: string | null = null;
let nextCdpId = 0;
const cdpPending = new Map<number, (message: any) => void>();

function cdp(method: string, params: Record<string, unknown> = {}): Promise<any> {
  assert.ok(socket, "Chrome debugging socket is open");
  const id = ++nextCdpId;
  const result = new Promise((resolveResult, reject) => {
    const timer = setTimeout(() => { cdpPending.delete(id); reject(new Error(`Chrome DevTools timed out: ${method}`)); }, 7_000);
    cdpPending.set(id, (message) => {
      clearTimeout(timer);
      if (message.error) reject(new Error(message.error.message));
      else resolveResult(message.result);
    });
  });
  socket.send(JSON.stringify({ id, method, params }));
  return result;
}

async function evaluate(expression: string): Promise<any> {
  const result = await cdp("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true, userGesture: true });
  if (result.exceptionDetails) throw new Error(result.exceptionDetails.exception?.description ?? result.exceptionDetails.text);
  return result.result.value;
}

async function waitForChromePage(profile: string): Promise<any> {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    if (chromeStartupError) throw chromeStartupError;
    if (chromeExit) throw new Error(`Chrome exited before DevTools became ready (${chromeExit.signal ?? chromeExit.code})`);
    const activePort = Number((await readFile(join(profile, "DevToolsActivePort"), "utf8").catch(() => "")).split(/\r?\n/, 1)[0]);
    if (Number.isInteger(activePort) && activePort > 0 && activePort <= 65_535) {
      const response = await fetch(`http://127.0.0.1:${activePort}/json`).catch(() => null);
      if (response?.ok) {
        const pages = await response.json();
        const page = pages.find((item: { type?: string }) => item.type === "page");
        if (page) return page;
      }
    }
    await wait(100);
  }
  throw new Error("Chrome did not expose a DevTools page within 30 seconds");
}

after(async () => {
  try { socket?.close(); } catch {}
  if (chromeProcess?.pid) {
    try { process.kill(-chromeProcess.pid, "SIGTERM"); } catch {}
    await new Promise<void>((resolveExit) => {
      if (chromeProcess?.exitCode !== null || chromeProcess?.signalCode !== null) resolveExit();
      else chromeProcess?.once("exit", () => resolveExit());
    });
  }
  if (httpServer?.listening) await new Promise<void>((resolveClose) => httpServer?.close(() => resolveClose()));
  await vite?.close();
  if (tempProfile) await rm(tempProfile, { recursive: true, force: true });
  if (tempViteCache) await rm(tempViteCache, { recursive: true, force: true });
});

test("a retained Resource A draft is saved only to A before the editor switches to B", async (context) => {
  try {
    tempViteCache = await mkdtemp(join(tmpdir(), "litecowork-resource-race-vite-"));
    vite = await createViteServer({
      configFile: false,
      root: packageRoot,
      cacheDir: tempViteCache,
      server: { middlewareMode: true, hmr: false, ws: false },
      appType: "custom",
      optimizeDeps: { noDiscovery: true, include: ["react", "react-dom/client"] },
      plugins: [react(), { name: "resource-race-test-document", configureServer(server) {
        server.middlewares.use("/__resource-race-test", async (_request, response, next) => {
          try {
            response.setHeader("content-type", "text/html");
            response.end(await server.transformIndexHtml("/__resource-race-test", html));
          } catch (error) { next(error); }
        });
        server.middlewares.use("/__resource-search-test", async (_request, response, next) => {
          try {
            response.setHeader("content-type", "text/html");
            response.end(await server.transformIndexHtml("/__resource-search-test", searchHtml));
          } catch (error) { next(error); }
        });
      } }],
    });
    httpServer = createHttpServer(vite.middlewares);
    await new Promise<void>((resolveListen, rejectListen) => {
      httpServer?.once("error", rejectListen);
      httpServer?.listen(0, "127.0.0.1", resolveListen);
    });
    const address = httpServer.address();
    assert.ok(address && typeof address === "object", "the browser harness listens on an OS-assigned port");
    tempProfile = await mkdtemp(join(tmpdir(), "litecowork-resource-race-chrome-"));
    chromeProcess = spawn(chrome, ["--headless=new", "--no-sandbox", "--disable-gpu", "--disable-dev-shm-usage", "--remote-debugging-port=0", "--remote-allow-origins=*", `--user-data-dir=${tempProfile}`, "about:blank"], { detached: process.platform !== "win32", stdio: "ignore" });
    chromeProcess.once("error", (error) => { chromeStartupError = error; });
    chromeProcess.once("exit", (code, signal) => { chromeExit = { code, signal }; });
    const target = await waitForChromePage(tempProfile);
    socket = new WebSocket(target.webSocketDebuggerUrl);
    await new Promise<void>((resolveSocket, rejectSocket) => {
      socket?.addEventListener("open", () => resolveSocket(), { once: true });
      socket?.addEventListener("error", rejectSocket, { once: true });
    });
    socket.addEventListener("message", (event) => {
      const message = JSON.parse(String(event.data));
      if (message.id) cdpPending.get(message.id)?.(message);
    });
    await cdp("Page.enable");
    await cdp("Runtime.enable");
    await cdp("Page.navigate", { url: `http://127.0.0.1:${address.port}/__resource-race-test` });
    await until(() => evaluate("window.__mounted === true"), "mounted Resource editor");
    await until(() => evaluate("document.querySelector('#resource-revision-title')?.textContent === 'A.txt'"), "Resource A details");
    await until(() => evaluate("[...document.querySelectorAll('button')].some(button => button.textContent.trim() === 'Edit small text file')"), "Resource A edit action");
    await evaluate("[...document.querySelectorAll('button')].find(button => button.textContent.trim() === 'Edit small text file').click()");
    await until(() => evaluate("document.querySelector('#resource-revision-text')?.value === 'Saved A source text'"), "Resource A text preview");
    await evaluate("(() => { const editor=document.querySelector('#resource-revision-text'); editor.focus(); editor.setSelectionRange(0,editor.value.length); return true; })()");
    await cdp("Input.insertText", { text: "Draft owned by Resource A" });
    await until(() => evaluate("document.querySelector('#resource-revision-text')?.value === 'Draft owned by Resource A'"), "dirty Resource A draft");

    await evaluate("window.selectResourceB()");
    await until(() => evaluate("Boolean(document.querySelector('[role=alertdialog]'))"), "Resource navigation confirmation");
    await evaluate("[...document.querySelectorAll('button')].find(button => button.textContent.trim() === 'Save draft and switch').click()");
    await until(() => evaluate("window.__calls.some(call => call.command === 'create_resource_revision_upload')"), "draft upload target");
    const targets = await evaluate("window.__calls.filter(call => call.command === 'create_resource_revision_upload').map(call => [call.args.workspaceId, call.args.resourceId])");
    assert.deepEqual(targets, [["workspace-a", "resource-a"]], "the retained A draft remains pinned to A while B is the pending destination");

    await until(() => evaluate("[...document.querySelectorAll('button')].find(button => button.textContent.trim() === 'Discard draft and switch')?.disabled === false"), "available discard action after failed save");
    await evaluate("[...document.querySelectorAll('button')].find(button => button.textContent.trim() === 'Discard draft and switch').click()");
    await until(() => evaluate("document.querySelector('#resource-revision-title')?.textContent === 'B.txt'"), "Resource B after explicit discard");
    assert.equal(await evaluate("document.querySelector('#resource-revision-text')"), null, "A's retained draft is cleared when the editor actually switches to B");
    assert.deepEqual(await evaluate("window.__calls.filter(call => call.command === 'create_resource_revision_upload').map(call => [call.args.workspaceId, call.args.resourceId])"), [["workspace-a", "resource-a"]]);
  } catch (error) {
    browserSetupFailure = error instanceof Error ? error.message : String(error);
    if (!process.env.LITECOWORK_CHROME && (error as Error).message.includes("ENOENT")) {
      context.skip("Chrome/Chromium is unavailable; set LITECOWORK_CHROME to its executable path.");
      return;
    }
    throw error;
  }
});

test("mounted Resource editor ignores deferred A detail, history, and head-preview responses after switching to B", async (context) => {
  if (browserSetupFailure || !socket) {
    context.skip(`The mounted Resource browser session is unavailable${browserSetupFailure ? `: ${browserSetupFailure}` : "."}`);
    return;
  }

  const clickButton = async (label: string) => {
    const clicked = await evaluate(`(() => { const button=[...document.querySelectorAll('button')].find(candidate=>candidate.textContent.trim()===${JSON.stringify(label)}); if(!button)return false; button.click(); return true; })()`);
    assert.equal(clicked, true, `button ${label} exists`);
  };
  const assertResourceBVisible = async (label: string) => {
    await until(() => evaluate("document.querySelector('#resource-revision-title')?.textContent === 'B.txt' && document.querySelector('.resource-revision-meta')?.textContent.includes('revision-b1') && document.querySelector('.resource-revision-history li strong')?.textContent.includes('revision-b1')"), label);
  };

  await evaluate("window.resetResource('resource-a')");
  await until(() => evaluate("document.querySelector('#resource-revision-title')?.textContent === 'A.txt'"), "Resource A before deferred detail race");
  await evaluate("window.deferResourceCall('get_resource_detail','resource-a'); window.resetResource('resource-a')");
  await until(() => evaluate("window.resourcePendingCount('get_resource_detail','resource-a') === 1"), "deferred Resource A detail");
  await evaluate("window.selectResourceB()");
  await assertResourceBVisible("Resource B detail and history after selection");
  await evaluate("window.resolveResourceCall('get_resource_detail','resource-a')");
  await wait(120);
  assert.equal(await evaluate("document.querySelector('#resource-revision-title')?.textContent"), "B.txt");
  assert.equal(await evaluate("document.querySelector('.resource-revision-meta')?.textContent.includes('revision-b1')"), true, "late A detail cannot replace B metadata");
  assert.equal(await evaluate("document.querySelector('.resource-revision-history li strong')?.textContent.includes('revision-b1')"), true, "late A detail cannot detach B's history from its metadata");
  assert.equal(await evaluate("Boolean(document.querySelector('.resource-revision-panel [role=alert]'))"), false, "late A detail does not report an error in B");

  await evaluate("window.deferResourceCall('get_resource_detail','resource-a'); window.resetResource('resource-a')");
  await until(() => evaluate("window.resourcePendingCount('get_resource_detail','resource-a') === 1"), "deferred failing Resource A detail");
  await evaluate("window.selectResourceB()");
  await assertResourceBVisible("Resource B detail before late Resource A detail failure");
  await evaluate("window.resolveResourceCall('get_resource_detail','resource-a', true)");
  await wait(120);
  assert.equal(await evaluate("document.querySelector('#resource-revision-title')?.textContent"), "B.txt");
  assert.equal(await evaluate("document.querySelector('.resource-revision-meta')?.textContent.includes('revision-b1')"), true, "late A detail failure cannot replace B metadata");
  assert.equal(await evaluate("Boolean(document.querySelector('.resource-revision-panel [role=alert]'))"), false, "late A detail failure cannot surface in B");

  await evaluate("window.deferResourceCall('list_resource_revisions','resource-a'); window.resetResource('resource-a')");
  await until(() => evaluate("window.resourcePendingCount('list_resource_revisions','resource-a') === 1"), "deferred Resource A history");
  await evaluate("window.selectResourceB()");
  await assertResourceBVisible("Resource B detail and history before late A history");
  await evaluate("window.resolveResourceCall('list_resource_revisions','resource-a')");
  await wait(120);
  assert.equal(await evaluate("document.querySelector('#resource-revision-title')?.textContent"), "B.txt");
  assert.equal(await evaluate("document.querySelector('.resource-revision-history li strong')?.textContent.includes('revision-b1')"), true, "late A history cannot replace B history");
  assert.equal(await evaluate("document.querySelector('.resource-revision-history .section-heading button')?.disabled"), false, "late A history cannot change B's loading state");
  assert.equal(await evaluate("Boolean(document.querySelector('.resource-revision-panel [role=alert]'))"), false, "late A history does not report an error in B");

  await evaluate("window.deferResourceCall('list_resource_revisions','resource-a'); window.resetResource('resource-a')");
  await until(() => evaluate("window.resourcePendingCount('list_resource_revisions','resource-a') === 1"), "deferred failing Resource A history");
  await evaluate("window.selectResourceB()");
  await assertResourceBVisible("Resource B detail and history before late Resource A history failure");
  await evaluate("window.resolveResourceCall('list_resource_revisions','resource-a', true)");
  await wait(120);
  assert.equal(await evaluate("document.querySelector('#resource-revision-title')?.textContent"), "B.txt");
  assert.equal(await evaluate("document.querySelector('.resource-revision-history li strong')?.textContent.includes('revision-b1')"), true, "late A history failure cannot replace B history");
  assert.equal(await evaluate("document.querySelector('.resource-revision-history .section-heading button')?.disabled"), false, "late A history failure cannot change B's loading state");
  assert.equal(await evaluate("Boolean(document.querySelector('.resource-revision-panel [role=alert]'))"), false, "late A history failure cannot surface in B");

  await evaluate("window.resetResource('resource-a')");
  await until(() => evaluate("document.querySelector('#resource-revision-title')?.textContent === 'A.txt' && document.querySelector('.resource-revision-meta')?.textContent.includes('revision-a1')"), "Resource A before deferred head preview");
  await evaluate("[...document.querySelectorAll('.resource-upload-recovery button')].find(button => button.textContent.trim() === 'Forget resume details')?.click()");
  await until(() => evaluate("!document.querySelector('.resource-upload-recovery')"), "stale A upload recovery descriptor cleared before preview race");
  await clickButton("Edit small text file");
  await until(() => evaluate("document.querySelector('#resource-revision-text')?.value === 'Saved A source text'"), "Resource A preview for a dirty draft");
  await evaluate("(() => { const editor=document.querySelector('#resource-revision-text'); editor.focus(); editor.setSelectionRange(0,editor.value.length); })()");
  await cdp("Input.insertText", { text: "A draft before a late head preview" });
  await until(() => evaluate("document.querySelector('#resource-revision-text')?.value === 'A draft before a late head preview'"), "Resource A dirty draft before conflict");
  await evaluate("window.conflictNextSave()");
  await clickButton("Save as new revision");
  await until(() => evaluate("Boolean(document.querySelector('.resource-rebase-review'))"), "Resource A conflict review");
  await evaluate("window.deferResourceCall('preview_resource_text','resource-a')");
  await clickButton("Preview current saved text before rebasing");
  await until(() => evaluate("window.resourcePendingCount('preview_resource_text','resource-a') === 1"), "deferred Resource A current-head preview");
  await evaluate("window.selectResourceB()");
  await until(() => evaluate("Boolean(document.querySelector('[role=alertdialog]'))"), "discard confirmation before opening B");
  await clickButton("Discard draft and switch");
  await assertResourceBVisible("Resource B after discarding A's draft");

  await clickButton("Edit small text file");
  await until(() => evaluate("document.querySelector('#resource-revision-text')?.value === 'Saved B source text'"), "Resource B preview for its draft");
  await evaluate("(() => { const editor=document.querySelector('#resource-revision-text'); editor.focus(); editor.setSelectionRange(0,editor.value.length); })()");
  await cdp("Input.insertText", { text: "B draft before head preview" });
  await until(() => evaluate("document.querySelector('#resource-revision-text')?.value === 'B draft before head preview'"), "Resource B dirty draft");
  await evaluate("window.conflictNextSave()");
  await clickButton("Save as new revision");
  await until(() => evaluate("Boolean(document.querySelector('.resource-rebase-review'))"), "Resource B conflict review");
  await evaluate("window.deferResourceCall('preview_resource_text','resource-b')");
  await clickButton("Preview current saved text before rebasing");
  await until(() => evaluate("window.resourcePendingCount('preview_resource_text','resource-b') === 1"), "deferred Resource B current-head preview");
  await evaluate("window.resolveResourceCall('preview_resource_text','resource-a')");
  await wait(120);
  assert.equal(await evaluate("document.querySelector('#resource-revision-title')?.textContent"), "B.txt", "late A preview success keeps B selected");
  assert.equal(await evaluate("document.querySelector('.resource-revision-meta')?.textContent.includes('revision-b1')"), true, "late A preview cannot change B metadata");
  assert.equal(await evaluate("document.querySelector('.resource-rebase-current-text')"), null, "late A preview text is not displayed in B");
  assert.equal(await evaluate("[...document.querySelectorAll('.resource-rebase-review button')].find(button => button.textContent.includes('Loading current head'))?.disabled"), true, "late A cleanup cannot clear B's in-flight preview indicator");
  assert.equal(await evaluate("Boolean(document.querySelector('.resource-rebase-review [role=alert]'))"), false, "late A preview does not report an error in B");
  await evaluate("window.resolveResourceCall('preview_resource_text','resource-b')");
  await until(() => evaluate("document.querySelector('.resource-rebase-current-text')?.textContent === 'Saved B source text'"), "Resource B current-head preview");
  assert.equal(await evaluate("document.querySelector('.resource-revision-meta')?.textContent.includes('revision-b1')"), true, "B preview remains pinned to B's current revision");
});

test("mounted Library ignores a deferred Workspace A search response after switching to B", async (context) => {
  if (browserSetupFailure || !socket) {
    context.skip(`The mounted Resource browser session is unavailable${browserSetupFailure ? `: ${browserSetupFailure}` : "."}`);
    return;
  }
  const address = httpServer?.address();
  assert.ok(address && typeof address === "object", "the browser harness HTTP server remains available");
  await cdp("Page.navigate", { url: `http://127.0.0.1:${address.port}/__resource-search-test` });
  await until(() => evaluate("window.__searchHarnessReady === true"), "mounted search harness entry");
  await evaluate("window.mountSearchApp()");
  await until(() => evaluate("document.querySelector('select[aria-label=\"Selected Workspace\"]')?.value === 'workspace-a'"), "production App with Workspace A selected");
  await clickButtonInPage("Library");
  await until(() => evaluate("Boolean(document.querySelector('#resource-search'))"), "Library Resource search");
  await evaluate("document.querySelector('#resource-search').focus()");
  await cdp("Input.insertText", { text: "query-a" });
  await until(() => evaluate("window.searchPendingCount('workspace-a') === 1"), "deferred Workspace A search response");

  await evaluate("(() => { const select=document.querySelector('select[aria-label=\"Selected Workspace\"]'); const setter=Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype,'value').set; setter.call(select,'workspace-b'); select.dispatchEvent(new Event('change',{bubbles:true})); })()");
  await until(() => evaluate("document.querySelector('select[aria-label=\"Selected Workspace\"]')?.value === 'workspace-b' && document.querySelector('#resource-search')?.value === ''"), "Library search reset for Workspace B");
  await evaluate("document.querySelector('#resource-search').focus()");
  await cdp("Input.insertText", { text: "query-b" });
  await until(() => evaluate("window.searchPendingCount('workspace-b') === 1"), "deferred Workspace B search response");
  await evaluate("window.resolveSearch('workspace-b')");
  await until(() => evaluate("document.body.innerText.includes('B search result.txt')"), "Workspace B search result");
  await evaluate("window.resolveSearch('workspace-a')");
  await wait(120);
  assert.equal(await evaluate("document.querySelector('select[aria-label=\"Selected Workspace\"]')?.value"), "workspace-b");
  assert.equal(await evaluate("document.body.innerText.includes('B search result.txt')"), true, "the current B search result remains visible");
  assert.equal(await evaluate("document.body.innerText.includes('A search result.txt')"), false, "a late A search response cannot populate B's Library");
});

async function clickButtonInPage(label: string) {
  const clicked = await evaluate(`(() => { const button=[...document.querySelectorAll('button')].find(candidate=>candidate.textContent.includes(${JSON.stringify(label)})); if(!button)return false; button.click(); return true; })()`);
  assert.equal(clicked, true, `button ${label} exists`);
}
