import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { RichResponseView } from "../presentation/RichResponseView";
import { parseRichPresentationResponse, type ValidatedRichPresentation } from "../presentation/rich-presentation";
import type { PresentationStreamState, TransientTurn } from "../presentation/presentation-stream";
import { activityTurnForRead, appendConversationListPage, appendConversationPage, conversationDraftScope, ConversationCreateRequestCache, ConversationRequestEpochs, ConversationWorkspaceSelectionEpochs, mergeConversationStreamSnapshot, parseConversationListPage, parseConversationList, parseConversationSnapshot, parseConversationSendOutcome, parsePendingConversationSends, pendingConversationSendText, retainPendingConversationSend, restorePendingConversationAttachmentChips, richPresentationCacheKey, type ConversationMessageView, type ConversationSnapshotView, type ConversationSummary, type PendingConversationCreate, type PendingConversationFirstTurn, type PendingConversationSend } from "./conversation-view";
import { ConversationStreamController, type ConversationStreamDriver, type StreamEnvelope } from "./conversation-stream";
import { buildConversationAgentChoices, type ConversationAgentBindingOption, type ConversationAgentProfileOption } from "./conversation-agent-selection";
import {
  COMPOSER_PLACEHOLDER,
  canRetryTurn,
  contentSignature,
  conversationTitle,
  defaultAgentOptionLabel,
  isAtBottom,
  isSendEligible,
  isTurnOccupyingComposer,
  retryReasonLabel,
  retryTarget,
  scrollFollowDecision,
  stopTarget,
  turnStatusLabel,
} from "./conversation-turn-state";
import { addAttachment, attachmentChipLabel, attachmentRefusalNotice, classifyAttachmentCursor, mergeAttachmentCandidates, removeAttachment, toResourceRefs, type AttachmentCandidate, type AttachmentChip } from "./conversation-attachments";
import { TurnActivityView } from "./TurnActivityView";
import { mergeActivity, parseActivityPage, type ActivityEntry } from "./turn-activity";
import { NativePermissionCard } from "../needs-you/NativePermissionCard";
import { ConversationTaskPromotionCache, findTaskPromotionSource, isKnownNoWriteTaskError, taskPromotionKey, taskPromotionLeads, type PinnedConversationResourceRef, type TaskPromotionDraft, type TaskPromotionRequest, type TaskPromotionSource } from "./task-promotion";
import {
  applyNativePermissionReceipt,
  emptyNativePermissionState,
  endLiveNativePermissions,
  markNativePermissionOutcomeUnknown,
  parseConversationPermissionSnapshot,
  reconcileConversationPermissionSnapshot,
  requireNativePermissionSnapshot,
  reduceNativePermissionFrame,
  type NativePermissionDecision,
  type NativePermissionScope,
  type NativePermissionState,
} from "../needs-you/native-permission";
import "./conversations-page.css";

// Keep ambiguous create receipts across keyed Workspace page remounts for this app session.
const pendingCreates = new ConversationCreateRequestCache();
const conversationWorkspaceSelections = new ConversationWorkspaceSelectionEpochs();
const taskPromotionRequests = new ConversationTaskPromotionCache();

function messageText(message: ConversationMessageView): string {
  return message.content.map(block => block.kind === "TEXT" ? block.text : "Attachment").join("\n");
}

function semanticText(message: ConversationMessageView): string {
  return message.content.filter((block): block is { kind: "TEXT"; text: string } => block.kind === "TEXT").map(block => block.text).join("\n");
}

function ConversationProducerLabel({
  message,
  bindings,
  profiles,
}: {
  message: ConversationMessageView;
  bindings: readonly ConversationAgentBindingOption[];
  profiles: readonly ConversationAgentProfileOption[];
}) {
  if (message.role !== "AGENT") return null;
  if (!message.agent_binding_id) {
    return <span className="conversation-producer-label">Unknown agent</span>;
  }
  const binding = bindings.find(candidate => candidate.agentBindingId === message.agent_binding_id);
  const profile = binding && profiles.find(candidate => candidate.agentProfileId === binding.agentProfileId);
  const label = profile?.displayName.trim() || `Unknown agent · ${message.agent_binding_id}`;
  return <span className="conversation-producer-label" aria-label={`Response from ${label}`}>{label}</span>;
}

type ConversationTaskCreateResult = {
  taskId: string;
  workspaceId: string;
  conversationId: string | null;
  sourceMessageRefs: string[];
  currentSpecRevision: number;
  status: string;
  objective: string;
  inputRefs: Array<{ workspaceId: string; resourceId: string; revisionId: string }>;
};

function ConversationTaskPromotionDialog({
  workspaceId,
  workspaceName,
  operatorReady,
  source,
  preferredLeadId,
  bindings,
  profiles,
  onClose,
  onSaved,
}: {
  workspaceId: string;
  workspaceName: string;
  operatorReady: boolean;
  source: TaskPromotionSource;
  preferredLeadId: string | null;
  bindings: readonly ConversationAgentBindingOption[];
  profiles: readonly ConversationAgentProfileOption[];
  onClose: () => void;
  onSaved: (taskId: string) => void;
}) {
  const key = taskPromotionKey(workspaceId, source);
  const existing = taskPromotionRequests.get(key);
  const leads = taskPromotionLeads(workspaceId, bindings, profiles);
  const [objective, setObjective] = useState(existing?.objective ?? source.objective);
  const [leadAgentBindingId, setLeadAgentBindingId] = useState(existing?.preferredLeadAgentBindingId
    ?? (preferredLeadId && leads.some(lead => lead.agentBindingId === preferredLeadId) ? preferredLeadId : leads[0]?.agentBindingId ?? ""));
  const [selectedInputs, setSelectedInputs] = useState<PinnedConversationResourceRef[]>(() =>
    existing ? existing.inputRefs.map(reference => ({ ...reference })) : [...source.pinnedInputs]);
  const [pending, setPending] = useState<TaskPromotionRequest | null>(existing);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const dialog = useRef<HTMLDialogElement | null>(null);
  const objectiveInput = useRef<HTMLTextAreaElement | null>(null);
  const canEdit = pending === null;
  const objectiveWithinLimit = new TextEncoder().encode(objective.trim()).byteLength <= 32 * 1024;

  useEffect(() => {
    const element = dialog.current;
    if (!element) return;
    if (!element.open) element.showModal();
    objectiveInput.current?.focus();
    return () => { if (element.open) element.close(); };
  }, []);

  const save = async () => {
    if (saving || !operatorReady || !leadAgentBindingId) return;
    const draft: TaskPromotionDraft = {
      workspaceId,
      source,
      objective: objective.trim(),
      leadAgentBindingId,
      inputRefs: selectedInputs,
    };
    const request = taskPromotionRequests.create(key, draft, () => crypto.randomUUID());
    setPending(request);
    setSaving(true);
    setError(null);
    try {
      const result = await invoke<ConversationTaskCreateResult>("create_task", {
        workspaceId: request.workspaceId,
        conversationId: request.conversationId,
        sourceMessageRefs: [...request.sourceMessageRefs],
        objective: request.objective,
        preferredLeadAgentBindingId: request.preferredLeadAgentBindingId,
        coworkerId: null,
        expectedCoworkerVersion: null,
        inputRefs: request.inputRefs.map(reference => ({
          workspaceId: reference.workspace_id,
          resourceId: reference.resource_id,
          revisionId: reference.revision_id,
        })),
        requestId: request.requestId,
      });
      const expectedInputs = request.inputRefs.map(reference => ({
        workspaceId: reference.workspace_id,
        resourceId: reference.resource_id,
        revisionId: reference.revision_id,
      }));
      if (result.workspaceId !== request.workspaceId
        || result.conversationId !== request.conversationId
        || JSON.stringify(result.sourceMessageRefs) !== JSON.stringify(request.sourceMessageRefs)
        || result.currentSpecRevision !== 1
        || result.status !== "READY"
        || result.objective !== request.objective
        || JSON.stringify(result.inputRefs) !== JSON.stringify(expectedInputs)) {
        throw new Error("The saved Task did not match this Conversation promotion. Retry the same save to reconcile it.");
      }
      taskPromotionRequests.clear(key, request.requestId);
      onSaved(result.taskId);
    } catch (failure) {
      const message = typeof failure === "string"
        ? failure
        : failure instanceof Error ? failure.message : "The Task save outcome is uncertain. Retry the exact same save.";
      if (isKnownNoWriteTaskError(message)) {
        taskPromotionRequests.clear(key, request.requestId);
        setPending(null);
        setError(`${message} No Task was saved. Keep the draft and choose an available lead.`);
      } else {
        setPending(request);
        setError("The save outcome is uncertain. Retry this exact save to reconcile it; its objective, lead, inputs, and request identity are locked until then.");
      }
    } finally {
      setSaving(false);
    }
  };

  const toggleInput = (reference: TaskPromotionSource["pinnedInputs"][number]) => {
    if (!canEdit) return;
    const isSelected = selectedInputs.some(item =>
      item.workspace_id === reference.workspace_id
        && item.resource_id === reference.resource_id
        && item.revision_id === reference.revision_id,
    );
    setSelectedInputs(current => isSelected
      ? current.filter(item => !(item.workspace_id === reference.workspace_id
        && item.resource_id === reference.resource_id
        && item.revision_id === reference.revision_id))
      : [...current, reference]);
  };

  return <dialog
    className="task-promotion-dialog"
    ref={dialog}
    aria-labelledby="task-promotion-title"
    onCancel={event => {
      if (pending) event.preventDefault();
      else onClose();
    }}
  >
    <form className="task-promotion-form" onSubmit={event => { event.preventDefault(); void save(); }}>
      <header className="task-promotion-heading">
        <div><p className="eyebrow">CREATE TASK</p><h2 id="task-promotion-title">Continue this reply as a Task</h2></div>
        {!pending && <button type="button" className="quiet-button" onClick={onClose} aria-label="Close Task promotion">Close</button>}
      </header>
      <p className="task-promotion-workspace">Workspace: <strong>{workspaceName}</strong></p>
      <label className="task-promotion-field">Task objective
        <textarea ref={objectiveInput} value={objective} onChange={event => setObjective(event.target.value)} disabled={!canEdit || saving} maxLength={32_768} rows={4} />
      </label>
      {!objectiveWithinLimit && <p className="task-promotion-error" role="alert">The Task objective must be 32 KiB or smaller.</p>}
      <label className="task-promotion-field">Lead agent
        <select value={leadAgentBindingId} onChange={event => setLeadAgentBindingId(event.target.value)} disabled={!canEdit || saving}>
          <option value="">Choose an enabled lead agent</option>
          {leads.map(lead => <option key={lead.agentBindingId} value={lead.agentBindingId}>{lead.label}</option>)}
        </select>
      </label>
      <fieldset className="task-promotion-inputs" disabled={!canEdit || saving}>
        <legend>Source inputs</legend>
        {source.pinnedInputs.length === 0 && <p>No exact pinned inputs were attached to the source prompt.</p>}
        {source.pinnedInputs.map(reference => <label key={`${reference.resource_id}:${reference.revision_id}`}>
          <input type="checkbox" checked={selectedInputs.some(item => item.resource_id === reference.resource_id && item.revision_id === reference.revision_id)} onChange={() => toggleInput(reference)} />
          <span>{reference.resource_id} · pinned revision</span>
        </label>)}
        {source.unresolvedInputs.map(reference => <p className="task-promotion-unresolved" key={`${reference.resource_id}:unresolved`}>
          {reference.resource_id} · revision unavailable; not included
        </p>)}
      </fieldset>
      {error && <p className={pending ? "task-promotion-uncertain" : "task-promotion-error"} role={pending ? "status" : "alert"}>{error}</p>}
      <footer className="task-promotion-actions">
        {!pending && <button type="button" className="quiet-button" onClick={onClose} disabled={saving}>Cancel</button>}
        <button type="submit" className="primary-button" disabled={saving || !operatorReady || !objective.trim() || !objectiveWithinLimit || !leadAgentBindingId}>
          {saving ? "Saving…" : pending ? "Retry same save" : "Create Task"}
        </button>
      </footer>
    </form>
  </dialog>;
}

