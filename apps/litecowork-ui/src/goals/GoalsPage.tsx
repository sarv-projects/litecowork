import { useEffect, useRef, useState } from "react";
import type { ArtifactVersionRef, Goal, GoalApi, GoalArtifactOption, GoalRevisionInput, GoalStatus, GoalTaskOption, RoutineRevisionRef } from "./goal-api";
import "./goals-page.css";

type Props = {
  api: GoalApi;
  workspaceId: string;
  coworkerIds?: { id: string; name: string }[];
  onOpenTask: (taskId: string) => void;
};
type Mode = "view" | "create" | "edit";
type GoalDraft = {
  objective: string; successCriteria: string; constraints: string; horizon: string; coworkerId: string;
  relatedTaskIds: string[]; relatedRoutineRefs: RoutineRevisionRef[];
  relatedArtifactRefs: ArtifactVersionRef[];
};
type GoalMutation =
  | { kind: "CREATE"; coworkerId: string | null; revision: GoalRevisionInput }
  | { kind: "REVISE"; goalId: string; expectedVersion: number; revision: GoalRevisionInput }
  | { kind: "STATUS"; goalId: string; expectedVersion: number; status: GoalStatus };
type PendingGoalMutation = {
  key: string;
  workspaceId: string;
  requestId: string;
  signature: string;
  mutation: GoalMutation;
  createdAt: string;
};
const MAX_PENDING_GOAL_MUTATIONS = 32;
const pendingGoalMutations = new Map<string, PendingGoalMutation>();
const pendingGoalMutationListeners = new Set<() => void>();
function publishPendingGoalMutationChange(): void {
  for (const listener of pendingGoalMutationListeners) listener();
}
function subscribePendingGoalMutations(listener: () => void): () => void {
  pendingGoalMutationListeners.add(listener);
  return () => pendingGoalMutationListeners.delete(listener);
}
function pendingForWorkspace(workspaceId: string): PendingGoalMutation[] {
  return [...pendingGoalMutations.values()].filter(item => item.workspaceId === workspaceId);
}
function cloneMutation(mutation: GoalMutation): GoalMutation {
  return JSON.parse(JSON.stringify(mutation)) as GoalMutation;
}
function mutationKey(workspaceId: string, mutation: GoalMutation): string {
  const target = mutation.kind === "CREATE" ? "new" : mutation.goalId;
  return JSON.stringify([workspaceId, mutation.kind === "STATUS" ? "status" : "revision", target]);
}
function freshRequestId(): string {
  const randomUUID = globalThis.crypto?.randomUUID;
  if (!randomUUID) throw new Error("This desktop session cannot create secure request identities. Restart LiteCowork before changing a Goal.");
  return randomUUID.call(globalThis.crypto);
}
function registerPendingMutation(workspaceId: string, input: GoalMutation): PendingGoalMutation {
  const mutation = cloneMutation(input);
  const key = mutationKey(workspaceId, mutation);
  const signature = JSON.stringify([workspaceId, mutation]);
  const existing = pendingGoalMutations.get(key);
  if (existing) {
    if (existing.signature !== signature) throw new Error("This Goal has an unresolved change. Retry the exact saved request or explicitly discard it before making a different change.");
    return existing;
  }
  if (pendingGoalMutations.size >= MAX_PENDING_GOAL_MUTATIONS) {
    throw new Error("Too many Goal changes have unresolved responses. Resolve or explicitly discard one before starting another.");
  }
  const pending = { key, workspaceId, requestId: freshRequestId(), signature, mutation, createdAt: new Date().toISOString() };
  pendingGoalMutations.set(key, pending);
  publishPendingGoalMutationChange();
  return pending;
}
function forgetPendingMutation(pending: PendingGoalMutation): boolean {
  if (pendingGoalMutations.get(pending.key)?.requestId !== pending.requestId) return false;
  pendingGoalMutations.delete(pending.key);
  publishPendingGoalMutationChange();
  return true;
}
function validateMutationReceipt(goal: Goal, pending: PendingGoalMutation): void {
  if (goal.workspace_id !== pending.workspaceId) throw new Error("The Goal response belongs to a different Workspace; the original request remains saved for retry.");
  const mutation = pending.mutation;
  if (mutation.kind === "CREATE") {
    if (goal.coworker_id !== mutation.coworkerId || goal.status !== "ACTIVE" || JSON.stringify(goal.revision) !== JSON.stringify(mutation.revision)) {
      throw new Error("The created Goal response does not match the saved request; the original request remains saved for retry.");
    }
  } else if (goal.goal_id !== mutation.goalId) {
    throw new Error("The Goal response identity does not match the saved request; the original request remains saved for retry.");
  } else if (mutation.kind === "REVISE" && JSON.stringify(goal.revision) !== JSON.stringify(mutation.revision)) {
    throw new Error("The revised Goal response does not match the saved request; the original request remains saved for retry.");
  } else if (mutation.kind === "STATUS" && goal.status !== mutation.status) {
    throw new Error("The Goal status response does not match the saved request; the original request remains saved for retry.");
  }
}
const STATUS_LABEL: Record<GoalStatus, string> = { ACTIVE: "Active", PAUSED: "Paused", COMPLETED: "Completed", ARCHIVED: "Archived" };

