/**
 * Decoding and rendering rules for grounded retrieval results.
 *
 * Every answer that LiteCowork shows from retrieved content must name the exact
 * immutable Resource revision it came from. This module refuses a response that
 * claims grounding without a revision-bound citation, and it keeps the explicit
 * "no adequate source" outcome visible instead of degrading it into an unreferenced
 * answer.
 */

export type GroundedRetrievalStatus =
  | "GROUNDED"
  | "INSUFFICIENT_EVIDENCE"
  | "PROVIDER_UNAVAILABLE";

export type InsufficientEvidenceReason =
  | "NO_AUTHORIZED_MATCH"
  | "PINNED_REVISION_NOT_INDEXED"
  | "EMPTY_SCOPE"
  | "ALL_CANDIDATES_REVOKED";

export type RetrievalWarning =
  | "EMBEDDED_INSTRUCTION_DETECTED"
  | "EXCERPT_TRUNCATED"
  | "INCOMPATIBLE_INDEX_GENERATION";

export type RetrievalTextSpan = {
  startUtf8Byte: number;
  endUtf8ByteExclusive: number;
};

export type RetrievalSegmentLocator =
  | { kind: "TEXT_SPAN"; span: RetrievalTextSpan }
  | { kind: "PAGE"; pageNumber: number };

export type RetrievalCitation = {
  citationId: string;
  resourceId: string;
  resourceRevisionId: string;
  contentDigest: string;
  locator: RetrievalSegmentLocator;
  matchedSpans: readonly RetrievalTextSpan[];
  excerpt: string;
  providerId: string;
  providerVersion: string;
  warnings: readonly RetrievalWarning[];
};

export type RetrievalProviderStatus = {
  providerId: string;
  providerVersion: string;
  availability: string;
  retrievalMode: string;
  placement: string;
  reasonCode: string | null;
  reason: string | null;
};

export type GroundedRetrievalResponse = {
  requestId: string;
  workspaceId: string;
  status: GroundedRetrievalStatus;
  provider: RetrievalProviderStatus;
  citations: readonly RetrievalCitation[];
  insufficientReason: InsufficientEvidenceReason | null;
  scannedCandidates: number;
  truncated: boolean;
};

export type GroundedAnswerView =
  | {
    kind: "grounded";
    citations: readonly RetrievalCitation[];
    providerLabel: string;
    truncated: boolean;
  }
  | {
    kind: "insufficient";
    reason: InsufficientEvidenceReason;
    providerLabel: string;
  }
  | {
    kind: "unavailable";
    reasonCode: string | null;
    providerLabel: string;
  };

const SHA256 = /^sha256:[a-f0-9]{64}$/;
const MAX_EXCERPT = 4096;
const MAX_IDENTIFIER = 256;

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function validIdentifier(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0 && value.length <= MAX_IDENTIFIER;
}

function decodeSpan(value: unknown): RetrievalTextSpan | null {
  if (!isRecord(value)) return null;
  const start = value.startUtf8Byte;
  const end = value.endUtf8ByteExclusive;
  if (typeof start !== "number" || typeof end !== "number") return null;
  if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end)) return null;
  if (start < 0 || end <= start) return null;
  return { startUtf8Byte: start, endUtf8ByteExclusive: end };
}

function decodeLocator(value: unknown): RetrievalSegmentLocator | null {
  if (!isRecord(value)) return null;
  if (value.kind === "TEXT_SPAN") {
    const span = decodeSpan(value.span);
    return span ? { kind: "TEXT_SPAN", span } : null;
  }
  if (value.kind === "PAGE") {
    const page = value.pageNumber;
    if (typeof page !== "number" || !Number.isSafeInteger(page) || page < 1) return null;
    return { kind: "PAGE", pageNumber: page };
  }
  return null;
}

function decodeWarnings(value: unknown): RetrievalWarning[] {
  if (!Array.isArray(value)) return [];
  const allowed: RetrievalWarning[] = [
    "EMBEDDED_INSTRUCTION_DETECTED",
    "EXCERPT_TRUNCATED",
    "INCOMPATIBLE_INDEX_GENERATION",
  ];
  return value.filter((entry): entry is RetrievalWarning =>
    typeof entry === "string" && (allowed as string[]).includes(entry));
}

function decodeCitation(value: unknown): RetrievalCitation | null {
  if (!isRecord(value)) return null;
  if (!validIdentifier(value.citationId)) return null;
  if (!validIdentifier(value.resourceId)) return null;
  if (!validIdentifier(value.resourceRevisionId)) return null;
  if (typeof value.contentDigest !== "string" || !SHA256.test(value.contentDigest)) return null;
  if (typeof value.excerpt !== "string" || value.excerpt.length === 0) return null;
  if (value.excerpt.length > MAX_EXCERPT) return null;
  if (!validIdentifier(value.providerId)) return null;
  const locator = decodeLocator(value.locator);
  if (!locator) return null;
  const matchedSpans = Array.isArray(value.matchedSpans)
    ? value.matchedSpans.map(decodeSpan).filter((span): span is RetrievalTextSpan => span !== null)
    : [];
  return {
    citationId: value.citationId,
    resourceId: value.resourceId,
    resourceRevisionId: value.resourceRevisionId,
    contentDigest: value.contentDigest,
    locator,
    matchedSpans,
    excerpt: value.excerpt,
    providerId: value.providerId,
    providerVersion: typeof value.providerVersion === "string" ? value.providerVersion : "",
    warnings: decodeWarnings(value.warnings),
  };
}

