import assert from "node:assert/strict";
import { after, test } from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import react from "@vitejs/plugin-react";
import { createServer } from "vite";

const packageRoot = new URL("..", import.meta.url).pathname;
const vite = await createServer({
  configFile: false,
  root: packageRoot,
  plugins: [react()],
  server: { middlewareMode: true, hmr: false, ws: false },
  optimizeDeps: { noDiscovery: true, include: [] },
  appType: "custom",
});
const { AgentCatalogSettings } = await vite.ssrLoadModule("/src/agents/AgentCatalogSettings.tsx");
after(async () => vite.close());

const profiles = [
  { agentProfileId: "profile-primary", providerKey: "PRIMARY", displayName: "Primary Agent", endpoints: [], discoveredAt: "2026-10-15T00:00:00Z", observations: [] },
  { agentProfileId: "profile-secondary", providerKey: "SECONDARY", displayName: "Secondary Agent", endpoints: [], discoveredAt: "2026-10-15T00:00:00Z", observations: [] },
  { agentProfileId: "profile-disabled", providerKey: "DISABLED", displayName: "Disabled Agent", endpoints: [], discoveredAt: "2026-10-15T00:00:00Z", observations: [] },
];

const bindings = [
  {
    agentBindingId: "binding-default",
    workspaceId: "workspace-1",
    agentProfileId: "profile-primary",
    runtimeId: "runtime-1",
    endpointSelectionPolicy: {},
    authRef: null,
    configuration: {},
    enabled: true,
    leadEligible: true,
    createdAt: "2026-10-15T00:00:00Z",
    version: 1,
  },
  {
    agentBindingId: "binding-secondary",
    workspaceId: "workspace-1",
    agentProfileId: "profile-secondary",
    runtimeId: "runtime-1",
    endpointSelectionPolicy: {},
    authRef: null,
    configuration: {},
    enabled: true,
    leadEligible: true,
    createdAt: "2026-10-15T00:00:00Z",
    version: 1,
  },
  {
    agentBindingId: "binding-disabled",
    workspaceId: "workspace-1",
    agentProfileId: "profile-disabled",
    runtimeId: "runtime-1",
    endpointSelectionPolicy: {},
    authRef: null,
    configuration: {},
    enabled: false,
    leadEligible: false,
    createdAt: "2026-10-15T00:00:00Z",
    version: 2,
  },
];

function renderBindings({ busyAction = null, runtimeEnrolled = true } = {}) {
  return renderToStaticMarkup(createElement(AgentCatalogSettings, {
    workspaceId: "workspace-1",
    workspaceName: "Workspace One",
    workspaceActive: true,
    runtimeReady: true,
    runtimeEnrolled,
    selectedDefaultLeadBindingId: "binding-default",
    installations: [],
    profiles,
    bindings,
    busyAction,
    onRefresh() {},
    onProbeCodex() {},
    onProbeOpenCode() {},
    onCreateBinding() {},
    onEnableBinding() {},
    onDisableBinding() {},
    onSetDefaultLead() {},
  }));
}

function bindingRow(html, profileName) {
  const nameIndex = html.indexOf(`<strong>${profileName}</strong>`);
  assert.notEqual(nameIndex, -1, `${profileName} binding is rendered`);
  const rowStart = html.lastIndexOf("<li", nameIndex);
  const rowEnd = html.indexOf("</li>", nameIndex);
  assert.ok(rowStart >= 0 && rowEnd > nameIndex, `${profileName} binding row is bounded`);
  return html.slice(rowStart, rowEnd + "</li>".length);
}

function buttons(markup) {
  return [...markup.matchAll(/<button\b[^>]*>[\s\S]*?<\/button>/g)].map(([html]) => ({
    html,
    label: html.replace(/<[^>]*>/g, "").replaceAll("&amp;", "&").trim(),
  }));
}

test("an enabled non-default binding offers Disable", () => {
  const html = renderBindings();
  const row = bindingRow(html, "Secondary Agent");
  assert.ok(buttons(row).some(button => /^Disable(?:\b|…)/.test(button.label)), "enabled non-default binding exposes Disable");
});

test("the current Workspace default cannot be disabled until the owner explicitly clears it", () => {
  const row = bindingRow(renderBindings(), "Primary Agent");
  assert.equal(buttons(row).some(button => /^Disable(?:\b|…)/.test(button.label)), false, "current default has no Disable action");
  assert.match(row, /clear[^<]*(?:Workspace )?default|default[^<]*clear/i, "the row explains that the Workspace default must be cleared first");
});

test("a disabled binding keeps its explicit Enable action", () => {
  const row = bindingRow(renderBindings(), "Disabled Agent");
  assert.ok(buttons(row).some(button => /^Enable(?:\b|…)/.test(button.label)), "disabled binding can be explicitly re-enabled");
});

test("a pending disable action blocks duplicate clicks", () => {
  const row = bindingRow(renderBindings({ busyAction: "disable:binding-secondary" }), "Secondary Agent");
  const action = buttons(row).find(button => /^(?:Disable|Disabling)(?:\b|…)/.test(button.label));
  assert.ok(action, "the pending binding action remains visibly labeled");
  assert.match(action.html, /\sdisabled(?:="")?(?:\s|>)/, "busy state disables another click");
});

test("Disable explains new session admission and the limit on later Task work", () => {
  const row = bindingRow(renderBindings(), "Secondary Agent");
  assert.match(row, /new conversation and task sessions/i, "copy names the future session-admission effect");
  assert.match(row, /sessions already admitted stay attached/i, "copy preserves admitted session pinning");
  assert.match(row, /later task steps may need another enabled agent/i, "copy does not promise later Task sessions can use the disabled agent");
  assert.doesNotMatch(row, /(?:stop|cancel|terminate) (?:the )?(?:active|current|running) session/i, "Disable is not presented as stopping active work");
});

test("an owner can disable an existing binding while Runtime enrollment is inactive", () => {
  const row = bindingRow(renderBindings({ runtimeEnrolled: false }), "Secondary Agent");
  const disable = buttons(row).find(button => /^Disable(?:\b|…)/.test(button.label));
  assert.ok(disable, "the owner can still revoke future admission without Runtime enrollment");
  assert.doesNotMatch(disable.html, /\sdisabled(?:="")?(?:\s|>)/, "the Disable control is available");
});

test("an owner can clear the Workspace default without Runtime enrollment", () => {
  const html = renderBindings({ runtimeEnrolled: false });
  const clear = buttons(html).find(button => button.label === "Clear Workspace default…");
  assert.ok(clear, "the Workspace policy action remains visible");
  assert.doesNotMatch(clear.html, /\sdisabled(?:="")?(?:\s|>)/, "clearing the default does not require Runtime enrollment");
  const enable = buttons(bindingRow(html, "Disabled Agent")).find(button => button.label === "Enable");
  assert.match(enable.html, /\sdisabled(?:="")?(?:\s|>)/, "enabling a binding still requires Runtime enrollment");
});

test("changing the Workspace default describes a saved preference without claiming admission is ready", () => {
  const html = renderBindings();
  assert.match(html, /An explicit lead choice takes priority/i);
  assert.match(html, /Work started for a Coworker uses that Coworker.s lead setting before this Workspace fallback/i);
  assert.match(html, /LiteCowork uses this Workspace choice only after those higher-priority lead choices/i);
  assert.match(html, /rechecks a compatible Runtime offer and session-specific authorization before starting work/i);
  assert.match(html, /A saved default may remain selected while those checks fail/i);
  assert.doesNotMatch(html, /eligible for new work/i);
});
