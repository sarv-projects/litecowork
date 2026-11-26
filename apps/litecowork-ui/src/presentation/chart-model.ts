export type SemanticTableModel = {
  readonly headers: readonly string[];
  readonly rows: readonly (readonly string[])[];
};

export type MetricScale = "UNIT" | "THOUSAND" | "MILLION" | "BILLION" | "LAKH" | "CRORE";
export type MetricAggregation = "NONE" | "SUM" | "AVERAGE" | "COUNT" | "MINIMUM" | "MAXIMUM" | "LAST";
export type MetricSemantics = {
  readonly unit: string;
  readonly scale: MetricScale;
  readonly timeInterval: string;
  readonly aggregation: MetricAggregation;
};

export type MetricModel = {
  readonly label: string;
  readonly sourceValue: string;
  readonly valueText: string;
  readonly accessibleSummary: string;
  readonly semantics: MetricSemantics;
  readonly sourceTable: SemanticTableModel;
};

export type ChartKind = "BAR" | "LINE" | "AREA";
export type ChartPoint = { readonly x: number; readonly y: number };
export type ChartBar = { readonly x: number; readonly y: number; readonly width: number; readonly height: number };
export type ChartSeriesModel = {
  readonly label: string;
  readonly sourceValues: readonly string[];
  readonly points: readonly ChartPoint[];
  readonly bars: readonly ChartBar[];
  readonly linePath: string;
  readonly areaPath: string;
};
export type ChartTick = { readonly y: number; readonly label: string };
export type ChartModel = {
  readonly chartKind: ChartKind;
  readonly accessibleSummary: string;
  readonly semantics: MetricSemantics;
  readonly categories: readonly string[];
  readonly series: readonly ChartSeriesModel[];
  readonly width: number;
  readonly height: number;
  readonly zeroY: number;
  readonly ticks: readonly ChartTick[];
  readonly categoryLabels: readonly { readonly index: number; readonly x: number; readonly label: string }[];
  readonly sourceTable: SemanticTableModel;
};

export type ChartModelInput = {
  readonly chartKind: ChartKind;
  readonly categoryColumnIndex: number;
  readonly seriesColumnIndexes: readonly number[];
  readonly accessibleSummary: string;
  readonly semantics: unknown;
};

const WIDTH = 720;
const HEIGHT = 300;
const PLOT = { left: 64, right: 16, top: 16, bottom: 48 } as const;
const NUMBER = /^[+-]?(?:(?:\d+\.?\d*)|(?:\.\d+))(?:[eE][+-]?\d+)?$/;
const INVALID_CONTEXT_TEXT = /[\u0000-\u001f\u007f-\u009f\u202a-\u202e\u2066-\u2069]/u;
const SCALE_LABELS: Record<MetricScale, string> = {
  UNIT: "unit scale",
  THOUSAND: "thousand scale",
  MILLION: "million scale",
  BILLION: "billion scale",
  LAKH: "lakh scale",
  CRORE: "crore scale",
};
const AGGREGATION_LABELS: Record<MetricAggregation, string> = {
  NONE: "no aggregation",
  SUM: "sum aggregation",
  AVERAGE: "average aggregation",
  COUNT: "count aggregation",
  MINIMUM: "minimum aggregation",
  MAXIMUM: "maximum aggregation",
  LAST: "last-value aggregation",
};

function safeContextText(value: unknown): value is string {
  return typeof value === "string" && value.length >= 1 && Array.from(value).length <= 80 && value.trim() === value
    && !INVALID_CONTEXT_TEXT.test(value) && isWellFormedString(value);
}

function safeSummaryText(value: unknown): value is string {
  return typeof value === "string" && value.length <= 2_000 && value.trim().length > 0
    && !INVALID_CONTEXT_TEXT.test(value) && isWellFormedString(value);
}

function isWellFormedString(value: string): boolean {
  for (let index = 0; index < value.length; index += 1) {
    const unit = value.charCodeAt(index);
    if (unit >= 0xd800 && unit <= 0xdbff) {
      const next = value.charCodeAt(index + 1);
      if (next < 0xdc00 || next > 0xdfff) return false;
      index += 1;
    } else if (unit >= 0xdc00 && unit <= 0xdfff) return false;
  }
  return true;
}

function containsSemanticLabel(summary: string, label: string): boolean {
  if (!label) return false;
  const firstIsWord = /[\p{L}\p{N}]/u.test(Array.from(label)[0]!);
  const lastIsWord = /[\p{L}\p{N}]/u.test(Array.from(label).at(-1)!);
  let start = summary.indexOf(label);
  while (start >= 0) {
    const before = Array.from(summary.slice(0, start)).at(-1);
    const after = Array.from(summary.slice(start + label.length))[0];
    const leftBoundary = !firstIsWord || before === undefined || !/[\p{L}\p{N}]/u.test(before);
    const rightBoundary = !lastIsWord || after === undefined || !/[\p{L}\p{N}]/u.test(after);
    if (leftBoundary && rightBoundary) return true;
    start = summary.indexOf(label, start + 1);
  }
  return false;
}

