import { strict as assert } from "node:assert";
import { test } from "node:test";
import { shouldAutoSelectFirstConversation } from "../src/conversations/conversation-selection.ts";

test("default Conversation entry opens the latest chat unless a New Chat intent is active", () => {
  assert.equal(shouldAutoSelectFirstConversation(null, false), true);
  assert.equal(shouldAutoSelectFirstConversation(null, true), false);
  assert.equal(shouldAutoSelectFirstConversation("conversation-selected", false), false);
  assert.equal(shouldAutoSelectFirstConversation("conversation-selected", true), false);
});
