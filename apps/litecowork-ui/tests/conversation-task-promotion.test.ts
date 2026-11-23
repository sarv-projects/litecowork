import test from "node:test";
import assert from "node:assert/strict";
import { ConversationTaskPromotionCache, findTaskPromotionSource, isKnownNoWriteTaskError, taskPromotionKey, taskPromotionLeads } from "../src/conversations/task-promotion.ts";

function message(overrides: Record<string, unknown> = {}) {
  return {
    message_id: "agent-1",
    role: "AGENT",
    agent_binding_id: "binding-1",
    turn_id: "turn-1",
    turn_status: "COMPLETED",
    created_at: "2026-11-23T10:00:00Z",
    content: [{ kind: "TEXT", text: "Answer" }],
    resource_refs: [],
    rich_presentation: null,
    ...overrides,
  } as const;
}

const bindings = [
  { agentBindingId: "ready", workspaceId: "workspace-1", agentProfileId: "p1", enabled: true, leadEligible: true },
  { agentBindingId: "disabled", workspaceId: "workspace-1", agentProfileId: "p2", enabled: false, leadEligible: true },
  { agentBindingId: "worker-only", workspaceId: "workspace-1", agentProfileId: "p3", enabled: true, leadEligible: false },
  { agentBindingId: "other-workspace", workspaceId: "workspace-2", agentProfileId: "p1", enabled: true, leadEligible: true },
];

test("Task promotion requires the exact completed same-turn USER and AGENT messages", () => {
  const messages = [
    message({ message_id: "user-1", role: "USER", agent_binding_id: null, content: [{ kind: "TEXT", text: "  Please analyze these numbers.  " }], resource_refs: [
      { workspace_id: "workspace-1", resource_id: "pinned", revision_id: "rev-1" },
      { workspace_id: "workspace-1", resource_id: "unresolved", revision_id: null },
    ] }),
    message({ message_id: "agent-1", turn_id: "turn-1" }),
  ];
  const source = findTaskPromotionSource("workspace-1", "conversation-1", messages, "agent-1");
  assert.deepEqual(source, {
    conversationId: "conversation-1",
    turnId: "turn-1",
    userMessageId: "user-1",
    agentMessageId: "agent-1",
    objective: "Please analyze these numbers.",
    pinnedInputs: [{ workspace_id: "workspace-1", resource_id: "pinned", revision_id: "rev-1" }],
    unresolvedInputs: [{ workspace_id: "workspace-1", resource_id: "unresolved", revision_id: null }],
  });
  assert.equal(findTaskPromotionSource("workspace-1", "conversation-1", messages, "user-1"), null);
});

test("Task promotion fails closed for unknown, incomplete, or mismatched turn lineage", () => {
  const user = message({ message_id: "user-1", role: "USER", agent_binding_id: null });
  assert.equal(findTaskPromotionSource("workspace-1", "conversation-1", [user, message({ turn_status: null })], "agent-1"), null);
  assert.equal(findTaskPromotionSource("workspace-1", "conversation-1", [user, message({ turn_id: "other-turn" })], "agent-1"), null);
  assert.equal(findTaskPromotionSource("workspace-1", "conversation-1", [message({ message_id: "user-1", role: "USER", agent_binding_id: null, turn_id: "different-turn" }), message()], "agent-1"), null, "USER and AGENT must share the exact turn id");
  assert.equal(findTaskPromotionSource("workspace-1", "conversation-1", [message({ message_id: "user-1", role: "USER", agent_binding_id: null }), message({ message_id: "user-2", role: "USER", agent_binding_id: null }), message()], "agent-1"), null, "ambiguous duplicate prompts must not be guessed");
  assert.equal(findTaskPromotionSource("workspace-1", "conversation-1", [message({ message_id: "user-1", role: "USER", agent_binding_id: null, content: [{ kind: "RESOURCE" }] }), message()], "agent-1"), null, "an attachment-only prompt cannot supply an editable objective");
});

test("Task promotion stays hidden for every unsettled or non-successful turn status", () => {
  const user = message({ message_id: "user-1", role: "USER", agent_binding_id: null });
  for (const turn_status of ["OPEN", "RUNNING", "WAITING_USER", "WAITING_DEPENDENCY", "CANCEL_REQUESTED", "FAILED", "CANCELLED", null, undefined]) {
    assert.equal(
      findTaskPromotionSource("workspace-1", "conversation-1", [user, message({ turn_status })], "agent-1"),
      null,
      `promotion must not appear for turn status ${String(turn_status)}`,
    );
  }
});

test("Task lead choices use enabled lead-eligible bindings in the exact Workspace", () => {
  const options = taskPromotionLeads("workspace-1", bindings, [
    { agentProfileId: "p1", displayName: "Agent one", providerKey: "NON-OPENCODE" },
  ]);
  assert.deepEqual(options, [{ agentBindingId: "ready", label: "Agent one" }]);
});

test("ambiguous Task saves retain one exact request until success or known pre-commit refusal", () => {
  const cache = new ConversationTaskPromotionCache();
  const source = {
    conversationId: "conversation-1",
    turnId: "turn-1",
    userMessageId: "user-1",
    agentMessageId: "agent-1",
    objective: "Original prompt",
    pinnedInputs: [{ workspace_id: "workspace-1", resource_id: "r1", revision_id: "rev1" }],
    unresolvedInputs: [],
  };
  const key = taskPromotionKey("workspace-1", source);
  const saved = cache.create(key, {
    workspaceId: "workspace-1",
    source,
    objective: source.objective,
    leadAgentBindingId: "binding-1",
    inputRefs: source.pinnedInputs,
  }, () => "request-1");
  assert.equal(cache.get(key), saved);
  const retry = cache.create(key, {
    workspaceId: "workspace-1",
    source,
    objective: "Edited after timeout",
    leadAgentBindingId: "binding-2",
    inputRefs: [],
  }, () => "request-2");
  assert.equal(retry, saved, "retry cannot alter body or idempotency key after an ambiguous outcome");
  assert.equal(isKnownNoWriteTaskError("Agent unavailable [AGENT_UNAVAILABLE; retryable=false; reference=req-1]"), true);
  assert.equal(isKnownNoWriteTaskError("Request invalid [INVALID_ARGUMENT; retryable=false; reference=req-1]"), true);
  assert.equal(isKnownNoWriteTaskError("Agent unavailable [AGENT_UNAVAILABLE; retryable=true; reference=req-1]"), false);
  assert.equal(isKnownNoWriteTaskError("Dependency unavailable [DEPENDENCY_UNAVAILABLE; retryable=false; reference=req-1]"), false);
  assert.equal(isKnownNoWriteTaskError("Task could not be saved"), false);
  assert.equal(cache.clear(key, "wrong-request"), false);
  assert.equal(cache.clear(key, "request-1"), true);
  const afterKnownNoWrite = cache.create(key, {
    workspaceId: "workspace-1",
    source,
    objective: "Revised after definitive refusal",
    leadAgentBindingId: "binding-2",
    inputRefs: [],
  }, () => "request-2");
  assert.equal(afterKnownNoWrite.requestId, "request-2");
  assert.equal(afterKnownNoWrite.objective, "Revised after definitive refusal");
});
