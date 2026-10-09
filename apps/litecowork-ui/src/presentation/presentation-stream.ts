import { orderPresentationItems, type PresentationItem } from "./presentation-types.ts";

/** Transport frames are untrusted and only update an ephemeral rendering projection. */
export type PresentationFrame =
  | { type: "stream.ready"; workspace_id: string; projection_version: number; cursor: string; max_frame_bytes: number }
  | { type: "presentation.snapshot"; projection_revision: number; cursor: string; items: unknown[] }
  | { type: "presentation.upsert"; projection_revision: number; cursor: string; item: unknown }
  | { type: "stream.resync_required" }
  | { type: "turn.delta"; workspace_id: string; conversation_id: string; turn_id: string; retry_ordinal: number; sequence: number; text_delta: string }
  | { type: "turn.settled"; conversation_id: string; turn_id: string; retry_ordinal: number; committed_message_id?: string }
  | { type: "rich.draft"; workspace_id: string; conversation_id: string; turn_id: string; retry_ordinal: number; agent_session_id: string; draft_id: string; sequence: number; event: string; payload: unknown };

export type TransientTurn = {
  conversationId: string;
  turnId: string;
  retryOrdinal: number;
  text: string;
  lastSequence: number;
  state: "STREAMING" | "RECONNECTING" | "INCOMPLETE";
};

export type RichDraftBlock = {
  blockId: string;
  parentId?: string;
  index: number;
  kind: string;
  text: string;
  closed: boolean;
  replacement?: unknown;
};

export type RichDraft = {
  conversationId: string;
  turnId: string;
  retryOrdinal: number;
  agentSessionId: string;
  draftId: string;
  lastSequence: number;
  status: "STREAMING" | "FINALIZING" | "PUBLISHED" | "FAILED" | "RECONNECTING";
  blocks: RichDraftBlock[];
  bytes: number;
  frameCount: number;
  publishedPresentationId?: string;
};

export type PresentationStreamState = {
  ready: boolean;
  resyncRequired: boolean;
  snapshotReady: boolean;
  workspaceId: string | null;
  projectionVersion: number | null;
  projectionRevision: number;
  cursor: string | null;
  maxFrameBytes: number;
  items: PresentationItem[];
  itemRevisions: Record<string, number>;
  turns: Record<string, TransientTurn>;
  drafts: Record<string, RichDraft>;
  retryFences: Record<string, number>;
  settledRetries: Record<string, number>;
};

export const emptyPresentationStream: PresentationStreamState = {
  ready: false, resyncRequired: false, snapshotReady: false, workspaceId: null, projectionVersion: null,
  projectionRevision: 0, cursor: null, maxFrameBytes: 0, items: [], itemRevisions: {}, turns: {}, drafts: {}, retryFences: {}, settledRetries: {},
};

const MAX_FRAME_BYTES = 1024 * 1024;
const MAX_ACTIVE_DRAFTS = 4;
const MAX_TRACKED_TURN_FENCES = 512;
const MAX_DRAFT_BLOCKS = 200;
const MAX_OPEN_BLOCKS = 32;
const MAX_DRAFT_DEPTH = 8;
const MAX_DRAFT_BYTES = 512 * 1024;
const MAX_DRAFT_FRAMES = 1000;
const MAX_REPLACEMENT_BYTES = 64 * 1024;
const DRAFT_BLOCK_KINDS = new Set([
  "TEXT_SLICE", "LAYOUT", "CARD", "CALLOUT", "TABLE", "CHART", "DIAGRAM", "MEDIA", "CHECKLIST", "DELIVERABLE_GROUP", "ARTIFACT_COLLECTION",
]);

