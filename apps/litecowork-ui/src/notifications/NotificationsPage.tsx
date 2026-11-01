import { useEffect, useRef, useState } from "react";
import {
  type Notification,
  NotificationApiError,
  type NotificationApi,
  type NotificationPreference,
  type NotificationPolicy,
  type NotificationStatus,
} from "./notification-api";
import { isNotificationRequestCurrent } from "./notification-request-scope";
import "./notifications-page.css";

type Props = { api: NotificationApi; workspaceId: string };

type EventClass =
  | "AUTOMATION_OCCURRENCE_SETTLED"
  | "AUTOMATION_OCCURRENCE_SKIPPED"
  | "TASK_COMPLETED"
  | "TASK_FAILED"
  | "DEPENDENCY_BLOCKED";

const EVENT_CLASSES: readonly EventClass[] = [
  "AUTOMATION_OCCURRENCE_SETTLED",
  "AUTOMATION_OCCURRENCE_SKIPPED",
  "TASK_COMPLETED",
  "TASK_FAILED",
  "DEPENDENCY_BLOCKED",
];

const EVENT_LABEL: Record<EventClass, string> = {
  AUTOMATION_OCCURRENCE_SETTLED: "Automation ran",
  AUTOMATION_OCCURRENCE_SKIPPED: "Automation skipped a due slot",
  TASK_COMPLETED: "Task completed",
  TASK_FAILED: "Task failed",
  DEPENDENCY_BLOCKED: "Waiting on a dependency",
};

const EVENT_DETAIL: Record<EventClass, string> = {
  AUTOMATION_OCCURRENCE_SETTLED: "A triggered occurrence settled into an ordinary Task.",
  AUTOMATION_OCCURRENCE_SKIPPED: "A due slot was recorded as skipped without starting a Task.",
  TASK_COMPLETED: "The Task behind an occurrence reached a successful terminal state.",
  TASK_FAILED: "The Task behind an occurrence reached a failed terminal state.",
  DEPENDENCY_BLOCKED: "The occurrence or its Task waits on a named dependency.",
};

const STATUSES: readonly NotificationStatus[] = ["PENDING", "SENDING", "SENT", "FAILED", "AMBIGUOUS", "SUPPRESSED"];

const STATUS_LABEL: Record<NotificationStatus, string> = {
  PENDING: "Pending",
  SENDING: "Sending",
  SENT: "Transport acknowledged",
  FAILED: "Failed",
  AMBIGUOUS: "Unknown outcome",
  SUPPRESSED: "Suppressed",
};

const POLICY_LABEL: Record<NotificationPolicy, string> = {
  ALWAYS: "Always announce",
  ON_SUCCESS: "Only when it succeeds",
  ON_FAILURE: "Only when it fails",
  ON_CONDITION: "When a named condition holds",
  SILENT: "Never announce",
};

type DeliveryView = {
  status: "LOADING" | "READY" | "ERROR";
  items: Notification[];
  nextCursor: string | null;
  error: string | null;
};

type PreferencesView = {
  status: "LOADING" | "READY" | "ERROR";
  items: NotificationPreference[];
  error: string | null;
};

type PolicyDraft = { policy: NotificationPolicy | ""; channels: string };

function errorText(error: unknown): string {
  if (error instanceof NotificationApiError) {
    switch (error.code) {
      case "WORKSPACE_ARCHIVED":
        return "This Workspace is archived. Delivery history stays readable, but it rejects new notification policies.";
      case "CONFLICT":
      case "STALE_VERSION":
        return "This policy changed since it was loaded. Reload the policies before saving again.";
      case "INVALID_ARGUMENT":
        return "The Runtime rejected this policy. Check the channel values.";
      case "DEPENDENCY_UNAVAILABLE":
        return "Notification storage is unavailable. Try again when the local Runtime is ready.";
      default:
        return `Notification request failed (${error.code}).`;
    }
  }
  return error instanceof Error ? error.message : "Notification request failed.";
}

function displayTime(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString();
}

function quietHoursText(preference: NotificationPreference | null): string {
  const quiet = preference?.quiet_hours;
  if (!quiet) return "No quiet hours";
  const days = quiet.weekdays.map(day => day.slice(0, 3).toLowerCase()).join(", ");
  return `${quiet.start_local_time}–${quiet.end_local_time} on ${days} (${quiet.timezone})`;
}

