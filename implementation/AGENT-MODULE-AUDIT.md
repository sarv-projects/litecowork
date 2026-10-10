# Agent Module Current-Source Audit and Refactor Map

**Date:** 2026-10-10  
**Status:** Planning/implementation handoff. This file does not claim the target is implemented.  
**Normative contracts:** `docs/AGENT-CONTROL.md`, `docs/AGENT-FABRIC.md`,
ADR-0025, `docs/API.md`, `implementation/UI.md`.

## 1. Current source state

The repository already contains useful bounded native-agent work, but its current public
surface is a **transitional two-agent slice**.

Current code truth:

- `apps/litecoworkd/src/agents/mod.rs`
  - `discover_local_installations()` fans out directly to Codex and OpenCode.
  - `InstallationState` is only `Missing | Installed | VersionUnavailable`.
  - authentication is always `Unknown`.
  - session readiness is always `NotProbed`.
  - `discover_version(binary, expected_prefix)` is a useful bounded primitive but is
    currently called through agent-specific modules rather than a module registry.
- `apps/litecoworkd/src/agents/codex_profile_probe.rs`
  - contains useful bounded Codex observation logic.
  - remains Codex-specific and should move behind `CodexAgentModule.lifecycle()`.
- `apps/litecoworkd/src/agents/opencode_profile_probe.rs`
  - contains useful bounded OpenCode observation logic.
  - remains OpenCode-specific and should move behind `OpenCodeAgentModule.lifecycle()`.
- `apps/litecoworkd/src/operator.rs`
  - `list_agent_profiles` is generic enough to keep.
  - `probe_local_agent_profile` branches on `CODEX | OPENCODE`; this is transitional.
  - target registry/install/update/auth/control/configuration routes do not yet exist.
- `apps/litecowork-ui/src/agents/agent-catalog-api.ts`
  - `AgentProviderKey = "CODEX" | "OPENCODE"`.
  - explicit `probeCodex()` and `probeOpenCode()`.
  - target should expose generic registry/lifecycle/control/binding functions.
- `apps/litecowork-ui/src/agents/AgentCatalogSettings.tsx`
  - already distinguishes installed inventory from profile/binding truth in several places.
  - still presents explicit Codex/OpenCode probe actions and is not the final registry
    master-detail design.
- `apps/litecowork-ui/src-tauri/src/lib.rs`
  - `probe_agent_profile(... provider_key ...)` validates only `CODEX | OPENCODE`.
  - Tauri currently owns provider-name branching that must disappear.
- `apps/litecowork-ui/src/App.tsx`
  - Settings wiring still carries `onProbeCodex`, `onProbeOpenCode`.
  - agent setup is part of the monolithic SettingsPage wiring.
- `crates/storage-core` / `crates/storage-sqlite`
  - current AgentBinding persistence is safe but intentionally minimal.
  - target typed configuration/default-session-option/credential-slot persistence is not
    implemented.
- `docs/schemas/sqlite-v16.sql`
  - target migration contract exists in docs only; no Rust migration implementation is
    claimed yet.

None of the above current-source limitations should be represented as final architecture.

## 2. Keep, refactor, replace

| Current item | Disposition | Target |
|---|---|---|
| bounded `--version` discovery primitive | KEEP/GENERALIZE | lifecycle adapter helper |
| Codex profile probe parsing/sanitization | KEEP/WRAP | `CodexAgentModule.lifecycle().probe_control_descriptor` |
| OpenCode profile probe parsing/sanitization | KEEP/WRAP | `OpenCodeAgentModule.lifecycle().probe_control_descriptor` |
| `list_agent_profiles` storage/API projection | KEEP | generic AgentProfile projection |
| AgentBinding enable/lead eligibility | KEEP | add typed configuration/defaults |
| `CODEX | OPENCODE` UI union | REMOVE | registry agent/module identity |
| `probeCodex/probeOpenCode` UI methods | REMOVE | `refreshAgentControl(agentProfileId)` |
| provider-key branching in Tauri | REMOVE | generic Operator API methods |
| provider-key branching in Operator Core route | REMOVE | AgentModuleRegistry lookup |
| standalone provider settings concept | DO NOT ADD | per-agent native/auth/config section |
| model names hard-coded in LiteCowork UI | DO NOT ADD | AgentControlDescriptor session options |
| copied native MCP/plugin/skill config | DO NOT ADD | native summary + LiteSPM bridge |
| raw secret in AgentBinding configuration | FORBIDDEN | native store or SecretRef slot |

