import assert from "node:assert/strict";
import test from "node:test";
import {
  isResourceIndexRebuildReceipt,
  performResourceIndexRebuild,
  type ResourceIndexRebuildPin,
  type ResourceIndexRebuildReceipt,
} from "../src/resources/resource-index-rebuild.ts";

const pin: ResourceIndexRebuildPin = {
  workspaceId: "workspace-1",
  resourceId: "resource-1",
  resourceRevisionId: "revision-1",
  contentDigest: `sha256:${"a".repeat(64)}`,
};

const receipt: ResourceIndexRebuildReceipt = {
  ...pin,
  requestId: "request-1",
  correlationId: "correlation-1",
  outcome: "INDEXED",
  reason: null,
};

test("ambiguous rebuild retries reuse the original request ID and exact revision pins", async () => {
  const requests = new Map<string, string>();
  const executed: Array<{ requestId: string; pin: ResourceIndexRebuildPin }> = [];
  let failFirst = true;
  const run = () => performResourceIndexRebuild({
    requests,
    pin,
    createRequestId: () => "request-1",
    execute: async (requestId, exactPin) => {
      executed.push({ requestId, pin: exactPin });
      if (failFirst) {
        failFirst = false;
        throw new Error("local service disconnected");
      }
      return receipt;
    },
  });

  const failed = await run();
  assert.equal(failed.kind, "failed");
  if (failed.kind !== "failed") return;
  assert.deepEqual(failed.retryPin, pin);
  assert.equal(requests.size, 1);

  const confirmed = await run();
  assert.equal(confirmed.kind, "confirmed");
  assert.deepEqual(executed, [
    { requestId: "request-1", pin },
    { requestId: "request-1", pin },
  ]);
  assert.equal(requests.size, 0);
});

test("a mismatched receipt stays unconfirmed and keeps the exact request for replay", async () => {
  const requests = new Map<string, string>();
  const failed = await performResourceIndexRebuild({
    requests,
    pin,
    createRequestId: () => "request-1",
    execute: async () => ({ ...receipt, resourceRevisionId: "revision-2" }),
  });

  assert.equal(failed.kind, "failed");
  if (failed.kind !== "failed") return;
  assert.deepEqual(failed.retryPin, pin);
  assert.match(failed.message, /different Resource revision/);
  assert.equal(requests.size, 1);
});

test("receipt validation rejects wrong request IDs and invalid outcome/reason combinations", () => {
  assert.equal(isResourceIndexRebuildReceipt(receipt, pin, "request-1"), true);
  assert.equal(isResourceIndexRebuildReceipt({ ...receipt, requestId: "other-request" }, pin, "request-1"), false);
  assert.equal(isResourceIndexRebuildReceipt({ ...receipt, correlationId: "" }, pin, "request-1"), false);
  assert.equal(isResourceIndexRebuildReceipt({ ...receipt, outcome: "UNKNOWN" } as never, pin, "request-1"), false);
  assert.equal(isResourceIndexRebuildReceipt({ ...receipt, outcome: "INDEXED", reason: "INVALID_UTF8" }, pin, "request-1"), false);
  assert.equal(isResourceIndexRebuildReceipt(null as never, pin, "request-1"), false);
});

test("a stale pin requires Library reload and clears only that exact retry identity", async () => {
  const requests = new Map<string, string>();
  const otherPin = { ...pin, resourceId: "resource-2" };
  const keep = await performResourceIndexRebuild({
    requests,
    pin: otherPin,
    createRequestId: () => "other-request",
    execute: async () => { throw new Error("Resource head changed. Reload the Library before retrying."); },
  });
  assert.equal(keep.kind, "failed");
  if (keep.kind !== "failed") return;
  assert.equal(keep.retryPin, null);
  assert.equal(requests.size, 0);
});

test("a fresh UI session starts a new retry identity without inheriting durable index health", async () => {
  const requests = new Map<string, string>();
  const firstIds: string[] = [];
  const first = await performResourceIndexRebuild({
    requests,
    pin,
    createRequestId: () => "request-1",
    execute: async (requestId) => { firstIds.push(requestId); throw new Error("temporary key access failure"); },
  });
  assert.equal(first.kind, "failed");
  assert.equal(requests.size, 1);
  assert.equal(requests.has(JSON.stringify([pin.workspaceId, pin.resourceId, pin.resourceRevisionId, pin.contentDigest])), true);

  const nextSessionIds: string[] = [];
  const nextSession = await performResourceIndexRebuild({
    requests: new Map(),
    pin,
    createRequestId: () => "request-2",
    execute: async (requestId) => { nextSessionIds.push(requestId); throw new Error("temporary key access failure"); },
  });
  assert.equal(nextSession.kind, "failed");
  assert.deepEqual(firstIds, ["request-1"]);
  assert.deepEqual(nextSessionIds, ["request-2"]);
});
