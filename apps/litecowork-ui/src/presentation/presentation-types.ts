/** Read-only presentation projection. It is never an execution or authority record. */
export type PresentationStatus = "IN_PROGRESS" | "WAITING" | "NEEDS_USER" | "COMPLETE" | "INCOMPLETE" | "FAILED" | "UNAVAILABLE" | "UNKNOWN";
export type PresentationFreshness = "CURRENT" | "STALE" | "UNKNOWN";
export type PresentationKind = "TEXT" | "ACTIVITY" | "TASK_CARD" | "USER_REQUEST" | "APPROVAL" | "ARTIFACT" | "CITATION" | "CODE" | "DIFF" | "TABLE" | "CHART" | "IMAGE" | "BROWSER" | "TERMINAL" | "MCP_APP" | "ERROR" | "UNSUPPORTED";

export type PresentationSourceRef = {
  kind: "MESSAGE" | "TASK" | "STEP" | "RESOURCE_REVISION" | "ARTIFACT_VERSION" | "USER_REQUEST" | "APPROVAL" | "INVOCATION";
  id: string;
  revision?: string;
};

type Base<K extends PresentationKind, P> = {
  item_key: string;
  kind: K;
  payload_version: number;
  source_refs: PresentationSourceRef[];
  occurred_at?: string;
  order_key: string;
  status?: PresentationStatus;
  label?: string;
  freshness: PresentationFreshness;
  payload: P;
};

export type PresentationItem =
  | Base<"TEXT", { text: string }>
  | Base<"ACTIVITY", { summary: string; detail?: string }>
  | Base<"TASK_CARD", { task_id: string; objective: string; status: string; step_summary?: string }>
  | Base<"USER_REQUEST", { title: string; message: string; request_id: string }>
  | Base<"APPROVAL", { title: string; summary: string; approval_id: string }>
  | Base<"ARTIFACT", { artifact_id: string; version: number; display_name: string; artifact_kind: string; verification_status?: string }>
  | Base<"CITATION", { text: string; source_label: string }>
  | Base<"CODE", { text: string; language?: string }>
  | Base<"DIFF", { text: string; file_count: number }>
  | Base<"TABLE", { columns: string[]; rows: string[][] }>
  | Base<"CHART", { title: string; alt_text: string; values: number[]; labels: string[] }>
  | Base<"IMAGE", { alt_text: string; media_type: string; content_ref: string }>
  | Base<"BROWSER", { title: string; state: string }>
  | Base<"TERMINAL", { title: string; text: string }>
  | Base<"MCP_APP", { app_ref: string; display_name: string }>
  | Base<"ERROR", { code: string; message: string; recovery_hint?: string }>
  | Base<"UNSUPPORTED", { original_kind: string }>;

const MAX_ITEMS_TEXT = 128 * 1024;
const MAX_TABLE_ROWS = 500;
const MAX_TABLE_COLUMNS = 32;

function object(value: unknown): Record<string, unknown> | null {
  return value !== null && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : null;
}
function boundedText(value: unknown, maximum = MAX_ITEMS_TEXT): value is string {
  return typeof value === "string" && value.length <= maximum && !value.includes("\0");
}
function status(value: unknown): value is PresentationStatus {
  return ["IN_PROGRESS", "WAITING", "NEEDS_USER", "COMPLETE", "INCOMPLETE", "FAILED", "UNAVAILABLE", "UNKNOWN"].includes(String(value));
}
function sourceRef(value: unknown): value is PresentationSourceRef {
  const ref = object(value);
  return !!ref && ["MESSAGE", "TASK", "STEP", "RESOURCE_REVISION", "ARTIFACT_VERSION", "USER_REQUEST", "APPROVAL", "INVOCATION"].includes(String(ref.kind))
    && boundedText(ref.id, 256) && ref.id.trim().length > 0
    && (ref.revision === undefined || boundedText(ref.revision, 128));
}

