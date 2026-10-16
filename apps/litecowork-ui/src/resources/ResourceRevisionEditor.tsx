import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { resourceLocationApi, resourceLocationStateForSelection, type ResourceLocationView, type ResourceLocationLoadState } from "./resource-location-api";
import {
  contextDocumentDeletionNeedsPolling,
  contextDocumentDeletionProgress,
  contextDocumentDeletionRejectionCode,
  contextDocumentDeletionReducer,
  decodeContextDocumentDeletionStatus,
  isWorkspaceNoteOwnedBySelectedWorkspace,
  newContextDocumentDeletionState,
  restoreContextDocumentDeletionFocus,
  type ContextDocumentDeletionRequest,
  type ContextDocumentDeletionScope,
  type ContextDocumentDeletionStatus,
} from "./context-document-deletion";
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
type ResourceIdentity = Pick<Resource, "workspaceId" | "resourceId">;
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

export function resourceNavigationDecision(
  current: ResourceIdentity,
  requestedResource: ResourceIdentity,
  requestedWorkspaceId: string,
  hasUnsavedDraft: boolean,
  operationPending: boolean,
): "SAME" | "WAIT" | "CONFIRM" | "SWITCH_RESOURCE" | "CLOSE_WORKSPACE" {
  if (current.workspaceId === requestedWorkspaceId
    && current.workspaceId === requestedResource.workspaceId
    && current.resourceId === requestedResource.resourceId) return "SAME";
  if (operationPending) return "WAIT";
  if (hasUnsavedDraft) return "CONFIRM";
  return requestedWorkspaceId === current.workspaceId ? "SWITCH_RESOURCE" : "CLOSE_WORKSPACE";
}

export function isCurrentResourceSelection(
  requested: ResourceIdentity,
  current: ResourceIdentity,
  requestGeneration: number,
  currentGeneration: number,
): boolean {
  return requested.workspaceId === current.workspaceId
    && requested.resourceId === current.resourceId
    && requestGeneration === currentGeneration;
}

