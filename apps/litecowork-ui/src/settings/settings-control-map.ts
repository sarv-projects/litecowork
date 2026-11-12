/**
 * One entry per interactive control rendered directly by SettingsView (011-T06, B-29/B-33).
 * Classes follow docs/V1-SETTINGS-CONTRACT-LEDGER.md "Pinned V1 interactive control set".
 * A control may only be interactive if it names a served operation or an applied local effect.
 */
export type SettingsControlClass = "LocalOnly" | "Native" | "Target" | "OperatorServed";

export type SettingsControlEntry = {
  class: SettingsControlClass;
  /** Served operationId, desktop bridge command, or local effect. */
  effect: string;
};

export const SETTINGS_CONTROL_MAP: Readonly<Record<string, SettingsControlEntry>> = {
  "nav-general": { class: "LocalOnly", effect: "category navigation" },
  "nav-workspaces": { class: "LocalOnly", effect: "category navigation" },
  "nav-agents": { class: "LocalOnly", effect: "category navigation" },
  "nav-notifications": { class: "LocalOnly", effect: "category navigation" },
  "notifications-open": { class: "LocalOnly", effect: "navigate to the Notifications page (Feature 010 owns quiet hours)" },
  "nav-runtime": { class: "LocalOnly", effect: "category navigation" },
  "pref-theme": { class: "LocalOnly", effect: "LocalPreferenceStore theme applied at document root" },
  "pref-density": { class: "LocalOnly", effect: "LocalPreferenceStore density applied at document root" },
  "pref-text-scale": { class: "LocalOnly", effect: "LocalPreferenceStore text scale applied at document root" },
  "pref-reduced-motion": { class: "LocalOnly", effect: "LocalPreferenceStore reduced motion applied at document root" },
  "workspaces-refresh": { class: "OperatorServed", effect: "listWorkspaces" },
  "workspaces-select": { class: "LocalOnly", effect: "selected Workspace in desktop state" },
  "workspaces-create-name": { class: "OperatorServed", effect: "createWorkspace draft" },
  "workspaces-create-submit": { class: "OperatorServed", effect: "createWorkspace" },
  "workspaces-instructions-text": { class: "OperatorServed", effect: "Workspace instructions draft" },
  "workspaces-instructions-save": { class: "OperatorServed", effect: "createWorkspaceInstructionRevision" },
  "workspaces-instructions-use-latest": { class: "LocalOnly", effect: "replace the instruction draft with the latest saved revision after a conflict" },
  "workspaces-archive-cancel": { class: "LocalOnly", effect: "dismiss archive confirmation" },
  "workspaces-archive-confirm": { class: "OperatorServed", effect: "archive_workspace" },
  "workspaces-archive-begin": { class: "LocalOnly", effect: "open two-step archive confirmation" },
  "workspaces-rename-name": { class: "OperatorServed", effect: "renameWorkspace draft" },
  "workspaces-rename-submit": { class: "OperatorServed", effect: "renameWorkspace" },
  "runtime-start": { class: "Native", effect: "start_local_runtime desktop bridge" },
  "runtime-refresh": { class: "OperatorServed", effect: "getLocalRuntimeLifecycle" },
  "runtime-enroll": { class: "OperatorServed", effect: "enroll_local_runtime desktop bridge (Workspace runtime binding)" },
  "runtime-enrollment-refresh": { class: "OperatorServed", effect: "list_workspace_runtime_bindings desktop bridge" },
  "runtime-stop-preview": { class: "OperatorServed", effect: "previewLocalRuntimeStop (read-only; Stop safely renders as text until requestLocalRuntimeStop is mounted)" },
};

/**
 * Controls rendered by AgentCatalogSettings (011-T09). Same classes as SETTINGS_CONTROL_MAP.
 * Descriptor, sign-in and install/update operations are not rendered as controls here: they are
 * unavailable text with the reason (Feature 002 owns their semantics).
 */
