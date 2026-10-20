import assert from "node:assert/strict";
import { after, test } from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import react from "@vitejs/plugin-react";
import { createServer } from "vite";

const vite = await createServer({
  configFile: false,
  root: new URL("..", import.meta.url).pathname,
  plugins: [react()],
  server: { middlewareMode: true, hmr: false },
  optimizeDeps: { noDiscovery: true, include: [] },
  appType: "custom",
});
const { createAgentQuotaObservationApi, decodeQuotaObservation, isQuotaObservationExpired } = await vite.ssrLoadModule("/src/agents/agent-quota-api.ts");
const { AgentQuotaObservation, QuotaObservationSummary } = await vite.ssrLoadModule("/src/agents/QuotaObservation.tsx");
const { AgentCatalogSettings } = await vite.ssrLoadModule("/src/agents/AgentCatalogSettings.tsx");
const observedInvokes = [];
globalThis.window = { __TAURI_INTERNALS__: { async invoke(command, args) {
  observedInvokes.push({ command, args });
  return null;
} } };
const { desktopAgentQuotaObservationApi } = await vite.ssrLoadModule("/src/agents/desktop-agent-quota-api.ts");
after(async () => vite.close());

const observation = {
  agent_binding_id: "binding-1",
  source: "codex-account-status",
  observed_at: "2026-10-18T12:00:00Z",
  expires_at: "2026-10-18T12:10:00Z",
  state: "LOW",
  remaining_hint: { quantity: 12, unit: "requests", currency: null, confidence: "EXACT" },
  reset_at: "2026-10-18T13:00:00Z",
};

test("quota observations are bound to the requested agent binding and preserve unknown state", () => {
  assert.equal(decodeQuotaObservation(null, "binding-1"), null);
  assert.throws(() => decodeQuotaObservation(observation, "binding-2"), /different agent binding/);
  assert.equal(decodeQuotaObservation({ ...observation, state: "UNKNOWN" }, "binding-1").state, "UNKNOWN");
});

test("desktop usage disclosure calls the fixed Workspace and binding scoped command", async () => {
  const before = observedInvokes.length;
  assert.equal(await desktopAgentQuotaObservationApi("workspace-1").getQuotaObservation("binding-1"), null);
  assert.deepEqual(observedInvokes.slice(before), [{
    command: "get_agent_quota_observation",
    args: { workspaceId: "workspace-1", agentBindingId: "binding-1" },
  }]);
});

test("path-like binding identifiers are rejected before the desktop command", async () => {
  const before = observedInvokes.length;
  await assert.rejects(desktopAgentQuotaObservationApi("workspace-1").getQuotaObservation("../other"), /identifier/i);
  assert.equal(observedInvokes.length, before);
});

test("invalid timestamps or non-finite quantified usage are rejected", () => {
  assert.throws(() => decodeQuotaObservation({ ...observation, observed_at: "tomorrow" }, "binding-1"), /timestamp/);
  assert.throws(() => decodeQuotaObservation({ ...observation, remaining_hint: { ...observation.remaining_hint, quantity: Infinity } }, "binding-1"), /quantity/);
});

test("usage hint confidence is required and follows the served OpenAPI enum", () => {
  const { confidence: _confidence, ...withoutConfidence } = observation.remaining_hint;
  assert.throws(() => decodeQuotaObservation({ ...observation, remaining_hint: withoutConfidence }, "binding-1"), /fields|confidence/);
  assert.throws(() => decodeQuotaObservation({ ...observation, remaining_hint: { ...observation.remaining_hint, confidence: "UNKNOWN" } }, "binding-1"), /confidence/);
  assert.equal(decodeQuotaObservation(observation, "binding-1").remaining_hint.confidence, "EXACT");
});

