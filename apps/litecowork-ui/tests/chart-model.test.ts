import assert from "node:assert/strict";
import { test } from "node:test";
import { buildChartModel, buildMetricModel, type SemanticTableModel } from "../src/presentation/chart-model.ts";

const table: SemanticTableModel = {
  headers: ["Region", "Revenue", "Costs"],
  rows: [["North", "1200", "800"], ["South", "2400", "900"]],
};
const metricSemantics = { unit: "INR", scale: "UNIT" as const, time_interval: "monthly", aggregation: "SUM" };
const metricSummary = "Revenue for North is reported in INR at unit scale for monthly interval with sum aggregation.";
const chartSummary = "Revenue and Costs by Region are reported in INR at unit scale for monthly interval with sum aggregation.";

test("metric model derives its label from the exact source header and row label", () => {
  const model = buildMetricModel(table, 0, 1, metricSemantics, metricSummary, "en-US");
  assert.deepEqual(model && {
    label: model.label,
    sourceValue: model.sourceValue,
    valueText: model.valueText,
    accessibleSummary: model.accessibleSummary,
    unit: model.semantics.unit,
    scale: model.semantics.scale,
    interval: model.semantics.timeInterval,
    aggregation: model.semantics.aggregation,
  }, {
    label: "Revenue — North",
    sourceValue: "1200",
    valueText: "1,200",
    accessibleSummary: metricSummary,
    unit: "INR",
    scale: "UNIT",
    interval: "monthly",
    aggregation: "SUM",
  });
});

test("one-column metric uses a one-based row label and rejects ambiguous multi-column labels", () => {
  const singleColumnSummary = "Score is reported for Row 2 in points at unit scale for not time-based interval with no aggregation.";
  const noAggregation = { unit: "points", scale: "UNIT", time_interval: "not time-based", aggregation: "NONE" };
  assert.equal(buildMetricModel({ headers: ["Score"], rows: [["4"], ["5"]] }, 1, 0, noAggregation, singleColumnSummary, "en-US")?.label, "Score — Row 2");
  assert.equal(buildMetricModel({ headers: ["Region", ""], rows: [["North", "2"]] }, 0, 1, metricSemantics, metricSummary), null);
  assert.equal(buildMetricModel(table, 0, 0, metricSemantics, metricSummary), null);
});

test("metric context must be declared in the exact source-backed summary and is never inferred", () => {
  assert.equal(buildMetricModel(table, 0, 1, metricSemantics, "Revenue for North is 1,200."), null);
  assert.equal(buildMetricModel(table, 0, 1, { ...metricSemantics, unit: "USD" }, metricSummary), null);
  assert.equal(buildMetricModel(table, 0, 1, { ...metricSemantics, unit: "IN" }, metricSummary), null);
  assert.equal(buildMetricModel(table, 0, 1, { ...metricSemantics, time_interval: "month" }, metricSummary), null);
  assert.equal(buildMetricModel(table, 0, 1, metricSemantics, "revenue for North is reported in INR at unit scale for monthly interval with sum aggregation."), null);
  assert.equal(buildMetricModel(table, 0, 1, { ...metricSemantics, unit: "INR\u202E" }, metricSummary), null);
});

test("chart model keeps exact categories, series labels, values, and row order", () => {
  const model = buildChartModel(table, {
    chartKind: "LINE",
    categoryColumnIndex: 0,
    seriesColumnIndexes: [1, 2],
    accessibleSummary: chartSummary,
    semantics: metricSemantics,
  });
  assert.ok(model);
  assert.deepEqual(model.categories, ["North", "South"]);
  assert.deepEqual(model.series.map(({ label, sourceValues }) => [label, sourceValues]), [
    ["Revenue", ["1200", "2400"]],
    ["Costs", ["800", "900"]],
  ]);
  assert.ok(model.series.every(series => series.points.length === 2 && series.points.every(point => Number.isFinite(point.x) && Number.isFinite(point.y))));
});

test("chart coordinate model stays finite for extreme, negative, constant, and zero values", () => {
  const extreme = buildChartModel({
    headers: ["Period", "Largest", "Smallest"],
    rows: [["A", String(Number.MAX_VALUE), String(-Number.MAX_VALUE)], ["B", "0", "0"]],
  }, { chartKind: "AREA", categoryColumnIndex: 0, seriesColumnIndexes: [1, 2], accessibleSummary: "Largest and Smallest by Period are INR at unit scale for monthly interval with sum aggregation.", semantics: metricSemantics });
  assert.ok(extreme);
  assert.ok(extreme.series.flatMap(series => series.points).every(point => Number.isFinite(point.x) && Number.isFinite(point.y)));
  assert.ok(extreme.series.flatMap(series => [series.linePath, series.areaPath]).every(path => !/(?:NaN|Infinity)/.test(path)));

  const constant = buildChartModel({ headers: ["Period", "Value"], rows: [["A", "7"], ["B", "7"]] }, {
    chartKind: "BAR", categoryColumnIndex: 0, seriesColumnIndexes: [1], accessibleSummary: "Value by Period is INR at unit scale for monthly interval with sum aggregation.", semantics: metricSemantics,
  });
  assert.ok(constant);
  assert.equal(constant.series[0]!.points[0]!.y, constant.series[0]!.points[1]!.y);
  assert.ok(constant.series[0]!.bars.every(bar => bar.height > 0));
});

test("chart model rejects duplicate, category-overlapping, nonnumeric, or out-of-range series", () => {
  const base = { chartKind: "BAR" as const, categoryColumnIndex: 0, accessibleSummary: chartSummary, semantics: metricSemantics };
  assert.equal(buildChartModel(table, { ...base, seriesColumnIndexes: [1, 1] }), null);
  assert.equal(buildChartModel(table, { ...base, seriesColumnIndexes: [0] }), null);
  assert.equal(buildChartModel(table, { ...base, seriesColumnIndexes: [8] }), null);
  assert.equal(buildChartModel({ headers: ["Region", "Revenue"], rows: [["North", "1"], ["South", "bad"]] }, { ...base, seriesColumnIndexes: [1] }), null);
});
