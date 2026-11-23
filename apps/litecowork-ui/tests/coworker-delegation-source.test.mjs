import assert from "node:assert/strict";
import { test } from "node:test";
import { createCoworkerApi } from "../src/coworkers/coworker-api.ts";

function coworker(revision = {}) {
  return {
    coworker_id: "coworker-1",
    workspace_id: "workspace-1",
    current_revision: 1,
    revision: {
      name: "Assistant",
      avatar_ref: null,
      role_description: "General help",
      default_lead_agent_binding_id: null,
      delegation_policy_source: "WORKSPACE_DEFAULT",
      delegation_strategy: "NATIVE_DEFAULT",
      enabled_delegation_profile_ids: [],
      delegation_budget_policy: null,
      lead_failover_policy: null,
      interaction_policy: {
        read_only_work: "STANDARD_TRUST_POLICY",
        draft_creation: "STANDARD_TRUST_POLICY",
        external_mutation: "REQUIRE_OWNER_APPROVAL",
        destructive_action: "HANDOFF_TO_OWNER",
        financial_commitment: "HANDOFF_TO_OWNER",
      },
      context_policy: {
        allowed_context_kinds: ["COWORKER_NOTES"],
        max_retrieved_items: 20,
        retain_task_summaries: true,
        require_user_confirmation_for_memory: true,
      },
      notification_policy: { blockers: "ALWAYS", completion: "ON_SUCCESS", failures: "ALWAYS" },
      ...revision,
    },
    status: "ACTIVE",
    is_primary: false,
    created_at: "2026-11-20T10:00:00Z",
    updated_at: "2026-11-20T10:00:00Z",
    version: 1,
  };
}

test("Coworker API preserves and validates the delegation source on reads", async () => {
  const api = createCoworkerApi("workspace-1", async () => new Response(JSON.stringify(coworker()), { status: 200 }));
  const saved = await api.get("coworker-1");
  assert.equal(saved.revision.delegation_policy_source, "WORKSPACE_DEFAULT");
  assert.equal(saved.revision.delegation_strategy, "NATIVE_DEFAULT");
  assert.deepEqual(saved.revision.enabled_delegation_profile_ids, []);

  const missingSourceApi = createCoworkerApi("workspace-1", async () => {
    const value = coworker();
    delete value.revision.delegation_policy_source;
    return new Response(JSON.stringify(value), { status: 200 });
  });
  await assert.rejects(missingSourceApi.get("coworker-1"), /Invalid Coworker API response/);
});

test("Coworker API rejects an inherited policy that contains an override", async () => {
  const api = createCoworkerApi("workspace-1", async () => new Response(JSON.stringify(coworker({
    delegation_strategy: "BALANCED",
  })), { status: 200 }));
  await assert.rejects(api.get("coworker-1"), /Workspace-default delegation policy/);
});

test("Coworker create forwards the selected source with its revision", async () => {
  let submitted;
  const response = coworker();
  const api = createCoworkerApi("workspace-1", async (_path, init) => {
    submitted = JSON.parse(init.body);
    return new Response(JSON.stringify(response), { status: 200 });
  });
  await api.create(response.revision, "request-1");
  assert.equal(submitted.workspace_id, "workspace-1");
  assert.equal(submitted.revision.delegation_policy_source, "WORKSPACE_DEFAULT");
});
