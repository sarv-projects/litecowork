import test from "node:test";
import assert from "node:assert/strict";
import {
  TaskPresentationStreamController,
  emptyTaskStream,
  isTaskSnapshotEpochCurrent,
  type TaskSnapshotStreamDriver,
  type TaskStreamEnvelope,
} from "../src/tasks/task-stream.ts";

test("an older unary snapshot cannot commit progress after a newer stream snapshot", () => {
  assert.equal(isTaskSnapshotEpochCurrent(4, 4), true);
  assert.equal(isTaskSnapshotEpochCurrent(4, 5), false);
  assert.equal(isTaskSnapshotEpochCurrent(Number.NaN, 4), false);
});

const workspaceId = "workspace-1";
const taskId = "task-1";

function item(itemKey: string, objective = "Build the product") {
  return {
    item_key: itemKey,
    kind: "TASK_CARD",
    payload_version: 1,
    source_refs: [{ kind: "TASK", id: taskId }],
    order_key: itemKey,
    status: "IN_PROGRESS",
    payload: { task_id: taskId, objective, status: "RUNNING" },
    freshness: "CURRENT",
  };
}

function ready(cursor = "ready-cursor") {
  return { type: "stream.ready", workspace_id: workspaceId, projection_version: 1, cursor, max_frame_bytes: 1024 };
}

