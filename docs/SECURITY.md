# Security Architecture

## Trust boundaries

1. Human operator surfaces
2. Local LiteCowork Runtime
3. Other paired runtimes/cloud Hub
4. External agents
5. LiteSPM/package sources
6. MCP/capability providers
7. Environments/sandboxes
8. External messaging channels
9. External content being processed
10. Embedded MCP App iframe
11. Untrusted generated interactive Artifact package and isolated renderer

No boundary is implicitly trusted merely because it is local.

## Threats and required controls

### Malicious capability/MCP
Controls:
- exact package digest/version lock
- declared permission review
- grant matching the authenticated session scope; Conversation/planning grants are read-only and secret-free
- least-privilege resource scope
- process/network isolation where possible
- effect ledger for mutations
- revocation and health monitoring

### Prompt injection from web/email/doc
Controls:
- external content treated as data, not authority
- system/user policy precedence preserved by agent adapters where possible
- capabilities require independent grants regardless of content instructions
- sensitive effects require approval/policy even if agent requests them

### Compromised agent
Controls:
- agent cannot directly mutate Core domain state
- bounded TaskPacket
- capability grants and environment write scope
- lease/fence validation on Core effects
- short-lived AgentSession-scoped Gateway credentials and method allowlists
- separate EnvironmentControlLease epochs for human/agent input ownership
- output verification

### Credential solicitation through provider input

Ordinary UserRequest answers are non-secret and may replicate with Workspace data.
The Gateway rejects secret-typed or suspicious credential requests; provider credentials
must use a Connection/SecretStore flow or a supported out-of-band authorization handoff.
MCP form input is non-sensitive only. URL-mode state remains encrypted in the source
Runtime and reaches the authenticated Operator only after an explicit no-store handoff.
Unknown MCP input methods fail closed rather than becoming arbitrary user prompts.

### Compromised runtime/device
Controls:
- per-device identity keys
- runtime revocation
- no automatic credential replication
- secret placement rules
- artifact/event integrity checks
- sensitive cloud access policy

### Replay attacks
Controls:
- single-use pairing tokens
- request IDs/nonces for approvals and mutations
- short-lived authorization/secret leases
- fencing epochs
- deduplicated external channel events

### Stale executor/split brain
Controls:
- execution lease epoch + Runtime-private fencing credential (durable state stores only its digest)
- Core mutation validation
- explicit lease expiry before failover

### Channel spoofing
Controls:
- ChannelBinding to authenticated external identity
- map inbound sender to the bound Workspace owner Principal; reject unknown/mismatched senders
- provider event dedupe
- assurance levels
- exact, one-use reply-to target for UserRequest responses; never select the latest open request
- current `RESPOND` action and binding status are rechecked when each reply arrives
- sensitive approval escalation to stronger surface

### Artifact poisoning/tampering
Controls:
- content digest
- immutable ArtifactVersion
- provenance
- blob verification after replication

### Generated interactive Artifact
Controls:
- self-contained package manifest and exact ArtifactVersion digest; path/archive validation before preview or execution
- inert static preview; executable content never runs in the privileged desktop renderer
- a distinct unprivileged renderer under a proven OS isolation profile; unsupported profiles remain preview-only
- deny-by-default network, filesystem, process, app IPC and secret access; loopback is not a bypass
- closed run-bound IPC with sequence/replay checks and run-scoped exact-revision read handles
- host-owned save preview and confirmation; app messages never authorize Effects or report verified completion
- process-tree quiescence and broker-handle revocation before STOPPED; interruption remains unknown
- explicit, deletable, Workspace/ArtifactVersion-namespaced local app state; no credential storage or sync

## Secrets

Secret bytes live in SecretStore implementation, not Task/Event payloads. Domain stores SecretRef and lease metadata only.

No browser cookies/native agent auth are silently uploaded to cloud.

## Network

Managed/default runtime connectivity is outbound authenticated TLS. Self-host options may use Tailscale/WireGuard/SSH but Core protocol is transport-independent.
Worker egress, DNS/redirect/SSRF protection, local IPC identity, filesystem TOCTOU,
upload/archive limits, and MCP App sandbox rules are normative in
[`NETWORK-SECURITY.md`](NETWORK-SECURITY.md). Sandbox-local firewall rules alone are not
the enforcement boundary.

MCP Apps have no host DOM, cookies, local storage, native IPC, local file, or SecretStore
access. CSP domains are checked by the host egress policy; every tool/resource request
passes the ordinary capability grant and Invocation checks. Consequential mutations also
require Effect recording before dispatch; audit records follow the operation's sensitivity
and Trust policy.

