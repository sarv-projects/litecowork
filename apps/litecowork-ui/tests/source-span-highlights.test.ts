import assert from "node:assert/strict";
import test from "node:test";
import {
  validateResourceSourceHighlights,
  type ResourceSourceMatch,
  type VerifiedResourcePreview,
} from "../src/resources/source-span-highlights.ts";

async function digest(text: string): Promise<string> {
  const bytes = new TextEncoder().encode(text);
  const hash = await crypto.subtle.digest("SHA-256", bytes);
  return `sha256:${Array.from(new Uint8Array(hash), byte => byte.toString(16).padStart(2, "0")).join("")}`;
}

async function preview(text: string, resourceRevisionId = "revision-3"): Promise<VerifiedResourcePreview> {
  return { text, resourceRevisionId, contentDigest: await digest(text) };
}

test("converts valid ASCII byte spans into safe highlighted text segments", async () => {
  const content = "prefix mutex suffix";
  const result = await validateResourceSourceHighlights({
    preview: await preview(content),
    expectedRevisionId: "revision-3",
    expectedContentDigest: await digest(content),
    matches: [{ term: "mutex", startUtf8Byte: 7, endUtf8ByteExclusive: 12 }],
  });
  assert.equal(result.kind, "highlighted");
  if (result.kind !== "highlighted") return;
  assert.deepEqual(result.segments, [
    { text: "prefix ", terms: [] },
    { text: "mutex", terms: ["mutex"] },
    { text: " suffix", terms: [] },
  ]);
});

test("maps UTF-8 byte offsets after multibyte letters and emoji to UTF-16 offsets", async () => {
  const content = "naïve 🦊 hello";
  const result = await validateResourceSourceHighlights({
    preview: await preview(content),
    expectedRevisionId: "revision-3",
    expectedContentDigest: await digest(content),
    matches: [{ term: "hello", startUtf8Byte: 12, endUtf8ByteExclusive: 17 }],
  });
  assert.equal(result.kind, "highlighted");
  if (result.kind !== "highlighted") return;
  assert.deepEqual(result.segments, [
    { text: "naïve 🦊 ", terms: [] },
    { text: "hello", terms: ["hello"] },
  ]);
});

test("falls back to plain preview for malformed, split-character, duplicate, overlapping, and out-of-bounds spans", async () => {
  const content = "naïve 🦊 mutex mutex";
  const pinnedPreview = await preview(content);
  const expectedContentDigest = await digest(content);
  const cases: ResourceSourceMatch[][] = [
    [{ term: "", startUtf8Byte: 0, endUtf8ByteExclusive: 1 }],
    [{ term: "Mutex", startUtf8Byte: 12, endUtf8ByteExclusive: 17 }],
    [{ term: "mutex", startUtf8Byte: 3, endUtf8ByteExclusive: 8 }],
    [
      { term: "mutex", startUtf8Byte: 12, endUtf8ByteExclusive: 17 },
      { term: "mutex", startUtf8Byte: 18, endUtf8ByteExclusive: 23 },
    ],
    [
      { term: "mutex", startUtf8Byte: 12, endUtf8ByteExclusive: 17 },
      { term: "mutex", startUtf8Byte: 16, endUtf8ByteExclusive: 21 },
    ],
    [
      { term: "alpha", startUtf8Byte: 0, endUtf8ByteExclusive: 5 },
      { term: "pha", startUtf8Byte: 2, endUtf8ByteExclusive: 5 },
    ],
    [{ term: "mutex", startUtf8Byte: -1, endUtf8ByteExclusive: 5 }],
    [{ term: "mutex", startUtf8Byte: 12.5, endUtf8ByteExclusive: 17 }],
    [{ term: "mutex", startUtf8Byte: 12, endUtf8ByteExclusive: 10_000 }],
  ];
  for (const matches of cases) {
    const result = await validateResourceSourceHighlights({
      preview: pinnedPreview,
      expectedRevisionId: "revision-3",
      expectedContentDigest,
      matches,
    });
    assert.equal(result.kind, "plain");
  }
  const distinctOverlappingTerms = await validateResourceSourceHighlights({
    preview: await preview("alphabet"),
    expectedRevisionId: "revision-3",
    expectedContentDigest: await digest("alphabet"),
    matches: [
      { term: "alpha", startUtf8Byte: 0, endUtf8ByteExclusive: 5 },
      { term: "pha", startUtf8Byte: 2, endUtf8ByteExclusive: 5 },
    ],
  });
  assert.equal(distinctOverlappingTerms.kind, "plain");
});

test("falls back to plain preview on digest or revision mismatch", async () => {
  const content = "mutex";
  const pinnedPreview = await preview(content);
  const matches = [{ term: "mutex", startUtf8Byte: 0, endUtf8ByteExclusive: 5 }];
  const wrongDigest = await validateResourceSourceHighlights({
    preview: { ...pinnedPreview, contentDigest: `sha256:${"0".repeat(64)}` },
    expectedRevisionId: "revision-3",
    expectedContentDigest: await digest(content),
    matches,
  });
  const wrongRevision = await validateResourceSourceHighlights({
    preview: pinnedPreview,
    expectedRevisionId: "revision-4",
    expectedContentDigest: await digest(content),
    matches,
  });
  const alteredText = await validateResourceSourceHighlights({
    preview: { ...pinnedPreview, text: "other" },
    expectedRevisionId: "revision-3",
    expectedContentDigest: await digest(content),
    matches,
  });
  assert.equal(wrongDigest.kind, "plain");
  assert.equal(wrongRevision.kind, "plain");
  assert.equal(alteredText.kind, "plain");
});
