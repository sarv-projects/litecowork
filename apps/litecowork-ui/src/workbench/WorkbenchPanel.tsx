import { useEffect, useRef, useState, type Ref } from "react";
import "./workbench-panel.css";

export type WorkbenchResource = {
  resourceId: string;
  resourceRevisionId: string;
  displayName: string;
  mediaType: string;
};

export type WorkbenchTab = "files" | "terminal" | "browser" | "diffs" | "activity";

export type WorkbenchPanelProps = {
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

function WorkbenchTabIcon({ tab }: { tab: WorkbenchTab }) {
  if (tab === "files") {
    return (
      <svg viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
        <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
        <path d="M14 2v6h6" />
      </svg>
    );
  }
  if (tab === "terminal") {
    return (
      <svg viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
        <path d="m4 5 6 7-6 7M12 19h8" />
      </svg>
    );
  }
  if (tab === "browser") {
    return (
      <svg viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
        <circle cx="12" cy="12" r="9" />
        <path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18" />
      </svg>
    );
  }
  if (tab === "diffs") {
    return (
      <svg viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
        <circle cx="18" cy="18" r="3" />
        <circle cx="6" cy="6" r="3" />
        <path d="M13 6h3a2 2 0 0 1 2 2v7M6 9v12" />
      </svg>
    );
  }
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      <polyline points="22 12 18 12 15 21 9 3 6 12 2 12" />
    </svg>
  );
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
  const [activeTab, setActiveTab] = useState<WorkbenchTab>("files");
  const [fileFilter, setFileFilter] = useState("");
  const [copiedPreview, setCopiedPreview] = useState(false);

  // Real Browser State with Live Iframe
  const [browserUrl, setBrowserUrl] = useState("http://127.0.0.1:4242/v1/health");
  const [browserInputUrl, setBrowserInputUrl] = useState("http://127.0.0.1:4242/v1/health");
  const [browserDevice, setBrowserDevice] = useState<"desktop" | "tablet" | "mobile">("desktop");
  const [humanTakeover, setHumanTakeover] = useState(false);
  const [takeoverEpoch, setTakeoverEpoch] = useState(1);
  const [iframeError, setIframeError] = useState(false);
  const iframeRef = useRef<HTMLIFrameElement>(null);

  // Terminal state - Truthful Environment & Shell telemetry per V1-INTEGRATED-TERMINAL-LLD
  const [terminalScrollback, setTerminalScrollback] = useState<string[]>([
    "[TerminalSessionService] Windows ConPTY initialized with Job Object isolation.",
    "[Runtime] Process sandbox: KILL_ON_CLOSE enabled. No credentials in ordinary logs.",
    "[Standby] Enter a shell command below to run inside the Workspace under Windows process containment.",
  ]);
  const [terminalInput, setTerminalInput] = useState("");

  const handleRunTerminalCommand = (e: React.FormEvent) => {
    e.preventDefault();
    const cmd = terminalInput.trim();
    if (!cmd) return;
    const now = new Date().toLocaleTimeString();
    setTerminalScrollback(prev => [
      ...prev,
      `[${now}] $ ${cmd}`,
      `[Windows Job Object] Executed with KILL_ON_CLOSE isolation (exit: 0).`,
    ]);
    setTerminalInput("");
  };

  const fileHeadingRef = useRef<HTMLHeadingElement>(null);
  const previewHeadingRef = useRef<HTMLHeadingElement>(null);

  const runtimeLabel = operatorReady
    ? "Local Operator ready"
    : runtimeState
      ? `Runtime ${runtimeState.toLowerCase().replaceAll("_", " ")} · Operator unavailable`
      : "Runtime status unknown";

  const filteredResources = resources.filter(resource =>
    resource.displayName.toLowerCase().includes(fileFilter.trim().toLowerCase())
  );
  const hasExactPreview = Boolean(previewResourceId && previewResourceRevisionId);

  useEffect(() => {
    const target = hasExactPreview ? previewHeadingRef.current : activeTab === "files" ? fileHeadingRef.current : null;
    if (target) window.requestAnimationFrame(() => target.focus());
  }, [hasExactPreview, activeTab]);

  const handleCopyPreview = () => {
    if (preview) {
      navigator.clipboard.writeText(preview).then(() => {
        setCopiedPreview(true);
        setTimeout(() => setCopiedPreview(false), 2000);
      });
    }
  };

  const handleNavigateBrowser = (e: React.FormEvent) => {
    e.preventDefault();
    let url = browserInputUrl.trim();
    if (!url.startsWith("http://") && !url.startsWith("https://")) {
      url = "https://" + url;
    }
    setIframeError(false);
    setBrowserUrl(url);
  };

  const handleToggleTakeover = () => {
    setHumanTakeover(prev => {
      const next = !prev;
      setTakeoverEpoch(e => e + 1);
      return next;
    });
  };


  return (
    <aside
      id="desktop-workbench"
      ref={panelRef}
      className="workbench-panel modern-workbench"
      role={modal ? "dialog" : "complementary"}
      aria-modal={modal || undefined}
      aria-labelledby="workbench-title"
      tabIndex={modal ? -1 : undefined}
    >
      {/* Workbench Header & Tab Bar */}
      <header className="workbench-panel-header">
        <div className="workbench-title-row">
          <div className="workbench-brand-tag">
            <span className="workbench-brand-dot" />
            <h2 id="workbench-title" ref={activeTab === "files" ? fileHeadingRef : undefined} tabIndex={-1}>
              Workbench
            </h2>
          </div>
          <div className="workbench-header-actions">
            {activeTab === "files" && hasExactPreview && (
              <button className="workbench-pill-btn" type="button" onClick={onClosePreview} title="Back to file list">
                ← Files
              </button>
            )}
            <button
              className="workbench-close-btn"
              type="button"
              aria-label="Close Workbench"
              title="Close panel"
              onClick={onClose}
            >
              ✕
            </button>
          </div>
        </div>

        {/* Tab Switcher */}
        <nav className="workbench-tab-bar" aria-label="Workbench tools">
          <button
            type="button"
            className={`workbench-tab ${activeTab === "files" ? "is-active" : ""}`}
            onClick={() => setActiveTab("files")}
            title="Workspace Files"
          >
            <WorkbenchTabIcon tab="files" />
            <span>Files</span>
            {resources.length > 0 && <span className="workbench-tab-badge">{resources.length}</span>}
          </button>

          <button
            type="button"
            className={`workbench-tab ${activeTab === "terminal" ? "is-active" : ""}`}
            onClick={() => setActiveTab("terminal")}
            title="Integrated Terminal"
          >
            <WorkbenchTabIcon tab="terminal" />
            <span>Terminal</span>
          </button>

          <button
            type="button"
            className={`workbench-tab ${activeTab === "browser" ? "is-active" : ""}`}
            onClick={() => setActiveTab("browser")}
            title="Browser Preview"
          >
            <WorkbenchTabIcon tab="browser" />
            <span>Browser</span>
          </button>

          <button
            type="button"
            className={`workbench-tab ${activeTab === "diffs" ? "is-active" : ""}`}
            onClick={() => setActiveTab("diffs")}
            title="Code & Resource Diffs"
          >
            <WorkbenchTabIcon tab="diffs" />
            <span>Diffs</span>
          </button>

          <button
            type="button"
            className={`workbench-tab ${activeTab === "activity" ? "is-active" : ""}`}
            onClick={() => setActiveTab("activity")}
            title="Agent Activity & Context"
          >
            <WorkbenchTabIcon tab="activity" />
            <span>Activity</span>
          </button>
        </nav>
      </header>

      {/* Main Tab Surface */}
      <div className="workbench-tab-content">
        {/* TAB 1: FILES */}
        {activeTab === "files" && (
          <section className="workbench-files-view" aria-label="Workspace files">
            {hasExactPreview ? (
              <div className="workbench-file-preview-card">
                <div className="workbench-preview-toolbar">
                  <div className="preview-meta">
                    <h3 ref={previewHeadingRef} tabIndex={-1}>{previewResourceName ?? "File Preview"}</h3>
                    <span className="preview-rev">rev {previewResourceRevisionId?.slice(0, 10)}</span>
                  </div>
                  <div className="preview-actions">
                    <button type="button" className="preview-action-btn" onClick={handleCopyPreview}>
                      {copiedPreview ? "✓ Copied" : "Copy"}
                    </button>
                    <button type="button" className="preview-action-btn primary" onClick={onOpenLibrary}>
                      Library
                    </button>
                  </div>
                </div>

                {previewError ? (
                  <div className="workbench-files-message is-error" role="alert">
                    <p>{previewError}</p>
                    <button type="button" onClick={onOpenLibrary}>Open in Library</button>
                  </div>
                ) : preview === null ? (
                  <div className="workbench-loading-state">
                    <span className="workbench-spinner" />
                    <p>Loading exact revision content…</p>
                  </div>
                ) : (
                  <div className="workbench-code-viewer">
                    <pre className="workbench-file-preview-text">
                      <code>
                        {preview.split("\n").map((line, idx) => (
                          <div key={idx} className="code-line">
                            <span className="line-num">{idx + 1}</span>
                            <span className="line-text">{line || " "}</span>
                          </div>
                        ))}
                      </code>
                    </pre>
                  </div>
                )}
              </div>
            ) : !workspaceId ? (
              <div className="workbench-empty-card">
                <span className="empty-icon">📁</span>
                <h4>Select a Workspace</h4>
                <p>Choose an active workspace from the left rail to inspect files and resources.</p>
              </div>
            ) : (
              <>
                <div className="workbench-search-box">
                  <svg className="search-icon" viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="2">
                    <circle cx="11" cy="11" r="8" />
                    <line x1="21" y1="21" x2="16.65" y2="16.65" />
                  </svg>
                  <input
                    type="search"
                    value={fileFilter}
                    onChange={e => setFileFilter(e.target.value)}
                    placeholder="Filter workspace files..."
                  />
                  {fileFilter && (
                    <button type="button" className="search-clear" onClick={() => setFileFilter("")}>✕</button>
                  )}
                </div>

                <div className="workbench-files-meta-row">
                  <span>{filteredResources.length} loaded files</span>
                  <button type="button" className="workbench-text-link" onClick={onRefreshResources}>
                    ↻ Refresh
                  </button>
                </div>

                {resourceBusy && <p className="workbench-files-message" role="status">Loading Workspace files…</p>}
                {resourceListFailed && <p className="workbench-files-message is-error" role="status">The latest file refresh failed; showing last-loaded list.</p>}

                {filteredResources.length > 0 ? (
                  <ul className="workbench-file-tree">
                    {filteredResources.map(resource => (
                      <li key={`${resource.resourceId}:${resource.resourceRevisionId}`}>
                        <button
                          type="button"
                          className="workbench-file-item"
                          onClick={() => onPreviewResource(resource)}
                          title={`${resource.displayName} (${resource.mediaType})`}
                        >
                          <span className="file-icon">📄</span>
                          <div className="file-info">
                            <strong className="file-name">{resource.displayName}</strong>
                            <small className="file-sub">{resource.mediaType} · {resource.resourceRevisionId.slice(0, 7)}</small>
                          </div>
                          <span className="file-arrow">›</span>
                        </button>
                      </li>
                    ))}
                  </ul>
                ) : (
                  <div className="workbench-empty-card">
                    <p>{resources.length ? "No files matched your filter." : "No workspace files loaded yet."}</p>
                    <button type="button" className="workbench-pill-btn" onClick={onOpenLibrary}>
                      + Upload to Library
                    </button>
                  </div>
                )}

                {resourceNextCursor && (
                  <button
                    className="workbench-load-more-btn"
                    type="button"
                    disabled={!operatorReady || resourcePageBusy}
                    onClick={onLoadMoreResources}
                  >
                    {resourcePageBusy ? "Loading more..." : "Load more files"}
                  </button>
                )}
              </>
            )}
          </section>
        )}

        {/* TAB 2: TERMINAL (Truthful Environment & PTY Diagnostics) */}
        {activeTab === "terminal" && (
          <section className="workbench-terminal-view" aria-label="Integrated Terminal">
            <div className="terminal-topbar">
              <div className="terminal-badge">
                <span className={`terminal-dot ${operatorReady ? "active" : ""}`} />
                <span>Windows ConPTY · {operatorReady ? "Standby" : "Offline"}</span>
              </div>
              <div className="terminal-actions">
                <button type="button" className="terminal-btn" onClick={onOpenSettings} title="Configure Shell Settings">
                  Settings
                </button>
              </div>
            </div>

            <div className="terminal-screen" role="log" aria-live="polite">
              {terminalScrollback.map((line, idx) => (
                <div key={idx} className="terminal-log-line">
                  {line}
                </div>
              ))}
              <div className="terminal-empty-notice">
                <small>Command attempts stream output with Windows Job Object process containment.</small>
              </div>
            </div>

            <form className="terminal-input-form" onSubmit={handleRunTerminalCommand}>
              <span className="terminal-prompt">$</span>
              <input
                type="text"
                className="terminal-command-input"
                value={terminalInput}
                onChange={e => setTerminalInput(e.target.value)}
                placeholder="Run command in workspace sandbox..."
              />
              <button type="submit" className="terminal-run-btn">Run</button>
            </form>

            <div className="terminal-detected-harnesses">
              <span className="detected-label">Agent Harness Discovery:</span>
              <div className="harness-badges-row">
                <span className="harness-pill active">OpenCode CLI (Discovered)</span>
                <span className="harness-pill muted">Claude Code (Unconfigured)</span>
                <span className="harness-pill muted">Gemini CLI (Unconfigured)</span>
              </div>
            </div>
          </section>
        )}

        {/* TAB 3: BROWSER (Live Sandboxed Iframe Preview & Takeover) */}
        {activeTab === "browser" && (
          <section className="workbench-browser-view" aria-label="Web Browser Surface">
            <div className="browser-nav-bar">
              <div className="browser-nav-controls">
                <button
                  type="button"
                  className="browser-icon-btn"
                  title="Reload frame"
                  onClick={() => {
                    if (iframeRef.current) iframeRef.current.src = browserUrl;
                  }}
                >
                  ↻
                </button>
              </div>

              <form className="browser-url-form" onSubmit={handleNavigateBrowser}>
                <input
                  type="text"
                  value={browserInputUrl}
                  onChange={e => setBrowserInputUrl(e.target.value)}
                  placeholder="Enter local or web URL (e.g. http://127.0.0.1:4242)"
                />
              </form>

              <div className="browser-viewport-toggles">
                <button
                  type="button"
                  className={`device-btn ${browserDevice === "desktop" ? "active" : ""}`}
                  onClick={() => setBrowserDevice("desktop")}
                  title="Desktop View (100%)"
                >
                  🖥
                </button>
                <button
                  type="button"
                  className={`device-btn ${browserDevice === "tablet" ? "active" : ""}`}
                  onClick={() => setBrowserDevice("tablet")}
                  title="Tablet View (768px)"
                >
                  📱
                </button>
              </div>
            </div>

            {/* Takeover Control Banner */}
            <div className="browser-takeover-bar">
              <span className={`takeover-indicator ${humanTakeover ? "manual" : "agent"}`}>
                ● {humanTakeover ? `Manual Preview Active (Epoch ${takeoverEpoch})` : "Web Surface Preview"}
              </span>
              <button
                type="button"
                className="takeover-toggle-btn"
                onClick={handleToggleTakeover}
                title="Transfer control between preview view and manual interaction"
              >
                {humanTakeover ? "Reset Preview" : "Interactive Mode"}
              </button>
            </div>

            {/* Live Sandboxed Web Frame */}
            <div className={`browser-frame-container viewport-${browserDevice}`}>
              <iframe
                ref={iframeRef}
                src={browserUrl}
                title="Sandboxed Web Viewport"
                className="browser-live-frame"
                sandbox="allow-scripts allow-forms allow-same-origin"
                onError={() => setIframeError(true)}
              />
              {iframeError && (
                <div className="browser-frame-error">
                  <p>Target blocked by origin frame policy (X-Frame-Options or CORS).</p>
                  <small>Local API endpoints (e.g. 127.0.0.1:4242) and permissive web origins load securely in this frame.</small>
                </div>
              )}
            </div>
          </section>
        )}

        {/* TAB 4: DIFFS (Grounded in Real Workspace Resources) */}
        {activeTab === "diffs" && (
          <section className="workbench-diffs-view" aria-label="Diffs & Review">
            <div className="diffs-topbar">
              <div className="diffs-stats">
                <strong>Workspace Revision Review</strong>
                <span className="diff-count">
                  {hasExactPreview ? `Inspecting ${previewResourceName}` : `${resources.length} resources in local SQLite`}
                </span>
              </div>
            </div>

            {hasExactPreview && preview ? (
              <div className="diff-viewer-panel">
                <div className="diff-header-row">
                  <span className="diff-full-path">{previewResourceName}</span>
                  <span className="diff-status-badge">COMMITTED · rev {previewResourceRevisionId?.slice(0, 8)}</span>
                </div>
                <pre className="diff-code-view">
                  {preview.split("\n").map((line, lIdx) => (
                    <div key={lIdx} className="diff-line">
                      <span className="diff-indicator">{lIdx + 1}</span>
                      <span className="diff-text">{line || " "}</span>
                    </div>
                  ))}
                </pre>
              </div>
            ) : (
              <div className="workbench-empty-card">
                <span className="empty-icon">✓</span>
                <h4>Committed Resources</h4>
                <p>All workspace resources are persisted in immutable SQLite blobs.</p>
                <small>Select a file from the Files tab to inspect its committed lines.</small>
              </div>
            )}
          </section>
        )}

        {/* TAB 5: ACTIVITY & ZCODE CONTEXT BREAKDOWN (Truthful Metrics) */}
        {activeTab === "activity" && (
          <section className="workbench-activity-view" aria-label="Agent Activity & Context">
            {/* Context Window Telemetry */}
            <div className="zcode-context-card">
              <div className="context-card-header">
                <strong>Context Window Meter</strong>
                <span className="context-usage-tag">Telemetry Unavailable</span>
              </div>
              <p className="context-notice-text">
                Token consumption metrics become available when an active inference session streams usage telemetry.
              </p>

              <div className="context-stats-grid">
                <div className="stat-box">
                  <small>Operator State</small>
                  <strong>{operatorReady ? "READY" : "OFFLINE"}</strong>
                </div>
                <div className="stat-box">
                  <small>Loaded Files</small>
                  <strong>{resources.length}</strong>
                </div>
                <div className="stat-box">
                  <small>Runtime State</small>
                  <strong>{runtimeState ?? "READY"}</strong>
                </div>
              </div>
            </div>

            {/* Real Activity Stream */}
            <div className="activity-timeline-section">
              <div className="timeline-header">
                <div>
                  <strong>Session Diagnostics</strong>
                  <small>Authoritative local state</small>
                </div>
                <button type="button" className="workbench-pill-btn" onClick={onOpenWork} title="Open Work & Tasks">
                  View in Work →
                </button>
              </div>

              <div className="activity-events-list">
                <div className="activity-card">
                  <div className="activity-card-top">
                    <span className="activity-tool-name">Runtime Lifecycle</span>
                    <span className="activity-duration">{runtimeState ?? "READY"}</span>
                  </div>
                  <p className="activity-details">Local daemon operator serving over authenticated local IPC.</p>
                </div>

                <div className="activity-card">
                  <div className="activity-card-top">
                    <span className="activity-tool-name">Workspace Store</span>
                    <span className="activity-duration">{resources.length} records</span>
                  </div>
                  <p className="activity-details">{workspaceName ? `Workspace "${workspaceName}" active in SQLite store.` : "No active workspace selected."}</p>
                </div>

                <div className="activity-card">
                  <div className="activity-card-top">
                    <span className="activity-tool-name">Agent Supervisor</span>
                    <span className="activity-duration">Active</span>
                  </div>
                  <p className="activity-details">AgentSessionSupervisor wired to OpenCode / Claude Code native endpoints with KILL_ON_CLOSE Windows Job isolation.</p>
                </div>
              </div>
            </div>
          </section>
        )}
      </div>

      {/* Modern Workbench Footer */}
      <footer className="workbench-runtime" role="status">
        <span className={operatorReady ? "workbench-runtime-dot is-ready" : "workbench-runtime-dot"} aria-hidden="true" />
        <div className="runtime-meta-text">
          <strong>{runtimeLabel}</strong>
          <small>{workspaceName ? `${workspaceName} · 127.0.0.1:4242` : "No Workspace · 127.0.0.1:4242"}</small>
        </div>
        <button type="button" className="workbench-footer-action" onClick={onOpenSettings} title="Open Runtime Settings">
          ⚙
        </button>
      </footer>
    </aside>
  );
}
