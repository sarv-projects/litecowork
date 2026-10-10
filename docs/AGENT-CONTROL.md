# Agent Registry, Configuration, Authentication, and Native Surface LLD

**Status:** vNext target contract.  
**Owner:** Agent Fabric.  
**Related:** [Architecture](../ARCHITECTURE.md), [Agent Fabric](AGENT-FABRIC.md),
[Delegation](DELEGATION.md), [Services](SERVICES.md), [API](API.md),
[Schemas](SCHEMAS.md), [Security](SECURITY.md), [Trust](TRUST.md),
[Experience](EXPERIENCE.md), [LiteSPM capability boundary](CAPABILITY-FABRIC.md).

## 1. Purpose

LiteCowork is built **around external agents, not over a reduced common denominator**.
A user who can install, update, sign in to, configure, select models/options for, invoke
native commands in, reference native resources from, or use native features of an agent
outside LiteCowork must not lose those capabilities merely because the agent is used
through LiteCowork.

The abstraction boundary is therefore:

```text
                         ACP Registry / supported distribution metadata
                                      |
                                      v
Operator Agent Registry -> AgentRegistryService
                                      |
                                      v
                              AgentModuleRegistry
                         / lifecycle       \ runtime
                        v                   v
            AgentLifecycleAdapter      AgentAdapter
          install/update/auth/config   session/input/events
                        \                   /
                         \                 /
                          v               v
                         AgentControlDescriptor
                                  |
                 +----------------+----------------+
                 |                                 |
                 v                                 v
          AgentBindingService              Conversation / Task
      durable Workspace defaults       pins exact option/config digests
                 |
                 v
      AgentBinding + SecretRefs

LiteSPM --------------------------------------------------------------+
  package discovery/install/update/supervision for connectors/MCP/    |
  plugins/skills/capabilities and qualified cross-agent bridges       |
                                                                      |
LiteCowork Capability Fabric <----------------------------------------+
  scoped CapabilityRefs/Grants/Activations/Invocations for actual use
```

Core must never branch on agent brand names. Brand/version-specific behavior belongs in
the agent module.

## 2. Non-negotiable invariants

1. **No global provider/model layer.** LiteCowork has no universal Providers screen,
   universal model catalog, or cost/model router. Provider accounts and provider-specific
   models are native-agent concerns unless a particular AgentModule explicitly exposes a
   secure LiteCowork-managed credential slot.
2. **Native harness integrity is product functionality.** Native configuration, prompts,
   slash commands, mention/reference semantics, skills, hooks, plugins, MCP settings,
   native subagents, memory, provider configuration, and agent-specific controls are not
   silently removed, renamed, copied, or rewritten.
3. **Registry listing is not readiness.** Being present in the ACP Registry means only
   that a distribution is discoverable. It does not prove installation, authentication,
   entitlement, model availability, session support, lead eligibility, or safe execution.
4. **Install state, auth state, compatibility, and session readiness are independent.**
   UI and admission must never collapse them into one “connected” boolean.
5. **Every configuration field has an owner.** It is one of:
   `NATIVE_AGENT`, `LITECOWORK_NON_SECRET`, `LITECOWORK_SECRET_REF`, or
   `LITESPM_CAPABILITY`. Unknown ownership is not configurable.
6. **Secret bytes never enter ordinary configuration JSON.** They are either handled by
   the native agent's own flow or written through the privileged local SecretStore path.
   Domain events, logs, AgentControlDescriptors, AgentBindings, TaskPackets, backups, and
   ordinary Artifacts never contain the secret bytes.
7. **Descriptors are time-bounded observations.** Models, reasoning values, auth methods,
   native commands, input surfaces, and extension support may change after an agent
   update or native configuration change. New work revalidates descriptor freshness.
8. **No silent option substitution.** If a selected model/reasoning/session option is no
   longer supported, new admission fails with a typed setup/configuration error and keeps
   the user draft/work intact.
9. **User-visible native affordances are adapter-driven.** The composer may show Agent,
   Model, Reasoning/session options, `/`, `@`, file/image/resource input, or other
   controls only when the selected adapter currently reports them.
10. **LiteSPM owns package lifecycle, LiteCowork owns scoped use.** LiteSPM can make one
    qualified connector/MCP/plugin/skill/capability available to many agents through
    adapter-supported bridges. LiteCowork still rechecks Task/Conversation authority,
    Grants, Effects, Evidence, and activation scope.
11. **Native extensions remain honest.** If an agent also has native-only MCP/plugins or
    hooks outside LiteSPM mediation, LiteCowork may display them as native harness state
    when safely observable, but must not claim LiteSPM/Core authorization or Effects for
    their actions.
12. **Agent updates are modular.** Upstream changes are handled by updating one
    AgentModule and its conformance fixtures. A Codex/OpenCode/Claude change must not
    require Core conditionals or unrelated agent rewrites.

