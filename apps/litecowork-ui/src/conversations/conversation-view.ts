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
  messages: ConversationMessageView[];
  active_turn: { status: string } | null;
};

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
    || !(value.active_turn === null || record(value.active_turn))) throw new Error("Conversation history is invalid.");
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
  return { workspace_id: workspaceId, conversation_id: conversationId, messages, active_turn: value.active_turn as { status: string } | null };
}
