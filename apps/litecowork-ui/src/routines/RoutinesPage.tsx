import { useEffect, useRef, useState } from "react";
import { RoutineApiError, type MaterializedRoutineTask, type Routine, type RoutineApi, type RoutineRevision } from "./routine-api";
import { pinnedResourceOptionKey, routineInputFields, type ResourceOption } from "./routine-inputs";
import "./routines-page.css";

type Props = { api: RoutineApi; workspaceId: string; resources: ResourceOption[]; resourcesNextCursor: string | null; resourcesPageBusy: boolean; onLoadMoreResources: () => Promise<void>; onOpenTask?: (task: MaterializedRoutineTask) => void };
type Editor = { routineId: string | null; version: number | null; name: string; objective: string; instructions: string; advanced: string };
type RoutineMutation =
  | { kind: "CREATE"; name: string; revision: Record<string, unknown> }
  | { kind: "REVISE"; routineId: string; expectedVersion: number; revision: Record<string, unknown> }
  | { kind: "ARCHIVE"; routineId: string; expectedVersion: number }
  | { kind: "RUN"; routineId: string; routineRevision: number; inputs: Record<string, unknown> };
type PendingRoutineMutation = {
  key: string;
  workspaceId: string;
  requestId: string;
  payload: RoutineMutation;
  bytes: number;
  activeCalls: number;
};
type RoutineMutationReceipt =
  | { kind: "CREATE"; value: Routine }
  | { kind: "REVISE"; value: RoutineRevision }
  | { kind: "ARCHIVE"; value: Routine }
  | { kind: "RUN"; value: MaterializedRoutineTask };

// A component ref is lost on route changes. Keep only unresolved exact commands in
// this process-local bounded registry; nothing is persisted to disk or localStorage.
const MAX_PENDING_MUTATIONS = 24;
const MAX_PENDING_MUTATION_BYTES = 1024 * 1024;
const pendingRoutineMutations = new Map<string, PendingRoutineMutation>();
const pendingRoutineMutationListeners = new Set<() => void>();
let pendingRoutineMutationBytes = 0;