## 3. Ownership matrix

| Concern | Owner | Durable? | Secret-bearing? |
|---|---|---:|---:|
| ACP Registry/distribution metadata | AgentRegistryService cache | No authority; cache only | No |
| Local installed version | AgentLifecycleAdapter observation | Runtime-local | No |
| Update availability | AgentLifecycleAdapter observation | Runtime-local | No |
| Native sign-in/account state | Native agent + AgentLifecycleAdapter observation | Runtime-local observation | Native store may contain secrets |
| Secure API-key slot explicitly supported by adapter | SecretStore + AgentBinding SecretRef | SecretRef durable; bytes local/private | Yes, bytes only in SecretStore |
| Native provider configuration | Native agent | Native-owned | Possibly |
| Non-secret adapter configuration | AgentBinding | Yes | No |
| Model/reasoning/session defaults | AgentBinding defaults + AgentSession pin | Yes/provenance | No |
| Current model/option catalog | AgentControlDescriptor | Time-bounded | No |
| Native slash commands / @ references | AgentControlDescriptor | Time-bounded display/input contract | No |
| Agent native tools/subagents/hooks | Native harness | Agent-owned | May reference native secrets |
| Cross-agent package lifecycle | LiteSPM | LiteSPM-owned | LiteSPM/SecretStore policy |
| Task/Conversation authorization for package use | LiteCowork Capability Fabric | Yes | SecretRefs only |
| AgentSession | Agent Fabric | Yes, normalized provenance | Native handles stay Runtime-local |

## 4. Module architecture

### 4.1 AgentModule

Every supported agent is one independently versioned module.

```text
interface AgentModule {
  manifest() -> AgentModuleManifest
  lifecycle() -> AgentLifecycleAdapter
  runtime() -> AgentAdapter
  capability_bridge() -> AgentCapabilityBridge?
}
```

`AgentModuleManifest`:

```text
AgentModuleManifest {
  module_id: string
  module_version: SemVer
  descriptor_api_version: u32

  registry_agent_ids: string[]      # ACP registry IDs this module recognizes
  display_name: string
  distribution_kinds: AgentDistributionKind[]
  supported_os: OperatingSystem[]

  lifecycle_features: {
    discover_installation: bool
    install: bool
    update: bool
    uninstall: bool
    native_auth: bool
    secure_credential_slots: bool
    open_native_config: bool
  }
}
```

The manifest is compiled/trusted LiteCowork code or a separately verified extension under
the future module policy. Untrusted ACP Registry metadata cannot invent executable
commands, auth callbacks, secret slots, or privileged setup actions.

### 4.2 AgentLifecycleAdapter

```text
interface AgentLifecycleAdapter {
  discover_installation(RuntimeContext) -> AgentInstallationObservation

  install(
    RuntimeContext,
    RegistryDistributionRef,
    InstallRequest
  ) -> AgentLifecycleOperation

  check_update(
    RuntimeContext,
    AgentInstallationObservation
  ) -> AgentUpdateObservation

  update(
    RuntimeContext,
    UpdateRequest
  ) -> AgentLifecycleOperation

  probe_control_descriptor(
    WorkspaceContext,
    AgentInstallationObservation
  ) -> AgentControlDescriptor

  begin_auth(
    WorkspaceContext,
    AgentControlDescriptor,
    AuthMethodId
  ) -> AgentAuthChallenge

  submit_credential(
    WorkspaceContext,
    AgentControlDescriptor,
    CredentialSlotId,
    SecretWriteHandle
  ) -> AgentAuthObservation

  refresh_auth(
    WorkspaceContext,
    AgentControlDescriptor
  ) -> AgentAuthObservation

  open_native_config(
    RuntimeContext,
    NativeConfigTargetId
  ) -> NativeConfigOpenReceipt
}
```

Lifecycle methods do not start a Conversation, Task, Attempt, model inference, or native
session. Installation/update operations are explicit owner actions.

### 4.3 Runtime AgentAdapter

The existing `AgentAdapter` remains the session/protocol boundary. It gains only
descriptor-driven input methods required to preserve the native surface:

```text
interface AgentAdapter {
  discover() -> AgentProfile
  probe() -> AgentCapabilities

  start_session(SessionSpec) -> SessionHandle
  resume_session(ResumeSessionSpec) -> SessionHandle

  send(SessionHandle, AgentInput) -> SendAck
  steer(SessionHandle, AgentInput) -> SendAck
  interrupt(SessionHandle) -> Ack
  cancel(SessionHandle) -> Ack

  attach_capabilities(SessionHandle, CapabilityAttachment[]) -> AttachmentResult
  attach_context(SessionHandle, ContextAttachment[]) -> AttachmentResult

  stream_events(SessionHandle, Cursor?) -> Stream<NormalizedAgentEvent>
  snapshot_session(SessionHandle, Cursor?) -> AgentSessionSnapshot?
  close(SessionHandle) -> Ack
}
```

