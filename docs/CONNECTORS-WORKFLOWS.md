# Connectors, Authentication, Workflow Backends, and Event Relay

**Status:** accepted target architecture, **not working connector-package support**.
LiteSPM ARCH/26 excludes first-class V1 connector packages and ARCH/29 is
a design-only runtime pending an explicit upstream decision. LiteCowork V1
may qualify concrete app connectors through other compliant providers;
installation/configuration lifecycle remains LiteSPM's where supported.
[Interop LLD](LITESPM-LITECOWORK-INTEROP-LLD.md) defines what must be negotiated.

**Status:** accepted target architecture. This document makes concrete the connector,
local-first OAuth, external workflow, and relay boundaries. It extends
[Capability Fabric](CAPABILITY-FABRIC.md), [Automation](AUTOMATION.md),
[Network Security](NETWORK-SECURITY.md), [Trust](TRUST.md), and
[Reference Implementations](REFERENCE-IMPLEMENTATIONS.md).

## 1. Frozen decisions

1. LiteCowork remains the control plane. A connector, MCP server, workflow engine,
   OAuth gateway, or integration platform never owns Task/Plan/Attempt/Effect/Evidence truth.
2. LiteSPM owns package installation/update/provenance. LiteCowork owns Workspace
   assignment, Connection references, scoped CapabilityGrants, Invocations, Effects,
   Automation occurrences, and user-visible history.
3. First-party high-value app connections SHOULD support local-first auth where the
   provider permits a public desktop client. Tokens stay in the local SecretStore by
   default; the model never sees them.
4. There is no mandatory Composio, Nango, Arcade, n8n, Activepieces, or Zapier dependency.
5. Existing deterministic workflow systems are optional `WorkflowBackendAdapter`
   providers. Their run is one bounded capability execution inside a LiteCowork Task,
   not a replacement for the Task Runtime.
6. Activepieces community Pieces are worth a compatibility spike because their core
   paths are permissively licensed at the checked revision. Compatibility remains
   behind LiteSPM qualification; no arbitrary npm package gets trusted because it calls
   itself a Piece.
7. n8n is an external/user-owned backend by default because its checked license is not
   a permissive embedding license.
8. A Coworker may have a logical durable private home and its Tasks may receive isolated
   computers/environments. LiteCowork does not allocate one permanent VM per Coworker
   merely to satisfy a UI metaphor.
9. A remote event relay may carry minimal signed event metadata. It does not need to hold
   provider OAuth refresh tokens. Actual provider content is fetched by an authorized
   connector when an eligible Runtime is available.
10. Connector assignment is not authority. Installed, connected, assigned, granted,
    activated, invoked, and effect-settled are distinct states.

## 2. Layering

```text
User / Coworker
      |
      v
Conversation / Task / Routine / Automation
      |
      +-----------------------------+
      |                             |
      v                             v
CapabilityBroker               TriggerCoordinator
      |                             |
      v                             v
ConnectorRuntime                 TriggerProvider
WorkflowBackendAdapter           EventRelayProvider
MCP / local app / API            schedule / event / poll
      |
      v
LiteSPM package/runtime boundary
      |
      +--> direct first-party connector
      +--> community ConnectorPackage
      +--> ActivepiecesPieceProvider
      +--> MCP provider
      +--> external n8n / Activepieces / Zapier adapter
      +--> optional Nango / Arcade / Composio provider
```

The model sees bounded capability descriptions and selected schemas. It does not receive a
10,000-tool dump, provider credentials, raw package manifests, or a universal workflow DSL.

## 3. ConnectorPackage contract

A `ConnectorPackage` is a LiteSPM-managed package type. LiteCowork never installs it
directly.

```text
ConnectorPackageManifest {
  package_id: PackageId
  package_version: SemVer
  publisher: PublisherIdentity
  package_digest: Sha256Digest
  license_spdx?: string
  source_url?: string

  connector_id: ConnectorId
  display_name: string
  category: string
  icon_ref?: ResourceRef

  auth_recipes: AuthRecipeDescriptor[]
  actions: ConnectorActionDescriptor[]
  triggers: ConnectorTriggerDescriptor[]
  resources: ConnectorResourceDescriptor[]

  runtime_requirements: RuntimeRequirement[]
  network_policy: NetworkPolicyDeclaration
  secret_requirements: SecretRequirement[]
  dependency_manifest: DependencyManifest
}
```

