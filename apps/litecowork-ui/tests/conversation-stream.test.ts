import test from "node:test";
import assert from "node:assert/strict";
import {
  ConversationStreamController,
  type ConversationStreamDriver,
  type StreamEnvelope,
} from "../src/conversations/conversation-stream.ts";
import { emptyPresentationStream } from "../src/presentation/presentation-stream.ts";
import { mergeConversationStreamSnapshot, parseConversationSnapshot } from "../src/conversations/conversation-view.ts";

const workspaceId = "workspace-1";
const conversationId = "conversation-1";

function snapshot(messageIds: string[], overrides: Record<string, unknown> = {}) {
  return {
    workspace_id: workspaceId,
    conversation_id: conversationId,
    projection_revision: 1,
    stream_cursor: "inner-history-cursor",
    origin_sequence: "12",
    items: messageIds.map((messageId, index) => ({
      message: {
        message_id: messageId,
        conversation_id: conversationId,
        author: { principal_id: "user-1", kind: "USER" },
        role: "USER",
        content: [{ kind: "TEXT", text: messageId }],
        created_at: `2026-10-11T12:00:${String(index).padStart(2, "0")}Z`,
      },
      rich_presentation: null,
      linked_items: [],
    })),
    next_cursor: null,
    active_turn: null,
    ...overrides,
  };
}

function projection(cursor: string, messageIds: string[], projectionRevision = 1) {
  return {
    type: "projection",
    workspace_id: workspaceId,
    projection_type: "conversation_presentation",
    projection_revision: projectionRevision,
    cursor,
    payload: {
      conversation_id: conversationId,
      stream_cursor: `history-${projectionRevision}`,
      origin_sequence: String(projectionRevision),
      items: snapshot(messageIds).items,
      next_cursor: null,
      active_turn: null,
    },
  };
}

function view(messageIds: string[], overrides: Record<string, unknown> = {}) {
  return parseConversationSnapshot(snapshot(messageIds, overrides), workspaceId, conversationId);
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(done => { resolve = done; });
  return { promise, resolve };
}

class FakeDriver implements ConversationStreamDriver {
  listener: ((envelope: StreamEnvelope) => void) | null = null;
  starts: Array<{ sessionId: string; resumeCursor: string | null }> = [];
  stops: string[] = [];
  acknowledgements: Array<{ sessionId: string; frameId: string }> = [];
  failedFrameId: string | null = null;
  private sequence = 0;

  async listen(listener: (envelope: StreamEnvelope) => void) {
    this.listener = listener;
    return () => { if (this.listener === listener) this.listener = null; };
  }
  async start(input: { sessionId: string; workspaceId: string; conversationId: string; resumeCursor: string | null }) {
    this.starts.push({ sessionId: input.sessionId, resumeCursor: input.resumeCursor });
  }
  async stop(sessionId: string) { this.stops.push(sessionId); }
  async acknowledge(sessionId: string, frameId: string) {
    this.acknowledgements.push({ sessionId, frameId });
    if (frameId === this.failedFrameId) throw new Error("native stream stopped");
  }
  newSessionId() { return `stream-${++this.sequence}`; }
  emit(envelope: StreamEnvelope) { this.listener?.(envelope); }
}

test("live snapshots merge without dropping messages from explicitly loaded pages", () => {
  const olderPages = view(["message-1", "message-2"], { next_cursor: "message-2" });
  const live = view(["message-1", "message-2", "message-3"], { projection_revision: 3, next_cursor: "message-3" });
  const merged = mergeConversationStreamSnapshot(olderPages, live, workspaceId, conversationId);
  assert.deepEqual(merged.messages.map(message => message.message_id), ["message-1", "message-2", "message-3"]);
  assert.equal(merged.next_cursor, "message-3");
  assert.throws(() => mergeConversationStreamSnapshot(olderPages, { ...live, conversation_id: "conversation-2" }, workspaceId, conversationId), /selected Conversation/);
});

