import assert from "node:assert/strict";
import { test } from "node:test";
import { emptyPresentationStream, reducePresentationFrame } from "../src/presentation/presentation-stream.ts";

const workspaceId = "workspace-1";
const conversationId = "conversation-1";
const turnId = "turn-1";
const agentSessionId = "session-1";
const draftId = "draft-1";

function ready() {
  let state = reducePresentationFrame(emptyPresentationStream, {
    type: "stream.ready", workspace_id: workspaceId, projection_version: 1, cursor: "cursor-1", max_frame_bytes: 1024 * 1024,
  });
  return reducePresentationFrame(state, { type: "presentation.snapshot", projection_revision: 1, cursor: "cursor-2", items: [] });
}

function draftFrame(event: string, payload: Record<string, unknown>, sequence: number, overrides: Record<string, unknown> = {}) {
  return {
    type: "rich.draft", workspace_id: workspaceId, conversation_id: conversationId, turn_id: turnId,
    retry_ordinal: 0, agent_session_id: agentSessionId, draft_id: draftId, sequence, event, payload,
    ...overrides,
  };
}

test("rejects Operator stream frame limits above the 1 MiB schema bound", () => {
  const state = reducePresentationFrame(emptyPresentationStream, {
    type: "stream.ready", workspace_id: workspaceId, projection_version: 1, cursor: "cursor-1", max_frame_bytes: 1024 * 1024 + 1,
  });
  assert.equal(state.ready, false);
});

test("rejects turn.delta sequence zero, matching the stream schema", () => {
  const state = reducePresentationFrame(ready(), {
    type: "turn.delta", workspace_id: workspaceId, conversation_id: conversationId, turn_id: turnId, retry_ordinal: 0, sequence: 0, text_delta: "bad",
  });
  assert.deepEqual(state.turns, {});
});

test("accumulates bounded rich draft text and publishes it under its fenced identity", () => {
  let state = ready();
  state = reducePresentationFrame(state, draftFrame("DRAFT_STARTED", {}, 1));
  state = reducePresentationFrame(state, draftFrame("BLOCK_OPENED", { block_id: "answer", index: 0, block_kind: "TEXT_SLICE" }, 2));
  state = reducePresentationFrame(state, draftFrame("TEXT_APPENDED", { block_id: "answer", text: "Hello " }, 3));
  state = reducePresentationFrame(state, draftFrame("TEXT_APPENDED", { block_id: "answer", text: "world" }, 4));
  state = reducePresentationFrame(state, draftFrame("BLOCK_CLOSED", { block_id: "answer" }, 5));
  state = reducePresentationFrame(state, draftFrame("DRAFT_FINALIZING", {}, 6));
  state = reducePresentationFrame(state, draftFrame("DRAFT_PUBLISHED", { presentation_id: "presentation-1" }, 7));

  const draft = state.drafts[JSON.stringify([conversationId, turnId])];
  assert.equal(draft?.status, "PUBLISHED");
  assert.equal(draft?.publishedPresentationId, "presentation-1");
  assert.equal(draft?.blocks[0]?.text, "Hello world");
  assert.equal(draft?.blocks[0]?.closed, true);
});

test("drops old retry/session frames after a newer retry starts", () => {
  let state = ready();
  state = reducePresentationFrame(state, draftFrame("DRAFT_STARTED", {}, 1));
  state = reducePresentationFrame(state, draftFrame("BLOCK_OPENED", { block_id: "old", index: 0, block_kind: "TEXT_SLICE" }, 2));
  state = reducePresentationFrame(state, draftFrame("DRAFT_STARTED", {}, 1, { retry_ordinal: 1, agent_session_id: "session-2", draft_id: "draft-2" }));
  state = reducePresentationFrame(state, draftFrame("TEXT_APPENDED", { block_id: "old", text: "stale" }, 3));
  state = reducePresentationFrame(state, draftFrame("BLOCK_OPENED", { block_id: "new", index: 0, block_kind: "TEXT_SLICE" }, 2, { retry_ordinal: 1, agent_session_id: "session-2", draft_id: "draft-2" }));
  state = reducePresentationFrame(state, draftFrame("TEXT_APPENDED", { block_id: "late", text: "stale" }, 3));

  const draft = state.drafts[JSON.stringify([conversationId, turnId])];
  assert.equal(draft?.retryOrdinal, 1);
  assert.equal(draft?.agentSessionId, "session-2");
  assert.equal(draft?.blocks[0]?.blockId, "new");
  assert.equal(draft?.blocks[0]?.text, "");
});