Every executable package is signature/provenance checked by LiteSPM. LiteCowork consumes a
qualified descriptor containing only bounded schemas and normalized capability metadata.

### 3.1 Auth recipes

```text
AuthRecipeKind =
  PUBLIC_OAUTH_PKCE_LOOPBACK
  | PUBLIC_OAUTH_PKCE_CUSTOM_URI
  | DEVICE_CODE
  | CONFIDENTIAL_OAUTH
  | API_KEY
  | SERVICE_ACCOUNT
  | NATIVE_SESSION
  | NO_AUTH

AuthRecipeDescriptor {
  recipe_id: string
  kind: AuthRecipeKind
  authorization_endpoint?: Uri
  token_endpoint?: Uri
  device_endpoint?: Uri
  client_id_source: BUILTIN_PUBLIC_ID | USER_CONFIGURED | DEPLOYMENT_CONFIGURED
  client_secret_required: bool
  pkce_required: bool
  redirect_policy: LOOPBACK_RANDOM_PORT | FIXED_HTTPS | CUSTOM_URI | NONE
  default_scopes: string[]
  optional_scopes: string[]
  scope_risk: READ | WRITE | SENSITIVE | RESTRICTED
  supports_refresh: bool
  revoke_endpoint?: Uri
  provider_constraints: string[]
}
```

A recipe describes protocol behavior. It does **not** grant scopes or authority. The user
chooses an account and requested feature set; Trust and connector policy derive the smallest
supported scope set.

### 3.2 Actions and triggers

```text
ConnectorActionDescriptor {
  action_key: string
  title: string
  description: string
  input_schema: JsonSchema
  output_schema: JsonSchema
  operation_class: READ | WRITE | SEND | DELETE | PURCHASE_LIKE | ADMIN
  idempotency: PROVIDER_KEY | CLIENT_KEY | READ_ONLY | UNKNOWN
  effect_kind?: EffectKind
  required_scopes: string[]
  timeout_class: INTERACTIVE | NORMAL | LONG
}

ConnectorTriggerDescriptor {
  trigger_key: string
  title: string
  payload_schema: JsonSchema
  delivery_modes: (PUSH | POLL | CURSOR_POLL)[]
  required_scopes: string[]
  stable_delivery_id: bool
  cursor_support: NONE | OPAQUE | MONOTONIC
  provider_replay_window?: Duration
}
```

A descriptor is untrusted package input until schema bounds, provenance and provider
qualification pass. A declared `READ` label cannot override Trust's own action
classification.

## 4. Connection record

The existing `Connection` aggregate is extended; there is no second hidden connector
database in LiteCowork.

```text
Connection {
  connection_id: ConnectionId
  workspace_id: WorkspaceId

  connector_ref: ConnectorRef
  connector_revision_digest: Sha256Digest
  auth_recipe_id: string
  external_provider_ref: string
  account_ref?: string
  account_display_hint?: string

  granted_scopes: string[]
  scope_set_digest: Sha256Digest
  secret_refs: SecretRef[]
  token_expires_at?: Timestamp
  refresh_capable: bool

  status:
    CONNECTING
    | CONNECTED
    | DEGRADED
    | REAUTH_REQUIRED
    | REVOKED
    | DISCONNECTED

  last_health_at?: Timestamp
  last_health_reason?: string
  created_at: Timestamp
  updated_at: Timestamp
  version: u64
}
```

`account_display_hint` is non-secret display metadata such as a redacted email address.
It is never treated as authenticated identity; authenticated account identity comes from
the provider result during setup/refresh.

A Connection contains SecretRefs, never token bytes. Secret placement is deployment
specific, but desktop defaults to the OS-protected LiteCowork SecretStore.

## 5. Local OAuth and credential boundary

### 5.1 Public desktop OAuth with loopback + PKCE

When a provider supports it:

```text
Connect
  -> create random state + PKCE verifier/challenge
  -> bind one-shot listener to 127.0.0.1 on random high port
  -> open system browser to provider authorization endpoint
  -> provider redirects to exact loopback callback
  -> validate state, issuer and callback ownership
  -> exchange code using PKCE
  -> query provider identity
  -> store refresh/access material in SecretStore
  -> commit Connection metadata + SecretRefs
  -> close listener
```

Security rules:

- bind loopback only, never `0.0.0.0`;
- random port allocated by the OS;
- state is cryptographically random, single-use, and expires quickly;
- PKCE uses a fresh verifier per attempt;
- reject an unexpected issuer, redirect URI, duplicate callback, wrong state, wrong port,
  or callback after cancellation;
- callback response body contains no token;
- no OAuth token is written to logs, events, crash metadata, clipboard, Conversation, or
  browser URL history under LiteCowork control;
- listener closes after first terminal outcome;
- restart loses the ephemeral auth attempt and requires a new attempt;
- completed credentials are written before Connection becomes `CONNECTED`;
- a Connection commit failure destroys newly returned credential material where feasible.

### 5.2 Confidential and hosted OAuth

Some providers/features require a confidential client or stable HTTPS callback. The package
may declare one of:

1. deployment-owned callback service;
2. user-configured OAuth app credentials;
3. optional external AuthBackend such as Nango or Arcade;
4. provider-specific native app/device flow.

The UI must name which party will hold credentials before authorization. It may not imply
"local-only" when a hosted auth backend owns refresh tokens.

### 5.3 API keys and service accounts

Credential entry uses a trusted local no-store form owned by the host, never the model.
The resulting SecretRef can be assigned to a Connection. Re-rendering the form reports
configured/not configured; it never echoes the original secret.

### 5.4 Provider-specific exceptions

AuthRecipe is deliberately provider-specific. For example, a Slack user-auth desktop flow
and a Slack bot installation may require different recipes/scopes/redirect constraints.
LiteCowork must not force both through one "generic OAuth" button.

## 6. ConnectionService

```text
interface ConnectionService {
  list(workspace_id, query, cursor?, limit) -> Page<ConnectionSummary>
  get(connection_id) -> ConnectionView

  begin_auth(
    connector_ref,
    recipe_id,
    requested_features,
    expected_workspace_version?,
    request_id
  ) -> AuthStartReceipt

  complete_auth(
    auth_attempt_id,
    ProviderCallback,
    request_id
  ) -> ConnectionView

  cancel_auth(auth_attempt_id, request_id) -> AuthAttemptView

  refresh(connection_id, reason, request_id) -> ConnectionView
  test(connection_id, request_id) -> ConnectionHealthView
  disconnect(connection_id, expected_version, request_id) -> ConnectionView
  revoke(connection_id, expected_version, request_id) -> ConnectionView

  resolve_capabilities(connection_id, query) -> CapabilityDescriptorPage
}
```

### begin_auth pseudocode

```text
assert workspace writable
descriptor = LiteSPM.resolve_connector(connector_ref)
assert descriptor qualified and digest current
recipe = descriptor.auth_recipe(recipe_id)
requested_scopes = derive_minimum_scopes(requested_features, recipe)
Trust.authorize_connection_setup(principal, workspace, connector, requested_scopes)

attempt = AuthAttempt.create(
  random_state,
  fresh_pkce_if_required,
  exact_recipe_digest,
  expires_at
)

if LOOPBACK_RANDOM_PORT:
  listener = LoopbackAuthHost.bind_127_0_0_1_random()
  attempt.bind_callback(listener.identity)

persist only non-secret attempt metadata/digests
return provider_authorization_url + trusted display metadata
```

### complete_auth pseudocode

```text
attempt = load_auth_attempt()
assert ACTIVE and unexpired and callback binding current
verify state/issuer/origin exactly
tokens = provider.exchange_code(ephemeral verifier)
identity = provider.resolve_account_identity(tokens)
secret_refs = SecretStore.write_atomic(tokens)
begin transaction:
  recheck workspace + descriptor digest + attempt version
  upsert Connection metadata and SecretRefs
  settle AuthAttempt
  append events
commit
zero ephemeral token buffers best-effort
return redacted ConnectionView
```