/** Context labels may describe the committed answer but never transform its source values. */
export function validateMetricSemantics(
  value: unknown,
  accessibleSummary: string,
  sourceLabels: readonly string[],
): MetricSemantics | null {
  if (value === null || typeof value !== "object" || Array.isArray(value) || !safeSummaryText(accessibleSummary)) return null;
  const context = value as Record<string, unknown>;
  const keys = Object.keys(context);
  if (keys.length !== 4 || !["unit", "scale", "time_interval", "aggregation"].every(key => keys.includes(key))) return null;
  if (!safeContextText(context.unit) || !safeContextText(context.time_interval)) return null;
  const scale = context.scale;
  const aggregation = context.aggregation;
  if (typeof scale !== "string" || !Object.hasOwn(SCALE_LABELS, scale)
    || typeof aggregation !== "string" || !Object.hasOwn(AGGREGATION_LABELS, aggregation)) return null;
  const semantics: MetricSemantics = {
    unit: context.unit,
    scale: scale as MetricScale,
    timeInterval: context.time_interval,
    aggregation: aggregation as MetricAggregation,
  };
  const summaryLower = accessibleSummary.toLowerCase();
  if (!containsSemanticLabel(accessibleSummary, semantics.unit) || !containsSemanticLabel(accessibleSummary, semantics.timeInterval)
    || !summaryLower.includes(SCALE_LABELS[semantics.scale])
    || !summaryLower.includes(AGGREGATION_LABELS[semantics.aggregation])
    || sourceLabels.some(label => typeof label !== "string" || !containsSemanticLabel(accessibleSummary, label))) return null;
  return semantics;
}

export function metricScaleLabel(scale: MetricScale): string {
  return SCALE_LABELS[scale];
}

export function metricAggregationLabel(aggregation: MetricAggregation): string {
  return AGGREGATION_LABELS[aggregation];
}

function numericCell(value: string): number | null {
  const candidate = value.trim();
  if (!NUMBER.test(candidate)) return null;
  const numeric = Number(candidate);
  return Number.isFinite(numeric) ? numeric : null;
}

function validTable(table: SemanticTableModel): boolean {
  return Array.isArray(table.headers) && table.headers.length > 0 && table.headers.length <= 20
    && table.headers.every(header => typeof header === "string" && header.length <= 8192)
    && Array.isArray(table.rows) && table.rows.length > 0 && table.rows.length <= 500
    && table.rows.every(row => Array.isArray(row) && row.length === table.headers.length
      && row.every(cell => typeof cell === "string" && cell.length <= 8192));
}

function grouped(value: number, locale?: string): string | null {
  try {
    return new Intl.NumberFormat(locale, { useGrouping: true, maximumFractionDigits: 20 }).format(value);
  } catch {
    return null;
  }
}

export function buildMetricModel(
  table: SemanticTableModel,
  rowIndex: number,
  columnIndex: number,
  semanticsInput: unknown,
  accessibleSummary: string,
  locale?: string,
): MetricModel | null {
  if (!validTable(table) || !Number.isSafeInteger(rowIndex) || rowIndex < 0 || rowIndex >= table.rows.length
    || !Number.isSafeInteger(columnIndex) || columnIndex < 0 || columnIndex >= table.headers.length) return null;
  const header = table.headers[columnIndex]!.trim();
  const sourceValue = table.rows[rowIndex]![columnIndex]!.trim();
  const numeric = numericCell(sourceValue);
  if (!header || numeric === null) return null;
  let rowLabel: string;
  if (table.headers.length === 1) {
    rowLabel = `Row ${rowIndex + 1}`;
  } else {
    if (columnIndex === 0) return null;
    rowLabel = table.rows[rowIndex]![0]!.trim();
    if (!rowLabel) return null;
  }
  const semantics = validateMetricSemantics(semanticsInput, accessibleSummary, table.headers.length === 1 ? [header] : [header, rowLabel]);
  if (!semantics) return null;
  const valueText = grouped(numeric, locale);
  if (valueText === null) return null;
  return { label: `${header} — ${rowLabel}`, sourceValue, valueText, accessibleSummary, semantics, sourceTable: table };
}

function coordinate(value: number): string {
  return Number(value.toFixed(3)).toString();
}

function axisValue(value: number, locale?: string): string | null {
  if (Math.abs(value) >= 1e12 || value !== 0 && Math.abs(value) < 1e-6) return value.toExponential(2);
  try {
    return new Intl.NumberFormat(locale, { useGrouping: true, maximumFractionDigits: 3 }).format(value);
  } catch {
    return null;
  }
}

