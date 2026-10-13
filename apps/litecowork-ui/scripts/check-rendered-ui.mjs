import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { mkdtemp, rm } from "node:fs/promises";
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
const viteCli = resolve(packageRoot, "node_modules/vite/bin/vite.js");
let vite;
let browser;
let socket;
let failure;
let interruptedSignal;
let nextCommandId = 0;
const pendingCommands = new Map();
const browserDiagnostics = [];

for (const signal of ["SIGINT", "SIGTERM"]) {
  process.once(signal, () => { interruptedSignal = signal; });
}

function throwIfInterrupted() {
  if (interruptedSignal) throw new Error(`Rendered UI check interrupted by ${interruptedSignal}.`);
}

function cdp(method, params = {}) {
  throwIfInterrupted();
  const id = ++nextCommandId;
  const response = new Promise((resolveResponse, rejectResponse) => {
    const timeout = setTimeout(() => {
      pendingCommands.delete(id);
      rejectResponse(new Error(`Chrome DevTools timed out on ${method}`));
    }, 8_000);
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
  assert.equal(await evaluate(expression, `click ${label}`), true, `button ${JSON.stringify(label)} is rendered`);
}

async function clickHarness(method, ...args) {
  const serialized = JSON.stringify(args);
  assert.equal(
    await evaluate(`window.litecoworkHarness.${method}(...${serialized})`, `harness ${method}`),
    true,
    `harness method ${method} is available`,
  );
}

async function pressKey(key, modifiers = 0) {
  const codes = { Tab: 9, Escape: 27, Enter: 13, Space: 32, "Control+a": 65 };
  const base = key.startsWith("Control+") ? key.slice("Control+".length) : key;
  const code = codes[key] ?? codes[base] ?? 0;
  await cdp("Input.dispatchKeyEvent", {
    type: "keyDown", key: key === "Space" ? " " : base, code: key === "Space" ? "Space" : base, modifiers, windowsVirtualKeyCode: code,
  });
  await cdp("Input.dispatchKeyEvent", {
    type: "keyUp", key: key === "Space" ? " " : base, code: key === "Space" ? "Space" : base, modifiers, windowsVirtualKeyCode: code,
  });
}

async function typeTextareaDraft(text) {
  await waitFor(async () => evaluate("document.querySelector('#resource-revision-text')?.value === 'Saved A source text'"), "Resource text editor");
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
  await waitFor(() => evaluate(`Boolean(document.querySelector(${JSON.stringify(selector)}))`), `${selector} input`);
  await evaluate(`document.querySelector(${JSON.stringify(selector)}).focus()`);
  await cdp("Input.insertText", { text });
  await waitFor(() => evaluate(`document.querySelector(${JSON.stringify(selector)})?.value === ${JSON.stringify(text)}`), `${selector} value`);
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

async function enterConversationPage({ workspaceId = "workspace-a", richMode = "success", availability = "AVAILABLE" } = {}) {
  await clickHarness("clearCalls");
  await clickHarness("configureConversation", workspaceId, richMode, availability);
  await waitFor(() => evaluate(`document.querySelector('.conversation-reader-heading h2')?.textContent === ${JSON.stringify(workspaceId === "workspace-a" ? "Conversation A" : "Workspace B conversation")}`), `Conversation page for ${workspaceId}`);
  await waitFor(() => evaluate("document.querySelector('.conversation-message') !== null"), "conversation message");
  await waitFor(() => evaluate("Boolean(document.querySelector('.conversation-list-heading button') && !document.querySelector('.conversation-list-heading button').disabled)"), "Conversation list ready");
  await evaluate("document.querySelector('.conversation-message')?.scrollIntoView({ block: 'center' })");
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

  await clickHarness("resetResource", true);
  await waitFor(() => evaluate("window.litecoworkHarness.resourcePendingCount('resource-a') === 1"), "deferred Resource A detail response");
  await clickHarness("selectResourceB");
  await waitFor(() => evaluate("window.litecoworkHarness.resourcePendingCount('resource-b') === 1"), "deferred Resource B detail response");
  await clickHarness("resolveResource", "resource-b");
  await waitFor(() => evaluate("document.querySelector('.resource-revision-meta')?.textContent.includes('revision-b1')"), "Resource B revision metadata");
  await clickHarness("resolveResource", "resource-a");
  await new Promise(resolveDelay => setTimeout(resolveDelay, 120));
  assert.equal(await evaluate("document.querySelector('#resource-revision-title')?.textContent"), "B.txt");
  assert.equal(await evaluate("document.querySelector('.resource-revision-meta')?.textContent.includes('revision-b1')"), true, "late Resource A detail must not replace B metadata");
  checks.push("A07 late Resource A detail response cannot populate the selected Resource B editor");

  return checks;
}

async function runConversationChecks() {
  const checks = [];

  await enterConversationPage({ richMode: "defer" });
  await waitForRichRequest(1);
  await waitFor(() => evaluate("document.querySelector('[role=status].conversation-rich-status')?.textContent.includes('Loading formatted response')"), "accessible rich loading status");
  await waitFor(async () => (await currentCalls()).some(call => call.command === "subscribe_conversation_stream"), "Conversation live stream subscription");
  await clickHarness("emitReady");
  await emitProjection("AVAILABLE", 2);
  await new Promise(resolveDelay => setTimeout(resolveDelay, 150));
  let calls = await currentCalls();
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
  await emitProjection("POLICY_OMITTED", 3);
  await waitFor(() => evaluate("!document.querySelector('[data-testid=rich-answer]') && document.querySelector('.conversation-rich-status[role=note]')?.textContent.includes('unavailable')"), "policy invalidation fallback");
  assert.equal(await evaluate("document.querySelector('[data-testid=semantic-answer]')?.textContent.includes('Semantic answer remains available.')"), true);
  await clickHarness("setRichMode", "success");
  await emitProjection("AVAILABLE", 4);
  await waitForRichRequest(2, "RichPresentation refetch after availability returns");
  await waitFor(() => evaluate("Boolean(document.querySelector('[data-testid=rich-answer]'))"), "RichPresentation restored after availability returns");
  await emitProjection("INTEGRITY_FAILED", 5);
  await waitFor(() => evaluate("!document.querySelector('[data-testid=rich-answer]') && document.querySelector('.conversation-rich-status[role=note]')?.textContent.includes('unavailable')"), "integrity invalidation fallback");
  assert.equal(await evaluate("document.querySelector('[data-testid=semantic-answer]')?.textContent.includes('Semantic answer remains available.')"), true);
  checks.push("A02 POLICY_OMITTED and INTEGRITY_FAILED evict rendered rich output while preserving semantic text; disclosure works from keyboard");

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

  await enterConversationPage({ richMode: "success", availability: "UNAVAILABLE" });
  await clickHarness("setCreateMode", "defer");
  await typeInputValue("#conversation-title", "A delayed title");
  await clickButton("Create");
  await waitFor(async () => (await currentCalls()).some(call => call.command === "create_conversation"), "delayed Conversation creation");
  const titleBeforeSwitch = await evaluate("document.querySelector('#conversation-title')?.value");
  await clickHarness("switchConversationWorkspaceB");
  await waitFor(() => evaluate("document.querySelector('.conversation-reader-heading h2')?.textContent === 'Workspace B conversation'"), "Workspace B history after switch");
  await typeInputValue("#conversation-title", "Workspace B draft");
  await clickHarness("resolveCreate");
  await new Promise(resolveDelay => setTimeout(resolveDelay, 150));
  assert.equal(titleBeforeSwitch, "A delayed title");
  assert.equal(await evaluate("document.querySelector('#conversation-title')?.value"), "Workspace B draft", "late Workspace A response must not clear the title entered for Workspace B");
  assert.equal(await evaluate("document.querySelector('.conversation-reader-heading h2')?.textContent"), "Workspace B conversation");
  assert.equal(await evaluate("[...document.querySelectorAll('.conversation-list-item')].some(item => item.textContent.includes('A delayed title'))"), false);
  assert.equal(await evaluate("document.querySelector('.conversation-intro')?.textContent.includes('Workspace B')"), true);
  checks.push("A04 late Workspace A creation response cannot replace Workspace B selection, list, title, or reader");

  const commitsBeforeRetry = await evaluate("window.litecoworkHarness.createCommitCount()");
  await enterConversationPage({ richMode: "success", availability: "UNAVAILABLE" });
  await clickHarness("clearCalls");
  await clickHarness("setCreateMode", "lose-response");
  await typeInputValue("#conversation-title", "Lost acknowledgement title");
  await clickButton("Create");
  await waitFor(() => evaluate("document.querySelector('.conversation-error')?.textContent.includes('test lost response')"), "ambiguous Conversation create failure");
  let retryCalls = (await currentCalls()).filter(call => call.command === "create_conversation");
  assert.equal(retryCalls.length, 1);
  const originalRequestId = retryCalls[0].args.requestId;
  await clickHarness("switchConversationWorkspaceB");
  await waitFor(() => evaluate("document.querySelector('.conversation-intro')?.textContent.includes('Workspace B')"), "Workspace B after ambiguous create");
  await clickHarness("switchConversationWorkspaceA");
  await waitFor(() => evaluate("document.querySelector('.conversation-intro')?.textContent.includes('Workspace A')"), "Workspace A after remount");
  await clickHarness("setCreateMode", "success");
  await typeInputValue("#conversation-title", "Lost acknowledgement title");
  await clickButton("Create");
  await waitFor(() => evaluate("document.querySelector('.conversation-reader-heading h2')?.textContent === 'Lost acknowledgement title'"), "replayed Conversation create receipt");
  retryCalls = (await currentCalls()).filter(call => call.command === "create_conversation");
  assert.equal(retryCalls.length, 2);
  assert.equal(retryCalls[1].args.requestId, originalRequestId, "retry after Workspace remount must reuse the ambiguous RequestId");
  assert.equal(await evaluate("window.litecoworkHarness.createCommitCount()"), commitsBeforeRetry + 1, "same-key retry creates one durable record");
  checks.push("A05 lost-response retry after Workspace unmount/remount reuses the same RequestId and one committed record");

  await enterConversationPage({ richMode: "success", availability: "UNAVAILABLE" });
  await clickHarness("configureConversationList", "51");
  await clickHarness("clearCalls");
  await clickButton("Refresh", ".conversation-list-heading");
  await waitFor(async () => (await currentCalls()).some(call => call.command === "list_conversations"), "Conversation refresh invocation");
  await waitFor(() => evaluate("document.querySelectorAll('.conversation-list-item').length === 50"), "first 50 Conversations");
  await clickButton("Load more conversations");
  await waitFor(() => evaluate("[...document.querySelectorAll('.conversation-list-item')].some(item => item.textContent.includes('Conversation 51'))"), "Conversation 51");
  const listCalls = (await currentCalls()).filter(call => call.command === "list_conversations");
  assert.equal(listCalls.length, 2);
  assert.equal(listCalls[1].args.cursor, "older-page", "Tauri bridge receives the opaque next cursor");
  checks.push("A06 mounted Conversation list follows the next cursor and renders item 51");

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

try {
  vite = spawn(process.execPath, [viteCli, "--host", "127.0.0.1", "--port", String(previewPort), "--strictPort"], {
    cwd: packageRoot,
    stdio: "ignore",
    detached: process.platform !== "win32",
  });
  await waitFor(async () => {
    const response = await fetch(`http://127.0.0.1:${previewPort}/tests/browser/harness.html`);
    return response.ok;
  }, "Vite rendered-UI harness");

  browser = spawn(chrome, [
    "--headless=new", "--no-sandbox", "--disable-gpu", "--disable-dev-shm-usage",
    "--disable-extensions", "--disable-background-networking", `--user-data-dir=${userData}`,
    `--remote-debugging-port=${debugPort}`, "--remote-allow-origins=*", "about:blank",
  ], { stdio: "ignore", detached: process.platform !== "win32" });
  await waitFor(async () => (await fetch(`http://127.0.0.1:${debugPort}/json/version`)).ok, "Chrome DevTools Protocol");
  const browserVersion = await (await fetch(`http://127.0.0.1:${debugPort}/json/version`)).json();
  const target = await (await fetch(`http://127.0.0.1:${debugPort}/json/new?about:blank`, { method: "PUT" })).json();
  socket = new WebSocket(target.webSocketDebuggerUrl);
  socket.addEventListener("message", event => {
    const message = JSON.parse(event.data);
    if (message.id) pendingCommands.get(message.id)?.(message);
    else if (message.method === "Runtime.exceptionThrown") {
      browserDiagnostics.push({ type: "exception", details: message.params.exceptionDetails?.exception?.description ?? message.params.exceptionDetails?.text });
    } else if (message.method === "Runtime.consoleAPICalled") {
      browserDiagnostics.push({ type: message.params.type, text: message.params.args?.map(argument => argument.value ?? argument.description).join(" ") });
    }
  });
  await new Promise((resolveOpen, rejectOpen) => {
    socket.addEventListener("open", resolveOpen, { once: true });
    socket.addEventListener("error", rejectOpen, { once: true });
  });

  await cdp("Page.enable");
  await cdp("Runtime.enable");
  await cdp("Log.enable");
  await cdp("DOM.enable");
  await cdp("Accessibility.enable");
  await cdp("Page.navigate", { url: `http://127.0.0.1:${previewPort}/tests/browser/harness.html` });
  await waitFor(() => evaluate("Boolean(document.querySelector('.harness-ready'))"), "rendered React harness");

  const checks = [
    ...await runResourceChecks(),
    ...await runConversationChecks(),
    ...await runLibraryNavigationChecks(),
  ];
  throwIfInterrupted();
  console.log(JSON.stringify({ browser: browserVersion.Browser, checks }, null, 2));
} catch (error) {
  failure = new Error(`${error.message}${browserDiagnostics.length ? `\nBrowser diagnostics: ${JSON.stringify(browserDiagnostics.slice(-40))}` : ""}`, { cause: error });
} finally {
  socket?.close();
  await Promise.allSettled([stopProcess(browser), stopProcess(vite)]);
  await rm(userData, { recursive: true, force: true });
}

if (failure) throw failure;
