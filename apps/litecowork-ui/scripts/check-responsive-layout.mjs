import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import net from "node:net";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const chrome = process.env.LITECOWORK_CHROME
  ?? ["/usr/bin/google-chrome", "/usr/bin/google-chrome-stable", "/usr/bin/chromium"]
    .find(candidate => existsSync(candidate));

if (!chrome) throw new Error("Set LITECOWORK_CHROME to a local Chrome/Chromium executable.");

async function freePort() {
  const server = net.createServer();
  await new Promise(resolveListen => server.listen(0, "127.0.0.1", resolveListen));
  const { port } = server.address();
  await new Promise(resolveClose => server.close(resolveClose));
  return port;
}

async function waitFor(check, label) {
  const deadline = Date.now() + 15_000;
  let lastError;
  while (Date.now() < deadline) {
    try {
      const value = await check();
      if (value) return value;
    } catch (error) {
      lastError = error;
    }
    await new Promise(resolveDelay => setTimeout(resolveDelay, 100));
  }
  throw new Error(`${label} did not become ready${lastError ? `: ${lastError.message}` : ""}`);
}

async function sendCdp(ws, method, params = {}) {
  const id = ++sendCdp.nextId;
  const response = new Promise((resolveResponse, rejectResponse) => {
    const timeout = setTimeout(() => rejectResponse(new Error(`Chrome DevTools timed out on ${method}`)), 5000);
    sendCdp.pending.set(id, message => {
      clearTimeout(timeout);
      resolveResponse(message);
    });
  });
  ws.send(JSON.stringify({ id, method, params }));
  const message = await response;
  if (message.error) throw new Error(`${method}: ${message.error.message}`);
  return message.result;
}
sendCdp.nextId = 0;
sendCdp.pending = new Map();

const previewPort = await freePort();
const debugPort = await freePort();
const userData = await mkdtemp(join(tmpdir(), "litecowork-responsive-chrome-"));
const preview = spawn(process.execPath, [
  resolve(packageRoot, "node_modules/vite/bin/vite.js"),
  "preview", "--host", "127.0.0.1", "--port", String(previewPort), "--strictPort",
], { stdio: "ignore", detached: true });
let browser;
let ws;
let failure;

