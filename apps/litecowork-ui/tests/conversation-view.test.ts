import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { appendConversationPage, appendConversationListPage, ConversationCreateRequestCache, ConversationRequestEpochs, ConversationWorkspaceSelectionEpochs, mergeConversationStreamSnapshot, parseConversationList, parseConversationListPage, parseConversationSnapshot, richPresentationCacheKey } from "../src/conversations/conversation-view.ts";

test("late Conversation create response is discarded after an A to B to A Workspace switch", async () => {
  const selections = new ConversationWorkspaceSelectionEpochs();
  selections.select("workspace-1");
  const requestSelection = selections.capture("workspace-1");
  assert.ok(requestSelection);

  let resolveResponse!: (conversationId: string) => void;
  const response = new Promise<string>(resolve => { resolveResponse = resolve; });
  let selectedConversationId: string | null = null;
  const projection = response.then(conversationId => {
    if (selections.isCurrent(requestSelection)) selectedConversationId = conversationId;
  });

  selections.select("workspace-2");
  selections.select("workspace-1");
  resolveResponse("stale-conversation");
  await projection;

  assert.equal(selections.isCurrent(requestSelection), false);
  assert.equal(selectedConversationId, null, "the stale response must not select a Conversation after returning to its original Workspace");
});

test("snapshot selection does not invalidate the initial Conversation list request", () => {
  const requests = new ConversationRequestEpochs();
  const listRequest = requests.beginList();
  requests.beginSnapshot();
  requests.invalidateSnapshot();
  assert.equal(requests.isCurrentList(listRequest), true);
});

test("Conversation snapshots preserve their bounded pagination cursor", () => {
  const message = { message_id: "message-1", conversation_id: "conversation-1", role: "AGENT", created_at: "2026-10-09T12:00:00Z", content: [{ kind: "TEXT", text: "Saved answer" }] };
  const snapshot = { workspace_id: "workspace-1", conversation_id: "conversation-1", projection_revision: 1, stream_cursor: "conversation:conversation-1:1", origin_sequence: "0", items: [{ message, rich_presentation: null, linked_items: [] }], next_cursor: "message-1", active_turn: null };
  assert.equal(parseConversationSnapshot(snapshot, "workspace-1", "conversation-1").next_cursor, "message-1");
  assert.equal(parseConversationSnapshot({ ...snapshot, next_cursor: null }, "workspace-1", "conversation-1").next_cursor, null);
  const { next_cursor: _nextCursor, ...snapshotWithoutCursor } = snapshot;
  assert.throws(() => parseConversationSnapshot(snapshotWithoutCursor, "workspace-1", "conversation-1"), /pagination cursor/);
  assert.throws(() => parseConversationSnapshot({ ...snapshot, next_cursor: "../foreign" }, "workspace-1", "conversation-1"), /pagination cursor/);
});

test("Conversation snapshots preserve a decimal source frontier separately from pagination", () => {
  const message = { message_id: "message-1", conversation_id: "conversation-1", role: "AGENT", created_at: "2026-10-09T12:00:00Z", content: [{ kind: "TEXT", text: "Saved answer" }] };
  const snapshot = {
    workspace_id: "workspace-1", conversation_id: "conversation-1",
    projection_revision: 7, stream_cursor: "conversation:conversation-1:7", origin_sequence: "9007199254740993",
    items: [{ message, rich_presentation: null, linked_items: [] }], next_cursor: "message-1", active_turn: null,
  };
  const parsed = parseConversationSnapshot(snapshot, "workspace-1", "conversation-1");
  assert.equal(parsed.origin_sequence, "9007199254740993");
  assert.equal(parsed.next_cursor, "message-1");
  assert.equal(parsed.stream_cursor, "conversation:conversation-1:7");
  assert.equal(parsed.projection_revision, 7);
  assert.throws(() => parseConversationSnapshot({ ...snapshot, origin_sequence: 9007199254740993 }, "workspace-1", "conversation-1"), /source frontier/);
});