### refresh pseudocode

```text
lock per connection
load Connection + secret lease
if not refresh_capable -> REAUTH_REQUIRED
tokens = SecretStore.resolve_for_provider_only()
result = provider.refresh(tokens)
if success:
  rotate SecretRefs atomically
  update expiry/status/identity if provider confirms
else if invalid_grant/revoked:
  mark REAUTH_REQUIRED
else:
  mark DEGRADED with bounded reason and retry policy
release lock
```

Refresh failures never delete historical Task/Effect provenance and never trigger a
consequential retry by themselves.

## 7. ConnectorRuntimeAdapter

```text
interface ConnectorRuntimeAdapter {
  describe(connector_ref) -> ConnectorDescriptor
  health(connection_id) -> HealthObservation

  search_actions(connection_id, natural_query, limit) -> ActionDescriptor[]
  get_action(connection_id, action_key) -> ActionDescriptor

  invoke(
    connection_id,
    action_key,
    normalized_input,
    invocation_context,
    idempotency_key?
  ) -> ProviderInvocationReceipt

  reconcile(provider_receipt) -> ProviderOutcome
  cancel(provider_receipt) -> CancelObservation

  list_triggers(connection_id) -> TriggerDescriptor[]
  subscribe(trigger_request) -> TriggerSubscriptionReceipt
  unsubscribe(subscription_ref) -> ProviderOutcome
  poll(trigger_request, opaque_cursor?) -> TriggerPollBatch
}
```

The adapter never receives more secret material than its exact operation needs. Capability
Gateway dispatch supplies an expiring SecretLease or provider-side injection, not token
bytes inside model-generated JSON.

## 8. Google Workspace first-party bundle

The first-party product surface should be one connection family:

```text
Google Workspace
  Gmail
  Calendar
  Drive
  Docs
  Sheets
```

One account may satisfy multiple services, but requested scopes remain feature-derived.
A user enabling Calendar read does not silently grant Gmail send. Each action still maps to
its own capability and Trust classification.

The connector package owns provider API quirks, pagination, incremental sync, stable object
identity, send-as identities, file export/import limits and trigger support. LiteCowork Core
owns Connection status, Resource provenance, CapabilityGrant, Invocation, Effect, Automation
and audit.

The same pattern applies to Microsoft 365. Slack, Notion, GitHub and other first-party
connectors may expose separate provider-specific recipes.

## 9. Event relay

Credential locality and event reachability are separate problems.

```text
EventRelayEnvelope {
  relay_event_id: string
  workspace_route_ref: OpaqueRouteRef
  connection_ref_digest: Sha256Digest
  provider_ref: string
  provider_event_type: string
  provider_delivery_id?: string
  observed_at: Timestamp
  opaque_cursor_hint?: string
  payload_digest?: Sha256Digest
  signature: RelaySignature
}
```

The managed relay MUST NOT require the provider OAuth refresh token merely to queue a wake
hint. Prefer provider webhook signatures or a provider-specific narrow subscription secret.
The envelope carries no email body, document bytes, chat contents, access token, signed URL,
or arbitrary unbounded provider payload unless a separately reviewed connector explicitly
requires it.

Supported hosting modes:

```text
LOCAL_POLL
LOCAL_PUSH_WHEN_REACHABLE
LITECOWORK_RELAY
USER_SELF_HOSTED_RELAY
EXTERNAL_WORKFLOW_BACKEND
```

### Relay intake

```text
verify relay signature and route
dedupe relay_event_id / provider_delivery_id
persist bounded receipt
if eligible TriggerHost online:
  wake/re-evaluate ConnectorEventTrigger
else:
  retain bounded pending hint according to policy
do not create Effect or fetch provider content inside relay transaction
```

When an authorized TriggerHost processes the hint, it resolves the current Connection and
fetches the actual provider object using its local SecretStore/provider adapter. If the
Connection is revoked or unavailable, the occurrence waits or is skipped according to the
Automation policy; the relay hint never revives credentials.

## 10. WorkflowBackendAdapter

