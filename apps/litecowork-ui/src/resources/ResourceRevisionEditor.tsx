import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./resource-revision-editor.css";

type Resource = {
  resourceId: string;
  workspaceId: string;
  resourceRevisionId: string;
  displayName: string;
  mediaType: string;
  contentDigest: string;
  sizeBytes: number;
};
type ContextDocument = { kind: string; status: string; owner_ref?: { kind?: string; workspace_id?: string; principal_id?: string; coworker_id?: string; goal_id?: string } };
type Detail = { resourceId: string; workspaceId: string; kind: string; displayName: string; currentRevisionId: string | null; version: number; contextDocument: ContextDocument | null };
type Revision = { resourceRevisionId: string; resourceId: string; parentRevisionIds: string[]; contentDigest: string | null; sizeBytes: number | null; mediaType: string | null; observedAt: string; isHead: boolean };
type RevisionPage = { items: Revision[]; nextCursor: string | null };
type UploadSession = { uploadId: string; workspaceId: string; displayName: string; mediaType: string; expectedSizeBytes: number; expectedDigest: string | null; chunkSizeBytes: number; receivedRanges: Array<{ startOffset: number; endOffsetInclusive: number; sha256: string }>; nextMissingOffset: number; state: string; resourceId: string | null; committedResourceId: string | null; expectedResourceVersion: number | null; parentRevisionIds: string[]; expiresAt: string };
type RevisionUploadRecovery = {
  workspaceId: string;
  resourceId: string;
  resourceVersion: number;
  parentRevisionIds: string[];
  mediaType: string;
  sizeBytes: number;
  expectedDigest: string;
  createRequestId: string;
  commitRequestId: string;
  uploadId: string | null;
};
type OwnerStatus = "ACTIVE" | "REVOKED";
type RevisionComparison = { left: Revision; right: Revision; leftText: string; rightText: string };

const MAX_REVISION_BYTES = 100 * 1024 * 1024;
const MAX_EDIT_TEXT_BYTES = 1024 * 1024;
// Kept only in this WebView process. The descriptor contains no selected path or bytes;
// it lets an owner reopen this editor and reselect the same file during the same app
// session. A process restart intentionally drops it rather than writing local UI state.
const revisionUploadRecoveries = new Map<string, RevisionUploadRecovery>();

function revisionUploadKey(workspaceId: string, resourceId: string): string {
  return `${workspaceId}\u0000${resourceId}`;
}

function sameRevisionUploadPins(recovery: RevisionUploadRecovery, resource: Resource, detail: Detail): boolean {
  return recovery.workspaceId === resource.workspaceId
    && recovery.resourceId === resource.resourceId
    && recovery.resourceVersion === detail.version
    && recovery.parentRevisionIds.length === 1
    && recovery.parentRevisionIds[0] === detail.currentRevisionId;
}

function assertRevisionUploadMatches(session: UploadSession, recovery: RevisionUploadRecovery, resource: Resource): void {
  if (session.workspaceId !== recovery.workspaceId || session.resourceId !== recovery.resourceId
    || session.displayName !== resource.displayName || session.expectedResourceVersion !== recovery.resourceVersion
    || session.parentRevisionIds.length !== recovery.parentRevisionIds.length
    || session.parentRevisionIds.some((id, index) => id !== recovery.parentRevisionIds[index])
    || session.mediaType !== recovery.mediaType || session.expectedSizeBytes !== recovery.sizeBytes
    || session.expectedDigest !== recovery.expectedDigest || !Number.isSafeInteger(session.chunkSizeBytes)
    || session.chunkSizeBytes <= 0 || session.chunkSizeBytes > 4 * 1024 * 1024
    || !Number.isSafeInteger(session.nextMissingOffset) || session.nextMissingOffset < 0
    || session.nextMissingOffset > recovery.sizeBytes || !Array.isArray(session.receivedRanges)) {
    throw new Error("The saved upload session does not match this Workspace, Resource version, and selected bytes.");
  }
  let covered = 0;
  for (const range of session.receivedRanges) {
    if (!Number.isSafeInteger(range.startOffset) || !Number.isSafeInteger(range.endOffsetInclusive)
      || range.startOffset !== covered || range.endOffsetInclusive < range.startOffset
      || range.endOffsetInclusive >= recovery.sizeBytes
      || !/^sha256:[0-9a-f]{64}$/.test(range.sha256)) {
      throw new Error("The saved upload progress is malformed. No more bytes were sent.");
    }
    const length = range.endOffsetInclusive - range.startOffset + 1;
    if (length > session.chunkSizeBytes || (range.endOffsetInclusive + 1 < recovery.sizeBytes && length !== session.chunkSizeBytes)) {
      throw new Error("The saved upload progress does not match the negotiated chunk size.");
    }
    covered = range.endOffsetInclusive + 1;
  }
  if (covered !== session.nextMissingOffset) throw new Error("The saved upload offset does not match its accepted ranges.");
  if (session.state === "CONTENT_RECEIVED" && session.nextMissingOffset !== recovery.sizeBytes) {
    throw new Error("The server marked an incomplete revision upload as ready to commit.");
  }
  if (!["OPEN", "CONTENT_RECEIVED", "COMMITTED", "FAILED", "EXPIRED"].includes(session.state)) {
    throw new Error("The server returned an unsupported revision upload state.");
  }
}