test("expired observations are not treated as current quota", () => {
  assert.equal(isQuotaObservationExpired(observation, Date.parse("2026-10-18T12:10:00Z")), true);
  const html = renderToStaticMarkup(createElement(QuotaObservationSummary, {
    observation: { ...observation, expires_at: "2026-10-18T12:10:00Z" },
    now: Date.parse("2026-10-18T12:11:00Z"),
  }));
  assert.match(html, /Unknown · expired/);
  assert.doesNotMatch(html, /12 requests/);
  assert.doesNotMatch(html, /progressbar/);
});

test("future observations are marked as future rather than recent", () => {
  const html = renderToStaticMarkup(createElement(QuotaObservationSummary, {
    observation: { ...observation, observed_at: "2026-10-18T12:02:00Z" },
    now: Date.parse("2026-10-18T12:01:00Z"),
  }));
  assert.match(html, /Unknown · future timestamp/);
  assert.match(html, /<dt>Observed<\/dt><dd>future timestamp<\/dd>/);
  assert.doesNotMatch(html, /12 requests/);
});

test("no observation is described as unavailable without rendering a zero or quota meter", () => {
  const html = renderToStaticMarkup(createElement(QuotaObservationSummary, { observation: null, now: Date.now() }));
  assert.match(html, /No qualified usage observation/);
  assert.doesNotMatch(html, /\b0\b/);
  assert.doesNotMatch(html, /progressbar/);
});

test("fresh quantified observations show source, age, state and unit without a percentage", () => {
  const html = renderToStaticMarkup(createElement(QuotaObservationSummary, {
    observation,
    now: Date.parse("2026-10-18T12:01:00Z"),
  }));
  assert.match(html, /codex-account-status/);
  assert.match(html, /12 requests/);
  assert.match(html, /exact/);
  assert.match(html, /Low/);
  assert.doesNotMatch(html, /%|progressbar/);
});

test("estimated remaining usage is labeled as an estimate", () => {
  const html = renderToStaticMarkup(createElement(QuotaObservationSummary, {
    observation: { ...observation, remaining_hint: { ...observation.remaining_hint, confidence: "ESTIMATED" } },
    now: Date.parse("2026-10-18T12:01:00Z"),
  }));
  assert.match(html, /Estimated remaining usage/);
  assert.match(html, /estimate/);
});

test("Workspace binding settings expose an on-demand usage observation disclosure", () => {
  const html = renderToStaticMarkup(createElement(AgentCatalogSettings, {
    workspaceId: "workspace-1",
    workspaceName: "Workspace One",
    workspaceActive: true,
    runtimeReady: true,
    runtimeEnrolled: true,
    selectedDefaultLeadBindingId: null,
    installations: [],
    profiles: [{
      agentProfileId: "agent-profile-1",
      providerKey: "CODEX",
      displayName: "Codex",
      endpoints: [],
      discoveredAt: "2026-10-18T12:00:00Z",
      observations: [],
    }],
    bindings: [{
      agentBindingId: "binding-1",
      workspaceId: "workspace-1",
      agentProfileId: "agent-profile-1",
      runtimeId: "runtime-1",
      endpointSelectionPolicy: {},
      authRef: null,
      configuration: {},
      enabled: true,
      leadEligible: false,
      createdAt: "2026-10-18T12:00:00Z",
      version: 1,
    }],
    onRefresh() {},
    onProbeCodex() {},
    onProbeOpenCode() {},
    onCreateBinding() {},
    onEnableBinding() {},
    onDisableBinding() {},
    onSetDefaultLead() {},
  }));
  assert.match(html, /View usage observation/);
  assert.match(html, /aria-expanded="false"/);
  assert.doesNotMatch(html, /No qualified usage observation/);
});

test("quota observation disclosure stays collapsed until the user opens it", () => {
  let requests = 0;
  const api = createAgentQuotaObservationApi(async () => { requests += 1; return null; });
  const html = renderToStaticMarkup(createElement(AgentQuotaObservation, { api, agentBindingId: "binding-1" }));
  assert.match(html, /aria-expanded="false"/);
  assert.match(html, /View usage observation/);
  assert.equal(requests, 0);
});
