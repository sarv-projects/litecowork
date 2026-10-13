export type ConversationSummary = {
  conversation_id: string;
  workspace_id: string;
  title: string | null;
  active_agent_binding_id: string | null;
  version: number;
  created_at: string;
};

export type ConversationMessageView = {
  message_id: string;
  role: "USER" | "AGENT" | "SYSTEM_NOTICE" | "CHANNEL";
  created_at: string;
  content: Array<{ kind: "TEXT"; text: string } | { kind: "RESOURCE" }>;
  rich_presentation: RichPresentationRefView | null;
};

export type RichPresentationRefView = {
  presentation_id: string;
  message_id: string;
  schema_version: 1;
  renderer_contract_version: number;
  semantic_content_digest: string;
  document_digest: string;
  document_size_bytes: number;
  availability: "AVAILABLE" | "FETCHING" | "UNAVAILABLE" | "POLICY_OMITTED" | "UNSUPPORTED_VERSION" | "INTEGRITY_FAILED";
};

export type ConversationSnapshotView = {
  workspace_id: string;
  conversation_id: string;
  /** Omitted only for the local empty state immediately after Conversation creation. */
  projection_revision?: number;
  stream_cursor?: string;
  origin_sequence?: string;
  messages: ConversationMessageView[];
  next_cursor: string | null;
  active_turn: { status: string } | null;
};

/** Separate latest-request fences for independently loaded Conversation views. */
export class ConversationRequestEpochs {
  private listEpoch = 0;
  private snapshotEpoch = 0;

  beginList(): number { return ++this.listEpoch; }
  isCurrentList(epoch: number): boolean { return epoch === this.listEpoch; }
  invalidateList(): void { this.listEpoch += 1; }

  beginSnapshot(): number { return ++this.snapshotEpoch; }
  isCurrentSnapshot(epoch: number): boolean { return epoch === this.snapshotEpoch; }
  invalidateSnapshot(): void { this.snapshotEpoch += 1; }
}

export function appendConversationPage(
  current: ConversationSnapshotView,
  page: ConversationSnapshotView,
): ConversationSnapshotView {
  if (current.workspace_id !== page.workspace_id || current.conversation_id !== page.conversation_id) {
    throw new Error("Conversation page does not match the selected history.");
  }
  const seen = new Set(current.messages.map(message => message.message_id));
  const additional = page.messages.filter(message => {
    if (seen.has(message.message_id)) return false;
    seen.add(message.message_id);
    return true;
  });
  return { ...current, ...page, messages: [...current.messages, ...additional] };
}

/**
 * Apply a fresh live-window snapshot without discarding pages the user already loaded.
 * Conversation messages are append-only, so unseen committed messages can be merged by
 * stable identity while refreshed rows replace their prior presentation references.
 */
export function mergeConversationStreamSnapshot(
  current: ConversationSnapshotView | null,
  live: ConversationSnapshotView,
  workspaceId: string,
  conversationId: string,
): ConversationSnapshotView {
  if (live.workspace_id !== workspaceId || live.conversation_id !== conversationId
    || (current && (current.workspace_id !== workspaceId || current.conversation_id !== conversationId))) {
    throw new Error("Live Conversation snapshot does not match the selected Conversation.");
  }
  const byId = new Map((current?.messages ?? []).map(message => [message.message_id, message]));
  for (const message of live.messages) byId.set(message.message_id, message);
  const messages = [...byId.values()].sort((left, right) =>
    left.created_at.localeCompare(right.created_at) || left.message_id.localeCompare(right.message_id));
  let nextCursor = current?.stream_cursor === undefined ? live.next_cursor : current.next_cursor;
  if (current && current.stream_cursor !== undefined && current.next_cursor !== null) {
    const currentCursorIndex = messages.findIndex(message => message.message_id === current.next_cursor);
    const liveCursorIndex = live.next_cursor === null ? -1 : messages.findIndex(message => message.message_id === live.next_cursor);
    if (liveCursorIndex > currentCursorIndex) nextCursor = live.next_cursor;
  }
  return {
    ...current,
    ...live,
    workspace_id: workspaceId,
    conversation_id: conversationId,
    messages,
    // The live stream snapshot is the first bounded page. Keep the deeper
    // forward-pagination position already reached by the selected history.
    next_cursor: nextCursor,
  };
}

const record = (value: unknown): value is Record<string, unknown> => value !== null && typeof value === "object" && !Array.isArray(value);
const boundedText = (value: unknown, max = 256): value is string => typeof value === "string" && value.length > 0 && value.length <= max && !/[\u0000-\u001f]/.test(value);
const sha256Digest = (value: unknown): value is string => typeof value === "string" && /^sha256:[a-f0-9]{64}$/.test(value);