test("rejects a sequence gap and drops partial rich draft content", () => {
  let state = ready();
  state = reducePresentationFrame(state, draftFrame("DRAFT_STARTED", {}, 1));
  state = reducePresentationFrame(state, draftFrame("BLOCK_OPENED", { block_id: "answer", index: 0, block_kind: "TEXT_SLICE" }, 2));
  state = reducePresentationFrame(state, draftFrame("TEXT_APPENDED", { block_id: "answer", text: "partial" }, 4));
  const draft = state.drafts[JSON.stringify([conversationId, turnId])];
  assert.equal(draft?.status, "RECONNECTING");
  assert.equal(draft?.blocks.length, 0);
});

test("settlement, resync, mismatched workspace, and over-budget text fence drafts", () => {
  let state = ready();
  state = reducePresentationFrame(state, draftFrame("DRAFT_STARTED", {}, 1, { workspace_id: "foreign" }));
  assert.equal(Object.keys(state.drafts).length, 0);

  state = reducePresentationFrame(state, draftFrame("DRAFT_STARTED", {}, 1));
  state = reducePresentationFrame(state, draftFrame("BLOCK_OPENED", { block_id: "answer", index: 0, block_kind: "TEXT_SLICE" }, 2));
  state = reducePresentationFrame(state, draftFrame("TEXT_APPENDED", { block_id: "answer", text: "x".repeat(16_385) }, 3));
  assert.equal(state.drafts[JSON.stringify([conversationId, turnId])]?.status, "FAILED");

  state = ready();
  state = reducePresentationFrame(state, draftFrame("DRAFT_STARTED", {}, 1));
  state = reducePresentationFrame(state, { type: "turn.settled", conversation_id: conversationId, turn_id: turnId, retry_ordinal: 0, committed_message_id: "message-1" });
  assert.equal(Object.keys(state.drafts).length, 0);
  state = reducePresentationFrame(state, draftFrame("DRAFT_STARTED", {}, 1));
  state = reducePresentationFrame(state, { type: "stream.resync_required" });
  assert.equal(Object.keys(state.drafts).length, 0);
});

test("caps retained rich draft bytes across individually valid text frames", () => {
  let state = ready();
  state = reducePresentationFrame(state, draftFrame("DRAFT_STARTED", {}, 1));
  state = reducePresentationFrame(state, draftFrame("BLOCK_OPENED", { block_id: "answer", index: 0, block_kind: "TEXT_SLICE" }, 2));
  for (let index = 0; index < 32; index += 1) {
    state = reducePresentationFrame(state, draftFrame("TEXT_APPENDED", { block_id: "answer", text: "x".repeat(16_384) }, index + 3));
  }
  assert.equal(state.drafts[JSON.stringify([conversationId, turnId])]?.bytes, 512 * 1024);

  state = reducePresentationFrame(state, draftFrame("TEXT_APPENDED", { block_id: "answer", text: "x" }, 35));
  const draft = state.drafts[JSON.stringify([conversationId, turnId])];
  assert.equal(draft?.status, "FAILED");
  assert.equal(draft?.blocks.length, 0);
  assert.equal(draft?.bytes, 0);
});

test("bounds concurrently retained rich drafts", () => {
  let state = ready();
  for (let index = 0; index < 4; index += 1) {
    state = reducePresentationFrame(state, draftFrame("DRAFT_STARTED", {}, 1, {
      conversation_id: `conversation-${index}`,
      turn_id: `turn-${index}`,
      draft_id: `draft-${index}`,
    }));
  }
  assert.equal(Object.keys(state.drafts).length, 4);
  state = reducePresentationFrame(state, draftFrame("DRAFT_STARTED", {}, 1, {
    conversation_id: "conversation-over-cap",
    turn_id: "turn-over-cap",
    draft_id: "draft-over-cap",
  }));
  assert.equal(Object.keys(state.drafts).length, 4);
  assert.equal(state.drafts[JSON.stringify(["conversation-over-cap", "turn-over-cap"])], undefined);
});

test("fences late draft starts after settlement without clearing a newer retry", () => {
  let state = ready();
  state = reducePresentationFrame(state, draftFrame("DRAFT_STARTED", {}, 1));
  state = reducePresentationFrame(state, { type: "turn.settled", conversation_id: conversationId, turn_id: turnId, retry_ordinal: 0 });
  state = reducePresentationFrame(state, draftFrame("DRAFT_STARTED", {}, 1));
  assert.equal(Object.keys(state.drafts).length, 0);

  state = reducePresentationFrame(state, draftFrame("DRAFT_STARTED", {}, 1, { retry_ordinal: 1, agent_session_id: "session-2", draft_id: "draft-2" }));
  state = reducePresentationFrame(state, { type: "turn.settled", conversation_id: conversationId, turn_id: turnId, retry_ordinal: 0 });
  assert.equal(state.drafts[JSON.stringify([conversationId, turnId])]?.retryOrdinal, 1);
});
