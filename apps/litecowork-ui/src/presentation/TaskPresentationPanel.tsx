import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { PresentationRuntime } from "./PresentationRuntime";
import { parsePresentationItem, type PresentationItem } from "./presentation-types";
import "./task-presentation.css";

type Snapshot = {
  workspace_id: string;
  task_id: string;
  computed_at: string;
  freshness: "CURRENT" | "STALE" | "UNKNOWN";
  items: unknown[];
};

function validateSnapshot(value: unknown, workspaceId: string, taskId: string): Snapshot {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Task presentation is invalid.");
  const row = value as Record<string, unknown>;
  if (row.workspace_id !== workspaceId || row.task_id !== taskId || !Array.isArray(row.items)
    || typeof row.computed_at !== "string"
    || !["CURRENT", "STALE", "UNKNOWN"].includes(String(row.freshness))) {
    throw new Error("Task presentation does not match the selected Task.");
  }
  return row as unknown as Snapshot;
}

function taskStatusLabel(status: string | undefined): string {
  const labels: Record<string, string> = {
    READY: "Ready",
    RUNNING: "Working",
    WAITING_USER: "Waiting for you",
    BLOCKED: "Blocked",
    VERIFYING: "Checking the result",
    NEEDS_USER: "Needs your decision",
    INCOMPLETE: "Not finished",
    PAUSE_REQUESTED: "Pausing safely",
    PAUSED: "Paused",
    COMPLETED: "Completed",
    FAILED: "Failed",
    CANCEL_REQUESTED: "Stopping",
    CANCELLED: "Cancelled",
  };
  return status ? labels[status] ?? "Status unavailable" : "Status unavailable";
}

function activityStatusLabel(status: PresentationItem["status"]): string | null {
  if (!status || status === "UNKNOWN") return null;
  const labels: Record<Exclude<PresentationItem["status"], undefined | "UNKNOWN">, string> = {
    IN_PROGRESS: "In progress",
    WAITING: "Waiting",
    NEEDS_USER: "Needs you",
    COMPLETE: "Completed",
    INCOMPLETE: "Not finished",
    FAILED: "Failed",
    UNAVAILABLE: "Unavailable",
  };
  return labels[status];
}

function savedTime(value: string | undefined): string | null {
  if (!value) return null;
  const timestamp = Date.parse(value);
  return Number.isFinite(timestamp) ? new Date(timestamp).toLocaleString() : null;
}

function TaskSnapshotView({ items }: { items: readonly PresentationItem[] }) {
  const task = items.find((item) => item.kind === "TASK_CARD");
  const activity = items.filter((item): item is Extract<PresentationItem, { kind: "ACTIVITY" }> => item.kind === "ACTIVITY");
  const artifacts = items.filter((item): item is Extract<PresentationItem, { kind: "ARTIFACT" }> => item.kind === "ARTIFACT");
  const recentActivity = [...activity].sort((a, b) => {
    const timeA = a.occurred_at ? Date.parse(a.occurred_at) : Number.NaN;
    const timeB = b.occurred_at ? Date.parse(b.occurred_at) : Number.NaN;
    if (Number.isFinite(timeA) && Number.isFinite(timeB) && timeA !== timeB) return timeB - timeA;
    if (Number.isFinite(timeA) !== Number.isFinite(timeB)) return Number.isFinite(timeB) ? 1 : -1;
    return b.order_key.localeCompare(a.order_key) || b.item_key.localeCompare(a.item_key);
  });
  const hasSnapshotItems = task || activity.length > 0 || artifacts.length > 0;

  return <div className="task-snapshot-view">
    {task?.kind === "TASK_CARD" && <section className="task-outcome" aria-label="Task summary">
      <div className="task-outcome-copy">
        <span className="task-section-eyebrow">Requested outcome</span>
        <h4>{task.payload.objective}</h4>
      </div>
      <span className="task-outcome-status">{taskStatusLabel(task.payload.status)}</span>
    </section>}

    {artifacts.length > 0 && <section className="task-results" aria-label="Saved outputs">
      <h4>Saved outputs</h4>
      <ul>
        {artifacts.map((item) => <li key={item.item_key}>
          <span className="task-result-mark" aria-hidden="true">↳</span>
          <span className="task-result-copy"><strong>{item.payload.display_name}</strong><small>{item.payload.artifact_kind} · version {item.payload.version}{item.payload.verification_status ? ` · ${item.payload.verification_status}` : ""}</small></span>
        </li>)}
      </ul>
    </section>}

    {activity.length > 0 && <section className="task-activity" aria-label="Task activity">
      <div className="task-activity-heading"><h4>Activity</h4><span>{activity.length} saved {activity.length === 1 ? "item" : "items"}</span></div>
      <ol className="task-activity-preview">
        {recentActivity.slice(0, 3).map((item) => <li key={item.item_key}>
          <span className="task-activity-marker" aria-hidden="true" />
          <span>{item.label ?? item.payload.summary}</span>
        </li>)}
      </ol>
      {activity.length > 3 && <details className="task-activity-details">
        <summary>View all activity</summary>
        <ol>
          {recentActivity.map((item) => <li key={item.item_key}>
            <span>{item.label ?? item.payload.summary}</span>
            {activityStatusLabel(item.status) && <small>{activityStatusLabel(item.status)}</small>}
            {savedTime(item.occurred_at) && <time dateTime={item.occurred_at}>{savedTime(item.occurred_at)}</time>}
          </li>)}
        </ol>
      </details>}
    </section>}

    {!hasSnapshotItems && <p className="task-presentation-empty">No saved outcome, activity, or outputs are available.</p>}

    {(task || activity.length > 0 || artifacts.length > 0) && <details className="task-technical-details">
      <summary>Work details and sources</summary>
      <div>
        {task && <SourceList item={task} />}
        {activity.map((item) => <div className="task-source-row" key={item.item_key}>
          <span>{item.label ?? item.payload.summary}{activityStatusLabel(item.status) ? ` · ${activityStatusLabel(item.status)}` : ""}</span>
          {savedTime(item.occurred_at) && <time dateTime={item.occurred_at}>{savedTime(item.occurred_at)}</time>}
          <SourceList item={item} />
        </div>)}
        {artifacts.map((item) => <div className="task-source-row" key={item.item_key}>
          <span>{item.payload.display_name} · version {item.payload.version}</span>
          <SourceList item={item} />
        </div>)}
      </div>
    </details>}

    {items.some((item) => item.kind !== "TASK_CARD" && item.kind !== "ACTIVITY" && item.kind !== "ARTIFACT")
      && <details className="task-technical-details">
        <summary>Other saved presentation items</summary>
        <PresentationRuntime items={items.filter((item) => item.kind !== "TASK_CARD" && item.kind !== "ACTIVITY" && item.kind !== "ARTIFACT")} ariaLabel="Other saved Task presentation items" />
      </details>}
  </div>;
}

