import assert from "node:assert/strict";
import { test } from "node:test";

const policyOrder = await import("../src/coworkers/lead-failover-order.ts");

test("moving a fallback lead changes its actual policy priority without changing the input", () => {
  assert.equal(typeof policyOrder.moveFallbackLead, "function");
  const initial = ["binding-fast", "binding-review", "binding-backup"];
  const actual = policyOrder.moveFallbackLead(initial, "binding-backup", -2);
  assert.deepEqual(actual, ["binding-backup", "binding-fast", "binding-review"]);
  assert.deepEqual(initial, ["binding-fast", "binding-review", "binding-backup"]);
});

test("unknown fallback and out-of-range moves leave the selected order unchanged", () => {
  assert.equal(typeof policyOrder.moveFallbackLead, "function");
  const initial = ["binding-fast", "binding-review"];
  assert.deepEqual(policyOrder.moveFallbackLead(initial, "missing", -1), initial);
  assert.deepEqual(policyOrder.moveFallbackLead(initial, "binding-fast", -1), initial);
  assert.deepEqual(policyOrder.moveFallbackLead(initial, "binding-review", 1), initial);
});

test("changing the Coworker lead removes it from its own fallback order", () => {
  assert.equal(typeof policyOrder.excludeCurrentLeadFallbacks, "function");
  const existing = ["binding-fast", "binding-review", "binding-fast"];
  const updated = policyOrder.excludeCurrentLeadFallbacks(existing, "binding-fast");
  assert.deepEqual(updated, ["binding-review"]);
  assert.deepEqual(existing, ["binding-fast", "binding-review", "binding-fast"]);
});

test("default Workspace lead does not hide stale fallback entries", () => {
  assert.deepEqual(policyOrder.excludeCurrentLeadFallbacks(["binding-old", "binding-a"], null), ["binding-old", "binding-a"]);
});

test("lead failover draft validation enforces bounded and internally consistent policy", () => {
  const allowed = {
    mode: "ALLOW_LISTED",
    triggers: ["AGENT_UNAVAILABLE"],
    fallback_agent_binding_ids: ["binding-a", "binding-b"],
    max_lead_changes: 2,
  };
  assert.equal(policyOrder.isValidLeadFailoverDraft(allowed, "lead"), true);
  assert.equal(policyOrder.isValidLeadFailoverDraft({ ...allowed, max_lead_changes: 3 }, "lead"), false);
  assert.equal(policyOrder.isValidLeadFailoverDraft({ ...allowed, fallback_agent_binding_ids: ["binding-a", "binding-a"] }, "lead"), false);
  assert.equal(policyOrder.isValidLeadFailoverDraft({ ...allowed, fallback_agent_binding_ids: ["lead"] }, "lead"), false);
  assert.equal(policyOrder.isValidLeadFailoverDraft({ ...allowed, triggers: [] }, "lead"), false);
  assert.equal(policyOrder.isValidLeadFailoverDraft({ ...allowed, fallback_agent_binding_ids: ["a", "b", "c", "d"] }, "lead"), false);
});

test("ask mode keeps its ordered fallback choices and never allows an automatic change", () => {
  // `ASK` presents the ordered fallback list as the owner's choices, so the list is
  // required even though no automatic lead change is permitted.
  const ask = {
    mode: "ASK",
    triggers: ["AGENT_UNAVAILABLE"],
    fallback_agent_binding_ids: ["binding-a", "binding-b"],
    max_lead_changes: 0,
  };
  assert.equal(policyOrder.isValidLeadFailoverDraft(ask, "lead"), true);
  assert.equal(policyOrder.isValidLeadFailoverDraft({ ...ask, fallback_agent_binding_ids: [] }, "lead"), false);
  assert.equal(policyOrder.isValidLeadFailoverDraft({ ...ask, max_lead_changes: 1 }, "lead"), false);
});