/** Strictly validates untrusted projection JSON before it reaches a renderer. */
export function parsePresentationItem(value: unknown): PresentationItem | null {
  const item = object(value);
  if (!item || !boundedText(item.item_key, 256) || !item.item_key.trim() || !boundedText(item.order_key, 256)
    || !Number.isSafeInteger(item.payload_version) || (item.payload_version as number) < 1
    || !Array.isArray(item.source_refs) || item.source_refs.length > 64 || !item.source_refs.every(sourceRef)
    || !["CURRENT", "STALE", "UNKNOWN"].includes(String(item.freshness))
    || (item.status !== undefined && !status(item.status))
    || (item.label !== undefined && !boundedText(item.label, 512))
    || (item.occurred_at !== undefined && !boundedText(item.occurred_at, 64))) return null;
  const payload = object(item.payload);
  if (!payload) return null;
  let encodedLength: number;
  try {
    const encoded = JSON.stringify(item);
    if (typeof encoded !== "string") return null;
    encodedLength = encoded.length;
  } catch { return null; }
  if (encodedLength > 512 * 1024) return null;
  const text = (key: string, max = MAX_ITEMS_TEXT) => boundedText(payload[key], max);
  const kind = item.kind;
  if (item.payload_version !== 1) {
    if (!boundedText(kind, 128)) return null;
    return { ...item, kind: "UNSUPPORTED", payload_version: 1, label: item.label ?? `Unsupported content: ${kind} (version ${item.payload_version})`, payload: { original_kind: `${kind}@${item.payload_version}` } } as unknown as PresentationItem;
  }
  const knownKinds: PresentationKind[] = ["TEXT", "ACTIVITY", "TASK_CARD", "USER_REQUEST", "APPROVAL", "ARTIFACT", "CITATION", "CODE", "DIFF", "TABLE", "CHART", "IMAGE", "BROWSER", "TERMINAL", "MCP_APP", "ERROR"];
  if (!knownKinds.includes(kind as PresentationKind)) {
    if (!boundedText(kind, 128)) return null;
    return { ...item, kind: "UNSUPPORTED", payload_version: 1, label: item.label ?? `Unsupported content: ${kind}`, payload: { original_kind: kind } } as unknown as PresentationItem;
  }
  switch (kind) {
    case "TEXT": if (!text("text")) return null; break;
    case "ACTIVITY": if (!text("summary", 2048) || (payload.detail !== undefined && !text("detail", 8192))) return null; break;
    case "TASK_CARD": if (!text("task_id", 256) || !text("objective", 8192) || !text("status", 64) || (payload.step_summary !== undefined && !text("step_summary", 2048))) return null; break;
    case "USER_REQUEST": if (!text("title", 512) || !text("message", 8192) || !text("request_id", 256)) return null; break;
    case "APPROVAL": if (!text("title", 512) || !text("summary", 8192) || !text("approval_id", 256)) return null; break;
    case "ARTIFACT": if (!text("artifact_id", 256) || !Number.isSafeInteger(payload.version) || (payload.version as number) < 1 || !text("display_name", 512) || !text("artifact_kind", 128) || (payload.verification_status !== undefined && !text("verification_status", 64))) return null; break;
    case "CITATION": if (!text("text", 8192) || !text("source_label", 512)) return null; break;
    case "CODE": if (!text("text") || (payload.language !== undefined && !text("language", 64))) return null; break;
    case "DIFF": if (!text("text") || !Number.isSafeInteger(payload.file_count) || (payload.file_count as number) < 0 || (payload.file_count as number) > 10_000) return null; break;
    case "TABLE": {
      const columns = payload.columns;
      const rows = payload.rows;
      if (!Array.isArray(columns) || columns.length > MAX_TABLE_COLUMNS || !columns.every(value => boundedText(value, 256))
        || !Array.isArray(rows) || rows.length > MAX_TABLE_ROWS
        || !rows.every(row => Array.isArray(row) && row.length === columns.length && row.every(value => boundedText(value, 8192)))) return null;
      break;
    }
    case "CHART": if (!text("title", 512) || !text("alt_text", 2048) || !Array.isArray(payload.values) || payload.values.length > 10_000 || !payload.values.every(value => typeof value === "number" && Number.isFinite(value)) || !Array.isArray(payload.labels) || payload.labels.length !== payload.values.length || !payload.labels.every(value => boundedText(value, 256))) return null; break;
    case "IMAGE": if (!text("alt_text", 2048) || !text("media_type", 128) || !text("content_ref", 512)) return null; break;
    case "BROWSER": if (!text("title", 512) || !text("state", 128)) return null; break;
    case "TERMINAL": if (!text("title", 512) || !text("text")) return null; break;
    case "MCP_APP": if (!text("app_ref", 512) || !text("display_name", 256)) return null; break;
    case "ERROR": if (!text("code", 128) || !text("message", 8192) || (payload.recovery_hint !== undefined && !text("recovery_hint", 2048))) return null; break;
    default: return null;
  }
  // Only version 1 reaches kind-specific renderers. Unknown kinds and versions are
  // converted above to a bounded Unsupported item so forward-compatible content stays
  // visible without interpreting an unknown payload schema.
  return item as unknown as PresentationItem;
}

