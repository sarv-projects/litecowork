import { evaluateCalc } from "./calc-reducer.ts";
import { buildChartModel, buildMetricModel, validateMetricSemantics, type ChartModel, type ChartModelInput, type MetricModel } from "./chart-model.ts";

/**
 * Narrow desktop renderer contract for immutable RichPresentation v1/v2 documents.
 * This intentionally accepts only closed display-only block types currently rendered
 * by the desktop. Unsupported/host-bound blocks degrade to the semantic message.
 */

const VALIDATED = Symbol("validated-rich-presentation");
const DIGEST = /^sha256:[a-f0-9]{64}$/;
const MAX_DOCUMENT_BYTES = 512 * 1024;
const MAX_SEMANTIC_BYTES = 1024 * 1024;
const MAX_BLOCKS = 200;
const MAX_DEPTH = 8;
const MAX_JSON_VALUES = 30_000;
const MAX_JSON_DEPTH = 32;
const MAX_JSON_ESTIMATED_BYTES = MAX_DOCUMENT_BYTES * 2;

export type WidthClass = "READABLE" | "WIDE" | "FULL_AVAILABLE";
export type SafeCalculatorInput = {
  readonly id: string;
  readonly label: string;
  readonly initialValue: number;
  readonly minValue: number;
  readonly maxValue: number;
  readonly step: number;
};
export type SafeCalculatorOutputFormat =
  | { readonly style: "NUMBER" | "INTEGER" | "PERCENTAGE" }
  | { readonly style: "CURRENCY"; readonly currencyCode: string };
export type SafeCalculatorModel = {
  readonly kind: "SAFE_CALCULATOR";
  readonly inputs: readonly SafeCalculatorInput[];
  readonly formulaAst: unknown;
  readonly outputFormat: SafeCalculatorOutputFormat;
  readonly fallbackText: string;
};
export type RichBlock =
  | { kind: "TEXT_SLICE"; text: string; width?: WidthClass }
  | { kind: "LAYOUT"; layout: "STACK" | "ROW" | "GRID"; width?: WidthClass; children: RichBlock[] }
  | { kind: "CARD"; title: string; children: RichBlock[] }
  | { kind: "CALLOUT"; tone: "NEUTRAL" | "INFO" | "WARNING"; text: string; accessibleSummary: string }
  | { kind: "TABLE"; headers: string[]; rows: Array<Array<string | number | boolean | null>>; accessibleSummary?: string }
  | ({ kind: "METRIC" } & MetricModel)
  | ({ kind: "CHART" } & ChartModel)
  | SafeCalculatorModel;

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

const INVALID_JSON_TREE = Symbol("invalid-json-tree");

/**
 * Copy a small JSON-compatible data tree using own data descriptors only. Browser JS
 * cannot identify a transparent Proxy, so reflective traps may run during this pass;
 * any exception fails closed. No getter or later property read on the supplied graph
 * is allowed, and all validation/serialization runs against this detached snapshot.
 */
