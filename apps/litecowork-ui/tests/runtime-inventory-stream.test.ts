import assert from "node:assert/strict";
import test from "node:test";
import {
  RuntimeInventoryStreamController,
  type RuntimeInventoryStreamDriver,
  type RuntimeInventoryStreamEnvelope,
} from "../src/runtime/runtime-inventory-stream.ts";

class FakeDriver implements RuntimeInventoryStreamDriver {
  listener: ((envelope: RuntimeInventoryStreamEnvelope) => void) | null = null;
  starts: Array<{ sessionId: string; workspaceId: string; runtimeId: string; resumeCursor: string | null }> = [];
  acknowledgements: string[] = [];
  stopped: string[] = [];
  private nextSession = 0;

  async listen(listener: (envelope: RuntimeInventoryStreamEnvelope) => void): Promise<() => void> {
    this.listener = listener;
    return () => { if (this.listener === listener) this.listener = null; };
  }

  async start(input: { sessionId: string; workspaceId: string; runtimeId: string; resumeCursor: string | null }): Promise<void> {
    this.starts.push(input);
  }

  async acknowledge(_sessionId: string, frameId: string): Promise<void> { this.acknowledgements.push(frameId); }
  async stop(sessionId: string): Promise<void> { this.stopped.push(sessionId); }
  newSessionId(): string { this.nextSession += 1; return `session-${this.nextSession}`; }

  emit(envelope: RuntimeInventoryStreamEnvelope): void { this.listener?.(envelope); }
}

function ready(workspaceId = "workspace-1"): Record<string, unknown> {
  return { type: "stream.ready", workspace_id: workspaceId, projection_version: 1, cursor: "ready-cursor", max_frame_bytes: 1024 };
}

function notification(workspaceId = "workspace-1", runtimeId = "runtime-1", cursor = "cursor-1"): Record<string, unknown> {
  return {
    type: "projection",
    workspace_id: workspaceId,
    projection_type: "runtime_inventory",
    projection_revision: 2,
    cursor,
    payload: { runtime_id: runtimeId, change_kind: "OFFERS_CHANGED" },
  };
}

function envelope(sessionId: string, frame: unknown, workspaceId = "workspace-1", runtimeId = "runtime-1", frameId = "frame-1"): RuntimeInventoryStreamEnvelope {
  return { sessionId, workspaceId, runtimeId, kind: "frame", frameId, frame };
}

async function waitFor(predicate: () => boolean): Promise<void> {
  for (let attempt = 0; attempt < 40; attempt += 1) {
    if (predicate()) return;
    await new Promise(resolve => setTimeout(resolve, 0));
  }
  assert.fail("condition did not settle");
}

test("inventory stream invalidations trigger an authorized reread and resume from accepted cursor", async () => {
  const driver = new FakeDriver();
  let refreshes = 0;
  const statuses: string[] = [];
  const controller = new RuntimeInventoryStreamController("workspace-1", "runtime-1", driver, () => { refreshes += 1; }, status => { statuses.push(status); }, async () => {});
  controller.start();
  await waitFor(() => driver.starts.length === 1 && driver.listener !== null);
  assert.equal(statuses[0], "CONNECTING");
  const firstSession = driver.starts[0]!.sessionId;
  driver.emit(envelope(firstSession, ready()));
  driver.emit(envelope(firstSession, notification()));
  await waitFor(() => refreshes === 1);
  assert.equal(controller.status, "LIVE");
  assert.deepEqual(driver.acknowledgements, ["frame-1", "frame-1"]);

  driver.emit({ sessionId: firstSession, workspaceId: "workspace-1", runtimeId: "runtime-1", kind: "closed" });
  await waitFor(() => driver.starts.length === 2);
  assert.equal(driver.starts[1]!.resumeCursor, "cursor-1");
  const secondSession = driver.starts[1]!.sessionId;
  driver.emit(envelope(secondSession, notification("workspace-1", "runtime-1", "cursor-2"), "workspace-1", "runtime-1", "frame-2"));
  await waitFor(() => refreshes === 2);
  await controller.dispose();
  assert.ok(driver.stopped.includes(firstSession));
  assert.ok(driver.stopped.includes(secondSession));
});

test("inventory stream rejects foreign identities and payloads with private fields", async () => {
  const driver = new FakeDriver();
  let refreshes = 0;
  const controller = new RuntimeInventoryStreamController("workspace-1", "runtime-1", driver, () => { refreshes += 1; }, undefined, async () => {});
  controller.start();
  await waitFor(() => driver.starts.length === 1 && driver.listener !== null);
  const session = driver.starts[0]!.sessionId;
  driver.emit(envelope(session, ready()));
  driver.emit(envelope(session, notification("workspace-1", "runtime-other"), "workspace-1", "runtime-other", "foreign-runtime"));
  driver.emit(envelope(session, notification("workspace-other"), "workspace-other", "runtime-1", "foreign-workspace"));
  driver.emit(envelope(session, {
    ...notification(),
    payload: { runtime_id: "runtime-1", change_kind: "OFFERS_CHANGED", endpoint_locator: "/private/socket" },
  }, "workspace-1", "runtime-1", "private-payload"));
  await new Promise(resolve => setTimeout(resolve, 0));
  assert.equal(refreshes, 0);

  driver.emit(envelope(session, notification(), "workspace-1", "runtime-1", "valid-frame"));
  await waitFor(() => refreshes === 1);
  await controller.dispose();
  assert.ok(driver.acknowledgements.includes("foreign-runtime"));
  assert.ok(driver.acknowledgements.includes("foreign-workspace"));
  assert.ok(driver.acknowledgements.includes("private-payload"));
});

test("resync discards the accepted cursor before reconnecting", async () => {
  const driver = new FakeDriver();
  const controller = new RuntimeInventoryStreamController("workspace-1", "runtime-1", driver, () => {}, undefined, async () => {});
  controller.start();
  await waitFor(() => driver.starts.length === 1 && driver.listener !== null);
  const firstSession = driver.starts[0]!.sessionId;
  driver.emit(envelope(firstSession, ready()));
  driver.emit(envelope(firstSession, notification()));
  await waitFor(() => controller.status === "LIVE");
  driver.emit(envelope(firstSession, {
    type: "stream.resync_required",
    workspace_id: "workspace-1",
    reason: "resume_cursor_invalid_or_expired",
  }, "workspace-1", "runtime-1", "resync"));
  driver.emit({ sessionId: firstSession, workspaceId: "workspace-1", runtimeId: "runtime-1", kind: "closed" });
  await waitFor(() => driver.starts.length === 2);
  assert.equal(driver.starts[1]!.resumeCursor, null);
  await controller.dispose();
});
