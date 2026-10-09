import { useEffect, useRef, useState } from "react";
import type {
  Coworker, CoworkerContextKind, CoworkerDelegationStrategy, CoworkerInteractionDefault,
  CoworkerPresence, CoworkerRevisionInput, CoworkerSettingsApi, LeadFailoverPolicy,
  WorkspacePrimaryReceipt,
} from "./coworker-api";
import "./coworker-settings.css";

export type CoworkerLeadBindingOption = {
  agent_binding_id: string;
  display_name: string;
  enabled: boolean;
  lead_eligible: boolean;
};
export type CoworkerWorkerProfileOption = {
  delegation_profile_id: string;
  name: string;
  agent_binding_id: string;
  enabled: boolean;
};
type Props = {
  api: CoworkerSettingsApi;
  workspaceId: string;
  workspaceVersion: number | null;
  leadBindings?: CoworkerLeadBindingOption[];
  workerProfiles?: CoworkerWorkerProfileOption[];
  workerProfilesError?: string | null;
  onWorkspaceUpdated?: (workspace: WorkspacePrimaryReceipt) => void;
};
type EditorMode = "view" | "create" | "edit";
type Operation = "save" | "pause" | "resume" | "archive" | "primary";

const INTERACTION_OPTIONS: { value: CoworkerInteractionDefault; label: string }[] = [
  { value: "STANDARD_TRUST_POLICY", label: "Use normal Trust checks" },
  { value: "REQUIRE_OWNER_APPROVAL", label: "Ask me before this action" },
  { value: "HANDOFF_TO_OWNER", label: "Stop and hand control to me" },
];
const CONTEXT_OPTIONS: { value: CoworkerContextKind; label: string }[] = [
  { value: "PERSONAL_PROFILE", label: "Personal profile" },
  { value: "COWORKER_NOTES", label: "Coworker notes" },
  { value: "WORKSPACE_NOTES", label: "Workspace notes" },
  { value: "GOAL_NOTES", label: "Goal notes" },
];
const FAILOVER_TRIGGERS: LeadFailoverPolicy["triggers"] = ["AGENT_UNAVAILABLE", "QUOTA_EXHAUSTED", "RUNTIME_UNAVAILABLE"];

function newRevision(): CoworkerRevisionInput {
  return {
    name: "",
    avatar_ref: null,
    role_description: "General-purpose assistant",
    default_lead_agent_binding_id: null,
    delegation_strategy: "BALANCED",
    enabled_delegation_profile_ids: [],
    delegation_budget_policy: null,
    lead_failover_policy: { mode: "DISABLED", triggers: [], fallback_agent_binding_ids: [], max_lead_changes: 0 },
    interaction_policy: {
      read_only_work: "STANDARD_TRUST_POLICY",
      draft_creation: "STANDARD_TRUST_POLICY",
      external_mutation: "REQUIRE_OWNER_APPROVAL",
      destructive_action: "HANDOFF_TO_OWNER",
      financial_commitment: "HANDOFF_TO_OWNER",
    },
    context_policy: {
      allowed_context_kinds: ["COWORKER_NOTES", "WORKSPACE_NOTES"],
      max_retrieved_items: 20,
      retain_task_summaries: true,
      require_user_confirmation_for_memory: true,
    },
    notification_policy: { blockers: "ALWAYS", completion: "ON_SUCCESS", failures: "ALWAYS" },
  };
}

function labelStatus(status: Coworker["status"]): string {
  return status === "ACTIVE" ? "Active" : status === "PAUSED" ? "Paused" : "Archived";
}
function labelActivity(status: CoworkerPresence["activity_status"]): string {
  const labels: Record<CoworkerPresence["activity_status"], string> = {
    AVAILABLE: "Available", PLANNING: "Planning", WORKING: "Working", WAITING: "Waiting", NEEDS_YOU: "Needs you",
  };
  return labels[status];
}
function labelRuntime(status: CoworkerPresence["runtime_status"]): string {
  const labels: Record<CoworkerPresence["runtime_status"], string> = {
    AVAILABLE: "Available", DEGRADED: "Needs attention", OFFLINE: "Offline", UNKNOWN: "Not known",
  };
  return labels[status];
}
function displayError(error: unknown): string {
  return error instanceof Error ? error.message : "Coworker details could not be loaded.";
}
function triggerLabel(trigger: LeadFailoverPolicy["triggers"][number]): string {
  return trigger === "AGENT_UNAVAILABLE" ? "Lead agent unavailable"
    : trigger === "QUOTA_EXHAUSTED" ? "Provider usage limit reached"
      : "Runtime unavailable";
}
function initial(name: string): string {
  return [...name.trim()][0]?.toLocaleUpperCase() ?? "·";
}

