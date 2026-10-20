export type ConversationSummary = {
  conversation_id: string;
  workspace_id: string;
  title: string | null;
  active_agent_binding_id: string | null;
  version: number;
  created_at: string;
};

export type ConversationListPage = {
  items: ConversationSummary[];
  next_cursor: string | null;
};

export type PendingConversationCreate = {
  workspaceId: string;
  title: string | null;
  requestId: string;
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

export type ConversationWorkspaceSelection = {
  workspaceId: string;
  epoch: number;
};

/** Draft keys bind unsent text to one Workspace and one saved Conversation. */
export function conversationDraftScope(workspaceId: string, conversationId: string): string {
  return `${workspaceId}\u0000${conversationId}`;
}

/** Fence async UI projections to the current Workspace selection, including A→B→A. */
export class ConversationWorkspaceSelectionEpochs {
  private workspaceId: string | null = null;
  private epoch = 0;

  select(workspaceId: string): void {
    if (workspaceId === this.workspaceId) return;
    this.workspaceId = workspaceId;
    this.epoch += 1;
  }

  capture(workspaceId: string): ConversationWorkspaceSelection | null {
    return this.workspaceId === workspaceId ? { workspaceId, epoch: this.epoch } : null;
  }

  isCurrent(selection: ConversationWorkspaceSelection): boolean {
    return selection.workspaceId === this.workspaceId && selection.epoch === this.epoch;
  }
}

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

/** Keep an ambiguous create receipt reachable for an unchanged command retry. */
export class ConversationCreateRequestCache {
  private readonly pending = new Map<string, PendingConversationCreate>();

  replace(value: unknown): PendingConversationCreate[] {
    if (!Array.isArray(value) || value.length > 64) {
      throw new Error("Local Runtime returned an invalid pending Conversation journal.");
    }
    const next = new Map<string, PendingConversationCreate>();
    for (const item of value) {
      if (!record(item)
        || Object.keys(item).sort().join(",") !== "requestId,title,workspaceId"
        || !boundedText(item.workspaceId)
        || !(item.title === null || (boundedText(item.title, 160)
          && item.title.trim() === item.title
          && Array.from(item.title).length <= 160))
        || typeof item.requestId !== "string"
        || !/^[!-~]{1,200}$/.test(item.requestId)) {
        throw new Error("Local Runtime returned an invalid pending Conversation request.");
      }
      const request = item as unknown as PendingConversationCreate;
      const key = this.key(request.workspaceId, request.title);
      if (next.has(key)) throw new Error("Local Runtime returned duplicate pending Conversation requests.");
      next.set(key, request);
    }
    this.pending.clear();
    for (const [key, request] of next) this.pending.set(key, request);
    return [...next.values()];
  }

  get(workspaceId: string, title: string | null, newRequestId: () => string): PendingConversationCreate {
    const key = this.key(workspaceId, title);
    const existing = this.pending.get(key);
    if (existing) return existing;
    if (this.pending.size >= 64) throw new Error("Resolve a pending Conversation creation before starting another.");
    const request = { workspaceId, title, requestId: newRequestId() };
    this.pending.set(key, request);
    return request;
  }

  remember(request: PendingConversationCreate): void {
    this.pending.set(this.key(request.workspaceId, request.title), request);
  }

  complete(request: PendingConversationCreate): void {
    const key = this.key(request.workspaceId, request.title);
    if (this.pending.get(key) === request) this.pending.delete(key);
  }

  completeExact(workspaceId: string, requestId: string): boolean {
    for (const [key, request] of this.pending) {
      if (request.workspaceId === workspaceId && request.requestId === requestId) {
        this.pending.delete(key);
        return true;
      }
    }
    return false;
  }

  private key(workspaceId: string, title: string | null): string {
    return JSON.stringify([workspaceId, title]);
  }
}

/** Stable binding for a fetched immutable presentation; object identity is intentionally irrelevant. */
export function richPresentationCacheKey(
  workspaceId: string,
  conversationId: string,
  messageId: string,
  reference: RichPresentationRefView | null,
): string | null {
  if (!reference || reference.availability !== "AVAILABLE" || reference.message_id !== messageId) return null;
  return JSON.stringify([
    workspaceId,
    conversationId,
    messageId,
    reference.presentation_id,
    reference.schema_version,
    reference.renderer_contract_version,
    reference.semantic_content_digest,
    reference.document_digest,
    reference.document_size_bytes,
  ]);
}

function parseConversationSummary(item: unknown, workspaceId: string): ConversationSummary {
  if (!record(item) || item.workspace_id !== workspaceId || !boundedText(item.conversation_id)
    || !(item.title === null || boundedText(item.title, 160))
    || !(item.active_agent_binding_id === null || boundedText(item.active_agent_binding_id))
    || !Number.isSafeInteger(item.version) || (item.version as number) < 1 || !boundedText(item.created_at, 64)) {
    throw new Error("Conversation list contains an invalid or out-of-Workspace item.");
  }
  return item as unknown as ConversationSummary;
}

export function parseConversationListPage(value: unknown, workspaceId: string): ConversationListPage {
  if (!record(value) || !Array.isArray(value.items) || value.items.length > 100) throw new Error("Conversation list is invalid.");
  const nextCursor = value.next_cursor;
  if (!Object.hasOwn(value, "next_cursor")
    || !(nextCursor === null || (boundedText(nextCursor, 2048) && /^[A-Za-z0-9_-]+$/.test(nextCursor)))) {
    throw new Error("Conversation list cursor is invalid.");
  }
  return {
    items: value.items.map(item => parseConversationSummary(item, workspaceId)),
    next_cursor: (nextCursor as string | null | undefined) ?? null,
  };
}

export function appendConversationListPage(current: ConversationListPage, page: ConversationListPage): ConversationListPage {
  const items = [...current.items];
  const positions = new Map(items.map((item, index) => [item.conversation_id, index]));
  for (const item of page.items) {
    const position = positions.get(item.conversation_id);
    if (position === undefined) {
      positions.set(item.conversation_id, items.length);
      items.push(item);
    } else {
      items[position] = item;
    }
  }
  return { items, next_cursor: page.next_cursor };
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

function sameCommittedMessage(left: ConversationMessageView, right: ConversationMessageView): boolean {
  if (left.role !== right.role || left.created_at !== right.created_at || left.content.length !== right.content.length) {
    return false;
  }
  return left.content.every((block, index) => {
    const next = right.content[index];
    return next !== undefined && block.kind === next.kind
      && (block.kind !== "TEXT" || (next.kind === "TEXT" && block.text === next.text));
  });
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
  if (current) {
    const committedById = new Map(current.messages.map(message => [message.message_id, message]));
    for (const message of live.messages) {
      const committed = committedById.get(message.message_id);
      if (committed && !sameCommittedMessage(committed, message)) {
        // Message IDs are immutable. Reject the entire inconsistent projection so
        // its cursor cannot replace the last accepted snapshot or cancel a fetch
        // bound to the committed semantic content.
        return current;
      }
    }
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
  return parseConversationListPage(value, workspaceId).items;
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
  if (!Object.hasOwn(value, "next_cursor")
    || !(value.next_cursor === null || (boundedText(value.next_cursor) && /^[A-Za-z0-9_-]+$/.test(value.next_cursor)))) {
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
