import assert from "node:assert/strict";
import { test } from "node:test";
import { emptyPresentationStream, reducePresentationFrame } from "../src/presentation/presentation-stream.ts";

const workspaceId = "workspace-1";
const conversationId = "conversation-1";
const turnId = "turn-1";
const agentSessionId = "session-1";
const draftId = "draft-1";
const textItem = {
  item_key: "message-1",
  kind: "TEXT",
  payload_version: 1,
  source_refs: [{ kind: "MESSAGE", id: "message-1" }],
  order_key: "001",
  freshness: "CURRENT",
  payload: { text: "Hello" },
};

function messageWrapper(messageId: string, role: "USER" | "AGENT", content: string, linkedItems: unknown[] = []) {
  return {
    message: {
      message_id: messageId,
      conversation_id: conversationId,
      author: { principal_id: `${role.toLowerCase()}-1`, kind: role },
      role,
      content: [{ kind: "TEXT", text: content }],
      created_at: "2026-10-11T12:00:00Z",
    },
    rich_presentation: null,
    linked_items: linkedItems,
  };
}

function conversationPayload(items: unknown[] = [], overrides: Record<string, unknown> = {}) {
  return {
    conversation_id: conversationId,
    stream_cursor: "conversation-history-1",
    origin_sequence: "1",
    items,
    next_cursor: null,
    active_turn: null,
    ...overrides,
  };
}

