import { useEffect, useRef, useState, type Ref } from "react";
import "./workbench-panel.css";

type WorkbenchResource = {
  resourceId: string;
  resourceRevisionId: string;
  displayName: string;
  mediaType: string;
};

type WorkbenchPanelProps = {
  panelRef: Ref<HTMLElement>;
  modal: boolean;
  workspaceId: string;
  workspaceName: string;
  runtimeState: string | null;
  operatorReady: boolean;
  resources: WorkbenchResource[];
  resourceNextCursor: string | null;
  resourcePageBusy: boolean;
  resourceBusy: boolean;
  resourceListFailed: boolean;
  previewResourceId: string | null;
  previewResourceRevisionId: string | null;
  previewResourceName: string | null;
  preview: string | null;
  previewError: string | null;
  onClose: () => void;
  onOpenLibrary: () => void;
  onOpenWork: () => void;
  onOpenSettings: () => void;
  onLoadMoreResources: () => void;
  onRefreshResources: () => void;
  onPreviewResource: (resource: WorkbenchResource) => void;
  onClosePreview: () => void;
};

function WorkbenchIcon({ kind }: { kind: "browser" | "terminal" | "files" | "computer" | "work" | "settings" }) {
  if (kind === "browser") return <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="9" /><path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18" /></svg>;
  if (kind === "terminal") return <svg viewBox="0 0 24 24" aria-hidden="true"><path d="m4 5 6 7-6 7M12 19h8" /></svg>;
  if (kind === "files") return <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" /><path d="M14 2v6h6" /></svg>;
  if (kind === "computer") return <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="3" y="3" width="18" height="13" rx="2" /><path d="M8 21h8M12 16v5" /></svg>;
  if (kind === "work") return <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="3" y="7" width="18" height="14" rx="2" /><path d="M8 7V5a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2M3 12h18" /></svg>;
  return <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="3" /><path d="m19.4 15 .1.1 1.4 1.1-1.4 2.4-1.7-.7a8 8 0 0 1-1.7 1l-.3 1.8h-2.8l-.3-1.8a8 8 0 0 1-1.7-1l-1.7.7-1.4-2.4 1.4-1.1a8 8 0 0 1 0-2l-1.4-1.1 1.4-2.4 1.7.7a8 8 0 0 1 1.7-1l.3-1.8h2.8l.3 1.8a8 8 0 0 1 1.7 1l1.7-.7 1.4 2.4-1.4 1.1a8 8 0 0 1-.1 2Z" /></svg>;
}

