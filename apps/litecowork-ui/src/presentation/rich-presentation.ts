/**
 * Narrow desktop renderer contract for immutable RichPresentation v1 documents.
 * This intentionally accepts only display-only block types currently rendered by the
 * desktop. Unsupported/host-bound blocks degrade to the semantic message.
 */

const VALIDATED = Symbol("validated-rich-presentation");
const DIGEST = /^sha256:[a-f0-9]{64}$/;
const MAX_DOCUMENT_BYTES = 512 * 1024;
const MAX_SEMANTIC_BYTES = 1024 * 1024;
const MAX_BLOCKS = 200;
const MAX_DEPTH = 8;
const MAX_JSON_VALUES = 30_000;
const MAX_JSON_DEPTH = 32;

export type WidthClass = "READABLE" | "WIDE" | "FULL_AVAILABLE";
export type RichBlock =
  | { kind: "TEXT_SLICE"; text: string; width?: WidthClass }
  | { kind: "LAYOUT"; layout: "STACK" | "ROW" | "GRID"; width?: WidthClass; children: RichBlock[] }
  | { kind: "CARD"; title: string; children: RichBlock[] }
  | { kind: "CALLOUT"; tone: "NEUTRAL" | "INFO" | "WARNING"; text: string; accessibleSummary: string }
  | { kind: "TABLE"; headers: string[]; rows: Array<Array<string | number | boolean | null>>; accessibleSummary?: string };

export type ValidatedRichPresentation = {
  readonly presentationId: string;
  readonly messageId: string;
  readonly semanticContentDigest: string;
  readonly rootBlocks: readonly RichBlock[];
  readonly accessibilitySummary?: string;
  readonly [VALIDATED]: true;
};

type ParseContext = { bytes: Uint8Array; semanticText: string; paths: string[]; count: number };
type RecordValue = Record<string, unknown>;

function isRecord(value: unknown): value is RecordValue {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function hasOnlyKeys(value: RecordValue, allowed: readonly string[]): boolean {
  return Object.keys(value).every((key) => allowed.includes(key));
}

function boundedString(value: unknown, max: number, nonempty = false): value is string {
  return typeof value === "string" && value.length <= max && isWellFormedUnicode(value)
    && !value.includes("\0") && (!nonempty || value.trim().length > 0);
}

function isWellFormedUnicode(value: string): boolean {
  for (let index = 0; index < value.length; index += 1) {
    const unit = value.charCodeAt(index);
    if (unit >= 0xd800 && unit <= 0xdbff) {
      const next = value.charCodeAt(index + 1);
      if (next < 0xdc00 || next > 0xdfff) return false;
      index += 1;
    } else if (unit >= 0xdc00 && unit <= 0xdfff) return false;
  }
  return true;
}

/**
 * Bound and validate the input tree before JSON.stringify. This avoids walking or
 * materializing arbitrarily large/deep objects and rejects getters/proxies' ordinary
 * object substitutes rather than executing attacker-provided serialization hooks.
 */
function isBoundedJsonValue(root: unknown): boolean {
  const encoder = new TextEncoder();
  const visited = new WeakSet<object>();
  const pending: Array<{ value: unknown; depth: number }> = [{ value: root, depth: 0 }];
  let values = 0;
  let estimatedBytes = 0;
  try {
    while (pending.length > 0) {
      const current = pending.pop()!;
      if (++values > MAX_JSON_VALUES || current.depth > MAX_JSON_DEPTH) return false;
      const value = current.value;
      if (value === null || typeof value === "boolean") { estimatedBytes += 5; continue; }
      if (typeof value === "number") {
        if (!Number.isFinite(value)) return false;
        estimatedBytes += 24;
        continue;
      }
      if (typeof value === "string") {
        if (!isWellFormedUnicode(value) || value.length > MAX_DOCUMENT_BYTES) return false;
        estimatedBytes += encoder.encode(value).byteLength + 2;
        if (estimatedBytes > MAX_DOCUMENT_BYTES) return false;
        continue;
      }
      if (typeof value !== "object" || value === null || visited.has(value)) return false;
      visited.add(value);
      if (Array.isArray(value)) {
        const ownKeys = Reflect.ownKeys(value);
        if (ownKeys.length !== value.length + 1 || value.length > MAX_JSON_VALUES) return false;
        estimatedBytes += 2 + value.length;
        for (let index = 0; index < value.length; index += 1) {
          const descriptor = Object.getOwnPropertyDescriptor(value, String(index));
          if (!descriptor || !("value" in descriptor) || !descriptor.enumerable) return false;
          pending.push({ value: descriptor.value, depth: current.depth + 1 });
        }
      } else {
        const prototype = Object.getPrototypeOf(value);
        if (prototype !== Object.prototype && prototype !== null) return false;
        const keys = Reflect.ownKeys(value);
        if (keys.length > MAX_JSON_VALUES) return false;
        estimatedBytes += 2 + keys.length;
        for (const key of keys) {
          if (typeof key !== "string") return false;
          const descriptor = Object.getOwnPropertyDescriptor(value, key);
          if (!descriptor || !("value" in descriptor) || !descriptor.enumerable) return false;
          if (!isWellFormedUnicode(key)) return false;
          estimatedBytes += encoder.encode(key).byteLength + 3;
          pending.push({ value: descriptor.value, depth: current.depth + 1 });
        }
      }
      if (estimatedBytes > MAX_DOCUMENT_BYTES) return false;
    }
    return estimatedBytes <= MAX_DOCUMENT_BYTES;
  } catch {
    return false;
  }
}

/** RFC 8785-compatible canonicalization for the already bounded I-JSON value tree. */
function canonicalJson(value: unknown): string | null {
  if (value === null || typeof value === "boolean" || typeof value === "string") {
    return JSON.stringify(value);
  }
  if (typeof value === "number") return Number.isFinite(value) ? JSON.stringify(value) : null;
  if (Array.isArray(value)) {
    const entries = value.map(canonicalJson);
    return entries.some((entry) => entry === null) ? null : `[${entries.join(",")}]`;
  }
  if (!isRecord(value)) return null;
  const entries: string[] = [];
  for (const key of Object.keys(value).sort()) {
    const entry = canonicalJson(value[key]);
    if (entry === null) return null;
    entries.push(`${JSON.stringify(key)}:${entry}`);
  }
  return `{${entries.join(",")}}`;
}

function optionalWidth(value: unknown): value is WidthClass | undefined {
  return value === undefined || value === "READABLE" || value === "WIDE" || value === "FULL_AVAILABLE";
}

function strictUtf8Slice(bytes: Uint8Array, start: unknown, end: unknown): string | null {
  if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end)) return null;
  const from = start as number;
  const to = end as number;
  if (from < 0 || to <= from || to > bytes.byteLength) return null;
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(bytes.subarray(from, to));
  } catch {
    return null;
  }
}