function ready() {
  let state = reducePresentationFrame(emptyPresentationStream, {
    type: "stream.ready", workspace_id: workspaceId, projection_version: 1, cursor: "cursor-1", max_frame_bytes: 1024 * 1024,
  });
  return reducePresentationFrame(state, {
    type: "projection", workspace_id: workspaceId, projection_type: "conversation_presentation",
    projection_revision: 1, cursor: "cursor-2", payload: conversationPayload(),
  });
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

test("rejects stream.ready projection versions below the schema minimum", () => {
  const state = reducePresentationFrame(emptyPresentationStream, {
    type: "stream.ready", workspace_id: workspaceId, projection_version: 0, cursor: "cursor-1", max_frame_bytes: 1024,
  });
  assert.equal(state.ready, false);
});

test("rejects stream.ready frames with fields outside the wire schema", () => {
  const state = reducePresentationFrame(emptyPresentationStream, {
    type: "stream.ready", workspace_id: workspaceId, projection_version: 1, cursor: "cursor-1", max_frame_bytes: 1024,
    authorization: "must-not-be-accepted",
  });
  assert.equal(state.ready, false);
});

test("does not accept stream.ready from a Workspace outside the selected stream", () => {
  const selected = { workspaceId, conversationId };
  const state = reducePresentationFrame(emptyPresentationStream, {
    type: "stream.ready", workspace_id: "workspace-foreign", projection_version: 1, cursor: "foreign-cursor", max_frame_bytes: 1024,
  }, selected);
  assert.equal(state, emptyPresentationStream);
  assert.equal(state.ready, false);
});

test("accepts resync only for the selected Workspace with the required reason", () => {
  const initial = ready();
  const missingReason = reducePresentationFrame(initial, {
    type: "stream.resync_required", workspace_id: workspaceId,
  });
  assert.equal(missingReason.resyncRequired, false);

  const foreignWorkspace = reducePresentationFrame(initial, {
    type: "stream.resync_required", workspace_id: "workspace-foreign", reason: "CURSOR_EXPIRED",
  });
  assert.equal(foreignWorkspace.resyncRequired, false);

  const validResync = reducePresentationFrame(initial, {
    type: "stream.resync_required", workspace_id: workspaceId, reason: "CURSOR_EXPIRED",
  });
  assert.equal(validResync.resyncRequired, true);
  assert.equal(validResync.cursor, null);
  assert.deepEqual(validResync.items, []);
});

test("accepts the fresh snapshot after a resync marker on the same stream session", () => {
  const resync = reducePresentationFrame(ready(), {
    type: "stream.resync_required", workspace_id: workspaceId, reason: "CURSOR_EXPIRED",
  });
  const recovered = reducePresentationFrame(resync, {
    type: "projection", workspace_id: workspaceId, projection_type: "conversation_presentation",
    projection_revision: 2, cursor: "cursor-fresh", payload: conversationPayload([
      messageWrapper("message-fresh", "AGENT", "Fresh authorized snapshot"),
    ]),
  });
  assert.equal(recovered.resyncRequired, false);
  assert.equal(recovered.snapshotReady, true);
  assert.equal(recovered.cursor, "cursor-fresh");
  assert.equal(recovered.conversationSnapshot?.messages[0]?.message_id, "message-fresh");
});

test("ignores resync frames with fields outside the wire schema", () => {
  const initial = ready();
  const state = reducePresentationFrame(initial, {
    type: "stream.resync_required", workspace_id: workspaceId, reason: "CURSOR_EXPIRED", internal: true,
  });
  assert.equal(state.resyncRequired, false);
  assert.equal(state.cursor, initial.cursor);
});

test("rejects turn.delta sequence zero, matching the stream schema", () => {
  const state = reducePresentationFrame(ready(), {
    type: "turn.delta", workspace_id: workspaceId, conversation_id: conversationId, turn_id: turnId, retry_ordinal: 0, sequence: 0, text_delta: "bad",
  });
  assert.deepEqual(state.turns, {});
});

test("accepts the canonical conversation projection snapshot frame", () => {
  const initial = reducePresentationFrame(emptyPresentationStream, {
    type: "stream.ready", workspace_id: workspaceId, projection_version: 1, cursor: "cursor-1", max_frame_bytes: 1024,
  });
  const state = reducePresentationFrame(initial, {
    type: "projection", workspace_id: workspaceId, projection_type: "conversation_presentation",
    projection_revision: 1, cursor: "cursor-2", payload: conversationPayload(),
  });
  assert.equal(state.snapshotReady, true);
  assert.equal(state.projectionRevision, 1);
  assert.equal(state.cursor, "cursor-2");
});

test("preserves USER and AGENT message wrappers in the conversation snapshot view", () => {
  const initial = reducePresentationFrame(emptyPresentationStream, {
    type: "stream.ready", workspace_id: workspaceId, projection_version: 1, cursor: "cursor-1", max_frame_bytes: 1024,
  });
  const state = reducePresentationFrame(initial, {
    type: "projection", workspace_id: workspaceId, projection_type: "conversation_presentation",
    projection_revision: 2, cursor: "cursor-2",
    payload: conversationPayload([
      messageWrapper("message-user", "USER", "Question"),
      messageWrapper("message-agent", "AGENT", "Answer", [textItem]),
    ], { stream_cursor: "conversation-history-2", origin_sequence: "9007199254740993", next_cursor: "message-agent" }),
  });

  assert.equal(state.conversationSnapshot?.conversation_id, conversationId);
  assert.deepEqual(state.conversationSnapshot?.messages.map(item => item.role), ["USER", "AGENT"]);
  assert.equal(state.conversationSnapshot?.messages[1]?.content[0]?.kind, "TEXT");
  assert.equal(state.conversationSnapshot?.origin_sequence, "9007199254740993");
  assert.equal(state.conversationSnapshot?.stream_cursor, "conversation-history-2");
  assert.equal(state.conversationSnapshot?.next_cursor, "message-agent");
  assert.deepEqual(state.items.map(item => item.item_key), ["message-1"]);
  assert.equal(state.cursor, "cursor-2");
});

test("rejects malformed conversation wrappers without advancing the cursor", () => {
  const initial = reducePresentationFrame(emptyPresentationStream, {
    type: "stream.ready", workspace_id: workspaceId, projection_version: 1, cursor: "cursor-1", max_frame_bytes: 1024,
  });
  const malformed = reducePresentationFrame(initial, {
    type: "projection", workspace_id: workspaceId, projection_type: "conversation_presentation",
    projection_revision: 2, cursor: "cursor-malformed",
    payload: conversationPayload([{ message: { message_id: "message-1" }, rich_presentation: null, linked_items: [] }], { origin_sequence: "2" }),
  });

  assert.equal(malformed.conversationSnapshot, null);
  assert.equal(malformed.snapshotReady, false);
  assert.equal(malformed.cursor, "cursor-1");
});

test("rejects a conversation snapshot from another Workspace without advancing the cursor", () => {
  const initial = reducePresentationFrame(emptyPresentationStream, {
    type: "stream.ready", workspace_id: workspaceId, projection_version: 1, cursor: "cursor-1", max_frame_bytes: 1024,
  });
  const foreign = reducePresentationFrame(initial, {
    type: "projection", workspace_id: "workspace-foreign", projection_type: "conversation_presentation",
    projection_revision: 2, cursor: "cursor-foreign",
    payload: conversationPayload([], { origin_sequence: "2" }),
  });

  assert.equal(foreign.conversationSnapshot, null);
  assert.equal(foreign.snapshotReady, false);
  assert.equal(foreign.cursor, "cursor-1");
});

test("rejects a same-Workspace snapshot for a different selected Conversation", () => {
  const initial = reducePresentationFrame(emptyPresentationStream, {
    type: "stream.ready", workspace_id: workspaceId, projection_version: 1, cursor: "cursor-1", max_frame_bytes: 1024,
  }, { workspaceId, conversationId });
  const foreign = reducePresentationFrame(initial, {
    type: "projection", workspace_id: workspaceId, projection_type: "conversation_presentation",
    projection_revision: 1, cursor: "cursor-foreign", payload: conversationPayload([], { conversation_id: "conversation-2" }),
  }, { workspaceId, conversationId });
  assert.equal(foreign.snapshotReady, false);
  assert.equal(foreign.cursor, "cursor-1");
});

test("ignores projection snapshots from another Workspace or with unknown frame fields", () => {
  const initial = reducePresentationFrame(emptyPresentationStream, {
    type: "stream.ready", workspace_id: workspaceId, projection_version: 1, cursor: "cursor-1", max_frame_bytes: 1024,
  });
  const foreign = reducePresentationFrame(initial, {
    type: "projection", workspace_id: "workspace-foreign", projection_type: "conversation_presentation",
    projection_revision: 1, cursor: "cursor-2", payload: conversationPayload(),
  });
  assert.equal(foreign.snapshotReady, false);
  const extra = reducePresentationFrame(initial, {
    type: "projection", workspace_id: workspaceId, projection_type: "conversation_presentation",
    projection_revision: 1, cursor: "cursor-2", payload: conversationPayload(), internal: true,
  });
  assert.equal(extra.snapshotReady, false);
});

test("applies canonical conversation projection upserts and keeps invalid payloads from advancing state", () => {
  const initial = ready();
  const updated = reducePresentationFrame(initial, {
    type: "projection", workspace_id: workspaceId, projection_type: "conversation_presentation",
    projection_revision: 2, cursor: "cursor-3", payload: { item: textItem },
  });
  assert.equal(updated.items.length, 1);
  assert.equal(updated.items[0]?.item_key, "message-1");
  assert.equal(updated.cursor, "cursor-3");

  const invalid = reducePresentationFrame(initial, {
    type: "projection", workspace_id: workspaceId, projection_type: "conversation_presentation",
    projection_revision: 2, cursor: "cursor-invalid", payload: { item: { item_key: "invalid" } },
  });
  assert.equal(invalid.cursor, initial.cursor);
  assert.equal(invalid.projectionRevision, initial.projectionRevision);

  const oversizedSnapshot = reducePresentationFrame(
    reducePresentationFrame(emptyPresentationStream, {
      type: "stream.ready", workspace_id: workspaceId, projection_version: 1, cursor: "cursor-1", max_frame_bytes: 1024,
    }),
    {
      type: "projection", workspace_id: workspaceId, projection_type: "conversation_presentation",
      projection_revision: 2, cursor: "cursor-too-large",
      payload: conversationPayload(Array.from({ length: 101 }, (_, index) => messageWrapper(`message-${index}`, "AGENT", "Answer"))),
    },
  );
  assert.equal(oversizedSnapshot.snapshotReady, false);
  assert.equal(oversizedSnapshot.cursor, "cursor-1");

  const malformedSnapshot = reducePresentationFrame(
    reducePresentationFrame(emptyPresentationStream, {
      type: "stream.ready", workspace_id: workspaceId, projection_version: 1, cursor: "cursor-1", max_frame_bytes: 1024,
    }),
    {
      type: "projection", workspace_id: workspaceId, projection_type: "conversation_presentation",
      projection_revision: 2, cursor: "cursor-partial",
      payload: conversationPayload([messageWrapper("message-1", "AGENT", "Answer", [textItem]), { message: { message_id: "invalid" }, rich_presentation: null, linked_items: [] }]),
    },
  );
  assert.equal(malformedSnapshot.snapshotReady, false);
  assert.deepEqual(malformedSnapshot.items, []);
  assert.equal(malformedSnapshot.cursor, "cursor-1");
});

test("applies a committed Conversation message upsert without moving manual page cursors", () => {
  const initial = ready();
  const next = reducePresentationFrame(initial, {
    type: "projection", workspace_id: workspaceId, projection_type: "conversation_presentation",
    projection_revision: 2, cursor: "cursor-3",
    payload: {
      conversation_id: conversationId,
      stream_cursor: "conversation-history-2",
      origin_sequence: "2",
      item: messageWrapper("message-2", "AGENT", "new committed answer"),
    },
  }, { workspaceId, conversationId });
  assert.equal(next.conversationSnapshot?.messages.length, 1);
  assert.equal(next.conversationSnapshot?.messages[0]?.message_id, "message-2");
  assert.equal(next.conversationSnapshot?.next_cursor, null);
  assert.equal(next.cursor, "cursor-3");
  assert.equal(next.projectionRevision, 2);
  const second = reducePresentationFrame(next, {
    type: "projection", workspace_id: workspaceId, projection_type: "conversation_presentation",
    projection_revision: 2, cursor: "cursor-4",
    payload: {
      conversation_id: conversationId,
      stream_cursor: "conversation-history-2",
      origin_sequence: "2",
      item: messageWrapper("message-3", "AGENT", "following committed answer"),
    },
  }, { workspaceId, conversationId });
  assert.deepEqual(second.conversationSnapshot?.messages.map(message => message.message_id), ["message-2", "message-3"]);
  assert.equal(second.cursor, "cursor-4");
});

test("rejects turn and rich-draft frames with fields outside their wire schemas", () => {
  const initial = ready();
  const turn = reducePresentationFrame(initial, {
    type: "turn.delta", workspace_id: workspaceId, conversation_id: conversationId, turn_id: turnId,
    retry_ordinal: 0, sequence: 1, text_delta: "answer", trusted: true,
  });
  assert.deepEqual(turn.turns, {});

  const draft = reducePresentationFrame(initial, draftFrame("DRAFT_STARTED", {}, 1, { hidden_context: "must-not-be-accepted" }));
  assert.deepEqual(draft.drafts, {});
});

test("rejects rich-draft payload fields outside the selected event schema", () => {
  let state = ready();
  state = reducePresentationFrame(state, draftFrame("DRAFT_STARTED", {}, 1));
  state = reducePresentationFrame(state, draftFrame("BLOCK_OPENED", {
    block_id: "answer", index: 0, block_kind: "TEXT_SLICE", authorization: "untrusted",
  }, 2));
  const draft = state.drafts[JSON.stringify([conversationId, turnId])];
  assert.equal(draft?.status, "FAILED");
  assert.deepEqual(draft?.blocks, []);
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
  state = reducePresentationFrame(state, { type: "turn.settled", workspace_id: workspaceId, conversation_id: conversationId, turn_id: turnId, retry_ordinal: 0, committed_message_id: "message-1" });
  assert.equal(Object.keys(state.drafts).length, 0);
  state = reducePresentationFrame(state, draftFrame("DRAFT_STARTED", {}, 1));
  state = reducePresentationFrame(state, { type: "stream.resync_required", workspace_id: workspaceId, reason: "CURSOR_EXPIRED" });
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
  state = reducePresentationFrame(state, { type: "turn.settled", workspace_id: workspaceId, conversation_id: conversationId, turn_id: turnId, retry_ordinal: 0 });
  state = reducePresentationFrame(state, draftFrame("DRAFT_STARTED", {}, 1));
  assert.equal(Object.keys(state.drafts).length, 0);

  state = reducePresentationFrame(state, draftFrame("DRAFT_STARTED", {}, 1, { retry_ordinal: 1, agent_session_id: "session-2", draft_id: "draft-2" }));
  state = reducePresentationFrame(state, { type: "turn.settled", workspace_id: workspaceId, conversation_id: conversationId, turn_id: turnId, retry_ordinal: 0 });
  assert.equal(state.drafts[JSON.stringify([conversationId, turnId])]?.retryOrdinal, 1);
});

test("ignores turn settlement frames from another Workspace", () => {
  let state = ready();
  state = reducePresentationFrame(state, {
    type: "turn.delta", workspace_id: workspaceId, conversation_id: conversationId, turn_id: turnId,
    retry_ordinal: 0, sequence: 1, text_delta: "partial",
  });
  const foreign = reducePresentationFrame(state, {
    type: "turn.settled", workspace_id: "workspace-foreign", conversation_id: conversationId,
    turn_id: turnId, retry_ordinal: 0,
  });
  assert.equal(foreign.turns[JSON.stringify([conversationId, turnId, 0])]?.state, "STREAMING");
});

test("bounds concurrent transient turns and rejects additional turn identities", () => {
  let state = ready();
  for (let index = 1; index <= 5; index += 1) {
    state = reducePresentationFrame(state, {
      type: "turn.delta", workspace_id: workspaceId, conversation_id: conversationId,
      turn_id: `turn-${index}`, retry_ordinal: 0, sequence: 1, text_delta: `response-${index}`,
    });
  }
  assert.equal(Object.keys(state.turns).length, 4);
  assert.equal(state.turns[JSON.stringify([conversationId, "turn-5", 0])], undefined);
});

test("bounds transient response bytes by UTF-8 size rather than JavaScript string length", () => {
  let state = ready();
  for (let sequence = 1; sequence <= 5; sequence += 1) {
    state = reducePresentationFrame(state, {
      type: "turn.delta", workspace_id: workspaceId, conversation_id: conversationId,
      turn_id: turnId, retry_ordinal: 0, sequence, text_delta: "🙂".repeat(16_000),
    });
  }
  const turn = state.turns[JSON.stringify([conversationId, turnId, 0])];
  assert.equal(turn?.state, "RECONNECTING");
  assert.equal(turn?.text, "");
});

test("marks an oversized single UTF-8 delta as reconnecting instead of leaving a silent spinner", () => {
  const state = reducePresentationFrame(ready(), {
    type: "turn.delta", workspace_id: workspaceId, conversation_id: conversationId,
    turn_id: turnId, retry_ordinal: 0, sequence: 1, text_delta: "界".repeat(22_000),
  });
  const turn = state.turns[JSON.stringify([conversationId, turnId, 0])];
  assert.equal(turn?.state, "RECONNECTING");
  assert.equal(turn?.text, "");
});
