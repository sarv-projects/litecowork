import { useEffect, useRef, useState } from "react";
import type { SuggestionApi, Suggestion, SuggestionKind, SuggestionPreference, SuggestionTaskAcceptanceReceipt, SuggestionVisibility } from "./suggestion-api";
import "./suggestions-page.css";

const KIND: Record<Suggestion["kind"], string> = {
  TASK_OPPORTUNITY: "Task idea",
  ROUTINE_OPPORTUNITY: "Routine idea",
  AUTOMATION_OPPORTUNITY: "Automation idea",
};
const PREFERENCE_KIND: Record<SuggestionKind, string> = {
  TASK_OPPORTUNITY: "Task ideas",
  ROUTINE_OPPORTUNITY: "Routine ideas",
  AUTOMATION_OPPORTUNITY: "Automation ideas",
};
const ACTION: Record<Suggestion["proposed_action"], string> = {
  TASK: "Would prepare a Task for your review",
  OPEN_ROUTINE_EDITOR: "Would open the Routine editor",
  OPEN_AUTOMATION_EDITOR: "Would open the Automation editor",
};
const formatTime = (value: string) => {
  const date = new Date(value);
  return Number.isNaN(date.valueOf()) ? "time unavailable" : date.toLocaleString();
};

type SuggestionMutation = {
  type: "suggestion";
  key: string;
  workspaceId: string;
  suggestionId: string;
  expectedVersion: number;
  operation: "dismiss" | "snooze" | "unsnooze";
  requestId: string;
  snoozedUntil?: string;
};
type PreferenceMutation = {
  type: "preference";
  key: string;
  workspaceId: string;
  kind: SuggestionKind;
  muted: boolean;
  expectedVersion: number;
  requestId: string;
};
type AcceptanceMutation = {
  type: "acceptance";
  key: string;
  workspaceId: string;
  suggestionId: string;
  expectedVersion: number;
  requestId: string;
};
type RecoverableMutation = SuggestionMutation | PreferenceMutation | AcceptanceMutation;
type MutationRecoveryStore = { workspaceId: string; records: Map<string, RecoverableMutation>; listeners: Set<() => void> };

// Key by the stable Workspace ID, not the API object: App recreates APIs when the
// selected Workspace changes, so object identity would lose A's request on A→B→A.
// This registry is process-local only and is intentionally not persisted.
const mutationRecovery = new Map<string, MutationRecoveryStore>();
const MAX_UNRESOLVED_MUTATIONS_PER_WORKSPACE = 64;
const MAX_UNRESOLVED_MUTATIONS_TOTAL = 256;

function recoveryStore(workspaceId: string): MutationRecoveryStore {
  if (!workspaceId.trim()) throw new Error("Select a Workspace before recovering suggestion changes.");
  let store = mutationRecovery.get(workspaceId);
  if (!store) {
    store = { workspaceId, records: new Map(), listeners: new Set() };
    mutationRecovery.set(workspaceId, store);
  }
  return store;
}

function notifyRecoveryStore(store: MutationRecoveryStore) {
  for (const listener of store.listeners) listener();
  if (store.records.size === 0 && store.listeners.size === 0 && mutationRecovery.get(store.workspaceId) === store) {
    mutationRecovery.delete(store.workspaceId);
  }
}

function canAddRecoveryMutation(store: MutationRecoveryStore): boolean {
  const total = [...mutationRecovery.values()].reduce((count, current) => count + current.records.size, 0);
  return store.records.size < MAX_UNRESOLVED_MUTATIONS_PER_WORKSPACE && total < MAX_UNRESOLVED_MUTATIONS_TOTAL;
}

function suggestionMutationKey(suggestionId: string, expectedVersion: number, operation: SuggestionMutation["operation"]): string {
  return JSON.stringify(["suggestion", suggestionId, expectedVersion, operation]);
}

