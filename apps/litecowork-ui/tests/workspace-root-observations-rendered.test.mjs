import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { createServer as createHttpServer } from "node:http";
import { test } from "node:test";
import react from "@vitejs/plugin-react";
import { createServer as createViteServer } from "vite";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const chrome = process.env.LITECOWORK_CHROME
  ?? ["/usr/bin/google-chrome", "/usr/bin/google-chrome-stable", "/usr/bin/chromium"]
    .find(candidate => existsSync(candidate));
const wait = milliseconds => new Promise(resolveWait => setTimeout(resolveWait, milliseconds));

async function until(check, label, timeoutMs = 10_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const value = await check();
    if (value) return value;
    await wait(50);
  }
  throw new Error(`Timed out waiting for ${label}`);
}

async function stopProcess(child) {
  if (!child?.pid) return;
  const target = process.platform === "win32" ? child.pid : -child.pid;
  try { process.kill(target, "SIGTERM"); }
  catch (error) { if (error.code !== "ESRCH") throw error; }
  await Promise.race([
    new Promise(resolveExit => {
      if (child.exitCode !== null || child.signalCode !== null) resolveExit();
      else child.once("exit", resolveExit);
    }),
    wait(3000),
  ]);
  if (process.platform !== "win32") {
    try { process.kill(target, 0); process.kill(target, "SIGKILL"); }
    catch (error) { if (error.code !== "ESRCH") throw error; }
  } else if (child.exitCode === null && child.signalCode === null) child.kill("SIGKILL");
}

function observation(name, idChar) {
  return {
    observation_id: `sha256:${idChar.repeat(64)}`,
    display_name: name,
    size_bytes: 120,
    modified_at: null,
    observed_at: "2026-10-17T10:00:00.000Z",
    freshness: "CURRENT",
    resource_id: null,
  };
}

