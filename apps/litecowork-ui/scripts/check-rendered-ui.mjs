import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import net from "node:net";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const chromeCandidates = [
  "/usr/bin/google-chrome",
  "/usr/bin/google-chrome-stable",
  "/usr/bin/chromium",
  process.env.PROGRAMFILES && join(process.env.PROGRAMFILES, "Google", "Chrome", "Application", "chrome.exe"),
  process.env["PROGRAMFILES(X86)"] && join(process.env["PROGRAMFILES(X86)"], "Google", "Chrome", "Application", "chrome.exe"),
  process.env.LOCALAPPDATA && join(process.env.LOCALAPPDATA, "Google", "Chrome", "Application", "chrome.exe"),
  process.env["PROGRAMFILES(X86)"] && join(process.env["PROGRAMFILES(X86)"], "Microsoft", "Edge", "Application", "msedge.exe"),
  process.env.PROGRAMFILES && join(process.env.PROGRAMFILES, "Microsoft", "Edge", "Application", "msedge.exe"),
].filter(Boolean);
const chrome = process.env.LITECOWORK_CHROME ?? chromeCandidates.find(candidate => existsSync(candidate));

if (!chrome) throw new Error("Set LITECOWORK_CHROME to a local Chrome/Chromium executable.");

async function freePort() {
  const server = net.createServer();
  await new Promise((resolveListen, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolveListen);
  });
  const { port } = server.address();
  await new Promise((resolveClose, reject) => server.close(error => error ? reject(error) : resolveClose()));
  return port;
}

