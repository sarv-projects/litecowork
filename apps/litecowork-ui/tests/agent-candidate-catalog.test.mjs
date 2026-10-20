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
const { AGENT_CANDIDATES } = await vite.ssrLoadModule("/src/agents/agent-candidate-catalog.ts");
const { AgentCatalogSettings } = await vite.ssrLoadModule("/src/agents/AgentCatalogSettings.tsx");
after(async () => vite.close());

const expectedIds = [
  "claude-code",
  "codex-cli",
  "opencode",
  "cline-cli",
  "gemini-cli",
  "github-copilot-cli",
  "qwen-code",
  "kiro-cli",
  "goose",
  "mistral-vibe",
  "cursor-cli",
  "kimi-code-cli",
  "jetbrains-junie-cli",
  "factory-droid",
  "auggie-cli",
];

function renderCandidateCatalog({ installations = [], profiles = [] } = {}) {
  return renderToStaticMarkup(createElement(AgentCatalogSettings, {
    workspaceId: "workspace-1",
    workspaceName: "Workspace One",
    workspaceActive: true,
    runtimeReady: true,
    runtimeEnrolled: true,
    selectedDefaultLeadBindingId: null,
    installations,
    profiles,
    bindings: [],
    onRefresh() {},
    onProbeCodex() {},
    onProbeOpenCode() {},
    onCreateBinding() {},
    onEnableBinding() {},
    onDisableBinding() {},
    onSetDefaultLead() {},
  }));
}

test("compiled candidate catalog contains all 15 unique stable portfolio IDs", () => {
  assert.deepEqual(AGENT_CANDIDATES.map((candidate) => candidate.id), expectedIds);
  assert.equal(new Set(AGENT_CANDIDATES.map((candidate) => candidate.id)).size, 15);
});

test("installation inventory, profile probes, module implementation, handshake, and qualification stay separate", () => {
  const byId = new Map(AGENT_CANDIDATES.map((candidate) => [candidate.id, candidate]));
  for (const candidate of AGENT_CANDIDATES) {
    assert.equal(candidate.installationAgentId, candidate.id, candidate.id);
    assert.equal(candidate.sessionModuleStatus, "NOT_IMPLEMENTED", candidate.id);
    assert.equal(candidate.sessionHandshakeStatus, "NOT_TESTED", candidate.id);
    assert.equal(candidate.qualifiedSupportStatus, "NOT_QUALIFIED", candidate.id);
  }
  for (const id of ["codex-cli", "opencode"]) {
    assert.equal(byId.get(id).catalogStage, "PROFILE_PROBE_IMPLEMENTED");
  }
  for (const candidate of AGENT_CANDIDATES.filter((entry) => !["codex-cli", "opencode"].includes(entry.id))) {
    assert.equal(candidate.catalogStage, "INSTALLATION_PROBE_IMPLEMENTED", candidate.id);
  }
});

test("candidate display labels profile probes without claiming a Workspace observation or live registry", () => {
  const html = renderCandidateCatalog();

  for (const id of expectedIds) {
    assert.match(html, new RegExp(`data-agent-candidate-id="${id}"`), `${id} remains visible`);
  }
  assert.match(html, /Agent candidates/);
  assert.match(html, /15 curated candidates/);
  assert.match(html, /ACP Registry service unavailable in this view/);
  assert.match(html, /Codex[\s\S]*?Profile probe implemented[\s\S]*?No observation recorded in this Workspace/);
  assert.match(html, /OpenCode[\s\S]*?Profile probe implemented[\s\S]*?No observation recorded in this Workspace/);
  assert.match(html, /Claude Code[\s\S]*?Installation probe implemented/);
  assert.match(html, /<dt>Session module<\/dt><dd>Not implemented<\/dd>/);
  assert.match(html, /<dt>Session handshake<\/dt><dd>Not tested<\/dd>/);
  assert.match(html, /<dt>Qualified support<\/dt><dd>Not qualified<\/dd>/);
  assert.doesNotMatch(html, /registry (?:connected|refreshed)|live registry entries/i);
});

test("a profile row without observations is not mislabeled as observed in this Workspace", () => {
  const html = renderCandidateCatalog({ profiles: [{
    agentProfileId: "profile-codex",
    providerKey: "CODEX",
    displayName: "Codex",
    endpoints: [],
    discoveredAt: "2026-10-16T00:00:00Z",
    observations: [],
  }] });

  assert.match(html, /data-agent-candidate-id="codex-cli"[\s\S]*?<dt>Workspace profile<\/dt><dd>No observation recorded in this Workspace<\/dd>/);
  assert.match(html, /data-agent-candidate-id="opencode"[\s\S]*?<dt>Workspace profile<\/dt><dd>No observation recorded in this Workspace<\/dd>/);
});

