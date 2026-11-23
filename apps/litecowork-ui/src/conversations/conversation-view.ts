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
  /** Exact first-turn request body, persisted privately before it reaches the daemon. */
  firstTurn?: PendingConversationFirstTurn;
};

export type PendingConversationFirstTurn = {
  content: Array<{ kind: "TEXT"; text: string } | {
    kind: "RESOURCE";
    resourceRef: { workspaceId: string; resourceId: string; revisionId?: string | null };
    displayName?: string | null;
  }>;
  resourceRefs: Array<{ workspaceId: string; resourceId: string; revisionId?: string | null }>;
  /** Exact binding selected in the New Chat selector, including the visible Workspace default. */
  agentBindingId: string;
  presentationPreference: "AUTO" | "SIMPLE" | "RICH";
};

export type PendingConversationSend = {
  workspaceId: string;
  conversationId: string;
  requestId: string;
  content: Array<{ kind: "TEXT"; text: string } | { kind: "RESOURCE"; [key: string]: unknown }>;
  resourceRefs: Array<{ workspace_id: string; resource_id: string; revision_id?: string | null }>;
  agentBindingId: string | null;
  presentationPreference: "AUTO" | "SIMPLE" | "RICH";
  attachmentLabels: string[];
};

export type ConversationSendOutcome =
  | { kind: "ADMITTED"; receipt: Record<string, unknown> }
  | { kind: "REFUSED"; status: 400 | 409 | 422; code: string; message: string };

/** Keep an unresolved exact request visible if local acknowledgement cannot be confirmed. */
export function retainPendingConversationSend(
  current: PendingConversationSend[],
  request: PendingConversationSend,
): PendingConversationSend[] {
  const exists = current.some(send =>
    send.workspaceId === request.workspaceId
    && send.conversationId === request.conversationId
    && send.requestId === request.requestId);
  return exists ? current : [...current, request];
}

export function parseConversationSendOutcome(
  value: unknown,
  conversationId: string,
  preference: "AUTO" | "SIMPLE" | "RICH",
): ConversationSendOutcome {
  if (!record(value)) throw new Error("Conversation send outcome is invalid.");
  const keys = Object.keys(value).sort().join(",");
  if (value.kind === "REFUSED"
    && keys === "code,kind,message,status"
    && typeof value.status === "number"
    && typeof value.code === "string"
    && typeof value.message === "string"
    && value.message.length > 0 && value.message.length <= 2048
    && !/[\u0000-\u001f]/.test(value.message)
    && ((value.status === 400 && ["INVALID_ARGUMENT", "INVALID_RESOURCE_REF"].includes(value.code))
      || (value.status === 409 && ["RESOURCE_STALE", "CONVERSATION_TURN_IN_PROGRESS"].includes(value.code))
      || (value.status === 422 && ["INVALID_RESOURCE_REF", "AGENT_UNAVAILABLE", "AGENT_NOT_LEAD_ELIGIBLE"].includes(value.code)))) {
    return {
      kind: "REFUSED",
      status: value.status as 400 | 409 | 422,
      code: value.code,
      message: value.message,
    };
  }
  if (value.kind !== "ADMITTED" || keys !== "kind,receipt" || !record(value.receipt)) {
    throw new Error("Conversation send outcome is invalid.");
  }
  const receipt = value.receipt;
  if (Object.keys(receipt).sort().join(",") !== [
    "accepted_at", "conversation_id", "correlation_id", "presentation_preference", "status", "turn_id", "user_message_id",
  ].sort().join(",")
    || receipt.conversation_id !== conversationId
    || typeof receipt.turn_id !== "string" || !/^[A-Za-z0-9_-]{1,256}$/.test(receipt.turn_id)
    || typeof receipt.user_message_id !== "string" || !/^[A-Za-z0-9_-]{1,256}$/.test(receipt.user_message_id)
    || typeof receipt.correlation_id !== "string" || !/^[A-Za-z0-9_-]{1,256}$/.test(receipt.correlation_id)
    || typeof receipt.accepted_at !== "string" || receipt.accepted_at.length === 0
    || receipt.accepted_at.length > 64 || /[\u0000-\u001f]/.test(receipt.accepted_at)
    || !["OPEN", "RUNNING", "WAITING_USER", "WAITING_DEPENDENCY", "CANCEL_REQUESTED", "COMPLETED", "FAILED", "CANCELLED"].includes(String(receipt.status))
    || receipt.presentation_preference !== preference) {
    throw new Error("Conversation send receipt is invalid.");
  }
  return { kind: "ADMITTED", receipt };
}