function preferenceMutationKey(kind: SuggestionKind, expectedVersion: number, muted: boolean): string {
  return JSON.stringify(["preference", kind, expectedVersion, muted]);
}

function acceptanceMutationKey(suggestionId: string, expectedVersion: number): string {
  return JSON.stringify(["acceptance", suggestionId, expectedVersion]);
}

function validAcceptanceReceipt(receipt: SuggestionTaskAcceptanceReceipt, request: AcceptanceMutation): boolean {
  return receipt.workspace_id === request.workspaceId
    && receipt.suggestion.suggestion_id === request.suggestionId
    && receipt.suggestion.status === "ACCEPTED"
    && receipt.suggestion.version === request.expectedVersion + 1
    && receipt.suggestion.result_task_id === receipt.task.task_id
    && receipt.task.workspace_id === request.workspaceId
    && receipt.task.status === "READY"
    && Number.isSafeInteger(receipt.task.version) && receipt.task.version > 0;
}

function validSuggestionReceipt(receipt: Suggestion, request: SuggestionMutation): boolean {
  if (receipt.workspace_id !== request.workspaceId || receipt.suggestion_id !== request.suggestionId
      || receipt.version <= request.expectedVersion) return false;
  if (request.operation === "dismiss") {
    return receipt.status === "DISMISSED" && receipt.resolution_reason === "DISMISSED_BY_OWNER"
      && receipt.resolved_at !== null && receipt.resolved_by !== null && receipt.result_task_id === null;
  }
  if (receipt.status !== "PROPOSED" || receipt.resolved_at !== null || receipt.resolved_by !== null
      || receipt.resolution_reason !== null || receipt.result_task_id !== null) return false;
  if (request.operation === "unsnooze") return receipt.snoozed_until === null;
  return request.snoozedUntil !== undefined && receipt.snoozed_until !== null
    && Date.parse(receipt.snoozed_until) === Date.parse(request.snoozedUntil);
}

function validPreferenceReceipt(
  receipt: SuggestionPreference,
  request: PreferenceMutation,
): boolean {
  return receipt.workspace_id === request.workspaceId && receipt.kind === request.kind && receipt.muted === request.muted
    && receipt.version > request.expectedVersion;
}