try {
  await waitFor(async () => (await fetch(`http://127.0.0.1:${previewPort}/`)).ok, "Vite preview");
  browser = spawn(chrome, [
    "--headless=new", "--no-sandbox", "--disable-gpu", "--disable-dev-shm-usage",
    "--disable-extensions", "--disable-background-networking", `--user-data-dir=${userData}`,
    `--remote-debugging-port=${debugPort}`, "--remote-allow-origins=*", "about:blank",
  ], { stdio: "ignore", detached: true });
  await waitFor(async () => (await fetch(`http://127.0.0.1:${debugPort}/json/version`)).ok, "Chrome DevTools");
  const target = await (await fetch(
    `http://127.0.0.1:${debugPort}/json/new?http://127.0.0.1:${previewPort}/`,
    { method: "PUT" },
  )).json();
  ws = new WebSocket(target.webSocketDebuggerUrl);
  ws.addEventListener("message", event => {
    const message = JSON.parse(event.data);
    if (message.id) sendCdp.pending.get(message.id)?.(message);
  });
  await new Promise((resolveOpen, rejectOpen) => {
    ws.addEventListener("open", resolveOpen, { once: true });
    ws.addEventListener("error", rejectOpen, { once: true });
  });
  await sendCdp(ws, "Page.enable");
  await sendCdp(ws, "Runtime.enable");
  await new Promise(resolveLoad => setTimeout(resolveLoad, 500));
  await waitFor(async () => {
    const result = await sendCdp(ws, "Runtime.evaluate", {
      expression: "Boolean(document.querySelector('.app-shell'))",
      returnByValue: true,
    });
    return result.result.value;
  }, "React desktop shell");

  const measure = async width => {
    await sendCdp(ws, "Emulation.setDeviceMetricsOverride", {
      width, height: 900, deviceScaleFactor: 1, mobile: false,
    });
    await new Promise(resolveLayout => setTimeout(resolveLayout, 200));
    const result = await sendCdp(ws, "Runtime.evaluate", {
      expression: `({width:innerWidth,root:document.documentElement.scrollWidth,body:document.body.scrollWidth,menu:getComputedStyle(document.querySelector('.mobile-navigation-toggle')).display,close:getComputedStyle(document.querySelector('.sidebar-close')).display})`,
      returnByValue: true,
    });
    return result.result.value;
  };

  const viewports = [];
  for (const width of [1280, 640, 512, 375]) {
    const measurement = await measure(width);
    viewports.push(measurement);
    assert.ok(measurement.root <= measurement.width, `document overflows horizontally at ${width}px: ${JSON.stringify(measurement)}`);
    assert.ok(measurement.body <= measurement.width, `body overflows horizontally at ${width}px: ${JSON.stringify(measurement)}`);
    if (width <= 900) {
      assert.ok(["flex", "inline-flex"].includes(measurement.menu), `menu is hidden at ${width}px`);
      assert.ok(["grid", "inline-grid"].includes(measurement.close), `drawer close control is hidden at ${width}px`);
    } else {
      assert.equal(measurement.menu, "none", "mobile menu must be hidden at desktop widths");
      assert.equal(measurement.close, "none", "drawer close must be hidden at desktop widths");
    }
  }

  await sendCdp(ws, "Emulation.setDeviceMetricsOverride", {
    width: 640, height: 900, deviceScaleFactor: 1, mobile: false,
  });
  await sendCdp(ws, "Runtime.evaluate", {
    expression: "document.querySelector('.mobile-navigation-toggle').click()",
  });
  await waitFor(async () => {
    const result = await sendCdp(ws, "Runtime.evaluate", {
      expression: "document.querySelector('.mobile-navigation-toggle').getAttribute('aria-expanded')",
      returnByValue: true,
    });
    return result.result.value === "true";
  }, "navigation drawer");
  const opened = (await sendCdp(ws, "Runtime.evaluate", {
    expression: `({expanded:document.querySelector('.mobile-navigation-toggle').getAttribute('aria-expanded'),modal:document.querySelector('#main-navigation').getAttribute('aria-modal'),inert:document.querySelector('.main-area').inert,focus:document.activeElement.getAttribute('aria-label')})`,
    returnByValue: true,
  })).result.value;
  assert.equal(opened.expanded, "true");
  assert.equal(opened.modal, "true");
  assert.equal(opened.inert, true);
  assert.equal(opened.focus, "Close navigation");

  await sendCdp(ws, "Runtime.evaluate", {
    expression: `(() => { const nodes=[...document.querySelector('#main-navigation').querySelectorAll('a[href],button:not(:disabled),select:not(:disabled),[tabindex]:not([tabindex="-1"])')]; nodes.at(-1).focus(); return nodes.length; })()`,
  });
  await sendCdp(ws, "Input.dispatchKeyEvent", { type: "keyDown", key: "Tab", code: "Tab", windowsVirtualKeyCode: 9 });
  await sendCdp(ws, "Input.dispatchKeyEvent", { type: "keyUp", key: "Tab", code: "Tab", windowsVirtualKeyCode: 9 });
  const tabWrapped = (await sendCdp(ws, "Runtime.evaluate", {
    expression: "document.activeElement.className",
    returnByValue: true,
  })).result.value;
  assert.equal(tabWrapped, "brand", "Tab from the last drawer control must wrap to the first");
  await sendCdp(ws, "Input.dispatchKeyEvent", { type: "keyDown", key: "Tab", code: "Tab", modifiers: 8, windowsVirtualKeyCode: 9 });
  await sendCdp(ws, "Input.dispatchKeyEvent", { type: "keyUp", key: "Tab", code: "Tab", modifiers: 8, windowsVirtualKeyCode: 9 });
  const shiftTabWrapped = (await sendCdp(ws, "Runtime.evaluate", {
    expression: "document.activeElement.tagName",
    returnByValue: true,
  })).result.value;
  assert.equal(shiftTabWrapped, "BUTTON", "Shift+Tab from the first drawer control must wrap to the last");

  await sendCdp(ws, "Input.dispatchKeyEvent", { type: "keyDown", key: "Escape", code: "Escape", windowsVirtualKeyCode: 27 });
  await sendCdp(ws, "Input.dispatchKeyEvent", { type: "keyUp", key: "Escape", code: "Escape", windowsVirtualKeyCode: 27 });
  await waitFor(async () => {
    const result = await sendCdp(ws, "Runtime.evaluate", {
      expression: `document.querySelector('.mobile-navigation-toggle').getAttribute('aria-expanded') === "false" && document.activeElement.getAttribute('aria-label') === "Open main navigation"`,
      returnByValue: true,
    });
    return result.result.value;
  }, "navigation close on Escape");
  const closed = (await sendCdp(ws, "Runtime.evaluate", {
    expression: `({expanded:document.querySelector('.mobile-navigation-toggle').getAttribute('aria-expanded'),inert:document.querySelector('.main-area').inert,focus:document.activeElement.getAttribute('aria-label')})`,
    returnByValue: true,
  })).result.value;
  assert.equal(closed.expanded, "false");
  assert.equal(closed.inert, false);
  assert.equal(closed.focus, "Open main navigation");

  await sendCdp(ws, "Runtime.evaluate", { expression: "document.querySelector('.mobile-navigation-toggle').click()" });
  await waitFor(async () => {
    const result = await sendCdp(ws, "Runtime.evaluate", {
      expression: "document.querySelector('.mobile-navigation-toggle').getAttribute('aria-expanded')",
      returnByValue: true,
    });
    return result.result.value === "true";
  }, "navigation drawer for backdrop test");
  await sendCdp(ws, "Runtime.evaluate", { expression: "document.querySelector('.sidebar-backdrop').click()" });
  await waitFor(async () => {
    const result = await sendCdp(ws, "Runtime.evaluate", {
      expression: `document.querySelector('.mobile-navigation-toggle').getAttribute('aria-expanded') === "false" && document.activeElement.getAttribute('aria-label') === "Open main navigation"`,
      returnByValue: true,
    });
    return result.result.value;
  }, "navigation close on backdrop");

  await sendCdp(ws, "Runtime.evaluate", { expression: "document.querySelector('.mobile-navigation-toggle').click()" });
  await waitFor(async () => {
    const result = await sendCdp(ws, "Runtime.evaluate", {
      expression: "document.querySelector('.mobile-navigation-toggle').getAttribute('aria-expanded')",
      returnByValue: true,
    });
    return result.result.value === "true";
  }, "navigation drawer for close-control test");
  await sendCdp(ws, "Runtime.evaluate", { expression: "document.querySelector('.sidebar-close').click()" });
  await waitFor(async () => {
    const result = await sendCdp(ws, "Runtime.evaluate", {
      expression: `document.querySelector('.mobile-navigation-toggle').getAttribute('aria-expanded') === "false" && document.activeElement.getAttribute('aria-label') === "Open main navigation"`,
      returnByValue: true,
    });
    return result.result.value;
  }, "navigation close control restores focus");

  await sendCdp(ws, "Runtime.evaluate", { expression: "document.querySelector('.mobile-navigation-toggle').click()" });
  await waitFor(async () => {
    const result = await sendCdp(ws, "Runtime.evaluate", {
      expression: "document.querySelector('.mobile-navigation-toggle').getAttribute('aria-expanded')",
      returnByValue: true,
    });
    return result.result.value === "true";
  }, "navigation drawer for selection test");
  await sendCdp(ws, "Runtime.evaluate", { expression: "document.querySelector('#main-navigation [aria-current=page]').click()" });
  await waitFor(async () => {
    const result = await sendCdp(ws, "Runtime.evaluate", {
      expression: `document.querySelector('.mobile-navigation-toggle').getAttribute('aria-expanded') === "false" && document.activeElement.getAttribute('aria-label') === "Open main navigation"`,
      returnByValue: true,
    });
    return result.result.value;
  }, "navigation selection closes drawer and restores focus");

  await sendCdp(ws, "Runtime.evaluate", { expression: "document.querySelector('.mobile-navigation-toggle').click()" });
  await waitFor(async () => {
    const result = await sendCdp(ws, "Runtime.evaluate", {
      expression: "document.querySelector('.mobile-navigation-toggle').getAttribute('aria-expanded')",
      returnByValue: true,
    });
    return result.result.value === "true";
  }, "navigation drawer for desktop resize test");
  await sendCdp(ws, "Emulation.setDeviceMetricsOverride", {
    width: 1280, height: 900, deviceScaleFactor: 1, mobile: false,
  });
  await waitFor(async () => {
    const result = await sendCdp(ws, "Runtime.evaluate", {
      expression: `document.querySelector('.mobile-navigation-toggle').getAttribute('aria-expanded') === "false" && document.activeElement.getAttribute('aria-current') === "page"`,
      returnByValue: true,
    });
    return result.result.value;
  }, "desktop resize closes drawer and focuses active navigation item");
  console.log(JSON.stringify({ viewports, drawerOpened: opened, tabWrapped, shiftTabWrapped, drawerClosedByEscape: closed, backdropClosed: true, closeControlRestoredFocus: true, selectionRestoredFocus: true, desktopResizeFocusRestored: true }, null, 2));
} catch (error) {
  failure = error;
} finally {
  ws?.close();
  for (const process of [browser, preview]) {
    if (!process || process.exitCode !== null) continue;
    try { process.kill(-process.pid, "SIGTERM"); }
    catch { process.kill("SIGTERM"); }
    await Promise.race([
      new Promise(resolveExit => process.once("exit", resolveExit)),
      new Promise(resolveDelay => setTimeout(resolveDelay, 3000)),
    ]);
    if (process.exitCode === null) {
      try { process.kill(-process.pid, "SIGKILL"); }
      catch { process.kill("SIGKILL"); }
    }
  }
  await rm(userData, { recursive: true, force: true });
}

if (failure) throw failure;
