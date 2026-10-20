import type { ResourceSourceMatch } from "./source-span-highlights";

export type ResourceSourceReference = {
  id: string;
  workspaceId: string;
  resourceId: string;
  revisionId: string;
  contentDigest: string;
  displayName: string;
  excerpt: string;
  location?: string | null;
  sourceMatches?: readonly ResourceSourceMatch[];
};

export type ExactResourceSourcePin = {
  workspaceId: string;
  resourceId: string;
  revisionId: string;
  contentDigest: string;
  sourceMatches: readonly ResourceSourceMatch[];
};

export type ResolvedResourceSourceReference =
  | {
    kind: "pinned-reference";
    id: string;
    displayName: string;
    excerpt: string;
    location: string | null;
    pin: ExactResourceSourcePin;
  }
  | { kind: "unavailable"; id: string; displayName: string };

const SHA256 = /^sha256:[a-f0-9]{64}$/;
const MAX_LABEL = 256;
const MAX_EXCERPT = 800;

export type IndexedResourceSearchSource = {
  resourceId: string;
  resourceRevisionId: string;
  sourceContentDigest: string;
  sourceMatches: readonly ResourceSourceMatch[];
  displayName: string;
  snippet: string | null;
};

export function resourceSearchSourceReferences(
  workspaceId: string,
  results: readonly IndexedResourceSearchSource[],
): ResourceSourceReference[] {
  return results
    .filter((result) => Array.isArray(result.sourceMatches) && result.sourceMatches.length > 0)
    .map((result) => ({
      id: `${result.resourceId}@${result.resourceRevisionId}`,
      workspaceId,
      resourceId: result.resourceId,
      revisionId: result.resourceRevisionId,
      contentDigest: result.sourceContentDigest,
      displayName: result.displayName,
      excerpt: result.snippet ?? "",
      location: "Text match in saved revision",
      sourceMatches: result.sourceMatches,
    }));
}

function hasSafeId(value: unknown): value is string {
  return typeof value === "string"
    && value.length > 0
    && value.length <= 128
    && /^[\x21-\x7e]+$/.test(value);
}

function hasSafeIdentity(value: unknown): value is string {
  return typeof value === "string"
    && value.length > 0
    && value.length <= 256
    && !Array.from(value).some((character) => /\p{C}/u.test(character));
}

function readableLabel(value: unknown, maxLength = MAX_LABEL): string {
  if (typeof value !== "string") return "Source file";
  const normalized = value.replace(/[\p{C}\s]+/gu, " ").trim();
  if (!normalized || normalized.length > maxLength) return "Source file";
  return normalized;
}

function readableExcerpt(value: unknown): string {
  if (typeof value !== "string") return "";
  return value.replace(/[\p{C}\s]+/gu, " ").trim().slice(0, MAX_EXCERPT);
}

export function resolveResourceSourceReferences(
  selectedWorkspaceId: string,
  references: readonly ResourceSourceReference[],
): ResolvedResourceSourceReference[] {
  const seenIds = new Set<string>();
  let fallbackId = 0;
  return references.map((reference) => {
    let id = hasSafeId(reference?.id) && !seenIds.has(reference.id) ? reference.id : "";
    while (!id || seenIds.has(id)) {
      id = `unavailable-${fallbackId++}`;
    }
    seenIds.add(id);
    const displayName = readableLabel(reference?.displayName);
    const exactPinIsValid = hasSafeIdentity(selectedWorkspaceId)
      && reference?.workspaceId === selectedWorkspaceId
      && hasSafeIdentity(reference.resourceId)
      && hasSafeIdentity(reference.revisionId)
      && typeof reference.contentDigest === "string"
      && SHA256.test(reference.contentDigest);
    if (!exactPinIsValid) return { kind: "unavailable", id, displayName };
    const excerpt = readableExcerpt(reference.excerpt);
    const location = reference.location == null
      ? null
      : readableLabel(reference.location, 128) === "Source file"
        ? null
        : readableLabel(reference.location, 128);
    return {
      kind: "pinned-reference",
      id,
      displayName,
      excerpt,
      location,
      pin: {
        workspaceId: reference.workspaceId,
        resourceId: reference.resourceId,
        revisionId: reference.revisionId,
        contentDigest: reference.contentDigest,
        sourceMatches: reference.sourceMatches ?? [],
      },
    };
  });
}
