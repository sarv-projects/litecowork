/**
 * Keyboard focus and Escape rules for the Coworker settings surface.
 *
 * Authority: `implementation/UI.md` (Sidebar and navigation, Create and edit
 * Coworker, Error and focus matrix, Keyboard and visual quality acceptance) and
 * `docs/EXPERIENCE.md`. The rules are pure so they can be verified without a
 * browser and reused by any Coworker surface.
 */

/** The control that owns keyboard focus after a Coworker surface transition. */
export type CoworkerFocusTarget =
  | { readonly kind: "coworker-name-input" }
  | { readonly kind: "coworker-detail-heading" }
  | { readonly kind: "coworker-roster-item"; readonly coworkerId: string }
  | { readonly kind: "coworker-create-action" }
  | { readonly kind: "coworker-archive-confirm" }
  | { readonly kind: "coworker-archive-trigger" }
  | { readonly kind: "none" };

/** A transition that moves keyboard focus on the Coworker surface. */
export type CoworkerFocusEvent =
  | { readonly type: "quick-create-opened" }
  | { readonly type: "edit-opened" }
  | { readonly type: "editor-cancelled"; readonly editingCoworkerId: string | null }
  | { readonly type: "editor-saved" }
  | { readonly type: "archive-confirm-opened" }
  | { readonly type: "archive-confirm-cancelled" }
  | { readonly type: "roster-item-selected"; readonly coworkerId: string };

/**
 * Resolve the focus target for one transition.
 *
 * Quick Create opens with focus on the name field. Cancelling returns focus to
 * the control that opened the editor, so a keyboard user is never dropped at the
 * top of the page. A successful save moves focus to the Coworker heading because
 * the form that held focus is removed from the page.
 */
export function resolveCoworkerFocus(event: CoworkerFocusEvent): CoworkerFocusTarget {
  switch (event.type) {
    case "quick-create-opened":
    case "edit-opened":
      return { kind: "coworker-name-input" };
    case "editor-cancelled":
      return event.editingCoworkerId
        ? { kind: "coworker-roster-item", coworkerId: event.editingCoworkerId }
        : { kind: "coworker-create-action" };
    case "editor-saved":
      return { kind: "coworker-detail-heading" };
    case "archive-confirm-opened":
      return { kind: "coworker-archive-confirm" };
    case "archive-confirm-cancelled":
      return { kind: "coworker-archive-trigger" };
    case "roster-item-selected":
      return { kind: "coworker-roster-item", coworkerId: event.coworkerId };
    default:
      return { kind: "none" };
  }
}

/** What the Escape key does inside the Coworker editor. */
export type CoworkerEscapeOutcome =
  | { readonly kind: "cancel-editor" }
  | { readonly kind: "keep-draft"; readonly message: string };

/**
 * Escape closes only a safe dialog.
 *
 * A clean editor closes and returns focus to the control that opened it. A dirty
 * editor keeps the draft: the owner confirms the discard explicitly with the
 * Cancel action, so a keystroke cannot silently destroy typed input.
 */
export function resolveCoworkerEscape(dirty: boolean): CoworkerEscapeOutcome {
  if (dirty) {
    return {
      kind: "keep-draft",
      message: "Your unsaved changes are still here. Use Cancel to discard them.",
    };
  }
  return { kind: "cancel-editor" };
}

/** True when the editor no longer matches the values it opened with. */
export function isCoworkerEditorDirty(
  baseline: unknown,
  current: unknown,
): boolean {
  return JSON.stringify(baseline ?? null) !== JSON.stringify(current ?? null);
}

/**
 * The visible status for a Coworker row. Status is never carried by color alone,
 * and a Coworker that is not reported keeps an explicit wording.
 */
export function coworkerStatusText(status: string, isPrimary: boolean): string {
  const base = status === "ACTIVE" ? "Active" : status === "PAUSED" ? "Paused" : "Archived";
  return isPrimary ? `${base} · Primary` : base;
}