`AgentInput` is not text-only:

```text
AgentInput {
  text?: string
  images?: PinnedResourceRef[]
  files?: PinnedResourceRef[]
  resources?: PinnedResourceRef[]

  native_command?: NativeCommandInvocation
  native_references?: NativeReferenceSelection[]

  # Adapter-owned option override validated against the current descriptor.
  session_options?: JsonObject
  descriptor_digest: Sha256Digest
}
```

If an agent cannot represent one member, the adapter reports it unsupported before send.

## 5. Registry and distribution contracts

### 5.1 AgentRegistryEntry

```text
AgentRegistryEntry {
  registry_agent_id: string
  display_name: string
  description?: string
  homepage?: Uri
  source_registry: string
  source_revision?: string

  distributions: AgentDistribution[]
  metadata_observed_at: Timestamp
  cache_expires_at: Timestamp?
}
```

`AgentDistribution` is descriptive only:

```text
AgentDistribution {
  distribution_id: string
  kind: NPM | PYPI | BINARY | HOMEBREW | WINGET | SCOOP | CARGO | MANUAL | OTHER
  package_ref: string
  version_constraint?: string
  os: OperatingSystem[]
  architecture?: CpuArchitecture[]
}
```

Registry metadata never carries a shell command that LiteCowork executes blindly. The
selected AgentModule translates a recognized distribution into a bounded install plan.

### 5.2 AgentInstallationObservation

```text
AgentInstallationObservation {
  registry_agent_id: string
  module_id: string
  runtime_id: RuntimeId
  runtime_incarnation_id: RuntimeIncarnationId

  state: MISSING | INSTALLING | INSTALLED | UPDATE_AVAILABLE |
         UPDATING | VERSION_UNAVAILABLE | BROKEN
  installed_version?: string
  available_version?: string
  installation_source?: string

  observed_at: Timestamp
  expires_at: Timestamp
}
```

This is operational Runtime truth. It is not an AgentProfile and creates no binding.

## 6. AgentControlDescriptor

```text
AgentControlDescriptor {
  descriptor_digest: Sha256Digest
  descriptor_api_version: u32

  registry_agent_id: string
  module_id: string
  module_version: string
  harness_version?: string
  protocol_version?: string

  installation: AgentInstallationObservation
  auth: AgentAuthDescriptor
  configuration: AgentConfigurationDescriptor
  session_options?: AgentSessionOptionDescriptor
  input_surface: AgentInputSurfaceDescriptor
  native_surface: AgentNativeSurfaceDescriptor
  capabilities: AgentCapabilities

  observed_at: Timestamp
  expires_at: Timestamp
}
```

The digest covers the normalized non-secret descriptor. The descriptor is bounded and
closed; raw provider/native config objects are never copied into it.

### 6.1 Authentication descriptors

```text
AgentAuthDescriptor {
  state: UNKNOWN | NEEDS_AUTH | AUTHENTICATING | AUTHENTICATED |
         EXPIRED | DEGRADED | ERROR
  account_label?: string                 # sanitized display only
  methods: AgentAuthMethodDescriptor[]
}

AgentAuthMethodDescriptor {
  auth_method_id: string
  display_name: string
  kind: NATIVE_FLOW | EXTERNAL_HANDOFF | DEVICE_CODE |
        SECRET_SLOT | NATIVE_CONFIG_ONLY
  credential_slot_id?: string
  requires_restart: bool
}

AgentCredentialSlotDescriptor {
  credential_slot_id: string
  display_name: string
  secret_kind: API_KEY | TOKEN | PASSWORD | OTHER
  storage_owner: NATIVE_AGENT | LITECOWORK_SECRET_STORE
  required: bool
}
```

A provider API key is therefore possible without creating a Providers screen:

- if the agent already owns provider configuration, `storage_owner=NATIVE_AGENT` and
  LiteCowork opens the native flow/config;
- if the adapter explicitly supports secure injection, the secret is written to
  SecretStore and only its `SecretRef` is attached to the binding slot.

### 6.2 Non-secret configuration

```text
AgentConfigurationDescriptor {
  schema_digest: Sha256Digest
  fields: AgentConfigurationField[]
  native_config_targets: NativeConfigTarget[]
}

AgentConfigurationField {
  field_id: string
  display_name: string
  type: BOOLEAN | ENUM | STRING | INTEGER | PATH | STRING_LIST
  required: bool
  default_value?: JsonValue
  constraints: JsonObject
  ownership: LITECOWORK_NON_SECRET
}

NativeConfigTarget {
  target_id: string
  display_name: string
  kind: FILE | DIRECTORY | NATIVE_SETTINGS | EXTERNAL_UI
}
```

