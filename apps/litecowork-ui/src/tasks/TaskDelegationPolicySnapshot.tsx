import type { components } from "../api/operator-api";
import "./task-delegation-policy-snapshot.css";

export type DelegationPolicySnapshot = components["schemas"]["ResolvedDelegationPolicySnapshot"];

function sourceLabel(source: DelegationPolicySnapshot["source"]): string {
  return source === "WORKSPACE_DEFAULT" ? "Workspace defaults" : "Coworker override";
}

function strategyLabel(strategy: DelegationPolicySnapshot["strategy"]): string {
  switch (strategy) {
    case "NATIVE_DEFAULT": return "Native agent default";
    case "BALANCED": return "Balanced";
    case "COST_SAVER": return "Cost saver";
    case "HOST_DELEGATION_ONLY": return "Host delegation only";
  }
}

function childModelPolicyLabel(policy: DelegationPolicySnapshot["native_child_model_policy"]): string {
  return policy === "AGENT_DEFAULT" ? "Let the native agent choose" : "Inherit the parent model when supported";
}

function profilePolicyLabel(policy: DelegationPolicySnapshot["worker_profiles"][number]["worker_model_selection_policy"]): string {
  return policy === "PROFILE_CONFIGURED" ? "Use the profile’s configured model" : "Match the parent only on an exact available route";
}

function formatResolvedAt(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString();
}

export function TaskDelegationPolicySnapshot({ snapshot }: { snapshot: DelegationPolicySnapshot | null }) {
  if (snapshot === null) {
    return (
      <section className="task-delegation-policy" aria-label="Delegation policy snapshot">
        <h3>Delegation policy at this revision</h3>
        <p>This legacy Task revision has no saved policy snapshot. Its delegation policy is unavailable.</p>
      </section>
    );
  }

  return (
    <section className="task-delegation-policy" aria-label="Delegation policy snapshot">
      <h3>Delegation policy at this revision</h3>
      <dl>
        <div><dt>Policy source</dt><dd>{sourceLabel(snapshot.source)}</dd></div>
        <div><dt>Workspace settings</dt><dd>Revision {snapshot.workspace_settings_revision}</dd></div>
        <div><dt>Coworker settings</dt><dd>{snapshot.coworker_id && snapshot.coworker_revision !== null
          ? `${snapshot.coworker_id} · revision ${snapshot.coworker_revision}`
          : "No Coworker revision pinned"}</dd></div>
        <div><dt>Delegation strategy</dt><dd>{strategyLabel(snapshot.strategy)}</dd></div>
        <div><dt>Native child model</dt><dd>{childModelPolicyLabel(snapshot.native_child_model_policy)}</dd></div>
        <div><dt>Resolved</dt><dd><time dateTime={snapshot.resolved_at}>{formatResolvedAt(snapshot.resolved_at)}</time></dd></div>
      </dl>
      <div className="task-delegation-policy-profiles">
        <h4>Worker profiles in scope</h4>
        {snapshot.worker_profiles.length === 0
          ? <p>No worker profiles are pinned for this strategy.</p>
          : <ul>{snapshot.worker_profiles.map((profile) => (
            <li key={`${profile.profile_id}:${profile.revision}`}>
              <strong>{profile.profile_id} · revision {profile.revision}</strong>
              <small>{profilePolicyLabel(profile.worker_model_selection_policy)}</small>
            </li>
          ))}</ul>}
      </div>
      <p className="task-delegation-policy-note">This records the policy pinned to the Task specification. It does not indicate that a worker has run or been qualified.</p>
    </section>
  );
}