test("Conversation pagination appends unseen messages in order and fences mismatched histories", () => {
  const message = (message_id: string) => ({ message_id, role: "AGENT" as const, created_at: "2026-10-09T12:00:00Z", content: [{ kind: "TEXT" as const, text: message_id }], rich_presentation: null });
  const base = { workspace_id: "workspace-1", conversation_id: "conversation-1", messages: [message("message-1"), message("message-2")], next_cursor: "message-2", active_turn: null };
  const page = { ...base, messages: [message("message-2"), message("message-3"), message("message-3")], next_cursor: null };
  const appended = appendConversationPage(base, page);
  assert.deepEqual(appended.messages.map(item => item.message_id), ["message-1", "message-2", "message-3"]);
  assert.equal(appended.next_cursor, null);
  assert.throws(() => appendConversationPage(base, { ...page, conversation_id: "conversation-2" }), /does not match/);
});

test("live Conversation refresh preserves the deeper loaded history cursor", () => {
  const message = (message_id: string, second: string) => ({ message_id, role: "AGENT" as const, created_at: `2026-10-09T12:00:${second}Z`, content: [{ kind: "TEXT" as const, text: message_id }], rich_presentation: null });
  const current = {
    workspace_id: "workspace-1", conversation_id: "conversation-1", stream_cursor: "page-150",
    messages: [message("message-001", "01"), message("message-100", "02"), message("message-150", "03")],
    next_cursor: "message-150", active_turn: null,
  };
  const live = {
    workspace_id: "workspace-1", conversation_id: "conversation-1",
    messages: [message("message-001", "01"), message("message-100", "02")],
    next_cursor: "message-100", active_turn: null,
  };
  const merged = mergeConversationStreamSnapshot(current, live, "workspace-1", "conversation-1");
  assert.deepEqual(merged.messages.map(item => item.message_id), ["message-001", "message-100", "message-150"]);
  assert.equal(merged.next_cursor, "message-150");
});

test("live snapshot rejects changed content for a committed Message ID without advancing accepted state", () => {
  const reference = {
    presentation_id: "presentation-1", message_id: "message-1", schema_version: 1 as const,
    renderer_contract_version: 1, semantic_content_digest: `sha256:${"a".repeat(64)}`,
    document_digest: `sha256:${"b".repeat(64)}`, document_size_bytes: 128, availability: "AVAILABLE" as const,
  };
  const message = (text: string) => ({
    message_id: "message-1", role: "AGENT" as const, created_at: "2026-10-09T12:00:00Z",
    content: [{ kind: "TEXT" as const, text }], rich_presentation: reference,
  });
  const current = {
    workspace_id: "workspace-1", conversation_id: "conversation-1", stream_cursor: "accepted-cursor",
    messages: [message("Original semantic answer")], next_cursor: "message-1", active_turn: null,
  };
  const live = {
    ...current, stream_cursor: "inconsistent-cursor", origin_sequence: "2",
    messages: [message("Changed text with the same immutable RichPresentation reference")], next_cursor: null,
  };

  const merged = mergeConversationStreamSnapshot(current, live, "workspace-1", "conversation-1");

  assert.equal(merged, current, "an inconsistent committed Message update must preserve the accepted snapshot");
  assert.equal(merged.messages[0]?.content[0]?.kind === "TEXT" ? merged.messages[0].content[0].text : null, "Original semantic answer");
  assert.equal(merged.stream_cursor, "accepted-cursor");
  assert.equal(merged.next_cursor, "message-1");
});

test("live Conversation refresh keeps an exhausted history exhausted", () => {
  const current = {
    workspace_id: "workspace-1", conversation_id: "conversation-1",
    projection_revision: 10, stream_cursor: "current", origin_sequence: "10",
    messages: [], next_cursor: null, active_turn: null,
  };
  const live = {
    workspace_id: "workspace-1", conversation_id: "conversation-1",
    projection_revision: 9, stream_cursor: "older", origin_sequence: "9",
    messages: [], next_cursor: "message-100", active_turn: null,
  };
  const merged = mergeConversationStreamSnapshot(current, live, "workspace-1", "conversation-1");
  assert.equal(merged.next_cursor, null);
});

