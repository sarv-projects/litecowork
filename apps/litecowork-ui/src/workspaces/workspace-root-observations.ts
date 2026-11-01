// Closed wire decoders and bounded paging helpers for the observed-file projection.
//
// The projection is a rebuildable, Runtime-local metadata view beneath one selected
// WorkspaceRoot. These helpers only validate what the Operator returned; they never
// invent a Resource, a content revision, or a deletion inference.

export type RootObservationFreshness = "CURRENT" | "STALE" | "UNKNOWN";

export type WorkspaceRootObservation = {
  observation_id: string;
  display_name: string;
  size_bytes: number;
  modified_at: string | null;
  observed_at: string;
  freshness: RootObservationFreshness;
  resource_id: string | null;
};

export type WorkspaceRootObservationPage = {
  workspace_id: string;
  workspace_root_id: string;
  items: WorkspaceRootObservation[];
  next_cursor: string | null;
};

export type WorkspaceRootObservationRequest = {
  workspaceId: string;
  workspaceRootId: string;
  rootVersion: number;
  query: string;
  freshness: RootObservationFreshness | "";
  cursor: string | null;
  limit: number;
};

export const MAX_OBSERVATION_QUERY_BYTES = 256;
export const MAX_OBSERVATION_CURSOR_BYTES = 4096;
export const MAX_OBSERVATION_DISPLAY_NAME_BYTES = 1024;
export const OBSERVATION_PAGE_LIMIT = 50;

const OBSERVATION_ID = /^sha256:[0-9a-f]{64}$/u;
const RESOURCE_ID = /^res_file_[0-9a-f]{64}$/u;
const FRESHNESS = new Set<RootObservationFreshness>(["CURRENT", "STALE", "UNKNOWN"]);

function isTimestamp(value: unknown): value is string {
  return typeof value === "string" && Number.isFinite(Date.parse(value));
}

function exactKeys(value: Record<string, unknown>, expected: readonly string[]): boolean {
  const keys = Object.keys(value);
  return keys.length === expected.length && expected.every((key) => Object.hasOwn(value, key));
}

export function utf8ByteLength(value: string): number {
  return new TextEncoder().encode(value).length;
}

/** The locally normalized search text the Operator accepts: bounded UTF-8, no NUL. */
export function normalizeObservationQuery(value: string): string | null {
  const query = value.trim();
  if (query.includes("\0") || utf8ByteLength(query) > MAX_OBSERVATION_QUERY_BYTES) return null;
  return query;
}

function isObservationItem(value: unknown): value is WorkspaceRootObservation {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const item = value as Record<string, unknown>;
  if (!exactKeys(item, [
    "observation_id",
    "display_name",
    "size_bytes",
    "modified_at",
    "observed_at",
    "freshness",
    "resource_id",
  ])) return false;
  return typeof item.observation_id === "string" && OBSERVATION_ID.test(item.observation_id)
    && typeof item.display_name === "string"
    && item.display_name.length > 0
    && !item.display_name.includes("\0")
    && utf8ByteLength(item.display_name) <= MAX_OBSERVATION_DISPLAY_NAME_BYTES
    && typeof item.size_bytes === "number"
    && Number.isSafeInteger(item.size_bytes)
    && item.size_bytes >= 0
    && (item.modified_at === null || isTimestamp(item.modified_at))
    && isTimestamp(item.observed_at)
    && typeof item.freshness === "string"
    && FRESHNESS.has(item.freshness as RootObservationFreshness)
    && (item.resource_id === null
      || (typeof item.resource_id === "string" && RESOURCE_ID.test(item.resource_id)));
}

/**
 * Accept only a complete page for the exact selected root, with a strictly advancing
 * cursor and no duplicate observation identity. `previousCursor` is the cursor that
 * produced this page, so a repeated or empty continuation cursor is rejected instead of
 * powering an unbounded "Load more" loop.
 *
 * A continuation may legitimately be empty: the last observed file can be renamed or
 * removed between pages, and the Operator then reports no further rows and no cursor.
 * That is the end of the list, not a malformed page. A page that advertises a
 * continuation cursor must still carry at least one row, because the Operator only
 * issues a cursor when another matching observation exists.
 */
export function isWorkspaceRootObservationPage(
  value: unknown,
  workspaceId: string,
  workspaceRootId: string,
  previousCursor: string | null = null,
): value is WorkspaceRootObservationPage {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const page = value as Record<string, unknown>;
  if (!exactKeys(page, ["workspace_id", "workspace_root_id", "items", "next_cursor"])) return false;
  if (page.workspace_id !== workspaceId || page.workspace_root_id !== workspaceRootId) return false;
  const nextCursor = page.next_cursor;
  if (!(nextCursor === null
    || (typeof nextCursor === "string"
      && nextCursor.length > 0
      && utf8ByteLength(nextCursor) <= MAX_OBSERVATION_CURSOR_BYTES
      && nextCursor !== previousCursor))) return false;
  if (!Array.isArray(page.items) || !page.items.every(isObservationItem)) return false;
  if (nextCursor !== null && page.items.length === 0) return false;
  return new Set((page.items as WorkspaceRootObservation[]).map((item) => item.observation_id)).size
    === page.items.length;
}

/**
 * Apply a validated page by stable observation identity. A continuation appends unseen
 * rows only, so a stale cursor cannot duplicate list keys or rewrite a row that the owner
 * is currently inspecting. A first page replaces the list.
 */
export function mergeWorkspaceRootObservationPage(
  items: WorkspaceRootObservation[],
  page: WorkspaceRootObservationPage,
  replace = false,
): { items: WorkspaceRootObservation[]; nextCursor: string | null } {
  const merged = replace ? [] : [...items];
  const seen = new Set(merged.map((item) => item.observation_id));
  for (const item of page.items) {
    if (seen.has(item.observation_id)) continue;
    seen.add(item.observation_id);
    merged.push(item);
  }
  return { items: merged, nextCursor: page.next_cursor };
}
