import assert from "node:assert/strict";
import test from "node:test";
import {
  clearWorkspaceRootStatusRequest,
  isCurrentWorkspaceRootRequest,
  isWorkspaceRootStatusResponse,
  performWorkspaceRootStatusAction,
  resourceLocationAvailabilityLabel,
  workspaceRootStatusRequest,
} from "../src/workspaces/workspace-root-status.ts";
import {
  isWorkspaceRootAddResponse,
  isWorkspaceRootRevokeResponse,
  workspaceRootMutationRequest,
} from "../src/workspaces/workspace-root-mutations.ts";
import { isWorkspaceRootProjection } from "../src/workspaces/workspace-root-status.ts";

const request = {
  workspaceId: "workspace-1",
  workspaceRootId: "root-1",
  action: "resume" as const,
  expectedVersion: 4,
};

test("WorkspaceRoot add and revoke retries reuse IDs for the exact mutation envelope", () => {
  const requests = new Map<string, string>();
  let created = 0;
  const createId = () => `mutation-${++created}`;
  const add = { action: "add" as const, workspaceId: "workspace-1", expectedVersion: 7, watchPolicy: "METADATA", replicationPolicy: "NONE" };
  const revoke = { action: "revoke" as const, workspaceId: "workspace-1", workspaceRootId: "root-1", expectedVersion: 3 };
  const addFirst = workspaceRootMutationRequest(requests, add, createId);

  assert.deepEqual(workspaceRootMutationRequest(requests, add, createId), addFirst);
  const changedAdd = workspaceRootMutationRequest(requests, { ...add, expectedVersion: 8 }, createId);
  const revokeFirst = workspaceRootMutationRequest(requests, revoke, createId);
  const changedRevoke = workspaceRootMutationRequest(requests, { ...revoke, expectedVersion: 4 }, createId);
  assert.equal(new Set([addFirst.requestId, changedAdd.requestId, revokeFirst.requestId, changedRevoke.requestId]).size, 4);
});

test("validates Add and Revoke receipts before applying them to the Library", () => {
  const root = { ...resultRoot, version: 1 };
  const add = { action: "add" as const, workspaceId: "workspace-1", expectedVersion: 7, watchPolicy: "METADATA", replicationPolicy: "NONE" };
  const revoke = { action: "revoke" as const, workspaceId: "workspace-1", workspaceRootId: "root-1", expectedVersion: 3 };
  assert.equal(isWorkspaceRootAddResponse(root, add), true);
  assert.equal(isWorkspaceRootAddResponse({ ...root, workspaceId: "workspace-2" }, add), false);
  assert.equal(isWorkspaceRootAddResponse({ ...root, status: "PAUSED" }, add), false);
  assert.equal(isWorkspaceRootAddResponse({ ...root, version: 0 }, add), false);
  assert.equal(isWorkspaceRootAddResponse({ ...root, replicationPolicy: "INVALID" }, add), false);
  assert.equal(isWorkspaceRootAddResponse(null as never, add), false);
  assert.equal(isWorkspaceRootAddResponse("invalid" as never, add), false);
  assert.equal(isWorkspaceRootRevokeResponse({ ...root, status: "REVOKED", version: 4, locationAvailability: "REVOKED" }, revoke), true);
  assert.equal(isWorkspaceRootRevokeResponse({ ...root, workspaceRootId: "root-2", status: "REVOKED", version: 4, locationAvailability: "REVOKED" }, revoke), false);
  assert.equal(isWorkspaceRootRevokeResponse({ ...root, status: "ACTIVE", version: 4, locationAvailability: "REVOKED" }, revoke), false);
  assert.equal(isWorkspaceRootRevokeResponse({ ...root, status: "REVOKED", version: 5, locationAvailability: "REVOKED" }, revoke), false);
  assert.equal(isWorkspaceRootRevokeResponse(undefined as never, revoke), false);
});

test("validates every refreshed WorkspaceRoot before it can replace a Library row", () => {
  assert.equal(isWorkspaceRootProjection(resultRoot, "workspace-1"), true);
  assert.equal(isWorkspaceRootProjection({ ...resultRoot, workspaceId: "workspace-2" }, "workspace-1"), false);
  assert.equal(isWorkspaceRootProjection({ ...resultRoot, status: "BROKEN" }, "workspace-1"), false);
  assert.equal(isWorkspaceRootProjection({ ...resultRoot, locationAvailability: "BROKEN" }, "workspace-1"), false);
  assert.equal(isWorkspaceRootProjection({ ...resultRoot, displayName: " " }, "workspace-1"), false);
  assert.equal(isWorkspaceRootProjection({ ...resultRoot, version: Number.MAX_SAFE_INTEGER + 1 }, "workspace-1"), false);
  assert.equal(isWorkspaceRootProjection(null as never, "workspace-1"), false);
});

