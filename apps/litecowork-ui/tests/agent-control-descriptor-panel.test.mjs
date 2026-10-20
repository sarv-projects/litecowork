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
const { AgentControlDescriptorPanel, UnavailableControlsNotice } = await vite.ssrLoadModule("/src/agents/AgentControlDescriptorPanel.tsx");
const { parseAgentControlDescriptor, parseAgentControlReadError } = await vite.ssrLoadModule("/src/agents/agent-control-api.ts");
after(async () => vite.close());

function descriptor() {
  return {
    descriptor_digest: `sha256:${"a".repeat(64)}`,
    descriptor_api_version: 1,
    registry_agent_id: "codex",
    module_id: "codex-app-server",
    module_version: "1.2.3",
    harness_version: null,
    protocol_version: "1",
    installation: {
      registry_agent_id: "codex", module_id: "codex-app-server", runtime_id: "runtime-1",
      runtime_incarnation_id: "incarnation-1", state: "INSTALLED", installed_version: "1.2.3",
      available_version: null, installation_source: null,
      observed_at: "2026-10-18T10:00:00Z", expires_at: "2026-10-18T11:00:00Z",
    },
    auth: { state: "UNKNOWN", account_label: null, methods: [], credential_slots: [] },
    configuration: { schema_digest: `sha256:${"b".repeat(64)}`, fields: [], native_config_targets: [] },
    session_options: { descriptor_digest: `sha256:${"c".repeat(64)}`, options: [] },
    input_surface: {
      text: true, images: false, files: true, resources: true, slash_commands: [], reference_kinds: [],
      supports_command_discovery: false, supports_reference_discovery: false,
    },
    native_surface: { native_config_preserved: true, native_extensions_preserved: true, native_subagents_preserved: false },
    capabilities: {
      protocol: "ACP", protocol_version: "1",
      session: { resume: false, steer: false, interrupt: false, cancel: false, fork: false, model_switch: "UNSUPPORTED" },
      input: { text: true, image: false, file: true, resources: true },
      extension: { mcp_stdio: false, mcp_http: false, skills: false, plugins: false, dynamic_attach: false },
      reporting: { tool_calls: false, plan: false, usage: false, native_subagents: false, approvals: false },
    },
    observed_at: "2026-10-18T10:00:00Z", expires_at: "2026-10-18T11:00:00Z",
  };
}

test("AgentControlDescriptor parser accepts bounded generated contract data", () => {
  assert.equal(parseAgentControlDescriptor(descriptor()).module_id, "codex-app-server");
  assert.throws(() => parseAgentControlDescriptor({ ...descriptor(), descriptor_digest: "bad" }));
  assert.throws(() => parseAgentControlDescriptor({ ...descriptor(), input_surface: { slash_commands: new Array(257) } }));
});

test("descriptor parser rejects malformed or externally resolving native option schemas", () => {
  const option = {
    option_id: "model",
    display_name: "Model",
    semantic_hint: "MODEL",
    value_schema: { type: "string", enum: ["native/model"] },
    mutable_scope: "NEW_SESSION",
  };
  assert.equal(parseAgentControlDescriptor({
    ...descriptor(),
    session_options: { descriptor_digest: `sha256:${"c".repeat(64)}`, options: [option] },
  }).session_options.options[0].option_id, "model");
  for (const invalid of [
    { ...option, mutable_scope: "ARBITRARY" },
    { ...option, value_schema: { $ref: "https://example.invalid/schema" } },
    { ...option, value_schema: { $id: "https://example.invalid/base", $ref: "#/definitions/model" } },
  ]) {
    assert.throws(() => parseAgentControlDescriptor({
      ...descriptor(),
      session_options: { descriptor_digest: `sha256:${"c".repeat(64)}`, options: [invalid] },
    }));
  }
  assert.throws(() => parseAgentControlDescriptor({
    ...descriptor(),
    session_options: { descriptor_digest: "bad", options: [option] },
  }));
});

test("descriptor errors preserve the typed unavailable code and reference", () => {
  const error = parseAgentControlReadError(new Error("unavailable [DEPENDENCY_UNAVAILABLE; retryable=false; reference=ref-123]"));
  assert.equal(error.code, "DEPENDENCY_UNAVAILABLE");
  assert.equal(error.retryable, false);
  assert.equal(error.reference, "ref-123");
});

test("settings panel clearly states that reading controls does not enable send", () => {
  const html = renderToStaticMarkup(createElement(AgentControlDescriptorPanel, {
    workspaceId: "workspace-1", profileId: "profile-1", displayName: "Codex",
    api: { get: async () => descriptor(), refresh: async () => descriptor() },
  }));
  assert.match(html, /Checking the local agent adapter/);
  assert.match(html, /does not start or authorize a conversation/);
  assert.match(html, /Checking…/);
  assert.doesNotMatch(html, /<button[^>]*>Send/);
});

test("typed unavailable state is visible without exposing daemon details", () => {
  const html = renderToStaticMarkup(createElement(UnavailableControlsNotice, {
    message: "LiteCowork has not verified this agent’s commands and input options yet. Try again later.",
    reference: "safe-ref-123",
  }));
  assert.match(html, /Agent controls are not available yet/);
  assert.match(html, /LiteCowork has not verified this agent’s commands and input options/);
  assert.match(html, /safe-ref-123/);
  assert.doesNotMatch(html, /secret|native config|\/home\//i);
});
