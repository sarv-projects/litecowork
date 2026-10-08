import { useEffect, useRef, useState } from "react";
import type { SuggestionApi, Suggestion, SuggestionKind, SuggestionPreference, SuggestionVisibility } from "./suggestion-api";
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

export function SuggestionsPage({ api, onTaskAccepted }: { api: SuggestionApi; onTaskAccepted: (taskId: string) => void }) {
  const [visibility, setVisibility] = useState<SuggestionVisibility>("VISIBLE");
  const [items, setItems] = useState<Suggestion[]>([]);
  const [next, setNext] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [reload, setReload] = useState(0);
  const [pendingAction, setPendingAction] = useState<string | null>(null);
  const [preferences, setPreferences] = useState<SuggestionPreference[] | null>(null);
  const [preferenceError, setPreferenceError] = useState<string | null>(null);
  const [pendingPreference, setPendingPreference] = useState<SuggestionKind | null>(null);
  const acceptanceRequestIds = useRef(new Map<string, string>());
  const ownerActionRequests = useRef(new Map<string, { requestId: string; snoozedUntil?: string }>());
  const preferenceRequestIds = useRef(new Map<string, string>());

  useEffect(() => {
    const controller = new AbortController();
    let current = true;
    setLoading(true); setError(null); setItems([]); setNext(null);
    void api.list(visibility, undefined, controller.signal).then(page => {
      if (current) { setItems(page.items); setNext(page.next_cursor); }
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
    setLoadingMore(true); setError(null);
    try {
      const page = await api.list(visibility, next);
      setItems(current => {
        const known = new Set(current.map(item => item.suggestion_id));
        return [...current, ...page.items.filter(item => !known.has(item.suggestion_id))];
      });
      setNext(page.next_cursor);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "More suggestions could not be loaded.");
    } finally { setLoadingMore(false); }
  }

  async function ownerAction(item: Suggestion, action: "dismiss" | "snooze" | "unsnooze") {
    if (pendingAction) return;
    setPendingAction(item.suggestion_id);
    setError(null);
    try {
      const requestKey = `${item.suggestion_id}:${item.version}:${action}`;
      let request = ownerActionRequests.current.get(requestKey);
      if (!request) {
        request = { requestId: crypto.randomUUID() };
        if (action === "snooze") {
          const expires = Date.parse(item.expires_at);
          const now = Date.now();
          const until = Math.min(now + 24 * 60 * 60 * 1000, expires);
          if (!Number.isFinite(expires) || until <= now) throw new Error("This idea has expired. Refresh to see its settled state.");
          request.snoozedUntil = new Date(until).toISOString();
        }
        ownerActionRequests.current.set(requestKey, request);
      }
      if (action === "dismiss") await api.dismiss(item.suggestion_id, item.version, request.requestId);
      else if (action === "unsnooze") await api.unsnooze(item.suggestion_id, item.version, request.requestId);
      else await api.snooze(item.suggestion_id, item.version, request.requestId, request.snoozedUntil!);
      ownerActionRequests.current.delete(requestKey);
      setReload(value => value + 1);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "The idea could not be updated.");
    } finally { setPendingAction(null); }
  }

  async function acceptTask(item: Suggestion) {
    if (pendingAction || item.proposed_action !== "TASK" || !item.proposed_task_spec) return;
    setPendingAction(item.suggestion_id);
    setError(null);
    const requestId = acceptanceRequestIds.current.get(item.suggestion_id) ?? crypto.randomUUID();
    acceptanceRequestIds.current.set(item.suggestion_id, requestId);
    try {
      const taskId = await api.acceptTask(item.suggestion_id, item.version, requestId);
      acceptanceRequestIds.current.delete(item.suggestion_id);
      onTaskAccepted(taskId);
    } catch (reason) {
      // Keep the same idempotency key so a retry can reconcile a committed response
      // that was lost in transit.
      setError(reason instanceof Error ? reason.message : "The Task could not be created.");
    } finally { setPendingAction(null); }
  }

  async function togglePreference(item: SuggestionPreference) {
    if (pendingPreference) return;
    const muted = !item.muted;
    const requestKey = `${item.kind}:${item.version}:${muted}`;
    const requestId = preferenceRequestIds.current.get(requestKey) ?? crypto.randomUUID();
    preferenceRequestIds.current.set(requestKey, requestId);
    setPendingPreference(item.kind);
    setPreferenceError(null);
    try {
      const updated = await api.setPreference(item.kind, muted, item.version, requestId);
      preferenceRequestIds.current.delete(requestKey);
      setPreferences(current => current?.map(value => value.kind === updated.kind ? updated : value) ?? [updated]);
      setReload(value => value + 1);
    } catch (reason) {
      setPreferenceError(reason instanceof Error ? reason.message : "Suggestion settings could not be updated.");
    } finally { setPendingPreference(null); }
  }

  return (
    <div className="page-content suggestions-page">
      <header className="suggestions-heading">
        <div><p className="suggestions-kicker">IDEAS</p><h1>Ideas</h1>
          <p>Ideas are proposals only. Viewing them never starts work or grants access.</p>
        </div>
        <button className="suggestions-refresh" type="button" disabled={loading} onClick={() => setReload(value => value + 1)}>Refresh</button>
      </header>
      <div className="suggestions-tabs" aria-label="Suggestion visibility">
        {(["VISIBLE", "SNOOZED", "ALL"] as const).map(value => (
          <button key={value} type="button" aria-pressed={visibility === value} className={visibility === value ? "selected" : ""}
            onClick={() => setVisibility(value)}>
            {value === "VISIBLE" ? "For you" : value === "SNOOZED" ? "Snoozed" : "All current"}
          </button>
        ))}
      </div>
      <details className="suggestion-preferences">
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
      {loading ? <section className="suggestions-state" aria-live="polite"><span className="suggestions-spinner" aria-hidden="true" />Loading suggestions…</section>
        : items.length === 0 && !error ? <section className="suggestions-state"><span className="suggestions-mark" aria-hidden="true">✦</span>
          <h2>{visibility === "SNOOZED" ? "Nothing snoozed" : "No suggestions yet"}</h2>
          <p>{visibility === "SNOOZED" ? "Snoozed ideas will appear here." : "Suggestions will appear here when an approved producer is available. Nothing is being generated in the background yet."}</p>
        </section>
        : <div className="suggestions-list" aria-live="polite">{items.map(item => <SuggestionCard key={item.suggestion_id} item={item}
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
