/**
 * Owner-visible index state for a managed Resource.
 *
 * "Uploaded" is not "searchable". This module keeps the four Core-owned index states
 * distinguishable and refuses a response that claims a ready index without naming the
 * exact revision it was built from.
 */

export type ResourceIndexState = "READY" | "STALE" | "ABSENT" | "NOT_INDEXABLE";

export type ResourceIndexSkipReason =
  | "UNSUPPORTED_TYPE"
  | "OVER_SIZE_LIMIT"
  | "INVALID_UTF8"
  | "CONTROL_CHARACTERS"
  | "TERM_LIMIT_EXCEEDED";

export type ResourceIndexStateResponse = {
  workspaceId: string;
  resourceId: string;
  state: ResourceIndexState;
  reason: ResourceIndexSkipReason | null;
  currentRevisionId: string | null;
  contentDigest: string | null;
  indexedRevisionId: string | null;
  indexedAt: string | null;
  parserId: string | null;
  supersededIndexCount: number;
};

const SHA256 = /^sha256:[a-f0-9]{64}$/;
const MAX_IDENTIFIER = 256;

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function optionalIdentifier(value: unknown): string | null {
  return typeof value === "string" && value.trim().length > 0 && value.length <= MAX_IDENTIFIER
    ? value
    : null;
}

const SKIP_REASONS: ResourceIndexSkipReason[] = [
  "UNSUPPORTED_TYPE",
  "OVER_SIZE_LIMIT",
  "INVALID_UTF8",
  "CONTROL_CHARACTERS",
  "TERM_LIMIT_EXCEEDED",
];

export function decodeResourceIndexState(
  value: unknown,
  expected: { workspaceId: string; resourceId: string },
): ResourceIndexStateResponse {
  if (!isRecord(value)) {
    throw new Error("The local Runtime returned an invalid Resource index state.");
  }
  if (value.workspaceId !== expected.workspaceId || value.resourceId !== expected.resourceId) {
    throw new Error("The local Runtime returned a Resource index state for another Resource.");
  }
  const state = value.state;
  if (state !== "READY" && state !== "STALE" && state !== "ABSENT" && state !== "NOT_INDEXABLE") {
    throw new Error("The local Runtime returned an unsupported Resource index state.");
  }
  const reason = typeof value.reason === "string"
    ? (SKIP_REASONS as string[]).includes(value.reason)
      ? (value.reason as ResourceIndexSkipReason)
      : null
    : null;
  const currentRevisionId = optionalIdentifier(value.currentRevisionId);
  const indexedRevisionId = optionalIdentifier(value.indexedRevisionId);
  if (state === "READY" && (currentRevisionId === null || indexedRevisionId === null)) {
    throw new Error("The local Runtime claimed a ready index without naming its revision.");
  }
  if (state === "READY" && currentRevisionId !== indexedRevisionId) {
    throw new Error("The local Runtime claimed a ready index for a different revision.");
  }
  const contentDigest = typeof value.contentDigest === "string" && SHA256.test(value.contentDigest)
    ? value.contentDigest
    : null;
  const supersededIndexCount = typeof value.supersededIndexCount === "number"
    && Number.isSafeInteger(value.supersededIndexCount)
    && value.supersededIndexCount >= 0
    ? value.supersededIndexCount
    : 0;
  return {
    workspaceId: value.workspaceId,
    resourceId: value.resourceId,
    state,
    reason,
    currentRevisionId,
    contentDigest,
    indexedRevisionId,
    indexedAt: optionalIdentifier(value.indexedAt),
    parserId: optionalIdentifier(value.parserId),
    supersededIndexCount,
  };
}

/** True only when the current revision is searchable through the local index. */
export function indexStateIsSearchable(state: ResourceIndexState): boolean {
  return state === "READY";
}

/** The owner-visible label of an index state. */
export function indexStateLabel(state: ResourceIndexStateResponse): string {
  switch (state.state) {
    case "READY":
      return "Searchable from the current revision.";
    case "STALE":
      return "The source changed. Rebuild the index before searching it.";
    case "ABSENT":
      return "Uploaded but not searchable yet. Build the index first.";
    case "NOT_INDEXABLE":
      return state.reason === "UNSUPPORTED_TYPE"
        ? "This file type is not indexable by the local provider."
        : `This file is not indexable (${state.reason ?? "UNKNOWN"}).`;
    default:
      return "The index state is unknown.";
  }
}