function text(value: unknown, max: number): value is string {
  return typeof value === "string" && value.length > 0 && value.length <= max && !value.includes("\0");
}
function validCursor(value: unknown): value is string { return text(value, 2048); }
function turnKey(conversationId: string, turnId: string, retryOrdinal?: number): string {
  return retryOrdinal === undefined ? JSON.stringify([conversationId, turnId]) : JSON.stringify([conversationId, turnId, retryOrdinal]);
}
function setRetryFence(fences: Record<string, number>, key: string, retryOrdinal: number): Record<string, number> {
  if ((fences[key] ?? -1) >= retryOrdinal) return fences;
  const next = { ...fences };
  delete next[key];
  next[key] = retryOrdinal;
  while (Object.keys(next).length > MAX_TRACKED_TURN_FENCES) delete next[Object.keys(next)[0]!];
  return next;
}
function object(value: unknown): Record<string, unknown> | null {
  return value !== null && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : null;
}
function utf8Length(value: string): number { return new TextEncoder().encode(value).byteLength; }
function draftFrameIdentity(frame: Record<string, unknown>, state: PresentationStreamState): boolean {
  return frame.workspace_id === state.workspaceId
    && text(frame.conversation_id, 256) && text(frame.turn_id, 256)
    && Number.isSafeInteger(frame.retry_ordinal) && (frame.retry_ordinal as number) >= 0
    && text(frame.agent_session_id, 256) && text(frame.draft_id, 256)
    && Number.isSafeInteger(frame.sequence) && (frame.sequence as number) >= 1;
}
function childDepth(blocks: readonly RichDraftBlock[], parentId?: string): number {
  if (!parentId) return 1;
  const parent = blocks.find((block) => block.blockId === parentId);
  return parent ? childDepth(blocks, parent.parentId) + 1 : Number.MAX_SAFE_INTEGER;
}
function failDraft(state: PresentationStreamState, key: string, draft: RichDraft, status: RichDraft["status"]): PresentationStreamState {
  return { ...state, drafts: { ...state.drafts, [key]: { ...draft, status, blocks: [], bytes: 0 } } };
}

/**
 * Apply one decoded protocol frame. The wire decoder must enforce max_frame_bytes;
 * this reducer independently bounds retained projection and transient draft state.
 */
