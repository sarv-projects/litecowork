

import { useState } from "react";
import { AgentCatalogSettings } from "../agents/AgentCatalogSettings";
import { desktopAgentCatalogApi } from "../agents/desktop-agent-catalog-api";
import type {
  AgentInstallationView,
  AgentProfileView,
  AgentBindingView,
  RuntimeWorkspaceBindingView,
  WorkspaceView,
  WorkspaceInstructionView,
  RuntimeStatus,
  RuntimeLifecycleView,
} from "../App";
import type { WorkspaceListState } from "../workspaces/WorkspaceFirstRun";
import type { RuntimeInventoryStreamStatus } from "../runtime/runtime-inventory-stream";
import "./settings-view.css";

export type SettingsCategory =
  | "general"
  | "workspaces"
  | "agents"
  | "runtime";

export type SettingsViewProps = {
  runtime: RuntimeStatus | null;
  runtimeLifecycle: RuntimeLifecycleView | null;
  runtimeLifecycleLoading: boolean;
  runtimeLifecycleError: string | null;
  runtimeError: string | null;
  runtimeBusy: boolean;
  startRuntime: () => Promise<void>;
  refreshRuntime: () => Promise<void>;
  agents: AgentInstallationView[];
  agentError: string | null;
  refreshAgents: () => Promise<void>;
  agentProfiles: AgentProfileView[];
  agentProfileError: string | null;
  runtimeInventoryStreamStatus: RuntimeInventoryStreamStatus | "UNAVAILABLE";
  agentBindings: AgentBindingView[];
  agentBindingError: string | null;
  agentActionMessage: string | null;
  agentActionError: string | null;
  agentActionBusy: string | null;
  runtimeBindings: RuntimeWorkspaceBindingView[];
  runtimeBindingError: string | null;
  runtimeBindingsLoading: boolean;
  runtimeEnrollmentBusy: boolean;
  runtimeEnrollmentMessage: string | null;
  runtimeEnrollmentError: string | null;
  onEnrollLocalRuntime: () => Promise<void>;
  onRefreshRuntimeBindings: () => Promise<void>;
  onProbeCodex: () => Promise<void>;
  onProbeOpenCode: () => Promise<void>;
  onRefreshAgentProfiles: () => Promise<void>;
  onRefreshAgentBindings: () => Promise<void>;
  onCreateAgentBinding: (profile: AgentProfileView, leadEligible: boolean) => Promise<void>;
  onEnableAgentBinding: (binding: AgentBindingView) => Promise<void>;
  onDisableAgentBinding: (binding: AgentBindingView) => Promise<void>;
  workspaces: WorkspaceView[];
  workspaceListState: WorkspaceListState;
  workspaceError: string | null;
  refreshWorkspaces: () => Promise<boolean>;
  workspaceName: string;
  setWorkspaceName: (name: string) => void;
  createWorkspace: () => Promise<void>;
  workspaceCreateBusy: boolean;
  workspaceCreatePending: boolean;
  selectedWorkspaceId: string;
  onSelectWorkspace: (id: string) => void;
  onUpdatePolicy: (policy: string) => Promise<void>;
  policyBusy: boolean;
  policyRetryPending: boolean;
  onArchiveWorkspace: () => Promise<void>;
  archiveBusy: boolean;
  onSetDefaultAgentBinding: (agentBindingId: string | null) => Promise<void>;
  defaultAgentBusy: boolean;
  instructions: string;
  onInstructionsChange: (value: string) => void;
  instructionHistory: WorkspaceInstructionView[];
  instructionMessage: string | null;
  instructionBusy: boolean;
  onSaveInstructions: () => Promise<void>;
  latestInstructionsAfterConflict: string | null;
  onUseLatestInstructions: () => void;
};