## 3. Target module layout

Exact crate/package naming can change before implementation, but ownership cannot.

~~~text
crates/
  agent-module-core/
    src/
      manifest.rs
      module.rs
      lifecycle.rs
      control_descriptor.rs
      validation.rs
      capability_bridge.rs

  agent-registry/
    src/
      client.rs
      cache.rs
      schema.rs
      service.rs

  agent-lifecycle/
    src/
      service.rs
      installation.rs
      auth.rs
      credentials.rs
      operations.rs
      errors.rs

apps/litecoworkd/src/agents/
  mod.rs
  registry.rs
  modules/
    codex/
      mod.rs
      lifecycle.rs
      runtime.rs
      conformance.rs
    opencode/
      mod.rs
      lifecycle.rs
      runtime.rs
      conformance.rs
    claude/
      ...
  control.rs

apps/litecowork-ui/src/agents/
  AgentRegistryPage.tsx
  AgentRegistryRail.tsx
  AgentDetailPanel.tsx
  InstallationCard.tsx
  AuthenticationCard.tsx
  AgentConfigurationCard.tsx
  AgentSessionOptionsCard.tsx
  AgentNativeSurfaceCard.tsx
  AgentBindingsCard.tsx
  agent-registry-api.ts
  agent-control-api.ts

apps/litecowork-ui/src/conversations/
  useComposerAgentSurface.ts
  AgentPicker.tsx
  AgentOptionControl.tsx
  NativeCommandPalette.tsx
  NativeReferencePalette.tsx
  DraftCompatibilityBanner.tsx
~~~

## 4. Daemon call graph

~~~text
Operator GET /agent-registry
  -> AgentRegistryService.list_registry()
     -> registry cache

Operator POST /agent-registry/{id}/install
  -> AgentLifecycleService.install()
     -> AgentModuleRegistry.match(id)
     -> module.lifecycle().compile/execute bounded plan
     -> lifecycle-operation store
     -> re-probe installation observation

Operator POST /agent-profiles/{profile}/refresh
  -> AgentLifecycleService.refresh_control_descriptor()
     -> resolve AgentModule from profile/registry identity
     -> lifecycle.discover_installation()
     -> lifecycle.probe_control_descriptor()
     -> validate_control_descriptor()
     -> runtime-local descriptor store
     -> normalized AgentProfile/RuntimeOffer projection

Operator POST /agent-profiles/{profile}/auth/{method}/begin
  -> AgentLifecycleService.begin_auth()
     -> descriptor method lookup
     -> module.lifecycle().begin_auth()

Operator POST /agent-profiles/{profile}/credentials/{slot}
  -> AgentLifecycleService.submit_secret()
     -> descriptor secure-slot validation
     -> SecretStore.write()
     -> AgentCredentialBinding(SecretRef)

Operator PUT /agent-bindings/{binding}/configuration
  -> AgentBindingService.configure_binding()
     -> fresh descriptor
     -> closed config validation
     -> SecretRef slot validation
     -> session-option validation
     -> one storage transaction + event + receipt
~~~

## 5. Target Rust interfaces

These mirror the normative LLD. Signatures may use project-specific Result/error types.

~~~rust
pub trait AgentModule: Send + Sync {
    fn manifest(&self) -> &AgentModuleManifest;
    fn lifecycle(&self) -> &dyn AgentLifecycleAdapter;
    fn runtime(&self) -> &dyn AgentAdapter;
    fn capability_bridge(&self) -> Option<&dyn AgentCapabilityBridge>;
}

pub trait AgentLifecycleAdapter: Send + Sync {
    fn discover_installation(
        &self,
        context: &RuntimeContext,
    ) -> Result<AgentInstallationObservation, AgentLifecycleError>;

    fn compile_install_plan(
        &self,
        distribution: &AgentDistribution,
    ) -> Result<AgentInstallPlan, AgentLifecycleError>;

    fn probe_control_descriptor(
        &self,
        context: &WorkspaceRuntimeContext,
        installation: &AgentInstallationObservation,
    ) -> Result<AgentControlDescriptor, AgentLifecycleError>;

    fn begin_auth(
        &self,
        context: &WorkspaceRuntimeContext,
        descriptor: &AgentControlDescriptor,
        method_id: &str,
    ) -> Result<AgentAuthChallenge, AgentLifecycleError>;

