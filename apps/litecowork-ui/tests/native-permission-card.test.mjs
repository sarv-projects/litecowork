import assert from "node:assert/strict";
import { after, test } from "node:test";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { transformWithEsbuild } from "vite";

const projectRoot = fileURLToPath(new URL("..", import.meta.url));
const tempDir = await mkdtemp(join(projectRoot, ".tmp-native-permission-card-"));
after(async () => rm(tempDir, { recursive: true, force: true }));

// This is a static-markup contract test. Remove only the component's stylesheet import so
// Vite does not need to start a server or process CSS; the production build owns CSS coverage.
const componentPath = join(projectRoot, "src/needs-you/NativePermissionCard.tsx");
const componentSource = await readFile(componentPath, "utf8");
assert.ok(componentSource.includes('import "./native-permission.css";'));
const { code } = await transformWithEsbuild(
  componentSource.replace('import "./native-permission.css";', ""),
  "NativePermissionCard.tsx",
  { loader: "tsx", jsx: "automatic" },
);
const compiledPath = join(tempDir, "NativePermissionCard.mjs");
await writeFile(compiledPath, code);
const { NativePermissionCard } = await import(pathToFileURL(compiledPath).href);

const prompt = {
  workspaceId: "workspace-1",
  conversationId: "conversation-1",
  turnId: "turn-1",
  retryOrdinal: 0,
  sequence: 1,
  promptToken: "a".repeat(64),
  state: "PENDING",
  action: "writeFile",
  explanation: "Update the selected file",
  resourceCount: 2,
  resources: ["src/main.rs", "External resource 2"],
  decision: null,
};

function render(overrides = {}) {
  return renderToStaticMarkup(createElement(NativePermissionCard, {
    prompt: { ...prompt, ...overrides },
    conversationTitle: "Build report",
    onDecision() {},
    onRefresh() {},
  }));
}

test("live permission card tells the user the agent is asking and discloses session-wide rejection", () => {
  const html = render();
  assert.match(html, /The agent is asking/);
  assert.match(html, /This is a request from the connected agent\. LiteCowork has not granted it\./);
  assert.match(html, /rejects every other request currently waiting in this agent session/);
  assert.match(html, /src\/main\.rs/);
  assert.match(html, /External resource 2/);
  assert.match(html, /Allow once/);
  assert.match(html, /Reject this and waiting requests/);
  assert.match(html, /<button[^>]*class="native-permission-allow"[^>]*>Allow once<\/button>/);
  assert.match(html, /<button[^>]*class="native-permission-reject"[^>]*>Reject this and waiting requests<\/button>/);
});

test("an uncertain decision stays visible as unconfirmed and cannot be repeated after session end", () => {
  const unknown = render({ state: "OUTCOME_UNKNOWN", decision: "ALLOW_ONCE" });
  assert.match(unknown, /Outcome could not be confirmed/);
  assert.match(unknown, /You chose Allow once; the agent’s result is not confirmed\./);
  assert.match(unknown, /Check live status/);
  assert.match(unknown, /<button[^>]*disabled=""[^>]*>Allow once<\/button>/);

  const ended = render({ state: "SESSION_ENDED", decision: "ALLOW_ONCE" });
  assert.match(ended, /Agent session ended/);
  assert.match(ended, /The agent session ended\. LiteCowork will not send a decision for this request\./);
  assert.match(ended, /You chose Allow once; the agent’s result is not confirmed\./);
  assert.doesNotMatch(ended, /Check live status/);
  assert.match(ended, /<button[^>]*disabled=""[^>]*>Allow once<\/button>/);
});
