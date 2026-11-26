

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
import { deriveRuntimeHealthRows } from "../runtime/runtime-health";
import { RuntimeStopPreviewPanel } from "../runtime/RuntimeStopPreviewPanel";
import type { StopPreviewState } from "../runtime/runtime-stop-state";
import type { LocalPreferences } from "./local-preferences";
import { agentSettingsDescription } from "./agents-heading";
import { ShortcutList } from "../shell/ShortcutList";
import "./settings-view.css";

export type SettingsCategory =
  | "general"
  | "workspaces"
  | "agents"
  | "notifications"
  | "runtime";

export type SettingsViewProps = {
  localPreferences: LocalPreferences;
  onChangeLocalPreferences: (next: LocalPreferences) => boolean | void;
  runtime: RuntimeStatus | null;
  runtimeLifecycle: RuntimeLifecycleView | null;
  runtimeLifecycleLoading: boolean;
  runtimeLifecycleError: string | null;
  runtimeError: string | null;
  runtimeBusy: boolean;
  startRuntime: () => Promise<void>;
  refreshRuntime: () => Promise<void>;
  /** Stop preview (previewLocalRuntimeStop). Optional so hosts without the bridge render it disabled. */
  stopPreview?: StopPreviewState;
  onPreviewStop?: () => Promise<void>;
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
  onRenameWorkspace: (name: string) => Promise<boolean>;
  renameBusy: boolean;
  renameError: string | null;
  onClearRenameError: () => void;
  onOpenNotifications: () => void;
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
  const [renameDraft, setRenameDraft] = useState<{ workspaceId: string; text: string } | null>(null);
  const [localPreferenceSaveFailed, setLocalPreferenceSaveFailed] = useState(false);
  const runtimeHealth = deriveRuntimeHealthRows(
    {
      operatorReady: props.runtime?.operatorReady ?? false,
      availability: props.runtimeLifecycle?.availability ?? null,
      recoveryState: props.runtimeLifecycle?.incarnation?.recoveryState ?? null,
    },
    typeof navigator === "undefined" ? "" : navigator.platform,
  );

  // General display preferences are LOCAL_PREF: applied at the document root by App and persisted locally.
  const { theme, density, textScale, reducedMotion, richPresentationEnabled } = props.localPreferences;
  const updatePreference = (patch: Partial<LocalPreferences>) => {
    const persisted = props.onChangeLocalPreferences({ ...props.localPreferences, ...patch });
    setLocalPreferenceSaveFailed(persisted === false);
  };

  const selectedWorkspace = props.workspaces.find(ws => ws.workspaceId === props.selectedWorkspaceId);
  const renameDraftText =
    renameDraft && renameDraft.workspaceId === props.selectedWorkspaceId ? renameDraft.text : (selectedWorkspace?.name ?? "");
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
        <button data-control-id="nav-general"
          className={`settings-nav-item ${activeCategory === "general" ? "active" : ""}`}
          type="button"
          onClick={() => setActiveCategory("general")}
        >
          <span className="settings-nav-icon">⚙</span>
          <span>General</span>
        </button>
        <button data-control-id="nav-workspaces"
          className={`settings-nav-item ${activeCategory === "workspaces" ? "active" : ""}`}
          type="button"
          onClick={() => setActiveCategory("workspaces")}
        >
          <span className="settings-nav-icon">📁</span>
          <span>Workspaces</span>
        </button>
        <div className="settings-sidebar-heading">Capabilities &amp; Tools</div>
        <button data-control-id="nav-agents"
          className={`settings-nav-item ${activeCategory === "agents" ? "active" : ""}`}
          type="button"
          onClick={() => setActiveCategory("agents")}
        >
          <span className="settings-nav-icon">🤖</span>
          <span>Agents</span>
        </button>
        <button data-control-id="nav-notifications"
          className={`settings-nav-item ${activeCategory === "notifications" ? "active" : ""}`}
          type="button"
          onClick={() => setActiveCategory("notifications")}
        >
          <span className="settings-nav-icon">🔔</span>
          <span>Notifications</span>
        </button>
        <div className="settings-sidebar-heading">System &amp; Trust</div>
        <button data-control-id="nav-runtime"
          className={`settings-nav-item ${activeCategory === "runtime" ? "active" : ""}`}
          type="button"
          onClick={() => setActiveCategory("runtime")}
        >
          <span className="settings-nav-icon">⚡</span>
          <span>Runtime</span>
        </button>
        <div className="settings-sidebar-heading">Not available yet</div>
        <ul className="settings-unavailable-list" aria-label="Unavailable Settings categories">
          <li>
            <strong>Usage &amp; Monitoring</strong>
            <span>Unavailable until the Feature 018 usage dashboard ships.</span>
          </li>
          <li>
            <strong>Storage &amp; Backup</strong>
            <span>Unavailable until Feature 017 data portability ships.</span>
          </li>
          <li>
            <strong>About &amp; Diagnostics</strong>
            <span>Unavailable until Feature 018 diagnostics ships.</span>
          </li>
        </ul>
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
                  Customize the visual appearance, density, motion, and display scaling of your LiteCowork desktop workspace.
                </p>
              </header>

              {localPreferenceSaveFailed && <p role="status" className="settings-badge warning" data-testid="local-preference-save-status">
                This change is only active for this session because local storage is unavailable. It may reset when LiteCowork closes.
              </p>}

              <ShortcutList />

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
                    <select data-control-id="pref-theme"
                      className="settings-select"
                      value={theme}
                      onChange={e => updatePreference({ theme: e.target.value as LocalPreferences["theme"] })}
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
                    <select data-control-id="pref-density"
                      className="settings-select"
                      value={density}
                      onChange={e => updatePreference({ density: e.target.value as LocalPreferences["density"] })}
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
                    <select data-control-id="pref-text-scale"
                      className="settings-select"
                      value={textScale}
                      onChange={e => updatePreference({ textScale: e.target.value as LocalPreferences["textScale"] })}
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
                      <input data-control-id="pref-reduced-motion"
                        type="checkbox"
                        checked={reducedMotion}
                        onChange={e => updatePreference({ reducedMotion: e.target.checked })}
                      />
                      <span className="settings-toggle-slider" />
                    </label>
                  </div>
                </div>

                <div className="settings-row">
                  <div className="settings-row-info">
                    <span className="settings-row-label" id="pref-rich-presentation-label">Show formatted answers</span>
                    <span className="settings-row-desc" id="pref-rich-presentation-description">
                      Show charts, tables, and other validated views when available. Turn this off to read the complete answer as text. Your messages and provider requests do not change.
                    </span>
                  </div>
                  <div className="settings-row-action">
                    <label className="settings-toggle">
                      <input data-control-id="pref-rich-presentation"
                        type="checkbox"
                        aria-labelledby="pref-rich-presentation-label"
                        aria-describedby="pref-rich-presentation-description"
                        checked={richPresentationEnabled}
                        onChange={e => updatePreference({ richPresentationEnabled: e.target.checked })}
                      />
                      <span className="settings-toggle-slider" />
                    </label>
                  </div>
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
                  <button data-control-id="workspaces-refresh" className="settings-btn" type="button" onClick={() => void props.refreshWorkspaces()}>
                    Refresh
                  </button>
                </div>

                <div className="settings-row">
                  <div className="settings-row-info">
                    <span className="settings-row-label">Current Workspace</span>
                    <span className="settings-row-desc">All conversations, tasks, and file resources belong to this workspace.</span>
                  </div>
                  <div className="settings-row-action">
                    <select data-control-id="workspaces-select"
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

                {selectedWorkspace && (
                  <div className="settings-row">
                    <div className="settings-row-info">
                      <span className="settings-row-label">Workspace Name</span>
                      <span className="settings-row-desc">
                        {selectedWorkspace.status === "ACTIVE"
                          ? "Renaming changes the display name only; it does not move files or change access."
                          : "This workspace is archived and read-only, so it cannot be renamed."}
                      </span>
                    </div>
                    {selectedWorkspace.status === "ACTIVE" ? (
                      <form
                        className="settings-row-action"
                        onSubmit={e => {
                          e.preventDefault();
                          void props.onRenameWorkspace(renameDraftText).then(renamed => {
                            if (renamed) setRenameDraft(null);
                          });
                        }}
                      >
                        <input data-control-id="workspaces-rename-name"
                          type="text"
                          className="settings-select"
                          aria-label="Workspace name"
                          value={renameDraftText}
                          maxLength={200}
                          disabled={props.renameBusy}
                          onChange={e => {
                            props.onClearRenameError();
                            setRenameDraft({ workspaceId: selectedWorkspace.workspaceId, text: e.target.value });
                          }}
                        />
                        <button data-control-id="workspaces-rename-submit"
                          className="settings-btn primary"
                          type="submit"
                          disabled={props.renameBusy || renameDraftText.trim() === "" || renameDraftText.trim() === selectedWorkspace.name || props.runtime?.operatorReady !== true}
                        >
                          {props.renameBusy ? "Renaming…" : "Rename"}
                        </button>
                      </form>
                    ) : (
                      <span className="settings-badge">Read-only</span>
                    )}
                  </div>
                )}
                {props.renameError && (
                  <p className="settings-error" role="alert" aria-live="assertive">{props.renameError}</p>
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
                  <input data-control-id="workspaces-create-name"
                    type="text"
                    className="settings-select"
                    style={{ flex: 1 }}
                    placeholder="Enter new workspace name (e.g. project-dev)"
                    value={props.workspaceName}
                    maxLength={160}
                    disabled={props.workspaceCreateBusy || props.workspaceListState !== "READY"}
                    onChange={e => props.setWorkspaceName(e.target.value)}
                  />
                  <button data-control-id="workspaces-create-submit"
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

                  <textarea data-control-id="workspaces-instructions-text"
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
                    <button data-control-id="workspaces-instructions-save"
                      className="settings-btn primary"
                      type="button"
                      disabled={props.instructionBusy || selectedWorkspace.status !== "ACTIVE"}
                      onClick={() => void props.onSaveInstructions()}
                    >
                      {props.instructionBusy ? "Saving…" : "Save Revision"}
                    </button>
                  </div>
                  {props.instructionMessage && <p className="settings-badge">{props.instructionMessage}</p>}
                  {props.latestInstructionsAfterConflict !== null && (
                    <section className="instruction-conflict" aria-label="Latest saved Workspace instructions">
                      <div className="settings-card-header">
                        <div>
                          <h3 className="settings-card-title">Latest saved version</h3>
                          <p className="settings-card-subtitle">Another save won. Your unsaved draft remains in the editor above.</p>
                        </div>
                        <button data-control-id="workspaces-instructions-use-latest" className="settings-btn" type="button" onClick={props.onUseLatestInstructions}>
                          Use this version
                        </button>
                      </div>
                      <pre>{props.latestInstructionsAfterConflict || "(This revision is empty.)"}</pre>
                    </section>
                  )}
                  {props.instructionHistory.length > 0 && (
                    <details className="instruction-history">
                      <summary>Revision history and source provenance ({props.instructionHistory.length})</summary>
                      <p className="settings-card-subtitle">
                        These are the immutable Workspace instruction sources returned by the authenticated Operator. They describe saved Workspace guidance; they do not claim which sources a Task or agent actually consumed.
                      </p>
                      <ol>
                        {[...props.instructionHistory].reverse().map(revision => (
                          <li key={`${revision.workspaceId}-${revision.revision}`}>
                            <strong>Revision {revision.revision}</strong>
                            <small> {new Date(revision.createdAt).toLocaleString()} · {revision.contentDigest.slice(0, 19)}…</small>
                            <small> Source Resource: {revision.contentRef.resource_id ?? "Unavailable"} · revision: {revision.contentRef.revision_id ?? "Unavailable"}</small>
                            <small> Parent instruction revisions: {revision.parentRevisions.length ? revision.parentRevisions.join(", ") : "None (initial revision)"}</small>
                          </li>
                        ))}
                      </ol>
                    </details>
                  )}
                  <p className="settings-card-subtitle" data-precedence-note="workspace-instructions">
                    Precedence: an Automation or Coworker instruction applies to its own runs in addition to these Workspace instructions; it never replaces or silently overrides them.
                  </p>
                </section>
              )}

              <section className="settings-card runtime-enrollment-card" aria-labelledby="runtime-enrollment-title">
                <div className="settings-card-header">
                  <div>
                    <h2 id="runtime-enrollment-title" className="settings-card-title">This computer’s Workspace access</h2>
                    <p className="settings-card-subtitle">Enrollment authorizes this local Runtime installation for the selected Workspace.</p>
                  </div>
                  <button data-control-id="runtime-enrollment-refresh" className="settings-btn" type="button"
                    onClick={() => void props.onRefreshRuntimeBindings()}
                    disabled={props.runtime?.operatorReady !== true || !props.selectedWorkspaceId}>
                    Refresh
                  </button>
                </div>
                {!selectedWorkspace ? (
                  <p className="settings-card-subtitle">Select a Workspace above to check this computer’s enrollment.</p>
                ) : (
                  <>
                    <p role="status" data-enrollment-state={currentRuntimeEnrolled ? "ENROLLED" : currentRuntimeBinding?.status ?? "NOT_ENROLLED"}>
                      {props.runtimeBindingsLoading ? "Checking…"
                        : props.runtimeBindingError ? "Could not check"
                        : !props.runtime?.runtimeId ? "Runtime unavailable"
                        : currentRuntimeEnrolled ? "Active on this computer"
                        : currentRuntimeBinding?.status === "PENDING" ? "Enrollment pending"
                        : "Not enrolled"}
                      {" "}for {selectedWorkspace.name}
                    </p>
                    {currentRuntimeBinding && (
                      <dl className="settings-details-grid">
                        <div><dt>Enrollment</dt><dd>{currentRuntimeBinding.enrollmentMode.replaceAll("_", " ").toLowerCase()}</dd></div>
                        <div><dt>Roles</dt><dd>{currentRuntimeBinding.roles.map(role => role.replaceAll("_", " ").toLowerCase()).join(", ") || "None reported"}</dd></div>
                      </dl>
                    )}
                    <p className="settings-card-subtitle">
                      This grants this installation its authorized local execution role in the Workspace. It does not pair another computer, start an agent, or execute a Task.
                    </p>
                    {props.runtimeBindingError && <p role="status" className="settings-badge danger">{props.runtimeBindingError}</p>}
                    {props.runtimeEnrollmentError && <p role="alert" className="settings-badge danger">{props.runtimeEnrollmentError}</p>}
                    {props.runtimeEnrollmentMessage && <p role="status" className="settings-badge">{props.runtimeEnrollmentMessage}</p>}
                    {!currentRuntimeEnrolled && (
                      <button data-control-id="runtime-enroll" className="settings-btn primary" type="button"
                        onClick={() => void props.onEnrollLocalRuntime()}
                        disabled={props.runtimeEnrollmentBusy || props.runtimeBindingsLoading || props.runtimeBindingError !== null || props.runtime?.operatorReady !== true || !props.runtime?.runtimeId || selectedWorkspace.status !== "ACTIVE"}>
                        {props.runtimeEnrollmentBusy ? "Enrolling this computer…" : "Enroll this computer"}
                      </button>
                    )}
                  </>
                )}
              </section>

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
                        <button data-control-id="workspaces-archive-cancel" className="settings-btn" type="button" onClick={() => setArchiveConfirmationFor(null)}>
                          Cancel
                        </button>
                        <button data-control-id="workspaces-archive-confirm"
                          className="settings-btn danger"
                          type="button"
                          disabled={props.archiveBusy}
                          onClick={() => void props.onArchiveWorkspace()}
                        >
                          {props.archiveBusy ? "Archiving…" : "Confirm Archive"}
                        </button>
                      </div>
                    ) : (
                      <button data-control-id="workspaces-archive-begin"
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
          {/* NOTIFICATIONS: link only. Quiet hours semantics belong to Feature 010. */}
          {/* ================================================================= */}
          {activeCategory === "notifications" && (
            <>
              <header className="settings-header">
                <h1 className="settings-title">Notifications</h1>
                <p className="settings-description">
                  Delivery history and notification policies are owned by the Notifications page.
                </p>
              </header>
              <section className="settings-card">
                <div className="settings-row">
                  <div className="settings-row-info">
                    <span className="settings-row-label">Quiet hours</span>
                    <span className="settings-row-desc" data-testid="quiet-hours-unavailable">
                      Quiet-hours editing is unavailable until Feature 010 serves its control surface. The saved policy is shown read-only on the Notifications page.
                    </span>
                  </div>
                  <div className="settings-row-action">
                    <button data-control-id="notifications-open"
                      className="settings-btn"
                      type="button"
                      onClick={() => props.onOpenNotifications()}
                    >
                      Open Notifications
                    </button>
                  </div>
                </div>
              </section>
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
                  {agentSettingsDescription(props.agentProfiles)}
                </p>
              </header>
              <p className="settings-card-subtitle" data-precedence-note="default-agent">
                Precedence: the Workspace default lead agent applies to new Conversations. A Coworker or Automation that names its own agent uses that agent for its own runs and does not change this default.
              </p>

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
                      <button data-control-id="runtime-start"
                        className="settings-btn primary"
                        type="button"
                        disabled={props.runtimeBusy}
                        onClick={() => void props.startRuntime()}
                      >
                        {props.runtimeBusy ? "Starting…" : "Start Local Runtime"}
                      </button>
                    )}
                    <button data-control-id="runtime-refresh" className="settings-btn" type="button" onClick={() => void props.refreshRuntime()}>
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
                    <dd>{runtimeHealth.operator}</dd>
                  </div>
                  <div>
                    <dt>Execution</dt>
                    <dd>{runtimeHealth.execution}</dd>
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
                        <dd>{runtimeHealth.recoveryState}</dd>
                      </div>
                    </>
                  )}
                </dl>

                <div aria-live="polite">
                  {props.runtimeLifecycleLoading && (
                    <p role="status" style={{ fontSize: "12.5px", color: "#4b5563" }}>Checking Runtime lifecycle…</p>
                  )}
                </div>
                {props.runtimeLifecycleError && (
                  <p role="alert" style={{ fontSize: "12.5px", color: "#b91c1c" }}>
                    Runtime lifecycle unavailable: {props.runtimeLifecycleError}
                  </p>
                )}
                {props.runtimeError && (
                  <p role="alert" style={{ fontSize: "12.5px", color: "#b91c1c" }}>
                    Runtime error: {props.runtimeError}
                  </p>
                )}

                <div style={{ marginTop: "12px" }}>
                  <h3 className="settings-card-title">Stop and update safety</h3>
                  <p className="settings-card-subtitle">
                    Preview what is running before the Runtime is stopped or LiteCowork is updated. Closing the window does not stop the Runtime.
                  </p>
                  <button data-control-id="runtime-stop-preview" className="settings-btn" type="button"
                    disabled={!props.onPreviewStop || !props.runtime?.operatorReady || props.stopPreview?.status === "LOADING"}
                    onClick={() => void props.onPreviewStop?.()}>
                    {props.stopPreview?.status === "LOADING" ? "Checking…" : "Preview stop"}
                  </button>
                  <RuntimeStopPreviewPanel
                    state={props.stopPreview ?? { status: "IDLE" }}
                    runtimeId={props.runtime?.runtimeId ?? null}
                    incarnationId={props.runtime?.localIncarnationId ?? null}
                  />
                </div>

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