A workflow backend is useful for deterministic or already-built processes. It is not an
agent planner.

```text
interface WorkflowBackendAdapter {
  health(connection_id) -> HealthObservation

  list_workflows(connection_id, query?, cursor?, limit)
    -> Page<WorkflowDescriptor>

  get_workflow(connection_id, workflow_ref)
    -> WorkflowDescriptor

  validate_input(connection_id, workflow_ref, input)
    -> ValidationResult

  invoke(
    connection_id,
    workflow_ref,
    input,
    mode: TEST | PRODUCTION,
    idempotency_key?,
    invocation_context
  ) -> ExternalWorkflowRunReceipt

  get_run(connection_id, external_run_ref) -> ExternalWorkflowRunView
  stream_run(connection_id, external_run_ref, cursor?) -> EventStream
  cancel_run(connection_id, external_run_ref) -> CancelObservation
  normalize_result(connection_id, external_run_ref) -> WorkflowResultEnvelope
}
```

A configured backend is represented by an ordinary Connection plus qualified provider
metadata. A discovered workflow is projected as a CapabilityRef. External run IDs remain
Runtime/provider-private where possible:

```text
WorkflowRunBinding { # Runtime-local
  invocation_id: CapabilityInvocationId
  runtime_id: RuntimeId
  runtime_incarnation: RuntimeIncarnation
  backend_connection_id: ConnectionId
  external_run_ref_ciphertext: bytes
  external_run_ref_digest: Sha256Digest
  created_at: Timestamp
}
```

LiteCowork's `CapabilityInvocation` remains authoritative for requested operation and
settlement. The external engine's run state is evidence/observation, not a second LiteCowork
Task state machine.

### Workflow invocation mapping

```text
Task Step
  -> CapabilityInvocation
  -> WorkflowBackendAdapter.invoke
  -> external workflow run
  -> normalized progress observations
  -> provider Effect observations
  -> WorkflowResultEnvelope
  -> verifier / parent Step settlement
```

If the external workflow performs sends/deletes/writes, those operations are not magically
reversible. The adapter declares expected effect classes and, where possible, returns
provider receipts. Cancellation means "requested cancellation"; it does not mean rollback.

## 11. Activepieces compatibility provider

Two distinct modes are allowed.

### Mode A: Piece compatibility

`ActivepiecesPieceProvider` is a LiteSPM provider capable of translating a *qualified*
community Piece into ConnectorPackage descriptors.

```text
probe_piece(package):
  verify package source/digest/license path
  resolve dependency manifest without executing install scripts
  parse Piece auth/action/trigger descriptors
  reject unsupported dynamic code-loading or unsafe dependency hooks
  map auth -> AuthRecipeDescriptor
  map actions -> ConnectorActionDescriptor
  map triggers -> ConnectorTriggerDescriptor
  run fixture tests in isolated environment
  publish only qualified descriptor
```

No Piece can self-register a Trust rule, SecretLease, Automation, Coworker assignment or
native agent tool. Package updates produce a new digest and requalification.

### Mode B: external Activepieces backend

A user may connect a self-hosted/managed Activepieces instance through
`WorkflowBackendAdapter`. LiteCowork invokes existing flows and observes runs; it does not
need to load Piece code locally.

These modes are independent and may ship at different times.

## 12. n8n, Zapier, Nango, Arcade and Composio

- **n8n:** external WorkflowBackend only by default. User supplies/connects an instance
  supported by the adapter. No bundled n8n database/runtime.
- **Zapier:** external workflow/action backend when a stable supported API exists. Never
  screen-scrape the user's Zapier editor as the integration contract.
- **Nango:** optional `AuthBackendAdapter` for deployments that intentionally centralize
  OAuth. Default desktop does not require it.
- **Arcade:** optional authenticated MCP/tool gateway. Treat returned sign-in URLs as
  provider handoffs and keep Trust around every resulting capability invocation.
- **Composio:** optional connector/tool provider. It may accelerate breadth but is not
  required for core Gmail/Calendar/etc.

## 13. AuthBackendAdapter

