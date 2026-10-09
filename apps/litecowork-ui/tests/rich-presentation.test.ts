import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import * as richPresentation from "../src/presentation/rich-presentation.ts";
const { isValidatedRichPresentation, parseRichPresentationDocument, resolveRichResponseContent } = richPresentation;
const parseResponseCandidate = (richPresentation as unknown as Record<string, unknown>).parseRichPresentationResponse;

const semantic = "Answer: café.\nMore detail.";
const messageId = "message-1";
const semanticDigest = `sha256:${"a".repeat(64)}`;

async function digest(value: Uint8Array): Promise<string> {
  const result = await globalThis.crypto.subtle.digest("SHA-256", value);
  return `sha256:${Array.from(new Uint8Array(result), (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}

async function documentFor(blocks: unknown[], text = semantic) {
  const provenance: Array<Record<string, unknown>> = [];
  const collect = (block: unknown, path: string) => {
    if (block === null || typeof block !== "object" || Array.isArray(block)) return;
    provenance.push({ block_path: path, origin: "MODEL_INTENT", resource_refs: [], artifact_refs: [], evidence_refs: [], verification_refs: [] });
    const children = (block as { children?: unknown[] }).children;
    children?.forEach((child, index) => collect(child, `${path}/children/${index}`));
  };
  blocks.forEach((block, index) => collect(block, `/${index}`));
  return {
    schema_version: 1,
    renderer_contract_version: 1,
    presentation_id: "presentation-1",
    message_id: messageId,
    semantic_content_digest: semanticDigest,
    root_blocks: blocks,
    citations: [],
    actions: [],
    block_provenance: provenance,
  };
}

async function textSlice(text = "Answer: café.") {
  const all = new TextEncoder().encode(semantic);
  const start = 0;
  const end = new TextEncoder().encode(text).byteLength;
  return { kind: "TEXT_SLICE", source: { start_utf8_byte: start, end_utf8_byte_exclusive: end, slice_digest: await digest(all.subarray(start, end)) }, width: "READABLE" };
}

async function parse(value: unknown, text = semantic, id = messageId, semanticHash = semanticDigest) {
  return parseRichPresentationDocument(value, text, id, semanticHash);
}

function canonicalJson(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  if (value !== null && typeof value === "object") {
    const record = value as Record<string, unknown>;
    return `{${Object.keys(record).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(record[key])}`).join(",")}}`;
  }
  return JSON.stringify(value);
}

async function responseFor(document: unknown, overrides: Record<string, unknown> = {}) {
  const bytes = new TextEncoder().encode(canonicalJson(document));
  return {
    workspace_id: "workspace-1",
    conversation_id: "conversation-1",
    message_id: messageId,
    presentation_id: "presentation-1",
    schema_version: 1,
    renderer_contract_version: 1,
    semantic_content_digest: semanticDigest,
    document_digest: await digest(bytes),
    document_size_bytes: bytes.byteLength,
    document,
    ...overrides,
  };
}

test("exports a response-envelope parser for Workspace/Conversation and document digest binding", async () => {
  assert.equal(typeof parseResponseCandidate, "function");
});

test("response parser rejects Workspace, Conversation, document digest, and byte-size mismatches", async () => {
  const document = await documentFor([await textSlice()]);
  const parseResponse = parseResponseCandidate;
  assert.equal(typeof parseResponse, "function");
  if (typeof parseResponse !== "function") return;
  const response = await responseFor(document);
  const binding = { workspaceId: "workspace-1", conversationId: "conversation-1", messageId, semanticContentDigest: semanticDigest };
  assert.ok(await parseResponse(response, binding, semantic));
  assert.equal(await parseResponse({ ...response, workspace_id: "workspace-2" }, binding, semantic), null);
  assert.equal(await parseResponse({ ...response, conversation_id: "conversation-2" }, binding, semantic), null);
  assert.equal(await parseResponse({ ...response, document_digest: `sha256:${"0".repeat(64)}` }, binding, semantic), null);
  assert.equal(await parseResponse({ ...response, document_size_bytes: response.document_size_bytes + 1 }, binding, semantic), null);
});

