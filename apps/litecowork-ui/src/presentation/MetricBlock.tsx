import { metricAggregationLabel, metricScaleLabel, type MetricModel } from "./chart-model";

function SourceTable({ model }: { model: MetricModel }) {
  return <div className="rich-response-table-scroll" role="region" aria-label={`Source data for ${model.label}`} tabIndex={0}>
    <table>
      <thead><tr>{model.sourceTable.headers.map((header, index) => <th scope="col" key={`${index}:${header}`}>{header}</th>)}</tr></thead>
      <tbody>{model.sourceTable.rows.map((row, rowIndex) => <tr key={rowIndex}>{row.map((cell, cellIndex) => <td key={cellIndex}>{cell}</td>)}</tr>)}</tbody>
    </table>
  </div>;
}

/** A metric is a formatted view of one bound semantic cell, never a calculation. */
export function MetricBlock({ model }: { model: MetricModel }) {
  return <figure className="rich-response-metric" data-testid="rich-metric">
    <figcaption className="rich-response-metric-label">{model.label}</figcaption>
    <p className="rich-response-metric-value"><data value={model.sourceValue}>{model.valueText}</data></p>
    <p className="rich-response-metric-context" data-testid="rich-metric-context">
      Unit: {model.semantics.unit}; scale: {metricScaleLabel(model.semantics.scale)}; interval: {model.semantics.timeInterval}; aggregation: {metricAggregationLabel(model.semantics.aggregation)}.
    </p>
    <p className="rich-response-metric-summary">{model.accessibleSummary}</p>
    <details className="rich-response-metric-source">
      <summary>View source data</summary>
      <SourceTable model={model} />
    </details>
  </figure>;
}