export function buildChartModel(
  table: SemanticTableModel,
  input: ChartModelInput,
  locale?: string,
): ChartModel | null {
  if (!validTable(table) || table.rows.length < 2 || table.rows.length > 60
    || !(input.chartKind === "BAR" || input.chartKind === "LINE" || input.chartKind === "AREA")
    || typeof input.accessibleSummary !== "string" || input.accessibleSummary.length > 2_000
    || !input.accessibleSummary.trim() || !Number.isSafeInteger(input.categoryColumnIndex)
    || input.categoryColumnIndex < 0 || input.categoryColumnIndex >= table.headers.length
    || !Array.isArray(input.seriesColumnIndexes) || input.seriesColumnIndexes.length < 1 || input.seriesColumnIndexes.length > 6) return null;
  const categoryHeader = table.headers[input.categoryColumnIndex]!.trim();
  if (!categoryHeader) return null;
  const categories = table.rows.map(row => row[input.categoryColumnIndex]!.trim());
  if (categories.some(category => !category)) return null;

  const used = new Set<number>([input.categoryColumnIndex]);
  const parsedSeries: Array<{ label: string; sourceValues: string[]; values: number[] }> = [];
  for (const columnIndex of input.seriesColumnIndexes) {
    if (!Number.isSafeInteger(columnIndex) || columnIndex < 0 || columnIndex >= table.headers.length || used.has(columnIndex)) return null;
    used.add(columnIndex);
    const label = table.headers[columnIndex]!.trim();
    if (!label) return null;
    const sourceValues = table.rows.map(row => row[columnIndex]!.trim());
    const values = sourceValues.map(numericCell);
    if (values.some(value => value === null)) return null;
    parsedSeries.push({ label, sourceValues, values: values as number[] });
  }
  const semantics = validateMetricSemantics(input.semantics, input.accessibleSummary, [categoryHeader, ...parsedSeries.map(series => series.label)]);
  if (!semantics) return null;

  const allValues = parsedSeries.flatMap(series => series.values);
  const maxMagnitude = Math.max(...allValues.map(value => Math.abs(value)));
  const normalized = allValues.map(value => maxMagnitude === 0 ? 0 : value / maxMagnitude);
  const normalizedMin = Math.min(0, ...normalized);
  const normalizedMax = Math.max(0, ...normalized);
  const normalizedSpan = normalizedMax - normalizedMin;
  const plotWidth = WIDTH - PLOT.left - PLOT.right;
  const plotHeight = HEIGHT - PLOT.top - PLOT.bottom;
  const yFor = (value: number): number => normalizedSpan === 0
    ? PLOT.top + plotHeight / 2
    : PLOT.top + ((normalizedMax - (maxMagnitude === 0 ? 0 : value / maxMagnitude)) / normalizedSpan) * plotHeight;
  const zeroY = yFor(0);
  const categoryStep = plotWidth / categories.length;
  const groupWidth = categoryStep * 0.78;
  const slotWidth = groupWidth / parsedSeries.length;
  const barWidth = Math.max(0.25, Math.min(slotWidth * 0.82, slotWidth - 0.25));

  const series: ChartSeriesModel[] = parsedSeries.map((source, seriesIndex) => {
    const points = source.values.map((value, index) => ({
      x: PLOT.left + categoryStep * (index + 0.5),
      y: yFor(value),
    }));
    const bars = points.map((point, index) => {
      const x = PLOT.left + categoryStep * (index + 0.5) - groupWidth / 2 + slotWidth * seriesIndex + (slotWidth - barWidth) / 2;
      return { x, y: Math.min(point.y, zeroY), width: barWidth, height: Math.abs(zeroY - point.y) };
    });
    const linePath = points.map((point, index) => `${index === 0 ? "M" : "L"}${coordinate(point.x)} ${coordinate(point.y)}`).join(" ");
    const first = points[0]!;
    const last = points.at(-1)!;
    const areaPath = `${linePath} L${coordinate(last.x)} ${coordinate(zeroY)} L${coordinate(first.x)} ${coordinate(zeroY)} Z`;
    return { label: source.label, sourceValues: source.sourceValues, points, bars, linePath, areaPath };
  });

  const ticks: ChartTick[] = Array.from({ length: 5 }, (_, index) => {
    const amount = index / 4;
    const normalizedValue = normalizedMax - normalizedSpan * amount;
    const value = maxMagnitude === 0 ? 0 : normalizedValue * maxMagnitude;
    const label = axisValue(value, locale);
    return label === null ? null : { y: PLOT.top + plotHeight * amount, label };
  }).filter((tick): tick is ChartTick => tick !== null);
  if (ticks.length !== 5 || series.some(item => item.points.some(point => !Number.isFinite(point.x) || !Number.isFinite(point.y)))) return null;

  const labelIndexes = new Set<number>();
  const maxLabels = Math.min(7, categories.length);
  for (let index = 0; index < maxLabels; index += 1) labelIndexes.add(Math.round(index * (categories.length - 1) / (maxLabels - 1)));
  const categoryLabels = [...labelIndexes].sort((left, right) => left - right).map(index => ({
    index,
    x: PLOT.left + categoryStep * (index + 0.5),
    label: categories[index]!,
  }));

  return {
    chartKind: input.chartKind,
    accessibleSummary: input.accessibleSummary,
    semantics,
    categories,
    series,
    width: WIDTH,
    height: HEIGHT,
    zeroY,
    ticks,
    categoryLabels,
    sourceTable: table,
  };
}
