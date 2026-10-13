import assert from "node:assert/strict";
import { registerHooks } from "node:module";
import test from "node:test";

registerHooks({
  resolve(specifier, context, nextResolve) {
    if (specifier.startsWith(".") && !/\.[a-z]+$/i.test(specifier)) {
      try { return nextResolve(`${specifier}.ts`, context); } catch { /* Preserve the normal resolver error below. */ }
    }
    return nextResolve(specifier, context);
  },
});

const { desktopAutomationApi } = await import("../src/automations/desktop-automation-api.ts");
const { desktopGoalApi } = await import("../src/goals/desktop-goal-api.ts");
const { desktopRoutineApi } = await import("../src/routines/desktop-routine-api.ts");
const { createRoutineApi } = await import("../src/routines/routine-api.ts");

type InvokeCall = { command: string; args: Record<string, unknown> };
const invokeCalls: InvokeCall[] = [];
const priorWindow = Object.getOwnPropertyDescriptor(globalThis, "window");
Object.defineProperty(globalThis, "window", {
  configurable: true,
  value: {
    __TAURI_INTERNALS__: {
      invoke: async (command: string, args: Record<string, unknown>) => {
        invokeCalls.push({ command, args });
        throw new Error("captured Tauri invoke");
      },
    },
  },
});
test.after(() => {
  if (priorWindow) Object.defineProperty(globalThis, "window", priorWindow);
  else delete (globalThis as typeof globalThis & { window?: unknown }).window;
});

const workspaceId = "workspace-1";
const goalRevision = {
  objective: "Ship the local V1",
  success_criteria: [],
  constraints: [],
  horizon: null,
  related_task_ids: [],
  related_routine_refs: [],
  related_artifact_refs: [],
};
const automation = {
  automation_id: "automation-1",
  workspace_id: workspaceId,
  name: "Manual review",
  current_revision: 1,
  status: "PAUSED" as const,
  version: 1,
  created_at: "2026-10-13T00:00:00Z",
  updated_at: "2026-10-13T00:00:00Z",
};
const automationRevision = {
  automation_id: automation.automation_id,
  revision: 1,
  routine_id: "routine-1",
  routine_revision: 1,
  triggers: [{ trigger_id: "manual", kind: "MANUAL" }],
  execution_policy: {},
  coworker_ref: null,
  authored_by: { principal_id: "owner-local", kind: "USER" },
  created_at: "2026-10-13T00:00:00Z",
};

async function assertDesktopAdapterForwards(
  name: string,
  invokeCommand: string,
  invokeRequest: (requestId: string) => Promise<unknown>,
): Promise<void> {
  for (const length of [129, 200]) {
    const requestId = "k".repeat(length);
    const before = invokeCalls.length;
    await assert.rejects(invokeRequest(requestId), /captured Tauri invoke/, `${name} should reach the native bridge at ${length} bytes`);
    assert.equal(invokeCalls.length, before + 1);
    assert.equal(invokeCalls.at(-1)?.command, invokeCommand);
    assert.equal(invokeCalls.at(-1)?.args.requestId, requestId);
  }

  const before = invokeCalls.length;
  await assert.rejects(invokeRequest("k".repeat(201)), /request identity/i, `${name} should reject 201 bytes`);
  assert.equal(invokeCalls.length, before, `${name} must reject an oversized key before native IPC`);
  await assert.rejects(invokeRequest("key with spaces"), /request identity/i, `${name} should reject non-graphic ASCII`);
  await assert.rejects(invokeRequest("caf\u00e9"), /request identity/i, `${name} should reject non-ASCII`);
}

test("desktop Goal, Automation, and Routine routes accept 129–200 byte ASCII keys", async () => {
  await assertDesktopAdapterForwards("Goal", "goal_request", requestId =>
    desktopGoalApi(workspaceId).create(null, goalRevision, requestId));

  await assertDesktopAdapterForwards("Automation", "automation_request", requestId =>
    desktopAutomationApi(workspaceId).run(automation, automationRevision, {}, requestId));

  await assertDesktopAdapterForwards("Routine", "routine_request", requestId =>
    desktopRoutineApi(workspaceId).create("Routine", { objective_template: "Review" }, requestId));
});

test("the generic Routine transport accepts 129–200 byte keys and rejects 201", async () => {
  const forwarded: string[] = [];
  const api = createRoutineApi(workspaceId, async (_path, init) => {
    forwarded.push(new Headers(init.headers).get("Idempotency-Key") ?? "");
    return Response.json({
      routine_id: "routine-1",
      workspace_id: workspaceId,
      name: "Routine",
      current_revision: 1,
      status: "ACTIVE",
      version: 1,
      created_at: "2026-10-13T00:00:00Z",
      updated_at: "2026-10-13T00:00:00Z",
    });
  });

  for (const length of [129, 200]) {
    const requestId = "r".repeat(length);
    await api.create("Routine", { objective_template: "Review" }, requestId);
    assert.equal(forwarded.at(-1), requestId);
  }

  await assert.rejects(api.create("Routine", {}, "r".repeat(201)), /request identity/i);
  assert.equal(forwarded.length, 2);
  await assert.rejects(api.create("Routine", {}, "id with space"), /request identity/i);
  await assert.rejects(api.create("Routine", {}, "caf\u00e9"), /request identity/i);
});
