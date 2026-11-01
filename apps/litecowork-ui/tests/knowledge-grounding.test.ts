import assert from "node:assert/strict";
import test from "node:test";
import {
  citationIsPoisoned,
  citationPin,
  decodeGroundedRetrieval,
  groundedAnswerView,
  type GroundedRetrievalResponse,
} from "../src/resources/retrieval-grounding.ts";
import {
  decodeResourceIndexState,
  indexStateIsSearchable,
  indexStateLabel,
} from "../src/resources/knowledge-index-state.ts";

const workspaceId = "workspace-1";
const requestId = "request-1";
const digest = `sha256:${"a".repeat(64)}`;

function citation(overrides: Record<string, unknown> = {}) {
  return {
    citationId: "resource-1:revision-1:0",
    resourceId: "resource-1",
    resourceRevisionId: "revision-1",
    contentDigest: digest,
    locator: { kind: "TEXT_SPAN", span: { startUtf8Byte: 0, endUtf8ByteExclusive: 5 } },
    matchedSpans: [{ startUtf8Byte: 0, endUtf8ByteExclusive: 5 }],
    excerpt: "hello",
    providerId: "litecowork.local-lexical",
    providerVersion: "litecowork.local-lexical/1",
    warnings: [],
    ...overrides,
  };
}

function response(overrides: Record<string, unknown> = {}) {
  return {
    requestId,
    workspaceId,
    status: "GROUNDED",
    provider: {
      providerId: "litecowork.local-lexical",
      providerVersion: "litecowork.local-lexical/1",
      availability: "QUALIFIED",
      retrievalMode: "LEXICAL_ONLY",
      placement: "LOCAL",
      reasonCode: null,
      reason: null,
    },
    citations: [citation()],
    scannedCandidates: 1,
    truncated: false,
    ...overrides,
  };
}

test("a grounded result must carry at least one revision-bound citation", () => {
  const decoded = decodeGroundedRetrieval(response(), { workspaceId, requestId });
  assert.equal(decoded.status, "GROUNDED");
  assert.equal(decoded.citations.length, 1);
  assert.deepEqual(citationPin(decoded.citations[0]), {
    resourceId: "resource-1",
    resourceRevisionId: "revision-1",
    contentDigest: digest,
  });
});

test("a grounded claim without a citation is refused", () => {
  assert.throws(
    () => decodeGroundedRetrieval(response({ citations: [] }), { workspaceId, requestId }),
    /grounded answer without a source reference/,
  );
});

test("insufficient evidence without a citation stays visible", () => {
  const decoded = decodeGroundedRetrieval(
    response({ status: "INSUFFICIENT_EVIDENCE", citations: [], insufficientReason: "NO_AUTHORIZED_MATCH" }),
    { workspaceId, requestId },
  );
  const view = groundedAnswerView(decoded);
  assert.equal(view.kind, "insufficient");
  if (view.kind !== "insufficient") return;
  assert.equal(view.reason, "NO_AUTHORIZED_MATCH");
});

test("insufficient evidence without a typed reason is refused", () => {
  assert.throws(
    () => decodeGroundedRetrieval(
      response({ status: "INSUFFICIENT_EVIDENCE", citations: [] }),
      { workspaceId, requestId },
    ),
    /insufficient evidence without a reason/,
  );
});

test("a citation without a valid content digest is refused", () => {
  assert.throws(
    () => decodeGroundedRetrieval(
      response({ citations: [citation({ contentDigest: "sha256:zz" })] }),
      { workspaceId, requestId },
    ),
    /not bound to an exact revision/,
  );
});

test("a citation with an empty span is refused", () => {
  assert.throws(
    () => decodeGroundedRetrieval(
      response({
        citations: [citation({
          locator: { kind: "TEXT_SPAN", span: { startUtf8Byte: 4, endUtf8ByteExclusive: 4 } },
        })],
      }),
      { workspaceId, requestId },
    ),
    /not bound to an exact revision/,
  );
});