## Supply chain

LiteSPM package verification metadata should include source, publisher, digest/signature, permissions, compatibility and last verification. LiteCowork consumes normalized lock metadata and independently applies task policy.


## Agent lifecycle, registry, and native configuration security

ACP/distribution registry metadata is untrusted supply-chain input. It may describe a
package, but it cannot provide an arbitrary executable command, credential destination,
native-config path, or privileged callback that LiteCowork blindly executes.
AgentModuleRegistry maps recognized registry identities to trusted/versioned lifecycle
adapters that compile bounded install/update/auth actions.

AgentControlDescriptors are sanitized **non-secret observations**. Raw native config,
provider responses, account tokens, API keys, cookies, endpoint locators, private prompts,
native session handles, and unbounded command output are forbidden.

Secret-bearing agent setup has two valid ownership modes:

1. **NATIVE_AGENT** — open/use the agent's own sign-in/configuration flow; LiteCowork
   observes only sanitized auth state.
2. **LITECOWORK_SECRET_STORE** — only an adapter-declared credential slot may accept local
   privileged secret input; bytes go directly to SecretStore, are zeroized from the
   request buffer, and only a SecretRef enters durable configuration.

AgentBinding non-secret configuration is closed-schema and descriptor-digest pinned.
Unknown fields and stale descriptor digests fail closed. A successfully installed,
authenticated, or configured agent receives no Workspace/Task authority until the
separate AgentBinding/Trust admission allows it.

Install/update subprocesses use a lifecycle-specific environment allowlist and official
module-defined distribution semantics. Registry text must never become a shell command.


## Connector, workflow, relay, and learning abuse cases

Additional security cases for the accepted connector/learning architecture:

- malicious ConnectorPackage lies about an operation being read-only;
- package update changes action/trigger schema or dependency install behavior;
- OAuth state/PKCE callback is intercepted or replayed locally;
- hosted AuthBackend is compromised or unavailable;
- token refresh races with revoke/scope downgrade;
- EventRelay forges, replays or reorders wake hints;
- external workflow reports success after partial external Effects;
- workflow cancellation returns success while downstream actions continue;
- a compatibility package uses install hooks/native dependencies to escape qualification;
- email/web/chat/document content injects instructions to remember or enable a schedule;
- model-generated learning candidate contains credentials or inappropriate private data.

Mitigation ownership remains split: LiteSPM verifies package provenance/dependencies,
ConnectionService/SecretStore own auth material, CapabilityBroker/Trust own use authority,
Effect reconciliation owns ambiguous mutations, TriggerCoordinator owns occurrence
dedupe/cursors, and LearningService owns candidate eligibility/redaction. No connector
adapter may bypass those layers.

## V1 detailed design integration — 2026-10-12

V1 Browser/Composer/Renderer threat model: untrusted native commands, prompt-injection metadata, unsafe terminal escapes/HTML, external Chrome privileged attach, profile data and cross-Workspace Workbench leakage. See NATIVE-COMPOSER-LLD.md, CHROME-BROWSER-LLD.md and RENDERING-CONFORMANCE-LLD.md. No secret in domain event/UI; every browser/agent action checked at provider boundary; human takeover fences stale agent actions.


## Non-normative desktop sandbox reference - 2026-10-31

The [Desktop Reference Implementation Audit](DESKTOP-REFERENCE-IMPLEMENTATION-AUDIT.md) reviews ZCode's Main-owned plugin guest sandbox and its origin/owner binding, CSP, guest permission restrictions and mediated MessagePort bridge. These are implementation patterns for defense in depth, not a replacement for this threat model or the OS-specific Environment qualification. Renderer isolation, CSP and a separate Chromium partition do not alone prove filesystem/process/network confinement; preserve the existing deny-by-default and fail-closed requirements.

## External workflow authoring and provider Code execution — 2026-11-06

External workflow products may offer both a visual canvas and provider-native AI/MCP
authoring. This creates a high-authority third-party control surface. The fact that the
workflow graph remains provider-owned does not make its tools read-only or low-risk.

### Tool authority is not one boolean

A future LiteCowork provider integration must separate these operation classes:

1. **Discover/read:** list workflows, inspect selected graph metadata, schemas and run
   history. Start with read-only access and an explicitly selected project/workflow set.
2. **Draft/mutate:** create a new draft; add/remove/reconfigure nodes/steps/branches; change
   code source, dependencies, input mappings or credentials references. This changes
   provider configuration but need not activate it.
