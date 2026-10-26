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
const { AgentCatalogSettings } = await vite.ssrLoadModule("/src/agents/AgentCatalogSettings.tsx");
after(async () => vite.close());

function renderOpenCodeCatalog(bindings = [], selectedDefaultLeadBindingId = null) {
  return renderToStaticMarkup(createElement(AgentCatalogSettings, {
    workspaceId: "workspace-1",
    workspaceName: "Workspace One",
    workspaceActive: true,
    runtimeReady: true,
    runtimeEnrolled: true,
    selectedDefaultLeadBindingId,
    installations: [{
      agentId: "opencode",
      displayName: "OpenCode",
      protocolCandidate: "OPENCODE_SERVER",
      installation: "INSTALLED",
      version: "2.0.26",
      authentication: "UNKNOWN",
      sessionReadiness: "NOT_PROBED",
    }],
    profiles: [{
      agentProfileId: "profile-opencode",
      providerKey: "OPENCODE",
      displayName: "OpenCode",
      endpoints: [{
        endpointId: "endpoint-opencode",
        agentProfileId: "profile-opencode",
        protocol: "OPENCODE_SERVER",
        topology: "LOCAL",
        protocolVersion: "2.0.26",
        capabilities: {},
      }],
      discoveredAt: "2026-10-15T00:00:00Z",
      observations: [{
        endpointId: "endpoint-opencode",
        runtimeId: "runtime-1",
        runtimeIncarnationId: "incarnation-1",
        compatible: false,
        readiness: "DEGRADED",
        observedAt: "2026-10-15T00:00:00Z",
        offerExpiresAt: "2099-01-01T00:00:00Z",
        constraints: {
          agent_id: "opencode",
          protocol: "OPENCODE_SERVER",
          protocol_version: "2.0.26",
          probe_readiness: "COMPLETE",
          failure: null,
          reported_provider_status: "OBSERVED",
          reported_connected_provider_ids: [],
          reported_connected_provider_ids_truncated: false,
          reported_provider_ids: ["openai"],
          reported_provider_ids_truncated: false,
          reported_model_catalog: "OBSERVED",
          configured_providers: [{
            id: "openai",
            display_name: "OpenAI",
            models: [{ id: "gpt-safe", display_name: "GPT Safe" }],
          }],
          catalog_truncated: false,
          authentication_observation: "UNKNOWN",
          session_model_selection: "NOT_QUALIFIED",
          inference_access_verified: false,
          host_process_stopped: true,
          writer_quiescence_proven: false,
          profile_admission_supported: false,
        },
      }],
    }],
    bindings,
    onRefresh() {},
    onProbeCodex() {},
    onProbeOpenCode() {},
    onCreateBinding() {},
    onEnableBinding() {},
    onSetDefaultLead() {},
  }));
}

