import assert from "node:assert/strict";
import test from "node:test";
import {
  resourceSearchSourceReferences,
  resolveResourceSourceReferences,
  type ResourceSourceReference,
} from "../src/resources/resource-source-references.ts";

const valid: ResourceSourceReference = {
  id: "source-1",
  workspaceId: "workspace-1",
  resourceId: "resource-1",
  revisionId: "revision-4",
  contentDigest: `sha256:${"a".repeat(64)}`,
  displayName: "Quarterly plan.txt",
  excerpt: "The launch window begins in October.",
  location: "Line 18",
};

test("source references preserve the exact pin and expose a readable location", () => {
  assert.deepEqual(resolveResourceSourceReferences("workspace-1", [valid]), [
    {
      kind: "pinned-reference",
      id: "source-1",
      displayName: "Quarterly plan.txt",
      excerpt: "The launch window begins in October.",
      location: "Line 18",
      pin: {
        workspaceId: "workspace-1",
        resourceId: "resource-1",
        revisionId: "revision-4",
        contentDigest: `sha256:${"a".repeat(64)}`,
        sourceMatches: [],
      },
    },
  ]);
});

test("cross-Workspace and malformed source references cannot be opened as verified pins", () => {
  const result = resolveResourceSourceReferences("workspace-1", [
    valid,
    { ...valid, id: "wrong-workspace", workspaceId: "workspace-2" },
    { ...valid, id: "bad-digest", contentDigest: "sha256:deadbeef" },
    { ...valid, id: "missing-revision", revisionId: "" },
  ]);
  assert.equal(result[0]?.kind, "pinned-reference");
  assert.deepEqual(result.slice(1), [
    { kind: "unavailable", id: "wrong-workspace", displayName: "Quarterly plan.txt" },
    { kind: "unavailable", id: "bad-digest", displayName: "Quarterly plan.txt" },
    { kind: "unavailable", id: "missing-revision", displayName: "Quarterly plan.txt" },
  ]);
});

test("untrusted source labels are bounded and blank labels receive a plain fallback", () => {
  const result = resolveResourceSourceReferences("workspace-1", [
    { ...valid, displayName: " \n " },
    { ...valid, id: "oversized", displayName: "x".repeat(513) },
  ]);
  assert.equal(result[0]?.displayName, "Source file");
  assert.equal(result[1]?.displayName, "Source file");
});

test("indexed search becomes source references only when exact content matches exist", () => {
  const sources = resourceSearchSourceReferences("workspace-1", [
    {
      resourceId: "resource-1",
      resourceRevisionId: "revision-4",
      sourceContentDigest: valid.contentDigest,
      sourceMatches: [{ term: "launch", startUtf8Byte: 0, endUtf8ByteExclusive: 6 }],
      displayName: valid.displayName,
      snippet: "launch window",
    },
    {
      resourceId: "resource-2",
      resourceRevisionId: "revision-9",
      sourceContentDigest: valid.contentDigest,
      sourceMatches: [],
      displayName: "Metadata only.txt",
      snippet: null,
    },
  ]);
  assert.equal(sources.length, 1);
  assert.equal(sources[0]?.workspaceId, "workspace-1");
  assert.equal(sources[0]?.revisionId, "revision-4");
  assert.deepEqual(sources[0]?.sourceMatches, [
    { term: "launch", startUtf8Byte: 0, endUtf8ByteExclusive: 6 },
  ]);
});