    fn open_native_config(
        &self,
        context: &RuntimeContext,
        target_id: &str,
    ) -> Result<NativeConfigOpenReceipt, AgentLifecycleError>;
}
~~~

### AgentModuleRegistry

~~~rust
pub struct AgentModuleRegistry {
    modules_by_id: HashMap<AgentModuleId, Arc<dyn AgentModule>>,
    registry_claims: HashMap<RegistryAgentId, Vec<AgentModuleId>>,
}

impl AgentModuleRegistry {
    pub fn register(&mut self, module: Arc<dyn AgentModule>) -> Result<(), ModuleError>;

    pub fn match_registry_agent(
        &self,
        registry_agent_id: &RegistryAgentId,
        os: OperatingSystem,
        descriptor_api_version: u32,
    ) -> Result<Option<Arc<dyn AgentModule>>, ModuleError>;
}
~~~

Logic:

~~~text
register(module):
  validate manifest is closed/bounded
  reject duplicate module_id
  record each claimed registry agent ID
  do not execute module code during registration

match_registry_agent(...):
  candidates = compatible claims
  filter unsupported OS / descriptor API
  if zero -> None
  if one -> module
  if more than one equally eligible -> AGENT_UNAVAILABLE + audit diagnostic
  never choose based on arbitrary registry ordering
~~~

### validate_control_descriptor

~~~rust
fn validate_control_descriptor(
    manifest: &AgentModuleManifest,
    descriptor: AgentControlDescriptor,
    now: Timestamp,
) -> Result<ValidatedAgentControlDescriptor, DescriptorError>
~~~

Checks:

~~~text
descriptor.module_id == manifest.module_id
descriptor.descriptor_api_version supported
descriptor.expires_at > now
all strings/arrays within limits
digest recomputes over canonical non-secret projection

auth:
  method IDs unique
  SECRET_SLOT method points to declared slot
  LITECOWORK_SECRET_STORE slot uses allowed secret kind

configuration:
  field IDs unique
  no secret field types/secret-looking ownership
  schema closed and bounded

session options:
  option IDs unique
  JSON schemas closed/bounded
  mutable scope known

native input:
  command IDs/tokens unique and bounded
  reference kinds/triggers unique enough for deterministic dispatch
  display strings sanitized

capabilities:
  only supported descriptor API feature names

reject raw:
  executable locator
  native session handle
  cookies/tokens/API key/password
  unbounded native config blob
~~~

## 6. Target TypeScript client API

Replace `AgentProviderKey`, `probeCodex`, and `probeOpenCode`.

~~~ts
export interface AgentRegistryApi {
  listRegistry(input: {
    query?: string;
    cursor?: string;
    limit?: number;
  }): Promise<AgentRegistryPage>;

  refreshRegistry(): Promise<AgentRegistrySnapshot>;

  listInstallations(): Promise<AgentInstallationObservation[]>;

  installAgent(input: {
    registryAgentId: string;
    distributionId: string;
    expectedRegistryRevision: string;
    requestId: string;
  }): Promise<AgentLifecycleOperation>;

  updateAgent(input: {
    registryAgentId: string;
    expectedInstalledVersion: string;
    targetVersion?: string | null;
    requestId: string;
  }): Promise<AgentLifecycleOperation>;

  refreshControlDescriptor(
    agentProfileId: string
  ): Promise<AgentControlDescriptor>;

  getControlDescriptor(
    agentProfileId: string
  ): Promise<AgentControlDescriptor>;

  beginAuth(input: {
    agentProfileId: string;
    authMethodId: string;
  }): Promise<AgentAuthChallenge>;

  submitCredential(input: {
    agentProfileId: string;
    credentialSlotId: string;
    descriptorDigest: string;
    secret: Uint8Array;
  }): Promise<AgentCredentialBinding>;

  openNativeConfig(input: {
    agentProfileId: string;
    targetId: string;
  }): Promise<NativeConfigOpenReceipt>;

  configureBinding(input: {
    agentBindingId: string;
    expectedVersion: number;
    requestId: string;
    descriptorDigest: string;
    configuration: Record<string, unknown>;
    credentialBindings: AgentCredentialBinding[];
    defaultSessionOptions: Record<string, unknown>;
  }): Promise<AgentBinding>;
}
~~~

The WebView-facing secret method must not use a reusable JavaScript query cache or persist
the secret in component/application state longer than the immediate controlled input
lifetime. Prefer a native Tauri command whose Rust side writes directly to the local
Operator/SecretStore path and returns only configured status.

