import test from "node:test";
import assert from "node:assert/strict";
import { parseConversationList, parseConversationSnapshot } from "../src/conversations/conversation-view.ts";

test("Conversation list accepts only records pinned to the selected Workspace", () => {
  const row = { conversation_id: "conversation-1", workspace_id: "workspace-1", title: "Notes", active_agent_binding_id: null, version: 1, created_at: "2026-10-09T12:00:00Z" };
  assert.equal(parseConversationList({ items: [row] }, "workspace-1")[0]?.conversation_id, "conversation-1");
  assert.throws(() => parseConversationList({ items: [{ ...row, workspace_id: "workspace-2" }] }, "workspace-1"), /out-of-Workspace/);
});

test("Conversation snapshot preserves committed semantic text and rejects message identity drift", () => {
  const message = { message_id: "message-1", conversation_id: "conversation-1", role: "AGENT", created_at: "2026-10-09T12:00:00Z", content: [{ kind: "TEXT", text: "Saved answer" }] };
  const snapshot = { workspace_id: "workspace-1", conversation_id: "conversation-1", items: [{ message, rich_presentation: null, linked_items: [] }], active_turn: null };
  assert.equal(parseConversationSnapshot(snapshot, "workspace-1", "conversation-1").messages[0]?.content[0]?.kind, "TEXT");
  assert.throws(() => parseConversationSnapshot({ ...snapshot, items: [{ ...snapshot.items[0], message: { ...message, conversation_id: "conversation-2" } }] }, "workspace-1", "conversation-1"), /invalid message/);
});

test("Conversation snapshot retains a bounded RichPresentation reference pinned to its message", () => {
  const message = { message_id: "message-1", conversation_id: "conversation-1", role: "AGENT", created_at: "2026-10-09T12:00:00Z", content: [{ kind: "TEXT", text: "Saved answer" }] };
  const rich = {
    presentation_id: "presentation-1", message_id: "message-1", schema_version: 1,
    renderer_contract_version: 1, semantic_content_digest: `sha256:${"a".repeat(64)}`,
    document_digest: `sha256:${"b".repeat(64)}`, document_size_bytes: 128, availability: "AVAILABLE",
  };
  const entry = { message, rich_presentation: rich, linked_items: [] };
  const snapshot = { workspace_id: "workspace-1", conversation_id: "conversation-1", items: [entry], active_turn: null };
  assert.deepEqual(parseConversationSnapshot(snapshot, "workspace-1", "conversation-1").messages[0]?.rich_presentation, rich);
  assert.throws(() => parseConversationSnapshot({ ...snapshot, items: [{ ...entry, rich_presentation: { ...rich, message_id: "message-2" } }] }, "workspace-1", "conversation-1"), /RichPresentation reference/);
  assert.throws(() => parseConversationSnapshot({ ...snapshot, items: [{ ...entry, rich_presentation: { ...rich, document_size_bytes: 2_000_000 } }] }, "workspace-1", "conversation-1"), /RichPresentation reference/);
  assert.throws(() => parseConversationSnapshot({ ...snapshot, items: [{ ...entry, rich_presentation: { ...rich, forged_status: true } }] }, "workspace-1", "conversation-1"), /RichPresentation reference/);
});
