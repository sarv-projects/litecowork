import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import {
  createSafeCalculatorState,
  evaluateSafeCalculator,
  formatSafeCalculatorFormula,
  formatSafeCalculatorValue,
  safeCalculatorReducer,
} from "../src/presentation/safe-calculator-state.ts";
import type { SafeCalculatorModel } from "../src/presentation/rich-presentation.ts";

const model: SafeCalculatorModel = {
  kind: "SAFE_CALCULATOR",
  inputs: [{ id: "amount", label: "Amount", initialValue: 12, minValue: 0, maxValue: 100, step: 1 }],
  formulaAst: {
    kind: "BINARY_OP", operator: "/",
    left: { kind: "LITERAL", value: 24 },
    right: { kind: "VARIABLE", input_id: "amount" },
  },
  outputFormat: { style: "NUMBER" },
  fallbackText: "The estimated amount is shown below.",
};

test("the browser harness loads axe only when the calculator accessibility audit runs", async () => {
  const harness = await readFile(new URL("./browser/harness.tsx", import.meta.url), "utf8");
  assert.doesNotMatch(harness, /^import\s+axe\s+from\s+["']axe-core["'];/m,
    "an unrelated Vite middleware page must not import axe-core's UMD entry as a native ES module");
  assert.match(harness, /async auditRichCalculatorA11y\(\)\s*\{[\s\S]*?await import\(["']axe-core["']\)/,
    "the bounded calculator audit still loads the registered axe-core dependency on demand");
});

test("formats the validated formula using input labels and inert operators", () => {
  assert.equal(formatSafeCalculatorFormula(model), "(24 ÷ Amount)");
});

test("local calculator state resets to exact source-bound starting values", () => {
  const original = createSafeCalculatorState(model);
  const changed = safeCalculatorReducer(original, { type: "change", id: "amount", value: "0" });
  assert.equal(changed.values.amount, "0");
  const reset = safeCalculatorReducer(changed, { type: "reset" });
  assert.equal(reset.values.amount, "12");
  assert.notEqual(reset, original);
});

test("invalid arithmetic and invalid field values fail without throwing", () => {
  assert.deepEqual(evaluateSafeCalculator(model, { amount: "0" }), { ok: false, reason: "DIVIDE_BY_ZERO" });
  assert.deepEqual(evaluateSafeCalculator(model, { amount: "101" }), { ok: false, reason: "INVALID_INPUTS" });
  assert.deepEqual(evaluateSafeCalculator(model, { amount: "" }), { ok: false, reason: "INVALID_INPUTS" });
});

test("formats percentage and currency using only the declared output format", () => {
  const percentage = { ...model, outputFormat: { style: "PERCENTAGE" as const } };
  const currency = { ...model, outputFormat: { style: "CURRENCY" as const, currencyCode: "USD" } };
  assert.equal(formatSafeCalculatorValue(2, percentage), "200%");
  assert.equal(formatSafeCalculatorValue(2, currency), "$2.00");
});