export function reducePresentationFrame(state: PresentationStreamState, frame: unknown): PresentationStreamState {
  if (!frame || typeof frame !== "object" || Array.isArray(frame)) return state;
  const f = frame as Record<string, unknown>;
  if (f.type === "stream.ready") {
    if (!text(f.workspace_id, 256) || !Number.isSafeInteger(f.projection_version) || !validCursor(f.cursor)
      || !Number.isSafeInteger(f.max_frame_bytes) || (f.max_frame_bytes as number) < 1 || (f.max_frame_bytes as number) > MAX_FRAME_BYTES) return state;
    return { ...emptyPresentationStream, ready: true, workspaceId: f.workspace_id, projectionVersion: f.projection_version as number, cursor: f.cursor, maxFrameBytes: f.max_frame_bytes as number };
  }
  if (f.type === "stream.resync_required") return { ...state, ready: false, resyncRequired: true, snapshotReady: false, cursor: null, items: [], itemRevisions: {}, turns: {}, drafts: {}, retryFences: {}, settledRetries: {} };
  if (!state.ready || state.resyncRequired) return state;
  if (f.type === "presentation.snapshot") {
    if (!Number.isSafeInteger(f.projection_revision) || (f.projection_revision as number) < 0 || !validCursor(f.cursor) || !Array.isArray(f.items)) return state;
    const items = orderPresentationItems(f.items);
    const itemRevisions = Object.fromEntries(items.map((item) => [item.item_key, f.projection_revision as number]));
    return { ...state, projectionRevision: f.projection_revision as number, cursor: f.cursor, snapshotReady: true, items, itemRevisions, turns: {}, drafts: {} };
  }
  if (f.type === "presentation.upsert") {
    if (!state.snapshotReady || !Number.isSafeInteger(f.projection_revision) || (f.projection_revision as number) <= state.projectionRevision || !validCursor(f.cursor)) return state;
    const [item] = orderPresentationItems([f.item]);
    if (!item) return { ...state, projectionRevision: f.projection_revision as number, cursor: f.cursor };
    const previousRevision = state.itemRevisions[item.item_key] ?? -1;
    if ((f.projection_revision as number) <= previousRevision) return { ...state, projectionRevision: f.projection_revision as number, cursor: f.cursor };
    const itemRevisions = { ...state.itemRevisions, [item.item_key]: f.projection_revision as number };
    const items = [...state.items.filter((current) => current.item_key !== item.item_key), item]
      .sort((a, b) => a.order_key.localeCompare(b.order_key) || a.item_key.localeCompare(b.item_key));
    return { ...state, projectionRevision: f.projection_revision as number, cursor: f.cursor, items, itemRevisions };
  }
  if (f.type === "turn.delta") {
    if (!state.snapshotReady) return state;
    if (f.workspace_id !== state.workspaceId || !text(f.conversation_id, 256) || !text(f.turn_id, 256) || !Number.isSafeInteger(f.retry_ordinal) || (f.retry_ordinal as number) < 0
      || !Number.isSafeInteger(f.sequence) || (f.sequence as number) < 1 || !text(f.text_delta, 32 * 1024)) return state;
    const baseKey = turnKey(f.conversation_id, f.turn_id);
    const latestRetry = state.retryFences[baseKey] ?? -1;
    const settledRetry = state.settledRetries[baseKey] ?? -1;
    if ((f.retry_ordinal as number) < latestRetry || (f.retry_ordinal as number) <= settledRetry) return state;
    const key = turnKey(f.conversation_id, f.turn_id, f.retry_ordinal as number);
    const previous = state.turns[key];
    if (previous && previous.state !== "STREAMING") return state;
    if (previous && (f.sequence as number) <= previous.lastSequence) return state;
    if (previous && (f.sequence as number) !== previous.lastSequence + 1) {
      return { ...state, turns: { ...state.turns, [key]: { ...previous, text: "", state: "RECONNECTING" } }, retryFences: setRetryFence(state.retryFences, baseKey, f.retry_ordinal as number) };
    }
    const nextText = `${previous?.text ?? ""}${f.text_delta}`;
    if (nextText.length > 256 * 1024) return previous
      ? { ...state, turns: { ...state.turns, [key]: { ...previous, text: "", state: "RECONNECTING" } } }
      : state;
    const turn: TransientTurn = { conversationId: f.conversation_id, turnId: f.turn_id, retryOrdinal: f.retry_ordinal as number, text: nextText, lastSequence: f.sequence as number, state: "STREAMING" };
    const drafts = { ...state.drafts };
    const activeDraft = drafts[baseKey];
    if (activeDraft && (f.retry_ordinal as number) > activeDraft.retryOrdinal) delete drafts[baseKey];
    return { ...state, turns: { ...state.turns, [key]: turn }, drafts, retryFences: setRetryFence(state.retryFences, baseKey, f.retry_ordinal as number) };
  }
  if (f.type === "turn.settled") {
    if (!text(f.conversation_id, 256) || !text(f.turn_id, 256) || !Number.isSafeInteger(f.retry_ordinal) || (f.retry_ordinal as number) < 0) return state;
    const key = turnKey(f.conversation_id, f.turn_id, f.retry_ordinal as number);
    const current = state.turns[key];
    const turns = { ...state.turns };
    if (current) {
      if (typeof f.committed_message_id === "string" && f.committed_message_id.length > 0) delete turns[key];
      else turns[key] = { ...current, state: "INCOMPLETE" };
    }
    const drafts = { ...state.drafts };
    const baseKey = turnKey(f.conversation_id, f.turn_id);
    const activeDraft = drafts[baseKey];
    if (activeDraft && activeDraft.retryOrdinal <= (f.retry_ordinal as number)) delete drafts[baseKey];
    const settledRetries = setRetryFence(state.settledRetries, baseKey, f.retry_ordinal as number);
    return { ...state, turns, drafts, retryFences: setRetryFence(state.retryFences, baseKey, f.retry_ordinal as number), settledRetries };
  }
  if (f.type === "rich.draft") return reduceRichDraft(state, f);
  return state;
}