test("reuses the same status request ID until a confirmed result clears it", () => {
  const requests = new Map<string, string>();
  let nextId = 0;
  const createId = () => `request-${++nextId}`;

  const first = workspaceRootStatusRequest(requests, request, createId);
  const retry = workspaceRootStatusRequest(requests, request, createId);
  assert.equal(first.requestId, "request-1");
  assert.deepEqual(retry, first);
  assert.equal(nextId, 1);

  clearWorkspaceRootStatusRequest(requests, first.key);
  assert.equal(workspaceRootStatusRequest(requests, request, createId).requestId, "request-2");
});

test("separates status request IDs by Workspace, root, action, and expected version", () => {
  const requests = new Map<string, string>();
  let nextId = 0;
  const createId = () => `request-${++nextId}`;

  const first = workspaceRootStatusRequest(requests, request, createId);
  const changed = [
    { ...request, workspaceId: "workspace-2" },
    { ...request, workspaceRootId: "root-2" },
    { ...request, action: "pause" as const },
    { ...request, expectedVersion: 5 },
  ].map((input) => workspaceRootStatusRequest(requests, input, createId));

  assert.equal(new Set([first, ...changed].map((item) => item.requestId)).size, 5);
});

test("only accepts the expected committed root transition and available resume location", () => {
  const result = {
    workspaceId: "workspace-1",
    workspaceRootId: "root-1",
    resourceId: "resource-1",
    displayName: "Project",
    watchPolicy: "METADATA",
    replicationPolicy: "NONE",
    status: "ACTIVE",
    version: 5,
    locationAvailability: "AVAILABLE",
  };

  assert.equal(isWorkspaceRootStatusResponse(result, request), true);
  assert.equal(isWorkspaceRootStatusResponse({ ...result, workspaceId: "workspace-2" }, request), false);
  assert.equal(isWorkspaceRootStatusResponse({ ...result, workspaceRootId: "root-2" }, request), false);
  assert.equal(isWorkspaceRootStatusResponse({ ...result, version: 6 }, request), false);
  assert.equal(isWorkspaceRootStatusResponse({ ...result, locationAvailability: "UNAVAILABLE" }, request), false);
  assert.equal(isWorkspaceRootStatusResponse({ ...result, resourceId: "" }, request), false);
  assert.equal(isWorkspaceRootStatusResponse({ ...result, displayName: " " }, request), false);
  assert.equal(isWorkspaceRootStatusResponse({ ...result, watchPolicy: "UNRECOGNIZED" }, request), false);
  assert.equal(isWorkspaceRootStatusResponse({ ...result, locationAvailability: "UNRECOGNIZED" }, request), false);
  assert.equal(isWorkspaceRootStatusResponse({ ...result, status: "PAUSED" }, request), false);
});

test("does not apply an in-flight status response after Workspace or list generation changes", () => {
  assert.equal(isCurrentWorkspaceRootRequest("workspace-1", 8, "workspace-1", 8), true);
  assert.equal(isCurrentWorkspaceRootRequest("workspace-1", 8, "workspace-2", 8), false);
  assert.equal(isCurrentWorkspaceRootRequest("workspace-1", 8, "workspace-1", 9), false);
});

test("labels the last committed location availability separately from root lifecycle status", () => {
  assert.equal(resourceLocationAvailabilityLabel("AVAILABLE"), "Last observed: available");
  assert.equal(resourceLocationAvailabilityLabel("OFFLINE"), "Last observed: offline");
  assert.equal(resourceLocationAvailabilityLabel("PLACEHOLDER"), "Last observed: placeholder");
  assert.equal(resourceLocationAvailabilityLabel("REVOKED"), "Last observed: revoked");
  assert.equal(resourceLocationAvailabilityLabel("UNKNOWN"), "Last observed: unknown");
  assert.equal(resourceLocationAvailabilityLabel("UNAVAILABLE"), "Last observed: unavailable");
});