test("recorded observation history is separate from current offer readiness", () => {
  const html = renderCandidateCatalog({ profiles: [{
    agentProfileId: "profile-codex",
    providerKey: "CODEX",
    displayName: "Codex",
    endpoints: [],
    discoveredAt: "2026-10-15T00:00:00Z",
    observations: [{
      endpointId: "endpoint-codex",
      runtimeId: "runtime-1",
      runtimeIncarnationId: "incarnation-1",
      compatible: false,
      readiness: "OFFLINE",
      observedAt: "2026-10-15T00:00:00Z",
      offerExpiresAt: "2000-01-01T00:00:00Z",
    }],
  }] });

  assert.match(html, /data-agent-candidate-id="codex-cli"[\s\S]*?<dt>Workspace profile<\/dt><dd>Observation history recorded in this Workspace<\/dd>/);
  assert.match(html, /Offer expired/);
  assert.match(html, /Codex[\s\S]*?Profile probe implemented[\s\S]*?Offer expired/);
  assert.match(html, /data-agent-candidate-id="opencode"[\s\S]*?<dt>Workspace profile<\/dt><dd>No observation recorded in this Workspace<\/dd>/);
});

test("native CLI installation observations appear on their candidate rows without changing support status", () => {
  const html = renderCandidateCatalog({ installations: [
    { agentId: "claude-code", displayName: "Claude Code", protocolCandidate: "CLI", installation: "INSTALLED", version: "2.1.3", authentication: "UNKNOWN", sessionReadiness: "NOT_PROBED" },
    { agentId: "gemini-cli", displayName: "Gemini CLI", protocolCandidate: "CLI", installation: "MISSING", version: null, authentication: "UNKNOWN", sessionReadiness: "NOT_PROBED" },
    { agentId: "qwen-code", displayName: "Qwen Code", protocolCandidate: "CLI", installation: "VERSION_UNAVAILABLE", version: null, authentication: "UNKNOWN", sessionReadiness: "NOT_PROBED" },
    { agentId: "cursor-cli", displayName: "Cursor CLI", protocolCandidate: "CLI", installation: "INSTALLED", version: "1.2.0", authentication: "UNKNOWN", sessionReadiness: "NOT_PROBED" },
    { agentId: "kiro-cli", displayName: "Kiro CLI", protocolCandidate: "CLI", installation: "INSTALLED", version: "1.8.0", authentication: "UNKNOWN", sessionReadiness: "NOT_PROBED" },
    { agentId: "goose", displayName: "Goose", protocolCandidate: "CLI", installation: "MISSING", version: null, authentication: "UNKNOWN", sessionReadiness: "NOT_PROBED" },
    { agentId: "mistral-vibe", displayName: "Mistral Vibe", protocolCandidate: "CLI", installation: "INSTALLED", version: "2.0.0", authentication: "UNKNOWN", sessionReadiness: "NOT_PROBED" },
    { agentId: "jetbrains-junie-cli", displayName: "JetBrains Junie CLI", protocolCandidate: "CLI", installation: "VERSION_UNAVAILABLE", version: null, authentication: "UNKNOWN", sessionReadiness: "NOT_PROBED" },
  ] });

  assert.match(html, /data-agent-candidate-id="claude-code"[\s\S]*?<dt>Installation<\/dt><dd>Installed · 2\.1\.3<\/dd>/);
  assert.match(html, /data-agent-candidate-id="gemini-cli"[\s\S]*?<dt>Installation<\/dt><dd>missing<\/dd>/);
  assert.match(html, /data-agent-candidate-id="qwen-code"[\s\S]*?<dt>Installation<\/dt><dd>version unavailable<\/dd>/);
  assert.match(html, /data-agent-candidate-id="cursor-cli"[\s\S]*?<dt>Installation<\/dt><dd>Installed · 1\.2\.0<\/dd>/);
  assert.match(html, /data-agent-candidate-id="kiro-cli"[\s\S]*?<dt>Installation<\/dt><dd>Installed · 1\.8\.0<\/dd>/);
  assert.match(html, /data-agent-candidate-id="goose"[\s\S]*?<dt>Installation<\/dt><dd>missing<\/dd>/);
  assert.match(html, /data-agent-candidate-id="mistral-vibe"[\s\S]*?<dt>Installation<\/dt><dd>Installed · 2\.0\.0<\/dd>/);
  assert.match(html, /data-agent-candidate-id="jetbrains-junie-cli"[\s\S]*?<dt>Installation<\/dt><dd>version unavailable<\/dd>/);
  for (const id of ["claude-code", "gemini-cli", "qwen-code", "cursor-cli", "kiro-cli", "goose", "mistral-vibe", "jetbrains-junie-cli"]) {
    assert.match(html, new RegExp(`data-agent-candidate-id="${id}"[\\s\\S]*?<dt>Session module<\\/dt><dd>Not implemented<\\/dd>`));
  }
});

