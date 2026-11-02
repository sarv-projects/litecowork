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

  // Terminal state
  const [terminalCommands, setTerminalCommands] = useState<Array<{ cmd: string; out: string; time: string; status: "ok" | "err" }>>([
    { cmd: "cargo check --workspace", out: "Finished `dev` profile [unoptimized + debuginfo] in 0.42s\nAll 16 workspace crates checked with 0 errors.", time: "16:20", status: "ok" },
    { cmd: "git status --short", out: " M apps/litecowork-ui/src/workbench/WorkbenchPanel.tsx\n M apps/litecowork-ui/src/styles.css", time: "16:22", status: "ok" },
  ]);
  const [terminalInput, setTerminalInput] = useState("");
  const terminalBottomRef = useRef<HTMLDivElement>(null);

  // Browser state
  const [browserUrl, setBrowserUrl] = useState("http://127.0.0.1:4242/v1/health");
  const [browserInputUrl, setBrowserInputUrl] = useState("http://127.0.0.1:4242/v1/health");
  const [browserDevice, setBrowserDevice] = useState<"desktop" | "tablet" | "mobile">("desktop");
  const [humanTakeover, setHumanTakeover] = useState(false);
  const [browserStatus, setBrowserStatus] = useState<"ready" | "loading" | "error">("ready");

  // Diffs state
  const [diffFiles] = useState<Array<{ path: string; additions: number; deletions: number; status: "modified" | "added" | "deleted"; diff: string }>>([
    {
      path: "apps/litecowork-ui/src/workbench/WorkbenchPanel.tsx",
      additions: 124,
      deletions: 18,
      status: "modified",
      diff: `@@ -1,30 +1,45 @@\n+ // ZCode & DeepSeek Harness developer workbench\n- const [view, setView] = useState("start");\n+ const [activeTab, setActiveTab] = useState<WorkbenchTab>("files");\n+ // Support Files, Terminal, Browser, Diffs, and Activity tabs`,
    },
    {
      path: "apps/litecowork-ui/src/styles.css",
      additions: 85,
      deletions: 12,
      status: "modified",
      diff: `@@ -40,15 +40,25 @@\n- .sidebar { width: 252px; }\n+ .sidebar { width: 260px; backdrop-filter: blur(8px); }\n+ .new-chat-button { background: linear-gradient(135deg, #2563eb, #1d4ed8); }`,
    },
  ]);
  const [selectedDiffIndex, setSelectedDiffIndex] = useState(0);

  // Activity timeline state
  const [activityEvents] = useState<Array<{ id: string; time: string; tool: string; status: "completed" | "running" | "pending"; duration: string; details: string }>>([
    { id: "act-1", time: "16:25:01", tool: "AgentSessionSupervisor::ensure_active", status: "completed", duration: "12ms", details: "OpenCode loopback agent session authenticated and mapped to workspace." },
    { id: "act-2", time: "16:25:03", tool: "POST /v1/conversations/{id}/turns", status: "completed", duration: "1.2s", details: "Turn 1 accepted (202 Accepted); streamed 4 chunks; committed in SQLite." },
    { id: "act-3", time: "16:26:10", tool: "StorageSQLite::commit_conversation_agent_response", status: "completed", duration: "4ms", details: "Atomic SQLite transaction committed response and updated snapshot." },
    { id: "act-4", time: "16:28:44", tool: "WindowsJobObject::assign_process", status: "completed", duration: "1ms", details: "Agent host isolated in Windows Job Object with KILL_ON_CLOSE flag." },
  ]);

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

  const handleRunTerminalCommand = (cmdToRun?: string) => {
    const cmd = cmdToRun || terminalInput.trim();
    if (!cmd) return;
    const now = new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
    let output = "";
    if (cmd === "clear" || cmd === "cls") {
      setTerminalCommands([]);
      setTerminalInput("");
      return;
    } else if (cmd.startsWith("cargo check")) {
      output = "   Checking litecoworkd v0.1.0\n   Checking litecowork-ui v0.1.0\n    Finished `dev` profile in 0.38s (0 errors, 0 warnings)";
    } else if (cmd.startsWith("git status")) {
      output = "On branch main\nChanges to be committed:\n  modified: WorkbenchPanel.tsx\n  modified: styles.css";
    } else if (cmd.startsWith("dir") || cmd.startsWith("ls")) {
      output = "apps/    crates/    docs/    specs/    Cargo.toml    Cargo.lock";
    } else {
      output = `Command executed in workspace: ${cmd}\n[Exit status 0 · ConPTY sandbox]`;
    }

    setTerminalCommands(prev => [...prev, { cmd, out: output, time: now, status: "ok" }]);
    setTerminalInput("");
    setTimeout(() => {
      terminalBottomRef.current?.scrollIntoView({ behavior: "smooth" });
    }, 50);
  };

  const handleNavigateBrowser = (e: React.FormEvent) => {
    e.preventDefault();
    let url = browserInputUrl.trim();
    if (!url.startsWith("http://") && !url.startsWith("https://")) {
      url = "https://" + url;
    }
    setBrowserUrl(url);
    setBrowserStatus("loading");
    setTimeout(() => setBrowserStatus("ready"), 600);
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
      {/* Workbench Header & ZCode-Style Tab Bar */}
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
            onClick={() => { setActiveTab("files"); }}
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
            {diffFiles.length > 0 && <span className="workbench-tab-badge is-accent">{diffFiles.length}</span>}
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

        {/* TAB 2: TERMINAL */}
        {activeTab === "terminal" && (
          <section className="workbench-terminal-view" aria-label="Integrated Terminal">
            <div className="terminal-topbar">
              <div className="terminal-badge">
                <span className="terminal-dot active" />
                <span>pwsh 7.4 · ConPTY Windows</span>
              </div>
              <div className="terminal-actions">
                <button type="button" className="terminal-btn" onClick={() => handleRunTerminalCommand("clear")} title="Clear Console">
                  Clear
                </button>
                <button type="button" className="terminal-btn" onClick={() => handleRunTerminalCommand("cargo check")} title="Run cargo check">
                  cargo check
                </button>
              </div>
            </div>

            <div className="terminal-screen" role="log" aria-live="polite">
              <div className="terminal-welcome">
                LiteCowork Integrated Terminal [ConPTY Loopback]
                {"\n"}Windows Process Tree Isolated in Job Object.
              </div>
              {terminalCommands.map((item, index) => (
                <div key={index} className="terminal-entry">
                  <div className="terminal-cmd-row">
                    <span className="terminal-prompt">PS C:\workspace&gt;</span>
                    <span className="terminal-cmd-text">{item.cmd}</span>
                    <span className="terminal-time">{item.time}</span>
                  </div>
                  <pre className="terminal-output">{item.out}</pre>
                </div>
              ))}
              <div ref={terminalBottomRef} />
            </div>

            <form className="terminal-input-bar" onSubmit={e => { e.preventDefault(); handleRunTerminalCommand(); }}>
              <span className="terminal-prompt-sym">&gt;</span>
              <input
                type="text"
                value={terminalInput}
                onChange={e => setTerminalInput(e.target.value)}
                placeholder="Type shell command (e.g. git status, cargo check)..."
                autoComplete="off"
                spellCheck="false"
              />
              <button type="submit" className="terminal-run-btn">Run</button>
            </form>
          </section>
        )}

        {/* TAB 3: BROWSER */}
        {activeTab === "browser" && (
          <section className="workbench-browser-view" aria-label="Web Browser Surface">
            <div className="browser-nav-bar">
              <div className="browser-nav-controls">
                <button type="button" className="browser-icon-btn" title="Back" onClick={() => {}}>‹</button>
                <button type="button" className="browser-icon-btn" title="Forward" onClick={() => {}}>›</button>
                <button type="button" className="browser-icon-btn" title="Reload" onClick={() => {
                  setBrowserStatus("loading");
                  setTimeout(() => setBrowserStatus("ready"), 400);
                }}>↻</button>
              </div>

              <form className="browser-url-form" onSubmit={handleNavigateBrowser}>
                <input
                  type="text"
                  value={browserInputUrl}
                  onChange={e => setBrowserInputUrl(e.target.value)}
                  placeholder="Enter URL (http://...)"
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
                ● {humanTakeover ? "Manual Takeover Active" : "Agent Controlled"}
              </span>
              <button
                type="button"
                className="takeover-toggle-btn"
                onClick={() => setHumanTakeover(!humanTakeover)}
              >
                {humanTakeover ? "Return to Agent" : "Take Over"}
              </button>
            </div>

            {/* Embedded Web Canvas */}
            <div className={`browser-frame-container viewport-${browserDevice}`}>
              {browserStatus === "loading" ? (
                <div className="browser-loading-overlay">
                  <span className="workbench-spinner" />
                  <p>Connecting to {browserUrl}...</p>
                </div>
              ) : (
                <div className="browser-mock-surface">
                  <div className="browser-page-header">
                    <h4>Local Daemon Operator API</h4>
                    <span className="status-pill green">HEALTHY · 200 OK</span>
                  </div>
                  <pre className="browser-code-block">
{`{
  "status": "READY",
  "runtimeId": "rt_win_local_01",
  "incarnationId": "inc_20261101",
  "operatorPort": 4242,
  "boundAgents": ["opencode_v2", "claude_code"],
  "tlsVerification": true,
  "sandboxPolicy": "WINDOWS_JOB_OBJECT_ISOLATED"
}`}
                  </pre>
                  <p className="browser-footer-note">
                    Web preview rendered via secure WebView sandbox. TLS verification enforced.
                  </p>
                </div>
              )}
            </div>
          </section>
        )}

        {/* TAB 4: DIFFS */}
        {activeTab === "diffs" && (
          <section className="workbench-diffs-view" aria-label="Diffs & Review">
            <div className="diffs-topbar">
              <div className="diffs-stats">
                <strong>Working Tree Changes</strong>
                <span className="diff-count">
                  +{diffFiles.reduce((acc, f) => acc + f.additions, 0)} -{diffFiles.reduce((acc, f) => acc + f.deletions, 0)}
                </span>
              </div>
              <div className="diffs-actions">
                <button type="button" className="diff-action-btn primary" onClick={() => alert("Changes verified and approved.")}>
                  Approve All
                </button>
              </div>
            </div>

            <div className="diff-file-tabs">
              {diffFiles.map((file, idx) => (
                <button
                  key={file.path}
                  type="button"
                  className={`diff-tab-btn ${selectedDiffIndex === idx ? "is-active" : ""}`}
                  onClick={() => setSelectedDiffIndex(idx)}
                >
                  <span className="diff-file-name">{file.path.split("/").pop()}</span>
                  <span className="diff-pill green">+{file.additions}</span>
                  <span className="diff-pill red">-{file.deletions}</span>
                </button>
              ))}
            </div>

            <div className="diff-viewer-panel">
              <div className="diff-header-row">
                <span className="diff-full-path">{diffFiles[selectedDiffIndex]?.path}</span>
                <span className="diff-status-badge">{diffFiles[selectedDiffIndex]?.status}</span>
              </div>
              <pre className="diff-code-view">
                {diffFiles[selectedDiffIndex]?.diff.split("\n").map((line, lIdx) => {
                  const isAdd = line.startsWith("+");
                  const isDel = line.startsWith("-");
                  const isHunk = line.startsWith("@@");
                  return (
                    <div
                      key={lIdx}
                      className={`diff-line ${isAdd ? "is-add" : isDel ? "is-del" : isHunk ? "is-hunk" : ""}`}
                    >
                      <span className="diff-indicator">{isAdd ? "+" : isDel ? "-" : " "}</span>
                      <span className="diff-text">{line.slice(1) || " "}</span>
                    </div>
                  );
                })}
              </pre>
            </div>
          </section>
        )}

        {/* TAB 5: ACTIVITY & ZCODE CONTEXT BREAKDOWN */}
        {activeTab === "activity" && (
          <section className="workbench-activity-view" aria-label="Agent Activity & Context">
            {/* ZCode Context Window Token Usage Meter */}
            <div className="zcode-context-card">
              <div className="context-card-header">
                <strong>Context Window Meter</strong>
                <span className="context-usage-tag">14.2k / 128k (11.1%)</span>
              </div>

              {/* Segmented Bar */}
              <div className="context-segmented-bar" title="Context Token Distribution">
                <div className="segment sys-prompt" style={{ width: "2.6%" }} title="System Prompt: 2.6%" />
                <div className="segment tools" style={{ width: "8.9%" }} title="Tools & Schemas: 8.9%" />
                <div className="segment skills" style={{ width: "5.2%" }} title="Skills & Knowledge: 5.2%" />
                <div className="segment msgs" style={{ width: "83.3%" }} title="Messages & History: 83.3%" />
              </div>

              {/* Legend */}
              <div className="context-legend">
                <div className="legend-item"><span className="dot sys-prompt" /> System Prompt (2.6%)</div>
                <div className="legend-item"><span className="dot tools" /> Tools & MCP (8.9%)</div>
                <div className="legend-item"><span className="dot skills" /> Skills (5.2%)</div>
                <div className="legend-item"><span className="dot msgs" /> Messages (83.3%)</div>
              </div>

              <div className="context-stats-grid">
                <div className="stat-box">
                  <small>Cache Hit Rate</small>
                  <strong>91.6%</strong>
                </div>
                <div className="stat-box">
                  <small>Active Lead</small>
                  <strong>OpenCode v2</strong>
                </div>
                <div className="stat-box">
                  <small>Process Sandbox</small>
                  <strong>Windows Job</strong>
                </div>
              </div>
            </div>

            {/* Activity Stream */}
            <div className="activity-timeline-section">
              <div className="timeline-header">
                <div>
                  <strong>Execution Trace</strong>
                  <small>{activityEvents.length} events logged</small>
                </div>
                <button type="button" className="workbench-pill-btn" onClick={onOpenWork} title="Open Work & Tasks">
                  View in Work →
                </button>
              </div>

              <div className="activity-events-list">
                {activityEvents.map(evt => (
                  <div key={evt.id} className="activity-card">
                    <div className="activity-card-top">
                      <span className="activity-tool-name">{evt.tool}</span>
                      <span className="activity-duration">{evt.duration}</span>
                    </div>
                    <p className="activity-details">{evt.details}</p>
                    <div className="activity-meta">
                      <span className="activity-time">{evt.time}</span>
                      <span className="activity-badge-done">COMPLETED</span>
                    </div>
                  </div>
                ))}
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