function SourceList({ item }: { item: PresentationItem }) {
  if (!item.source_refs.length) return null;
  return <ul className="task-source-list">{item.source_refs.map((source, index) => <li key={`${source.kind}:${source.id}:${index}`}>
    {source.kind.replaceAll("_", " ")} · {source.id}{source.revision ? ` · revision ${source.revision}` : ""}
  </li>)}</ul>;
}

/** Read-only item projection. It never starts or advances a Task. */
export function TaskPresentationPanel({ workspaceId, taskId, taskVersion, operatorReady }: {
  workspaceId: string;
  taskId: string;
  taskVersion: number;
  operatorReady: boolean;
}) {
  const [items, setItems] = useState<readonly PresentationItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);
  const [savedSnapshotNotice, setSavedSnapshotNotice] = useState<string | null>(null);
  const [snapshotAt, setSnapshotAt] = useState<string | null>(null);
  const [freshness, setFreshness] = useState<Snapshot["freshness"]>("UNKNOWN");
  const [invalidItemCount, setInvalidItemCount] = useState(0);
  const [savedSnapshotIdentity, setSavedSnapshotIdentity] = useState<string | null>(null);
  const panelRef = useRef<HTMLElement | null>(null);
  const requestInFlight = useRef(false);
  const pendingRefresh = useRef<(() => void) | null>(null);
  const refreshNow = useRef<(() => void) | null>(null);
  const hasSnapshot = useRef(false);
  const snapshotIdentity = useRef<string | null>(null);

  useEffect(() => {
    let active = true;
    let panelIntersecting = typeof IntersectionObserver === "undefined";
    let documentVisible = document.visibilityState === "visible";
    let pollTimer: number | undefined;
    const identity = `${workspaceId}\u0000${taskId}`;
    if (snapshotIdentity.current !== identity) {
      snapshotIdentity.current = identity;
      setItems([]);
      setSnapshotAt(null);
      setFreshness("UNKNOWN");
      setInvalidItemCount(0);
      setSavedSnapshotIdentity(null);
      hasSnapshot.current = false;
    }
    setFailure(null);
    setSavedSnapshotNotice(null);
    setLoading(false);
    if (!workspaceId || !taskId || !operatorReady) {
      setLoading(false);
      if (workspaceId && taskId && hasSnapshot.current) {
        setSavedSnapshotNotice("The local Runtime is offline. Showing the last saved snapshot, which may be out of date.");
      } else if (workspaceId && taskId) {
        setFailure("Task presentation is unavailable while the local Runtime is offline.");
      }
      return () => { active = false; };
    }
    const mayRefresh = () => active && documentVisible && panelIntersecting;
    const loadSnapshot = () => {
      if (!mayRefresh()) return;
      // `invoke` has no cancellation handle. Serialize reads across Task changes and
      // queue one refresh for the newest panel identity instead of overlapping calls.
      if (requestInFlight.current) {
        pendingRefresh.current = loadSnapshot;
        return;
      }
      requestInFlight.current = true;
      if (!hasSnapshot.current) setLoading(true);
      void invoke<unknown>("get_task_presentation", { workspaceId, taskId }).then(raw => {
        const snapshot = validateSnapshot(raw, workspaceId, taskId);
        if (!mayRefresh()) return;
        // Validate each projected item at the IPC boundary. Invalid records stay out of
        // renderers while the owning Task remains viewable.
        const parsed = snapshot.items.map(parsePresentationItem);
        const valid = parsed.filter((item): item is PresentationItem => item !== null);
        setItems(valid);
        setInvalidItemCount(parsed.length - valid.length);
        setSnapshotAt(snapshot.computed_at);
        setFreshness(snapshot.freshness);
        setFailure(null);
        setSavedSnapshotNotice(null);
        setSavedSnapshotIdentity(identity);
        hasSnapshot.current = true;
      }).catch(error => {
        if (!mayRefresh()) return;
        if (hasSnapshot.current) {
          setFailure(null);
          setSavedSnapshotNotice(operatorReady
            ? "The latest snapshot could not be refreshed. Showing saved data, which may be out of date."
            : "The local Runtime is offline. Showing the last saved snapshot, which may be out of date.");
        } else {
          setFailure(typeof error === "string" ? error : error instanceof Error ? error.message : "Task presentation could not be loaded.");
        }
      }).finally(() => {
        requestInFlight.current = false;
        if (active) setLoading(false);
        const queued = pendingRefresh.current;
        pendingRefresh.current = null;
        if (queued) queueMicrotask(queued);
      });
    };

    const onVisibilityChange = () => {
      documentVisible = document.visibilityState === "visible";
      if (mayRefresh()) loadSnapshot();
    };
    document.addEventListener("visibilitychange", onVisibilityChange);
    let observer: IntersectionObserver | undefined;
    if (typeof IntersectionObserver !== "undefined" && panelRef.current) {
      observer = new IntersectionObserver(entries => {
        panelIntersecting = entries.some(entry => entry.isIntersecting);
        if (mayRefresh()) loadSnapshot();
      });
      observer.observe(panelRef.current);
    }
    // The endpoint is a finite authenticated snapshot, not a live event stream. Poll
    // only while this panel is on screen and the desktop document is foregrounded.
    pollTimer = window.setInterval(loadSnapshot, 5_000);
    refreshNow.current = loadSnapshot;
    loadSnapshot();
    return () => {
      active = false;
      document.removeEventListener("visibilitychange", onVisibilityChange);
      observer?.disconnect();
      if (pollTimer !== undefined) window.clearInterval(pollTimer);
      if (refreshNow.current === loadSnapshot) refreshNow.current = null;
      if (pendingRefresh.current === loadSnapshot) pendingRefresh.current = null;
    };
  }, [workspaceId, taskId, taskVersion, operatorReady]);

  return <section ref={panelRef} className="task-presentation-panel" aria-label="Task outcome and activity">
    <div className="task-presentation-heading"><div><h3>Outcome and activity</h3><p>Read-only details derived from saved Task records.</p></div>
      <div className="task-presentation-actions">
        {snapshotAt && <small>{freshness === "UNKNOWN"
          ? `Source freshness unknown · snapshot ${new Date(snapshotAt).toLocaleString()}`
          : freshness === "STALE"
            ? `Saved snapshot may be stale · ${new Date(snapshotAt).toLocaleString()}`
            : `${savedSnapshotNotice ? "Last saved snapshot" : "Saved snapshot"} · ${new Date(snapshotAt).toLocaleString()}`}</small>}
        <button type="button" className="quiet-button" disabled={loading || !operatorReady || !workspaceId || !taskId} onClick={() => refreshNow.current?.()} aria-label="Refresh saved Task activity">
          {loading ? "Refreshing…" : "Refresh"}
        </button>
      </div>
    </div>
    {loading && <p role="status">Loading saved Task activity…</p>}
    {failure && <p role="status" className="task-presentation-note">{failure}</p>}
    {savedSnapshotNotice && <p role="status" className="task-presentation-note">{savedSnapshotNotice}</p>}
    {invalidItemCount > 0 && <p role="status" className="task-presentation-note">{invalidItemCount} unsupported or invalid presentation item{invalidItemCount === 1 ? " was" : "s were"} hidden. The saved Task remains available.</p>}
    {!loading && !failure && savedSnapshotIdentity === `${workspaceId}\u0000${taskId}` && <TaskSnapshotView items={items} />}
  </section>;
}
