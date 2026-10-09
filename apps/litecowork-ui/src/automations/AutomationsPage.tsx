import { useEffect, useRef, useState } from "react";
import type { Automation, AutomationApi, AutomationRevision, AutomationStatus, MaterializedAutomationTask, Routine, RoutineRevision } from "./automation-api";
import { pinnedResourceOptionKey, routineInputFields, type ResourceOption } from "../routines/routine-inputs";
import { AutomationEditor } from "./AutomationEditor";
import "./automations-page.css";

type Props = { api: AutomationApi; workspaceId: string; resources: ResourceOption[]; resourcesNextCursor: string | null; resourcesPageBusy: boolean; onLoadMoreResources: () => Promise<void>; onOpenTask?: (taskId: string) => void };
type PendingRequest = { signature: string; id: string };
type PendingRun = PendingRequest & { key: string; workspaceId: string; triggerId: string; automation: Automation; revision: AutomationRevision; inputs: Record<string, unknown> };
const MAX_PENDING_RUNS = 32;
const statusLabel: Record<AutomationStatus, string> = { ENABLED: "Enabled", PAUSED: "Paused", DISABLED: "Disabled" };

// Route-scoped React state is insufficient for an idempotent mutation: navigation can
// unmount this page after the daemon commits but before the response is observed. Keep
// only bounded request envelopes in process memory. Never use localStorage/sessionStorage
// here; inputs may contain user text and ResourceRefs, but never Resource bytes.
const pendingManualRuns = new Map<string, PendingRun>();
const pendingRunListeners = new Set<() => void>();
function publishPendingRunChange(): void {
  for (const listener of pendingRunListeners) listener();
}
function subscribePendingRuns(listener: () => void): () => void {
  pendingRunListeners.add(listener);
  return () => pendingRunListeners.delete(listener);
}
function manualRunKey(workspaceId: string, automationId: string, revision: number, triggerId: string): string {
  return JSON.stringify([workspaceId, automationId, revision, triggerId]);
}
function pendingRunsForWorkspace(workspaceId: string): PendingRun[] {
  return [...pendingManualRuns.values()].filter(run => run.workspaceId === workspaceId);
}
function copyJsonRecord(value: Record<string, unknown>): Record<string, unknown> {
  return JSON.parse(JSON.stringify(value)) as Record<string, unknown>;
}
function manualTriggerId(revision: AutomationRevision): string | null {
  const manual = revision.triggers.filter(spec => {
    const definition = spec.trigger && typeof spec.trigger === "object" ? spec.trigger as Record<string, unknown> : null;
    return definition?.kind === "MANUAL";
  });
  const id = manual.length === 1 ? manual[0].trigger_id : null;
  return typeof id === "string" && id.length > 0 ? id : null;
}

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : "The Automation request could not be completed.";
}
function dateText(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? "Date unavailable" : date.toLocaleString();
}
function statusClass(status: AutomationStatus): string { return `automation-status automation-status-${status.toLowerCase()}`; }
function freshRequestId(): string {
  const randomUUID = globalThis.crypto?.randomUUID;
  if (!randomUUID) throw new Error("This desktop session cannot create secure request identities. Restart LiteCowork before running an Automation.");
  return randomUUID.call(globalThis.crypto);
}
function manualTriggerCount(revision: AutomationRevision | null): number {
  return revision?.triggers.filter(spec => {
    const definition = spec.trigger && typeof spec.trigger === "object" ? spec.trigger as Record<string, unknown> : null;
    return definition?.kind === "MANUAL";
  }).length ?? 0;
}
function triggerLabels(revision: AutomationRevision | null): string[] {
  return revision?.triggers.map(spec => {
    const definition = spec.trigger && typeof spec.trigger === "object" ? spec.trigger as Record<string, unknown> : null;
    return typeof definition?.kind === "string" ? definition.kind.replaceAll("_", " ") : "Unknown trigger";
  }) ?? [];
}