function publishPendingRoutineMutations(): void {
  pendingRoutineMutationListeners.forEach(listener => listener());
}
function subscribePendingRoutineMutations(listener: () => void): () => void {
  pendingRoutineMutationListeners.add(listener);
  return () => pendingRoutineMutationListeners.delete(listener);
}
function pendingForWorkspace(workspaceId: string): PendingRoutineMutation[] {
  return [...pendingRoutineMutations.values()].filter(item => item.workspaceId === workspaceId);
}
function freezeJson<T>(value: T): T {
  if (value && typeof value === "object") {
    Object.values(value as Record<string, unknown>).forEach(child => freezeJson(child));
    Object.freeze(value);
  }
  return value;
}
function registerPendingMutation(workspaceId: string, payload: RoutineMutation): PendingRoutineMutation {
  // Clone through JSON because every Routine command is a JSON wire payload. This
  // freezes the exact values before dispatch and prevents later form edits changing it.
  const serialized = JSON.stringify(payload);
  const immutablePayload = freezeJson(JSON.parse(serialized) as RoutineMutation);
  const signature = JSON.stringify([workspaceId, serialized]);
  const key = `${workspaceId}\u0000${signature}`;
  const existing = pendingRoutineMutations.get(key);
  if (existing) return existing;
  if (pendingRoutineMutations.size >= MAX_PENDING_MUTATIONS || pendingRoutineMutationBytes >= MAX_PENDING_MUTATION_BYTES) {
    throw new Error("The in-memory Routine retry limit is full. Retry or explicitly discard an unresolved request before starting another.");
  }
  // Generate the identity only after capacity admission. Recompute the counted bytes
  // with the actual identity used below.
  const requestId = freshRequestId();
  const bytes = new TextEncoder().encode(JSON.stringify([workspaceId, requestId, immutablePayload, signature, key])).byteLength;
  if (pendingRoutineMutationBytes + bytes > MAX_PENDING_MUTATION_BYTES) {
    throw new Error("The in-memory Routine retry limit is full. Retry or explicitly discard an unresolved request before starting another.");
  }
  const entry: PendingRoutineMutation = { key, workspaceId, requestId, payload: immutablePayload, bytes, activeCalls: 0 };
  pendingRoutineMutations.set(key, entry);
  pendingRoutineMutationBytes += bytes;
  publishPendingRoutineMutations();
  return entry;
}
function removePendingMutation(entry: PendingRoutineMutation): void {
  if (pendingRoutineMutations.get(entry.key) !== entry) return;
  pendingRoutineMutations.delete(entry.key);
  pendingRoutineMutationBytes = Math.max(0, pendingRoutineMutationBytes - entry.bytes);
  publishPendingRoutineMutations();
}
function isDefinitiveRoutineRejection(error: unknown): boolean {
  // Only known local Operator status/code pairs prove pre-commit rejection. In
  // particular, retain timeouts, auth/ownership changes (which can be observed
  // after commit), rate limits, malformed intermediary responses, and all unknowns.
  if (!(error instanceof RoutineApiError)) return false;
  const contractRejections = new Set([
    "400:INVALID_ARGUMENT",
    "409:CONFLICT", "409:ROUTINE_ARCHIVED", "409:ROUTINE_ARCHIVE_BLOCKED", "409:WORKSPACE_ARCHIVED",
    "413:INVALID_ARGUMENT",
    "422:INVALID_ARGUMENT", "422:AGENT_UNAVAILABLE",
  ]);
  return contractRejections.has(`${error.status}:${error.code}`);
}
function canonicalJson(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  if (value && typeof value === "object") {
    const row = value as Record<string, unknown>;
    return `{${Object.keys(row).sort().map(key => `${JSON.stringify(key)}:${canonicalJson(row[key])}`).join(",")}}`;
  }
  return JSON.stringify(value) ?? "null";
}
function validateRoutineMutationReceipt(entry: PendingRoutineMutation, receipt: RoutineMutationReceipt): void {
  if (entry.payload.kind !== receipt.kind) throw new Error("The Routine receipt does not match the submitted operation.");
  switch (receipt.kind) {
    case "CREATE":
      if (entry.payload.kind !== "CREATE" || receipt.value.workspace_id !== entry.workspaceId || receipt.value.name !== entry.payload.name) {
        throw new Error("The created Routine receipt does not match the submitted Workspace and name.");
      }
      return;
    case "REVISE": {
      if (entry.payload.kind !== "REVISE" || receipt.value.routine_id !== entry.payload.routineId
        || receipt.value.revision !== entry.payload.expectedVersion + 1) {
        throw new Error("The Routine revision receipt does not match the submitted Routine and expected revision.");
      }
      const returnedDefinition = { ...receipt.value } as Record<string, unknown>;
      delete returnedDefinition.routine_id; delete returnedDefinition.revision;
      delete returnedDefinition.authored_by; delete returnedDefinition.created_at;
      if (canonicalJson(returnedDefinition) !== canonicalJson(entry.payload.revision)) {
        throw new Error("The Routine revision receipt does not match the submitted definition payload.");
      }
      return;
    }
    case "ARCHIVE":
      if (entry.payload.kind !== "ARCHIVE" || receipt.value.workspace_id !== entry.workspaceId
        || receipt.value.routine_id !== entry.payload.routineId || receipt.value.status !== "ARCHIVED"
        || receipt.value.version !== entry.payload.expectedVersion + 1) {
        throw new Error("The archived Routine receipt does not match the submitted Workspace, Routine, and status.");
      }
      return;
    case "RUN":
      if (entry.payload.kind !== "RUN" || receipt.value.workspace_id !== entry.workspaceId
        || receipt.value.routine_id !== entry.payload.routineId || receipt.value.routine_revision !== entry.payload.routineRevision
        || receipt.value.status !== "READY" || !receipt.value.task_id || receipt.value.current_spec_revision < 1) {
        throw new Error("The saved Task receipt does not match the submitted Workspace, Routine revision, and READY status.");
      }
      return;
  }
}
async function executePendingMutation(api: RoutineApi, entry: PendingRoutineMutation): Promise<RoutineMutationReceipt> {
  // Same-process retries may overlap if an earlier page/request hung during a route
  // change. They carry the identical idempotency key and immutable command. A local
  // component busy guard prevents repeated clicks in one mounted view.
  entry.activeCalls += 1;
  publishPendingRoutineMutations();
  try {
    const { payload, requestId } = entry;
    let receipt: RoutineMutationReceipt;
    switch (payload.kind) {
      case "CREATE": receipt = { kind: "CREATE", value: await api.create(payload.name, payload.revision, requestId) }; break;
      case "REVISE": receipt = { kind: "REVISE", value: await api.revise(payload.routineId, payload.expectedVersion, payload.revision, requestId) }; break;
      case "ARCHIVE": receipt = { kind: "ARCHIVE", value: await api.archive(payload.routineId, payload.expectedVersion, requestId) }; break;
      case "RUN": receipt = { kind: "RUN", value: await api.run(payload.routineId, payload.routineRevision, payload.inputs, requestId) }; break;
    }
    // Validate the decoded result against the exact immutable command before releasing
    // the request ID/payload needed to recover an ambiguous server commit.
    validateRoutineMutationReceipt(entry, receipt);
    entry.activeCalls = Math.max(0, entry.activeCalls - 1);
    removePendingMutation(entry);
    return receipt;
  } catch (error) {
    entry.activeCalls = Math.max(0, entry.activeCalls - 1);
    // A definitive rejection from one overlapping call cannot settle the request
    // while another exact attempt is still capable of committing.
    if (isDefinitiveRoutineRejection(error) && entry.activeCalls === 0) removePendingMutation(entry);
    else publishPendingRoutineMutations();
    throw error;
  }
}

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
  const result: Record<string, unknown> = { ...revision };
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

