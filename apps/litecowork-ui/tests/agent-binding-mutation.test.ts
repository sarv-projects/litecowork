import assert from "node:assert/strict";
import test from "node:test";

import {
  AgentBindingRefreshFence,
  agentBindingMutationFailureMessage,
  agentActionFeedbackForWorkspace,
  agentBindingMutationSuccessMessage,
  clearAgentActionBusyIfMatches,
  getOrCreateAgentBindingMutationRequest,
  mergeAgentBindingProjection,
  mergeAgentBindingProjections,
  reconcileAgentBindingMutationRequests,
  updateAgentActionFeedback,
  type PendingAgentBindingMutation,
} from "../src/agents/agent-binding-mutation.ts";

test("ambiguous binding mutation retry reuses its exact request identity", () => {
  const pending = new Map<string, PendingAgentBindingMutation>();
  const identity = {
    operation: "disable" as const,
    workspaceId: "workspace-a",
    agentBindingId: "binding-a",
    expectedVersion: 7,
  };

  const first = getOrCreateAgentBindingMutationRequest(pending, identity, () => "request-a");
  const retry = getOrCreateAgentBindingMutationRequest(pending, identity, () => "request-b");

  assert.strictEqual(retry, first);
  assert.equal(retry.requestId, "request-a");
});

test("binding mutation retry identity is fenced by operation, Workspace, binding, and version", () => {
  const pending = new Map<string, PendingAgentBindingMutation>();
  const requestIds = ["request-disable", "request-enable", "request-other-workspace", "request-new-version"];
  const nextId = () => requestIds.shift()!;
  const original = getOrCreateAgentBindingMutationRequest(pending, {
    operation: "disable",
    workspaceId: "workspace-a",
    agentBindingId: "binding-a",
    expectedVersion: 7,
  }, nextId);

  assert.notEqual(getOrCreateAgentBindingMutationRequest(pending, {
    operation: "enable",
    workspaceId: "workspace-a",
    agentBindingId: "binding-a",
    expectedVersion: 7,
  }, nextId).requestId, original.requestId);
  assert.notEqual(getOrCreateAgentBindingMutationRequest(pending, {
    operation: "disable",
    workspaceId: "workspace-b",
    agentBindingId: "binding-a",
    expectedVersion: 7,
  }, nextId).requestId, original.requestId);
  assert.notEqual(getOrCreateAgentBindingMutationRequest(pending, {
    operation: "disable",
    workspaceId: "workspace-a",
    agentBindingId: "binding-a",
    expectedVersion: 8,
  }, nextId).requestId, original.requestId);
});

test("stale binding projections do not retire an ambiguous mutation but a newer version does", () => {
  const pending = new Map<string, PendingAgentBindingMutation>();
  getOrCreateAgentBindingMutationRequest(pending, {
    operation: "disable",
    workspaceId: "workspace-a",
    agentBindingId: "binding-a",
    expectedVersion: 7,
  }, () => "request-a");

  reconcileAgentBindingMutationRequests(pending, "workspace-a", [{
    agentBindingId: "binding-a",
    version: 7,
  }]);
  assert.equal(pending.size, 1);

  reconcileAgentBindingMutationRequests(pending, "workspace-b", [{
    agentBindingId: "binding-a",
    version: 8,
  }]);
  assert.equal(pending.size, 1);

  reconcileAgentBindingMutationRequests(pending, "workspace-a", [{
    agentBindingId: "binding-a",
    version: 8,
  }]);
  assert.equal(pending.size, 0);
});

test("refreshes for one Workspace do not invalidate another Workspace's in-flight binding list", () => {
  const fence = new AgentBindingRefreshFence();
  const workspaceARequest = fence.begin("workspace-a");
  const workspaceBRequest = fence.begin("workspace-b");
  const workspaceARetry = fence.begin("workspace-a");

  assert.equal(fence.isCurrent("workspace-a", workspaceARequest, "workspace-b"), false);
  assert.equal(fence.isCurrent("workspace-b", workspaceBRequest, "workspace-b"), true);
  assert.equal(fence.isCurrent("workspace-a", workspaceARetry, "workspace-b"), false);
  assert.equal(fence.isCurrent("workspace-a", workspaceARetry, "workspace-a"), true);
});