test("Conversation list accepts only records pinned to the selected Workspace", () => {
  const row = { conversation_id: "conversation-1", workspace_id: "workspace-1", title: "Notes", active_agent_binding_id: null, version: 1, created_at: "2026-10-09T12:00:00Z" };
  assert.equal(parseConversationList({ items: [row], next_cursor: null }, "workspace-1")[0]?.conversation_id, "conversation-1");
  assert.throws(() => parseConversationList({ items: [{ ...row, workspace_id: "workspace-2" }], next_cursor: null }, "workspace-1"), /out-of-Workspace/);
});

test("Conversation catalog pages retain and validate the opaque next cursor", () => {
  const row = { conversation_id: "conversation-1", workspace_id: "workspace-1", title: "Notes", active_agent_binding_id: null, version: 1, created_at: "2026-10-09T12:00:00Z" };
  const page = parseConversationListPage({ items: [row], next_cursor: "eyJ3b3Jrc3BhY2VfaWQiOiJ3b3Jrc3BhY2UtMSJ9" }, "workspace-1");
  assert.equal(page.items[0]?.conversation_id, "conversation-1");
  assert.equal(page.next_cursor, "eyJ3b3Jrc3BhY2VfaWQiOiJ3b3Jrc3BhY2UtMSJ9");
  assert.equal(parseConversationListPage({ items: [row], next_cursor: null }, "workspace-1").next_cursor, null);
  assert.throws(() => parseConversationListPage({ items: [row] }, "workspace-1"), /cursor/);
  assert.throws(() => parseConversationListPage({ items: [row], next_cursor: "../other" }, "workspace-1"), /cursor/);
  assert.throws(() => parseConversationListPage({ items: [{ ...row, workspace_id: "workspace-2" }], next_cursor: null }, "workspace-1"), /out-of-Workspace/);
});

test("Conversation catalog page append keeps newest-first order and deduplicates records", () => {
  const row = (conversation_id: string) => ({ conversation_id, workspace_id: "workspace-1", title: null, active_agent_binding_id: null, version: 1, created_at: "2026-10-09T12:00:00Z" });
  const first = { items: [row("newest"), row("shared")], next_cursor: "older-cursor" };
  const second = { items: [row("shared"), row("oldest")], next_cursor: null };
  const merged = appendConversationListPage(first, second);
  assert.deepEqual(merged.items.map(item => item.conversation_id), ["newest", "shared", "oldest"]);
  assert.equal(merged.next_cursor, null);
});

test("ambiguous Conversation create retries reuse identity only for the same Workspace and title", () => {
  let allocations = 0;
  const makeId = () => `request-${++allocations}`;
  const requests = new ConversationCreateRequestCache();
  const first = requests.get("workspace-1", "Draft", makeId);
  const retry = requests.get("workspace-1", "Draft", makeId);
  const changedTitle = requests.get("workspace-1", "Renamed", makeId);
  const changedWorkspace = requests.get("workspace-2", "Draft", makeId);
  const returnToAmbiguousDraft = requests.get("workspace-1", "Draft", makeId);
  assert.equal(first.requestId, "request-1");
  assert.equal(retry.requestId, first.requestId);
  assert.equal(changedTitle.requestId, "request-2");
  assert.equal(changedWorkspace.requestId, "request-3");
  assert.equal(returnToAmbiguousDraft.requestId, first.requestId);
  requests.complete(first);
  assert.equal(requests.get("workspace-1", "Draft", makeId).requestId, "request-4");
});

