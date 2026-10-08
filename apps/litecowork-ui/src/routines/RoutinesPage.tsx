import { useEffect, useRef, useState } from "react";
import type { Routine, RoutineApi, RoutineRevision } from "./routine-api";
import "./routines-page.css";

type Props = { api: RoutineApi; workspaceId: string };
type Pending = { signature: string; id: string };
type Editor = { routineId: string | null; version: number | null; name: string; objective: string; instructions: string; advanced: string };
const emptyAdvanced = {
  input_schema: { type: "object", properties: {}, additionalProperties: false },
  constraints: [], non_goals: [], required_outputs: [], acceptance_criteria: [],
  approvals_required: [], input_bindings: [], required_capabilities: [],
  preferred_agent_binding_id: null, placement_preference: "AUTO", budget_ceiling: null,
  verification_policy: {},
};

function errorText(error: unknown): string { return error instanceof Error ? error.message : "The Routine request could not be completed."; }
function dateText(value: string): string { const date = new Date(value); return Number.isNaN(date.getTime()) ? "Date unavailable" : date.toLocaleString(); }
function definitionFields(revision: RoutineRevision): Record<string, unknown> {
  const result = { ...revision };
  delete result.routine_id; delete result.revision; delete result.authored_by; delete result.created_at;
  delete result.objective_template; delete result.instructions;
  return result;
}
function parseAdvanced(source: string): Record<string, unknown> {
  if (source.length > 96 * 1024) throw new Error("The advanced definition must be under 96 KB.");
  const value: unknown = JSON.parse(source);
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Advanced definition must be a JSON object.");
  const fields = value as Record<string, unknown>;
  const required = ["input_schema", "constraints", "non_goals", "required_outputs", "acceptance_criteria", "approvals_required", "input_bindings", "required_capabilities", "placement_preference", "verification_policy"];
  for (const key of required) if (!(key in fields)) throw new Error(`Advanced definition is missing required field “${key}”.`);
  return fields;
}
function freshRequestId(): string {
  const randomUUID = globalThis.crypto?.randomUUID;
  if (!randomUUID) throw new Error("This desktop session cannot create secure request identities. Restart LiteCowork before saving.");
  return randomUUID.call(globalThis.crypto);
}

