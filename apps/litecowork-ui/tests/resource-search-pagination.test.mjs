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
  } else if (child.exitCode === null && child.signalCode === null) {
    child.kill("SIGKILL");
  }
}

function resource(resourceId, displayName) {
  return {
    resourceId,
    resourceRevisionId: `${resourceId}-revision-1`,
    sourceContentDigest: `sha256:${"a".repeat(64)}`,
    sourceMatches: [],
    displayName,
    freshness: "CURRENT",
    matchReasons: ["CONTENT_INDEXED"],
    snippet: `${displayName} matched the indexed query.`,
  };
}

test("rendered indexed search advances across an empty page and stops at the terminal cursor", async t => {
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
  const browserDiagnostics = [];

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
  };

  const clickButton = async label => {
    const clicked = await evaluate(`(() => {
      const button = [...document.querySelectorAll("button")].find(candidate => candidate.textContent.trim() === ${JSON.stringify(label)});
      if (!button) return false;
      button.click();
      return true;
    })()`);
    assert.equal(clicked, true, `the rendered ${JSON.stringify(label)} button exists`);
  };

  const clickNavigationItem = async label => {
    const clicked = await evaluate(`(() => {
      const button = [...document.querySelectorAll(".nav-list .nav-item")]
        .find(candidate => candidate.querySelector("span:last-child")?.textContent.trim() === ${JSON.stringify(label)});
      if (!button) return false;
      button.click();
      return true;
    })()`);
    assert.equal(clicked, true, `the rendered ${JSON.stringify(label)} navigation item exists`);
  };

  try {
    viteCache = await mkdtemp(join(tmpdir(), "litecowork-resource-search-vite-"));
    profile = await mkdtemp(join(tmpdir(), "litecowork-resource-search-chrome-"));
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
        name: "resource-search-rendered-test-document",
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
    assert.ok(address && typeof address === "object", "the Vite middleware server is listening");
    const harnessResponse = await fetch(`http://127.0.0.1:${address.port}/tests/browser/harness.html`);
    assert.equal(harnessResponse.status, 200, `Vite serves the existing browser harness: ${await harnessResponse.text()}`);

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
      if (chromeSpawnError || chromeExit) {
        throw new Error(`Chrome exited before DevTools started: ${chromeSpawnError?.message ?? JSON.stringify(chromeExit)} ${chromeStderr}`);
      }
      try {
        const value = Number((await readFile(portFile, "utf8")).split(/\r?\n/, 1)[0]);
        return Number.isInteger(value) && value > 0 ? value : null;
      } catch { return null; }
    }, "Chrome DevTools port");
    const target = await until(async () => {
      const response = await fetch(`http://127.0.0.1:${debugPort}/json`);
      if (!response.ok) return null;
      return (await response.json()).find(entry => entry.type === "page") ?? null;
    }, "Chrome page target");
    socket = new WebSocket(target.webSocketDebuggerUrl);
    await new Promise((resolveSocket, rejectSocket) => {
      socket.addEventListener("open", resolveSocket, { once: true });
      socket.addEventListener("error", rejectSocket, { once: true });
    });
    socket.addEventListener("message", event => {
      const message = JSON.parse(event.data);
      if (message.id) pendingCommands.get(message.id)?.(message);
      else if (message.method === "Runtime.exceptionThrown") {
        browserDiagnostics.push(message.params.exceptionDetails?.exception?.description ?? message.params.exceptionDetails?.text);
      } else if (message.method === "Runtime.consoleAPICalled") {
        browserDiagnostics.push(`${message.params.type}: ${message.params.args?.map(argument => argument.value ?? argument.description).join(" ")}`);
      } else if (message.method === "Log.entryAdded") {
        browserDiagnostics.push(`log: ${message.params.entry.text}`);
      }
    });
    await cdp("Page.enable");
    await cdp("Runtime.enable");
    await cdp("Log.enable");
    await cdp("Page.navigate", { url: `http://127.0.0.1:${address.port}/tests/browser/harness.html` });
    try {
      await until(() => evaluate("Boolean(document.querySelector('.harness-ready'))"), "rendered production harness", 30_000);
    } catch (error) {
      const pageState = await evaluate(`JSON.stringify({
        location: location.href,
        readyState: document.readyState,
        title: document.title,
        text: document.body.innerText,
        html: document.body.innerHTML.slice(0, 1200),
      })`).catch(failure => failure.message);
      throw new Error(`${error.message}; browser diagnostics=${JSON.stringify(browserDiagnostics)}; page=${pageState}`);
    }

    await evaluate(`(() => {
      const bridge = window.__TAURI_INTERNALS__;
      const originalInvoke = bridge.invoke.bind(bridge);
      window.__resourceSearchCalls = [];
      window.__resourceSearchPending = [];
      bridge.invoke = (command, args, options) => {
        if (command === "list_resources") {
          return Promise.resolve({
            items: [{
              resourceId: "library-seed",
              workspaceId: "workspace-a",
              resourceRevisionId: "library-seed-revision-1",
              displayName: "Library seed.txt",
              mediaType: "text/plain",
              contentDigest: "sha256:${"b".repeat(64)}",
              sizeBytes: 24,
            }],
            nextCursor: null,
          });
        }
        if (command === "search_resources") {
          window.__resourceSearchCalls.push(args);
          return new Promise(resolvePage => window.__resourceSearchPending.push(resolvePage));
        }
        return originalInvoke(command, args, options);
      };
      window.resolveResourceSearchPage = (index, page) => window.__resourceSearchPending[index](page);
      return true;
    })()`);
    await evaluate(`window.litecoworkHarness.mountProductionApp("first-use-conversation")`);
    await until(() => evaluate("Boolean(document.querySelector('.workspace-switcher'))"), "production App Workspace shell");
    await clickNavigationItem("Library");
    await until(() => evaluate("Boolean(document.querySelector('#resource-search'))"), "Library search controls");

    await evaluate(`(() => {
      const mode = document.querySelector("#resource-search-mode");
      Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, "value").set.call(mode, "INDEXED_CONTENT");
      mode.dispatchEvent(new Event("change", { bubbles: true }));
      return true;
    })()`);
    await evaluate(`(() => {
      const query = document.querySelector("#resource-search");
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set.call(query, "pagination regression");
      query.dispatchEvent(new Event("input", { bubbles: true }));
      return true;
    })()`);

    await until(() => evaluate("window.__resourceSearchCalls.length === 1"), "first indexed Resource search request");
    assert.equal(await evaluate("window.__resourceSearchCalls[0].mode"), "INDEXED_CONTENT");
    assert.equal(await evaluate("window.__resourceSearchCalls[0].cursor"), null, "the first page starts without a cursor");
    await evaluate(`window.resolveResourceSearchPage(0, ${JSON.stringify({
      items: [resource("resource-first", "First indexed result.txt")],
      nextCursor: "scan-frontier-1",
      mode: "INDEXED_CONTENT",
      contentScan: null,
    })})`);
    await until(() => evaluate("document.body.innerText.includes('First indexed result.txt')"), "first indexed result");

    await clickButton("Load more results");
    await until(() => evaluate("window.__resourceSearchCalls.length === 2"), "empty continuation page request");
    assert.equal(await evaluate("window.__resourceSearchCalls[1].cursor"), "scan-frontier-1", "the first continuation uses the returned cursor");
    await evaluate(`window.resolveResourceSearchPage(1, ${JSON.stringify({
      items: [],
      nextCursor: "scan-frontier-2",
      mode: "INDEXED_CONTENT",
      contentScan: null,
    })})`);
    await until(() => evaluate(`(() => {
      const button = [...document.querySelectorAll("button")].find(candidate => candidate.textContent.trim() === "Load more results");
      return Boolean(button && !button.disabled);
    })()`), "enabled Load more button after an empty advancing page");
    assert.equal(await evaluate("document.body.innerText.includes('Workspace search returned a repeated page or cursor.')"), false, "a changed cursor is not reported as a repeated-cursor error");
    assert.equal(await evaluate("[...document.querySelectorAll('.resource-list li strong')].filter(node => node.textContent === 'First indexed result.txt').length"), 1, "the empty page leaves the existing result exactly once");

    await clickButton("Load more results");
    await until(() => evaluate("window.__resourceSearchCalls.length === 3"), "result page request after the empty continuation");
    assert.equal(await evaluate("window.__resourceSearchCalls[2].cursor"), "scan-frontier-2", "the next request advances beyond the empty page cursor");
    await evaluate(`window.resolveResourceSearchPage(2, ${JSON.stringify({
      items: [resource("resource-second", "Second indexed result.txt")],
      nextCursor: null,
      mode: "INDEXED_CONTENT",
      contentScan: null,
    })})`);
    await until(() => evaluate("document.body.innerText.includes('Second indexed result.txt')"), "result from the page after the empty continuation");

    assert.deepEqual(await evaluate("window.__resourceSearchCalls.map(call => call.cursor)"), [null, "scan-frontier-1", "scan-frontier-2"]);
    assert.deepEqual(await evaluate("[...document.querySelectorAll('.resource-list li strong')].map(node => node.textContent)"), [
      "First indexed result.txt",
      "Second indexed result.txt",
    ], "results remain unique across the empty page and the next populated page");
    assert.equal(await evaluate("[...document.querySelectorAll('button')].some(button => button.textContent.trim() === 'Load more results')"), false, "a null terminal cursor removes Load more");
  } finally {
    try { socket?.close(); } catch {}
    await stopProcess(chromeProcess).catch(() => {});
    if (httpServer?.listening) await new Promise(resolveClose => httpServer.close(resolveClose));
    await vite?.close();
    if (profile) await rm(profile, { recursive: true, force: true });
    if (viteCache) await rm(viteCache, { recursive: true, force: true });
  }
});
