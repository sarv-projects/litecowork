import { useEffect, useRef, useState } from "react";
import type { Artifact, ArtifactApi, PinnedResourceRef } from "./artifact-api";
import { ArtifactWorkbench } from "./ArtifactWorkbench";

type Props = { api: ArtifactApi; workspaceId: string; onOpenSource?: (ref: PinnedResourceRef) => void };

/** Integration entry point: lists real committed Artifacts, then opens exact versions. */
export function ArtifactLibrary({ api, workspaceId, onOpenSource }: Props) {
  const [status, setStatus] = useState<Artifact["library_status"]>("SAVED");
  const [refresh, setRefresh] = useState(0);
  const [items, setItems] = useState<Artifact[]>([]);
  const [cursor, setCursor] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [failure, setFailure] = useState<string | null>(null);
  const controllerRef = useRef<AbortController | null>(null);

  useEffect(() => {
    const controller = new AbortController(); controllerRef.current = controller;
    setItems([]); setCursor(null); setSelectedId(null); setFailure(null); setLoading(true);
    void api.list({ libraryStatus: status }, controller.signal).then(page => {
      if (controller.signal.aborted) return;
      if (page.items.some(item => item.workspace_id !== workspaceId)) throw new Error("Artifact page belongs to another Workspace.");
      setItems(page.items); setCursor(page.next_cursor);
    }).catch(error => {
      if (!controller.signal.aborted) setFailure(error instanceof Error ? error.message : "Artifact Library is unavailable.");
    }).finally(() => { if (!controller.signal.aborted) setLoading(false); });
    return () => { controller.abort(); controllerRef.current?.abort(); };
  }, [api, workspaceId, status, refresh]);

  async function loadMore() {
    if (!cursor || loading) return;
    const controller = new AbortController(); controllerRef.current = controller;
    setLoading(true); setFailure(null);
    try {
      const page = await api.list({ libraryStatus: status, cursor }, controller.signal);
      if (controller.signal.aborted) return;
      if (page.items.some(item => item.workspace_id !== workspaceId)) throw new Error("Artifact page belongs to another Workspace.");
      setItems(current => {
        const ids = new Set(current.map(item => item.artifact_id));
        return [...current, ...page.items.filter(item => !ids.has(item.artifact_id))];
      });
      setCursor(page.next_cursor);
    } catch (error) {
      if (!controller.signal.aborted) { setItems([]); setSelectedId(null); setFailure(error instanceof Error ? error.message : "Artifact Library is unavailable."); }
    } finally { if (!controller.signal.aborted) setLoading(false); }
  }

  return <section aria-label="Artifact Library">
    <div className="artifact-workbench-actions">
      <label>Artifacts <select value={status} onChange={event => setStatus(event.target.value as Artifact["library_status"])}>
        <option value="SAVED">Saved</option><option value="TRANSIENT">Task outputs</option><option value="ARCHIVED">Archived</option>
      </select></label>
      <button type="button" onClick={() => setRefresh(value => value + 1)}>Refresh artifacts</button>
    </div>
    {loading && <p role="status">Loading committed Artifacts…</p>}
    {failure && <p role="alert">{failure}</p>}
    {!loading && !failure && items.length === 0 && <p>No {status === "SAVED" ? "saved" : status === "ARCHIVED" ? "archived" : "transient"} Artifacts in this Workspace.</p>}
    <ul>{items.map(item => <li key={item.artifact_id}><button type="button" onClick={() => setSelectedId(item.artifact_id)}>{item.display_name} · version {item.current_version}</button></li>)}</ul>
    {cursor && <button type="button" disabled={loading} onClick={() => void loadMore()}>Load older Artifacts</button>}
    {selectedId && <ArtifactWorkbench key={`${workspaceId}:${selectedId}`} api={api} workspaceId={workspaceId} artifactId={selectedId} onClose={() => setSelectedId(null)} onOpenSource={onOpenSource} />}
  </section>;
}