export const AGENT_CATALOG_CONTROL_MAP: Readonly<Record<string, SettingsControlEntry>> = {
  "agent-default-clear-begin": { class: "LocalOnly", effect: "open clear-default confirmation" },
  "agent-default-clear-cancel": { class: "LocalOnly", effect: "dismiss clear-default confirmation" },
  "agent-default-clear-confirm": { class: "Native", effect: "set_workspace_default_agent_binding desktop bridge (clear)" },
  "agent-catalog-refresh": { class: "Native", effect: "list_agent_installations, list_agent_profiles and list_agent_bindings desktop bridge" },
  "agent-registry-refresh": { class: "Native", effect: "refresh_agent_registry desktop bridge" },
  "agent-registry-search": { class: "Native", effect: "list_agent_registry query draft" },
  "agent-registry-search-submit": { class: "Native", effect: "list_agent_registry desktop bridge" },
  "agent-registry-load-more": { class: "Native", effect: "list_agent_registry next page" },
  "agent-probe-codex": { class: "Native", effect: "probe_agent_profile desktop bridge (CODEX)" },
  "agent-probe-opencode": { class: "Native", effect: "probe_agent_profile desktop bridge (OPENCODE)" },
  "agent-control-inspect": { class: "LocalOnly", effect: "toggle the module-declared AgentControlDescriptor panel (module-less or stale descriptors render UNAVAILABLE)" },
  "agent-binding-lead-eligible": { class: "LocalOnly", effect: "binding draft: lead eligibility sent with create_agent_binding" },
  "agent-binding-create": { class: "Native", effect: "create_agent_binding desktop bridge" },
  "agent-binding-enable": { class: "Native", effect: "enable_agent_binding desktop bridge" },
  "agent-binding-disable": { class: "Native", effect: "disable_agent_binding desktop bridge" },
  "agent-binding-set-default": { class: "Native", effect: "set_workspace_default_agent_binding desktop bridge" },
  "worker-profiles-refresh": { class: "Native", effect: "list_delegation_profiles desktop bridge" },
  "worker-profile-edit": { class: "Native", effect: "revise_delegation_profile desktop bridge (opens the editor)" },
  "worker-profile-duplicate-begin": { class: "LocalOnly", effect: "open duplicate form" },
  "worker-profile-disable": { class: "Native", effect: "delegation profile status change desktop bridge (DISABLED)" },
  "worker-profile-archive-begin": { class: "LocalOnly", effect: "open archive confirmation" },
  "worker-profile-duplicate-use-latest": { class: "LocalOnly", effect: "rebase duplicate draft on the latest source revision" },
  "worker-profile-duplicate-name": { class: "LocalOnly", effect: "duplicate draft name" },
  "worker-profile-duplicate-cancel": { class: "LocalOnly", effect: "dismiss duplicate form" },
  "worker-profile-duplicate-submit": { class: "Native", effect: "duplicate_delegation_profile desktop bridge" },
  "worker-profile-archive-review-latest": { class: "LocalOnly", effect: "rebase archive confirmation on the latest revision" },
  "worker-profile-archive-cancel": { class: "LocalOnly", effect: "dismiss archive confirmation" },
  "worker-profile-archive-confirm": { class: "Native", effect: "delegation profile status change desktop bridge (ARCHIVED)" },
  "worker-profile-create-begin": { class: "LocalOnly", effect: "open create-profile editor" },
  "worker-profile-form-name": { class: "LocalOnly", effect: "profile editor draft name" },
  "worker-profile-form-routing": { class: "LocalOnly", effect: "profile editor draft routing" },
  "worker-profile-form-instructions": { class: "LocalOnly", effect: "profile editor draft instructions" },
  "worker-profile-form-load-latest": { class: "Native", effect: "list_delegation_profile_revisions desktop bridge (reload latest)" },
  "worker-profile-form-cancel": { class: "LocalOnly", effect: "dismiss profile editor" },
  "worker-profile-form-save": { class: "Native", effect: "create_delegation_profile or revise_delegation_profile desktop bridge" },
};