export function ResourceRevisionEditor({ resource: requestedResource, activeWorkspaceId, onClose, onCommitted, onSelectResource, onSelectWorkspace, onNavigationSaveFailure }: {
  resource: Resource;
  activeWorkspaceId: string;
  onClose: () => void;
  onCommitted: (resource: Resource) => void;
  onSelectResource: (resource: Resource) => void;
  onSelectWorkspace: (workspaceId: string) => void;
  onNavigationSaveFailure?: (message: string) => void;
}) {
  const [resource, setResource] = useState(requestedResource);
  const [pendingNavigation, setPendingNavigation] = useState<{
    key: string;
    requestedWorkspaceId: string;
    requestedResource: Resource;
    requiresContinue: boolean;
  } | null>(null);
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
  const [deletion, dispatchDeletion] = useReducer(
    contextDocumentDeletionReducer,
    { workspaceId: requestedResource.workspaceId, resourceId: requestedResource.resourceId, generation: 0 },
    newContextDocumentDeletionState,
  );
  const [pendingDeletionDetailRefresh, setPendingDeletionDetailRefresh] = useState<string | null>(null);
  const pendingDeletionDetailRefreshRef = useRef<string | null>(null);
  const [deletionConfirmation, setDeletionConfirmation] = useState(false);
  const deletionTriggerRef = useRef<HTMLButtonElement | null>(null);
  const deletionConfirmRef = useRef<HTMLButtonElement | null>(null);
  const deletionPanelRef = useRef<HTMLElement | null>(null);
  const deletionMountedRef = useRef(false);
  const [compareOpen, setCompareOpen] = useState(false);
  const [compareLeftId, setCompareLeftId] = useState("");
  const [compareRightId, setCompareRightId] = useState("");
  const [compareBusy, setCompareBusy] = useState(false);
  const [compareError, setCompareError] = useState<string | null>(null);
  const [comparison, setComparison] = useState<RevisionComparison | null>(null);
  const comparisonRequest = useRef(0);
  const resourceReadRequest = useRef(0);
  const resourceSelectionGeneration = useRef(0);
  const deletionStatusReadSequence = useRef(0);
  const selectedResourceRef = useRef({ workspaceId: requestedResource.workspaceId, resourceId: requestedResource.resourceId });
  const headPreviewRequest = useRef(0);
  const [locationState, setLocationState] = useState<ResourceLocationLoadState | null>(null);
  const locationsRequest = useRef(0);
  const selectedLocationState = resourceLocationStateForSelection(
    locationState,
    resource.workspaceId,
    resource.resourceId,
  );
  const hasUnsavedDraft = selectedFile !== null || (draftText !== null && draftText !== textBaseline);
  const operationPending = busy || statusBusy || statusRequest !== null || deletion.busy || deletion.request !== null;
  const navigationDecision = resourceNavigationDecision(
    resource,
    requestedResource,
    activeWorkspaceId,
    hasUnsavedDraft,
    operationPending,
  );
  const navigationKey = JSON.stringify([activeWorkspaceId, requestedResource.workspaceId, requestedResource.resourceId]);

  useEffect(() => {
    deletionMountedRef.current = true;
    return () => { deletionMountedRef.current = false; };
  }, []);

  const switchResource = (next: Resource) => {
    selectedResourceRef.current = { workspaceId: next.workspaceId, resourceId: next.resourceId };
    resourceSelectionGeneration.current += 1;
    deletionStatusReadSequence.current += 1;
    pendingDeletionDetailRefreshRef.current = null;
    setPendingDeletionDetailRefresh(null);
    dispatchDeletion({ type: "select", scope: { workspaceId: next.workspaceId, resourceId: next.resourceId, generation: resourceSelectionGeneration.current } });
    resourceReadRequest.current += 1;
    headPreviewRequest.current += 1;
    comparisonRequest.current += 1;
    locationsRequest.current += 1;
    setResource(next);
    setDetail(null);
    setHistory([]);
    setCursor(null);
    setLoading(true);
    setMessage(null);
    setError(null);
    setSelectedFile(null);
    setPendingFileReplacement(null);
    setDraftText(null);
    setTextBaseline(null);
    setConfirmDiscard(false);
    setConfirmReplaceDraft(false);
    setRebaseRequired(false);
    setRebaseReloadPending(false);
    setRebaseApproval(null);
    setReviewedHeadText(null);
    setHeadPreviewBusy(false);
    setHeadPreviewError(null);
    setUploadBytes(0);
    setUploadRecovery(revisionUploadRecoveries.get(revisionUploadKey(next.workspaceId, next.resourceId)) ?? null);
    setStatusConfirmation(null);
    setStatusRequest(null);
    setStatusError(null);
    setDeletionConfirmation(false);
    setCompareOpen(false);
    setCompareLeftId("");
    setCompareRightId("");
    setCompareBusy(false);
    setCompareError(null);
    setComparison(null);
    setLocationState(null);
    setPendingNavigation(null);
  };

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
    locationsRequest.current += 1;
    setLocationState(null);
    return () => { locationsRequest.current += 1; };
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

  const reload = useCallback(async (
    append = false,
    expectedContextStatus?: ContextDocumentDeletionStatus["status"],
  ) => {
    const request = ++resourceReadRequest.current;
    const requestGeneration = resourceSelectionGeneration.current;
    const selection = { workspaceId: resource.workspaceId, resourceId: resource.resourceId };
    setLoading(true);
    setError(null);
    try {
      const [nextDetail, page] = await Promise.all([
        invoke<Detail>("get_resource_detail", { workspaceId: resource.workspaceId, resourceId: resource.resourceId }),
        invoke<RevisionPage>("list_resource_revisions", { workspaceId: resource.workspaceId, resourceId: resource.resourceId, cursor: append ? cursor : null }),
      ]);
      if (!isCurrentResourceSelection(selection, selectedResourceRef.current, requestGeneration, resourceSelectionGeneration.current)
        || request !== resourceReadRequest.current) return false;
      if (nextDetail.workspaceId !== resource.workspaceId || nextDetail.resourceId !== resource.resourceId) throw new Error("Resource response did not match this Workspace.");
      if (expectedContextStatus !== undefined && nextDetail.contextDocument?.status !== expectedContextStatus) {
        throw new Error("Resource details have not caught up with the confirmed ContextDocument status.");
      }
      setDetail(nextDetail);
      setHistory((current) => append ? [...current, ...page.items] : page.items);
      setCursor(page.nextCursor);
      if (!append) setRebaseReloadPending(false);
      return true;
    } catch (cause) {
      if (isCurrentResourceSelection(selection, selectedResourceRef.current, requestGeneration, resourceSelectionGeneration.current)
        && request === resourceReadRequest.current) {
        setError(messageOf(cause, "Resource details and revision history could not be loaded."));
      }
      return false;
    } finally {
      if (isCurrentResourceSelection(selection, selectedResourceRef.current, requestGeneration, resourceSelectionGeneration.current)
        && request === resourceReadRequest.current) setLoading(false);
    }
  }, [cursor, resource.resourceId, resource.workspaceId]);

  useEffect(() => {
    setDetail(null);
    setHistory([]);
    setCursor(null);
    setLoading(true);
    setError(null);
    void reload(false);
    return () => { resourceReadRequest.current += 1; };
  }, [resource.resourceId, resource.workspaceId]);

  const context = detail?.contextDocument ?? null;
  const canDeleteContextDocument = isWorkspaceNoteOwnedBySelectedWorkspace(context, resource.workspaceId);
  const deletionStateMatchesResource = deletion.scope.workspaceId === resource.workspaceId
    && deletion.scope.resourceId === resource.resourceId
    && deletion.scope.generation === resourceSelectionGeneration.current;
  const contextStatus = deletionStateMatchesResource
    ? deletion.status?.status === "DELETION_PENDING" || deletion.status?.status === "DELETED"
      ? deletion.status.status
      : context?.status ?? deletion.status?.status
    : context?.status;
  const currentDeletionDetailRefreshKey = deletionStateMatchesResource && deletion.status
    ? JSON.stringify([deletion.scope.workspaceId, deletion.scope.resourceId, deletion.scope.generation, deletion.status.status])
    : null;
  const terminalDeletionDetailRefreshPending = deletion.status?.status === "DELETED"
    && currentDeletionDetailRefreshKey !== null
    && pendingDeletionDetailRefresh === currentDeletionDetailRefreshKey;
  const isCurrentTextEditable = isSmallText(resource.mediaType, resource.sizeBytes);
  const recoveryMatchesCurrentHead = !!(detail && uploadRecovery && sameRevisionUploadPins(uploadRecovery, resource, detail));
  const rebaseIsApproved = rebaseRequired && !rebaseReloadPending && detail?.currentRevisionId !== null
    && detail?.currentRevisionId !== undefined && rebaseApproval?.resourceVersion === detail.version
    && rebaseApproval.headId === detail.currentRevisionId;
  const headTextMatchesCurrent = reviewedHeadText !== null && reviewedHeadText.resourceVersion === detail?.version
    && reviewedHeadText.headId === detail?.currentRevisionId;
  const canSaveDraft = hasUnsavedDraft && contextStatus === "ACTIVE" && detail?.currentRevisionId !== null
    && detail?.currentRevisionId !== undefined && !operationPending && (!rebaseRequired || rebaseIsApproved)
    && (selectedFile !== null || draftText !== null);
  const notifyNavigationSaveFailure = (message: string) => {
    window.requestAnimationFrame(() => onNavigationSaveFailure?.(message));
  };

  useEffect(() => {
    if (navigationDecision === "SAME") {
      if (pendingNavigation !== null) setPendingNavigation(null);
      if (!hasUnsavedDraft && !operationPending
        && (resource.displayName !== requestedResource.displayName
          || resource.mediaType !== requestedResource.mediaType
          || resource.sizeBytes !== requestedResource.sizeBytes
          || resource.resourceRevisionId !== requestedResource.resourceRevisionId)) {
        setResource(requestedResource);
      }
      return;
    }
    if (navigationDecision === "WAIT" || navigationDecision === "CONFIRM") {
      setPendingNavigation((current) => current?.key === navigationKey
        ? current
        : {
          key: navigationKey,
          requestedWorkspaceId: activeWorkspaceId,
          requestedResource,
          requiresContinue: navigationDecision === "WAIT",
        });
      return;
    }
    if (pendingNavigation?.key === navigationKey && pendingNavigation.requiresContinue) return;
    if (navigationDecision === "SWITCH_RESOURCE") {
      switchResource(requestedResource);
    } else if (navigationDecision === "CLOSE_WORKSPACE") {
      onClose();
    }
  }, [activeWorkspaceId, hasUnsavedDraft, navigationDecision, navigationKey, onClose, operationPending,
    pendingNavigation, requestedResource, resource.displayName, resource.mediaType, resource.resourceRevisionId,
    resource.sizeBytes]);

  const stayOnCurrentResource = () => {
    setPendingNavigation(null);
    if (activeWorkspaceId !== resource.workspaceId) onSelectWorkspace(resource.workspaceId);
    else onSelectResource(resource);
  };

  const continuePendingNavigation = () => {
    if (!pendingNavigation || operationPending || hasUnsavedDraft) return;
    const destinationWorkspaceId = pendingNavigation.requestedWorkspaceId;
    if (destinationWorkspaceId !== resource.workspaceId) {
      setPendingNavigation(null);
      onClose();
      return;
    }
    switchResource(pendingNavigation.requestedResource);
  };

  const discardDraftAndSwitch = () => {
    if (!pendingNavigation || operationPending) return;
    const destinationWorkspaceId = pendingNavigation.requestedWorkspaceId;
    const destination = pendingNavigation.requestedResource;
    setSelectedFile(null);
    setPendingFileReplacement(null);
    setDraftText(null);
    setTextBaseline(null);
    if (destinationWorkspaceId !== resource.workspaceId) {
      setPendingNavigation(null);
      onClose();
    } else {
      switchResource(destination);
    }
  };

  const saveDraftAndSwitch = () => {
    if (!pendingNavigation || operationPending || !hasUnsavedDraft || (rebaseRequired && !rebaseIsApproved)) return;
    setPendingNavigation((current) => current?.key === navigationKey ? { ...current, requiresContinue: false } : current);
    if (selectedFile) void saveRevision(selectedFile);
    else void saveText();
  };

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
    if (operationPending) return;
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
    const request = ++headPreviewRequest.current;
    const requestGeneration = resourceSelectionGeneration.current;
    const selection = { workspaceId: resource.workspaceId, resourceId: resource.resourceId };
    setHeadPreviewBusy(true);
    setHeadPreviewError(null);
    setReviewedHeadText(null);
    try {
      const text = await invoke<string>("preview_resource_text", {
        workspaceId: resource.workspaceId,
        resourceId: resource.resourceId,
        revisionId: detail.currentRevisionId,
      });
      if (request !== headPreviewRequest.current
        || !isCurrentResourceSelection(selection, selectedResourceRef.current, requestGeneration, resourceSelectionGeneration.current)) return;
      setReviewedHeadText({ resourceVersion: detail.version, headId: detail.currentRevisionId, text });
    } catch (cause) {
      if (request === headPreviewRequest.current
        && isCurrentResourceSelection(selection, selectedResourceRef.current, requestGeneration, resourceSelectionGeneration.current)) {
        setHeadPreviewError(messageOf(cause, "The current Resource head could not be previewed."));
      }
    } finally {
      if (request === headPreviewRequest.current
        && isCurrentResourceSelection(selection, selectedResourceRef.current, requestGeneration, resourceSelectionGeneration.current)) {
        setHeadPreviewBusy(false);
      }
    }
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

  const loadResourceLocations = async () => {
    const request = ++locationsRequest.current;
    const selection = { workspaceId: resource.workspaceId, resourceId: resource.resourceId };
    setLocationState({ ...selection, busy: true, error: null, locations: null });
    try {
      const locations = await resourceLocationApi.list(selection.workspaceId, selection.resourceId);
      if (request === locationsRequest.current) {
        setLocationState({ ...selection, busy: false, error: null, locations });
      }
    } catch (cause) {
      if (request === locationsRequest.current) {
        setLocationState({
          ...selection,
          busy: false,
          error: messageOf(cause, "Saved Resource locations are unavailable."),
          locations: null,
        });
      }
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
      dispatchDeletion({ type: "select", scope: {
        workspaceId: resource.workspaceId,
        resourceId: resource.resourceId,
        generation: resourceSelectionGeneration.current,
      } });
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
    if (!detail || !context || hasUnsavedDraft || statusRequest !== null || deletion.request !== null
      || contextStatus !== (target === "REVOKED" ? "ACTIVE" : "REVOKED")) return;
    const request = { target, expectedVersion: detail.version, requestId: crypto.randomUUID() };
    setStatusRequest(request);
    void commitContextStatus(request);
  };

  const deletionScopeIsCurrent = (scope: ContextDocumentDeletionScope) =>
    deletionMountedRef.current && isCurrentResourceSelection(
      { workspaceId: scope.workspaceId, resourceId: scope.resourceId },
      selectedResourceRef.current,
      scope.generation,
      resourceSelectionGeneration.current,
    );

  const showDeletionStatus = (scope: ContextDocumentDeletionScope, status: ContextDocumentDeletionStatus) => {
    if (!deletionScopeIsCurrent(scope)) return false;
    const matchingDetail = detail?.workspaceId === scope.workspaceId && detail.resourceId === scope.resourceId;
    if (!matchingDetail || (status.status !== "DELETION_PENDING" && status.status !== "DELETED")) return false;
    const refreshKey = JSON.stringify([scope.workspaceId, scope.resourceId, scope.generation, status.status]);
    const refreshResourceDetail = detail?.contextDocument?.status !== status.status
      || pendingDeletionDetailRefreshRef.current === refreshKey;
    if (refreshResourceDetail && pendingDeletionDetailRefreshRef.current !== refreshKey) {
      pendingDeletionDetailRefreshRef.current = refreshKey;
      setPendingDeletionDetailRefresh(refreshKey);
    }
    return refreshResourceDetail;
  };

  const refreshDeletionResourceDetail = async (
    scope: ContextDocumentDeletionScope,
    status: ContextDocumentDeletionStatus,
  ) => {
    if (!showDeletionStatus(scope, status)) return;
    const refreshKey = JSON.stringify([scope.workspaceId, scope.resourceId, scope.generation, status.status]);
    if (await reload(false, status.status) && deletionScopeIsCurrent(scope)
      && pendingDeletionDetailRefreshRef.current === refreshKey) {
      pendingDeletionDetailRefreshRef.current = null;
      setPendingDeletionDetailRefresh(null);
    }
  };

  const readDeletionStatus = async (scope: ContextDocumentDeletionScope) => {
    if (!deletionScopeIsCurrent(scope)) return null;
    const sequence = ++deletionStatusReadSequence.current;
    dispatchDeletion({ type: "statusStarted", scope, sequence });
    try {
      const value = await invoke<unknown>("get_context_document_deletion_status", {
        workspaceId: scope.workspaceId,
        resourceId: scope.resourceId,
      });
      const status = decodeContextDocumentDeletionStatus(value, scope.resourceId);
      if (sequence === deletionStatusReadSequence.current && deletionScopeIsCurrent(scope)) {
        dispatchDeletion({ type: "statusReceived", scope, sequence, status });
        await refreshDeletionResourceDetail(scope, status);
      }
      return status;
    } catch (cause) {
      if (sequence === deletionStatusReadSequence.current && deletionScopeIsCurrent(scope)) {
        dispatchDeletion({ type: "statusFailed", scope, sequence, error: messageOf(cause, "ContextDocument deletion status could not be read.") });
      }
      return null;
    }
  };

  const commitContextDocumentDeletion = async (request: ContextDocumentDeletionRequest) => {
    if (!deletionScopeIsCurrent(request)) return;
    dispatchDeletion({ type: "postStarted", request });
    try {
      const value = await invoke<unknown>("begin_context_document_deletion", {
        workspaceId: request.workspaceId,
        resourceId: request.resourceId,
        expectedVersion: request.expectedVersion,
        requestId: request.requestId,
      });
      const status = decodeContextDocumentDeletionStatus(value, request.resourceId);
      if (status.status !== "DELETION_PENDING" && status.status !== "DELETED") {
        throw new Error("Local Runtime did not confirm ContextDocument deletion admission.");
      }
      if (deletionScopeIsCurrent(request)) {
        dispatchDeletion({ type: "postAccepted", request, status });
        setMessage(status.status === "DELETED"
          ? "Local deletion is complete. Resource and revision metadata remain for provenance; content already delivered to an active native agent session cannot be recalled."
          : "Local deletion was admitted. LiteCowork is removing the registered local content and index objects; already-delivered native session content cannot be recalled.");
        await refreshDeletionResourceDetail(request, status);
      }
    } catch (cause) {
      if (!deletionScopeIsCurrent(request)) return;
      const errorText = messageOf(cause, "ContextDocument deletion outcome is unknown.");
      const rejectionCode = contextDocumentDeletionRejectionCode(errorText);
      if (rejectionCode !== null) {
        dispatchDeletion({ type: "postRejected", request, error: errorText });
        await readDeletionStatus(request);
        if (rejectionCode === "RESOURCE_CONFLICT") {
          await reload(false);
          if (deletionScopeIsCurrent(request)) {
            setMessage("No deletion was admitted because the Resource changed. Review its refreshed status and version before requesting deletion again.");
          }
        } else if (deletionScopeIsCurrent(request)) {
          setMessage(errorText);
        }
      } else {
        dispatchDeletion({ type: "postFailed", request, error: errorText });
        await readDeletionStatus(request);
      }
    }
  };

  const beginContextDocumentDeletion = () => {
    if (!detail || !context || (contextStatus !== "ACTIVE" && contextStatus !== "REVOKED")
      || hasUnsavedDraft || operationPending || statusRequest !== null) return;
    const request: ContextDocumentDeletionRequest = {
      workspaceId: resource.workspaceId,
      resourceId: resource.resourceId,
      generation: resourceSelectionGeneration.current,
      expectedVersion: detail.version,
      requestId: crypto.randomUUID(),
    };
    dispatchDeletion({ type: "begin", request });
    setDeletionConfirmation(false);
    void commitContextDocumentDeletion(request);
  };

  useEffect(() => {
    if (!deletionConfirmation) return;
    const trigger = deletionTriggerRef.current;
    deletionConfirmRef.current?.focus();
    return () => {
      restoreContextDocumentDeletionFocus(trigger, deletionPanelRef.current,
        (target) => document.activeElement === (target as unknown as Element));
    };
  }, [deletionConfirmation]);

  useEffect(() => {
    const detailStatus = detail?.contextDocument?.status;
    const currentStatus = deletionStateMatchesResource ? deletion.status?.status : null;
    if (currentStatus !== "DELETED"
      && (detailStatus === "DELETION_PENDING" || detailStatus === "DELETED")
      && currentStatus !== detailStatus && !deletion.checkingStatus) {
      void readDeletionStatus({
        workspaceId: resource.workspaceId,
        resourceId: resource.resourceId,
        generation: resourceSelectionGeneration.current,
      });
    }
  }, [deletion.checkingStatus, deletion.status?.status, detail?.contextDocument?.status,
    deletionStateMatchesResource, resource.resourceId, resource.workspaceId]);

  useEffect(() => {
    if (!contextDocumentDeletionNeedsPolling(deletion.status)) return;
    const scope = deletion.scope;
    let cancelled = false;
    let timer: number | undefined;
    let latestStatus = deletion.status;
    const poll = async () => {
      if (cancelled || !deletionScopeIsCurrent(scope)) return;
      const sequence = ++deletionStatusReadSequence.current;
      dispatchDeletion({ type: "statusStarted", scope, sequence });
      try {
        const value = await invoke<unknown>("get_context_document_deletion_status", {
          workspaceId: scope.workspaceId,
          resourceId: scope.resourceId,
        });
        const status = decodeContextDocumentDeletionStatus(value, scope.resourceId);
        if (!cancelled && sequence === deletionStatusReadSequence.current && deletionScopeIsCurrent(scope)) {
          latestStatus = status;
          dispatchDeletion({ type: "statusReceived", scope, sequence, status });
          await refreshDeletionResourceDetail(scope, status);
        }
      } catch (cause) {
        if (!cancelled && sequence === deletionStatusReadSequence.current && deletionScopeIsCurrent(scope)) {
          dispatchDeletion({ type: "statusFailed", scope, sequence, error: messageOf(cause, "ContextDocument deletion status could not be read.") });
        }
      }
      if (!cancelled && contextDocumentDeletionNeedsPolling(latestStatus)) {
        timer = window.setTimeout(() => void poll(), 1500);
      }
    };
    timer = window.setTimeout(() => void poll(), 1500);
    return () => {
      cancelled = true;
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, [deletion.status, deletion.scope, resource.resourceId, resource.workspaceId]);

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
    if (file.size > MAX_REVISION_BYTES) {
      const message = "A revision file cannot exceed 100 MiB.";
      setError(message);
      notifyNavigationSaveFailure(message);
      return;
    }
    setBusy(true); setError(null); setMessage(null); setUploadBytes(0);
    let navigationSaveFailure: string | null = null;
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
        setResource(committed);
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
        offset = end;
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
      setResource(committed);
      onCommitted(committed);
      await reload(false);
      setMessage("New Resource revision saved.");
    } catch (cause) {
      const text = messageOf(cause, "Resource revision could not be saved.");
      navigationSaveFailure = text;
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
    } finally {
      setBusy(false);
      const failure = navigationSaveFailure;
      if (failure) notifyNavigationSaveFailure(failure);
    }
  };

  const saveText = async () => {
    if (draftText === null) return;
    const encoded = new TextEncoder().encode(draftText);
    if (encoded.byteLength > MAX_EDIT_TEXT_BYTES) {
      const message = "Text edits are limited to 1 MiB.";
      setError(message);
      notifyNavigationSaveFailure(message);
      return;
    }
    const blob = new Blob([encoded], { type: resource.mediaType || "text/plain" });
    await saveRevision(new File([blob], resource.displayName, { type: resource.mediaType || "text/plain" }));
  };

  return <section
    className="resource-revision-panel"
    aria-labelledby="resource-revision-title"
    data-library-draft={hasUnsavedDraft ? "DIRTY" : "CLEAN"}
    data-library-operation-pending={operationPending ? "true" : "false"}
    data-library-can-save={canSaveDraft ? "true" : "false"}
  >
    <header className="resource-revision-header">
      <div><p className="eyebrow">RESOURCE HISTORY</p><h2 id="resource-revision-title">{resource.displayName}</h2><p>Each save creates a new immutable revision.</p></div>
      <button className="quiet-button" type="button" onClick={requestClose} disabled={operationPending}>{hasUnsavedDraft ? "Close editor…" : "Close"}</button>
    </header>
    {pendingNavigation && <div className="resource-discard-backdrop">
      <section className="resource-discard-dialog" role="alertdialog" aria-modal="true" aria-labelledby="resource-navigation-title" aria-describedby="resource-navigation-description" onKeyDown={(event) => {
        if (event.key === "Escape") {
          event.preventDefault();
          stayOnCurrentResource();
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
        <h3 id="resource-navigation-title">Leave {resource.displayName}?</h3>
        <p id="resource-navigation-description">{operationPending
          ? "A Resource save or status request is still resolving. Wait for its result before changing the editor target. Your current draft stays here."
          : hasUnsavedDraft
            ? "This exact local draft belongs to the current Resource. Choose what to do before opening the selected Resource or Workspace."
            : "A request for the current Resource just finished. Continue only after confirming the selected destination."}</p>
        <div className="resource-discard-actions">
          <button className="quiet-button" type="button" autoFocus onClick={stayOnCurrentResource}>Stay on current Resource</button>
          {hasUnsavedDraft ? <>
            <button className="primary-button" type="button" disabled={operationPending || (rebaseRequired && !rebaseIsApproved)} onClick={saveDraftAndSwitch}>Save draft and switch</button>
            <button className="primary-button" type="button" disabled={operationPending} onClick={discardDraftAndSwitch}>Discard draft and switch</button>
          </> : <button className="primary-button" type="button" disabled={operationPending} onClick={continuePendingNavigation}>
            {pendingNavigation.requestedWorkspaceId === resource.workspaceId ? "Continue to selected Resource" : "Continue to selected Workspace"}
          </button>}
        </div>
      </section>
    </div>}
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
    {deletionConfirmation && <div className="resource-discard-backdrop">
      <section className="resource-discard-dialog" role="alertdialog" aria-modal="true" aria-labelledby="resource-deletion-confirm-title" aria-describedby="resource-deletion-confirm-description" onKeyDown={(event) => {
        if (event.key === "Escape") {
          event.preventDefault();
          setDeletionConfirmation(false);
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
        <h3 id="resource-deletion-confirm-title">Permanently delete local content?</h3>
        <p id="resource-deletion-confirm-description">LiteCowork will permanently remove this Resource’s content bytes from encrypted local storage and its Core-managed derived indexes. Any in-progress revision uploads for this Resource will expire, and their staged chunks will be removed. Resource and revision identities and digest metadata remain for provenance. The request is rejected if any revision has an external or provider-backed location. Content already delivered to an active native agent session cannot be recalled, and this action does not stop that session.</p>
        <div className="resource-discard-actions">
          <button className="quiet-button" type="button" onClick={() => setDeletionConfirmation(false)}>Cancel</button>
          <button ref={deletionConfirmRef} className="primary-button resource-context-delete-confirm" type="button" onClick={beginContextDocumentDeletion}>Permanently delete local content</button>
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
        <span><strong>Context status</strong>{contextStatus ?? "Not a ContextDocument"}</span>
      </div>
      <section className="resource-location-observations" aria-labelledby="resource-location-observations-title">
        <div className="section-heading">
          <div><h3 id="resource-location-observations-title">Saved locations</h3><p>Availability and freshness come from stored observations; loading this list does not contact a provider.</p></div>
          <button className="quiet-button" type="button" disabled={selectedLocationState?.busy} onClick={() => void loadResourceLocations()}>
            {selectedLocationState?.busy
              ? "Loading…"
              : selectedLocationState?.error
                ? "Try again"
                : selectedLocationState?.locations === null || !selectedLocationState
                  ? "Show locations"
                  : "Refresh locations"}
          </button>
        </div>
        {selectedLocationState?.busy && <p className="inline-status" role="status">Loading saved location observations…</p>}
        {selectedLocationState?.error && <p className="inline-error" role="alert">{selectedLocationState.error}</p>}
        {selectedLocationState?.locations !== null && selectedLocationState?.locations !== undefined
          && !selectedLocationState.busy && selectedLocationState.locations.length === 0 && !selectedLocationState.error
          && <p className="inline-status" role="status">No saved location observations are recorded for this Resource.</p>}
        {selectedLocationState?.locations && selectedLocationState.locations.length > 0 && <ul className="resource-location-list">
          {selectedLocationState.locations.map((location: ResourceLocationView) => <li key={location.location_id}>
            <h4>Location {location.location_id}</h4>
            <dl>
              <div><dt>Availability</dt><dd>{location.availability}</dd></div>
              <div><dt>Freshness</dt><dd>{location.freshness}</dd></div>
              <div><dt>Write access</dt><dd>{location.writable ? "Writable" : "Read-only"}</dd></div>
              {location.runtime_id && <div><dt>Runtime ID</dt><dd>{location.runtime_id}</dd></div>}
              {location.environment_id && <div><dt>Environment ID</dt><dd>{location.environment_id}</dd></div>}
              {location.connection_id && <div><dt>Connection ID</dt><dd>{location.connection_id}</dd></div>}
              <div><dt>Last observed</dt><dd>{location.observed_at}</dd></div>
              {location.last_checked_at && <div><dt>Observation checked</dt><dd>{location.last_checked_at}</dd></div>}
            </dl>
          </li>)}
        </ul>}
      </section>
      {context ? <p className="resource-context-owner">{context.kind} · owner-authored ContextDocument</p> : <p className="inline-status">Revision editing is available only for owner-authored ContextDocuments.</p>}
      {context && <section className="resource-context-status" aria-labelledby="resource-context-status-title">
        <div><h3 id="resource-context-status-title">Context availability</h3><p>Current status: <strong>{contextStatus}</strong></p></div>
        {contextStatus === "ACTIVE" || contextStatus === "REVOKED" ? <>
          <p>Revoking blocks future LiteCowork content reads and index updates, but retains the stored bytes. It cannot recall content already delivered to an agent session.</p>
          {hasUnsavedDraft && <p>Save or discard the local revision draft before changing ContextDocument availability.</p>}
          {statusRequest && !statusError && <p role="status">Waiting for the previous status request to resolve. Editing and closing are paused so its result can be confirmed or retried safely.</p>}
          {statusError && <p className="inline-error" role="alert">{statusError}</p>}
          {statusRequest && statusError
            ? <button className="quiet-button" type="button" disabled={statusBusy} onClick={() => void commitContextStatus(statusRequest)}>{statusBusy ? "Retrying…" : "Retry the same status request"}</button>
            : statusConfirmation
              ? <div className="resource-context-status-confirm"><p>{statusConfirmation === "REVOKED"
                ? "Revoke future reads? The retained content will remain stored; already-delivered content cannot be recalled."
                : "Restore future reads? This allows new LiteCowork reads of the retained content."}</p><button className="primary-button" type="button" disabled={statusBusy || busy || hasUnsavedDraft || statusRequest !== null || deletion.busy || deletion.request !== null} onClick={() => beginContextStatusChange(statusConfirmation)}>{statusBusy ? "Updating…" : statusConfirmation === "REVOKED" ? "Confirm revocation" : "Confirm restoration"}</button><button className="quiet-button" type="button" disabled={statusBusy || busy || statusRequest !== null || deletion.busy || deletion.request !== null} onClick={() => setStatusConfirmation(null)}>Cancel</button></div>
              : <button className="quiet-button" type="button" disabled={statusBusy || busy || hasUnsavedDraft || statusRequest !== null || deletion.busy || deletion.request !== null} onClick={() => setStatusConfirmation(contextStatus === "ACTIVE" ? "REVOKED" : "ACTIVE")}>{contextStatus === "ACTIVE" ? "Revoke future reads…" : "Restore future reads…"}</button>}
        </> : <p>This status is managed by the deletion lifecycle. Owner status changes are unavailable here.</p>}
      </section>}
      {context && contextStatus !== "ACTIVE" && <p className="inline-status">This ContextDocument is {contextStatus?.toLowerCase()}. Revision editing is unavailable.</p>}
      {canDeleteContextDocument && <section ref={deletionPanelRef} tabIndex={-1} className="resource-context-deletion" aria-labelledby="resource-context-deletion-title">
        <div><h3 id="resource-context-deletion-title">Permanent local deletion</h3>
          <p>This removes Core-managed local content and derived index objects. Resource and revision identity metadata remains for provenance. Already-delivered content cannot be recalled from a native agent session.</p>
        </div>
        {deletion.error && <p className="inline-error" role="alert">{deletion.error}</p>}
        {contextStatus === "ACTIVE" || contextStatus === "REVOKED" ? <>
          {hasUnsavedDraft && <p>Save or discard this local revision draft before requesting deletion.</p>}
          {deletion.request && !deletion.error && <p role="status">Waiting for the deletion request to resolve. The exact request identity is retained so a retry cannot create a duplicate.</p>}
          <div className="resource-context-deletion-actions">
            {deletion.request && deletion.error ? <>
              <button className="quiet-button" type="button" disabled={deletion.busy || deletion.checkingStatus} onClick={() => void commitContextDocumentDeletion(deletion.request!)}>{deletion.busy ? "Retrying…" : "Retry the same deletion request"}</button>
              <button className="quiet-button" type="button" disabled={deletion.checkingStatus} onClick={() => void readDeletionStatus(deletion.scope)}>{deletion.checkingStatus ? "Checking…" : "Check deletion status"}</button>
            </> : <button ref={deletionTriggerRef} className="quiet-button" type="button" disabled={operationPending || hasUnsavedDraft || statusRequest !== null} onClick={() => setDeletionConfirmation(true)}>
              {deletion.busy ? "Requesting deletion…" : "Delete local content…"}
            </button>}
          </div>
          {deletion.checkingStatus && <p className="inline-status" role="status">Checking the durable ContextDocument deletion status…</p>}
        </> : contextStatus === "DELETION_PENDING" ? <>
          <p className="inline-status" role="status">{deletion.status ? contextDocumentDeletionProgress(deletion.status) : "Deletion is pending. Checking durable removal receipts…"}</p>
          {deletion.checkingStatus && <p className="inline-status" role="status">Checking the durable ContextDocument deletion status…</p>}
          {deletion.error && <button className="quiet-button" type="button" disabled={deletion.checkingStatus} onClick={() => void readDeletionStatus(deletion.scope)}>{deletion.checkingStatus ? "Checking…" : "Retry status check"}</button>}
        </> : contextStatus === "DELETED" ? <>
          <p className="inline-status" role="status">Deletion is complete. The local encrypted content has been removed; Resource and revision metadata remains for provenance. Content already delivered to an active native agent session cannot be recalled.</p>
          {terminalDeletionDetailRefreshPending && <>
            <p className="inline-error" role="alert">Deletion is confirmed, but Resource details have not refreshed. The version shown is from the last successful read.</p>
            <button className="quiet-button" type="button" disabled={loading} onClick={() => deletion.status && void refreshDeletionResourceDetail(deletion.scope, deletion.status)}>
              {loading ? "Refreshing Resource details…" : "Retry Resource detail refresh"}
            </button>
          </>}
        </>
          : <p className="inline-status">Deletion status is unavailable.</p>}
      </section>}
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
      {contextStatus === "ACTIVE" && !detail.currentRevisionId && <p className="inline-status">This Resource has conflicting or unavailable revision heads. Resolve the conflict before editing; this screen will not select a branch for you.</p>}
      {contextStatus === "ACTIVE" && rebaseRequired && <section className="resource-rebase-review" aria-labelledby="resource-rebase-title">
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
      {contextStatus === "ACTIVE" && detail.currentRevisionId && <div className="resource-revision-edit">
        <h3>Create a revision</h3>
        <p>New uploads are pinned to Resource version {detail.version} and current head {detail.currentRevisionId}. A concurrent change blocks commit.</p>
        <label className="resource-revision-file">{uploadRecovery ? "Choose the original file to resume this revision upload" : "Choose a revised local file"}
          <input type="file" disabled={busy || statusRequest !== null} onChange={(event) => {
            const file = event.currentTarget.files?.[0] ?? null;
            event.currentTarget.value = "";
            if (file) stageReplacementFile(file);
          }} />
        </label>
        {selectedFile && <div className="resource-revision-staged"><span>{selectedFile.name} · {formatBytes(selectedFile.size)}</span><button className="primary-button" type="button" data-library-draft-save disabled={busy || statusRequest !== null || (rebaseRequired && !rebaseIsApproved)} onClick={() => void saveRevision(selectedFile)}>{busy ? `Uploading ${formatBytes(uploadBytes)} of ${formatBytes(selectedFile.size)}…` : uploadRecovery ? "Verify file and resume upload" : rebaseRequired ? "Save authorized draft as new revision" : "Upload as new revision"}</button></div>}
        {!selectedFile && isCurrentTextEditable && <button className="quiet-button" type="button" disabled={busy || hasUnsavedDraft || statusRequest !== null} onClick={() => void openTextEditor()}>{draftText === null ? "Edit small text file" : "Reload current text"}</button>}
        {draftText !== null && <div className="resource-revision-text-editor"><label htmlFor="resource-revision-text">Edit content (up to 1 MiB)</label><textarea id="resource-revision-text" value={draftText} disabled={busy || statusRequest !== null} onChange={(event) => { setDraftText(event.currentTarget.value); setRebaseApproval(null); }} spellCheck={false} /><div className="resource-revision-actions"><span>{formatBytes(new TextEncoder().encode(draftText).byteLength)} · unsaved draft</span><button className="primary-button" type="button" data-library-draft-save disabled={busy || statusRequest !== null || (rebaseRequired && !rebaseIsApproved)} onClick={() => void saveText()}>{busy ? `Uploading ${formatBytes(uploadBytes)}…` : rebaseRequired ? "Save authorized draft as new revision" : "Save as new revision"}</button></div></div>}
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