export function pendingConversationSendText(send: PendingConversationSend): string {
  return send.content
    .filter((block): block is { kind: "TEXT"; text: string } => block.kind === "TEXT")
    .map(block => block.text)
    .join("\n");
}

export function restorePendingConversationAttachmentChips(
  send: PendingConversationSend,
  candidates: readonly {
    resourceId: string;
    workspaceId: string;
    resourceRevisionId: string;
    displayName: string;
    mediaType: string;
    sizeBytes: number;
  }[],
): {
  resourceId: string;
  workspaceId: string;
  resourceRevisionId: string;
  displayName: string;
  mediaType: string;
  sizeBytes: number;
}[] {
  if (send.resourceRefs.length !== send.attachmentLabels.length) {
    throw new Error("Pending Conversation attachment labels do not match its Resource references.");
  }
  return send.resourceRefs.map((reference, index) => {
    const candidate = candidates.find(item =>
      item.workspaceId === send.workspaceId && item.resourceId === reference.resource_id);
    return {
      resourceId: reference.resource_id,
      workspaceId: send.workspaceId,
      // Keep the revision the user originally selected, even when the picker now
      // exposes a newer Resource revision after a restart.
      resourceRevisionId: reference.revision_id ?? candidate?.resourceRevisionId ?? "",
      displayName: send.attachmentLabels[index] ?? candidate?.displayName ?? reference.resource_id,
      // A revoked or unavailable Resource still needs a removable chip. This local
      // display fallback is intentionally non-attachable and is never authority.
      mediaType: candidate?.mediaType ?? "application/x-litecowork-recovered-resource",
      sizeBytes: candidate?.sizeBytes ?? Number.MAX_SAFE_INTEGER,
    };
  });
}

export type ConversationMessageView = {
  message_id: string;
  role: "USER" | "AGENT" | "SYSTEM_NOTICE" | "CHANNEL";
  /** LiteCowork product identities for attribution; provider-native handles stay private. */
  agent_binding_id?: string | null;
  agent_session_id?: string | null;
  /** Immutable owning turn identity; null remains unknown for legacy/orphan rows. */
  turn_id: string | null;
  /** Same-snapshot projection only; it is not Stop, Retry, or Task authority. */
  turn_status: ConversationTurnStatus | null;
  created_at: string;
  content: Array<{ kind: "TEXT"; text: string } | { kind: "RESOURCE" }>;
  resource_refs: ConversationResourceRefView[];
  rich_presentation: RichPresentationRefView | null;
};

export type ConversationTurnStatus =
  | "OPEN" | "RUNNING" | "WAITING_USER" | "WAITING_DEPENDENCY" | "CANCEL_REQUESTED"
  | "COMPLETED" | "FAILED" | "CANCELLED";

