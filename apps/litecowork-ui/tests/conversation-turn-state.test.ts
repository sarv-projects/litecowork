import assert from "node:assert/strict";
import test from "node:test";
import {
  AT_BOTTOM_THRESHOLD_PX,
  contentSignature,
  conversationTitle,
  defaultAgentOptionLabel,
  isAtBottom,
  isSendEligible,
  isTurnOccupyingComposer,
  retryReasonLabel,
  retryTarget,
  scrollFollowDecision,
  stopTarget,
  turnStatusLabel,
} from "../src/conversations/conversation-turn-state.ts";
import { parseConversationSnapshot } from "../src/conversations/conversation-view.ts";

const idle = { activeTurn: null, transientStreaming: false, submitting: false };
const ready = { ...idle, draft: "hello", hasWorkspace: true, operatorReady: true, bindingEligible: true };

test("send is fenced for every nonterminal turn status and the gap after the 202", () => {
  assert.equal(isSendEligible(ready), true);
  for (const status of ["OPEN", "RUNNING", "WAITING_USER", "WAITING_DEPENDENCY", "CANCEL_REQUESTED"]) {
    assert.equal(isSendEligible({ ...ready, activeTurn: { status } }), false, status);
    assert.equal(isTurnOccupyingComposer({ ...idle, activeTurn: { status } }), true, status);
  }
  for (const status of ["COMPLETED", "FAILED", "CANCELLED"]) {
    assert.equal(isSendEligible({ ...ready, activeTurn: { status } }), true, status);
  }
  assert.equal(isSendEligible({ ...ready, submitting: true }), false, "submit in flight");
  assert.equal(isSendEligible({ ...ready, transientStreaming: true }), false, "streamed turn visible");
});

test("send requires a draft, a Workspace, a ready Operator and a dispatch-eligible binding (A-24)", () => {
  assert.equal(isSendEligible({ ...ready, draft: "   " }), false);
  assert.equal(isSendEligible({ ...ready, hasWorkspace: false }), false);
  assert.equal(isSendEligible({ ...ready, operatorReady: false }), false);
  assert.equal(isSendEligible({ ...ready, bindingEligible: false }), false);
});

test("Stop binds to the projected turn id and aggregate version", () => {
  assert.deepEqual(stopTarget({ turn_id: "turn-1", version: 4, status: "RUNNING", retry_ordinal: 0 }), { turnId: "turn-1", expectedVersion: 4 });
  assert.equal(stopTarget({ turn_id: "turn-1", version: 5, status: "WAITING_USER", retry_ordinal: 0 }), null, "waiting-user cancellation is not served yet");
  assert.equal(stopTarget({ turn_id: "turn-1", version: 5, status: "WAITING_DEPENDENCY", retry_ordinal: 0 }), null, "waiting-dependency cancellation is not served yet");
  assert.equal(stopTarget({ turn_id: "turn-1", version: 6, status: "CANCEL_REQUESTED", retry_ordinal: 0 }), null, "already stopping");
  assert.equal(stopTarget({ turn_id: "turn-1", version: 1, status: "OPEN", retry_ordinal: 0 }), null, "nothing to interrupt yet");
  assert.equal(stopTarget(null), null);
  assert.equal(turnStatusLabel("CANCEL_REQUESTED"), "Stopping");
  assert.equal(turnStatusLabel("FAILED"), null);
});

test("Retry appears only for the latest FAILED turn with no turn in flight", () => {
  const failed = { turn_id: "turn-1", version: 4, retry_ordinal: 0, reason_code: "AGENT_TURN_TIMEOUT" };
  assert.deepEqual(retryTarget(null, failed, false), failed);
  assert.equal(retryTarget(null, null, false), null);
  assert.equal(retryTarget({ status: "OPEN" }, failed, false), null, "a retry already reopened the turn");
  assert.equal(retryTarget(null, failed, true), null, "double click while pending");
});