## 7. Tauri refactor

Current:
~~~text
probe_agent_profile(workspace_id, provider_key)
  if CODEX -> ...
  else if OPENCODE -> ...
~~~

Target:
~~~text
list_agent_registry(...)
refresh_agent_registry()
install_registry_agent(...)
update_registry_agent(...)
refresh_agent_control_descriptor(agent_profile_id)
begin_agent_auth(agent_profile_id, auth_method_id)
submit_agent_credential_native(agent_profile_id, slot_id, secret_bytes)
open_agent_native_config(agent_profile_id, target_id)
configure_agent_binding(...)
~~~

Tauri validates only transport/local UI boundaries and forwards typed operations. It does
not contain per-agent brand branching.

## 8. UI refactor sequence

1. Introduce generated/typed Agent Registry/control API types without changing the current
   user-facing page.
2. Add `AgentRegistryPage` behind the existing Settings Agent entry.
3. Move current inventory/probe display into adapter-backed generic projections.
4. Remove `AgentProviderKey` and explicit probe buttons only after generic routes work.
5. Add install/update/auth/native-config cards.
6. Add binding configuration/default options.
7. Introduce `useComposerAgentSurface`.
8. Replace combined/static agent/model UI with dependent controls.
9. Add slash/reference semantic token palettes.
10. Add setup-drift draft preservation.
11. Remove old Codex/OpenCode-specific UI/Tauri branches after migration tests prove no
    regression.

At every step, current working native functionality must remain reachable.

## 9. Storage implementation target

Add storage-core ports for:

~~~rust
trait AgentLifecycleStore {
    fn get_registry_cache(...);
    fn replace_registry_cache(...);
    fn upsert_installation_observation(...);
    fn begin_lifecycle_operation(...);
    fn settle_lifecycle_operation(...);
    fn put_control_descriptor(...);
    fn get_fresh_control_descriptor(...);
}

trait AgentBindingStore {
    fn configure_agent_binding(
        &self,
        commit: ConfigureAgentBindingCommit,
    ) -> Result<AgentBindingRecord, StoreError>;
}
~~~

`configure_agent_binding` must atomically:
- compare AgentBinding aggregate version;
- compare/idempotently claim request ID;
- persist non-secret config/default option digests;
- persist credential slot + SecretRef bindings only;
- increment aggregate version;
- append `agent.binding.configuration.changed.v1`;
- update projection/snapshot;
- commit idempotency receipt.

No database trigger can replace adapter-schema validation.

## 10. Current code deletion gates

Do **not** delete a current path until its replacement has CODE + SYSTEM evidence.

| Old path | Delete after |
|---|---|
| `AgentProviderKey` union | generic registry/control UI uses no brand branch |
| `probeCodex()` / `probeOpenCode()` | generic descriptor refresh works for both |
| Tauri `provider_key` branch | generic native commands/routes are tested |
| daemon `probe_local_agent_profile` provider branch | AgentModuleRegistry routes both |
| direct Codex/OpenCode inventory fanout | module discovery reproduces bounded inventory |
| empty-only AgentBinding config rule | descriptor/config storage tests pass |
| transitional `auth_ref` | credential-slot migration and compatibility complete |

## 11. Research still required

Before E03-S06–S11 acceptance:

- exact current ACP Registry distribution schema/cache/integrity contract;
- exact ACP auth method discovery/challenge semantics;
- Codex native install/update/sign-in/model/options/input/command/reference surfaces;
- Claude Agent equivalent;
- OpenCode equivalent, including native provider/model configuration;
- Gemini CLI/Cline only when chosen for launch support;
- OS keychain secret-slot behavior on supported Linux/Windows/macOS;
- upstream update drift detection and supported version policy;
- LiteSPM capability bridge compatibility matrix.

Unknown remains UNKNOWN; mock documentation is not provider qualification.

## 12. Testing order

~~~text
CODE
  module/descriptor/schema/property tests
  storage/idempotency/event tests
  UI reducer/hook/accessibility tests
  no-secret/log tests

SYSTEM
  real registry fetch/cache
  install/update/interrupt/recovery
  real native auth/API-key path
  external config drift
  actual native model/options/slash/@/attachments
  real LiteSPM capability through selected agent

USER
  install -> configure -> chat
  switch agent without losing draft
  compare native harness vs LiteCowork
  update agent -> refresh descriptor
  use one LiteSPM capability through two compatible agents
~~~

No agent receives a supported label until its versioned conformance pack passes.
