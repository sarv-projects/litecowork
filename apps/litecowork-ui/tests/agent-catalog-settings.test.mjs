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
