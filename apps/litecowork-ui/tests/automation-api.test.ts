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


function testReceipt(overrides: Record<string, unknown> = {}) {
  return {
    test_run_id: "test-run-1",
    automation_id: "automation-1",
    automation_revision: 2,
    task: {
      task: {
        task_id: "task-test-1",
        workspace_id: "workspace-1",
        status: "READY",
        automation_id: null,
        automation_occurrence_id: null,
        automation_test_run_id: "test-run-1",
        routine_id: "routine-1",
        routine_revision: 4,
        current_spec_revision: 1,
      },
      current_spec_revision: {
        task_id: "task-test-1",
        workspace_id: "workspace-1",
        revision: 1,
      },
    },
    ...overrides,
  };
}

test("exact-revision test Run uses the test route and returns the pinned READY Task", async () => {
  const requests: Array<{ path: string; init: RequestInit }> = [];
  const api = createAutomationApi("workspace-1", async (path, init) => {
    requests.push({ path, init });
    return Response.json(testReceipt(), { status: 201 });
  });

  const result = await api.testRun(automation, revision, { project: "LiteCowork" }, "test-request-1");

  assert.deepEqual(result, {
    test_run_id: "test-run-1",
    task_id: "task-test-1",
    workspace_id: "workspace-1",
    automation_id: "automation-1",
    automation_revision: 2,
    routine_id: "routine-1",
    routine_revision: 4,
    status: "READY",
    current_spec_revision: 1,
  });
  assert.equal(requests.length, 1);
  assert.equal(requests[0].path, "/v1/automations/automation-1/test-runs");
  assert.equal(requests[0].init.method, "POST");
  const headers = new Headers(requests[0].init.headers);
  assert.equal(headers.get("Idempotency-Key"), "test-request-1");
  assert.equal(headers.get("If-Match"), '"3"');
  assert.deepEqual(JSON.parse(String(requests[0].init.body)), {
    automation_revision: 2,
    inputs: { project: "LiteCowork" },
  });
});

test("test Run rejects a receipt that also carries occurrence provenance", async () => {
  const receipt = testReceipt();
  (receipt.task.task as Record<string, unknown>).automation_id = "automation-1";
  const api = createAutomationApi("workspace-1", async () => Response.json(receipt, { status: 201 }));
  await assert.rejects(
    api.testRun(automation, revision, {}, "test-request-2"),
    /test Task response does not match/,
  );
});

