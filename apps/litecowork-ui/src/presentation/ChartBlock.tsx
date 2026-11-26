import { useId, useState } from "react";
import { metricAggregationLabel, metricScaleLabel, type ChartModel } from "./chart-model";

function SourceTable({ model, id, hidden }: { model: ChartModel; id: string; hidden: boolean }) {
  return <div id={id} className="rich-response-chart-table rich-response-table-scroll" role="region" aria-label="Chart data table" tabIndex={0} hidden={hidden}>
    <table>
      <thead><tr>{model.sourceTable.headers.map((header, index) => <th scope="col" key={`${index}:${header}`}>{header}</th>)}</tr></thead>
      <tbody>{model.sourceTable.rows.map((row, rowIndex) => <tr key={rowIndex}>{row.map((cell, cellIndex) => <td key={cellIndex}>{cell}</td>)}</tr>)}</tbody>
    </table>
  </div>;
}

function SeriesMark({ index }: { index: number }) {
  return <span className={`rich-response-chart-legend-mark chart-series-${index}`} aria-hidden="true" />;
}

/** Host-generated SVG only; the exact bound table remains one keyboard action away. */
export function ChartBlock({ model }: { model: ChartModel }) {
  const id = useId().replace(/:/g, "");
  const [showData, setShowData] = useState(false);
  const summaryId = `rich-chart-summary-${id}`;
  const tableId = `rich-chart-table-${id}`;
  const plotBottom = model.height - 48;
  const plotRight = model.width - 16;

  return <figure className="rich-response-chart" data-testid="rich-chart" data-chart-kind={model.chartKind}>
    <figcaption id={summaryId} className="rich-response-chart-summary">{model.accessibleSummary}</figcaption>
    <p className="rich-response-chart-context" data-testid="rich-chart-context">
      Unit: {model.semantics.unit}; scale: {metricScaleLabel(model.semantics.scale)}; interval: {model.semantics.timeInterval}; aggregation: {metricAggregationLabel(model.semantics.aggregation)}.
    </p>
    <div className="rich-response-chart-visual" hidden={showData}>
      <svg className="rich-response-chart-svg" viewBox={`0 0 ${model.width} ${model.height}`} role="img" aria-labelledby={summaryId}>
        <title>{model.accessibleSummary}</title>
        <desc>Static {model.chartKind.toLowerCase()} chart. Use the Show data table button to inspect every source value.</desc>
        {model.ticks.map((tick, index) => <g key={`tick-${index}`}>
          <line className="rich-response-chart-grid" x1="64" x2={plotRight} y1={tick.y} y2={tick.y} />
          <text className="rich-response-chart-axis-label" x="58" y={tick.y + 4} textAnchor="end">{tick.label}</text>
        </g>)}
        {model.categoryLabels.map(category => <text className="rich-response-chart-axis-label" key={category.index}
          x={category.x} y={plotBottom + 20} textAnchor="middle" aria-hidden="true">
          <title>{category.label}</title>{category.label.length > 14 ? `${category.label.slice(0, 13)}…` : category.label}
        </text>)}
        {model.series.map((series, seriesIndex) => {
          const seriesClass = `chart-series-${seriesIndex}`;
          if (model.chartKind === "BAR") return <g className={seriesClass} key={series.label}>
            {series.bars.map((bar, index) => <rect className="rich-response-chart-bar" key={`${seriesIndex}-${index}`}
              x={bar.x} y={bar.y} width={bar.width} height={bar.height} rx="1" />)}
          </g>;
          return <g className={seriesClass} key={series.label}>
            {model.chartKind === "AREA" && <path className="rich-response-chart-area" d={series.areaPath} />}
            <path className="rich-response-chart-line" d={series.linePath} />
            {series.points.map((point, index) => <circle className="rich-response-chart-point" key={`${seriesIndex}-${index}`} cx={point.x} cy={point.y} r="3" />)}
          </g>;
        })}
      </svg>
      <ul className="rich-response-chart-legend" aria-label="Chart series">
        {model.series.map((series, index) => <li key={`${index}:${series.label}`}><SeriesMark index={index} />{series.label}</li>)}
      </ul>
    </div>
    <SourceTable model={model} id={tableId} hidden={!showData} />
    <button className="rich-response-chart-toggle" type="button" aria-expanded={showData} aria-controls={tableId}
      onClick={() => setShowData(value => !value)}>
      {showData ? "Show chart" : "Show data table"}
    </button>
  </figure>;
}