test("a result for another Workspace or request is refused", () => {
  assert.throws(
    () => decodeGroundedRetrieval(response({ workspaceId: "workspace-2" }), { workspaceId, requestId }),
    /another Workspace/,
  );
  assert.throws(
    () => decodeGroundedRetrieval(response({ requestId: "request-2" }), { workspaceId, requestId }),
    /another request/,
  );
});

test("poisoned content is flagged and never silently trusted", () => {
  const decoded = decodeGroundedRetrieval(
    response({ citations: [citation({ warnings: ["EMBEDDED_INSTRUCTION_DETECTED"] })] }),
    { workspaceId, requestId },
  );
  assert.equal(citationIsPoisoned(decoded.citations[0]), true);
});

test("the index state keeps uploaded and searchable apart", () => {
  const ready = decodeResourceIndexState(
    {
      workspaceId,
      resourceId: "resource-1",
      state: "READY",
      currentRevisionId: "revision-1",
      contentDigest: digest,
      indexedRevisionId: "revision-1",
      indexedAt: "2026-10-20T10:00:00Z",
      parserId: "litecowork.plain-text.v1",
      supersededIndexCount: 0,
    },
    { workspaceId, resourceId: "resource-1" },
  );
  assert.equal(indexStateIsSearchable(ready.state), true);
  assert.match(indexStateLabel(ready), /Searchable/);

  const absent = decodeResourceIndexState(
    {
      workspaceId,
      resourceId: "resource-1",
      state: "ABSENT",
      currentRevisionId: "revision-1",
      contentDigest: digest,
      indexedRevisionId: null,
      indexedAt: null,
      parserId: null,
      supersededIndexCount: 0,
    },
    { workspaceId, resourceId: "resource-1" },
  );
  assert.equal(indexStateIsSearchable(absent.state), false);
  assert.match(indexStateLabel(absent), /not searchable/i);

  const stale = decodeResourceIndexState(
    {
      workspaceId,
      resourceId: "resource-1",
      state: "STALE",
      currentRevisionId: "revision-2",
      contentDigest: digest,
      indexedRevisionId: "revision-1",
      indexedAt: "2026-10-20T10:00:00Z",
      parserId: "litecowork.plain-text.v1",
      supersededIndexCount: 1,
    },
    { workspaceId, resourceId: "resource-1" },
  );
  assert.equal(indexStateIsSearchable(stale.state), false);
  assert.match(indexStateLabel(stale), /source changed/i);
});

test("a ready index without a named revision is refused", () => {
  assert.throws(
    () => decodeResourceIndexState(
      {
        workspaceId,
        resourceId: "resource-1",
        state: "READY",
        currentRevisionId: null,
        indexedRevisionId: null,
        supersededIndexCount: 0,
      },
      { workspaceId, resourceId: "resource-1" },
    ),
    /ready index without naming its revision/,
  );
});

test("an index state for another Resource is refused", () => {
  assert.throws(
    () => decodeResourceIndexState(
      {
        workspaceId,
        resourceId: "resource-2",
        state: "ABSENT",
        supersededIndexCount: 0,
      },
      { workspaceId, resourceId: "resource-1" },
    ),
    /another Resource/,
  );
});

test("the grounded view reports an unavailable provider instead of an answer", () => {
  const decoded: GroundedRetrievalResponse = decodeGroundedRetrieval(
    response({
      status: "PROVIDER_UNAVAILABLE",
      citations: [],
      insufficientReason: null,
      provider: {
        providerId: "litecowork.local-lexical",
        providerVersion: "1",
        availability: "INCOMPATIBLE",
        retrievalMode: "LEXICAL_ONLY",
        placement: "LOCAL",
        reasonCode: "PROVIDER_INDEX_GENERATION_INCOMPATIBLE",
        reason: "outdated index",
      },
    }),
    { workspaceId, requestId },
  );
  const view = groundedAnswerView(decoded);
  assert.equal(view.kind, "unavailable");
  if (view.kind !== "unavailable") return;
  assert.equal(view.reasonCode, "PROVIDER_INDEX_GENERATION_INCOMPATIBLE");
});