export type ConversationResourceRefView = {
  workspace_id: string;
  resource_id: string;
  revision_id: string | null;
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

import type { ActiveTurnView, RetryableTurnView } from "./conversation-turn-state.ts";

export type ConversationSnapshotView = {
  workspace_id: string;
  conversation_id: string;
  /** Omitted only for the local empty state immediately after Conversation creation. */
  projection_revision?: number;
  stream_cursor?: string;
  origin_sequence?: string;
  messages: ConversationMessageView[];
  next_cursor: string | null;
  /** Identity and aggregate version of the in-flight turn; Stop sends them as If-Match. */
  active_turn: ActiveTurnView | null;
  /** The latest turn when it FAILED, with the version Retry sends and its typed reason. Omitted only for the local empty state. */
  retryable_turn?: RetryableTurnView | null;
  /** Latest turn identity for read-only REPORTED activity reload; omitted only by legacy/local empty snapshots. */
  activity_turn?: ConversationActivityTurnView | null;
};

export type ConversationActivityTurnView = {
  turn_id: string;
  retry_ordinal: number;
  status: "OPEN" | "RUNNING" | "WAITING_USER" | "WAITING_DEPENDENCY" | "CANCEL_REQUESTED" | "COMPLETED" | "FAILED" | "CANCELLED";
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
        || !["requestId,title,workspaceId", "firstTurn,requestId,title,workspaceId"].includes(Object.keys(item).sort().join(","))
        || !boundedText(item.workspaceId)
        || !(item.title === null || (boundedText(item.title, 160)
          && item.title.trim() === item.title
          && Array.from(item.title).length <= 160))
        || typeof item.requestId !== "string"
        || !/^[!-~]{1,200}$/.test(item.requestId)
        || (item.firstTurn !== undefined && !validPendingFirstTurn(item.firstTurn, item.workspaceId))) {
        throw new Error("Local Runtime returned an invalid pending Conversation request.");
      }
      const request = item as unknown as PendingConversationCreate;
      const key = this.key(request.workspaceId, request.title, request.firstTurn);
      if (next.has(key)) throw new Error("Local Runtime returned duplicate pending Conversation requests.");
      next.set(key, request);
    }
    this.pending.clear();
    for (const [key, request] of next) this.pending.set(key, request);
    return [...next.values()];
  }

  get(workspaceId: string, title: string | null, newRequestId: () => string, firstTurn?: PendingConversationFirstTurn): PendingConversationCreate {
    if (firstTurn !== undefined && !validPendingFirstTurn(firstTurn, workspaceId)) {
      throw new Error("The first Conversation turn is not valid for recovery.");
    }
    const key = this.key(workspaceId, title, firstTurn);
    const existing = this.pending.get(key);
    if (existing) return existing;
    if (this.pending.size >= 64) throw new Error("Resolve a pending Conversation creation before starting another.");
    const request = { workspaceId, title, requestId: newRequestId(), ...(firstTurn ? { firstTurn } : {}) };
    this.pending.set(key, request);
    return request;
  }

  remember(request: PendingConversationCreate): void {
    this.pending.set(this.key(request.workspaceId, request.title, request.firstTurn), request);
  }

  complete(request: PendingConversationCreate): void {
    const key = this.key(request.workspaceId, request.title, request.firstTurn);
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

  private key(workspaceId: string, title: string | null, firstTurn?: PendingConversationFirstTurn): string {
    return stableJson([workspaceId, title, firstTurn ?? null]);
  }
}