export function WorkbenchPanel({
  panelRef,
  modal,
  workspaceId,
  workspaceName,
  runtimeState,
  operatorReady,
  resources,
  resourceNextCursor,
  resourcePageBusy,
  resourceBusy,
  resourceListFailed,
  previewResourceId,
  previewResourceRevisionId,
  previewResourceName,
  preview,
  previewError,
  onClose,
  onOpenLibrary,
  onOpenWork,
  onOpenSettings,
  onLoadMoreResources,
  onRefreshResources,
  onPreviewResource,
  onClosePreview,
}: WorkbenchPanelProps) {
  const [view, setView] = useState<"start" | "files">("start");
  const [fileFilter, setFileFilter] = useState("");
  const fileHeadingRef = useRef<HTMLHeadingElement>(null);
  const previewHeadingRef = useRef<HTMLHeadingElement>(null);
  const runtimeLabel = operatorReady
    ? "Local Operator ready"
    : runtimeState
      ? `Runtime ${runtimeState.toLowerCase().replaceAll("_", " ")} · Operator unavailable`
      : "Runtime status unknown";
  const filteredResources = resources.filter(resource => resource.displayName.toLowerCase().includes(fileFilter.trim().toLowerCase()));
  const hasExactPreview = Boolean(previewResourceId && previewResourceRevisionId);

  useEffect(() => {
    const target = hasExactPreview ? previewHeadingRef.current : view === "files" ? fileHeadingRef.current : null;
    if (target) window.requestAnimationFrame(() => target.focus());
  }, [hasExactPreview, view]);

  const runWorkspaceAction = (action: () => void) => {
    action();
    if (modal) onClose();
  };
  const retryPreview = () => {
    const resource = resources.find(item => item.resourceId === previewResourceId && item.resourceRevisionId === previewResourceRevisionId);
    if (resource) onPreviewResource(resource);
  };

  return (
    <aside
      id="desktop-workbench"
      ref={panelRef}
      className="workbench-panel"
      role={modal ? "dialog" : "complementary"}
      aria-modal={modal || undefined}
      aria-labelledby="workbench-title"
      tabIndex={modal ? -1 : undefined}
    >
      <header className="workbench-panel-header">
        <div>
          <div className="eyebrow">{view === "start" ? "START" : "WORKSPACE FILES"}</div>
          <h2 id="workbench-title" ref={view === "files" ? fileHeadingRef : undefined} tabIndex={view === "files" ? -1 : undefined}>
            {view === "start" ? "Workbench" : "Files"}
          </h2>
        </div>
        {view === "files" && <button className="workbench-back-button" type="button" onClick={() => { onClosePreview(); setView("start"); }}>← Start</button>}
        {modal && <button className="workbench-close icon-button" type="button" aria-label="Close Workbench" onClick={onClose}>×</button>}
      </header>

      {view === "start" ? (
        <>
          <p className="workbench-intro">Open a desktop surface or explore this Workspace.</p>
          <section aria-label="Desktop tools">
            <div className="workbench-tool-grid">
              <button className="workbench-tool-card is-unavailable" type="button" disabled aria-describedby="workbench-browser-reason">
                <WorkbenchIcon kind="browser" />
                <strong>Browser</strong>
                <span className="workbench-tool-status">Unavailable</span>
              </button>
              <button className="workbench-tool-card is-unavailable" type="button" disabled aria-describedby="workbench-terminal-reason">
                <WorkbenchIcon kind="terminal" />
                <strong>Terminal</strong>
                <span className="workbench-tool-status">Unavailable</span>
              </button>
              <button className="workbench-tool-card" type="button" disabled={!workspaceId} aria-describedby={!workspaceId ? "workbench-files-reason" : undefined} onClick={() => { setFileFilter(""); setView("files"); }}>
                <WorkbenchIcon kind="files" />
                <strong>Files</strong>
                <span className="workbench-tool-status">{workspaceId ? "Browse Workspace" : "Workspace required"}</span>
              </button>
            </div>
            <div className="workbench-reasons">
              <p id="workbench-browser-reason">No qualified Browser provider is connected.</p>
              <p id="workbench-terminal-reason">No qualified Terminal provider is connected.</p>
              {!workspaceId && <p id="workbench-files-reason">Choose a Workspace in the sidebar before opening files.</p>}
            </div>
          </section>

          <details className="workbench-more">
            <summary>More tools</summary>
            <button className="workbench-more-tool" type="button" disabled aria-describedby="workbench-computer-reason">
              <WorkbenchIcon kind="computer" />
              <span><strong>Computer</strong><small>Unavailable</small></span>
            </button>
            <p id="workbench-computer-reason">No qualified Computer provider is connected.</p>
          </details>

          <div className="workbench-divider" />

          <section className="workbench-shortcuts" aria-label="Workspace shortcuts">
            <div className="workbench-section-label">WORKSPACE</div>
            <button className="workbench-shortcut" type="button" onClick={() => runWorkspaceAction(onOpenWork)}>
              <WorkbenchIcon kind="work" />
              <span><strong>Work</strong><small>Tasks and activity</small></span>
              <span className="workbench-chevron" aria-hidden="true">→</span>
            </button>
            <button className="workbench-shortcut" type="button" onClick={() => runWorkspaceAction(onOpenSettings)}>
              <WorkbenchIcon kind="settings" />
              <span><strong>Runtime & settings</strong><small>{workspaceName || "Workspace setup"}</small></span>
              <span className="workbench-chevron" aria-hidden="true">→</span>
            </button>
          </section>
        </>
      ) : (
        <section className="workbench-files-view" aria-label="Workspace files">
          {hasExactPreview ? (
            <div className="workbench-file-preview">
              <button className="workbench-back-button" type="button" onClick={onClosePreview}>← File list</button>
              <h3 ref={previewHeadingRef} tabIndex={-1}>{previewResourceName ?? "Text preview"}</h3>
              <p className="workbench-file-revision">Exact revision: {previewResourceRevisionId}</p>
              {previewError ? (
                <div className="workbench-files-message is-error" role="alert">
                  <p>{previewError}</p>
                  {resources.some(item => item.resourceId === previewResourceId && item.resourceRevisionId === previewResourceRevisionId)
                    ? <button type="button" onClick={retryPreview}>Retry preview</button>
                    : <button type="button" onClick={() => runWorkspaceAction(onOpenLibrary)}>Open full Library</button>}
                </div>
              ) : preview === null ? (
                <p className="workbench-files-message" role="status">Loading exact revision preview…</p>
              ) : (
                <pre className="workbench-file-preview-text">{preview}</pre>
              )}
            </div>
          ) : !workspaceId ? (
            <p className="workbench-files-message" role="status">Choose a Workspace in the sidebar before opening files.</p>
          ) : resourceListFailed && resources.length === 0 ? (
            <div className="workbench-files-message is-error" role="alert">
              <p>{operatorReady ? "Workspace files could not be loaded." : "The last Workspace file list could not be loaded. Reconnect the Local Operator to retry."}</p>
              <button type="button" onClick={() => runWorkspaceAction(operatorReady ? onRefreshResources : onOpenSettings)}>{operatorReady ? "Retry loading files" : "Open Runtime & settings"}</button>
            </div>
          ) : !operatorReady && resources.length === 0 ? (
            <div className="workbench-files-message" role="status">
              <p>Workspace files need the Local Operator.</p>
              <button type="button" onClick={() => runWorkspaceAction(onOpenSettings)}>Open Runtime & settings</button>
            </div>
          ) : resourceBusy && resources.length === 0 ? (
            <p className="workbench-files-message" role="status">Loading Workspace files…</p>
          ) : (
            <>
              <label className="workbench-files-filter">
                <span className="sr-only">Filter loaded Workspace files</span>
                <input type="search" value={fileFilter} onChange={event => setFileFilter(event.target.value)} placeholder="Filter loaded files" />
              </label>
              <p className="workbench-files-count">{resources.length} loaded {resources.length === 1 ? "file" : "files"} · Current Workspace</p>
              {!operatorReady && <p id="workbench-preview-offline-reason" className="workbench-files-message" role="status">Showing last-loaded file metadata. Preview and more results require the Local Operator.</p>}
              {resourceListFailed && resources.length > 0 && <p className="workbench-files-message is-error" role="status">The latest file refresh failed; showing the last-loaded list.</p>}
              {filteredResources.length ? (
                <ul className="workbench-file-list">
                  {filteredResources.map(resource => (
                    <li key={`${resource.resourceId}:${resource.resourceRevisionId}`}>
                      <button type="button" disabled={!operatorReady} aria-describedby={!operatorReady ? "workbench-preview-offline-reason" : undefined} onClick={() => onPreviewResource(resource)}>
                        <WorkbenchIcon kind="files" />
                        <span><strong>{resource.displayName}</strong><small>{resource.mediaType} · rev {resource.resourceRevisionId.slice(0, 8)}</small></span>
                      </button>
                    </li>
                  ))}
                </ul>
              ) : (
                <p className="workbench-files-message">{resources.length ? "No matches among the loaded files." : "No Workspace files are currently loaded."}</p>
              )}
              {resourceNextCursor && <button className="workbench-load-more" type="button" disabled={!operatorReady || resourcePageBusy} aria-describedby={!operatorReady ? "workbench-preview-offline-reason" : undefined} onClick={onLoadMoreResources}>{resourcePageBusy ? "Loading…" : "Load more files"}</button>}
              <button className="workbench-files-link" type="button" onClick={() => runWorkspaceAction(onOpenLibrary)}>Open full Library</button>
            </>
          )}
        </section>
      )}

      <footer className="workbench-runtime" role="status">
        <span className={operatorReady ? "workbench-runtime-dot is-ready" : "workbench-runtime-dot"} aria-hidden="true" />
        <span><strong>{runtimeLabel}</strong><small>{workspaceName || "No Workspace selected"}</small></span>
      </footer>
    </aside>
  );
}
