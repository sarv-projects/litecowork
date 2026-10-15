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

async function selectResourceUploadFile(fill = 37) {
  assert.equal(await evaluate(`(() => {
    const input = document.querySelectorAll('[aria-label="Add files to this Workspace"] input[type="file"]')[1];
    if (!input || input.disabled) return false;
    const transfer = new DataTransfer();
    const bytes = new Uint8Array(4 * 1024 * 1024 + 1).fill(${fill});
    transfer.items.add(new File([bytes], "f90-ambiguous-ack.txt", { type: "text/plain", lastModified: 1700000000000 }));
    input.files = transfer.files;
    input.dispatchEvent(new Event("change", { bubbles: true }));
    return true;
  })()`), true, "enabled Library input accepts the same 4 MiB + 1 byte test file");
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
    ...await runWorkspaceFirstRunChecks(),
    ...await runWorkspaceAppChecks(),
    ...await runAgentBindingUiChecks(),
    ...await runResourceUploadRecoveryChecks(),
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