/** Strict ordering for callers that must reject invalid projections outright. The renderer
 * does not use this: silently dropping a saved item loses transcript content, so
 * `projectPresentationEntries` keeps a visible placeholder instead. */
export function orderPresentationItems(items: readonly unknown[]): PresentationItem[] {
  return items.map(parsePresentationItem).filter((item): item is PresentationItem => item !== null)
    .sort((a, b) => a.order_key.localeCompare(b.order_key) || a.item_key.localeCompare(b.item_key));
}

/** A saved projection value that failed validation. Only bounded, individually revalidated
 * identity metadata is retained; its payload is never interpreted or displayed. */
export type RejectedPresentationItem = {
  readonly reportedKind: string | null;
  readonly reportedVersion: number | null;
  readonly sourceRefs: PresentationSourceRef[];
};

/** Renderer input: a validated item, or a visible placeholder for one that could not be
 * validated. No saved item silently disappears from the transcript (UIV1-054). */
export type PresentationEntry =
  | { readonly entry: "ITEM"; readonly key: string; readonly orderKey: string; readonly item: PresentationItem }
  | { readonly entry: "REJECTED"; readonly key: string; readonly orderKey: string; readonly rejected: RejectedPresentationItem };

export type PresentationProjection = {
  readonly entries: readonly PresentationEntry[];
  readonly rejectedCount: number;
  readonly omittedCount: number;
};

/** More than this is a faulty projection, not a richer response. */
export const MAX_DISPLAYED_PRESENTATION_ITEMS = 300;
const DISPLAY_KIND = /^[A-Za-z0-9][A-Za-z0-9_.:@ /-]*$/;

function displayKind(value: unknown): string | null {
  if (!boundedText(value, 128)) return null;
  const normalized = value.replace(/\s+/g, " ").trim();
  return normalized.length > 0 && DISPLAY_KIND.test(normalized) ? normalized : null;
}

function displayVersion(value: unknown): number | null {
  return Number.isSafeInteger(value) && (value as number) >= 1 && (value as number) <= 1_000_000 ? value as number : null;
}

function salvagedSourceRefs(value: unknown): PresentationSourceRef[] {
  const refs = object(value)?.source_refs;
  return Array.isArray(refs) ? refs.slice(0, 64).filter(sourceRef) : [];
}

/**
 * Project untrusted projection values for rendering. Valid values keep their typed payload;
 * every invalid value becomes a bounded placeholder that states it could not be displayed,
 * so the surface is honest about partial or unsupported content instead of hiding it.
 */
export function projectPresentationEntries(
  items: readonly unknown[],
  limit = MAX_DISPLAYED_PRESENTATION_ITEMS,
): PresentationProjection {
  const maximum = Number.isSafeInteger(limit)
    ? Math.min(Math.max(1, limit), MAX_DISPLAYED_PRESENTATION_ITEMS)
    : MAX_DISPLAYED_PRESENTATION_ITEMS;
  const shown = items.slice(0, maximum);
  let rejectedCount = 0;
  const entries = shown.map((value, index): PresentationEntry => {
    const parsed = parsePresentationItem(value);
    if (parsed) {
      return { entry: "ITEM", key: `item:${parsed.item_key}`, orderKey: parsed.order_key, item: parsed };
    }
    rejectedCount += 1;
    const record = object(value);
    const orderKey = record && boundedText(record.order_key, 256) ? record.order_key : "";
    const key = `rejected:${index}`;
    return {
      entry: "REJECTED",
      key,
      orderKey,
      rejected: {
        reportedKind: record ? displayKind(record.kind) : null,
        reportedVersion: record ? displayVersion(record.payload_version) : null,
        sourceRefs: salvagedSourceRefs(value),
      },
    };
  });
  entries.sort((a, b) => a.orderKey.localeCompare(b.orderKey) || a.key.localeCompare(b.key));
  return { entries, rejectedCount, omittedCount: Math.max(0, items.length - shown.length) };
}