test("test Run preserves the same request key across an ambiguous retry", async () => {
  const requestIds: string[] = [];
  const api = createAutomationApi("workspace-1", async (_path, init) => {
    requestIds.push(new Headers(init.headers).get("Idempotency-Key") ?? "");
    return Response.json(testReceipt(), { status: requestIds.length === 1 ? 201 : 200 });
  });

  await api.testRun(automation, revision, { project: "LiteCowork" }, "same-test-key");
  await api.testRun(automation, revision, { project: "LiteCowork" }, "same-test-key");
  assert.deepEqual(requestIds, ["same-test-key", "same-test-key"]);
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

test("manual Run accepts the shared 200 byte idempotency-key boundary", async () => {
  const requestIds: string[] = [];
  const api = createAutomationApi("workspace-1", async (_path, init) => {
    requestIds.push(new Headers(init.headers).get("Idempotency-Key") ?? "");
    return Response.json(readyReceipt(), { status: 201 });
  });

  for (const length of [129, 200]) {
    const requestId = "k".repeat(length);
    await api.run(automation, revision, { project: "LiteCowork" }, requestId);
    assert.equal(requestIds.at(-1), requestId);
  }

  await assert.rejects(
    api.run(automation, revision, {}, "k".repeat(201)),
    /request identity or version is invalid/,
  );
  assert.equal(requestIds.length, 2, "a 201 byte key must not reach the transport");
});

function healthProjection(overrides: Record<string, unknown> = {}) {
  return {
    automation_id: "automation-1",
    current_revision: 2,
    status: "ENABLED",
    host_ready: true,
    blockers: [],
    next_occurrence_at: "2026-10-10T09:00:00Z",
    last_occurrence_at: "2026-10-09T09:00:00Z",
    nonterminal_occurrences: 1,
    occurrence_counts: {
      pending: 0, claimed: 0, waiting_dependency: 0, started: 1,
      completed: 2, skipped: 1, failed: 0,
    },
    routine_health: {
      routine_id: "routine-1",
      current_revision: 4,
      last_run_at: "2026-10-09T09:05:00Z",
      last_success_at: "2026-10-09T09:05:00Z",
      recent_success_rate: 0.75,
      sample_size: 4,
      average_duration_ms: 1200,
      observed_costs: [],
      required_dependency_health: [],
      drift_state: "HEALTHY",
      drift_evidence_refs: [],
    },
    drift_state: "HEALTHY",
    drift_evidence_refs: [],
    ...overrides,
  };
}

test("the health projection is read from the Workspace-scoped health route", async () => {
  const requests: Array<{ path: string; init: RequestInit }> = [];
  const api = createAutomationApi("workspace-1", async (path, init) => {
    requests.push({ path, init });
    return Response.json(healthProjection());
  });

  const health = await api.getHealth("automation-1");

  assert.equal(requests.length, 1);
  assert.equal(requests[0].path, "/v1/automations/automation-1/health");
  assert.equal(new Headers(requests[0].init.headers).get("X-Workspace-ID"), "workspace-1");
  assert.equal(health.host_ready, true);
  assert.equal(health.occurrence_counts.skipped, 1);
  assert.equal(health.routine_health.recent_success_rate, 0.75);
  assert.equal(health.drift_state, "HEALTHY");
});

test("the health projection never invents a healthy verdict without an observation", async () => {
  const api = createAutomationApi("workspace-1", async () => Response.json(healthProjection({
    routine_health: {
      routine_id: "routine-1",
      current_revision: 4,
      last_run_at: null,
      last_success_at: null,
      recent_success_rate: null,
      sample_size: 0,
      average_duration_ms: null,
      observed_costs: [],
      required_dependency_health: [],
      drift_state: "UNKNOWN",
      drift_evidence_refs: [],
    },
    drift_state: "UNKNOWN",
  })));
  const health = await api.getHealth("automation-1");
  assert.equal(health.drift_state, "UNKNOWN");
  assert.equal(health.routine_health.recent_success_rate, null);
  assert.equal(health.routine_health.sample_size, 0);
});

test("a health projection for another definition is rejected", async () => {
  const api = createAutomationApi("workspace-1", async () => Response.json(healthProjection({ automation_id: "automation-2" })));
  await assert.rejects(api.getHealth("automation-1"), /different definition/);
});

test("the run history reports the persisted occurrence and its Task", async () => {
  const requests: string[] = [];
  const api = createAutomationApi("workspace-1", async (path) => {
    requests.push(path);
    return Response.json({
      items: [{
        occurrence_id: "occurrence-1",
        automation_id: "automation-1",
        automation_revision: 2,
        routine_id: "routine-1",
        routine_revision: 4,
        trigger_id: "trigger-daily",
        trigger_host_runtime_id: "runtime-1",
        occurrence_key: "a".repeat(64),
        version: 3,
        claim_epoch: 1,
        claim_expires_at: null,
        status: "STARTED",
        task_id: "task-1",
        scheduled_for: "2026-03-01T09:00:00Z",
        covered_misfire_range: null,
        trigger_input_ref: null,
        trigger_payload_digest: null,
        blockers: [],
        created_at: "2026-03-01T09:00:00Z",
        updated_at: "2026-03-01T09:00:01Z",
      }],
      next_cursor: null,
    });
  });

  const page = await api.listOccurrences("automation-1");

  assert.equal(requests[0], "/v1/automations/automation-1/occurrences?limit=50");
  assert.equal(page.items.length, 1);
  assert.equal(page.items[0].status, "STARTED");
  assert.equal(page.items[0].task_id, "task-1");
  assert.equal(page.next_cursor, null);
});

test("an unknown occurrence status is rejected instead of rendered", async () => {
  const api = createAutomationApi("workspace-1", async () => Response.json({
    items: [{
      occurrence_id: "occurrence-1",
      automation_id: "automation-1",
      automation_revision: 2,
      routine_id: "routine-1",
      routine_revision: 4,
      trigger_id: "trigger-daily",
      trigger_host_runtime_id: "runtime-1",
      occurrence_key: "a".repeat(64),
      version: 1,
      claim_epoch: 0,
      claim_expires_at: null,
      status: "SOMETHING_ELSE",
      task_id: null,
      scheduled_for: null,
      covered_misfire_range: null,
      trigger_input_ref: null,
      trigger_payload_digest: null,
      blockers: [],
      created_at: "2026-03-01T09:00:00Z",
      updated_at: "2026-03-01T09:00:01Z",
    }],
  }));
  await assert.rejects(api.listOccurrences("automation-1"), /Invalid Automation occurrence status/);
});
