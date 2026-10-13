import assert from "node:assert/strict";
import { File } from "node:buffer";
import test from "node:test";
import {
  matchesResourceUploadIdentity,
  prepareResourceUploadCommitRequestId,
  receivedUploadRangesMatchFile,
  resumableResourceUploadSession,
  type ResourceUploadIdentity,
  type ResourceUploadSessionIdentity,
  type ResourceUploadReceivedRange,
} from "../src/resources/resource-upload-resume.ts";

const identity: ResourceUploadIdentity = {
  workspaceId: "workspace-a",
  displayName: "project/notes.txt",
  mediaType: "text/plain",
  folderRelativePath: "project/notes.txt",
  sizeBytes: 5,
  contextDocument: null,
};

function session(overrides: Partial<ResourceUploadSessionIdentity> = {}): ResourceUploadSessionIdentity {
  return {
    workspaceId: identity.workspaceId,
    displayName: identity.displayName,
    mediaType: identity.mediaType,
    folderRelativePath: identity.folderRelativePath,
    expectedSizeBytes: identity.sizeBytes,
    contextDocument: null,
    expectedDigest: `sha256:${"a".repeat(64)}`,
    chunkSizeBytes: 4 * 1024 * 1024,
    state: "OPEN",
    expiresAt: "2030-01-01T00:00:00Z",
    ...overrides,
  };
}

test("upload resume identity pins Workspace, path, bytes metadata, media type, and ContextDocument", () => {
  assert.equal(matchesResourceUploadIdentity(session(), identity), true);
  for (const changed of [
    { workspaceId: "workspace-b" },
    { displayName: "project/renamed.txt" },
    { mediaType: "application/octet-stream" },
    { folderRelativePath: "other/notes.txt" },
    { expectedSizeBytes: 6 },
    {
      contextDocument: {
        kind: "WORKSPACE_NOTES",
        owner_ref: { kind: "WORKSPACE", workspace_id: "workspace-a" },
      },
    },
  ]) {
    assert.equal(matchesResourceUploadIdentity(session(changed), identity), false);
  }
});

test("ContextDocument identity is exact and rejects malformed extra fields", () => {
  const contextDocument = {
    kind: "WORKSPACE_NOTES" as const,
    owner_ref: { kind: "WORKSPACE" as const, workspace_id: "workspace-a" },
  };
  const expected = { ...identity, contextDocument };
  assert.equal(matchesResourceUploadIdentity(session({ contextDocument }), expected), true);
  assert.equal(matchesResourceUploadIdentity(session({ contextDocument: { ...contextDocument, status: "ACTIVE" } }), expected), false);
  assert.equal(
    matchesResourceUploadIdentity(
      session({ contextDocument: { ...contextDocument, owner_ref: { ...contextDocument.owner_ref, unexpected: true } } }),
      expected,
    ),
    false,
  );
  for (const owner_ref of [null, "workspace-a", { kind: "WORKSPACE" }]) {
    assert.equal(matchesResourceUploadIdentity(session({ contextDocument: { ...contextDocument, owner_ref } }), expected), false);
  }
  assert.equal(matchesResourceUploadIdentity(session({ contextDocument: null }), expected), false);
});

test("only an exact live OPEN or CONTENT_RECEIVED session with a safe chunk size resumes", () => {
  const now = Date.parse("2026-10-12T00:00:00Z");
  const valid = session();
  assert.equal(resumableResourceUploadSession(valid, identity, valid.expectedDigest!, now), true);
  assert.equal(
    resumableResourceUploadSession(session({ state: "CONTENT_RECEIVED" }), identity, valid.expectedDigest!, now),
    true,
  );

  for (const invalid of [
    session({ expectedDigest: null }),
    session({ expectedDigest: `sha256:${"b".repeat(64)}` }),
    session({ state: "COMMITTED" }),
    session({ state: "FAILED" }),
    session({ state: "EXPIRED" }),
    session({ expiresAt: "2026-10-11T23:59:59Z" }),
    session({ expiresAt: "invalid" }),
    session({ chunkSizeBytes: 0 }),
    session({ chunkSizeBytes: 4 * 1024 * 1024 + 1 }),
    session({ workspaceId: "workspace-b" }),
  ]) {
    assert.equal(resumableResourceUploadSession(invalid, identity, valid.expectedDigest!, now), false);
  }
});

