import { useLayoutEffect, useRef, useState } from "react";

export type LibraryNavigationChoice = "SAVE" | "DISCARD" | "STAY";
export type PendingLibraryNavigation<Page extends string = string> = {
  destination: Page;
  operationPending: boolean;
  canSave: boolean;
  saveError: string | null;
};

export function libraryNavigationDecision(
  currentPage: string,
  destinationPage: string,
  hasUnsavedDraft: boolean,
  choice: LibraryNavigationChoice | null,
): string {
  if (currentPage !== "Library" || destinationPage === "Library" || !hasUnsavedDraft) return "NAVIGATE";
  return choice ?? "CONFIRM";
}

export function resourceEditorHasUnsavedDraft(draftState: string | null): boolean {
  return draftState === "DIRTY";
}

export function libraryNavigationMainIsInert(navigationOpen: boolean, hasPendingNavigation: boolean): boolean {
  return navigationOpen || hasPendingNavigation;
}

function resourceRevisionSaveButton(editor: ParentNode | null): HTMLButtonElement | null {
  return editor?.querySelector<HTMLButtonElement>("button[data-library-draft-save]") ?? null;
}

export function useLibraryNavigation<Page extends string>(initialPage: Page) {
  const [page, setPage] = useState<Page>(initialPage);
  const pageRef = useRef(page);
  pageRef.current = page;
  const [pendingNavigation, setPendingNavigation] = useState<PendingLibraryNavigation<Page> | null>(null);
  const pendingNavigationRef = useRef<PendingLibraryNavigation<Page> | null>(null);
  const focusTargetRef = useRef<HTMLElement | null>(null);
  const restoreFocusAfterStayRef = useRef(false);

  const updatePendingNavigation = (value: PendingLibraryNavigation<Page> | null) => {
    pendingNavigationRef.current = value;
    setPendingNavigation(value);
  };

  useLayoutEffect(() => {
    if (pendingNavigation !== null || !restoreFocusAfterStayRef.current) return;
    restoreFocusAfterStayRef.current = false;
    const target = focusTargetRef.current;
    if (target?.isConnected) target.focus({ preventScroll: true });
  }, [pendingNavigation]);

  const requestNavigation = (destination: Page, blockedFocusTarget?: HTMLElement | null): boolean => {
    const editor = document.querySelector<HTMLElement>(".resource-revision-panel");
    const dirty = resourceEditorHasUnsavedDraft(editor?.getAttribute("data-library-draft") ?? null);
    if (libraryNavigationDecision(pageRef.current, destination, dirty, null) === "CONFIRM") {
      const saveButton = resourceRevisionSaveButton(editor);
      focusTargetRef.current = blockedFocusTarget
        ?? (document.activeElement instanceof HTMLElement ? document.activeElement : null);
      updatePendingNavigation({
        destination,
        operationPending: editor?.getAttribute("data-library-operation-pending") !== "false",
        canSave: editor?.getAttribute("data-library-can-save") === "true" && saveButton !== null && !saveButton.disabled,
        saveError: null,
      });
      return false;
    }
    pageRef.current = destination;
    setPage(destination);
    return true;
  };

  const stayInLibrary = () => {
    const current = pendingNavigationRef.current;
    if (current && libraryNavigationDecision(pageRef.current, current.destination, true, "STAY") !== "STAY") return;
    restoreFocusAfterStayRef.current = true;
    updatePendingNavigation(null);
  };

  const saveLibraryDraftBeforeNavigation = () => {
    const current = pendingNavigationRef.current;
    if (!current
      || libraryNavigationDecision(pageRef.current, current.destination, true, "SAVE") !== "SAVE"
      || current.operationPending
      || !current.canSave) return;
    const saveButton = resourceRevisionSaveButton(document.querySelector(".resource-revision-panel"));
    if (!saveButton || saveButton.disabled) return;
    updatePendingNavigation({ ...current, operationPending: true, canSave: false, saveError: null });
    saveButton.click();
  };

  const discardLibraryDraftAndNavigate = () => {
    const current = pendingNavigationRef.current;
    if (!current
      || libraryNavigationDecision(pageRef.current, current.destination, true, "DISCARD") !== "DISCARD"
      || current.operationPending) return;
    pageRef.current = current.destination;
    updatePendingNavigation(null);
    setPage(current.destination);
  };

  const draftSaveFailed = (message: string) => {
    const current = pendingNavigationRef.current;
    if (!current) return;
    updatePendingNavigation({
      ...current,
      operationPending: false,
      canSave: false,
      saveError: message,
    });
  };

  const draftCommitted = () => {
    const current = pendingNavigationRef.current;
    if (!current) return;
    const destination = current.destination;
    pageRef.current = destination;
    updatePendingNavigation(null);
    setPage(destination);
  };

  return {
    page,
    pendingNavigation,
    requestNavigation,
    stayInLibrary,
    saveLibraryDraftBeforeNavigation,
    discardLibraryDraftAndNavigate,
    draftSaveFailed,
    draftCommitted,
  };
}

export function LibraryNavigationDialog({
  pending,
  onStay,
  onSave,
  onDiscard,
}: {
  pending: PendingLibraryNavigation | null;
  onStay: () => void;
  onSave: () => void;
  onDiscard: () => void;
}) {
  const stayButtonRef = useRef<HTMLButtonElement>(null);
  const wasOperationPendingRef = useRef(false);
  useLayoutEffect(() => {
    const operationPending = pending?.operationPending === true;
    if (operationPending && !wasOperationPendingRef.current) {
      stayButtonRef.current?.focus({ preventScroll: true });
    }
    wasOperationPendingRef.current = operationPending;
  }, [pending?.operationPending]);

  if (!pending) return null;
  return <div className="resource-discard-backdrop">
    <section className="resource-discard-dialog" role="alertdialog" aria-modal="true" aria-labelledby="library-navigation-title" aria-describedby="library-navigation-description" onKeyDown={(event) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onStay();
        return;
      }
      if (event.key !== "Tab") return;
      const controls = event.currentTarget.querySelectorAll<HTMLElement>("button:not(:disabled)");
      const first = controls.item(0);
      const last = controls.item(controls.length - 1);
      if (!first || !last) return;
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    }}>
      <h3 id="library-navigation-title">Leave the Library?</h3>
      <p id="library-navigation-description">{pending.saveError
        ? "The Resource draft was not saved. Your draft is still in the Library. Stay to review the error or authorize any required rebase before trying again, or discard the draft."
        : pending.operationPending
          ? "A Resource save or status request is still resolving. Keep the Library open until its result is clear."
          : "This Resource draft is saved only in the current editor. Save it, discard it, or stay in the Library to keep editing."}</p>
      {pending.saveError && <p className="inline-error" role="alert" aria-live="assertive">{pending.saveError}</p>}
      <div className="resource-discard-actions">
        <button className="primary-button" type="button" disabled={pending.operationPending || !pending.canSave} onClick={onSave}>Save draft and leave Library</button>
        <button className="quiet-button" type="button" disabled={pending.operationPending} onClick={onDiscard}>Discard draft and leave Library</button>
        <button className="quiet-button" type="button" ref={stayButtonRef} autoFocus onClick={onStay}>Stay in Library</button>
      </div>
    </section>
  </div>;
}