export function ResourceRevisionEditor({ resource, onClose, onCommitted }: { resource: Resource; onClose: () => void; onCommitted: (resource: Resource) => void }) {
  const [detail, setDetail] = useState<Detail | null>(null);
  const [history, setHistory] = useState<Revision[]>([]);
  const [cursor, setCursor] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(true);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const [pendingFileReplacement, setPendingFileReplacement] = useState<File | null>(null);
  const [draftText, setDraftText] = useState<string | null>(null);
  const [textBaseline, setTextBaseline] = useState<string | null>(null);
  const [confirmDiscard, setConfirmDiscard] = useState(false);
  const [confirmReplaceDraft, setConfirmReplaceDraft] = useState(false);
  const [rebaseRequired, setRebaseRequired] = useState(false);
  const [rebaseReloadPending, setRebaseReloadPending] = useState(false);
  const [rebaseApproval, setRebaseApproval] = useState<{ resourceVersion: number; headId: string } | null>(null);
  const [reviewedHeadText, setReviewedHeadText] = useState<{ resourceVersion: number; headId: string; text: string } | null>(null);
  const [headPreviewBusy, setHeadPreviewBusy] = useState(false);
  const [headPreviewError, setHeadPreviewError] = useState<string | null>(null);
  const [uploadBytes, setUploadBytes] = useState(0);
  const [uploadRecovery, setUploadRecovery] = useState<RevisionUploadRecovery | null>(() =>
    revisionUploadRecoveries.get(revisionUploadKey(resource.workspaceId, resource.resourceId)) ?? null);
  const [statusConfirmation, setStatusConfirmation] = useState<OwnerStatus | null>(null);
  const [statusRequest, setStatusRequest] = useState<{ target: OwnerStatus; expectedVersion: number; requestId: string } | null>(null);
  const [statusBusy, setStatusBusy] = useState(false);
  const [statusError, setStatusError] = useState<string | null>(null);
  const [compareOpen, setCompareOpen] = useState(false);
  const [compareLeftId, setCompareLeftId] = useState("");
  const [compareRightId, setCompareRightId] = useState("");
  const [compareBusy, setCompareBusy] = useState(false);
  const [compareError, setCompareError] = useState<string | null>(null);
  const [comparison, setComparison] = useState<RevisionComparison | null>(null);
  const comparisonRequest = useRef(0);

  useEffect(() => {
    comparisonRequest.current += 1;
    setCompareLeftId("");
    setCompareRightId("");
    setCompareBusy(false);
    setCompareError(null);
    setComparison(null);
    setCompareOpen(false);
    return () => { comparisonRequest.current += 1; };
  }, [resource.resourceId, resource.workspaceId]);

  useEffect(() => {
    setUploadRecovery(revisionUploadRecoveries.get(revisionUploadKey(resource.workspaceId, resource.resourceId)) ?? null);
  }, [resource.resourceId, resource.workspaceId]);

  const rememberUploadRecovery = (recovery: RevisionUploadRecovery | null) => {
    const key = revisionUploadKey(resource.workspaceId, resource.resourceId);
    if (recovery) revisionUploadRecoveries.set(key, recovery);
    else revisionUploadRecoveries.delete(key);
    setUploadRecovery(recovery);
  };

  const reload = useCallback(async (append = false) => {
    setLoading(true);
    setError(null);
    try {
      const [nextDetail, page] = await Promise.all([
        invoke<Detail>("get_resource_detail", { workspaceId: resource.workspaceId, resourceId: resource.resourceId }),
        invoke<RevisionPage>("list_resource_revisions", { workspaceId: resource.workspaceId, resourceId: resource.resourceId, cursor: append ? cursor : null }),
      ]);
      if (nextDetail.workspaceId !== resource.workspaceId || nextDetail.resourceId !== resource.resourceId) throw new Error("Resource response did not match this Workspace.");
      setDetail(nextDetail);
      setHistory((current) => append ? [...current, ...page.items] : page.items);
      setCursor(page.nextCursor);
      if (!append) setRebaseReloadPending(false);
    } catch (cause) {
      setError(messageOf(cause, "Resource details and revision history could not be loaded."));
    } finally {
      setLoading(false);
    }
  }, [cursor, resource.resourceId, resource.workspaceId]);

  useEffect(() => { void reload(false); }, [resource.resourceId, resource.workspaceId]);

  const context = detail?.contextDocument ?? null;
  const isCurrentTextEditable = isSmallText(resource.mediaType, resource.sizeBytes);
  const hasUnsavedDraft = selectedFile !== null || (draftText !== null && draftText !== textBaseline);
  const recoveryMatchesCurrentHead = !!(detail && uploadRecovery && sameRevisionUploadPins(uploadRecovery, resource, detail));
  const rebaseIsApproved = rebaseRequired && !rebaseReloadPending && detail?.currentRevisionId !== null
    && detail?.currentRevisionId !== undefined && rebaseApproval?.resourceVersion === detail.version
    && rebaseApproval.headId === detail.currentRevisionId;
  const headTextMatchesCurrent = reviewedHeadText !== null && reviewedHeadText.resourceVersion === detail?.version
    && reviewedHeadText.headId === detail?.currentRevisionId;

  const stageReplacementFile = (file: File) => {
    if (hasUnsavedDraft) {
      setPendingFileReplacement(file);
      setConfirmReplaceDraft(true);
      return;
    }
    setSelectedFile(file);
    setDraftText(null);
    setTextBaseline(null);
    setRebaseApproval(null);
    setError(null);
  };

  const replaceDraftWithSelectedFile = () => {
    if (!pendingFileReplacement) return;
    setSelectedFile(pendingFileReplacement);
    setPendingFileReplacement(null);
    setDraftText(null);
    setTextBaseline(null);
    setRebaseApproval(null);
    setConfirmReplaceDraft(false);
    setError(null);
  };

  const requestClose = () => {
    if (busy || statusBusy || statusRequest !== null) return;
    if (hasUnsavedDraft) {
      setConfirmDiscard(true);
      return;
    }
    onClose();
  };

  const discardDraftAndClose = () => {
    setSelectedFile(null);
    setPendingFileReplacement(null);
    setDraftText(null);
    setTextBaseline(null);
    setConfirmDiscard(false);
    setConfirmReplaceDraft(false);
    onClose();
  };

  const approveRebase = () => {
    if (!detail?.currentRevisionId || rebaseReloadPending || !hasUnsavedDraft) return;
    if (uploadRecovery && !sameRevisionUploadPins(uploadRecovery, resource, detail)) {
      // This approval abandons the old server session descriptor and authorizes only
      // the currently reselected complete draft against the reviewed current head.
      // The old server-side upload remains until its normal expiry.
      rememberUploadRecovery(null);
    }
    setRebaseApproval({ resourceVersion: detail.version, headId: detail.currentRevisionId });
    setMessage(`Draft is now explicitly based on Resource version ${detail.version}, head ${detail.currentRevisionId}. It will create a new revision from this content; LiteCowork will not merge content automatically. Any older incomplete server upload remains until it expires.`);
  };

  const previewCurrentHeadForReview = async () => {
    if (!detail?.currentRevisionId || rebaseReloadPending || !isCurrentTextEditable || statusRequest !== null) return;
    setHeadPreviewBusy(true);
    setHeadPreviewError(null);
    setReviewedHeadText(null);
    try {
      const text = await invoke<string>("preview_resource_text", {
        workspaceId: resource.workspaceId,
        resourceId: resource.resourceId,
        revisionId: detail.currentRevisionId,
      });
      setReviewedHeadText({ resourceVersion: detail.version, headId: detail.currentRevisionId, text });
    } catch (cause) {
      setHeadPreviewError(messageOf(cause, "The current Resource head could not be previewed."));
    } finally { setHeadPreviewBusy(false); }
  };

  const compareSelectedRevisions = async () => {
    if (!compareLeftId || !compareRightId || compareLeftId === compareRightId) {
      setCompareError("Choose two different committed revisions of this Resource.");
      return;
    }
    const left = history.find((revision) => revision.resourceRevisionId === compareLeftId && revision.resourceId === resource.resourceId);
    const right = history.find((revision) => revision.resourceRevisionId === compareRightId && revision.resourceId === resource.resourceId);
    if (!left || !right || !isComparableRevision(left) || !isComparableRevision(right)) {
      setCompareError("Both selected revisions must be loaded, committed plain-text or Markdown revisions no larger than 1 MiB.");
      return;
    }
    const request = ++comparisonRequest.current;
    setCompareBusy(true);
    setCompareError(null);
    try {
      const [leftText, rightText] = await Promise.all([
        invoke<string>("preview_resource_text", { workspaceId: resource.workspaceId, resourceId: resource.resourceId, revisionId: left.resourceRevisionId }),
        invoke<string>("preview_resource_text", { workspaceId: resource.workspaceId, resourceId: resource.resourceId, revisionId: right.resourceRevisionId }),
      ]);
      if (request !== comparisonRequest.current) return;
      if (typeof leftText !== "string" || typeof rightText !== "string") throw new Error("Local Runtime returned an unsupported text comparison.");
      if (new TextEncoder().encode(leftText).byteLength > MAX_EDIT_TEXT_BYTES || new TextEncoder().encode(rightText).byteLength > MAX_EDIT_TEXT_BYTES) {
        throw new Error("Each compared revision is limited to 1 MiB.");
      }
      setComparison({ left, right, leftText, rightText });
    } catch (cause) {
      if (request === comparisonRequest.current) {
        setCompareError(messageOf(cause, "The selected Resource revisions could not be compared."));
      }
    } finally {
      if (request === comparisonRequest.current) setCompareBusy(false);
    }
  };

  const changeComparisonPin = (side: "left" | "right", revisionId: string) => {
    comparisonRequest.current += 1;
    setCompareBusy(false);
    setCompareError(null);
    setComparison(null);
    if (side === "left") setCompareLeftId(revisionId);
    else setCompareRightId(revisionId);
  };

  const commitContextStatus = async (request: { target: OwnerStatus; expectedVersion: number; requestId: string }) => {
    setStatusBusy(true);
    setStatusError(null);
    try {
      const updated = await invoke<Detail>("set_context_document_status", {
        workspaceId: resource.workspaceId,
        resourceId: resource.resourceId,
        expectedVersion: request.expectedVersion,
        requestId: request.requestId,
        targetStatus: request.target,
      });
      if (updated.workspaceId !== resource.workspaceId || updated.resourceId !== resource.resourceId
        || updated.contextDocument?.status !== request.target) {
        throw new Error("Local Runtime returned a different ContextDocument status.");
      }
      setDetail(updated);
      setStatusRequest(null);
      setStatusConfirmation(null);
      setMessage(request.target === "REVOKED"
        ? "Future LiteCowork reads are blocked. The stored bytes remain, and content already delivered to an agent cannot be recalled."
        : "Future LiteCowork reads are allowed again. This does not recover any content that may have been deleted elsewhere.");
    } catch (cause) {
      const text = messageOf(cause, "ContextDocument status could not be updated.");
      setStatusError(text);
      if (text.includes("RESOURCE_CONFLICT")) {
        setStatusRequest(null);
        setStatusConfirmation(null);
        setStatusError(null);
        setMessage("The Resource changed before this status update. Current metadata has been refreshed; review its status before trying again.");
        await reload(false);
      }
    } finally { setStatusBusy(false); }
  };

  const beginContextStatusChange = (target: OwnerStatus) => {
    if (!detail || !context || hasUnsavedDraft || statusRequest !== null
      || context.status !== (target === "REVOKED" ? "ACTIVE" : "REVOKED")) return;
    const request = { target, expectedVersion: detail.version, requestId: crypto.randomUUID() };
    setStatusRequest(request);
    void commitContextStatus(request);
  };

  const openTextEditor = async () => {
    if (!detail?.currentRevisionId || !isCurrentTextEditable || statusRequest !== null) return;
    setBusy(true); setError(null); setMessage(null);
    try {
      const text = await invoke<string>("preview_resource_text", {
        workspaceId: resource.workspaceId,
        resourceId: resource.resourceId,
        revisionId: detail.currentRevisionId,
      });
      setDraftText(text);
      setTextBaseline(text);
      setSelectedFile(null);
    } catch (cause) {
      setError(messageOf(cause, "Current text could not be opened for editing."));
    } finally { setBusy(false); }
  };

  const saveRevision = async (file: File) => {
    if (!detail || !context || context.status !== "ACTIVE" || !detail.currentRevisionId || statusRequest !== null
      || (rebaseRequired && !rebaseIsApproved)) return;
    if (file.size > MAX_REVISION_BYTES) { setError("A revision file cannot exceed 100 MiB."); return; }
    setBusy(true); setError(null); setMessage(null); setUploadBytes(0);
    try {
      const bytes = await file.arrayBuffer();
      const expectedDigest = `sha256:${await digestHex(bytes)}`;
      const mediaType = file.type || resource.mediaType || "application/octet-stream";
      const parentRevisionIds = [detail.currentRevisionId];
      let recovery = revisionUploadRecoveries.get(revisionUploadKey(resource.workspaceId, resource.resourceId)) ?? uploadRecovery;
      if (recovery) {
        if (recovery.expectedDigest !== expectedDigest || recovery.sizeBytes !== file.size || recovery.mediaType !== mediaType) {
          throw new Error("These bytes do not match the interrupted upload. Choose the original file, or explicitly forget its local resume details before starting a new revision.");
        }
      } else {
        recovery = {
          workspaceId: resource.workspaceId,
          resourceId: resource.resourceId,
          resourceVersion: detail.version,
          parentRevisionIds,
          mediaType,
          sizeBytes: file.size,
          expectedDigest,
          createRequestId: crypto.randomUUID(),
          commitRequestId: crypto.randomUUID(),
          uploadId: null,
        };
        // Keep only non-secret transfer identity in memory before the first request so
        // a lost session-creation response can be replayed with the same request ID.
        rememberUploadRecovery(recovery);
      }

      const session = recovery.uploadId
        ? await invoke<UploadSession>("get_resource_upload", { workspaceId: recovery.workspaceId, uploadId: recovery.uploadId })
        : await invoke<UploadSession>("create_resource_revision_upload", {
          workspaceId: recovery.workspaceId,
          resourceId: recovery.resourceId,
          expectedResourceVersion: recovery.resourceVersion,
          parentRevisionIds: recovery.parentRevisionIds,
          mediaType: recovery.mediaType,
          sizeBytes: recovery.sizeBytes,
          expectedDigest: recovery.expectedDigest,
          requestId: recovery.createRequestId,
        });
      assertRevisionUploadMatches(session, recovery, resource);
      // A successful earlier commit advances the Resource head, so replaying its exact
      // commit receipt is allowed after that change. An open transfer is never extended
      // against a changed Resource version/head.
      if (session.state !== "COMMITTED" && !sameRevisionUploadPins(recovery, resource, detail)) {
        throw new Error("RESOURCE_CONFLICT: An interrupted upload is pinned to an older Resource version or head. Review the current Resource history and explicitly authorize a new revision before continuing.");
      }
      if (!recovery.uploadId) {
        recovery = { ...recovery, uploadId: session.uploadId };
        rememberUploadRecovery(recovery);
      }
      if (session.state === "FAILED" || session.state === "EXPIRED") {
        throw new Error(`This revision upload is ${session.state.toLowerCase()} and cannot be resumed. Review the current history, then forget its local resume details to start a new upload.`);
      }
      const expiry = Date.parse(session.expiresAt);
      if (!Number.isFinite(expiry)) throw new Error("The upload session has no valid expiry; no more bytes were sent.");
      if ((session.state === "OPEN" || session.state === "CONTENT_RECEIVED") && expiry <= Date.now()) {
        throw new Error("This revision upload has expired. Review the current history, then forget its local resume details to start a new upload.");
      }
      const bytesView = new Uint8Array(bytes);
      // Verify each server-reported accepted range against the reselected bytes before
      // extending the session. The whole-file digest alone does not prove chunk receipts.
      for (const range of session.receivedRanges) {
        const end = range.endOffsetInclusive + 1;
        const chunkDigest = `sha256:${await digestHex(bytesView.slice(range.startOffset, end).buffer)}`;
        if (chunkDigest !== range.sha256) throw new Error("An accepted upload range does not match the reselected file. No additional bytes were sent.");
      }
      if (session.state === "COMMITTED") {
        const committed = await invoke<Resource>("commit_resource_revision_upload", {
          workspaceId: recovery.workspaceId,
          uploadId: session.uploadId,
          requestId: recovery.commitRequestId,
        });
        if (session.committedResourceId && committed.resourceId !== session.committedResourceId) {
          throw new Error("The recovered commit does not match the Resource recorded by the upload session.");
        }
        rememberUploadRecovery(null);
        setSelectedFile(null); setDraftText(null); setTextBaseline(null);
        setConfirmDiscard(false);
        setRebaseRequired(false);
        setRebaseReloadPending(false);
        setRebaseApproval(null);
        setReviewedHeadText(null);
        onCommitted(committed);
        await reload(false);
        setMessage("New Resource revision saved.");
        return;
      }
      let offset = session.nextMissingOffset;
      if (offset !== file.size && offset % session.chunkSizeBytes !== 0) {
        throw new Error("The server-reported resume offset is not on a chunk boundary. No additional bytes were sent.");
      }
      setUploadBytes(offset);
      for (let index = Math.floor(offset / session.chunkSizeBytes); offset < file.size; index += 1) {
        const end = Math.min(offset + session.chunkSizeBytes, file.size);
        const chunk = bytesView.slice(offset, end);
        await invoke("upload_resource_chunk", {
          workspaceId: recovery.workspaceId,
          uploadId: session.uploadId,
          chunkIndex: index,
          contentRange: `bytes ${offset}-${end - 1}/${file.size}`,
          chunkSha256: await digestHex(chunk.buffer),
          contentBase64: bytesToBase64(chunk),
          requestId: crypto.randomUUID(),
        });
        setUploadBytes(end);
      }
      setMessage("All selected bytes uploaded. Committing the new revision…");
      const committed = await invoke<Resource>("commit_resource_revision_upload", {
        workspaceId: recovery.workspaceId,
        uploadId: session.uploadId,
        requestId: recovery.commitRequestId,
      });
      rememberUploadRecovery(null);
      setSelectedFile(null); setDraftText(null); setTextBaseline(null);
      setConfirmDiscard(false);
      setRebaseRequired(false);
      setRebaseReloadPending(false);
      setRebaseApproval(null);
      setReviewedHeadText(null);
      onCommitted(committed);
      await reload(false);
      setMessage("New Resource revision saved.");
    } catch (cause) {
      const text = messageOf(cause, "Resource revision could not be saved.");
      setError(text);
      if (text.includes("RESOURCE_CONFLICT")) {
        setRebaseRequired(true);
        setRebaseReloadPending(true);
        setRebaseApproval(null);
        setReviewedHeadText(null);
        setHeadPreviewError(null);
        setMessage("No revision was committed. Your exact local draft is preserved. Review the refreshed Resource version, current head, and history; explicitly authorize using this draft as a new revision before retrying.");
        await reload(false);
      }
    } finally { setBusy(false); }
  };

  const saveText = async () => {
    if (draftText === null) return;
    const encoded = new TextEncoder().encode(draftText);
    if (encoded.byteLength > MAX_EDIT_TEXT_BYTES) { setError("Text edits are limited to 1 MiB."); return; }
    const blob = new Blob([encoded], { type: resource.mediaType || "text/plain" });
    await saveRevision(new File([blob], resource.displayName, { type: resource.mediaType || "text/plain" }));
  };

  return <section className="resource-revision-panel" aria-labelledby="resource-revision-title">
    <header className="resource-revision-header">
      <div><p className="eyebrow">RESOURCE HISTORY</p><h2 id="resource-revision-title">{resource.displayName}</h2><p>Each save creates a new immutable revision.</p></div>
      <button className="quiet-button" type="button" onClick={requestClose} disabled={busy || statusBusy || statusRequest !== null}>{hasUnsavedDraft ? "Close editor…" : "Close"}</button>
    </header>
    {(confirmDiscard || confirmReplaceDraft) && <div className="resource-discard-backdrop">
      <section className="resource-discard-dialog" role="alertdialog" aria-modal="true" aria-labelledby="resource-discard-title" aria-describedby="resource-discard-description" onKeyDown={(event) => {
        if (event.key === "Escape") {
          event.preventDefault();
          setConfirmDiscard(false);
          setConfirmReplaceDraft(false);
          setPendingFileReplacement(null);
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
        <h3 id="resource-discard-title">{confirmReplaceDraft ? "Replace the current draft?" : "Discard this unsaved revision?"}</h3>
        <p id="resource-discard-description">{confirmReplaceDraft
          ? "The current text or selected file has not been uploaded. Replacing it will discard that local draft and stage the newly selected file."
          : "The selected file or text draft has not been uploaded. Closing now will remove this draft from the editor."}</p>
        <div className="resource-discard-actions">
          <button className="quiet-button" type="button" autoFocus onClick={() => { setConfirmDiscard(false); setConfirmReplaceDraft(false); setPendingFileReplacement(null); }}>Keep current draft</button>
          {confirmReplaceDraft
            ? <button className="primary-button" type="button" onClick={replaceDraftWithSelectedFile}>Discard draft and use selected file</button>
            : <button className="primary-button" type="button" onClick={discardDraftAndClose}>Discard draft and close</button>}
        </div>
      </section>
    </div>}
    {loading && !detail ? <p className="inline-status" role="status">Loading Resource metadata…</p> : null}
    {error && <p className="inline-error" role="alert">{error}</p>}
    {message && <p className="inline-status" role="status">{message}</p>}
    {detail && <>
      <div className="resource-revision-meta">
        <span><strong>Kind</strong>{detail.kind}</span>
        <span><strong>Resource version</strong>{detail.version}</span>
        <span><strong>Current revision</strong>{detail.currentRevisionId ?? "Conflicted or unavailable"}</span>
        <span><strong>Context status</strong>{context?.status ?? "Not a ContextDocument"}</span>
      </div>
      {context ? <p className="resource-context-owner">{context.kind} · owner-authored ContextDocument</p> : <p className="inline-status">Revision editing is available only for owner-authored ContextDocuments.</p>}
      {context && <section className="resource-context-status" aria-labelledby="resource-context-status-title">
        <div><h3 id="resource-context-status-title">Context availability</h3><p>Current status: <strong>{context.status}</strong></p></div>
        {context.status === "ACTIVE" || context.status === "REVOKED" ? <>
          <p>Revoking blocks future LiteCowork content reads and index updates, but retains the stored bytes. It cannot recall content already delivered to an agent session.</p>
          {hasUnsavedDraft && <p>Save or discard the local revision draft before changing ContextDocument availability.</p>}
          {statusRequest && !statusError && <p role="status">Waiting for the previous status request to resolve. Editing and closing are paused so its result can be confirmed or retried safely.</p>}
          {statusError && <p className="inline-error" role="alert">{statusError}</p>}
          {statusRequest && statusError
            ? <button className="quiet-button" type="button" disabled={statusBusy} onClick={() => void commitContextStatus(statusRequest)}>{statusBusy ? "Retrying…" : "Retry the same status request"}</button>
            : statusConfirmation
              ? <div className="resource-context-status-confirm"><p>{statusConfirmation === "REVOKED"
                ? "Revoke future reads? The retained content will remain stored; already-delivered content cannot be recalled."
                : "Restore future reads? This allows new LiteCowork reads of the retained content."}</p><button className="primary-button" type="button" disabled={statusBusy || busy || hasUnsavedDraft || statusRequest !== null} onClick={() => beginContextStatusChange(statusConfirmation)}>{statusBusy ? "Updating…" : statusConfirmation === "REVOKED" ? "Confirm revocation" : "Confirm restoration"}</button><button className="quiet-button" type="button" disabled={statusBusy || busy || statusRequest !== null} onClick={() => setStatusConfirmation(null)}>Cancel</button></div>
              : <button className="quiet-button" type="button" disabled={statusBusy || busy || hasUnsavedDraft || statusRequest !== null} onClick={() => setStatusConfirmation(context.status === "ACTIVE" ? "REVOKED" : "ACTIVE")}>{context.status === "ACTIVE" ? "Revoke future reads…" : "Restore future reads…"}</button>}
        </> : <p>This status is managed by the deletion lifecycle. Owner status changes are unavailable here.</p>}
      </section>}
      {context && context.status !== "ACTIVE" && <p className="inline-status">This ContextDocument is {context.status.toLowerCase()}. Revision editing is unavailable.</p>}
      {uploadRecovery && <section className="resource-upload-recovery" aria-labelledby="resource-upload-recovery-title">
        <div><h3 id="resource-upload-recovery-title">Interrupted revision upload</h3>
          <p>{recoveryMatchesCurrentHead
            ? `This app session remembers an upload for the exact Resource version ${uploadRecovery.resourceVersion}. Choose the same file to verify its SHA-256 digest and resume from the server’s accepted offset.`
            : `The saved upload is pinned to Resource version ${uploadRecovery.resourceVersion} and its original head. Current Resource state differs or is still loading; it will not be rebased or continued automatically.`}</p>
          <small>No file bytes or local file path are retained by the resume record. It lasts only while this LiteCowork app session remains open. The server upload expires independently.</small>
        </div>
        <button className="quiet-button" type="button" disabled={busy || statusBusy || statusRequest !== null} onClick={() => {
          rememberUploadRecovery(null);
          setSelectedFile(null);
          setMessage("Local resume details cleared. The incomplete server upload remains until its normal expiry; no Resource revision was removed.");
        }}>Forget resume details</button>
      </section>}
      {context?.status === "ACTIVE" && !detail.currentRevisionId && <p className="inline-status">This Resource has conflicting or unavailable revision heads. Resolve the conflict before editing; this screen will not select a branch for you.</p>}
      {context?.status === "ACTIVE" && rebaseRequired && <section className="resource-rebase-review" aria-labelledby="resource-rebase-title">
        <h3 id="resource-rebase-title">The Resource changed before this revision was saved</h3>
        <p>Your exact local draft and original text baseline are preserved. Review Resource version {detail.version}, current head {detail.currentRevisionId ?? "unavailable or conflicted"}, and the revision history below. This action does not merge; it authorizes your complete draft as the next revision on the current head.</p>
        {rebaseReloadPending && <p role="status">Refreshing current Resource metadata before any rebase can be authorized…</p>}
        {!rebaseReloadPending && detail.currentRevisionId && isCurrentTextEditable && <>
          <button className="quiet-button" type="button" disabled={headPreviewBusy || statusRequest !== null} onClick={() => void previewCurrentHeadForReview()}>{headPreviewBusy ? "Loading current head…" : headTextMatchesCurrent ? "Refresh current-head preview" : "Preview current saved text before rebasing"}</button>
          {headPreviewError && <p className="inline-error" role="alert">{headPreviewError}</p>}
          {headTextMatchesCurrent && <pre className="resource-rebase-current-text" aria-label={`Current saved text at ${reviewedHeadText.headId}`} tabIndex={0}>{reviewedHeadText.text}</pre>}
        </>}
        {!rebaseReloadPending && detail.currentRevisionId && <button className="quiet-button" type="button" disabled={!hasUnsavedDraft || statusRequest !== null} onClick={approveRebase}>{rebaseIsApproved ? "Draft authorized for this head" : "I reviewed this head; use my draft as the next revision"}</button>}
        {!rebaseReloadPending && !detail.currentRevisionId && <p role="alert">The Resource has no single current head. The draft remains local; this editor will not choose or merge branches.</p>}
      </section>}
      {context?.status === "ACTIVE" && detail.currentRevisionId && <div className="resource-revision-edit">
        <h3>Create a revision</h3>
        <p>New uploads are pinned to Resource version {detail.version} and current head {detail.currentRevisionId}. A concurrent change blocks commit.</p>
        <label className="resource-revision-file">{uploadRecovery ? "Choose the original file to resume this revision upload" : "Choose a revised local file"}
          <input type="file" disabled={busy || statusRequest !== null} onChange={(event) => {
            const file = event.currentTarget.files?.[0] ?? null;
            event.currentTarget.value = "";
            if (file) stageReplacementFile(file);
          }} />
        </label>
        {selectedFile && <div className="resource-revision-staged"><span>{selectedFile.name} · {formatBytes(selectedFile.size)}</span><button className="primary-button" type="button" disabled={busy || statusRequest !== null || (rebaseRequired && !rebaseIsApproved)} onClick={() => void saveRevision(selectedFile)}>{busy ? `Uploading ${formatBytes(uploadBytes)} of ${formatBytes(selectedFile.size)}…` : uploadRecovery ? "Verify file and resume upload" : rebaseRequired ? "Save authorized draft as new revision" : "Upload as new revision"}</button></div>}
        {!selectedFile && isCurrentTextEditable && <button className="quiet-button" type="button" disabled={busy || hasUnsavedDraft || statusRequest !== null} onClick={() => void openTextEditor()}>{draftText === null ? "Edit small text file" : "Reload current text"}</button>}
        {draftText !== null && <div className="resource-revision-text-editor"><label htmlFor="resource-revision-text">Edit content (up to 1 MiB)</label><textarea id="resource-revision-text" value={draftText} disabled={busy || statusRequest !== null} onChange={(event) => { setDraftText(event.currentTarget.value); setRebaseApproval(null); }} spellCheck={false} /><div className="resource-revision-actions"><span>{formatBytes(new TextEncoder().encode(draftText).byteLength)} · unsaved draft</span><button className="primary-button" type="button" disabled={busy || statusRequest !== null || (rebaseRequired && !rebaseIsApproved)} onClick={() => void saveText()}>{busy ? `Uploading ${formatBytes(uploadBytes)}…` : rebaseRequired ? "Save authorized draft as new revision" : "Save as new revision"}</button></div></div>}
        {busy && <p className="inline-status" role="status">Uploading {formatBytes(uploadBytes)} of {formatBytes(selectedFile?.size ?? new TextEncoder().encode(draftText ?? "").byteLength)}. This transfer uses the authenticated local Runtime.</p>}
      </div>}
    </>}
    <div className="resource-revision-history">
      <div className="section-heading"><div><h3>Revision history</h3><p>Parent IDs show the exact ancestry pinned by each revision.</p></div><button className="quiet-button" type="button" disabled={loading || busy} onClick={() => void reload(false)}>Refresh</button></div>
      {history.length === 0 && !loading ? <p className="empty-inline">No revision history is available.</p> : <ol>{history.map((revision) => <li key={revision.resourceRevisionId}>
        <div><strong>{revision.isHead ? "Current head" : "Revision"} · {revision.resourceRevisionId}</strong><small>{revision.observedAt} · {formatBytes(revision.sizeBytes ?? 0)} · {revision.mediaType ?? "Unknown media type"}</small><small>Parents: {revision.parentRevisionIds.length ? revision.parentRevisionIds.join(", ") : "None (initial revision)"}</small></div>
      </li>)}</ol>}
      {cursor && <button className="quiet-button" type="button" disabled={loading || busy} onClick={() => void reload(true)}>Load more history</button>}
      <section className="resource-revision-compare" aria-labelledby="resource-revision-compare-title">
        <h3 id="resource-revision-compare-title">Compare text revisions</h3>
        <p>Choose both saved revisions explicitly. Comparison reads only these exact revisions and displays plain text; HTML and SVG are never rendered.</p>
        <button className="quiet-button" type="button" aria-expanded={compareOpen} onClick={() => { const nextOpen = !compareOpen; setCompareOpen(nextOpen); comparisonRequest.current += 1; setCompareBusy(false); setCompareError(null); if (!nextOpen) setComparison(null); }}>
          {compareOpen ? "Hide comparison" : "Compare revisions"}
        </button>
        {compareOpen && <div className="resource-revision-compare__controls">
          <label>First saved revision
            <select value={compareLeftId} onChange={(event) => changeComparisonPin("left", event.currentTarget.value)} disabled={compareBusy}>
              <option value="">Choose a revision</option>
              {history.filter((revision) => revision.resourceId === resource.resourceId && isComparableRevision(revision)).map((revision) => <option key={revision.resourceRevisionId} value={revision.resourceRevisionId}>{revision.isHead ? "Current head" : "Revision"} · {revision.resourceRevisionId} · {revision.mediaType}</option>)}
            </select>
          </label>
          <label>Second saved revision
            <select value={compareRightId} onChange={(event) => changeComparisonPin("right", event.currentTarget.value)} disabled={compareBusy}>
              <option value="">Choose a revision</option>
              {history.filter((revision) => revision.resourceId === resource.resourceId && isComparableRevision(revision)).map((revision) => <option key={revision.resourceRevisionId} value={revision.resourceRevisionId}>{revision.isHead ? "Current head" : "Revision"} · {revision.resourceRevisionId} · {revision.mediaType}</option>)}
            </select>
          </label>
          <button className="primary-button" type="button" disabled={compareBusy || !compareLeftId || !compareRightId || compareLeftId === compareRightId} onClick={() => void compareSelectedRevisions()}>
            {compareBusy ? "Loading exact revisions…" : "Compare selected revisions"}
          </button>
        </div>}
        {compareError && <p className="inline-error" role="alert">{compareError}</p>}
        {comparison && <div className="resource-revision-compare__panes" aria-label="Selected Resource revision text comparison">
          <section><h4>First · {comparison.left.resourceRevisionId}</h4><p>{comparison.left.observedAt} · {comparison.left.mediaType} · {formatBytes(comparison.left.sizeBytes ?? 0)}</p><pre tabIndex={0} aria-label={`Plain-text contents of Resource revision ${comparison.left.resourceRevisionId}`}>{comparison.leftText}</pre></section>
          <section><h4>Second · {comparison.right.resourceRevisionId}</h4><p>{comparison.right.observedAt} · {comparison.right.mediaType} · {formatBytes(comparison.right.sizeBytes ?? 0)}</p><pre tabIndex={0} aria-label={`Plain-text contents of Resource revision ${comparison.right.resourceRevisionId}`}>{comparison.rightText}</pre></section>
        </div>}
      </section>
    </div>
  </section>;
}

function isSmallText(mediaType: string, sizeBytes: number): boolean {
  const type = mediaType.toLowerCase().split(";", 1)[0].trim();
  return sizeBytes <= MAX_EDIT_TEXT_BYTES && (type.startsWith("text/") || ["application/json", "application/xml", "application/yaml", "application/x-yaml", "application/javascript"].includes(type));
}

function isComparableRevision(revision: Revision): boolean {
  const mediaType = revision.mediaType?.toLowerCase().split(";", 1)[0].trim();
  return revision.resourceId.length > 0
    && revision.resourceRevisionId.length > 0
    && revision.contentDigest !== null
    && /^sha256:[0-9a-f]{64}$/.test(revision.contentDigest)
    && revision.sizeBytes !== null
    && Number.isSafeInteger(revision.sizeBytes)
    && revision.sizeBytes >= 0
    && revision.sizeBytes <= MAX_EDIT_TEXT_BYTES
    && (mediaType === "text/plain" || mediaType === "text/markdown" || mediaType === "text/x-markdown");
}

async function digestHex(buffer: ArrayBuffer): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", buffer);
  return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("");
}

function bytesToBase64(bytes: Uint8Array): string {
  let binary = "";
  for (let offset = 0; offset < bytes.length; offset += 0x8000) binary += String.fromCharCode(...bytes.subarray(offset, Math.min(offset + 0x8000, bytes.length)));
  return btoa(binary);
}

function formatBytes(size: number): string {
  if (!size) return "0 B";
  return size < 1024 ? `${size} B` : size < 1024 * 1024 ? `${Math.round(size / 1024)} KB` : `${(size / (1024 * 1024)).toFixed(1)} MB`;
}

function messageOf(error: unknown, fallback: string): string {
  return typeof error === "string" ? error : error instanceof Error ? error.message : fallback;
}