/** Saved Routine definitions only. There is deliberately no Run or scheduling action. */
export function RoutinesPage({ api, workspaceId }: Props) {
  const [items, setItems] = useState<Routine[]>([]);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [selected, setSelected] = useState<Routine | null>(null);
  const [currentRevision, setCurrentRevision] = useState<RoutineRevision | null>(null);
  const [loading, setLoading] = useState(true);
  const [detailLoading, setDetailLoading] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [listError, setListError] = useState<string | null>(null);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [reload, setReload] = useState(0);
  const [editor, setEditor] = useState<Editor | null>(null);
  const [busy, setBusy] = useState(false);
  const [confirmArchive, setConfirmArchive] = useState(false);
  const requestIds = useRef(new Map<string, Pending>());
  const listGeneration = useRef(0);
  const detailGeneration = useRef(0);

  useEffect(() => {
    const controller = new AbortController();
    const generation = ++listGeneration.current;
    ++detailGeneration.current;
    setItems([]); setNextCursor(null); setSelected(null); setCurrentRevision(null);
    setListError(null); setDetailError(null); setActionError(null); setMessage(null); setEditor(null); setConfirmArchive(false);
    setLoading(Boolean(workspaceId));
    if (!workspaceId) { setLoading(false); return () => controller.abort(); }
    void api.list(undefined, controller.signal).then(page => {
      if (controller.signal.aborted || generation !== listGeneration.current) return;
      if (page.items.some(item => item.workspace_id !== workspaceId)) throw new Error("Routine page belongs to another Workspace.");
      setItems(page.items); setNextCursor(page.next_cursor);
      setSelectedId(current => current && page.items.some(item => item.routine_id === current) ? current : page.items[0]?.routine_id ?? null);
    }).catch(error => { if (!controller.signal.aborted && generation === listGeneration.current) setListError(errorText(error)); })
      .finally(() => { if (!controller.signal.aborted && generation === listGeneration.current) setLoading(false); });
    return () => controller.abort();
  }, [api, workspaceId, reload]);

  useEffect(() => {
    if (!selectedId) { setSelected(null); setCurrentRevision(null); setDetailLoading(false); return; }
    const controller = new AbortController();
    const generation = ++detailGeneration.current;
    setSelected(null); setCurrentRevision(null); setDetailError(null); setConfirmArchive(false); setDetailLoading(true);
    void (async () => {
      const routine = await api.get(selectedId, controller.signal);
      if (routine.routine_id !== selectedId || routine.workspace_id !== workspaceId) throw new Error("Routine identity or Workspace does not match the selection.");
      let cursor: string | undefined;
      let found: RoutineRevision | undefined;
      // Revision pages are ascending; stop once the pinned current revision is found.
      for (let pageNumber = 0; pageNumber < 100; pageNumber++) {
        const page = await api.revisions(selectedId, cursor, controller.signal);
        found = page.items.find(revision => revision.revision === routine.current_revision);
        if (found || !page.next_cursor) break;
        cursor = page.next_cursor;
      }
      if (!found) throw new Error("The current immutable Routine revision could not be loaded.");
      if (controller.signal.aborted || generation !== detailGeneration.current) return;
      setSelected(routine); setCurrentRevision(found);
      setItems(current => current.map(item => item.routine_id === routine.routine_id ? routine : item));
    })().catch(error => { if (!controller.signal.aborted && generation === detailGeneration.current) setDetailError(errorText(error)); })
      .finally(() => { if (!controller.signal.aborted && generation === detailGeneration.current) setDetailLoading(false); });
    return () => controller.abort();
  }, [api, selectedId, workspaceId, reload]);

  const loadMore = async () => {
    if (!nextCursor || loadingMore) return;
    const generation = listGeneration.current;
    setLoadingMore(true); setListError(null);
    try {
      const page = await api.list(nextCursor);
      if (generation !== listGeneration.current) return;
      if (page.items.some(item => item.workspace_id !== workspaceId)) throw new Error("Routine page belongs to another Workspace.");
      setItems(current => { const known = new Set(current.map(item => item.routine_id)); return [...current, ...page.items.filter(item => !known.has(item.routine_id))]; });
      setNextCursor(page.next_cursor);
    } catch (error) { if (generation === listGeneration.current) setListError(errorText(error)); }
    finally { if (generation === listGeneration.current) setLoadingMore(false); }
  };

  const startCreate = () => {
    setActionError(null); setMessage(null); setEditor({ routineId: null, version: null, name: "", objective: "", instructions: "", advanced: JSON.stringify(emptyAdvanced, null, 2) });
  };
  const startEdit = () => {
    if (!selected || !currentRevision || selected.status !== "ACTIVE") return;
    const fields = definitionFields(currentRevision);
    setActionError(null); setMessage(null);
    setEditor({ routineId: selected.routine_id, version: selected.version, name: selected.name, objective: currentRevision.objective_template, instructions: currentRevision.instructions, advanced: JSON.stringify(fields, null, 2) });
  };
  const pendingFor = (signature: string): Pending => {
    const existing = requestIds.current.get(signature);
    if (existing) return existing;
    const pending = { signature, id: freshRequestId() };
    requestIds.current.set(signature, pending);
    return pending;
  };
  const save = async () => {
    if (!editor || busy) return;
    setActionError(null); setMessage(null);
    try {
      if (!editor.name.trim() || editor.name.trim().length > 120) throw new Error("Name must contain 1–120 characters.");
      if (!editor.objective.trim() || editor.objective.length > 4000) throw new Error("Objective must contain 1–4,000 characters.");
      if (!editor.instructions.trim()) throw new Error("Instructions are required.");
      if (editor.instructions.length > 32000) throw new Error("Instructions must be no longer than 32,000 characters.");
      const revision = { ...parseAdvanced(editor.advanced), objective_template: editor.objective, instructions: editor.instructions };
      const signature = JSON.stringify([workspaceId, editor.routineId, editor.version, editor.name.trim(), revision]);
      const pending = pendingFor(signature);
      setBusy(true);
      if (editor.routineId) {
        const createdRevision = await api.revise(editor.routineId, editor.version!, revision, pending.id);
        requestIds.current.delete(signature);
        setMessage(`Saved immutable revision ${createdRevision.revision}. Existing Automation pins remain unchanged.`);
        setEditor(null); setReload(value => value + 1);
      } else {
        const created = await api.create(editor.name.trim(), revision, pending.id);
        requestIds.current.delete(signature);
        setMessage("Routine saved. It has not been run or scheduled."); setEditor(null); setReload(value => value + 1); setSelectedId(created.routine_id);
      }
    } catch (error) { setActionError(errorText(error)); }
    finally { setBusy(false); }
  };
  const archive = async () => {
    if (!selected || selected.status !== "ACTIVE" || busy) return;
    setActionError(null); setMessage(null);
    try {
      const signature = JSON.stringify([workspaceId, selected.routine_id, selected.version, "archive"]);
      const pending = pendingFor(signature);
      setBusy(true);
      const updated = await api.archive(selected.routine_id, selected.version, pending.id);
      requestIds.current.delete(signature); setSelected(updated);
      setItems(current => current.map(item => item.routine_id === updated.routine_id ? updated : item));
      setMessage("Routine archived. Existing Tasks and Automation history retain their pinned revisions."); setConfirmArchive(false);
    } catch (error) { setActionError(errorText(error)); }
    finally { setBusy(false); }
  };

  return <div className="page-content routines-content">
    <header className="routines-heading">
      <div><div className="eyebrow">WORKSPACE LIBRARY</div><h1>Routines</h1><p>Save reusable work instructions as versioned definitions. Saving here does not run or schedule them.</p></div>
      <div className="routines-heading-actions"><button className="quiet-button" type="button" onClick={() => setReload(value => value + 1)} disabled={loading}>Refresh</button><button className="primary-button" type="button" onClick={startCreate} disabled={!workspaceId || Boolean(editor)}>New Routine</button></div>
    </header>

    <section className="routine-safety-note" role="note"><strong>Definitions only</strong><span>This desktop build can save and revise Routine definitions. Run-now, scheduling, and Task creation are not connected here.</span></section>
    {message && <p className="routine-message" role="status">{message}</p>}
    {!workspaceId ? <section className="routine-state"><h2>Select a Workspace</h2><p>Choose a Workspace to view its saved Routines.</p></section> : loading ? <section className="routine-state" role="status">Loading saved Routines…</section> : listError ? <section className="routine-state routine-error" role="alert"><h2>Routines could not be loaded</h2><p>{listError}</p><button className="text-button" type="button" onClick={() => setReload(value => value + 1)}>Try again</button></section> : <>
      {items.length === 0 && !editor ? <section className="routine-state"><h2>No saved Routines</h2><p>Create a reusable definition. You can connect it to execution after the local Task pipeline is available.</p><button className="primary-button" type="button" onClick={startCreate}>Create a Routine</button></section> : <div className="routines-layout">
        {items.length > 0 && <section className="routines-list" aria-label="Saved Routines"><div className="routines-list-heading"><h2>Saved</h2><span>{items.length}{nextCursor ? "+" : ""}</span></div><ul>{items.map(item => <li key={item.routine_id}><button type="button" className={`routine-row${selectedId === item.routine_id && !editor ? " is-selected" : ""}`} onClick={() => { setEditor(null); setSelectedId(item.routine_id); setActionError(null); }} aria-current={selectedId === item.routine_id && !editor ? "true" : undefined}><span><strong>{item.name}</strong><small>Revision {item.current_revision}</small></span><span className={`routine-status routine-status-${item.status.toLowerCase()}`}>{item.status === "ACTIVE" ? "Active" : "Archived"}</span></button></li>)}</ul>{nextCursor && <button className="quiet-button routine-load-more" type="button" disabled={loadingMore} onClick={() => void loadMore()}>{loadingMore ? "Loading…" : "Load more"}</button>}</section>}

        <section className="routine-detail" aria-label={editor ? "Routine editor" : "Selected Routine"}>
          {editor ? <>
            <div className="routine-detail-heading"><div><div className="eyebrow">{editor.routineId ? "NEW IMMUTABLE REVISION" : "NEW DEFINITION"}</div><h2>{editor.routineId ? "Revise Routine" : "Create Routine"}</h2></div><button className="text-button" type="button" onClick={() => { setEditor(null); setActionError(null); }}>Cancel</button></div>
            <p className="routine-helper">These instructions define reusable work only. They do not grant access or bypass Workspace policy.</p>
            <label className="routine-field">Name<input value={editor.name} maxLength={120} disabled={busy || Boolean(editor.routineId)} onChange={event => setEditor({ ...editor, name: event.target.value })} />{editor.routineId && <small>Routine names stay the same when you create a new definition revision.</small>}</label>
            <label className="routine-field">Objective template<textarea value={editor.objective} maxLength={4000} rows={4} disabled={busy} onChange={event => setEditor({ ...editor, objective: event.target.value })} /></label>
            <label className="routine-field">Instructions <span aria-hidden="true">(required)</span><textarea required value={editor.instructions} maxLength={32000} rows={6} disabled={busy} onChange={event => setEditor({ ...editor, instructions: event.target.value })} /></label>
            <details className="routine-advanced"><summary>Inputs, outputs, placement, budget, and verification</summary><p>Edit the remaining contract fields as JSON. Existing values are copied from the selected immutable revision before editing.</p><label className="routine-field">Routine definition fields<textarea className="routine-json" value={editor.advanced} spellCheck={false} disabled={busy} onChange={event => setEditor({ ...editor, advanced: event.target.value })} /></label></details>
            {actionError && <p className="routine-inline-error" role="alert">{actionError}</p>}
            <div className="routine-form-actions"><button className="quiet-button" type="button" disabled={busy} onClick={() => setEditor(null)}>Cancel</button><button className="primary-button" type="button" disabled={busy} onClick={() => void save()}>{busy ? "Saving…" : editor.routineId ? "Save new revision" : "Save Routine"}</button></div>
          </> : detailLoading ? <p role="status">Loading current Routine revision…</p> : detailError ? <div className="routine-state routine-error" role="alert"><p>{detailError}</p><button className="text-button" type="button" onClick={() => setReload(value => value + 1)}>Reload</button></div> : selected && currentRevision ? <>
            <div className="routine-detail-heading"><div><div className="eyebrow">CURRENT REVISION · {currentRevision.revision}</div><h2>{selected.name}</h2></div><span className={`routine-status routine-status-${selected.status.toLowerCase()}`}>{selected.status === "ACTIVE" ? "Active" : "Archived"}</span></div>
            <div className="routine-actions">{selected.status === "ACTIVE" && <><button className="quiet-button" type="button" disabled={busy} onClick={startEdit}>Revise</button>{!confirmArchive && <button className="routine-danger-button" type="button" disabled={busy} onClick={() => { setActionError(null); setConfirmArchive(true); }}>Archive</button>}</>}</div>
            {confirmArchive && <div className="routine-confirm" role="group" aria-label="Confirm archive"><p>Archive this definition? Existing Tasks and Automation revisions remain pinned, but new references will be blocked.</p><button className="quiet-button" type="button" disabled={busy} onClick={() => setConfirmArchive(false)}>Keep active</button><button className="routine-danger-button" type="button" disabled={busy} onClick={() => void archive()}>{busy ? "Archiving…" : "Confirm archive"}</button></div>}
            {actionError && <p className="routine-inline-error" role="alert">{actionError}</p>}
            <section className="routine-definition"><h3>Objective template</h3><p className="routine-preserve">{currentRevision.objective_template}</p><h3>Instructions</h3><p className="routine-preserve">{currentRevision.instructions || "No additional instructions."}</p><details><summary>Definition fields</summary><pre>{JSON.stringify(definitionFields(currentRevision), null, 2)}</pre></details></section>
            <p className="routine-provenance">Created {dateText(selected.created_at)} · updated {dateText(selected.updated_at)} · version {selected.version}</p>
          </> : <div className="routine-state"><p>Select a Routine to review its current revision.</p></div>}
        </section>
      </div>}
    </>}
  </div>;
}