test("accepts a bounded semantic slice and binds it to the expected Message", async () => {
  const document = await documentFor([await textSlice()]);
  const parsed = await parse(document);
  assert.ok(parsed);
  assert.equal(parsed.messageId, messageId);
  assert.equal(parsed.rootBlocks[0]?.kind, "TEXT_SLICE");
  assert.ok(isValidatedRichPresentation(parsed));
  assert.equal(isValidatedRichPresentation({ presentationId: "forged", rootBlocks: [{ kind: "HOST_PROJECTION" }] }), false);
});

test("rejects wrong Message or semantic content digest", async () => {
  const document = await documentFor([await textSlice()]);
  assert.equal(await parse(document, semantic, "other-message"), null);
  assert.equal(await parse(document, semantic, messageId, `sha256:${"b".repeat(64)}`), null);
});

test("rejects stale or tampered semantic slices", async () => {
  const document = await documentFor([await textSlice()]);
  const changed = structuredClone(document) as { root_blocks: Array<{ source: { slice_digest: string } }> };
  changed.root_blocks[0]!.source.slice_digest = `sha256:${"0".repeat(64)}`;
  assert.equal(await parse(changed), null);
});

test("rejects a UTF-8 byte range that splits a multibyte character", async () => {
  const bytes = new TextEncoder().encode(semantic);
  const accentEnd = new TextEncoder().encode("Answer: ca").byteLength + 2;
  const block = { kind: "TEXT_SLICE", source: { start_utf8_byte: 0, end_utf8_byte_exclusive: accentEnd, slice_digest: await digest(bytes.subarray(0, accentEnd)) } };
  assert.equal(await parse(await documentFor([block])), null);
});

test("rejects out-of-range and emoji-splitting UTF-8 slices", async () => {
  const text = "A 🙂 B";
  const bytes = new TextEncoder().encode(text);
  const emojiStart = new TextEncoder().encode("A ").byteLength;
  const splitEmoji = {
    kind: "TEXT_SLICE",
    source: {
      start_utf8_byte: emojiStart,
      end_utf8_byte_exclusive: emojiStart + 2,
      slice_digest: await digest(bytes.subarray(emojiStart, emojiStart + 2)),
    },
  };
  const outOfRange = {
    kind: "TEXT_SLICE",
    source: { start_utf8_byte: 0, end_utf8_byte_exclusive: bytes.byteLength + 1, slice_digest: await digest(bytes) },
  };
  assert.equal(await parse(await documentFor([splitEmoji], text), text), null);
  assert.equal(await parse(await documentFor([outOfRange], text), text), null);
});

test("rejects a semantic string containing an unpaired surrogate", async () => {
  assert.equal(await parse(await documentFor([]), "broken \ud800 text"), null);
});

test("accepts a table with finite scalar cells and rejects non-finite values", async () => {
  const semanticTable = "Two scores.\n\n| Name | Score |\n| --- | --- |\n| A | 1 |\n| B | — |";
  const valid = await documentFor([{ kind: "TABLE", headers: ["Name", "Score"], rows: [["A", 1], ["B", null]], accessible_summary: "Two scores." }], semanticTable);
  assert.ok(await parse(valid, semanticTable));
  const invalid = await documentFor([{ kind: "TABLE", headers: ["Score"], rows: [[Number.NaN]] }]);
  assert.equal(await parse(invalid), null);
});

test("rejects rich-only table facts and preserves the complete semantic fallback", async () => {
  const semanticText = "The notes compare the plans but do not provide prices.";
  const document = await documentFor([{ kind: "TABLE", headers: ["Plan", "Price"], rows: [["Basic", "$9"], ["Pro", "$29"]] }], semanticText);
  const parsed = await parse(document, semanticText);
  const resolved = resolveRichResponseContent(semanticText, parsed);
  assert.equal(parsed, null);
  assert.equal(resolved.presentation, null);
  assert.equal(resolved.semanticText, semanticText);
});

test("rejects rich-only card titles and accessibility summaries", async () => {
  const semanticText = "The notes contain a summary.";
  const card = await documentFor([{ kind: "CARD", title: "Guaranteed savings", children: [] }], semanticText);
  const summary = await documentFor([], semanticText);
  summary.accessibility_summary = "The report proves guaranteed savings.";
  assert.equal(await parse(card, semanticText), null);
  assert.equal(await parse(summary, semanticText), null);
});