test("retry reasons are fixed safe text and unknown codes never leak raw strings", () => {
  assert.equal(retryReasonLabel("RUNTIME_RESTARTED"), "LiteCowork restarted while the agent was working.");
  assert.equal(retryReasonLabel("SOMETHING_NEW_WITH_SECRET_ses_abc"), "The agent could not finish this turn.");
  assert.equal(retryReasonLabel(null), "The agent could not finish this turn.");
});

test("fallback labels are honest instead of invented model or title strings", () => {
  assert.equal(defaultAgentOptionLabel(null), "No agent selected");
  assert.equal(defaultAgentOptionLabel("Workspace default: OpenCode"), "Workspace default: OpenCode");
  assert.equal(conversationTitle(undefined), "Untitled conversation");
  assert.equal(conversationTitle("Plan"), "Plan");
});

test("autoscroll follows a reader at the bottom and only indicates for a reader in history", () => {
  const bottom = { scrollTop: 900, clientHeight: 100, scrollHeight: 1000 };
  const nearBottom = { scrollTop: 900 - AT_BOTTOM_THRESHOLD_PX, clientHeight: 100, scrollHeight: 1000 };
  const history = { scrollTop: 100, clientHeight: 100, scrollHeight: 1000 };
  assert.equal(isAtBottom(bottom), true);
  assert.equal(isAtBottom(nearBottom), true);
  assert.equal(isAtBottom(history), false);
  assert.equal(scrollFollowDecision({ wasAtBottom: true, hasNewContent: true, selectionActive: false }), "FOLLOW");
  assert.equal(scrollFollowDecision({ wasAtBottom: false, hasNewContent: true, selectionActive: false }), "INDICATE");
  assert.equal(scrollFollowDecision({ wasAtBottom: true, hasNewContent: true, selectionActive: true }), "INDICATE", "never steal a text selection");
  assert.equal(scrollFollowDecision({ wasAtBottom: false, hasNewContent: false, selectionActive: false }), "NONE");
});

test("content signature changes for a new message and for streamed text growth only", () => {
  const base = contentSignature(["a", "b"], 0);
  assert.notEqual(contentSignature(["a", "b", "c"], 0), base);
  assert.notEqual(contentSignature(["a", "b"], 12), base);
  assert.equal(contentSignature(["a", "b"], 0), base);
});

const snapshotPayload = (extra: Record<string, unknown>) => ({
  workspace_id: "workspace-1", conversation_id: "conversation-1", projection_revision: 1,
  stream_cursor: "conversation:conversation-1:1", origin_sequence: "1", items: [], next_cursor: null,
  active_turn: null, ...extra,
});

test("the projected snapshot carries the active turn identity and version for Stop", () => {
  const view = parseConversationSnapshot(snapshotPayload({
    active_turn: { turn_id: "turn-1", version: 3, status: "RUNNING", retry_ordinal: 0, presentation_preference: "RICH" },
  }), "workspace-1", "conversation-1");
  assert.deepEqual(stopTarget(view.active_turn), { turnId: "turn-1", expectedVersion: 3 });
  assert.throws(() => parseConversationSnapshot(snapshotPayload({ active_turn: { status: "RUNNING" } }), "workspace-1", "conversation-1"));
});

test("the projected snapshot carries the retryable FAILED turn with its typed reason", () => {
  const view = parseConversationSnapshot(snapshotPayload({
    retryable_turn: { turn_id: "turn-1", version: 4, retry_ordinal: 0, reason_code: "AGENT_TURN_TIMEOUT" },
  }), "workspace-1", "conversation-1");
  assert.deepEqual(retryTarget(view.active_turn, view.retryable_turn, false), { turn_id: "turn-1", version: 4, retry_ordinal: 0, reason_code: "AGENT_TURN_TIMEOUT" });
  assert.throws(() => parseConversationSnapshot(snapshotPayload({ retryable_turn: { turn_id: "turn-1" } }), "workspace-1", "conversation-1"));
});
