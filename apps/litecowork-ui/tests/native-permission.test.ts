import assert from "node:assert/strict";
import test from "node:test";
import {
  applyNativePermissionReceipt,
  endLiveNativePermissions,
  emptyNativePermissionState,
  markNativePermissionOutcomeUnknown,
  parseConversationPermissionSnapshot,
  parseWorkspacePermissionSnapshot,
  reconcileConversationPermissionSnapshot,
  requireNativePermissionSnapshot,
  reduceNativePermissionFrame,
} from "../src/needs-you/native-permission.ts";

const scope = {
  workspaceId: "workspace-1",
  conversationId: "conversation-1",
  turnId: "turn-1",
  retryOrdinal: 0,
};

function frame(sequence: number, extra: Record<string, unknown> = {}) {
  return {
    type: "turn.permission",
    workspace_id: scope.workspaceId,
    conversation_id: scope.conversationId,
    turn_id: scope.turnId,
    retry_ordinal: scope.retryOrdinal,
    sequence,
    prompt_token: "a".repeat(64),
    state: "PENDING",
    action: "writeFile",
    explanation: "Update the selected file",
    resource_count: 1,
    resources: ["Resource 1"],
    decision: null,
    ...extra,
  };
}

function snapshot(sequence: number, state: string, promptToken = "a".repeat(64)) {
  return {
    workspace_id: scope.workspaceId,
    conversation_id: scope.conversationId,
    turn_id: scope.turnId,
    retry_ordinal: scope.retryOrdinal,
    sequence,
    items: [{
      sequence,
      prompt_token: promptToken,
      state,
      action: "writeFile",
      explanation: "Update the selected file",
      resource_count: 1,
      resources: ["Resource 1"],
      decision: state === "PENDING" ? null : "ALLOW_ONCE",
    }],
  };
}

test("native permission frames are strict, scoped, and detect a sequence gap", () => {
  const first = reduceNativePermissionFrame(emptyNativePermissionState, frame(1), scope);
  assert.equal(first.lastSequence, 1);
  assert.equal(Object.values(first.prompts)[0]?.state, "PENDING");
  assert.equal(reduceNativePermissionFrame(first, frame(1), scope), first, "duplicate frames are ignored");
  assert.equal(reduceNativePermissionFrame(first, frame(2, { workspace_id: "foreign" }), scope), first);
  assert.equal(reduceNativePermissionFrame(first, frame(2, { native_session_id: "must-not-escape" }), scope), first);

  const gap = reduceNativePermissionFrame(first, frame(3, { state: "ANSWERED", decision: "ALLOW_ONCE" }), scope);
  assert.equal(gap.needsRefresh, true);
  assert.equal(gap.lastSequence, 1);
  assert.equal(Object.values(gap.prompts)[0]?.state, "PENDING", "a gap cannot overwrite the last confirmed state");
});

test("an exact live snapshot replaces one attempt and an older snapshot cannot roll it back", () => {
  const initial = reduceNativePermissionFrame(emptyNativePermissionState, frame(1), scope);
  const latestSnapshot = parseConversationPermissionSnapshot(snapshot(2, "ANSWERED"), scope);
  assert.ok(latestSnapshot);
  const latest = reconcileConversationPermissionSnapshot(initial, latestSnapshot);
  assert.equal(latest.lastSequence, 2);
  assert.equal(Object.values(latest.prompts)[0]?.state, "ANSWERED");
  assert.equal(Object.values(latest.prompts)[0]?.decision, "ALLOW_ONCE");

  const olderSnapshot = parseConversationPermissionSnapshot(snapshot(1, "PENDING"), scope);
  assert.ok(olderSnapshot);
  assert.equal(reconcileConversationPermissionSnapshot(latest, olderSnapshot), latest);
});

test("reconnect marks the cached attempt stale until an exact live snapshot arrives", () => {
  const initial = reduceNativePermissionFrame(emptyNativePermissionState, frame(1), scope);
  const stale = requireNativePermissionSnapshot(initial);
  assert.equal(stale.needsRefresh, true);
  assert.equal(Object.values(stale.prompts)[0]?.state, "PENDING", "cached state remains visible but callers must disable decisions");
  assert.equal(requireNativePermissionSnapshot(stale), stale, "repeated reconnect signals do not churn state");
  const permissionSnapshot = parseConversationPermissionSnapshot({
    ...snapshot(2, "SESSION_ENDED"),
    items: [{ ...snapshot(2, "SESSION_ENDED").items[0], sequence: 2, decision: null }],
  }, scope);
  assert.ok(permissionSnapshot);
  const reconciled = reconcileConversationPermissionSnapshot(stale, permissionSnapshot);
  assert.equal(reconciled.needsRefresh, false);
  assert.equal(Object.values(reconciled.prompts)[0]?.state, "SESSION_ENDED");
});