function renderCodexCatalog() {
  return renderToStaticMarkup(createElement(AgentCatalogSettings, {
    workspaceId: "workspace-1",
    workspaceName: "Workspace One",
    workspaceActive: true,
    runtimeReady: true,
    runtimeEnrolled: true,
    selectedDefaultLeadBindingId: null,
    installations: [{
      agentId: "codex",
      displayName: "Codex",
      protocolCandidate: "CODEX_APP_SERVER",
      installation: "INSTALLED",
      version: "0.162.0",
      authentication: "UNKNOWN",
      sessionReadiness: "NOT_PROBED",
    }],
    profiles: [{
      agentProfileId: "profile-codex",
      providerKey: "CODEX",
      displayName: "Codex",
      endpoints: [{
        endpointId: "endpoint-codex",
        agentProfileId: "profile-codex",
        protocol: "CODEX_APP_SERVER",
        topology: "LOCAL",
        protocolVersion: "0.162.0",
        capabilities: {},
      }],
      discoveredAt: "2026-10-15T00:00:00Z",
      observations: [{
        endpointId: "endpoint-codex",
        runtimeId: "runtime-1",
        runtimeIncarnationId: "incarnation-1",
        compatible: true,
        readiness: "STARTABLE",
        observedAt: "2026-10-15T00:00:00Z",
        offerExpiresAt: "2099-01-01T00:00:00Z",
        constraints: {
          protocol: "CODEX_APP_SERVER",
          readiness: "COMPLETE",
          protocol_initialize: "SUPPORTED",
          account_read: "SUPPORTED",
          authentication: "CONFIGURED",
          model_list: "SUPPORTED",
          model_catalog: "OBSERVED",
          listed_model_options: [{ id: "gpt-safe" }],
          session_start: "UNKNOWN",
          host_process_stopped: true,
          writer_quiescence_proven: false,
          conversation_eligibility: {
            status: "NOT_ELIGIBLE",
            diagnostic_code: "CODEX_CONVERSATION_SAFETY_UNQUALIFIED",
            blockers: [
              "RESTRICTED_READ_ROOTS_NOT_QUALIFIED",
              "READ_ONLY_WRITE_BOUNDARY_NOT_QUALIFIED",
              "SHELL_NETWORK_BOUNDARY_NOT_QUALIFIED",
              "NATIVE_MCP_TOOLS_NOT_DISABLED",
              "NATIVE_APP_TOOLS_NOT_DISABLED",
              "NATIVE_PLUGINS_AND_HOOKS_NOT_DISABLED",
            ],
          },
        },
      }],
    }],
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

function openCodeBinding(enabled) {
  return {
    agentBindingId: "binding-opencode",
    workspaceId: "workspace-1",
    agentProfileId: "profile-opencode",
    runtimeId: "runtime-1",
    endpointSelectionPolicy: {},
    authRef: null,
    configuration: {},
    enabled,
    leadEligible: true,
    createdAt: "2026-10-15T00:00:00Z",
    version: 2,
  };
}

test("OpenCode catalog observation does not claim connection, authentication, or lead eligibility", () => {
  const html = renderOpenCodeCatalog();

  assert.match(html, /CLI detected · 2\.0\.26/, "installation inventory remains visible");
  assert.match(html, /Probe completed/, "the probe outcome remains distinct from installation");
  assert.match(html, /Provider catalog response[\s\S]*?Observed/, "a successful provider catalog read is shown as catalog evidence");
  assert.match(html, /Provider connection[\s\S]*?Unknown · not reported by this probe/, "an empty connected-ID list stays unknown");
  assert.match(html, /Authentication[\s\S]*?Unknown/, "the catalog does not imply authentication");
  assert.match(html, /Task lead eligibility[\s\S]*?Not eligible/, "incompatible profile admission is not presented as lead eligible");
  assert.match(html, /OpenAI[\s\S]*?GPT Safe/, "the sanitized provider/model catalog remains visible");
  assert.doesNotMatch(html, /connected-provider list|No connected provider IDs were reported|Connected[”"] is only OpenCode/i);
});

test("binding admission copy describes the current state for enabled and disabled bindings", () => {
  const enabled = renderOpenCodeCatalog([openCodeBinding(true)]);
  assert.match(enabled, /Disabling this binding prevents new Conversation and Task sessions from using this agent\./);
  assert.doesNotMatch(enabled, /New Conversation and Task sessions cannot use this agent\./);

  const disabled = renderOpenCodeCatalog([openCodeBinding(false)]);
  assert.match(disabled, /New Conversation and Task sessions cannot use this agent\./);
  assert.doesNotMatch(disabled, /Disabling this binding prevents new Conversation and Task sessions from using this agent\./);
});

test("Codex offer compatibility, process readiness, and Conversation safety remain independent", () => {
  const html = renderCodexCatalog();

  assert.match(html, /Runtime offer compatibility[\s\S]*?Compatible for Workspace binding/);
  assert.match(html, /Process readiness[\s\S]*?Startable/);
  assert.match(html, /Codex Conversation turns are unavailable on this profile\./);
  assert.match(html, /Conversation turns[\s\S]*?Not eligible · safety controls not qualified/);
  assert.doesNotMatch(html, /The current offer is incompatible for Conversation admission/);
});

test("an incompatible profile explains why binding is unavailable instead of asking for a refresh", () => {
  const html = renderOpenCodeCatalog();

  assert.match(html, /Runtime offer compatibility[\s\S]*?Not compatible for Workspace binding/);
  assert.match(html, /This profile cannot be bound while its Runtime offer is incompatible\./);
  assert.match(html, /Binding unavailable/);
  assert.doesNotMatch(html, /Refresh the offer first/);
});

test("a saved default lead is a preference, not a claim that work can start now", () => {
  const html = renderOpenCodeCatalog([openCodeBinding(true)], "binding-opencode");

  assert.match(html, /OpenCode · lead eligible in saved settings/);
  assert.doesNotMatch(html, /eligible for new work/);
  assert.match(html, /An explicit lead choice takes priority/);
  assert.match(html, /Work started for a Coworker uses that Coworker.s lead setting before this Workspace fallback/);
  assert.match(html, /LiteCowork rechecks a compatible Runtime offer and session-specific authorization before starting work\./);
  assert.match(html, /Enabled · lead eligible by settings/);
});

test("Codex Conversation safety blockers are visible without opening probe details", () => {
  const html = renderCodexCatalog();
  const safetyNote = html.slice(html.indexOf("Codex Conversation turns are unavailable"), html.indexOf("<details class=\"agent-catalog__capabilities agent-catalog__probe-diagnostics\""));

  assert.match(safetyNote, /File reads are not proven to stay within an explicit allowed root\./);
  assert.match(safetyNote, /The read-only write boundary is not qualified\./);
  assert.match(safetyNote, /Shell network access is not proven disabled\./);
  assert.match(safetyNote, /Native MCP tools are not proven disabled or mediated\./);
  assert.match(safetyNote, /Native app tools are not proven disabled or mediated\./);
  assert.match(safetyNote, /Native plugins and hooks are not proven disabled or mediated\./);
});


test("agent profile exposes native-control inspection on demand without probing during initial render", () => {
  const html = renderCodexCatalog();

  assert.match(html, /Inspect native controls/, "profile offers an explicit read-only inspection action");
  assert.match(html, /aria-expanded=\"false\"/, "the inspector starts collapsed");
  assert.doesNotMatch(html, /Checking the local agent adapter/, "initial profile rendering does not issue or display a descriptor read");
});