test("stream frames are fenced to the selected Conversation and accepted cursor", async () => {
  const driver = new FakeDriver();
  const states: Array<{ cursor: string | null; conversationId: string | null }> = [];
  const controller = new ConversationStreamController(workspaceId, conversationId, driver, state => {
    states.push({ cursor: state.cursor, conversationId: state.conversationSnapshot?.conversation_id ?? null });
  }, () => {}, async () => new Promise<void>(() => {}));
  controller.start();
  await new Promise(resolve => setImmediate(resolve));
  const sessionId = driver.starts[0]!.sessionId;
  driver.emit({
    sessionId, workspaceId, conversationId, kind: "frame", frameId: "frame-1",
    frame: { type: "stream.ready", workspace_id: workspaceId, projection_version: 1, cursor: "resume-cursor-1", max_frame_bytes: 1024 },
  });
  driver.emit({ sessionId, workspaceId, conversationId, kind: "frame", frameId: "frame-2", frame: projection("applied-cursor", ["message-1"], 1) });
  driver.emit({ sessionId, workspaceId, conversationId: "conversation-2", kind: "frame", frameId: "frame-3", frame: projection("foreign-cursor", ["message-2"], 2) });
  await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(states.at(-1), { cursor: "applied-cursor", conversationId });
  await controller.dispose();
  assert.deepEqual(driver.stops, [sessionId]);
  assert.deepEqual(driver.acknowledgements, [
    { sessionId, frameId: "frame-1" },
    { sessionId, frameId: "frame-2" },
    { sessionId, frameId: "frame-3" },
  ]);
});

test("reconnect resumes only from the last accepted outer cursor", async () => {
  const driver = new FakeDriver();
  const controller = new ConversationStreamController(workspaceId, conversationId, driver, () => {}, () => {}, async () => {});
  controller.start();
  await new Promise(resolve => setImmediate(resolve));
  const first = driver.starts[0]!;
  driver.emit({
    sessionId: first.sessionId, workspaceId, conversationId, kind: "frame", frameId: "frame-1",
    frame: { type: "stream.ready", workspace_id: workspaceId, projection_version: 1, cursor: "base-cursor", max_frame_bytes: 1024 },
  });
  driver.emit({ sessionId: first.sessionId, workspaceId, conversationId, kind: "frame", frameId: "frame-2", frame: projection("last-applied", ["message-1"], 1) });
  driver.emit({ sessionId: first.sessionId, workspaceId, conversationId, kind: "error", message: "connection closed" });
  for (let attempts = 0; attempts < 20 && driver.starts.length < 2; attempts++) await new Promise(resolve => setImmediate(resolve));
  assert.equal(driver.starts[1]?.resumeCursor, "last-applied");
  await controller.dispose();
});

test("acknowledgement failure reconnects from the cursor already reduced by the UI", async () => {
  const driver = new FakeDriver();
  driver.failedFrameId = "frame-2";
  const controller = new ConversationStreamController(workspaceId, conversationId, driver, () => {}, () => {}, async () => {});
  controller.start();
  await new Promise(resolve => setImmediate(resolve));
  const first = driver.starts[0]!;
  driver.emit({
    sessionId: first.sessionId, workspaceId, conversationId, kind: "frame", frameId: "frame-1",
    frame: { type: "stream.ready", workspace_id: workspaceId, projection_version: 1, cursor: "base-cursor", max_frame_bytes: 1024 },
  });
  driver.emit({
    sessionId: first.sessionId, workspaceId, conversationId, kind: "frame", frameId: "frame-2",
    frame: projection("last-applied", ["message-1"], 1),
  });
  for (let attempts = 0; attempts < 20 && driver.starts.length < 2; attempts++) await new Promise(resolve => setImmediate(resolve));
  assert.equal(driver.starts[1]?.resumeCursor, "last-applied");
  await controller.dispose();
});

test("an event from an obsolete session cannot change the current state", async () => {
  const driver = new FakeDriver();
  const controller = new ConversationStreamController(workspaceId, conversationId, driver, () => {}, () => {}, async () => new Promise<void>(() => {}));
  controller.start();
  await new Promise(resolve => setImmediate(resolve));
  const oldSessionId = driver.starts[0]!.sessionId;
  await controller.dispose();
  driver.emit({
    sessionId: oldSessionId, workspaceId, conversationId, kind: "frame", frameId: "frame-1",
    frame: { type: "stream.ready", workspace_id: workspaceId, projection_version: 1, cursor: "late", max_frame_bytes: 1024 },
  });
  assert.equal(controller.state, emptyPresentationStream);
});
