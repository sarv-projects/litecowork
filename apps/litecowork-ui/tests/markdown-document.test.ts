import assert from "node:assert/strict";
import { test } from "node:test";
import { parseMarkdownDocument } from "../src/artifacts/markdown-document.ts";

test("parses nested ordered and unordered lists without flattening parent relationships", () => {
  const document = parseMarkdownDocument("- First\n  - Child\n    1. Grandchild\n- Second");
  const list = document.blocks[0];
  assert.equal(list?.kind, "list");
  if (list?.kind !== "list") return;
  assert.equal(list.ordered, false);
  assert.equal(list.items[0]?.children[0]?.items[0]?.children[0]?.ordered, true);
  assert.equal(list.items[1]?.content[0]?.text, "Second");
});

test("parses GFM table headers, rows, and alignment", () => {
  const document = parseMarkdownDocument("| Month | Revenue |\n| :--- | ---: |\n| Jan | 12 |\n| Feb | 18 |");
  const table = document.blocks[0];
  assert.equal(table?.kind, "table");
  if (table?.kind !== "table") return;
  assert.deepEqual(table.alignments, ["left", "right"]);
  assert.deepEqual(table.headers.map(cell => cell.map(token => token.text).join("")), ["Month", "Revenue"]);
  assert.equal(table.rows.length, 2);

  const oneColumn = parseMarkdownDocument("| Status |\n| --- |\n| Ready |");
  assert.equal(oneColumn.blocks[0]?.kind, "table");
});

test("an over-bound or malformed table falls back as one literal block", () => {
  const wideHeader = `| ${Array.from({ length: 33 }, (_, index) => `H${index}`).join(" | ")} |`;
  const wideRule = `| ${Array.from({ length: 33 }, () => "---").join(" | ")} |`;
  const wide = `${wideHeader}\n${wideRule}\n| ${Array.from({ length: 33 }, () => "x").join(" | ")} |`;
  const tooWide = parseMarkdownDocument(wide);
  assert.equal(tooWide.blocks.length, 1);
  assert.equal(tooWide.blocks[0]?.kind, "literal");
  assert.equal(tooWide.blocks[0]?.kind === "literal" ? tooWide.blocks[0].text : "", wide);

  const largeCell = `| ${"x".repeat(8_193)} | B |\n| --- | --- |\n| value | ok |`;
  const tooLargeCell = parseMarkdownDocument(largeCell);
  assert.equal(tooLargeCell.blocks.length, 1);
  assert.equal(tooLargeCell.blocks[0]?.kind, "literal");
  assert.equal(tooLargeCell.blocks[0]?.kind === "literal" ? tooLargeCell.blocks[0].text : "", largeCell);

  const mismatched = parseMarkdownDocument("| A | B |\n| --- | --- |\n| one |\n| two | three |");
  assert.equal(mismatched.blocks.length, 1);
  assert.equal(mismatched.blocks[0]?.kind, "literal");
});

test("parses horizontal rules and fenced code language labels", () => {
  const document = parseMarkdownDocument("Before\n\n---\n\n```typescript\nconst answer = 42;\n```");
  assert.deepEqual(document.blocks.map(block => block.kind), ["paragraph", "rule", "code"]);
  const code = document.blocks[2];
  assert.equal(code?.kind, "code");
  if (code?.kind === "code") {
    assert.equal(code.language, "typescript");
    assert.equal(code.code, "const answer = 42;");
  }
});

test("unsupported syntax degrades locally and keeps neighboring blocks", () => {
  const document = parseMarkdownDocument("# Heading\n\n    unsupported indented code\n\nA normal paragraph remains.");
  assert.deepEqual(document.blocks.map(block => block.kind), ["heading", "literal", "paragraph"]);
  const fallback = document.blocks[1];
  assert.equal(fallback?.kind === "literal" ? fallback.text : "", "    unsupported indented code");
});

test("raw HTML, image syntax, and unsafe links remain inert source text", () => {
  const document = parseMarkdownDocument('<img src=x onerror="run()"> ![remote](https://example.invalid/a.png) [unsafe](javascript:run())');
  assert.equal(document.blocks.length, 1);
  const block = document.blocks[0];
  assert.equal(block?.kind, "paragraph");
  if (block?.kind !== "paragraph") return;
  const tokens = block.lines.flat();
  assert.equal(tokens.some(token => token.kind === "link"), false);
  assert.equal(tokens.map(token => token.text).join(""), '<img src=x onerror="run()"> ![remote](https://example.invalid/a.png) [unsafe](javascript:run())');
});

test("document bounds produce one explicit literal fallback instead of unbounded parsing", () => {
  const source = "a".repeat(1_048_577);
  const document = parseMarkdownDocument(source);
  assert.equal(document.limited, true);
  assert.equal(document.blocks.length, 1);
  assert.equal(document.blocks[0]?.kind, "literal");
  assert.equal(document.blocks[0]?.kind === "literal" ? document.blocks[0].text.length : 0, source.length);
});

test("line and line-count limits are checked before allocating the parse tree", () => {
  const overlongLine = parseMarkdownDocument("x".repeat(16_385));
  assert.equal(overlongLine.limited, true);
  assert.equal(overlongLine.blocks[0]?.kind, "literal");

  const tooManyLines = parseMarkdownDocument("\n".repeat(20_000));
  assert.equal(tooManyLines.limited, true);
  assert.equal(tooManyLines.blocks.length, 1);
  assert.equal(tooManyLines.blocks[0]?.kind, "literal");
});

test("excessive list depth and item count degrade only the list", () => {
  const tooDeep = Array.from({ length: 9 }, (_, depth) => `${"  ".repeat(depth)}- item ${depth}`).join("\n");
  const tooMany = Array.from({ length: 2_001 }, (_, index) => `- item ${index}`).join("\n");
  const unsafeOrderedStart = `${"9".repeat(40)}. value\n10. next`;
  assert.equal(parseMarkdownDocument(tooDeep).blocks[0]?.kind, "literal");
  assert.equal(parseMarkdownDocument(tooMany).blocks[0]?.kind, "literal");
  assert.equal(parseMarkdownDocument(unsafeOrderedStart).blocks[0]?.kind, "literal");
});

test("rendered block count stays bounded while preserving the remaining source", () => {
  const source = Array.from({ length: 2_001 }, (_, index) => `Paragraph ${index}`).join("\n\n");
  const document = parseMarkdownDocument(source);
  assert.equal(document.limited, true);
  assert.ok(document.blocks.length <= 2_000);
  assert.ok(document.blocks.at(-1)?.kind === "literal");
});

