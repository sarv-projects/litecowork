import { canOfferLocalRuntimeStart } from "../runtime/runtime-start-readiness";
import type { PendingWorkspaceCreate } from "./workspace-onboarding";

type RuntimeStatus = {
  state: string;
  processRunning: boolean;
  operatorReady: boolean;
  daemonAvailable: boolean;
  blockers: string[];
};

export type WorkspaceListState = "LOADING" | "READY" | "ERROR";

export function WorkspaceFirstRun({
  runtime,
  runtimeError,
  runtimeBusy,
  onStartRuntime,
  onRefreshRuntime,
  workspaceListState,
  workspaceError,
  onRefreshWorkspaces,
  workspaceName,
  onWorkspaceNameChange,
  workspaceCreateBusy,
  workspaceCreatePending,
  onCreateWorkspace,
  onOpenSettings,
  workspacesCount,
}: {
  runtime: RuntimeStatus | null;
  runtimeError: string | null;
  runtimeBusy: boolean;
  onStartRuntime: () => Promise<void>;
  onRefreshRuntime: () => Promise<void>;
  workspaceListState: WorkspaceListState;
  workspaceError: string | null;
  onRefreshWorkspaces: () => Promise<boolean>;
  workspaceName: string;
  onWorkspaceNameChange: (name: string) => void;
  workspaceCreateBusy: boolean;
  workspaceCreatePending: PendingWorkspaceCreate | null;
  onCreateWorkspace: () => Promise<void>;
  onOpenSettings: () => void;
  workspacesCount: number;
}) {
  const pendingName = workspaceCreatePending?.name ?? null;
  const canStart = canOfferLocalRuntimeStart(runtime);

  return (
    <section className="workspace-first-run workspace-list-card" aria-labelledby="first-workspace-title">
      <div className="eyebrow">LOCAL DESKTOP SETUP</div>
      <h2 id="first-workspace-title">{workspacesCount === 0 ? "Set up your first Workspace" : "Restore your Workspace selection"}</h2>
      <p className="workspace-first-run-intro">
        {workspacesCount === 0
          ? "Workspaces keep your local work and Resources together on this computer. Create one to continue."
          : "LiteCowork is restoring your saved Workspace selection. Choose the saved Workspace in Settings if it does not return."}
      </p>

      {workspaceListState === "LOADING" && (
        <p role="status" className="composer-hint">Checking local Workspaces…</p>
      )}

      {workspaceListState === "ERROR" && (
        <div className="workspace-first-run-state" role="alert">
          <strong>Local Workspaces could not be checked</strong>
          <p>{workspaceError ?? "The authenticated local Operator did not return a Workspace list."}</p>
          <button className="quiet-button" type="button" onClick={() => void onRefreshWorkspaces()}>Retry Workspace check</button>
        </div>
      )}

      {workspaceListState === "READY" && workspacesCount > 0 && (
        <p role="status" className="composer-hint">
          Restoring your saved Workspace selection. If it remains unavailable, choose a local Workspace in Settings.
          <button className="text-button" type="button" onClick={onOpenSettings}>Open Workspace settings</button>
        </p>
      )}

      {runtime?.operatorReady !== true && (
        <div className="workspace-first-run-state" role="status">
          <strong>
            {runtime === null
              ? runtimeError ? "Local Runtime status is unavailable" : "Checking the local Runtime"
              : runtime.processRunning
                ? "Local Runtime is reconnecting"
                : canStart
                  ? "Local Runtime is offline"
                  : "Local Runtime is unavailable"}
          </strong>
          <p>
            {runtimeError
              ?? (runtime === null
                ? "The desktop has not confirmed authenticated local readiness yet."
                : runtime.processRunning
                  ? "The daemon process is running, but the authenticated Operator API has not confirmed this Runtime incarnation."
                  : canStart
                    ? "Start the local Runtime to create an encrypted local Workspace."
                    : runtime.blockers.length > 0
                      ? `Workspace setup is blocked: ${runtime.blockers.join(", ")}.`
                      : "The local daemon is not available. Refresh its status or open Runtime settings.")}
          </p>
          <div className="button-row">
            {canStart && <button className="primary-button" type="button" disabled={runtimeBusy} onClick={() => void onStartRuntime()}>{runtimeBusy ? "Starting Runtime…" : "Start local Runtime"}</button>}
            <button className="quiet-button" type="button" onClick={() => void onRefreshRuntime()}>Refresh Runtime status</button>
            {!canStart && <button className="text-button" type="button" onClick={onOpenSettings}>Open Runtime settings</button>}
          </div>
        </div>
      )}

      {workspaceListState === "READY" && workspacesCount === 0 && runtime?.operatorReady === true && (
        <form className="workspace-create-form" aria-label="Create your first Workspace" onSubmit={(event) => { event.preventDefault(); void onCreateWorkspace(); }}>
          <label htmlFor="first-workspace-name">Workspace name</label>
          <input
            id="first-workspace-name"
            type="text"
            value={workspaceName}
            maxLength={160}
            autoComplete="organization"
            aria-describedby="first-workspace-help"
            disabled={workspaceCreateBusy || pendingName !== null}
            onChange={(event) => onWorkspaceNameChange(event.currentTarget.value)}
            placeholder="For example, Personal or Studio"
          />
          <button className="primary-button" type="submit" disabled={workspaceCreateBusy || !workspaceName.trim() || runtime.operatorReady !== true}>
            {workspaceCreateBusy ? "Creating Workspace…" : pendingName !== null ? "Retry Workspace creation" : "Create local Workspace"}
          </button>
          {pendingName !== null && <p className="composer-hint" role="status">The previous response was interrupted. Retry “{pendingName}” to confirm the original request; its idempotency key is retained.</p>}
          {workspaceError && <p className="inline-error" role="alert">{workspaceError}</p>}
          <p id="first-workspace-help" className="composer-hint">Stored locally with the LOCAL_ONLY policy. Creating a Workspace does not start a Coworker, Task, or agent.</p>
        </form>
      )}
    </section>
  );
}