export function SuggestionsPage({ api, onTaskAccepted }: { api: SuggestionApi; onTaskAccepted: (taskId: string) => void }) {
  const [visibility, setVisibility] = useState<SuggestionVisibility>("VISIBLE");
  const [items, setItems] = useState<Suggestion[]>([]);
  const [next, setNext] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [statusAnnouncement, setStatusAnnouncement] = useState("");
  const [reload, setReload] = useState(0);
  const [pendingAction, setPendingAction] = useState<string | null>(null);
  const [preferences, setPreferences] = useState<SuggestionPreference[] | null>(null);
  const [preferenceError, setPreferenceError] = useState<string | null>(null);
  const [pendingPreference, setPendingPreference] = useState<SuggestionKind | null>(null);
  const store = recoveryStore(api.workspaceId);
  const [recoveryRevision, setRecoveryRevision] = useState(0);
  const listGeneration = useRef(0);

  useEffect(() => {
    const listener = () => setRecoveryRevision(value => value + 1);
    store.listeners.add(listener);
    // Keep empty per-Workspace stores: deleting them in effect cleanup breaks
    // React StrictMode's setup/cleanup/setup cycle and can orphan later requests.
    return () => { store.listeners.delete(listener); };
  }, [store]);

  useEffect(() => {
    const controller = new AbortController();
    listGeneration.current += 1;
    let current = true;
    setLoading(true); setLoadingMore(false); setError(null); setItems([]); setNext(null);
    void api.list(visibility, undefined, controller.signal).then(page => {
      if (current) {
        setItems(page.items); setNext(page.next_cursor);
        const label = visibility === "VISIBLE" ? "For you" : visibility === "SNOOZED" ? "Snoozed" : "All current";
        setStatusAnnouncement(`${label}: ${page.items.length} ${page.items.length === 1 ? "idea" : "ideas"} loaded.`);
      }
    }).catch(reason => {
      if (current && !(reason instanceof DOMException && reason.name === "AbortError")) {
        setError(reason instanceof Error ? reason.message : "Suggestions could not be loaded.");
      }
    }).finally(() => { if (current) setLoading(false); });
    return () => { current = false; controller.abort(); };
  }, [api, visibility, reload]);

  useEffect(() => {
    let current = true;
    void api.preferences().then(value => {
      if (current) { setPreferences(value); setPreferenceError(null); }
    }).catch(reason => {
      if (current) setPreferenceError(reason instanceof Error ? reason.message : "Suggestion settings could not be loaded.");
    });
    return () => { current = false; };
  }, [api, reload]);

  async function loadMore() {
    if (!next || loadingMore) return;
    const activeGeneration = listGeneration.current;
    setLoadingMore(true); setError(null);
    try {
      const page = await api.list(visibility, next);
      if (activeGeneration !== listGeneration.current) return;
      const known = new Set(items.map(item => item.suggestion_id));
      const additions = page.items.filter(item => !known.has(item.suggestion_id));
      setItems(current => {
        const known = new Set(current.map(item => item.suggestion_id));
        return [...current, ...page.items.filter(item => !known.has(item.suggestion_id))];
      });
      setNext(page.next_cursor);
      setStatusAnnouncement(`${additions.length} more ${additions.length === 1 ? "idea" : "ideas"} loaded.`);
    } catch (reason) {
      if (activeGeneration === listGeneration.current) setError(reason instanceof Error ? reason.message : "More suggestions could not be loaded.");
    } finally { if (activeGeneration === listGeneration.current) setLoadingMore(false); }
  }

  async function ownerAction(item: Suggestion, action: "dismiss" | "snooze" | "unsnooze") {
    if (item.workspace_id !== api.workspaceId) {
      setError("This idea belongs to a different Workspace. Reload before changing it.");
      return;
    }
    if (pendingAction) return;
    setPendingAction(item.suggestion_id);
    setError(null);
    try {
      const requestKey = suggestionMutationKey(item.suggestion_id, item.version, action);
      let request = store.records.get(requestKey) as SuggestionMutation | undefined;
      if (!request) {
        if (!canAddRecoveryMutation(store)) {
          throw new Error("There are too many unresolved idea changes. Retry or discard one before starting another.");
        }
        request = {
          type: "suggestion", key: requestKey, workspaceId: api.workspaceId, suggestionId: item.suggestion_id,
          expectedVersion: item.version, operation: action, requestId: crypto.randomUUID(),
        };
        if (action === "snooze") {
          const expires = Date.parse(item.expires_at);
          const now = Date.now();
          const until = Math.min(now + 24 * 60 * 60 * 1000, expires);
          if (!Number.isFinite(expires) || until <= now) throw new Error("This idea has expired. Refresh to see its settled state.");
          request.snoozedUntil = new Date(until).toISOString();
        }
        store.records.set(requestKey, request);
        notifyRecoveryStore(store);
        setRecoveryRevision(value => value + 1);
      }
      const receipt = request.operation === "dismiss"
        ? await api.dismiss(request.suggestionId, request.expectedVersion, request.requestId)
        : request.operation === "unsnooze"
          ? await api.unsnooze(request.suggestionId, request.expectedVersion, request.requestId)
          : await api.snooze(request.suggestionId, request.expectedVersion, request.requestId, request.snoozedUntil!);
      if (!validSuggestionReceipt(receipt, request)) throw new Error("The Runtime response did not confirm this exact idea change. Retry the saved request or discard it after reviewing the latest state.");
      store.records.delete(requestKey);
      notifyRecoveryStore(store);
      setRecoveryRevision(value => value + 1);
      setStatusAnnouncement(action === "dismiss" ? "Idea dismissed." : action === "snooze" ? "Idea snoozed." : "Idea shown now.");
      setReload(value => value + 1);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "The idea could not be updated.");
    } finally { setPendingAction(null); }
  }

  async function acceptTask(item: Suggestion) {
    if (pendingAction || item.workspace_id !== api.workspaceId || item.proposed_action !== "TASK" || !item.proposed_task_spec) return;
    setPendingAction(item.suggestion_id);
    setError(null);
    const key = acceptanceMutationKey(item.suggestion_id, item.version);
    let request = store.records.get(key) as AcceptanceMutation | undefined;
    if (!request) {
      if (!canAddRecoveryMutation(store)) {
        setError("There are too many unresolved idea changes. Retry or discard one before creating another Task.");
        setPendingAction(null);
        return;
      }
      request = {
        type: "acceptance", key, workspaceId: api.workspaceId,
        suggestionId: item.suggestion_id, expectedVersion: item.version, requestId: crypto.randomUUID(),
      };
      store.records.set(key, request);
      notifyRecoveryStore(store);
      setRecoveryRevision(value => value + 1);
    }
    try {
      const receipt = await api.acceptTask(request.suggestionId, request.expectedVersion, request.requestId);
      if (!validAcceptanceReceipt(receipt, request)) throw new Error("The Runtime response did not confirm this exact Suggestion and READY Task. Retry the saved request or discard it after reviewing the latest state.");
      if (store.records.get(key) === request) {
        store.records.delete(key);
        notifyRecoveryStore(store);
        setRecoveryRevision(value => value + 1);
      }
      setStatusAnnouncement("Task saved for review. It has not started running.");
      onTaskAccepted(receipt.task.task_id);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "The Task could not be created.");
    } finally { setPendingAction(null); }
  }

  async function togglePreference(item: SuggestionPreference) {
    if (item.workspace_id !== api.workspaceId) {
      setPreferenceError("These settings belong to a different Workspace. Reload before changing them.");
      return;
    }
    if (pendingPreference) return;
    const muted = !item.muted;
    const requestKey = preferenceMutationKey(item.kind, item.version, muted);
    let request = store.records.get(requestKey) as PreferenceMutation | undefined;
    if (!request) {
      if (!canAddRecoveryMutation(store)) {
        setPreferenceError("There are too many unresolved idea changes. Retry or discard one before starting another.");
        return;
      }
      request = { type: "preference", key: requestKey, workspaceId: api.workspaceId, kind: item.kind, muted, expectedVersion: item.version, requestId: crypto.randomUUID() };
      store.records.set(requestKey, request);
      notifyRecoveryStore(store);
      setRecoveryRevision(value => value + 1);
    }
    setPendingPreference(item.kind);
    setPreferenceError(null);
    try {
      const updated = await api.setPreference(request.kind, request.muted, request.expectedVersion, request.requestId);
      if (!validPreferenceReceipt(updated, request)) throw new Error("The Runtime response did not confirm this exact settings change. Retry the saved request or discard it after reviewing the latest state.");
      store.records.delete(requestKey);
      notifyRecoveryStore(store);
      setRecoveryRevision(value => value + 1);
      setPreferences(current => current?.map(value => value.kind === updated.kind ? updated : value) ?? [updated]);
      setStatusAnnouncement(`${PREFERENCE_KIND[updated.kind]} ${updated.muted ? "muted" : "allowed"}.`);
      setReload(value => value + 1);
    } catch (reason) {
      setPreferenceError(reason instanceof Error ? reason.message : "Suggestion settings could not be updated.");
    } finally { setPendingPreference(null); }
  }

  async function retryMutation(request: RecoverableMutation) {
    if (request.workspaceId !== api.workspaceId) {
      setError("This saved request belongs to a different Workspace and cannot be submitted here.");
      return;
    }
    if (request.type === "acceptance") {
      if (pendingAction) return;
      setPendingAction(request.suggestionId);
      setError(null);
      try {
        const receipt = await api.acceptTask(request.suggestionId, request.expectedVersion, request.requestId);
        if (!validAcceptanceReceipt(receipt, request)) throw new Error("The Runtime response did not confirm this exact Suggestion and READY Task. Retry the saved request or discard it after reviewing the latest state.");
        if (store.records.get(request.key) === request) {
          store.records.delete(request.key);
          notifyRecoveryStore(store);
          setRecoveryRevision(value => value + 1);
        }
        setStatusAnnouncement("Task saved for review. It has not started running.");
        onTaskAccepted(receipt.task.task_id);
      } catch (reason) {
        setError(reason instanceof Error ? reason.message : "The saved Task acceptance could not be confirmed.");
      } finally { setPendingAction(null); }
      return;
    }
    if (request.type === "suggestion") {
      if (pendingAction) return;
      setPendingAction(request.suggestionId);
      setError(null);
      try {
        const receipt = request.operation === "dismiss"
          ? await api.dismiss(request.suggestionId, request.expectedVersion, request.requestId)
          : request.operation === "unsnooze"
            ? await api.unsnooze(request.suggestionId, request.expectedVersion, request.requestId)
            : await api.snooze(request.suggestionId, request.expectedVersion, request.requestId, request.snoozedUntil!);
        if (!validSuggestionReceipt(receipt, request)) throw new Error("The Runtime response did not confirm this exact idea change. Retry the saved request or discard it after reviewing the latest state.");
        store.records.delete(request.key);
        notifyRecoveryStore(store);
        setRecoveryRevision(value => value + 1);
        setStatusAnnouncement("Idea change confirmed.");
        setReload(value => value + 1);
      } catch (reason) {
        setError(reason instanceof Error ? reason.message : "The saved idea change could not be confirmed.");
      } finally { setPendingAction(null); }
      return;
    }
    if (pendingPreference) return;
    setPendingPreference(request.kind);
    setPreferenceError(null);
    try {
      const updated = await api.setPreference(request.kind, request.muted, request.expectedVersion, request.requestId);
      if (!validPreferenceReceipt(updated, request)) throw new Error("The Runtime response did not confirm this exact settings change. Retry the saved request or discard it after reviewing the latest state.");
      store.records.delete(request.key);
      notifyRecoveryStore(store);
      setRecoveryRevision(value => value + 1);
      setPreferences(values => values?.map(value => value.kind === updated.kind ? updated : value) ?? [updated]);
      setStatusAnnouncement(`${PREFERENCE_KIND[updated.kind]} ${updated.muted ? "muted" : "allowed"}.`);
      setReload(value => value + 1);
    } catch (reason) {
      setPreferenceError(reason instanceof Error ? reason.message : "The saved settings change could not be confirmed.");
    } finally { setPendingPreference(null); }
  }

  function discardMutation(request: RecoverableMutation) {
    if (request.workspaceId !== api.workspaceId) return;
    const warning = request.type === "acceptance"
      ? "Discard this saved Task-creation retry? The Task may already have been created. Refresh and review the Suggestions and Work pages before trying again."
      : request.type === "suggestion"
      ? "Discard this saved retry? The idea may already have changed. Refresh and review its current state before trying a different action."
      : "Discard this saved settings retry? The preference may already have changed. Reload settings before making a new change.";
    if (!window.confirm(warning)) return;
    store.records.delete(request.key);
    notifyRecoveryStore(store);
    setRecoveryRevision(value => value + 1);
    setReload(value => value + 1);
  }

  const unresolvedMutations = [...store.records.values()].filter(request => request.workspaceId === api.workspaceId);

  return (
    <div className="page-content suggestions-page">
      <p className="sr-only" role="status" aria-live="polite" aria-atomic="true">{statusAnnouncement}</p>
      <header className="suggestions-heading">
        <div><p className="suggestions-kicker">IDEAS</p><h1>Ideas</h1>
          <p>Ideas are proposals only. Viewing them never starts work or grants access.</p>
        </div>
        <button className="suggestions-refresh" type="button" disabled={loading} onClick={() => setReload(value => value + 1)}>Refresh</button>
      </header>
      <div className="suggestions-tabs" role="group" aria-label="Suggestion visibility">
        {(["VISIBLE", "SNOOZED", "ALL"] as const).map(value => (
          <button key={value} type="button" aria-pressed={visibility === value} className={visibility === value ? "selected" : ""}
            onClick={() => setVisibility(value)}>
            {value === "VISIBLE" ? "For you" : value === "SNOOZED" ? "Snoozed" : "All current"}
          </button>
        ))}
      </div>
      {unresolvedMutations.length > 0 && <section className="suggestions-error" aria-live="polite" aria-label="Unresolved idea changes" data-revision={recoveryRevision}>
        <strong>A previous idea change has no confirmed receipt.</strong>
        <p>Retry sends the exact saved Workspace-scoped request and RequestId. Discarding removes only this local retry record.</p>
        <ul>{unresolvedMutations.map(request => <li key={request.key}>
          <span>{request.type === "suggestion"
            ? `${request.operation} idea ${request.suggestionId}`
            : request.type === "preference"
              ? `${request.muted ? "Mute" : "Allow"} ${PREFERENCE_KIND[request.kind]}`
              : `Create Task from idea ${request.suggestionId}`}</span>
          <button type="button" disabled={pendingAction !== null || pendingPreference !== null} onClick={() => void retryMutation(request)}>Retry exact request</button>
          <button type="button" disabled={pendingAction !== null || pendingPreference !== null} onClick={() => discardMutation(request)}>Discard retry</button>
        </li>)}</ul>
      </section>}
      <details className="suggestion-preferences" aria-busy={pendingPreference !== null}>
        <summary>Suggestion settings</summary>
        <p>Muting dismisses current proposed ideas of that kind. Unmuting affects future ideas only.</p>
        {preferenceError && <div className="suggestions-error" role="alert"><span>{preferenceError}</span><button type="button" onClick={() => setReload(value => value + 1)}>Reload settings</button></div>}
        {!preferences && !preferenceError && <p aria-live="polite">Loading settings…</p>}
        {preferences && <div className="suggestion-preference-list">{preferences.map(item => <div className="suggestion-preference-row" key={item.kind}>
          <div><strong>{PREFERENCE_KIND[item.kind]}</strong><small>{item.muted ? "Muted" : "Allowed"}</small></div>
          <button type="button" aria-pressed={item.muted} disabled={pendingPreference !== null}
            onClick={() => void togglePreference(item)}>
            {pendingPreference === item.kind ? "Saving…" : item.muted ? "Allow" : "Mute"}
          </button>
        </div>)}</div>}
      </details>
      {error && <div className="suggestions-error" role="alert"><span>{error}</span><button type="button" onClick={() => setReload(value => value + 1)}>Try again</button></div>}
      {loading ? <section className="suggestions-state" role="status" aria-busy="true"><span className="suggestions-spinner" aria-hidden="true" />Loading suggestions…</section>
        : items.length === 0 && !error ? <section className="suggestions-state"><span className="suggestions-mark" aria-hidden="true">✦</span>
          <h2>{visibility === "SNOOZED" ? "Nothing snoozed" : "No suggestions yet"}</h2>
          <p>{visibility === "SNOOZED" ? "Snoozed ideas will appear here." : "Suggestions will appear here when an approved producer is available. Nothing is being generated in the background yet."}</p>
        </section>
        : <div className="suggestions-list" aria-busy={pendingAction !== null}>{items.map(item => <SuggestionCard key={item.suggestion_id} item={item}
          busy={pendingAction === item.suggestion_id || pendingAction !== null}
          onAcceptTask={() => void acceptTask(item)}
          onOpenTask={() => { if (item.result_task_id) onTaskAccepted(item.result_task_id); }}
          onDismiss={() => void ownerAction(item, "dismiss")}
          onSnooze={() => void ownerAction(item, "snooze")}
          onUnsnooze={() => void ownerAction(item, "unsnooze")} />)}</div>}
      {next && !loading && <button className="suggestions-more" type="button" disabled={loadingMore} onClick={() => void loadMore()}>{loadingMore ? "Loading…" : "Load more"}</button>}
    </div>
  );
}