/** Workspace-scoped Coworker settings. All writes create revisions or explicit status commands. */
export function CoworkerSettings({ api, workspaceId, workspaceVersion, leadBindings, workerProfiles, workerProfilesError, onWorkspaceUpdated }: Props) {
  const [items, setItems] = useState<Coworker[]>([]);
  const [cursor, setCursor] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [selected, setSelected] = useState<Coworker | null>(null);
  const [presence, setPresence] = useState<CoworkerPresence | null>(null);
  const [presenceMessage, setPresenceMessage] = useState<string | null>(null);
  const [listLoading, setListLoading] = useState(true);
  const [listFailure, setListFailure] = useState<string | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailFailure, setDetailFailure] = useState<string | null>(null);
  const [mode, setMode] = useState<EditorMode>("view");
  const [form, setForm] = useState<CoworkerRevisionInput | null>(null);
  const [operation, setOperation] = useState<Operation | null>(null);
  const [formMessage, setFormMessage] = useState<string | null>(null);
  const [archiveConfirm, setArchiveConfirm] = useState(false);
  const [reloadKey, setReloadKey] = useState(0);
  const [workspaceVersionLocal, setWorkspaceVersionLocal] = useState(workspaceVersion);
  const listGeneration = useRef(0);
  const requestKeys = useRef(new Map<string, { signature: string; requestId: string }>());

  useEffect(() => setWorkspaceVersionLocal(workspaceVersion), [workspaceVersion]);

  useEffect(() => {
    if (!workspaceId) {
      setItems([]);
      setCursor(null);
      setSelectedId(null);
      setSelected(null);
      setPresence(null);
      setListLoading(false);
      setListFailure(null);
      return;
    }
    const controller = new AbortController();
    const generation = ++listGeneration.current;
    setListLoading(true);
    setListFailure(null);
    setItems([]);
    setCursor(null);
    setSelectedId(null);
    setSelected(null);
    setPresence(null);
    setMode("view");
    setForm(null);
    void api.list(undefined, controller.signal).then(page => {
      if (controller.signal.aborted || generation !== listGeneration.current) return;
      if (page.items.some(item => item.workspace_id !== workspaceId)) throw new Error("Coworker list belongs to a different Workspace.");
      setItems(page.items);
      setCursor(page.next_cursor);
      setSelectedId(page.items[0]?.coworker_id ?? null);
    }).catch(error => {
      if (!controller.signal.aborted && generation === listGeneration.current) setListFailure(displayError(error));
    }).finally(() => {
      if (!controller.signal.aborted && generation === listGeneration.current) setListLoading(false);
    });
    return () => controller.abort();
  }, [api, workspaceId, reloadKey]);

  useEffect(() => {
    if (!selectedId || mode === "create") {
      setSelected(null);
      setPresence(null);
      setPresenceMessage(null);
      setDetailLoading(false);
      setDetailFailure(null);
      return;
    }
    const controller = new AbortController();
    setDetailLoading(true);
    setDetailFailure(null);
    setPresence(null);
    setPresenceMessage(null);
    void api.get(selectedId, controller.signal).then(item => {
      if (controller.signal.aborted) return;
      if (item.workspace_id !== workspaceId || item.coworker_id !== selectedId) throw new Error("Coworker response scope mismatch.");
      setSelected(item);
      setItems(current => current.map(row => row.coworker_id === item.coworker_id ? item : row));
    }).catch(error => {
      if (!controller.signal.aborted) setDetailFailure(displayError(error));
    }).finally(() => { if (!controller.signal.aborted) setDetailLoading(false); });
    void api.getPresence(selectedId, controller.signal).then(value => {
      if (!controller.signal.aborted) setPresence(value);
    }).catch(() => {
      if (!controller.signal.aborted) setPresenceMessage("Live availability could not be loaded. Coworker setup and history are still available.");
    });
    return () => controller.abort();
  }, [api, workspaceId, selectedId, mode, reloadKey]);

  function requestId(key: string, signature: string): string {
    const existing = requestKeys.current.get(key);
    if (existing?.signature === signature) return existing.requestId;
    const request = { signature, requestId: crypto.randomUUID() };
    requestKeys.current.set(key, request);
    return request.requestId;
  }
  function clearRequest(key: string) { requestKeys.current.delete(key); }

  function startCreate() {
    setSelectedId(null);
    setSelected(null);
    setPresence(null);
    setMode("create");
    setForm(newRevision());
    setFormMessage(null);
    setDetailFailure(null);
    setArchiveConfirm(false);
  }
  function startEdit() {
    if (!selected || selected.status === "ARCHIVED") return;
    setForm({
      ...selected.revision,
      enabled_delegation_profile_ids: [...selected.revision.enabled_delegation_profile_ids],
      interaction_policy: { ...selected.revision.interaction_policy },
      context_policy: { ...selected.revision.context_policy, allowed_context_kinds: [...selected.revision.context_policy.allowed_context_kinds] },
      notification_policy: { ...selected.revision.notification_policy },
      lead_failover_policy: selected.revision.lead_failover_policy ? {
        ...selected.revision.lead_failover_policy,
        triggers: [...selected.revision.lead_failover_policy.triggers],
        fallback_agent_binding_ids: [...selected.revision.lead_failover_policy.fallback_agent_binding_ids],
      } : null,
      delegation_budget_policy: selected.revision.delegation_budget_policy ? { ...selected.revision.delegation_budget_policy } : null,
    });
    setMode("edit");
    setFormMessage(null);
    setDetailFailure(null);
    setArchiveConfirm(false);
  }
  function cancelEdit() {
    setMode("view");
    setForm(null);
    setFormMessage(null);
    setDetailFailure(null);
  }
  function changeForm<K extends keyof CoworkerRevisionInput>(key: K, value: CoworkerRevisionInput[K]) {
    setForm(current => current ? { ...current, [key]: value } : current);
    setFormMessage(null);
  }
  function updatePolicy<K extends keyof CoworkerRevisionInput["interaction_policy"]>(key: K, value: CoworkerInteractionDefault) {
    setForm(current => current ? { ...current, interaction_policy: { ...current.interaction_policy, [key]: value } } : current);
  }

  async function saveRevision(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!form || operation) return;
    const normalized = { ...form, name: form.name.trim(), role_description: form.role_description.trim() };
    if (!normalized.name || normalized.name.length > 120) { setFormMessage("Enter a name between 1 and 120 characters."); return; }
    if (normalized.role_description.length > 2000) { setFormMessage("Keep the role description within 2,000 characters."); return; }
    const signature = JSON.stringify([workspaceId, mode, selected?.coworker_id ?? null, selected?.version ?? null, normalized]);
    const key = mode === "create" ? "create" : `revise:${selected?.coworker_id}`;
    const id = requestId(key, signature);
    setOperation("save"); setFormMessage(null); setDetailFailure(null);
    try {
      const saved = mode === "create"
        ? await api.create(normalized, id)
        : selected ? await api.revise(selected.coworker_id, selected.version, normalized, id) : null;
      if (!saved) throw new Error("Select a Coworker before saving changes.");
      if (saved.workspace_id !== workspaceId) throw new Error("Saved Coworker belongs to a different Workspace.");
      clearRequest(key);
      setItems(current => mode === "create" ? [saved, ...current] : current.map(row => row.coworker_id === saved.coworker_id ? saved : row));
      setSelectedId(saved.coworker_id);
      setSelected(saved);
      setMode("view"); setForm(null); setFormMessage(mode === "create" ? "Coworker created." : `Saved as revision ${saved.current_revision}.`);
    } catch (error) {
      setDetailFailure(displayError(error));
      if (error instanceof Error && "status" in error && (error as { status?: number }).status === 409) setFormMessage("Your draft is preserved. Reload the current Coworker before retrying.");
    } finally { setOperation(null); }
  }

  async function transition(status: "PAUSED" | "ACTIVE" | "ARCHIVED") {
    if (!selected || operation || selected.status === "ARCHIVED") return;
    if (status === "ARCHIVED" && selected.is_primary) {
      setDetailFailure("Clear this Coworker as the Workspace primary before archiving it.");
      return;
    }
    const signature = JSON.stringify([workspaceId, selected.coworker_id, selected.version, status]);
    const key = `status:${selected.coworker_id}`;
    const id = requestId(key, signature);
    setOperation(status === "PAUSED" ? "pause" : status === "ACTIVE" ? "resume" : "archive");
    setDetailFailure(null); setFormMessage(null); setArchiveConfirm(false);
    try {
      const updated = await api.changeStatus(selected.coworker_id, selected.version, status, id);
      clearRequest(key);
      setSelected(updated);
      setItems(current => current.map(row => row.coworker_id === updated.coworker_id ? updated : row));
      setFormMessage(status === "PAUSED" ? "Paused. Existing Tasks continue under their own policy." : status === "ACTIVE" ? "Resumed for future proactive work." : "Archived. Linked history is retained.");
      if (status === "ARCHIVED") setMode("view");
    } catch (error) { setDetailFailure(displayError(error)); }
    finally { setOperation(null); }
  }

  async function setPrimary(targetId: string | null) {
    if (workspaceVersionLocal === null || operation) return;
    const signature = JSON.stringify([workspaceId, workspaceVersionLocal, targetId]);
    const key = `primary:${workspaceId}`;
    const id = requestId(key, signature);
    setOperation("primary"); setDetailFailure(null); setFormMessage(null);
    try {
      const updatedWorkspace = await api.setPrimary(targetId, workspaceVersionLocal, id);
      clearRequest(key);
      setWorkspaceVersionLocal(updatedWorkspace.version);
      onWorkspaceUpdated?.(updatedWorkspace);
      setItems(current => current.map(item => ({ ...item, is_primary: item.coworker_id === updatedWorkspace.primary_coworker_id })));
      setSelected(current => current ? { ...current, is_primary: current.coworker_id === updatedWorkspace.primary_coworker_id } : current);
      setFormMessage(targetId ? "Primary Coworker updated. New work can use this Coworker by default." : "Primary Coworker cleared. Existing Tasks are unchanged.");
    } catch (error) { setDetailFailure(displayError(error)); }
    finally { setOperation(null); }
  }

  async function loadMore() {
    if (!cursor || listLoading) return;
    const controller = new AbortController();
    setListLoading(true); setListFailure(null);
    try {
      const page = await api.list(cursor, controller.signal);
      if (page.items.some(item => item.workspace_id !== workspaceId)) throw new Error("Coworker page belongs to a different Workspace.");
      setItems(current => {
        const seen = new Set(current.map(item => item.coworker_id));
        return [...current, ...page.items.filter(item => !seen.has(item.coworker_id))];
      });
      setCursor(page.next_cursor);
    } catch (error) { setListFailure(displayError(error)); }
    finally { setListLoading(false); }
  }

  async function reloadSelected() {
    if (!selectedId) { setReloadKey(value => value + 1); return; }
    setDetailLoading(true); setDetailFailure(null);
    try {
      const latest = await api.get(selectedId);
      if (latest.workspace_id !== workspaceId || latest.coworker_id !== selectedId) throw new Error("Coworker response scope mismatch.");
      setSelected(latest);
      setItems(current => current.map(item => item.coworker_id === latest.coworker_id ? latest : item));
      setPresence(await api.getPresence(selectedId));
      setMode("view"); setForm(null); setFormMessage("Showing the latest saved Coworker revision.");
    } catch (error) { setDetailFailure(displayError(error)); }
    finally { setDetailLoading(false); }
  }

  const eligibleLeadBindings = leadBindings?.filter(item => item.enabled && item.lead_eligible) ?? [];
  const activeForm = form;

  return <main className="coworker-settings" aria-labelledby="coworker-settings-title">
    <header className="coworker-settings-header">
      <div><p className="coworker-settings-kicker">Workspace settings</p><h1 id="coworker-settings-title">Coworkers</h1><p>Optional specialized assistants for ongoing work. Create one in seconds, then personalize it when needed. Ordinary conversations do not require a Coworker.</p></div>
      <button className="coworker-primary-action" type="button" onClick={startCreate} disabled={!workspaceId || mode === "create"}>+ New Coworker</button>
    </header>

    {!workspaceId ? <section className="coworker-empty"><h2>Select a Workspace</h2><p>Choose a Workspace before setting up its Coworkers.</p></section> : <div className="coworker-settings-layout">
      <nav className="coworker-roster" aria-label="Workspace Coworkers">
        <div className="coworker-roster-heading"><h2>In this Workspace</h2><span>{items.length}</span></div>
        {listLoading && items.length === 0 && <p role="status" className="coworker-muted">Loading Coworkers…</p>}
        {listFailure && <div className="coworker-error" role="alert"><p>{listFailure}</p><button type="button" onClick={() => setReloadKey(value => value + 1)}>Retry</button></div>}
        {!listLoading && !listFailure && items.length === 0 && <div className="coworker-roster-empty"><p>No Coworkers yet.</p><button type="button" onClick={startCreate}>Create the first one</button></div>}
        <ul>{items.map(item => <li key={item.coworker_id}>
          <button type="button" className={`coworker-roster-item${selectedId === item.coworker_id && mode !== "create" ? " is-selected" : ""}`} aria-current={selectedId === item.coworker_id && mode !== "create" ? "page" : undefined} onClick={() => { setMode("view"); setForm(null); setFormMessage(null); setDetailFailure(null); setArchiveConfirm(false); setSelectedId(item.coworker_id); }}>
            <span className="coworker-roster-avatar" aria-hidden="true">{initial(item.revision.name)}</span>
            <span className="coworker-roster-copy"><strong>{item.revision.name}</strong><small>{item.status === "ACTIVE" ? "Active" : item.status === "PAUSED" ? "Paused" : "Archived"}{item.is_primary ? " · Primary" : ""}</small></span>
            <span className={`coworker-status-dot status-${item.status.toLowerCase()}`} aria-hidden="true" />
          </button>
        </li>)}</ul>
        {cursor && <button className="coworker-load-more" type="button" disabled={listLoading} onClick={() => void loadMore()}>{listLoading ? "Loading…" : "Load more"}</button>}
      </nav>

      <section className="coworker-detail" aria-label={mode === "create" ? "Create Coworker" : "Coworker details"}>
        {mode === "create" && activeForm ? <CoworkerEditor form={activeForm} mode="create" leadBindings={leadBindings} eligibleLeadBindings={eligibleLeadBindings} workerProfiles={workerProfiles} workerProfilesError={workerProfilesError} busy={operation === "save"} message={formMessage} onChange={changeForm} onInteractionChange={updatePolicy} onCancel={cancelEdit} onSubmit={saveRevision} />
          : mode === "edit" && activeForm && selected ? <CoworkerEditor form={activeForm} mode="edit" leadBindings={leadBindings} eligibleLeadBindings={eligibleLeadBindings} workerProfiles={workerProfiles} workerProfilesError={workerProfilesError} busy={operation === "save"} message={formMessage} onChange={changeForm} onInteractionChange={updatePolicy} onCancel={cancelEdit} onSubmit={saveRevision} />
            : selectedId ? detailLoading && !selected ? <p role="status" className="coworker-muted">Loading Coworker…</p>
              : selected ? <>
                <div className="coworker-identity">
                  <span className="coworker-profile-avatar" aria-hidden="true">{initial(selected.revision.name)}</span>
                  <div className="coworker-identity-copy"><div className="coworker-title-row"><h2>{selected.revision.name}</h2>{selected.is_primary && <span className="coworker-primary-badge">Primary</span>}</div><p>{selected.revision.role_description || "No role description yet."}</p><small>Revision {selected.current_revision} · Saved {new Date(selected.updated_at).toLocaleString()}</small></div>
                  <div className="coworker-identity-actions">
                    {selected.status !== "ARCHIVED" && <button type="button" className="coworker-secondary-action" onClick={startEdit} disabled={operation !== null}>Edit</button>}
                    <button type="button" className="coworker-overflow-action" aria-label="Refresh Coworker" title="Refresh" onClick={() => void reloadSelected()} disabled={detailLoading || operation !== null}>↻</button>
                  </div>
                </div>

                <section className="coworker-presence" aria-label="Current availability">
                  <div><small>Proactive work</small><strong>{labelStatus(selected.status)}</strong></div>
                  <div><small>Activity</small><strong>{presence ? labelActivity(presence.activity_status) : "Not reported"}</strong></div>
                  <div><small>Default lead Runtime</small><strong>{presence ? labelRuntime(presence.runtime_status) : "Not reported"}</strong></div>
                  {presence && <p>{presence.active_task_count} active · {presence.waiting_task_count} waiting · {presence.needs_you_count} need you</p>}
                  {presenceMessage && <p role="status">{presenceMessage}</p>}
                </section>

                {detailFailure && <div className="coworker-error" role="alert"><p>{detailFailure}</p><button type="button" onClick={() => void reloadSelected()}>Reload latest</button></div>}
                {formMessage && <p className="coworker-notice" role="status" aria-live="polite">{formMessage}</p>}

                <section className="coworker-summary-section"><h3>Lead and workers</h3>
                  <dl className="coworker-summary-list"><div><dt>Lead agent</dt><dd>{selected.revision.default_lead_agent_binding_id ? leadBindings?.find(item => item.agent_binding_id === selected.revision.default_lead_agent_binding_id)?.display_name ?? `Saved binding ${selected.revision.default_lead_agent_binding_id}` : "Use Workspace default"}</dd></div>
                    <div><dt>Delegation strategy</dt><dd>{strategyLabel(selected.revision.delegation_strategy)}</dd></div>
                    <div><dt>Worker profiles</dt><dd>{selected.revision.enabled_delegation_profile_ids.length ? `${selected.revision.enabled_delegation_profile_ids.length} assigned` : "None assigned"}</dd></div></dl>
                  {workerProfiles === undefined && <p className="coworker-helper">Enabling profiles happens separately in Settings → Agents. Assignment here only allows this Coworker to consider an enabled profile; it does not start an agent or grant app access.</p>}
                </section>
                <section className="coworker-summary-section"><h3>Interaction defaults</h3><p className="coworker-helper">These preferences never create a Grant or override a stricter Trust rule.</p>
                  <dl className="coworker-summary-list">{interactionRows().map(([label, key]) => <div key={key}><dt>{label}</dt><dd>{interactionLabel(selected.revision.interaction_policy[key])}</dd></div>)}</dl>
                </section>
                <section className="coworker-summary-section coworker-control-row">
                  <div><h3>Workspace default</h3><p className="coworker-helper">New work can start with the primary Coworker. Existing Tasks keep their recorded origin.</p></div>
                  <button type="button" className="coworker-secondary-action" disabled={operation !== null || workspaceVersionLocal === null || selected.status === "ARCHIVED" || selected.is_primary} onClick={() => void setPrimary(selected.coworker_id)}>{operation === "primary" ? "Saving…" : selected.is_primary ? "Primary Coworker" : "Make primary"}</button>
                  {selected.is_primary && <button type="button" className="coworker-text-action" disabled={operation !== null || workspaceVersionLocal === null} onClick={() => void setPrimary(null)}>Clear primary</button>}
                </section>

                {selected.status !== "ARCHIVED" && <section className="coworker-lifecycle">
                  <h3>Availability</h3>
                  <p>{selected.status === "ACTIVE" ? "Pausing stops new proactive and scheduled work. Existing Tasks continue under their own policy." : "Resuming allows future proactive work and scheduled admissions after dependency checks."}</p>
                  {selected.status === "ACTIVE" ? <button type="button" className="coworker-secondary-action" disabled={operation !== null} onClick={() => void transition("PAUSED")}>{operation === "pause" ? "Pausing…" : "Pause Coworker"}</button>
                    : <button type="button" className="coworker-secondary-action" disabled={operation !== null} onClick={() => void transition("ACTIVE")}>{operation === "resume" ? "Resuming…" : "Resume Coworker"}</button>}
                  {!archiveConfirm ? <button type="button" className="coworker-danger-action" disabled={operation !== null || selected.is_primary} title={selected.is_primary ? "Clear the primary selection before archiving." : undefined} onClick={() => setArchiveConfirm(true)}>Archive</button>
                    : <div className="coworker-archive-confirm" role="group" aria-label="Confirm archive"><p>Archiving retains history. Active Coworker Automations or Tasks can block this action.</p><button type="button" className="coworker-secondary-action" onClick={() => setArchiveConfirm(false)} disabled={operation !== null}>Keep Coworker</button><button type="button" className="coworker-danger-action" onClick={() => void transition("ARCHIVED")} disabled={operation !== null}>{operation === "archive" ? "Archiving…" : "Confirm archive"}</button></div>}
                </section>}
              </> : detailFailure ? <div className="coworker-error" role="alert"><p>{detailFailure}</p><button type="button" onClick={() => void reloadSelected()}>Retry</button></div> : <p className="coworker-muted">Coworker details are unavailable.</p>
              : <p className="coworker-muted">Choose a Coworker to view its settings.</p>
        }
      </section>
    </div>}
  </main>;
}

