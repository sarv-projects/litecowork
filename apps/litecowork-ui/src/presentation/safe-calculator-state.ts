import { evaluateCalc, type CalcEvaluation } from "./calc-reducer.ts";
import type { SafeCalculatorModel } from "./rich-presentation.ts";

export type SafeCalculatorState = {
  readonly initialValues: Readonly<Record<string, string>>;
  readonly values: Readonly<Record<string, string>>;
};

export type SafeCalculatorAction =
  | { readonly type: "change"; readonly id: string; readonly value: string }
  | { readonly type: "reset" };

export function createSafeCalculatorState(model: SafeCalculatorModel): SafeCalculatorState {
  const initialValues = Object.fromEntries(model.inputs.map(input => [input.id, String(input.initialValue)]));
  return { initialValues, values: { ...initialValues } };
}

export function safeCalculatorReducer(state: SafeCalculatorState, action: SafeCalculatorAction): SafeCalculatorState {
  if (action.type === "reset") return { ...state, values: { ...state.initialValues } };
  if (!Object.hasOwn(state.initialValues, action.id)) return state;
  return { ...state, values: { ...state.values, [action.id]: action.value } };
}

function stepAligned(value: number, minimum: number, step: number): boolean {
  const quotient = (value - minimum) / step;
  if (!Number.isFinite(quotient)) return false;
  const nearest = Math.floor(quotient + 0.5);
  return Math.abs(quotient - nearest) <= 1e-9 * Math.max(Math.abs(quotient), 1);
}

export function isSafeCalculatorInputValid(raw: string, minimum: number, maximum: number, step: number): boolean {
  if (typeof raw !== "string" || raw.trim() === "") return false;
  const value = Number(raw);
  return Number.isFinite(value) && value >= minimum && value <= maximum && stepAligned(value, minimum, step);
}

export function evaluateSafeCalculator(model: SafeCalculatorModel, rawValues: Readonly<Record<string, string>>): CalcEvaluation {
  const values = new Map<string, number>();
  for (const input of model.inputs) {
    const raw = rawValues[input.id];
    if (!isSafeCalculatorInputValid(raw, input.minValue, input.maxValue, input.step)) return { ok: false, reason: "INVALID_INPUTS" };
    values.set(input.id, Number(raw));
  }
  return evaluateCalc(model.formulaAst, values);
}

export function formatSafeCalculatorValue(value: number, model: SafeCalculatorModel): string {
  try {
    switch (model.outputFormat.style) {
      case "NUMBER":
        return new Intl.NumberFormat(undefined, { maximumFractionDigits: 6 }).format(value);
      case "INTEGER":
        return new Intl.NumberFormat(undefined, { maximumFractionDigits: 0 }).format(value);
      case "PERCENTAGE":
        return new Intl.NumberFormat(undefined, { style: "percent", maximumFractionDigits: 4 }).format(value);
      case "CURRENCY":
        return new Intl.NumberFormat(undefined, {
          style: "currency", currency: model.outputFormat.currencyCode, maximumFractionDigits: 2,
        }).format(value);
    }
  } catch {
    // Some three-letter codes are not supported by the host ICU data. Preserve the
    // explicit code and numeric value rather than throwing or inventing a symbol.
    const number = new Intl.NumberFormat(undefined, { maximumFractionDigits: 6 }).format(value);
    return model.outputFormat.style === "CURRENCY" ? `${number} ${model.outputFormat.currencyCode}` : number;
  }
}

export function formatSafeCalculatorFormula(model: SafeCalculatorModel): string {
  const labels = new Map(model.inputs.map(input => [input.id, input.label]));
  let visited = 0;
  const visit = (value: unknown, depth: number): string => {
    if (++visited > 64 || depth > 12 || value === null || typeof value !== "object" || Array.isArray(value)) return "formula unavailable";
    const node = value as Record<string, unknown>;
    if (node.kind === "LITERAL" && typeof node.value === "number" && Number.isFinite(node.value)) return String(node.value);
    if (node.kind === "VARIABLE" && typeof node.input_id === "string") return labels.get(node.input_id) ?? "input";
    if (node.kind === "UNARY_OP" && node.operator === "-") return `−(${visit(node.operand, depth + 1)})`;
    if (node.kind === "BINARY_OP" && ["+", "-", "*", "/", "%"].includes(String(node.operator))) {
      const operator = node.operator === "*" ? "×" : node.operator === "/" ? "÷" : String(node.operator);
      return `(${visit(node.left, depth + 1)} ${operator} ${visit(node.right, depth + 1)})`;
    }
    if (node.kind === "FUNCTION" && typeof node.function === "string" && Array.isArray(node.arguments)) {
      return `${node.function.toLowerCase()}(${node.arguments.map(argument => visit(argument, depth + 1)).join(", ")})`;
    }
    return "formula unavailable";
  };
  return visit(model.formulaAst, 0);
}