function projection(cursor: string, revision: number, keys: string[], overrides: Record<string, unknown> = {}) {
  return {
    type: "projection",
    workspace_id: workspaceId,
    projection_type: "task_presentation",
    projection_revision: revision,
    cursor,
    payload: {
      workspace_id: workspaceId,
      task_id: taskId,
      origin_sequence: String(revision),
      computed_at: "2026-10-11T12:00:00Z",
      freshness: "CURRENT",
      items: keys.map(key => item(key)),
      ...overrides,
    },
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(done => { resolve = done; });
  return { promise, resolve };
}

class FakeDriver implements TaskSnapshotStreamDriver {
  listener: ((envelope: TaskStreamEnvelope) => void) | null = null;
  starts: Array<{ sessionId: string; workspaceId: string; taskId: string; resumeCursor: string | null }> = [];
  stops: string[] = [];
  acknowledgements: Array<{ sessionId: string; frameId: string }> = [];
  private sequence = 0;
  onStart?: (input: { sessionId: string; workspaceId: string; taskId: string; resumeCursor: string | null }) => void;

  async listen(listener: (envelope: TaskStreamEnvelope) => void) {
    this.listener = listener;
    return () => { if (this.listener === listener) this.listener = null; };
  }
  async start(input: { sessionId: string; workspaceId: string; taskId: string; resumeCursor: string | null }) {
    this.starts.push(input);
    this.onStart?.(input);
  }
  async acknowledge(sessionId: string, frameId: string) { this.acknowledgements.push({ sessionId, frameId }); }
  async stop(sessionId: string) { this.stops.push(sessionId); }
  newSessionId() { return `task-stream-${++this.sequence}`; }
  emit(envelope: TaskStreamEnvelope) { this.listener?.(envelope); }
}

function frame(driver: FakeDriver, sessionId: string, frameId: string, payload: unknown, overrides: Record<string, unknown> = {}) {
  driver.emit({ sessionId, workspaceId, taskId, kind: "frame", frameId, frame: payload, ...overrides } as TaskStreamEnvelope);
}

test("listener is installed before start can synchronously deliver ready", async () => {
  const driver = new FakeDriver();
  const controller = new TaskPresentationStreamController(workspaceId, taskId, driver);
  driver.onStart = input => frame(driver, input.sessionId, "ready-frame", ready());
  controller.start();
  for (let attempts = 0; attempts < 20 && !controller.state.ready; attempts++) await new Promise(resolve => setImmediate(resolve));
  assert.equal(controller.state.ready, true);
  assert.deepEqual(driver.acknowledgements, [{ sessionId: "task-stream-1", frameId: "ready-frame" }]);
  await controller.dispose();
});

test("accepts only matching workspace and task snapshots and ACKs discarded frames once", async () => {
  const driver = new FakeDriver();
  const controller = new TaskPresentationStreamController(workspaceId, taskId, driver, () => {}, () => {}, async () => new Promise<void>(() => {}));
  controller.start();
  await new Promise(resolve => setImmediate(resolve));
  const sessionId = driver.starts[0]!.sessionId;
  assert.equal(driver.starts[0]?.resumeCursor, null);
  frame(driver, sessionId, "ready", ready());
  frame(driver, sessionId, "foreign-envelope", projection("foreign-envelope-cursor", 1, ["foreign"]), { taskId: "task-2" });
  frame(driver, sessionId, "foreign-payload", projection("foreign-payload-cursor", 2, ["foreign"], { task_id: "task-2" }));
  frame(driver, sessionId, "malformed-envelope", projection("malformed-envelope-cursor", 2, ["foreign"]), { internal: true });
  frame(driver, sessionId, "valid", projection("valid-cursor", 3, ["valid"]));
  await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(controller.state.items.map(value => value.item_key), ["valid"]);
  assert.equal(controller.state.cursor, "valid-cursor");
  assert.deepEqual(driver.acknowledgements.map(value => value.frameId), ["ready", "foreign-envelope", "foreign-payload", "malformed-envelope", "valid"]);
  await controller.dispose();
});

test("malformed snapshots do not advance cursor or partially replace items", async () => {
  const driver = new FakeDriver();
  const controller = new TaskPresentationStreamController(workspaceId, taskId, driver, () => {}, () => {}, async () => new Promise<void>(() => {}));
  controller.start();
  await new Promise(resolve => setImmediate(resolve));
  const sessionId = driver.starts[0]!.sessionId;
  frame(driver, sessionId, "ready", ready());
  frame(driver, sessionId, "first", projection("cursor-1", 1, ["old"]));
  frame(driver, sessionId, "malformed", projection("bad-cursor", 2, ["new"], { origin_sequence: "02", extra: "private" }));
  frame(driver, sessionId, "foreign-item", projection("foreign-item-cursor", 3, ["foreign"], {
    items: [{ ...item("foreign"), payload: { task_id: "task-2", objective: "Other task", status: "RUNNING" } }],
  }));
  await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(controller.state.items.map(value => value.item_key), ["old"]);
  assert.equal(controller.state.cursor, "cursor-1");
  await controller.dispose();
});

test("valid snapshots replace prior content atomically and preserve wire item ordering", async () => {
  const driver = new FakeDriver();
  const controller = new TaskPresentationStreamController(workspaceId, taskId, driver, () => {}, () => {}, async () => new Promise<void>(() => {}));
  controller.start();
  await new Promise(resolve => setImmediate(resolve));
  const sessionId = driver.starts[0]!.sessionId;
  frame(driver, sessionId, "ready", ready());
  frame(driver, sessionId, "first", projection("cursor-1", 3, ["z", "a"]));
  assert.deepEqual(controller.state.items.map(value => value.item_key), ["z", "a"]);
  frame(driver, sessionId, "second", projection("cursor-2", 4, ["replacement"]));
  await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(controller.state.items.map(value => value.item_key), ["replacement"]);
  assert.equal(controller.state.projectionRevision, 4);
  assert.equal(controller.state.cursor, "cursor-2");
  await controller.dispose();
});

test("ignores older snapshots and rejects projection revisions that disagree with the durable frontier", async () => {
  const driver = new FakeDriver();
  const controller = new TaskPresentationStreamController(workspaceId, taskId, driver, () => {}, () => {}, async () => new Promise<void>(() => {}));
  controller.start();
  await new Promise(resolve => setImmediate(resolve));
  const sessionId = driver.starts[0]!.sessionId;
  frame(driver, sessionId, "ready", ready());
  frame(driver, sessionId, "newer", projection("cursor-newer", 8, ["newer"]));
  frame(driver, sessionId, "older", projection("cursor-older", 7, ["older"]));
  frame(driver, sessionId, "mismatched-frontier", projection("cursor-wrong-frontier", 9, ["wrong"], { origin_sequence: "8" }));
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(controller.state.projectionRevision, 8);
  assert.equal(controller.state.cursor, "cursor-newer");
  assert.deepEqual(controller.state.items.map(value => value.item_key), ["newer"]);
  await controller.dispose();
});

test("resync clears snapshot and accepted cursor until the replacement arrives", async () => {
  const driver = new FakeDriver();
  const controller = new TaskPresentationStreamController(workspaceId, taskId, driver, () => {}, () => {}, async () => new Promise<void>(() => {}));
  controller.start();
  await new Promise(resolve => setImmediate(resolve));
  const sessionId = driver.starts[0]!.sessionId;
  frame(driver, sessionId, "ready", ready());
  frame(driver, sessionId, "first", projection("cursor-1", 1, ["old"]));
  frame(driver, sessionId, "resync", { type: "stream.resync_required", workspace_id: workspaceId, reason: "CURSOR_EXPIRED" });
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(controller.state.cursor, null);
  assert.equal(controller.state.snapshotReady, false);
  assert.deepEqual(controller.state.items, []);
  frame(driver, sessionId, "replacement", projection("cursor-2", 2, ["fresh"]));
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(controller.state.snapshotReady, true);
  assert.equal(controller.state.resyncRequired, false);
  assert.deepEqual(controller.state.items.map(value => value.item_key), ["fresh"]);
  await controller.dispose();
});

test("resync marks replacement delivery pending without claiming the stream reconnected", async () => {
  const driver = new FakeDriver();
  const statuses: string[] = [];
  const controller = new TaskPresentationStreamController(
    workspaceId,
    taskId,
    driver,
    () => {},
    status => statuses.push(status),
    async () => new Promise<void>(() => {}),
  );
  controller.start();
  await new Promise(resolve => setImmediate(resolve));
  const sessionId = driver.starts[0]!.sessionId;
  frame(driver, sessionId, "ready", ready());
  frame(driver, sessionId, "snapshot", projection("cursor-1", 1, ["previous"]));
  assert.equal(controller.status, "LIVE");

  frame(driver, sessionId, "resync", { type: "stream.resync_required", workspace_id: workspaceId, reason: "REPLAY_GAP" });

  assert.equal(controller.status, "RESYNCING");
  assert.equal(controller.state.snapshotReady, false);
  assert.deepEqual(statuses, ["LIVE", "RESYNCING"]);
  await controller.dispose();
});

test("reconnect resumes only from the cursor of the last valid snapshot", async () => {
  const driver = new FakeDriver();
  const controller = new TaskPresentationStreamController(workspaceId, taskId, driver, () => {}, () => {}, async () => {});
  controller.start();
  await new Promise(resolve => setImmediate(resolve));
  const first = driver.starts[0]!;
  frame(driver, first.sessionId, "ready", ready());
  frame(driver, first.sessionId, "snapshot", projection("accepted-cursor", 1, ["task"]));
  driver.emit({ sessionId: first.sessionId, workspaceId, taskId, kind: "closed" });
  for (let attempts = 0; attempts < 20 && driver.starts.length < 2; attempts++) await new Promise(resolve => setImmediate(resolve));
  assert.equal(driver.starts[1]?.resumeCursor, "accepted-cursor");
  await controller.dispose();
});

test("a disposed controller can restart from its retained accepted cursor", async () => {
  const driver = new FakeDriver();
  const controller = new TaskPresentationStreamController(workspaceId, taskId, driver, () => {}, () => {}, async () => {});
  controller.start();
  await new Promise(resolve => setImmediate(resolve));
  const first = driver.starts[0]!;
  frame(driver, first.sessionId, "ready", ready("unaccepted-ready-cursor"));
  frame(driver, first.sessionId, "snapshot", projection("accepted-cursor", 1, ["task"]));
  await controller.dispose();

  controller.start();
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(driver.starts[1]?.resumeCursor, "accepted-cursor");
  assert.equal(driver.starts[1]?.sessionId, "task-stream-2");
  await controller.dispose();
});

test("stale session frames are ignored and dispose stops session and cancels retry wait", async () => {
  const driver = new FakeDriver();
  const delay = deferred<void>();
  const controller = new TaskPresentationStreamController(workspaceId, taskId, driver, () => {}, () => {}, () => delay.promise);
  controller.start();
  await new Promise(resolve => setImmediate(resolve));
  const oldSessionId = driver.starts[0]!.sessionId;
  const staleListener = driver.listener!;
  driver.emit({ sessionId: oldSessionId, workspaceId, taskId, kind: "error" });
  for (let attempts = 0; attempts < 20 && driver.stops.length < 1; attempts++) await new Promise(resolve => setImmediate(resolve));
  await controller.dispose();
  staleListener({ sessionId: oldSessionId, workspaceId, taskId, kind: "frame", frameId: "late", frame: projection("late", 1, ["stale"]) });
  assert.equal(controller.state, emptyTaskStream);
  assert.deepEqual(driver.stops, [oldSessionId]);
});