function decodeProvider(value: unknown): RetrievalProviderStatus | null {
  if (!isRecord(value)) return null;
  if (!validIdentifier(value.providerId)) return null;
  return {
    providerId: value.providerId,
    providerVersion: typeof value.providerVersion === "string" ? value.providerVersion : "",
    availability: typeof value.availability === "string" ? value.availability : "UNKNOWN",
    retrievalMode: typeof value.retrievalMode === "string" ? value.retrievalMode : "UNKNOWN",
    placement: typeof value.placement === "string" ? value.placement : "UNKNOWN",
    reasonCode: typeof value.reasonCode === "string" ? value.reasonCode : null,
    reason: typeof value.reason === "string" ? value.reason : null,
  };
}

/**
 * Decodes a grounded retrieval response. The response is refused when it claims
 * grounding without a revision-bound citation, or when it reports insufficient
 * evidence while still carrying a citation.
 */
export function decodeGroundedRetrieval(
  value: unknown,
  expected: { workspaceId: string; requestId?: string },
): GroundedRetrievalResponse {
  if (!isRecord(value)) {
    throw new Error("The local Runtime returned an invalid grounded retrieval result.");
  }
  if (value.workspaceId !== expected.workspaceId) {
    throw new Error("The local Runtime returned a grounded retrieval result for another Workspace.");
  }
  if (!validIdentifier(value.requestId)) {
    throw new Error("The local Runtime returned a grounded retrieval result without a request id.");
  }
  if (expected.requestId !== undefined && value.requestId !== expected.requestId) {
    throw new Error("The local Runtime returned a grounded retrieval result for another request.");
  }
  const status = value.status;
  if (status !== "GROUNDED" && status !== "INSUFFICIENT_EVIDENCE" && status !== "PROVIDER_UNAVAILABLE") {
    throw new Error("The local Runtime returned an unsupported grounded retrieval status.");
  }
  const provider = decodeProvider(value.provider);
  if (!provider) {
    throw new Error("The local Runtime returned a grounded retrieval result without a provider.");
  }
  if (!Array.isArray(value.citations)) {
    throw new Error("The local Runtime returned a grounded retrieval result without citations.");
  }
  const citations = value.citations.map(decodeCitation);
  if (citations.some((citation) => citation === null)) {
    throw new Error("The local Runtime returned a citation that is not bound to an exact revision.");
  }
  const decoded = citations as RetrievalCitation[];
  if (status === "GROUNDED" && decoded.length === 0) {
    throw new Error("The local Runtime claimed a grounded answer without a source reference.");
  }
  if (status !== "GROUNDED" && decoded.length > 0) {
    throw new Error("The local Runtime returned a citation without a grounded answer.");
  }
  const insufficientReason = typeof value.insufficientReason === "string"
    ? (value.insufficientReason as InsufficientEvidenceReason)
    : null;
  if (status === "INSUFFICIENT_EVIDENCE" && insufficientReason === null) {
    throw new Error("The local Runtime reported insufficient evidence without a reason.");
  }
  return {
    requestId: value.requestId,
    workspaceId: value.workspaceId,
    status,
    provider,
    citations: decoded,
    insufficientReason,
    scannedCandidates: typeof value.scannedCandidates === "number" ? value.scannedCandidates : 0,
    truncated: value.truncated === true,
  };
}

/** The exact revision pin a citation viewer opens. */
export function citationPin(citation: RetrievalCitation): {
  resourceId: string;
  resourceRevisionId: string;
  contentDigest: string;
} {
  return {
    resourceId: citation.resourceId,
    resourceRevisionId: citation.resourceRevisionId,
    contentDigest: citation.contentDigest,
  };
}

/** True when the citation carries content that tried to address the model. */
export function citationIsPoisoned(citation: RetrievalCitation): boolean {
  return citation.warnings.includes("EMBEDDED_INSTRUCTION_DETECTED");
}

/** Builds the answer surface state from a decoded retrieval result. */
export function groundedAnswerView(response: GroundedRetrievalResponse): GroundedAnswerView {
  const providerLabel = `${response.provider.providerId} (${response.provider.retrievalMode})`;
  if (response.status === "GROUNDED") {
    return {
      kind: "grounded",
      citations: response.citations,
      providerLabel,
      truncated: response.truncated,
    };
  }
  if (response.status === "INSUFFICIENT_EVIDENCE") {
    return {
      kind: "insufficient",
      reason: response.insufficientReason ?? "NO_AUTHORIZED_MATCH",
      providerLabel,
    };
  }
  return {
    kind: "unavailable",
    reasonCode: response.provider.reasonCode,
    providerLabel,
  };
}