No secret field may appear in `AgentConfigurationField`.

### 6.3 Session options

```text
AgentSessionOptionDescriptor {
  descriptor_digest: Sha256Digest
  options: AgentSessionOption[]
}

AgentSessionOption {
  option_id: string
  display_name: string
  semantic_hint?: MODEL | REASONING | EFFORT | MODE | OTHER
  value_schema: JsonSchema
  mutable_scope: LIVE_SESSION | NEW_SESSION | UNSUPPORTED
}
```

The UI may give special visual treatment to `MODEL` or `REASONING`, but the stored
value remains an opaque adapter-owned option. Agents may expose zero, one, or many options.

### 6.4 Native input surface

```text
AgentInputSurfaceDescriptor {
  text: bool
  images: bool
  files: bool
  resources: bool

  slash_commands: NativeCommandDescriptor[]
  reference_kinds: NativeReferenceKindDescriptor[]

  supports_command_discovery: bool
  supports_reference_discovery: bool
}

NativeCommandDescriptor {
  command_id: string
  token: string             # e.g. "/review"
  display_name: string
  description?: string
  argument_schema?: JsonSchema
}

NativeReferenceKindDescriptor {
  reference_kind_id: string
  trigger: string           # commonly "@", but not assumed
  display_name: string
  description?: string
  selection_mode: NATIVE_PICKER | HOST_RESOURCE_PICKER | TEXT_COMPLETION
}
```

Command/reference descriptors are sanitized display/input metadata. They cannot grant
authority or cause command execution until the user/agent actually submits an input.

## 7. AgentBinding target shape

The target AgentBinding remains the durable Workspace authorization/default object, but
its configuration is no longer an untyped empty placeholder.

```text
AgentBinding {
  agent_binding_id
  workspace_id
  agent_profile_id
  runtime_id?
  endpoint_selection_policy

  configuration_descriptor_digest?
  configuration: JsonObject                 # non-secret only
  credential_bindings: AgentCredentialBinding[]
  default_session_options_descriptor_digest?
  default_session_options: JsonObject

  enabled
  lead_eligible
  created_at
  version
}

AgentCredentialBinding {
  credential_slot_id: string
  secret_ref: SecretRef
}
```

Rules:

- configuration keys/values MUST validate against the current
  `AgentConfigurationDescriptor`;
- `credential_slot_id` MUST exist in the descriptor and have
  `storage_owner=LITECOWORK_SECRET_STORE`;
- native-owned credentials are never represented as SecretRefs;
- default session options MUST validate against the pinned session-option descriptor;
- updates increment AgentBinding.version and emit one configuration/defaults event;
- active AgentSessions remain pinned to the option/configuration digest admitted at start;
- new sessions revalidate descriptor freshness and compatibility.

## 8. State machines

### 8.1 Installation

```text
MISSING
  -- explicit Install --> INSTALLING
INSTALLING
  -- success --> INSTALLED
  -- failure --> BROKEN
INSTALLED
  -- newer supported distribution observed --> UPDATE_AVAILABLE
UPDATE_AVAILABLE
  -- explicit Update --> UPDATING
UPDATING
  -- success --> INSTALLED
  -- failure with old version intact --> UPDATE_AVAILABLE
  -- corrupt/unusable --> BROKEN
BROKEN
  -- Repair/Reinstall --> INSTALLING
```

Install/update never silently occurs because a chat selected an unavailable agent.

### 8.2 Authentication

```text
UNKNOWN
  -- probe --> NEEDS_AUTH | AUTHENTICATED | DEGRADED | ERROR
NEEDS_AUTH
  -- explicit method --> AUTHENTICATING
AUTHENTICATING
  -- native/adapter observation --> AUTHENTICATED
  -- cancel --> NEEDS_AUTH
  -- failure --> ERROR
AUTHENTICATED
  -- expiry/revocation/config change --> EXPIRED | NEEDS_AUTH | DEGRADED
EXPIRED
  -- reauth --> AUTHENTICATING
```

Auth state is operational observation; credentials are not replicated domain state.

### 8.3 Descriptor freshness

```text
ABSENT -> FRESH -> STALE -> REFRESHING -> FRESH
                         \-> FAILED
```

New Conversation/Task admission requires a fresh compatible descriptor when selected
options/configuration depend on it. Existing sessions keep their pinned descriptor digest.

## 9. Services

### 9.1 AgentRegistryService

```text
refresh_registry(force: bool) -> AgentRegistrySnapshot
list_registry(query, cursor?, limit) -> Page<AgentRegistryEntry>
get_registry_entry(registry_agent_id) -> AgentRegistryEntry
```

Pseudocode:

```text
function refresh_registry(force):
    if !force and cache.is_fresh():
        return cache.snapshot

    payload = registry_client.fetch_bounded()
    verified = validate_registry_schema_and_size(payload)

    for entry in verified.entries:
        module = AgentModuleRegistry.match(entry.registry_agent_id)
        project only safe descriptive distribution metadata
        annotate module support as SUPPORTED | NO_ADAPTER | UNSUPPORTED_OS

    atomically replace local cache
    return snapshot
```

Registry fetch failure leaves the last non-expired cached snapshot visible with a stale
banner; it never changes installed/binding/auth state.

### 9.2 AgentModuleRegistry

```text
register(module: AgentModule)
match(registry_agent_id) -> AgentModule?
get(module_id) -> AgentModule
list_supported() -> AgentModuleManifest[]
```

Pseudocode:

```text
function match(registry_agent_id):
    candidates = modules where registry_agent_id in manifest.registry_agent_ids
    require exactly one compatible highest descriptor_api_version
    if ambiguous:
        return error AGENT_UNAVAILABLE
    return candidate?
```

### 9.3 AgentLifecycleService

```text
list_installations(RuntimeId) -> AgentInstallationObservation[]
install(RegistryAgentId, DistributionId, expected_registry_revision, RequestId)
  -> AgentLifecycleOperation
check_update(RegistryAgentId) -> AgentUpdateObservation
update(RegistryAgentId, expected_installation_version, RequestId)
  -> AgentLifecycleOperation
refresh_control_descriptor(WorkspaceId, RegistryAgentId)
  -> AgentControlDescriptor
begin_auth(WorkspaceId, RegistryAgentId, AuthMethodId)
  -> AgentAuthChallenge
submit_secret(WorkspaceId, RegistryAgentId, CredentialSlotId, SecretInput)
  -> AgentAuthObservation
open_native_config(RegistryAgentId, NativeConfigTargetId)
  -> NativeConfigOpenReceipt
```

Pseudocode for install/update:

```text
function install(agent_id, distribution_id, expected_registry_revision, request_id):
    require_local_owner()
    entry = AgentRegistryService.get(agent_id)
    require entry.revision == expected_registry_revision

    module = AgentModuleRegistry.match(agent_id)
      or fail AGENT_UNAVAILABLE

    distribution = exact entry.distributions[distribution_id]
    plan = module.lifecycle().compile_install_plan(distribution)

    require plan contains only module-approved bounded operations
    operation = lifecycle_store.begin_idempotent(request_id, digest(plan))

    result = module.lifecycle().install(runtime_context, distribution, operation)
    observe actual installed executable/version
    persist local operation result + audit metadata
    never create AgentProfile/Binding automatically
    return result
```

Pseudocode for descriptor refresh:

```text
function refresh_control_descriptor(workspace_id, agent_id):
    require_workspace_owner(workspace_id)
    require_active_runtime_workspace_binding(EXECUTOR)

    installation = lifecycle.discover_installation()
    require installation.state in {INSTALLED, UPDATE_AVAILABLE}

    descriptor = module.lifecycle().probe_control_descriptor(
        workspace_context,
        installation
    )

    validate descriptor:
      closed schema and size bounds
      no secret-shaped fields or raw native config
      all SECRET_SLOT methods reference declared credential slots
      all LITECOWORK_SECRET_STORE slots use approved secret kinds
      session option schemas are closed and bounded
      native command/reference tokens are bounded/sanitized
      descriptor API version supported

    store Runtime-local descriptor with expiry
    publish normalized AgentProfile/RuntimeOffer observation if applicable
    return descriptor
```

### 9.4 AgentBindingService additions

```text
get_control_descriptor(AgentBindingId) -> AgentControlDescriptor
configure_binding(
  AgentBindingId,
  expected_version,
  descriptor_digest,
  non_secret_configuration,
  credential_bindings,
  default_session_options,
  RequestId
) -> AgentBinding
```

Pseudocode:

```text
function configure_binding(request):
    binding = require_same_workspace_binding(request.id)
    descriptor = require_fresh_descriptor(binding.agent_profile_id)

    require descriptor.digest == request.descriptor_digest
    validate_json(request.configuration, descriptor.configuration.fields)

    for credential in request.credential_bindings:
        slot = descriptor.auth.credential_slots[credential.slot_id]
        require slot.storage_owner == LITECOWORK_SECRET_STORE
        require SecretStore.exists_and_owned(credential.secret_ref)

    validate_session_options(
        request.default_session_options,
        descriptor.session_options
    )

    canonical = normalize_non_secret_binding_config(request)
    in one transaction:
        compare binding.version == expected_version
        update configuration/defaults/credential SecretRefs
        binding.version += 1
        append agent.binding.configuration.changed.v1
        write idempotency receipt
    return projection
```

### 9.5 Secret submission

