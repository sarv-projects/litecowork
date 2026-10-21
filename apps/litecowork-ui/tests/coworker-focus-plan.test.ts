import assert from "node:assert/strict";
import { test } from "node:test";
import {
  coworkerStatusText,
  isCoworkerEditorDirty,
  resolveCoworkerEscape,
  resolveCoworkerFocus,
} from "../src/coworkers/coworker-focus-plan.ts";

test("Quick Create opens with keyboard focus on the name field", () => {
  assert.deepEqual(resolveCoworkerFocus({ type: "quick-create-opened" }), { kind: "coworker-name-input" });
  assert.deepEqual(resolveCoworkerFocus({ type: "edit-opened" }), { kind: "coworker-name-input" });
});

test("cancelling an editor returns focus to the control that opened it", () => {
  assert.deepEqual(
    resolveCoworkerFocus({ type: "editor-cancelled", editingCoworkerId: "coworker-7" }),
    { kind: "coworker-roster-item", coworkerId: "coworker-7" },
  );
  assert.deepEqual(
    resolveCoworkerFocus({ type: "editor-cancelled", editingCoworkerId: null }),
    { kind: "coworker-create-action" },
  );
});

test("a saved editor moves focus to the Coworker heading because the form is removed", () => {
  assert.deepEqual(resolveCoworkerFocus({ type: "editor-saved" }), { kind: "coworker-detail-heading" });
});

test("the archive confirmation takes focus and returns it to the archive action", () => {
  assert.deepEqual(resolveCoworkerFocus({ type: "archive-confirm-opened" }), { kind: "coworker-archive-confirm" });
  assert.deepEqual(resolveCoworkerFocus({ type: "archive-confirm-cancelled" }), { kind: "coworker-archive-trigger" });
});

test("selecting a roster row keeps focus on that row", () => {
  assert.deepEqual(
    resolveCoworkerFocus({ type: "roster-item-selected", coworkerId: "coworker-3" }),
    { kind: "coworker-roster-item", coworkerId: "coworker-3" },
  );
});

test("Escape closes a clean editor but keeps a dirty draft", () => {
  assert.deepEqual(resolveCoworkerEscape(false), { kind: "cancel-editor" });

  const dirty = resolveCoworkerEscape(true);
  assert.equal(dirty.kind, "keep-draft");
  assert.match(dirty.kind === "keep-draft" ? dirty.message : "", /unsaved changes/i);
  assert.match(dirty.kind === "keep-draft" ? dirty.message : "", /Cancel/i);
});

test("an edited draft is detected without inventing changes for an untouched form", () => {
  const baseline = { name: "Research Assistant", role_description: "Compare sources." };
  assert.equal(isCoworkerEditorDirty(baseline, { ...baseline }), false);
  assert.equal(isCoworkerEditorDirty(baseline, { ...baseline, name: "Research" }), true);
  assert.equal(isCoworkerEditorDirty(null, null), false);
});

test("Coworker status is spelled out and never carried by color alone", () => {
  assert.equal(coworkerStatusText("ACTIVE", false), "Active");
  assert.equal(coworkerStatusText("PAUSED", false), "Paused");
  assert.equal(coworkerStatusText("ARCHIVED", true), "Archived · Primary");
});
