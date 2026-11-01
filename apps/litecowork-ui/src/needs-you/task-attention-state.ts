export type TaskAttentionLoadState = {
  generation: number;
  loadingMore: boolean;
};

export type TaskAttentionLoadAction =
  | { type: "invalidate"; generation: number }
  | { type: "start-more"; generation: number }
  | { type: "finish-more"; generation: number };

export type NeedsYouStatus = "OPEN" | "RESOLVED";
export type NeedsYouKind = "APPROVAL" | "USER_REQUEST" | "TASK_BLOCKER";
export type NeedsYouItemView = {
  itemId: string;
  kind: NeedsYouKind;
  status: NeedsYouStatus;
  approvalId: string | null;
  userRequestId: string | null;
  taskId: string | null;
  blockerId: string | null;
  title: string;
  safeSummary: string;
  createdAt: string;
  resolvedAt: string | null;
};
export type NeedsYouPage = { items: NeedsYouItemView[]; nextCursor: string | null };

function isNonEmptyString(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0;
}

function isNullableString(value: unknown): value is string | null {
  return value === null || isNonEmptyString(value);
}

function isValidNeedsYouItem(value: unknown, status: NeedsYouStatus): value is NeedsYouItemView {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const item = value as Record<string, unknown>;
  const validKind = item.kind === "APPROVAL" || item.kind === "USER_REQUEST" || item.kind === "TASK_BLOCKER";
  const validStatus = item.status === "OPEN" || item.status === "RESOLVED";
  const createdAt = typeof item.createdAt === "string" ? Date.parse(item.createdAt) : Number.NaN;
  const resolvedAt = item.resolvedAt === null
    ? null
    : typeof item.resolvedAt === "string" ? Date.parse(item.resolvedAt) : Number.NaN;
  return isNonEmptyString(item.itemId)
    && validKind
    && validStatus
    && item.status === status
    && isNullableString(item.approvalId)
    && isNullableString(item.userRequestId)
    && isNullableString(item.taskId)
    && isNullableString(item.blockerId)
    && isNonEmptyString(item.title)
    && isNonEmptyString(item.safeSummary)
    && Number.isFinite(createdAt)
    && (resolvedAt === null || Number.isFinite(resolvedAt));
}

/** Accept only a complete page for the selected status and a strictly advancing cursor. */
export function isValidNeedsYouPage(status: NeedsYouStatus, page: unknown, previousCursor: string | null): page is NeedsYouPage {
  if (!page || typeof page !== "object" || Array.isArray(page)) return false;
  const candidate = page as Record<string, unknown>;
  if (!Array.isArray(candidate.items)
    || !(candidate.nextCursor === null
      || (isNonEmptyString(candidate.nextCursor) && candidate.nextCursor !== previousCursor))
    || !candidate.items.every(item => isValidNeedsYouItem(item, status))) return false;
  return new Set(candidate.items.map(item => (item as NeedsYouItemView).itemId)).size === candidate.items.length;
}

/** Append a cursor page by stable inbox identity without allowing duplicates to rewrite saved rows. */
export function mergeNeedsYouPage(
  items: NeedsYouItemView[],
  page: NeedsYouPage,
): { items: NeedsYouItemView[]; nextCursor: string | null } {
  const merged = [...items];
  const seen = new Set(items.map(item => item.itemId));
  for (const item of page.items) {
    if (seen.has(item.itemId)) continue;
    seen.add(item.itemId);
    merged.push(item);
  }
  return { items: merged, nextCursor: page.nextCursor };
}

/** Keeps the pagination busy state scoped to the request generation that owns it. */
export function taskAttentionLoadReducer(
  state: TaskAttentionLoadState,
  action: TaskAttentionLoadAction,
): TaskAttentionLoadState {
  switch (action.type) {
    case "invalidate":
      return { generation: action.generation, loadingMore: false };
    case "start-more":
      return state.generation === action.generation ? { ...state, loadingMore: true } : state;
    case "finish-more":
      return state.generation === action.generation ? { ...state, loadingMore: false } : state;
  }
}

/**
 * Motion baseline for the Needs You inbox.
 *
 * The contract allows a newly appearing approval or blocker card to use one 280ms border
 * emphasis, and only after the committed projection already contains it. Historical
 * hydration, a cursor resync, and the first page load render the current state without
 * replaying that transition, so the baseline is scoped: when the scope changes the
 * previously rendered identities are dropped and nothing is emphasised.
 */
export type NeedsYouMotionBaseline = {
  scopeKey: string | null;
  itemIds: ReadonlySet<string>;
};

export const EMPTY_NEEDS_YOU_MOTION_BASELINE: NeedsYouMotionBaseline = {
  scopeKey: null,
  itemIds: new Set<string>(),
};

export function nextNeedsYouMotionBaseline(
  baseline: NeedsYouMotionBaseline,
  scopeKey: string,
  itemIds: readonly string[],
): { baseline: NeedsYouMotionBaseline; entering: string[] } {
  const next = new Set(itemIds);
  if (baseline.scopeKey !== scopeKey) {
    // A new scope (Workspace or status filter) is a fresh projection, not new work.
    return { baseline: { scopeKey, itemIds: next }, entering: [] };
  }
  const entering = itemIds.filter((itemId) => !baseline.itemIds.has(itemId));
  for (const itemId of baseline.itemIds) next.add(itemId);
  return { baseline: { scopeKey, itemIds: next }, entering };
}
