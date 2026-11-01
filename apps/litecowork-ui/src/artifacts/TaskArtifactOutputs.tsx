import { useEffect, useState } from "react";
import type { Artifact, ArtifactApi } from "./artifact-api";
import { ArtifactWorkbench } from "./ArtifactWorkbench";
import "./artifact-workbench.css";

type Props = { api: ArtifactApi; workspaceId: string; taskId: string; operatorReady: boolean };

/** Lazy, read-only view of committed outputs owned by one exact Task. */
export function TaskArtifactOutputs({ api, workspaceId, taskId, operatorReady }: Props) {
  const [open, setOpen] = useState(false);
  const [items, setItems] = useState<Artifact[] | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);
  const [reload, setReload] = useState(0);

  useEffect(() => {
    if (!open) return;
    const controller = new AbortController();
    setSelectedId(null);
    setFailure(null);
    if (!operatorReady) {
      setItems(null);
      setLoading(false);
      setFailure("Task outputs are unavailable while the local Runtime is offline.");
      return () => controller.abort();
    }

    setItems(null);
    setLoading(true);
    void api.listForTask(taskId, workspaceId, controller.signal).then(outputs => {
      if (!controller.signal.aborted) setItems(outputs);
    }).catch(error => {
      if (!controller.signal.aborted) {
        setFailure(error instanceof Error ? error.message : "Committed Task outputs could not be loaded.");
      }
    }).finally(() => {
      if (!controller.signal.aborted) setLoading(false);
    });
    return () => controller.abort();
  }, [api, workspaceId, taskId, operatorReady, open, reload]);

  const countLabel = loading ? "Loading…"
    : failure ? "Unavailable"
      : items === null ? "Open to load"
        : items.length === 0 ? "None yet"
          : `${items.length} ${items.length === 1 ? "Artifact" : "Artifacts"}`;

  return <details className="task-work-details task-artifact-outputs" onToggle={event => setOpen(event.currentTarget.open)}>
    <summary><span>Outputs</span><small>{countLabel}</small></summary>
    <div className="task-artifact-outputs-content">
      {loading && <p className="task-loading" role="status">Loading committed outputs…</p>}
      {failure && <div className="task-stale-note" role="alert"><p>{failure}</p>{operatorReady && <button type="button" className="text-button" disabled={loading} onClick={() => setReload(value => value + 1)}>Try again</button>}</div>}
      {!loading && !failure && items?.length === 0 && <p className="task-detail-note">This Task has no committed Artifact outputs.</p>}
      {!loading && !failure && items && items.length > 0 && <ul className="task-artifact-output-list" aria-label="Committed Task Artifacts">
        {items.map(item => <li key={item.artifact_id}>
          <button className="task-artifact-output-row" type="button" aria-expanded={selectedId === item.artifact_id} onClick={() => setSelectedId(current => current === item.artifact_id ? null : item.artifact_id)}>
            <span className="task-artifact-output-icon" aria-hidden="true">▤</span>
            <span className="task-artifact-output-copy"><strong>{item.display_name}</strong><small>{item.kind} · version {item.current_version} · {item.library_status.toLowerCase()}</small></span>
            <span className="task-row-arrow" aria-hidden="true">›</span>
          </button>
        </li>)}
      </ul>}
      {selectedId && <ArtifactWorkbench key={`${workspaceId}:${taskId}:${selectedId}`} api={api} workspaceId={workspaceId} artifactId={selectedId} onClose={() => setSelectedId(null)} />}
    </div>
  </details>;
}
