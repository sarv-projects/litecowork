import { useEffect, useRef, useState } from "react";
import type { Automation, AutomationApi, AutomationCoworker, AutomationDefinitionInput, AutomationExecutionPolicy, AutomationRevision, CoworkerRevisionRef, Routine, TriggerSpec } from "./automation-api";

type Props = {
  api: AutomationApi;
  workspaceId: string;
  existing?: { automation: Automation; revision: AutomationRevision };
  onCancel: () => void;
  onSaved: (automation: Automation, action: "created" | "revised") => void;
};
type TriggerKind = "SCHEDULE" | "ONE_SHOT" | "MANUAL";
type RetryPolicy = AutomationExecutionPolicy["retry_policy"];
const MAX_ROUTINE_REVISION_PAGES = 100;
const CRON_FIELD = "[0-9*/,-]+";
const CRON_FIVE_FIELDS = new RegExp(`^${CRON_FIELD}(?:\\s+${CRON_FIELD}){4}$`);

async function findRoutineRevision(api: AutomationApi, workspaceId: string, routineId: string, revision: number, signal?: AbortSignal) {
  const routine = await api.getRoutine(routineId, signal);
  if (routine.workspace_id !== workspaceId || routine.routine_id !== routineId) throw new Error("Selected Routine is outside this Workspace.");
  let cursor: string | undefined;
  for (let pageIndex = 0; pageIndex < MAX_ROUTINE_REVISION_PAGES; pageIndex += 1) {
    const page = await api.listRoutineRevisions(routineId, cursor, signal);
    if (page.items.some(item => item.routine_id !== routineId)) throw new Error("Routine revision page identity mismatch.");
    const found = page.items.find(item => item.revision === revision);
    if (found) return { routine, revision: found };
    if (!page.next_cursor) break;
    cursor = page.next_cursor;
  }
  throw new Error("The selected immutable Routine revision could not be found in this Workspace.");
}

function errorText(error: unknown): string { return error instanceof Error ? error.message : "The Automation could not be saved."; }
function idempotencyKey(): string {
  const randomUUID = globalThis.crypto?.randomUUID;
  if (!randomUUID) throw new Error("This desktop session cannot create secure request identities. Restart LiteCowork before saving.");
  return randomUUID.call(globalThis.crypto);
}
function randomTriggerId(): string { return `trigger_${idempotencyKey().replaceAll("-", "")}`; }
function currentTimezone(): string { return Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC"; }
function toLocalInput(utc: unknown): string {
  if (typeof utc !== "string") return "";
  const date = new Date(utc);
  return Number.isNaN(date.getTime()) ? "" : new Date(date.getTime() - date.getTimezoneOffset() * 60_000).toISOString().slice(0, 16);
}
function decodeTrigger(existing: AutomationRevision | undefined): { kind: TriggerKind; cron: string; timezone: string; scheduledAt: string; misfire: "SKIP" | "RUN_ONCE_WHEN_AVAILABLE" } {
  const first = existing?.triggers.length === 1 ? existing.triggers[0] : null;
  const trigger = first?.trigger && typeof first.trigger === "object" ? first.trigger as Record<string, unknown> : null;
  if (trigger?.kind === "SCHEDULE") return {
    kind: "SCHEDULE", cron: typeof trigger.rrule_or_cron === "string" ? trigger.rrule_or_cron : "0 9 * * 1-5",
    timezone: typeof trigger.timezone === "string" ? trigger.timezone : currentTimezone(), scheduledAt: "",
    misfire: trigger.misfire_policy && typeof trigger.misfire_policy === "object"
      && (trigger.misfire_policy as Record<string, unknown>).kind === "RUN_ONCE_WHEN_AVAILABLE" ? "RUN_ONCE_WHEN_AVAILABLE" : "SKIP",
  };
  if (trigger?.kind === "ONE_SHOT") return {
    kind: "ONE_SHOT", cron: "0 9 * * 1-5", timezone: currentTimezone(),
    scheduledAt: toLocalInput(trigger.scheduled_at),
    misfire: trigger.misfire_policy && typeof trigger.misfire_policy === "object"
      && (trigger.misfire_policy as Record<string, unknown>).kind === "RUN_ONCE_WHEN_AVAILABLE" ? "RUN_ONCE_WHEN_AVAILABLE" : "SKIP",
  };
  if (trigger?.kind === "MANUAL") return { kind: "MANUAL", cron: "0 9 * * 1-5", timezone: currentTimezone(), scheduledAt: "", misfire: "SKIP" };
  return { kind: "SCHEDULE", cron: "0 9 * * 1-5", timezone: currentTimezone(), scheduledAt: "", misfire: "SKIP" };
}
function defaultPolicy(): AutomationExecutionPolicy {
  return {
    placement_preference: "LOCAL_ONLY", max_concurrent_occurrences: 1, overlap_policy: "SKIP",
    retry_policy: { max_attempts: 0, initial_backoff_ms: 1000, max_backoff_ms: 30000, multiplier: 2, jitter: true, retryable_error_codes: [] },
    budget_ceiling: null, notification_policy: "ON_FAILURE", wake_policy: "NEVER",
  };
}
function boundedInteger(value: string, min: number, max: number): number | null {
  if (!/^\d+$/.test(value)) return null;
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) && parsed >= min && parsed <= max ? parsed : null;
}
function textRows(value: string): string[] { return value.split("\n").map(line => line.trim()).filter(Boolean); }