test("rendered Library observed files search, paginate, and discard a response after the root version changes", async t => {
  if (!chrome) {
    t.skip("Set LITECOWORK_CHROME to a local Chrome/Chromium executable.");
    return;
  }

  let vite;
  let httpServer;
  let chromeProcess;
  let socket;
  let profile;
  let viteCache;
  let chromeStderr = "";
  let chromeExit = null;
  let chromeSpawnError = null;
  let nextCommandId = 0;
  const pendingCommands = new Map();
  const diagnostics = [];

  const cdp = (method, params = {}) => {
    const id = ++nextCommandId;
    const response = new Promise((resolveResponse, rejectResponse) => {
      const timeout = setTimeout(() => {
        pendingCommands.delete(id);
        rejectResponse(new Error(`Chrome DevTools timed out on ${method}`));
      }, 8000);
      pendingCommands.set(id, message => {
        clearTimeout(timeout);
        message.error ? rejectResponse(new Error(`${method}: ${message.error.message}`)) : resolveResponse(message.result);
      });
    });
    socket.send(JSON.stringify({ id, method, params }));
    return response;
  };
  const evaluate = async expression => {
    const response = await cdp("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true, userGesture: true });
    if (response.exceptionDetails) throw new Error(response.exceptionDetails.exception?.description ?? response.exceptionDetails.text);
    return response.result.value;
  };
  const click = async (selector, label) => {
    const clicked = await evaluate(`(() => { const button=document.querySelector(${JSON.stringify(selector)}); if(!button || button.disabled)return false; button.click(); return true; })()`);
    assert.equal(clicked, true, `enabled ${label} button exists`);
  };

  try {
    viteCache = await mkdtemp(join(tmpdir(), "litecowork-observed-files-vite-"));
    profile = await mkdtemp(join(tmpdir(), "litecowork-observed-files-chrome-"));
    const harnessHtml = (await readFile(join(packageRoot, "tests/browser/harness.html"), "utf8"))
      .replace('src="./harness.tsx"', 'src="/tests/browser/harness.tsx"');
    vite = await createViteServer({
      configFile: false,
      root: packageRoot,
      cacheDir: viteCache,
      server: { middlewareMode: true, hmr: false, ws: false },
      appType: "custom",
      optimizeDeps: { noDiscovery: true, include: ["react", "react-dom/client"] },
      plugins: [react(), {
        name: "observed-files-rendered-test-document",
        configureServer(server) {
          server.middlewares.use("/tests/browser/harness.html", async (_request, response, next) => {
            try {
              response.setHeader("content-type", "text/html");
              response.end(await server.transformIndexHtml("/tests/browser/harness.html", harnessHtml));
            } catch (error) { next(error); }
          });
        },
      }],
    });
    httpServer = createHttpServer(vite.middlewares);
    await new Promise((resolveListen, rejectListen) => {
      httpServer.once("error", rejectListen);
      httpServer.listen(0, "127.0.0.1", resolveListen);
    });
    const address = httpServer.address();
    assert.ok(address && typeof address === "object");
    const harnessResponse = await fetch(`http://127.0.0.1:${address.port}/tests/browser/harness.html`);
    assert.equal(harnessResponse.status, 200);

    chromeProcess = spawn(chrome, [
      "--headless=new", "--no-sandbox", "--disable-gpu", "--disable-dev-shm-usage",
      "--remote-debugging-port=0", "--remote-allow-origins=*", `--user-data-dir=${profile}`, "about:blank",
    ], { detached: process.platform !== "win32", stdio: ["ignore", "ignore", "pipe"] });
    chromeProcess.stderr?.setEncoding("utf8");
    chromeProcess.stderr?.on("data", chunk => { chromeStderr = `${chromeStderr}${chunk}`.slice(-4096); });
    chromeProcess.once("error", error => { chromeSpawnError = error; });
    chromeProcess.once("exit", (code, signal) => { chromeExit = { code, signal }; });
    const portFile = join(profile, "DevToolsActivePort");
    const debugPort = await until(async () => {
      if (chromeSpawnError || chromeExit) throw new Error(`Chrome exited before DevTools started: ${chromeSpawnError?.message ?? JSON.stringify(chromeExit)} ${chromeStderr}`);
      try {
        const value = Number((await readFile(portFile, "utf8")).split(/\r?\n/, 1)[0]);
        return Number.isInteger(value) && value > 0 ? value : null;
      } catch { return null; }
    }, "Chrome DevTools port");
    const target = await until(async () => {
      const response = await fetch(`http://127.0.0.1:${debugPort}/json`);
      return response.ok ? (await response.json()).find(entry => entry.type === "page") ?? null : null;
    }, "Chrome page target");
    socket = new WebSocket(target.webSocketDebuggerUrl);
    await new Promise((resolveSocket, rejectSocket) => {
      socket.addEventListener("open", resolveSocket, { once: true });
      socket.addEventListener("error", rejectSocket, { once: true });
    });
    socket.addEventListener("message", event => {
      const message = JSON.parse(event.data);
      if (message.id) pendingCommands.get(message.id)?.(message);
      else if (message.method === "Runtime.exceptionThrown") diagnostics.push(message.params.exceptionDetails?.exception?.description ?? message.params.exceptionDetails?.text);
      else if (message.method === "Runtime.consoleAPICalled") diagnostics.push(`${message.params.type}: ${message.params.args?.map(argument => argument.value ?? argument.description).join(" ")}`);
      else if (message.method === "Log.entryAdded") diagnostics.push(`log: ${message.params.entry.text}`);
    });
    await cdp("Page.enable");
    await cdp("Runtime.enable");
    await cdp("Log.enable");
    await cdp("Page.navigate", { url: `http://127.0.0.1:${address.port}/tests/browser/harness.html` });
    await until(() => evaluate("Boolean(document.querySelector('.harness-ready'))"), "rendered production harness", 30_000);

    await evaluate(`(() => {
      const bridge = window.__TAURI_INTERNALS__;
      const originalInvoke = bridge.invoke.bind(bridge);
      window.__observationCalls = [];
      window.__observationPending = [];
      window.__observationPauseCalls = [];
      bridge.invoke = (command, args, options) => {
        if (command === "search_workspace_root_observations") {
          window.__observationCalls.push(args);
          if (args.query === "version-check") return new Promise(resolve => window.__observationPending.push(resolve));
          let page;
          if (args.query === "plan" && args.cursor) page = {
            workspace_id: "workspace-a", workspace_root_id: "workspace-root-a",
            items: [${JSON.stringify(observation("Plan two.txt", "2"))}], next_cursor: null,
          };
          else if (args.query === "plan") page = {
            workspace_id: "workspace-a", workspace_root_id: "workspace-root-a",
            items: [${JSON.stringify(observation("Plan one.txt", "1"))}], next_cursor: "plan-next",
          };
          else page = {
            workspace_id: "workspace-a", workspace_root_id: "workspace-root-a",
            items: [${JSON.stringify(observation("Quarterly notes.txt", "3"))}], next_cursor: null,
          };
          return Promise.resolve(page);
        }
        if (command === "pause_workspace_root") {
          window.__observationPauseCalls.push(args);
          return Promise.resolve({
            workspaceId: args.workspaceId, workspaceRootId: args.workspaceRootId,
            resourceId: "resource-folder-a", displayName: "Project folder",
            watchPolicy: "METADATA", replicationPolicy: "NONE",
            status: "PAUSED", version: args.expectedVersion + 1, locationAvailability: "AVAILABLE",
          });
        }
        return originalInvoke(command, args, options);
      };
      window.resolveStaleObservationPage = () => window.__observationPending.shift()({
        workspace_id: "workspace-a", workspace_root_id: "workspace-root-a",
        items: [${JSON.stringify(observation("Late stale result.txt", "4"))}], next_cursor: null,
      });
      return true;
    })()`);
    await evaluate(`window.litecoworkHarness.mountProductionApp("workspace-root-scan-unsupported")`);
    await until(() => evaluate("Boolean(document.querySelector('.workspace-switcher'))"), "production Workspace shell");
    const libraryClicked = await evaluate(`(() => { const item=[...document.querySelectorAll('.nav-list .nav-item')].find(node=>node.textContent.trim().includes('Library')); if(!item)return false; item.click(); return true; })()`);
    assert.equal(libraryClicked, true, "Library navigation item exists");
    await until(() => evaluate("Boolean(document.querySelector('.workspace-root-observations'))"), "observed-files panel mounted from Library root fixture");
    await click(".workspace-root-observations-toggle", "Browse observed files");
    await until(() => evaluate("document.body.innerText.includes('Quarterly notes.txt')"), "first observed metadata row");
    assert.equal(await evaluate("document.body.innerText.includes('not files added to the Library')"), true, "the UI explains these are metadata observations rather than Library Resources");
    assert.equal(await evaluate("window.__observationCalls[0].rootVersion"), 7, "the page request carries the selected WorkspaceRoot version");

    await evaluate(`(() => {
      const input=document.querySelector('.workspace-root-observations input');
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set.call(input,'plan');
      input.dispatchEvent(new Event('input',{bubbles:true}));
      const form=input.closest('form');
      form.requestSubmit();
    })()`);
    await until(() => evaluate("window.__observationCalls.filter(call => call.query === 'plan').length === 1"), "submitted observed-file name search");
    await until(() => evaluate("document.body.innerText.includes('Plan one.txt')"), "first search result");
    const loadMoreClicked = await evaluate(`(() => { const button=[...document.querySelectorAll('.workspace-root-observations button')].find(node=>node.textContent.includes('Load more observed files')); if(!button || button.disabled)return false; button.click(); return true; })()`);
    assert.equal(loadMoreClicked, true, "Load more observed files is enabled");
    await until(() => evaluate("window.__observationCalls.filter(call => call.query === 'plan').length === 2"), "second search page request");
    assert.equal(await evaluate("window.__observationCalls.filter(call => call.query === 'plan')[1].cursor"), "plan-next");
    await until(() => evaluate("document.body.innerText.includes('Plan two.txt')"), "second search result");

    await click('[aria-label="Scan folder metadata in Project folder"]', "Scan folder");
    await until(() => evaluate("window.litecoworkHarness.workspaceRootScanPendingCount() === 1"), "unsupported-platform scan request");
    await evaluate(`window.litecoworkHarness.rejectWorkspaceRootScan("SCAN_UNSUPPORTED_PLATFORM")`);
    await until(() => evaluate("document.body.innerText.includes('Metadata scanning is not available on this platform')"), "separate unsupported-platform explanation");

    await evaluate(`(() => {
      const input=document.querySelector('.workspace-root-observations input');
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set.call(input,'version-check');
      input.dispatchEvent(new Event('input',{bubbles:true}));
      input.closest('form').requestSubmit();
    })()`);
    await until(() => evaluate("window.__observationCalls.some(call => call.query === 'version-check')"), "request pinned to the old root version");
    await click('[aria-label="Pause Project folder"]', "Pause folder");
    await until(() => evaluate("document.body.innerText.includes('This folder is paused. Resume it to refresh observed files.')"), "paused root state");
    assert.equal(await evaluate("window.__observationPauseCalls[0].expectedVersion"), 7, "the pause action is fenced by the displayed root version");
    await evaluate("window.resolveStaleObservationPage()");
    await wait(100);
    assert.equal(await evaluate("document.body.innerText.includes('Late stale result.txt')"), false, "a response from the previous root version cannot populate the refreshed panel");
    assert.equal(await evaluate("document.body.innerText.includes('not files added to the Library')"), true);
    assert.deepEqual(await evaluate("window.__observationCalls.map(call => [call.query, call.rootVersion, call.cursor])"), [
      ["", 7, null], ["plan", 7, null], ["plan", 7, "plan-next"], ["version-check", 7, null],
    ]);
  } catch (error) {
    const renderedState = await evaluate(`JSON.stringify({
      body: document.body.innerText,
      observationCalls: window.__observationCalls,
      pauseCalls: window.__observationPauseCalls,
    })`).catch(() => "unavailable");
    error.message = `${error.message}; rendered state=${renderedState}; browser diagnostics=${JSON.stringify(diagnostics)}`;
    throw error;
  } finally {
    try { socket?.close(); } catch {}
    await stopProcess(chromeProcess).catch(() => {});
    if (httpServer?.listening) await new Promise(resolveClose => httpServer.close(resolveClose));
    await vite?.close();
    if (profile) await rm(profile, { recursive: true, force: true });
    if (viteCache) await rm(viteCache, { recursive: true, force: true });
  }
});
