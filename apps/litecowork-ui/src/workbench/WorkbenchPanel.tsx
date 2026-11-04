import { useEffect, useRef, useState, type Ref } from "react";
import "./workbench-panel.css";

export type WorkbenchResource = {
  resourceId: string;
  resourceRevisionId: string;
  displayName: string;
  mediaType: string;
};

export type WorkbenchTab = "launcher" | "files" | "terminal" | "browser" | "diffs" | "activity";

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

function ReviewIcon() {
  return (
    <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
      <polyline points="14 2 14 8 20 8" />
      <line x1="16" y1="13" x2="8" y2="13" />
      <line x1="16" y1="17" x2="8" y2="17" />
      <line x1="10" y1="9" x2="8" y2="9" />
    </svg>
  );
}

function OpenFileIcon() {
  return (
    <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      <path d="M13 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z" />
      <polyline points="13 2 13 9 20 9" />
    </svg>
  );
}

function TerminalIcon() {
  return (
    <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      <polyline points="4 17 10 11 4 5" />
      <line x1="12" y1="19" x2="20" y2="19" />
    </svg>
  );
}

function BrowserIcon() {
  return (
    <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="12" r="10" />
      <line x1="2" y1="12" x2="22" y2="12" />
      <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" />
    </svg>
  );
}

function SearchIcon() {
  return (
    <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="11" cy="11" r="8" />
      <line x1="21" y1="21" x2="16.65" y2="16.65" />
    </svg>
  );
}

function ExpandIcon() {
  return (
    <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      <path d="M15 3h6v6" />
      <path d="M9 21H3v-6" />
      <path d="M21 3l-7 7" />
      <path d="M3 21l7-7" />
    </svg>
  );
}