function draftFor(preference: NotificationPreference | null): PolicyDraft {
  return preference
    ? { policy: preference.policy, channels: preference.preferred_channels.join(", ") }
    : { policy: "", channels: "" };
}

function channelsFor(draft: PolicyDraft): string[] {
  const seen = new Set<string>();
  return draft.channels
    .split(",")
    .map(channel => channel.trim())
    .filter(channel => {
      if (channel.length === 0 || seen.has(channel)) return false;
      seen.add(channel);
      return true;
    });
}

function freshRequestId(): string {
  const randomUUID = globalThis.crypto?.randomUUID;
  if (!randomUUID) throw new Error("This desktop session cannot create secure request identities. Restart LiteCowork before changing a notification policy.");
  return randomUUID.call(globalThis.crypto);
}

export function NotificationsPage({ api, workspaceId }: Props) {
  const activeWorkspace = useRef(workspaceId);
  const [reloadToken, setReloadToken] = useState(0);
  const [statusFilter, setStatusFilter] = useState<NotificationStatus | "ALL">("ALL");
  const [deliveries, setDeliveries] = useState<DeliveryView>({ status: "LOADING", items: [], nextCursor: null, error: null });
  const [preferences, setPreferences] = useState<PreferencesView>({ status: "LOADING", items: [], error: null });
  const [drafts, setDrafts] = useState<Record<string, PolicyDraft>>({});
  const [savingClass, setSavingClass] = useState<EventClass | null>(null);
  const [conflictClass, setConflictClass] = useState<EventClass | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const [loadingMore, setLoadingMore] = useState(false);

  useEffect(() => {
    activeWorkspace.current = workspaceId;
  }, [workspaceId]);

  useEffect(() => {
    const controller = new AbortController();
    setDeliveries({ status: "LOADING", items: [], nextCursor: null, error: null });
    void api.list(statusFilter === "ALL" ? null : statusFilter, null, controller.signal)
      .then(page => {
        if (controller.signal.aborted || !isNotificationRequestCurrent(workspaceId, activeWorkspace.current)) return;
        setDeliveries({ status: "READY", items: page.items, nextCursor: page.next_cursor, error: null });
      })
      .catch(error => {
        if (controller.signal.aborted || !isNotificationRequestCurrent(workspaceId, activeWorkspace.current)) return;
        setDeliveries({ status: "ERROR", items: [], nextCursor: null, error: errorText(error) });
      });
    return () => controller.abort();
  }, [api, workspaceId, statusFilter, reloadToken]);

  useEffect(() => {
    const controller = new AbortController();
    setPreferences({ status: "LOADING", items: [], error: null });
    void api.preferences(controller.signal)
      .then(items => {
        if (controller.signal.aborted || !isNotificationRequestCurrent(workspaceId, activeWorkspace.current)) return;
        setPreferences({ status: "READY", items, error: null });
        setDrafts(Object.fromEntries(items.map(item => [item.event_class, draftFor(item)])));
      })
      .catch(error => {
        if (controller.signal.aborted || !isNotificationRequestCurrent(workspaceId, activeWorkspace.current)) return;
        setPreferences({ status: "ERROR", items: [], error: errorText(error) });
      });
    return () => controller.abort();
  }, [api, workspaceId, reloadToken]);

  async function loadMore() {
    const cursor = deliveries.nextCursor;
    if (!cursor || loadingMore) return;
    const requestWorkspaceId = workspaceId;
    const filter = statusFilter === "ALL" ? null : statusFilter;
    setLoadingMore(true);
    setFailure(null);
    try {
      const page = await api.list(filter, cursor);
      if (!isNotificationRequestCurrent(requestWorkspaceId, activeWorkspace.current)) return;
      setDeliveries(current => {
        const known = new Set(current.items.map(item => item.delivery_id));
        return {
          ...current,
          items: [...current.items, ...page.items.filter(item => !known.has(item.delivery_id))],
          nextCursor: page.next_cursor,
        };
      });
    } catch (error) {
      if (isNotificationRequestCurrent(requestWorkspaceId, activeWorkspace.current)) setFailure(errorText(error));
    } finally {
      if (isNotificationRequestCurrent(requestWorkspaceId, activeWorkspace.current)) setLoadingMore(false);
    }
  }

  async function savePreference(eventClass: EventClass) {
    if (savingClass) return;
    const stored = preferences.items.find(item => item.event_class === eventClass) ?? null;
    const draft = drafts[eventClass] ?? draftFor(stored);
    if (!draft.policy) {
      setFailure("Choose a policy before saving.");
      return;
    }
    const channels = channelsFor(draft);
    if (channels.length > 64) {
      setFailure("A notification policy accepts at most 64 channels.");
      return;
    }
    const requestWorkspaceId = workspaceId;
    let requestId: string;
    try {
      requestId = freshRequestId();
    } catch (error) {
      setFailure(errorText(error));
      return;
    }
    setSavingClass(eventClass);
    setFailure(null);
    setMessage(null);
    setConflictClass(null);
    try {
      const saved = await api.setPreference(eventClass, stored?.version ?? 0, {
        policy: draft.policy,
        preferred_channels: channels,
        quiet_hours: stored?.quiet_hours ?? null,
      }, requestId);
      if (!isNotificationRequestCurrent(requestWorkspaceId, activeWorkspace.current)) return;
      setPreferences(current => {
        const others = current.items.filter(item => item.event_class !== saved.event_class);
        return { status: "READY", items: [...others, saved].sort((left, right) => left.event_class.localeCompare(right.event_class)), error: null };
      });
      setDrafts(current => ({ ...current, [saved.event_class]: draftFor(saved) }));
      setMessage(saved.policy === "SILENT"
        ? `${EVENT_LABEL[eventClass]} will not be announced.`
        : `${EVENT_LABEL[eventClass]} policy saved at version ${saved.version}.`);
    } catch (error) {
      if (!isNotificationRequestCurrent(requestWorkspaceId, activeWorkspace.current)) return;
      setFailure(errorText(error));
      if (error instanceof NotificationApiError && (error.code === "CONFLICT" || error.code === "STALE_VERSION")) {
        setConflictClass(eventClass);
      }
    } finally {
      if (isNotificationRequestCurrent(requestWorkspaceId, activeWorkspace.current)) setSavingClass(null);
    }
  }

  const preferencesReady = preferences.status === "READY";

  return (
    <div className="page-content subpage-content notifications-page">
      <div className="notifications-heading">
        <div>
          <div className="eyebrow">WORKSPACE SIGNALS</div>
          <h1>Notifications</h1>
          <p>Delivery history and the local policies that decide whether a settled Automation outcome is announced. Notifications never start work, and this view never sends one.</p>
        </div>
        <button
          className="notification-secondary-action"
          type="button"
          disabled={deliveries.status === "LOADING" || preferences.status === "LOADING"}
          onClick={() => { setMessage(null); setFailure(null); setConflictClass(null); setReloadToken(token => token + 1); }}
        >
          Refresh
        </button>
      </div>

      {message ? <p className="notification-message" role="status">{message}</p> : null}
      {failure ? <p className="notification-failure" role="alert">{failure}</p> : null}

      <section className="notifications-panel" aria-labelledby="notification-history-heading">
        <div className="notifications-panel-heading">
          <div>
            <h2 id="notification-history-heading">Delivery history</h2>
            <p>A delivery exists only because a persisted outcome exists. A pending record is not a delivered message.</p>
            <p>A transport-acknowledged delivery is not a Task outcome and is never proof that the work succeeded.</p>
          </div>
          <label className="notification-filter">
            <span>Status</span>
            <select
              value={statusFilter}
              onChange={event => setStatusFilter(event.target.value as NotificationStatus | "ALL")}
              disabled={deliveries.status === "LOADING"}
            >
              <option value="ALL">All statuses</option>
              {STATUSES.map(status => <option key={status} value={status}>{STATUS_LABEL[status]}</option>)}
            </select>
          </label>
        </div>

        {deliveries.status === "LOADING" ? (
          <p className="notification-state" role="status">Loading notification deliveries…</p>
        ) : deliveries.status === "ERROR" ? (
          <div className="notification-state notification-state-error" role="alert">
            <p>{deliveries.error}</p>
            <button className="notification-secondary-action" type="button" onClick={() => setReloadToken(token => token + 1)}>Try again</button>
          </div>
        ) : deliveries.items.length === 0 ? (
          <div className="notification-state">
            <p>No notification deliveries yet.</p>
            <small>A delivery is recorded after an Automation occurrence settles. Nothing is created for drafts, conversations, or agent messages.</small>
          </div>
        ) : (
          <>
            <table className="notifications-table">
              <thead>
                <tr>
                  <th scope="col">Status</th>
                  <th scope="col">Created</th>
                  <th scope="col">Attempts</th>
                  <th scope="col">Source event</th>
                  <th scope="col">Last error</th>
                </tr>
              </thead>
              <tbody>
                {deliveries.items.map(item => (
                  <tr key={item.delivery_id}>
                    <td><span className={`notification-status notification-status-${item.status.toLowerCase()}`}>{STATUS_LABEL[item.status]}</span></td>
                    <td><time dateTime={item.created_at}>{displayTime(item.created_at)}</time></td>
                    <td>{item.attempt_count}</td>
                    <td className="notification-source" title={item.source_event_id}>{item.source_event_id}</td>
                    <td>{item.last_error_code ?? "—"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
            {deliveries.nextCursor ? (
              <button className="notification-secondary-action notification-load-more" type="button" disabled={loadingMore} onClick={() => void loadMore()}>
                {loadingMore ? "Loading…" : "Load older deliveries"}
              </button>
            ) : null}
          </>
        )}
      </section>

      <section className="notifications-panel" aria-labelledby="notification-policies-heading">
        <div className="notifications-panel-heading">
          <div>
            <h2 id="notification-policies-heading">Policies</h2>
            <p>Each save is versioned. A class without a saved policy keeps using the Runtime default; nothing is assumed here.</p>
          </div>
        </div>

        {preferences.status === "LOADING" ? (
          <p className="notification-state" role="status">Loading notification policies…</p>
        ) : preferences.status === "ERROR" ? (
          <div className="notification-state notification-state-error" role="alert">
            <p>{preferences.error}</p>
            <button className="notification-secondary-action" type="button" onClick={() => setReloadToken(token => token + 1)}>Try again</button>
          </div>
        ) : (
          <ul className="notification-policies">
            {EVENT_CLASSES.map(eventClass => {
              const stored = preferences.items.find(item => item.event_class === eventClass) ?? null;
              const draft = drafts[eventClass] ?? draftFor(stored);
              const currentChannels = stored ? stored.preferred_channels.join(", ") : "";
              const dirty = draft.policy !== (stored?.policy ?? "") || draft.channels.trim() !== currentChannels;
              const saving = savingClass === eventClass;
              return (
                <li key={eventClass} className="notification-policy">
                  <div className="notification-policy-copy">
                    <h3>{EVENT_LABEL[eventClass]}</h3>
                    <p>{EVENT_DETAIL[eventClass]}</p>
                    <small>{stored ? `Version ${stored.version} · ${quietHoursText(stored)}` : "No saved policy · using the Runtime default"}</small>
                  </div>
                  <label className="notification-field">
                    <span>Policy</span>
                    <select
                      value={draft.policy}
                      disabled={!preferencesReady || savingClass !== null}
                      onChange={event => setDrafts(current => ({ ...current, [eventClass]: { ...draft, policy: event.target.value as NotificationPolicy | "" } }))}
                    >
                      <option value="">Choose policy</option>
                      {(["ALWAYS", "ON_SUCCESS", "ON_FAILURE", "ON_CONDITION", "SILENT"] as const).map(policy => (
                        <option key={policy} value={policy}>{POLICY_LABEL[policy]}</option>
                      ))}
                    </select>
                  </label>
                  <label className="notification-field">
                    <span>Channels</span>
                    <input
                      type="text"
                      value={draft.channels}
                      placeholder="Primary desktop, email"
                      disabled={!preferencesReady || savingClass !== null}
                      onChange={event => setDrafts(current => ({ ...current, [eventClass]: { ...draft, channels: event.target.value } }))}
                    />
                    <small>Comma-separated channel values. Quiet hours are preserved from the saved policy.</small>
                  </label>
                  <div className="notification-policy-actions">
                    {conflictClass === eventClass ? (
                      <button className="notification-secondary-action" type="button" onClick={() => { setConflictClass(null); setFailure(null); setReloadToken(token => token + 1); }}>Reload policies</button>
                    ) : null}
                    <button
                      className="notification-primary-action"
                      type="button"
                      disabled={!preferencesReady || savingClass !== null || draft.policy === "" || !dirty}
                      onClick={() => void savePreference(eventClass)}
                    >
                      {saving ? "Saving…" : "Save policy"}
                    </button>
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </section>
    </div>
  );
}
