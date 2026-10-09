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

type TaskProgress = {
  task_id: string;
  computed_at: string;
  last_activity_at: string | null;
  last_activity_source: "TASK_EVENT" | "STEP_EVENT" | "ATTEMPT_EVENT" | null;
  last_evidence_at: string | null;
  activity_summary: string | null;
  active_workstreams: Array<{
    step_id: string;
    title: string;
    step_status: string;
    active_attempt_ids: string[];
    worker_labels: string[];
    last_activity_at: string | null;
  }>;
  blockers: Array<{ code: string; safe_message: string; resolution_hint: string }>;
  newest_artifact: { workspace_id: string; artifact_id: string; version: number } | null;
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

function validateProgress(value: unknown, workspaceId: string, taskId: string): TaskProgress {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Task progress is invalid.");
  const row = value as Record<string, unknown>;
  const nullableText = (field: unknown) => field === null || typeof field === "string";
  if (row.task_id !== taskId || typeof row.computed_at !== "string"
    || !nullableText(row.last_activity_at) || !nullableText(row.last_activity_source)
    || !nullableText(row.last_evidence_at) || !nullableText(row.activity_summary)
    || !Array.isArray(row.active_workstreams) || row.active_workstreams.length > 100
    || !Array.isArray(row.blockers) || row.blockers.length > 100
    || !(row.newest_artifact === null || (row.newest_artifact && typeof row.newest_artifact === "object"))) {
    throw new Error("Task progress does not match the selected Task.");
  }
  const sources = ["TASK_EVENT", "STEP_EVENT", "ATTEMPT_EVENT"];
  if (row.last_activity_source !== null && !sources.includes(String(row.last_activity_source))) {
    throw new Error("Task progress contains an unsupported activity source.");
  }
  for (const workstream of row.active_workstreams) {
    if (!workstream || typeof workstream !== "object" || Array.isArray(workstream)) throw new Error("Task workstream is invalid.");
    const item = workstream as Record<string, unknown>;
    if (typeof item.step_id !== "string" || typeof item.title !== "string"
      || typeof item.step_status !== "string" || !Array.isArray(item.active_attempt_ids)
      || !item.active_attempt_ids.every(id => typeof id === "string")
      || !Array.isArray(item.worker_labels) || !item.worker_labels.every(label => typeof label === "string")
      || !nullableText(item.last_activity_at)) throw new Error("Task workstream is invalid.");
  }
  for (const blocker of row.blockers) {
    if (!blocker || typeof blocker !== "object" || Array.isArray(blocker)) throw new Error("Task blocker is invalid.");
    const item = blocker as Record<string, unknown>;
    if (typeof item.code !== "string" || typeof item.safe_message !== "string" || typeof item.resolution_hint !== "string") {
      throw new Error("Task blocker is invalid.");
    }
  }
  const artifact = row.newest_artifact;
  if (artifact !== null) {
    const item = artifact as Record<string, unknown>;
    if (item.workspace_id !== workspaceId || typeof item.artifact_id !== "string"
      || !Number.isSafeInteger(item.version) || (item.version as number) < 1) {
      throw new Error("Task progress Artifact is invalid.");
    }
  }
  return row as unknown as TaskProgress;
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

function eventSourceLabel(source: TaskProgress["last_activity_source"]): string {
  if (source === "TASK_EVENT") return "Task update";
  if (source === "STEP_EVENT") return "Step update";
  if (source === "ATTEMPT_EVENT") return "Work attempt update";
  return "Saved activity";
}

function workStatusLabel(status: string): string {
  const labels: Record<string, string> = {
    PENDING: "Queued", READY: "Ready", BLOCKED: "Blocked",
    RUNNING: "Working", WAITING_USER: "Waiting for you",
    VERIFYING: "Checking the result", COMPLETED: "Completed",
    FAILED: "Failed", CANCEL_REQUESTED: "Stopping",
    CANCELLED: "Cancelled", SUPERSEDED: "Replaced",
  };
  return labels[status] ?? "Status unavailable";
}

function TaskProgressSummary({ progress }: { progress: TaskProgress }) {
  return <section className="task-progress-summary" aria-label="Latest saved progress">
    <div className="task-progress-summary-heading">
      <h4>Latest saved progress</h4>
      {progress.last_activity_at && <small>{eventSourceLabel(progress.last_activity_source)} · {savedTime(progress.last_activity_at)}</small>}
    </div>
    {progress.activity_summary && <p>{progress.activity_summary}</p>}
    {progress.active_workstreams.length > 0
      ? <ul>{progress.active_workstreams.map(workstream => <li key={workstream.step_id}>
        <strong>{workstream.title}</strong>
        <span>{workStatusLabel(workstream.step_status)}{workstream.worker_labels.length ? ` · ${workstream.worker_labels.join(", ")}` : ""}</span>
      </li>)}</ul>
      : <p className="task-progress-muted">No active persisted work attempts.</p>}
    <p className="task-progress-evidence">{progress.last_evidence_at
      ? `Latest saved evidence · ${savedTime(progress.last_evidence_at)}`
      : "No saved evidence is available yet."}</p>
    {progress.blockers.length > 0 && <ul className="task-progress-blockers">
      {progress.blockers.map((blocker, index) => <li key={`${blocker.code}:${index}`}>
        <strong>{blocker.safe_message}</strong><span>{blocker.resolution_hint}</span>
      </li>)}
    </ul>}
  </section>;
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
  // Keep transient UI state tied to the Task that produced it. The panel can be
  // reused while the user switches Tasks, before the next effect has reset state.
  const [failure, setFailure] = useState<{ identity: string; message: string } | null>(null);
  const [savedSnapshotNotice, setSavedSnapshotNotice] = useState<{ identity: string; message: string } | null>(null);
  const [snapshotAt, setSnapshotAt] = useState<string | null>(null);
  const [freshness, setFreshness] = useState<Snapshot["freshness"]>("UNKNOWN");
  const [progress, setProgress] = useState<TaskProgress | null>(null);
  const [progressFailure, setProgressFailure] = useState<string | null>(null);
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
      setProgress(null);
      setProgressFailure(null);
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
        setSavedSnapshotNotice({ identity, message: "The local Runtime is offline. Showing the last saved snapshot, which may be out of date." });
      } else if (workspaceId && taskId) {
        setFailure({ identity, message: "Task presentation is unavailable while the local Runtime is offline." });
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
      void Promise.all([
        invoke<unknown>("get_task_presentation", { workspaceId, taskId }),
        invoke<unknown>("get_task_progress", { workspaceId, taskId }).then(
          raw => ({ progress: validateProgress(raw, workspaceId, taskId), failure: null as string | null }),
          error => ({ progress: null, failure: typeof error === "string" ? error : error instanceof Error ? error.message : "Saved progress is unavailable." }),
        ),
      ]).then(([raw, progressResult]) => {
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
        setProgress(progressResult.progress);
        setProgressFailure(progressResult.failure);
        setFailure(null);
        setSavedSnapshotNotice(null);
        setSavedSnapshotIdentity(identity);
        hasSnapshot.current = true;
      }).catch(error => {
        if (!mayRefresh()) return;
        if (hasSnapshot.current) {
          setFailure(null);
          setSavedSnapshotNotice({ identity, message: operatorReady
            ? "The latest snapshot could not be refreshed. Showing saved data, which may be out of date."
            : "The local Runtime is offline. Showing the last saved snapshot, which may be out of date." });
        } else {
          setFailure({ identity, message: typeof error === "string" ? error : error instanceof Error ? error.message : "Task presentation could not be loaded." });
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

  const selectedIdentity = `${workspaceId}\u0000${taskId}`;
  const hasSelectedSnapshot = savedSnapshotIdentity === selectedIdentity;
  const selectedFailure = failure?.identity === selectedIdentity ? failure.message : null;
  const selectedSnapshotNotice = savedSnapshotNotice?.identity === selectedIdentity ? savedSnapshotNotice.message : null;

  return <section ref={panelRef} className="task-presentation-panel" aria-label="Task outcome and activity">
    <div className="task-presentation-heading"><div><h3>Outcome and activity</h3><p>Read-only details derived from saved Task records.</p></div>
      <div className="task-presentation-actions">
        {hasSelectedSnapshot && snapshotAt && <small>{freshness === "UNKNOWN"
          ? `Source freshness unknown · snapshot ${new Date(snapshotAt).toLocaleString()}`
          : freshness === "STALE"
            ? `Saved snapshot may be stale · ${new Date(snapshotAt).toLocaleString()}`
            : `${selectedSnapshotNotice ? "Last saved snapshot" : "Saved snapshot"} · ${new Date(snapshotAt).toLocaleString()}`}</small>}
        <button type="button" className="quiet-button" disabled={loading || !operatorReady || !workspaceId || !taskId} onClick={() => refreshNow.current?.()} aria-label="Refresh saved Task activity">
          {loading ? "Refreshing…" : "Refresh"}
        </button>
      </div>
    </div>
    {loading && <p role="status">Loading saved Task activity…</p>}
    {selectedFailure && <p role="status" className="task-presentation-note">{selectedFailure}</p>}
    {selectedSnapshotNotice && <p role="status" className="task-presentation-note">{selectedSnapshotNotice}</p>}
    {hasSelectedSnapshot && progressFailure && <p role="status" className="task-presentation-note">Latest saved progress is unavailable. The presentation snapshot remains available.</p>}
    {hasSelectedSnapshot && invalidItemCount > 0 && <p role="status" className="task-presentation-note">{invalidItemCount} unsupported or invalid presentation item{invalidItemCount === 1 ? " was" : "s were"} hidden. The saved Task remains available.</p>}
    {hasSelectedSnapshot && progress && <TaskProgressSummary progress={progress} />}
    {!loading && !selectedFailure && hasSelectedSnapshot && <TaskSnapshotView items={items} />}
  </section>;
}