```text
interface AuthBackendAdapter {
  begin(provider, requested_scopes, callback_context) -> AuthBackendStart
  poll_or_complete(attempt_ref, callback?) -> AuthBackendOutcome
  get_connection_health(external_connection_ref) -> HealthObservation
  revoke(external_connection_ref) -> RevokeObservation
  materialize_provider_credential(
    external_connection_ref,
    exact_operation,
    secret_lease
  ) -> ProviderCredentialHandle
}
```

The last operation must prefer provider-side injection or a narrowly scoped ephemeral
handle. A hosted AuthBackend must not hand raw refresh tokens to the model.

## 14. UI contract

The ordinary connection path is:

```text
Connections & Tools
  -> search app
  -> choose account / Connect
  -> trusted browser sign-in or local secret form
  -> account + requested access summary
  -> Connected
```

Advanced details show provider, package source/version/digest, auth owner, granted scopes,
connection health, last refresh, and capability count.

Do not show a giant per-tool authorization grid during onboarding. `Manage access` may
restrict eligible resources/actions later, but action-time Trust still applies.

Task-time repair:

```text
Task blocked: "Google Calendar isn't connected"
  [Connect Google Calendar]
  -> auth
  -> assign Connection to current Coworker/Workspace
  -> revalidate same Task
  -> reconcile any ambiguous prior Effect
  -> resume only unfinished safe work
```

## 15. Workflow UI

Do **not** add a second visual workflow-builder to LiteCowork merely because n8n/Activepieces
exist. The default UI is:

```text
Use existing workflow
  Backend: n8n
  Workflow: Publish weekly report
  Inputs: ...
  Run mode: Test | Production
  Approval: ...
```

Advanced users can open the source workflow system. LiteCowork stores only the stable
reference, input/output contract, run provenance and normalized outcome.

## 16. Unified product IA: no Cowork | Code split

Coding is a Task intent, not a separate root domain. LiteCowork keeps one Conversation and
Task architecture.

```text
+ New
  Chat
  Code task
  Research
  Work with files
  Automation
```

A "Code task" is a launch preset that can preselect a project/folder, preferred coding
AgentBinding, worktree policy and verification defaults. It still creates the same
Conversation/Task/Plan/Attempt objects.

Switching AgentBinding changes the composer controls according to
`AgentControlDescriptor`; there is no duplicate Code chat implementation.

## 17. Computer/environment semantics

Product language may say a Coworker has "its own computer" when the Coworker has a durable
private working scope, browser profile and eligible Environment policy. The implementation is
logical:

```text
Coworker
  private Resources / indexes / memories
  browser profile policy
  allowed Workspace roots
  default Environment preference

Task/Attempt
  obtains actual Environment
  obtains browser/terminal/computer leases
  receives scoped CapabilityGrants/SecretLeases
  releases or suspends according to policy
```

Environment kinds can be local native, WSL, container, remote sandbox or future cloud.
A permanent VM is not required and must not be created just to satisfy the metaphor.

## 18. Failure and edge-case matrix

| Case | Required behavior |
|---|---|
| OAuth user cancels | AuthAttempt CANCELLED; no Connection; draft/task preserved |
| OAuth callback duplicated | First terminal callback wins; later callback rejected |
| Wrong state/issuer/port | Fail closed; no token exchange/Connection |
| Token exchange succeeds but DB commit fails | Do not expose Connected; purge/rotate newly returned secrets best-effort |
| Refresh token revoked | Connection -> REAUTH_REQUIRED; waiting Tasks keep blockers |
| Scope removed | Re-resolve capabilities; new calls denied; existing ambiguous Effects reconcile |
| Connector package digest changes | Existing Connection stays bound to pinned qualified revision until explicit migration/compatibility review |
| Package removed | Connection DEGRADED/unsupported; history remains readable |
| Account reused across Coworkers | One Connection may be assigned multiple times with independent ceilings; no duplicate OAuth required |
| Coworker assignment revoked | Future eligibility fenced; shared Connection remains for other assignments |
| Provider returns 429 | Retry policy bounded; do not call it quota exhaustion unless provider confirms |
| Provider timeout after write | Mark Effect ambiguous; reconcile before retry |
| Workflow backend unavailable | Invocation waits/fails by policy; LiteCowork Task truth remains intact |
| External workflow cancel acknowledged | Continue polling until terminal/unknown; do not claim rollback |
| Workflow emits partial side effects then fails | Preserve effects/evidence; verifier determines Task recovery |
| Laptop asleep with local poll trigger | Misfire/reconciliation policy applies; no invented intermediate events |
| Managed relay receives event while laptop offline | Queue bounded hint; fetch real object only after authorized Runtime returns |
| Relay duplicate | Dedupe by relay/provider delivery identity |
| Relay compromised | Signature/route failure drops event; relay cannot mint Connection/Grant |
| Remote connector payload contains prompt injection | Treat as untrusted Resource content; never changes Trust/learning policy |
| Secret store locked | Connection not ready; never fall back to plaintext |
| External auth backend down | Only Connections depending on it degrade; direct local connectors continue |
| Activepieces Piece has install script/native dependency | Reject compatibility or require isolated qualification; never execute during catalog parse |
| n8n licensing/config unsupported | Adapter unavailable with explicit reason; never silently embed workaround |

