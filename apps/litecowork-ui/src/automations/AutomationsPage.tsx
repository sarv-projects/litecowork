import { useEffect, useRef, useState } from "react";
import type { Automation, AutomationApi, AutomationStatus } from "./automation-api";
import type { AutomationRevision } from "./automation-api";
import { AutomationEditor } from "./AutomationEditor";
import "./automations-page.css";

type Props = { api: AutomationApi; workspaceId: string };
type PendingRequest = { signature: string; id: string };
const statusLabel: Record<AutomationStatus, string> = { ENABLED: "Enabled", PAUSED: "Paused", DISABLED: "Disabled" };

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : "The Automation request could not be completed.";
}
function dateText(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? "Date unavailable" : date.toLocaleString();
}
function statusClass(status: AutomationStatus): string { return `automation-status automation-status-${status.toLowerCase()}`; }

/** This page manages stored definitions and safe-stop state only; it never runs triggers. */
export function AutomationsPage({ api, workspaceId }: Props) {
  const [items, setItems] = useState<Automation[]>([]);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [selected, setSelected] = useState<Automation | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [detailLoading, setDetailLoading] = useState(false);
  const [listError, setListError] = useState<string | null>(null);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [confirmDisable, setConfirmDisable] = useState(false);
  const [editorMode, setEditorMode] = useState<"create" | "revise" | null>(null);
  const [editRevision, setEditRevision] = useState<AutomationRevision | null>(null);
  const [revisionLoading, setRevisionLoading] = useState(false);
  const [revisionError, setRevisionError] = useState<string | null>(null);
  const [reload, setReload] = useState(0);
  const [detailReload, setDetailReload] = useState(0);
  const listGeneration = useRef(0);
  const detailGeneration = useRef(0);
  const requestIds = useRef(new Map<string, PendingRequest>());

  useEffect(() => {
    const controller = new AbortController();
    const generation = ++listGeneration.current;
    ++detailGeneration.current;
    setItems([]); setNextCursor(null); setSelectedId(null); setSelected(null);
    setListError(null); setDetailError(null); setActionError(null); setMessage(null); setConfirmDisable(false);
    setLoading(Boolean(workspaceId));
    if (!workspaceId) { setLoading(false); return () => controller.abort(); }
    void api.list(undefined, controller.signal).then(page => {
      if (controller.signal.aborted || generation !== listGeneration.current) return;
      if (page.items.some(item => item.workspace_id !== workspaceId)) throw new Error("Automation list belongs to another Workspace.");
      setItems(page.items);
      setNextCursor(page.next_cursor);
      setSelectedId(page.items[0]?.automation_id ?? null);
    }).catch(error => {
      if (!controller.signal.aborted && generation === listGeneration.current) setListError(errorText(error));
    }).finally(() => {
      if (!controller.signal.aborted && generation === listGeneration.current) setLoading(false);
    });
    return () => controller.abort();
  }, [api, workspaceId, reload]);

  useEffect(() => {
    if (!selectedId) { setSelected(null); setDetailLoading(false); return; }
    const controller = new AbortController();
    const generation = ++detailGeneration.current;
    setSelected(null); setDetailError(null); setConfirmDisable(false); setDetailLoading(true);
    void api.get(selectedId, controller.signal).then(found => {
      if (controller.signal.aborted || generation !== detailGeneration.current) return;
      if (found.automation_id !== selectedId || found.workspace_id !== workspaceId) throw new Error("Automation detail identity or Workspace does not match the selection.");
      setSelected(found);
      setItems(current => current.map(item => item.automation_id === found.automation_id ? found : item));
    }).catch(error => {
      if (!controller.signal.aborted && generation === detailGeneration.current) setDetailError(errorText(error));
    }).finally(() => {
      if (!controller.signal.aborted && generation === detailGeneration.current) setDetailLoading(false);
    });
    return () => controller.abort();
  }, [api, selectedId, workspaceId, detailReload]);

  const loadMore = async () => {
    if (!nextCursor || loadingMore) return;
    const generation = listGeneration.current;
    setLoadingMore(true); setListError(null);
    try {
      const page = await api.list(nextCursor);
      if (generation !== listGeneration.current) return;
      if (page.items.some(item => item.workspace_id !== workspaceId)) throw new Error("Automation page belongs to another Workspace.");
      setItems(current => {
        const known = new Set(current.map(item => item.automation_id));
        return [...current, ...page.items.filter(item => !known.has(item.automation_id))];
      });
      setNextCursor(page.next_cursor);
    } catch (error) {
      if (generation === listGeneration.current) setListError(errorText(error));
    } finally {
      if (generation === listGeneration.current) setLoadingMore(false);
    }
  };

  const transition = async (action: "pause" | "disable") => {
    if (!selected || selected.workspace_id !== workspaceId || busy) return;
    const signature = JSON.stringify([workspaceId, selected.automation_id, selected.version, action]);
    let pending = requestIds.current.get(signature);
    if (!pending) {
      const randomUUID = globalThis.crypto?.randomUUID;
      if (!randomUUID) { setActionError("This desktop session cannot create secure request identities. Restart LiteCowork before changing an Automation."); return; }
      pending = { signature, id: randomUUID.call(globalThis.crypto) };
      requestIds.current.set(signature, pending);
    }
    setBusy(true); setActionError(null); setMessage(null); setConfirmDisable(false);
    try {
      const updated = action === "pause"
        ? await api.pause(selected.automation_id, selected.version, pending.id)
        : await api.disable(selected.automation_id, selected.version, pending.id);
      requestIds.current.delete(signature);
      setSelected(updated);
      setItems(current => current.map(item => item.automation_id === updated.automation_id ? updated : item));
      setMessage(action === "pause" ? "Automation paused. This only changes its stored status; trigger execution is not available in this build." : "Automation disabled. Existing history is retained.");
    } catch (error) {
      setActionError(errorText(error));
    } finally { setBusy(false); }
  };

  const startRevision = async () => {
    if (!selected || selected.status !== "PAUSED" || revisionLoading) return;
    setRevisionLoading(true); setRevisionError(null); setEditorMode(null); setEditRevision(null);
    try {
      const revision = await api.getCurrentRevision(selected);
      if (revision.automation_id !== selected.automation_id || revision.revision !== selected.current_revision) throw new Error("The fetched Automation revision is not the current head.");
      setEditRevision(revision); setEditorMode("revise");
    } catch (problem) { setRevisionError(errorText(problem)); }
    finally { setRevisionLoading(false); }
  };

  return (
    <div className="page-content automations-content">
      <header className="automations-heading">
        <div><div className="eyebrow">WORKSPACE RESPONSIBILITIES</div><h1>Automations</h1><p>Saved definitions connect a Routine to a future trigger. They do not run in this desktop build.</p></div>
        <div className="automation-heading-actions"><button className="primary-button" type="button" onClick={() => { setEditorMode("create"); setEditRevision(null); }} disabled={loading || !workspaceId}>New Automation</button><button className="quiet-button" type="button" onClick={() => setReload(value => value + 1)} disabled={loading || loadingMore}>Refresh</button></div>
      </header>

      <section className="automation-capability-note" role="note" aria-label="Automation execution status">
        <strong>Trigger hosting and Task creation are not connected.</strong>
        <span>Definitions shown here are records only. LiteCowork will not run a schedule, webhook, or create Tasks from these Automations.</span>
        <span>Definitions must pin a real saved Routine revision from this Workspace. Saving always creates or leaves the Automation paused; pause, run, and resume workflows are not exposed.</span>
      </section>

      {editorMode && <AutomationEditor key={`${workspaceId}:${editorMode}:${editRevision?.automation_id ?? "new"}`} api={api} workspaceId={workspaceId}
        existing={editorMode === "revise" && selected && editRevision ? { automation: selected, revision: editRevision } : undefined}
        onCancel={() => { setEditorMode(null); setEditRevision(null); }}
        onSaved={(updated, action) => {
          setEditorMode(null); setEditRevision(null); setSelected(updated); setSelectedId(updated.automation_id);
          setItems(current => action === "created" ? [updated, ...current.filter(item => item.automation_id !== updated.automation_id)] : current.map(item => item.automation_id === updated.automation_id ? updated : item));
          setMessage(action === "created" ? "Automation saved paused. It will not run until trigger hosting and Task creation are implemented." : "New Automation revision saved; the definition remains paused.");
        }} />}

      {!workspaceId ? <section className="automation-empty"><h2>Select a Workspace</h2><p>Choose a Workspace to view its saved Automation definitions.</p></section> : loading ? (
        <section className="automation-empty" aria-live="polite"><span className="automation-spinner" aria-hidden="true" /><p>Loading saved definitions…</p></section>
      ) : listError ? (
        <section className="automation-empty automation-error" role="alert"><h2>Automations could not be loaded</h2><p>{listError}</p><button className="text-button" type="button" onClick={() => setReload(value => value + 1)}>Try again</button></section>
      ) : items.length === 0 ? (
        <section className="automation-empty"><div className="automation-empty-icon" aria-hidden="true">◷</div><h2>No saved Automations</h2><p>When Routine selection is connected, you can save a definition here. Nothing is scheduled or running now.</p></section>
      ) : (
        <div className="automation-layout">
          <section className="automation-list" aria-label="Saved Automation definitions">
            <div className="automation-list-header"><h2>Definitions</h2><span>{items.length}{nextCursor ? "+" : ""}</span></div>
            <ul>{items.map(item => (
              <li key={item.automation_id}>
                <button type="button" className={`automation-row${selectedId === item.automation_id ? " selected" : ""}`} onClick={() => { setEditorMode(null); setEditRevision(null); setSelectedId(item.automation_id); }} aria-current={selectedId === item.automation_id ? "true" : undefined}>
                  <span className="automation-row-main"><strong>{item.name}</strong><small>Revision {item.current_revision}</small></span>
                  <span className={statusClass(item.status)}>{statusLabel[item.status]}</span>
                </button>
              </li>
            ))}</ul>
            {nextCursor && <button className="quiet-button automation-more" type="button" onClick={() => void loadMore()} disabled={loadingMore}>{loadingMore ? "Loading…" : "Load more"}</button>}
          </section>

          <section className="automation-detail" aria-label="Selected Automation">
            {detailLoading ? <p role="status">Loading current Automation state…</p> : detailError ? <div className="automation-inline-error" role="alert"><p>{detailError}</p><button type="button" className="text-button" onClick={() => setDetailReload(value => value + 1)}>Reload current state</button></div> : selected ? <>
              <div className="automation-detail-top"><div><div className="eyebrow">SAVED DEFINITION</div><h2>{selected.name}</h2></div><span className={statusClass(selected.status)}>{statusLabel[selected.status]}</span></div>
              <dl className="automation-facts">
                <div><dt>Current revision</dt><dd>{selected.current_revision}</dd></div>
                <div><dt>Updated</dt><dd>{dateText(selected.updated_at)}</dd></div>
                <div><dt>Record version</dt><dd>{selected.version}</dd></div>
              </dl>
              {selected.status === "ENABLED" && <p className="automation-state-warning">Stored status is Enabled, but trigger hosting and occurrence-to-Task execution are unavailable here. This screen cannot resume or run it.</p>}
              {selected.status === "PAUSED" && <p className="automation-state-note">Paused definition. Trigger execution is unavailable in this build.</p>}
              {selected.status === "DISABLED" && <p className="automation-state-note">Disabled permanently. The definition remains available for history.</p>}
              {detailError && <p className="automation-inline-error" role="alert">{detailError}</p>}
              {actionError && <p className="automation-inline-error" role="alert">{actionError}</p>}
              {revisionError && <p className="automation-inline-error" role="alert">{revisionError}</p>}
              {message && <p className="automation-inline-success" role="status">{message}</p>}
              {confirmDisable && <div className="automation-confirm" role="group" aria-label="Confirm permanent disable"><p>Disable this definition permanently? This keeps the record but cannot be undone from this screen.</p><button type="button" className="quiet-button" disabled={busy} onClick={() => setConfirmDisable(false)}>Keep it</button><button type="button" className="danger-button" disabled={busy} onClick={() => void transition("disable")}>{busy ? "Disabling…" : "Confirm disable"}</button></div>}
              {selected.status !== "DISABLED" && <div className="automation-actions">
                {selected.status === "PAUSED" && <button className="quiet-button" type="button" disabled={busy || revisionLoading} onClick={() => void startRevision()}>{revisionLoading ? "Loading pinned revision…" : "Edit definition"}</button>}
                {selected.status === "ENABLED" && <button className="quiet-button" type="button" disabled={busy} onClick={() => void transition("pause")}>{busy ? "Pausing…" : "Pause definition"}</button>}
                {!confirmDisable && <button className="danger-button" type="button" disabled={busy} onClick={() => { setActionError(null); setConfirmDisable(true); }}>Disable permanently</button>}
              </div>}
              {selected.status === "ENABLED" && <p className="automation-policy-note">Pause this stored definition before revising it. This build cannot resume it after the edit.</p>}
              <p className="automation-provenance">Created {dateText(selected.created_at)} · {selected.automation_id}</p>
            </> : <div className="automation-empty"><p>Select a definition to review its stored state.</p></div>}
          </section>
        </div>
      )}
      {listError && items.length > 0 && <p className="automation-inline-error" role="alert">{listError}</p>}
    </div>
  );
}
