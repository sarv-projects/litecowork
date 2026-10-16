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
const stylesPath = join(packageRoot, "src/styles.css");
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

function cssDeclarations(css, selector) {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = css.match(new RegExp(`${escaped}\\s*\\{([^}]*)\\}`));
  assert.ok(match, `the stylesheet defines ${selector}`);
  return Object.fromEntries([...match[1].matchAll(/([\w-]+)\s*:\s*([^;]+);/g)]
    .map(([, property, value]) => [property, value.trim()]));
}

function parseRgb(value, variables, seen = new Set()) {
  const token = value.match(/^var\(\s*(--[\w-]+)\s*\)$/);
  if (token) {
    assert.ok(!seen.has(token[1]), `CSS token ${token[1]} does not cycle`);
    const nextSeen = new Set(seen).add(token[1]);
    return parseRgb(variables[token[1]], variables, nextSeen);
  }
  const named = value.toLowerCase() === "white" ? "#ffffff" : value;
  const hex = named.match(/^#([\da-f]{3}|[\da-f]{6})$/i)?.[1];
  assert.ok(hex, `contrast test can resolve color ${JSON.stringify(value)}`);
  const expanded = hex.length === 3 ? [...hex].map(channel => `${channel}${channel}`).join("") : hex;
  return [0, 2, 4].map(index => Number.parseInt(expanded.slice(index, index + 2), 16) / 255);
}

function luminance(rgb) {
  const channels = rgb.map(channel => channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4);
  return 0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2];
}

test("normal quiet-button text meets WCAG AA contrast on its white surface", async () => {
  const css = await readFile(stylesPath, "utf8");
  const root = css.match(/:root\s*\{([^}]*)\}/)?.[1] ?? "";
  const variables = Object.fromEntries([...root.matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)]
    .map(([, name, value]) => [name, value.trim()]));
  const button = cssDeclarations(css, ".quiet-button");
  const foregroundValue = button.color;
  const backgroundValue = button.background;
  assert.ok(foregroundValue, "quiet buttons declare their text color");
  assert.ok(backgroundValue, "quiet buttons declare their surface color");
  const foreground = luminance(parseRgb(foregroundValue, variables));
  const background = luminance(parseRgb(backgroundValue, variables));
  const ratio = (Math.max(foreground, background) + 0.05) / (Math.min(foreground, background) + 0.05);
  assert.ok(ratio >= 4.5, `normal quiet-button text contrast is ${ratio.toFixed(2)}:1 (WCAG AA requires 4.5:1)`);
});

