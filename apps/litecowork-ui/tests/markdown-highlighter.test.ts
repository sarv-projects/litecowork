import assert from "node:assert/strict";
import { test } from "node:test";
import { highlightCode } from "../src/artifacts/markdown-highlighter.ts";

test("highlights supported source as typed text tokens without changing the source", () => {
  const source = 'const answer = "safe"; // keep text';
  const tokens = highlightCode(source, "typescript");
  assert.ok(tokens);
  assert.equal(tokens.map(token => token.text).join(""), source);
  assert.ok(tokens.some(token => token.kind === "keyword" && token.text === "const"));
  assert.ok(tokens.some(token => token.kind === "string" && token.text === '"safe"'));
  assert.ok(tokens.some(token => token.kind === "comment" && token.text === "// keep text"));
});

test("leaves unsupported languages and oversized blocks unhighlighted", () => {
  assert.equal(highlightCode("<b>still text</b>", "made-up"), null);
  assert.equal(highlightCode("x".repeat(100_001), "typescript"), null);
  assert.equal(highlightCode("a;".repeat(11_000), "typescript"), null);
});