```text
function submit_secret(workspace_id, agent_id, slot_id, secret_bytes):
    require_local_authenticated_owner()
    descriptor = require_fresh_descriptor(agent_id)
    slot = descriptor.auth.credential_slots[slot_id]
    require slot.storage_owner == LITECOWORK_SECRET_STORE

    secret_ref = SecretStore.write(
        scope=workspace_id,
        kind=slot.secret_kind,
        bytes=secret_bytes
    )

    zeroize request buffer
    audit only slot_id + secret_ref metadata, never secret bytes
    return secret_ref
```

The ordinary Operator JSON API MUST NOT echo the secret. Desktop IPC should use a
no-log/no-cache privileged command body with strict size limits.

### 9.6 Composer projection

```text
resolve_composer_surface(
  workspace_id,
  selected_agent_binding_id?,
  coworker_revision?
) -> ComposerAgentSurface
```

Pseudocode:

```text
function resolve_composer_surface(...):
    binding = explicit selection
           ?? coworker default
           ?? workspace default
           ?? NONE

    if binding == NONE:
        return NO_AGENT_CONFIGURED

    descriptor = get_or_refresh_control_descriptor(binding)
    if auth not AUTHENTICATED and adapter requires auth:
        return SETUP_REQUIRED preserving draft

    options = merge(
      binding.default_session_options,
      conversation explicit overrides
    )
    validate options against descriptor

    return {
      agent picker entry,
      model option if semantic_hint MODEL exists,
      reasoning/effort options if exposed,
      additional agent options in overflow,
      native input flags,
      slash commands,
      reference kinds,
      attachment/resource accept types
    }
```

Typing `/` or a declared reference trigger queries this already-bounded descriptor.
LiteCowork does not reinterpret a native command as a LiteCowork command unless the UI
visibly distinguishes the two namespaces.

### 9.7 Conversation admission

```text
function admit_conversation_turn(draft, surface_selection):
    binding = resolve_selected_binding(surface_selection.agent)
    descriptor = require_fresh_control_descriptor(binding)

    validate:
      binding enabled + lead_eligible
      endpoint currently eligible
      auth state acceptable
      explicit session options supported
      selected model/value still present when descriptor uses enum/catalog
      draft attachments compatible with input_surface
      native command/reference invocation valid for descriptor
      Trust/Workspace read scope

    persist semantic user message + durable ConversationTurn
    start fresh AgentSession with:
      binding id
      endpoint id
      descriptor digest
      normalized configuration digest
      normalized session-option digest
      input/resource refs
    on setup drift:
      fail with AGENT_NATIVE_CONFIG_CHANGED
      do not lose draft
```

## 10. Native command and reference namespaces

LiteCowork has its own host actions (attach Resource, create Task, schedule responsibility,
open Workbench). Native agents may also expose `/`, `@`, commands, references, modes,
or provider-specific input controls.

Rules:

- native tokens remain labeled with the selected agent;
- LiteCowork host actions use a visually separate namespace/menu;
- collisions are never silently resolved;
- changing agents recomputes the entire native palette;
- unknown/stale native commands fail before send and prompt descriptor refresh;
- history records the semantic input and selected descriptor digest, not private native
  autocomplete state.

## 11. LiteSPM boundary

LiteSPM is the package-system authority for cross-agent connectors, MCP servers, plugins,
skills, and related capability packages.

Target flow:

```text
User installs package in LiteSPM
  -> LiteSPM verifies/distributes/supervises exact package version
  -> LiteCowork Capability Fabric discovers qualified CapabilityOffer
  -> AgentCapabilityBridge asks selected AgentModule how that capability can be exposed:
       DIRECT_NATIVE_ATTACHMENT
       LITECOWORK_GATEWAY
       HOST_CONTEXT_ONLY
       UNSUPPORTED
  -> LiteCowork authorizes scoped use for Conversation/Task
  -> Agent receives only the qualified attachment form
```

The same LiteSPM package can therefore be usable from multiple agents without the user
manually installing a different copy into every harness, **when** each AgentModule has a
qualified bridge for that package/capability class.

Pseudocode:

```text
function resolve_capability_attachment(agent_descriptor, capability_offer, grant):
    require grant current
    bridge = AgentModuleRegistry
      .get(agent_descriptor.module_id)
      .capability_bridge()

    route = bridge.select_route(
      agent_descriptor,
      capability_offer
    )

    match route:
      DIRECT_NATIVE_ATTACHMENT:
        require provider can enforce the same scoped grant/effect/fencing contract
        return native attachment
      LITECOWORK_GATEWAY:
        return stable Gateway capability reference
      HOST_CONTEXT_ONLY:
        return read-only bounded context attachment
      UNSUPPORTED:
        return explicit incompatibility
```

Native-only plugins/MCP/hooks that the user already configured remain native harness
state. LiteSPM does not silently delete or rewrite them. The UI distinguishes
`Managed by LiteSPM` from `Native to <Agent>`.

## 12. Operator API target