function ConversationMessageContent({ workspaceId, conversationId, message, autoLoadRich, presentationEpoch }: {
  workspaceId: string;
  conversationId: string;
  message: ConversationMessageView;
  autoLoadRich: boolean;
  presentationEpoch: number;
}) {
  const element = useRef<HTMLDivElement | null>(null);
  const [requestedKey, setRequestedKey] = useState<string | null>(null);
  const [retry, setRetry] = useState(0);
  const [loadState, setLoadState] = useState<{ key: string; status: "LOADING" | "READY" | "FAILED" } | null>(null);
  const [loadedPresentation, setLoadedPresentation] = useState<{ key: string; presentation: ValidatedRichPresentation } | null>(null);
  const reference = message.rich_presentation;
  const presentationId = reference?.presentation_id ?? null;
  const semanticContentDigest = reference?.semantic_content_digest ?? null;
  const text = semanticText(message);
  const immutableCacheKey = richPresentationCacheKey(workspaceId, conversationId, message.message_id, reference);
  const cacheKey = immutableCacheKey === null ? null : `${immutableCacheKey}\u0000${presentationEpoch}`;
  const requested = cacheKey !== null && requestedKey === cacheKey;
  const status = cacheKey === null || loadState?.key !== cacheKey ? "IDLE" : loadState.status;
  const presentation = cacheKey !== null && loadedPresentation?.key === cacheKey ? loadedPresentation.presentation : null;

  useEffect(() => {
    if (!autoLoadRich || cacheKey === null || !element.current || typeof IntersectionObserver === "undefined") return;
    const observer = new IntersectionObserver(entries => {
      if (entries.some(entry => entry.isIntersecting)) {
        setRequestedKey(current => current === cacheKey ? current : cacheKey);
        observer.disconnect();
      }
    }, { rootMargin: "320px" });
    observer.observe(element.current);
    return () => observer.disconnect();
  }, [autoLoadRich, cacheKey]);

  const shouldLoad = Boolean(cacheKey && requested);
  useEffect(() => {
    if (!shouldLoad || !reference || !presentationId || !semanticContentDigest || cacheKey === null || status !== "IDLE") return;
    let current = true;
    setLoadState({ key: cacheKey, status: "LOADING" });
    void invoke<unknown>("get_rich_presentation", {
      workspaceId,
      conversationId,
      messageId: message.message_id,
      presentationId,
      semanticContentDigest,
    }).then(async value => {
      const parsed = await parseRichPresentationResponse(value, {
        workspaceId,
        conversationId,
        messageId: message.message_id,
        presentationId: reference.presentation_id,
        schemaVersion: reference.schema_version,
        rendererContractVersion: reference.renderer_contract_version,
        semanticContentDigest,
        documentDigest: reference.document_digest,
        documentSizeBytes: reference.document_size_bytes,
      }, text);
      if (!current) return;
      const boundPresentation = parsed?.presentationId === presentationId ? parsed : null;
      if (boundPresentation) setLoadedPresentation({ key: cacheKey, presentation: boundPresentation });
      else setLoadedPresentation(currentValue => currentValue?.key === cacheKey ? null : currentValue);
      setLoadState({ key: cacheKey, status: boundPresentation ? "READY" : "FAILED" });
    }).catch(() => {
      if (current) setLoadState({ key: cacheKey, status: "FAILED" });
    });
    return () => { current = false; };
  // The key contains stable immutable bindings instead of the ref object or current
  // viewport eligibility. Live projection merges allocate fresh objects, and an
  // auto-load message may move out of the recent viewport set while its fetch is in
  // flight; neither event should strand its state at LOADING.
  // `status` is intentionally omitted: IDLE -> LOADING must not cancel itself.
  }, [cacheKey, conversationId, message.message_id, presentationId, requestedKey, retry, semanticContentDigest, text, workspaceId]);

  useEffect(() => {
    setLoadedPresentation(current => current?.key === cacheKey ? current : null);
    setLoadState(current => current?.key === cacheKey ? current : null);
    setRequestedKey(current => current === cacheKey ? current : null);
  }, [cacheKey]);

  if (!reference || message.role !== "AGENT") return <div className="conversation-message-body">{messageText(message)}</div>;
  const availability = reference.availability;
  const canFetch = availability === "AVAILABLE";
  return <div className="conversation-message-body" ref={element}>
    <RichResponseView semanticText={text || messageText(message)} presentation={presentation} />
    {!presentation && canFetch && status === "IDLE" && !shouldLoad && <button type="button" className="conversation-rich-load" onClick={() => cacheKey && setRequestedKey(cacheKey)}>Show formatted response</button>}
    {status === "LOADING" && <p className="conversation-rich-status" role="status">Loading formatted response…</p>}
    {status === "FAILED" && <p className="conversation-rich-status" role="note">Formatted view unavailable; the complete text response is still shown. <button type="button" onClick={() => { if (!cacheKey) return; setLoadState(null); setRequestedKey(cacheKey); setRetry(value => value + 1); }}>Retry</button></p>}
    {!canFetch && <p className="conversation-rich-status" role="note">Formatted view unavailable; the complete text response is still shown.</p>}
  </div>;
}

function timeLabel(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? "Saved conversation" : date.toLocaleString();
}

type ConversationAgentBindingPage = {
  items: ConversationAgentBindingOption[];
  nextCursor: string | null;
};

type ConversationAgentProfilePage = {
  items: ConversationAgentProfileOption[];
  nextCursor: string | null;
};

type ConversationAgentCatalogStatus = "LOADING" | "READY" | "FAILED" | "OFFLINE";
type ConversationAgentMutationNotice = {
  conversationId: string;
  status: "SAVING" | "SAVED" | "FAILED";
  message: string;
};