test("a late older binding mutation response cannot replace a newer Workspace projection", () => {
  const latest = { workspaceId: "workspace-a", agentBindingId: "binding-a", version: 3, enabled: true };
  const staleResponse = { workspaceId: "workspace-a", agentBindingId: "binding-a", version: 2, enabled: false };

  assert.deepEqual(mergeAgentBindingProjection([latest], staleResponse), [latest]);
  assert.deepEqual(mergeAgentBindingProjection([latest], { ...staleResponse, version: 4 }), [
    { ...staleResponse, version: 4 },
  ]);
  assert.deepEqual(mergeAgentBindingProjection([latest], { ...staleResponse, workspaceId: "workspace-b", version: 4 }), [latest]);
});

test("an older binding list refresh cannot regress a newer mutation projection", () => {
  const latest = { workspaceId: "workspace-a", agentBindingId: "binding-a", version: 4, enabled: true };
  const olderPage = [{ ...latest, version: 3, enabled: false }];

  assert.deepEqual(mergeAgentBindingProjections([latest], olderPage), [latest]);
  assert.deepEqual(mergeAgentBindingProjections([], olderPage), olderPage);
});

test("mutation success feedback follows the newest refreshed binding state", () => {
  assert.match(agentBindingMutationSuccessMessage("disable", {
    agentBindingId: "binding-a",
    version: 3,
    enabled: false,
  }), /cannot use this agent/i);
  assert.match(agentBindingMutationSuccessMessage("disable", {
    agentBindingId: "binding-a",
    version: 4,
    enabled: true,
  }), /changed again.*review its current status/i);
  assert.match(agentBindingMutationSuccessMessage("disable", undefined), /could not be confirmed/i);
});

test("agent action feedback is retained per Workspace and only projects into its own settings", () => {
  const workspaceA = updateAgentActionFeedback({}, "workspace-a", {
    message: "Binding disabled.",
    busy: null,
  });
  const both = updateAgentActionFeedback(workspaceA, "workspace-b", {
    error: "Profile refresh failed.",
    busy: "probe",
  });

  assert.deepEqual(agentActionFeedbackForWorkspace(both, "workspace-a"), {
    message: "Binding disabled.",
    error: null,
    busy: null,
  });
  assert.deepEqual(agentActionFeedbackForWorkspace(both, "workspace-b"), {
    message: null,
    error: "Profile refresh failed.",
    busy: "probe",
  });
  assert.deepEqual(agentActionFeedbackForWorkspace(both, null), {
    message: null,
    error: null,
    busy: null,
  });
  const cleared = clearAgentActionBusyIfMatches(both, "workspace-b", "probe");
  assert.equal(agentActionFeedbackForWorkspace(cleared, "workspace-b").busy, null);
  assert.equal(agentActionFeedbackForWorkspace(cleared, "workspace-a").message, "Binding disabled.");
  assert.strictEqual(clearAgentActionBusyIfMatches(both, "workspace-b", "create:profile"), both, "a late action cannot clear a newer action's busy state");
});

test("binding mutation conflicts explain the current default before generic retry guidance", () => {
  assert.equal(
    agentBindingMutationFailureMessage("disable", "binding-a", 2, { workspaceId: "workspace-a", agentBindingId: "binding-a", version: 2 }, "binding-a"),
    "Clear or change the Workspace default before disabling this agent.",
  );
  assert.match(
    agentBindingMutationFailureMessage("disable", "binding-a", 2, { workspaceId: "workspace-a", agentBindingId: "binding-a", version: 3 }, null),
    /binding changed/i,
  );
});