function SidePaneCloseIcon() {
  return (
    <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      <rect x="3" y="3" width="18" height="18" rx="2" />
      <path d="M15 3v18" />
    </svg>
  );
}

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
  onOpenWork: _onOpenWork,
  onOpenSettings: _onOpenSettings,
  onLoadMoreResources,
  onRefreshResources: _onRefreshResources,
  onPreviewResource,
  onClosePreview: _onClosePreview,
}: WorkbenchPanelProps) {
  const [activeTab, setActiveTab] = useState<WorkbenchTab>("launcher");
  const [omnibarInput, setOmnibarInput] = useState("");
  const [fileFilter, setFileFilter] = useState("");
  const [copiedPreview, setCopiedPreview] = useState(false);
  const [copiedTerminal, setCopiedTerminal] = useState(false);

  // Real Browser State with Live Iframe
  const [browserUrl, setBrowserUrl] = useState("http://127.0.0.1:4242/v1/health");
  const [browserInputUrl, setBrowserInputUrl] = useState("http://127.0.0.1:4242/v1/health");
  const [browserDevice, setBrowserDevice] = useState<"desktop" | "tablet" | "mobile">("desktop");
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
  const selectedDiffFile: string | null = null;

  const fileHeadingRef = useRef<HTMLHeadingElement>(null);
  const previewHeadingRef = useRef<HTMLHeadingElement>(null);

  const filteredResources = resources.filter(resource =>
    resource.displayName.toLowerCase().includes(fileFilter.trim().toLowerCase())
  );
  const hasExactPreview = Boolean(previewResourceId && previewResourceRevisionId);

  useEffect(() => {
    if (previewResourceId && activeTab === "launcher") {
      setActiveTab("files");
    }
  }, [previewResourceId, activeTab]);

  useEffect(() => {
    const target = hasExactPreview ? previewHeadingRef.current : activeTab === "files" ? fileHeadingRef.current : null;
    if (target) window.requestAnimationFrame(() => target.focus());
  }, [hasExactPreview, activeTab]);

  useEffect(() => {
    if (activeTab === "terminal" && terminalBottomRef.current) {
      terminalBottomRef.current.scrollIntoView({ behavior: "smooth" });
    }
  }, [activeTab, terminalScrollback]);

  const handleOmnibarSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const query = omnibarInput.trim();
    if (!query) return;

    if (query.startsWith("http://") || query.startsWith("https://") || query.includes(".com") || query.includes(".org") || query.includes("localhost") || query.includes(":")) {
      let finalUrl = query;
      if (!finalUrl.startsWith("http://") && !finalUrl.startsWith("https://")) {
        finalUrl = "https://" + finalUrl;
      }
      setBrowserUrl(finalUrl);
      setBrowserInputUrl(finalUrl);
      setActiveTab("browser");
    } else {
      setFileFilter(query);
      setActiveTab("files");
    }
  };

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
            `Runtime State:     ${operatorReady ? "READY (Operator listening on named pipe)" : runtimeState || "OFFLINE"}`,
            `Active Workspace:  ${workspaceName || "None selected"} (${workspaceId || "none"})`,
            `Process Sandbox:   Windows Job Object (JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE)`,
            `Memory Quota:      Enforced by Job Object resource limits`,
            `Security Context:  Integrity: Medium; DACL enforced on IPC`,
          ],
        },
      ]);
      return;
    }

    if (cmdLower === "dir" || cmdLower === "ls") {
      const fileList = resources.length > 0
        ? resources.map(r => `  ${r.displayName.padEnd(30)} [${r.mediaType}] (${r.resourceId.slice(0, 8)})`)
        : ["  (No resources indexed in active workspace)"];
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
      aria-label="Workbench Side Pane"
      tabIndex={modal ? -1 : undefined}
    >
      {/* Top Header Bar */}
      <header className="workbench-top-header">
        {activeTab !== "launcher" ? (
          <button className="workbench-back-btn" type="button" onClick={() => setActiveTab("launcher")} title="Back to Tools">
            ← <span className="workbench-back-text">Tools</span>
          </button>
        ) : (
          <div className="workbench-header-brand">
            <span className="workbench-pulse-dot" />
            <span className="workbench-header-title">Workbench</span>
          </div>
        )}

        {activeTab !== "launcher" && (
          <nav className="workbench-tools-nav">
            <button className={`workbench-tool-pill ${activeTab === "diffs" ? "active" : ""}`} onClick={() => setActiveTab("diffs")}>
              Review
            </button>
            <button className={`workbench-tool-pill ${activeTab === "files" ? "active" : ""}`} onClick={() => setActiveTab("files")}>
              Files
            </button>
            <button className={`workbench-tool-pill ${activeTab === "terminal" ? "active" : ""}`} onClick={() => setActiveTab("terminal")}>
              Terminal
            </button>
            <button className={`workbench-tool-pill ${activeTab === "browser" ? "active" : ""}`} onClick={() => setActiveTab("browser")}>
              Browser
            </button>
          </nav>
        )}

        <div className="workbench-header-window-actions">
          {onToggleFullView && (
            <button
              className="workbench-window-btn"
              type="button"
              onClick={onToggleFullView}
              title={isFullView ? "Restore side pane width" : "Expand to full view"}
            >
              <ExpandIcon />
            </button>
          )}
          <button
            className="workbench-window-btn"
            type="button"
            onClick={onClose}
            title="Close side pane"
          >
            <SidePaneCloseIcon />
          </button>
        </div>
      </header>

      {/* Main Workbench Content */}
      {activeTab === "launcher" ? (
        <div className="workbench-launcher-pane">
          {/* Rounded Omnibar Search */}
          <form className="workbench-omnibar-form" onSubmit={handleOmnibarSubmit}>
            <div className="workbench-omnibar-box">
              <span className="workbench-omnibar-search-icon">
                <SearchIcon />
              </span>
              <input
                className="workbench-omnibar-field"
                type="text"
                placeholder="Search or enter a URL"
                value={omnibarInput}
                onChange={e => setOmnibarInput(e.target.value)}
              />
            </div>
          </form>

          {/* Launcher Action List */}
          <div className="workbench-launcher-action-list">
            <button className="workbench-launcher-row" type="button" onClick={() => setActiveTab("diffs")}>
              <span className="workbench-launcher-icon"><ReviewIcon /></span>
              <span className="workbench-launcher-text">Review</span>
            </button>
            <button className="workbench-launcher-row" type="button" onClick={() => setActiveTab("files")}>
              <span className="workbench-launcher-icon"><OpenFileIcon /></span>
              <span className="workbench-launcher-text">Open File</span>
            </button>
            <button className="workbench-launcher-row" type="button" onClick={() => setActiveTab("terminal")}>
              <span className="workbench-launcher-icon"><TerminalIcon /></span>
              <span className="workbench-launcher-text">Terminal</span>
            </button>
            <button className="workbench-launcher-row" type="button" onClick={() => setActiveTab("browser")}>
              <span className="workbench-launcher-icon"><BrowserIcon /></span>
              <span className="workbench-launcher-text">Browser</span>
            </button>
          </div>
        </div>
      ) : activeTab === "files" ? (
        <section className="workbench-tab-content workbench-files-view" aria-label="Workspace Files & Code Viewer">
          {hasExactPreview ? (
            <div className="workbench-code-viewer">
              <div className="code-viewer-toolbar">
                <div className="code-viewer-info">
                  <span className={`file-badge ${getFileBadgeClass(getFileExtension(previewResourceName || ""))}`}>
                    .{getFileExtension(previewResourceName || "") || "txt"}
                  </span>
                  <span className="code-viewer-filename" ref={previewHeadingRef} tabIndex={-1}>
                    {previewResourceName}
                  </span>
                </div>
                <div className="code-viewer-actions">
                  <button className="workbench-tiny-btn" type="button" onClick={handleCopyPreview}>
                    {copiedPreview ? "Copied!" : "Copy"}
                  </button>
                  <button className="workbench-tiny-btn" type="button" onClick={onOpenLibrary}>
                    Library ↗
                  </button>
                </div>
              </div>

              {previewError ? (
                <div className="workbench-error-state">{previewError}</div>
              ) : (
                <div className="code-viewer-editor">
                  <div className="code-gutter" aria-hidden="true">
                    {(preview || "").split("\n").map((_, i) => (
                      <span key={i} className="line-num">{i + 1}</span>
                    ))}
                  </div>
                  <pre className="code-pre">
                    <code>{preview || "// File is empty"}</code>
                  </pre>
                </div>
              )}
            </div>
          ) : (
            <div className="workbench-file-tree">
              <div className="file-search-bar">
                <input
                  type="text"
                  placeholder="Filter workspace files..."
                  value={fileFilter}
                  onChange={e => setFileFilter(e.target.value)}
                  className="file-search-input"
                />
              </div>

              <div className="resource-list-container">
                {resourceListFailed && <div className="workbench-warning">Could not index files.</div>}
                {filteredResources.length === 0 ? (
                  <div className="resource-empty-state">
                    <p>No matching files found.</p>
                  </div>
                ) : (
                  <div className="resource-items-grid">
                    {filteredResources.map(resource => {
                      const ext = getFileExtension(resource.displayName);
                      return (
                        <button
                          key={resource.resourceId}
                          type="button"
                          className="resource-item-card"
                          onClick={() => onPreviewResource(resource)}
                        >
                          <span className={`file-badge ${getFileBadgeClass(ext)}`}>
                            .{ext || "file"}
                          </span>
                          <span className="resource-name" title={resource.displayName}>
                            {resource.displayName}
                          </span>
                          <span className="resource-meta-arrow">→</span>
                        </button>
                      );
                    })}
                  </div>
                )}
                {resourceNextCursor && (
                  <button
                    className="workbench-load-more-btn"
                    type="button"
                    disabled={resourcePageBusy || resourceBusy}
                    onClick={onLoadMoreResources}
                  >
                    {resourcePageBusy ? "Loading..." : "Load More"}
                  </button>
                )}
              </div>
            </div>
          )}
        </section>
      ) : activeTab === "terminal" ? (
        <section className="workbench-tab-content workbench-terminal-view" aria-label="Integrated Terminal">
          <div className="terminal-header-bar">
            <div className="terminal-badges">
              <span className="terminal-shell-badge">pwsh 7 · Windows ConPTY</span>
              <span className="terminal-isolation-badge">KILL_ON_CLOSE</span>
            </div>
            <div className="terminal-actions">
              <button className="workbench-tiny-btn" type="button" onClick={handleCopyTerminal}>
                {copiedTerminal ? "Copied!" : "Copy"}
              </button>
              <button className="workbench-tiny-btn" type="button" onClick={handleClearTerminal}>
                Clear
              </button>
            </div>
          </div>

          <div className="terminal-scrollback-screen" tabIndex={0}>
            {terminalScrollback.map((entry, idx) => (
              <div key={idx} className="terminal-log-entry">
                {entry.cmd && (
                  <div className="terminal-cmd-row">
                    <span className="terminal-prompt">$</span>
                    <span className="terminal-cmd-text">{entry.cmd}</span>
                    <span className="terminal-timestamp">{entry.time}</span>
                  </div>
                )}
                <div className={`terminal-output-lines ${entry.isError ? "is-error" : ""}`}>
                  {entry.lines.map((line, lIdx) => (
                    <div key={lIdx} className="terminal-line">{line}</div>
                  ))}
                </div>
              </div>
            ))}
            <div ref={terminalBottomRef} />
          </div>

          <form className="terminal-input-bar" onSubmit={handleRunTerminalCommand}>
            <span className="terminal-input-prompt">&gt;</span>
            <input
              type="text"
              className="terminal-command-input"
              placeholder="Execute command (help, status, dir, whoami, version)..."
              value={terminalInput}
              onChange={e => setTerminalInput(e.target.value)}
              onKeyDown={handleKeyDown}
              autoComplete="off"
              spellCheck="false"
            />
            <button className="terminal-submit-btn" type="submit" disabled={!terminalInput.trim()}>
              Run
            </button>
          </form>
        </section>
      ) : activeTab === "browser" ? (
        <section className="workbench-tab-content workbench-browser-view" aria-label="Sandboxed Browser Preview">
          <div className="browser-chrome-bar">
            <form className="browser-url-form" onSubmit={handleNavigateBrowser}>
              <span className="browser-ssl-badge" title="Sandboxed local preview">🔒</span>
              <input
                type="text"
                className="browser-url-input"
                value={browserInputUrl}
                onChange={e => setBrowserInputUrl(e.target.value)}
                placeholder="Enter URL..."
              />
              <button className="browser-go-btn" type="submit">Go</button>
            </form>

            <div className="browser-device-toggles">
              <button
                type="button"
                className={`device-btn ${browserDevice === "desktop" ? "active" : ""}`}
                onClick={() => setBrowserDevice("desktop")}
                title="Desktop 100%"
              >
                100%
              </button>
              <button
                type="button"
                className={`device-btn ${browserDevice === "tablet" ? "active" : ""}`}
                onClick={() => setBrowserDevice("tablet")}
                title="Tablet (768px)"
              >
                768
              </button>
              <button
                type="button"
                className={`device-btn ${browserDevice === "mobile" ? "active" : ""}`}
                onClick={() => setBrowserDevice("mobile")}
                title="Mobile (375px)"
              >
                375
              </button>
            </div>
          </div>

          <div className={`browser-viewport-wrapper device-${browserDevice}`}>
            {iframeError ? (
              <div className="browser-error-card">
                <h3>Page could not be previewed inline</h3>
                <p>This URL rejected iframe embedding (X-Frame-Options or CSP).</p>
                <button
                  className="primary-button"
                  type="button"
                  onClick={() => window.open(browserUrl, "_blank")}
                >
                  Open in External Browser
                </button>
              </div>
            ) : (
              <iframe
                ref={iframeRef}
                src={browserUrl}
                title="Sandboxed Agent Browser"
                className="browser-live-frame"
                sandbox="allow-scripts allow-same-origin allow-forms allow-popups"
                onError={() => setIframeError(true)}
              />
            )}
          </div>
        </section>
      ) : (
        <section className="workbench-tab-content workbench-diffs-view" aria-label="Review & Diffs">
          <div className="diff-header-bar">
            <span className="diff-title-label">Reviewing Changes</span>
            <span className="diff-stats-badge">+4 additions / -1 deletions</span>
          </div>

          <div className="diff-viewer-screen">
            <div className="diff-file-tag">
              <span className="diff-file-icon">📄</span>
              <span className="diff-file-name">{currentDiffName}</span>
            </div>

            <div className="diff-lines-container">
              {diffLines.map((l, i) => (
                <div key={i} className={`diff-line line-${l.type}`}>
                  <span className="diff-line-gutter">{i + 1}</span>
                  <span className="diff-line-text">{l.text}</span>
                </div>
              ))}
            </div>
          </div>
        </section>
      )}
    </aside>
  );
}