export function SettingsView(props: SettingsViewProps) {
  const [activeCategory, setActiveCategory] = useState<SettingsCategory>("workspaces");
  const [archiveConfirmationFor, setArchiveConfirmationFor] = useState<string | null>(null);

  // General Preferences (component-local state; not persisted or applied globally)
  const [theme, setTheme] = useState<"system" | "light" | "dark">("light");
  const [density, setDensity] = useState<"comfortable" | "compact">("comfortable");
  const [textScale, setTextScale] = useState<"100" | "110" | "125">("100");
  const [reducedMotion, setReducedMotion] = useState(false);

  const selectedWorkspace = props.workspaces.find(ws => ws.workspaceId === props.selectedWorkspaceId);
  const currentRuntimeBinding = props.runtimeBindings.find(
    binding => binding.workspaceId === props.selectedWorkspaceId && binding.runtimeId === props.runtime?.runtimeId && binding.status === "ACTIVE"
  ) ?? props.runtimeBindings.find(
    binding => binding.workspaceId === props.selectedWorkspaceId && binding.runtimeId === props.runtime?.runtimeId
  );
  const currentRuntimeEnrolled = currentRuntimeBinding?.status === "ACTIVE";

  return (
    <div className="settings-shell">
      {/* Settings Category Navigation Sidebar */}
      <aside className="settings-sidebar" aria-label="Settings Categories">
        <div className="settings-sidebar-heading">Workspace &amp; App</div>
        <button
          className={`settings-nav-item ${activeCategory === "general" ? "active" : ""}`}
          type="button"
          onClick={() => setActiveCategory("general")}
        >
          <span className="settings-nav-icon">⚙</span>
          <span>General</span>
        </button>
        <button
          className={`settings-nav-item ${activeCategory === "workspaces" ? "active" : ""}`}
          type="button"
          onClick={() => setActiveCategory("workspaces")}
        >
          <span className="settings-nav-icon">📁</span>
          <span>Workspaces</span>
        </button>
        <div className="settings-sidebar-heading">Capabilities &amp; Tools</div>
        <button
          className={`settings-nav-item ${activeCategory === "agents" ? "active" : ""}`}
          type="button"
          onClick={() => setActiveCategory("agents")}
        >
          <span className="settings-nav-icon">🤖</span>
          <span>Agents</span>
        </button>
        <div className="settings-sidebar-heading">System &amp; Trust</div>
        <button
          className={`settings-nav-item ${activeCategory === "runtime" ? "active" : ""}`}
          type="button"
          onClick={() => setActiveCategory("runtime")}
        >
          <span className="settings-nav-icon">⚡</span>
          <span>Runtime</span>
        </button>
      </aside>

      {/* Main Settings Detail Content */}
      <main className="settings-content" tabIndex={-1}>
        <div className="settings-section-container">
          {/* ================================================================= */}
          {/* 1. GENERAL & DESKTOP                                              */}
          {/* ================================================================= */}
          {activeCategory === "general" && (
            <>
              <header className="settings-header">
                <h1 className="settings-title">General &amp; Desktop</h1>
                <p className="settings-description">
                  Customize the visual appearance, density, keyboard interaction, and display scaling of your LiteCowork desktop workspace.
                </p>
              </header>

              <section className="settings-card">
                <div className="settings-card-header">
                  <div>
                    <h2 className="settings-card-title">Appearance &amp; Theme</h2>
                    <p className="settings-card-subtitle">Choose how LiteCowork adapts to your operating system styling.</p>
                  </div>
                </div>

                <div className="settings-row">
                  <div className="settings-row-info">
                    <span className="settings-row-label">Interface Theme</span>
                    <span className="settings-row-desc">Select between a calm high-contrast light mode, dark mode, or system sync.</span>
                  </div>
                  <div className="settings-row-action">
                    <select
                      className="settings-select"
                      value={theme}
                      onChange={e => setTheme(e.target.value as "system" | "light" | "dark")}
                    >
                      <option value="light">Light (Calm Workstation)</option>
                      <option value="dark">Dark (Deep Contrast)</option>
                      <option value="system">Follow System</option>
                    </select>
                  </div>
                </div>

                <div className="settings-row">
                  <div className="settings-row-info">
                    <span className="settings-row-label">Layout Density</span>
                    <span className="settings-row-desc">Adjust padding in lists, sidebar items, and conversation blocks.</span>
                  </div>
                  <div className="settings-row-action">
                    <select
                      className="settings-select"
                      value={density}
                      onChange={e => setDensity(e.target.value as "comfortable" | "compact")}
                    >
                      <option value="comfortable">Comfortable</option>
                      <option value="compact">Compact</option>
                    </select>
                  </div>
                </div>

                <div className="settings-row">
                  <div className="settings-row-info">
                    <span className="settings-row-label">Text Scale</span>
                    <span className="settings-row-desc">Base font scaling across conversation transcripts and documents.</span>
                  </div>
                  <div className="settings-row-action">
                    <select
                      className="settings-select"
                      value={textScale}
                      onChange={e => setTextScale(e.target.value as "100" | "110" | "125")}
                    >
                      <option value="100">100% (Default 15px)</option>
                      <option value="110">110% (16.5px)</option>
                      <option value="125">125% (18.75px)</option>
                    </select>
                  </div>
                </div>

                <div className="settings-row">
                  <div className="settings-row-info">
                    <span className="settings-row-label">Reduce Motion</span>
                    <span className="settings-row-desc">Minimize panel sliding animations and stream scroll transitions.</span>
                  </div>
                  <div className="settings-row-action">
                    <label className="settings-toggle">
                      <input
                        type="checkbox"
                        checked={reducedMotion}
                        onChange={e => setReducedMotion(e.target.checked)}
                      />
                      <span className="settings-toggle-slider" />
                    </label>
                  </div>
                </div>
              </section>

              <section className="settings-card">
                <div className="settings-card-header">
                  <div>
                    <h2 className="settings-card-title">Keyboard Shortcuts</h2>
                    <p className="settings-card-subtitle">Global shortcuts configured for high-speed local workstation navigation.</p>
                  </div>
                </div>
                <div className="settings-row">
                  <div className="settings-row-info">
                    <span className="settings-row-label">New Conversation</span>
                    <span className="settings-row-desc">Clear draft and focus conversation composer</span>
                  </div>
                  <kbd className="settings-badge">Ctrl + N</kbd>
                </div>
                <div className="settings-row">
                  <div className="settings-row-info">
                    <span className="settings-row-label">Toggle Side Workbench</span>
                    <span className="settings-row-desc">Show or hide the right tool workbench panel</span>
                  </div>
                  <kbd className="settings-badge">Ctrl + W</kbd>
                </div>
                <div className="settings-row">
                  <div className="settings-row-info">
                    <span className="settings-row-label">Global Search / Library</span>
                    <span className="settings-row-desc">Quickly find saved files, artifacts, and conversations</span>
                  </div>
                  <kbd className="settings-badge">Ctrl + K</kbd>
                </div>
              </section>
            </>
          )}

          {/* ================================================================= */}
          {/* 2. WORKSPACES & ROOTS                                             */}
          {/* ================================================================= */}
          {activeCategory === "workspaces" && (
            <>
              <header className="settings-header">
                <h1 className="settings-title">Workspaces &amp; Roots</h1>
                <p className="settings-description">
                  Manage local workspaces, file boundaries, instructions, and storage policies. A workspace is the resource and trust boundary.
                </p>
              </header>

              <section className="settings-card">
                <div className="settings-card-header">
                  <div>
                    <h2 className="settings-card-title">Active Workspace</h2>
                    <p className="settings-card-subtitle">Select or inspect the active project root.</p>
                  </div>
                  <button className="settings-btn" type="button" onClick={() => void props.refreshWorkspaces()}>
                    Refresh
                  </button>
                </div>

                <div className="settings-row">
                  <div className="settings-row-info">
                    <span className="settings-row-label">Current Workspace</span>
                    <span className="settings-row-desc">All conversations, tasks, and file resources belong to this workspace.</span>
                  </div>
                  <div className="settings-row-action">
                    <select
                      className="settings-select"
                      value={props.selectedWorkspaceId}
                      onChange={e => props.onSelectWorkspace(e.target.value)}
                    >
                      {props.workspaces.map(ws => (
                        <option key={ws.workspaceId} value={ws.workspaceId}>
                          {ws.name} ({ws.status})
                        </option>
                      ))}
                    </select>
                  </div>
                </div>

                {selectedWorkspace && (
                  <dl className="settings-details-grid">
                    <div>
                      <dt>Workspace ID</dt>
                      <dd>{selectedWorkspace.workspaceId}</dd>
                    </div>
                    <div>
                      <dt>Status</dt>
                      <dd>{selectedWorkspace.status}</dd>
                    </div>
                    <div>
                      <dt>Storage Policy</dt>
                      <dd>{selectedWorkspace.replicationPolicy}</dd>
                    </div>
                    <div>
                      <dt>Version</dt>
                      <dd>Rev {selectedWorkspace.version}</dd>
                    </div>
                  </dl>
                )}
              </section>

              <section className="settings-card">
                <div className="settings-card-header">
                  <div>
                    <h2 className="settings-card-title">Create Workspace</h2>
                    <p className="settings-card-subtitle">Register a new local workspace boundary.</p>
                  </div>
                </div>
                <form
                  style={{ display: "flex", gap: "10px" }}
                  onSubmit={e => {
                    e.preventDefault();
                    void props.createWorkspace();
                  }}
                >
                  <input
                    type="text"
                    className="settings-select"
                    style={{ flex: 1 }}
                    placeholder="Enter new workspace name (e.g. project-dev)"
                    value={props.workspaceName}
                    maxLength={160}
                    disabled={props.workspaceCreateBusy || props.workspaceListState !== "READY"}
                    onChange={e => props.setWorkspaceName(e.target.value)}
                  />
                  <button
                    className="settings-btn primary"
                    type="submit"
                    disabled={props.workspaceCreateBusy || !props.workspaceName.trim() || props.runtime?.operatorReady !== true}
                  >
                    {props.workspaceCreateBusy ? "Creating…" : "Create Workspace"}
                  </button>
                </form>
                {props.workspaceError && <p className="settings-badge danger">{props.workspaceError}</p>}
              </section>

              {selectedWorkspace && (
                <section className="settings-card">
                  <div className="settings-card-header">
                    <div>
                      <h2 className="settings-card-title">Workspace Instructions</h2>
                      <p className="settings-card-subtitle">
                        Shared conventions and guidelines provided to agents during conversation and task turns.
                      </p>
                    </div>
                    <span className="settings-badge">
                      {props.instructionHistory.length > 0 ? `Rev ${props.instructionHistory[props.instructionHistory.length - 1].revision}` : "None"}
                    </span>
                  </div>

                  <textarea
                    rows={6}
                    className="settings-select"
                    style={{ width: "100%", fontFamily: "inherit", resize: "vertical" }}
                    value={props.instructions}
                    maxLength={65536}
                    disabled={props.instructionBusy || selectedWorkspace.status !== "ACTIVE"}
                    onChange={e => props.onInstructionsChange(e.target.value)}
                    placeholder="Document architecture invariants, style standards, and rules for agents working in this workspace..."
                  />

                  <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                    <small style={{ color: "#6b7280" }}>
                      {new TextEncoder().encode(props.instructions).byteLength.toLocaleString()} / 65,536 bytes (Immutable revision)
                    </small>
                    <button
                      className="settings-btn primary"
                      type="button"
                      disabled={props.instructionBusy || selectedWorkspace.status !== "ACTIVE"}
                      onClick={() => void props.onSaveInstructions()}
                    >
                      {props.instructionBusy ? "Saving…" : "Save Revision"}
                    </button>
                  </div>
                  {props.instructionMessage && <p className="settings-badge">{props.instructionMessage}</p>}
                </section>
              )}

              {selectedWorkspace && (
                <section className="settings-card">
                  <div className="settings-card-header">
                    <div>
                      <h2 className="settings-card-title">Workspace Lifecycle</h2>
                      <p className="settings-card-subtitle">Archive this workspace into a terminal, read-only state.</p>
                    </div>
                    <span className={`settings-badge ${selectedWorkspace.status === "ACTIVE" ? "success" : "warning"}`}>
                      {selectedWorkspace.status}
                    </span>
                  </div>
                  {selectedWorkspace.status === "ACTIVE" ? (
                    archiveConfirmationFor === selectedWorkspace.workspaceId ? (
                      <div style={{ display: "flex", gap: "10px", alignItems: "center" }}>
                        <span style={{ fontSize: "13px", color: "#b91c1c" }}>Confirm read-only transition?</span>
                        <button className="settings-btn" type="button" onClick={() => setArchiveConfirmationFor(null)}>
                          Cancel
                        </button>
                        <button
                          className="settings-btn danger"
                          type="button"
                          disabled={props.archiveBusy}
                          onClick={() => void props.onArchiveWorkspace()}
                        >
                          {props.archiveBusy ? "Archiving…" : "Confirm Archive"}
                        </button>
                      </div>
                    ) : (
                      <button
                        className="settings-btn danger"
                        type="button"
                        onClick={() => setArchiveConfirmationFor(selectedWorkspace.workspaceId)}
                      >
                        Archive Workspace…
                      </button>
                    )
                  ) : (
                    <p style={{ fontSize: "13px", color: "#6b7280" }}>This workspace is archived and read-only.</p>
                  )}
                </section>
              )}
            </>
          )}

          {/* ================================================================= */}
          {/* 3. AGENT HARNESSES & MODELS                                       */}
          {/* ================================================================= */}
          {activeCategory === "agents" && (
            <>
              <header className="settings-header">
                <h1 className="settings-title">Agent Harnesses &amp; Models</h1>
                <p className="settings-description">
                  Configure detected native agent harnesses (OpenCode, Codex, Claude Code, Gemini). LiteCowork coordinates native harnesses without imposing an external reasoning loop.
                </p>
              </header>

              <AgentCatalogSettings
                workspaceId={props.selectedWorkspaceId || null}
                workspaceName={selectedWorkspace?.name}
                workspaceActive={selectedWorkspace?.status === "ACTIVE"}
                runtimeReady={props.runtime?.operatorReady === true}
                runtimeEnrolled={currentRuntimeEnrolled === true}
                selectedDefaultLeadBindingId={selectedWorkspace?.defaultAgentBindingId ?? null}
                installations={props.agents}
                profiles={props.agentProfiles}
                runtimeInventoryStreamStatus={props.runtimeInventoryStreamStatus === "STOPPED" ? "UNAVAILABLE" : props.runtimeInventoryStreamStatus}
                bindings={props.agentBindings}
                error={[props.agentError, props.agentProfileError, props.agentBindingError].filter(Boolean).join(" ") || null}
                actionMessage={props.agentActionMessage}
                actionError={props.agentActionError}
                busyAction={props.agentActionBusy}
                defaultAgentBusy={props.defaultAgentBusy}
                registryApi={desktopAgentCatalogApi}
                onRefresh={() => {
                  void props.refreshAgents();
                  void props.onRefreshAgentProfiles();
                  void props.onRefreshAgentBindings();
                }}
                onProbeCodex={() => void props.onProbeCodex()}
                onProbeOpenCode={() => void props.onProbeOpenCode()}
                onCreateBinding={(profile, leadEligible) => void props.onCreateAgentBinding(profile, leadEligible)}
                onEnableBinding={binding => void props.onEnableAgentBinding(binding)}
                onDisableBinding={binding => void props.onDisableAgentBinding(binding)}
                onSetDefaultLead={id => void props.onSetDefaultAgentBinding(id)}
              />
            </>
          )}

          {/* ================================================================= */}
          {/* 4. BROWSER USE (ZCode Audit Inspiration §2.2)                     */}
          {/* ================================================================= */}

          {/* ================================================================= */}
          {/* 5. TERMINAL & SHELL                                               */}
          {/* ================================================================= */}

          {/* ================================================================= */}
          {/* 6. PERMISSIONS & SECURITY (ZCode Audit Inspiration §2.7)          */}
          {/* ================================================================= */}

          {/* ================================================================= */}
          {/* 7. LOCAL RUNTIME & DIAGNOSTICS                                    */}
          {/* ================================================================= */}
          {activeCategory === "runtime" && (
            <>
              <header className="settings-header">
                <h1 className="settings-title">Local Runtime &amp; Diagnostics</h1>
                <p className="settings-description">
                  Inspect the authenticated <code>litecoworkd</code> daemon, incarnation lifecycle, socket health, and startup policy.
                </p>
              </header>

              <section className="settings-card">
                <div className="settings-card-header">
                  <div>
                    <h2 className="settings-card-title">Runtime Connection</h2>
                    <p className="settings-card-subtitle">
                      {props.runtime?.operatorReady ? "Local Operator API is serving and ready." : "Daemon is currently disconnected."}
                    </p>
                  </div>
                  <div style={{ display: "flex", gap: "8px" }}>
                    {props.runtime?.state !== "READY" && (
                      <button
                        className="settings-btn primary"
                        type="button"
                        disabled={props.runtimeBusy}
                        onClick={() => void props.startRuntime()}
                      >
                        {props.runtimeBusy ? "Starting…" : "Start Local Runtime"}
                      </button>
                    )}
                    <button className="settings-btn" type="button" onClick={() => void props.refreshRuntime()}>
                      Refresh Status
                    </button>
                  </div>
                </div>

                <dl className="settings-details-grid">
                  <div>
                    <dt>Daemon State</dt>
                    <dd>{props.runtime?.state ?? "UNKNOWN"}</dd>
                  </div>
                  <div>
                    <dt>Operator API</dt>
                    <dd>{props.runtime?.operatorReady ? "Connected (Localhost)" : "Not Serving"}</dd>
                  </div>
                  <div>
                    <dt>Runtime ID</dt>
                    <dd>{props.runtime?.runtimeId ?? "Unavailable"}</dd>
                  </div>
                  <div>
                    <dt>Incarnation ID</dt>
                    <dd>{props.runtime?.localIncarnationId ?? "None"}</dd>
                  </div>
                  {props.runtimeLifecycle && (
                    <>
                      <div>
                        <dt>Startup Policy</dt>
                        <dd>{props.runtimeLifecycle.startupPolicy}</dd>
                      </div>
                      <div>
                        <dt>Recovery State</dt>
                        <dd>{props.runtimeLifecycle.incarnation.recoveryState}</dd>
                      </div>
                    </>
                  )}
                </dl>

                {props.runtimeLifecycle?.blockers && props.runtimeLifecycle.blockers.length > 0 && (
                  <div style={{ marginTop: "12px" }}>
                    <strong style={{ fontSize: "13px", color: "#b91c1c" }}>Active Blockers:</strong>
                    <ul style={{ margin: "6px 0 0", paddingLeft: "18px", fontSize: "12.5px", color: "#4b5563" }}>
                      {props.runtimeLifecycle.blockers.map(b => (
                        <li key={b.blockerId}>
                          <strong>{b.safeMessage}</strong>: {b.resolutionHint}
                        </li>
                      ))}
                    </ul>
                  </div>
                )}
              </section>
            </>
          )}

          {/* ================================================================= */}
          {/* 8. SKILLS & MCP (LITESPM)                                         */}
          {/* ================================================================= */}

          {/* ================================================================= */}
          {/* 9. COWORKER DEFAULTS                                              */}
          {/* ================================================================= */}
        </div>
      </main>
    </div>
  );
}
