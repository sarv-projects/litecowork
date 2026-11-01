import assert from "node:assert/strict";
import { test } from "node:test";
import { isValidatedRichPresentation, resolveRichResponseContent } from "../src/presentation/rich-presentation.ts";

const semanticText = "Complete semantic answer.";
const trusted = Object.assign({
  presentationId: "p1", messageId: "m1", semanticContentDigest: `sha256:${"a".repeat(64)}`,
  rootBlocks: [{ kind: "TEXT_SLICE", text: semanticText }],
}, { [Symbol.for("test-only")]: true });

// The real brand is deliberately required; unvalidated objects must retain semantic fallback.
test("selects a validated rich presentation as the primary view and retains semantic fallback", async () => {
  const selected = resolveRichResponseContent(semanticText, null);
  assert.equal(selected.primary, "SEMANTIC");
  assert.equal(selected.semanticText, semanticText);

  // Construct a real branded presentation through the production parser.
  const bytes = new TextEncoder().encode(semanticText);
  const digestBytes = await globalThis.crypto.subtle.digest("SHA-256", bytes);
  const sliceDigest = `sha256:${Array.from(new Uint8Array(digestBytes), (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
  const document = {
    schema_version: 1, renderer_contract_version: 1, presentation_id: "p1", message_id: "m1",
    semantic_content_digest: `sha256:${"a".repeat(64)}`,
    root_blocks: [{ kind: "TEXT_SLICE", source: { start_utf8_byte: 0, end_utf8_byte_exclusive: bytes.byteLength, slice_digest: sliceDigest } }],
    citations: [], actions: [], block_provenance: [{ block_path: "/0", origin: "SEMANTIC_MESSAGE", resource_refs: [], artifact_refs: [], evidence_refs: [], verification_refs: [] }],
  };
  const { parseRichPresentationDocument } = await import("../src/presentation/rich-presentation.ts");
  const presentation = await parseRichPresentationDocument(document, semanticText, "m1", document.semantic_content_digest);
  assert.ok(presentation && isValidatedRichPresentation(presentation));
  const resolved = resolveRichResponseContent(semanticText, presentation);
  assert.equal(resolved.primary, "RICH");
  assert.equal(resolved.presentation, presentation);
  assert.equal(resolved.semanticText, semanticText);
});

test("does not promote an unvalidated presentation over semantic content", () => {
  const resolved = resolveRichResponseContent(semanticText, trusted);
  assert.equal(resolved.primary, "SEMANTIC");
  assert.equal(resolved.presentation, null);
  assert.equal(resolved.semanticText, semanticText);
});