test("resume verifies every accepted byte range against the reselected file", async () => {
  const bytes = "first chunksecond chunk";
  const chunkSize = 12;
  const first = new TextEncoder().encode(bytes.slice(0, chunkSize));
  const second = new TextEncoder().encode(bytes.slice(chunkSize));
  const digest = async (value: Uint8Array) => {
    const result = await crypto.subtle.digest("SHA-256", value);
    return `sha256:${Array.from(new Uint8Array(result), (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
  };
  const receivedRanges: ResourceUploadReceivedRange[] = [
    { startOffset: 0, endOffsetInclusive: chunkSize - 1, sha256: await digest(first) },
    { startOffset: chunkSize, endOffsetInclusive: bytes.length - 1, sha256: await digest(second) },
  ];
  const sessionPins = { chunkSizeBytes: chunkSize, receivedRanges };
  assert.equal(await receivedUploadRangesMatchFile(new File([bytes], "notes.txt"), sessionPins), true);
  assert.equal(await receivedUploadRangesMatchFile(new File([`${bytes.slice(0, -1)}!`], "notes.txt"), sessionPins), false);
});

test("resume rejects changed unuploaded bytes even when every accepted prefix still matches", async () => {
  const original = new TextEncoder().encode("accepted-prefix-original-tail");
  const changed = new TextEncoder().encode("accepted-prefix-modified-tail");
  assert.equal(changed.length, original.length);
  const chunkSizeBytes = "accepted-prefix".length;
  const digest = async (value: Uint8Array) => {
    const result = await crypto.subtle.digest("SHA-256", value);
    return `sha256:${Array.from(new Uint8Array(result), (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
  };
  const acceptedPrefix = original.slice(0, chunkSizeBytes);
  const acceptedRanges: ResourceUploadReceivedRange[] = [{
    startOffset: 0,
    endOffsetInclusive: chunkSizeBytes - 1,
    sha256: await digest(acceptedPrefix),
  }];
  const originalDigest = await digest(original);
  const changedDigest = await digest(changed);
  const file = new File([changed], "notes.txt");
  const expectedIdentity = { ...identity, sizeBytes: original.length };
  const sessionIdentity = session({
    expectedSizeBytes: original.length,
    expectedDigest: originalDigest,
    chunkSizeBytes,
  });

  assert.equal(
    await receivedUploadRangesMatchFile(file, { chunkSizeBytes, receivedRanges: acceptedRanges }),
    true,
  );
  assert.equal(
    resumableResourceUploadSession(sessionIdentity, expectedIdentity, changedDigest, Date.now()),
    false,
  );
});

test("resume rejects malformed, misaligned, duplicate, and tampered accepted ranges", async () => {
  const file = new File(["abcdefghij"], "notes.txt");
  const valid: ResourceUploadReceivedRange = {
    startOffset: 0,
    endOffsetInclusive: 3,
    sha256: `sha256:${"0".repeat(64)}`,
  };
  const cases: unknown[] = [
    null,
    "not-a-range",
    [valid, { ...valid, startOffset: 4, endOffsetInclusive: 7 }],
    [{ ...valid, startOffset: 1 }],
    [{ ...valid, endOffsetInclusive: 4 }],
    [{ ...valid, extra: "rejected by the closed upload-range contract" }],
    [{ ...valid, sha256: "not-a-digest" }],
    [{ ...valid, sha256: `sha256:${"f".repeat(64)}` }],
  ];
  for (const receivedRanges of cases) {
    assert.equal(
      await receivedUploadRangesMatchFile(file, {
        chunkSizeBytes: 4,
        receivedRanges: (Array.isArray(receivedRanges) ? receivedRanges : [receivedRanges]) as ResourceUploadReceivedRange[],
      }),
      false,
    );
  }
});

test("COMMITTED recovery persists a newly allocated RequestId once and reuses it on retry", () => {
  const resume: { commitRequestId?: string } = {};
  const persisted: string[] = [];
  let generated = 0;
  const createRequestId = () => {
    generated += 1;
    return `request-${generated}`;
  };
  const persist = () => {
    assert.ok(resume.commitRequestId, "commit RequestId must be saved before IPC starts");
    persisted.push(resume.commitRequestId);
  };

  const first = prepareResourceUploadCommitRequestId(resume, persist, createRequestId);
  // The response can be lost after the daemon commits; a later recovery must reuse the key.
  const retry = prepareResourceUploadCommitRequestId(resume, persist, createRequestId);

  assert.equal(first, "request-1");
  assert.equal(retry, first);
  assert.deepEqual(persisted, ["request-1"]);
  assert.equal(generated, 1);
});

test("commit IPC is prevented when the recovery RequestId cannot be persisted", () => {
  const resume: { commitRequestId?: string } = {};
  assert.throws(
    () => prepareResourceUploadCommitRequestId(resume, () => false, () => "request-not-saved"),
    /Could not persist resource upload recovery metadata/,
  );
  assert.equal(resume.commitRequestId, "request-not-saved");
});