function draftFromGoal(goal: Goal): GoalDraft {
  const date = goal.revision.horizon ? new Date(goal.revision.horizon) : null;
  const localHorizon = date && !Number.isNaN(date.getTime())
    ? new Date(date.getTime() - date.getTimezoneOffset() * 60_000).toISOString().slice(0, 16)
    : "";
  return {
    objective: goal.revision.objective,
    successCriteria: goal.revision.success_criteria.join("\n"),
    constraints: goal.revision.constraints.join("\n"),
    horizon: localHorizon,
    coworkerId: goal.coworker_id ?? "",
    relatedTaskIds: [...goal.revision.related_task_ids],
    relatedRoutineRefs: goal.revision.related_routine_refs.map(ref => ({ ...ref })),
    relatedArtifactRefs: goal.revision.related_artifact_refs.map(ref => ({ ...ref })),
  };
}
function splitLines(value: string): string[] { return value.split("\n").map(line => line.trim()).filter(Boolean); }
function makeRevision(draft: GoalDraft): GoalRevisionInput {
  const parsedHorizon = draft.horizon.trim() ? new Date(draft.horizon) : null;
  return {
    objective: draft.objective.trim(),
    success_criteria: splitLines(draft.successCriteria),
    constraints: splitLines(draft.constraints),
    horizon: parsedHorizon && !Number.isNaN(parsedHorizon.getTime()) ? parsedHorizon.toISOString() : null,
    related_task_ids: [...draft.relatedTaskIds],
    related_routine_refs: draft.relatedRoutineRefs.map(ref => ({ ...ref })),
    related_artifact_refs: draft.relatedArtifactRefs.map(ref => ({ ...ref })),
  };
}
function errorText(error: unknown): string {
  return error instanceof Error ? error.message : "The Goal request could not be completed.";
}
function statusClass(status: GoalStatus): string { return `goal-status goal-status-${status.toLowerCase()}`; }