function stableJson(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(stableJson).join(",")}]`;
  if (record(value)) return `{${Object.keys(value).sort().map(key => `${JSON.stringify(key)}:${stableJson(value[key])}`).join(",")}}`;
  const encoded = JSON.stringify(value);
  if (encoded === undefined) throw new Error("Conversation request identity is not serializable.");
  return encoded;
}

function validPendingFirstTurn(value: unknown, workspaceId: string): value is PendingConversationFirstTurn {
  if (!record(value)
    || Object.keys(value).sort().join(",") !== "agentBindingId,content,presentationPreference,resourceRefs"
    || !Array.isArray(value.content) || value.content.length < 1 || value.content.length > 256
    || !Array.isArray(value.resourceRefs) || value.resourceRefs.length > 256
    || !boundedText(value.agentBindingId)
    || !["AUTO", "SIMPLE", "RICH"].includes(String(value.presentationPreference))) return false;
  const validRef = (ref: unknown) => record(ref)
    && ["resourceId,workspaceId", "resourceId,revisionId,workspaceId"].includes(Object.keys(ref).sort().join(","))
    && boundedText(ref.workspaceId)
    && ref.workspaceId === workspaceId
    && boundedText(ref.resourceId)
    && (ref.revisionId === undefined || ref.revisionId === null || boundedText(ref.revisionId));
  return value.resourceRefs.every(validRef) && value.content.every(block => {
    if (!record(block)) return false;
    if (block.kind === "TEXT") return Object.keys(block).sort().join(",") === "kind,text"
      && typeof block.text === "string" && block.text.length <= 256 * 1024;
    if (block.kind === "RESOURCE") return ["displayName,kind,resourceRef", "kind,resourceRef"].includes(Object.keys(block).sort().join(","))
      && validRef(block.resourceRef)
      && (block.displayName === undefined || block.displayName === null || boundedText(block.displayName, 1024));
    return false;
  });
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
  if (left.role !== right.role || left.created_at !== right.created_at
    || (left.agent_binding_id ?? null) !== (right.agent_binding_id ?? null)
    || (left.agent_session_id ?? null) !== (right.agent_session_id ?? null)
    || left.turn_id !== right.turn_id
    || JSON.stringify(left.resource_refs) !== JSON.stringify(right.resource_refs)
    || left.content.length !== right.content.length) {
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

export function parsePendingConversationSends(value: unknown): PendingConversationSend[] {
  if (!Array.isArray(value) || value.length > 64) {
    throw new Error("Pending Conversation sends are invalid.");
  }
  const sends: PendingConversationSend[] = [];
  const requestIds = new Set<string>();
  const conversations = new Set<string>();
  for (const entry of value) {
    if (!record(entry)
      || !boundedText(entry.workspaceId, 256)
      || !boundedText(entry.conversationId, 256)
      || !boundedText(entry.requestId, 200)
      || !Array.isArray(entry.content) || entry.content.length === 0 || entry.content.length > 256
      || !Array.isArray(entry.resourceRefs) || entry.resourceRefs.length > 16
      || !(entry.agentBindingId === null || boundedText(entry.agentBindingId, 256))
      || !["AUTO", "SIMPLE", "RICH"].includes(String(entry.presentationPreference))
      || !Array.isArray(entry.attachmentLabels) || entry.attachmentLabels.length > 256
      || entry.attachmentLabels.some(label => !boundedText(label, 256))) {
      throw new Error("Pending Conversation sends are invalid.");
    }
    const workspaceId = entry.workspaceId;
    const conversationId = entry.conversationId;
    const requestId = entry.requestId;
    let totalTextBytes = 0;
    const content = entry.content.map(block => {
      if (!record(block)) throw new Error("Pending Conversation content is invalid.");
      if (block.kind === "TEXT" && Object.keys(block).sort().join(",") === "kind,text"
        && typeof block.text === "string" && block.text.length <= 262_144) {
        totalTextBytes += new TextEncoder().encode(block.text).byteLength;
        if (totalTextBytes > 1_048_576) throw new Error("Pending Conversation content is too large.");
        return { kind: "TEXT" as const, text: block.text };
      }
      if (block.kind === "RESOURCE"
        && Object.keys(block).every(key => ["kind", "resource_ref", "display_name"].includes(key))
        && record(block.resource_ref)
        && Object.keys(block.resource_ref).every(key => ["workspace_id", "resource_id", "revision_id"].includes(key))
        && Object.hasOwn(block.resource_ref, "workspace_id")
        && Object.hasOwn(block.resource_ref, "resource_id")
        && boundedText(block.resource_ref.workspace_id, 256)
        && block.resource_ref.workspace_id === workspaceId
        && boundedText(block.resource_ref.resource_id, 256)
        && (block.resource_ref.revision_id === undefined
          || block.resource_ref.revision_id === null
          || boundedText(block.resource_ref.revision_id, 256))
        && (block.display_name === undefined
          || block.display_name === null
          || (typeof block.display_name === "string" && block.display_name.length <= 512 && !/[\u0000-\u001f]/.test(block.display_name)))) {
        return { ...block, kind: "RESOURCE" as const };
      }
      throw new Error("Pending Conversation content is invalid.");
    });
    const resourceRefs = entry.resourceRefs.map(reference => {
      if (!record(reference) || !boundedText(reference.workspace_id, 256)
        || reference.workspace_id !== workspaceId
        || !boundedText(reference.resource_id, 256)
        || Object.keys(reference).some(key => !["workspace_id", "resource_id", "revision_id"].includes(key))
        || !(reference.revision_id === undefined || reference.revision_id === null || boundedText(reference.revision_id, 256))) {
        throw new Error("Pending Conversation Resource references are invalid.");
      }
      return {
        workspace_id: reference.workspace_id,
        resource_id: reference.resource_id,
        ...(typeof reference.revision_id === "string" || reference.revision_id === null
          ? { revision_id: reference.revision_id }
          : {}),
      };
    });
    if (entry.attachmentLabels.length !== resourceRefs.length) {
      throw new Error("Pending Conversation attachment labels do not match its Resource references.");
    }
    const key = `${workspaceId}\u0000${conversationId}`;
    if (requestIds.has(requestId) || conversations.has(key)) {
      throw new Error("Pending Conversation sends contain duplicate identities.");
    }
    requestIds.add(requestId);
    conversations.add(key);
    sends.push({
      workspaceId,
      conversationId,
      requestId,
      content,
      resourceRefs,
      agentBindingId: typeof entry.agentBindingId === "string" ? entry.agentBindingId : null,
      presentationPreference: entry.presentationPreference as PendingConversationSend["presentationPreference"],
      attachmentLabels: entry.attachmentLabels as string[],
    });
  }
  return sends;
}
const boundedText = (value: unknown, max = 256): value is string => typeof value === "string" && value.length > 0 && value.length <= max && !/[\u0000-\u001f]/.test(value);
const sha256Digest = (value: unknown): value is string => typeof value === "string" && /^sha256:[a-f0-9]{64}$/.test(value);
const conversationTurnStatuses: readonly ConversationTurnStatus[] = [
  "OPEN", "RUNNING", "WAITING_USER", "WAITING_DEPENDENCY", "CANCEL_REQUESTED",
  "COMPLETED", "FAILED", "CANCELLED",
];

function parseConversationResourceRefs(value: unknown, workspaceId: string): ConversationResourceRefView[] {
  if (value === undefined) return [];
  if (!Array.isArray(value) || value.length > 128) throw new Error("Conversation resource references are invalid.");
  return value.map((reference) => {
    if (!record(reference)
      || !(Object.keys(reference).sort().join(",") === "resource_id,workspace_id"
        || Object.keys(reference).sort().join(",") === "resource_id,revision_id,workspace_id")
      || reference.workspace_id !== workspaceId || !boundedText(reference.resource_id)
      || !(reference.revision_id === undefined || reference.revision_id === null || boundedText(reference.revision_id))) {
      throw new Error("Conversation resource references are invalid.");
    }
    return {
      workspace_id: workspaceId,
      resource_id: reference.resource_id,
      revision_id: typeof reference.revision_id === "string" ? reference.revision_id : null,
    };
  });
}

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
    || !(value.active_turn === null || parseActiveTurn(value.active_turn) !== null)
    || !(value.retryable_turn === undefined || value.retryable_turn === null || parseRetryableTurn(value.retryable_turn) !== null)
    || !(value.activity_turn === undefined || value.activity_turn === null || parseActivityTurn(value.activity_turn) !== null)
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
      || Object.keys(entry).some(key => !["message", "rich_presentation", "linked_items", "turn_status"].includes(key))
      || !Array.isArray(entry.linked_items) || entry.linked_items.length > 200) throw new Error("Conversation message is invalid.");
    const message = entry.message;
    if (message.conversation_id !== conversationId || !boundedText(message.message_id)
      || !["USER", "AGENT", "SYSTEM_NOTICE", "CHANNEL"].includes(String(message.role))
      || !(message.agent_binding_id === undefined || message.agent_binding_id === null || boundedText(message.agent_binding_id))
      || !(message.agent_session_id === undefined || message.agent_session_id === null || boundedText(message.agent_session_id))
      || !(message.turn_id === undefined || message.turn_id === null || boundedText(message.turn_id))
      || !boundedText(message.created_at, 64) || !Array.isArray(message.content) || message.content.length > 128) {
      throw new Error("Conversation history contains an invalid message.");
    }
    const turnStatus = entry.turn_status === undefined || entry.turn_status === null
      ? null
      : conversationTurnStatuses.includes(entry.turn_status as ConversationTurnStatus)
        ? entry.turn_status as ConversationTurnStatus
        : undefined;
    if (turnStatus === undefined) throw new Error("Conversation message is invalid.");
    const content = message.content.map((block) => {
      if (!record(block)) throw new Error("Conversation content is invalid.");
      if (block.kind === "TEXT" && typeof block.text === "string" && block.text.length <= 262_144) return { kind: "TEXT" as const, text: block.text };
      if (block.kind === "RESOURCE") return { kind: "RESOURCE" as const };
      throw new Error("Conversation contains unsupported content.");
    });
    return {
      message_id: message.message_id,
      role: message.role as ConversationMessageView["role"],
      agent_binding_id: typeof message.agent_binding_id === "string" ? message.agent_binding_id : null,
      agent_session_id: typeof message.agent_session_id === "string" ? message.agent_session_id : null,
      turn_id: typeof message.turn_id === "string" ? message.turn_id : null,
      turn_status: turnStatus,
      created_at: message.created_at,
      content,
      resource_refs: parseConversationResourceRefs(message.resource_refs, workspaceId),
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
    active_turn: value.active_turn === null ? null : parseActiveTurn(value.active_turn),
    retryable_turn: value.retryable_turn === undefined || value.retryable_turn === null ? null : parseRetryableTurn(value.retryable_turn),
    activity_turn: value.activity_turn === undefined
      ? undefined
      : value.activity_turn === null ? null : parseActivityTurn(value.activity_turn),
  };
}

/**
 * Read identity for the latest REPORTED activity page. This is deliberately separate
 * from active/retryable control projections; old daemon and local-empty snapshots fall
 * back to those existing identities until the new snapshot field is present.
 */
export function activityTurnForRead(snapshot: ConversationSnapshotView): ConversationActivityTurnView | null {
  if (snapshot.activity_turn !== undefined) return snapshot.activity_turn;
  if (snapshot.active_turn) {
    return {
      turn_id: snapshot.active_turn.turn_id,
      retry_ordinal: snapshot.active_turn.retry_ordinal,
      status: snapshot.active_turn.status as ConversationActivityTurnView["status"],
    };
  }
  if (snapshot.retryable_turn) {
    return {
      turn_id: snapshot.retryable_turn.turn_id,
      retry_ordinal: snapshot.retryable_turn.retry_ordinal,
      status: "FAILED",
    };
  }
  return null;
}

const NONTERMINAL = ["OPEN", "RUNNING", "WAITING_USER", "WAITING_DEPENDENCY", "CANCEL_REQUESTED"];

function parseActiveTurn(value: unknown): ActiveTurnView | null {
  if (!record(value) || !boundedText(value.turn_id) || !Number.isSafeInteger(value.version) || (value.version as number) < 1
    || !NONTERMINAL.includes(String(value.status))
    || !Number.isSafeInteger(value.retry_ordinal) || (value.retry_ordinal as number) < 0) return null;
  return {
    turn_id: value.turn_id,
    version: value.version as number,
    status: value.status as string,
    retry_ordinal: value.retry_ordinal as number,
    ...(typeof value.presentation_preference === "string" ? { presentation_preference: value.presentation_preference } : {}),
  };
}

function parseRetryableTurn(value: unknown): RetryableTurnView | null {
  if (!record(value) || !boundedText(value.turn_id) || !Number.isSafeInteger(value.version) || (value.version as number) < 1
    || !Number.isSafeInteger(value.retry_ordinal) || (value.retry_ordinal as number) < 0
    || !(value.reason_code === null || boundedText(value.reason_code, 128))) return null;
  return { turn_id: value.turn_id, version: value.version as number, retry_ordinal: value.retry_ordinal as number, reason_code: value.reason_code as string | null };
}

function parseActivityTurn(value: unknown): ConversationActivityTurnView | null {
  const statuses = ["OPEN", "RUNNING", "WAITING_USER", "WAITING_DEPENDENCY", "CANCEL_REQUESTED", "COMPLETED", "FAILED", "CANCELLED"];
  if (!record(value) || !boundedText(value.turn_id)
    || !Number.isSafeInteger(value.retry_ordinal) || (value.retry_ordinal as number) < 0
    || !statuses.includes(String(value.status))) return null;
  return {
    turn_id: value.turn_id,
    retry_ordinal: value.retry_ordinal as number,
    status: value.status as ConversationActivityTurnView["status"],
  };
}