function snapshotBoundedJsonValue(root: unknown): unknown | typeof INVALID_JSON_TREE {
  const encoder = new TextEncoder();
  const visited = new WeakSet<object>();
  let values = 0;
  let estimatedBytes = 0;
  const snapshot = (value: unknown, depth: number): unknown | typeof INVALID_JSON_TREE => {
    if (++values > MAX_JSON_VALUES || depth > MAX_JSON_DEPTH) return INVALID_JSON_TREE;
    if (value === null || typeof value === "boolean") { estimatedBytes += 5; return value; }
    if (typeof value === "number") {
      if (!Number.isFinite(value) || (Number.isInteger(value) && !Number.isSafeInteger(value))) return INVALID_JSON_TREE;
      estimatedBytes += 24;
      return value;
    }
    if (typeof value === "string") {
      if (!isWellFormedUnicode(value) || value.length > MAX_DOCUMENT_BYTES) return INVALID_JSON_TREE;
      estimatedBytes += encoder.encode(value).byteLength + 2;
      return estimatedBytes <= MAX_JSON_ESTIMATED_BYTES ? value : INVALID_JSON_TREE;
    }
    if (typeof value !== "object" || value === null || visited.has(value)) return INVALID_JSON_TREE;
    visited.add(value);
    if (Array.isArray(value)) {
      const keys = Reflect.ownKeys(value);
      if (keys.length !== value.length + 1 || value.length > MAX_JSON_VALUES
        || keys.some((key) => typeof key !== "string" || key !== "length" && !/^(0|[1-9]\d*)$/.test(key)
          || key !== "length" && Number(key) >= value.length)) return INVALID_JSON_TREE;
      estimatedBytes += 2 + value.length;
      const result: unknown[] = [];
      for (let index = 0; index < value.length; index += 1) {
        const descriptor = Object.getOwnPropertyDescriptor(value, String(index));
        if (!descriptor || !("value" in descriptor) || !descriptor.enumerable) return INVALID_JSON_TREE;
        const child = snapshot(descriptor.value, depth + 1);
        if (child === INVALID_JSON_TREE) return INVALID_JSON_TREE;
        result.push(child);
      }
      return result;
    }
    const prototype = Object.getPrototypeOf(value);
    if (prototype !== Object.prototype && prototype !== null) return INVALID_JSON_TREE;
    const keys = Reflect.ownKeys(value);
    if (keys.length > MAX_JSON_VALUES) return INVALID_JSON_TREE;
    estimatedBytes += 2 + keys.length;
    const result: RecordValue = Object.create(null) as RecordValue;
    for (const key of keys) {
      if (typeof key !== "string" || !isWellFormedUnicode(key)) return INVALID_JSON_TREE;
      const descriptor = Object.getOwnPropertyDescriptor(value, key);
      if (!descriptor || !("value" in descriptor) || !descriptor.enumerable) return INVALID_JSON_TREE;
      estimatedBytes += encoder.encode(key).byteLength + 3;
      const child = snapshot(descriptor.value, depth + 1);
      if (child === INVALID_JSON_TREE) return INVALID_JSON_TREE;
      result[key] = child;
    }
    return estimatedBytes <= MAX_JSON_ESTIMATED_BYTES ? result : INVALID_JSON_TREE;
  };
  try {
    const result = snapshot(root, 0);
    return estimatedBytes <= MAX_JSON_ESTIMATED_BYTES ? result : INVALID_JSON_TREE;
  } catch {
    return INVALID_JSON_TREE;
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

type ValidatedContractBlock = RichBlock;

type SemanticTable = { headers: string[]; rows: string[][] };

function parseExactSemanticTable(value: string): SemanticTable | null {
  const lines = value.split(/\r\n|\r|\n/);
  while (lines.at(-1) === "") lines.pop();
  if (lines.length < 3) return null;
  const headers = markdownPipeRow(lines[0]!);
  const separator = markdownPipeRow(lines[1]!);
  if (!headers || !separator || headers.length < 1 || headers.length > 20
    || separator.length !== headers.length || !separator.every((cell) => /^:?-{3,}:?$/.test(cell))) return null;
  const rows: string[][] = [];
  for (const line of lines.slice(2)) {
    const row = markdownPipeRow(line);
    if (!row) return null;
    if (row.length !== headers.length || rows.length >= 500) return null;
    rows.push(row);
  }
  return rows.length > 0 ? { headers, rows } : null;
}

function finiteSemanticNumber(value: string): number | null {
  const trimmed = value.trim();
  if (!/^[+-]?(?:(?:\d+\.?\d*)|(?:\.\d+))(?:[eE][+-]?\d+)?$/.test(trimmed)) return null;
  const number = Number(trimmed);
  return Number.isFinite(number) ? number : null;
}

function validIndex(value: unknown, length: number): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0 && (value as number) < length;
}

async function boundTable(
  value: unknown,
  sliceKey: "table_slice" | "semantic_table_slice",
  ctx: ParseContext,
  withCell: boolean,
): Promise<{ text: string; table: SemanticTable; rowIndex?: number; columnIndex?: number } | null> {
  const keys = withCell ? ["source", sliceKey, "row_index", "column_index"] : ["source", sliceKey];
  if (!isRecord(value) || !hasOnlyKeys(value, keys) || value.source !== "SEMANTIC_TABLE") return null;
  if (withCell && (!Number.isSafeInteger(value.row_index) || !Number.isSafeInteger(value.column_index))) return null;
  const text = await readSlice(value[sliceKey], ctx);
  if (text === null) return null;
  const table = parseExactSemanticTable(text);
  if (!table) return null;
  if (!withCell) return { text, table };
  if (!validIndex(value.row_index, table.rows.length) || !validIndex(value.column_index, table.headers.length)) return null;
  return { text, table, rowIndex: value.row_index, columnIndex: value.column_index };
}

async function validateMetricBlock(value: RecordValue, ctx: ParseContext): Promise<boolean> {
  if (!hasOnlyKeys(value, ["kind", "binding", "semantics", "accessible_summary"]) || !isRecord(value.binding)
    || !boundedString(value.accessible_summary, 2_000, true)
    || !semanticLineMatches(ctx.semanticText, value.accessible_summary)
    || !hasOnlyKeys(value.binding, ["source", "table_slice", "row_index", "column_index"])) return false;
  const bound = await boundTable(value.binding, "table_slice", ctx, true);
  if (!bound || finiteSemanticNumber(bound.table.rows[bound.rowIndex!]![bound.columnIndex!]!) === null) return false;
  const header = bound.table.headers[bound.columnIndex!]!.trim();
  const sourceLabels = bound.table.headers.length === 1
    ? [header]
    : [header, bound.table.rows[bound.rowIndex!]![0]!.trim()];
  return validateMetricSemantics(value.semantics, value.accessible_summary, sourceLabels) !== null;
}

async function validateChartBlock(value: RecordValue, ctx: ParseContext): Promise<boolean> {
  if (!hasOnlyKeys(value, ["kind", "chart_kind", "binding", "category_column_index", "series_column_indexes", "accessible_summary", "semantics"])
    || !["LINE", "BAR", "AREA"].includes(String(value.chart_kind))
    || !boundedString(value.accessible_summary, 2_000)
    || !semanticLineMatches(ctx.semanticText, value.accessible_summary as string)
    || !isRecord(value.binding)) return false;
  const bound = await boundTable(value.binding, "semantic_table_slice", ctx, false);
  if (!bound || bound.table.rows.length < 2 || bound.table.rows.length > 60) return false;
  if (!validIndex(value.category_column_index, bound.table.headers.length)
    || !Array.isArray(value.series_column_indexes) || value.series_column_indexes.length < 1 || value.series_column_indexes.length > 6) return false;
  const seriesColumns = value.series_column_indexes as unknown[];
  const columns = new Set<number>([value.category_column_index]);
  for (const candidate of seriesColumns) {
    if (!validIndex(candidate, bound.table.headers.length) || columns.has(candidate)) return false;
    columns.add(candidate);
  }
  if (!bound.table.rows.every((row) => row[value.category_column_index as number]!.trim().length > 0
    && seriesColumns.every((column: unknown) => finiteSemanticNumber(row[column as number]!) !== null))) return false;
  const sourceLabels = [bound.table.headers[value.category_column_index as number]!.trim(),
    ...seriesColumns.map((column) => bound.table.headers[column as number]!.trim())];
  return validateMetricSemantics(value.semantics, value.accessible_summary as string, sourceLabels) !== null;
}

function validCalcIdentifier(value: unknown): value is string {
  return typeof value === "string" && value.length <= 64 && /^[A-Za-z][A-Za-z0-9_-]*$/.test(value);
}

function stepAligned(value: number, minimum: number, step: number): boolean {
  const quotient = (value - minimum) / step;
  if (!Number.isFinite(quotient)) return false;
  const nearest = Math.floor(quotient + 0.5);
  return Math.abs(quotient - nearest) <= 1e-9 * Math.max(Math.abs(quotient), 1);
}

function sourceLabelMatches(semanticText: string, label: string): boolean {
  return semanticLineMatches(semanticText, label) || semanticText.split(/\r\n|\r|\n/).some((line) => markdownPipeRow(line)?.includes(label));
}

function freezeJsonTree<T>(value: T): T {
  if (value !== null && typeof value === "object" && !Object.isFrozen(value)) {
    for (const child of Object.values(value as Record<string, unknown>)) freezeJsonTree(child);
    Object.freeze(value);
  }
  return value;
}

async function validateCalculatorBlock(value: RecordValue, ctx: ParseContext): Promise<SafeCalculatorModel | null> {
  if (!hasOnlyKeys(value, ["kind", "inputs", "formula_ast", "result_binding", "output_format", "fallback_text"])
    || !Array.isArray(value.inputs) || value.inputs.length < 1 || value.inputs.length > 8
    || !isRecord(value.output_format) || !boundedString(value.fallback_text, 2_000)
    || !semanticLineMatches(ctx.semanticText, value.fallback_text as string)) return null;
  const initialValues = new Map<string, number>();
  const inputs: SafeCalculatorInput[] = [];
  for (const candidate of value.inputs) {
    if (!isRecord(candidate) || !hasOnlyKeys(candidate, ["id", "label", "binding", "min_value", "max_value", "step"])
      || !validCalcIdentifier(candidate.id) || initialValues.has(candidate.id)
      || !boundedString(candidate.label, 200) || !sourceLabelMatches(ctx.semanticText, candidate.label as string)
      || typeof candidate.min_value !== "number" || !Number.isFinite(candidate.min_value)
      || typeof candidate.max_value !== "number" || !Number.isFinite(candidate.max_value)
      || typeof candidate.step !== "number" || !Number.isFinite(candidate.step) || candidate.step <= 0) return null;
    const binding = await boundTable(candidate.binding, "table_slice", ctx, true);
    if (!binding) return null;
    const initial = finiteSemanticNumber(binding.table.rows[binding.rowIndex!]![binding.columnIndex!]!);
    if (initial === null || candidate.min_value > candidate.max_value || initial < candidate.min_value || initial > candidate.max_value
      || !stepAligned(initial, candidate.min_value, candidate.step)) return null;
    initialValues.set(candidate.id, initial);
    inputs.push({
      id: candidate.id,
      label: candidate.label as string,
      initialValue: initial,
      minValue: candidate.min_value,
      maxValue: candidate.max_value,
      step: candidate.step,
    });
  }
  const format = value.output_format;
  let outputFormat: SafeCalculatorOutputFormat;
  if (format.style === "NUMBER" || format.style === "INTEGER") {
    if (!hasOnlyKeys(format, ["style"])) return null;
    outputFormat = { style: format.style };
  } else if (format.style === "PERCENTAGE") {
    if (!hasOnlyKeys(format, ["style"]) || !/%|percent/i.test(ctx.semanticText)) return null;
    outputFormat = { style: "PERCENTAGE" };
  } else if (format.style === "CURRENCY") {
    if (!hasOnlyKeys(format, ["style", "currency_code"]) || typeof format.currency_code !== "string"
      || !/^[A-Z]{3}$/.test(format.currency_code) || !ctx.semanticText.includes(format.currency_code)) return null;
    outputFormat = { style: "CURRENCY", currencyCode: format.currency_code };
  } else return null;

  const calculated = evaluateCalc(value.formula_ast, initialValues);
  const result = await boundTable(value.result_binding, "table_slice", ctx, true);
  if (!calculated.ok || !result) return null;
  const expected = finiteSemanticNumber(result.table.rows[result.rowIndex!]![result.columnIndex!]!);
  if (expected === null || Math.abs(calculated.value - expected) > 1e-9 * Math.max(1, Math.abs(calculated.value), Math.abs(expected))) return null;
  return Object.freeze({
    kind: "SAFE_CALCULATOR",
    inputs: Object.freeze(inputs.map(input => Object.freeze(input))),
    formulaAst: freezeJsonTree(value.formula_ast),
    outputFormat: Object.freeze(outputFormat),
    fallbackText: value.fallback_text as string,
  });
}

function semanticTableBlock(table: SemanticTable): RichBlock {
  return { kind: "TABLE", headers: [...table.headers], rows: table.rows.map(row => [...row]) };
}

async function parseBlock(value: unknown, path: string, ctx: ParseContext, depth: number, rendererVersion: number): Promise<ValidatedContractBlock | null> {
  if (!isRecord(value) || depth > MAX_DEPTH || ++ctx.count > MAX_BLOCKS || typeof value.kind !== "string") return null;
  ctx.paths.push(path);

  if (value.kind === "METRIC" || value.kind === "CHART" || value.kind === "SAFE_CALCULATOR") {
    if (rendererVersion !== 2) return null;
    if (value.kind === "METRIC") {
      if (!await validateMetricBlock(value, ctx)) return null;
      const bound = await boundTable(value.binding, "table_slice", ctx, true);
      if (!bound) return null;
      const model = buildMetricModel(bound.table, bound.rowIndex!, bound.columnIndex!, value.semantics, value.accessible_summary as string);
      return model ? { kind: "METRIC", ...model } : semanticTableBlock(bound.table);
    }
    if (value.kind === "CHART") {
      if (!await validateChartBlock(value, ctx)) return null;
      const bound = await boundTable(value.binding, "semantic_table_slice", ctx, false);
      if (!bound) return null;
      const input: ChartModelInput = {
        chartKind: value.chart_kind as "BAR" | "LINE" | "AREA",
        categoryColumnIndex: value.category_column_index as number,
        seriesColumnIndexes: value.series_column_indexes as number[],
        accessibleSummary: value.accessible_summary as string,
        semantics: value.semantics,
      };
      const model = buildChartModel(bound.table, input);
      return model ? { kind: "CHART", ...model } : semanticTableBlock(bound.table);
    }
    return validateCalculatorBlock(value, ctx);
  }

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
      const child = await parseBlock(value.children[index], `${path}/children/${index}`, ctx, depth + 1, rendererVersion);
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

type DocumentValidation = {
  rendererContractVersion: 1 | 2;
  renderable: ValidatedRichPresentation | null;
};

/** Contract validation is broader than the currently enabled desktop renderer. */
async function validateDocumentInternal(
  value: unknown,
  semanticText: string,
  expectedMessageId: string,
  expectedSemanticContentDigest: string,
): Promise<DocumentValidation | null> {
  if (!boundedString(semanticText, MAX_SEMANTIC_BYTES) || !boundedString(expectedMessageId, 256, true)
    || !DIGEST.test(expectedSemanticContentDigest)) return null;
  const encoder = new TextEncoder();
  const bytes = encoder.encode(semanticText);
  if (bytes.byteLength > MAX_SEMANTIC_BYTES) return null;
  const snapshot = snapshotBoundedJsonValue(value);
  if (snapshot === INVALID_JSON_TREE) return null;
  let encoded: string;
  try {
    encoded = JSON.stringify(snapshot);
  } catch { return null; }
  if (typeof encoded !== "string" || encoder.encode(encoded).byteLength > MAX_DOCUMENT_BYTES) return null;

  const document = isRecord(snapshot) ? snapshot : null;
  const rendererVersion = document?.renderer_contract_version;
  if (!document || !hasOnlyKeys(document, ["schema_version", "renderer_contract_version", "presentation_id", "message_id", "semantic_content_digest", "root_blocks", "citations", "actions", "accessibility_summary", "block_provenance"])
    || document.schema_version !== 1 || !(rendererVersion === 1 || rendererVersion === 2)
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
  const rootBlocks: ValidatedContractBlock[] = [];
  for (let index = 0; index < document.root_blocks.length; index += 1) {
    const block = await parseBlock(document.root_blocks[index], `/${index}`, ctx, 0, rendererVersion);
    if (!block) return null;
    rootBlocks.push(block);
  }
  if (!validateProvenance(document.block_provenance, ctx.paths)) return null;
  return {
    rendererContractVersion: rendererVersion,
    renderable: {
      presentationId: document.presentation_id as string,
      messageId: expectedMessageId,
      semanticContentDigest: expectedSemanticContentDigest,
      rootBlocks: rootBlocks as RichBlock[],
      ...(typeof document.accessibility_summary === "string" ? { accessibilitySummary: document.accessibility_summary } : {}),
      [VALIDATED]: true,
    },
  };
}

/**
 * Validate a bounded immutable document against the canonical renderer profiles. The
 * result is parser-branded only when every block in its renderer version is supported
 * by the current desktop; valid contract-only blocks remain semantic fallback.
 */
export async function validateRichPresentationDocument(
  value: unknown,
  semanticText: string,
  expectedMessageId: string,
  expectedSemanticContentDigest: string,
): Promise<1 | 2 | null> {
  const result = await validateDocumentInternal(value, semanticText, expectedMessageId, expectedSemanticContentDigest);
  return result?.rendererContractVersion ?? null;
}

/** Validate and return content only when every block has a native desktop renderer. */
export async function parseRichPresentationDocument(
  value: unknown,
  semanticText: string,
  expectedMessageId: string,
  expectedSemanticContentDigest: string,
): Promise<ValidatedRichPresentation | null> {
  const result = await validateDocumentInternal(value, semanticText, expectedMessageId, expectedSemanticContentDigest);
  return result?.renderable ?? null;
}

export type RichPresentationResponseBinding = {
  readonly workspaceId: string;
  readonly conversationId: string;
  readonly messageId: string;
  readonly presentationId: string;
  readonly schemaVersion: number;
  readonly rendererContractVersion: number;
  readonly semanticContentDigest: string;
  readonly documentDigest: string;
  readonly documentSizeBytes: number;
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
  const snapshot = snapshotBoundedJsonValue(value);
  const expectedSnapshot = snapshotBoundedJsonValue(expected);
  if (snapshot === INVALID_JSON_TREE || !isRecord(snapshot) || expectedSnapshot === INVALID_JSON_TREE || !isRecord(expectedSnapshot)) return null;
  const response = snapshot;
  const binding = expectedSnapshot as unknown as RichPresentationResponseBinding;
  if (!hasOnlyKeys(response, [
      "workspace_id", "conversation_id", "message_id", "presentation_id",
      "schema_version", "renderer_contract_version", "semantic_content_digest",
      "document_digest", "document_size_bytes", "document",
    ])
    || !boundedString(binding.workspaceId, 256, true)
    || !boundedString(binding.conversationId, 256, true)
    || !boundedString(binding.messageId, 256, true)
    || !boundedString(binding.presentationId, 256, true)
    || binding.schemaVersion !== 1
    || !(binding.rendererContractVersion === 1 || binding.rendererContractVersion === 2)
    || !DIGEST.test(binding.semanticContentDigest)
    || !DIGEST.test(binding.documentDigest)
    || !Number.isSafeInteger(binding.documentSizeBytes)
    || binding.documentSizeBytes < 1
    || binding.documentSizeBytes > MAX_DOCUMENT_BYTES
    || response.workspace_id !== binding.workspaceId
    || response.conversation_id !== binding.conversationId
    || response.message_id !== binding.messageId
    || response.presentation_id !== binding.presentationId
    || response.schema_version !== binding.schemaVersion
    || response.renderer_contract_version !== binding.rendererContractVersion
    || response.semantic_content_digest !== binding.semanticContentDigest
    || response.document_digest !== binding.documentDigest
    || response.document_size_bytes !== binding.documentSizeBytes
    || !isRecord(response.document)) return null;

  const canonical = canonicalJson(response.document);
  if (canonical === null) return null;
  const bytes = new TextEncoder().encode(canonical);
  if (bytes.byteLength !== response.document_size_bytes
    || await sha256(bytes) !== response.document_digest) return null;

  if (response.document.presentation_id !== response.presentation_id
    || response.document.schema_version !== response.schema_version
    || response.document.renderer_contract_version !== response.renderer_contract_version) return null;

  return parseRichPresentationDocument(
    response.document,
    semanticText,
    binding.messageId,
    binding.semanticContentDigest,
  );
}

export function isValidatedRichPresentation(value: unknown): value is ValidatedRichPresentation {
  return isRecord(value) && (value as Partial<ValidatedRichPresentation>)[VALIDATED] === true;
}

export function resolveRichResponseContent(semanticText: string, presentation?: unknown, enabled = true): {
  semanticText: string;
  presentation: ValidatedRichPresentation | null;
  primary: "RICH" | "SEMANTIC";
} {
  const validated = enabled && isValidatedRichPresentation(presentation) ? presentation : null;
  return { semanticText, presentation: validated, primary: validated ? "RICH" : "SEMANTIC" };
}