export function ConversationsPage({
  workspaceId,
  workspaceName,
  operatorReady,
  executionReady,
  runtimeLifecycleLoading,
  onRefreshRuntimeStatus,
  draft,
  onDraftChange,
  onOpenAgentSettings: _onOpenAgentSettings,
  workspaceDefaultAgentBindingId = null,
  selectedConversationId: externalSelectedId = null,
  onSelectConversation,
  onTaskCreated,
  onConversationsLoaded,
  onRegisterLoadMoreConversations,
  onConversationListPaginationChange,
  onToggleWorkbench,
  workbenchOpen = false,
}: {
  workspaceId: string;
  workspaceName: string;
  operatorReady: boolean;
  executionReady: boolean;
  runtimeLifecycleLoading: boolean;
  onRefreshRuntimeStatus: () => Promise<void>;
  draft: string;
  onDraftChange: (value: string) => void;
  onOpenAgentSettings: () => void;
  workspaceDefaultAgentBindingId?: string | null;
  selectedConversationId?: string | null;
  onSelectConversation?: (id: string | null) => void;
  onTaskCreated?: (taskId: string) => void;
  onConversationsLoaded?: (items: ConversationSummary[]) => void;
  onRegisterLoadMoreConversations?: (handler: (() => void) | null) => void;
  onConversationListPaginationChange?: (workspaceId: string, hasMore: boolean, loading: boolean) => void;
  onToggleWorkbench?: () => void;
  workbenchOpen?: boolean;
}) {
  const [items, setItems] = useState<ConversationSummary[]>([]);
  const [listCursor, setListCursor] = useState<string | null>(null);
  const [loadingMoreConversations, setLoadingMoreConversations] = useState(false);
  
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const selectedConversationId = useRef<string | null>(selectedId);
  const [newChatAgentBindingId, setNewChatAgentBindingId] = useState<string | null>(null);
  const newChatWorkspace = useRef(workspaceId);
  const [conversationDrafts, setConversationDrafts] = useState<Record<string, string>>({});
  const [agentBindings, setAgentBindings] = useState<ConversationAgentBindingOption[]>([]);
  const [agentProfiles, setAgentProfiles] = useState<ConversationAgentProfileOption[]>([]);
  const [agentCatalogStatus, setAgentCatalogStatus] = useState<ConversationAgentCatalogStatus>("LOADING");
  const [_agentCatalogError, setAgentCatalogError] = useState<string | null>(null);
  const [agentCatalogAttempt, _setAgentCatalogAttempt] = useState(0);
  const [agentMutationNotice, setAgentMutationNotice] = useState<ConversationAgentMutationNotice | null>(null);
  const agentMutationLock = useRef(false);
  const [snapshot, setSnapshot] = useState<ConversationSnapshotView | null>(null);
  const [taskPromotionSource, setTaskPromotionSource] = useState<TaskPromotionSource | null>(null);
  const [richPresentationEpochs, setRichPresentationEpochs] = useState<Map<string, number>>(() => new Map());
  const [busy, setBusy] = useState(false);
  const [_listStatus, setListStatus] = useState<"LOADING" | "READY" | "FAILED">("LOADING");
  const [createJournalStatus, setCreateJournalStatus] = useState<"LOADING" | "READY" | "FAILED">("LOADING");
  const [createJournalAttempt, setCreateJournalAttempt] = useState(0);
  const [pendingCreateRequests, setPendingCreateRequests] = useState<PendingConversationCreate[]>([]);
  const [pendingConversationSends, setPendingConversationSends] = useState<PendingConversationSend[]>([]);
  const [sendJournalStatus, setSendJournalStatus] = useState<"LOADING" | "READY" | "FAILED">("LOADING");
  const [sendJournalAttempt, setSendJournalAttempt] = useState(0);
  const [loadingMore, setLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [createReceiptError, setCreateReceiptError] = useState<{ workspaceId: string; requestId: string; message: string } | null>(null);
  const [snapshotLoadedKey, setSnapshotLoadedKey] = useState<string | null>(null);
  const [presentationStream, setPresentationStream] = useState<PresentationStreamState | null>(null);
  const [presentationStreamKey, setPresentationStreamKey] = useState<string | null>(null);
  const [streamStatus, setStreamStatus] = useState<"CONNECTING" | "LIVE" | "RECONNECTING" | null>(null);
  const [nativePermissionRecord, setNativePermissionRecord] = useState<{ key: string; state: NativePermissionState } | null>(null);
  const [nativePermissionActions, setNativePermissionActions] = useState<Record<string, NativePermissionDecision>>({});
  const [nativePermissionSnapshotError, setNativePermissionSnapshotError] = useState(false);
  const nativePermissionScopeRef = useRef<NativePermissionScope | null>(null);
  const lastPermissionAutoRefreshKey = useRef<string | null>(null);
  const permissionActionLock = useRef(new Set<string>());
  const taskPromotionTriggers = useRef(new Map<string, HTMLButtonElement>());
  const requests = useRef(new ConversationRequestEpochs());
  const activeListEpoch = useRef<number | null>(null);
  const activeSnapshotEpoch = useRef<number | null>(null);
  const previousOperatorReady = useRef(operatorReady);
  const preserveOnReconnect = useRef(false);
  const snapshotLoadedKeyRef = useRef(snapshotLoadedKey);
  snapshotLoadedKeyRef.current = snapshotLoadedKey;
  const mounted = useRef(false);
  const activeWorkspace = useRef(workspaceId);
  const latestDraft = useRef(draft);
  latestDraft.current = draft;
  activeWorkspace.current = workspaceId;
  selectedConversationId.current = selectedId;

  const streamBottomRef = useRef<HTMLDivElement>(null);
  const streamContainerRef = useRef<HTMLDivElement>(null);

  const openTaskPromotion = useCallback((source: TaskPromotionSource) => {
    setTaskPromotionSource(source);
  }, []);

  const closeTaskPromotion = useCallback(() => {
    const messageId = taskPromotionSource?.agentMessageId;
    setTaskPromotionSource(null);
    if (messageId) requestAnimationFrame(() => taskPromotionTriggers.current.get(messageId)?.focus());
  }, [taskPromotionSource]);

  const reloadPendingConversationSends = useCallback(async (requestWorkspaceId: string) => {
    const value = await invoke<unknown>("list_pending_conversation_sends", {});
    const parsed = parsePendingConversationSends(value);
    if (activeWorkspace.current === requestWorkspaceId) {
      setPendingConversationSends(parsed);
      setSendJournalStatus("READY");
    }
    return parsed;
  }, []);

  const acknowledgePendingConversationSend = async (request: PendingConversationSend): Promise<boolean> => {
    try {
      await invoke<void>("acknowledge_conversation_send", {
        workspaceId: request.workspaceId,
        conversationId: request.conversationId,
        requestId: request.requestId,
      });
    } catch {
      try {
        const remaining = await reloadPendingConversationSends(request.workspaceId);
        return !remaining.some(send =>
          send.workspaceId === request.workspaceId
          && send.conversationId === request.conversationId
          && send.requestId === request.requestId);
      } catch {
        if (activeWorkspace.current === request.workspaceId) {
          setPendingConversationSends(current => retainPendingConversationSend(current, request));
          setSendJournalStatus("FAILED");
        }
        return false;
      }
    }
    setPendingConversationSends(current => current.filter(send =>
      send.workspaceId !== request.workspaceId
      || send.conversationId !== request.conversationId
      || send.requestId !== request.requestId));
    return true;
  };

  useEffect(() => {
    if (externalSelectedId !== undefined && externalSelectedId !== null && externalSelectedId !== selectedId) {
      setSelectedId(externalSelectedId);
    }
  }, [externalSelectedId, selectedId]);

  useEffect(() => {
    onSelectConversation?.(selectedId);
  }, [selectedId, onSelectConversation]);
  useEffect(() => {
    if (!workspaceId || !operatorReady) {
      setAgentCatalogStatus("OFFLINE");
      return;
    }
    let current = true;
    const requestWorkspaceId = workspaceId;
    setAgentCatalogStatus("LOADING");
    setAgentCatalogError(null);
    void Promise.all([
      invoke<ConversationAgentBindingPage>("list_agent_bindings", { workspaceId: requestWorkspaceId }),
      invoke<ConversationAgentProfilePage>("list_agent_profiles", { workspaceId: requestWorkspaceId }),
    ]).then(([bindingPage, profilePage]) => {
      if (!current || activeWorkspace.current !== requestWorkspaceId) return;
      if (!bindingPage || !Array.isArray(bindingPage.items)
        || !profilePage || !Array.isArray(profilePage.items)) {
        throw new Error("The local Runtime returned an invalid agent catalog.");
      }
      setAgentBindings(bindingPage.items);
      setAgentProfiles(profilePage.items);
      setAgentCatalogStatus("READY");
    }).catch(cause => {
      if (!current || activeWorkspace.current !== requestWorkspaceId) return;
      setAgentCatalogStatus("FAILED");
      setAgentCatalogError(cause instanceof Error ? cause.message : String(cause));
    });
    return () => { current = false; };
  }, [agentCatalogAttempt, operatorReady, workspaceId]);
  const selectionKey = workspaceId && selectedId ? conversationDraftScope(workspaceId, selectedId) : null;
  const composerDraftKey = selectionKey;
  const composerDraft = composerDraftKey ? conversationDrafts[composerDraftKey] ?? "" : draft;
  const attachmentDraftKey = workspaceId
    ? conversationDraftScope(workspaceId, selectedId ?? "__new_chat_draft__")
    : null;

  useEffect(() => {
    if (newChatWorkspace.current === workspaceId) return;
    newChatWorkspace.current = workspaceId;
    setNewChatAgentBindingId(null);
  }, [workspaceId]);

  useLayoutEffect(() => {
    conversationWorkspaceSelections.select(workspaceId);
  }, [workspaceId]);

  useEffect(() => {
    if (!operatorReady || !workspaceId) {
      setPendingCreateRequests([]);
      setPendingConversationSends([]);
      setCreateJournalStatus("READY");
      setSendJournalStatus("READY");
      return;
    }
    let current = true;
    const requestWorkspaceId = workspaceId;
    setCreateJournalStatus("LOADING");
    void invoke<unknown>("list_pending_conversation_creates", {})
      .then(value => {
        if (!current || activeWorkspace.current !== requestWorkspaceId) return;
        setPendingCreateRequests(pendingCreates.replace(value));
        setCreateJournalStatus("READY");
      })
      .catch(cause => {
        if (!current || activeWorkspace.current !== requestWorkspaceId) return;
        setCreateJournalStatus("FAILED");
        setError(cause instanceof Error ? cause.message : String(cause));
      });
    setSendJournalStatus("LOADING");
    void reloadPendingConversationSends(requestWorkspaceId)
      .catch(cause => {
        if (!current || activeWorkspace.current !== requestWorkspaceId) return;
        setSendJournalStatus("FAILED");
        setError(cause instanceof Error ? cause.message : String(cause));
      });
    return () => { current = false; };
  }, [createJournalAttempt, operatorReady, reloadPendingConversationSends, sendJournalAttempt, workspaceId]);

  const refresh = useCallback(async (preserveCurrent = false) => {
    const current = requests.current.beginList();
    activeListEpoch.current = current;
    setLoadingMoreConversations(false);
    if (!operatorReady || !workspaceId) {
      setBusy(false);
      if (!workspaceId) {
        setItems([]);
        setListCursor(null);
        setLoadingMoreConversations(false);
        setSnapshot(null);
        setSnapshotLoadedKey(null);
        setSelectedId(null);
        setListStatus("READY");
      }
      return;
    }
    if (!preserveCurrent) {
      setListCursor(null);
      setSnapshot(null);
      setSnapshotLoadedKey(null);
      setSelectedId(null);
    }
    setListStatus("LOADING");
    setBusy(true); setError(null);
    try {
      const response = await invoke<unknown>("list_conversations", { workspaceId, cursor: null });
      if (activeWorkspace.current !== workspaceId || !requests.current.isCurrentList(current)) return;
      const page = parseConversationListPage(response, workspaceId);
      setItems(page.items);
      onConversationsLoaded?.(page.items);
      setListCursor(page.next_cursor);
      setLoadingMoreConversations(false);
      if (!externalSelectedId) {
        setSelectedId(page.items[0]?.conversation_id ?? null);
      }
      setListStatus("READY");
    } catch (cause) {
      if (activeWorkspace.current === workspaceId && requests.current.isCurrentList(current)) {
        setListStatus("FAILED");
        setError(cause instanceof Error ? cause.message : String(cause));
      }
    } finally { if (activeWorkspace.current === workspaceId && requests.current.isCurrentList(current)) setBusy(false); }
  }, [operatorReady, workspaceId]);

  const loadMoreConversations = useCallback(async () => {
    const cursor = listCursor;
    const current = activeListEpoch.current;
    if (!cursor || !operatorReady || loadingMoreConversations || current === null
      || !requests.current.isCurrentList(current)) return;

    const requestWorkspaceId = workspaceId;
    setLoadingMoreConversations(true);
    try {
      const value = await invoke<unknown>("list_conversations", { workspaceId: requestWorkspaceId, cursor });
      if (activeWorkspace.current !== requestWorkspaceId || !requests.current.isCurrentList(current)) return;
      const page = parseConversationListPage(value, requestWorkspaceId);
      if (page.next_cursor === cursor) {
        throw new Error("Conversation list did not advance its pagination cursor.");
      }
      const merged = appendConversationListPage({ items, next_cursor: cursor }, page);
      setItems(merged.items);
      onConversationsLoaded?.(merged.items);
      setListCursor(page.next_cursor);
    } catch (cause) {
      if (activeWorkspace.current === requestWorkspaceId && requests.current.isCurrentList(current)) {
        setError(cause instanceof Error ? cause.message : String(cause));
      }
    } finally {
      if (activeWorkspace.current === requestWorkspaceId && requests.current.isCurrentList(current)) {
        setLoadingMoreConversations(false);
      }
    }
  }, [items, listCursor, loadingMoreConversations, onConversationsLoaded, operatorReady, workspaceId]);

  useEffect(() => {
    onRegisterLoadMoreConversations?.(loadMoreConversations);
    return () => onRegisterLoadMoreConversations?.(null);
  }, [loadMoreConversations, onRegisterLoadMoreConversations]);

  useEffect(() => {
    onConversationListPaginationChange?.(workspaceId, listCursor !== null, loadingMoreConversations);
  }, [listCursor, loadingMoreConversations, onConversationListPaginationChange, workspaceId]);

  useEffect(() => {
    const reconnecting = !previousOperatorReady.current && operatorReady;
    previousOperatorReady.current = operatorReady;
    preserveOnReconnect.current = reconnecting;
    void refresh(reconnecting);
    return () => { requests.current.invalidateList(); };
  }, [operatorReady, refresh]);

  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);

  
  useEffect(() => {
    const current = requests.current.beginSnapshot();
    activeSnapshotEpoch.current = current;
    setLoadingMore(false);
    if (!operatorReady) {
      setBusy(false);
      if (snapshotLoadedKeyRef.current !== selectionKey) {
        setSnapshot(null);
        setSnapshotLoadedKey(null);
      }
      return () => { requests.current.invalidateSnapshot(); };
    }
    if (!preserveOnReconnect.current || snapshotLoadedKeyRef.current !== selectionKey) {
      setSnapshot(null);
      setSnapshotLoadedKey(null);
    }
    preserveOnReconnect.current = false;
    if (!workspaceId || !selectedId) return () => { requests.current.invalidateSnapshot(); };
    setBusy(true); setError(null);
    void invoke<unknown>("get_conversation_presentation", { workspaceId, conversationId: selectedId })
      .then(value => {
        if (activeWorkspace.current !== workspaceId || !requests.current.isCurrentSnapshot(current)) return;
        setSnapshot(parseConversationSnapshot(value, workspaceId, selectedId));
        setSnapshotLoadedKey(`${workspaceId}\u0000${selectedId}`);
      })
      .catch(cause => { if (activeWorkspace.current === workspaceId && requests.current.isCurrentSnapshot(current)) setError(cause instanceof Error ? cause.message : String(cause)); })
      .finally(() => { if (activeWorkspace.current === workspaceId && requests.current.isCurrentSnapshot(current)) setBusy(false); });
    return () => {
      requests.current.invalidateSnapshot();
      if (activeSnapshotEpoch.current === current) activeSnapshotEpoch.current = null;
    };
  }, [operatorReady, selectedId, workspaceId]);

  useEffect(() => {
    if (!operatorReady || !selectionKey || snapshotLoadedKey !== selectionKey) {
      setPresentationStream(null);
      setPresentationStreamKey(null);
      setStreamStatus(null);
      return;
    }
    setPresentationStream(null);
    setPresentationStreamKey(selectionKey);
    const driver: ConversationStreamDriver = {
      listen: async callback => listen<unknown>("conversation-presentation-stream", event => callback(event.payload as StreamEnvelope)),
      start: input => invoke<void>("subscribe_conversation_stream", input),
      acknowledge: (sessionId, frameId) => invoke<void>("acknowledge_conversation_stream_frame", { sessionId, frameId }),
      stop: sessionId => invoke<void>("unsubscribe_conversation_stream", { sessionId }),
      newSessionId: () => crypto.randomUUID(),
    };
    const controller = new ConversationStreamController(workspaceId, selectedId!, driver, (state, frame) => {
      setPresentationStream(state);
      setPresentationStreamKey(selectionKey);
      const live = state.conversationSnapshot;
      if (frame && typeof frame === "object" && !Array.isArray(frame)) {
        const projectionFrame = frame as Record<string, unknown>;
        if (projectionFrame.type === "projection" && projectionFrame.projection_type === "conversation_presentation") {
          const invalidatedMessageIds = live?.messages
            .filter(message => message.rich_presentation !== null && message.rich_presentation.availability !== "AVAILABLE")
            .map(message => message.message_id) ?? [];
          if (invalidatedMessageIds.length > 0) {
            setRichPresentationEpochs(current => {
              const next = new Map(current);
              for (const messageId of invalidatedMessageIds) next.set(messageId, (next.get(messageId) ?? 0) + 1);
              return next;
            });
          }
        }
      }
      if (live && activeWorkspace.current === workspaceId) {
        setSnapshot(current => mergeConversationStreamSnapshot(current, live, workspaceId, selectedId!));
      }
    }, setStreamStatus, undefined, frame => {
      const scope = nativePermissionScopeRef.current;
      if (!scope || !frame || typeof frame !== "object" || Array.isArray(frame)) return;
      const value = frame as Record<string, unknown>;
      const key = JSON.stringify([scope.workspaceId, scope.conversationId, scope.turnId, scope.retryOrdinal]);
      if (value.type === "turn.permission") {
        setNativePermissionRecord(current => {
          const base = current?.key === key ? current.state : emptyNativePermissionState;
          const next = reduceNativePermissionFrame(base, frame, scope);
          return next === base ? current : { key, state: next };
        });
      } else if (value.type === "turn.settled"
        && value.workspace_id === scope.workspaceId
        && value.conversation_id === scope.conversationId
        && value.turn_id === scope.turnId
        && value.retry_ordinal === scope.retryOrdinal) {
        setNativePermissionRecord(current => {
          if (current?.key !== key) return current;
          const next = endLiveNativePermissions(current.state);
          return next === current.state ? current : { key, state: next };
        });
      }
    }, () => {
      const scope = nativePermissionScopeRef.current;
      if (!scope) return;
      const key = JSON.stringify([scope.workspaceId, scope.conversationId, scope.turnId, scope.retryOrdinal]);
      // A resumed stream may have missed the final permission-state frame. Keep
      // the old card visible, but make it non-actionable until the exact attempt
      // snapshot replaces it.
      lastPermissionAutoRefreshKey.current = null;
      setNativePermissionRecord(current => {
        const base = current?.key === key ? current.state : emptyNativePermissionState;
        return { key, state: requireNativePermissionSnapshot(base) };
      });
    });
    controller.start();
    setStreamStatus(controller.status);
    return () => { void controller.dispose(); };
  }, [operatorReady, selectionKey, selectedId, snapshotLoadedKey, workspaceId]);

  const loadMoreMessages = async () => {
    if (!snapshot?.next_cursor || !selectedId || loadingMore) return;
    const current = activeSnapshotEpoch.current;
    if (current === null || !requests.current.isCurrentSnapshot(current)) return;
    const requestWorkspaceId = workspaceId;
    const cursor = snapshot.next_cursor;
    setLoadingMore(true);
    try {
      const value = await invoke<unknown>("get_conversation_presentation", { workspaceId: requestWorkspaceId, conversationId: selectedId, cursor });
      if (activeWorkspace.current !== requestWorkspaceId || !requests.current.isCurrentSnapshot(current)) return;
      const page = parseConversationSnapshot(value, requestWorkspaceId, selectedId);
      if (page.next_cursor === cursor) throw new Error("Conversation history did not advance its pagination cursor.");
      setSnapshot(existing => existing?.workspace_id === requestWorkspaceId && existing.conversation_id === selectedId ? appendConversationPage(existing, page) : existing);
    } catch (cause) {
      if (activeWorkspace.current === requestWorkspaceId && requests.current.isCurrentSnapshot(current)) setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      if (activeWorkspace.current === requestWorkspaceId && requests.current.isCurrentSnapshot(current)) setLoadingMore(false);
    }
  };

  const submitCreate = async (request: PendingConversationCreate) => {
    if (!operatorReady || busy || createJournalStatus !== "READY") return;
    const requestSelection = conversationWorkspaceSelections.capture(request.workspaceId);
    if (!requestSelection) return;
    const isCurrentCreateRequest = () => mounted.current
      && activeWorkspace.current === request.workspaceId
      && conversationWorkspaceSelections.isCurrent(requestSelection);
    const clearCreateReceiptError = () => setCreateReceiptError(current => current?.workspaceId === request.workspaceId && current.requestId === request.requestId ? null : current);
    clearCreateReceiptError();
    setBusy(true); setError(null);
    try {
      const response = request.firstTurn
        ? await invoke<unknown>("create_conversation_with_first_turn", {
          workspaceId: request.workspaceId,
          title: request.title,
          requestId: request.requestId,
          firstTurn: request.firstTurn,
        })
        : await invoke<unknown>("create_conversation", {
          workspaceId: request.workspaceId, title: request.title, requestId: request.requestId,
        });
      if (!isCurrentCreateRequest()) return;
      let conversationValue = response;
      let admittedConversationId: string | null = null;
      if (request.firstTurn) {
        if (typeof response !== "object" || response === null || Array.isArray(response)) {
          throw new Error("Local Runtime returned an invalid first-turn receipt.");
        }
        const admission = response as Record<string, unknown>;
        const receipt = admission.turn;
        if (typeof receipt !== "object" || receipt === null || Array.isArray(receipt)) {
          throw new Error("Local Runtime returned an invalid first-turn receipt.");
        }
        const turnReceipt = receipt as Record<string, unknown>;
        conversationValue = admission.conversation;
        admittedConversationId = typeof turnReceipt.conversation_id === "string" ? turnReceipt.conversation_id : null;
        if (typeof turnReceipt.turn_id !== "string"
          || typeof turnReceipt.user_message_id !== "string"
          || typeof turnReceipt.correlation_id !== "string"
          || typeof turnReceipt.conversation_id !== "string"
          || turnReceipt.status !== "OPEN") {
          throw new Error("Local Runtime returned an invalid first-turn receipt.");
        }
      }
      // The first-turn response is composite; validate its Conversation with the same
      // strict parser used by the persisted Conversation list.
      const created = parseConversationList({ items: [conversationValue], next_cursor: null }, request.workspaceId)[0];
      if (!created) throw new Error("Local Runtime returned an invalid Conversation.");
      if (request.firstTurn && (created.version !== 2
        || admittedConversationId !== created.conversation_id
        || !created.active_agent_binding_id
        || created.active_agent_binding_id !== request.firstTurn.agentBindingId)) {
        throw new Error("Local Runtime returned an unexpected first-turn Conversation.");
      }
      let cleanupError: string | null = null;
      try {
        await invoke<void>("acknowledge_conversation_create", {
          workspaceId: request.workspaceId,
          requestId: request.requestId,
        });
        clearCreateReceiptError();
        pendingCreates.completeExact(request.workspaceId, request.requestId);
        if (mounted.current) {
          setPendingCreateRequests(current => current.filter(item =>
            item.workspaceId !== request.workspaceId || item.requestId !== request.requestId));
        }
      } catch (cause) {
        cleanupError = cause instanceof Error ? cause.message : String(cause);
        pendingCreates.remember(request);
        if (isCurrentCreateRequest()) {
          setCreateReceiptError({
            workspaceId: request.workspaceId,
            requestId: request.requestId,
            message: `Conversation created, but its retry receipt could not be cleared: ${cleanupError}. Retrying this pending request is safe.`,
          });
          setPendingCreateRequests(current => current.some(item =>
            item.workspaceId === request.workspaceId && item.requestId === request.requestId)
            ? current
            : [...current, request]);
        }
      }
      if (!isCurrentCreateRequest()) return;
      if (request.firstTurn) {
        const sentText = request.firstTurn.content.find(block => block.kind === "TEXT")?.text;
        if (sentText && latestDraft.current.trim() === sentText) onDraftChange("");
        const firstDraftKey = conversationDraftScope(request.workspaceId, "__new_chat_draft__");
        setAttachmentChips(current => {
          if (!(firstDraftKey in current)) return current;
          const next = { ...current };
          delete next[firstDraftKey];
          return next;
        });
      }
      setItems(current => {
        const nextList = [created, ...current.filter(item => item.conversation_id !== created.conversation_id)];
        onConversationsLoaded?.(nextList);
        return nextList;
      });
      setSelectedId(created.conversation_id);
      if (request.firstTurn) {
        setSnapshot(null);
        setSnapshotLoadedKey(null);
      } else {
        setSnapshot({ workspace_id: request.workspaceId, conversation_id: created.conversation_id, messages: [], next_cursor: null, active_turn: null });
        setSnapshotLoadedKey(`${request.workspaceId}\u0000${created.conversation_id}`);
      }
    } catch (cause) {
      if (!isCurrentCreateRequest()) return;
      const createError = cause instanceof Error ? cause.message : String(cause);
      setError(createError);
      try {
        const journal = await invoke<unknown>("list_pending_conversation_creates", {});
        if (!isCurrentCreateRequest()) return;
        setPendingCreateRequests(pendingCreates.replace(journal));
        setCreateJournalStatus("READY");
      } catch (journalCause) {
        if (!isCurrentCreateRequest()) return;
        pendingCreates.remember(request);
        setPendingCreateRequests(current => current.some(item =>
          item.workspaceId === request.workspaceId && item.requestId === request.requestId)
          ? current
          : [...current, request]);
        setCreateJournalStatus("FAILED");
        const journalError = journalCause instanceof Error ? journalCause.message : String(journalCause);
        setError(`${createError} Pending retry state could not be refreshed: ${journalError}`);
      }
    } finally {
      if (isCurrentCreateRequest()) setBusy(false);
    }
  };

  const selected = items.find(item => item.conversation_id === selectedId) ?? null;
  const agentChoices = buildConversationAgentChoices(
    workspaceId,
    agentBindings,
    agentProfiles,
    selected?.active_agent_binding_id ?? newChatAgentBindingId ?? workspaceDefaultAgentBindingId,
  );
  const workspaceDefaultChoice = workspaceDefaultAgentBindingId
    ? agentChoices.find(choice => choice.agentBindingId === workspaceDefaultAgentBindingId && choice.selectable)
    : undefined;
  const workspaceDefaultOptionLabel = workspaceDefaultAgentBindingId === null
    ? "Workspace default (not configured)"
    : workspaceDefaultChoice
      ? `Workspace default · ${workspaceDefaultChoice.label}`
      : "Workspace default (binding details unavailable)";
  const visibleAgentMutationNotice = selected && agentMutationNotice?.conversationId === selected.conversation_id
    ? agentMutationNotice
    : null;
  const saveConversationAgentBinding = async (agentBindingId: string | null) => {
    const conversationId = selected?.conversation_id;
    if (!conversationId || !operatorReady || agentMutationLock.current) return;
    agentMutationLock.current = true;
    const requestWorkspaceId = workspaceId;
    const requestConversationId = conversationId;
    setAgentMutationNotice({
      conversationId,
      status: "SAVING",
      message: "Saving this Conversation’s agent selection…",
    });
    const parseRecord = (value: unknown): ConversationSummary => {
      const record = parseConversationList({ items: [value], next_cursor: null }, requestWorkspaceId)[0];
      if (!record || record.conversation_id !== requestConversationId) {
        throw new Error("The local Runtime returned a different Conversation.");
      }
      return record;
    };
    const applyRecord = (record: ConversationSummary) => {
      setItems(current => current.map(item => item.conversation_id === requestConversationId ? record : item));
    };
    const setNoticeIfSelected = (notice: ConversationAgentMutationNotice) => {
      if (activeWorkspace.current === requestWorkspaceId
        && selectedConversationId.current === requestConversationId) {
        setAgentMutationNotice(notice);
      }
    };
    try {
      const currentRecord = parseRecord(await invoke<unknown>("get_conversation", {
        workspaceId: requestWorkspaceId,
        conversationId: requestConversationId,
      }));
      if (currentRecord.active_agent_binding_id === agentBindingId) {
        applyRecord(currentRecord);
        setNoticeIfSelected({
          conversationId,
          status: "SAVED",
          message: "This Conversation already uses that agent selection.",
        });
        return;
      }
      const savedRecord = parseRecord(await invoke<unknown>("set_conversation_agent_binding", {
        workspaceId: requestWorkspaceId,
        conversationId: requestConversationId,
        expectedVersion: currentRecord.version,
        agentBindingId,
        requestId: crypto.randomUUID(),
      }));
      if (savedRecord.active_agent_binding_id !== agentBindingId
        || savedRecord.version !== currentRecord.version + 1) {
        throw new Error("The agent selection response did not match the requested change.");
      }
      applyRecord(savedRecord);
      setNoticeIfSelected({
        conversationId,
        status: "SAVED",
        message: "Saved for future turns. Any running session keeps its existing agent.",
      });
    } catch (cause) {
      let latest: ConversationSummary | null = null;
      try {
        latest = parseRecord(await invoke<unknown>("get_conversation", {
          workspaceId: requestWorkspaceId,
          conversationId: requestConversationId,
        }));
        applyRecord(latest);
      } catch {
        // Preserve the last committed UI state when readback is also unavailable.
      }
      if (latest?.active_agent_binding_id === agentBindingId) {
        setNoticeIfSelected({
          conversationId,
          status: "SAVED",
          message: "The selection was saved; the response was confirmed by reloading this Conversation.",
        });
      } else {
        const detail = cause instanceof Error && cause.message.trim()
          ? cause.message
          : "The change could not be confirmed.";
        setNoticeIfSelected({
          conversationId,
          status: "FAILED",
          message: latest
            ? "A newer Conversation version was loaded. Review the current selection and try again."
            : `Agent selection was not confirmed. ${detail}`,
        });
      }
    } finally {
      agentMutationLock.current = false;
    }
  };
  const currentSnapshot = snapshotLoadedKey === selectionKey && snapshot?.workspace_id === workspaceId && snapshot?.conversation_id === selectedId ? snapshot : null;
  const currentPresentationStream = presentationStreamKey === selectionKey
    && presentationStream?.workspaceId === workspaceId
    ? presentationStream
    : null;
  // Live native permission state exists only for the currently running attempt.
  // A retryable FAILED turn is historical and must never recreate an actionable
  // native prompt or trigger a live-session lookup.
  const permissionTurn = currentSnapshot?.active_turn?.status === "RUNNING"
    ? currentSnapshot.active_turn
    : null;
  const livePermissionScope: NativePermissionScope | null = permissionTurn && workspaceId && selectedId
    ? { workspaceId, conversationId: selectedId, turnId: permissionTurn.turn_id, retryOrdinal: permissionTurn.retry_ordinal }
    : null;
  const livePermissionKey = livePermissionScope
    ? JSON.stringify([livePermissionScope.workspaceId, livePermissionScope.conversationId, livePermissionScope.turnId, livePermissionScope.retryOrdinal])
    : null;
  nativePermissionScopeRef.current = livePermissionScope;
  const currentNativePermissionState = livePermissionKey && nativePermissionRecord?.key === livePermissionKey
    ? nativePermissionRecord.state
    : emptyNativePermissionState;
  const refreshNativePermissionSnapshot = useCallback(async (scope: NativePermissionScope, key: string) => {
    if (!operatorReady) throw new Error("Local Runtime is unavailable");
    const value = await invoke<unknown>("list_conversation_turn_permissions", {
      workspaceId: scope.workspaceId,
      conversationId: scope.conversationId,
      turnId: scope.turnId,
    });
    const parsed = parseConversationPermissionSnapshot(value, scope);
    if (!parsed) throw new Error("Live permission snapshot did not match the active attempt");
    const currentScope = nativePermissionScopeRef.current;
    const currentKey = currentScope ? JSON.stringify([currentScope.workspaceId, currentScope.conversationId, currentScope.turnId, currentScope.retryOrdinal]) : null;
    if (currentKey === key) {
      setNativePermissionRecord(current => {
        const base = current?.key === key ? current.state : emptyNativePermissionState;
        return { key, state: reconcileConversationPermissionSnapshot(base, parsed) };
      });
      setNativePermissionSnapshotError(false);
    }
    return parsed;
  }, [operatorReady]);

  useEffect(() => {
    if (!livePermissionScope || !livePermissionKey || !operatorReady) return;
    const scope = livePermissionScope;
    const key = livePermissionKey;
    let active = true;
    lastPermissionAutoRefreshKey.current = null;
    setNativePermissionSnapshotError(false);
    setNativePermissionActions({});
    setNativePermissionRecord(current => current?.key === key ? current : { key, state: emptyNativePermissionState });
    void refreshNativePermissionSnapshot(scope, key).catch(() => {
      if (active) setNativePermissionSnapshotError(true);
    });
    return () => { active = false; };
  }, [livePermissionKey, operatorReady, refreshNativePermissionSnapshot]);

  useEffect(() => {
    if (!livePermissionScope || !livePermissionKey || !currentNativePermissionState.needsRefresh
      || lastPermissionAutoRefreshKey.current === livePermissionKey) return;
    lastPermissionAutoRefreshKey.current = livePermissionKey;
    void refreshNativePermissionSnapshot(livePermissionScope, livePermissionKey).catch(() => setNativePermissionSnapshotError(true));
  }, [currentNativePermissionState, livePermissionKey, livePermissionScope, refreshNativePermissionSnapshot]);

  const sendNativePermissionDecision = useCallback(async (promptToken: string, decision: NativePermissionDecision) => {
    const scope = nativePermissionScopeRef.current;
    const stateKey = scope ? JSON.stringify([scope.workspaceId, scope.conversationId, scope.turnId, scope.retryOrdinal]) : null;
    const prompt = stateKey && nativePermissionRecord?.key === stateKey
      ? nativePermissionRecord.state.prompts[promptToken]
      : null;
    if (!scope || !stateKey || !operatorReady || prompt?.state !== "PENDING"
      || nativePermissionActions[promptToken] || permissionActionLock.current.has(promptToken)) return;
    permissionActionLock.current.add(promptToken);
    setNativePermissionActions(current => ({ ...current, [promptToken]: decision }));
    try {
      const receipt = await invoke<unknown>("reply_conversation_permission", {
        workspaceId: scope.workspaceId,
        conversationId: scope.conversationId,
        turnId: scope.turnId,
        promptToken,
        decision,
        idempotencyKey: crypto.randomUUID(),
      });
      setNativePermissionRecord(current => current?.key === stateKey
        ? { key: stateKey, state: applyNativePermissionReceipt(current.state, receipt) }
        : current);
    } catch {
      try {
        const snapshot = await refreshNativePermissionSnapshot(scope, stateKey);
        if (!snapshot.items.some(item => item.promptToken === promptToken)) {
          setNativePermissionRecord(current => current?.key === stateKey
            ? { key: stateKey, state: markNativePermissionOutcomeUnknown(current.state, promptToken, decision) }
            : current);
        }
      } catch {
        setNativePermissionRecord(current => current?.key === stateKey
          ? { key: stateKey, state: markNativePermissionOutcomeUnknown(current.state, promptToken, decision) }
          : current);
        setNativePermissionSnapshotError(true);
      }
    } finally {
      permissionActionLock.current.delete(promptToken);
      setNativePermissionActions(current => {
        const next = { ...current };
        delete next[promptToken];
        return next;
      });
    }
  }, [nativePermissionActions, nativePermissionRecord, operatorReady, refreshNativePermissionSnapshot]);
  const transientTurns = Object.values(currentPresentationStream?.turns ?? {})
    .filter((turn): turn is TransientTurn => turn.conversationId === selectedId)
    .sort((a, b) => a.turnId.localeCompare(b.turnId) || a.retryOrdinal - b.retryOrdinal);
  const automaticRichIds = new Set((currentSnapshot?.messages ?? []).filter(message => message.role === "AGENT" && message.rich_presentation?.availability === "AVAILABLE").slice(-6).map(message => message.message_id));
  const activeStatus = currentSnapshot?.active_turn?.status;
  const [sendingTurn, setSendingTurn] = useState(false);
  const [turnError, setTurnError] = useState<string | null>(null);
  const [retryPending, setRetryPending] = useState(false);
  const [stopPending, setStopPending] = useState(false);

  // REPORTED activity (T25): reload the latest exact turn/retry tuple, including completed
  // turns, after restart. The activity projection is read-only and never supplies Stop/Retry authority.
  const activityTurn = currentSnapshot ? activityTurnForRead(currentSnapshot) : null;
  const activityTurnId = activityTurn?.turn_id ?? null;
  const activityRetry = activityTurn?.retry_ordinal ?? 0;
  const activityStatus = activityTurn?.status ?? null;
  const [storedActivity, setStoredActivity] = useState<{ key: string; entries: ActivityEntry[] } | null>(null);
  useEffect(() => {
    if (!workspaceId || !selectedId || !activityTurnId) return;
    const key = JSON.stringify([workspaceId, selectedId, activityTurnId, activityRetry]);
    let current = true;
    invoke<unknown>("list_conversation_turn_activity", {
      workspaceId, conversationId: selectedId, turnId: activityTurnId, retryOrdinal: activityRetry,
    }).then(page => {
      const entries = parseActivityPage(page, activityTurnId, activityRetry);
      if (current && entries) setStoredActivity({ key, entries });
    }).catch(() => { /* activity is an aid; the transcript and turn state stay authoritative */ });
    return () => { current = false; };
  }, [workspaceId, selectedId, activityTurnId, activityRetry, activityStatus]);
  const activityKey = activityTurnId && selectedId ? JSON.stringify([workspaceId, selectedId, activityTurnId, activityRetry]) : null;
  const liveActivity = activityTurnId && selectedId
    ? currentPresentationStream?.activities[JSON.stringify([selectedId, activityTurnId, activityRetry])]?.entries ?? []
    : [];
  const activityEntries = mergeActivity(storedActivity && storedActivity.key === activityKey ? storedActivity.entries : [], liveActivity);

  const transientStreaming = transientTurns.some(t => t.state === "STREAMING" || t.state === "RECONNECTING");
  // Every nonterminal status, a visible streamed turn and an in-flight submit (including the gap
  // after the 202 and before OPEN is projected) occupy the composer (001-T18, AC-024).
  const isTurnRunning = isTurnOccupyingComposer({ activeTurn: currentSnapshot?.active_turn ?? null, transientStreaming, submitting: sendingTurn });
  const stopping = stopTarget(currentSnapshot?.active_turn);
  const retrying = retryTarget(currentSnapshot?.active_turn, currentSnapshot?.retryable_turn, sendingTurn || retryPending);

  // A-24 disabled-until-dispatch-eligible UI gate (docs/V1-UI-SCREEN-CONTRACT.md "Conversation screen" steps 1/3):
  // Send requires a selectable AgentBinding, not merely a non-empty draft. Selection checks
  // Workspace, enabled/lead-eligible state and a known Conversation dispatch branch; it does not
  // prove provider authentication, native-session readiness, or qualification of the user journey.
  // When no Conversation is selected yet, the composer creates one via `create()`; eligibility is
  // re-checked against the resulting Conversation's binding, so a draft with no dispatch-eligible binding
  // cannot submit a turn.
  const selectedAgentBindingId = selected?.active_agent_binding_id
    ?? newChatAgentBindingId
    ?? workspaceDefaultAgentBindingId;
  const selectedBindingEligible = selectedAgentBindingId !== null
    && agentChoices.some(choice => choice.agentBindingId === selectedAgentBindingId && choice.selectable);
  // For an unstarted Conversation (no selectedId) a dispatch-eligible binding still must exist to submit; the
  // workspace default (when selectable) or any selectable choice makes Send eligible after creation.
  const bindingEligibleForSend = selectedBindingEligible;
  const sendEligible = isSendEligible({
    activeTurn: currentSnapshot?.active_turn ?? null,
    transientStreaming,
    submitting: sendingTurn,
    draft: composerDraft,
    hasWorkspace: Boolean(workspaceId),
    operatorReady,
    executionReady,
    bindingEligible: bindingEligibleForSend,
  });

  // Composer attachments (T14): chips are kept per selected Conversation and survive a refused send.
  const [attachmentChips, setAttachmentChips] = useState<Record<string, AttachmentChip[]>>({});
  const [pickerOpen, setPickerOpen] = useState(false);
  const [pickerState, setPickerState] = useState<{
    status: "LOADING" | "READY" | "FAILED";
    items: AttachmentCandidate[];
    nextCursor: string | null;
    loadingMore: boolean;
    moreFailed: boolean;
    cursorStalled: boolean;
  }>({ status: "LOADING", items: [], nextCursor: null, loadingMore: false, moreFailed: false, cursorStalled: false });
  const pickerGeneration = useRef(0);
  const pickerMoreRequest = useRef<{ generation: number; cursor: string } | null>(null);
  const consumedPickerCursors = useRef(new Set<string>());
  const pickerScopeKey = JSON.stringify([workspaceId, selectedId ?? "__new_chat_draft__"]);
  const pickerScopeRef = useRef(pickerScopeKey);
  pickerScopeRef.current = pickerScopeKey;

  useEffect(() => {
    pickerGeneration.current += 1;
    pickerMoreRequest.current = null;
    consumedPickerCursors.current.clear();
    setPickerOpen(false);
    setPickerState({ status: "LOADING", items: [], nextCursor: null, loadingMore: false, moreFailed: false, cursorStalled: false });
  }, [pickerScopeKey]);

  const chips = attachmentDraftKey ? attachmentChips[attachmentDraftKey] ?? [] : [];
  const pendingSendsInWorkspace = pendingConversationSends.filter(send => send.workspaceId === workspaceId);
  const pendingSendForSelection = selectedId
    ? pendingSendsInWorkspace.find(send => send.conversationId === selectedId) ?? null
    : null;
  const openPicker = () => {
    if (!workspaceId) return;
    const generation = pickerGeneration.current + 1;
    pickerGeneration.current = generation;
    pickerMoreRequest.current = null;
    consumedPickerCursors.current.clear();
    const scopeKey = pickerScopeKey;
    setPickerOpen(true);
    setPickerState({ status: "LOADING", items: [], nextCursor: null, loadingMore: false, moreFailed: false, cursorStalled: false });
    invoke<{ items: AttachmentCandidate[]; nextCursor: string | null }>("list_resources", { workspaceId, cursor: null })
      .then(page => {
        if (generation !== pickerGeneration.current || scopeKey !== pickerScopeRef.current) return;
        const cursorStatus = classifyAttachmentCursor(null, page.nextCursor, []);
        setPickerState({
          status: "READY",
          items: mergeAttachmentCandidates([], page.items, workspaceId),
          nextCursor: cursorStatus === "MORE" ? page.nextCursor : null,
          loadingMore: false,
          moreFailed: false,
          cursorStalled: cursorStatus === "STALLED",
        });
      })
      .catch(() => {
        if (generation !== pickerGeneration.current || scopeKey !== pickerScopeRef.current) return;
        setPickerState({ status: "FAILED", items: [], nextCursor: null, loadingMore: false, moreFailed: false, cursorStalled: false });
      });
  };

  const closePicker = () => {
    pickerGeneration.current += 1;
    pickerMoreRequest.current = null;
    setPickerOpen(false);
  };

  const loadMorePickerFiles = async () => {
    const cursor = pickerState.nextCursor;
    if (!workspaceId || !cursor || pickerMoreRequest.current) return;
    const generation = pickerGeneration.current;
    const scopeKey = pickerScopeKey;
    const request = { generation, cursor };
    pickerMoreRequest.current = request;
    setPickerState(current => ({ ...current, loadingMore: true, moreFailed: false }));
    try {
      const page = await invoke<{ items: AttachmentCandidate[]; nextCursor: string | null }>("list_resources", { workspaceId, cursor });
      if (generation !== pickerGeneration.current || scopeKey !== pickerScopeRef.current) return;
      const cursorStatus = classifyAttachmentCursor(cursor, page.nextCursor, [...consumedPickerCursors.current, cursor]);
      consumedPickerCursors.current.add(cursor);
      setPickerState(current => ({
        ...current,
        status: "READY",
        items: mergeAttachmentCandidates(current.items, page.items, workspaceId),
        nextCursor: cursorStatus === "MORE" ? page.nextCursor : null,
        loadingMore: false,
        moreFailed: false,
        cursorStalled: cursorStatus === "STALLED",
      }));
    } catch {
      if (generation !== pickerGeneration.current || scopeKey !== pickerScopeRef.current) return;
      setPickerState(current => ({ ...current, loadingMore: false, moreFailed: true }));
    } finally {
      if (pickerMoreRequest.current === request) pickerMoreRequest.current = null;
    }
  };

  const retryPendingConversationSend = async (request: PendingConversationSend) => {
    if (!operatorReady || sendingTurn || request.workspaceId !== workspaceId) return;
    const selectedAtRetryStart = selectedId;
    setSendingTurn(true);
    setTurnError(null);
    try {
      const rawOutcome = await invoke<unknown>("submit_conversation_turn", {
        workspaceId: request.workspaceId,
        conversationId: request.conversationId,
        requestId: request.requestId,
        content: request.content,
        resourceRefs: request.resourceRefs,
        agentBindingId: request.agentBindingId,
        presentationPreference: request.presentationPreference,
        attachmentLabels: request.attachmentLabels,
      });
      if (activeWorkspace.current !== request.workspaceId) return;
      const draftKey = conversationDraftScope(request.workspaceId, request.conversationId);
      const savedText = pendingConversationSendText(request);
      const outcome = parseConversationSendOutcome(rawOutcome, request.conversationId, request.presentationPreference);
      if (outcome.kind === "REFUSED") {
        if (!await acknowledgePendingConversationSend(request)) {
          setTurnError(`${attachmentRefusalNotice(outcome.message) ?? outcome.message} The saved request could not be cleared locally yet; retrying it is safe.`);
          return;
        }
        setConversationDrafts(current => {
          const currentText = current[draftKey] ?? "";
          if (currentText.trim() && currentText.trim() !== savedText.trim()) return current;
          return { ...current, [draftKey]: savedText };
        });
        const restoredChips = restorePendingConversationAttachmentChips(request, pickerState.items);
        setAttachmentChips(current => (current[draftKey]?.length ?? 0) > 0
          ? current
          : { ...current, [draftKey]: restoredChips });
        setTurnError(attachmentRefusalNotice(outcome.message) ?? outcome.message);
        if (selectedConversationId.current === selectedAtRetryStart) setSelectedId(request.conversationId);
        return;
      }
      if (!await acknowledgePendingConversationSend(request)) {
        setTurnError("The message was admitted, but its local recovery record could not be cleared. Retry the saved send to confirm it safely.");
        if (selectedConversationId.current === request.conversationId) await refreshSnapshot();
        return;
      }
      setConversationDrafts(current => {
        if ((current[draftKey] ?? "").trim() !== savedText.trim()) return current;
        return { ...current, [draftKey]: "" };
      });
      setAttachmentChips(current => {
        const currentChips = current[draftKey] ?? [];
        const currentRefs = toResourceRefs(currentChips, request.workspaceId);
        if (JSON.stringify(currentRefs) !== JSON.stringify(request.resourceRefs)) return current;
        return { ...current, [draftKey]: [] };
      });
      if (selectedConversationId.current === request.conversationId) await refreshSnapshot();
    } catch (cause) {
      const message = cause instanceof Error ? cause.message : String(cause);
      setTurnError(attachmentRefusalNotice(message) ?? message);
      try {
        const remaining = await reloadPendingConversationSends(request.workspaceId);
        const exactRequestStillPending = remaining.some(send =>
          send.workspaceId === request.workspaceId
          && send.conversationId === request.conversationId
          && send.requestId === request.requestId);
        if (!exactRequestStillPending) {
          // An untyped command error cannot establish refusal. Keep the exact
          // request visible in memory and offer same-key replay; never infer
          // pre-commit from a missing local journal row.
          setPendingConversationSends(current => retainPendingConversationSend(current, request));
        }
      } catch {
        // Keep the exact request visible; the journal check can be retried explicitly.
        if (activeWorkspace.current === request.workspaceId) {
          setPendingConversationSends(current => retainPendingConversationSend(current, request));
          setSendJournalStatus("FAILED");
        }
      }
    } finally {
      if (activeWorkspace.current === request.workspaceId) setSendingTurn(false);
    }
  };

  const sendTurn = async () => {
    if (selectedId && pendingSendForSelection) {
      await retryPendingConversationSend(pendingSendForSelection);
      return;
    }
    const text = composerDraft.trim();
    if (!text || !workspaceId || !operatorReady || !executionReady || sendingTurn || isTurnRunning) return;
    if (!bindingEligibleForSend || (selectedId && sendJournalStatus !== "READY")) return;
    if (!selectedId) {
      if (busy || createJournalStatus !== "READY" || !attachmentDraftKey) return;
      try {
        // Pin the exact binding shown by the selector. If this is the Workspace
        // default, retries cannot drift after the default changes while ambiguous.
        const selectedAgent = selectedAgentBindingId;
        if (!selectedAgent) return;
        const firstTurn: PendingConversationFirstTurn = {
          content: [{ kind: "TEXT", text }],
          resourceRefs: toResourceRefs(chips, workspaceId).map(reference => ({
            workspaceId: reference.workspace_id,
            resourceId: reference.resource_id,
            revisionId: reference.revision_id,
          })),
          agentBindingId: selectedAgent,
          presentationPreference: "RICH",
        };
        const normalizedTitle = Array.from(text).slice(0, 36).join("");
        const request = pendingCreates.get(workspaceId, normalizedTitle, () => crypto.randomUUID(), firstTurn);
        setSendingTurn(true);
        void submitCreate(request).finally(() => setSendingTurn(false));
      } catch (cause) {
        setTurnError(cause instanceof Error ? cause.message : String(cause));
      }
      return;
    }

    setSendingTurn(true);
    setTurnError(null);
    const draftKey = composerDraftKey;
    const reqId = typeof crypto !== "undefined" && crypto.randomUUID ? crypto.randomUUID() : `turn-req-${Date.now()}`;
    const references = toResourceRefs(chips, workspaceId);
    const savedRequest: PendingConversationSend = {
      workspaceId,
      conversationId: selectedId,
      requestId: reqId,
      content: [{ kind: "TEXT", text }],
      resourceRefs: references,
      agentBindingId: selectedAgentBindingId,
      presentationPreference: "RICH",
      attachmentLabels: chips.map(chip => chip.displayName),
    };
    try {
      const rawOutcome = await invoke<unknown>("submit_conversation_turn", {
        workspaceId,
        conversationId: selectedId,
        requestId: reqId,
        content: savedRequest.content,
        resourceRefs: savedRequest.resourceRefs,
        agentBindingId: savedRequest.agentBindingId,
        presentationPreference: savedRequest.presentationPreference,
        attachmentLabels: savedRequest.attachmentLabels,
      });
      if (activeWorkspace.current !== workspaceId) return;
      const outcome = parseConversationSendOutcome(rawOutcome, selectedId, savedRequest.presentationPreference);
      if (outcome.kind === "REFUSED") {
        if (!await acknowledgePendingConversationSend(savedRequest)) {
          setTurnError(`${attachmentRefusalNotice(outcome.message) ?? outcome.message} The saved request could not be cleared locally yet; retrying it is safe.`);
          return;
        }
        setTurnError(attachmentRefusalNotice(outcome.message) ?? outcome.message);
        return;
      }
      if (!await acknowledgePendingConversationSend(savedRequest)) {
        setTurnError("The message was admitted, but its local recovery record could not be cleared. Retry the saved send to confirm it safely.");
        await refreshSnapshot();
        return;
      }
      if (draftKey) {
        setConversationDrafts(current => (current[draftKey] ?? "").trim() === text
          ? { ...current, [draftKey]: "" }
          : current);
        setAttachmentChips(current => JSON.stringify(toResourceRefs(current[draftKey] ?? [], workspaceId)) === JSON.stringify(references)
          ? { ...current, [draftKey]: [] }
          : current);
      }
      await refreshSnapshot();
    } catch (cause) {
      const msg = cause instanceof Error ? cause.message : String(cause);
      setTurnError(attachmentRefusalNotice(msg) ?? msg);
      try {
        await reloadPendingConversationSends(workspaceId);
      } catch {
        if (activeWorkspace.current === workspaceId) {
          setPendingConversationSends(current => retainPendingConversationSend(current, savedRequest));
          setSendJournalStatus("FAILED");
        }
      }
    } finally {
      if (activeWorkspace.current === workspaceId) setSendingTurn(false);
    }
  };

  const refreshSnapshot = async () => {
    if (!workspaceId || !selectedId) return;
    try {
      const value = await invoke<unknown>("get_conversation_presentation", { workspaceId, conversationId: selectedId });
      const fresh = parseConversationSnapshot(value, workspaceId, selectedId);
      setSnapshot(current => current && current.conversation_id === selectedId ? mergeConversationStreamSnapshot(current, fresh, workspaceId, selectedId) : current);
    } catch {
      // The live stream will deliver the next snapshot; nothing else to do here.
    }
  };
  const cancelTurn = async () => {
    if (!selectedId || !workspaceId || !operatorReady || !stopping || stopPending) return;
    const reqId = typeof crypto !== "undefined" && crypto.randomUUID ? crypto.randomUUID() : `cancel-req-${Date.now()}`;
    setStopPending(true);
    setTurnError(null);
    try {
      await invoke<unknown>("cancel_conversation_turn", {
        workspaceId,
        conversationId: selectedId,
        turnId: stopping.turnId,
        expectedVersion: stopping.expectedVersion,
        requestId: reqId,
      });
    } catch {
      // A stale version or a racing settlement: reload the projected turn and let the owner decide again.
      setTurnError("The turn changed before Stop was applied. The conversation was refreshed; try Stop again if it is still running.");
      await refreshSnapshot();
    } finally {
      setStopPending(false);
    }
  };
  const retryTurn = async () => {
    if (!selectedId || !workspaceId || !operatorReady || !executionReady || !retrying || retryPending) return;
    const reqId = typeof crypto !== "undefined" && crypto.randomUUID ? crypto.randomUUID() : `retry-req-${Date.now()}`;
    setRetryPending(true);
    setTurnError(null);
    try {
      await invoke<unknown>("retry_conversation_turn", {
        workspaceId,
        conversationId: selectedId,
        turnId: retrying.turn_id,
        expectedVersion: retrying.version,
        requestId: reqId,
      });
    } catch {
      setTurnError("The turn changed before Retry was applied. The conversation was refreshed; try Retry again if it is still failed.");
      await refreshSnapshot();
    } finally {
      setRetryPending(false);
    }
  };
  const statusLabel = turnStatusLabel(activeStatus);
  const streamStatusLabel = currentPresentationStream?.resyncRequired ? "Refreshing saved conversation updates…"
    : streamStatus === "LIVE" ? "Saved history updates live"
    : streamStatus === "RECONNECTING" ? "Reconnecting saved-history updates…"
      : streamStatus === "CONNECTING" ? "Connecting saved-history updates…"
        : null;
  // Stick-to-bottom autoscroll (001-T26, AC-030): follow new content only while the reader is
  // already at the bottom and not selecting text; otherwise keep the position and show an indicator.
  const [showNewMessages, setShowNewMessages] = useState(false);
  const atBottomRef = useRef(true);
  const lastContentSignatureRef = useRef<string | null>(null);
  const followScrollSelectionKey = useRef<string | null>(null);
  const composerTextareaRef = useRef<HTMLTextAreaElement>(null);
  const wasOccupyingRef = useRef(false);
  const onStreamScroll = () => {
    const element = streamContainerRef.current;
    if (!element) return;
    const atBottom = isAtBottom(element);
    atBottomRef.current = atBottom;
    if (atBottom) setShowNewMessages(false);
  };
  const scrollToBottom = (behavior: ScrollBehavior = "auto") => {
    const element = streamContainerRef.current;
    if (!element) return;
    element.scrollTo({ top: element.scrollHeight, behavior });
    atBottomRef.current = true;
    setShowNewMessages(false);
  };
  const contentIds = (currentSnapshot?.messages ?? []).map(message => message.message_id);
  const transientLength = transientTurns.reduce((total, turn) => total + turn.text.length, 0);
  const signature = contentSignature(contentIds, transientLength);
  useEffect(() => {
    if (followScrollSelectionKey.current !== selectionKey) {
      // A different Conversation starts at the bottom without an indicator.
      followScrollSelectionKey.current = selectionKey;
      lastContentSignatureRef.current = signature;
      atBottomRef.current = true;
      setShowNewMessages(false);
      scrollToBottom();
      return;
    }
    if (lastContentSignatureRef.current === signature) return;
    lastContentSignatureRef.current = signature;
    const selection = typeof window !== "undefined" ? window.getSelection?.() : null;
    const decision = scrollFollowDecision({
      wasAtBottom: atBottomRef.current,
      hasNewContent: true,
      selectionActive: Boolean(selection && !selection.isCollapsed && selection.toString().length > 0),
    });
    if (decision === "FOLLOW") scrollToBottom();
    else if (decision === "INDICATE") setShowNewMessages(true);
  }, [signature, selectionKey]);
  // When a turn leaves the composer fence, return focus to the composer (Stop's accessibility contract).
  useEffect(() => {
    if (wasOccupyingRef.current && !isTurnRunning) composerTextareaRef.current?.focus();
    wasOccupyingRef.current = isTurnRunning;
  }, [isTurnRunning]);

    
  return (
    <div className="conversations-page-modern">
      {/* Top Breadcrumbs & Window Actions Bar */}
      <header className="conversation-top-breadcrumbs">
        <span className="crumb-workspace">{workspaceName}</span>
        <span className="crumb-sep">/</span>
        <strong className="crumb-title">{conversationTitle(selected?.title)}</strong>
        {statusLabel && <span className="conversation-header-badge">{statusLabel}</span>}
        {streamStatusLabel && <span className="conversation-header-badge">{streamStatusLabel}</span>}
        {(error || createReceiptError) && <span className="conversation-header-badge is-error">{error || createReceiptError?.message}</span>}
        <div className="conversation-top-actions">
          <button className="conversation-action-icon-btn" type="button" title="More options are unavailable" aria-label="More options (unavailable)" disabled>
            <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
              <circle cx="12" cy="5" r="2" />
              <circle cx="12" cy="12" r="2" />
              <circle cx="12" cy="19" r="2" />
            </svg>
          </button>
          <button
            className="conversation-action-icon-btn"
            type="button"
            onClick={onToggleWorkbench}
            title={workbenchOpen ? "Close side pane" : "Open side pane"}
          >
            <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
              <rect x="3" y="3" width="18" height="18" rx="2" />
              <path d="M15 3v18" />
            </svg>
          </button>
        </div>
      </header>

      {createJournalStatus === "FAILED" && (
        <div className="conversation-create-journal-error" role="alert">
          <p>Pending Conversation requests could not be checked. New Chat is paused to avoid duplicating an uncertain create request.</p>
          <button
            className="quiet-button"
            type="button"
            disabled={!operatorReady}
            onClick={() => {
              setCreateJournalStatus("LOADING");
              setCreateJournalAttempt(current => current + 1);
            }}
          >
            Retry checking pending Conversation requests
          </button>
        </div>
      )}

      {sendJournalStatus === "LOADING" && operatorReady && (
        <div className="conversation-create-journal-error" role="status">
          <p>Checking for a message that may need safe retry…</p>
        </div>
      )}
      {sendJournalStatus === "FAILED" && operatorReady && (
        <div className="conversation-create-journal-error" role="alert">
          <p>Pending sends could not be checked. Sending in an existing Conversation is paused until the saved request is confirmed.</p>
          <button className="quiet-button" type="button" onClick={() => {
            setSendJournalStatus("LOADING");
            setSendJournalAttempt(current => current + 1);
          }}>
            Check pending sends again
          </button>
        </div>
      )}
      {pendingSendsInWorkspace.map(request => {
        const text = pendingConversationSendText(request);
        const title = items.find(item => item.conversation_id === request.conversationId)?.title;
        return (
          <div className="conversation-create-journal-error" role="status" key={`${request.workspaceId}\u0000${request.conversationId}\u0000${request.requestId}`}>
            <p>
              <strong>Saved send needs resolution.</strong> The exact request is retained on this device; retry uses the same identity to confirm or replay it without creating a duplicate message
              {title ? ` in “${title}”` : " in this Conversation"}.
              {text && <> Preview: “{text.slice(0, 160)}{text.length > 160 ? "…" : ""}”</>}
              {request.attachmentLabels.length > 0 && <> Attachments: {request.attachmentLabels.join(", ")}.</>}
            </p>
            {text && <details>
              <summary>View saved message</summary>
              <p>{text}</p>
            </details>}
            {selectedId !== request.conversationId && (
              <button className="quiet-button" type="button" onClick={() => setSelectedId(request.conversationId)}>
                Open Conversation
              </button>
            )}
            <button
              className="quiet-button"
              type="button"
              disabled={!operatorReady || sendingTurn}
              onClick={() => void retryPendingConversationSend(request)}
            >
              {sendingTurn && selectedId === request.conversationId ? "Checking saved send…" : "Retry saved send"}
            </button>
          </div>
        );
      })}

      {createJournalStatus === "READY" && pendingCreateRequests
        .filter(request => request.workspaceId === workspaceId)
        .map(request => {
          const firstText = request.firstTurn?.content.find(block => block.kind === "TEXT")?.text;
          return (
            <div className="conversation-create-journal-error" role="status" key={request.requestId}>
              <p>{request.firstTurn
                ? `A first message is saved for safe retry: “${firstText?.slice(0, 96) ?? request.title ?? "New conversation"}”.`
                : `A Conversation creation receipt is awaiting confirmation: “${request.title ?? "Untitled conversation"}”.`}</p>
              <button className="quiet-button" type="button" disabled={!operatorReady || busy}
                onClick={() => void submitCreate(request)}>
                Retry safely
              </button>
            </div>
          );
        })}

      {/* Main Conversation Canvas */}
      <div className="conversation-stream-container" ref={streamContainerRef} onScroll={onStreamScroll}>
        <div className="conversation-messages-flow">
          {(currentSnapshot?.messages && currentSnapshot.messages.length > 0) || transientTurns.length > 0 ? (
            <>
              {currentSnapshot?.next_cursor && (
                <button
                  type="button"
                  className="conversation-load-more"
                  onClick={() => void loadMoreMessages()}
                  disabled={!operatorReady || busy || loadingMore}
                >
                  {loadingMore ? "Loading messages..." : "Load earlier messages"}
                </button>
              )}

              {(currentSnapshot?.messages ?? []).map(message => {
                const source = selected?.conversation_id
                  ? findTaskPromotionSource(workspaceId, selected.conversation_id, currentSnapshot?.messages ?? [], message.message_id)
                  : null;
                return <article className={"message-row " + message.role.toLowerCase()} key={message.message_id}>
                  {message.role === "USER" ? (
                    <div className="message-user-wrap">
                      <div className="message-user-bubble">{messageText(message)}</div><time className="message-time">{timeLabel(message.created_at)}</time>
                    </div>
                  ) : (
                    <div className="message-assistant-wrap">
                      <ConversationProducerLabel message={message} bindings={agentBindings} profiles={agentProfiles} />
                      <div className="message-assistant-body">
                        <ConversationMessageContent
                          workspaceId={workspaceId}
                          conversationId={selected?.conversation_id ?? ""}
                          message={message}
                          autoLoadRich={automaticRichIds.has(message.message_id)}
                          presentationEpoch={richPresentationEpochs.get(message.message_id) ?? 0}
                        />
                        {source && <button
                          ref={element => {
                            if (element) taskPromotionTriggers.current.set(message.message_id, element);
                            else taskPromotionTriggers.current.delete(message.message_id);
                          }}
                          type="button"
                          className="conversation-task-promotion-trigger"
                          disabled={!operatorReady}
                          title={!operatorReady ? "Start the local Runtime to create a Task" : "Create a Task from this completed reply"}
                          onClick={() => openTaskPromotion(source)}
                        >Continue as Task</button>}
                      </div>
                    </div>
                  )}
                </article>;
              })}

              {activityEntries.length > 0 && !retrying && (
                <article className="message-row agent turn-activity-row">
                  <div className="message-assistant-wrap"><div className="message-assistant-body"><TurnActivityView entries={activityEntries} /></div></div>
                </article>
              )}

              {Object.values(currentNativePermissionState.prompts).sort((a, b) => a.sequence - b.sequence).map(prompt => (
                <article className="message-row agent native-permission-row" key={`native-permission-${prompt.promptToken}`}>
                  <div className="message-assistant-wrap"><div className="message-assistant-body">
                    <NativePermissionCard
                      prompt={prompt}
                      disabled={!operatorReady || nativePermissionSnapshotError || currentNativePermissionState.needsRefresh || prompt.state !== "PENDING"}
                      refreshDisabled={!operatorReady}
                      sendingDecision={nativePermissionActions[prompt.promptToken] ?? null}
                      onDecision={decision => void sendNativePermissionDecision(prompt.promptToken, decision)}
                      onRefresh={() => {
                        if (!livePermissionScope || !livePermissionKey) return;
                        void refreshNativePermissionSnapshot(livePermissionScope, livePermissionKey)
                          .catch(() => setNativePermissionSnapshotError(true));
                      }}
                    />
                  </div></div>
                </article>
              ))}
              {nativePermissionSnapshotError && <div className="native-permission-notice native-permission-refresh-error" role="alert">
                <span>The live agent request could not be checked. Decisions stay disabled until its current state is confirmed.</span>
                {livePermissionScope && livePermissionKey && <button type="button" className="quiet-button" disabled={!operatorReady}
                  onClick={() => void refreshNativePermissionSnapshot(livePermissionScope, livePermissionKey)
                    .catch(() => setNativePermissionSnapshotError(true))}>Check live status</button>}
              </div>}

              {transientTurns.map(turn => (
                <article
                  className="message-row agent transient"
                  key={`${turn.conversationId}\0${turn.turnId}\0${turn.retryOrdinal}`}
                >
                  <div className="message-assistant-wrap">
                    <div className="message-assistant-body">
                      {turn.text}
                      <span className="assistant-streaming-cursor" />
                    </div>
                  </div>
                </article>
              ))}

              {retrying && (
                <article className="message-row agent turn-failed" role="group" aria-label={`Turn failed. ${retryReasonLabel(retrying.reason_code)}`}>
                  <div className="message-assistant-wrap">
                    <div className="message-assistant-body turn-failed-card">
                      <p><strong>This turn failed.</strong> {retryReasonLabel(retrying.reason_code)} Your message was kept.</p>
                      {!executionReady && <p role="status">Retry is paused until the current Runtime is online and ready.</p>}
                      <TurnActivityView entries={activityEntries} />
                      <button
                        className="quiet-button"
                        type="button"
                        disabled={!canRetryTurn(retrying, retryPending || !operatorReady, executionReady)}
                        aria-label={`Retry turn. ${retryReasonLabel(retrying.reason_code)}`}
                        onClick={() => void retryTurn()}
                      >
                        {retryPending ? "Retrying…" : "Retry"}
                      </button>
                    </div>
                  </div>
                </article>
              )}
            </>
          ) : (
            <div className="message-assistant-wrap">
              {/*
                A-26 empty-state contract (docs/V1-UI-SCREEN-CONTRACT.md "Conversation screen"):
                the empty canvas must be truthful and LiteCowork-branded. It must NOT carry
                third-party "Antigravity" branding and must NOT advertise any capability the
                current build cannot perform. Ordinary chat routes through the selected native
                agent harness; nothing here claims terminal execution, verification or other
                unqualified behavior.
              */}
              <div className="message-assistant-body conversation-empty-state">
                <p>
                  Start a conversation in this Workspace. Messages are sent to the native agent you select,
                  which owns its own reasoning, models and tools.
                </p>
                <p>
                  Choose an available agent in the composer below, then type your message. Ordinary chat does
                  not create a Task or require a Coworker.
                </p>
              </div>
            </div>
          )}
        </div>

        {/* Scroll to bottom button, with the new-messages indicator while the reader is in history */}
        <div className="scroll-to-bottom-wrap">
          {showNewMessages && (
            <span className="new-messages-indicator" role="status">New messages</span>
          )}
          <button
            className="scroll-to-bottom-btn"
            type="button"
            onClick={() => scrollToBottom("smooth")}
            title={showNewMessages ? "New messages: scroll to bottom" : "Scroll to bottom"}
            aria-label={showNewMessages ? "New messages, scroll to bottom" : "Scroll to bottom"}
          >
            ↓
          </button>
        </div>
        <div ref={streamBottomRef} />
      </div>

      {/* Floating Composer Card */}
      <div className="floating-composer-card">
        {turnError && <p className="composer-turn-error" role="alert">{turnError}</p>}
        {visibleAgentMutationNotice && <p className="composer-notice">{visibleAgentMutationNotice.message}</p>}
        {!executionReady && !pendingSendForSelection && !isTurnRunning && (
          <p className="composer-notice" role="status">
            {runtimeLifecycleLoading
              ? "Checking Runtime recovery status. Fresh sends and retries are paused."
              : "Fresh sends and retries are paused until the current Runtime is online and ready."}{" "}
            <button className="text-button" type="button" onClick={() => void onRefreshRuntimeStatus()}>Refresh Runtime status</button>
          </p>
        )}
        <textarea
          className="composer-textarea"
          aria-label="Message draft"
          ref={composerTextareaRef}
          placeholder={pendingSendForSelection ? "A saved send is awaiting confirmation above" : isTurnRunning ? "Assistant is responding..." : COMPOSER_PLACEHOLDER}
          value={composerDraft}
          disabled={isTurnRunning || pendingSendForSelection !== null || sendJournalStatus !== "READY"}
          onChange={event => {
            if (!composerDraftKey) return;
            setConversationDrafts(current => ({ ...current, [composerDraftKey]: event.target.value }));
          }}
          onKeyDown={event => {
            if (event.key === "Enter" && !event.shiftKey) {
              event.preventDefault();
              void sendTurn();
            }
          }}
          rows={1}
        />
        <div className="composer-footer-toolbar">
          <div className="composer-footer-left">
            <button
              className="composer-pill-btn"
              type="button"
              title="Attach a text file from this Workspace"
              aria-label="Add context or file"
              aria-expanded={pickerOpen}
              disabled={!operatorReady || !workspaceId || isTurnRunning || pendingSendForSelection !== null || sendJournalStatus !== "READY"}
              onClick={() => (pickerOpen ? closePicker() : openPicker())}
            >
              +
            </button>
            {pickerOpen && (
              <div className="composer-attachment-picker" role="dialog" aria-label="Attach a file from this Workspace" onKeyDown={event => { if (event.key === "Escape") closePicker(); }}>
                {pickerState.status === "LOADING" && <p role="status">Loading files…</p>}
                {pickerState.status === "FAILED" && <p role="alert">Files could not be loaded. <button className="text-button" type="button" onClick={openPicker}>Retry</button></p>}
                {pickerState.status === "READY" && pickerState.cursorStalled && <p role="alert">The file list did not advance. <button className="text-button" type="button" onClick={openPicker}>Reopen the picker to refresh</button></p>}
                {pickerState.status === "READY" && pickerState.items.length === 0 && pickerState.nextCursor !== null && <p>No text files on this page. More files may be available below.</p>}
                {pickerState.status === "READY" && pickerState.items.length === 0 && pickerState.nextCursor === null && !pickerState.cursorStalled && <p>No text files in this Workspace yet.</p>}
                <ul>
                  {pickerState.items.map(item => (
                    <li key={item.resourceId}>
                      <button type="button" onClick={() => {
                        if (!attachmentDraftKey || !workspaceId || pendingSendForSelection) return;
                        setAttachmentChips(current => ({ ...current, [attachmentDraftKey]: addAttachment(current[attachmentDraftKey] ?? [], item, workspaceId) }));
                        closePicker();
                      }}>{item.displayName}</button>
                    </li>
                  ))}
                </ul>
                {pickerState.status === "READY" && pickerState.moreFailed && <p role="alert">More files could not be loaded. Your selected files are still available. <button className="text-button" type="button" disabled={pickerState.loadingMore} onClick={() => void loadMorePickerFiles()}>Retry</button></p>}
                {pickerState.status === "READY" && pickerState.nextCursor !== null && !pickerState.moreFailed && <button className="text-button" type="button" disabled={pickerState.loadingMore} onClick={() => void loadMorePickerFiles()}>{pickerState.loadingMore ? "Loading more files…" : "Load more files"}</button>}
              </div>
            )}
            {chips.map(chip => (
              <span className="composer-attachment-chip" key={chip.resourceId}>
                {chip.displayName}
                <button type="button" aria-label={attachmentChipLabel(chip)} disabled={pendingSendForSelection !== null} onClick={() => attachmentDraftKey && setAttachmentChips(current => ({ ...current, [attachmentDraftKey]: removeAttachment(current[attachmentDraftKey] ?? [], chip.resourceId) }))}>×</button>
              </span>
            ))}
            <div className="model-selector-pill" title="Active Model">
              <div className="agent-select-wrapper">
                <select
                  value={selected?.active_agent_binding_id ?? newChatAgentBindingId ?? ""}
                  disabled={!operatorReady || agentCatalogStatus === "LOADING" || agentMutationNotice?.status === "SAVING" || pendingSendForSelection !== null}
                  onChange={event => selected
                    ? void saveConversationAgentBinding(event.target.value || null)
                    : setNewChatAgentBindingId(event.target.value || null)}
                >
                  <option value="">{defaultAgentOptionLabel(workspaceDefaultOptionLabel)}</option>
                  {agentChoices.map(choice => (
                    <option key={choice.agentBindingId} value={choice.agentBindingId} disabled={!choice.selectable}>
                      {choice.label}
                    </option>
                  ))}
                </select>
              </div>
              <span className="pill-chevron">⌄</span>
            </div>
          </div>

          <div className="composer-footer-right">
            <button className="composer-icon-btn" type="button" title="Voice input is unavailable" aria-label="Voice input (unavailable)" disabled>
              <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.8">
                <path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z" />
                <path d="M19 10v2a7 7 0 0 1-14 0v-2" />
                <line x1="12" y1="19" x2="12" y2="23" />
                <line x1="8" y1="23" x2="16" y2="23" />
              </svg>
            </button>
            {isTurnRunning ? (
              <button
                className="composer-submit-btn is-running"
                type="button"
                disabled={!stopping || stopPending || !operatorReady}
                onClick={() => void cancelTurn()}
                title={activeStatus === "CANCEL_REQUESTED" ? "Stopping" : stopping ? "Stop responding" : "Preparing"}
                aria-label={activeStatus === "CANCEL_REQUESTED" ? "Stopping the response" : stopping ? "Stop responding" : "Preparing the response"}
              >
                ■
              </button>
            ) : (
              <button
                className="composer-submit-btn"
                type="button"
                disabled={!sendEligible || (selectedId !== null && (sendJournalStatus !== "READY" || pendingSendForSelection !== null))}
                title={
                  !composerDraft.trim()
                    ? "Enter a message"
                    : selectedId && sendJournalStatus !== "READY"
                      ? "Checking pending send recovery"
                      : !operatorReady
                      ? "Operator is not ready"
                      : !executionReady
                        ? "Runtime must be online and ready before starting new work"
                      : !bindingEligibleForSend
                        ? "Select an available agent to send"
                        : sendingTurn
                          ? "Sending…"
                          : "Send message"
                }
                onClick={() => void sendTurn()}
              >
                ✦
              </button>
            )}
          </div>
        </div>
      </div>
      {taskPromotionSource && <ConversationTaskPromotionDialog
        workspaceId={workspaceId}
        workspaceName={workspaceName}
        operatorReady={operatorReady}
        source={taskPromotionSource}
        preferredLeadId={(currentSnapshot?.messages ?? []).find(message => message.message_id === taskPromotionSource.agentMessageId)?.agent_binding_id ?? null}
        bindings={agentBindings}
        profiles={agentProfiles}
        onClose={closeTaskPromotion}
        onSaved={taskId => {
          setTaskPromotionSource(null);
          onTaskCreated?.(taskId);
        }}
      />}
    </div>
  );
};