3. **Validate/test:** run schema checks and test executions. Tests may still invoke real
   third-party APIs or write to connected accounts; the word "test" is not proof of
   side-effect-free behavior.
4. **Publish/activate/enable:** make a version eligible for recurring, webhook or
   production invocation. This is a distinct consequential permission and requires owner
   confirmation bound to the exact graph revision/digest and effect summary.
5. **Run production:** execute an enabled provider graph. This is distinct from publishing
   and is subject to Task scope, declared external effects, provider policy and
   reconciliation.
6. **Delete/disable/revoke:** destructive or availability-changing operations. They need
   separate explicit authority and cannot be inferred from create/edit permission.

Do not collapse any of these into a generic "workflow MCP access" grant. The agent's ability
to list/read must never imply permission to mutate, publish, activate, run or delete.

### Provider-specific permission observations

- n8n instance-level MCP requires instance owner/admin enablement and per-workflow exposure.
  Connected clients have explicit permission scopes; clients share the same exposed
  workflow set, so avoid enabling a broad library just because one Coworker needs one
  workflow. A user-level workflow visibility boundary still applies. Inspect the exact
  n8n version because access-settings and per-client UI capabilities differ by version.
- Activepieces discovery is documented as always-on/read-only, while other tool categories
  are enabled/disabled per project. The authenticated MCP project is the operation scope.
  Flow deletion is documented as irreversible. These are provider-side permission settings,
  not LiteCowork grants.
- Credentials remain in provider-owned connection stores. LiteCowork must not request or
  persist raw provider credentials because an agent workflow builder asks for them.
  Setup instructions should direct the user to the provider's own credential UI/OAuth flow.

### Prompt injection and untrusted provider state

Treat workflow names, descriptions, node labels, prompts, code, comments/notes, input
schemas, sample data, run outputs, error text, webhook payloads and tool results as
untrusted data. They may contain prompt injection or misleading claims about required
scopes, safety or successful completion. Never let provider text alter LiteCowork grants,
Connection scope, Workspace boundaries, approval policy, model routing or Task authority.
Escape/display provider text as data; schema-validate normalized fields; keep provider
identifiers and opaque run handles out of public domain events unless the owner contract
explicitly permits a protected reference.

### Code nodes and external execution

n8n JavaScript/Python Code nodes, Activepieces JavaScript/TypeScript CODE steps and
Node-RED Function nodes execute inside the external product's runtime. Their actual
authority depends on the provider's account identity, credential references, host
filesystem, environment variables, dependency loading, network/egress configuration,
sandbox/worker mode and provider version. They are not LiteCowork Environment processes.
LiteCowork Trust and Environment cannot claim to mediate internal provider-node effects
unless every such effect is explicitly surfaced through a qualified provider gateway and
reconciled.

Before enabling provider Code nodes or agent-created graphs, require:
- exact provider instance and version observation;
- execution mode/runner/sandbox and network policy observation where the provider exposes it;
- enabled language/node types and dependency-loading policy;
- credential references and external service scopes;
- host filesystem, process and network effects disclosed by the provider;
- current provider security-advisory review;
- a draft/test/publish flow with exact graph revision and explicit user confirmation;
- provider run identity and failure/partial-effect reconciliation.

The n8n advisory GHSA-9pq8-m8gp-4p53 (CVE-2026-49444) is condition-specific: it affects
instances where Python Task Runner is enabled and a workflow containing a Python Code Node
can be modified by an authenticated user; the disclosed impact is arbitrary code execution
on the task-runner container. The published fixed versions are 1.123.48, 2.21.8 and 2.22.4.
This does not mean every n8n Code node is vulnerable, and fixing this advisory alone does
not establish that the rest of an instance is secure. Recheck the current provider advisory
and exact deployment configuration before use.

### LiteCowork Task and Evidence boundary

A provider-side workflow test, published run, successful status or node-level output is an
external observation, not LiteCowork Task completion. The local parent Task records the
single authorized CapabilityInvocation, provider reference/run observation, declared
external effects and independent verification required by the Task. Do not synthesize local
Task Steps from external provider nodes. An ambiguous provider run must be reconciled
against the original provider run/idempotency identity before another consequential run.

Current state: no production WorkflowBackendAdapter or WorkflowAuthoringAdapter was found
in apps/ or crates/ in the 2026-11-06 source scan. This section is a target security
contract for future provider qualification; it does not authorize or claim an active
external workflow capability.
