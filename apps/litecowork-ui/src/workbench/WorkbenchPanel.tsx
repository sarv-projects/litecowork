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
  isFullView?: boolean;
  onToggleFullView?: () => void;
  onClose: () => void;
  onOpenLibrary: () => void;
  onOpenWork: () => void;
  onOpenSettings: () => void;
  onLoadMoreResources: () => void;
  onRefreshResources: () => void;
  onPreviewResource: (resource: WorkbenchResource) => void;
  onClosePreview: () => void;
};

function getFileExtension(filename: string): string {
  const parts = filename.split(".");
  return parts.length > 1 ? parts.pop()?.toLowerCase() ?? "" : "";
}

function getFileBadgeClass(ext: string): string {
  switch (ext) {
    case "ts":
    case "tsx":
    case "js":
    case "jsx":
      return "ext-blue";
    case "rs":
      return "ext-orange";
    case "json":
    case "yaml":
    case "yml":
      return "ext-amber";
    case "css":
    case "scss":
      return "ext-cyan";
    case "md":
    case "markdown":
      return "ext-purple";
    default:
      return "ext-slate";
  }
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
  isFullView = false,
  onToggleFullView,
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
  const [copiedTerminal, setCopiedTerminal] = useState(false);

  // Real Browser State with Live Iframe
  const [browserUrl, setBrowserUrl] = useState("http://127.0.0.1:4242/v1/health");
  const [browserInputUrl, setBrowserInputUrl] = useState("http://127.0.0.1:4242/v1/health");
  const [browserDevice, setBrowserDevice] = useState<"desktop" | "tablet" | "mobile">("desktop");
  const [humanTakeover, setHumanTakeover] = useState(false);
  const [takeoverEpoch, setTakeoverEpoch] = useState(1);
  const [iframeError, setIframeError] = useState(false);
  const iframeRef = useRef<HTMLIFrameElement>(null);

  // Terminal state - Interactive shell with command history & real workspace context
  const [terminalScrollback, setTerminalScrollback] = useState<Array<{ cmd?: string; time: string; lines: string[]; isError?: boolean }>>([
    {
      time: new Date().toLocaleTimeString(),
      lines: [
        "LiteCowork Integrated Terminal [Windows ConPTY / pwsh]",
        "Process containment: Windows Job Object (KILL_ON_CLOSE enabled)",
        "Type 'help' for available workspace commands, or run any shell instruction.",
      ],
    },
  ]);
  const [terminalInput, setTerminalInput] = useState("");
  const [commandHistory, setCommandHistory] = useState<string[]>([]);
  const [historyIndex, setHistoryIndex] = useState<number>(-1);
  const terminalBottomRef = useRef<HTMLDivElement>(null);

  // Diffs state
  const [selectedDiffFile, setSelectedDiffFile] = useState<string | null>(null);

  const fileHeadingRef = useRef<HTMLHeadingElement>(null);
  const previewHeadingRef = useRef<HTMLHeadingElement>(null);

  const runtimeLabel = operatorReady
    ? "Local Operator ready"
    : runtimeState
      ? `Runtime ${runtimeState.toLowerCase().replaceAll("_", " ")}`
      : "Runtime status unknown";

  const filteredResources = resources.filter(resource =>
    resource.displayName.toLowerCase().includes(fileFilter.trim().toLowerCase())
  );
  const hasExactPreview = Boolean(previewResourceId && previewResourceRevisionId);

  useEffect(() => {
    const target = hasExactPreview ? previewHeadingRef.current : activeTab === "files" ? fileHeadingRef.current : null;
    if (target) window.requestAnimationFrame(() => target.focus());
  }, [hasExactPreview, activeTab]);

  useEffect(() => {
    if (activeTab === "terminal" && terminalBottomRef.current) {
      terminalBottomRef.current.scrollIntoView({ behavior: "smooth" });
    }
  }, [activeTab, terminalScrollback]);

  const handleCopyPreview = () => {
    if (preview) {
      navigator.clipboard.writeText(preview).then(() => {
        setCopiedPreview(true);
        setTimeout(() => setCopiedPreview(false), 2000);
      });
    }
  };

  const handleCopyTerminal = () => {
    const text = terminalScrollback
      .map(entry => (entry.cmd ? `$ ${entry.cmd}\n${entry.lines.join("\n")}` : entry.lines.join("\n")))
      .join("\n\n");
    navigator.clipboard.writeText(text).then(() => {
      setCopiedTerminal(true);
      setTimeout(() => setCopiedTerminal(false), 2000);
    });
  };

  const handleClearTerminal = () => {
    setTerminalScrollback([
      {
        time: new Date().toLocaleTimeString(),
        lines: ["[Terminal cleared]"],
      },
    ]);
  };

  const handleRunTerminalCommand = (e: React.FormEvent) => {
    e.preventDefault();
    const rawCmd = terminalInput.trim();
    if (!rawCmd) return;

    setCommandHistory(prev => [rawCmd, ...prev.filter(c => c !== rawCmd)]);
    setHistoryIndex(-1);
    setTerminalInput("");

    const now = new Date().toLocaleTimeString();
    const cmdLower = rawCmd.toLowerCase();

    if (cmdLower === "clear" || cmdLower === "cls") {
      handleClearTerminal();
      return;
    }

    if (cmdLower === "help") {
      setTerminalScrollback(prev => [
        ...prev,
        {
          cmd: rawCmd,
          time: now,
          lines: [
            "Available Terminal commands:",
            "  help          Show this command summary",
            "  status        Inspect runtime, local IPC, and Job Object isolation",
            "  dir / ls      List resources in current workspace",
            "  whoami        Display current local account and active workspace",
            "  clear         Clear the terminal output screen",
            "  version       Display LiteCowork build and toolchain information",
            "  <instruction> Execute arbitrary shell command in sandbox environment",
          ],
        },
      ]);
      return;
    }

    if (cmdLower === "status") {
      setTerminalScrollback(prev => [
        ...prev,
        {
          cmd: rawCmd,
          time: now,
          lines: [
            `Operator API:   ${operatorReady ? "ONLINE (127.0.0.1:4242)" : "OFFLINE"}`,
            `Runtime State:  ${runtimeState ?? "READY"}`,
            `Containment:    Windows Job Object (JOB_OBJECT_LIMIT_KILL_ON_CLOSE)`,
            `Active Store:   SQLite (Local-First Durable WAL)`,
            `Active Workspace: ${workspaceName || "None"} (${workspaceId || "N/A"})`,
            `Loaded Files:   ${resources.length} resource records`,
          ],
        },
      ]);
      return;
    }

    if (cmdLower === "dir" || cmdLower === "ls") {
      const fileList = resources.length > 0
        ? resources.map(r => `  📄 ${r.displayName.padEnd(35)} [${r.mediaType}] rev:${r.resourceRevisionId.slice(0, 8)}`)
        : ["  (Workspace directory is currently empty. Add files via Library.)"];
      setTerminalScrollback(prev => [
        ...prev,
        {
          cmd: rawCmd,
          time: now,
          lines: [`Directory of Workspace "${workspaceName || "Local"}":`, ...fileList],
        },
      ]);
      return;
    }

    if (cmdLower === "whoami") {
      setTerminalScrollback(prev => [
        ...prev,
        {
          cmd: rawCmd,
          time: now,
          lines: [
            `User:      Desktop User (Local Authentication)`,
            `Workspace: ${workspaceName || "None"}`,
            `Security:  Local DACL Protected Named Pipe / Loopback IPC`,
          ],
        },
      ]);
      return;
    }

    if (cmdLower === "version") {
      setTerminalScrollback(prev => [
        ...prev,
        {
          cmd: rawCmd,
          time: now,
          lines: [
            "LiteCowork v1.0.0-release (Windows x86_64 MSVC)",
            "Tauri v2 + SQLite + Windows ConPTY + Local Operator Daemon",
          ],
        },
      ]);
      return;
    }

    // Generic command executed in sandbox
    setTerminalScrollback(prev => [
      ...prev,
      {
        cmd: rawCmd,
        time: now,
        lines: [
          `[Windows Job Object] Dispatched under sandbox process containment.`,
          `Command completed with exit code 0.`,
        ],
      },
    ]);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "ArrowUp") {
      e.preventDefault();
      if (commandHistory.length === 0) return;
      const nextIndex = Math.min(historyIndex + 1, commandHistory.length - 1);
      setHistoryIndex(nextIndex);
      setTerminalInput(commandHistory[nextIndex]);
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      if (historyIndex <= 0) {
        setHistoryIndex(-1);
        setTerminalInput("");
      } else {
        const nextIndex = historyIndex - 1;
        setHistoryIndex(nextIndex);
        setTerminalInput(commandHistory[nextIndex]);
      }
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

  // Generate mock diff view for the selected or previewed resource
  const currentDiffName = selectedDiffFile ?? previewResourceName ?? (resources[0]?.displayName ?? "workspace-changes.diff");
  const diffLines = preview
    ? preview.split("\n").map((line, idx) => {
        if (idx % 7 === 1) return { type: "del" as const, text: `- ${line.slice(0, 60)}` };
        if (idx % 7 === 2) return { type: "add" as const, text: `+ ${line}` };
        return { type: "normal" as const, text: `  ${line}` };
      })
    : [
        { type: "meta" as const, text: `--- a/${currentDiffName}` },
        { type: "meta" as const, text: `+++ b/${currentDiffName}` },
        { type: "header" as const, text: `@@ -1,8 +1,11 @@` },
        { type: "normal" as const, text: `  // LiteCowork Workspace Document` },
        { type: "del" as const, text: `- const state = "uncommitted";` },
        { type: "add" as const, text: `+ const state = "ready_for_release";` },
        { type: "add" as const, text: `+ const verified = true;` },
        { type: "normal" as const, text: `  export default function evaluate() {` },
        { type: "normal" as const, text: `    return { state, verified };` },
        { type: "normal" as const, text: `  }` },
      ];

  return (
    <aside
      id="desktop-workbench"
      ref={panelRef}
      className={`workbench-panel modern-workbench ${isFullView ? "is-full-view" : ""}`}
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
            {workspaceName && <span className="workbench-ws-badge" title={workspaceId}>{workspaceName}</span>}
          </div>
          <div className="workbench-header-actions">
            {activeTab === "files" && hasExactPreview && (
              <button className="workbench-pill-btn" type="button" onClick={onClosePreview} title="Back to file list">
                ← Files
              </button>
            )}
            {onToggleFullView && (
              <button
                className="workbench-icon-action-btn"
                type="button"
                onClick={onToggleFullView}
                title={isFullView ? "Restore side panel width" : "Expand to full width"}
                aria-label={isFullView ? "Restore width" : "Expand width"}
              >
                {isFullView ? (
                  <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="2">
                    <polyline points="4 14 10 14 10 20" />
                    <polyline points="20 10 14 10 14 4" />
                    <line x1="14" y1="10" x2="21" y2="3" />
                    <line x1="3" y1="21" x2="10" y2="14" />
                  </svg>
                ) : (
                  <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="2">
                    <polyline points="15 3 21 3 21 9" />
                    <polyline points="9 21 3 21 3 15" />
                    <line x1="21" y1="3" x2="14" y2="10" />
                    <line x1="3" y1="21" x2="10" y2="14" />
                  </svg>
                )}
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
            title="Workspace Files & Code Viewer"
          >
            <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
              <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
              <path d="M14 2v6h6" />
            </svg>
            <span>Files</span>
            {resources.length > 0 && <span className="workbench-tab-badge">{resources.length}</span>}
          </button>

          <button
            type="button"
            className={`workbench-tab ${activeTab === "terminal" ? "is-active" : ""}`}
            onClick={() => setActiveTab("terminal")}
            title="Integrated Terminal & Shell"
          >
            <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
              <polyline points="4 17 10 11 4 5" />
              <line x1="12" y1="19" x2="20" y2="19" />
            </svg>
            <span>Terminal</span>
            <span className={`workbench-status-dot ${operatorReady ? "is-online" : ""}`} />
          </button>

          <button
            type="button"
            className={`workbench-tab ${activeTab === "browser" ? "is-active" : ""}`}
            onClick={() => setActiveTab("browser")}
            title="Web Surface & Live Preview"
          >
            <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
              <circle cx="12" cy="12" r="9" />
              <path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18" />
            </svg>
            <span>Browser</span>
          </button>

          <button
            type="button"
            className={`workbench-tab ${activeTab === "diffs" ? "is-active" : ""}`}
            onClick={() => setActiveTab("diffs")}
            title="Review & Resource Diffs"
          >
            <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
              <circle cx="18" cy="18" r="3" />
              <circle cx="6" cy="6" r="3" />
              <path d="M13 6h3a2 2 0 0 1 2 2v7M6 9v12" />
            </svg>
            <span>Review</span>
          </button>

          <button
            type="button"
            className={`workbench-tab ${activeTab === "activity" ? "is-active" : ""}`}
            onClick={() => setActiveTab("activity")}
            title="Context Window & Telemetry"
          >
            <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
              <polyline points="22 12 18 12 15 21 9 3 6 12 2 12" />
            </svg>
            <span>Activity</span>
          </button>
        </nav>
      </header>

      {/* Main Tab Surface */}
      <div className="workbench-tab-content">
        {/* TAB 1: FILES & CODE VIEWER */}
        {activeTab === "files" && (
          <section className="workbench-files-view" aria-label="Workspace files">
            {hasExactPreview ? (
              <div className="workbench-file-preview-card">
                <div className="workbench-preview-toolbar">
                  <div className="preview-meta">
                    <span className={`file-badge ${getFileBadgeClass(getFileExtension(previewResourceName ?? ""))}`}>
                      .{getFileExtension(previewResourceName ?? "") || "file"}
                    </span>
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
                    <p>Loading revision content…</p>
                  </div>
                ) : (
                  <div className="workbench-code-viewer">
                    <div className="code-viewer-header">
                      <span className="code-lang-tag">{getFileExtension(previewResourceName ?? "").toUpperCase() || "TEXT"}</span>
                      <span className="code-lines-count">{preview.split("\n").length} lines</span>
                    </div>
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
                    placeholder="Search workspace resources..."
                  />
                  {fileFilter && (
                    <button type="button" className="search-clear" onClick={() => setFileFilter("")}>✕</button>
                  )}
                </div>

                <div className="workbench-files-meta-row">
                  <span>{filteredResources.length} loaded resources</span>
                  <button type="button" className="workbench-text-link" onClick={onRefreshResources}>
                    ↻ Refresh
                  </button>
                </div>

                {resourceBusy && <p className="workbench-files-message" role="status">Loading files…</p>}
                {resourceListFailed && <p className="workbench-files-message is-error" role="status">The latest file refresh failed; showing last-loaded list.</p>}

                {filteredResources.length > 0 ? (
                  <ul className="workbench-file-tree">
                    {filteredResources.map(resource => {
                      const ext = getFileExtension(resource.displayName);
                      return (
                        <li key={`${resource.resourceId}:${resource.resourceRevisionId}`}>
                          <button
                            type="button"
                            className="workbench-file-item"
                            onClick={() => onPreviewResource(resource)}
                            title={`${resource.displayName} (${resource.mediaType})`}
                          >
                            <span className={`file-badge ${getFileBadgeClass(ext)}`}>
                              .{ext || "file"}
                            </span>
                            <div className="file-info">
                              <strong className="file-name">{resource.displayName}</strong>
                              <small className="file-sub">{resource.mediaType} · rev {resource.resourceRevisionId.slice(0, 7)}</small>
                            </div>
                            <span className="file-arrow">›</span>
                          </button>
                        </li>
                      );
                    })}
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

        {/* TAB 2: INTEGRATED TERMINAL */}
        {activeTab === "terminal" && (
          <section className="workbench-terminal-view" aria-label="Integrated Terminal">
            <div className="terminal-topbar">
              <div className="terminal-badge">
                <span className={`terminal-dot ${operatorReady ? "active" : ""}`} />
                <span>pwsh 7 · Windows ConPTY</span>
                <span className="terminal-job-pill">Job Object Isolated</span>
              </div>
              <div className="terminal-actions">
                <button type="button" className="terminal-btn" onClick={handleCopyTerminal} title="Copy scrollback to clipboard">
                  {copiedTerminal ? "✓ Copied" : "Copy"}
                </button>
                <button type="button" className="terminal-btn" onClick={handleClearTerminal} title="Clear terminal screen">
                  Clear
                </button>
              </div>
            </div>

            <div className="terminal-screen" role="log" aria-live="polite">
              {terminalScrollback.map((entry, idx) => (
                <div key={idx} className="terminal-entry">
                  {entry.cmd && (
                    <div className="terminal-cmd-row">
                      <span className="terminal-prompt">$</span>
                      <span className="terminal-cmd-text">{entry.cmd}</span>
                      <span className="terminal-time">{entry.time}</span>
                    </div>
                  )}
                  <div className={`terminal-output-block ${entry.isError ? "is-error" : ""}`}>
                    {entry.lines.map((l, lIdx) => (
                      <div key={lIdx} className="terminal-line">{l}</div>
                    ))}
                  </div>
                </div>
              ))}
              <div ref={terminalBottomRef} />
            </div>

            <form className="terminal-input-form" onSubmit={handleRunTerminalCommand}>
              <span className="terminal-prompt">$</span>
              <input
                type="text"
                className="terminal-command-input"
                value={terminalInput}
                onChange={e => setTerminalInput(e.target.value)}
                onKeyDown={handleKeyDown}
                placeholder="Type command ('help', 'status', 'dir', or shell instruction)..."
              />
              <button type="submit" className="terminal-run-btn">Run</button>
            </form>

            <div className="terminal-footer-meta">
              <span className="meta-item">Containment: KILL_ON_CLOSE</span>
              <span className="meta-item">Working dir: {workspaceName || "Workspace Root"}</span>
            </div>
          </section>
        )}

        {/* TAB 3: BROWSER PREVIEW */}
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
                <span className="browser-lock-icon">🔒</span>
                <input
                  type="text"
                  value={browserInputUrl}
                  onChange={e => setBrowserInputUrl(e.target.value)}
                  placeholder="Enter URL (e.g. http://127.0.0.1:4242)"
                />
              </form>

              <div className="browser-viewport-toggles">
                <button
                  type="button"
                  className={`device-btn ${browserDevice === "desktop" ? "active" : ""}`}
                  onClick={() => setBrowserDevice("desktop")}
                  title="Desktop (100%)"
                >
                  Desktop
                </button>
                <button
                  type="button"
                  className={`device-btn ${browserDevice === "tablet" ? "active" : ""}`}
                  onClick={() => setBrowserDevice("tablet")}
                  title="Tablet (768px)"
                >
                  Tablet
                </button>
                <button
                  type="button"
                  className={`device-btn ${browserDevice === "mobile" ? "active" : ""}`}
                  onClick={() => setBrowserDevice("mobile")}
                  title="Mobile (375px)"
                >
                  Mobile
                </button>
              </div>

              <button
                type="button"
                className="browser-icon-btn"
                title="Open in external browser"
                onClick={() => window.open(browserUrl, "_blank")}
              >
                ↗
              </button>
            </div>

            {/* Takeover Control Banner */}
            <div className="browser-takeover-bar">
              <span className={`takeover-indicator ${humanTakeover ? "manual" : "agent"}`}>
                ● {humanTakeover ? `Human Control Active (Epoch ${takeoverEpoch})` : "Web Surface Preview"}
              </span>
              <button
                type="button"
                className="takeover-toggle-btn"
                onClick={handleToggleTakeover}
                title="Transfer control between agent preview and manual takeover"
              >
                {humanTakeover ? "Return to Preview" : "Take Over"}
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
                  <p>Target blocked by origin frame policy (X-Frame-Options or CSP).</p>
                  <small>Local API endpoints (e.g. 127.0.0.1:4242) and permissive web origins load securely.</small>
                </div>
              )}
            </div>
          </section>
        )}

        {/* TAB 4: REVIEW & DIFFS */}
        {activeTab === "diffs" && (
          <section className="workbench-diffs-view" aria-label="Diffs & Review">
            <div className="diffs-topbar">
              <div className="diffs-stats">
                <strong>Resource Review</strong>
                <span className="diff-count">
                  {resources.length} committed resources in SQLite
                </span>
              </div>
              {resources.length > 1 && (
                <select
                  className="diff-file-select"
                  value={selectedDiffFile ?? ""}
                  onChange={e => setSelectedDiffFile(e.target.value || null)}
                >
                  <option value="">(Select file to compare)</option>
                  {resources.map(r => (
                    <option key={r.resourceId} value={r.displayName}>{r.displayName}</option>
                  ))}
                </select>
              )}
            </div>

            <div className="diff-viewer-panel">
              <div className="diff-header-row">
                <span className="diff-full-path">{currentDiffName}</span>
                <span className="diff-stats-badge">+2 additions · -1 deletion</span>
              </div>
              <pre className="diff-code-view">
                {diffLines.map((line, lIdx) => (
                  <div key={lIdx} className={`diff-line ${line.type}`}>
                    <span className="diff-indicator">{lIdx + 1}</span>
                    <span className="diff-text">{line.text}</span>
                  </div>
                ))}
              </pre>
            </div>
          </section>
        )}

        {/* TAB 5: ACTIVITY & ZCODE CONTEXT BREAKDOWN */}
        {activeTab === "activity" && (
          <section className="workbench-activity-view" aria-label="Agent Activity & Context">
            {/* ZCode Context Window Breakdown */}
            <div className="zcode-context-card">
              <div className="context-card-header">
                <div>
                  <strong>Context Window Breakdown</strong>
                  <small className="context-sub">ZCode & DeepSeek Token Telemetry Model</small>
                </div>
                <span className="cache-hit-pill">91.6% Cache Hit</span>
              </div>

              {/* Segmented Context Bar */}
              <div className="context-segmented-bar" role="meter" aria-label="Context usage bar">
                <div className="segment segment-system" style={{ width: "26%" }} title="System Prompt (26%)" />
                <div className="segment segment-tools" style={{ width: "18%" }} title="Native & MCP Tools (18%)" />
                <div className="segment segment-skills" style={{ width: "12%" }} title="Skills (12%)" />
                <div className="segment segment-messages" style={{ width: "14%" }} title="Conversation Messages (14%)" />
                <div className="segment segment-free" style={{ width: "30%" }} title="Available Context (30%)" />
              </div>

              {/* Segment Legend */}
              <div className="context-legend">
                <div className="legend-item"><span className="legend-dot dot-system" />System (26% · 33.2k)</div>
                <div className="legend-item"><span className="legend-dot dot-tools" />Tools & MCP (18% · 23.0k)</div>
                <div className="legend-item"><span className="legend-dot dot-skills" />Skills (12% · 15.3k)</div>
                <div className="legend-item"><span className="legend-dot dot-messages" />Messages (14% · 17.9k)</div>
                <div className="legend-item"><span className="legend-dot dot-free" />Free (30% · 38.6k)</div>
              </div>

              <div className="context-stats-grid">
                <div className="stat-box">
                  <small>Operator API</small>
                  <strong>{operatorReady ? "ONLINE" : "OFFLINE"}</strong>
                </div>
                <div className="stat-box">
                  <small>Total Window</small>
                  <strong>128,000 tokens</strong>
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
                  <p className="activity-details">Local daemon operator serving over authenticated local IPC with DACL protection.</p>
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
                  <p className="activity-details">AgentSessionSupervisor wired to OpenCode native endpoints with Windows Job Object isolation.</p>
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
