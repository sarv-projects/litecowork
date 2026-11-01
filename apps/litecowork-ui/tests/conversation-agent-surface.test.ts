import assert from "node:assert/strict";
import test from "node:test";
import type { components } from "../src/api/operator-api.ts";
import {
  deriveConversationAgentSurface,
  switchConversationAgent,
  type ConversationComposerState,
  type ConversationNativeToken,
} from "../src/conversations/conversation-agent-surface.ts";

type Descriptor = components["schemas"]["AgentControlDescriptor"];

function descriptor(input: Partial<Descriptor> = {}): Descriptor {
  return {
    descriptor_digest: `sha256:${"a".repeat(64)}`,
    descriptor_api_version: 1,
    registry_agent_id: "agent-a",
    module_id: "agent-a-module",
    module_version: "1.0.0",
    installation: {
      registry_agent_id: "agent-a",
      module_id: "agent-a-module",
      runtime_id: "runtime-a",
      runtime_incarnation_id: "runtime-incarnation-a",
      state: "INSTALLED",
      installed_version: "1.0.0",
      available_version: null,
      installation_source: "NATIVE_MANAGED",
      observed_at: "2026-10-18T12:00:00Z",
      expires_at: "2026-10-18T12:05:00Z",
    },
    auth: { state: "AUTHENTICATED", methods: [], credential_slots: [] },
    configuration: { schema_digest: `sha256:${"b".repeat(64)}`, fields: [], native_config_targets: [] },
    input_surface: {
      text: true,
      images: false,
      files: true,
      resources: false,
      slash_commands: [{ command_id: "review", token: "/review", display_name: "Review" }],
      reference_kinds: [{ reference_kind_id: "file", trigger: "@", display_name: "File", selection_mode: "NATIVE_PICKER" }],
      supports_command_discovery: true,
      supports_reference_discovery: true,
    },
    native_surface: {
      native_config_preserved: true,
      native_extensions_preserved: true,
      native_subagents_preserved: true,
    },
    capabilities: {} as Descriptor["capabilities"],
    observed_at: "2026-10-18T12:00:00Z",
    expires_at: "2026-10-18T12:05:00Z",
    ...input,
  };
}

function state(overrides: Partial<ConversationComposerState> = {}): ConversationComposerState {
  return {
    workspaceId: "workspace-a",
    conversationId: "conversation-a",
    agentBindingId: "binding-a",
    descriptor: descriptor(),
    descriptorObservedAt: "2026-10-18T12:00:00Z",
    enabled: true,
    leadEligible: true,
    draft: "hello",
    attachments: [],
    nativeTokens: [],
    selectedOptions: { model: "provider-a/model-a" },
    ...overrides,
  };
}

function token(overrides: Partial<ConversationNativeToken> = {}): ConversationNativeToken {
  return {
    id: "token-a",
    kind: "SLASH_COMMAND",
    range: [0, 7],
    ownerBindingId: "binding-a",
    descriptorDigest: `sha256:${"a".repeat(64)}`,
    nativeId: "review",
    rawLiteral: "/review",
    status: "VALID",
    ...overrides,
  };
}

test("agent surface uses only the selected descriptor and preserves host/native namespaces", () => {
  const surface = deriveConversationAgentSurface(state(), Date.parse("2026-10-18T12:01:00Z"));
  assert.equal(surface.status, "READY");
  assert.deepEqual(surface.controls.commands.map(command => command.token), ["/review"]);
  assert.deepEqual(surface.controls.references.map(reference => reference.trigger), ["@"]);
  assert.equal(surface.controls.hostActionsAreSeparate, true);
});

test("switching agents keeps text and unsupported attachments visible, clears agent-owned options", () => {
  const before = state({ attachments: [{ id: "image-1", kind: "IMAGE", name: "diagram.png" }] });
  const after = switchConversationAgent(before, {
    workspaceId: "workspace-a",
    conversationId: "conversation-a",
    agentBindingId: "binding-b",
    descriptor: descriptor({
      descriptor_digest: `sha256:${"c".repeat(64)}`,
      input_surface: { ...descriptor().input_surface, files: false, slash_commands: [], reference_kinds: [] },
    }),
    descriptorObservedAt: "2026-10-18T12:00:30Z",
    enabled: true,
    leadEligible: true,
  });
  assert.equal(after.draft, "hello");
  assert.deepEqual(after.attachments, before.attachments);
  assert.deepEqual(after.selectedOptions, {});
  assert.equal(deriveConversationAgentSurface(after, Date.parse("2026-10-18T12:01:00Z")).status, "INCOMPATIBLE_DRAFT");
  assert.deepEqual(after.attachments, [{ id: "image-1", kind: "IMAGE", name: "diagram.png" }]);
});

