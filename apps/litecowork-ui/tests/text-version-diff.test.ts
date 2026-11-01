import assert from "node:assert/strict";
import test from "node:test";
import { lineDiff } from "../src/artifacts/text-version-diff-model.ts";

test("equal and empty versions produce only unchanged rows", () => {
  assert.deepEqual(lineDiff("", ""), {
    rows: [{ kind: "same", comparedLine: 1, selectedLine: 1, comparedText: "", selectedText: "" }],
    additions: 0,
    removals: 0,
  });
  assert.deepEqual(lineDiff("first\nsecond", "first\nsecond"), {
    rows: [
      { kind: "same", comparedLine: 1, selectedLine: 1, comparedText: "first", selectedText: "first" },
      { kind: "same", comparedLine: 2, selectedLine: 2, comparedText: "second", selectedText: "second" },
    ],
    additions: 0,
    removals: 0,
  });
});

test("reports additions, removals, and replacements with exact line numbers", () => {
  assert.deepEqual(lineDiff("keep\nremove\nreplace-me", "keep\nadd\nreplace-you"), {
    rows: [
      { kind: "same", comparedLine: 1, selectedLine: 1, comparedText: "keep", selectedText: "keep" },
      { kind: "removed", comparedLine: 2, selectedLine: null, comparedText: "remove", selectedText: "" },
      { kind: "removed", comparedLine: 3, selectedLine: null, comparedText: "replace-me", selectedText: "" },
      { kind: "added", comparedLine: null, selectedLine: 2, comparedText: "", selectedText: "add" },
      { kind: "added", comparedLine: null, selectedLine: 3, comparedText: "", selectedText: "replace-you" },
    ],
    additions: 2,
    removals: 2,
  });
});

test("uses a stable remove-then-add order for reordered lines", () => {
  assert.deepEqual(lineDiff("A\nB", "B\nA"), {
    rows: [
      { kind: "removed", comparedLine: 1, selectedLine: null, comparedText: "A", selectedText: "" },
      { kind: "same", comparedLine: 2, selectedLine: 1, comparedText: "B", selectedText: "B" },
      { kind: "added", comparedLine: null, selectedLine: 2, comparedText: "", selectedText: "A" },
    ],
    additions: 1,
    removals: 1,
  });
});

test("normalizes CRLF and bare carriage returns before comparing lines", () => {
  assert.deepEqual(lineDiff("one\r\ntwo\rthree", "one\ntwo\nthree"), {
    rows: [
      { kind: "same", comparedLine: 1, selectedLine: 1, comparedText: "one", selectedText: "one" },
      { kind: "same", comparedLine: 2, selectedLine: 2, comparedText: "two", selectedText: "two" },
      { kind: "same", comparedLine: 3, selectedLine: 3, comparedText: "three", selectedText: "three" },
    ],
    additions: 0,
    removals: 0,
  });
});

test("keeps hostile markup as literal source text", () => {
  const hostile = "<img src=x onerror=alert(1)>";
  const result = lineDiff(hostile, hostile);
  assert.equal(result?.rows[0].comparedText, hostile);
  assert.equal(result?.rows[0].selectedText, hostile);
});

test("falls back beyond the line-count or line-length limits", () => {
  assert.equal(lineDiff(Array.from({ length: 401 }, () => "line").join("\n"), "small"), null);
  assert.equal(lineDiff("x".repeat(16_385), "small"), null);
});

test("accepts the largest grid allowed by the combined line and cell bounds", () => {
  const text = Array.from({ length: 400 }, (_, index) => `line-${index}`).join("\n");
  const result = lineDiff(text, text);
  assert.equal(result?.rows.length, 400);
  assert.equal(result?.additions, 0);
  assert.equal(result?.removals, 0);
});
