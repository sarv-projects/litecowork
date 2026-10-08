import { orderPresentationItems, type PresentationItem } from "./presentation-types";

/** Transport frames are untrusted and only update an ephemeral rendering projection. */
export type PresentationFrame =
  | { type: "stream.ready"; workspace_id: string; projection_version: number; cursor: string; max_frame_bytes: number }
  | { type: "presentation.snapshot"; projection_revision: number; cursor: string; items: unknown[] }
  | { type: "presentation.upsert"; projection_revision: number; cursor: string; item: unknown }
  | { type: "stream.resync_required" }
  | { type: "turn.delta"; conversation_id: string; turn_id: string; retry_ordinal: number; sequence: number; text_delta: string }
  | { type: "turn.settled"; conversation_id: string; turn_id: string; retry_ordinal: number; committed_message_id?: string };

export type TransientTurn = {
  conversationId: string;
  turnId: string;
  retryOrdinal: number;
  text: string;
  lastSequence: number;
  state: "STREAMING" | "RECONNECTING" | "INCOMPLETE";
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
};

export const emptyPresentationStream: PresentationStreamState = {
  ready: false, resyncRequired: false, snapshotReady: false, workspaceId: null, projectionVersion: null,
  projectionRevision: 0, cursor: null, maxFrameBytes: 0, items: [], itemRevisions: {}, turns: {},
};

function text(value: unknown, max: number): value is string {
  return typeof value === "string" && value.length > 0 && value.length <= max && !value.includes("\0");
}
function validCursor(value: unknown): value is string { return text(value, 2048); }
function turnKey(conversationId: string, turnId: string, retryOrdinal: number): string {
  return JSON.stringify([conversationId, turnId, retryOrdinal]);
}

/**
 * Apply one decoded protocol frame. Caller must also enforce the negotiated frame byte
 * ceiling while decoding; this reducer bounds text again and ignores frames before ready.
 */
export function reducePresentationFrame(state: PresentationStreamState, frame: unknown): PresentationStreamState {
  if (!frame || typeof frame !== "object" || Array.isArray(frame)) return state;
  const f = frame as Record<string, unknown>;
  if (f.type === "stream.ready") {
    if (!text(f.workspace_id, 256) || !Number.isSafeInteger(f.projection_version) || !validCursor(f.cursor)
      || !Number.isSafeInteger(f.max_frame_bytes) || (f.max_frame_bytes as number) < 1024 || (f.max_frame_bytes as number) > 4 * 1024 * 1024) return state;
    return { ...emptyPresentationStream, ready: true, workspaceId: f.workspace_id, projectionVersion: f.projection_version as number, cursor: f.cursor, maxFrameBytes: f.max_frame_bytes as number };
  }
  if (f.type === "stream.resync_required") return { ...state, ready: false, resyncRequired: true, snapshotReady: false, cursor: null, items: [], itemRevisions: {}, turns: {} };
  if (!state.ready || state.resyncRequired) return state;
  if (f.type === "presentation.snapshot") {
    if (!Number.isSafeInteger(f.projection_revision) || (f.projection_revision as number) < 0 || !validCursor(f.cursor) || !Array.isArray(f.items)) return state;
    const items = orderPresentationItems(f.items);
    const itemRevisions = Object.fromEntries(items.map((item) => [item.item_key, f.projection_revision as number]));
    return { ...state, projectionRevision: f.projection_revision as number, cursor: f.cursor, snapshotReady: true, items, itemRevisions, turns: {} };
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
    if (!text(f.conversation_id, 256) || !text(f.turn_id, 256) || !Number.isSafeInteger(f.retry_ordinal) || (f.retry_ordinal as number) < 0
      || !Number.isSafeInteger(f.sequence) || (f.sequence as number) < 0 || !text(f.text_delta, 32 * 1024)) return state;
    const key = turnKey(f.conversation_id, f.turn_id, f.retry_ordinal as number);
    const previous = state.turns[key];
    if (previous && previous.state !== "STREAMING") return state;
    if (previous && (f.sequence as number) <= previous.lastSequence) return state;
    if (previous && (f.sequence as number) !== previous.lastSequence + 1) {
      return { ...state, turns: { ...state.turns, [key]: { ...previous, text: "", state: "RECONNECTING" } } };
    }
    const nextText = `${previous?.text ?? ""}${f.text_delta}`;
    // Bound transient memory independently of the advertised transport limit.
    if (nextText.length > 256 * 1024) return { ...state, turns: { ...state.turns, [key]: { ...previous!, text: "", state: "RECONNECTING" } } };
    const turn: TransientTurn = { conversationId: f.conversation_id, turnId: f.turn_id, retryOrdinal: f.retry_ordinal as number, text: nextText, lastSequence: f.sequence as number, state: "STREAMING" };
    return { ...state, turns: { ...state.turns, [key]: turn } };
  }
  if (f.type === "turn.settled") {
    if (!text(f.conversation_id, 256) || !text(f.turn_id, 256) || !Number.isSafeInteger(f.retry_ordinal) || (f.retry_ordinal as number) < 0) return state;
    const key = turnKey(f.conversation_id, f.turn_id, f.retry_ordinal as number);
    const current = state.turns[key];
    if (!current) return state;
    const turns = { ...state.turns };
    if (typeof f.committed_message_id === "string" && f.committed_message_id.length > 0) delete turns[key];
    else turns[key] = { ...current, state: "INCOMPLETE" };
    return { ...state, turns };
  }
  return state;
}