test("live permission snapshots reject foreign, malformed, and over-cap responses", () => {
  assert.equal(parseConversationPermissionSnapshot(snapshot(1, "PENDING"), { ...scope, turnId: "other" }), null);
  assert.equal(parseConversationPermissionSnapshot({ ...snapshot(1, "PENDING"), unexpected: true }, scope), null);
  assert.equal(parseConversationPermissionSnapshot({
    ...snapshot(1, "PENDING"),
    items: Array.from({ length: 33 }, (_, index) => ({ ...snapshot(1, "PENDING", String(index + 1).padStart(64, "a")).items[0] })),
  }, scope), null);
});

test("workspace Needs You snapshot keeps its Workspace scope and rejects native identifiers", () => {
  const workspaceSnapshot = {
    workspace_id: scope.workspaceId,
    items: [{
      conversation_id: scope.conversationId,
      conversation_title: "Research",
      turn_id: scope.turnId,
      retry_ordinal: 0,
      sequence: 1,
      prompt: snapshot(1, "PENDING").items[0],
    }],
  };
  assert.equal(parseWorkspacePermissionSnapshot(workspaceSnapshot, "foreign"), null);
  assert.equal(parseWorkspacePermissionSnapshot({
    ...workspaceSnapshot,
    items: [{ ...workspaceSnapshot.items[0], native_session_id: "must-not-escape" }],
  }, scope.workspaceId), null);
  assert.equal(parseWorkspacePermissionSnapshot(workspaceSnapshot, scope.workspaceId)?.items.length, 1);
});

test("reply receipts update only the matching current prompt and sequence", () => {
  const initial = reduceNativePermissionFrame(emptyNativePermissionState, frame(1), scope);
  const answered = applyNativePermissionReceipt(initial, {
    prompt_token: "a".repeat(64),
    state: "ANSWERED",
    decision: "ALLOW_ONCE",
    sequence: 2,
  });
  assert.equal(Object.values(answered.prompts)[0]?.state, "ANSWERED");
  assert.equal(applyNativePermissionReceipt(answered, {
    prompt_token: "a".repeat(64), state: "PENDING", decision: null, sequence: 1,
  }), answered, "an older receipt cannot roll back the stream");
  assert.equal(applyNativePermissionReceipt(answered, {
    prompt_token: "b".repeat(64), state: "ANSWERED", decision: "ALLOW_ONCE", sequence: 3,
  }), answered, "a receipt for another prompt is ignored");
  const gap = applyNativePermissionReceipt(initial, {
    prompt_token: "a".repeat(64), state: "ANSWERED", decision: "ALLOW_ONCE", sequence: 3,
  });
  assert.equal(gap.needsRefresh, true, "a receipt gap triggers an exact snapshot refresh");
});

test("terminal turn and ambiguous reply fail closed without re-enabling a decision", () => {
  const initial = reduceNativePermissionFrame(emptyNativePermissionState, frame(1), scope);
  const uncertain = markNativePermissionOutcomeUnknown(initial, "a".repeat(64), "REJECT_SESSION_PENDING");
  assert.equal(Object.values(uncertain.prompts)[0]?.state, "OUTCOME_UNKNOWN");
  assert.equal(Object.values(uncertain.prompts)[0]?.decision, "REJECT_SESSION_PENDING", "keep the decision as unconfirmed context");
  const ended = endLiveNativePermissions(initial);
  assert.equal(Object.values(ended.prompts)[0]?.state, "SESSION_ENDED");
  const endedUncertain = endLiveNativePermissions(uncertain);
  assert.equal(Object.values(endedUncertain.prompts)[0]?.state, "SESSION_ENDED");
  assert.equal(Object.values(endedUncertain.prompts)[0]?.decision, "REJECT_SESSION_PENDING", "session end must not imply the provider acknowledged it");
  assert.equal(markNativePermissionOutcomeUnknown(endedUncertain, "a".repeat(64), "ALLOW_ONCE"), endedUncertain);
});
