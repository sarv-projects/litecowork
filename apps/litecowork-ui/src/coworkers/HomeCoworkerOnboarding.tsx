import { useEffect, useRef, useState } from "react";
import type { Coworker, CoworkerRevisionInput, CoworkerSettingsApi, WorkspacePrimaryReceipt } from "./coworker-api";

const assistantRevision: CoworkerRevisionInput = {
  name: "Assistant", avatar_ref: null, role_description: "General-purpose assistant",
  default_lead_agent_binding_id: null, delegation_strategy: "BALANCED",
  enabled_delegation_profile_ids: [], delegation_budget_policy: null,
  lead_failover_policy: { mode: "DISABLED", triggers: [], fallback_agent_binding_ids: [], max_lead_changes: 0 },
  interaction_policy: {
    read_only_work: "STANDARD_TRUST_POLICY", draft_creation: "STANDARD_TRUST_POLICY",
    external_mutation: "REQUIRE_OWNER_APPROVAL", destructive_action: "HANDOFF_TO_OWNER",
    financial_commitment: "HANDOFF_TO_OWNER",
  },
  context_policy: {
    allowed_context_kinds: ["COWORKER_NOTES", "WORKSPACE_NOTES"], max_retrieved_items: 20,
    retain_task_summaries: true, require_user_confirmation_for_memory: true,
  },
  notification_policy: { blockers: "ALWAYS", completion: "ON_SUCCESS", failures: "ALWAYS" },
};

/** Optional setup only. These commands neither save a Task nor start an agent. */
export function HomeCoworkerOnboarding({ api, workspaceId, workspaceVersion, items, hasMore, disabled, onCreated, onPrimary, onManage }: {
  api: CoworkerSettingsApi;
  workspaceId: string;
  workspaceVersion: number;
  items: Coworker[];
  hasMore: boolean;
  disabled: boolean;
  onCreated: (coworker: Coworker) => void;
  onPrimary: (workspace: WorkspacePrimaryReceipt) => void;
  onManage: () => void;
}) {
  const [dismissed, setDismissed] = useState(false);
  const [selectedId, setSelectedId] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [pending, setPending] = useState<{ kind: "CREATE"; requestId: string } | { kind: "PRIMARY"; requestId: string; coworkerId: string; version: number } | null>(null);
  const alive = useRef(true);
  const inFlight = useRef(false);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  const active = items.filter(item => item.workspace_id === workspaceId && item.status === "ACTIVE");
  const selected = active.find(item => item.coworker_id === selectedId);

  async function submit(kind: "CREATE" | "PRIMARY") {
    if (inFlight.current || disabled) return;
    const command = pending ?? (kind === "CREATE"
      ? { kind, requestId: crypto.randomUUID() }
      : selected ? { kind, requestId: crypto.randomUUID(), coworkerId: selected.coworker_id, version: workspaceVersion } : null);
    if (!command || command.kind !== kind) return;
    inFlight.current = true;
    setPending(command);
    setBusy(true);
    setMessage(null);
    try {
      if (command.kind === "CREATE") {
        const coworker = await api.create(assistantRevision, command.requestId);
        if (!alive.current) return;
        if (coworker.workspace_id !== workspaceId || coworker.status !== "ACTIVE") throw new Error("Created Coworker could not be confirmed in this Workspace.");
        onCreated(coworker);
        setSelectedId(coworker.coworker_id);
        setMessage("Assistant created. Choose Make primary when you want to use it as this Workspace’s default.");
      } else {
        // Check eligibility before a fresh command. An unresolved retry must reach
        // the server's receipt lookup even if the Coworker changed afterward.
        if (!pending) {
          const coworker = await api.get(command.coworkerId);
          if (!alive.current) return;
          if (coworker.workspace_id !== workspaceId || coworker.status !== "ACTIVE") {
            setPending(null);
            throw new Error("This Coworker is no longer active. Open Coworkers to review it.");
          }
        }
        const receipt = await api.setPrimary(command.coworkerId, command.version, command.requestId);
        if (!alive.current) return;
        if (receipt.workspace_id !== workspaceId || receipt.primary_coworker_id !== command.coworkerId || receipt.version <= command.version) throw new Error("Primary Coworker response could not be confirmed.");
        onPrimary(receipt);
      }
      setPending(null);
    } catch (error) {
      if (alive.current) setMessage(`${error instanceof Error ? error.message : "Setup could not be confirmed."} Retry keeps the original request. For a version conflict, open Coworkers and reload before choosing again.`);
    } finally {
      inFlight.current = false;
      if (alive.current) setBusy(false);
    }
  }

  if (dismissed) return null;
  return <section className="setup-callout" aria-label="Set up your primary Coworker" aria-busy={busy}>
    <div>
      <h2>Choose your primary Coworker</h2>
      <p>A Coworker holds preferences for future work. Setup is optional; your draft stays here.</p>
      {items.length === 0 && !hasMore ? <>
        <p>Create “Assistant” with a general-purpose role, no worker profiles, and owner approval for external changes. It uses the Workspace lead default; configure an available lead in Settings before saving work.</p>
        <button type="button" className="secondary-button" disabled={disabled || busy || pending?.kind === "PRIMARY"} onClick={() => void submit("CREATE")}>{pending?.kind === "CREATE" ? "Retry creating Assistant" : "Create Assistant"}</button>
      </> : <>
        {active.length > 0 ? <>
          <label>Primary Coworker <select aria-label="Primary Coworker" value={selectedId} disabled={disabled || busy || pending !== null} onChange={event => setSelectedId(event.target.value)}>
            <option value="">Choose an active Coworker</option>
            {active.map(item => <option key={item.coworker_id} value={item.coworker_id}>{item.revision.name}</option>)}
          </select></label>
          <button type="button" className="secondary-button" disabled={disabled || busy || (!selected && pending?.kind !== "PRIMARY") || pending?.kind === "CREATE"} onClick={() => void submit("PRIMARY")}>{pending?.kind === "PRIMARY" ? "Retry making primary" : "Make primary"}</button>
        </> : <p>No active Coworker is loaded. Open Coworkers to review or create one.</p>}
        {hasMore && <p>This is a partial roster. Open Coworkers to see more.</p>}
        {pending?.kind === "CREATE" && <button type="button" className="secondary-button" disabled={disabled || busy} onClick={() => void submit("CREATE")}>Retry creating Assistant</button>}
      </>}
      <p>Making primary changes the default for future work. It does not change existing Tasks or start an agent.</p>
      {message && <p role="status">{message}</p>}
      <button type="button" className="text-button" disabled={busy} onClick={onManage}>Open Coworkers</button>
      <button type="button" className="text-button" disabled={busy || pending !== null} onClick={() => setDismissed(true)}>Not now</button>
    </div>
  </section>;
}