function reduceRichDraft(state: PresentationStreamState, frame: Record<string, unknown>): PresentationStreamState {
  if (!state.snapshotReady || !draftFrameIdentity(frame, state) || typeof frame.event !== "string") return state;
  const conversationId = frame.conversation_id as string;
  const turnId = frame.turn_id as string;
  const retryOrdinal = frame.retry_ordinal as number;
  const agentSessionId = frame.agent_session_id as string;
  const draftId = frame.draft_id as string;
  const key = turnKey(conversationId, turnId);
  const payload = object(frame.payload);
  const sequence = frame.sequence as number;
  const existing = state.drafts[key];
  const latestRetry = state.retryFences[key] ?? -1;
  const settledRetry = state.settledRetries[key] ?? -1;

  if (frame.event === "DRAFT_STARTED") {
    if (!payload || Object.keys(payload).length !== 0 || sequence !== 1 || retryOrdinal < latestRetry || retryOrdinal <= settledRetry) return state;
    if (existing) {
      if (retryOrdinal <= existing.retryOrdinal) return state;
    } else if (Object.keys(state.drafts).length >= MAX_ACTIVE_DRAFTS) return state;
    const draft: RichDraft = { conversationId, turnId, retryOrdinal, agentSessionId, draftId, lastSequence: sequence, status: "STREAMING", blocks: [], bytes: 0, frameCount: 1 };
    return { ...state, drafts: { ...state.drafts, [key]: draft }, retryFences: setRetryFence(state.retryFences, key, retryOrdinal) };
  }

  if (!existing || !["STREAMING", "FINALIZING"].includes(existing.status) || existing.retryOrdinal !== retryOrdinal
    || existing.agentSessionId !== agentSessionId || existing.draftId !== draftId) return state;
  if (sequence <= existing.lastSequence) return state;
  if (sequence !== existing.lastSequence + 1) return failDraft(state, key, existing, "RECONNECTING");
  if (existing.frameCount >= MAX_DRAFT_FRAMES) return failDraft(state, key, existing, "FAILED");
  const next = { ...existing, lastSequence: sequence, frameCount: existing.frameCount + 1 };

  if (frame.event === "DRAFT_FAILED") {
    if (!payload || !["INVALID_INTENT", "LIMIT_EXCEEDED", "UNSUPPORTED", "COMPILER_FAILURE", "CANCELLED"].includes(String(payload.reason_class))) return failDraft(state, key, next, "FAILED");
    return failDraft(state, key, next, "FAILED");
  }
  if (frame.event === "DRAFT_FINALIZING") {
    if (existing.status !== "STREAMING" || !payload || Object.keys(payload).length !== 0 || next.blocks.some((block) => !block.closed)) return failDraft(state, key, next, "FAILED");
    return { ...state, drafts: { ...state.drafts, [key]: { ...next, status: "FINALIZING" } } };
  }
  if (frame.event === "DRAFT_PUBLISHED") {
    if (existing.status !== "FINALIZING" || !payload || !text(payload.presentation_id, 256)) return failDraft(state, key, next, "FAILED");
    // Publication may follow finalization in the stream; accept it only after all blocks closed.
    if (next.blocks.some((block) => !block.closed)) return failDraft(state, key, next, "FAILED");
    return { ...state, drafts: { ...state.drafts, [key]: { ...next, status: "PUBLISHED", publishedPresentationId: payload.presentation_id } } };
  }
  if (!payload || existing.status !== "STREAMING") return failDraft(state, key, next, "FAILED");

  if (frame.event === "BLOCK_OPENED") {
    if (!text(payload.block_id, 128) || !Number.isSafeInteger(payload.index) || (payload.index as number) < 0 || (payload.index as number) >= MAX_DRAFT_BLOCKS
      || !text(payload.block_kind, 64) || !DRAFT_BLOCK_KINDS.has(payload.block_kind)) return failDraft(state, key, next, "FAILED");
    const parentId = payload.parent_id === undefined ? undefined : text(payload.parent_id, 128) ? payload.parent_id : null;
    if (parentId === null || next.blocks.length >= MAX_DRAFT_BLOCKS || next.blocks.some((block) => block.blockId === payload.block_id)) return failDraft(state, key, next, "FAILED");
    const siblings = next.blocks.filter((block) => block.parentId === parentId);
    if (siblings.some((block) => block.index === payload.index)) return failDraft(state, key, next, "FAILED");
    if (parentId !== undefined) {
      const parent = next.blocks.find((block) => block.blockId === parentId);
      if (!parent || parent.closed || !["LAYOUT", "CARD"].includes(parent.kind)) return failDraft(state, key, next, "FAILED");
    }
    if (childDepth(next.blocks, parentId) > MAX_DRAFT_DEPTH || next.blocks.filter((block) => !block.closed).length >= MAX_OPEN_BLOCKS) return failDraft(state, key, next, "FAILED");
    const block: RichDraftBlock = { blockId: payload.block_id, ...(parentId ? { parentId } : {}), index: payload.index as number, kind: payload.block_kind, text: "", closed: false };
    return { ...state, drafts: { ...state.drafts, [key]: { ...next, blocks: [...next.blocks, block] } } };
  }

  if (frame.event === "TEXT_APPENDED") {
    if (!text(payload.block_id, 128) || typeof payload.text !== "string" || payload.text.includes("\0") || utf8Length(payload.text) > 16_384) return failDraft(state, key, next, "FAILED");
    const index = next.blocks.findIndex((block) => block.blockId === payload.block_id);
    if (index < 0 || next.blocks[index]!.closed || next.blocks[index]!.kind !== "TEXT_SLICE") return failDraft(state, key, next, "FAILED");
    const addedBytes = utf8Length(payload.text);
    if (next.bytes + addedBytes > MAX_DRAFT_BYTES) return failDraft(state, key, next, "FAILED");
    const blocks = [...next.blocks];
    blocks[index] = { ...blocks[index]!, text: blocks[index]!.text + payload.text };
    return { ...state, drafts: { ...state.drafts, [key]: { ...next, blocks, bytes: next.bytes + addedBytes } } };
  }

  if (frame.event === "BLOCK_CLOSED") {
    if (!text(payload.block_id, 128)) return failDraft(state, key, next, "FAILED");
    const index = next.blocks.findIndex((block) => block.blockId === payload.block_id);
    if (index < 0 || next.blocks[index]!.closed || next.blocks.some((block) => block.parentId === payload.block_id && !block.closed)) return failDraft(state, key, next, "FAILED");
    const blocks = [...next.blocks];
    blocks[index] = { ...blocks[index]!, closed: true };
    return { ...state, drafts: { ...state.drafts, [key]: { ...next, blocks } } };
  }

  if (frame.event === "BLOCK_REPLACED") {
    if (!text(payload.block_id, 128) || !payload.block || typeof payload.block !== "object" || Array.isArray(payload.block)) return failDraft(state, key, next, "FAILED");
    let encoded: string;
    try { encoded = JSON.stringify(payload.block); } catch { return failDraft(state, key, next, "FAILED"); }
    if (encoded.length > MAX_REPLACEMENT_BYTES || next.bytes + utf8Length(encoded) > MAX_DRAFT_BYTES) return failDraft(state, key, next, "FAILED");
    const replacement = object(payload.block);
    const index = next.blocks.findIndex((block) => block.blockId === payload.block_id);
    if (!replacement || index < 0 || next.blocks[index]!.closed || replacement.kind !== next.blocks[index]!.kind) return failDraft(state, key, next, "FAILED");
    const blocks = [...next.blocks];
    blocks[index] = { ...blocks[index]!, replacement: JSON.parse(encoded) as unknown };
    return { ...state, drafts: { ...state.drafts, [key]: { ...next, blocks, bytes: next.bytes + utf8Length(encoded) } } };
  }

  return failDraft(state, key, next, "FAILED");
}