function markdownPipeRow(line: string): string[] | null {
  const trimmed = line.trim();
  if (!trimmed.startsWith("|") || !trimmed.endsWith("|")) return null;
  // Escaped pipes require a fuller Markdown parser; fail closed instead of guessing.
  const cells = trimmed.slice(1, -1).split("|").map((cell) => cell.trim());
  if (cells.some((cell) => cell.includes("\\"))) return null;
  return cells;
}

function semanticLineMatches(semanticText: string, candidate: string): boolean {
  return semanticText.replace(/\r\n?/g, "\n").split("\n").some((sourceLine) => {
    let line = sourceLine.trim();
    line = line.replace(/^#{1,6}[ \t]+/, "").trim();
    const emphasis = /^(\*\*|__|\*|_)(.+)\1$/.exec(line);
    if (emphasis?.[2]?.trim()) line = emphasis[2].trim();
    return line === candidate;
  });
}

function tableMatchesSemanticMarkdown(
  semanticText: string,
  headers: readonly string[],
  rows: readonly (readonly (string | number | boolean | null)[])[],
): boolean {
  const lines = semanticText.replace(/\r\n?/g, "\n").split("\n");
  for (let index = 0; index + 1 < lines.length; index += 1) {
    const semanticHeaders = markdownPipeRow(lines[index]);
    const separator = markdownPipeRow(lines[index + 1]);
    if (!semanticHeaders || !separator || semanticHeaders.length !== headers.length || separator.length !== headers.length) continue;
    if (!semanticHeaders.every((cell, cellIndex) => cell === headers[cellIndex])) continue;
    if (!separator.every((cell) => /^:?-{3,}:?$/.test(cell))) continue;

    const semanticRows: string[][] = [];
    let rowIndex = index + 2;
    while (rowIndex < lines.length) {
      const row = markdownPipeRow(lines[rowIndex]);
      if (!row) break;
      if (row.length !== headers.length) { semanticRows.length = 0; break; }
      semanticRows.push(row);
      rowIndex += 1;
    }
    if (semanticRows.length !== rows.length) continue;
    const exactRows = rows.every((row, currentRow) => row.every((cell, currentCell) =>
      semanticRows[currentRow]?.[currentCell] === (cell === null ? "—" : String(cell))));
    if (exactRows) return true;
  }
  return false;
}

async function sha256(bytes: Uint8Array): Promise<string | null> {
  const subtle = globalThis.crypto?.subtle;
  if (!subtle) return null;
  try {
    const digest = await subtle.digest("SHA-256", bytes);
    return `sha256:${Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
  } catch {
    return null;
  }
}

async function readSlice(value: unknown, ctx: ParseContext): Promise<string | null> {
  if (!isRecord(value) || !hasOnlyKeys(value, ["start_utf8_byte", "end_utf8_byte_exclusive", "slice_digest"])
    || typeof value.slice_digest !== "string" || !DIGEST.test(value.slice_digest)) return null;
  const text = strictUtf8Slice(ctx.bytes, value.start_utf8_byte, value.end_utf8_byte_exclusive);
  if (text === null) return null;
  const from = value.start_utf8_byte as number;
  const to = value.end_utf8_byte_exclusive as number;
  const actual = await sha256(ctx.bytes.subarray(from, to));
  return actual === value.slice_digest ? text : null;
}

async function parseBlock(value: unknown, path: string, ctx: ParseContext, depth: number): Promise<RichBlock | null> {
  if (!isRecord(value) || depth > MAX_DEPTH || ++ctx.count > MAX_BLOCKS || typeof value.kind !== "string") return null;
  ctx.paths.push(path);

  if (value.kind === "TEXT_SLICE") {
    if (!hasOnlyKeys(value, ["kind", "source", "width"]) || !optionalWidth(value.width)) return null;
    const text = await readSlice(value.source, ctx);
    return text === null ? null : { kind: "TEXT_SLICE", text, ...(value.width ? { width: value.width } : {}) };
  }

  if (value.kind === "LAYOUT" || value.kind === "CARD") {
    const isLayout = value.kind === "LAYOUT";
    const allowed = isLayout ? ["kind", "layout", "width", "children"] : ["kind", "title", "children"];
    if (!hasOnlyKeys(value, allowed) || !Array.isArray(value.children) || value.children.length < (isLayout ? 1 : 0) || value.children.length > 20) return null;
    if (isLayout && (!(value.layout === "STACK" || value.layout === "ROW" || value.layout === "GRID") || !optionalWidth(value.width))) return null;
    if (!isLayout && (!boundedString(value.title, 300, true) || !semanticLineMatches(ctx.semanticText, value.title))) return null;
    const children: RichBlock[] = [];
    for (let index = 0; index < value.children.length; index += 1) {
      const child = await parseBlock(value.children[index], `${path}/children/${index}`, ctx, depth + 1);
      if (!child) return null;
      children.push(child);
    }
    return isLayout
      ? { kind: "LAYOUT", layout: value.layout as "STACK" | "ROW" | "GRID", ...(value.width ? { width: value.width as WidthClass } : {}), children }
      : { kind: "CARD", title: value.title as string, children };
  }

  if (value.kind === "CALLOUT") {
    if (!hasOnlyKeys(value, ["kind", "tone", "source", "accessible_summary"])
      || !(value.tone === "NEUTRAL" || value.tone === "INFO" || value.tone === "WARNING")
      || !boundedString(value.accessible_summary, 1000) || !semanticLineMatches(ctx.semanticText, value.accessible_summary)) return null;
    const text = await readSlice(value.source, ctx);
    return text === null ? null : { kind: "CALLOUT", tone: value.tone, text, accessibleSummary: value.accessible_summary };
  }

  if (value.kind === "TABLE") {
    if (!hasOnlyKeys(value, ["kind", "headers", "rows", "accessible_summary"]) || !Array.isArray(value.headers)
      || value.headers.length < 1 || value.headers.length > 20 || !value.headers.every((header) => boundedString(header, 300))
      || !Array.isArray(value.rows) || value.rows.length > 500
      || (value.accessible_summary !== undefined && (!boundedString(value.accessible_summary, 2000) || !semanticLineMatches(ctx.semanticText, value.accessible_summary)))) return null;
    const rows: Array<Array<string | number | boolean | null>> = [];
    for (const row of value.rows) {
      if (!Array.isArray(row) || row.length !== value.headers.length || !row.every((cell) =>
        cell === null || typeof cell === "string" && boundedString(cell, 8192)
        || typeof cell === "boolean" || typeof cell === "number" && Number.isFinite(cell))) return null;
      rows.push(row as Array<string | number | boolean | null>);
    }
    // RichPresentation cannot introduce facts. Until the schema carries a source slice,
    // display a TABLE only when every header/cell exactly matches a complete Markdown
    // table in the committed semantic message.
    if (!tableMatchesSemanticMarkdown(ctx.semanticText, value.headers as string[], rows)) return null;
    return { kind: "TABLE", headers: value.headers as string[], rows, ...(typeof value.accessible_summary === "string" ? { accessibleSummary: value.accessible_summary } : {}) };
  }

  // Host projections, media, actions, charts, diagrams, and future blocks are not
  // interpreted here. The caller keeps rendering the complete semantic message.
  return null;
}

function validateProvenance(value: unknown, paths: readonly string[]): boolean {
  if (!Array.isArray(value) || value.length !== paths.length || value.length > MAX_BLOCKS) return false;
  const expected = new Set(paths);
  const seen = new Set<string>();
  // This renderer accepts only origins that do not claim trusted host state or an
  // Artifact/MCP binding. Those need an owning resolver before they can be displayed.
  // This UI renderer has no authorized Resource/Capability/Evidence resolver. Only
  // semantic/message and model-layout provenance with no external references can be
  // displayed here; unresolved bindings must use the semantic fallback.
  const origins = ["SEMANTIC_MESSAGE", "MODEL_INTENT"];
  for (const item of value) {
    if (!isRecord(item) || !hasOnlyKeys(item, ["block_path", "origin", "agent_session_id", "invocation_id", "resource_refs", "artifact_refs", "evidence_refs", "verification_refs"])
      || typeof item.block_path !== "string" || !expected.has(item.block_path) || seen.has(item.block_path)
      || !origins.includes(String(item.origin)) || !Array.isArray(item.resource_refs) || !Array.isArray(item.artifact_refs)
      || !Array.isArray(item.evidence_refs) || !Array.isArray(item.verification_refs)
      || item.resource_refs.length > 100 || item.artifact_refs.length > 100 || item.evidence_refs.length > 100 || item.verification_refs.length > 100
      || (item.agent_session_id !== undefined && !boundedString(item.agent_session_id, 256, true))
      || (item.invocation_id !== undefined && !boundedString(item.invocation_id, 256, true))
      || item.resource_refs.length !== 0 || item.artifact_refs.length !== 0
      || item.evidence_refs.length !== 0 || item.verification_refs.length !== 0
      || item.agent_session_id !== undefined || item.invocation_id !== undefined) return false;
    seen.add(item.block_path);
  }
  return seen.size === expected.size;
}

/**
 * Validate a bounded presentation before it reaches React. `expectedSemanticContentDigest`
 * must come from the authenticated message snapshot; each displayed slice is also hashed
 * here against its exact UTF-8 byte range. Unsupported/host-bound blocks return null.
 */
export async function parseRichPresentationDocument(
  value: unknown,
  semanticText: string,
  expectedMessageId: string,
  expectedSemanticContentDigest: string,
): Promise<ValidatedRichPresentation | null> {
  if (!boundedString(semanticText, MAX_SEMANTIC_BYTES) || !boundedString(expectedMessageId, 256, true)
    || !DIGEST.test(expectedSemanticContentDigest)) return null;
  const encoder = new TextEncoder();
  const bytes = encoder.encode(semanticText);
  if (bytes.byteLength > MAX_SEMANTIC_BYTES) return null;
  if (!isBoundedJsonValue(value)) return null;
  let encoded: string;
  try {
    encoded = JSON.stringify(value);
  } catch { return null; }
  if (typeof encoded !== "string" || encoder.encode(encoded).byteLength > MAX_DOCUMENT_BYTES) return null;

  const document = isRecord(value) ? value : null;
  if (!document || !hasOnlyKeys(document, ["schema_version", "renderer_contract_version", "presentation_id", "message_id", "semantic_content_digest", "root_blocks", "citations", "actions", "accessibility_summary", "block_provenance"])
    || document.schema_version !== 1 || document.renderer_contract_version !== 1
    || !boundedString(document.presentation_id, 256, true) || document.message_id !== expectedMessageId
    || document.semantic_content_digest !== expectedSemanticContentDigest || !Array.isArray(document.root_blocks)
    || document.root_blocks.length > MAX_BLOCKS
    || (document.accessibility_summary !== undefined && (!boundedString(document.accessibility_summary, 4000) || !semanticLineMatches(semanticText, document.accessibility_summary)))) return null;
  const citations = document.citations ?? [];
  const actions = document.actions ?? [];
  if (!Array.isArray(citations) || citations.length > 500 || !Array.isArray(actions) || actions.length > 200) return null;

  // This renderer has no authorized source/action resolver. Do not interpret links,
  // Artifact refs, citations, media URLs, or host state from this document.
  if (citations.length !== 0 || actions.length !== 0) return null;
  const ctx: ParseContext = { bytes, semanticText, paths: [], count: 0 };
  const rootBlocks: RichBlock[] = [];
  for (let index = 0; index < document.root_blocks.length; index += 1) {
    const block = await parseBlock(document.root_blocks[index], `/${index}`, ctx, 0);
    if (!block) return null;
    rootBlocks.push(block);
  }
  if (!validateProvenance(document.block_provenance, ctx.paths)) return null;
  return {
    presentationId: document.presentation_id as string,
    messageId: expectedMessageId,
    semanticContentDigest: expectedSemanticContentDigest,
    rootBlocks,
    ...(typeof document.accessibility_summary === "string" ? { accessibilitySummary: document.accessibility_summary } : {}),
    [VALIDATED]: true,
  };
}

export type RichPresentationResponseBinding = {
  readonly workspaceId: string;
  readonly conversationId: string;
  readonly messageId: string;
  readonly semanticContentDigest: string;
};

/**
 * Validate the complete authenticated API envelope before rendering. The server
 * authorizes Workspace/message ownership; the client independently checks those
 * bindings and the canonical immutable document's size and digest.
 */
export async function parseRichPresentationResponse(
  value: unknown,
  expected: RichPresentationResponseBinding,
  semanticText: string,
): Promise<ValidatedRichPresentation | null> {
  if (!isBoundedJsonValue(value) || !isRecord(value)
    || !hasOnlyKeys(value, [
      "workspace_id", "conversation_id", "message_id", "presentation_id",
      "schema_version", "renderer_contract_version", "semantic_content_digest",
      "document_digest", "document_size_bytes", "document",
    ])
    || !boundedString(expected.workspaceId, 256, true)
    || !boundedString(expected.conversationId, 256, true)
    || !boundedString(expected.messageId, 256, true)
    || !DIGEST.test(expected.semanticContentDigest)
    || value.workspace_id !== expected.workspaceId
    || value.conversation_id !== expected.conversationId
    || value.message_id !== expected.messageId
    || !boundedString(value.presentation_id, 256, true)
    || value.schema_version !== 1
    || !Number.isSafeInteger(value.renderer_contract_version)
    || (value.renderer_contract_version as number) < 1
    || value.semantic_content_digest !== expected.semanticContentDigest
    || typeof value.document_digest !== "string" || !DIGEST.test(value.document_digest)
    || !Number.isSafeInteger(value.document_size_bytes)
    || (value.document_size_bytes as number) < 1
    || (value.document_size_bytes as number) > MAX_DOCUMENT_BYTES
    || !isRecord(value.document)
    || !isBoundedJsonValue(value.document)) return null;

  const canonical = canonicalJson(value.document);
  if (canonical === null) return null;
  const bytes = new TextEncoder().encode(canonical);
  if (bytes.byteLength !== value.document_size_bytes
    || await sha256(bytes) !== value.document_digest) return null;

  if (value.document.presentation_id !== value.presentation_id
    || value.document.schema_version !== value.schema_version
    || value.document.renderer_contract_version !== value.renderer_contract_version) return null;

  return parseRichPresentationDocument(
    value.document,
    semanticText,
    expected.messageId,
    expected.semanticContentDigest,
  );
}

export function isValidatedRichPresentation(value: unknown): value is ValidatedRichPresentation {
  return isRecord(value) && (value as Partial<ValidatedRichPresentation>)[VALIDATED] === true;
}

export function resolveRichResponseContent(semanticText: string, presentation?: unknown): {
  semanticText: string;
  presentation: ValidatedRichPresentation | null;
  primary: "RICH" | "SEMANTIC";
} {
  const validated = isValidatedRichPresentation(presentation) ? presentation : null;
  return { semanticText, presentation: validated, primary: validated ? "RICH" : "SEMANTIC" };
}