The machine OpenAPI owns exact wire shapes. Required product operations:

```text
GET    /agent-registry
POST   /agent-registry/refresh

GET    /agent-installations
POST   /agent-registry/{registryAgentId}/install
POST   /agent-registry/{registryAgentId}/update

GET    /agent-profiles
POST   /agent-profiles/{agentProfileId}/refresh
GET    /agent-profiles/{agentProfileId}/control-descriptor

POST   /agent-profiles/{agentProfileId}/auth/{authMethodId}/begin
POST   /agent-profiles/{agentProfileId}/credentials/{credentialSlotId}
POST   /agent-profiles/{agentProfileId}/native-config/{targetId}/open

GET    /agent-bindings
POST   /agent-bindings
GET    /agent-bindings/{agentBindingId}
PUT    /agent-bindings/{agentBindingId}/configuration
POST   /agent-bindings/{agentBindingId}/enable
POST   /agent-bindings/{agentBindingId}/disable
```

Install/update/auth/open-native-config are authenticated local-Operator operations. A
future remote Operator must not receive these privileges merely by having Workspace read
access.

## 13. Events and persistence

### Replicated domain event

`agent.binding.configuration.changed.v1` is emitted when the durable Workspace binding
changes non-secret configuration, credential SecretRefs, or default session options.

Payload contains:

```text
workspace_id
agent_binding_id
aggregate_version
descriptor_digest
configuration_digest
credential_slot_ids[]           # IDs only, never SecretRef target metadata or bytes
default_session_options_digest
requested_by
changed_at
```

Installation/update/auth observations are Runtime-local operational state and do not
become Workspace domain events. Security-relevant owner actions still create AuditRecords.

### Runtime-local storage

Target local operational tables/projections:

- `agent_registry_cache`
- `agent_installation_observations`
- `agent_control_descriptors`
- `agent_lifecycle_operations`

Durable binding configuration remains under the AgentBinding aggregate. Secret bytes stay
in SecretStore.

## 14. UI contract

### 14.1 Chat composer

Default visible controls:

```text
[ + ] [ Agent ] [ Model? ] [ Reasoning/agent options? ] [ Native / @ ... ] [ Send ]
```

- Agent is always explicit when more than one lead-eligible binding exists.
- Model appears only when the selected agent descriptor exposes a MODEL semantic option.
- Reasoning/Effort appears only when exposed.
- Additional options live in an agent-options popover; LiteCowork does not discard them.
- `/` and `@` autocomplete use the selected agent's current descriptor.
- Changing Agent immediately invalidates/rebuilds dependent Model/Reasoning/native
  controls. Unsupported draft attachments remain visible and block Send with an explicit
  explanation; they are not silently dropped.

### 14.2 Settings > Agent Registry

Left pane:

- live/cached ACP Registry list;
- search/filter;
- Supported / Installed / Setup needed / Update available states;
- no readiness claim from registry listing alone.

Right pane per selected agent:

1. identity and adapter module/version;
2. installation + official distribution/source;
3. update status;
4. native sign-in/account state;
5. API-key/credential slots when that adapter genuinely supports them;
6. native config targets;
7. model/session option descriptor;
8. native input surface (`/`, `@`, text/image/files/resources);
9. native extension state summary;
10. negotiated capabilities;
11. Workspace bindings/default lead;
12. diagnostics/probe timestamps;
13. adapter compatibility/update information.

If not installed, the primary action is **Install**.
If installed but unauthenticated, it is **Sign in / Configure**.
If ready, it is **Use / Set as default** or **Update** when a newer supported version is
available.

There is no separate global Providers page.

### 14.3 Local models

Settings > Local models is inventory only. A local endpoint/model appears in an Agent
Model picker only when that agent adapter exposes it as a valid native/session option.

## 15. Failure and recovery

| Failure | Required behavior |
|---|---|
| Registry offline | Use bounded cached metadata with stale label; installed agents remain usable |
| Installer interrupted | Lifecycle operation records UNKNOWN/FAILED; re-probe actual install before retry |
| Agent updated outside LiteCowork | Version/config digest drift invalidates descriptor; refresh before new admission |
| Native auth expires | New admission blocks with setup-needed; active work follows adapter/provider settlement rules |
| API key rejected | Secret stays private; auth state becomes ERROR/NEEDS_AUTH; do not echo key |
| Model removed | New session override rejected; draft preserved |
| Native command removed | Refresh descriptor; do not send stale command as plain text unless user explicitly chooses |
| Descriptor expires mid-session | Existing session remains pinned; new session/continuation revalidates |
| LiteSPM package unavailable | Capability attachment fails explicitly; native unrelated harness functions remain usable |
| Adapter/module too old | Show “Adapter update required”; never fake unsupported features |
| ACP Registry has agent but no module | Show “Adapter not yet supported”; do not execute registry-provided arbitrary install/auth logic |