test("metadata must match a complete semantic line, not a context fragment", async () => {
  const semanticText = "The downloaded archive is not safe to open.";
  const card = await documentFor([{ kind: "CARD", title: "safe", children: [] }], semanticText);
  const accessibleSummary = await documentFor([], semanticText);
  accessibleSummary.accessibility_summary = "safe";
  assert.equal(await parse(card, semanticText), null);
  assert.equal(await parse(accessibleSummary, semanticText), null);

  const headingText = "## **Summary**";
  const heading = await documentFor([{ kind: "CARD", title: "Summary", children: [] }], headingText);
  assert.ok(await parse(heading, headingText));
});

test("rejects host projections, executable/remote media, and model action references", async () => {
  const hostBlock = await documentFor([{ kind: "HOST_PROJECTION", projection_ref: { projection_kind: "APPROVAL", source_id: "approval-1" } }]);
  const mediaBlock = await documentFor([{ kind: "MEDIA", resource_ref: { resource_id: "r", revision_id: "v" }, media_kind: "IMAGE", alt_text: "image" }]);
  const actionDoc = await documentFor([{ kind: "TABLE", headers: ["x"], rows: [] }]);
  (actionDoc.actions as unknown[]).push({ kind: "EXTERNAL_HTTPS", url: "https://example.com" });
  assert.equal(await parse(hostBlock), null);
  assert.equal(await parse(mediaBlock), null);
  assert.equal(await parse(actionDoc), null);
});

test("rejects every host-bound system-state block family", async () => {
  const trustedKinds = [
    "TASK_PROJECTION", "ATTEMPT_PROJECTION", "USER_REQUEST", "APPROVAL", "ARTIFACT_VIEWER",
    "VERIFICATION", "EFFECT", "RUNTIME_STATUS", "COST", "QUOTA", "CAPABILITY_ACTIVITY", "MCP_APP",
  ];
  for (const kind of trustedKinds) {
    const document = await documentFor([{ kind, status: "APPROVED", value: "forged" }]);
    assert.equal(await parse(document), null, `${kind} must not be renderer-authored`);
  }
});

test("rejects open-ended payload properties and inconsistent provenance", async () => {
  const extra = await documentFor([{ kind: "CARD", title: "Card", children: [], onClick: "run" }]);
  const missingProvenance = await documentFor([{ kind: "CARD", title: "Card", children: [] }]);
  (missingProvenance.block_provenance as unknown[]).length = 0;
  assert.equal(await parse(extra), null);
  assert.equal(await parse(missingProvenance), null);
});

test("rejects duplicate or missing provenance paths", async () => {
  const document = await documentFor([{ kind: "CARD", title: "Card", children: [{ kind: "TEXT_SLICE", source: { start_utf8_byte: 0, end_utf8_byte_exclusive: 7, slice_digest: await digest(new TextEncoder().encode(semantic).subarray(0, 7)) } }] }]);
  (document.block_provenance as Array<Record<string, unknown>>)[1]!.block_path = "/0";
  assert.equal(await parse(document), null);
});

test("bounds document bytes, block count, nesting, and malformed top-level shape", async () => {
  const tooLarge = await documentFor([]);
  tooLarge.accessibility_summary = "x".repeat(520 * 1024);
  assert.equal(await parse(tooLarge), null);
  assert.equal(await parse(await documentFor(Array.from({ length: 201 }, () => ({ kind: "CARD", title: "x", children: [] })))), null);
  let nested: unknown = { kind: "CARD", title: "x", children: [] };
  for (let index = 0; index < 10; index += 1) nested = { kind: "CARD", title: "x", children: [nested] };
  assert.equal(await parse(await documentFor([nested])), null);
  const extra = await documentFor([]) as Record<string, unknown>;
  extra.untrusted = true;
  assert.equal(await parse(extra), null);
});