test("Settings keeps a plain local status visible and exposes technical details by keyboard", async t => {
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
  let commandId = 0;
  const pendingCommands = new Map();

  const cdp = (method, params = {}) => {
    const id = ++commandId;
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

  const key = async (keyName, code, windowsVirtualKeyCode) => {
    await cdp("Input.dispatchKeyEvent", { type: "rawKeyDown", key: keyName, code, windowsVirtualKeyCode });
    if (keyName === "Enter" || keyName === " ") {
      await cdp("Input.dispatchKeyEvent", {
        type: "char",
        key: keyName,
        code,
        text: keyName === "Enter" ? "\r" : " ",
        unmodifiedText: keyName === "Enter" ? "\r" : " ",
        windowsVirtualKeyCode,
      });
    }
    await cdp("Input.dispatchKeyEvent", { type: "keyUp", key: keyName, code, windowsVirtualKeyCode });
  };

  const stopChrome = async () => {
    if (!chromeProcess?.pid) return;
    const target = process.platform === "win32" ? chromeProcess.pid : -chromeProcess.pid;
    try { process.kill(target, "SIGTERM"); }
    catch (error) { if (error.code !== "ESRCH") throw error; }
    await Promise.race([
      new Promise(resolveExit => {
        if (chromeProcess.exitCode !== null || chromeProcess.signalCode !== null) resolveExit();
        else chromeProcess.once("exit", resolveExit);
      }),
      wait(3000),
    ]);
    if (process.platform !== "win32") {
      try { process.kill(target, 0); process.kill(target, "SIGKILL"); }
      catch (error) { if (error.code !== "ESRCH") throw error; }
    } else if (chromeProcess.exitCode === null && chromeProcess.signalCode === null) {
      chromeProcess.kill("SIGKILL");
    }
  };

  try {
    viteCache = await mkdtemp(join(tmpdir(), "litecowork-settings-vite-"));
    profile = await mkdtemp(join(tmpdir(), "litecowork-settings-chrome-"));
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
        name: "settings-accessibility-test-document",
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
    assert.ok(address && typeof address === "object", "the rendered UI test server is listening");

    chromeProcess = spawn(chrome, [
      "--headless=new", "--no-sandbox", "--disable-gpu", "--disable-dev-shm-usage",
      "--remote-debugging-port=0", "--remote-allow-origins=*", `--user-data-dir=${profile}`, "about:blank",
    ], { detached: process.platform !== "win32", stdio: ["ignore", "ignore", "ignore"] });
    const portFile = join(profile, "DevToolsActivePort");
    const debugPort = await until(async () => {
      if (chromeProcess.exitCode !== null) throw new Error(`Chrome exited with ${chromeProcess.exitCode}`);
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
    });
    await cdp("Page.enable");
    await cdp("Runtime.enable");
    await cdp("Page.navigate", { url: `http://127.0.0.1:${address.port}/tests/browser/harness.html` });
    await until(() => evaluate("Boolean(document.querySelector('.harness-ready'))"), "rendered production harness", 30_000);
    assert.equal(await evaluate('window.litecoworkHarness.mountProductionApp("runtime-refresh-failure")'), true, "the production App is mounted with the local Runtime fixture");
    await until(() => evaluate("Boolean(document.querySelector('.nav-list .nav-item'))"), "production App navigation");
    assert.equal(await evaluate(`(() => {
      const item = [...document.querySelectorAll('.nav-list .nav-item')]
        .find(candidate => candidate.querySelector('span:last-child')?.textContent.trim() === 'Settings');
      item?.click();
      return Boolean(item);
    })()`), true, "the rendered Settings navigation item is available");
    await until(() => evaluate("document.querySelector('.page-content h1')?.textContent === 'Settings'"), "production Settings page");
    await until(() => evaluate("Boolean(document.querySelector('details.runtime-advanced-details'))"), "collapsed Advanced details disclosure");

    const status = await evaluate(`(() => {
      const card = document.querySelector('.runtime-card');
      return {
        text: card?.innerText ?? '',
        disclosureOpen: document.querySelector('details.runtime-advanced-details')?.open ?? null,
        summaryTag: document.querySelector('.runtime-advanced-details summary')?.tagName ?? null,
        hiddenRuntimeIdVisible: document.body.innerText.includes('runtime-local-test'),
      };
    })()`);
    assert.match(status.text, /Connected on this computer/i, "Settings shows a plain-language local service status");
    assert.match(status.text, /Task execution is not available yet/i, "Settings explains the current product limitation in plain language");
    assert.match(status.text, /Refresh status/, "a recovery action stays visible beside the status");
    assert.equal(status.disclosureOpen, false, "technical details start collapsed");
    assert.equal(status.summaryTag, "SUMMARY", "the disclosure uses the native keyboard-accessible summary element");
    assert.equal(status.hiddenRuntimeIdVisible, false, "Runtime identifiers are hidden while the disclosure is collapsed");

    await evaluate("document.querySelector('.runtime-card .button-row .quiet-button')?.focus()");
    await key("Tab", "Tab", 9);
    assert.equal(await evaluate("document.activeElement?.matches('.runtime-advanced-details summary') ?? false"), true, "Tab moves from the visible recovery action to Advanced details");
    await key("Enter", "Enter", 13);
    await until(() => evaluate("document.querySelector('details.runtime-advanced-details')?.open === true"), "keyboard-expanded details");
    assert.equal(await evaluate("document.body.innerText.includes('runtime-local-test')"), true, "Runtime identifiers are available after expansion");
    await key(" ", "Space", 32);
    await until(() => evaluate("document.querySelector('details.runtime-advanced-details')?.open === false"), "keyboard-collapsed details");
    assert.equal(await evaluate("document.body.innerText.includes('runtime-local-test')"), false, "Space collapses the technical details again");

    await evaluate("[...document.querySelectorAll('.runtime-card button')].find(button => button.textContent.trim() === 'Refresh status')?.click()");
    await until(() => evaluate("[...document.querySelectorAll('[role=alert]')].some(node => /Desktop Runtime status is unavailable/i.test(node.textContent ?? ''))"), "local service refresh error");
    const recovery = await evaluate(`(() => ({
      text: document.querySelector('.runtime-card')?.innerText ?? '',
      hasRetry: [...document.querySelectorAll('.runtime-card button')].some(button => button.textContent.trim() === 'Refresh status'),
    }))()`);
    assert.match(recovery.text, /Connection issue/i, "the failed refresh has a plain-language status");
    assert.match(recovery.text, /Refresh status to try again/i, "the visible status explains the recovery action");
    assert.equal(recovery.hasRetry, true, "the refresh action remains available after a failed check");
  } finally {
    socket?.close();
    await stopChrome();
    if (httpServer) await new Promise(resolveClose => httpServer.close(resolveClose));
    await vite?.close();
    if (profile) await rm(profile, { recursive: true, force: true });
    if (viteCache) await rm(viteCache, { recursive: true, force: true });
  }
});