## 16. Security requirements

- Registry descriptions, package names, URLs, commands, model labels, native command
  descriptions, and provider output are untrusted input.
- Agent modules define bounded executable/install/auth actions; registry metadata does not.
- Secret submission endpoints are no-store, no-log, size-bounded, local-owner only.
- Account labels must be sanitized and optional.
- Native config paths/locators are Runtime-private and excluded from replication/backups.
- `open_native_config` resolves a module-known target ID; the caller cannot provide an
  arbitrary filesystem path or shell command.
- Update/install subprocesses run under a dedicated lifecycle policy and cannot inherit
  arbitrary daemon environment variables.
- Agent setup never grants Workspace/Task authority by itself.
- Enabling a binding remains a separate owner authorization step.

## 17. Required research/qualification before implementation acceptance

These are named gates, not assumptions:

1. **ACP Registry distribution contract:** exact fields, update/version semantics, cache
   headers, integrity/signature guarantees, and installation metadata supported by the
   current published registry.
2. **ACP authentication methods:** exact protocol support for auth method discovery,
   browser/device-code/API-key flows, cancellation, expiry, and account observation.
3. **Per-agent adapters:** Codex, Claude Agent, OpenCode, Gemini CLI, Cline, and any
   launch-critical agent require a version matrix of install, update, auth, model/session
   options, `/`, `@`, native resources, extensions, stop/quiescence, and config drift.
4. **SecretStore UX:** OS keychain behavior on Windows/Linux, zeroization boundary,
   crash/log leakage tests, and migration/backup behavior of SecretRefs.
5. **Native command discovery:** determine which agents expose a machine-readable command
   catalogue versus requiring a native passthrough/picker.
6. **Native reference semantics:** verify how each harness represents file/resource/agent
   mentions; do not infer support from documentation screenshots.
7. **Install/update safety:** official package source verification, rollback, concurrent
   installs, partial update recovery, administrator prompts, and Windows/WSL placement.
8. **LiteSPM bridge matrix:** which package classes can be attached natively per agent,
   which require Gateway mediation, and which are unsupported.

## 18. Required CODE / SYSTEM / USER tests

### CODE

- registry schema/size/duplicate ID/pathological metadata;
- module matching ambiguity;
- install plan cannot execute untrusted registry command text;
- auth state transitions;
- secret bytes absent from events/logs/API response/config JSON;
- configuration closed-schema validation;
- credential slot ID and SecretRef ownership validation;
- descriptor expiry/drift;
- model/reasoning option dependency on selected agent;
- unknown option rejection/no silent substitution;
- native command/reference sanitization and stale selection rejection;
- changing agent clears incompatible dependent options;
- binding configure optimistic concurrency/idempotency;
- active AgentSession provenance remains pinned after binding edit;
- LiteSPM capability attachment route cannot widen Grant authority.

### SYSTEM

For every launch-critical agent/version/OS:

- install from missing state;
- update from supported older version;
- failed/interrupted install and recovery;
- native sign-in success/cancel/expiry;
- secure API-key slot if supported;
- native config remains unchanged except explicit native/user action;
- model/session option list compared with the native harness;
- `/` and `@` behavior compared with native harness;
- image/file/resource acceptance compared with native harness;
- external native configuration change invalidates descriptor;
- one LiteSPM-managed capability used through every claimed compatible agent route;
- restart/reprobe without secret or locator leakage.

### USER

Owner acceptance must demonstrate:

1. choose an uninstalled registry agent -> install -> sign in/configure -> create binding;
2. switch Agent in composer and observe Model/Reasoning/`/`/`@` controls change;
3. perform one native action that is available in the original harness and verify it was
   not lost in LiteCowork;
4. configure an API key through the appropriate native/secure path without exposing it in
   chat/logs;
5. update an agent and verify LiteCowork refreshes capabilities instead of using stale UI;
6. use one LiteSPM-managed connector/MCP/skill from two different compatible agents;
7. disable a binding and prove new work cannot silently fall back.

## 19. Implementation decomposition

The implementation plan MUST split this contract into reviewable stories:

1. AgentModuleRegistry + registry cache.
2. Generic installation observation and module matching.
3. Install/update lifecycle operation framework.
4. AgentControlDescriptor + descriptor validator.
5. Auth state/method framework + native auth handoff.
6. Secret credential-slot flow.
7. AgentBinding configuration/default-session-option schema and storage.
8. Operator APIs/Tauri bridge.
9. Agent Registry UI.
10. Composer agent/model/reasoning/native surface.
11. Native command/reference projection.
12. Descriptor-drift/session-admission enforcement.
13. LiteSPM cross-agent capability bridge qualification.
14. Per-agent conformance packs and real-provider acceptance.

No story may claim “supports agent X” merely because it appears in the ACP Registry or its
binary is installed.
