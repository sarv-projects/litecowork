import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import * as richPresentation from "../src/presentation/rich-presentation.ts";

const validateCandidate = (richPresentation as unknown as Record<string, unknown>).validateRichPresentationDocument as
  | ((value: unknown, semanticText: string, messageId: string, semanticDigest: string) => Promise<number | null>)
  | undefined;
const parseCandidate = richPresentation.parseRichPresentationDocument;
const resolveContent = richPresentation.resolveRichResponseContent;
const corpus = JSON.parse(readFileSync(new URL("../../../crates/domain-presentation/tests/corpus/cases.json", import.meta.url), "utf8")) as {
  corpus_version: number;
  cases: Array<{ id: string; scenario: string; expected: { accept: boolean; reason: string } }>;
};
const semanticDigest = `sha256:${"a".repeat(64)}`;
const messageId = "message-test";

async function digest(bytes: Uint8Array): Promise<string> {
  const result = await globalThis.crypto.subtle.digest("SHA-256", bytes);
  return `sha256:${Array.from(new Uint8Array(result), (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}

async function makeSlice(text: string, selected: string) {
  const encoder = new TextEncoder();
  const tableStart = text.indexOf(selected);
  assert.ok(tableStart >= 0, `fixture slice is missing: ${selected.slice(0, 40)}`);
  const start = encoder.encode(text.slice(0, tableStart)).byteLength;
  const end = start + encoder.encode(selected).byteLength;
  const bytes = encoder.encode(text).subarray(start, end);
  return { start_utf8_byte: start, end_utf8_byte_exclusive: end, slice_digest: await digest(bytes) };
}

function provenance(blocks: unknown[]) {
  const entries: Array<Record<string, unknown>> = [];
  const visit = (value: unknown, path: string) => {
    if (value === null || typeof value !== "object" || Array.isArray(value)) return;
    entries.push({ block_path: path, origin: "SEMANTIC_MESSAGE", resource_refs: [], artifact_refs: [], evidence_refs: [], verification_refs: [] });
    const children = (value as { children?: unknown[] }).children;
    children?.forEach((child, index) => visit(child, `${path}/children/${index}`));
  };
  blocks.forEach((block, index) => visit(block, `/${index}`));
  return entries;
}

function makeDocument(rendererVersion: number, semanticText: string, rootBlocks: unknown[]) {
  return {
    schema_version: 1,
    renderer_contract_version: rendererVersion,
    presentation_id: "presentation-test",
    message_id: messageId,
    semantic_content_digest: semanticDigest,
    root_blocks: rootBlocks,
    block_provenance: provenance(rootBlocks),
  };
}

function metricChartText() {
  return "Revenue for Feb is reported in INR at lakh scale for the monthly interval with sum aggregation.\n\nRevenue by Month is reported in INR at lakh scale for the monthly interval with sum aggregation.\n\nRevenue trend.\n\n| Month | Revenue |\n| --- | --- |\n| Jan | 12 |\n| Feb | 18 |";
}

async function metricChartBlocks(text: string, badIndex: boolean, contextMode: "valid" | "missing" | "ungrounded" | "unsafe" | "invalid-enum" | "unit-substring" = "valid") {
  const table = "| Month | Revenue |\n| --- | --- |\n| Jan | 12 |\n| Feb | 18 |";
  const tableSlice = await makeSlice(text, table);
  const semantics = contextMode === "unsafe"
    ? { unit: "INR\u202E", scale: "LAKH", time_interval: "monthly", aggregation: "SUM" }
    : contextMode === "invalid-enum"
      ? { unit: "INR", scale: "UNSUPPORTED", time_interval: "monthly", aggregation: "SUM" }
      : contextMode === "unit-substring"
        ? { unit: "IN", scale: "LAKH", time_interval: "monthly", aggregation: "SUM" }
        : { unit: "INR", scale: "LAKH", time_interval: "monthly", aggregation: "SUM" };
  const metricSummary = contextMode === "ungrounded" ? "Revenue trend." : "Revenue for Feb is reported in INR at lakh scale for the monthly interval with sum aggregation.";
  const chartSummary = contextMode === "ungrounded" ? "Revenue trend." : "Revenue by Month is reported in INR at lakh scale for the monthly interval with sum aggregation.";
  const metricContext = contextMode === "missing" ? {} : { semantics: { ...semantics }, accessible_summary: metricSummary };
  const chartContext = contextMode === "missing" ? {} : { semantics: { ...semantics } };
  return [
    { kind: "METRIC", binding: { source: "SEMANTIC_TABLE", table_slice: { ...tableSlice }, row_index: badIndex ? 99 : 1, column_index: 1 }, ...metricContext },
    { kind: "CHART", chart_kind: "LINE", binding: { source: "SEMANTIC_TABLE", semantic_table_slice: { ...tableSlice } }, category_column_index: 0, series_column_indexes: [1], accessible_summary: chartSummary, ...chartContext },
  ];
}

async function calculatorBlock(text: string, badResult: boolean) {
  const inputTable = "| Input | Value |\n| --- | --- |\n| Amount | 12 |";
  const resultTable = "| Input | Value |\n| --- | --- |\n| Amount | 12 |\n| Estimated amount | 24 |";
  return {
    kind: "SAFE_CALCULATOR",
    inputs: [{
      id: "amount", label: "Amount",
      binding: { source: "SEMANTIC_TABLE", table_slice: await makeSlice(text, inputTable), row_index: 0, column_index: 1 },
      min_value: 0, max_value: 100, step: 1,
    }],
    formula_ast: { kind: "BINARY_OP", operator: "*", left: { kind: "VARIABLE", input_id: "amount" }, right: { kind: "LITERAL", value: 2 } },
    result_binding: { source: "SEMANTIC_TABLE", table_slice: await makeSlice(text, resultTable), row_index: badResult ? 0 : 1, column_index: 1 },
    output_format: { style: "NUMBER" },
    fallback_text: "The estimated amount is shown below.",
  };
}

function boundaryDocument(lastCellLength: number) {
  const cells = Array.from({ length: 500 }, () => "x".repeat(1_040));
  cells[499] = "x".repeat(lastCellLength);
  const semanticText = `| Value |\n| --- |\n${cells.map((cell) => `| ${cell} |\n`).join("")}`;
  const document = makeDocument(1, semanticText, [{ kind: "TABLE", headers: ["Value"], rows: cells.map((cell) => [cell]) }]);
  return { semanticText, document, byteLength: new TextEncoder().encode(JSON.stringify(document)).byteLength, lastCellLength };
}

function exactBoundaryDocument() {
  const cap = 512 * 1024;
  let low = 0;
  let high = 8_192;
  while (low <= high) {
    const middle = low + Math.floor((high - low) / 2);
    const candidate = boundaryDocument(middle);
    if (candidate.byteLength === cap) return candidate;
    if (candidate.byteLength < cap) low = middle + 1;
    else high = middle - 1;
  }
  throw new Error("corpus fixture must be tunable to the exact document-byte boundary");
}

async function buildCase(scenario: string): Promise<{ version: number; semanticText: string; document: unknown }> {
  if (["NON_FINITE_JSON_REJECTED", "MAX_SAFE_INTEGER_VALID", "UNSAFE_INTEGER_TOKEN_REJECTED", "UNSAFE_EXPONENT_INTEGER_REJECTED"].includes(scenario)) {
    const token = scenario === "NON_FINITE_JSON_REJECTED" ? "1e400"
      : scenario === "MAX_SAFE_INTEGER_VALID" ? "9007199254740991"
      : scenario === "UNSAFE_INTEGER_TOKEN_REJECTED" ? "9007199254740992" : "1e+21";
    const raw = `{"schema_version":1,"renderer_contract_version":1,"presentation_id":"presentation-test","message_id":"${messageId}","semantic_content_digest":"${semanticDigest}","root_blocks":[{"kind":"TABLE","headers":["Value"],"rows":[[${token}]]}],"block_provenance":[{"block_path":"/0","origin":"SEMANTIC_MESSAGE","resource_refs":[],"artifact_refs":[],"evidence_refs":[],"verification_refs":[]}]}`;
    const semanticText = `| Value |\n| --- |\n| ${token} |`;
    return { version: 1, semanticText, document: JSON.parse(raw) };
  }
  if (scenario === "MAX_DOCUMENT_BYTES_VALID" || scenario === "EXCESS_DOCUMENT_BYTES_REJECTED") {
    const exact = exactBoundaryDocument();
    if (scenario === "MAX_DOCUMENT_BYTES_VALID") return { version: 1, semanticText: exact.semanticText, document: exact.document };
    return { version: 1, ...boundaryDocument(exact.lastCellLength + 1) };
  }

  const simpleTableText = "Summary\n\n| Metric | Value |\n| --- | --- |\n| Total | 12 |";
  switch (scenario) {
    case "V1_TABLE_VALID":
      return { version: 1, semanticText: simpleTableText, document: makeDocument(1, simpleTableText, [{ kind: "TABLE", headers: ["Metric", "Value"], rows: [["Total", 12]] }]) };
    case "V2_METRIC_CHART_VALID": {
      const text = metricChartText();
      return { version: 2, semanticText: text, document: makeDocument(2, text, await metricChartBlocks(text, false)) };
    }
    case "V2_METRIC_CHART_MISSING_CONTEXT": {
      const text = metricChartText();
      return { version: 2, semanticText: text, document: makeDocument(2, text, await metricChartBlocks(text, false, "missing")) };
    }
    case "V2_METRIC_CHART_UNGROUNDED_CONTEXT": {
      const text = metricChartText();
      return { version: 2, semanticText: text, document: makeDocument(2, text, await metricChartBlocks(text, false, "ungrounded")) };
    }
    case "V2_METRIC_CHART_UNSAFE_CONTEXT": {
      const text = metricChartText();
      return { version: 2, semanticText: text, document: makeDocument(2, text, await metricChartBlocks(text, false, "unsafe")) };
    }
    case "V2_METRIC_CHART_INVALID_ENUM": {
      const text = metricChartText();
      return { version: 2, semanticText: text, document: makeDocument(2, text, await metricChartBlocks(text, false, "invalid-enum")) };
    }
    case "V2_METRIC_CHART_TOKEN_SUBSTRING": {
      const text = metricChartText();
      return { version: 2, semanticText: text, document: makeDocument(2, text, await metricChartBlocks(text, false, "unit-substring")) };
    }
    case "V2_CALCULATOR_VALID":
    case "V2_CALCULATOR_BAD_RESULT": {
      const text = "Estimate.\n\n| Input | Value |\n| --- | --- |\n| Amount | 12 |\n| Estimated amount | 24 |\nThe estimated amount is shown below.";
      return { version: 2, semanticText: text, document: makeDocument(2, text, [await calculatorBlock(text, scenario === "V2_CALCULATOR_BAD_RESULT")]) };
    }
    case "V1_HOSTILE_TEXT_VALID": {
      const text = "<img src=x onerror=alert(1)> [link](javascript:alert(1))";
      return { version: 1, semanticText: text, document: makeDocument(1, text, [{ kind: "TEXT_SLICE", source: await makeSlice(text, text) }]) };
    }
    case "V1_UNKNOWN_KIND":
      return { version: 1, semanticText: "Summary", document: makeDocument(1, "Summary", [{ kind: "FUTURE_WIDGET", payload: { run: "anything" } }]) };
    case "V1_REJECTED_KINDS":
      return { version: 1, semanticText: "Summary", document: makeDocument(1, "Summary", ["HOST_PROJECTION", "MCP_APP", "DIAGRAM", "MEDIA", "CHECKLIST", "DELIVERABLE_GROUP", "METRIC", "CHART", "SAFE_CALCULATOR"].map((kind) => ({ kind }))) };
    case "V2_REJECTED_KINDS":
      return { version: 2, semanticText: "Summary", document: makeDocument(2, "Summary", ["HOST_PROJECTION", "MCP_APP", "DIAGRAM", "MEDIA", "CHECKLIST", "DELIVERABLE_GROUP", "INTERACTIVE_ARTIFACT", "FUTURE_WIDGET"].map((kind) => ({ kind }))) };
    case "V2_METRIC_BAD_INDEX": {
      const text = metricChartText();
      return { version: 2, semanticText: text, document: makeDocument(2, text, await metricChartBlocks(text, true)) };
    }
    case "MAX_NESTING_VALID":
    case "EXCESS_NESTING_REJECTED": {
      const count = scenario === "MAX_NESTING_VALID" ? 9 : 10;
      let nested: Record<string, unknown> = { kind: "CARD", title: "Summary", children: [] };
      for (let index = 1; index < count; index += 1) nested = { kind: "CARD", title: "Summary", children: [nested] };
      return { version: 1, semanticText: "Summary", document: makeDocument(1, "Summary", [nested]) };
    }
    case "MAX_BLOCK_COUNT_VALID":
    case "EXCESS_BLOCK_COUNT_REJECTED": {
      const count = scenario === "MAX_BLOCK_COUNT_VALID" ? 200 : 201;
      return { version: 1, semanticText: "Summary", document: makeDocument(1, "Summary", Array.from({ length: count }, () => ({ kind: "CARD", title: "Summary", children: [] }))) };
    }
    default:
      throw new Error(`unknown shared corpus scenario ${scenario}`);
  }
}

test("shared Rust and TypeScript validators agree on every golden corpus case", async () => {
  assert.equal(corpus.corpus_version, 1);
  assert.equal(typeof validateCandidate, "function");
  if (!validateCandidate) return;
  for (const testCase of corpus.cases) {
    const fixture = await buildCase(testCase.scenario);
    const result = await validateCandidate(fixture.document, fixture.semanticText, messageId, semanticDigest);
    assert.equal(result !== null, testCase.expected.accept, `${testCase.id}: ${testCase.expected.reason}`);
    if (result !== null) assert.equal(result, fixture.version, `${testCase.id}: renderer profile`);
  }
});

test("renderer-v2 METRIC and CHART parse while unsupported calculators stay semantic-only", async () => {
  assert.equal(typeof validateCandidate, "function");
  if (!validateCandidate) return;
  const fixture = await buildCase("V2_METRIC_CHART_VALID");
  assert.equal(await validateCandidate(fixture.document, fixture.semanticText, messageId, semanticDigest), 2);
  const parsed = await parseCandidate(fixture.document, fixture.semanticText, messageId, semanticDigest);
  assert.ok(parsed);
  assert.deepEqual(parsed.rootBlocks.map((block) => block.kind), ["METRIC", "CHART"]);
  assert.equal(resolveContent(fixture.semanticText, parsed).primary, "RICH");

  const calculator = await buildCase("V2_CALCULATOR_VALID");
  assert.equal(await validateCandidate(calculator.document, calculator.semanticText, messageId, semanticDigest), 2);
  assert.equal(await parseCandidate(calculator.document, calculator.semanticText, messageId, semanticDigest), null);
  assert.deepEqual(resolveContent(fixture.semanticText, null), {
    semanticText: fixture.semanticText,
    presentation: null,
    primary: "SEMANTIC",
  });
});

test("bounded descriptor snapshot does not invoke getters or later Proxy reads", async () => {
  assert.equal(typeof validateCandidate, "function");
  if (!validateCandidate) return;
  const fixture = await buildCase("V1_TABLE_VALID");
  let getterRan = false;
  const accessor = { ...(fixture.document as Record<string, unknown>) };
  Object.defineProperty(accessor, "extra", { enumerable: true, get() { getterRan = true; return "no"; } });
  assert.equal(await validateCandidate(accessor, fixture.semanticText, messageId, semanticDigest), null);
  assert.equal(getterRan, false);

  const cycle = { ...(fixture.document as Record<string, unknown>) } as Record<string, unknown>;
  cycle.loop = cycle;
  assert.equal(await validateCandidate(cycle, fixture.semanticText, messageId, semanticDigest), null);

  let propertyRead = false;
  const transparentProxy = new Proxy(fixture.document as object, {
    get(target, property, receiver) {
      propertyRead = true;
      throw new Error(`unexpected proxy property read: ${String(property)}`);
    },
  });
  assert.equal(await validateCandidate(transparentProxy, fixture.semanticText, messageId, semanticDigest), 1);
  assert.equal(propertyRead, false);

  const throwingProxy = new Proxy(fixture.document as object, { ownKeys() { throw new Error("reflection denied"); } });
  assert.equal(await validateCandidate(throwingProxy, fixture.semanticText, messageId, semanticDigest), null);
});