test("pending Conversation create identity restores from a persisted journal after frontend restart", () => {
  const firstProcess = new ConversationCreateRequestCache();
  const original = firstProcess.get("workspace-1", "Draft", () => "request-durable-1");
  const journalSnapshot = JSON.parse(JSON.stringify([original]));

  const restartedProcess = new ConversationCreateRequestCache();
  restartedProcess.replace(journalSnapshot);
  const retry = restartedProcess.get("workspace-1", "Draft", () => "request-must-not-replace-durable-key");

  assert.equal(retry.requestId, "request-durable-1");
  assert.equal(retry.workspaceId, "workspace-1");
  assert.equal(retry.title, "Draft");
});

test("pending Conversation acknowledgement removes only an exact Workspace and request identity", () => {
  const requests = new ConversationCreateRequestCache();
  const workspaceOne = requests.get("workspace-1", "Draft", () => "request-shared");
  const workspaceTwo = requests.get("workspace-2", "Draft", () => "request-shared");

  assert.equal(requests.completeExact("workspace-2", "request-shared"), true);
  assert.equal(requests.get("workspace-1", "Draft", () => "must-not-replace"), workspaceOne);
  assert.notEqual(requests.get("workspace-2", "Draft", () => "request-new"), workspaceTwo);
  assert.equal(requests.completeExact("workspace-1", "request-shared"), true);
  assert.equal(requests.get("workspace-1", "Draft", () => "request-after-ack").requestId, "request-after-ack");
  assert.equal(requests.completeExact("workspace-1", "missing"), false);
});

test("pending Conversation create journal replacement rejects malformed entries without dropping old retries", () => {
  const requests = new ConversationCreateRequestCache();
  const original = requests.get("workspace-1", "Draft", () => "request-original");

  assert.throws(() => requests.replace([{ workspaceId: "workspace-2", title: "Changed", requestId: "bad\nkey" }]), /invalid pending/);
  assert.equal(requests.get("workspace-1", "Draft", () => "request-new").requestId, original.requestId);
});

test("pending Conversation create receipts stay bounded without evicting ambiguous identities", () => {
  const requests = new ConversationCreateRequestCache();
  let allocations = 0;
  const makeId = () => `request-${++allocations}`;
  for (let index = 0; index < 64; index += 1) requests.get("workspace-1", `Draft ${index}`, makeId);
  const first = requests.get("workspace-1", "Draft 0", makeId);
  assert.equal(first.requestId, "request-1");
  assert.throws(() => requests.get("workspace-1", "Draft 64", makeId), /Resolve a pending/);
  requests.complete(first);
  assert.equal(requests.get("workspace-1", "Draft 64", makeId).requestId, "request-65");
});

test("RichPresentation cache bindings are stable across equivalent live snapshot objects and reject invalidated refs", () => {
  const reference = {
    presentation_id: "presentation-1", message_id: "message-1", schema_version: 1 as const,
    renderer_contract_version: 1, semantic_content_digest: `sha256:${"a".repeat(64)}`,
    document_digest: `sha256:${"b".repeat(64)}`, document_size_bytes: 128, availability: "AVAILABLE" as const,
  };
  const key = richPresentationCacheKey("workspace-1", "conversation-1", "message-1", reference);
  assert.ok(key);
  assert.equal(richPresentationCacheKey("workspace-1", "conversation-1", "message-1", { ...reference }), key);
  assert.notEqual(richPresentationCacheKey("workspace-2", "conversation-1", "message-1", reference), key);
  assert.equal(richPresentationCacheKey("workspace-1", "conversation-1", "message-1", { ...reference, availability: "POLICY_OMITTED" }), null);
  assert.equal(richPresentationCacheKey("workspace-1", "conversation-1", "message-1", { ...reference, availability: "INTEGRITY_FAILED" }), null);
  assert.notEqual(richPresentationCacheKey("workspace-1", "conversation-1", "message-1", reference), richPresentationCacheKey("workspace-1", "conversation-1", "message-1", { ...reference, document_digest: `sha256:${"c".repeat(64)}` }));
});

