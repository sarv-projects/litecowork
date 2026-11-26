import assert from "node:assert/strict";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { createServer, createServerModuleRunnerTransport } from "vite";
import { ModuleRunner } from "vite/module-runner";

const vite = await createServer({
  configFile: false,
  root: fileURLToPath(new URL("..", import.meta.url)),
  // Node-side SSR assertions need TSX conversion, not React Refresh/Babel.
  plugins: [],
  server: { middlewareMode: true, hmr: false },
  optimizeDeps: { noDiscovery: true, include: [] },
  appType: "custom",
});
const transport = createServerModuleRunnerTransport({ channel: vite.environments.ssr.hot });
transport.timeout = 120_000;
const runner = new ModuleRunner({
  transport,
  hmr: false,
  root: vite.config.root,
  sourcemapInterceptor: false,
});
after(async () => {
  await runner.close();
  await vite.close();
});
const { AgentControlDescriptorPanel, UnavailableControlsNotice, DescriptorSummary, isAgentControlDescriptorExpired } = await runner.import("/src/agents/AgentControlDescriptorPanel.tsx");
const {
  parseAgentControlDescriptor,
  parseAgentControlReadError,
  parseAgentAuthChallenge,
  parseNativeConfigOpenReceipt,
} = await runner.import("/src/agents/agent-control-api.ts");

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

test("agent action responses are bounded and correlated to the requested descriptor item", () => {
  assert.deepEqual(parseAgentAuthChallenge({
    kind: "NATIVE_FLOW_STARTED", auth_method_id: "native-login", expires_at: null,
    secret: "must-not-cross-the-desktop-bridge",
  }, "native-login"), {
    kind: "NATIVE_FLOW_STARTED", auth_method_id: "native-login", expires_at: null,
  });
  assert.throws(() => parseAgentAuthChallenge({
    kind: "SECRET_REQUIRED", auth_method_id: "another-method", secret: "hidden",
  }, "native-login"));
  assert.deepEqual(parseNativeConfigOpenReceipt({
    agent_profile_id: "profile-1", target_id: "settings", opened: true, path: "must-not-cross",
  }, "profile-1", "settings"), {
    agent_profile_id: "profile-1", target_id: "settings", opened: true,
  });
  assert.throws(() => parseNativeConfigOpenReceipt({
    agent_profile_id: "profile-1", target_id: "other", opened: true,
  }, "profile-1", "settings"));
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

test("stale descriptors pause declared controls and tell the user to refresh", () => {
  const declared = descriptor();
  declared.auth = { state: "NEEDS_AUTH", account_label: null, credential_slots: [], methods: [
    { auth_method_id: "native-login", display_name: "Native login", kind: "NATIVE_FLOW", requires_restart: false },
  ] };
  declared.configuration.native_config_targets = [{ target_id: "settings", display_name: "Settings", kind: "FILE" }];
  const html = renderToStaticMarkup(createElement(DescriptorSummary, { descriptor: declared, stale: true, actions: { onBeginAuth() {}, onOpenNativeConfig() {} } }));
  assert.match(html, /These controls are stale\. Refresh controls to check them again/);
  assert.match(html, /<button[^>]*disabled[^>]*>Sign in: Native login/);
  assert.match(html, /<button[^>]*disabled[^>]*>Open native config: Settings/);
});

test("descriptor freshness expires exactly at its declared deadline", () => {
  const expiresAt = "2026-10-18T11:00:00Z";
  const deadline = Date.parse(expiresAt);
  assert.equal(isAgentControlDescriptorExpired(expiresAt, deadline - 1), false);
  assert.equal(isAgentControlDescriptorExpired(expiresAt, deadline), true);
  assert.equal(isAgentControlDescriptorExpired("invalid", deadline - 1), true);
});

test("sign-in and native config controls render only when the descriptor declares them", () => {
  const none = renderToStaticMarkup(createElement(DescriptorSummary, { descriptor: descriptor() }));
  assert.match(none, /No sign-in method is declared by this agent module/);
  assert.match(none, /Open native config is unavailable/);
  assert.doesNotMatch(none, /Sign in:/);
  assert.match(none, /native harness state/);
  for (const axis of ["Detection", "Installation", "Configuration", "Authentication", "Readiness"]) {
    assert.match(none, new RegExp(`<b>${axis}</b>`));
  }
  const declared = descriptor();
  declared.auth = { state: "AUTHENTICATING", account_label: null, credential_slots: [], methods: [
    { auth_method_id: "native-login", display_name: "Native login", kind: "NATIVE_FLOW", requires_restart: false },
  ] };
  declared.configuration.native_config_targets = [{ target_id: "settings", display_name: "Settings", kind: "FILE" }];
  const html = renderToStaticMarkup(createElement(DescriptorSummary, { descriptor: declared, actions: { onBeginAuth() {}, onOpenNativeConfig() {} } }));
  assert.match(html, /Authenticating/);
  assert.match(html, /<button[^>]*disabled[^>]*>Authenticating/);
  assert.match(html, /Open native config: Settings/);
});