test("newly inventoried portfolio candidates stay explicit about probe-only evidence", () => {
  const html = renderCandidateCatalog({ installations: [
    { agentId: "github-copilot-cli", displayName: "GitHub Copilot CLI", protocolCandidate: "CLI", installation: "INSTALLED", version: "1.0.78", authentication: "UNKNOWN", sessionReadiness: "NOT_PROBED" },
    { agentId: "kimi-code-cli", displayName: "Kimi Code CLI", protocolCandidate: "CLI", installation: "MISSING", version: null, authentication: "UNKNOWN", sessionReadiness: "NOT_PROBED" },
    { agentId: "factory-droid", displayName: "Factory Droid", protocolCandidate: "CLI", installation: "INSTALLED", version: "0.151.0", authentication: "UNKNOWN", sessionReadiness: "NOT_PROBED" },
    { agentId: "auggie-cli", displayName: "Auggie CLI", protocolCandidate: "CLI", installation: "VERSION_UNAVAILABLE", version: null, authentication: "UNKNOWN", sessionReadiness: "NOT_PROBED" },
  ] });

  for (const id of ["github-copilot-cli", "kimi-code-cli", "factory-droid", "auggie-cli"]) {
    assert.match(html, new RegExp(`data-agent-candidate-id="${id}"[\\s\\S]*?<dt>Session module<\\/dt><dd>Not implemented<\\/dd>`));
    assert.match(html, new RegExp(`data-agent-candidate-id="${id}"[\\s\\S]*?<dt>Qualified support<\\/dt><dd>Not qualified<\\/dd>`));
  }
  assert.match(html, /GitHub Copilot CLI[\s\S]*?Installed · 1\.0\.78/);
  assert.match(html, /Kimi Code CLI[\s\S]*?missing/);
  assert.match(html, /Factory Droid[\s\S]*?Installed · 0\.151\.0/);
  assert.match(html, /Auggie CLI[\s\S]*?version unavailable/);
});

test("installation state is shown only for agents covered by the current inventory", () => {
  const html = renderCandidateCatalog({ installations: [
    {
      agentId: "codex-cli",
      displayName: "Codex",
      protocolCandidate: "CODEX_APP_SERVER",
      installation: "INSTALLED",
      version: "0.162.0",
      authentication: "UNKNOWN",
      sessionReadiness: "NOT_PROBED",
    },
    {
      agentId: "cline-cli",
      displayName: "Cline CLI",
      protocolCandidate: "ACP",
      installation: "INSTALLED",
      version: "3.0.72",
      authentication: "UNKNOWN",
      sessionReadiness: "NOT_PROBED",
    },
  ] });

  assert.match(html, /<dt>Installation<\/dt><dd>Installed · 0\.162\.0<\/dd>/);
  assert.match(html, /<li data-agent-candidate-id="cline-cli" data-catalog-stage="INSTALLATION_PROBE_IMPLEMENTED">[\s\S]*?<dt>Installation<\/dt><dd>Installed · 3\.0\.72<\/dd>[\s\S]*?<dt>Workspace profile<\/dt><dd>Profile probe not implemented<\/dd>[\s\S]*?<dt>Session module<\/dt><dd>Not implemented<\/dd>[\s\S]*?<dt>Session handshake<\/dt><dd>Not tested<\/dd>[\s\S]*?<dt>Qualified support<\/dt><dd>Not qualified<\/dd>[\s\S]*?<\/li>/);
  assert.match(html, /<strong>Cline CLI<\/strong><span>3\.0\.72 · acp<\/span>[\s\S]*?Auth: unknown · session: not probed/);
  assert.match(html, /<dt>Installation<\/dt><dd>Not checked by the current inventory<\/dd>/);
  assert.match(html, /The local installation inventory covers all 15 curated candidates/);
  assert.match(html, /These observations do not prove sign-in, session readiness, or agent support/);
});

test("local installation inventory labels missing and unavailable rows without claiming support", () => {
  const html = renderCandidateCatalog({ installations: [
    { agentId: "codex-cli", displayName: "Codex", protocolCandidate: "CODEX_APP_SERVER", installation: "MISSING", version: null, authentication: "UNKNOWN", sessionReadiness: "NOT_PROBED" },
    { agentId: "cline-cli", displayName: "Cline CLI", protocolCandidate: "ACP", installation: "VERSION_UNAVAILABLE", version: null, authentication: "UNKNOWN", sessionReadiness: "NOT_PROBED" },
  ] });
  const inventory = html.match(/<section[^>]*aria-labelledby="agent-installed-title"[\s\S]*?<\/section>/)?.[0];
  assert.ok(inventory, "local inventory section is rendered");
  assert.match(inventory, /Local installation inventory/);
  assert.match(inventory, /Rows may be missing or have unavailable versions/);
  assert.match(inventory, /does not prove sign-in, model access, a usable session, or qualified support/);
  assert.match(inventory, /<strong>Codex<\/strong>[\s\S]*?>missing<\/span>/);
  assert.match(inventory, /<strong>Cline CLI<\/strong>[\s\S]*?>version unavailable<\/span>/);
  assert.doesNotMatch(inventory, /Installed on this computer/);
});

test("empty local inventory reports only that no known executable was detected", () => {
  const html = renderCandidateCatalog();
  assert.match(html, /No known executable was detected by the current inventory provider/);
  assert.doesNotMatch(html, /No supported coding-agent executable/);
});
