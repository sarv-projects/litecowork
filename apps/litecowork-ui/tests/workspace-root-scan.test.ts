import assert from "node:assert/strict";
import test from "node:test";
import {
  acceptWorkspaceRootScanResponse,
  isCurrentWorkspaceRootScan,
  isWorkspaceRootScanResponse,
  isWorkspaceRootScannable,
  workspaceRootScanFailure,
  workspaceRootScanAnnouncement,
  workspaceRootScanRequest,
} from "../src/workspaces/workspace-root-scan.ts";

const scanInput = {
  workspaceId: "workspace-1",
  workspaceRootId: "root-1",
  expectedVersion: 4,
};

const currentRoot = {
  workspaceId: "workspace-1",
  workspaceRootId: "root-1",
  version: 4,
  status: "ACTIVE",
  locationAvailability: "AVAILABLE",
} as const;

function summary(requestId: string) {
  return {
    workspaceId: "workspace-1",
    workspaceRootId: "root-1",
    requestId,
    rootVersion: 4,
    status: "COMPLETE" as const,
    entriesExamined: 5,
    filesObserved: 3,
    partialReason: null,
    completedAt: "2026-10-16T12:00:00.000Z",
  };
}

test("only an ACTIVE, AVAILABLE root in the selected Workspace can be scanned", () => {
  assert.equal(isWorkspaceRootScannable(currentRoot, "workspace-1"), true);
  assert.equal(isWorkspaceRootScannable(currentRoot, "workspace-2"), false);
  assert.equal(isWorkspaceRootScannable({ ...currentRoot, status: "PAUSED" }, "workspace-1"), false);
  assert.equal(isWorkspaceRootScannable({ ...currentRoot, locationAvailability: "OFFLINE" }, "workspace-1"), false);
  assert.equal(isWorkspaceRootScannable({ ...currentRoot, version: Number.MAX_SAFE_INTEGER + 1 }, "workspace-1"), false);
  assert.equal(isWorkspaceRootScannable({ ...currentRoot, workspaceRootId: "" }, "workspace-1"), false);
  assert.equal(isWorkspaceRootScannable(null as never, "workspace-1"), false);
});

test("ambiguous retries reuse the request ID for the same Workspace, root, and version", () => {
  const pending = new Map<string, string>();
  let nextId = 0;
  const createId = () => `scan-${++nextId}`;

  const first = workspaceRootScanRequest(pending, scanInput, createId);
  const retry = workspaceRootScanRequest(pending, scanInput, createId);
  const changedVersion = workspaceRootScanRequest(pending, { ...scanInput, expectedVersion: 5 }, createId);
  const changedWorkspace = workspaceRootScanRequest(pending, { ...scanInput, workspaceId: "workspace-2" }, createId);

  assert.equal(first.requestId, "scan-1");
  assert.deepEqual(retry, first);
  assert.equal(changedVersion.requestId, "scan-2");
  assert.equal(changedWorkspace.requestId, "scan-3");
});

test("invalid scan responses keep the exact request ID for retry; a valid summary clears it", () => {
  const pending = new Map<string, string>();
  let nextId = 0;
  const createId = () => `scan-${++nextId}`;
  const request = workspaceRootScanRequest(pending, scanInput, createId);

  assert.equal(acceptWorkspaceRootScanResponse(pending, request, { ...summary(request.requestId), requestId: "wrong-request" }), false);
  assert.deepEqual(workspaceRootScanRequest(pending, scanInput, createId), request);
  assert.equal(acceptWorkspaceRootScanResponse(pending, request, summary(request.requestId)), true);
  assert.equal(workspaceRootScanRequest(pending, scanInput, createId).requestId, "scan-2");
});

test("validates scan identity, version, status, counts, reason, and completion time", () => {
  const pending = new Map<string, string>();
  const request = workspaceRootScanRequest(pending, scanInput, () => "scan-1");
  const valid = summary(request.requestId);

  assert.equal(isWorkspaceRootScanResponse(valid, request), true);
  assert.equal(isWorkspaceRootScanResponse({ ...valid, workspaceId: "workspace-2" }, request), false);
  assert.equal(isWorkspaceRootScanResponse({ ...valid, workspaceRootId: "root-2" }, request), false);
  assert.equal(isWorkspaceRootScanResponse({ ...valid, rootVersion: 5 }, request), false);
  assert.equal(isWorkspaceRootScanResponse({ ...valid, status: "RUNNING" }, request), false);
  assert.equal(isWorkspaceRootScanResponse({ ...valid, entriesExamined: -1 }, request), false);
  assert.equal(isWorkspaceRootScanResponse({ ...valid, filesObserved: 6 }, request), false);
  assert.equal(isWorkspaceRootScanResponse({ ...valid, partialReason: {} }, request), false);
  assert.equal(isWorkspaceRootScanResponse({ ...valid, completedAt: "not-a-time" }, request), false);
  assert.equal(isWorkspaceRootScanResponse(null, request), false);
});