/** Routine editor plus save-only materialization into an ordinary READY Task. */
export function RoutinesPage({ api, workspaceId, resources, resourcesNextCursor, resourcesPageBusy, onLoadMoreResources, onOpenTask }: Props) {
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
  const [runInputs, setRunInputs] = useState<Record<string, unknown>>({});
  const [runResult, setRunResult] = useState<MaterializedRoutineTask | null>(null);
  const [pendingMutations, setPendingMutations] = useState<PendingRoutineMutation[]>(() => pendingForWorkspace(workspaceId));
  const [discardKey, setDiscardKey] = useState<string | null>(null);
  const listGeneration = useRef(0);
  const detailGeneration = useRef(0);

  useEffect(() => {
    const refresh = () => setPendingMutations(pendingForWorkspace(workspaceId));
    refresh();
    return subscribePendingRoutineMutations(refresh);
  }, [workspaceId]);

  useEffect(() => {
    const controller = new AbortController();
    const generation = ++listGeneration.current;
    ++detailGeneration.current;
    setItems([]); setNextCursor(null); setSelected(null); setCurrentRevision(null); setRunInputs({}); setRunResult(null);
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
    setSelected(null); setCurrentRevision(null); setDetailError(null); setConfirmArchive(false); setDetailLoading(true); setRunInputs({}); setRunResult(null);
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
  const applyReceipt = (entry: PendingRoutineMutation, receipt: RoutineMutationReceipt) => {
    switch (receipt.kind) {
      case "CREATE":
        setMessage("Routine saved. It has not been run or scheduled.");
        if (entry.payload.kind === "CREATE" && editor && !editor.routineId && editor.name.trim() === entry.payload.name) setEditor(null);
        setReload(value => value + 1); setSelectedId(receipt.value.routine_id); break;
      case "REVISE":
        setMessage(`Saved immutable revision ${receipt.value.revision}. Existing Automation pins remain unchanged.`);
        if (entry.payload.kind === "REVISE" && editor?.routineId === entry.payload.routineId) setEditor(null);
        setReload(value => value + 1); break;
      case "ARCHIVE":
        if (selectedId === receipt.value.routine_id) setSelected(receipt.value);
        setItems(current => current.map(item => item.routine_id === receipt.value.routine_id ? receipt.value : item));
        setMessage("Routine archived. Existing Tasks and Automation history retain their pinned revisions."); setConfirmArchive(false); break;
      case "RUN":
        if (selectedId === receipt.value.routine_id) setRunResult(receipt.value);
        setMessage(`Saved Task ${receipt.value.task_id} is READY. Planning and execution have not started.`); break;
    }
  };
  const hasUnresolvedTarget = (payload: RoutineMutation): boolean => pendingForWorkspace(workspaceId).some(entry => {
    if (entry.workspaceId !== workspaceId) return false;
    if (payload.kind === "CREATE") return entry.payload.kind === "CREATE" && entry.payload.name === payload.name;
    if (payload.kind === "REVISE" || payload.kind === "ARCHIVE") return entry.payload.kind === payload.kind && entry.payload.routineId === payload.routineId;
    return entry.payload.kind === "RUN" && entry.payload.routineId === payload.routineId;
  });
  const retryPending = async (entry: PendingRoutineMutation) => {
    if (entry.workspaceId !== workspaceId || busy) return;
    setActionError(null); setMessage(null); setBusy(true);
    try { applyReceipt(entry, await executePendingMutation(api, entry)); }
    catch (error) { setActionError(errorText(error)); }
    finally { setBusy(false); }
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
      const payload: RoutineMutation = editor.routineId
        ? { kind: "REVISE", routineId: editor.routineId, expectedVersion: editor.version!, revision }
        : { kind: "CREATE", name: editor.name.trim(), revision };
      if (hasUnresolvedTarget(payload) && !pendingForWorkspace(workspaceId).some(entry => JSON.stringify(entry.payload) === JSON.stringify(payload))) {
        throw new Error("A previous Routine change for this item has an unknown outcome. Retry its exact request or explicitly discard the retry first.");
      }
      const pending = registerPendingMutation(workspaceId, payload);
      setBusy(true);
      applyReceipt(pending, await executePendingMutation(api, pending));
    } catch (error) { setActionError(errorText(error)); }
    finally { setBusy(false); }
  };
  const archive = async () => {
    if (!selected || selected.status !== "ACTIVE" || busy) return;
    setActionError(null); setMessage(null);
    try {
      const payload: RoutineMutation = { kind: "ARCHIVE", routineId: selected.routine_id, expectedVersion: selected.version };
      if (hasUnresolvedTarget(payload) && !pendingForWorkspace(workspaceId).some(entry => JSON.stringify(entry.payload) === JSON.stringify(payload))) {
        throw new Error("A previous archive request has an unknown outcome. Retry its exact request or explicitly discard the retry first.");
      }
      const pending = registerPendingMutation(workspaceId, payload);
      setBusy(true);
      applyReceipt(pending, await executePendingMutation(api, pending));
    } catch (error) { setActionError(errorText(error)); }
    finally { setBusy(false); }
  };

  const runNow = async () => {
    if (!selected || !currentRevision || selected.status !== "ACTIVE" || busy) return;
    setActionError(null); setMessage(null); setRunResult(null);
    const contract = routineInputFields(currentRevision);
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
        // Preserve an exact selected ref for idempotent retry even if the catalog head changes.
        // SQLite rechecks Resource ownership/revision availability at Task admission.
        inputs[field.name] = { workspace_id: workspaceId, resource_id: ref.resource_id, revision_id: ref.revision_id };
        continue;
      }
      const text = typeof value === "string" ? value : "";
      if (field.enumValues && text.length > 0 && !field.enumValues.includes(text)) { setActionError(`${field.label} must be one of the listed choices.`); return; }
      const characterCount = Array.from(text).length;
      const byteCount = new TextEncoder().encode(text).byteLength;
      if (field.required && text.trim().length === 0) { setActionError(`${field.label} is required.`); return; }
      if (characterCount > field.maxLength || characterCount < field.minLength) {
        setActionError(`${field.label} must be ${field.minLength ? `${field.minLength}–` : "at most "}${field.maxLength} characters.`);
        return;
      }
      if (byteCount > field.maxBytes) { setActionError(`${field.label} exceeds its ${field.maxBytes}-byte UTF-8 limit.`); return; }
      if (text.length > 0) inputs[field.name] = text;
    }
    const payload: RoutineMutation = { kind: "RUN", routineId: selected.routine_id, routineRevision: currentRevision.revision, inputs };
    if (hasUnresolvedTarget(payload) && !pendingForWorkspace(workspaceId).some(entry => JSON.stringify(entry.payload) === JSON.stringify(payload))) {
      setActionError("A previous Run request for this Routine has an unknown outcome. Retry its exact inputs or explicitly discard the retry before creating another Task.");
      return;
    }
    try {
      const pending = registerPendingMutation(workspaceId, payload);
      setBusy(true);
      applyReceipt(pending, await executePendingMutation(api, pending));
    } catch (error) { setActionError(errorText(error)); }
    finally { setBusy(false); }
  };
  const selectedRunContract = selected && currentRevision ? routineInputFields(currentRevision) : null;
  const selectedHasPendingRun = Boolean(selected && pendingMutations.some(entry => entry.workspaceId === workspaceId && entry.payload.kind === "RUN" && entry.payload.routineId === selected.routine_id));

  const discardPending = (entry: PendingRoutineMutation) => {
    if (entry.workspaceId !== workspaceId) return;
    removePendingMutation(entry);
    setDiscardKey(null);
    setActionError(null);
  };

  return <div className="page-content routines-content">
    <header className="routines-heading">
      <div><div className="eyebrow">WORKSPACE LIBRARY</div><h1>Routines</h1><p>Save reusable work instructions, then create a Task pinned to the exact revision when you are ready.</p></div>
      <div className="routines-heading-actions"><button className="quiet-button" type="button" onClick={() => setReload(value => value + 1)} disabled={loading}>Refresh</button><button className="primary-button" type="button" onClick={startCreate} disabled={!workspaceId || Boolean(editor)}>New Routine</button></div>
    </header>

    <section className="routine-safety-note" role="note"><strong>Run now saves work for review</strong><span>It creates an ordinary READY Task pinned to this Routine revision. No agent starts until a separate supported planning path is available.</span></section>
    {workspaceId && pendingMutations.length > 0 && <section className="routine-pending" aria-labelledby="routine-pending-heading">
      <h2 id="routine-pending-heading">Routine requests awaiting confirmation</h2>
      <p>These exact requests are held only in this app process. Retry reuses the original request ID and payload. They are lost if LiteCowork closes; passwords and API keys should not be entered in Routine text.</p>
      <ul>{pendingMutations.map(entry => {
        const label = entry.payload.kind === "CREATE" ? `Create “${entry.payload.name}”`
          : entry.payload.kind === "REVISE" ? `Revise Routine ${entry.payload.routineId}`
            : entry.payload.kind === "ARCHIVE" ? `Archive Routine ${entry.payload.routineId}`
              : `Run Routine ${entry.payload.routineId} · revision ${entry.payload.routineRevision}`;
        return <li key={entry.key} className="routine-pending-item">
          <div><strong>{label}</strong><small>{entry.activeCalls > 0 ? `${entry.activeCalls} request attempt${entry.activeCalls === 1 ? " is" : "s are"} still in progress. An exact retry is safe.` : "The server receipt was not confirmed; the change may already have committed."}</small></div>
          <div className="routine-pending-actions">
            <button type="button" className="quiet-button" disabled={busy} onClick={() => void retryPending(entry)}>Retry exact request</button>
            {discardKey !== entry.key && <button type="button" className="routine-danger-button" disabled={busy} onClick={() => setDiscardKey(entry.key)}>Discard retry</button>}
            {discardKey === entry.key && <div className="routine-discard-confirm" role="group" aria-label="Confirm discard retry">
              <p>Discarding forgets this request ID and payload. If the original request committed, a later new request may create a duplicate. Check the Routine or Task list first.</p>
              <button type="button" className="quiet-button" disabled={busy} onClick={() => setDiscardKey(null)}>Keep retry</button>
              <button type="button" className="routine-danger-button" disabled={busy} onClick={() => discardPending(entry)}>Confirm discard</button>
            </div>}
          </div>
        </li>;
      })}</ul>
    </section>}
    {message && <p className="routine-message" role="status">{message}</p>}
    {!workspaceId ? <section className="routine-state"><h2>Select a Workspace</h2><p>Choose a Workspace to view its saved Routines.</p></section> : loading ? <section className="routine-state" role="status">Loading saved Routines…</section> : listError ? <section className="routine-state routine-error" role="alert"><h2>Routines could not be loaded</h2><p>{listError}</p><button className="text-button" type="button" onClick={() => setReload(value => value + 1)}>Try again</button></section> : <>
      {items.length === 0 && !editor ? <section className="routine-state"><h2>No saved Routines</h2><p>Create a reusable definition, then save a READY Task from it when needed.</p><button className="primary-button" type="button" onClick={startCreate}>Create a Routine</button></section> : <div className="routines-layout">
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
            {selected.status === "ACTIVE" && selectedRunContract && <section className="routine-run-form" aria-labelledby="routine-run-heading">
              <h3 id="routine-run-heading">Run this Routine</h3>
              <p>Inputs are validated and pinned to this immutable revision. This saves a Task as READY; it does not start an agent.</p>
              {selectedRunContract.fields.map(field => field.kind === "TEXT" ? <label className="routine-field" key={field.name}>{field.label}{field.required && <span> (required)</span>}
                {field.enumValues ? <select required={field.required} disabled={busy || selectedHasPendingRun} value={typeof runInputs[field.name] === "string" ? runInputs[field.name] as string : ""} onChange={event => setRunInputs(current => ({ ...current, [field.name]: event.target.value }))}><option value="">Choose…</option>{field.enumValues.map(choice => <option key={choice} value={choice}>{choice}</option>)}</select> : <textarea rows={2} maxLength={Math.min(field.maxLength * 2, 32_768)} minLength={field.minLength || undefined} required={field.required} disabled={busy || selectedHasPendingRun} value={typeof runInputs[field.name] === "string" ? runInputs[field.name] as string : ""} onChange={event => setRunInputs(current => ({ ...current, [field.name]: event.target.value }))} />}
                <small>{field.enumValues ? "Choose one of the values defined by this Routine." : `Maximum ${field.maxLength} characters and ${field.maxBytes} UTF-8 bytes.`}</small>
              </label> : <label className="routine-field" key={field.name}>{field.label}{field.required && <span> (required)</span>}
                <select disabled={busy || selectedHasPendingRun} required={field.required} value={(() => { const ref = runInputs[field.name]; if (!ref || typeof ref !== "object" || Array.isArray(ref)) return ""; const source = ref as Record<string, unknown>; const item = resources.find(resource => resource.workspaceId === workspaceId && resource.resourceId === source.resource_id && resource.resourceRevisionId === source.revision_id); return item ? pinnedResourceOptionKey(item) : ""; })()} onChange={event => { const item = resources.find(resource => resource.workspaceId === workspaceId && pinnedResourceOptionKey(resource) === event.target.value); setRunInputs(current => ({ ...current, [field.name]: item ? { workspace_id: workspaceId, resource_id: item.resourceId, revision_id: item.resourceRevisionId } : null })); }}>
                  <option value="">{field.required ? "Choose a Resource revision…" : "No Resource"}</option>
                  {resources.filter(resource => resource.workspaceId === workspaceId).map(resource => <option key={pinnedResourceOptionKey(resource)} value={pinnedResourceOptionKey(resource)}>{resource.displayName} · {resource.mediaType} · {resource.resourceRevisionId}</option>)}
                </select>
                <small>The exact Workspace, Resource, and immutable revision are pinned to the Task.</small>
              </label>)}
              {selectedRunContract.fields.some(field => field.kind === "RESOURCE_REF") && resourcesNextCursor && <button type="button" className="text-button" disabled={resourcesPageBusy} onClick={() => void onLoadMoreResources()}>{resourcesPageBusy ? "Loading Resources…" : "Load more Resources"}</button>}
              {selectedHasPendingRun && <p className="routine-inline-error" role="note">A previous Run request has an unknown outcome. Retry or explicitly discard it in the notice above before creating another Task.</p>}
              {selectedRunContract.blockedReason && <p className="routine-inline-error" role="note">Run unavailable: {selectedRunContract.blockedReason}</p>}
              {runResult && <div className="routine-run-result" role="status"><strong>Saved Task · READY</strong><span>{runResult.task_id} · Routine revision {runResult.routine_revision} · TaskSpec revision {runResult.current_spec_revision}</span><p>Planning and execution have not started.</p>{onOpenTask && <button type="button" className="quiet-button" onClick={() => onOpenTask(runResult)}>Open Task details</button>}</div>}
              <button type="button" className="primary-button" disabled={busy || selectedHasPendingRun || Boolean(selectedRunContract.blockedReason)} onClick={() => void runNow()}>{busy ? "Saving Task…" : selectedHasPendingRun ? "Resolve previous Run first" : "Run now"}</button>
            </section>}
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