/** This page manages definitions and explicit one-shot ManualTrigger Task creation; it never hosts automatic triggers. */
export function AutomationsPage({ api, workspaceId, resources, resourcesNextCursor, resourcesPageBusy, onLoadMoreResources, onOpenTask }: Props) {
  const [items, setItems] = useState<Automation[]>([]);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [selected, setSelected] = useState<Automation | null>(null);
  const [selectedRevision, setSelectedRevision] = useState<AutomationRevision | null>(null);
  const [pinnedRoutine, setPinnedRoutine] = useState<Routine | null>(null);
  const [pinnedRoutineRevision, setPinnedRoutineRevision] = useState<RoutineRevision | null>(null);
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
  const [runInputs, setRunInputs] = useState<Record<string, unknown>>({});
  const [runResult, setRunResult] = useState<MaterializedAutomationTask | null>(null);
  const [unresolvedRuns, setUnresolvedRuns] = useState<PendingRun[]>([]);
  const [reload, setReload] = useState(0);
  const [detailReload, setDetailReload] = useState(0);
  const listGeneration = useRef(0);
  const detailGeneration = useRef(0);
  const requestIds = useRef(new Map<string, PendingRequest>());

  useEffect(() => {
    const controller = new AbortController();
    const generation = ++listGeneration.current;
    ++detailGeneration.current;
    setItems([]); setNextCursor(null); setSelectedId(null); setSelected(null); setSelectedRevision(null); setPinnedRoutine(null); setPinnedRoutineRevision(null); setRunInputs({}); setRunResult(null); setUnresolvedRuns(pendingRunsForWorkspace(workspaceId));
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
    if (!selectedId) { setSelected(null); setSelectedRevision(null); setPinnedRoutine(null); setPinnedRoutineRevision(null); setDetailLoading(false); return; }
    const controller = new AbortController();
    const generation = ++detailGeneration.current;
    setSelected(null); setSelectedRevision(null); setPinnedRoutine(null); setPinnedRoutineRevision(null); setRunInputs({}); setRunResult(null); setDetailError(null); setConfirmDisable(false); setDetailLoading(true);
    void (async () => {
      const found = await api.get(selectedId, controller.signal);
      if (found.automation_id !== selectedId || found.workspace_id !== workspaceId) throw new Error("Automation detail identity or Workspace does not match the selection.");
      const revision = await api.getCurrentRevision(found, controller.signal);
      if (revision.automation_id !== found.automation_id || revision.revision !== found.current_revision) throw new Error("The current immutable Automation revision could not be verified.");
      const routine = await api.getRoutine(revision.routine_id, controller.signal);
      if (routine.workspace_id !== workspaceId || routine.routine_id !== revision.routine_id) throw new Error("Pinned Routine belongs to a different Workspace.");
      const routineRevision = await api.getRoutineRevision(revision.routine_id, revision.routine_revision, controller.signal);
      if (routineRevision.routine_id !== revision.routine_id || routineRevision.revision !== revision.routine_revision) throw new Error("The exact pinned Routine revision could not be verified.");
      if (controller.signal.aborted || generation !== detailGeneration.current) return;
      setSelected(found); setSelectedRevision(revision); setPinnedRoutine(routine); setPinnedRoutineRevision(routineRevision);
      setItems(current => current.map(item => item.automation_id === found.automation_id ? found : item));
    })().catch(error => {
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

  const runWithRequest = async (pending: PendingRun) => {
    if (pending.workspaceId !== workspaceId || pending.automation.workspace_id !== workspaceId) {
      setActionError("This pending Run belongs to a different Workspace. Switch to its original Workspace to resolve it."); return;
    }
    setBusy(true); setActionError(null); setMessage(null); setRunResult(null);
    try {
      const task = await api.run(pending.automation, pending.revision, pending.inputs, pending.id);
      pendingManualRuns.delete(pending.key);
      publishPendingRunChange();
      setUnresolvedRuns(pendingRunsForWorkspace(workspaceId));
      setRunResult(task);
      setMessage(`Saved Task ${task.task_id} is READY. Planning and agent execution have not started.`);
    } catch (error) {
      setUnresolvedRuns(pendingRunsForWorkspace(workspaceId));
      setActionError(errorText(error));
    } finally { setBusy(false); }
  };

  const runNow = async () => {
    if (!selected || !selectedRevision || !pinnedRoutine || !pinnedRoutineRevision || busy) return;
    setActionError(null); setMessage(null); setRunResult(null);
    if (selected.status === "DISABLED") { setActionError("Disabled Automations cannot create new Tasks."); return; }
    if (manualTriggerCount(selectedRevision) !== 1) { setActionError("This definition must contain exactly one Manual trigger to create a Task here."); return; }
    const triggerId = manualTriggerId(selectedRevision);
    if (!triggerId) { setActionError("The Manual trigger identity is unavailable. Reload the saved Automation before running it."); return; }
    if (pinnedRoutine.status !== "ACTIVE") { setActionError("The pinned Routine is archived. Restore or revise the definition before creating a Task."); return; }
    const contract = routineInputFields(pinnedRoutineRevision);
    if (contract.blockedReason) { setActionError(contract.blockedReason); return; }
    const inputs: Record<string, unknown> = {};
    for (const field of contract.fields) {
      const value = runInputs[field.name] ?? (field.kind === "TEXT" ? "" : null);
      if (field.kind === "RESOURCE_REF") {
        if (value === null) {
          if (field.required) { setActionError(`${field.label} is required.`); return; }
          continue;
        }
        if (!value || typeof value !== "object" || Array.isArray(value)) { setActionError(`${field.label} must be selected from this Workspace's Resources.`); return; }
        const ref = value as Record<string, unknown>;
        if (Object.keys(ref).sort().join(",") !== "resource_id,revision_id,workspace_id"
          || ref.workspace_id !== workspaceId || typeof ref.resource_id !== "string" || !ref.resource_id
          || typeof ref.revision_id !== "string" || !ref.revision_id) { setActionError(`${field.label} selection is stale or belongs to another Workspace. Select it again from this Workspace's Resource catalog.`); return; }
        // Preserve an exact selected ref for idempotent retry if the catalog head changes.
        // Task admission rechecks Workspace ownership and historical revision availability.
        inputs[field.name] = { workspace_id: workspaceId, resource_id: ref.resource_id, revision_id: ref.revision_id };
        continue;
      }
      const text = typeof value === "string" ? value : "";
      if (field.enumValues && text.length > 0 && !field.enumValues.includes(text)) { setActionError(`${field.label} must be one of the listed choices.`); return; }
      const characters = Array.from(text).length;
      const bytes = new TextEncoder().encode(text).byteLength;
      if (field.required && text.trim().length === 0) { setActionError(`${field.label} is required.`); return; }
      if (characters > field.maxLength || characters < field.minLength) {
        setActionError(`${field.label} must be ${field.minLength ? `${field.minLength}–` : "at most "}${field.maxLength} characters.`); return;
      }
      if (bytes > field.maxBytes) { setActionError(`${field.label} exceeds its ${field.maxBytes}-byte UTF-8 limit.`); return; }
      if (text.length > 0) inputs[field.name] = text;
    }
    const key = manualRunKey(workspaceId, selected.automation_id, selectedRevision.revision, triggerId);
    const signature = JSON.stringify([workspaceId, selected.automation_id, selected.version, selectedRevision.revision, triggerId, inputs]);
    const priorForAutomation = pendingRunsForWorkspace(workspaceId).find(run => run.automation.automation_id === selected.automation_id);
    if (priorForAutomation && priorForAutomation.key !== key) {
      setActionError("A Run request for this Automation still has an unknown outcome. Resolve its original request before creating another Task."); return;
    }
    const stored = pendingManualRuns.get(key);
    if (stored && stored.signature !== signature) {
      setActionError("This Manual trigger has an unresolved request with different inputs. Retry its exact request or explicitly discard its retry record before creating another Task."); return;
    }
    let pending = stored;
    if (!pending) {
      if (pendingManualRuns.size >= MAX_PENDING_RUNS) {
        setActionError("The in-memory unresolved Run limit is full. Resolve or explicitly discard a pending request before starting another."); return;
      }
      try { pending = { key, signature, id: freshRequestId(), workspaceId, triggerId, automation: selected, revision: selectedRevision, inputs: copyJsonRecord(inputs) }; }
      catch (error) { setActionError(errorText(error)); return; }
      pendingManualRuns.set(key, pending);
      publishPendingRunChange();
      setUnresolvedRuns(pendingRunsForWorkspace(workspaceId));
    }
    await runWithRequest(pending);
  };

  const discardUnresolvedRun = (pending: PendingRun) => {
    if (busy || pending.workspaceId !== workspaceId) return;
    pendingManualRuns.delete(pending.key);
    publishPendingRunChange();
    setUnresolvedRuns(pendingRunsForWorkspace(workspaceId));
    setActionError(null);
  };

  const selectedPendingRuns = selected
    ? unresolvedRuns.filter(run => run.automation.automation_id === selected.automation_id)
    : [];

  useEffect(() => {
    const refresh = () => setUnresolvedRuns(pendingRunsForWorkspace(workspaceId));
    const unsubscribe = subscribePendingRuns(refresh);
    refresh();
    return unsubscribe;
  }, [workspaceId]);

  return (
    <div className="page-content automations-content">
      <header className="automations-heading">
        <div><div className="eyebrow">WORKSPACE RESPONSIBILITIES</div><h1>Automations</h1><p>Save a Routine definition for later triggers, or create one Task manually from a Manual-trigger definition.</p></div>
        <div className="automation-heading-actions"><button className="primary-button" type="button" onClick={() => { setEditorMode("create"); setEditRevision(null); }} disabled={loading || !workspaceId}>New Automation</button><button className="quiet-button" type="button" onClick={() => setReload(value => value + 1)} disabled={loading || loadingMore}>Refresh</button></div>
      </header>

      <section className="automation-capability-note" role="note" aria-label="Automation execution status">
        <strong>Schedules are saved but are not hosted in this desktop build.</strong>
        <span>A Manual trigger can create one ordinary READY Task. This does not start planning or an agent; review the Task in Work.</span>
        <span>Saving creates or leaves the Automation paused. Manual runs require the local Runtime and an active local trigger-host binding.</span>
      </section>

      {unresolvedRuns.map(pending => <section className="automation-run-retry" role="alert" key={pending.key}>
        <div><strong>{pending.automation.name} · revision {pending.revision.revision} has an unknown Run outcome.</strong><span>The exact request is held in this desktop process. Retry it to recover its receipt before creating another Task.</span></div>
        <button className="quiet-button" type="button" disabled={busy || pending.workspaceId !== workspaceId} onClick={() => void runWithRequest(pending)}>{busy ? "Checking…" : "Retry exact request"}</button>
        <button className="text-button" type="button" disabled={busy || pending.workspaceId !== workspaceId} onClick={() => discardUnresolvedRun(pending)}>Discard retry record</button>
        <small>Discarding may allow a duplicate Task if the original request committed.</small>
      </section>)}

      {editorMode && <AutomationEditor key={`${workspaceId}:${editorMode}:${editRevision?.automation_id ?? "new"}`} api={api} workspaceId={workspaceId}
        existing={editorMode === "revise" && selected && editRevision ? { automation: selected, revision: editRevision } : undefined}
        onCancel={() => { setEditorMode(null); setEditRevision(null); }}
        onSaved={(updated, action) => {
          setEditorMode(null); setEditRevision(null); setSelected(updated); setSelectedId(updated.automation_id);
          setItems(current => action === "created" ? [updated, ...current.filter(item => item.automation_id !== updated.automation_id)] : current.map(item => item.automation_id === updated.automation_id ? updated : item));
          setDetailReload(value => value + 1);
          setMessage(action === "created" ? "Automation saved paused. Recurring triggers are off; Manual trigger definitions can create one Task when you choose Run once." : "New Automation revision saved; the definition remains paused.");
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
              {selectedRevision && <p className="automation-state-note">Triggers in this revision: {triggerLabels(selectedRevision).join(" · ") || "None"}. Only Manual runs are available here.</p>}
              {selected.status === "ENABLED" && <p className="automation-state-warning">Stored status is Enabled, but recurring trigger hosting is unavailable here. Manual Run now remains an explicit one-shot action.</p>}
              {selected.status === "PAUSED" && <p className="automation-state-note">Paused definition. Recurring triggers remain off. A Manual trigger can still create one Task when you choose Run now.</p>}
              {selected.status === "DISABLED" && <p className="automation-state-note">Disabled permanently. The definition remains available for history.</p>}
              {selectedRevision && manualTriggerCount(selectedRevision) > 1 && <p className="automation-state-warning">Run unavailable: this revision has multiple Manual triggers. Revise it to keep one unambiguous Manual trigger.</p>}
              {detailError && <p className="automation-inline-error" role="alert">{detailError}</p>}
              {actionError && <p className="automation-inline-error" role="alert">{actionError}</p>}
              {revisionError && <p className="automation-inline-error" role="alert">{revisionError}</p>}
              {message && <p className="automation-inline-success" role="status">{message}</p>}
              {selectedRevision && manualTriggerCount(selectedRevision) === 1 && pinnedRoutine && pinnedRoutineRevision && <section className="automation-run-form" aria-labelledby="automation-run-heading">
                <h3 id="automation-run-heading">Create one Task</h3>
                <p>This uses Automation revision {selectedRevision.revision} and Routine revision {selectedRevision.routine_revision}. It saves one Task as READY; it does not start planning or an agent.</p>
                {routineInputFields(pinnedRoutineRevision).fields.map(field => field.kind === "TEXT" ? <label className="automation-field" key={field.name}><span>{field.label}{field.required ? " · required" : ""}</span>{field.enumValues ? <select required={field.required} disabled={busy || selectedPendingRuns.length > 0} value={typeof runInputs[field.name] === "string" ? runInputs[field.name] as string : ""} onChange={event => { const choice = event.currentTarget.value; setRunInputs(current => ({ ...current, [field.name]: choice })); }}><option value="">Choose…</option>{field.enumValues.map(choice => <option key={choice} value={choice}>{choice}</option>)}</select> : <textarea rows={2} maxLength={Math.min(field.maxLength * 2, 32_768)} minLength={field.minLength || undefined} required={field.required} disabled={busy || selectedPendingRuns.length > 0} value={typeof runInputs[field.name] === "string" ? runInputs[field.name] as string : ""} onChange={event => { const text = event.currentTarget.value; setRunInputs(current => ({ ...current, [field.name]: text })); }} />}<small>{field.enumValues ? "Choose one of the values defined by this Routine." : `Maximum ${field.maxLength} characters and ${field.maxBytes} UTF-8 bytes.`}</small></label> : <label className="automation-field" key={field.name}><span>{field.label}{field.required ? " · required" : ""}</span><select disabled={busy || selectedPendingRuns.length > 0} required={field.required} value={(() => { const ref = runInputs[field.name]; if (!ref || typeof ref !== "object" || Array.isArray(ref)) return ""; const source = ref as Record<string, unknown>; const item = resources.find(resource => resource.workspaceId === workspaceId && resource.resourceId === source.resource_id && resource.resourceRevisionId === source.revision_id); return item ? pinnedResourceOptionKey(item) : ""; })()} onChange={event => { const optionKey = event.currentTarget.value; const item = resources.find(resource => resource.workspaceId === workspaceId && pinnedResourceOptionKey(resource) === optionKey); setRunInputs(current => ({ ...current, [field.name]: item ? { workspace_id: workspaceId, resource_id: item.resourceId, revision_id: item.resourceRevisionId } : null })); }}><option value="">{field.required ? "Choose a Resource revision…" : "No Resource"}</option>{resources.filter(resource => resource.workspaceId === workspaceId).map(resource => <option key={pinnedResourceOptionKey(resource)} value={pinnedResourceOptionKey(resource)}>{resource.displayName} · {resource.mediaType} · {resource.resourceRevisionId}</option>)}</select><small>The exact Workspace, Resource, and immutable revision are pinned to the Task.</small></label>)}
                {routineInputFields(pinnedRoutineRevision).fields.some(field => field.kind === "RESOURCE_REF") && resourcesNextCursor && <button type="button" className="text-button" disabled={resourcesPageBusy} onClick={() => void onLoadMoreResources()}>{resourcesPageBusy ? "Loading Resources…" : "Load more Resources"}</button>}
                {routineInputFields(pinnedRoutineRevision).blockedReason && <p className="automation-inline-error" role="note">Run unavailable: {routineInputFields(pinnedRoutineRevision).blockedReason}</p>}
                {pinnedRoutine.status !== "ACTIVE" && <p className="automation-inline-error" role="note">Run unavailable: the pinned Routine is archived.</p>}
                {selected.status === "DISABLED" && <p className="automation-inline-error" role="note">Run unavailable: this Automation is disabled.</p>}
                {runResult && runResult.automation_id === selected.automation_id && <div className="automation-run-result" role="status"><strong>Saved Task · READY</strong><span>{runResult.task_id} · Automation revision {runResult.automation_revision} · Routine revision {runResult.routine_revision}</span><p>Planning and agent execution have not started.</p>{onOpenTask && <button type="button" className="quiet-button" onClick={() => onOpenTask(runResult.task_id)}>Open Task in Work</button>}</div>}
                <button type="button" className="primary-button" disabled={busy || selected.status === "DISABLED" || pinnedRoutine.status !== "ACTIVE" || Boolean(routineInputFields(pinnedRoutineRevision).blockedReason) || selectedPendingRuns.length > 0} onClick={() => void runNow()}>{busy ? "Saving Task…" : selectedPendingRuns.length > 0 ? "Resolve previous Run first" : "Run once"}</button>
              </section>}
              {confirmDisable && <div className="automation-confirm" role="group" aria-label="Confirm permanent disable"><p>Disable this definition permanently? This keeps the record but cannot be undone from this screen.</p><button type="button" className="quiet-button" disabled={busy} onClick={() => setConfirmDisable(false)}>Keep it</button><button type="button" className="danger-button" disabled={busy} onClick={() => void transition("disable")}>{busy ? "Disabling…" : "Confirm disable"}</button></div>}
              {selected.status !== "DISABLED" && <div className="automation-actions">
                {selected.status === "PAUSED" && <button className="quiet-button" type="button" disabled={busy || revisionLoading} onClick={() => void startRevision()}>{revisionLoading ? "Loading pinned revision…" : "Edit definition"}</button>}
                {selected.status === "ENABLED" && <button className="quiet-button" type="button" disabled={busy} onClick={() => void transition("pause")}>{busy ? "Pausing…" : "Pause definition"}</button>}
                {!confirmDisable && <button className="danger-button" type="button" disabled={busy} onClick={() => { setActionError(null); setConfirmDisable(true); }}>Disable permanently</button>}
              </div>}
              {selected.status === "ENABLED" && <p className="automation-policy-note">Pause this stored definition before revising it. Recurring trigger hosting is not available in this desktop build.</p>}
              <p className="automation-provenance">Created {dateText(selected.created_at)} · {selected.automation_id}</p>
            </> : <div className="automation-empty"><p>Select a definition to review its stored state.</p></div>}
          </section>
        </div>
      )}
      {listError && items.length > 0 && <p className="automation-inline-error" role="alert">{listError}</p>}
    </div>
  );
}
