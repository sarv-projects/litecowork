import assert from "node:assert/strict";
import { test } from "node:test";
import { focusArtifactEditorTarget, type ArtifactEditorFocusTarget } from "../src/artifacts/artifact-editor-focus.ts";

function target(connected = true) {
  let focused = false;
  const value: ArtifactEditorFocusTarget = { isConnected: connected, focus: () => { focused = true; } };
  return { value, focused: () => focused };
}

test("moves focus into the editor and returns to the same edit or restore action", () => {
  const editor = target();
  const edit = target();
  const restore = target();
  const status = target();
  const fallback = target();
  const targets = { editor: editor.value, edit: edit.value, restore: restore.value, publishedStatus: status.value, fallback: fallback.value };

  assert.equal(focusArtifactEditorTarget("EDITOR", targets), true);
  assert.equal(editor.focused(), true);
  assert.equal(focusArtifactEditorTarget("EDIT", targets), true);
  assert.equal(edit.focused(), true);
  assert.equal(focusArtifactEditorTarget("RESTORE", targets), true);
  assert.equal(restore.focused(), true);
  assert.equal(focusArtifactEditorTarget("PUBLISHED_STATUS", targets), true);
  assert.equal(status.focused(), true);
});

test("uses a stable heading when the original action or publication notice is no longer mounted", () => {
  const staleEdit = target(false);
  const staleRestore = target(false);
  const staleStatus = target(false);
  const fallback = target();
  const targets = {
    editor: null,
    edit: staleEdit.value,
    restore: staleRestore.value,
    publishedStatus: staleStatus.value,
    fallback: fallback.value,
  };

  assert.equal(focusArtifactEditorTarget("EDIT", targets), true);
  assert.equal(focusArtifactEditorTarget("RESTORE", targets), true);
  assert.equal(focusArtifactEditorTarget("PUBLISHED_STATUS", targets), true);
  assert.equal(fallback.focused(), true);
  assert.equal(focusArtifactEditorTarget("EDITOR", { ...targets, fallback: null }), false);
});