/** Goals describe durable intent and organize linked work; they never execute Tasks. */
export function GoalsPage({ api, workspaceId, coworkerIds = [], onOpenTask }: Props) {
  const [items, setItems] = useState<Goal[]>([]);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [pageBusy, setPageBusy] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [selected, setSelected] = useState<Goal | null>(null);
  const [loading, setLoading] = useState(true);
  const [detailLoading, setDetailLoading] = useState(false);
  const [listFailure, setListFailure] = useState<string | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [mode, setMode] = useState<Mode>("view");
  const [draft, setDraft] = useState<GoalDraft | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [confirmArchive, setConfirmArchive] = useState(false);
  const [taskOptions, setTaskOptions] = useState<GoalTaskOption[]>([]);
  const [taskCursor, setTaskCursor] = useState<string | null>(null);
  const [artifactOptions, setArtifactOptions] = useState<GoalArtifactOption[]>([]);
  const [artifactCursor, setArtifactCursor] = useState<string | null>(null);
  const [catalogBusy, setCatalogBusy] = useState(false);
  const [catalogFailure, setCatalogFailure] = useState<string | null>(null);
  const [activityAnnouncement, setActivityAnnouncement] = useState("");
  const [pendingMutations, setPendingMutations] = useState<PendingGoalMutation[]>(() => pendingForWorkspace(workspaceId));
  const [discardingKey, setDiscardingKey] = useState<string | null>(null);
  const [reload, setReload] = useState(0);
  const generation = useRef(0);
  const visiblePendingMutations = pendingMutations.filter(item => item.workspaceId === workspaceId);

  useEffect(() => {
    const update = () => setPendingMutations(pendingForWorkspace(workspaceId));
    update();
    return subscribePendingGoalMutations(update);
  }, [workspaceId]);

  useEffect(() => {
    const controller = new AbortController();
    const activeGeneration = ++generation.current;
    setItems([]); setNextCursor(null); setSelected(null); setSelectedId(null); setDraft(null); setMode("view");
    setLoading(Boolean(workspaceId)); setListFailure(null); setFailure(null); setMessage(null); setConfirmArchive(false);
    setActivityAnnouncement("");
    if (!workspaceId) { setLoading(false); return () => controller.abort(); }
    void api.list(undefined, controller.signal).then(page => {
      if (controller.signal.aborted || activeGeneration !== generation.current) return;
      if (page.items.some(item => item.workspace_id !== workspaceId)) throw new Error("Goal list belongs to a different Workspace.");
      setItems(page.items);
      setNextCursor(page.next_cursor);
      setSelectedId(page.items[0]?.goal_id ?? null);
      setActivityAnnouncement(`${page.items.length} ${page.items.length === 1 ? "Goal" : "Goals"} loaded.`);
    }).catch(error => {
      if (!controller.signal.aborted && activeGeneration === generation.current) setListFailure(errorText(error));
    }).finally(() => {
      if (!controller.signal.aborted && activeGeneration === generation.current) setLoading(false);
    });
    return () => controller.abort();
  }, [api, workspaceId, reload]);

  useEffect(() => {
    const controller = new AbortController();
    setCatalogBusy(Boolean(workspaceId));
    setCatalogFailure(null);
    setTaskOptions([]); setTaskCursor(null); setArtifactOptions([]); setArtifactCursor(null);
    if (!workspaceId) { setCatalogBusy(false); return () => controller.abort(); }
    void Promise.all([
      api.listRelatedTasks(undefined, controller.signal),
      api.listRelatedArtifacts(undefined, controller.signal),
    ]).then(([tasks, artifacts]) => {
      if (controller.signal.aborted) return;
      setTaskOptions(tasks.items); setTaskCursor(tasks.next_cursor);
      setArtifactOptions(artifacts.items); setArtifactCursor(artifacts.next_cursor);
    }).catch(error => {
      if (!controller.signal.aborted) setCatalogFailure(errorText(error));
    }).finally(() => { if (!controller.signal.aborted) setCatalogBusy(false); });
    return () => controller.abort();
  }, [api, workspaceId]);

  useEffect(() => {
    if (!selectedId || mode === "create") { setSelected(null); setDetailLoading(false); return; }
    const controller = new AbortController();
    const activeGeneration = generation.current;
    setSelected(null); setFailure(null);
    setDetailLoading(true);
    setActivityAnnouncement("Loading Goal details.");
    void api.get(selectedId, controller.signal).then(found => {
      if (controller.signal.aborted || activeGeneration !== generation.current) return;
      if (found.goal_id !== selectedId) throw new Error("Goal detail identity mismatch.");
      if (found.workspace_id !== workspaceId) throw new Error("Goal detail belongs to a different Workspace.");
      setSelected(found);
      setItems(current => current.map(item => item.goal_id === found.goal_id ? found : item));
      setActivityAnnouncement("Goal details loaded.");
    }).catch(error => {
      if (!controller.signal.aborted && activeGeneration === generation.current) {
        setFailure(errorText(error));
      }
    }).finally(() => { if (!controller.signal.aborted && activeGeneration === generation.current) setDetailLoading(false); });
    return () => controller.abort();
  }, [api, workspaceId, selectedId, mode, reload]);

  async function sendMutation(pending: PendingGoalMutation): Promise<Goal> {
    if (pending.workspaceId !== workspaceId) throw new Error("This saved Goal change belongs to another Workspace. Switch back to retry it.");
    const mutation = cloneMutation(pending.mutation);
    const goal = mutation.kind === "CREATE"
      ? await api.create(mutation.coworkerId, mutation.revision, pending.requestId)
      : mutation.kind === "REVISE"
        ? await api.revise(mutation.goalId, mutation.expectedVersion, mutation.revision, pending.requestId)
        : await api.changeStatus(mutation.goalId, mutation.expectedVersion, mutation.status, pending.requestId);
    validateMutationReceipt(goal, pending);
    return goal;
  }
  function applyMutationReceipt(goal: Goal, pending: PendingGoalMutation): void {
    // A late response after the owner explicitly discarded this envelope must not
    // masquerade as resolving a newer request for the same Goal.
    if (!forgetPendingMutation(pending)) return;
    setItems(current => pending.mutation.kind === "CREATE"
      ? [goal, ...current.filter(item => item.goal_id !== goal.goal_id)]
      : current.map(item => item.goal_id === goal.goal_id ? goal : item));
    setSelectedId(goal.goal_id); setSelected(goal); setMode("view"); setDraft(null);
    setMessage(pending.mutation.kind === "CREATE" ? "Goal saved. No Task was started." : pending.mutation.kind === "REVISE" ? "Goal details updated." : pending.mutation.status === "COMPLETED" ? "Goal marked complete by you." : `Goal is now ${STATUS_LABEL[pending.mutation.status].toLowerCase()}.`);
    setConfirmArchive(false);
  }
  async function retryPendingMutation(pending: PendingGoalMutation) {
    if (pending.workspaceId !== workspaceId || busy) return;
    setBusy(true); setFailure(null); setMessage(null);
    try {
      const goal = await sendMutation(pending);
      applyMutationReceipt(goal, pending);
    } catch (error) {
      setFailure(errorText(error));
    } finally { setBusy(false); }
  }
  async function loadRelatedCatalog(append = false) {
    if (catalogBusy) return;
    setCatalogBusy(true); setCatalogFailure(null);
    try {
      const [tasks, artifacts] = await Promise.all([
        api.listRelatedTasks(append ? taskCursor ?? undefined : undefined),
        api.listRelatedArtifacts(append ? artifactCursor ?? undefined : undefined),
      ]);
      setTaskOptions(current => append ? [...current, ...tasks.items.filter(item => !current.some(old => old.task_id === item.task_id))] : tasks.items);
      setArtifactOptions(current => append ? [...current, ...artifacts.items.filter(item => !current.some(old => old.artifact_id === item.artifact_id))] : artifacts.items);
      setTaskCursor(tasks.next_cursor); setArtifactCursor(artifacts.next_cursor);
    } catch (error) { setCatalogFailure(errorText(error)); }
    finally { setCatalogBusy(false); }
  }
  function beginCreate() {
    setSelectedId(null); setSelected(null); setMode("create"); setConfirmArchive(false); setFailure(null); setMessage(null);
    setDraft({ objective: "", successCriteria: "", constraints: "", horizon: "", coworkerId: "", relatedTaskIds: [], relatedRoutineRefs: [], relatedArtifactRefs: [] });
  }
  function beginEdit() {
    if (!selected || selected.status === "ARCHIVED") return;
    setDraft(draftFromGoal(selected)); setMode("edit"); setFailure(null); setMessage(null); setConfirmArchive(false);
  }
  function cancelEdit() { setMode("view"); setDraft(null); setMessage(null); setFailure(null); }

  async function save() {
    if (!draft || !workspaceId || busy) return;
    const revision = makeRevision(draft);
    if (!revision.objective) { setMessage("Add an objective before saving."); return; }
    if (revision.success_criteria.length === 0) { setMessage("Add at least one success criterion so this Goal has a clear outcome."); return; }
    if (draft.horizon.trim() && Number.isNaN(Date.parse(draft.horizon))) { setMessage("Enter a valid date and time for the target horizon."); return; }
    const mutation: GoalMutation | null = mode === "create"
      ? { kind: "CREATE", coworkerId: draft.coworkerId || null, revision }
      : selected ? { kind: "REVISE", goalId: selected.goal_id, expectedVersion: selected.version, revision } : null;
    if (!mutation) return;
    let pending: PendingGoalMutation;
    try { pending = registerPendingMutation(workspaceId, mutation); } catch (error) { setFailure(errorText(error)); return; }
    setBusy(true); setFailure(null); setMessage(null);
    try {
      const saved = await sendMutation(pending);
      applyMutationReceipt(saved, pending);
    } catch (error) {
      setFailure(errorText(error));
      if (/changed elsewhere|reload the latest/i.test(errorText(error))) setMessage("Reload to review the current Goal before retrying.");
    } finally { setBusy(false); }
  }

  async function changeStatus(status: GoalStatus) {
    if (!selected || busy || selected.status === status) return;
    let pending: PendingGoalMutation;
    try { pending = registerPendingMutation(workspaceId, { kind: "STATUS", goalId: selected.goal_id, expectedVersion: selected.version, status }); }
    catch (error) { setFailure(errorText(error)); return; }
    setBusy(true); setFailure(null); setMessage(null);
    try {
      const changed = await sendMutation(pending);
      applyMutationReceipt(changed, pending);
    } catch (error) { setFailure(errorText(error)); }
    finally { setBusy(false); }
  }

  function retryLoad() { setReload(value => value + 1); }

  async function loadMore() {
    if (!nextCursor || pageBusy) return;
    const activeGeneration = generation.current;
    setPageBusy(true);
    try {
      const page = await api.list(nextCursor);
      if (activeGeneration !== generation.current || page.items.some(item => item.workspace_id !== workspaceId)) return;
      const existing = new Set(items.map(item => item.goal_id));
      const additions = page.items.filter(item => !existing.has(item.goal_id));
      setItems(current => {
        const currentIds = new Set(current.map(item => item.goal_id));
        return [...current, ...additions.filter(item => !currentIds.has(item.goal_id))];
      });
      setNextCursor(page.next_cursor);
      setActivityAnnouncement(`${additions.length} more ${additions.length === 1 ? "Goal" : "Goals"} loaded. ${items.length + additions.length} shown.`);
    } catch (error) {
      if (activeGeneration === generation.current) setListFailure(errorText(error));
    } finally { if (activeGeneration === generation.current) setPageBusy(false); }
  }

  return (
    <div className="goals-page">
      <p className="sr-only" role="status" aria-live="polite" aria-atomic="true">{activityAnnouncement}</p>
      <header className="goals-heading">
        <div><p className="goals-kicker">WORKSPACE</p><h1>Goals</h1><p>Keep longer-term outcomes visible and connect them to work already underway.</p></div>
        <button className="goal-primary-action" type="button" disabled={!workspaceId || loading} onClick={beginCreate}>New Goal</button>
      </header>
      {workspaceId && visiblePendingMutations.length > 0 && <section className="goal-pending-mutations" aria-label="Unconfirmed Goal changes">
        <h2>Goal changes awaiting confirmation</h2>
        <p>A previous response was not confirmed. Retry sends the exact saved change with its original request identity. These records last only while LiteCowork stays open.</p>
        <ul>{visiblePendingMutations.map(pending => <li key={pending.key}>
          <div><strong>{pending.mutation.kind === "CREATE" ? pending.mutation.revision.objective : pending.mutation.kind === "REVISE" ? `Edit Goal ${pending.mutation.goalId}` : `${STATUS_LABEL[pending.mutation.status]} Goal ${pending.mutation.goalId}`}</strong>
            <small>{pending.mutation.kind === "CREATE" ? "Create Goal" : pending.mutation.kind === "REVISE" ? `Revision based on version ${pending.mutation.expectedVersion}` : `Status change based on version ${pending.mutation.expectedVersion}`} · saved {new Date(pending.createdAt).toLocaleString()}</small>
          </div>
          <div className="goal-pending-actions"><button className="goal-secondary-action" type="button" disabled={busy} onClick={() => void retryPendingMutation(pending)}>Retry exact change</button>
            {discardingKey !== pending.key
              ? <button className="goal-text-action" type="button" disabled={busy} onClick={() => setDiscardingKey(pending.key)}>Discard saved change…</button>
              : <div className="goal-pending-discard"><p>Discarding forgets this retry identity only. The change may already have committed; a new create or revision could duplicate it, and a new status change could supersede it.</p><button className="goal-secondary-action" type="button" disabled={busy} onClick={() => setDiscardingKey(null)}>Keep retry</button><button className="goal-danger-action" type="button" disabled={busy} onClick={() => { forgetPendingMutation(pending); setDiscardingKey(null); }}>Discard retry identity</button></div>}
          </div>
        </li>)}</ul>
      </section>}
      {!workspaceId && <section className="goal-state"><h2>Select a Workspace</h2><p>Goals belong to a Workspace and are only shown after one is selected.</p></section>}
      {workspaceId && loading && <section className="goal-state" role="status"><span className="goal-spinner" aria-hidden="true" /><p>Loading Goals…</p></section>}
      {workspaceId && !loading && listFailure && <section className="goal-state goal-state-error" role="alert"><h2>Goals couldn’t be loaded</h2><p>{listFailure}</p><button className="goal-secondary-action" type="button" onClick={retryLoad}>Reload Goals</button></section>}
      {workspaceId && !loading && !listFailure && items.length === 0 && mode !== "create" && (
        <section className="goal-state"><div className="goal-empty-mark" aria-hidden="true">◎</div><h2>No Goals yet</h2><p>Add a Goal to describe what you want to achieve. It organizes related work; it never starts Tasks by itself.</p><button className="goal-primary-action" type="button" onClick={beginCreate}>Create your first Goal</button></section>
      )}
      {workspaceId && !loading && !listFailure && (items.length > 0 || mode === "create") && (
        <div className="goals-layout">
          {items.length > 0 && <nav className="goals-roster" aria-label="Goals in this Workspace" aria-busy={pageBusy}>
            <div className="goals-roster-heading"><h2>Your Goals</h2><span>{items.length}</span></div>
            <ul>{items.map(item => <li key={item.goal_id}><button type="button" className={`goals-roster-item${selectedId === item.goal_id ? " is-selected" : ""}`} aria-current={selectedId === item.goal_id ? "page" : undefined} onClick={() => { setSelectedId(item.goal_id); setMode("view"); setDraft(null); setMessage(null); setFailure(null); setConfirmArchive(false); }}>
              <strong>{item.revision.objective}</strong><span className={statusClass(item.status)}>{STATUS_LABEL[item.status]}</span>
            </button></li>)}</ul>
            {nextCursor && <button className="goal-load-more" type="button" disabled={pageBusy} onClick={() => void loadMore()}>{pageBusy ? "Loading…" : "Load more Goals"}</button>}
          </nav>}
          <section className="goal-detail" aria-label={mode === "create" ? "Create Goal" : mode === "edit" ? "Edit Goal" : "Goal details"} aria-busy={detailLoading || busy}>
            {mode === "create" || mode === "edit" ? (
              <GoalEditor mode={mode} draft={draft!} coworkerIds={coworkerIds} taskOptions={taskOptions} artifactOptions={artifactOptions} catalogBusy={catalogBusy} catalogFailure={catalogFailure} hasMoreCatalog={Boolean(taskCursor || artifactCursor)} onLoadMoreCatalog={() => void loadRelatedCatalog(true)} busy={busy} message={message} failure={failure} onChange={setDraft} onSave={() => void save()} onCancel={cancelEdit} />
            ) : detailLoading ? <div className="goal-detail-loading"><span className="goal-spinner" aria-hidden="true" /><p>Loading Goal…</p></div> : selected ? <>
              <div className="goal-detail-heading"><div><span className={statusClass(selected.status)}>{STATUS_LABEL[selected.status]}</span><h2>{selected.revision.objective}</h2><p>Updated {new Date(selected.updated_at).toLocaleString()}</p></div>
                {selected.status !== "ARCHIVED" && <button className="goal-secondary-action" type="button" disabled={busy} onClick={beginEdit}>Edit Goal</button>}
              </div>
              {message && <p className="goal-notice" role="status">{message}</p>}{failure && <p className="goal-error" role="alert">{failure}<button type="button" onClick={retryLoad}>Reload current state</button></p>}
              <section className="goal-detail-section"><h3>Success criteria</h3><ul className="goal-criteria">{selected.revision.success_criteria.map((criterion, index) => <li key={`${index}-${criterion}`}><span aria-hidden="true">○</span>{criterion}</li>)}</ul></section>
              {selected.revision.constraints.length > 0 && <section className="goal-detail-section"><h3>Constraints</h3><ul>{selected.revision.constraints.map((constraint, index) => <li key={`${index}-${constraint}`}>{constraint}</li>)}</ul></section>}
              {selected.revision.horizon && <section className="goal-detail-section"><h3>Target horizon</h3><p>{new Date(selected.revision.horizon).toLocaleString()}</p></section>}
              {selected.revision.related_task_ids.length > 0 && <section className="goal-detail-section"><h3>Related Tasks</h3><ul className="goal-linked-tasks">{selected.revision.related_task_ids.map(id => {
                const task = taskOptions.find(option => option.task_id === id);
                return <li key={id}><button className="goal-linked-task-open" type="button" onClick={() => onOpenTask(id)} aria-label={`Open linked Task ${task?.objective ?? id}`}>
                  <span className="goal-linked-task-title">{task?.objective ?? `Task ${id}`}</span>
                  <span className="goal-linked-task-meta">{task ? task.status.toLowerCase().replaceAll("_", " ") : "Open linked work"} <span aria-hidden="true">→</span></span>
                </button></li>;
              })}</ul><p className="goal-muted">Opening a linked Task only shows its current Work details. The Goal does not start or revise it.</p></section>}
              {selected.revision.related_artifact_refs.length > 0 && <section className="goal-detail-section"><h3>Related Artifacts</h3><ul className="goal-linked-tasks">{selected.revision.related_artifact_refs.map(ref => <li key={`${ref.artifact_id}:${ref.version}`}><span>{artifactOptions.find(item => item.artifact_id === ref.artifact_id)?.display_name ?? ref.artifact_id}</span><span>Version {ref.version}</span></li>)}</ul></section>}
              {selected.revision.related_routine_refs.length > 0 && <section className="goal-detail-section"><h3>Related routines</h3><ul className="goal-linked-tasks">{selected.revision.related_routine_refs.map(ref => <li key={`${ref.routine_id}:${ref.revision}`}><span>{ref.routine_id}</span><span>Revision {ref.revision}</span></li>)}</ul></section>}
              <section className="goal-detail-section goal-progress-section"><h3>Progress from linked work</h3>{selected.progress
                ? <><p>{selected.progress.summary}</p>{selected.progress.availability === "PARTIAL" && <small>Partial projection · counts marked unavailable are not inferred as zero.</small>}<small>Projection computed {new Date(selected.progress.computed_at).toLocaleString()}</small>{selected.progress.limitations.length > 0 && <ul className="goal-progress-limitations">{selected.progress.limitations.map(code => <li key={code}>{({
                  VERIFICATION_RUN_READ_MODEL_UNAVAILABLE: "Verified outcomes are not yet available.",
                  TASK_DEPENDENCY_FRESHNESS_UNAVAILABLE: "Task input freshness is not yet available.",
                  ARTIFACT_DEPENDENCY_FRESHNESS_UNAVAILABLE: "Artifact source freshness is not yet available.",
                  ARTIFACT_EVIDENCE_REFERENCE_UNRESOLVED: "An Artifact cites Evidence that is not available in this Workspace.",
                  EVIDENCE_LIST_TRUNCATED: "Some Evidence references are omitted from this view.",
                } as const)[code]}</li>)}</ul>}{selected.progress.contributions.length > 0 && <ul className="goal-contributions">{selected.progress.contributions.map(item => <li key={item.task_id}><span>{item.task_id}</span><span>{item.outcome_state === "VERIFIED" ? "Verified outcome" : item.outcome_state.toLocaleLowerCase().replaceAll("_", " ")}{item.evidence_refs.length > 0 ? ` · ${item.evidence_refs.length} Evidence references` : ""}</span></li>)}</ul>}{selected.progress.artifact_evidence_refs.length > 0 && <ul className="goal-contributions">{selected.progress.artifact_evidence_refs.map(item => <li key={`${item.artifact_id}:${item.version}`}><span>{artifactOptions.find(option => option.artifact_id === item.artifact_id)?.display_name ?? item.artifact_id} · v{item.version}</span><span>{item.evidence_refs.length} committed Evidence references</span></li>)}</ul>}</>
                : <p className="goal-muted">Progress is unavailable because this Runtime has not supplied a Task and Evidence projection. No percentage or completion is inferred.</p>}
              </section>
              {selected.status !== "ARCHIVED" && <footer className="goal-lifecycle">
                {selected.status === "ACTIVE" && <button className="goal-secondary-action" type="button" disabled={busy} onClick={() => void changeStatus("PAUSED")}>Pause</button>}
                {selected.status === "PAUSED" && <button className="goal-secondary-action" type="button" disabled={busy} onClick={() => void changeStatus("ACTIVE")}>Resume</button>}
                {selected.status !== "COMPLETED" && <button className="goal-secondary-action" type="button" disabled={busy} onClick={() => void changeStatus("COMPLETED")}>Mark complete</button>}
                {!confirmArchive ? <button className="goal-text-action" type="button" disabled={busy} onClick={() => setConfirmArchive(true)}>Archive</button>
                  : <div className="goal-archive-confirm"><p>Archive this Goal? This hides it from active Goals; linked Tasks are unchanged.</p><button className="goal-secondary-action" type="button" disabled={busy} onClick={() => setConfirmArchive(false)}>Keep Goal</button><button className="goal-danger-action" type="button" disabled={busy} onClick={() => void changeStatus("ARCHIVED")}>Archive Goal</button></div>}
              </footer>}
            </> : failure ? <p className="goal-error" role="alert">{failure}<button type="button" onClick={retryLoad}>Reload current state</button></p> : <div className="goal-state"><h2>Select a Goal</h2><p>Choose a Goal from the list to review its criteria and linked work.</p></div>}
          </section>
        </div>
      )}
    </div>
  );
}