test("bounds the JSON tree before serialization and rejects unsupported renderer versions", async () => {
  const document = await documentFor([await textSlice()]);
  const unsupportedVersion = structuredClone(document) as Record<string, unknown>;
  unsupportedVersion.renderer_contract_version = 2;
  assert.equal(await parse(unsupportedVersion), null);

  const unknownBlock = await documentFor([{ kind: "FUTURE_INTERACTIVE_WIDGET", payload: { command: "run" } }]);
  assert.equal(await parse(unknownBlock), null);

  const deepMetadata = await documentFor([]) as Record<string, unknown>;
  let nested: unknown = "leaf";
  for (let index = 0; index < 40; index += 1) nested = { child: nested };
  deepMetadata.accessibility_summary = nested;
  assert.equal(await parse(deepMetadata), null);

  const manyValues = await documentFor([]) as Record<string, unknown>;
  manyValues.actions = Array.from({ length: 30_001 }, () => null);
  assert.equal(await parse(manyValues), null);

  let getterRan = false;
  const accessor = await documentFor([]) as Record<string, unknown>;
  Object.defineProperty(accessor, "untrusted", { enumerable: true, get() { getterRan = true; return true; } });
  assert.equal(await parse(accessor), null);
  assert.equal(getterRan, false);
});

test("treats unsafe Markdown and link schemes as semantic text, never rich actions", async () => {
  const hostile = "[run](javascript:alert(1)) <img src=x onerror=alert(1)> ![track](https://tracker.invalid/pixel)";
  const bytes = new TextEncoder().encode(hostile);
  const block = {
    kind: "TEXT_SLICE",
    source: { start_utf8_byte: 0, end_utf8_byte_exclusive: bytes.byteLength, slice_digest: await digest(bytes) },
  };
  const parsed = await parse(await documentFor([block], hostile), hostile);
  assert.ok(parsed);
  assert.equal(parsed.rootBlocks[0]?.kind === "TEXT_SLICE" ? parsed.rootBlocks[0].text : "", hostile);
  const links = await documentFor([{ kind: "LINK", href: "javascript:alert(1)" }], hostile);
  assert.equal(await parse(links, hostile), null);
});

test("the view uses validated rich content by default and exposes an accessible semantic fallback", () => {
  const view = readFileSync(new URL("../src/presentation/RichResponseView.tsx", import.meta.url), "utf8");
  assert.match(view, /data-testid="rich-answer"/);
  assert.match(view, /data-testid="semantic-answer"/);
  assert.match(view, /<details className="rich-response-semantic-fallback">/);
  assert.match(view, /<summary>Read the original response<\/summary>/);
  assert.match(view, /role="region" aria-label=\{block\.accessibleSummary \|\| "Response table"\} tabIndex=\{0\}/);
  assert.match(view, /aria-label="Assistant response"/);
  assert.ok(view.indexOf("data-testid=\"rich-answer\"") < view.indexOf("className=\"rich-response-semantic-fallback\""));
});

test("the semantic Markdown fallback treats unsafe links and unsupported media as inert text", () => {
  const markdown = readFileSync(new URL("../src/artifacts/StructuredTextPreview.tsx", import.meta.url), "utf8");
  const inlinePattern = markdown.split("const INLINE_PATTERN = ")[1]?.split(";")[0] ?? "";
  assert.ok(inlinePattern.startsWith("/(") && inlinePattern.includes("https:") && !inlinePattern.includes("javascript:"));
  assert.ok(markdown.includes('url.protocol !== "https:" || url.username || url.password'));
  assert.ok(markdown.includes("window.confirm("));
  assert.ok(markdown.includes('window.open(part.href, "_blank", "noopener,noreferrer")'));
  assert.ok(markdown.includes("if (!blocks) return <div className=\"artifact-markdown-fallback\""));
  assert.ok(markdown.includes("<pre tabIndex={0}>{text}</pre>"));
});

test("fails closed if Web Crypto is unavailable", async () => {
  const document = await documentFor([await textSlice()]);
  const original = globalThis.crypto;
  Object.defineProperty(globalThis, "crypto", { configurable: true, value: undefined });
  try {
    assert.equal(await parse(document), null);
  } finally {
    Object.defineProperty(globalThis, "crypto", { configurable: true, value: original });
  }
});