test("reloads every needed page after an ambiguous action and reuses its request ID for retry", async () => {
  const requests = new Map<string, string>();
  const executedIds: string[] = [];
  const pages: Array<{ items: Array<typeof resultRoot>; nextCursor: string | null }> = [
    { items: [{ ...resultRoot, workspaceRootId: "other-root" }], nextCursor: "page-2" },
    { items: [resultRoot], nextCursor: null },
  ];
  let listCalls = 0;
  let failFirstAction = true;
  const run = () => performWorkspaceRootStatusAction({
    requests,
    request,
    createRequestId: () => "stable-request-id",
    isCurrent: () => true,
    execute: async (requestId) => {
      executedIds.push(requestId);
      if (failFirstAction) {
        failFirstAction = false;
        throw new Error("response lost");
      }
      return resultRoot;
    },
    list: async () => pages[listCalls++],
  });

  const uncertain = await run();
  assert.equal(uncertain.kind, "uncertain");
  if (uncertain.kind !== "uncertain") return;
  assert.equal(uncertain.targetFound, true);
  assert.equal(uncertain.refreshedRoots.at(-1)?.workspaceRootId, request.workspaceRootId);
  assert.equal(uncertain.refreshError, null);
  assert.equal(listCalls, 2);
  assert.equal(requests.size, 1);

  const confirmed = await run();
  assert.equal(confirmed.kind, "confirmed");
  assert.deepEqual(executedIds, ["stable-request-id", "stable-request-id"]);
  assert.equal(requests.size, 0);
});

test("reloads after a malformed committed response without accepting it", async () => {
  const requests = new Map<string, string>();
  let listCalls = 0;
  const outcome = await performWorkspaceRootStatusAction({
    requests,
    request,
    createRequestId: () => "malformed-request-id",
    isCurrent: () => true,
    execute: async () => ({ ...resultRoot, version: resultRoot.version + 1 }),
    list: async () => {
      listCalls += 1;
      return { items: [resultRoot], nextCursor: null };
    },
  });

  assert.equal(outcome.kind, "uncertain");
  if (outcome.kind !== "uncertain") return;
  assert.match(outcome.failureMessage, /did not confirm/);
  assert.equal(outcome.targetFound, true);
  assert.equal(listCalls, 1);
  assert.equal(requests.size, 1);
});

test("stops before reloading when the Workspace changes during the action", async () => {
  const requests = new Map<string, string>();
  let current = true;
  let listCalls = 0;
  const outcome = await performWorkspaceRootStatusAction({
    requests,
    request,
    createRequestId: () => "stale-request-id",
    isCurrent: () => current,
    execute: async () => {
      current = false;
      throw new Error("late response");
    },
    list: async () => {
      listCalls += 1;
      return { items: [], nextCursor: null };
    },
  });

  assert.deepEqual(outcome, { kind: "stale" });
  assert.equal(listCalls, 0);
});

test("reports a repeated cursor as a failed refresh and preserves the action identity", async () => {
  const requests = new Map<string, string>();
  const outcome = await performWorkspaceRootStatusAction({
    requests,
    request,
    createRequestId: () => "repeated-cursor-request-id",
    isCurrent: () => true,
    execute: async () => { throw new Error("response lost"); },
    list: async () => ({ items: [], nextCursor: "same-cursor" }),
  });

  assert.equal(outcome.kind, "uncertain");
  if (outcome.kind !== "uncertain") return;
  assert.match(outcome.refreshError ?? "", /repeated/);
  assert.equal(requests.size, 1);
});

test("rejects a malformed same-Workspace root during status recovery instead of returning it for merge", async () => {
  const requests = new Map<string, string>();
  const outcome = await performWorkspaceRootStatusAction({
    requests,
    request,
    createRequestId: () => "malformed-list-request-id",
    isCurrent: () => true,
    execute: async () => { throw new Error("response lost"); },
    list: async () => ({ items: [{ ...resultRoot, status: "BROKEN" }], nextCursor: null }),
  });

  assert.equal(outcome.kind, "uncertain");
  if (outcome.kind !== "uncertain") return;
  assert.equal(outcome.refreshedRoots.length, 0);
  assert.match(outcome.refreshError ?? "", /invalid folder list/);
  assert.equal(requests.size, 1);
});

const resultRoot = {
  workspaceId: "workspace-1",
  workspaceRootId: "root-1",
  resourceId: "resource-1",
  watchPolicy: "METADATA",
  replicationPolicy: "NONE",
  status: "ACTIVE",
  version: 5,
  locationAvailability: "AVAILABLE",
  displayName: "Project",
};