function GoalEditor({ mode, draft, coworkerIds, taskOptions, artifactOptions, catalogBusy, catalogFailure, hasMoreCatalog, onLoadMoreCatalog, busy, message, failure, onChange, onSave, onCancel }: {
  mode: "create" | "edit"; draft: GoalDraft; coworkerIds: Props["coworkerIds"];
  taskOptions: GoalTaskOption[]; artifactOptions: GoalArtifactOption[]; catalogBusy: boolean; catalogFailure: string | null;
  hasMoreCatalog: boolean; onLoadMoreCatalog: () => void; busy: boolean;
  message: string | null; failure: string | null; onChange: (value: GoalDraft) => void; onSave: () => void; onCancel: () => void;
}) {
  const field = (name: keyof GoalDraft, value: string) => onChange({ ...draft, [name]: value });
  return <div className="goal-editor">
    <div><p className="goals-kicker">{mode === "create" ? "NEW GOAL" : "EDIT GOAL"}</p><h2>{mode === "create" ? "What do you want to achieve?" : "Update this Goal"}</h2><p>A Goal records intent. Saving it never creates or starts a Task.</p></div>
    {failure && <p className="goal-error" role="alert">{failure}</p>}{message && <p className="goal-notice" role="status">{message}</p>}
    <label className="goal-field">Objective<textarea autoFocus maxLength={4000} rows={3} value={draft.objective} onChange={event => field("objective", event.target.value)} placeholder="For example: Prepare LiteCowork for its first local beta" /></label>
    <label className="goal-field">How will you recognize success?<span>One criterion per line</span><textarea rows={4} value={draft.successCriteria} onChange={event => field("successCriteria", event.target.value)} placeholder="A verified desktop build is ready\nCore local workflows are documented" /></label>
    <label className="goal-field">Constraints <span>Optional · one per line</span><textarea rows={3} value={draft.constraints} onChange={event => field("constraints", event.target.value)} placeholder="Local-first; no external team dependencies" /></label>
    <label className="goal-field">Target horizon <span>Optional</span><input type="datetime-local" value={draft.horizon} onChange={event => field("horizon", event.target.value)} /></label>
    {coworkerIds && coworkerIds.length > 0 && <label className="goal-field">Coworker <span>Optional · this only groups the Goal</span><select value={draft.coworkerId} onChange={event => field("coworkerId", event.target.value)}><option value="">No Coworker</option>{coworkerIds.map(coworker => <option key={coworker.id} value={coworker.id}>{coworker.name}</option>)}</select></label>}
    <section className="goal-link-picker" aria-labelledby="goal-task-picker-heading">
      <div><h3 id="goal-task-picker-heading">Link existing Tasks</h3><p>Links organize this Goal; they never start or change a Task.</p></div>
      {taskOptions.length === 0 && !catalogBusy ? <p className="goal-muted">No Tasks are available in this Workspace.</p> : <ul>{taskOptions.map(task => <li key={task.task_id}><label><input type="checkbox" checked={draft.relatedTaskIds.includes(task.task_id)} onChange={event => onChange({ ...draft, relatedTaskIds: event.target.checked ? [...draft.relatedTaskIds, task.task_id] : draft.relatedTaskIds.filter(id => id !== task.task_id) })} /><span><strong>{task.objective}</strong><small>{task.status.toLowerCase().replaceAll("_", " ")} · {task.task_id}</small></span></label></li>)}</ul>}
    </section>
    <section className="goal-link-picker" aria-labelledby="goal-artifact-picker-heading">
      <div><h3 id="goal-artifact-picker-heading">Link Artifact versions</h3><p>Each link pins the selected Artifact’s current version.</p></div>
      {artifactOptions.length === 0 && !catalogBusy ? <p className="goal-muted">No Artifacts are available in this Workspace.</p> : <ul>{artifactOptions.map(artifact => {
        const linked = draft.relatedArtifactRefs.find(ref => ref.artifact_id === artifact.artifact_id);
        return <li key={artifact.artifact_id}><label><input type="checkbox" checked={Boolean(linked)} onChange={event => onChange({ ...draft, relatedArtifactRefs: event.target.checked
          ? [...draft.relatedArtifactRefs.filter(ref => ref.artifact_id !== artifact.artifact_id), { workspace_id: artifact.workspace_id, artifact_id: artifact.artifact_id, version: artifact.current_version }]
          : draft.relatedArtifactRefs.filter(ref => ref.artifact_id !== artifact.artifact_id) })} /><span><strong>{artifact.display_name}</strong><small>{artifact.kind} · current version {artifact.current_version}{linked && linked.version !== artifact.current_version ? ` · linked v${linked.version}` : ""}</small></span></label></li>;
      })}</ul>}
    </section>
    {catalogFailure && <p className="goal-error" role="alert">Workspace items could not be loaded: {catalogFailure}<button type="button" onClick={onLoadMoreCatalog}>Retry</button></p>}
    {catalogBusy && <p className="goal-muted" role="status">Loading Workspace items…</p>}
    {hasMoreCatalog && <button className="goal-load-more" type="button" disabled={catalogBusy} onClick={onLoadMoreCatalog}>Load more Tasks and Artifacts</button>}
    <div className="goal-editor-actions"><button className="goal-secondary-action" type="button" disabled={busy} onClick={onCancel}>Cancel</button><button className="goal-primary-action" type="button" disabled={busy} onClick={onSave}>{busy ? "Saving…" : mode === "create" ? "Save Goal" : "Save changes"}</button></div>
  </div>;
}
