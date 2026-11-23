import { useEffect, useId, useRef, useState, type Ref } from "react";
import "./workbench-panel.css";

export type WorkbenchResource = {
  resourceId: string;
  resourceRevisionId: string;
  displayName: string;
  mediaType: string;
};

export type WorkbenchTab = "launcher" | "files" | "terminal" | "browser" | "diffs" | "activity";

export type WorkbenchPanelProps = {
  initialTab?: WorkbenchTab;
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

export function UnavailableNotice({
  title,
  detail,
  blockers,
  availableNow,
}: {
  title: string;
  detail: string;
  blockers: string[];
  availableNow?: string;
}) {
  const titleId = useId();
  return (
    <div className="workbench-unavailable" role="status" aria-labelledby={titleId} tabIndex={0}>
      <h3 id={titleId} className="workbench-unavailable-title">{title}</h3>
      <p className="workbench-unavailable-detail">{detail}</p>
      <p className="workbench-unavailable-label">Not available in this build:</p>
      <ul className="workbench-unavailable-list">
        {blockers.map(blocker => (
          <li key={blocker}>{blocker}</li>
        ))}
      </ul>
      {availableNow && <p className="workbench-unavailable-available">{availableNow}</p>}
    </div>
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

export function TerminalPane({ platform }: { platform: string }) {
  const windows = /win/i.test(platform);
  return (
    <section className="workbench-tab-content workbench-terminal-view" aria-label="Terminal">
      <div className="terminal-header-bar">
        <div className="terminal-badges">
          <span className="terminal-shell-badge">No terminal runtime served</span>
        </div>
      </div>

      <UnavailableNotice
        title="Terminal execution is unavailable"
        detail="Nothing is executed from this pane and no process output is shown, because no shell runtime is served by this build."
        blockers={[
          "a daemon-hosted terminal session service",
          windows ? "a qualified Windows Environment provider" : "an admitted Environment for this session",
          "a live activity record that reports real progress and exit status",
        ]}
        availableNow="Workbench → Files lists the resources this Workspace has actually indexed."
      />
    </section>
  );
}

export function ActivityPane() {
  return (
    <section className="workbench-tab-content workbench-activity-view" aria-label="Activity">
      <UnavailableNotice
        title="No recorded activity source is served by this build"
        detail="This build does not serve an Activity record source, so no live or historical process activity is shown."
        blockers={["a durable Activity read backed by committed records"]}
      />
    </section>
  );
}

export function ComputerStatusNotice() {
  return (
    <p className="workbench-computer-status" role="status" aria-label="Computer use availability">
      Computer use is unavailable in this build.
    </p>
  );
}

export function ReviewPane() {
  return (
    <section className="workbench-tab-content workbench-diffs-view" aria-label="Review">
      <div className="diff-header-bar">
        <span className="diff-title-label">Review</span>
      </div>

      <UnavailableNotice
        title="Nothing to review yet"
        detail="No change set is served by this build, so this pane shows no diff. Changes are never inferred from document text."
        blockers={[
          "a durable Task or Attempt whose outputs include changed files",
          "a diff producer that reports real added and removed lines",
          "independent verification of the change before it is presented",
        ]}
        availableNow="Workbench → Files can show the exact revision you open."
      />
    </section>
  );
}

export function WorkbenchPanel({
  initialTab = "launcher",
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
  const [activeTab, setActiveTab] = useState<WorkbenchTab>(initialTab);
  const [omnibarInput, setOmnibarInput] = useState("");
  const [fileFilter, setFileFilter] = useState("");
  const [copiedPreview, setCopiedPreview] = useState(false);

  // Embedded page preview. This pane is the application renderer, not an isolated
  // browser runtime: it starts empty and claims no containment, because this build
  // serves no browser-control surface to back such a claim.
  const [browserUrl, setBrowserUrl] = useState("");
  const [browserInputUrl, setBrowserInputUrl] = useState("");
  const [browserDevice, setBrowserDevice] = useState<"desktop" | "tablet" | "mobile">("desktop");
  const [iframeError, setIframeError] = useState(false);
  const iframeRef = useRef<HTMLIFrameElement>(null);

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

  const handleNavigateBrowser = (e: React.FormEvent) => {
    e.preventDefault();
    let url = browserInputUrl.trim();
    if (!url.startsWith("http://") && !url.startsWith("https://")) {
      url = "https://" + url;
    }
    setIframeError(false);
    setBrowserUrl(url);
  };

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
            <button className={`workbench-tool-pill ${activeTab === "activity" ? "active" : ""}`} onClick={() => setActiveTab("activity")}>
              Activity
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

          {/* Real context, taken from the loaded Workspace and the operator connection. */}
          <div className="workbench-context-strip">
            <span
              className="workbench-context-chip"
              title={workspaceId ? `Workspace ${workspaceId}` : "No Workspace selected"}
            >
              {workspaceName || "No Workspace selected"}
            </span>
            <span
              className={`workbench-context-chip ${operatorReady ? "is-ready" : "is-offline"}`}
              title="Local operator connection"
            >
              {operatorReady ? "Operator connected" : "Operator not connected"}
            </span>
            {runtimeState && (
              <span className="workbench-context-chip" title="Runtime state reported by the daemon">
                Runtime {runtimeState}
              </span>
            )}
          </div>

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
            <button className="workbench-launcher-row" type="button" onClick={() => setActiveTab("activity")}>
              <span className="workbench-launcher-icon"><ReviewIcon /></span>
              <span className="workbench-launcher-text">Activity</span>
            </button>
            <ComputerStatusNotice />
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
        <TerminalPane platform={typeof navigator === "undefined" ? "unknown" : navigator.platform} />
      ) : activeTab === "activity" ? (
        <ActivityPane />
      ) : activeTab === "browser" ? (
        <section className="workbench-tab-content workbench-browser-view" aria-label="Embedded page preview">
          <div className="browser-chrome-bar">
            <form className="browser-url-form" onSubmit={handleNavigateBrowser}>
              <span className="browser-ssl-badge" title="Embedded page preview, not an isolated browser runtime">🌐</span>
              <input
                type="text"
                className="browser-url-input"
                value={browserInputUrl}
                onChange={e => setBrowserInputUrl(e.target.value)}
                placeholder="Enter a URL to preview..."
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
            {!browserUrl ? (
              <div className="browser-intro-card">
                <h3>Embedded page preview</h3>
                <p>
                  This pane embeds a page in the application renderer. It is not an isolated
                  browser runtime, and no agent browses through it.
                </p>
                <p>Enter a URL above to preview a page. Sites that refuse embedding will not load.</p>
              </div>
            ) : iframeError ? (
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
                title="Embedded page preview"
                className="browser-live-frame"
                sandbox="allow-scripts allow-forms allow-popups"
                onError={() => setIframeError(true)}
              />
            )}
          </div>
        </section>
      ) : (
        <ReviewPane />
      )}
    </aside>
  );
}