test("rejects scan summaries over the traversal cap or with arbitrary partial reasons", () => {
  const request = workspaceRootScanRequest(new Map(), scanInput, () => "scan-1");
  const valid = summary(request.requestId);

  assert.equal(isWorkspaceRootScanResponse({ ...valid, entriesExamined: 1_000, filesObserved: 1_000 }, request), true);
  assert.equal(isWorkspaceRootScanResponse({ ...valid, entriesExamined: 1_001, filesObserved: 1_000 }, request), false);
  assert.equal(isWorkspaceRootScanResponse({
    ...valid,
    status: "PARTIAL",
    partialReason: "/private/customer-folder",
  }, request), false);
  assert.equal(isWorkspaceRootScanResponse({
    ...valid,
    status: "PARTIAL",
    partialReason: "human supplied reason",
  }, request), false);
  for (const partialReason of [
    "TIME_LIMIT_REACHED",
    "SCAN_BUDGET_REACHED",
    "ENTRY_CHANGED_OR_UNREADABLE",
    "MOUNT_BOUNDARY_REACHED",
  ] as const) {
    assert.equal(isWorkspaceRootScanResponse({ ...valid, status: "PARTIAL", partialReason }, request), true);
  }
});

test("late scan results are fenced by Workspace, root list generation, version, and availability", () => {
  const pending = new Map<string, string>();
  const request = workspaceRootScanRequest(pending, scanInput, () => "scan-1");

  assert.equal(isCurrentWorkspaceRootScan(request, 8, "workspace-1", 8, currentRoot), true);
  assert.equal(isCurrentWorkspaceRootScan(request, 8, "workspace-2", 8, currentRoot), false);
  assert.equal(isCurrentWorkspaceRootScan(request, 8, "workspace-1", 9, currentRoot), false);
  assert.equal(isCurrentWorkspaceRootScan(request, 8, "workspace-1", 8, { ...currentRoot, version: 5 }), false);
  assert.equal(isCurrentWorkspaceRootScan(request, 8, "workspace-1", 8, { ...currentRoot, status: "PAUSED" }), false);
  assert.equal(isCurrentWorkspaceRootScan(request, 8, "workspace-1", 8, { ...currentRoot, locationAvailability: "UNAVAILABLE" }), false);
  assert.equal(isCurrentWorkspaceRootScan(request, 8, "workspace-1", 8, null), false);
});

test("a typed stale-version error requires a root refresh even when the cached projection is unchanged", () => {
  const request = workspaceRootScanRequest(new Map(), scanInput, () => "scan-1");
  const failure = workspaceRootScanFailure("STALE_WORKSPACE_VERSION", request, currentRoot);

  assert.equal(failure.kind, "stale");
  assert.equal(failure.retrySameRequest, false);
  assert.match(failure.message, /leave and reopen the Library/i);
  assert.doesNotMatch(failure.message, /retry.*same scan/i);
});

test("deterministic scan failures do not suggest retrying the same request", () => {
  const request = workspaceRootScanRequest(new Map(), scanInput, () => "scan-1");
  const unsupported = workspaceRootScanFailure("SCAN_UNSUPPORTED_PLATFORM", request, currentRoot);
  assert.equal(unsupported.kind, "unsupported");
  assert.equal(unsupported.retrySameRequest, false);
  assert.match(unsupported.message, /not available on this platform/i);
  assert.doesNotMatch(unsupported.message, /retry/i);

  for (const code of [
    "INVALID_ARGUMENT",
    "NOT_FOUND",
    "RESOURCE_IDENTITY_UNAVAILABLE",
    "CONFLICT",
  ]) {
    const failure = workspaceRootScanFailure(code, request, currentRoot);
    assert.equal(failure.kind, "rejected", `${code} should be deterministic`);
    assert.equal(failure.retrySameRequest, false, `${code} should not advise retrying`);
    assert.doesNotMatch(failure.message, /retry.*same scan request/i, `${code} should not advise retrying`);
  }

  const unavailable = workspaceRootScanFailure("SCAN_UNAVAILABLE", request, currentRoot);
  assert.equal(unavailable.kind, "ambiguous");
  assert.equal(unavailable.retrySameRequest, true);
  assert.match(unavailable.message, /retry to check the same scan request/i);
});

test("scan announcements distinguish empty completion from partial counts and reason", () => {
  const empty = { ...summary("scan-1"), entriesExamined: 7, filesObserved: 0 };
  assert.equal(workspaceRootScanAnnouncement(empty), "No files were observed in the scanned entries. Examined 7 folder entries.");

  const complete = { ...summary("scan-1"), entriesExamined: 5, filesObserved: 3 };
  assert.equal(workspaceRootScanAnnouncement(complete), "Scan complete: counted metadata for 3 files in 5 folder entries.");

  const partial = {
    ...summary("scan-1"),
    status: "PARTIAL" as const,
    entriesExamined: 4,
    filesObserved: 2,
    partialReason: "SCAN_BUDGET_REACHED",
  };
  assert.equal(workspaceRootScanAnnouncement(partial), "Partial scan: counted metadata for 2 files in 4 folder entries. Reason: Scan size limit reached.");

  assert.doesNotMatch(workspaceRootScanAnnouncement(complete), /indexed|imported|deleted/i);
});
