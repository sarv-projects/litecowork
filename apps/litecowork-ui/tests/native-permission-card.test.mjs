import assert from "node:assert/strict";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import react from "@vitejs/plugin-react";
import { createServer } from "vite";

const vite = await createServer({
  configFile: false,
  root: fileURLToPath(new URL("..", import.meta.url)),
  plugins: [react()],
  server: { middlewareMode: true, hmr: false, ws: false },
  optimizeDeps: { noDiscovery: true, include: [] },
  appType: "custom",
});
const { NativePermissionCard } = await vite.ssrLoadModule("/src/needs-you/NativePermissionCard.tsx");
after(async () => vite.close());

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