function SuggestionCard({ item, busy, onAcceptTask, onOpenTask, onDismiss, onSnooze, onUnsnooze }: {
  item: Suggestion; busy: boolean; onAcceptTask: () => void; onOpenTask: () => void; onDismiss: () => void; onSnooze: () => void; onUnsnooze: () => void;
}) {
  const objective = typeof item.proposed_task_spec?.objective === "string" ? item.proposed_task_spec.objective : null;
  return <article className="suggestion-card">
    <div className="suggestion-card-top"><span className="suggestion-kind">{KIND[item.kind]}</span>
      {item.snoozed_until && <span className="suggestion-snoozed">Snoozed until {formatTime(item.snoozed_until)}</span>}
      {item.status !== "PROPOSED" && <span className="suggestion-snoozed">{item.status.toLowerCase()}</span>}
    </div>
    <h2>{item.reason || "A new idea"}</h2>
    <p className="suggestion-action">{ACTION[item.proposed_action]}.</p>
    {objective && <p className="suggestion-task-objective">Proposed Task: {objective}</p>}
    {item.status === "ACCEPTED" && item.result_task_id
      ? <p className="suggestion-safety">A Task was saved. Acceptance did not start work.</p>
      : item.proposed_action === "TASK"
        ? <p className="suggestion-safety">Creating this saves a normal Task for review. It will not start work.</p>
      : <p className="suggestion-safety">No Task has been created. Viewing this idea does not start work.</p>}
    {(item.source_refs.length > 0 || item.goal_refs.length > 0) && <details className="suggestion-provenance">
      <summary>Why this idea</summary>
      {item.source_refs.length > 0 && <div><strong>Source revisions</strong><ul>{item.source_refs.map(ref => <li key={`${ref.resource_id}:${ref.revision_id}`}>{ref.resource_id} · {ref.revision_id}</li>)}</ul></div>}
      {item.goal_refs.length > 0 && <div><strong>Related Goals</strong><ul>{item.goal_refs.map(ref => <li key={`${ref.goal_id}:${ref.revision}`}>{ref.goal_id} · revision {ref.revision}</li>)}</ul></div>}
    </details>}
    {item.status === "PROPOSED" && <div className="suggestion-actions">
      {item.proposed_action === "TASK" && item.proposed_task_spec &&
        <button type="button" disabled={busy} onClick={onAcceptTask}>{busy ? "Saving Task…" : "Create Task"}</button>}
      {item.snoozed_until
        ? <button type="button" disabled={busy} onClick={onUnsnooze}>Show now</button>
        : <button type="button" disabled={busy} onClick={onSnooze}>{Date.parse(item.expires_at) - Date.now() < 24 * 60 * 60 * 1000 ? "Hide until expiry" : "Snooze for a day"}</button>}
      <button type="button" className="suggestion-dismiss" disabled={busy} onClick={onDismiss}>Dismiss</button>
    </div>}
    {item.status === "ACCEPTED" && item.proposed_action === "TASK" && item.result_task_id &&
      <div className="suggestion-actions"><button type="button" onClick={onOpenTask}>Open Task</button></div>}
    <footer>Expires {formatTime(item.expires_at)}</footer>
  </article>;
}
