import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./task-spec-revision-history.css";

type TaskSpecRevision = {
  taskId: string;
  workspaceId: string;
  revision: number;
  parentRevisions: number[];
  objective: string;
  authoredBy: { principalId: string; kind: string };
  createdAt: string;
};

function formatRevisionTime(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? "Timestamp unavailable" : date.toLocaleString();
}

export function TaskSpecRevisionHistory({ workspaceId, taskId, currentRevision, operatorReady, taskBusy, taskEditing, onRefreshTask }: {
  workspaceId: string;
  taskId: string;
  currentRevision: number;
  operatorReady: boolean;
  taskBusy?: boolean;
  taskEditing?: boolean;
  onRefreshTask?: () => void;
}) {
  const [open, setOpen] = useState(false);
  const [history, setHistory] = useState<TaskSpecRevision[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const requestGeneration = useRef(0);

  useEffect(() => {
    requestGeneration.current += 1;
    setHistory(null);
    setLoading(false);
    setError(null);
  }, [workspaceId, taskId, currentRevision]);

  const loadHistory = async () => {
    if (!operatorReady || loading) return;
    const generation = ++requestGeneration.current;
    setLoading(true);
    setError(null);
    try {
      const revisions = await invoke<TaskSpecRevision[]>("list_task_spec_revisions", { workspaceId, taskId });
      if (requestGeneration.current !== generation) return;
      if (revisions.some((item) => item.taskId !== taskId || item.workspaceId !== workspaceId)
        || revisions.some((item, index) => item.revision !== index + 1
          || !item.objective.trim()
          || !item.createdAt.trim()
          || !item.authoredBy?.kind
          || !item.authoredBy?.principalId)) {
        throw new Error("The local Runtime returned inconsistent Task specification history.");
      }
      setHistory(revisions);
    } catch (cause) {
      if (requestGeneration.current !== generation) return;
      setError(typeof cause === "string" ? cause : cause instanceof Error ? cause.message : "Task specification history could not be loaded.");
    } finally {
      if (requestGeneration.current === generation) setLoading(false);
    }
  };

  const historyHead = history && history.length > 0 ? history[history.length - 1].revision : history ? 0 : null;
  const taskDetailIsStale = historyHead !== null && historyHead > currentRevision;
  const historyIsBehind = historyHead !== null && historyHead < currentRevision;

  useEffect(() => {
    if (open && history === null && !loading && error === null) {
      if (operatorReady) void loadHistory();
      else setError("Task specification history is unavailable while the local Runtime is offline.");
    }
  }, [open, history, loading, error, operatorReady]);

  return (
    <details className="task-spec-history" onToggle={(event) => setOpen(event.currentTarget.open)}>
      <summary>
        <span>Task specification history</span>
        <small>{history ? `${history.length} immutable ${history.length === 1 ? "revision" : "revisions"}` : `Current revision ${currentRevision}`}</small>
      </summary>
      <p className="task-spec-history-note">Read-only history of the exact Task objectives saved over time. Revisions cannot be restored or edited here.</p>
      {taskDetailIsStale && <p className="task-spec-history-status" role="status">
        History contains revision {historyHead}, newer than the Task detail’s loaded revision {currentRevision}. The Task detail may be out of date.
        {onRefreshTask && <button className="text-button" type="button" onClick={onRefreshTask} disabled={!operatorReady || taskBusy || taskEditing}>Reload Task</button>}
      </p>}
      {historyIsBehind && <p className="task-spec-history-status" role="status">
        History ends at revision {historyHead}, while the loaded Task detail is revision {currentRevision}. The history response may be stale or incomplete.
        {onRefreshTask && <button className="text-button" type="button" onClick={onRefreshTask} disabled={!operatorReady || taskBusy || taskEditing}>Reload Task</button>}
      </p>}
      {(taskDetailIsStale || historyIsBehind) && taskEditing && <p className="task-spec-history-status">Finish or cancel the objective draft before reloading the Task.</p>}
      {!operatorReady && history && <p className="task-spec-history-status" role="status">Showing previously loaded history. The local Runtime is offline, so this view may be out of date.</p>}
      {error && <p className="task-spec-history-error" role="alert">{error}</p>}
      {loading && <p className="task-spec-history-status" role="status">Loading revision history…</p>}
      {history && <ol className="task-spec-history-list">
        {[...history].reverse().map((revision) => (
          <li key={`${revision.taskId}:${revision.revision}`}>
            <div className="task-spec-history-heading">
              <strong>Revision {revision.revision}{revision.revision === currentRevision && !taskDetailIsStale && !historyIsBehind ? " · Current" : revision.revision === historyHead && taskDetailIsStale ? " · Latest saved" : ""}</strong>
              <time dateTime={revision.createdAt}>{formatRevisionTime(revision.createdAt)}</time>
            </div>
            <p>{revision.objective}</p>
            <small>Authored by {revision.authoredBy.kind} · {revision.authoredBy.principalId}</small>
            <small>Parent revisions: {revision.parentRevisions.length ? revision.parentRevisions.map((parent) => `#${parent}`).join(", ") : "None (initial revision)"}</small>
          </li>
        ))}
      </ol>}
      {operatorReady && (history || error) && <button className="quiet-button task-spec-history-refresh" type="button" onClick={() => void loadHistory()} disabled={loading}>
        {loading ? "Refreshing…" : history ? "Refresh history" : "Try again"}
      </button>}
    </details>
  );
}