test("tokens from a prior binding or descriptor block send without deleting literal text", () => {
  const next = switchConversationAgent(state({ nativeTokens: [token()] }), {
    workspaceId: "workspace-a",
    conversationId: "conversation-a",
    agentBindingId: "binding-b",
    descriptor: descriptor({ descriptor_digest: `sha256:${"d".repeat(64)}` }),
    descriptorObservedAt: "2026-10-18T12:00:30Z",
    enabled: true,
    leadEligible: true,
  });
  assert.equal(next.draft, "hello");
  assert.equal(next.nativeTokens[0]?.rawLiteral, "/review");
  assert.equal(next.nativeTokens[0]?.status, "INCOMPATIBLE");
  assert.equal(deriveConversationAgentSurface(next, Date.parse("2026-10-18T12:01:00Z")).status, "INCOMPATIBLE_DRAFT");
});

test("unknown, expired, disabled, and non-lead states never report ready", () => {
  assert.equal(deriveConversationAgentSurface(state({ descriptor: null }), Date.now()).status, "NEEDS_REFRESH");
  assert.equal(deriveConversationAgentSurface(state(), Date.parse("2026-10-18T12:06:00Z")).status, "NEEDS_REFRESH");
  assert.equal(deriveConversationAgentSurface(state({ enabled: false }), Date.parse("2026-10-18T12:01:00Z")).status, "BLOCKED");
  assert.equal(deriveConversationAgentSurface(state({ leadEligible: false }), Date.parse("2026-10-18T12:01:00Z")).status, "BLOCKED");
});


test("an authenticated descriptor cannot make a missing, broken, stale, or mismatched installation ready", () => {
  const now = Date.parse("2026-10-18T12:01:00Z");
  const base = descriptor();
  for (const installation of [
    { ...base.installation, state: "MISSING" as const },
    { ...base.installation, state: "BROKEN" as const },
    { ...base.installation, expires_at: "2026-10-18T12:00:30Z" },
    { ...base.installation, module_id: "another-module" },
    { ...base.installation, registry_agent_id: "another-agent" },
  ]) {
    const surface = deriveConversationAgentSurface(state({ descriptor: { ...base, installation } }), now);
    assert.equal(surface.status, "NEEDS_REFRESH");
  }
});

test("an installed agent with an available update can retain its fresh descriptor until refresh", () => {
  const surface = deriveConversationAgentSurface(state({
    descriptor: { ...descriptor(), installation: { ...descriptor().installation, state: "UPDATE_AVAILABLE" } },
  }), Date.parse("2026-10-18T12:01:00Z"));
  assert.equal(surface.status, "READY");
});


test("refreshing a binding with a new descriptor clears stale native session options", () => {
  const before = state({ selectedOptions: { model: "vendor-a/private-model", effort: "high" } });
  const refreshedDescriptor = descriptor({ descriptor_digest: `sha256:${"e".repeat(64)}` });
  const after = switchConversationAgent(before, {
    workspaceId: "workspace-a",
    conversationId: "conversation-a",
    agentBindingId: "binding-a",
    descriptor: refreshedDescriptor,
    descriptorObservedAt: "2026-10-18T12:00:30Z",
    enabled: true,
    leadEligible: true,
  });
  assert.deepEqual(after.selectedOptions, {});
  assert.equal(after.draft, before.draft);
});


test("native tokens are invalidated when the same command identity changes its native syntax", () => {
  const oldToken = token();
  const refreshedDescriptor = descriptor({
    descriptor_digest: `sha256:${"f".repeat(64)}`,
    input_surface: {
      ...descriptor().input_surface,
      slash_commands: [{ command_id: "review", token: "/inspect", display_name: "Inspect" }],
    },
  });
  const next = switchConversationAgent(state({
    draft: "/review",
    nativeTokens: [{ ...oldToken, descriptorDigest: refreshedDescriptor.descriptor_digest }],
  }), {
    workspaceId: "workspace-a",
    conversationId: "conversation-a",
    agentBindingId: "binding-a",
    descriptor: refreshedDescriptor,
    descriptorObservedAt: "2026-10-18T12:00:30Z",
    enabled: true,
    leadEligible: true,
  });
  assert.equal(next.nativeTokens[0]?.status, "INCOMPATIBLE");
  assert.equal(deriveConversationAgentSurface(next, Date.parse("2026-10-18T12:01:00Z")).status, "INCOMPATIBLE_DRAFT");
});


test("a selected native token must still match its exact draft range", () => {
  const current = state({
    draft: "please run /review now",
    nativeTokens: [{ ...token(), range: [11, 18], rawLiteral: "/review" }],
  });
  assert.equal(deriveConversationAgentSurface(current, Date.parse("2026-10-18T12:01:00Z")).status, "READY");
  const staleRange = { ...current, nativeTokens: [{ ...current.nativeTokens[0]!, range: [0, 7] }] };
  assert.equal(deriveConversationAgentSurface(staleRange, Date.parse("2026-10-18T12:01:00Z")).status, "INCOMPATIBLE_DRAFT");
});
