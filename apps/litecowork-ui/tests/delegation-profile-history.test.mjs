import assert from "node:assert/strict";
import { test } from "node:test";
import { createDelegationProfileCatalogApi } from "../src/coworkers/delegation-profile-api.ts";

function revision(number) {
  return {
    delegation_profile_id: "profile-1",
    revision: number,
    workspace_id: "workspace-1",
    name: `Worker ${number}`,
    routing_description: `Revision ${number}`,
    instructions: null,
    authored_by: { kind: "USER", id: "owner-1" },
    created_at: `2026-10-16T10:0${number}:00Z`,
  };
}

test("loads immutable worker profile history in descending order across pages", async () => {
  const requests = [];
  const api = createDelegationProfileCatalogApi("workspace-1", async operation => {
    requests.push(operation.path);
    const cursor = new URL(operation.path, "https://operator.invalid").searchParams.get("cursor");
    return new Response(JSON.stringify(cursor
      ? { items: [revision(1)], next_cursor: null }
      : { items: [revision(3), revision(2)], next_cursor: "page-2" }), { status: 200 });
  });

  const history = await api.listRevisions("profile-1");

  assert.deepEqual(history.map(item => item.revision), [3, 2, 1]);
  assert.equal(requests.length, 2);
  assert.match(requests[0], /^\/v1\/delegation-profiles\/profile-1\/revisions\?/);
  assert.equal(new URL(requests[1], "https://operator.invalid").searchParams.get("cursor"), "page-2");
});

test("rejects history that repeats or reorders immutable revision numbers", async () => {
  const api = createDelegationProfileCatalogApi("workspace-1", async () => new Response(JSON.stringify({
    items: [revision(2), revision(3)],
    next_cursor: null,
  }), { status: 200 }));

  await assert.rejects(api.listRevisions("profile-1"), /out of order/);
});

test("loads a scope-checked, low-confidence performance projection without treating unknown outcomes as zero", async () => {
  const requests = [];
  const api = createDelegationProfileCatalogApi("workspace-1", async operation => {
    requests.push(operation.path);
    return new Response(JSON.stringify({
      delegation_profile_id: "profile-1",
      task_category: "RESEARCH",
      sample_count: 2,
      verifier_pass_rate: null,
      median_latency_ms: 1200,
      median_observed_cost: null,
      retry_rate: null,
      human_intervention_rate: null,
      failure_categories: [],
      confidence: "LOW",
    }), { status: 200 });
  });

  const projection = await api.getPerformance("profile-1", "RESEARCH");

  assert.equal(new URL(requests[0], "https://operator.invalid").searchParams.get("task_category"), "RESEARCH");
  assert.equal(projection.sample_count, 2);
  assert.equal(projection.verifier_pass_rate, null);
  assert.equal(projection.median_observed_cost, null);
  assert.equal(projection.confidence, "LOW");
});

test("rejects a performance projection returned for a different profile", async () => {
  const api = createDelegationProfileCatalogApi("workspace-1", async () => new Response(JSON.stringify({
    delegation_profile_id: "another-profile",
    task_category: "OTHER",
    sample_count: 0,
    verifier_pass_rate: null,
    median_latency_ms: null,
    median_observed_cost: null,
    retry_rate: null,
    human_intervention_rate: null,
    failure_categories: [],
    confidence: "LOW",
  }), { status: 200 }));

  await assert.rejects(api.getPerformance("profile-1"), /scope/i);
});

test("rejects a performance projection whose rates exceed the bounded range", async () => {
  const api = createDelegationProfileCatalogApi("workspace-1", async () => new Response(JSON.stringify({
    delegation_profile_id: "profile-1",
    task_category: "OTHER",
    sample_count: 1,
    verifier_pass_rate: 1.2,
    median_latency_ms: null,
    median_observed_cost: null,
    retry_rate: null,
    human_intervention_rate: null,
    failure_categories: [],
    confidence: "LOW",
  }), { status: 200 }));

  await assert.rejects(api.getPerformance("profile-1"), /rate/i);
});