test("Conversation UI wires stable rich bindings, Workspace response fencing, retry identity, and cursor pagination", () => {
  const page = readFileSync(new URL("../src/conversations/ConversationsPage.tsx", import.meta.url), "utf8");
  assert.match(page, /richPresentationCacheKey\(workspaceId, conversationId, message\.message_id, reference\)/);
  assert.match(page, /\}, \[cacheKey, conversationId, message\.message_id, presentationId, requestedKey, retry, semanticContentDigest, text, workspaceId\]\)/);
  assert.match(page, /auto-load message may move out of the recent viewport set while its fetch is in/);
  assert.doesNotMatch(page, /\}, \[[^\]]*\breference\b[^\]]*\]\)/);
  assert.match(page, /conversationWorkspaceSelections\.isCurrent\(requestSelection\)/, "create UI updates are fenced to the captured Workspace selection epoch");
  assert.match(page, /useLayoutEffect\(\(\) => \{\s*conversationWorkspaceSelections\.select\(workspaceId\)/, "the shared selection epoch updates when the active page commits a Workspace switch");
  assert.ok([...page.matchAll(/isCurrentCreateRequest\(\)/g)].length >= 7, "create response, acknowledgement, and recovery updates use the current request fence");
  assert.match(page, /requestId: request\.requestId/);
  assert.match(page, /pendingCreates\.get\(workspaceId, normalizedTitle/);
  assert.match(page, /pendingCreates\.completeExact\(request\.workspaceId, request\.requestId\)/);
  assert.match(page, /pendingCreates\.remember\(request\)/);
  assert.match(page, /Retry checking pending Conversation requests/);
  assert.match(page, /list_pending_conversation_creates/);
  assert.match(page, /item\.workspaceId !== request\.workspaceId \|\| item\.requestId !== request\.requestId/);
  assert.match(page, /list_conversations", \{ workspaceId: requestWorkspaceId, cursor \}/);
  assert.match(page, /appendConversationListPage\(/);

  const bridge = readFileSync(new URL("../src-tauri/src/conversation_bridge.rs", import.meta.url), "utf8");
  assert.match(bridge, /pub\(crate\) async fn list_conversations\([\s\S]*?cursor: Option<String>/);
  assert.match(bridge, /request = request\.query\(&\[\("cursor", cursor\)\]\)/);
  assert.match(bridge, /fn valid_cursor\(/);
});

test("Conversation snapshot preserves committed semantic text and rejects message identity drift", () => {
  const message = { message_id: "message-1", conversation_id: "conversation-1", role: "AGENT", created_at: "2026-10-09T12:00:00Z", content: [{ kind: "TEXT", text: "Saved answer" }] };
  const snapshot = { workspace_id: "workspace-1", conversation_id: "conversation-1", projection_revision: 1, stream_cursor: "conversation:conversation-1:1", origin_sequence: "0", items: [{ message, rich_presentation: null, linked_items: [] }], next_cursor: null, active_turn: null };
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
  const snapshot = { workspace_id: "workspace-1", conversation_id: "conversation-1", projection_revision: 1, stream_cursor: "conversation:conversation-1:1", origin_sequence: "0", items: [entry], next_cursor: null, active_turn: null };
  assert.deepEqual(parseConversationSnapshot(snapshot, "workspace-1", "conversation-1").messages[0]?.rich_presentation, rich);
  assert.throws(() => parseConversationSnapshot({ ...snapshot, items: [{ ...entry, rich_presentation: { ...rich, message_id: "message-2" } }] }, "workspace-1", "conversation-1"), /RichPresentation reference/);
  assert.throws(() => parseConversationSnapshot({ ...snapshot, items: [{ ...entry, rich_presentation: { ...rich, document_size_bytes: 2_000_000 } }] }, "workspace-1", "conversation-1"), /RichPresentation reference/);
  assert.throws(() => parseConversationSnapshot({ ...snapshot, items: [{ ...entry, rich_presentation: { ...rich, forged_status: true } }] }, "workspace-1", "conversation-1"), /RichPresentation reference/);
});
