export type ResourceSourceMatch = {
  term: string;
  startUtf8Byte: number;
  endUtf8ByteExclusive: number;
};

export type VerifiedResourcePreview = {
  text: string;
  resourceRevisionId: string;
  contentDigest: string;
};

export type HighlightSegment = {
  text: string;
  terms: string[];
};

export type SourceHighlightResult =
  | { kind: "highlighted"; segments: HighlightSegment[] }
  | { kind: "plain"; reason: string };

const SHA256 = /^sha256:[a-f0-9]{64}$/;
const MAX_SOURCE_BYTES = 1_048_576;

function isNonNegativeSafeInteger(value: number): boolean {
  return Number.isSafeInteger(value) && value >= 0;
}

async function digestUtf8(text: string): Promise<string | null> {
  if (!globalThis.crypto?.subtle) return null;
  try {
    const digest = await globalThis.crypto.subtle.digest("SHA-256", new TextEncoder().encode(text));
    return `sha256:${Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
  } catch {
    return null;
  }
}

/**
 * Validate search spans against the exact pinned preview, then convert their UTF-8
 * byte offsets to UTF-16 offsets used by JavaScript strings. Any uncertainty falls
 * back to the ordinary unmarked preview.
 */
export async function validateResourceSourceHighlights(input: {
  preview: VerifiedResourcePreview;
  expectedRevisionId: string;
  expectedContentDigest: string;
  matches: ResourceSourceMatch[];
}): Promise<SourceHighlightResult> {
  const { preview, expectedRevisionId, expectedContentDigest, matches } = input;
  if (!expectedRevisionId || preview.resourceRevisionId !== expectedRevisionId) {
    return { kind: "plain", reason: "The preview belongs to a different Resource revision." };
  }
  if (!SHA256.test(expectedContentDigest) || preview.contentDigest !== expectedContentDigest) {
    return { kind: "plain", reason: "The preview digest does not match the indexed search result." };
  }
  const actualDigest = await digestUtf8(preview.text);
  if (actualDigest !== expectedContentDigest) {
    return { kind: "plain", reason: "The preview bytes do not match the indexed search result." };
  }

  const encoder = new TextEncoder();
  const encoded = encoder.encode(preview.text);
  if (encoded.byteLength > MAX_SOURCE_BYTES || matches.length === 0 || matches.length > 32) {
    return { kind: "plain", reason: "The search spans are outside the supported preview bounds." };
  }

  // Map only valid UTF-8 code-point boundaries to JS UTF-16 code-unit offsets.
  const byteToTextOffset = new Map<number, number>([[0, 0]]);
  let byteOffset = 0;
  let textOffset = 0;
  for (const character of preview.text) {
    const codePoint = character.codePointAt(0)!;
    byteOffset += codePoint <= 0x7f ? 1 : codePoint <= 0x7ff ? 2 : codePoint <= 0xffff ? 3 : 4;
    textOffset += character.length;
    byteToTextOffset.set(byteOffset, textOffset);
  }

  const ordered = [...matches].sort((left, right) => left.startUtf8Byte - right.startUtf8Byte);
  const seenTerms = new Set<string>();
  let previousEnd = 0;
  const ranges: Array<{ start: number; end: number; term: string }> = [];
  for (const match of ordered) {
    if (typeof match.term !== "string" || match.term.length === 0 || Array.from(match.term).length > 128
      || !/^[\p{L}\p{N}]+$/u.test(match.term)
      || match.term.toLowerCase() !== match.term
      || seenTerms.has(match.term)
      || !isNonNegativeSafeInteger(match.startUtf8Byte)
      || !isNonNegativeSafeInteger(match.endUtf8ByteExclusive)
      || match.startUtf8Byte >= match.endUtf8ByteExclusive
      || match.endUtf8ByteExclusive > encoded.byteLength
      || match.startUtf8Byte < previousEnd) {
      return { kind: "plain", reason: "The search spans are invalid for this preview." };
    }
    const start = byteToTextOffset.get(match.startUtf8Byte);
    const end = byteToTextOffset.get(match.endUtf8ByteExclusive);
    if (start === undefined || end === undefined) {
      return { kind: "plain", reason: "A search span splits a UTF-8 character." };
    }
    const observed = preview.text.slice(start, end);
    if (observed.toLowerCase() !== match.term) {
      return { kind: "plain", reason: "A search span does not identify its indexed term." };
    }
    seenTerms.add(match.term);
    ranges.push({ start, end, term: match.term });
    previousEnd = match.endUtf8ByteExclusive;
  }

  const segments: HighlightSegment[] = [];
  let cursor = 0;
  for (const range of ranges) {
    if (range.start > cursor) segments.push({ text: preview.text.slice(cursor, range.start), terms: [] });
    segments.push({ text: preview.text.slice(range.start, range.end), terms: [range.term] });
    cursor = range.end;
  }
  if (cursor < preview.text.length) segments.push({ text: preview.text.slice(cursor), terms: [] });
  return { kind: "highlighted", segments };
}