function parseRichPresentationRef(value: unknown, messageId: string): RichPresentationRefView | null {
  if (value === null) return null;
  if (!record(value)
    || Object.keys(value).sort().join(",") !== [
      "availability", "document_digest", "document_size_bytes", "message_id", "presentation_id",
      "renderer_contract_version", "schema_version", "semantic_content_digest",
    ].sort().join(",")
    || !boundedText(value.presentation_id)
    || value.message_id !== messageId
    || value.schema_version !== 1
    || !Number.isSafeInteger(value.renderer_contract_version)
    || (value.renderer_contract_version as number) < 1
    || !sha256Digest(value.semantic_content_digest)
    || !sha256Digest(value.document_digest)
    || !Number.isSafeInteger(value.document_size_bytes)
    || (value.document_size_bytes as number) < 1
    || (value.document_size_bytes as number) > 1_048_576
    || !["AVAILABLE", "FETCHING", "UNAVAILABLE", "POLICY_OMITTED", "UNSUPPORTED_VERSION", "INTEGRITY_FAILED"].includes(String(value.availability))) {
    throw new Error("Conversation RichPresentation reference is invalid.");
  }
  return value as unknown as RichPresentationRefView;
}

export function parseConversationList(value: unknown, workspaceId: string): ConversationSummary[] {
  if (!record(value) || !Array.isArray(value.items) || value.items.length > 100) throw new Error("Conversation list is invalid.");
  return value.items.map((item) => {
    if (!record(item) || item.workspace_id !== workspaceId || !boundedText(item.conversation_id)
      || !(item.title === null || boundedText(item.title, 160))
      || !(item.active_agent_binding_id === null || boundedText(item.active_agent_binding_id))
      || !Number.isSafeInteger(item.version) || (item.version as number) < 1 || !boundedText(item.created_at, 64)) {
      throw new Error("Conversation list contains an invalid or out-of-Workspace item.");
    }
    return item as unknown as ConversationSummary;
  });
}

export function parseConversationSnapshot(value: unknown, workspaceId: string, conversationId: string): ConversationSnapshotView {
  if (!record(value) || value.workspace_id !== workspaceId || value.conversation_id !== conversationId
    || !Array.isArray(value.items) || value.items.length > 100
    || !(value.active_turn === null || record(value.active_turn))
    || !Number.isSafeInteger(value.projection_revision) || (value.projection_revision as number) < 1
    || !boundedText(value.stream_cursor, 2048)
    || typeof value.origin_sequence !== "string" || !/^(0|[1-9][0-9]{0,18})$/.test(value.origin_sequence)) {
    throw new Error("Conversation history or source frontier is invalid.");
  }
  if (value.next_cursor !== undefined && value.next_cursor !== null
    && !(boundedText(value.next_cursor) && /^[A-Za-z0-9_-]+$/.test(value.next_cursor))) {
    throw new Error("Conversation pagination cursor is invalid.");
  }
  const messages: ConversationMessageView[] = value.items.map((entry) => {
    if (!record(entry) || !record(entry.message) || !("rich_presentation" in entry)
      || !Array.isArray(entry.linked_items) || entry.linked_items.length > 200) throw new Error("Conversation message is invalid.");
    const message = entry.message;
    if (message.conversation_id !== conversationId || !boundedText(message.message_id)
      || !["USER", "AGENT", "SYSTEM_NOTICE", "CHANNEL"].includes(String(message.role))
      || !boundedText(message.created_at, 64) || !Array.isArray(message.content) || message.content.length > 128) {
      throw new Error("Conversation history contains an invalid message.");
    }
    const content = message.content.map((block) => {
      if (!record(block)) throw new Error("Conversation content is invalid.");
      if (block.kind === "TEXT" && typeof block.text === "string" && block.text.length <= 262_144) return { kind: "TEXT" as const, text: block.text };
      if (block.kind === "RESOURCE") return { kind: "RESOURCE" as const };
      throw new Error("Conversation contains unsupported content.");
    });
    return {
      message_id: message.message_id,
      role: message.role as ConversationMessageView["role"],
      created_at: message.created_at,
      content,
      rich_presentation: parseRichPresentationRef(entry.rich_presentation, message.message_id),
    };
  });
  return {
    workspace_id: workspaceId,
    conversation_id: conversationId,
    projection_revision: value.projection_revision as number,
    stream_cursor: value.stream_cursor,
    origin_sequence: value.origin_sequence,
    messages,
    next_cursor: (value.next_cursor as string | null | undefined) ?? null,
    active_turn: value.active_turn as { status: string } | null,
  };
}
