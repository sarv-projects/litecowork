import assert from "node:assert/strict";
import test from "node:test";
import * as attachmentModule from "../src/conversations/conversation-attachments.ts";
import { addAttachment, attachmentChipLabel, attachmentRefusalNotice, isAttachable, MAX_ATTACHMENTS, removeAttachment, toResourceRefs } from "../src/conversations/conversation-attachments.ts";

const candidate = (id: string, over: Record<string, unknown> = {}) => ({
  resourceId: id, workspaceId: "w", resourceRevisionId: `rev-${id}`, displayName: `${id}.txt`, mediaType: "text/plain", sizeBytes: 10, ...over,
});

test("only small text Resources from the selected Workspace become chips, once each", () => {
  let chips = addAttachment([], candidate("a"), "w");
  chips = addAttachment(chips, candidate("a"), "w");
  assert.equal(chips.length, 1, "duplicate ignored");
  assert.equal(addAttachment(chips, candidate("b", { workspaceId: "other" }), "w").length, 1, "foreign Workspace ignored");
  assert.equal(addAttachment(chips, candidate("c", { mediaType: "image/png" }), "w").length, 1, "binary not offered");
  assert.equal(addAttachment(chips, candidate("d", { sizeBytes: 128 * 1024 + 1 }), "w").length, 1, "over the bound not offered");
  assert.equal(isAttachable(candidate("e", { mediaType: "application/json" })), true);
});

test("chips are bounded, removable and carry an accessible name", () => {
  let chips: ReturnType<typeof addAttachment> = [];
  for (let n = 0; n < MAX_ATTACHMENTS + 3; n += 1) chips = addAttachment(chips, candidate(`r${n}`), "w");
  assert.equal(chips.length, MAX_ATTACHMENTS);
  assert.equal(removeAttachment(chips, "r0").length, MAX_ATTACHMENTS - 1);
  assert.equal(attachmentChipLabel(chips[0]!), "Remove attachment r0.txt");
});

test("wire refs pin the exact revision and drop chips from another Workspace", () => {
  const chips = [candidate("a"), candidate("b", { workspaceId: "other" })];
  assert.deepEqual(toResourceRefs(chips, "w"), [{ workspace_id: "w", resource_id: "a", revision_id: "rev-a" }]);
});

test("daemon refusals map to calm notices and unknown ones fall through", () => {
  assert.match(attachmentRefusalNotice("An attached Resource changed; reattach its current revision")!, /changed/);
  assert.match(attachmentRefusalNotice("Only text Resources can be attached to a Conversation turn")!, /text files/);
  assert.equal(attachmentRefusalNotice("Local Conversation service is unavailable"), null);
});

test("paged picker results keep first-seen revisions and ignore ineligible or duplicate resources", () => {
  const merge = attachmentModule.mergeAttachmentCandidates;
  assert.deepEqual(merge?.(
    [candidate("a"), candidate("b")],
    [candidate("b", { resourceRevisionId: "rev-b-new" }), candidate("c"), candidate("foreign", { workspaceId: "other" }), candidate("binary", { mediaType: "image/png" })],
    "w",
  ), [candidate("a"), candidate("b"), candidate("c")]);
});

test("picker cursor must advance and may not revisit a successfully consumed cursor", () => {
  const classify = attachmentModule.classifyAttachmentCursor;
  assert.equal(classify?.("cursor-a", null, ["cursor-a"]), "EXHAUSTED");
  assert.equal(classify?.("cursor-a", "cursor-b", ["cursor-a"]), "MORE");
  assert.equal(classify?.("cursor-a", "cursor-a", ["cursor-a"]), "STALLED");
  assert.equal(classify?.("cursor-c", "cursor-a", ["cursor-a", "cursor-c"]), "STALLED");
  assert.equal(classify?.("cursor-c", "界".repeat(683), []), "STALLED", "the daemon enforces the cursor limit in UTF-8 bytes");
});
