import assert from "node:assert/strict";
import test from "node:test";
import {
  createAutomationApi,
  type Automation,
  type AutomationRevision,
} from "../src/automations/automation-api.ts";

const automation: Automation = {
  automation_id: "automation-1",
  workspace_id: "workspace-1",
  name: "Manual review",
  current_revision: 2,
  status: "PAUSED",
  version: 3,
  created_at: "2026-10-09T10:00:00Z",
  updated_at: "2026-10-09T10:00:00Z",
};

const revision: AutomationRevision = {
  automation_id: "automation-1",
  revision: 2,
  routine_id: "routine-1",
  routine_revision: 4,
  triggers: [{ trigger_id: "manual", kind: "MANUAL" }],
  execution_policy: {} as AutomationRevision["execution_policy"],
  coworker_ref: null,
  authored_by: { principal_id: "owner-local", kind: "USER" },
  created_at: "2026-10-09T10:00:00Z",
};

function readyReceipt(status = "READY") {
  return {
    automation_id: "automation-1",
    automation_revision: 2,
    trigger_id: "manual",
    occurrence_id: "occurrence-1",
    occurrence_version: 3,
    occurrence_status: "STARTED",
    task: {
      task: {
        task_id: "task-1",
        workspace_id: "workspace-1",
        status,
        automation_id: "automation-1",
        automation_occurrence_id: "occurrence-1",
        routine_id: "routine-1",
        routine_revision: 4,
        current_spec_revision: 1,
      },
      current_spec_revision: {
        task_id: "task-1",
        workspace_id: "workspace-1",
        revision: 1,
      },
    },
  };
}

test("manual Run sends the pinned revision and returns a validated READY Task receipt", async () => {
  const requests: Array<{ path: string; init: RequestInit }> = [];
  const api = createAutomationApi("workspace-1", async (path, init) => {
    requests.push({ path, init });
    return Response.json(readyReceipt(), { status: 201 });
  });

  const result = await api.run(automation, revision, { project: "LiteCowork" }, "run-request-1");

  assert.deepEqual(result, {
    task_id: "task-1",
    workspace_id: "workspace-1",
    automation_id: "automation-1",
    automation_revision: 2,
    routine_revision: 4,
    status: "READY",
    current_spec_revision: 1,
    occurrence_id: "occurrence-1",
  });
  assert.equal(requests.length, 1);
  assert.equal(requests[0].path, "/v1/automations/automation-1/run");
  assert.equal(requests[0].init.method, "POST");
  const headers = new Headers(requests[0].init.headers);
  assert.equal(headers.get("Idempotency-Key"), "run-request-1");
  assert.equal(headers.get("If-Match"), '"3"');
  assert.deepEqual(JSON.parse(String(requests[0].init.body)), {
    automation_revision: 2,
    inputs: { project: "LiteCowork" },
  });
});

test("manual Run rejects a receipt that claims planning started instead of READY", async () => {
  const api = createAutomationApi("workspace-1", async () => Response.json(readyReceipt("PLANNING"), { status: 201 }));
  await assert.rejects(
    api.run(automation, revision, {}, "run-request-2"),
    /saved Task response does not match/,
  );
});

test("manual Run preserves the same request key across an ambiguous retry", async () => {
  const requestIds: string[] = [];
  const api = createAutomationApi("workspace-1", async (_path, init) => {
    requestIds.push(new Headers(init.headers).get("Idempotency-Key") ?? "");
    return Response.json(readyReceipt(), { status: requestIds.length === 1 ? 201 : 200 });
  });

  await api.run(automation, revision, { project: "LiteCowork" }, "same-run-key");
  await api.run(automation, revision, { project: "LiteCowork" }, "same-run-key");
  assert.deepEqual(requestIds, ["same-run-key", "same-run-key"]);
});