function CoworkerEditor({ form, mode, leadBindings, eligibleLeadBindings, workerProfiles, workerProfilesError, busy, message, onChange, onInteractionChange, onCancel, onSubmit }: {
  form: CoworkerRevisionInput; mode: "create" | "edit"; leadBindings?: CoworkerLeadBindingOption[]; eligibleLeadBindings: CoworkerLeadBindingOption[]; workerProfiles?: CoworkerWorkerProfileOption[]; workerProfilesError?: string | null; busy: boolean; message: string | null;
  onChange: <K extends keyof CoworkerRevisionInput>(key: K, value: CoworkerRevisionInput[K]) => void;
  onInteractionChange: (key: keyof CoworkerRevisionInput["interaction_policy"], value: CoworkerInteractionDefault) => void;
  onCancel: () => void; onSubmit: (event: React.FormEvent<HTMLFormElement>) => void;
}) {
  const currentFailover = form.lead_failover_policy ?? { mode: "DISABLED" as const, triggers: [], fallback_agent_binding_ids: [], max_lead_changes: 0 };
  function updateFailover(next: LeadFailoverPolicy) { onChange("lead_failover_policy", next); }
  function updateTrigger(trigger: LeadFailoverPolicy["triggers"][number], checked: boolean) {
    const triggers = checked ? [...currentFailover.triggers, trigger] : currentFailover.triggers.filter(item => item !== trigger);
    updateFailover({ ...currentFailover, triggers });
  }
  function updateFallback(bindingId: string, checked: boolean) {
    const fallback_agent_binding_ids = checked
      ? [...currentFailover.fallback_agent_binding_ids.filter(item => item !== bindingId), bindingId].slice(0, 3)
      : currentFailover.fallback_agent_binding_ids.filter(item => item !== bindingId);
    updateFailover({ ...currentFailover, fallback_agent_binding_ids });
  }
  const currentWorkerIds = form.enabled_delegation_profile_ids;
  const knownWorkerIds = new Set(workerProfiles?.map(item => item.delegation_profile_id) ?? []);
  const unavailableAssignedWorkers = currentWorkerIds.filter(id => !knownWorkerIds.has(id));
  const formValid = form.name.trim().length > 0 && form.name.trim().length <= 120 && form.role_description.length <= 2000
    && (currentFailover.mode === "DISABLED" || currentFailover.triggers.length > 0)
    && (currentFailover.mode !== "ALLOW_LISTED" || (currentFailover.fallback_agent_binding_ids.length > 0 && currentFailover.max_lead_changes > 0));

  return <form className="coworker-editor" onSubmit={onSubmit}>
    <div className="coworker-editor-heading"><div><span className="coworker-profile-avatar" aria-hidden="true">{initial(form.name || "?")}</span></div><div><p className="coworker-settings-kicker">{mode === "create" ? "New identity" : "New revision"}</p><h2>{mode === "create" ? "Create a Coworker" : `Edit ${form.name}`}</h2><p>{mode === "create" ? "Give it a name and describe what it helps with. Connections, agents and advanced setup are optional and can be changed later." : "Changes apply to future work. Existing Tasks keep their saved specification."}</p></div></div>

    <section className="coworker-form-section"><h3>Identity</h3>
      <label className="coworker-field">Name<input autoFocus={mode === "create"} maxLength={120} required value={form.name} onChange={event => onChange("name", event.currentTarget.value)} placeholder="Assistant" /></label>
      <label className="coworker-field">Role description<textarea maxLength={2000} value={form.role_description} onChange={event => onChange("role_description", event.currentTarget.value)} placeholder="What should this Coworker focus on?" /></label>
      {form.avatar_ref && <p className="coworker-helper">An avatar Resource revision is already pinned and will be preserved. Avatar selection is not available in this editor.</p>}
    </section>

    <details className="coworker-advanced coworker-customize"><summary>Customize agent, workers and access (optional)</summary>
    <section className="coworker-form-section"><h3>Lead and worker profiles</h3>
      {leadBindings === undefined ? <div className="coworker-field"><span>Lead agent</span><p className="coworker-helper">{form.default_lead_agent_binding_id ? `Saved binding ${form.default_lead_agent_binding_id} is preserved.` : "Use the Workspace default."} Agent bindings are managed in Settings → Agents.</p></div>
        : <label className="coworker-field">Lead agent<select value={form.default_lead_agent_binding_id ?? ""} onChange={event => onChange("default_lead_agent_binding_id", event.currentTarget.value || null)}><option value="">Use Workspace default</option>{form.default_lead_agent_binding_id && !eligibleLeadBindings.some(item => item.agent_binding_id === form.default_lead_agent_binding_id) && <option value={form.default_lead_agent_binding_id}>Saved binding · revalidation required</option>}{eligibleLeadBindings.map(item => <option key={item.agent_binding_id} value={item.agent_binding_id}>{item.display_name}</option>)}</select><small>Only enabled, lead-eligible bindings are offered.</small></label>}
      <label className="coworker-field">Delegation strategy<select value={form.delegation_strategy} onChange={event => onChange("delegation_strategy", event.currentTarget.value as CoworkerDelegationStrategy)}><option value="NATIVE_DEFAULT">Use the lead’s native behavior</option><option value="BALANCED">Balanced</option><option value="COST_SAVER">Prefer lower-cost suitable workers</option><option value="HOST_DELEGATION_ONLY">Host-delegated workers only when supported</option></select></label>
      {workerProfiles === undefined ? <div className="coworker-field"><span>Allowed worker profiles</span><p className="coworker-helper">{workerProfilesError ?? `${currentWorkerIds.length ? `${currentWorkerIds.length} assignment${currentWorkerIds.length === 1 ? " is" : "s are"} preserved.` : "No worker profiles assigned."} Enable profiles separately in Settings → Agents. Enabling a profile does not start it.`}</p></div>
        : <fieldset className="coworker-check-group"><legend>Allowed worker profiles</legend><p className="coworker-helper">Only enabled profiles may be assigned. Assignment does not start an agent, grant app access, or override Trust.</p>
          {workerProfiles.length === 0 && currentWorkerIds.length === 0 && <p className="coworker-helper">No enabled worker profiles are available in this Workspace.</p>}
          {workerProfiles.map(profile => <label key={profile.delegation_profile_id}><input type="checkbox" checked={currentWorkerIds.includes(profile.delegation_profile_id)} disabled={!profile.enabled && !currentWorkerIds.includes(profile.delegation_profile_id)} onChange={event => onChange("enabled_delegation_profile_ids", event.currentTarget.checked ? [...currentWorkerIds, profile.delegation_profile_id] : currentWorkerIds.filter(id => id !== profile.delegation_profile_id))} /><span><strong>{profile.name}</strong><small>{profile.enabled ? "Enabled in Workspace" : "Not enabled · remove this assignment to clear it"}</small></span></label>)}
          {unavailableAssignedWorkers.map(id => <label key={id} className="coworker-stale-option"><input type="checkbox" checked onChange={() => onChange("enabled_delegation_profile_ids", currentWorkerIds.filter(item => item !== id))} /><span><strong>Unavailable profile</strong><small>{id} · remove it to clear this saved assignment</small></span></label>)}
        </fieldset>}
      {form.delegation_budget_policy && <p className="coworker-helper">An existing delegation budget policy is preserved during this edit. Budget policy editing is not exposed in this settings surface yet.</p>}
    </section>

    <section className="coworker-form-section"><h3>Interaction defaults</h3><p className="coworker-helper">These preferences do not create Grants or bypass stricter Workspace, provider, or Trust requirements.</p>
      {interactionRows().map(([label, key]) => <label className="coworker-field" key={key}>{label}<select value={form.interaction_policy[key]} onChange={event => onInteractionChange(key, event.currentTarget.value as CoworkerInteractionDefault)}>{INTERACTION_OPTIONS.map(item => <option key={item.value} value={item.value}>{item.label}</option>)}</select></label>)}
    </section>

    </details>

    <details className="coworker-advanced"><summary>More settings</summary>
      <section className="coworker-form-section"><h3>Lead change preference</h3><p className="coworker-helper">A lead change starts a new session from the saved Task state; it does not move a running process.</p>
        <label className="coworker-field">If the lead becomes unavailable<select value={currentFailover.mode} onChange={event => {
          const mode = event.currentTarget.value as LeadFailoverPolicy["mode"];
          updateFailover(mode === "DISABLED" ? { mode, triggers: [], fallback_agent_binding_ids: [], max_lead_changes: 0 }
            : mode === "ASK" ? { mode, triggers: currentFailover.triggers.length ? currentFailover.triggers : ["AGENT_UNAVAILABLE"], fallback_agent_binding_ids: [], max_lead_changes: 0 }
              : { mode, triggers: currentFailover.triggers.length ? currentFailover.triggers : ["AGENT_UNAVAILABLE"], fallback_agent_binding_ids: [...currentFailover.fallback_agent_binding_ids], max_lead_changes: Math.max(1, currentFailover.max_lead_changes) });
        }}><option value="DISABLED">Stop and tell me</option><option value="ASK">Ask before choosing a new lead</option><option value="ALLOW_LISTED">Use one of my listed leads automatically</option></select></label>
        {currentFailover.mode !== "DISABLED" && <fieldset className="coworker-check-group"><legend>When to ask or switch</legend>{FAILOVER_TRIGGERS.map(trigger => <label key={trigger}><input type="checkbox" checked={currentFailover.triggers.includes(trigger)} onChange={event => updateTrigger(trigger, event.currentTarget.checked)} /><span>{triggerLabel(trigger)}</span></label>)}</fieldset>}
        {currentFailover.mode === "ALLOW_LISTED" && <>
          <fieldset className="coworker-check-group"><legend>Fallback lead order (up to 3)</legend>{eligibleLeadBindings.filter(item => item.agent_binding_id !== form.default_lead_agent_binding_id).map(binding => <label key={binding.agent_binding_id}><input type="checkbox" checked={currentFailover.fallback_agent_binding_ids.includes(binding.agent_binding_id)} disabled={!currentFailover.fallback_agent_binding_ids.includes(binding.agent_binding_id) && currentFailover.fallback_agent_binding_ids.length >= 3} onChange={event => updateFallback(binding.agent_binding_id, event.currentTarget.checked)} /><span>{binding.display_name}</span></label>)}{currentFailover.fallback_agent_binding_ids.filter(id => !eligibleLeadBindings.some(item => item.agent_binding_id === id)).map(id => <label className="coworker-stale-option" key={id}><input type="checkbox" checked onChange={() => updateFallback(id, false)} /><span>Unavailable binding · remove {id}</span></label>)}</fieldset>
          <label className="coworker-field">Maximum lead changes<input type="number" min={1} max={3} value={currentFailover.max_lead_changes} onChange={event => updateFailover({ ...currentFailover, max_lead_changes: Number(event.currentTarget.value) })} /></label>
        </>}
      </section>
      <section className="coworker-form-section"><h3>Context</h3><fieldset className="coworker-check-group"><legend>Context this Coworker may retrieve</legend>{CONTEXT_OPTIONS.map(option => <label key={option.value}><input type="checkbox" checked={form.context_policy.allowed_context_kinds.includes(option.value)} onChange={event => onChange("context_policy", { ...form.context_policy, allowed_context_kinds: event.currentTarget.checked ? [...form.context_policy.allowed_context_kinds, option.value] : form.context_policy.allowed_context_kinds.filter(item => item !== option.value) })} /><span>{option.label}</span></label>)}</fieldset>
        <label className="coworker-field">Maximum retrieved items<input type="number" min={0} max={100} value={form.context_policy.max_retrieved_items} onChange={event => onChange("context_policy", { ...form.context_policy, max_retrieved_items: Number(event.currentTarget.value) })} /></label>
        <label className="coworker-check-inline"><input type="checkbox" checked={form.context_policy.retain_task_summaries} onChange={event => onChange("context_policy", { ...form.context_policy, retain_task_summaries: event.currentTarget.checked })} />Allow this Coworker’s Task summaries to inform future context</label>
        <p className="coworker-helper">Current version: durable memory proposals require review. Automatic quiet learning is an accepted target feature and is not active until qualified provider and storage support exist.</p>
      </section>
      <section className="coworker-form-section"><h3>Notifications</h3>
        <label className="coworker-field">When work is blocked<select value={form.notification_policy.blockers} onChange={event => onChange("notification_policy", { ...form.notification_policy, blockers: event.currentTarget.value as CoworkerRevisionInput["notification_policy"]["blockers"] })}><option value="ALWAYS">Notify me</option><option value="SILENT">Keep it in Needs You</option></select></label>
        <label className="coworker-field">When work completes<select value={form.notification_policy.completion} onChange={event => onChange("notification_policy", { ...form.notification_policy, completion: event.currentTarget.value as CoworkerRevisionInput["notification_policy"]["completion"] })}><option value="ALWAYS">Always notify</option><option value="ON_SUCCESS">Notify when successful</option><option value="SILENT">Keep it in Work</option></select></label>
        <label className="coworker-field">When work fails<select value={form.notification_policy.failures} onChange={event => onChange("notification_policy", { ...form.notification_policy, failures: event.currentTarget.value as CoworkerRevisionInput["notification_policy"]["failures"] })}><option value="ALWAYS">Notify me</option><option value="SILENT">Keep it in Work</option></select></label>
      </section>
    </details>
    {message && <p className="coworker-notice" role="status">{message}</p>}
    <div className="coworker-form-actions"><button type="button" className="coworker-secondary-action" onClick={onCancel} disabled={busy}>Cancel</button><button type="submit" className="coworker-primary-action" disabled={busy || !formValid}>{busy ? "Saving…" : mode === "create" ? "Create Coworker" : "Save revision"}</button></div>
  </form>;
}

function strategyLabel(strategy: CoworkerDelegationStrategy): string {
  const labels: Record<CoworkerDelegationStrategy, string> = {
    NATIVE_DEFAULT: "Lead agent’s native strategy", BALANCED: "Balanced", COST_SAVER: "Prefer suitable lower-cost workers", HOST_DELEGATION_ONLY: "Host-delegated workers only",
  };
  return labels[strategy];
}
function interactionLabel(value: CoworkerInteractionDefault): string {
  return INTERACTION_OPTIONS.find(item => item.value === value)?.label ?? "Standard Trust checks";
}
function interactionRows(): [string, keyof CoworkerRevisionInput["interaction_policy"]][] {
  return [
    ["Read-only work", "read_only_work"], ["Create drafts", "draft_creation"], ["External changes", "external_mutation"],
    ["Destructive actions", "destructive_action"], ["Financial commitments", "financial_commitment"],
  ].map(([label, key]) => [label, key as keyof CoworkerRevisionInput["interaction_policy"]]);
}