async function waitFor(check, label, timeoutMs = 12_000) {
  const deadline = Date.now() + timeoutMs;
  let lastError;
  while (Date.now() < deadline) {
    throwIfInterrupted();
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

async function waitForNavigationDrawerVisible(label) {
  await waitFor(() => evaluate(`(() => {
    const drawer = document.querySelector("#main-navigation");
    const more = drawer?.querySelector(".sidebar-more-summary");
    return Boolean(drawer?.classList.contains("is-open") && getComputedStyle(drawer).visibility === "visible" && drawer.getClientRects().length > 0 && more && getComputedStyle(more).visibility === "visible" && more.getClientRects().length > 0);
  })()`), label);
}

async function openNavigationByPointer() {
  const containerSelector = await evaluate("document.querySelector('.conversation-navigation-toggle') ? '.conversation-top-breadcrumbs' : '.topbar'");
  await evaluate(`document.querySelector(${JSON.stringify(containerSelector)} + " .mobile-navigation-toggle")?.scrollIntoView({ block: "nearest" })`);
  await clickButtonContainingByPointer(["open main navigation"], containerSelector);
}

function processIsRunning(child) {
  return child && child.exitCode === null && child.signalCode === null;
}

async function stopProcess(child) {
  if (!child?.pid) return;
  const signalTarget = process.platform === "win32" ? child.pid : -child.pid;
  try {
    process.kill(signalTarget, "SIGTERM");
  } catch (error) {
    if (error.code !== "ESRCH") throw error;
  }
  await Promise.race([
    new Promise(resolveExit => {
      if (child.exitCode !== null || child.signalCode !== null) resolveExit();
      else child.once("exit", resolveExit);
    }),
    new Promise(resolveDelay => setTimeout(resolveDelay, 3_000)),
  ]);
  let groupAlive = false;
  if (process.platform !== "win32") {
    try { process.kill(signalTarget, 0); groupAlive = true; }
    catch (error) { if (error.code !== "ESRCH") throw error; }
  }
  if (processIsRunning(child) || groupAlive) {
    try { process.kill(signalTarget, "SIGKILL"); }
    catch (error) { if (error.code !== "ESRCH") throw error; }
    await Promise.race([
      new Promise(resolveExit => {
        if (child.exitCode !== null || child.signalCode !== null) resolveExit();
        else child.once("exit", resolveExit);
      }),
      new Promise(resolveDelay => setTimeout(resolveDelay, 3_000)),
    ]);
  }
}

const previewPort = await freePort();
const debugPort = await freePort();
const userData = await mkdtemp(join(tmpdir(), "litecowork-rendered-ui-chrome-"));
const renderedOutput = join(userData, "rendered-output");
const chromeProfile = join(userData, "chrome-profile");
const viteCli = resolve(packageRoot, "node_modules/vite/bin/vite.js");
const browserViteConfig = resolve(packageRoot, "vite.browser.config.ts");
let vite;
let browser;
let socket;
let failure;
let interruptedSignal;
let openedTarget;
let navigationResult;
let nextCommandId = 0;
const pendingCommands = new Map();
const browserDiagnostics = [];
const browserRequestUrls = new Map();
const viteOutput = [];
const chromeOutput = [];

function captureOutput(target, stream) {
  stream?.on("data", chunk => {
    target.push(...chunk.toString().split(/\r?\n/).filter(Boolean));
    if (target.length > 80) target.splice(0, target.length - 80);
  });
}

function requireSuccessfulExit(child, label) {
  return new Promise((resolveExit, rejectExit) => {
    child.once("error", rejectExit);
    child.once("exit", (code, signal) => {
      if (code === 0) resolveExit();
      else rejectExit(new Error(`${label} exited with ${signal ?? `code ${code}`}.`));
    });
  });
}

for (const signal of ["SIGINT", "SIGTERM"]) {
  process.once(signal, () => { interruptedSignal = signal; });
}

function throwIfInterrupted() {
  if (interruptedSignal) throw new Error(`Rendered UI check interrupted by ${interruptedSignal}.`);
}

function cdp(method, params = {}, timeoutMs = 8_000) {
  throwIfInterrupted();
  const id = ++nextCommandId;
  const response = new Promise((resolveResponse, rejectResponse) => {
    const timeout = setTimeout(() => {
      pendingCommands.delete(id);
      rejectResponse(new Error(`Chrome DevTools timed out on ${method}`));
    }, timeoutMs);
    pendingCommands.set(id, message => {
      clearTimeout(timeout);
      if (message.error) rejectResponse(new Error(`${method}: ${message.error.message}`));
      else resolveResponse(message.result);
    });
  });
  socket.send(JSON.stringify({ id, method, params }));
  return response;
}

async function evaluate(expression, label = "browser evaluation") {
  const response = await cdp("Runtime.evaluate", {
    expression,
    returnByValue: true,
    awaitPromise: true,
    userGesture: true,
  });
  if (response.exceptionDetails) {
    throw new Error(`${label}: ${response.exceptionDetails.exception?.description ?? response.exceptionDetails.text}`);
  }
  return response.result.value;
}

async function clickButton(label, scopeSelector = null) {
  const root = scopeSelector === null ? "document" : `document.querySelector(${JSON.stringify(scopeSelector)})`;
  const expression = `(() => {
    const root = ${root};
    const button = [...root.querySelectorAll("button")].find(candidate => candidate.textContent.trim() === ${JSON.stringify(label)});
    if (!button) return false;
    button.click();
    return true;
  })()`;
  const clicked = await evaluate(expression, `click ${label}`);
  const labels = clicked ? null : await evaluate(`${root}.querySelectorAll("button").length ? [...${root}.querySelectorAll("button")].map(candidate => candidate.textContent.trim()) : []`);
  assert.equal(clicked, true, `button ${JSON.stringify(label)} is rendered${labels ? `; rendered button labels: ${JSON.stringify(labels)}` : ""}`);
}

async function clickButtonContaining(allTerms, scopeSelector = null) {
  const root = scopeSelector === null ? "document" : `document.querySelector(${JSON.stringify(scopeSelector)})`;
  const expression = `(() => {
    const root = ${root};
    if (!root) return false;
    const terms = ${JSON.stringify(allTerms.map(term => term.toLowerCase()))};
    const button = [...root.querySelectorAll("button")].find(candidate => {
      const name = (candidate.getAttribute("aria-label") ?? candidate.textContent).trim().toLowerCase();
      return terms.every(term => name.includes(term));
    });
    if (!button) return false;
    button.click();
    return true;
  })()`;
  assert.equal(await evaluate(expression, `click button containing ${allTerms.join(" + ")}`), true, `button containing ${JSON.stringify(allTerms.join(" + "))} is rendered`);
}

async function clickButtonContainingByPointer(allTerms, scopeSelector = null) {
  const root = scopeSelector === null ? "document" : `document.querySelector(${JSON.stringify(scopeSelector)})`;
  const position = await evaluate(`(() => {
    const root = ${root};
    if (!root) return null;
    const terms = ${JSON.stringify(allTerms.map(term => term.toLowerCase()))};
    const button = [...root.querySelectorAll("button")].find(candidate => {
      const name = (candidate.getAttribute("aria-label") ?? candidate.textContent).trim().toLowerCase();
      return terms.every(term => name.includes(term));
    });
    if (!button || button.disabled) return null;
    const rect = button.getBoundingClientRect();
    return { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 };
  })()`);
  assert.ok(position, `button containing ${JSON.stringify(allTerms.join(" + "))} is available for pointer input`);
  await cdp("Input.dispatchMouseEvent", { type: "mouseMoved", x: position.x, y: position.y });
  await cdp("Input.dispatchMouseEvent", { type: "mousePressed", x: position.x, y: position.y, button: "left", clickCount: 1 });
  await cdp("Input.dispatchMouseEvent", { type: "mouseReleased", x: position.x, y: position.y, button: "left", clickCount: 1 });
}

async function clickHarness(method, ...args) {
  const serialized = JSON.stringify(args);
  assert.equal(
    await evaluate(`window.litecoworkHarness.${method}(...${serialized})`, `harness ${method}`),
    true,
    `harness method ${method} is available`,
  );
}

async function pressKey(key, modifiers = 0, emitCharacter = false) {
  const codes = { Tab: 9, Escape: 27, Enter: 13, Space: 32, "Control+a": 65 };
  const base = key.startsWith("Control+") ? key.slice("Control+".length) : key;
  const code = codes[key] ?? codes[base] ?? 0;
  const event = { key: key === "Space" ? " " : base, code: key === "Space" ? "Space" : base, modifiers, windowsVirtualKeyCode: code, nativeVirtualKeyCode: code };
  await cdp("Input.dispatchKeyEvent", {
    type: "keyDown", ...event,
  });
  if (key === "Enter" && emitCharacter) await cdp("Input.dispatchKeyEvent", { type: "char", ...event, text: "\r", unmodifiedText: "\r" });
  await cdp("Input.dispatchKeyEvent", {
    type: "keyUp", ...event,
  });
}

async function typeTextareaDraft(text, initialValue = "Saved A source text") {
  await waitFor(async () => evaluate(`document.querySelector('#resource-revision-text')?.value === ${JSON.stringify(initialValue)}`), "Resource text editor");
  await evaluate(`(() => {
    const editor = document.querySelector("#resource-revision-text");
    editor.focus();
    editor.setSelectionRange(0, editor.value.length);
    return true;
  })()`);
  await cdp("Input.insertText", { text });
  await waitFor(() => evaluate(`document.querySelector("#resource-revision-text")?.value === ${JSON.stringify(text)}`), "edited Resource draft");
}

async function setTextareaDraft(text) {
  await waitFor(() => evaluate("Boolean(document.querySelector('#resource-revision-text'))"), "Resource text editor");
  await evaluate(`(() => {
    const editor = document.querySelector("#resource-revision-text");
    const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value").set;
    setter.call(editor, ${JSON.stringify(text)});
    editor.dispatchEvent(new Event("input", { bubbles: true }));
    return true;
  })()`);
  await waitFor(() => evaluate(`document.querySelector("#resource-revision-text")?.value.length === ${text.length}`), "programmatically edited Resource draft");
  const byteLength = new TextEncoder().encode(text).byteLength;
  const displayedSize = byteLength < 1024 ? `${byteLength} B` : byteLength < 1024 * 1024 ? `${Math.round(byteLength / 1024)} KB` : `${(byteLength / (1024 * 1024)).toFixed(1)} MB`;
  await waitFor(() => evaluate(`document.querySelector(".resource-revision-text-editor .resource-revision-actions span")?.textContent.startsWith(${JSON.stringify(`${displayedSize} · unsaved draft`)})`), "Resource draft state update");
}

async function typeInputValue(selector, text) {
  if (selector === "#conversation-title") {
    await evaluate(`(() => { const details = document.querySelector("#conversation-create-details"); if (details) details.open = true; return true; })()`);
  }
  await waitFor(() => evaluate(`(() => { const input = document.querySelector(${JSON.stringify(selector)}); return Boolean(input && !input.disabled); })()`), `${selector} enabled input`);
  await evaluate(`document.querySelector(${JSON.stringify(selector)}).focus()`);
  await cdp("Input.insertText", { text });
  await waitFor(() => evaluate(`document.querySelector(${JSON.stringify(selector)})?.value === ${JSON.stringify(text)}`), `${selector} value`);
}

async function selectResourceUploadFiles(files) {
  const serializedFiles = JSON.stringify(files);
  assert.equal(await evaluate(`(() => {
    const input = document.querySelectorAll('[aria-label="Add files to this Workspace"] input[type="file"]')[1];
    if (!input || input.disabled) return false;
    const transfer = new DataTransfer();
    for (const spec of ${serializedFiles}) {
      const bytes = new Uint8Array(spec.size ?? (4 * 1024 * 1024 + 1)).fill(spec.fill ?? 37);
      const file = new File([bytes], spec.name, { type: "text/plain", lastModified: 1700000000000 });
      window.litecoworkHarness.gateResourceUploadFileDigest(file);
      transfer.items.add(file);
    }
    input.files = transfer.files;
    input.dispatchEvent(new Event("change", { bubbles: true }));
    return true;
  })()`), true, "enabled Library input accepts the same 4 MiB + 1 byte test file");
}

async function selectResourceUploadFile(fill = 37, name = "f90-ambiguous-ack.txt") {
  await selectResourceUploadFiles([{ fill, name }]);
}

async function openResourceUploadScenario(scenario, label) {
  await clickHarness("mountProductionApp", scenario);
  await waitFor(() => evaluate(`Boolean(document.querySelector('.workspace-switcher'))`), `production App shell for ${label}`);
  await clickButtonContaining(["library"]);
  await waitFor(() => evaluate(`(() => { const inputs = document.querySelectorAll('[aria-label="Add files to this Workspace"] input[type="file"]'); return inputs.length === 2 && inputs[1].disabled === false; })()`), `selected Workspace and enabled upload input for ${label}`);
  await clickHarness("clearCalls");
}

async function runResourceUploadPauseChecks() {
  const checks = [];
  const uploadCommands = ["create_resource_upload", "get_resource_upload", "upload_resource_chunk", "commit_resource_upload"];

  const preSessionName = "f90-pause-before-session.txt";
  await openResourceUploadScenario("resource-upload-pause-before-session", "pause before session creation");
  await clickHarness("deferNextResourceFileDigest", preSessionName);
  await selectResourceUploadFile(37, preSessionName);
  await waitFor(() => evaluate("window.litecoworkHarness.resourceFileDigestPendingCount() === 1"), "deferred whole-file digest before upload session creation");
  await clickButton("Pause upload");
  await clickHarness("resolveResourceFileDigest");
  await waitFor(() => evaluate("document.querySelector('.resource-intake-status')?.textContent.includes('Upload paused before finishing') === true"), "pause completion before upload session creation");
  let calls = await evaluate("window.litecoworkHarness.calls()");
  assert.equal(calls.filter(call => uploadCommands.includes(call.command)).length, 0, "pause before session creation sends no upload lifecycle IPC");
  assert.equal(await evaluate("Object.keys(JSON.parse(localStorage.getItem('litecowork.resourceUploadResume.v1') ?? '{}')).length"), 0, "pause before session creation has no upload session recovery descriptor to retain");
  assert.equal(calls.some(call => /cancel.*resource.upload/i.test(call.command)), false, "client pause does not send a server cancellation command");
  checks.push("E02-S03 pause before session creation completes the gated digest and sends no upload IPC or cancellation");

  const createPendingName = "f90-pause-create-response.txt";
  await openResourceUploadScenario("resource-upload-pause-create-pending", "pause during create response");
  await clickHarness("deferNextResourceUploadCreate");
  await selectResourceUploadFile(37, createPendingName);
  await waitFor(() => evaluate("window.litecoworkHarness.resourceUploadCreatePendingCount() === 1"), "deferred create response after server-side session creation");
  const createPendingRequest = await evaluate(`(() => window.litecoworkHarness.calls().find(call => call.command === "create_resource_upload")?.args.requestId)()`);
  const createPendingSessionCount = await evaluate("window.litecoworkHarness.resourceUploadServerSessionCount()");
  assert.equal(createPendingSessionCount, 1, "the deferred create response represents one session already created by the harness service");
  await clickButton("Pause upload");
  await clickHarness("resolveResourceUploadCreate");
  await waitFor(() => evaluate("document.querySelector('.resource-intake-status')?.textContent.includes('Upload paused before finishing') === true"), "pause after in-flight create response");
  calls = await evaluate("window.litecoworkHarness.calls()");
  let createResume = await evaluate(`(() => Object.values(JSON.parse(localStorage.getItem("litecowork.resourceUploadResume.v1") ?? "{}")).find(value => value.displayName === ${JSON.stringify(createPendingName)}))()`);
  assert.ok(createResume?.uploadId, "the create response's session identity is retained for recovery");
  assert.equal(createResume.createRequestId, createPendingRequest, "the session keeps the original create RequestId");
  assert.equal(calls.filter(call => call.command === "upload_resource_chunk").length, 0, "no chunk starts after pause while create was pending");
  assert.equal(calls.filter(call => call.command === "commit_resource_upload").length, 0, "no commit starts after pause while create was pending");
  assert.equal(await evaluate("window.litecoworkHarness.resourceUploadServerSessionCount()"), 1, "pause does not imply server-side session cancellation");
  await selectResourceUploadFile(37, createPendingName);
  await waitFor(() => evaluate("window.litecoworkHarness.calls().filter(call => call.command === 'commit_resource_upload').length === 1"), "same created session resumes after an explicit re-selection");
  calls = await evaluate("window.litecoworkHarness.calls()");
  assert.equal(calls.filter(call => call.command === "create_resource_upload").length, 1, "resume uses the returned session without creating a replacement");
  assert.equal(calls.find(call => call.command === "get_resource_upload")?.args.uploadId, createResume.uploadId, "resume reads the exact session returned by the in-flight create");
  assert.deepEqual(calls.filter(call => call.command === "upload_resource_chunk").map(call => call.args.chunkIndex), [0, 1], "resume starts chunks only after the user reselects the file");
  assert.equal(await evaluate("window.litecoworkHarness.resourceUploadServerSessionCount()"), 1, "the explicit resume continues the original server session");
  checks.push("E02-S03 pause while create IPC is pending waits for its response, then retains and resumes that exact session without claiming cancellation");

  const progressPendingName = "f90-pause-progress-response.txt";
  await openResourceUploadScenario("resource-upload-pause-progress-pending", "pause during upload progress response");
  await selectResourceUploadFile(37, progressPendingName);
  await waitFor(() => evaluate("document.querySelector('.resource-intake-status')?.textContent.includes('lost the chunk acknowledgement') === true"), "an accepted chunk with an uncertain response before progress retry");
  const progressInitialCalls = await evaluate("window.litecoworkHarness.calls()");
  const progressCreateRequestId = progressInitialCalls.find(call => call.command === "create_resource_upload")?.args.requestId;
  const progressChunkRequestId = progressInitialCalls.find(call => call.command === "upload_resource_chunk")?.args.requestId;
  let progressResume = await evaluate(`(() => Object.values(JSON.parse(localStorage.getItem("litecowork.resourceUploadResume.v1") ?? "{}")).find(value => value.displayName === ${JSON.stringify(progressPendingName)}))()`);
  assert.ok(progressResume?.uploadId, "the uncertain accepted chunk has a durable-session identity to verify");
  await selectResourceUploadFile(37, progressPendingName);
  await waitFor(() => evaluate("window.litecoworkHarness.resourceProgressPendingCount() === 1"), "deferred progress read for the saved session");
  await clickButton("Pause upload");
  await clickHarness("resolveResourceProgressRead");
  await waitFor(() => evaluate("document.querySelector('.resource-intake-status')?.textContent.includes('Upload paused before finishing') === true"), "pause after in-flight progress response");
  calls = await evaluate("window.litecoworkHarness.calls()");
  assert.equal(calls.filter(call => call.command === "create_resource_upload").length, 1, "pause during progress verification starts no replacement session");
  assert.equal(calls.filter(call => call.command === "get_resource_upload").length, 1, "the in-flight verification is the only progress read before resume");
  assert.deepEqual(calls.filter(call => call.command === "upload_resource_chunk").map(call => call.args.chunkIndex), [0], "no later chunk starts after the deferred progress response");
  assert.equal(calls.filter(call => call.command === "commit_resource_upload").length, 0, "pause during progress verification does not commit");
  progressResume = await evaluate(`(() => Object.values(JSON.parse(localStorage.getItem("litecowork.resourceUploadResume.v1") ?? "{}")).find(value => value.displayName === ${JSON.stringify(progressPendingName)}))()`);
  assert.equal(progressResume?.uploadId, progressInitialCalls.find(call => call.command === "upload_resource_chunk")?.args.uploadId, "the saved progress response remains tied to the original upload session");
  assert.equal(progressResume.createRequestId, progressCreateRequestId, "the original create RequestId remains persisted during pause");
  assert.equal(progressResume.chunkRequestIds?.["0"], progressChunkRequestId, "the original uncertain chunk RequestId remains persisted during pause");
  await selectResourceUploadFile(37, progressPendingName);
  await waitFor(() => evaluate("window.litecoworkHarness.calls().filter(call => call.command === 'commit_resource_upload').length === 1"), "explicit re-selection resumes the verified session");
  calls = await evaluate("window.litecoworkHarness.calls()");
  assert.equal(calls.filter(call => call.command === "create_resource_upload").length, 1, "resuming after pause does not replace the session");
  assert.equal(calls.filter(call => call.command === "get_resource_upload").length, 2, "only explicit re-selection performs another progress read");
  assert.deepEqual(calls.filter(call => call.command === "upload_resource_chunk").map(call => call.args.chunkIndex), [0, 1], "accepted chunk is skipped and only the final missing chunk is sent");
  assert.equal(calls.find(call => call.command === "upload_resource_chunk")?.args.requestId, progressChunkRequestId, "the accepted chunk's recorded RequestId is unchanged");
  checks.push("E02-S03 pause while get_resource_upload is pending waits for its response, sends no later mutation, and resumes the retained session only after re-selection");

  const chunkPauseName = "f90-pause-pending-chunk.txt";
  await openResourceUploadScenario("resource-upload-pause-chunk-pending", "pause during a chunk request");
  await clickHarness("deferNextResourceUploadChunk", 0);
  await selectResourceUploadFile(37, chunkPauseName);
  await waitFor(() => evaluate("window.litecoworkHarness.resourceUploadChunkPendingCount() === 1"), "deferred chunk zero response after accepted range");
  await clickButton("Pause upload");
  await clickHarness("resolveResourceUploadChunk");
  await waitFor(() => evaluate("document.querySelector('.resource-intake-status')?.textContent.includes('Upload paused before finishing') === true"), "pause after in-flight chunk response");
  calls = await evaluate("window.litecoworkHarness.calls()");
  const acceptedChunkCalls = calls.filter(call => call.command === "upload_resource_chunk");
  assert.deepEqual(acceptedChunkCalls.map(call => call.args.chunkIndex), [0], "pause waits for chunk zero and sends no later chunk");
  assert.equal(calls.filter(call => call.command === "create_resource_upload").length, 1, "pause keeps the session already created for this unchanged file");
  assert.equal(calls.filter(call => call.command === "commit_resource_upload").length, 0, "pause does not commit an incomplete file");
  let resume = await evaluate(`(() => Object.values(JSON.parse(localStorage.getItem("litecowork.resourceUploadResume.v1") ?? "{}")).find(value => value.displayName === ${JSON.stringify(chunkPauseName)}))()`);
  assert.ok(resume?.uploadId, "the accepted chunk retains a locally recoverable upload session");
  assert.equal(resume.createRequestId, calls.find(call => call.command === "create_resource_upload")?.args.requestId, "the original create RequestId remains persisted");
  assert.equal(resume.chunkRequestIds?.["0"], acceptedChunkCalls[0].args.requestId, "the accepted chunk RequestId remains persisted");
  assert.match(await evaluate("document.querySelector('.resource-intake-status')?.textContent ?? ''"), /Any accepted chunks remain resumable/, "pause explains accepted chunks remain resumable");
  assert.equal(await evaluate("/cancelled|server.*cancel/i.test(document.querySelector('.resource-intake-status')?.textContent ?? '')"), false, "pause does not claim server cancellation");
  await selectResourceUploadFile(37, chunkPauseName);
  await waitFor(() => evaluate("window.litecoworkHarness.calls().filter(call => call.command === 'commit_resource_upload').length === 1"), "same unchanged file resumes and commits");
  calls = await evaluate("window.litecoworkHarness.calls()");
  assert.equal(calls.filter(call => call.command === "create_resource_upload").length, 1, "resume keeps the original session rather than creating a replacement");
  assert.equal(calls.find(call => call.command === "get_resource_upload")?.args.uploadId, resume.uploadId, "resume status read targets the exact saved session");
  assert.deepEqual(calls.filter(call => call.command === "upload_resource_chunk").map(call => call.args.chunkIndex), [0, 1], "resume sends only the missing final chunk");
  assert.ok(calls.filter(call => call.command === "upload_resource_chunk").every(call => call.args.uploadId === resume.uploadId), "all accepted chunk writes stay on the same session");
  assert.equal(await evaluate("Object.keys(JSON.parse(localStorage.getItem('litecowork.resourceUploadResume.v1') ?? '{}')).length"), 0, "confirmed commit clears local resume metadata");
  checks.push("E02-S03 pause during an accepted chunk waits for its response, sends no later chunk, and resumes the same file/session");

  const firstName = "f90-pause-first-file.txt";
  const secondName = "f90-pause-second-file.txt";
  await openResourceUploadScenario("resource-upload-pause-between-files", "pause between files");
  await clickHarness("deferNextResourceFileDigest", secondName);
  await selectResourceUploadFiles([
    { fill: 17, name: firstName, size: 48 },
    { fill: 29, name: secondName, size: 64 },
  ]);
  await waitFor(() => evaluate("window.litecoworkHarness.resourceFileDigestPendingCount() === 1"), "second-file digest after first file commit");
  calls = await evaluate("window.litecoworkHarness.calls()");
  assert.equal(calls.filter(call => call.command === "commit_resource_upload").length, 1, "first file commit is confirmed before the inter-file pause");
  assert.equal(await evaluate(`Boolean([...document.querySelectorAll('.resource-list li')].some(row => row.textContent.includes(${JSON.stringify(firstName)})))`), true, "the first file's committed Resource is already shown");
  await clickButton("Pause upload");
  await clickHarness("resolveResourceFileDigest");
  await waitFor(() => evaluate(`document.querySelector('.resource-intake-status')?.textContent.includes(${JSON.stringify(secondName)}) && document.querySelector('.resource-intake-status')?.textContent.includes('1 file was saved')`), "pause before second file session creation");
  calls = await evaluate("window.litecoworkHarness.calls()");
  const creates = calls.filter(call => call.command === "create_resource_upload");
  assert.equal(creates.length, 1, "pause between files starts no second session");
  assert.equal(creates[0].args.displayName, firstName, "only the first selected file created a session");
  assert.equal(calls.filter(call => call.command === "upload_resource_chunk").length, 1, "no chunk operation starts for the second file");
  assert.equal(calls.filter(call => call.command === "commit_resource_upload").length, 1, "no second-file commit starts");
  checks.push("E02-S03 pause between files preserves the first committed Resource and starts no operation for the next file");

  const beforeCommitName = "f90-pause-before-commit.txt";
  await openResourceUploadScenario("resource-upload-pause-before-commit", "pause before commit");
  await clickHarness("deferNextResourceUploadChunk", 1);
  await selectResourceUploadFile(37, beforeCommitName);
  await waitFor(() => evaluate("window.litecoworkHarness.resourceUploadChunkPendingCount() === 1 && window.litecoworkHarness.calls().some(call => call.command === 'upload_resource_chunk' && call.args.chunkIndex === 1)"), "deferred final chunk before commit");
  await clickButton("Pause upload");
  await clickHarness("resolveResourceUploadChunk");
  await waitFor(() => evaluate("document.querySelector('.resource-intake-status')?.textContent.includes('Upload paused before finishing') === true"), "pause after final chunk response but before commit");
  calls = await evaluate("window.litecoworkHarness.calls()");
  resume = await evaluate(`(() => Object.values(JSON.parse(localStorage.getItem("litecowork.resourceUploadResume.v1") ?? "{}")).find(value => value.displayName === ${JSON.stringify(beforeCommitName)}))()`);
  assert.ok(resume?.uploadId, "the complete but uncommitted upload retains its recovery session");
  assert.deepEqual(calls.filter(call => call.command === "upload_resource_chunk").map(call => call.args.chunkIndex), [0, 1], "the final accepted chunk remains represented exactly once");
  assert.equal(calls.filter(call => call.command === "commit_resource_upload").length, 0, "pause before commit does not send the commit request");
  await selectResourceUploadFile(37, beforeCommitName);
  await waitFor(() => evaluate("window.litecoworkHarness.calls().filter(call => call.command === 'commit_resource_upload').length === 1"), "same uncommitted session commits after re-selection");
  calls = await evaluate("window.litecoworkHarness.calls()");
  assert.equal(calls.filter(call => call.command === "create_resource_upload").length, 1, "re-selection does not create a replacement session");
  assert.equal(calls.find(call => call.command === "get_resource_upload")?.args.uploadId, resume.uploadId, "re-selection verifies the exact retained upload session");
  assert.deepEqual(calls.filter(call => call.command === "upload_resource_chunk").map(call => call.args.chunkIndex), [0, 1], "verified accepted ranges avoid duplicate chunk uploads");
  checks.push("E02-S03 pause before commit retains a complete upload session and re-selection commits without replacement or duplicate chunks");

  const commitPendingName = "f90-pause-commit-response.txt";
  await openResourceUploadScenario("resource-upload-pause-commit-pending", "pause during commit response");
  await clickHarness("deferNextResourceUploadCommit");
  await selectResourceUploadFiles([{ fill: 37, name: commitPendingName, size: 64 }]);
  await waitFor(() => evaluate("window.litecoworkHarness.resourceUploadCommitPendingCount() === 1"), "deferred commit response after server-side commit");
  const committedBeforeResponse = await evaluate("window.litecoworkHarness.resourceUploadCommittedReceipt()");
  assert.equal(committedBeforeResponse?.resourceId, "resource-f90", "the deferred request represents a commit already accepted by the server side");
  const commitResumeBeforeResponse = await evaluate(`(() => Object.values(JSON.parse(localStorage.getItem("litecowork.resourceUploadResume.v1") ?? "{}")).find(value => value.displayName === ${JSON.stringify(commitPendingName)}))()`);
  assert.ok(commitResumeBeforeResponse?.commitRequestId, "the pending commit has a persisted request identity");
  assert.equal(await evaluate("window.litecoworkHarness.resourceUploadListOmitsCommittedResource()"), true, "the fixture's list_resources response deliberately omits the committed object");
  assert.equal(await evaluate(`Boolean([...document.querySelectorAll('.resource-list li')].some(row => row.textContent.includes(${JSON.stringify(commitPendingName)})))`), false, "the Resource is not projected before its commit response arrives");
  await clickButton("Pause upload");
  assert.equal(await evaluate("document.querySelector('.resource-intake-status')?.textContent.includes('in-flight request will finish')"), true, "pause describes the in-flight commit honestly");
  await clickHarness("resolveResourceUploadCommit");
  await waitFor(() => evaluate(`Boolean([...document.querySelectorAll('.resource-list li')].some(row => row.textContent.includes(${JSON.stringify(commitPendingName)})))`), "exact Resource appears after committed response");
  const displayedCommit = await evaluate(`(() => {
    const row = [...document.querySelectorAll('.resource-list li')].find(candidate => candidate.querySelector('strong')?.textContent === ${JSON.stringify(commitPendingName)});
    return row ? { name: row.querySelector('strong')?.textContent, size: row.querySelector('small')?.textContent, digest: row.querySelector('.resource-digest')?.textContent } : null;
  })()`);
  assert.deepEqual(displayedCommit, {
    name: committedBeforeResponse.displayName,
    size: `${committedBeforeResponse.sizeBytes} B · ${committedBeforeResponse.mediaType}`,
    digest: `${committedBeforeResponse.contentDigest.slice(0, 19)}…`,
  }, "the exact commit response Resource is shown with its returned name, size/media type, and digest");
  assert.equal(await evaluate("Object.keys(JSON.parse(localStorage.getItem('litecowork.resourceUploadResume.v1') ?? '{}')).length"), 0, "confirmed committed response clears resume metadata");
  assert.equal(await evaluate("document.querySelector('.resource-intake-status')?.textContent.includes('Upload paused before finishing')"), false, "a commit already accepted during Pause is reported as completed, not cancelled");
  calls = await evaluate("window.litecoworkHarness.calls()");
  const pendingCommitCalls = calls.filter(call => call.command === "commit_resource_upload");
  assert.equal(pendingCommitCalls.length, 1, "pending commit is issued exactly once");
  assert.equal(pendingCommitCalls[0].args.requestId, commitResumeBeforeResponse.commitRequestId, "the pending commit uses its persisted commit RequestId");
  checks.push("E02-S03 pause during commit waits for the exact committed Resource response and displays that Resource truthfully");

  return checks;
}

async function typeWorkspaceName(text) {
  await waitFor(() => evaluate(`(() => [...document.querySelectorAll("input")].some(input => {
    const label = [...(input.labels ?? [])].map(node => node.textContent).join(" ");
    return /workspace.*name|name.*workspace/i.test(label + " " + (input.getAttribute("aria-label") ?? ""));
  }))()`), "accessible Workspace name input");
  await evaluate(`(() => {
    const input = [...document.querySelectorAll("input")].find(candidate => {
      const label = [...(candidate.labels ?? [])].map(node => node.textContent).join(" ");
      return /workspace.*name|name.*workspace/i.test(label + " " + (candidate.getAttribute("aria-label") ?? ""));
    });
    if (!input) return false;
    input.focus();
    input.setSelectionRange(0, input.value.length);
    return true;
  })()`);
  await cdp("Input.insertText", { text });
  await waitFor(() => evaluate(`(() => {
    const input = [...document.querySelectorAll("input")].find(candidate => {
      const label = [...(candidate.labels ?? [])].map(node => node.textContent).join(" ");
      return /workspace.*name|name.*workspace/i.test(label + " " + (candidate.getAttribute("aria-label") ?? ""));
    });
    return input?.value === ${JSON.stringify(text)};
  })()`), "Workspace name value");
}

async function openDirtyResource() {
  await clickHarness("resetResource");
  await waitFor(() => evaluate("document.querySelector('#resource-revision-title')?.textContent === 'A.txt'"), "Resource A editor");
  await waitFor(() => evaluate("[...document.querySelectorAll('button')].some(button => button.textContent.trim() === 'Edit small text file')"), "Resource A text controls");
  await clickButton("Edit small text file");
  await typeTextareaDraft("Draft from Resource A");
}

async function expectNavigationDialog() {
  await waitFor(() => evaluate("Boolean(document.querySelector('.resource-discard-dialog[role=alertdialog][aria-modal=true]'))"), "Resource navigation confirmation");
  const dialog = await evaluate(`(() => {
    const node = document.querySelector(".resource-discard-dialog[role=alertdialog]");
    const title = document.getElementById(node.getAttribute("aria-labelledby"));
    const description = document.getElementById(node.getAttribute("aria-describedby"));
    return {
      role: node.getAttribute("role"),
      modal: node.getAttribute("aria-modal"),
      title: title?.textContent.trim(),
      description: description?.textContent.trim(),
      focused: document.activeElement?.textContent.trim(),
    };
  })()`);
  assert.deepEqual(dialog, {
    role: "alertdialog",
    modal: "true",
    title: "Leave A.txt?",
    description: "This exact local draft belongs to the current Resource. Choose what to do before opening the selected Resource or Workspace.",
    focused: "Stay on current Resource",
  });

  const root = await cdp("DOM.getDocument", { depth: 1 });
  const found = await cdp("DOM.querySelector", {
    nodeId: root.root.nodeId,
    selector: ".resource-discard-dialog[role=alertdialog]",
  });
  assert.ok(found.nodeId, "alert dialog has a DOM node for accessibility inspection");
  const ax = await cdp("Accessibility.getPartialAXTree", { nodeId: found.nodeId, fetchRelatives: true });
  const nodes = ax.nodes.filter(node => !node.ignored);
  const dialogNode = nodes.find(node => node.role?.value === "alertdialog");
  assert.equal(dialogNode?.name?.value, "Leave A.txt?");
  assert.match(dialogNode?.description?.value ?? "", /exact local draft belongs to the current Resource/);
}

async function currentCalls() {
  return evaluate("window.litecoworkHarness.calls()");
}

async function enterConversationPage({ workspaceId = "workspace-a", richMode = "success", availability = "AVAILABLE", messageId = "message-a" } = {}) {
  await clickHarness("clearCalls");
  await clickHarness("configureConversationList", "single");
  await clickHarness("configureConversation", workspaceId, richMode, availability, true, messageId);
  await clickHarness("mountProductionApp", "first-use-conversation");
  await waitFor(() => evaluate(`document.querySelector('.conversation-top-breadcrumbs .crumb-title')?.textContent === ${JSON.stringify(workspaceId === "workspace-a" ? "Conversation A" : "Workspace B conversation")}`), `Conversation page for ${workspaceId}`);
  await waitFor(() => evaluate("document.querySelector('.message-row.agent .conversation-message-body') !== null"), "conversation message");
  await evaluate("document.querySelector('.message-row.agent')?.scrollIntoView({ block: 'center' })");
}

async function waitForRichRequest(count, label = "RichPresentation request") {
  await waitFor(async () => {
    const calls = await currentCalls();
    return calls.filter(call => call.command === "get_rich_presentation").length >= count;
  }, label);
}

async function emitProjection(availability, revision) {
  assert.equal(await evaluate(`window.litecoworkHarness.emitSnapshot(${JSON.stringify(availability)}, ${revision})`), true);
}

async function runResourceChecks() {
  const checks = [];

  await openDirtyResource();
  await clickButton("Select Resource B");
  await expectNavigationDialog();
  await pressKey("Tab", 8);
  assert.equal(await evaluate("document.activeElement.textContent.trim()"), "Discard draft and switch", "Shift+Tab wraps to the dialog's final action");
  await pressKey("Tab");
  assert.equal(await evaluate("document.activeElement.textContent.trim()"), "Stay on current Resource", "Tab wraps to the dialog's first action");
  await pressKey("Escape");
  await waitFor(() => evaluate("!document.querySelector('.resource-discard-dialog[role=alertdialog]')"), "Stay on Resource after Escape");
  assert.equal(await evaluate("document.querySelector('#resource-revision-title')?.textContent"), "A.txt");
  assert.equal(await evaluate("document.querySelector('#resource-revision-text')?.value"), "Draft from Resource A");
  checks.push("A01 Stay preserves dirty draft; alertdialog AX name/description, focus, Tab/Shift+Tab and Escape verified");

  await openDirtyResource();
  await clickHarness("switchWorkspaceB");
  await expectNavigationDialog();
  await clickButton("Stay on current Resource");
  await waitFor(() => evaluate("!document.querySelector('.resource-discard-dialog[role=alertdialog]')"), "Stay during Workspace switch");
  assert.equal(await evaluate("window.litecoworkHarness.activeWorkspace()"), "workspace-a");
  assert.equal(await evaluate("document.querySelector('#resource-revision-text')?.value"), "Draft from Resource A");
  checks.push("A01 Workspace switch stays on the original Workspace with its local draft intact");

  await openDirtyResource();
  await clickHarness("clearCalls");
  await clickButton("Select Resource B");
  await expectNavigationDialog();
  await clickButton("Discard draft and switch");
  await waitFor(() => evaluate("document.querySelector('#resource-revision-title')?.textContent === 'B.txt'"), "Discard and open Resource B");
  let calls = await currentCalls();
  assert.equal(calls.some(call => ["create_resource_revision_upload", "upload_resource_chunk", "commit_resource_revision_upload"].includes(call.command)), false);
  checks.push("A01 Discard switches to Resource B without sending Resource A bytes");

  await openDirtyResource();
  await clickHarness("clearCalls");
  await clickButton("Select Resource B");
  await expectNavigationDialog();
  await clickButton("Save draft and switch");
  await waitFor(() => evaluate("document.querySelector('#resource-revision-title')?.textContent === 'B.txt'"), "Save Resource A draft and switch");
  calls = await currentCalls();
  const create = calls.find(call => call.command === "create_resource_revision_upload");
  const chunk = calls.find(call => call.command === "upload_resource_chunk");
  const commit = calls.find(call => call.command === "commit_resource_revision_upload");
  assert.ok(create && chunk && commit, "saving a text draft must create, send, and commit a revision");
  assert.equal(calls.filter(call => call.command === "upload_resource_chunk").length, 1, "one negotiated chunk must advance the upload to commit");
  assert.equal(create.args.workspaceId, "workspace-a");
  assert.equal(create.args.resourceId, "resource-a");
  assert.equal(chunk.args.workspaceId, "workspace-a");
  assert.equal(chunk.args.uploadId, "upload-a");
  assert.equal(new TextDecoder().decode(Uint8Array.from(atob(chunk.args.contentBase64), character => character.charCodeAt(0))), "Draft from Resource A");
  assert.equal(commit.args.workspaceId, "workspace-a");
  assert.equal(calls.some(call => call.args?.resourceId === "resource-b" && call.command.includes("resource_revision_upload")), false);
  checks.push("A01 Save commits the exact A draft against A before switching to B");

  await clickHarness("resetResource", "detail");
  await waitFor(() => evaluate("window.litecoworkHarness.resourcePendingCount('detail', 'resource-a') === 1"), "deferred Resource A detail response");
  assert.equal(await evaluate("document.querySelector('[role=status]')?.textContent.includes('Loading Resource metadata')"), true, "initial detail request exposes the loading state");
  await clickHarness("selectResourceB");
  await waitFor(() => evaluate("window.litecoworkHarness.resourcePendingCount('detail', 'resource-b') === 1"), "deferred Resource B detail response");
  await clickHarness("resolveResource", "detail", "resource-b");
  await waitFor(() => evaluate("document.querySelector('.resource-revision-meta')?.textContent.includes('revision-b1')"), "Resource B revision metadata");
  assert.equal(await evaluate("document.querySelector('.resource-revision-history li strong')?.textContent.includes('revision-b1')"), true, "B detail is paired with B history");
  assert.equal(await evaluate("Boolean(document.querySelector('[role=status]')?.textContent.includes('Loading Resource metadata'))"), false, "B detail completion clears metadata loading");
  assert.equal(await evaluate("Boolean(document.querySelector('.resource-revision-panel [role=alert]'))"), false, "the current detail request has no Resource editor error");
  await clickHarness("resolveResource", "detail", "resource-a");
  await new Promise(resolveDelay => setTimeout(resolveDelay, 120));
  assert.equal(await evaluate("document.querySelector('#resource-revision-title')?.textContent"), "B.txt");
  assert.equal(await evaluate("document.querySelector('.resource-revision-meta')?.textContent.includes('revision-b1')"), true, "late successful Resource A detail must not replace B metadata");
  assert.equal(await evaluate("document.querySelector('.resource-revision-history li strong')?.textContent.includes('revision-b1')"), true, "late successful Resource A detail must not replace B history");
  assert.equal(await evaluate("Boolean(document.querySelector('.resource-revision-panel [role=alert]'))"), false, "late detail completion does not add an error to B");
  checks.push("A07 detail: B completes before stale A success; Resource B metadata/history remain selected and loading/error state is clear");

  await clickHarness("resetResource", "history");
  await waitFor(() => evaluate("window.litecoworkHarness.resourcePendingCount('history', 'resource-a') === 1"), "deferred Resource A history response");
  await clickHarness("selectResourceB");
  await waitFor(() => evaluate("window.litecoworkHarness.resourcePendingCount('history', 'resource-b') === 1"), "deferred Resource B history response");
  await clickHarness("resolveResource", "history", "resource-b");
  await waitFor(() => evaluate("document.querySelector('.resource-revision-meta')?.textContent.includes('revision-b1') && document.querySelector('.resource-revision-history li strong')?.textContent.includes('revision-b1')"), "Resource B detail and revision history");
  assert.equal(await evaluate("document.querySelector('.resource-revision-history .section-heading button')?.disabled"), false, "completed B history request clears the busy state");
  await clickHarness("rejectResource", "history", "resource-a");
  await new Promise(resolveDelay => setTimeout(resolveDelay, 120));
  assert.equal(await evaluate("document.querySelector('#resource-revision-title')?.textContent"), "B.txt");
  assert.equal(await evaluate("document.querySelector('.resource-revision-meta')?.textContent.includes('revision-b1')"), true, "stale A history rejection leaves B metadata selected");
  assert.equal(await evaluate("document.querySelector('.resource-revision-history li strong')?.textContent.includes('revision-b1')"), true, "stale A history rejection cannot replace B history");
  assert.equal(await evaluate("Boolean(document.querySelector('.resource-revision-panel [role=alert]'))"), false, "stale A history rejection cannot surface a Resource editor error in B");
  assert.equal(await evaluate("document.querySelector('.resource-revision-history .section-heading button')?.disabled"), false, "stale A history rejection cannot strand B history busy");
  checks.push("A07 history: B completes before stale A rejection; B history stays visible with no leaked error or busy state");

  await clickHarness("resetResource");
  await waitFor(() => evaluate("document.querySelector('#resource-revision-title')?.textContent === 'A.txt' && Boolean(document.querySelector('.resource-revision-meta'))"), "Resource A before head-preview race");
  await clickHarness("setResourceCommitMode", "conflict");
  await clickButton("Edit small text file");
  await typeTextareaDraft("A draft before head preview");
  await clickButton("Save as new revision");
  await waitFor(() => evaluate("Boolean(document.querySelector('.resource-rebase-review'))"), "Resource A conflict review");
  await clickHarness("deferNextResourcePreview", "resource-a");
  await clickButton("Preview current saved text before rebasing");
  await waitFor(() => evaluate("window.litecoworkHarness.resourcePreviewPendingCount('resource-a') === 1"), "deferred Resource A current-head preview");
  assert.equal(await evaluate("[...document.querySelectorAll('.resource-rebase-review button')].find(button => button.textContent.includes('Loading current head'))?.disabled"), true, "A head preview reports its in-flight state");
  await clickButton("Forget resume details");
  await clickHarness("selectResourceB");
  await expectNavigationDialog();
  await clickButton("Discard draft and switch");
  await waitFor(() => evaluate("document.querySelector('#resource-revision-title')?.textContent === 'B.txt' && document.querySelector('.resource-revision-meta')?.textContent.includes('revision-b1')"), "Resource B after discarding A draft");
  await clickButton("Edit small text file");
  await typeTextareaDraft("B draft before head preview", "Saved B source text");
  await clickButton("Save as new revision");
  await waitFor(() => evaluate("Boolean(document.querySelector('.resource-rebase-review'))"), "Resource B conflict review");
  await clickButton("Forget resume details");
  await clickHarness("deferNextResourcePreview", "resource-b");
  await clickButton("Preview current saved text before rebasing");
  await waitFor(() => evaluate("window.litecoworkHarness.resourcePreviewPendingCount('resource-b') === 1"), "deferred Resource B current-head preview");
  await clickHarness("rejectResourcePreview", "resource-a");
  await new Promise(resolveDelay => setTimeout(resolveDelay, 120));
  assert.equal(await evaluate("document.querySelector('#resource-revision-title')?.textContent"), "B.txt", "late A preview failure leaves Resource B selected");
  assert.equal(await evaluate("document.querySelector('.resource-revision-meta')?.textContent.includes('revision-b1')"), true, "late A preview failure leaves B's current revision visible");
  assert.equal(await evaluate("[...document.querySelectorAll('.resource-rebase-review button')].find(button => button.textContent.includes('Loading current head'))?.disabled"), true, "stale A preview cleanup cannot clear B's in-flight indicator");
  assert.equal(await evaluate("Boolean(document.querySelector('.resource-rebase-review [role=alert]'))"), false, "stale A preview rejection cannot render an error in B");
  await clickHarness("resolveResourcePreview", "resource-b");
  await waitFor(() => evaluate("document.querySelector('.resource-rebase-current-text')?.textContent === 'Saved B source text'"), "Resource B saved head preview");
  assert.equal(await evaluate("document.querySelector('.resource-revision-meta')?.textContent.includes('revision-b1')"), true, "B preview is pinned to the visible B revision");
  assert.equal(await evaluate("[...document.querySelectorAll('.resource-rebase-review button')].some(button => button.textContent.includes('Loading current head'))"), false, "B preview completion clears its busy label");
  assert.equal(await evaluate("Boolean(document.querySelector('.resource-rebase-review [role=alert]'))"), false, "B preview success has no stale error");
  checks.push("A07 head preview: stale A rejection cannot leak text/error or clear B busy; B preview resolves against B's selected head");

  await clickHarness("resetResource");
  await waitFor(() => evaluate("document.querySelector('#resource-revision-title')?.textContent === 'A.txt'"), "Resource editor reset after A07 races");

  return checks;
}

async function runContextDocumentDeletionChecks() {
  const checks = [];
  await clickHarness("resetResource");
  await waitFor(() => evaluate("document.querySelector('#resource-revision-title')?.textContent === 'A.txt'"), "Resource A before deletion owner-scope check");
  await clickHarness("setResourceContextDocumentOwner", "COWORKER_NOTES", "workspace-a");
  await waitFor(() => evaluate("document.querySelector('#resource-revision-title')?.textContent === 'A.txt' && !document.querySelector('.resource-context-deletion')"), "unsupported Coworker Notes have no Workspace deletion action");
  assert.equal(await evaluate("[...document.querySelectorAll('button')].some(button => button.textContent.trim() === 'Delete local content…')"), false, "a Coworker-owned ContextDocument has no unsupported Workspace deletion affordance");
  await clickHarness("setResourceContextDocumentOwner", "WORKSPACE_NOTES", "workspace-a");
  await waitFor(() => evaluate("document.querySelector('#resource-revision-title')?.textContent === 'A.txt' && Boolean(document.querySelector('.resource-context-deletion'))"), "Resource A deletion controls");
  checks.push("E02-S03 owner scope: Workspace Notes owned by the selected Workspace show local deletion; Coworker Notes do not");
  await clickHarness("clearCalls");

  await clickButton("Delete local content…");
  await waitFor(() => evaluate("Boolean(document.querySelector('.resource-discard-dialog[role=alertdialog][aria-modal=true]'))"), "ContextDocument deletion confirmation");
  const dialog = await evaluate(`(() => {
    const node = document.querySelector(".resource-discard-dialog[role=alertdialog]");
    const title = document.getElementById(node.getAttribute("aria-labelledby"));
    const description = document.getElementById(node.getAttribute("aria-describedby"));
    return {
      role: node.getAttribute("role"),
      modal: node.getAttribute("aria-modal"),
      title: title?.textContent.trim(),
      focused: document.activeElement?.textContent.trim(),
      destructive: description?.textContent.includes("permanently remove this Resource’s content bytes"),
      uploads: description?.textContent.includes("in-progress revision uploads for this Resource will expire"),
    };
  })()`);
  assert.deepEqual(dialog, {
    role: "alertdialog",
    modal: "true",
    title: "Permanently delete local content?",
    focused: "Permanently delete local content",
    destructive: true,
    uploads: true,
  });
  await pressKey("Tab");
  assert.equal(await evaluate("document.activeElement.textContent.trim()"), "Cancel", "Tab from the final dialog control wraps to Cancel");
  await pressKey("Tab", 8);
  assert.equal(await evaluate("document.activeElement.textContent.trim()"), "Permanently delete local content", "Shift+Tab wraps to the destructive action");
  await pressKey("Escape");
  await waitFor(() => evaluate("!document.querySelector('.resource-discard-dialog[role=alertdialog]')"), "deletion confirmation cancelled with Escape");
  assert.equal(await evaluate("document.activeElement.textContent.trim()"), "Delete local content…", "cancelling restores focus to the deletion trigger");
  calls = await currentCalls();
  assert.equal(calls.some(call => call.command === "begin_context_document_deletion" || call.command === "get_context_document_deletion_status"), false, "cancellation sends no deletion mutation or status request");
  checks.push("E02-S03 cancel: accessible alertdialog, initial focus, Tab/Shift+Tab trap, Escape cancellation, focus restoration, and no mutation verified");

  await clickButton("Delete local content…");
  await waitFor(() => evaluate("Boolean(document.querySelector('.resource-discard-dialog[role=alertdialog][aria-modal=true]'))"), "reopened ContextDocument deletion confirmation");
  await waitFor(() => evaluate("document.activeElement?.textContent.trim() === 'Permanently delete local content'"), "deletion confirmation keyboard focus");
  await clickButton("Permanently delete local content", ".resource-discard-dialog");
  await waitFor(() => evaluate("window.litecoworkHarness.calls().filter(call => call.command === 'begin_context_document_deletion').length === 1"), "confirmed deletion admission request");
  await waitFor(() => evaluate("window.litecoworkHarness.calls().filter(call => call.command === 'get_context_document_deletion_status').length === 1"), "durable status check after ambiguous deletion response");
  await waitFor(() => evaluate("document.querySelector('.resource-context-deletion [role=alert]')?.textContent.includes('lost the deletion acknowledgement') && [...document.querySelectorAll('.resource-context-deletion button')].some(button => button.textContent.trim() === 'Retry the same deletion request')"), "ambiguous deletion retry control");
  calls = await currentCalls();
  const firstRequest = calls.find(call => call.command === "begin_context_document_deletion");
  assert.ok(firstRequest, "explicit confirmation sends one deletion admission request");
  assert.equal(firstRequest.args.workspaceId, "workspace-a");
  assert.equal(firstRequest.args.resourceId, "resource-a");
  assert.equal(firstRequest.args.expectedVersion, 1);
  assert.match(firstRequest.args.requestId, /^[0-9a-f-]{36}$/i, "the request has a UUID idempotency identity");
  assert.equal(await evaluate("document.querySelector('.resource-revision-meta')?.textContent.includes('Context statusACTIVE')"), true, "an ambiguous response with ACTIVE status does not claim deletion was admitted");

  await clickButton("Retry the same deletion request", ".resource-context-deletion");
  await waitFor(() => evaluate("window.litecoworkHarness.calls().filter(call => call.command === 'begin_context_document_deletion').length === 2"), "same-identity deletion retry");
  calls = await currentCalls();
  const retries = calls.filter(call => call.command === "begin_context_document_deletion");
  assert.equal(retries[1].args.requestId, firstRequest.args.requestId, "retry must reuse the exact idempotency request ID");
  assert.equal(retries[1].args.expectedVersion, firstRequest.args.expectedVersion, "retry remains pinned to the exact Resource version");
  assert.equal(retries[1].args.resourceId, firstRequest.args.resourceId);
  assert.equal(retries[1].args.workspaceId, firstRequest.args.workspaceId);
  await waitFor(() => evaluate("document.querySelector('.resource-context-deletion [role=status]')?.textContent.includes('0 of 2 deletion targets are acknowledged')"), "admitted deletion pending state");
  await waitFor(() => evaluate("document.querySelector('.resource-revision-meta')?.textContent.includes('Resource version2')"), "fresh Resource version after deletion admission");
  await waitFor(() => evaluate("document.querySelector('.resource-context-deletion [role=status]')?.textContent.includes('1 of 2 deletion targets are acknowledged')"), "polled deletion progress");
  await waitFor(() => evaluate("document.querySelector('.resource-revision-meta')?.textContent.includes('Context statusDELETED') && document.querySelector('.resource-context-deletion [role=status]')?.textContent.includes('Deletion is complete')"), "verified deletion completion");
  calls = await currentCalls();
  const statusReads = calls.filter(call => call.command === "get_context_document_deletion_status");
  assert.equal(statusReads.length, 3, "the UI checks the ambiguous outcome, then polls pending and terminal status");
  assert.ok(statusReads.every(call => call.args.workspaceId === "workspace-a" && call.args.resourceId === "resource-a"), "all status checks remain scoped to Resource A");
  assert.equal(calls.filter(call => call.command === "begin_context_document_deletion").length, 2, "polling does not issue another deletion mutation");
  checks.push("E02-S03 confirm/retry: explicit confirmation; ambiguous failure retains the same request ID/version; polling reports pending progress then only reports DELETED after the terminal receipt");

  await clickHarness("resetResource");
  await waitFor(() => evaluate("document.querySelector('#resource-revision-title')?.textContent === 'A.txt' && document.querySelector('.resource-revision-meta')?.textContent.includes('Context statusACTIVE')"), "Resource A before lost-ack status recovery");
  await clickHarness("setContextDocumentDeletionFirstStatus", "DELETION_PENDING");
  await clickHarness("failNextContextDocumentDeletionDetailRefresh");
  await clickHarness("clearCalls");
  await clickButton("Delete local content…");
  await waitFor(() => evaluate("Boolean(document.querySelector('.resource-discard-dialog[role=alertdialog][aria-modal=true]'))"), "second ContextDocument deletion confirmation");
  await clickButton("Permanently delete local content", ".resource-discard-dialog");
  await waitFor(() => evaluate("window.litecoworkHarness.calls().filter(call => call.command === 'begin_context_document_deletion').length === 1"), "second confirmed deletion request");
  await waitFor(() => evaluate("window.litecoworkHarness.calls().filter(call => call.command === 'get_context_document_deletion_status').length === 1"), "lost acknowledgement reconciled by pending status");
  await waitFor(() => evaluate("window.litecoworkHarness.calls().some(call => call.command === 'get_resource_detail')"), "first Resource detail refresh after recovered deletion status");
  assert.equal(await evaluate("document.querySelector('.resource-revision-meta')?.textContent.includes('Resource version1')"), true, "a failed detail refresh cannot claim that the Resource version was updated");
  await waitFor(() => evaluate("window.litecoworkHarness.calls().filter(call => call.command === 'get_context_document_deletion_status').length >= 2 && window.litecoworkHarness.calls().filter(call => call.command === 'get_resource_detail').length >= 2 && document.querySelector('.resource-revision-meta')?.textContent.includes('Resource version2')"), "same-status poll retries Resource detail refresh after the first refresh fails");
  await waitFor(() => evaluate("document.querySelector('.resource-revision-meta')?.textContent.includes('Resource version2')"), "Resource version refreshed after pending status recovery");
  assert.equal(await evaluate("document.querySelector('.resource-revision-meta')?.textContent.includes('Context statusDELETION_PENDING')"), true, "recovered status and refreshed Resource version describe the same committed tombstone");
  assert.equal(await evaluate("[...document.querySelectorAll('.resource-context-deletion button')].some(button => button.textContent.trim() === 'Retry the same deletion request')"), false, "a confirmed pending plan no longer offers a redundant POST retry");
  await waitFor(() => evaluate("document.querySelector('.resource-revision-meta')?.textContent.includes('Context statusDELETED')"), "recovered deletion reaches terminal status");
  calls = await currentCalls();
  assert.equal(calls.filter(call => call.command === "begin_context_document_deletion").length, 1, "confirmed pending recovery does not submit a duplicate mutation");
  checks.push("E02-S03 lost acknowledgement recovery: a pending status GET refreshes the tombstoned Resource version, suppresses duplicate POST, and polls to verified DELETED");

  await clickHarness("resetResource");
  await waitFor(() => evaluate("document.querySelector('#resource-revision-title')?.textContent === 'A.txt' && document.querySelector('.resource-revision-meta')?.textContent.includes('Context statusACTIVE')"), "Resource A before terminal detail-refresh recovery");
  await clickHarness("setContextDocumentDeletionFirstStatus", "DELETION_PENDING");
  await clickHarness("failNextContextDocumentDeletionDetailRefresh", 3);
  await clickHarness("clearCalls");
  await clickButton("Delete local content…");
  await waitFor(() => evaluate("Boolean(document.querySelector('.resource-discard-dialog[role=alertdialog][aria-modal=true]'))"), "terminal refresh deletion confirmation");
  await clickButton("Permanently delete local content", ".resource-discard-dialog");
  await waitFor(() => evaluate("window.litecoworkHarness.calls().filter(call => call.command === 'begin_context_document_deletion').length === 1"), "terminal refresh deletion admission");
  await waitFor(() => evaluate("document.querySelector('.resource-context-deletion [role=status]')?.textContent.includes('Deletion is complete') && [...document.querySelectorAll('.resource-context-deletion button')].some(button => button.textContent.trim() === 'Retry Resource detail refresh')"), "manual Resource detail refresh after terminal status refresh failures");
  assert.equal(await evaluate("document.querySelector('.resource-revision-meta')?.textContent.includes('Resource version1')"), true, "terminal deletion does not present a stale Resource version as refreshed");
  let terminalRecoveryCalls = await currentCalls();
  assert.equal(terminalRecoveryCalls.filter(call => call.command === "get_context_document_deletion_status").length, 3, "the terminal status stops automatic polling after its detail refresh fails");
  assert.equal(terminalRecoveryCalls.filter(call => call.command === "get_resource_detail").length, 3, "the first pending, repeated pending, and terminal detail reads each fail once");
  await new Promise(resolveDelay => setTimeout(resolveDelay, 1700));
  terminalRecoveryCalls = await currentCalls();
  assert.equal(terminalRecoveryCalls.filter(call => call.command === "get_context_document_deletion_status").length, 3, "a confirmed terminal receipt does not poll forever");
  await clickButton("Retry Resource detail refresh", ".resource-context-deletion");
  await waitFor(() => evaluate("document.querySelector('.resource-revision-meta')?.textContent.includes('Resource version2') && document.querySelector('.resource-revision-meta')?.textContent.includes('Context statusDELETED')"), "owner-triggered Resource detail refresh after terminal failure");
  terminalRecoveryCalls = await currentCalls();
  assert.equal(terminalRecoveryCalls.filter(call => call.command === "get_context_document_deletion_status").length, 3, "manual detail recovery needs no extra deletion-status mutation");
  checks.push("E02-S03 terminal detail recovery: repeated failed reads stop at confirmed DELETED, disclose the stale version, and offer an explicit Resource-only retry");

  await clickHarness("resetResource");
  await waitFor(() => evaluate("document.querySelector('#resource-revision-title')?.textContent === 'A.txt' && document.querySelector('.resource-revision-meta')?.textContent.includes('Context statusACTIVE')"), "Resource A before stale terminal detail response");
  await clickHarness("returnStaleContextDocumentDeletionDetailStatus");
  await clickHarness("clearCalls");
  await clickButton("Delete local content…");
  await waitFor(() => evaluate("Boolean(document.querySelector('.resource-discard-dialog[role=alertdialog][aria-modal=true]'))"), "stale terminal response deletion confirmation");
  await clickButton("Permanently delete local content", ".resource-discard-dialog");
  await waitFor(() => evaluate("window.litecoworkHarness.calls().filter(call => call.command === 'get_context_document_deletion_status').length === 1"), "ambiguous admission reconciled before stale terminal response");
  await clickButton("Retry the same deletion request", ".resource-context-deletion");
  await waitFor(() => evaluate("document.querySelector('.resource-revision-meta')?.textContent.includes('Context statusDELETION_PENDING')"), "pending detail before stale terminal response");
  await waitFor(() => evaluate("document.querySelector('.resource-context-deletion [role=status]')?.textContent.includes('Deletion is complete') && [...document.querySelectorAll('.resource-context-deletion button')].some(button => button.textContent.trim() === 'Retry Resource detail refresh')"), "stale matching-Resource detail is rejected after terminal confirmation");
  assert.equal(await evaluate("document.querySelector('.resource-revision-meta')?.textContent.includes('Resource version2')"), true, "a stale matching-ID detail response preserves the last successful Resource version while refresh remains pending");
  await clickButton("Retry Resource detail refresh", ".resource-context-deletion");
  await waitFor(() => evaluate("document.querySelector('.resource-revision-meta')?.textContent.includes('Resource version2') && document.querySelector('.resource-revision-meta')?.textContent.includes('Context statusDELETED')"), "fresh terminal Resource detail after stale response");
  assert.equal(await evaluate("[...document.querySelectorAll('.resource-context-deletion button')].some(button => button.textContent.trim() === 'Retry Resource detail refresh')"), false, "only a detail response with the confirmed terminal status clears the refresh requirement");
  checks.push("E02-S03 stale detail recovery: a matching-ID but older ContextDocument status cannot clear the terminal refresh requirement; an explicit retry applies current metadata");
  return checks;
}

async function runConversationChecks() {
  const checks = [];

  await clickHarness("configureConversation", "workspace-a", "success", "UNAVAILABLE");
  await waitFor(() => evaluate("document.querySelector('.conversation-empty-state') !== null"), "current empty Conversation presentation");
  assert.equal(await evaluate("document.querySelector('.conversation-empty-state')?.textContent.includes('native agent you select')"), true, "the empty state explains that the connected native agent owns model behavior");
  assert.equal(await evaluate("document.querySelector('.conversation-empty-state')?.textContent.includes('does not create a Task')"), true, "ordinary Conversation remains distinct from Task creation");
  assert.equal(await evaluate("document.querySelector('.composer-textarea')?.getAttribute('aria-label')"), "Message draft", "the current message composer has an accessible name");
  await typeInputValue(".composer-textarea", "Summarize the budget in this project");
  assert.equal(await evaluate("document.querySelector('.composer-textarea')?.value"), "Summarize the budget in this project", "the current composer retains an unsent draft");
  assert.equal((await currentCalls()).some(call => call.command === "create_conversation" || call.command === "create_conversation_turn"), false, "typing does not persist a Conversation or send a turn");
  checks.push("current empty Conversation identifies native-agent ownership, keeps Task separate, and holds a draft without creating or sending");

  await clickHarness("configureConversation", "workspace-a", "success", "UNAVAILABLE", false);
  await waitFor(() => evaluate("document.querySelector('.composer-notice[role=status]')?.textContent.includes('Fresh sends and retries are paused')"), "Runtime-unready Conversation state");
  assert.equal(await evaluate("document.querySelector('.composer-submit-btn')?.disabled"), true, "a fresh send is disabled while the current Runtime is not ready");
  checks.push("Runtime-unready Conversation keeps fresh send visibly paused");

  await clickHarness("configureConversationList", "single");
  await clickHarness("configureConversation", "workspace-a", "success", "UNAVAILABLE", true, "message-a", true);
  await clickHarness("mountProductionApp", "first-use-conversation");
  await waitFor(() => evaluate("document.querySelector('.conversation-empty-state') !== null && window.litecoworkHarness.hasActiveConversationStream()"), "empty Conversation presentation stream subscription");
  assert.equal(await evaluate("[...document.querySelectorAll('.conversation-header-badge')].some(node => node.textContent.includes('Connecting saved-history updates'))"), true, "a new presentation stream has a visible connecting state");
  await clickHarness("emitReady");
  await clickHarness("emitEmptyConversationSnapshot", 2);
  await waitFor(() => evaluate("[...document.querySelectorAll('.conversation-header-badge')].some(node => node.textContent.includes('Saved history updates live')) && document.querySelector('.conversation-empty-state') !== null"), "empty durable conversation snapshot with a live stream");
  await clickHarness("emitTurnDelta", "turn-rendered", 1, "Temporary answer <img src=x onerror=alert(1)>.");
  await waitFor(() => evaluate("document.querySelector('.message-row.agent.transient .message-assistant-body')?.textContent.includes('Temporary answer <img')"), "temporary response rendered as semantic text");
  assert.equal(await evaluate("document.querySelector('.message-row.agent.transient img') === null"), true, "untrusted streamed text is rendered as escaped React text, never interpreted markup");
  assert.equal(await evaluate("document.querySelector('.conversation-empty-state') === null"), true, "a streamed response replaces the saved-empty placeholder without claiming it is durable");
  await clickHarness("emitTurnSettled", "turn-rendered", 0);
  await waitFor(() => evaluate("document.querySelector('.message-row.agent.transient .message-assistant-body')?.textContent.includes('Temporary answer <img')"), "an uncommitted settled turn remains transient rather than saved");
  assert.equal(await evaluate("[...document.querySelectorAll('.message-row.agent:not(.transient)')].some(node => node.textContent.includes('Temporary answer <img'))"), false, "an uncommitted response is not presented as a durable assistant message");
  await clickHarness("emitConversationResync");
  await waitFor(() => evaluate("[...document.querySelectorAll('.conversation-header-badge')].some(node => node.textContent.includes('Refreshing saved conversation updates'))"), "presentation cursor resynchronization state");
  assert.equal(await evaluate("document.querySelector('.message-row.agent.transient') === null"), true, "resynchronization clears stale transient text");
  await clickHarness("emitSnapshot", "UNAVAILABLE", 3);
  await waitFor(() => evaluate("document.querySelector('.conversation-message-body')?.textContent.includes('Semantic answer remains available.')"), "durable semantic snapshot after stream resynchronization");
  await clickHarness("failConversationStream");
  await waitFor(() => evaluate("[...document.querySelectorAll('.conversation-header-badge')].some(node => node.textContent.includes('Reconnecting saved-history updates'))"), "stream error reconnect state");
  assert.equal(await evaluate("document.body.innerText.includes('private transport details stay hidden')"), false, "native stream errors do not leak transport details into the UI");
  checks.push("Conversation presentation renders escaped transient text, keeps uncommitted replies out of durable history, and hides transport details during recovery");

  await clickHarness("configureConversationList", "single");
  await clickHarness("mountProductionApp", "first-use-conversation");
  await waitFor(() => evaluate("document.querySelector('.conversation-top-breadcrumbs .crumb-title')?.textContent === 'Conversation A'"), "production App saved Conversation");
  const directDestinations = await evaluate(`(() => {
    const sidebar = document.querySelector('#main-navigation');
    return {
      newChat: Boolean(sidebar?.querySelector('.sidebar-new-chat-pill')),
      recentChats: Boolean(sidebar?.querySelector('.sidebar-recent-chats-wrap')),
      projects: [...(sidebar?.querySelectorAll('button') ?? [])].some(button => button.textContent.includes('Projects')),
      needsYou: [...(sidebar?.querySelectorAll('button') ?? [])].some(button => button.textContent.includes('Needs You')),
      settings: Boolean(sidebar?.querySelector('.sidebar-settings-btn')),
      more: Boolean(sidebar?.querySelector('.sidebar-more summary')),
      inventedCount: Boolean(sidebar?.querySelector('.sidebar-section-count')),
    };
  })()`);
  assert.deepEqual(directDestinations, { newChat: true, recentChats: true, projects: true, needsYou: true, settings: true, more: true, inventedCount: false }, "the compact shell exposes core destinations without invented counts");
  await clickButtonContaining(["projects"]);
  await waitFor(() => evaluate("document.querySelector('.page-content h1')?.textContent === 'Projects'"), "Workspace-backed Projects page");
  assert.equal(await evaluate("document.querySelector('.workspace-project-card h2')?.textContent"), "Workspace A", "Projects reflects the existing Workspace projection");
  await clickButton("Open current Workspace");
  await waitFor(() => evaluate("document.querySelector('.conversation-empty-state') !== null"), "fresh blank chat after opening the current Workspace");
  await clickButtonContaining(["new chat"]);
  await waitFor(() => evaluate("document.querySelector('.conversation-empty-state') !== null"), "explicit New Chat remains blank instead of reselecting saved history");
  let calls = await currentCalls();
  assert.equal(calls.some(call => call.command === "create_conversation" || call.command === "create_conversation_turn"), false, "Projects, opening a Workspace and New Chat do not create or send a Conversation");
  checks.push("A-123 Projects lists an existing Workspace, opens a fresh blank Conversation, and New Chat does not silently reselect history or persist anything");

  await cdp("Emulation.setDeviceMetricsOverride", { width: 1440, height: 1000, deviceScaleFactor: 1, mobile: false });
  await evaluate("(() => { const summary = document.querySelector('.sidebar-more summary'); summary?.scrollIntoView({ block: 'nearest' }); summary?.focus(); return document.activeElement === summary; })()");
  await pressKey("Enter", 0, true);
  await waitFor(() => evaluate("document.querySelector('.sidebar-more')?.open === true"), "keyboard-open More disclosure");
  assert.equal(await evaluate("document.querySelector('.sidebar-more summary')?.matches(':focus-visible')"), true, "More remains visibly focused when opened from the keyboard");
  await clickButtonContaining(["new task"]);
  await waitFor(() => evaluate("document.querySelector('.breadcrumbs strong')?.textContent === 'New task'"), "New Task route through More");
  assert.equal(await evaluate("document.querySelector('.sidebar-more summary')?.textContent.includes('New task')"), true, "More identifies its active secondary destination");
  assert.equal(await evaluate("document.querySelector('.sidebar-more button[aria-current=page]')?.textContent.includes('New task')"), true, "the active secondary route is marked for assistive technology");
  checks.push("A-123 More opens by keyboard and identifies the active secondary route");

  await cdp("Emulation.setDeviceMetricsOverride", { width: 800, height: 900, deviceScaleFactor: 1, mobile: false });
  await waitFor(() => evaluate("window.innerWidth <= 900 && Boolean(document.querySelector('.mobile-navigation-toggle'))"), "narrow shell navigation controls");
  await evaluate("document.querySelector('.mobile-navigation-toggle')?.focus()");
  await pressKey("Space");
  await waitFor(() => evaluate("document.querySelector('.sidebar')?.classList.contains('is-open') === true"), "keyboard-open narrow navigation");
  await waitFor(() => evaluate("document.activeElement?.classList.contains('sidebar-close') === true"), "focus moves into the narrow navigation dialog");
  await pressKey("Escape");
  await waitFor(() => evaluate("document.querySelector('.sidebar')?.classList.contains('is-open') === false"), "Escape closes narrow navigation");
  await waitFor(() => evaluate("document.activeElement?.classList.contains('mobile-navigation-toggle') === true"), "focus returns to the narrow navigation trigger");
  await pressKey("Space");
  await waitFor(() => evaluate("document.querySelector('.sidebar')?.classList.contains('is-open') === true"), "reopen narrow navigation by keyboard");
  await clickButtonContaining(["projects"]);
  await waitFor(() => evaluate("document.querySelector('.page-content h1')?.textContent === 'Projects' && document.querySelector('.sidebar')?.classList.contains('is-open') === false"), "narrow navigation reaches Projects and closes");
  await waitFor(() => evaluate("document.activeElement?.classList.contains('mobile-navigation-toggle') === true"), "focus returns to narrow navigation trigger after selection");
  checks.push("A-123 narrow navigation opens and closes by keyboard, restores focus, and returns focus after route selection");

  await clickHarness("clearCalls");
  await evaluate("document.querySelector('.mobile-navigation-toggle')?.focus()");
  await pressKey("Space");
  await waitFor(() => evaluate("document.querySelector('.sidebar')?.classList.contains('is-open') === true"), "open navigation before Library");
  await clickButtonContaining(["library"]);
  await waitFor(() => evaluate("document.querySelector('.page-content h1')?.textContent === 'Library'"), "Library from compact More navigation");
  await clickButton("History & edit");
  await waitFor(() => evaluate("document.querySelector('#resource-revision-title')?.textContent === 'A.txt'"), "Library Resource editor");
  await clickButton("Edit small text file");
  await typeTextareaDraft("Draft from Resource A");

  await evaluate("document.querySelector('.mobile-navigation-toggle')?.focus()");
  await pressKey("Space");
  await waitFor(() => evaluate("document.querySelector('.sidebar')?.classList.contains('is-open') === true"), "open navigation to retest New Chat guard");
  await waitForNavigationDrawerVisible("visible navigation before reopening Library");
  if (!(await evaluate("document.querySelector('.sidebar-more')?.open === true"))) await clickButtonContaining(["more"]);
  await clickButtonContaining(["library"]);
  await waitFor(() => evaluate("document.querySelector('.page-content h1')?.textContent === 'Library'"), "return to Library for New Chat guard");
  await clickButton("History & edit");
  await waitFor(() => evaluate("document.querySelector('#resource-revision-title')?.textContent === 'A.txt'"), "Library Resource editor after Workspace selection");
  if (!(await evaluate("Boolean(document.querySelector('#resource-revision-text'))"))) await clickButton("Edit small text file");
  await typeTextareaDraft("Draft retained for New Chat", "Draft from Resource A");
  await evaluate("document.querySelector('.mobile-navigation-toggle')?.focus()");
  await pressKey("Space");
  await waitFor(() => evaluate("document.querySelector('.sidebar')?.classList.contains('is-open') === true"), "open navigation while Library draft is dirty");
  await clickButtonContaining(["new chat"]);
  await waitFor(() => evaluate("document.querySelector('.resource-discard-dialog[role=alertdialog] h3')?.textContent === 'Leave the Library?'"), "guard New Chat while Library draft is dirty");
  assert.equal(await evaluate("document.querySelector('.page-content h1')?.textContent"), "Library", "the dirty Library remains the current route until the user chooses");
  assert.equal(await evaluate("document.querySelector('#resource-revision-text')?.value"), "Draft retained for New Chat", "the guarded New Chat preserves the local Resource draft");
  await pressKey("Escape");
  await waitFor(() => evaluate("document.querySelector('.resource-discard-dialog') === null"), "Stay in Library after Escape");
  assert.equal(await evaluate("document.activeElement?.classList.contains('mobile-navigation-toggle') === true"), true, "choosing Stay restores focus to the narrow-menu trigger");
  assert.equal(await evaluate("document.querySelector('#resource-revision-text')?.value"), "Draft retained for New Chat", "choosing Stay preserves the dirty draft");
  await pressKey("Space");
  await waitFor(() => evaluate("document.querySelector('.sidebar')?.classList.contains('is-open') === true"), "reopen navigation after staying in Library");
  await clickButtonContaining(["new chat"]);
  await waitFor(() => evaluate("document.querySelector('.resource-discard-dialog[role=alertdialog] h3')?.textContent === 'Leave the Library?'"), "second guarded New Chat request");
  await clickButton("Discard draft and leave Library");
  await waitFor(() => evaluate("document.querySelector('.conversation-empty-state') !== null"), "New Chat after explicit draft discard");
  assert.equal(await evaluate("document.querySelector('#resource-revision-text') === null"), true, "explicit discard leaves the Library editor");
  calls = await currentCalls();
  assert.equal(calls.some(call => call.command === "create_conversation" || call.command === "create_conversation_turn"), false, "navigation through the dirty-draft guard does not persist or send a Conversation");
  checks.push("A-123 New Chat preserves the dirty Library guard, stays without losing the draft, restores focus, and navigates only after explicit discard");

  const firstUseAppCalls = await currentCalls();
  assert.equal(firstUseAppCalls.some(call => call.command === "create_conversation" || call.command === "create_conversation_turn"), false, "navigation performs no Conversation create or send");
  checks.push("production App navigation does not turn page changes into Conversation creates or sends");

  // Keep the pre-existing Home keyboard-focus and unsent Conversation draft checks
  // alongside the new compact-shell journey; the A-123 coverage must not replace them.
  await cdp("Emulation.setDeviceMetricsOverride", { width: 1440, height: 1000, deviceScaleFactor: 1, mobile: false });
  await clickHarness("configureConversationList", "empty");
  await clickHarness("mountProductionApp", "first-use-conversation");
  await waitFor(() => evaluate("Boolean(document.querySelector('.sidebar-workspace-switcher'))"), "production App shell for empty Conversation");
  await evaluate("document.querySelector('.sidebar-more summary')?.click()");
  await waitFor(() => evaluate("document.querySelector('.sidebar-more')?.open === true"), "More before opening the Home Task composer");
  await clickButtonContaining(["new task"]);
  await waitFor(() => evaluate("document.querySelector('.breadcrumbs strong')?.textContent === 'New task'"), "production Home route through More");
  await waitFor(() => evaluate("Boolean(document.querySelector('#task-draft'))"), "production Home Task composer");
  await evaluate("document.querySelector('#task-draft').focus()");
  await pressKey("Tab");
  await pressKey("Tab", 8);
  const homeComposerFocus = await evaluate(`(() => {
    const composer = document.querySelector('#task-draft');
    const style = composer ? getComputedStyle(composer) : null;
    return {
      focused: document.activeElement === composer,
      focusVisible: composer?.matches(':focus-visible') ?? false,
      outlineStyle: style?.outlineStyle ?? 'none',
      outlineWidth: style?.outlineWidth ?? '0px',
    };
  })()`);
  assert.equal(homeComposerFocus.focused, true, "keyboard navigation returns focus to the Home Task composer");
  assert.equal(homeComposerFocus.focusVisible, true, "the Home Task composer identifies keyboard focus");
  assert.notEqual(homeComposerFocus.outlineStyle, "none", "the Home Task composer shows a visible keyboard focus indicator");
  assert.ok(Number.parseFloat(homeComposerFocus.outlineWidth) >= 2, "the Home Task composer uses a visible focus ring at least 2px wide");
  checks.push("Home Task composer retains a visible semantic focus ring when reached by keyboard");
  await clickButtonContaining(["new chat"]);
  await waitFor(() => evaluate("document.querySelector('.conversation-empty-state') !== null"), "production App empty Conversation");
  await typeInputValue(".composer-textarea", "Explain the project plan in plain language");
  assert.equal(await evaluate("document.querySelector('.conversation-empty-state button') === null"), true, "the ready empty state does not route a user into an unrelated setup action");
  await clickButtonContaining(["settings"]);
  await waitFor(() => evaluate("document.querySelector('.breadcrumbs strong')?.textContent === 'Settings'"), "normal navigation opens Settings");
  assert.equal(await evaluate("document.querySelector('.composer-textarea') === null"), true, "the composer unmounts while Settings is open");
  await clickButtonContaining(["new chat"]);
  await waitFor(() => evaluate("document.querySelector('.composer-textarea')?.value === 'Explain the project plan in plain language'"), "draft restored after Settings navigation");
  const preservedDraftCalls = await currentCalls();
  assert.equal(preservedDraftCalls.some(call => call.command === "create_conversation" || call.command === "create_conversation_turn"), false, "opening and returning from setup performs no Conversation mutation");
  checks.push("production App keeps an unsent question in memory across ordinary Settings navigation and restores it without IPC persistence");

  await clickHarness("configureConversationList", "single");
  await enterConversationPage({ richMode: "defer" });
  await waitForRichRequest(1);
  await waitFor(() => evaluate("document.querySelector('[role=status].conversation-rich-status')?.textContent.includes('Loading formatted response')"), "accessible rich loading status");
  await waitFor(async () => (await currentCalls()).some(call => call.command === "subscribe_conversation_stream"), "Conversation live stream subscription");
  await clickHarness("emitReady");
  await emitProjection("AVAILABLE", 2);
  await new Promise(resolveDelay => setTimeout(resolveDelay, 150));
  calls = await currentCalls();
  assert.equal(calls.filter(call => call.command === "get_rich_presentation").length, 1, "equivalent live snapshot must not start a duplicate fetch");
  await clickHarness("resolveRich");
  await waitFor(() => evaluate("Boolean(document.querySelector('[data-testid=rich-answer]'))"), "RichPresentation after live snapshot");
  assert.equal(await evaluate("document.querySelector('[role=status].conversation-rich-status') === null"), true, "loading status clears after the delayed fetch");
  assert.equal(await evaluate("document.querySelector('[data-testid=rich-answer]')?.textContent"), "Semantic answer remains available.");
  checks.push("A03 delayed RichPresentation fetch reaches READY across an equivalent live snapshot without duplicate requests");

  await evaluate("document.querySelector('.rich-response-semantic-fallback summary')?.focus()");
  await pressKey("Enter");
  if (!(await evaluate("document.querySelector('.rich-response-semantic-fallback')?.open === true"))) {
    await pressKey("Space");
  }
  await waitFor(() => evaluate("document.querySelector('.rich-response-semantic-fallback')?.open === true"), "keyboard access to semantic fallback");
  assert.equal(await evaluate("document.querySelector('[data-testid=semantic-answer]')?.textContent.includes('Semantic answer remains available.')"), true);
  await clickHarness("setRichMode", "defer");
  await evaluate(`Promise.all([
    window.litecoworkHarness.emitSnapshot("POLICY_OMITTED", 3),
    window.litecoworkHarness.emitSnapshot("AVAILABLE", 4),
  ])`);
  await waitForRichRequest(2, "RichPresentation refetch after rapid policy invalidation and re-availability");
  assert.equal(await evaluate("document.querySelector('[data-testid=rich-answer]') === null"), true, "the invalidated cached RichPresentation stays discarded while the same immutable ref is refetched");
  assert.equal(await evaluate("document.querySelector('[data-testid=semantic-answer]')?.textContent.includes('Semantic answer remains available.')"), true);
  await clickHarness("resolveRich");
  await waitFor(() => evaluate("Boolean(document.querySelector('[data-testid=rich-answer]'))"), "RichPresentation restored after availability returns");
  await emitProjection("INTEGRITY_FAILED", 5);
  await waitFor(() => evaluate("!document.querySelector('[data-testid=rich-answer]') && document.querySelector('.conversation-rich-status[role=note]')?.textContent.includes('unavailable')"), "integrity invalidation fallback");
  assert.equal(await evaluate("document.querySelector('[data-testid=semantic-answer]')?.textContent.includes('Semantic answer remains available.')"), true);
  await clickHarness("setRichMode", "success");
  await emitProjection("AVAILABLE", 6);
  await waitForRichRequest(3, "RichPresentation refetch after integrity availability returns");
  await waitFor(() => evaluate("Boolean(document.querySelector('[data-testid=rich-answer]'))"), "RichPresentation restored after integrity availability returns");
  await emitProjection("UNAVAILABLE", 7);
  await waitFor(() => evaluate("!document.querySelector('[data-testid=rich-answer]') && document.querySelector('.conversation-rich-status[role=note]')?.textContent.includes('unavailable')"), "unavailable invalidation fallback");
  assert.equal(await evaluate("document.querySelector('[data-testid=semantic-answer]')?.textContent.includes('Semantic answer remains available.')"), true);
  assert.equal((await currentCalls()).filter(call => call.command === "get_rich_presentation").length, 3, "UNAVAILABLE must evict the cached output without issuing another rich fetch");
  checks.push("A02 POLICY_OMITTED, INTEGRITY_FAILED, and UNAVAILABLE evict rendered rich output while preserving semantic text; disclosure works from keyboard");

  await enterConversationPage({ messageId: "__proto__" });
  await waitForRichRequest(1, "RichPresentation for a message with a prototype-named ID");
  await waitFor(() => evaluate("Boolean(document.querySelector('[data-testid=rich-answer]'))"), "prototype-named message RichPresentation");
  await waitFor(async () => (await currentCalls()).some(call => call.command === "subscribe_conversation_stream"), "prototype-named Conversation stream subscription");
  await clickHarness("emitReady");
  await emitProjection("POLICY_OMITTED", 8);
  await waitFor(() => evaluate("!document.querySelector('[data-testid=rich-answer]')"), "policy invalidation for prototype-named message");
  await emitProjection("AVAILABLE", 9);
  await waitForRichRequest(2, "RichPresentation refetch for prototype-named message");
  await waitFor(() => evaluate("Boolean(document.querySelector('[data-testid=rich-answer]'))"), "prototype-named message RichPresentation after refetch");
  checks.push("A02 invalidation and refetch remain safe for a `__proto__` message ID");

  await enterConversationPage({ richMode: "reject" });
  await waitFor(() => evaluate("document.querySelector('.conversation-rich-status[role=note] button')?.textContent === 'Retry'"), "failed rich fetch retry control");
  assert.equal(await evaluate("document.querySelector('.conversation-rich-status[role=note]')?.textContent.includes('complete text response is still shown')"), true);
  await clickHarness("setRichMode", "defer");
  await clickButton("Retry", ".conversation-rich-status[role=note]");
  await waitForRichRequest(2, "rich retry request");
  await waitFor(() => evaluate("document.querySelector('[role=status].conversation-rich-status')?.textContent.includes('Loading formatted response')"), "rich retry loading status");
  await clickHarness("resolveRich");
  await waitFor(() => evaluate("Boolean(document.querySelector('[data-testid=rich-answer]'))"), "retried RichPresentation");
  checks.push("A03 failed fetch exposes an accessible Retry action and retry transitions from FAILED through LOADING to READY");

  await clickHarness("configureConversationList", "single");
  await clickHarness("mountProductionApp", "first-use-conversation");
  await waitFor(() => evaluate("document.querySelector('.sidebar-recent-chats-wrap')?.textContent.includes('Conversation A')"), "saved Conversation in Recent Chats");
  assert.equal(await evaluate("document.querySelector('.sidebar-chat-item-row[aria-current=true]')?.textContent.includes('Conversation A')"), true, "the selected saved Conversation is marked in the compact sidebar");
  checks.push("the compact sidebar projects and marks the selected saved Conversation");
  return checks;
}

async function runShellReachabilityChecks() {
  const checks = [];
  await clickHarness("configureConversationList", "single");
  await clickHarness("mountProductionApp", "first-use-conversation");
  await waitFor(() => evaluate("Boolean(document.querySelector('#main-navigation .sidebar-new-chat-pill') && document.querySelector('.sidebar-workspace-select'))"), "production compact shell");
  await waitFor(() => evaluate("document.querySelector('.sidebar-recent-chat-list')?.textContent.includes('Conversation A') === true"), "Recent Chats source projection");
  await evaluate(`(() => {
    window.__lcFocusTrace = [];
    document.addEventListener("focusin", event => window.__lcFocusTrace.push({ type: "focusin", target: event.target?.outerHTML?.slice(0, 240) ?? null }));
    document.addEventListener("keydown", event => window.__lcFocusTrace.push({ type: "keydown", key: event.key, trusted: event.isTrusted, target: event.target?.outerHTML?.slice(0, 160) ?? null, moreOpen: document.querySelector(".sidebar-more")?.open ?? null }));
    document.addEventListener("toggle", event => window.__lcFocusTrace.push({ type: "toggle", trusted: event.isTrusted, open: event.target?.open ?? null }), true);
    const nativeFocus = HTMLElement.prototype.focus;
    HTMLElement.prototype.focus = function (...args) {
      const entry = { type: "focus-call", target: this.outerHTML?.slice(0, 240) ?? null, disabled: this.disabled ?? false, inert: this.closest("[inert]") !== null, display: getComputedStyle(this).display, visibility: getComputedStyle(this).visibility, rects: this.getClientRects().length, before: document.activeElement?.outerHTML?.slice(0, 120) ?? null };
      window.__lcFocusTrace.push(entry);
      const result = nativeFocus.apply(this, args);
      entry.after = document.activeElement?.outerHTML?.slice(0, 120) ?? null;
      return result;
    };
    return true;
  })()`);

  const destinations = await evaluate(`(() => {
    const sidebar = document.querySelector('#main-navigation');
    return {
      newChat: Boolean(sidebar?.querySelector('.sidebar-new-chat-pill')),
      recentChats: Boolean(sidebar?.querySelector('.sidebar-recent-chats-wrap')),
      projects: [...(sidebar?.querySelectorAll('button') ?? [])].some(button => button.textContent.includes('Projects')),
      needsYou: [...(sidebar?.querySelectorAll('button') ?? [])].some(button => button.textContent.includes('Needs You')),
      settings: Boolean(sidebar?.querySelector('.sidebar-settings-btn')),
      more: Boolean(sidebar?.querySelector('.sidebar-more summary')),
      inventedCount: Boolean(sidebar?.querySelector('.sidebar-section-count')),
    };
  })()`);
  assert.deepEqual(destinations, { newChat: true, recentChats: true, projects: true, needsYou: true, settings: true, more: true, inventedCount: false }, "the compact shell exposes core destinations without invented counts");

  await cdp("Emulation.setDeviceMetricsOverride", { width: 800, height: 900, deviceScaleFactor: 1, mobile: false });
  const narrowMenuState = await evaluate("(() => { const control = document.querySelector('.conversation-navigation-toggle'); return { innerWidth: window.innerWidth, mediaMatches: matchMedia('(max-width: 900px)').matches, found: Boolean(control), display: control ? getComputedStyle(control).display : null, rects: control?.getClientRects().length ?? 0 }; })()");
  assert.ok(narrowMenuState.innerWidth <= 900 && narrowMenuState.mediaMatches && narrowMenuState.found && narrowMenuState.display !== "none" && narrowMenuState.rects > 0, `narrow Conversation navigation control is visible: ${JSON.stringify(narrowMenuState)}`);
  await clickHarness("clearCalls");
  await openNavigationByPointer();
  await waitFor(() => evaluate("document.querySelector('.sidebar')?.classList.contains('is-open') === true"), "open narrow navigation from Conversation");
  await waitFor(() => evaluate("document.activeElement?.classList.contains('sidebar-close') === true"), "Conversation navigation focus moves into the drawer");
  await clickButtonContaining(["projects"]);
  await waitFor(() => evaluate("document.querySelector('.page-content h1')?.textContent === 'Projects'"), "Workspace-backed Projects page");
  await waitFor(() => evaluate("document.querySelector('.sidebar')?.classList.contains('is-open') === false"), "close navigation after opening Projects");
  assert.equal(await evaluate("document.querySelector('.workspace-project-card h2')?.textContent"), "Workspace A", "Projects reflects the existing Workspace projection");
  await clickButton("Open current Workspace");
  await waitFor(() => evaluate("document.querySelector('.conversation-empty-state') !== null"), "fresh blank chat after opening the current Workspace");
  await openNavigationByPointer();
  await waitFor(() => evaluate("document.querySelector('.sidebar')?.classList.contains('is-open') === true"), "open narrow navigation from the fresh Conversation");
  await waitForNavigationDrawerVisible("visible narrow navigation from the fresh Conversation");
  await clickButtonContaining(["new chat"]);
  await waitFor(() => evaluate("document.querySelector('.conversation-empty-state') !== null"), "explicit New Chat remains blank instead of reselecting saved history");
  await waitFor(() => evaluate("document.querySelector('.sidebar')?.classList.contains('is-open') === false"), "New Chat closes narrow navigation");
  await waitFor(() => evaluate("document.activeElement?.classList.contains('conversation-navigation-toggle') === true"), "New Chat returns focus to the Conversation menu");
  assert.equal(await evaluate("window.litecoworkHarness.activeWorkspace()"), "workspace-a", "opening the current Workspace preserves its existing identity");
  let calls = await currentCalls();
  assert.equal(calls.some(call => call.command === "create_conversation" || call.command === "create_conversation_turn" || call.command === "create_task" || call.command === "start_runtime"), false, "Projects, opening a Workspace and New Chat do not create or start durable work");
  checks.push("A-123 Projects lists the existing Workspace, opens a fresh blank Conversation, and New Chat causes no Conversation, Task or Runtime creation");

  await openNavigationByPointer();
  await waitFor(() => evaluate("document.querySelector('.sidebar')?.classList.contains('is-open') === true"), "reopen narrow navigation for More");
  await waitForNavigationDrawerVisible("visible navigation before focusing More");
  await evaluate("(() => { const summary = document.querySelector('.sidebar-more summary'); summary?.scrollIntoView({ block: 'nearest' }); summary?.focus(); return document.activeElement === summary; })()");
  assert.equal(await evaluate("document.activeElement === document.querySelector('.sidebar-more summary')"), true, "More summary receives keyboard focus while the drawer is open");
  assert.equal(await evaluate("document.querySelector('.sidebar-more')?.open"), false, "More starts collapsed before keyboard activation");
  await pressKey("Enter", 0, true);
  await waitFor(() => evaluate("document.querySelector('.sidebar-more')?.open === true"), "keyboard-open More disclosure after Enter");
  assert.equal(await evaluate("document.querySelector('.sidebar-more summary')?.matches(':focus-visible')"), true, "More remains visibly focused when opened from the keyboard");
  await clickButtonContaining(["new task"]);
  await waitFor(() => evaluate("document.querySelector('.breadcrumbs strong')?.textContent === 'New task'"), "New Task route through More");
  await waitFor(() => evaluate("document.querySelector('.sidebar')?.classList.contains('is-open') === false"), "close narrow navigation after secondary route selection");
  assert.equal(await evaluate("document.querySelector('.sidebar-more summary')?.textContent.includes('New task')"), true, "More identifies its active secondary destination");
  assert.equal(await evaluate("document.querySelector('.sidebar-more button[aria-current=page]')?.textContent.includes('New task')"), true, "the active secondary route is marked for assistive technology");
  checks.push("More opens by keyboard and exposes the active secondary route");

  await waitFor(() => evaluate("window.innerWidth <= 900 && Boolean(document.querySelector('.mobile-navigation-toggle'))"), "narrow shell navigation controls");
  await openNavigationByPointer();
  await waitFor(() => evaluate("document.querySelector('.sidebar')?.classList.contains('is-open') === true"), "open narrow navigation from the page menu control");
  await waitFor(() => evaluate("document.activeElement?.classList.contains('sidebar-close') === true"), "focus moves into the narrow navigation dialog");
  await pressKey("Escape");
  await waitFor(() => evaluate("document.querySelector('.sidebar')?.classList.contains('is-open') === false"), "Escape closes narrow navigation");
  await waitFor(() => evaluate("document.activeElement?.classList.contains('mobile-navigation-toggle') === true"), "focus returns to the narrow navigation trigger");
  await openNavigationByPointer();
  await waitFor(() => evaluate("document.querySelector('.sidebar')?.classList.contains('is-open') === true"), "reopen narrow navigation");
  await waitForNavigationDrawerVisible("visible navigation before Projects selection");
  await clickButtonContaining(["projects"]);
  await waitFor(() => evaluate("document.querySelector('.page-content h1')?.textContent === 'Projects' && document.querySelector('.sidebar')?.classList.contains('is-open') === false"), "narrow navigation reaches Projects and closes");
  await waitFor(() => evaluate("document.activeElement?.classList.contains('mobile-navigation-toggle') === true"), "focus returns after narrow route selection");
  checks.push("narrow navigation opens from its visible menu control, closes by keyboard Escape, restores focus, and returns focus after route selection");

  await openNavigationByPointer();
  await waitFor(() => evaluate("document.querySelector('.sidebar')?.classList.contains('is-open') === true"), "open navigation before Library");
  await waitForNavigationDrawerVisible("visible navigation before Library selection");
  await clickButtonContaining(["library"]);
  await waitFor(() => evaluate("document.querySelector('.page-content h1')?.textContent === 'Library'"), "Library from compact More navigation");
  await clickButton("History & edit");
  await waitFor(() => evaluate("document.querySelector('#resource-revision-title')?.textContent === 'A.txt'"), "Library Resource editor");
  await clickButton("Edit small text file");
  await typeTextareaDraft("Draft from Resource A");
  await clickHarness("clearCalls");
  await openNavigationByPointer();
  await waitFor(() => evaluate("document.querySelector('.sidebar')?.classList.contains('is-open') === true"), "open navigation before guarded Projects selection");
  await waitForNavigationDrawerVisible("visible navigation before guarded Projects selection");
  await clickButtonContaining(["projects"]);
  await waitFor(() => evaluate("document.querySelector('.resource-discard-dialog[role=alertdialog] h3')?.textContent === 'Leave the Library?'"), "guard Projects while Library draft is dirty");
  assert.equal(await evaluate("document.querySelector('.page-content h1')?.textContent"), "Library", "guarded Projects leaves the dirty Library current");
  assert.equal(await evaluate("document.querySelector('#resource-revision-text')?.value"), "Draft from Resource A", "guarded Projects preserves the local Resource draft");
  await pressKey("Escape");
  await waitFor(() => evaluate("document.querySelector('.resource-discard-dialog') === null"), "Stay in Library after guarded Projects request");
  assert.equal(await evaluate("document.activeElement?.classList.contains('mobile-navigation-toggle') === true"), true, "choosing Stay restores focus before Projects selection");
  await openNavigationByPointer();
  await waitFor(() => evaluate("document.querySelector('.sidebar')?.classList.contains('is-open') === true"), "reopen navigation after staying before Projects");
  await waitForNavigationDrawerVisible("visible navigation after staying before Projects");
  await clickButtonContaining(["projects"]);
  await waitFor(() => evaluate("document.querySelector('.resource-discard-dialog[role=alertdialog] h3')?.textContent === 'Leave the Library?'"), "second guarded Projects request");
  await clickButton("Discard draft and leave Library");
  await waitFor(() => evaluate("document.querySelector('.page-content h1')?.textContent === 'Projects'"), "Projects after explicit Library draft discard");
  await clickButton("Open current Workspace");
  await waitFor(() => evaluate("document.querySelector('.conversation-empty-state') !== null"), "fresh blank Conversation after guarded Workspace selection");
  await waitFor(() => evaluate("document.querySelector('.sidebar')?.classList.contains('is-open') === false"), "close navigation after guarded Workspace selection");
  const guardedWorkspaceCalls = await currentCalls();
  assert.deepEqual(
    guardedWorkspaceCalls.filter(call => /^(create_|start_|send_|submit_|approve_|grant_|invoke_|connect_)/.test(call.command)),
    [],
    "the guarded Workspace selection performs no durable write, Runtime/provider start, grant, approval or invocation",
  );

  await openNavigationByPointer();
  await waitFor(() => evaluate("document.querySelector('.sidebar')?.classList.contains('is-open') === true"), "open navigation to retest New Chat guard");
  await waitForNavigationDrawerVisible("visible navigation before reopening Library");
  if (!(await evaluate("document.querySelector('.sidebar-more')?.open === true"))) await clickButtonContaining(["more"]);
  await clickButtonContaining(["library"]);
  await waitFor(() => evaluate("document.querySelector('.page-content h1')?.textContent === 'Library'"), "return to Library for New Chat guard");
  await clickButton("History & edit");
  await waitFor(() => evaluate("document.querySelector('#resource-revision-title')?.textContent === 'A.txt'"), "Library Resource editor after Workspace selection");
  await clickButton("Edit small text file");
  await typeTextareaDraft("Draft retained for New Chat");
  await openNavigationByPointer();
  await waitFor(() => evaluate("document.querySelector('.sidebar')?.classList.contains('is-open') === true"), "open navigation while Library draft is dirty");
  await waitForNavigationDrawerVisible("visible navigation while Library draft is dirty");
  await clickButtonContaining(["new chat"]);
  await waitFor(() => evaluate("document.querySelector('.resource-discard-dialog[role=alertdialog] h3')?.textContent === 'Leave the Library?'"), "guard New Chat while Library draft is dirty");
  assert.equal(await evaluate("document.querySelector('.page-content h1')?.textContent"), "Library", "the dirty Library remains current until the user chooses");
  assert.equal(await evaluate("document.querySelector('#resource-revision-text')?.value"), "Draft retained for New Chat", "the guarded New Chat preserves the local Resource draft");
  await pressKey("Escape");
  await waitFor(() => evaluate("document.querySelector('.resource-discard-dialog') === null"), "Stay in Library after Escape");
  assert.equal(await evaluate("document.activeElement?.classList.contains('mobile-navigation-toggle') === true"), true, "choosing Stay restores focus to the narrow-menu trigger");
  assert.equal(await evaluate("document.querySelector('#resource-revision-text')?.value"), "Draft retained for New Chat", "choosing Stay preserves the dirty draft");
  await openNavigationByPointer();
  await waitFor(() => evaluate("document.querySelector('.sidebar')?.classList.contains('is-open') === true"), "reopen navigation after staying in Library");
  await waitForNavigationDrawerVisible("visible navigation after staying in Library");
  await clickButtonContaining(["new chat"]);
  await waitFor(() => evaluate("document.querySelector('.resource-discard-dialog[role=alertdialog] h3')?.textContent === 'Leave the Library?'"), "second guarded New Chat request");
  await clickButton("Discard draft and leave Library");
  await waitFor(() => evaluate("document.querySelector('.conversation-empty-state') !== null"), "New Chat after explicit draft discard");
  assert.equal(await evaluate("document.querySelector('#resource-revision-text') === null"), true, "explicit discard leaves the Library editor");
  calls = await currentCalls();
  assert.equal(calls.some(call => call.command === "create_conversation" || call.command === "create_conversation_turn"), false, "navigation through the dirty-draft guard does not persist or send a Conversation");
  checks.push("Projects-to-Workspace and New Chat respect the dirty Library guard, preserve drafts on Stay, restore focus, and navigate after explicit discard");

  await cdp("Emulation.clearDeviceMetricsOverride");
  return checks;
}

async function runLibraryNavigationChecks() {
  const checks = [];
  await clickHarness("activateLibraryNavigationGuard");
  await waitFor(() => evaluate("document.querySelector('#resource-revision-title')?.textContent === 'A.txt'"), "Library guard Resource editor");
  await waitFor(() => evaluate("[...document.querySelectorAll('button')].some(button => button.textContent.trim() === 'Edit small text file')"), "Library guard Resource text controls");
  await clickButton("Edit small text file");
  await typeTextareaDraft("Draft retained after failed navigation save");

  await cdp("Emulation.setDeviceMetricsOverride", { width: 390, height: 844, deviceScaleFactor: 1, mobile: true });
  await clickButton("Open mobile menu");
  await waitFor(() => evaluate("document.querySelector('.library-test-navigation')?.hidden === false"), "open mobile navigation fixture");
  await clickButton("Home", ".library-test-navigation");
  await waitFor(() => evaluate("Boolean(document.querySelector('.resource-discard-dialog[role=alertdialog]'))"), "mobile Library leave confirmation");
  await pressKey("Escape");
  await waitFor(() => evaluate("!document.querySelector('.resource-discard-dialog[role=alertdialog]')"), "mobile Stay on Library");
  const mobileFocus = await evaluate("({ id: document.activeElement.id, width: window.innerWidth, visible: document.activeElement.getClientRects().length > 0, navHidden: document.querySelector('.library-test-navigation')?.hidden })");
  assert.equal(mobileFocus.id, "test-mobile-menu", `mobile Stay returns focus to the visible menu trigger: ${JSON.stringify(mobileFocus)}`);
  assert.equal(mobileFocus.visible, true, "focus is not restored to a hidden mobile navigation item");
  assert.equal(await evaluate("document.querySelector('#resource-revision-title')?.textContent"), "A.txt");
  await cdp("Emulation.clearDeviceMetricsOverride");
  checks.push("A01 mobile Stay keeps the Resource draft and restores focus to the visible menu trigger");

  await clickHarness("setResourceCommitMode", "reject");
  await clickButton("Leave Library for Home");
  await clickHarness("clearCalls");
  await waitFor(() => evaluate("Boolean(document.querySelector('.resource-discard-dialog[role=alertdialog]'))"), "Library leave confirmation before failed save");
  await clickButton("Save draft and leave Library");
  await waitFor(() => evaluate("document.querySelector('.resource-discard-dialog [role=alert]')?.textContent.includes('Rendered Resource save failed')"), "announced Library save failure");
  assert.equal(await evaluate("document.querySelector('.resource-discard-dialog button')?.disabled"), true, "Save cannot be retried from an error state before reviewing it");
  assert.equal(await evaluate("[...document.querySelectorAll('.resource-discard-dialog button')].find(button => button.textContent.trim() === 'Discard draft and leave Library')?.disabled"), false, "failed save permits an explicit discard");
  assert.equal(await evaluate("document.querySelector('#library-navigation-home') === null"), true, "a failed save does not navigate away");
  await clickButton("Stay in Library");
  await waitFor(() => evaluate("!document.querySelector('.resource-discard-dialog[role=alertdialog]')"), "review failed Resource save");
  await waitFor(() => evaluate("document.querySelector('.resource-revision-panel')?.getAttribute('data-library-operation-pending') === 'false' && document.querySelector('.resource-revision-panel')?.getAttribute('data-library-can-save') === 'true'"), "Resource editor becomes retryable after failure review");
  assert.equal(await evaluate("document.querySelector('#resource-revision-text')?.value"), "Draft retained after failed navigation save");
  assert.equal(await evaluate("Boolean(document.querySelector('.resource-revision-panel [role=alert]'))"), true, "the Resource error is visible after returning to the editor");
  checks.push("A01 failed Save is announced in the modal, preserves the draft, and returns to an accessible editor for review");

  await setTextareaDraft("x".repeat(1024 * 1024 + 1));
  await clickButton("Leave Library for Home");
  await waitFor(() => evaluate("Boolean(document.querySelector('.resource-discard-dialog[role=alertdialog]'))"), "Library leave confirmation before oversized text validation");
  await clickButton("Save draft and leave Library");
  await waitFor(() => evaluate("document.querySelector('.resource-discard-dialog [role=alert]')?.textContent.includes('Text edits are limited to 1 MiB.')"), "announced oversized Resource text validation");
  assert.equal(await evaluate("[...document.querySelectorAll('.resource-discard-dialog button')].find(button => button.textContent.trim() === 'Discard draft and leave Library')?.disabled"), false, "oversized text validation leaves Discard available");
  assert.equal(await evaluate("[...document.querySelectorAll('.resource-discard-dialog button')].find(button => button.textContent.trim() === 'Stay in Library')?.disabled"), false, "oversized text validation leaves Stay available");
  assert.equal(await evaluate("document.querySelector('#library-navigation-home') === null"), true, "oversized text validation does not navigate away");
  await clickButton("Stay in Library");
  await waitFor(() => evaluate("!document.querySelector('.resource-discard-dialog[role=alertdialog]')"), "return to oversized Resource text draft");
  await waitFor(() => evaluate("document.querySelector('.resource-revision-panel [role=alert]')?.textContent.includes('Text edits are limited to 1 MiB.')"), "Resource text size error after returning to editor");
  await setTextareaDraft("Draft retained after failed navigation save");
  checks.push("A01 oversized text Save reports the validation error and leaves Stay/Discard available");

  await clickHarness("setResourceCommitMode", "defer");
  await clickButton("Leave Library for Home");
  await clickHarness("clearCalls");
  await waitFor(() => evaluate("Boolean(document.querySelector('.resource-discard-dialog[role=alertdialog]'))"), "Library leave confirmation before deferred save");
  assert.equal(await evaluate("[...document.querySelectorAll('.resource-discard-dialog button')].find(button => button.textContent.trim() === 'Save draft and leave Library')?.disabled"), false, "a valid retained draft enables Save before the deferred attempt");
  await clickButton("Save draft and leave Library");
  await waitFor(() => evaluate("[...document.querySelectorAll('.resource-discard-dialog button')].find(button => button.textContent.trim() === 'Discard draft and leave Library')?.disabled === true"), "discard lock while save is running");
  assert.equal(await evaluate("document.querySelector('.resource-discard-dialog')?.contains(document.activeElement)"), true, "focus stays inside the dialog after Save becomes disabled");
  assert.equal(await evaluate("document.querySelector('.library-test-main')?.inert"), true, "the Library main content is inert while the alertdialog is open");
  await pressKey("Tab");
  assert.equal(await evaluate("document.querySelector('.resource-discard-dialog')?.contains(document.activeElement)"), true, "Tab cannot move focus into inert Library content while Save is pending");
  await waitFor(async () => (await currentCalls()).some(call => call.command === "commit_resource_revision_upload"), "deferred Resource commit");
  assert.equal(await evaluate("[...document.querySelectorAll('.resource-discard-dialog button')].find(button => button.textContent.trim() === 'Discard draft and leave Library')?.disabled"), true, "Discard is disabled while Save has an in-flight commit");
  assert.equal(await evaluate("document.querySelector('#library-navigation-home') === null"), true, "the Library stays mounted while the commit is unresolved");
  await clickHarness("resolveResourceCommit");
  await waitFor(() => evaluate("Boolean(document.querySelector('#library-navigation-home'))"), "navigate after confirmed Resource commit");
  checks.push("A01 Save locks out Discard during a delayed commit and navigates only after the commit receipt arrives");

  await clickButton("Return to Library");
  await waitFor(() => evaluate("document.querySelector('#resource-revision-title')?.textContent === 'A.txt'"), "return to Resource editor for multi-chunk upload");
  await clickButton("Edit small text file");
  await setTextareaDraft("y".repeat(64 * 1024 + 1));
  await clickHarness("setResourceCommitMode", "success");
  await clickButton("Leave Library for Home");
  await waitFor(() => evaluate("Boolean(document.querySelector('.resource-discard-dialog[role=alertdialog]'))"), "Library leave confirmation before multi-chunk save");
  await clickHarness("clearCalls");
  await clickButton("Save draft and leave Library");
  await waitFor(async () => (await currentCalls()).filter(call => call.command === "upload_resource_chunk").length >= 2, "two accepted Resource upload chunks");
  const chunkCalls = (await currentCalls()).filter(call => call.command === "upload_resource_chunk");
  assert.deepEqual(chunkCalls.slice(0, 2).map(call => call.args.chunkIndex), [0, 1], "accepted chunk advances the next chunk index");
  assert.deepEqual(chunkCalls.slice(0, 2).map(call => call.args.contentRange), ["bytes 0-65535/65537", "bytes 65536-65536/65537"]);
  await waitFor(async () => (await currentCalls()).some(call => call.command === "commit_resource_revision_upload"), "multi-chunk Resource commit");
  assert.equal((await currentCalls()).filter(call => call.command === "commit_resource_revision_upload").length, 1, "multi-chunk upload commits one revision");
  await waitFor(() => evaluate("Boolean(document.querySelector('#library-navigation-home'))"), "navigate after multi-chunk Resource commit");
  checks.push("A12 multi-chunk Resource upload advances from chunk zero to chunk one and commits once");
  return checks;
}

async function runWorkspaceFirstRunChecks() {
  const checks = [];

  await clickHarness("configureWorkspaceFirstRun", "loading");
  await waitFor(() => evaluate(`(() => [...document.querySelectorAll('[role="status"], [aria-live]')]
    .some(node => /checking/i.test(node.textContent ?? "")))()`), "announced Workspace list loading state");
  checks.push("E02-S01 Workspace list loading is announced as a status");

  await clickHarness("configureWorkspaceFirstRun", "error");
  await waitFor(() => evaluate(`/workspace.*(?:unavailable|could not be checked|could not be loaded)/i.test(document.querySelector('[aria-label="First-run Workspace setup"]')?.textContent ?? "")`), "explicit unavailable Workspace list state");
  assert.equal(await evaluate(`[...document.querySelectorAll('button')].some(button => /refresh|retry/i.test(button.textContent) && !button.disabled)`), true, "Workspace list failure offers an enabled refresh/retry action");
  await clickButtonContaining(["retry"]);
  assert.equal((await evaluate("window.litecoworkHarness.workspaceActionCounts()")).refreshWorkspaces, 1, "Workspace refresh invokes its callback once");
  checks.push("E02-S01 failed Workspace listing explains unavailability and exposes a working refresh action");

  await clickHarness("configureWorkspaceFirstRun", "offline");
  await waitFor(() => evaluate(`/local runtime/i.test(document.querySelector('[aria-label="First-run Workspace setup"]')?.textContent ?? "") && /unavailable|offline|not running|not responding/i.test(document.querySelector('[aria-label="First-run Workspace setup"]')?.textContent ?? "")`), "explicit offline local Runtime state");
  assert.equal(await evaluate(`[...document.querySelectorAll('button')].some(button => /refresh.*runtime|runtime.*refresh/i.test(button.textContent) && !button.disabled)`), true, "offline Runtime offers an enabled Runtime refresh action");
  assert.equal(await evaluate(`[...document.querySelectorAll('button')].some(button => /start.*runtime|runtime.*start/i.test(button.textContent) && !button.disabled)`), true, "stopped local Runtime offers an enabled start action when eligible");
  await clickButtonContaining(["refresh", "runtime"]);
  await clickButtonContaining(["start", "runtime"]);
  const runtimeActions = await evaluate("window.litecoworkHarness.workspaceActionCounts()");
  assert.equal(runtimeActions.refreshRuntime, 1);
  assert.equal(runtimeActions.startRuntime, 1);
  checks.push("E02-S01 offline Runtime state is explicit and its eligible refresh/start actions call their handlers");

  await clickHarness("configureWorkspaceFirstRun", "existing");
  await waitFor(() => evaluate(`(() => {
    const text = document.querySelector('[aria-label="First-run Workspace setup"]')?.textContent ?? '';
    return /workspace/i.test(text) && /restor|select|already/i.test(text);
  })()`), "existing Workspace selection/restoration state");
  assert.equal(await evaluate(`/create one to continue/i.test(document.querySelector('[aria-label="First-run Workspace setup"]')?.textContent ?? '')`), false, "existing Workspace restoration does not tell the user to create another Workspace");
  assert.equal(await evaluate(`Boolean(document.querySelector('[aria-label="First-run Workspace setup"] form'))`), false, "saved Workspaces are restored/selected instead of offering duplicate creation");
  assert.equal(await evaluate(`[...document.querySelectorAll('[aria-label="First-run Workspace setup"] button')].some(button => /create.*workspace/i.test(button.textContent))`), false, "no duplicate Workspace create action is shown when one already exists");
  assert.equal((await evaluate("window.litecoworkHarness.workspaceActionCounts()")).createWorkspace, 0, "existing Workspace selection does not create a duplicate");
  checks.push("E02-S01 existing saved Workspace state shows restoration/selection and suppresses duplicate creation");

  await clickHarness("configureWorkspaceFirstRun", "ready");
  await waitFor(() => evaluate(`Boolean(document.querySelector('[aria-label="First-run Workspace setup"] form'))`), "authenticated first-run Workspace form");
  const formState = await evaluate(`(() => {
    const root = document.querySelector('[aria-label="First-run Workspace setup"]');
    const input = [...(root?.querySelectorAll('input') ?? [])].find(candidate => {
      const label = [...(candidate.labels ?? [])].map(node => node.textContent).join(' ');
      return /workspace.*name|name.*workspace/i.test(label + ' ' + (candidate.getAttribute('aria-label') ?? ''));
    });
    const form = input?.form;
    const submit = form?.querySelector('button[type="submit"], input[type="submit"]');
    const actionText = [...(root?.querySelectorAll('button') ?? [])]
      .map(button => button.textContent.trim())
      .filter(Boolean);
    return {
      labeled: Boolean(input),
      value: input?.value ?? null,
      submitDisabled: submit?.disabled ?? null,
      inputLocked: input?.disabled === true || input?.readOnly === true,
      text: root?.textContent ?? '',
      actionText,
      hasTaskComposer: Boolean(root?.querySelector('textarea, [data-testid="home-task-composer"], .home-task-composer')),
    };
  })()`);
  assert.equal(formState.labeled, true, "Workspace setup has a visibly associated name label");
  assert.equal(formState.value, "", "new Workspace name starts blank");
  assert.equal(formState.submitDisabled, true, "blank Workspace name cannot be submitted");
  assert.equal(formState.inputLocked, false, "new Workspace form is editable before submission");
  assert.equal(formState.hasTaskComposer, false, "first-run setup does not expose a Task composer");
  assert.equal(/(?:task|agent|coworker|worker)\s+(?:is\s+)?(?:ready|created|started|connected|available)/i.test(formState.text), false, "first-run setup makes no Task or agent availability claim");
  assert.equal(formState.actionText.some(label => /task|agent|coworker|worker/i.test(label)), false, "first-run actions do not imply Task or agent execution");

  await clickHarness("configureWorkspaceFirstRun", "busy");
  await waitFor(() => evaluate(`Boolean(document.querySelector('[aria-label="First-run Workspace setup"] form button[type="submit"]')?.disabled)`), "busy Workspace create button");
  const busyButtonText = await evaluate(`document.querySelector('[aria-label="First-run Workspace setup"] form button[type="submit"]')?.textContent ?? ''`);
  assert.match(busyButtonText, /creat|saving|setting up/i, "create button describes its busy state");
  assert.equal((await evaluate("window.litecoworkHarness.workspaceActionCounts()")).createWorkspace, 0, "busy fixture has not submitted a Workspace request");
  checks.push("E02-S01 Operator-ready setup labels the Workspace name, disables blank/busy submission, and shows no Task or agent claim");

  await clickHarness("configureWorkspaceFirstRun", "pending");
  await waitFor(() => evaluate(`Boolean(document.querySelector('[aria-label="First-run Workspace setup"] form'))`), "ambiguous Workspace create retry form");
  const pendingState = await evaluate(`(() => {
    const root = document.querySelector('[aria-label="First-run Workspace setup"]');
    const input = [...(root?.querySelectorAll('input') ?? [])].find(candidate => {
      const label = [...(candidate.labels ?? [])].map(node => node.textContent).join(' ');
      return /workspace.*name|name.*workspace/i.test(label + ' ' + (candidate.getAttribute('aria-label') ?? ''));
    });
    return { value: input?.value, locked: input?.disabled === true || input?.readOnly === true };
  })()`);
  assert.equal(pendingState.value, "Local Workspace", "ambiguous create retains the exact Workspace name");
  assert.equal(pendingState.locked, true, "ambiguous create locks the retained Workspace name while offering retry");
  await clickButtonContaining(["retry"]);
  assert.equal((await evaluate("window.litecoworkHarness.workspaceActionCounts()")).createWorkspace, 1, "retry invokes the existing create callback once");
  checks.push("E02-S01 ambiguous Workspace creation retains and locks the name while exposing a retry");

  await clickHarness("configureWorkspaceFirstRun", "ready");
  await typeWorkspaceName("Rendered local Workspace");
  await clickButtonContaining(["create", "workspace"], '[aria-label="First-run Workspace setup"]');
  await waitFor(() => evaluate("window.litecoworkHarness.workspaceActionCounts().createWorkspace === 1"), "single Workspace create callback");
  await new Promise(resolveDelay => setTimeout(resolveDelay, 120));
  assert.equal((await evaluate("window.litecoworkHarness.workspaceActionCounts()")).createWorkspace, 1, "one submit event invokes Workspace create exactly once");
  checks.push("E02-S01 submitting a named Workspace invokes the create callback once");

  return checks;
}

async function runWorkspaceAppChecks() {
  const checks = [];

  await clickHarness("mountProductionApp", "workspace-list-failure");
  await waitFor(() => evaluate(`document.querySelector('.workspace-switcher')?.getAttribute('aria-label')?.includes('READY') === true`), "production App Runtime readiness before Workspace list failure");
  await waitFor(() => evaluate("window.litecoworkHarness.productionAppWorkspaceListCount() === 1"), "deferred initial production App Workspace list");
  await clickButtonContaining(["settings"]);
  await waitFor(() => evaluate(`document.querySelector('.page-content h1')?.textContent === 'Settings'`), "production App Settings page while the initial Workspace list is pending");
  const createDisabledWhileListPending = await evaluate("document.querySelector('#workspace-name')?.form?.querySelector('button[type=submit]')?.disabled === true");
  assert.equal(await evaluate("document.querySelector('#workspace-name')?.disabled === true"), true, "Workspace name remains locked while the saved Workspace list is unknown");
  await clickButton("Create Workspace");
  assert.equal(await evaluate("window.litecoworkHarness.productionAppWorkspaceCreateCount()"), 0, "Workspace creation is not sent while the initial list is pending");
  await clickHarness("rejectInitialAppWorkspaceList");
  await waitFor(() => evaluate(`[...document.querySelectorAll('[role="status"]')].some(node => /Workspace list is unavailable until the authenticated local Operator API is ready/i.test(node.textContent ?? ''))`), "initial Workspace list failure in production Settings");
  const createDisabledAfterListFailure = await evaluate("document.querySelector('#workspace-name')?.form?.querySelector('button[type=submit]')?.disabled === true");
  assert.equal(await evaluate("document.querySelector('#workspace-name')?.disabled === true"), true, "Workspace name remains locked after the saved Workspace list fails");
  await clickButton("Create Workspace");
  assert.equal(await evaluate("window.litecoworkHarness.productionAppWorkspaceCreateCount()"), 0, "Workspace creation is not sent after the initial list fails");
  await clickButton("Refresh", ".workspace-list-card");
  await waitFor(() => evaluate("window.litecoworkHarness.productionAppWorkspaceListCount() === 2"), "manual retry after initial Workspace list failure");
  await typeInputValue("#workspace-name", "Wait for Workspace list");
  await waitFor(() => evaluate("document.querySelector('#workspace-name')?.form?.querySelector('button[type=submit]')?.disabled === false"), "Workspace create enabled after empty list retry succeeds");
  assert.deepEqual(
    [createDisabledWhileListPending, createDisabledAfterListFailure],
    [true, true],
    "the Settings create action stays disabled while Workspace state is pending and after the list fails",
  );
  assert.equal(await evaluate("window.litecoworkHarness.productionAppWorkspaceCreateCount()"), 0, "a successful empty-list retry does not submit Workspace creation");
  checks.push("E02-S01 Settings keeps Workspace creation disabled while the initial list is pending or failed, then enables it after an empty retry");

  await clickHarness("mountProductionApp", "workspace-list-race");
  await waitFor(() => evaluate(`document.querySelector('.workspace-switcher')?.getAttribute('aria-label')?.includes('READY') === true`), "production App Runtime readiness");
  await waitFor(() => evaluate("window.litecoworkHarness.productionAppWorkspaceListCount() === 1"), "deferred initial production App Workspace list before creation race");
  await clickButtonContaining(["settings"]);
  await waitFor(() => evaluate(`document.querySelector('.page-content h1')?.textContent === 'Settings'`), "production App Settings page");
  await clickHarness("resolveInitialAppWorkspaceList");
  await typeInputValue("#workspace-name", "Created during list refresh");
  await clickButton("Create Workspace");
  await waitFor(() => evaluate("window.litecoworkHarness.productionAppWorkspaceCreateCount() === 1"), "deferred Workspace create receipt");
  await clickHarness("deferNextAppWorkspaceListRefresh");
  await clickButton("Refresh", ".workspace-list-card");
  await waitFor(() => evaluate("window.litecoworkHarness.productionAppWorkspaceListCount() === 2"), "deferred stale Workspace list refresh");
  await clickHarness("resolveAppWorkspaceCreateReceipt");
  await waitFor(() => evaluate(`document.querySelector('.workspace-list')?.textContent.includes('Created during list refresh') === true`), "created Workspace in production Settings");
  await clickHarness("resolveStaleAppWorkspaceList");
  const createdWorkspaceSurvivedOldList = await evaluate(`document.querySelector('.workspace-list')?.textContent.includes('Created during list refresh') === true`);
  assert.equal(createdWorkspaceSurvivedOldList, true, "a late stale Workspace list refresh cannot erase a Workspace whose create receipt arrived first");
  checks.push("E02-S01 production App keeps a newly created Workspace when a stale manual list refresh arrives later");

  await clickHarness("mountProductionApp", "workspace-policy-lost-ack");
  await waitFor(() => evaluate(`document.querySelector('.workspace-switcher')?.getAttribute('aria-label')?.includes('READY') === true`), "production App Runtime readiness before policy retry");
  await waitFor(() => evaluate("window.litecoworkHarness.productionAppWorkspaceListCount() === 1"), "production App initial policy Workspace list");
  await clickButtonContaining(["settings"]);
  await waitFor(() => evaluate(`document.querySelector('.page-content h1')?.textContent === 'Settings' && document.querySelector('.workspace-list')?.textContent.includes('Policy Workspace') === true`), "production Settings policy Workspace");
  await clickButton("Set this computer only");
  await waitFor(() => evaluate(`document.querySelector('.workspace-policy-form button')?.textContent === 'Retry exact update'`), "visible exact retry after lost policy response and failed Workspace refresh");
  const policyRetryBefore = await evaluate("window.litecoworkHarness.productionAppWorkspacePolicyRequests()");
  assert.equal(policyRetryBefore.length, 1, "the first policy update is sent once");
  assert.equal(policyRetryBefore[0].workspaceId, "workspace-policy-a");
  assert.equal(policyRetryBefore[0].replicationPolicy, "LOCAL_ONLY");
  assert.equal(policyRetryBefore[0].expectedVersion, 4);
  assert.equal(typeof policyRetryBefore[0].requestId, "string");
  assert.equal(await evaluate("window.litecoworkHarness.productionAppWorkspaceListCount()"), 2, "the lost response is followed by a failed list refresh");
  await clickButton("Retry exact update");
  await waitFor(() => evaluate("window.litecoworkHarness.productionAppWorkspacePolicyRequests().length === 2"), "exact Workspace policy retry receipt");
  const policyRetryAfter = await evaluate("window.litecoworkHarness.productionAppWorkspacePolicyRequests()");
  assert.deepEqual(policyRetryAfter[1], policyRetryAfter[0], "retry preserves the same Workspace, target policy, expected version, and RequestId");
  await waitFor(() => evaluate(`document.querySelector('.workspace-policy-form .status-pill')?.textContent === 'This computer only'`), "committed Workspace policy after exact replay");
  checks.push("E02-S02 Settings replays the exact Workspace policy request after a lost acknowledgement and failed refresh");

  await clickHarness("mountProductionApp", "workspace-policy-lost-ack-confirmed");
  await waitFor(() => evaluate(`document.querySelector('.workspace-switcher')?.getAttribute('aria-label')?.includes('READY') === true`), "production App Runtime readiness before policy confirmation refresh");
  await waitFor(() => evaluate("window.litecoworkHarness.productionAppWorkspaceListCount() === 1"), "production App initial policy Workspace list before confirmation refresh");
  await clickButtonContaining(["settings"]);
  await clickButton("Set this computer only");
  await waitFor(() => evaluate(`document.querySelector('.workspace-policy-form .status-pill')?.textContent === 'This computer only'`), "lost policy acknowledgement confirmed by Workspace refresh");
  assert.equal(await evaluate(`document.querySelector('.workspace-list-card .inline-error')?.textContent.includes('could not be confirmed') ?? false`), false, "a fresh Workspace projection that confirms the target policy clears the ambiguous-update error");
  checks.push("E02-S02 Settings treats a fresh Workspace read of the requested policy as confirmation after a lost acknowledgement");

  await clickHarness("mountProductionApp", "workspace-policy-list-race");
  await waitFor(() => evaluate(`document.querySelector('.workspace-switcher')?.getAttribute('aria-label')?.includes('READY') === true`), "production App Runtime readiness before Workspace list race");
  await waitFor(() => evaluate("window.litecoworkHarness.productionAppWorkspaceListCount() === 1"), "production App initial policy Workspace list before stale-list race");
  await clickButtonContaining(["settings"]);
  await clickHarness("deferNextAppWorkspaceListRefresh");
  await clickButton("Refresh", ".workspace-list-card");
  await waitFor(() => evaluate("window.litecoworkHarness.productionAppWorkspaceListCount() === 2"), "deferred older Workspace list before policy update");
  await clickButton("Set this computer only");
  await waitFor(() => evaluate(`document.querySelector('.workspace-policy-form .status-pill')?.textContent === 'This computer only'`), "newer policy receipt before old list resolves");
  await clickHarness("resolveStaleAppWorkspaceList", [{
    workspaceId: "workspace-policy-a",
    name: "Policy Workspace",
    replicationPolicy: "METADATA_ONLY",
    defaultAgentBindingId: null,
    primaryCoworkerId: null,
    status: "ACTIVE",
    version: 4,
  }]);
  assert.equal(await evaluate(`document.querySelector('.workspace-policy-form .status-pill')?.textContent`), "This computer only", "a delayed v4 list cannot roll back the v5 policy receipt");
  checks.push("E02-S02 Settings preserves a newer Workspace policy receipt when an older list response arrives later");

  await clickHarness("mountProductionApp", "runtime-refresh-failure");
  await waitFor(() => evaluate(`Boolean(document.querySelector('form[aria-label="Create your first Workspace"]'))`), "production App first-run Workspace form");
  await clickButtonContaining(["settings"]);
  await waitFor(() => evaluate(`document.querySelector('.page-content h1')?.textContent === 'Settings'`), "production App Settings page for Runtime error");
  await typeInputValue("#workspace-name", "Must wait for Runtime status");
  await clickButton("Refresh status");
  await waitFor(() => evaluate(`[...document.querySelectorAll('[role="alert"]')].some(node => /Desktop Runtime status is unavailable/i.test(node.textContent ?? ''))`), "rendered Runtime status refresh error");
  const createStateAfterRuntimeFailure = await evaluate(`(() => {
    const form = document.querySelector('#workspace-name')?.form;
    const submit = form?.querySelector('button[type="submit"]');
    return {
      disabled: submit?.disabled ?? null,
      workspaceCreated: document.querySelector('.workspace-list')?.textContent.includes('Must wait for Runtime status') === true,
    };
  })()`);
  assert.equal(createStateAfterRuntimeFailure.disabled, true, "a failed Runtime readiness refresh disables Workspace creation even after an earlier READY result");
  await clickButton("Create Workspace");
  assert.equal(await evaluate("window.litecoworkHarness.productionAppWorkspaceCreateCount()"), 0, "the disabled Workspace action does not issue a create command after Runtime status fails");
  assert.equal(createStateAfterRuntimeFailure.workspaceCreated, false, "the failed Runtime refresh leaves no falsely created Workspace in the rendered list");
  checks.push("E02-S01 production App reports a failed Runtime refresh and disables Workspace creation after stale READY status");

  return checks;
}

async function openWorkspaceRootScanScenario(scenario) {
  await clickHarness("mountProductionApp", scenario);
  await waitFor(() => evaluate(`document.querySelector('.workspace-switcher')?.getAttribute('aria-label')?.includes('READY') === true`), "production App Runtime readiness for WorkspaceRoot scan");
  await clickButtonContaining(["library"]);
  await waitFor(() => evaluate(`Boolean([...document.querySelectorAll('.persistent-folder-list li')].find(row => row.querySelector('strong')?.textContent === 'Project folder' && row.querySelector('button[aria-label="Scan folder metadata in Project folder"]')))`), "eligible persistent folder and Scan folder action");
  await clickHarness("clearCalls");
}

async function runWorkspaceRootScanChecks() {
  const checks = [];
  const limitationCopy = await (async () => {
    await openWorkspaceRootScanScenario("workspace-root-scan-complete");
    return evaluate(`document.querySelector('[aria-label="Persistent folder scopes"] .section-heading p')?.textContent ?? ''`);
  })();
  assert.match(limitationCopy, /manual scan counts folder metadata only/i);
  assert.match(limitationCopy, /does not add files to the Library, read file content, watch for changes, or infer deletions/i);

  await clickButton("Scan folder");
  await waitFor(() => evaluate(`(() => {
    const button = document.querySelector('[aria-label="Scan folder metadata in Project folder"]');
    const status = document.querySelector('.persistent-folder-list li small[role="status"]');
    return button?.textContent.trim() === 'Scanning…' && button.disabled && /Scanning folder metadata/i.test(status?.textContent ?? '');
  })()`), "visible and disabled folder scan loading state");
  assert.equal(await evaluate("window.litecoworkHarness.workspaceRootScanPendingCount()"), 1, "the rendered IPC fixture is holding the scan response so loading was observable");
  const scanCall = await evaluate("window.litecoworkHarness.calls().find(call => call.command === 'scan_workspace_root')");
  assert.deepEqual(scanCall.args, {
    workspaceId: "workspace-a",
    workspaceRootId: "workspace-root-a",
    expectedVersion: 7,
    requestId: scanCall.args.requestId,
  }, "the explicit action carries the selected Workspace, root, pinned version, and generated request ID");
  assert.equal(typeof scanCall.args.requestId, "string");
  assert.ok(scanCall.args.requestId.length > 0);
  await clickHarness("resolveWorkspaceRootScan", "COMPLETE");
  await waitFor(() => evaluate(`document.querySelector('.persistent-folder-list li small[role="status"]')?.textContent === 'Scan complete: counted metadata for 8 files in 12 folder entries.'`), "count-only COMPLETE scan summary");
  const completeMessage = await evaluate("document.querySelector('.persistent-folder-list li small[role=\"status\"]')?.textContent ?? ''");
  assert.doesNotMatch(completeMessage, /content|index|delete|deletion|library|watch/i, "the COMPLETE result makes no content, indexing, deletion, or watching claim");
  assert.equal((await evaluate("window.litecoworkHarness.calls()")).filter(call => call.command === "scan_workspace_root").length, 1, "one explicit Scan folder click issues one scan command");
  checks.push("E02-S03 explicit Scan folder shows an observable busy state, pins the selected version, then reports only truthful COMPLETE metadata counts");

  await openWorkspaceRootScanScenario("workspace-root-scan-partial");
  await clickButton("Scan folder");
  await waitFor(() => evaluate("window.litecoworkHarness.workspaceRootScanPendingCount() === 1"), "deferred PARTIAL scan request");
  await clickHarness("resolveWorkspaceRootScan", "PARTIAL");
  await waitFor(() => evaluate(`document.querySelector('.persistent-folder-list li small[role="status"]')?.textContent === 'Partial scan: counted metadata for 3 files in 7 folder entries. Reason: Scan size limit reached.'`), "truthful PARTIAL scan count and reason");
  const partialMessage = await evaluate("document.querySelector('.persistent-folder-list li small[role=\"status\"]')?.textContent ?? ''");
  assert.doesNotMatch(partialMessage, /content|index|delete|deletion|library|watch/i, "the PARTIAL result does not claim content, indexing, deletion, or watching behavior");
  checks.push("E02-S03 PARTIAL scan reports bounded metadata counts and a user-readable reason without implying indexing, content access, or deletion inference");

  await openWorkspaceRootScanScenario("workspace-root-scan-stale");
  await clickButton("Scan folder");
  await waitFor(() => evaluate("window.litecoworkHarness.workspaceRootScanPendingCount() === 1"), "deferred stale-version scan request");
  await clickHarness("rejectWorkspaceRootScanAsStale");
  await waitFor(() => evaluate(`document.querySelector('.persistent-folder-list li small[role="alert"]')?.textContent.includes('Leave and reopen the Library to refresh its status before scanning again.') === true`), "stale-version guidance to reload the Library");
  const staleProjection = await evaluate(`(() => {
    const row = document.querySelector('.persistent-folder-list li');
    const button = row?.querySelector('button[aria-label="Scan folder metadata in Project folder"]');
    return {
      rowName: row?.querySelector('strong')?.textContent ?? null,
      rowText: row?.textContent ?? '',
      scanDisabled: button?.disabled ?? null,
      scanLabel: button?.textContent.trim() ?? null,
    };
  })()`);
  assert.equal(staleProjection.rowName, "Project folder");
  assert.match(staleProjection.rowText, /Active · folder watching and indexing are not active in this build/);
  assert.match(staleProjection.rowText, /Last observed: available/);
  assert.equal(staleProjection.scanLabel, "Scan folder");
  assert.equal(staleProjection.scanDisabled, true, "stale root version disables the scan action until refresh");
  const staleCalls = await evaluate("window.litecoworkHarness.calls()");
  assert.equal(staleCalls.filter(call => call.command === "scan_workspace_root").length, 1, "the stale response is not retried under the old request");
  assert.equal(staleCalls.filter(call => call.command === "list_workspace_roots").length, 0, "the stale response does not silently refresh and replace the cached root row");
  checks.push("E02-S03 stale WorkspaceRoot version retains the cached root projection, directs Library reload, and disables another scan");

  await openWorkspaceRootScanScenario("workspace-root-scan-unsupported");
  await clickButton("Scan folder");
  await waitFor(() => evaluate("window.litecoworkHarness.workspaceRootScanPendingCount() === 1"), "deferred unsupported-platform scan request");
  await clickHarness("rejectWorkspaceRootScan", "SCAN_UNSUPPORTED_PLATFORM");
  await waitFor(() => evaluate(`document.querySelector('.persistent-folder-list li small[role="alert"]')?.textContent.includes('not available on this platform') === true`), "unsupported-platform guidance");
  const unsupportedScan = await evaluate(`(() => {
    const row = document.querySelector('.persistent-folder-list li');
    return { text: row?.textContent ?? '', hasButton: Boolean(row?.querySelector('button[aria-label="Scan folder metadata in Project folder"]')) };
  })()`);
  assert.equal(unsupportedScan.hasButton, false, "the unsupported platform has no scan action");
  assert.doesNotMatch(unsupportedScan.text, /retry to check the same scan request/i, "unsupported platform guidance never recommends a same-key retry");
  assert.equal((await evaluate("window.litecoworkHarness.calls()")).filter(call => call.command === "scan_workspace_root").length, 1, "unsupported scan is not retried");
  checks.push("E02-S03 unsupported platform reports the limitation and removes the scan action");

  await openWorkspaceRootScanScenario("workspace-root-scan-rejected");
  await clickButton("Scan folder");
  await waitFor(() => evaluate("window.litecoworkHarness.workspaceRootScanPendingCount() === 1"), "deferred rejected scan request");
  await clickHarness("rejectWorkspaceRootScan", "RESOURCE_IDENTITY_UNAVAILABLE");
  await waitFor(() => evaluate(`document.querySelector('.persistent-folder-list li small[role="alert"]')?.textContent.includes('remove and add it again') === true`), "identity-loss re-add guidance");
  const rejectedScan = await evaluate(`(() => {
    const row = document.querySelector('.persistent-folder-list li');
    const button = row?.querySelector('button[aria-label="Scan folder metadata in Project folder"]');
    return { text: row?.textContent ?? '', disabled: button?.disabled ?? null };
  })()`);
  assert.equal(rejectedScan.disabled, true, "a deterministic identity rejection disables another scan until recovery");
  assert.doesNotMatch(rejectedScan.text, /retry to check the same scan request/i, "identity-loss guidance never recommends a same-key retry");
  assert.equal((await evaluate("window.litecoworkHarness.calls()")).filter(call => call.command === "scan_workspace_root").length, 1, "deterministically rejected scan is not retried");
  checks.push("E02-S03 deterministic identity rejection disables scanning and directs the user to re-add the folder");

  await openWorkspaceRootScanScenario("workspace-root-scan-ambiguous");
  await clickButton("Scan folder");
  await waitFor(() => evaluate("window.litecoworkHarness.workspaceRootScanPendingCount() === 1"), "deferred ambiguous scan request");
  await clickHarness("rejectWorkspaceRootScan", "SCAN_UNAVAILABLE");
  await waitFor(() => evaluate(`document.querySelector('.persistent-folder-list li small[role="alert"]')?.textContent.includes('Retry to check the same scan request.') === true`), "same-request ambiguous retry guidance");
  const firstAmbiguousRequest = await evaluate(`window.litecoworkHarness.calls().find(call => call.command === 'scan_workspace_root')?.args.requestId ?? null`);
  assert.equal(await evaluate(`(() => { const button = document.querySelector('.persistent-folder-list li button[aria-label="Scan folder metadata in Project folder"]'); return Boolean(button && !button.disabled); })()`), true, "an ambiguous result retains an enabled Scan folder retry");
  await clickButton("Scan folder");
  await waitFor(() => evaluate("window.litecoworkHarness.workspaceRootScanPendingCount() === 1"), "same-key retry remains pending for explicit resolution");
  const retriedScanCalls = await evaluate("window.litecoworkHarness.calls().filter(call => call.command === 'scan_workspace_root')");
  assert.equal(retriedScanCalls.length, 2, "only the explicit retry issues the second scan command");
  assert.equal(retriedScanCalls[1].args.requestId, firstAmbiguousRequest, "the ambiguous retry reuses the original idempotency identity");
  await clickHarness("resolveWorkspaceRootScan", "COMPLETE");
  await waitFor(() => evaluate(`document.querySelector('.persistent-folder-list li small[role="status"]')?.textContent === 'Scan complete: counted metadata for 8 files in 12 folder entries.'`), "same-key retry resolves to the committed scan summary");
  checks.push("E02-S03 ambiguous scan failure keeps an actionable retry pinned to the original request identity");

  return checks;
}

async function runAgentBindingUiChecks() {
  const checks = [];
  await clickHarness("mountAgentCatalogDefaultPrompt");
  await waitFor(() => evaluate("Boolean(document.querySelector('.agent-catalog__bindings'))"), "Workspace agent binding controls");
  await clickButton("Clear Workspace default…");
  assert.equal(
    await evaluate("Boolean(document.querySelector('[aria-label=\"Confirm clearing Workspace default lead\"]'))"),
    true,
    "the owner sees a confirmation before clearing the selected default",
  );
  await clickHarness("selectAgentCatalogWorkspace", "workspace-b");
  await clickHarness("selectAgentCatalogWorkspace", "workspace-a");
  await waitFor(() => evaluate("document.querySelector('main[data-agent-catalog-workspace=\"workspace-a\"] .agent-catalog__default-note') !== null"), "return to the original Workspace default controls");
  assert.equal(
    await evaluate("Boolean(document.querySelector('[aria-label=\"Confirm clearing Workspace default lead\"]'))"),
    false,
    "switching away and back cannot revive an old default-clear confirmation",
  );
  await clickButton("Disable");
  await waitFor(() => evaluate("document.querySelector('[data-last-disabled-agent-binding]')?.textContent === 'binding-secondary'"), "Disable action receives the selected non-default binding");
  checks.push("E03-S01 default-clear confirmation resets after Workspace switch and return, and Disable routes the exact binding");
  return checks;
}

async function runResourceUploadRecoveryChecks() {
  const checks = [];
  checks.push(...await runResourceUploadPauseChecks());

  await openResourceUploadScenario("resource-upload-ambiguous-create", "ambiguous create RequestId retry");
  const ambiguousCreateName = "f90-ambiguous-create.txt";
  await selectResourceUploadFile(37, ambiguousCreateName);
  await waitFor(() => evaluate("document.querySelector('.resource-intake-status')?.textContent.includes('lost the create acknowledgement after session creation') === true"), "accepted create with lost response");
  let ambiguousCreateCalls = await evaluate("window.litecoworkHarness.calls()");
  const firstCreateRequestId = ambiguousCreateCalls.find(call => call.command === "create_resource_upload")?.args.requestId;
  assert.ok(firstCreateRequestId, "the first create request has a stable RequestId");
  assert.equal(await evaluate("window.litecoworkHarness.resourceUploadServerSessionCount()"), 1, "the fixture accepted one create and records one session before losing the response");
  assert.equal(ambiguousCreateCalls.filter(call => call.command === "upload_resource_chunk").length, 0, "no chunk starts when create acknowledgement is uncertain");
  await selectResourceUploadFile(37, ambiguousCreateName);
  await waitFor(() => evaluate("window.litecoworkHarness.calls().filter(call => call.command === 'commit_resource_upload').length === 1"), "create retry and eventual upload completion");
  ambiguousCreateCalls = await evaluate("window.litecoworkHarness.calls()");
  const createRetries = ambiguousCreateCalls.filter(call => call.command === "create_resource_upload");
  assert.equal(createRetries.length, 2, "the same create intent is retried once after the lost response");
  assert.equal(createRetries[0].args.requestId, firstCreateRequestId, "the original create uses the captured RequestId");
  assert.equal(createRetries[1].args.requestId, firstCreateRequestId, "the retry reuses the exact create RequestId");
  assert.equal(await evaluate("window.litecoworkHarness.resourceUploadServerSessionCount()"), 1, "the deterministic receipt fixture maps the same key and payload to one session");
  assert.equal(ambiguousCreateCalls.filter(call => call.command === "commit_resource_upload").length, 1, "the recovered create commits once after its chunks are sent");
  checks.push("E02-S03 client reuses the exact create RequestId after an uncertain response (the rendered receipt fixture does not qualify provider idempotency)");

  await openResourceUploadScenario("resource-upload-ambiguous-chunk-unaccepted", "unaccepted chunk RequestId retry");
  const ambiguousChunkName = "f90-ambiguous-chunk-unaccepted.txt";
  await selectResourceUploadFile(37, ambiguousChunkName);
  await waitFor(() => evaluate("document.querySelector('.resource-intake-status')?.textContent.includes('lost the chunk response before accepting the chunk') === true"), "chunk response lost before the fixture accepts the range");
  let uncertainChunkCalls = await evaluate("window.litecoworkHarness.calls()");
  const originalUnacceptedChunk = uncertainChunkCalls.find(call => call.command === "upload_resource_chunk");
  assert.ok(originalUnacceptedChunk?.args.requestId, "the uncertain chunk write has a stable RequestId");
  assert.equal(await evaluate("window.litecoworkHarness.calls().filter(call => call.command === 'commit_resource_upload').length"), 0, "no commit follows a chunk response lost before acceptance");
  await selectResourceUploadFile(37, ambiguousChunkName);
  await waitFor(() => evaluate("window.litecoworkHarness.calls().filter(call => call.command === 'commit_resource_upload').length === 1"), "retry of the missing chunk and eventual upload completion");
  uncertainChunkCalls = await evaluate("window.litecoworkHarness.calls()");
  const uncertainChunkRetries = uncertainChunkCalls.filter(call => call.command === "upload_resource_chunk");
  assert.deepEqual(uncertainChunkRetries.map(call => call.args.chunkIndex), [0, 0, 1], "the missing first chunk is retried before the final chunk");
  assert.equal(uncertainChunkRetries[0].args.requestId, originalUnacceptedChunk.args.requestId, "the first attempt uses the captured chunk RequestId");
  assert.equal(uncertainChunkRetries[1].args.requestId, originalUnacceptedChunk.args.requestId, "retry reuses the exact RequestId for the same unaccepted chunk payload");
  assert.equal(uncertainChunkRetries[0].args.contentRange, uncertainChunkRetries[1].args.contentRange, "the retried RequestId carries the same byte range");
  assert.equal(uncertainChunkRetries[0].args.chunkSha256, uncertainChunkRetries[1].args.chunkSha256, "the retried RequestId carries the same chunk digest");
  checks.push("E02-S03 client reuses the exact chunk RequestId after progress proves the range was not accepted (fixture behavior is not provider evidence)");

  await clickHarness("mountProductionApp", "resource-upload-commit-progress-read-failure");
  await waitFor(() => evaluate(`Boolean(document.querySelector('.workspace-switcher'))`), "production App shell for ambiguous upload commit recovery");
  await clickButtonContaining(["library"]);
  await waitFor(() => evaluate(`document.querySelectorAll('[aria-label="Add files to this Workspace"] input[type="file"]')[1]?.disabled === false`), "selected Workspace and enabled upload input for ambiguous commit recovery");
  await clickHarness("clearCalls");
  await selectResourceUploadFile();
  await waitFor(() => evaluate(`document.querySelector('.resource-intake-status')?.textContent.includes("lost the commit acknowledgement after commit")`), "committed upload with lost response");
  const committedUploadIdentity = await evaluate(`(() => {
    const call = window.litecoworkHarness.calls().find(item => item.command === "create_resource_upload");
    return call?.args.requestId ?? null;
  })()`);
  const committedRequestIdentity = await evaluate(`(() => {
    const call = window.litecoworkHarness.calls().find(item => item.command === "commit_resource_upload");
    return call?.args.requestId ?? null;
  })()`);
  await selectResourceUploadFile();
  await waitFor(() => evaluate(`/no replacement upload session was started/i.test(document.querySelector('.resource-intake-status')?.textContent ?? "")`), "status-read failure after an ambiguous committed upload");
  let ambiguousCommitCalls = await evaluate(`window.litecoworkHarness.calls()`);
  assert.equal(ambiguousCommitCalls.filter(call => call.command === "create_resource_upload").length, 1, "an unread commit receipt cannot create a duplicate upload while status is unavailable");
  assert.equal(ambiguousCommitCalls.filter(call => call.command === "commit_resource_upload").length, 1, "the ambiguous commit is not retried until its durable session is reread");
  assert.equal(ambiguousCommitCalls.find(call => call.command === "create_resource_upload")?.args.requestId, committedUploadIdentity, "the committed upload's original create RequestId is retained");
  await selectResourceUploadFile();
  await waitFor(() => evaluate(`document.querySelector('.resource-list')?.textContent.includes("f90-ambiguous-ack.txt") === true`), "same committed Resource recovers after upload status reads recover");
  ambiguousCommitCalls = await evaluate(`window.litecoworkHarness.calls()`);
  const commitReplays = ambiguousCommitCalls.filter(call => call.command === "commit_resource_upload");
  assert.equal(ambiguousCommitCalls.filter(call => call.command === "create_resource_upload").length, 1, "committed recovery keeps the original upload session");
  assert.equal(commitReplays.length, 2, "the persisted commit receipt is replayed once after status becomes readable");
  assert.equal(commitReplays[0].args.requestId, committedRequestIdentity, "the first commit uses the persisted RequestId");
  assert.equal(commitReplays[1].args.requestId, committedRequestIdentity, "recovery reuses the exact commit RequestId");
  assert.equal(await evaluate(`document.querySelectorAll('.resource-list li').length`), 1, "the recovered commit is represented by one Resource row");
  checks.push("E02-S03 failed progress reads after a lost commit response preserve the exact committed session and commit RequestId");

  await clickHarness("mountProductionApp", "resource-upload-ambiguous-chunk");
  await waitFor(() => evaluate(`Boolean(document.querySelector('.workspace-switcher'))`), "production App shell for Resource intake recovery");
  await clickButtonContaining(["library"]);
  await waitFor(() => evaluate(`(() => { const inputs = document.querySelectorAll('[aria-label="Add files to this Workspace"] input[type="file"]'); return inputs.length === 2 && inputs[1].disabled === false; })()`), "selected Workspace and enabled file upload input");
  await clickHarness("clearCalls");
  await selectResourceUploadFile();
  await waitFor(() => evaluate(`window.litecoworkHarness.calls().filter(call => call.command === "upload_resource_chunk").length === 1`), "first accepted upload chunk with lost acknowledgement");
  await waitFor(() => evaluate(`document.querySelector('.resource-intake-status')?.textContent.includes("Rendered harness lost the chunk acknowledgement")`), "truthful upload error after ambiguous accepted chunk");
  assert.equal((await evaluate(`window.litecoworkHarness.calls().filter(call => call.command === "commit_resource_upload").length`)), 0, "the UI must not commit while the final chunk is still missing");

  await selectResourceUploadFile();
  await waitFor(() => evaluate(`window.litecoworkHarness.calls().filter(call => call.command === "commit_resource_upload").length === 1`), "single commit after upload recovery");
  const calls = await evaluate(`window.litecoworkHarness.calls()`);
  const chunkCalls = calls.filter(call => call.command === "upload_resource_chunk");
  assert.deepEqual(calls.filter(call => call.command === "create_resource_upload").length, 1, "retry reuses the original upload session");
  assert.deepEqual(calls.filter(call => call.command === "get_resource_upload").length, 1, "retry rereads durable accepted ranges");
  assert.deepEqual(chunkCalls.map(call => call.args.chunkIndex), [0, 1], "retry skips the accepted first chunk and sends only the missing final chunk");
  assert.deepEqual(chunkCalls.map(call => call.args.contentRange), ["bytes 0-4194303/4194305", "bytes 4194304-4194304/4194305"]);
  assert.deepEqual(calls.filter(call => ["create_resource_upload", "upload_resource_chunk", "get_resource_upload", "commit_resource_upload"].includes(call.command)).map(call => call.command), ["create_resource_upload", "upload_resource_chunk", "get_resource_upload", "upload_resource_chunk", "commit_resource_upload"], "commit follows the re-read and acceptance of the only missing chunk");
  assert.equal(await evaluate(`document.querySelector('.resource-list')?.textContent.includes("f90-ambiguous-ack.txt") === true`), true, "the committed Resource appears in the Library only after its receipt");

  await clickHarness("mountProductionApp", "resource-upload-ambiguous-chunk-bad-range");
  await waitFor(() => evaluate(`Boolean(document.querySelector('.workspace-switcher'))`), "production App shell for received-range integrity recovery");
  await clickButtonContaining(["library"]);
  await waitFor(() => evaluate(`(() => { const inputs = document.querySelectorAll('[aria-label="Add files to this Workspace"] input[type="file"]'); return inputs.length === 2 && inputs[1].disabled === false; })()`), "selected Workspace and enabled file upload input for integrity recovery");
  await clickHarness("clearCalls");
  await selectResourceUploadFile();
  await waitFor(() => evaluate(`window.litecoworkHarness.calls().filter(call => call.command === "upload_resource_chunk").length === 1`), "accepted first chunk before malformed range receipt");
  await waitFor(() => evaluate(`document.querySelector('.resource-intake-status')?.textContent.includes("Rendered harness lost the chunk acknowledgement")`), "ambiguous first chunk acknowledgement before range verification");
  await selectResourceUploadFile();
  await waitFor(() => evaluate(`window.litecoworkHarness.calls().filter(call => call.command === "upload_resource_chunk").length === 3`), "fresh upload re-sends a chunk whose returned range hash does not match the selected bytes");
  await waitFor(() => evaluate(`document.querySelector('.resource-list')?.textContent.includes("f90-ambiguous-ack.txt") === true`), "commit after every chunk on the replacement upload is accepted");
  const integrityCalls = await evaluate(`window.litecoworkHarness.calls()`);
  const integrityChunks = integrityCalls.filter(call => call.command === "upload_resource_chunk");
  assert.equal(integrityCalls.filter(call => call.command === "get_resource_upload").length, 1, "retry inspects the accepted range before deciding whether to resume it");
  assert.equal(integrityCalls.filter(call => call.command === "create_resource_upload").length, 2, "a range with an unverifiable byte digest starts a fresh upload session");
  assert.deepEqual(integrityChunks.map(call => call.args.chunkIndex), [0, 0, 1], "the UI retransmits the range whose accepted digest failed verification");
  const integrityLifecycle = integrityCalls.filter(call => ["create_resource_upload", "upload_resource_chunk", "get_resource_upload", "commit_resource_upload"].includes(call.command));
  assert.deepEqual(integrityLifecycle.map(call => call.command), ["create_resource_upload", "upload_resource_chunk", "get_resource_upload", "create_resource_upload", "upload_resource_chunk", "upload_resource_chunk", "commit_resource_upload"], "no commit occurs until the replacement session accepts both chunks");
  checks.push("F90 lost chunk acknowledgement rereads accepted ranges, verifies the reselected bytes, skips the committed chunk, and commits once after the final chunk");

  for (const status of ["FAILED", "EXPIRED", "ELAPSED_OPEN"]) {
    await clickHarness("mountProductionApp", "resource-upload-terminal-explicit-forget");
    await waitFor(() => evaluate(`Boolean(document.querySelector('.workspace-switcher'))`), `production App shell for ${status} upload recovery`);
    await clickButtonContaining(["library"]);
    await waitFor(() => evaluate(`document.querySelectorAll('[aria-label="Add files to this Workspace"] input[type="file"]')[1]?.disabled === false`), `selected Workspace and enabled upload input for ${status} recovery`);
    await clickHarness("clearCalls");
    await selectResourceUploadFile();
    await waitFor(() => evaluate(`document.querySelector('.resource-intake-status')?.textContent.includes("Rendered harness lost the chunk acknowledgement")`), `${status} fixture's accepted chunk acknowledgement loss`);
    await clickHarness("setResourceUploadState", status);
    await selectResourceUploadFile();
    await waitFor(() => evaluate(`Boolean(document.querySelector('[aria-label="Upload recovery needs your review"]'))`), `${status} upload requires explicit local forget`);
    let terminalCalls = await evaluate(`window.litecoworkHarness.calls()`);
    assert.equal(terminalCalls.filter(call => call.command === "create_resource_upload").length, 1, `${status} cannot start a replacement before explicit forget`);
    assert.equal(terminalCalls.filter(call => call.command === "upload_resource_chunk").length, 1, `${status} cannot append bytes before explicit forget`);
    assert.equal(terminalCalls.filter(call => call.command === "commit_resource_upload").length, 0, `${status} cannot be committed after terminal/elapsed status`);
    const originalRequestId = terminalCalls.find(call => call.command === "create_resource_upload")?.args.requestId;
    await clickButton("Forget upload resume details");
    assert.equal(await evaluate(`Object.keys(JSON.parse(localStorage.getItem("litecowork.resourceUploadResume.v1") ?? "{}")).length`), 0, `${status} forget removes the local recovery record`);
    terminalCalls = await evaluate(`window.litecoworkHarness.calls()`);
    assert.equal(terminalCalls.filter(call => call.command === "create_resource_upload").length, 1, `${status} forget itself does not contact the server`);
    assert.equal(await evaluate(`document.querySelector('.resource-intake-status')?.textContent.includes("server-side upload session was not deleted")`), true, `${status} forget truthfully leaves server-side session state unchanged`);
    await selectResourceUploadFile();
    await waitFor(() => evaluate(`document.querySelector('.resource-list')?.textContent.includes("f90-ambiguous-ack.txt") === true`), `${status} replacement after explicit forget`);
    terminalCalls = await evaluate(`window.litecoworkHarness.calls()`);
    const creates = terminalCalls.filter(call => call.command === "create_resource_upload");
    assert.equal(creates.length, 2, `${status} starts one new session only after explicit forget`);
    assert.notEqual(creates[1].args.requestId, originalRequestId, `${status} replacement uses a new create RequestId after explicit forget`);
  }
  checks.push("E02-S03 FAILED, EXPIRED, and elapsed open uploads retain local recovery until an explicit forget action");

  await clickHarness("mountProductionApp", "resource-upload-progress-read-failure");
  await waitFor(() => evaluate(`Boolean(document.querySelector('.workspace-switcher'))`), "production App shell for upload progress-read recovery");
  await clickButtonContaining(["library"]);
  await waitFor(() => evaluate(`document.querySelectorAll('[aria-label="Add files to this Workspace"] input[type="file"]')[1]?.disabled === false`), "selected Workspace and enabled upload input for progress-read recovery");
  await clickHarness("clearCalls");
  await selectResourceUploadFile();
  await waitFor(() => evaluate(`document.querySelector('.resource-intake-status')?.textContent.includes("Rendered harness lost the chunk acknowledgement")`), "ambiguous chunk acknowledgement before progress-read failure");
  const initialUploadIdentity = await evaluate(`(() => {
    const call = window.litecoworkHarness.calls().find(item => item.command === "create_resource_upload");
    return call?.args.requestId ?? null;
  })()`);
  const initialChunkIdentity = await evaluate(`(() => {
    const call = window.litecoworkHarness.calls().find(item => item.command === "upload_resource_chunk");
    return call?.args.requestId ?? null;
  })()`);
  await selectResourceUploadFile();
  await waitFor(() => evaluate(`/no replacement upload session was started/i.test(document.querySelector('.resource-intake-status')?.textContent ?? "")`), "truthful failure when saved upload progress cannot be confirmed");
  const callsAfterProgressFailure = await evaluate(`window.litecoworkHarness.calls()`);
  assert.equal(callsAfterProgressFailure.filter(call => call.command === "create_resource_upload").length, 1, "a failed status read cannot create a replacement upload");
  assert.equal(callsAfterProgressFailure.filter(call => call.command === "upload_resource_chunk").length, 1, "a failed status read cannot send another chunk");
  assert.equal(callsAfterProgressFailure.filter(call => call.command === "commit_resource_upload").length, 0, "a failed status read cannot commit an unverified session");
  assert.equal(callsAfterProgressFailure.find(call => call.command === "create_resource_upload")?.args.requestId, initialUploadIdentity, "the original create RequestId remains the only one used");
  assert.equal(callsAfterProgressFailure.find(call => call.command === "upload_resource_chunk")?.args.requestId, initialChunkIdentity, "the original accepted chunk RequestId remains recorded");
  await selectResourceUploadFile();
  await waitFor(() => evaluate(`document.querySelector('.resource-list')?.textContent.includes("f90-ambiguous-ack.txt") === true`), "same upload resumes after progress reads recover");
  const recoveredCalls = await evaluate(`window.litecoworkHarness.calls()`);
  assert.equal(recoveredCalls.filter(call => call.command === "create_resource_upload").length, 1, "recovery keeps one durable upload session");
  assert.deepEqual(recoveredCalls.filter(call => call.command === "upload_resource_chunk").map(call => call.args.chunkIndex), [0, 1], "recovery verifies the accepted first chunk and sends only the missing final chunk");
  assert.equal(recoveredCalls.filter(call => call.command === "commit_resource_upload").length, 1, "the original upload commits once after recovery");
  checks.push("E02-S03 a failed upload progress read preserves the exact session and chunk identity until the same upload can resume");

  await clickHarness("mountProductionApp", "resource-upload-malformed-progress");
  await waitFor(() => evaluate(`Boolean(document.querySelector('.workspace-switcher'))`), "production App shell for malformed upload progress");
  await clickButtonContaining(["library"]);
  await waitFor(() => evaluate(`document.querySelectorAll('[aria-label="Add files to this Workspace"] input[type="file"]')[1]?.disabled === false`), "selected Workspace and enabled upload input for malformed progress");
  await clickHarness("clearCalls");
  await selectResourceUploadFile();
  await waitFor(() => evaluate(`document.querySelector('.resource-intake-status')?.textContent.includes("Rendered harness lost the chunk acknowledgement")`), "ambiguous chunk acknowledgement before malformed progress");
  await selectResourceUploadFile();
  await waitFor(() => evaluate(`/no replacement upload session was started/i.test(document.querySelector('.resource-intake-status')?.textContent ?? "")`), "malformed upload progress fails closed");
  let malformedCalls = await evaluate(`window.litecoworkHarness.calls()`);
  assert.equal(malformedCalls.filter(call => call.command === "create_resource_upload").length, 1, "a malformed status response cannot replace the saved upload");
  assert.equal(malformedCalls.filter(call => call.command === "upload_resource_chunk").length, 1, "a malformed status response cannot send more bytes");
  assert.equal(malformedCalls.filter(call => call.command === "commit_resource_upload").length, 0, "a malformed status response cannot commit");
  await selectResourceUploadFile();
  await waitFor(() => evaluate(`document.querySelector('.resource-list')?.textContent.includes("f90-ambiguous-ack.txt") === true`), "original upload recovers after malformed status response");
  malformedCalls = await evaluate(`window.litecoworkHarness.calls()`);
  assert.equal(malformedCalls.filter(call => call.command === "create_resource_upload").length, 1, "the original session is resumed after the invalid response clears");
  assert.deepEqual(malformedCalls.filter(call => call.command === "upload_resource_chunk").map(call => call.args.chunkIndex), [0, 1], "only the missing chunk is sent after recovery");
  checks.push("E02-S03 a malformed saved upload status preserves the exact session and sends no replacement mutation");

  await clickHarness("mountProductionApp", "resource-upload-mismatched-progress");
  await waitFor(() => evaluate(`Boolean(document.querySelector('.workspace-switcher'))`), "production App shell for mismatched upload progress");
  await clickButtonContaining(["library"]);
  await waitFor(() => evaluate(`document.querySelectorAll('[aria-label="Add files to this Workspace"] input[type="file"]')[1]?.disabled === false`), "selected Workspace and enabled upload input for mismatched progress");
  await clickHarness("clearCalls");
  await selectResourceUploadFile();
  await waitFor(() => evaluate(`document.querySelector('.resource-intake-status')?.textContent.includes("Rendered harness lost the chunk acknowledgement")`), "ambiguous chunk acknowledgement before mismatched progress");
  await selectResourceUploadFile();
  await waitFor(() => evaluate(`document.querySelector('.resource-intake-status')?.textContent.includes("did not match this Workspace and upload")`), "mismatched upload progress fails closed");
  const mismatchedCalls = await evaluate(`window.litecoworkHarness.calls()`);
  assert.equal(mismatchedCalls.filter(call => call.command === "create_resource_upload").length, 1, "a progress response for another Workspace cannot start a second session");
  assert.equal(mismatchedCalls.filter(call => call.command === "upload_resource_chunk").length, 1, "a progress response for another Workspace cannot send bytes");
  assert.equal(mismatchedCalls.filter(call => call.command === "commit_resource_upload").length, 0, "a mismatched upload response cannot commit");
  checks.push("E02-S03 mismatched upload status scope preserves the saved identity and sends no follow-up mutation");

  await clickHarness("mountProductionApp", "resource-upload-progress-read-deferred");
  await waitFor(() => evaluate(`Boolean(document.querySelector('.workspace-switcher'))`), "production App shell for Workspace-fenced upload status");
  await clickButtonContaining(["library"]);
  await waitFor(() => evaluate(`document.querySelectorAll('[aria-label="Add files to this Workspace"] input[type="file"]')[1]?.disabled === false`), "selected Workspace and enabled upload input for Workspace-fenced status");
  await clickHarness("clearCalls");
  await selectResourceUploadFile();
  await waitFor(() => evaluate(`document.querySelector('.resource-intake-status')?.textContent.includes("Rendered harness lost the chunk acknowledgement")`), "ambiguous chunk acknowledgement before Workspace switch");
  await selectResourceUploadFile();
  await waitFor(() => evaluate(`window.litecoworkHarness.resourceProgressPendingCount() === 1`), "deferred upload progress read");
  await evaluate(`(() => { const select = document.querySelector('select[aria-label="Selected Workspace"]'); const setter = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, "value")?.set; setter?.call(select, "workspace-b"); select?.dispatchEvent(new Event("change", { bubbles: true })); return true; })()`);
  await waitFor(() => evaluate(`document.querySelector('select[aria-label="Selected Workspace"]')?.value === "workspace-b"`), "switch to Workspace B during upload progress read");
  await evaluate(`(() => { const select = document.querySelector('select[aria-label="Selected Workspace"]'); const setter = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, "value")?.set; setter?.call(select, "workspace-a"); select?.dispatchEvent(new Event("change", { bubbles: true })); return true; })()`);
  await waitFor(() => evaluate(`document.querySelector('select[aria-label="Selected Workspace"]')?.value === "workspace-a"`), "return to Workspace A before upload progress rejection");
  await clickHarness("rejectResourceProgressRead");
  await waitFor(() => evaluate(`document.querySelectorAll('[aria-label="Add files to this Workspace"] input[type="file"]')[1]?.disabled === false`), "stale upload status request settles after Workspace round trip");
  assert.equal(await evaluate(`document.querySelector('.resource-intake-status') === null`), true, "A→B→A invalidates the older import's progress and error projection");
  await selectResourceUploadFile();
  await waitFor(() => evaluate(`document.querySelector('.resource-list')?.textContent.includes("f90-ambiguous-ack.txt") === true`), "same upload recovers after a stale status read rejection");
  const workspaceFencedCalls = await evaluate(`window.litecoworkHarness.calls()`);
  assert.equal(workspaceFencedCalls.filter(call => call.command === "create_resource_upload").length, 1, "Workspace switches retain one upload session");
  assert.deepEqual(workspaceFencedCalls.filter(call => call.command === "upload_resource_chunk").map(call => call.args.chunkIndex), [0, 1], "Workspace round trips do not duplicate accepted chunks");
  checks.push("E02-S03 a late progress-read failure cannot repopulate the wrong Workspace after an A→B→A switch");

  return checks;
}

try {
  vite = spawn(process.execPath, [viteCli, "build", "--config", browserViteConfig, "--outDir", renderedOutput, "--emptyOutDir"], {
    cwd: packageRoot,
    stdio: ["ignore", "pipe", "pipe"],
    detached: process.platform !== "win32",
  });
  captureOutput(viteOutput, vite.stdout);
  captureOutput(viteOutput, vite.stderr);
  await requireSuccessfulExit(vite, "Rendered UI harness build");
  vite = spawn(process.execPath, [viteCli, "preview", "--config", browserViteConfig, "--outDir", renderedOutput, "--host", "127.0.0.1", "--port", String(previewPort), "--strictPort"], {
    cwd: packageRoot,
    stdio: ["ignore", "pipe", "pipe"],
    detached: process.platform !== "win32",
  });
  captureOutput(viteOutput, vite.stdout);
  captureOutput(viteOutput, vite.stderr);
  await waitFor(async () => {
    const response = await fetch(`http://127.0.0.1:${previewPort}/tests/browser/harness.html`);
    return response.ok;
  }, "Vite rendered-UI harness");

  browser = spawn(chrome, [
    "--headless=new", "--no-sandbox", "--disable-gpu", "--disable-dev-shm-usage",
    "--disable-extensions", "--disable-background-networking", `--user-data-dir=${chromeProfile}`,
    `--remote-debugging-port=${debugPort}`, "--remote-allow-origins=*", "about:blank",
  ], { stdio: ["ignore", "pipe", "pipe"], detached: process.platform !== "win32" });
  captureOutput(chromeOutput, browser.stdout);
  captureOutput(chromeOutput, browser.stderr);
  await waitFor(async () => (await fetch(`http://127.0.0.1:${debugPort}/json/version`)).ok, "Chrome DevTools Protocol");
  const browserVersion = await (await fetch(`http://127.0.0.1:${debugPort}/json/version`)).json();
  openedTarget = await (await fetch(`http://127.0.0.1:${debugPort}/json/new?about:blank`, { method: "PUT" })).json();
  socket = new WebSocket(openedTarget.webSocketDebuggerUrl);
  socket.addEventListener("message", event => {
    const message = JSON.parse(event.data);
    if (message.id) pendingCommands.get(message.id)?.(message);
    else if (message.method === "Runtime.exceptionThrown") {
      browserDiagnostics.push({ type: "exception", details: message.params.exceptionDetails?.exception?.description ?? message.params.exceptionDetails?.text });
    } else if (message.method === "Network.requestWillBeSent") {
      browserRequestUrls.set(message.params.requestId, message.params.request.url);
    } else if (message.method === "Network.loadingFailed") {
      browserDiagnostics.push({ type: "request-failed", url: browserRequestUrls.get(message.params.requestId) ?? message.params.requestId, error: message.params.errorText, canceled: message.params.canceled });
      browserRequestUrls.delete(message.params.requestId);
    } else if (message.method === "Network.loadingFinished") {
      browserRequestUrls.delete(message.params.requestId);
    } else if (message.method === "Network.responseReceived" && message.params.response.status >= 400) {
      browserDiagnostics.push({ type: "http-error", url: message.params.response.url, status: message.params.response.status });
    } else if (message.method === "Runtime.consoleAPICalled") {
      browserDiagnostics.push({ type: message.params.type, text: message.params.args?.map(argument => argument.value ?? argument.description).join(" ") });
    } else if (message.method === "Page.domContentEventFired" || message.method === "Page.loadEventFired") {
      browserDiagnostics.push({ type: message.method });
    } else if (message.method === "Inspector.targetCrashed" || message.method === "Runtime.executionContextCreated") {
      browserDiagnostics.push({ type: message.method });
    }
  });
  await new Promise((resolveOpen, rejectOpen) => {
    socket.addEventListener("open", resolveOpen, { once: true });
    socket.addEventListener("error", rejectOpen, { once: true });
  });

  await cdp("Page.enable");
  await cdp("Runtime.enable");
  await cdp("Network.enable");
  await cdp("Log.enable");
  await cdp("DOM.enable");
  await cdp("Accessibility.enable");
  await cdp("Page.bringToFront");
  navigationResult = await cdp("Page.navigate", { url: `http://127.0.0.1:${previewPort}/tests/browser/harness.html` });
  await waitFor(() => evaluate("Boolean(document.querySelector('.harness-ready'))"), "rendered React harness", 60_000);

  const checkGroups = new Map([
    ["resource", runResourceChecks],
    ["context-document-deletion", runContextDocumentDeletionChecks],
    ["conversation", runConversationChecks],
    ["shell-reachability", runShellReachabilityChecks],
    ["library-navigation", runLibraryNavigationChecks],
    ["workspace-first-run", runWorkspaceFirstRunChecks],
    ["workspace-app", runWorkspaceAppChecks],
    ["workspace-root-scan", runWorkspaceRootScanChecks],
    ["agent-binding-ui", runAgentBindingUiChecks],
    ["resource-upload", runResourceUploadRecoveryChecks],
  ]);
  const selectedGroup = process.env.LITECOWORK_RENDERED_CHECK_GROUP;
  if (selectedGroup && !checkGroups.has(selectedGroup)) throw new Error(`Unknown rendered check group: ${selectedGroup}`);
  const groups = selectedGroup ? [checkGroups.get(selectedGroup)] : [...checkGroups.values()];
  const checks = [];
  for (const runGroup of groups) checks.push(...await runGroup());
  throwIfInterrupted();
  console.log(JSON.stringify({ browser: browserVersion.Browser, checks }, null, 2));
} catch (error) {
  let pageText = "";
  let pageSnapshot = null;
  let chromeTargets = null;
  try {
    const snapshot = await cdp("Runtime.evaluate", { expression: `(() => { const close = document.querySelector("#main-navigation .sidebar-close"); const active = document.activeElement; return { readyState: document.readyState, hasFocus: document.hasFocus(), url: location.href, title: document.title, body: document.body?.innerText?.slice(-1600) ?? null, root: document.querySelector("#root")?.innerHTML?.slice(0, 1200) ?? null, active: active ? { tag: active.tagName, className: active.className, label: active.getAttribute("aria-label"), html: active.outerHTML.slice(0, 300) } : null, close: close ? { tag: close.tagName, className: close.className, inert: close.closest("[inert]") !== null, display: getComputedStyle(close).display, visibility: getComputedStyle(close).visibility, rects: close.getClientRects().length, disabled: close.disabled, html: close.outerHTML } : null, more: [...document.querySelectorAll(".sidebar-more")].map(item => ({ open: item.open, visibility: getComputedStyle(item.querySelector("summary")).visibility, className: item.className })), focusTrace: (window.__lcFocusTrace ?? []).slice(-20), scripts: [...document.scripts].map(script => ({ src: script.src, type: script.type, ready: script.readyState ?? null })), resources: performance.getEntriesByType("resource").map(entry => ({ name: entry.name, type: entry.initiatorType, duration: Math.round(entry.duration), bytes: entry.transferSize })).slice(-50) }; })()`, returnByValue: true });
    pageSnapshot = snapshot.result?.value ?? snapshot;
    pageText = JSON.stringify(pageSnapshot);
  } catch {}
  try { chromeTargets = await (await fetch(`http://127.0.0.1:${debugPort}/json/list`)).json(); } catch {}
  if (chromeTargets) error.message += `\nChrome targets: ${JSON.stringify(chromeTargets.map(({ id, type, url, title }) => ({ id, type, url, title })))}`;
  const recentFocusKeyTrace = pageSnapshot?.focusTrace?.slice(-12) ?? [];
  failure = new Error(`${error.message}${pageText ? `\nRendered page snapshot: ${pageText.slice(0, 6000)}` : ""}${recentFocusKeyTrace.length ? `\nRecent focus/key events: ${JSON.stringify(recentFocusKeyTrace)}` : ""}${browserRequestUrls.size ? `\nPending browser requests: ${JSON.stringify([...browserRequestUrls.values()])}` : ""}${browserDiagnostics.length ? `\nBrowser diagnostics: ${JSON.stringify(browserDiagnostics.slice(-60))}` : ""}${viteOutput.length ? `\nVite output: ${JSON.stringify(viteOutput.slice(-40))}` : ""}${chromeOutput.length ? `\nChrome output: ${JSON.stringify(chromeOutput.slice(-40))}` : ""}`, { cause: error });
} finally {
  socket?.close();
  await Promise.allSettled([stopProcess(browser), stopProcess(vite)]);
  await rm(userData, { recursive: true, force: true });
}

if (failure) throw failure;
