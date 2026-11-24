import type { WorkspaceView } from "../App";
import type { WorkspaceListState } from "../workspaces/WorkspaceFirstRun";
import "./workspace-projects-page.css";

type Props = {
  workspaces: readonly WorkspaceView[];
  selectedWorkspaceId: string;
  listState: WorkspaceListState;
  error: string | null;
  runtimeReady: boolean;
  busy?: boolean;
  onRetry: () => void;
  onOpenWorkspace: (workspaceId: string) => void;
  onOpenSettings: () => void;
};

function WorkspaceStatus({ status }: { status: string }) {
  const label = status === "ACTIVE" ? "Active" : status === "ARCHIVED" ? "Archived" : "Status unavailable";
  return <span className={`workspace-project-status${status === "ACTIVE" ? " is-active" : ""}`}>{label}</span>;
}

export function WorkspaceProjectsPage({
  workspaces,
  selectedWorkspaceId,
  listState,
  error,
  runtimeReady,
  busy = false,
  onRetry,
  onOpenWorkspace,
  onOpenSettings,
}: Props) {
  return (
    <div className="page-content subpage-content workspace-projects-page">
      <div className="eyebrow">YOUR WORKSPACE</div>
      <h1>Projects</h1>
      <p className="workspace-projects-intro">
        Each project is an existing Workspace. Workspace is the security and resource boundary;
        chats, files and work stay within it.
      </p>

      {listState === "LOADING" ? (
        <section className="subpage-panel workspace-projects-state" role="status">Loading Workspaces…</section>
      ) : !runtimeReady ? (
        <section className="subpage-panel workspace-projects-state" role="status">
          <h2>Local Runtime is unavailable</h2>
          <p>Workspace rows are hidden until the local Runtime is ready. Start it, then refresh the Workspace list.</p>
          <button className="quiet-button" type="button" onClick={onOpenSettings}>Open Settings</button>
        </section>
      ) : listState === "ERROR" ? (
        <section className="subpage-panel workspace-projects-state" role="alert">
          <h2>Projects are unavailable</h2>
          <p>{error ?? "The authenticated Workspace list could not be loaded."}</p>
          <button className="quiet-button" type="button" onClick={onRetry} disabled={busy}>Retry</button>
        </section>
      ) : workspaces.length === 0 ? (
        <section className="subpage-panel workspace-projects-state">
          <h2>No Workspaces yet</h2>
          <p>Create or select a Workspace before starting a chat or adding resources.</p>
          <button className="quiet-button" type="button" onClick={onOpenSettings}>Open Workspace settings</button>
        </section>
      ) : (
        <section className="workspace-project-list" aria-label="Workspaces">
          {workspaces.map((workspace) => {
            const selected = workspace.workspaceId === selectedWorkspaceId;
            return (
              <article className={`workspace-project-card${selected ? " is-selected" : ""}`} key={workspace.workspaceId}>
                <div className="workspace-project-card-copy">
                  <h2>{workspace.name}</h2>
                  <WorkspaceStatus status={workspace.status} />
                </div>
                <button
                  className="quiet-button"
                  type="button"
                  aria-pressed={selected}
                  disabled={busy}
                  onClick={() => onOpenWorkspace(workspace.workspaceId)}
                >
                  {selected ? "Open current Workspace" : "Open Workspace"}
                </button>
              </article>
            );
          })}
        </section>
      )}
    </div>
  );
}
