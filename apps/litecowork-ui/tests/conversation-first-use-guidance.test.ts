import assert from "node:assert/strict";
import test from "node:test";

const guidanceModule = await import("../src/conversations/first-use-guidance.ts").catch(() => null);

test("first-use conversation guidance directs users to the missing prerequisite and keeps drafts local", () => {
  assert.equal(typeof guidanceModule?.firstUseConversationGuidance, "function");
  const guidance = guidanceModule!.firstUseConversationGuidance;

  const workspaceRequired = guidance({ workspaceAvailable: false, operatorReady: false });
  assert.equal(workspaceRequired.state, "WORKSPACE_REQUIRED");
  assert.equal(workspaceRequired.actionLabel, "Choose a Workspace in Settings");
  assert.match(workspaceRequired.heading, /Workspace.*load conversations/i);
  assert.match(workspaceRequired.message, /check whether any conversations are saved/i);
  assert.match(workspaceRequired.message, /open window/i);

  const runtimeRequired = guidance({ workspaceAvailable: true, operatorReady: false });
  assert.equal(runtimeRequired.state, "RUNTIME_REQUIRED");
  assert.equal(runtimeRequired.actionLabel, "Open Runtime settings");
  assert.match(runtimeRequired.heading, /Reconnect.*saved conversations/i);
  assert.match(runtimeRequired.message, /load this Workspace.*history or send messages/i);
  assert.match(runtimeRequired.message, /draft/i);
  assert.match(runtimeRequired.message, /open window/i);

  const sendingUnavailable = guidance({ workspaceAvailable: true, operatorReady: true });
  assert.equal(sendingUnavailable.state, "SENDING_NOT_AVAILABLE");
  assert.equal(sendingUnavailable.heading, "Conversation sending isn't available yet");
  assert.equal(sendingUnavailable.actionLabel, null);
  assert.match(sendingUnavailable.message, /draft/i);
  assert.match(sendingUnavailable.message, /opening Settings will not send it automatically/i);
  assert.match(sendingUnavailable.message, /current window/i);
  assert.match(sendingUnavailable.message, /not saved or sent/i);
});