/** Create/revise writes inert definitions only; it never enables or runs an Automation. */
export function AutomationEditor({ api, workspaceId, existing, onCancel, onSaved }: Props) {
  const [name, setName] = useState(existing?.automation.name ?? "");
  const [routines, setRoutines] = useState<Routine[]>([]);
  const [routineCursor, setRoutineCursor] = useState<string | null>(null);
  const [routinesBusy, setRoutinesBusy] = useState(true);
  const [routineBusy, setRoutineBusy] = useState(false);
  const [routineError, setRoutineError] = useState<string | null>(null);
  const [coworkers, setCoworkers] = useState<AutomationCoworker[]>([]);
  const [coworkerCursor, setCoworkerCursor] = useState<string | null>(null);
  const [coworkersBusy, setCoworkersBusy] = useState(true);
  const [coworkerError, setCoworkerError] = useState<string | null>(null);
  const initialCoworkerSelection = existing?.revision.coworker_ref ? "KEEP_EXISTING" : "NONE";
  const [coworkerSelection, setCoworkerSelection] = useState(initialCoworkerSelection);
  const [selectedRoutineId, setSelectedRoutineId] = useState(existing?.revision.routine_id ?? "");
  const [selectedRevision, setSelectedRevision] = useState(existing?.revision.routine_revision ?? 0);
  const [verifiedRevision, setVerifiedRevision] = useState<{ routine_id: string; revision: number; objective_template: string } | null>(null);
  const [triggerKind, setTriggerKind] = useState<TriggerKind>(decodeTrigger(existing?.revision).kind);
  const [cron, setCron] = useState(decodeTrigger(existing?.revision).cron);
  const [timezone, setTimezone] = useState(decodeTrigger(existing?.revision).timezone);
  const [scheduledAt, setScheduledAt] = useState(decodeTrigger(existing?.revision).scheduledAt);
  const [misfire, setMisfire] = useState<"SKIP" | "RUN_ONCE_WHEN_AVAILABLE">(decodeTrigger(existing?.revision).misfire);
  // Revising retains every exact trigger field by default. Trigger replacement is a
  // separate explicit choice, preventing hidden fields/IDs from being dropped.
  const [replaceTriggerSet, setReplaceTriggerSet] = useState(!existing);
  const basePolicy = existing?.revision.execution_policy ?? defaultPolicy();
  const [placement, setPlacement] = useState<AutomationExecutionPolicy["placement_preference"]>(basePolicy.placement_preference);
  const [maxConcurrent, setMaxConcurrent] = useState(String(basePolicy.max_concurrent_occurrences));
  const [overlap, setOverlap] = useState(basePolicy.overlap_policy);
  const [maxRetries, setMaxRetries] = useState(String(basePolicy.retry_policy.max_attempts));
  const [initialBackoff, setInitialBackoff] = useState(String(basePolicy.retry_policy.initial_backoff_ms));
  const [maxBackoff, setMaxBackoff] = useState(String(basePolicy.retry_policy.max_backoff_ms));
  const [multiplier, setMultiplier] = useState(String(basePolicy.retry_policy.multiplier));
  const [jitter, setJitter] = useState(basePolicy.retry_policy.jitter);
  const [retryCodes, setRetryCodes] = useState(basePolicy.retry_policy.retryable_error_codes.join("\n"));
  const [notification, setNotification] = useState(basePolicy.notification_policy);
  const [wakePolicy, setWakePolicy] = useState(basePolicy.wake_policy);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const routineListGeneration = useRef(0);
  const revisionCheckGeneration = useRef(0);
  const requestIds = useRef(new Map<string, string>());

  useEffect(() => {
    const controller = new AbortController();
    const ownGeneration = ++routineListGeneration.current;
    setRoutines([]); setRoutineCursor(null); setRoutinesBusy(true); setRoutineError(null); setVerifiedRevision(null);
    if (!workspaceId) { setRoutinesBusy(false); return () => controller.abort(); }
    void api.listRoutines(undefined, controller.signal).then(async page => {
      if (controller.signal.aborted || ownGeneration !== routineListGeneration.current) return;
      if (page.items.some(item => item.workspace_id !== workspaceId)) throw new Error("Routine list belongs to a different Workspace.");
      let nextRoutines = page.items;
      // A currently pinned archived Routine remains selectable only to preserve its exact
      // existing reference while the owner edits a paused Automation.
      if (existing && !nextRoutines.some(item => item.routine_id === existing.revision.routine_id)) {
        try {
          const pinned = await api.getRoutine(existing.revision.routine_id, controller.signal);
          if (pinned.workspace_id !== workspaceId || pinned.routine_id !== existing.revision.routine_id) throw new Error("Pinned Routine identity mismatch.");
          nextRoutines = [pinned, ...nextRoutines];
        } catch { /* A missing pinned dependency remains visible through the detail warning below. */ }
      }
      if (controller.signal.aborted || ownGeneration !== routineListGeneration.current) return;
      setRoutines(nextRoutines); setRoutineCursor(page.next_cursor);
      if (!selectedRoutineId) {
        const firstActive = nextRoutines.find(item => item.status === "ACTIVE");
        if (firstActive) { setSelectedRoutineId(firstActive.routine_id); setSelectedRevision(firstActive.current_revision); }
      }
    }).catch(problem => {
      if (!controller.signal.aborted && ownGeneration === routineListGeneration.current) setRoutineError(errorText(problem));
    }).finally(() => {
      if (!controller.signal.aborted && ownGeneration === routineListGeneration.current) setRoutinesBusy(false);
    });
    return () => controller.abort();
  }, [api, workspaceId]);

  useEffect(() => {
    const controller = new AbortController();
    setCoworkers([]); setCoworkerCursor(null); setCoworkersBusy(true); setCoworkerError(null);
    if (!workspaceId) { setCoworkersBusy(false); return () => controller.abort(); }
    void api.listCoworkers(undefined, controller.signal).then(async page => {
      if (controller.signal.aborted) return;
      if (page.items.some(item => item.workspace_id !== workspaceId)) throw new Error("Coworker list belongs to a different Workspace.");
      let available = page.items;
      const pinnedId = existing?.revision.coworker_ref?.coworker_id;
      if (pinnedId && !available.some(item => item.coworker_id === pinnedId)) {
        const pinned = await api.getCoworker(pinnedId, controller.signal);
        if (pinned.workspace_id !== workspaceId || pinned.coworker_id !== pinnedId) throw new Error("Pinned Coworker identity mismatch.");
        available = [pinned, ...available];
      }
      if (controller.signal.aborted) return;
      setCoworkers(available); setCoworkerCursor(page.next_cursor);
    }).catch(problem => {
      if (!controller.signal.aborted) setCoworkerError(errorText(problem));
    }).finally(() => { if (!controller.signal.aborted) setCoworkersBusy(false); });
    return () => controller.abort();
  }, [api, workspaceId, existing?.revision.coworker_ref?.coworker_id]);

  useEffect(() => {
    if (!selectedRoutineId || selectedRevision < 1) { setVerifiedRevision(null); return; }
    const controller = new AbortController();
    const ownGeneration = ++revisionCheckGeneration.current;
    setRoutineBusy(true); setRoutineError(null); setVerifiedRevision(null);
    void (async () => {
      const routine = await api.getRoutine(selectedRoutineId, controller.signal);
      if (routine.status === "ARCHIVED" && (!existing || existing.revision.routine_id !== routine.routine_id || existing.revision.routine_revision !== selectedRevision)) {
        throw new Error("Archived Routines can only be retained as the exact existing pinned reference.");
      }
      const verified = await findRoutineRevision(api, workspaceId, selectedRoutineId, selectedRevision, controller.signal);
      if (selectedRevision === routine.current_revision && routine.status !== "ACTIVE") throw new Error("The current Routine revision is not active.");
      setVerifiedRevision(verified.revision);
    })().catch(problem => {
      if (!controller.signal.aborted && ownGeneration === revisionCheckGeneration.current) setRoutineError(errorText(problem));
    }).finally(() => {
      if (!controller.signal.aborted && ownGeneration === revisionCheckGeneration.current) setRoutineBusy(false);
    });
    return () => controller.abort();
  }, [api, selectedRoutineId, selectedRevision, workspaceId]);

  const selectedRoutine = routines.find(item => item.routine_id === selectedRoutineId) ?? null;
  const coworkerRefForSave = async (): Promise<CoworkerRevisionRef | null> => {
    if (coworkerSelection === "NONE") return null;
    if (coworkerSelection === "KEEP_EXISTING") return existing?.revision.coworker_ref ?? null;
    const [prefix, id, rawRevision] = coworkerSelection.split(":");
    const revision = Number(rawRevision);
    if (prefix !== "CURRENT" || !id || !Number.isSafeInteger(revision) || revision < 1) throw new Error("Choose a valid Coworker revision.");
    const coworker = await api.getCoworker(id);
    if (coworker.workspace_id !== workspaceId || coworker.coworker_id !== id) throw new Error("Selected Coworker is outside this Workspace.");
    if (coworker.status !== "ACTIVE") throw new Error("Only an active Coworker can be newly assigned to scheduled work.");
    if (coworker.current_revision !== revision) throw new Error("This Coworker changed after selection. Refresh and choose its current revision again.");
    return { coworker_id: id, revision };
  };
  const triggerSummary = existing?.revision.triggers.map((spec, index) => {
    const definition = spec.trigger && typeof spec.trigger === "object" ? spec.trigger as Record<string, unknown> : {};
    return `${index + 1}. ${String(definition.kind ?? "Unknown trigger")} · ${String(spec.trigger_id ?? "ID unavailable")}`;
  }).join("; ") ?? "";

  const buildTriggers = (): TriggerSpec[] => {
    if (existing && !replaceTriggerSet) return existing.revision.triggers;
    if (triggerKind === "MANUAL") {
      const existingSpec = existing?.revision.triggers.length === 1 ? existing.revision.triggers[0] : null;
      const existingDef = existingSpec?.trigger && typeof existingSpec.trigger === "object" ? existingSpec.trigger as Record<string, unknown> : null;
      const id = existingDef?.kind === "MANUAL" && typeof existingSpec?.trigger_id === "string" ? existingSpec.trigger_id : randomTriggerId();
      return [{ trigger_id: id, placement: "AUTO", runtime_id: null, trigger: { kind: "MANUAL" } }];
    }
    if (triggerKind === "SCHEDULE") {
      if (!CRON_FIVE_FIELDS.test(cron.trim()) || cron.trim().length > 128) throw new Error("Enter a five-field cron expression using bounded numeric syntax (maximum 128 characters).");
      if (!timezone.trim() || timezone.length > 80) throw new Error("Enter an IANA time zone (maximum 80 characters).");
      try { new Intl.DateTimeFormat("en", { timeZone: timezone.trim() }).format(); } catch { throw new Error("That time zone is not supported by this desktop."); }
      const existingSpec = existing?.revision.triggers.length === 1 ? existing.revision.triggers[0] : null;
      const existingDef = existingSpec?.trigger && typeof existingSpec.trigger === "object" ? existingSpec.trigger as Record<string, unknown> : null;
      const id = existingDef?.kind === "SCHEDULE" && typeof existingSpec?.trigger_id === "string" ? existingSpec.trigger_id : randomTriggerId();
      return [{ trigger_id: id, placement: "AUTO", runtime_id: null, trigger: {
        kind: "SCHEDULE", recurrence_format: "CRON_5", timezone: timezone.trim(), rrule_or_cron: cron.trim(),
        recurrence_semantics_version: 1, start_at: null, end_at: null,
        misfire_policy: { kind: misfire }, ambiguous_local_time: "EARLIER", nonexistent_local_time: "SKIP",
      } }];
    }
    if (!scheduledAt) throw new Error("Choose a date and time for this one-shot trigger.");
    const parsed = new Date(scheduledAt);
    if (Number.isNaN(parsed.getTime())) throw new Error("One-shot date and time are invalid.");
    const scheduledUtc = parsed.toISOString();
    const existingSpec = existing?.revision.triggers.length === 1 ? existing.revision.triggers[0] : null;
    const existingDef = existingSpec?.trigger && typeof existingSpec.trigger === "object" ? existingSpec.trigger as Record<string, unknown> : null;
    const id = existingDef?.kind === "ONE_SHOT" && existingDef.scheduled_at === scheduledUtc && typeof existingSpec?.trigger_id === "string"
      ? existingSpec.trigger_id : randomTriggerId();
    return [{ trigger_id: id, placement: "AUTO", runtime_id: null, trigger: {
      kind: "ONE_SHOT", scheduled_at: scheduledUtc, misfire_policy: { kind: misfire },
    } }];
  };

  const buildPolicy = (): AutomationExecutionPolicy => {
    const concurrent = boundedInteger(maxConcurrent, 1, 10);
    const retries = boundedInteger(maxRetries, 0, 5);
    const initial = boundedInteger(initialBackoff, 0, 60_000);
    const maximum = boundedInteger(maxBackoff, 0, 300_000);
    const parsedMultiplier = Number(multiplier);
    const codes = textRows(retryCodes);
    if (concurrent === null || retries === null || initial === null || maximum === null || maximum < initial) throw new Error("Concurrency and retry values must fit the stated limits; maximum backoff must be at least the initial backoff.");
    if (!Number.isFinite(parsedMultiplier) || parsedMultiplier < 1 || parsedMultiplier > 5) throw new Error("Retry multiplier must be between 1 and 5.");
    if (codes.length > 20 || codes.some(code => code.length > 64 || /[\u0000-\u001f\u007f]/.test(code)) || new Set(codes).size !== codes.length) throw new Error("Use up to 20 unique retry error codes, each no longer than 64 characters.");
    const retry: RetryPolicy = { max_attempts: retries, initial_backoff_ms: initial, max_backoff_ms: maximum, multiplier: parsedMultiplier, jitter, retryable_error_codes: codes };
    return {
      placement_preference: placement, max_concurrent_occurrences: concurrent, overlap_policy: overlap, retry_policy: retry,
      budget_ceiling: existing?.revision.execution_policy.budget_ceiling ?? null,
      notification_policy: notification, wake_policy: wakePolicy,
    };
  };

  const save = async () => {
    if (saving) return;
    setError(null); setNotice(null);
    try {
      if (!workspaceId || !selectedRoutine || selectedRoutine.workspace_id !== workspaceId) throw new Error("Select a saved Routine in the current Workspace.");
      if (name.trim().length === 0 || name.trim().length > 120) throw new Error("Automation name must be between 1 and 120 characters.");
      if (!verifiedRevision || verifiedRevision.routine_id !== selectedRoutineId || verifiedRevision.revision !== selectedRevision) throw new Error("Wait for the exact saved Routine revision to be verified before saving.");
      // Re-read head and immutable revision immediately before mutation. The API/store
      // transaction rechecks Workspace ownership and exact Routine existence as well.
      const { routine: currentRoutine, revision: freshRevision } = await findRoutineRevision(api, workspaceId, selectedRoutineId, selectedRevision);
      const isExistingArchivedPin = currentRoutine.status === "ARCHIVED" && existing?.revision.routine_id === selectedRoutineId && existing.revision.routine_revision === selectedRevision;
      if (currentRoutine.status !== "ACTIVE" && !isExistingArchivedPin) throw new Error("The selected Routine is no longer active. Refresh the selector.");
      if (freshRevision.routine_id !== selectedRoutineId || freshRevision.revision !== selectedRevision) throw new Error("The exact pinned Routine revision could not be revalidated. Refresh the selector.");
      const input: AutomationDefinitionInput = {
        name: name.trim(), routine_id: selectedRoutineId, routine_revision: selectedRevision,
        triggers: buildTriggers(), execution_policy: buildPolicy(), coworker_ref: await coworkerRefForSave(),
      };
      const signature = JSON.stringify([workspaceId, existing?.automation.automation_id ?? "create", existing?.automation.version ?? 0, input]);
      let requestId = requestIds.current.get(signature);
      if (!requestId) { requestId = idempotencyKey(); requestIds.current.set(signature, requestId); }
      setSaving(true);
      const updated = existing
        ? await api.reviseDefinition(existing.automation.automation_id, existing.automation.version, input, requestId)
        : await api.createDefinition(input, requestId);
      if (updated.status !== "PAUSED") throw new Error("The server did not return the required PAUSED state. Refresh before continuing.");
      requestIds.current.delete(signature);
      onSaved(updated, existing ? "revised" : "created");
    } catch (problem) {
      setError(errorText(problem));
    } finally { setSaving(false); }
  };

  return (
    <section className="automation-editor" aria-label={existing ? "Revise Automation" : "Create Automation"}>
      <header className="automation-editor-header"><div><div className="eyebrow">PAUSED DEFINITION</div><h2>{existing ? `Revise ${existing.automation.name}` : "New Automation"}</h2><p>Saving creates or revises a paused record only. No trigger will run and no Task will start.</p></div><button type="button" className="quiet-button" onClick={onCancel} disabled={saving}>Cancel</button></header>
      <div className="automation-editor-fields">
        <label className="automation-field"><span>Name</span><input value={name} maxLength={120} onChange={event => setName(event.currentTarget.value)} placeholder="Weekday project summary" /></label>
        <label className="automation-field"><span>Saved Routine</span><select value={selectedRoutineId} disabled={routinesBusy || saving} onChange={event => {
          const selected = routines.find(item => item.routine_id === event.currentTarget.value);
          setSelectedRoutineId(event.currentTarget.value); setVerifiedRevision(null);
          if (selected) setSelectedRevision(selected.current_revision);
        }}>
          <option value="">Choose a saved Routine</option>
          {routines.map(routine => <option key={routine.routine_id} value={routine.routine_id} disabled={routine.status === "ARCHIVED" && (!existing || routine.routine_id !== existing.revision.routine_id)}>{routine.name} · {routine.status === "ACTIVE" ? "Current" : "Archived"} revision {routine.current_revision}</option>)}
        </select><small>Only the selected Workspace’s stored Routine and verified immutable revision are accepted.</small></label>
        <label className="automation-field"><span>Coworker responsibility</span><select value={coworkerSelection} disabled={coworkersBusy || saving} onChange={event => setCoworkerSelection(event.currentTarget.value)}>
          <option value="NONE">No Coworker pin</option>
          {existing?.revision.coworker_ref && <option value="KEEP_EXISTING">Keep {coworkers.find(item => item.coworker_id === existing.revision.coworker_ref?.coworker_id)?.name ?? "existing Coworker"} · pinned revision {existing.revision.coworker_ref.revision}</option>}
          {coworkers.filter(item => item.status === "ACTIVE").map(item => <option key={`${item.coworker_id}:${item.current_revision}`} value={`CURRENT:${item.coworker_id}:${item.current_revision}`}>{item.name} · current revision {item.current_revision}</option>)}
        </select><small>A pin snapshots this Coworker’s lead and worker preferences. Later Coworker edits do not change this Automation revision. Creation stays paused.</small></label>
        {existing?.revision.coworker_ref && coworkerSelection === "KEEP_EXISTING" && <p className="automation-policy-note">Keeping exact Coworker pin {existing.revision.coworker_ref.coworker_id} · revision {existing.revision.coworker_ref.revision}.</p>}
        {coworkerError && <p className="automation-inline-error" role="alert">Coworkers could not be loaded: {coworkerError}</p>}
        {coworkerCursor && <button type="button" className="text-button automation-routine-more" disabled={coworkersBusy || saving} onClick={() => {
          setCoworkersBusy(true); setCoworkerError(null);
          void api.listCoworkers(coworkerCursor).then(page => {
            if (page.items.some(item => item.workspace_id !== workspaceId)) throw new Error("Coworker page belongs to another Workspace.");
            setCoworkers(current => [...current, ...page.items.filter(item => !current.some(old => old.coworker_id === item.coworker_id))]);
            setCoworkerCursor(page.next_cursor);
          }).catch(problem => setCoworkerError(errorText(problem))).finally(() => setCoworkersBusy(false));
        }}>Load more Coworkers</button>}
        {selectedRoutine && <div className="automation-selected-revision" role="status">
          <span>{verifiedRevision ? `Verified Routine revision ${verifiedRevision.revision}` : routineBusy ? "Verifying saved Routine revision…" : "Routine revision not verified"}</span>
          {existing && selectedRoutine.routine_id === existing.revision.routine_id && existing.revision.routine_revision !== selectedRoutine.current_revision && <button type="button" className="text-button" disabled={routineBusy || saving} onClick={() => { setVerifiedRevision(null); setSelectedRevision(existing.revision.routine_revision); }}>Keep pinned revision {existing.revision.routine_revision}</button>}
          {selectedRevision !== selectedRoutine.current_revision && <button type="button" className="text-button" disabled={routineBusy || saving} onClick={() => { setVerifiedRevision(null); setSelectedRevision(selectedRoutine.current_revision); }}>Use current revision {selectedRoutine.current_revision}</button>}
        </div>}
        {routineCursor && <button type="button" className="text-button automation-routine-more" disabled={routinesBusy || saving} onClick={() => void api.listRoutines(routineCursor).then(page => {
          if (page.items.some(item => item.workspace_id !== workspaceId)) throw new Error("Routine page belongs to another Workspace.");
          setRoutines(current => [...current, ...page.items.filter(item => !current.some(old => old.routine_id === item.routine_id))]); setRoutineCursor(page.next_cursor);
        }).catch(problem => setRoutineError(errorText(problem)))}>Load more Routines</button>}

        {existing && existing.revision.triggers.length > 0 && <div className="automation-trigger-preserve">
          <strong>Existing trigger set</strong><span>{triggerSummary}</span>
          <label><input type="checkbox" checked={!replaceTriggerSet} disabled={saving} onChange={event => setReplaceTriggerSet(!event.currentTarget.checked)} /> Keep all existing triggers exactly as saved</label>
          <small>Replacing the set removes its current trigger IDs and creates one new trigger. Existing triggers are otherwise preserved without edits.</small>
        </div>}
        {(!existing || replaceTriggerSet) && <fieldset className="automation-fieldset"><legend>Trigger definition</legend>
          <div className="automation-trigger-kinds" role="group" aria-label="Trigger type"><button type="button" className={triggerKind === "SCHEDULE" ? "selected" : ""} disabled={saving} onClick={() => setTriggerKind("SCHEDULE")}>Recurring schedule</button><button type="button" className={triggerKind === "ONE_SHOT" ? "selected" : ""} disabled={saving} onClick={() => setTriggerKind("ONE_SHOT")}>One time</button><button type="button" className={triggerKind === "MANUAL" ? "selected" : ""} disabled={saving} onClick={() => setTriggerKind("MANUAL")}>Manual</button></div>
          {triggerKind === "SCHEDULE" ? <>
            <label className="automation-field"><span>Cron schedule · five fields</span><input value={cron} maxLength={128} onChange={event => setCron(event.currentTarget.value)} placeholder="0 9 * * 1-5"/><small>Syntax is bounded here; provider scheduling and timezone execution are not active.</small></label>
            <label className="automation-field"><span>IANA time zone</span><input value={timezone} maxLength={80} onChange={event => setTimezone(event.currentTarget.value)} placeholder="America/New_York"/></label>
          </> : triggerKind === "ONE_SHOT" ? <label className="automation-field"><span>Scheduled date and time</span><input type="datetime-local" value={scheduledAt} onChange={event => setScheduledAt(event.currentTarget.value)} /></label> : <p className="automation-policy-note">Manual trigger only. Saving keeps this definition paused. You can explicitly create one READY Task from the definition; no schedule is started.</p>}
          {triggerKind !== "MANUAL" && <label className="automation-field"><span>Missed trigger policy</span><select value={misfire} onChange={event => setMisfire(event.currentTarget.value as typeof misfire)}><option value="SKIP">Skip missed time</option><option value="RUN_ONCE_WHEN_AVAILABLE">One run when available</option></select><small>This is saved policy only. This build does not schedule occurrences.</small></label>}
        </fieldset>}

        <fieldset className="automation-fieldset"><legend>Execution policy</legend>
          <label className="automation-field"><span>Task placement preference</span><select value={typeof placement === "string" ? placement : "SPECIFIC_RUNTIME"} onChange={event => {
            const value = event.currentTarget.value;
            if (value !== "SPECIFIC_RUNTIME") setPlacement(value as AutomationExecutionPolicy["placement_preference"]);
          }}>
            <option value="AUTO">Automatic</option><option value="LOCAL_ONLY">This desktop</option><option value="CLOUD_PREFERRED">Cloud preferred</option><option value="CLOUD_ONLY">Cloud only</option>
            {typeof placement !== "string" && <option value="SPECIFIC_RUNTIME">Specific Runtime (preserved)</option>}
          </select></label>
          <label className="automation-field"><span>Maximum overlapping occurrences · 1–10</span><input type="number" min={1} max={10} step={1} value={maxConcurrent} onChange={event => setMaxConcurrent(event.currentTarget.value)} /></label>
          <label className="automation-field"><span>Overlap behavior</span><select value={overlap} onChange={event => setOverlap(event.currentTarget.value as AutomationExecutionPolicy["overlap_policy"])}>{["SKIP", "QUEUE", "CANCEL_OLD", "ALLOW"].map(value => <option key={value} value={value}>{value.replaceAll("_", " ")}</option>)}</select></label>
          <label className="automation-field"><span>Retry attempts · 0–5</span><input type="number" min={0} max={5} step={1} value={maxRetries} onChange={event => setMaxRetries(event.currentTarget.value)} /></label>
          <div className="automation-two-fields"><label className="automation-field"><span>Initial backoff · 0–60,000 ms</span><input type="number" min={0} max={60000} step={1000} value={initialBackoff} onChange={event => setInitialBackoff(event.currentTarget.value)} /></label><label className="automation-field"><span>Maximum backoff · 0–300,000 ms</span><input type="number" min={0} max={300000} step={1000} value={maxBackoff} onChange={event => setMaxBackoff(event.currentTarget.value)} /></label></div>
          <div className="automation-two-fields"><label className="automation-field"><span>Retry multiplier · 1–5</span><input type="number" min={1} max={5} step={0.1} value={multiplier} onChange={event => setMultiplier(event.currentTarget.value)} /></label><label className="automation-check"><input type="checkbox" checked={jitter} onChange={event => setJitter(event.currentTarget.checked)} /> Add retry timing jitter</label></div>
          <label className="automation-field"><span>Retryable error codes · one per line, max 20</span><textarea rows={3} value={retryCodes} maxLength={1400} onChange={event => setRetryCodes(event.currentTarget.value)} placeholder="AGENT_UNAVAILABLE" /></label>
          <label className="automation-field"><span>Notifications</span><select value={notification} onChange={event => setNotification(event.currentTarget.value as AutomationExecutionPolicy["notification_policy"])}>{["ALWAYS", "ON_SUCCESS", "ON_FAILURE", "ON_CONDITION", "SILENT"].map(value => <option key={value} value={value}>{value.replaceAll("_", " ")}</option>)}</select></label>
          <label className="automation-field"><span>Runtime wake policy</span><select value={wakePolicy} onChange={event => setWakePolicy(event.currentTarget.value as AutomationExecutionPolicy["wake_policy"])}>{["NEVER", "TRY_WAKE", "REQUIRE_RUNTIME_AWAKE"].map(value => <option key={value} value={value}>{value.replaceAll("_", " ")}</option>)}</select></label>
          <p className="automation-policy-note">Maximum is bounded to 10 concurrent occurrences and 5 retries. Any existing budget ceiling is preserved exactly; this editor cannot raise or remove it. Saving does not authorize a Task or start a Runtime.</p>
        </fieldset>
      </div>
      {routineError && <p className="automation-inline-error" role="alert">{routineError}</p>}
      {error && <p className="automation-inline-error" role="alert">{error}</p>}
      {notice && <p className="automation-inline-success" role="status">{notice}</p>}
      <footer className="automation-editor-footer"><button type="button" className="quiet-button" onClick={onCancel} disabled={saving}>Cancel</button><button type="button" className="primary-button" disabled={saving || routinesBusy || routineBusy || coworkersBusy || !verifiedRevision} onClick={() => void save()}>{saving ? "Saving paused definition…" : existing ? "Save new revision · paused" : "Create paused definition"}</button></footer>
    </section>
  );
}
