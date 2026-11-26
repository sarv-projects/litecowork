import assert from "node:assert/strict";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { createServer } from "vite";

const vite = await createServer({
  configFile: false,
  root: fileURLToPath(new URL("..", import.meta.url)),
  // Static SSR assertions use Vite's built-in TSX transform.
  plugins: [],
  server: { middlewareMode: true, hmr: false },
  optimizeDeps: { noDiscovery: true, include: [] },
  appType: "custom",
});
after(async () => vite.close());
const { AgentCatalogSettings } = await vite.ssrLoadModule("/src/agents/AgentCatalogSettings.tsx");

function render(extra = {}) {
  return renderToStaticMarkup(createElement(AgentCatalogSettings, {
    workspaceId: "workspace-1",
    workspaceName: "Workspace One",
    workspaceActive: true,
    runtimeReady: true,
    runtimeEnrolled: true,
    selectedDefaultLeadBindingId: null,
    installations: [
      { agentId: "codex", displayName: "Codex", protocolCandidate: "CODEX_APP_SERVER", installation: "INSTALLED", version: "1.0.0", authentication: "UNKNOWN", sessionReadiness: "NOT_PROBED" },
      { agentId: "opencode", displayName: "OpenCode", protocolCandidate: "OPENCODE_SERVER", installation: "INSTALLED", version: "2.0.32", authentication: "UNKNOWN", sessionReadiness: "NOT_PROBED" },
    ],
    profiles: [],
    bindings: [],
    onRefresh() {},
    onProbeCodex() {},
    onProbeOpenCode() {},
    onCreateBinding() {},
    onEnableBinding() {},
    onSetDefaultLead() {},
    ...extra,
  }));
}

function probeButtonDisabled(html, controlId) {
  const match = html.match(new RegExp(`<button data-control-id="${controlId}"[^>]*>`));
  assert.ok(match, `${controlId} renders`);
  return /\sdisabled(=""|\s|>)/.test(match[0]);
}

test("a pending probe shows progress and disables every probe control (pending state)", () => {
  const html = render({ busyAction: "probe:OPENCODE" });
  assert.match(html, /Probing…/);
  assert.equal(probeButtonDisabled(html, "agent-probe-opencode"), true);
  assert.equal(probeButtonDisabled(html, "agent-probe-codex"), true);
});

test("an idle catalog offers the probes when the inventory confirms the executables", () => {
  const html = render();
  assert.equal(probeButtonDisabled(html, "agent-probe-opencode"), false);
  assert.doesNotMatch(html, /Probing…/);
});

test("a receipt message renders as a status and an error renders as an alert", () => {
  const receipt = render({ actionMessage: "OpenCode profile probed." });
  assert.match(receipt, /<p class="agent-catalog__success" role="status">OpenCode profile probed\.<\/p>/);
  const failure = render({ actionError: "Probe failed safely." });
  assert.match(failure, /<p class="agent-catalog__error" role="alert">Probe failed safely\.<\/p>/);
  assert.doesNotMatch(failure, /agent-catalog__success/);
});