## 19. Tests required before shipping

1. PKCE/state/loopback adversarial callback suite.
2. Token bytes absent from logs/events/crash dumps/UI state snapshots.
3. SecretStore locked/unavailable/restart/rotation cases on every desktop OS.
4. Refresh serialization and revoked-token handling.
5. Scope downgrade and package-digest migration.
6. Same Connection assigned to multiple Coworkers with independent revocation.
7. Capability search bounded with 10k+ action descriptors.
8. Provider timeout after a real write; no duplicate retry.
9. Relay duplicate/reorder/delay/signature failure/offline replay.
10. External workflow partial side effects, cancellation ambiguity and lost polling cursor.
11. Activepieces compatibility fixture for auth/action/trigger plus malicious package fixtures.
12. User-owned n8n execution with test vs production intent and normalized run history.
13. Task-time Connect resumes exactly the blocked Task and not a replacement.
14. Disconnect during running Invocation fences new calls and reconciles the old one.
15. Desktop UI never asks the model to collect credentials in chat.
16. No global Cowork/Code mode divergence; Code preset uses canonical Conversation/Task path.

## 20. Implementation file/module target

Suggested implementation boundaries; exact language placement may be adjusted without
changing ownership:

```text
# Logical TARGET layers, not current repository directories:
# Connection / AuthAttempt / AuthRecipe / SecretRef / ProviderAdapter
#   -> reconcile with existing crates/storage-core, storage-sqlite,
#      existing domain/application services and apps/litecoworkd.
# EventRelay / connector triggers / external workflow receipts
#   -> reconcile with existing domain-responsibility Automation owners.
# UI Connections / Discover / Automation
#   -> reconcile with apps/litecowork-ui/src/ existing routes.
# DO NOT create a parallel crates/application/ or crates/ports/ tree
# based solely on this historical illustrative layout.
# LiteSPM package management is independent; negotiate its local
# protocol instead of directly accessing its storage.
```

Core dependencies point inward to ports/domain contracts. Provider SDKs and third-party
framework code remain in adapter/provider packages, never in Task domain crates.

## 21. Release sequencing

1. canonical Connection schema + AuthAttempt + SecretStore integration;
2. one direct local-first connector with read-only action;
3. write action + Effect reconciliation;
4. trigger subscription/poll path + Automation occurrence;
5. Google Workspace bundle;
6. EventRelayProvider spike;
7. WorkflowBackendAdapter + one user-owned backend;
8. Activepieces Piece compatibility spike;
9. optional external auth/tool gateways;
10. broader connector catalog only after package provenance, update and failure tests pass.

## V1 detailed design integration — 2026-10-12

V1-FUNCTION-FILE-CATALOG.md and DESKTOP-SHELL-LLD.md enumerate connection lifecycle, Settings/Discover state and task-time repair. LiteSPM owns package installation and lifecycle, LiteCowork owns authorized Task-scoped capability; installed/connected/assigned/ready/granted/invoked must display differently. Test one real read/write/trigger provider before broad catalog claims. Existing connection/auth/event relay designs remain normative.
