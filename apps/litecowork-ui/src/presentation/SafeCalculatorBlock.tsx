import { useId, useReducer } from "react";
import type { CalcEvaluation } from "./calc-reducer";
import type { SafeCalculatorModel } from "./rich-presentation";
import { createSafeCalculatorState, evaluateSafeCalculator, formatSafeCalculatorFormula, formatSafeCalculatorValue, isSafeCalculatorInputValid, safeCalculatorReducer } from "./safe-calculator-state";

function resultText(result: CalcEvaluation, model: SafeCalculatorModel): string {
  return result.ok ? formatSafeCalculatorValue(result.value, model) : "Cannot calculate with these values";
}

/** View-local, bounded arithmetic only. This component has no host/provider API. */
export function SafeCalculatorBlock({ model }: { model: SafeCalculatorModel }) {
  const id = useId().replace(/:/g, "");
  const [state, dispatch] = useReducer(safeCalculatorReducer, model, createSafeCalculatorState);
  const result = evaluateSafeCalculator(model, state.values);
  const resultId = `rich-calculator-result-${id}`;
  const instructionsId = `rich-calculator-instructions-${id}`;

  return <section className="rich-response-calculator" data-testid="rich-calculator">
    <fieldset className="rich-response-calculator-inputs" aria-describedby={instructionsId}>
      <legend>Adjust the values</legend>
      <p id={instructionsId} className="rich-response-calculator-instructions">Changes stay in this view and are not sent to an agent.</p>
      {model.inputs.map(input => {
        const inputId = `rich-calculator-${id}-${input.id}`;
        const descriptionId = `${inputId}-bounds`;
        const raw = state.values[input.id] ?? "";
        const invalid = !isSafeCalculatorInputValid(raw, input.minValue, input.maxValue, input.step);
        return <div className="rich-response-calculator-field" key={input.id}>
          <label htmlFor={inputId}>{input.label}</label>
          <input id={inputId} type="number" inputMode="decimal" required
            min={input.minValue} max={input.maxValue} step={input.step}
            aria-describedby={descriptionId} aria-invalid={invalid ? "true" : undefined}
            value={raw} onChange={event => dispatch({ type: "change", id: input.id, value: event.currentTarget.value })} />
          <span className="rich-response-calculator-bounds" id={descriptionId}>
            Minimum {formatSafeCalculatorValue(input.minValue, model)}; maximum {formatSafeCalculatorValue(input.maxValue, model)}; step {formatSafeCalculatorValue(input.step, model)}.
          </span>
        </div>;
      })}
    </fieldset>
    <p className="rich-response-calculator-formula">Formula: <span>{formatSafeCalculatorFormula(model)}</span></p>
    <p className="rich-response-calculator-result-label">Estimated result</p>
    <output id={resultId} className="rich-response-calculator-result" aria-live="polite" aria-atomic="true" aria-label="Calculated result">
      {resultText(result, model)}
    </output>
    <button className="rich-response-calculator-reset" type="button" onClick={() => dispatch({ type: "reset" })}>
      Reset to original values
    </button>
    <p className="rich-response-calculator-fallback" data-testid="rich-calculator-fallback">{model.fallbackText}</p>
  </section>;
}
