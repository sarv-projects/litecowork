# Digest docs-07 — ADRs, audits, reconciliation, competitive evidence

Files: 18 read / 18 listed | Lines read: 6,576 | Bytes: 455,100 (444 KiB)

## Per-file notes

### docs/AGENT-CONTROL.md (1313)
- Owner Agent Fabric; "vNext target contract". LLD for AgentModule registry, install/auth/config, native surface, composer projection, Conversation admission. Says the "25-entry external candidate roster is not an assertion of 25 operational native session adapters".
- 12 invariants: no global provider/model layer; registry listing ≠ readiness; install/auth/compatibility/session readiness independent; every config field owned by `NATIVE_AGENT | LITECOWORK_NON_SECRET | LITECOWORK_SECRET_REF | LITESPM_CAPABILITY`; no secret bytes in config JSON; descriptors time-bounded; no silent option substitution.
- Types: `AgentModule`, `AgentLifecycleAdapter` (`probe_control_descriptor`, `begin_auth`, `submit_credential`, `open_native_config`), `AgentAdapter`, `AgentControlDescriptor` (`descriptor_digest: Sha256Digest`), `AgentCredentialSlotDescriptor.storage_owner`, `AgentSessionOption.mutable_scope = LIVE_SESSION|NEW_SESSION|UNSUPPORTED`, auth states `UNKNOWN|NEEDS_AUTH|AUTHENTICATING|AUTHENTICATED|EXPIRED|DEGRADED|ERROR`.
- `AgentBinding` gains `configuration_descriptor_digest`, `credential_bindings[]`, `default_session_options`; one replicated event `agent.binding.configuration.changed.v1` (credential **slot IDs only**); Runtime-local tables `agent_registry_cache`, `agent_installation_observations`, `agent_control_descriptors`, `agent_lifecycle_operations`.
- Admission §9.7: persist USER + turn `OPEN`, reserve session `STARTING`, invoke adapter **outside** SQLite, then atomically ACTIVE+RUNNING after `SESSION_READY`; native handle only in Runtime-local host binding. Credential route needs `Idempotency-Key` (1–200 printable ASCII) and stays unavailable until exact replay of the original `SecretRef`.
- §17 lists 8 research gates; §19 splits 14 stories; two-axis addendum: Axis A local harness module, Axis B **hosted agent provider** (V1 scope; `E11-S01..S05` is post-V1 "our Runtime in cloud").

### docs/CHANNELS.md (274)
- Channels are human transports, not Task owners. `ChannelAdapter`, `ChannelCapabilities` (10 booleans incl. `supports_deferred_acknowledgement`, `replay_window_ms`), `DeliveryReceipt`.
- `ChannelBinding.status = ACTIVE|REVOKED|DEGRADED`; one `ChannelHostAssignment` per ACTIVE binding owned by RuntimeMesh (`host_epoch` + bounded lease); `ACTIVE→DRAINING` stops new claims/polls/sends.
- `ChannelEventReceipt`: `RECEIVED → PROCESSING → ACCEPTED|REJECTED|FAILED`; dedupe `(channel_binding_id, provider_event_id)`; `claim_epoch` fences late workers; changed payload digest = conflict.
- `ChannelAction.RESPOND` separate from `STEER` and both approval actions; only non-sensitive `FORM` with one pending `UserRequest`; `EXTERNAL_URL`, Approvals, credentials require Operator.
- Ingress cursor only in encrypted `ChannelIngressCursorBinding` on the assigned Runtime; ack handle process-memory only; explicit `accept_ingress_gap` is an audited loss.
- No channel-specific Task/session store; thread→Conversation unique per `(channel_binding_id, provider_thread_id)`; EventRelay ≠ ChannelBinding; channel content is not automatic learning.

### docs/COMPETITIVE-RESEARCH.md (608)
- Non-normative dated research ("evidence, not LiteCowork requirements"). **Date defect:** title says "latest snapshot: 2026-10-25" but body contains a 2026-11-01 pass.
- Snapshots: 2026-10-10 table (Claude Cowork Pro/Max change announced 2026-10-06; ChatGPT Work; OpenHands Agent Canvas; OpenClaw; Agent Zero; Manus Cloud Computer; Cursor Automations; Goose; Copilot hooks; Devin 404/search-index only; OpenWorker; OpenCoworkAI; Zapier/n8n), sources "checked on 2026-10-04, selectively rechecked 2026-10-05".
- 2026-11-01 rivals index with pinned revisions `ec75891b819713acf29cb659bd9c72d1ed34f76a` (athulsreekumar/opendot, MIT) and `11cd2a9a20d1e033157636efc570b41384c66d85` (iOfficeAI/AionUi, Apache-2.0, ~2373 files); name-collision table (OpenAI dots vs OpenDot vs CopilotKit/OpenDots vs composio-community/open-dot).
- Capability ladder `L0`–`L9`; 6-row UI benchmark table; 10 adoption decisions ("chat is the home surface", "right Workbench is user-controlled", boundaries not command spam).
- Later addenda: 2026-10-14 Manus 2.0 (`manusui/` 20 screenshots; "up to 20 concurrent subtasks" = vendor claim); 2026-10-15; 2026-10-24 11-product audit; 2026-10-24 LiteSPM check (**69,376 catalog listings / 26,554 one-command installable**; 50 MCP host targets, 81 skill targets); 2026-10-25 route terms (SIWC, Claude Code legal, OpenCode permissions "default allow"); 2026-10-25 delta (Dots on GPT-6 Astra, Chat+Work+Codex one app, Claude Sept 16 rollout, Copilot Cowork App Skill + Claude Fable 5.1).
- Repeated caveat: vendor pages prove presentation, not reliability; "no comparative timed study or packaged-build usability test was run".

### docs/ENVIRONMENTS.md (435)
- Runtime = `litecoworkd`; Environment = substrate for an Attempt. `EnvironmentSpec.lifetime = ATTEMPT|TASK_RETAINED|WORKSPACE_PERSISTENT`; `sharing_scope = ATTEMPT_PRIVATE|TASK_SHARED|COWORKER_PRIVATE|WORKSPACE_SHARED|USER_SHARED` (`USER_SHARED` unavailable in v1).
- `budget_ceiling`/`budget_enforcement_policy = REQUIRE_PROVIDER_ENFORCED|ALLOW_HOST_MONITORED` immutable after provisioning, no top-up/reset API; `LIMIT_REACHED` stops admission; replacement is a new Environment identity.
- `IsolationSpec`: filesystem `SHARED_READONLY|PRIVATE_COPY|WORKTREE|CONTAINER_FS|VM_FS`; process `HOST|NAMESPACE|CONTAINER|VM|REMOTE`; network `NONE|RESTRICTED|DEFAULT|CUSTOM`.
- `IsolationAttestation` + `QuiescenceObservation` Runtime-local only, excluded from events/snapshots/backups/prompts; every enforcement field must be `ENFORCED`; provider stop ack ≠ descendants stopped.
- `GitWorktree` isolates changes not reads/process trees; `LocalWorkspace` with `process: HOST` must not host an external file-writing agent; Linux qualification ≠ macOS/Windows.
- **Defect:** attestation sentence reads "current Runtime incarnation, provider archive/legacy-implementation-c7e9c05/version, OS build" — stray archive-path fragment where "provider version" belongs.

### docs/PRODUCT-CONTRACT-INDEX.md (38)
- Navigation + one-view contract; V1 = complete local desktop/laptop product; cloud/mobile/cross-device/remote Runtime post-V1; narrow-window responsiveness still V1.
- 8 bullets: Conversation front door; work durable when needed; subagent routes (Workspace default → Coworker inherit/narrow → Task pins at admission, no per-Task override); three-pane shell; one Routine execution model; outputs are product surfaces; LiteSPM inbuilt package manager.
- Authority map (7 topics) names primary + supporting owner; `COMPETITIVE-RESEARCH.md` declared non-normative.
- Rejected: universal "all models" endpoint; subscription sharing/credential copying/silent fallback; catalog presence ≠ launchable; interactive app is a typed immutable Artifact; cloud/mobile not required for V1; template/installed package cannot enable background work or widen Trust.

### docs/PRODUCT-STRATEGY.md (96)
- "active strategy derived from Revision 9 and the 30 October 2026 competitive engineering audit"; subordinate to `ARCHITECTURE.md`, `PRODUCT.md`, release contract, owner contracts.
- Moat = 7 items incl. **Agent Relay** (change lead agent without claiming private-session migration), Workspace continuity, portable capabilities (LiteSPM), evidence-bearing work, native fidelity, local ownership, one Workbench.
- 8-bullet promise; 10-item non-cloud capability floor; excludes competitor cloud computers, laptop-off continuation on LiteCowork Runtime, cloud-only storage, cross-device continuation.
- Two views kept separate: **V1 Core completeness** vs **competitive parity**; scope V1 / V1.1 (rewind/fork, voice) / V2 (hosted cloud Runtime, multi-device, mobile Dispatch, org RBAC).
- Baseline `c7e9c05ab09563a554515bdd7e0e7d86c99fffa0`; "not an assertion that the journeys are currently implemented or qualified".

### docs/REPOSITORY-DOCUMENTATION-AUDIT.md (160)
- Audit date **2026-10-31**, baseline `c7e9c05…`; executive finding: active tree is documentation-only, so all CODE/SERVED/INTEGRATED/QUALIFIED claims must be re-established.
- Corpus 137 active Markdown / 45,090 lines (root 6/1,614; docs 115/42,326; specs 10/966; `.specify` 6/184) plus historical archive 116/39,531 and ignored source archive 255/94,259.
- Fixed: **44 broken links** to former `implementation/` rewritten to `docs/archive/legacy-implementation-c7e9c05/`; ARCHITECTURE had two invariants numbered 15 → renumbered 1–31; `docs/README.md` no longer presents Revision 9 as homepage; LiteSPM endpoint authority unresolved (machine-service candidate ≠ public catalog host).
- §4.7 reconciliation: `operator-api.openapi.yaml` OpenAPI 3.1.0, **203 paths / 229 operations / 229 operationIds**, 2 target-only unmounted (`POST /v1/workspace-roots`, `PATCH /v1/workspace-roots/{id}`); `domain-event.schema.json` **148** types = `EVENTS.md`; `error-codes.schema.json` **131** codes all in `SCHEMAS.md`; **25** ADRs (0022/0023 Accepted, 0024 Accepted target, rest Accepted for vNext); 28-file SQLite chain replayed to **v28**, `foreign_key_check` zero rows, `quick_check` ok, **135 tables**, `user_version` 0.
- Worktree `feature/conversation-session-supervisor` @ `3052341473e964bececf7f34c9d45923848e5218`: test `conversation_activation_commits_session_turn_and_local_host_binding_atomically` passed 2026-10-31; earlier `cargo test -p storage-sqlite --locked` = **593** unit + **2** integration, **not rerun** after final redaction asserts; Linux Secret Service test ignored; validator blocked by missing `pydantic-core 2.46.5`.
- Still open: no served Conversation Send, no native supervisor caller, no watcher/content search, no recurring TriggerCoordinator, no Coworker route promotion; blockers B-01–B-05, B-07–B-10 (B-06 resolved via AC-017–AC-019); §4.9 gives the 16-domain current/target register.
- **Internal inconsistency:** §4.1 "863 relative Markdown links in 143 active files" vs §6/§7/§8 "137 files and 877 local links".

### docs/ROUTINES.md (224)
- Taxonomy: `Skill` how / `Routine` what / `Automation` when / `Task` one execution / `Workflow` external graph.
- `Routine` (`ACTIVE|ARCHIVED`) + immutable `RoutineRevision` (`objective_template`, `input_schema`, `required_capabilities`, `preferred_agent_binding_id`, `placement_preference`, `budget_ceiling`, `verification_policy`); text is data, never authority; delete blocked while referenced.
- `POST /v1/routines/{id}/run`: `Idempotency-Key`, exact revision + bounded `inputs`, non-null `conversation_id` rejected; creates one `READY` Task + `TaskSpecRevision(1)` and **no** Plan/Step/Session/Attempt/lease/Environment/Effect/grant; same-key retry replays the receipt.
- Input dialect ≤**64** top-level properties, no unbound/nested, `TEXT` (`minLength`/`maxLength`/`enum`) or exact `RESOURCE_REF`; empty `{}` valid.
- `RoutineHealth`: last **10** terminal Tasks, `<3` shows "Not enough runs yet", `drift_state = HEALTHY|WARNING|DRIFTED|UNKNOWN`, `DRIFTED` needs explicit incompatibility (`SKILL_DRIFT_DETECTED`), transient provider error = `WARNING` only.
- `AutomationRevision` pins one `RoutineRevision`; Routine edits never change existing Automations; `ACTIVE→ARCHIVED` needs enabled Automations paused/rebound; StandingResponsibility links but never owns or executes.

### docs/RUNTIME-MESH.md (744)
- Multiple `litecoworkd` instances "without turning the product into a distributed consensus system"; `RuntimeDescriptor` is a projection, `offers[]` is the single bounded inventory envelope.
- `DeviceIdentity`: Ed25519, `device_id` = `ed25519-sha256:` + hex SHA-256 of the raw 32-byte key; seed only in platform credential store (ADR-0021).
- New `RuntimeIncarnationId` per daemon start must register before presence/offers; duplicate identical registration idempotent, mismatch rejected. Pairing 6 steps, single-use tokens; Workspace-scoped revoke ≠ `revoke_runtime`.
- `ExecutionLease`/`FencingCredential`: domain-separated HMAC-SHA-256 over `(workspace_id, lease_id, task_id, step_id, attempt_id, runtime_id, runtime_incarnation_id, epoch)`; stores only `issuer_key_version` + `SHA-256(opaque_value)`; new incarnation cannot renew; expiry never proves an escaped process stopped.
- Replication: `origin_sequence` per Workspace/origin, `ReplicationReceipt.disposition = EVENT_STORED|POLICY_OMITTED`, `aggregate_state_ref` blob verified before apply, gap → `SNAPSHOT_REQUIRED`; HLC `(physical_ms, logical_counter, runtime_id)` presentation-only.
- Channel host: `clock_skew_margin_ms >= 30000`, `safe_reassign_after = expires_at + margin`; drain-proof counters derived by service, never caller-supplied; v1→v4 migration rejects ACTIVE-without-lease.
- Conflict rules per entity (append-only TaskSpecRevision/PlanRevision/ConversationMessage, ArtifactVersion `STALE_VERSION`, first terminal Approval wins); "no generic last-writer-wins". Hub failure: no auto election; backup complete point-in-time, **no incremental chains**.

### docs/STATE-MACHINES.md (1427)
- "State transitions are domain rules, not UI conventions": one legal transition + one event per transaction; ~50-row aggregate→owner table at the end.
- `Workspace ACTIVE→ARCHIVED` requires full quiescence else `WORKSPACE_NOT_QUIESCENT`; `default_agent_binding_id`, `lead_eligible`, `primary_coworker_id`, `SELECTED_FOLDERS` are ACTIVE→ACTIVE versioned updates with named events.
- `Task`: `READY|RUNNING|VERIFYING|COMPLETED|FAILED|BLOCKED|NEEDS_USER|INCOMPLETE|WAITING_USER|PAUSE_REQUESTED|PAUSED|CANCEL_REQUESTED|CANCELLED`; **no** `VERIFYING→RUNNING`; `LeadFailoverPolicy ASK|ALLOW_LISTED` + `max_lead_changes` + `task.lead_agent.changed.v1` (`cause=OWNER_REQUEST|POLICY_FAILOVER`).
- `AgentSession STARTING→ACTIVE→INTERRUPTING|CLOSING|LOST`; `AgentHostInstance STARTING→READY↔BUSY→DEGRADED|FAILED→STOPPING→STOPPED`, `active_session_count` derived.
- `ConversationTurn OPEN→RUNNING→WAITING_USER→WAITING_DEPENDENCY→RUNNING`, `retry_ordinal`, `conversation.turn.created.v2` pins binding, `conversation.turn.resumed.v1`; retry only from `FAILED`.
- `Effect PROPOSED→STARTED→ACKNOWLEDGED→OBSERVED→VERIFIED` + `AMBIGUOUS→RECONCILING`. Explicit code gap: the SQLite writer **rejects transitions to `STARTED`** (incl. reconciliation retries) until the atomic Trust/Invocation/ApprovalUse dispatch exists.
- Also `Environment`, `EnvironmentControlLease`, `CapabilityInvocation` (`INPUT_REQUIRED`, `AMBIGUOUS`), `ProviderInputBinding`, `ProviderCircuit`, `BudgetReservation`, `SkillProposal`, `ResourceUploadSession`, `Artifact`, `Interactive Artifact`, `VerificationRun`, `Approval`+`ApprovalUse`, `UserRequest`/`NotificationDelivery`, `Resource/WorkspaceRoot/ContextDocument` (`ACTIVE↔REVOKED→DELETION_PENDING→DELETED`), `Automation`/`Occurrence` (`claim_epoch` vs `version`), `ExecutionLease`, `Handoff`, `Connection`, `ChannelEventReceipt`, `RichPresentation ABSENT→PUBLISHED`.
- Target-only, "MUST NOT appear in the shipped API/event allowlist": `StandingResponsibility`, `MemoryCandidate`, `AuthAttempt`, `LearningCandidate`.

### docs/TRUST.md (322)
- `Principal.kind = USER|SERVICE|RUNTIME|AGENT|CHANNEL_IDENTITY`; `PolicyDecision ALLOW|DENY|REQUIRE_APPROVAL`; `SecretRef.placement = LOCAL_ONLY|CLOUD_AVAILABLE|RUNTIME_BOUND|EXTERNAL_AGENT_OWNED`; `SecretLease ACTIVE|REVOKED|EXPIRED`.
- `GrantRequest.scope` exactly one of `CONVERSATION{conversation_id}`, `TASK_PLANNING{task_id}`, `ATTEMPT_EXECUTION{task_id,attempt_id}`; conversation/planning grants read-only, no `SecretRef`s, expire; narrow only.
- 10-step authorization order; **default DENY when a required fact is unknown**; `ApprovalRequest` persists `target_ref`, `scope_digest`, `action_digest`; one `ApprovalUse` per `approval_id`, second use = `APPROVAL_ALREADY_CONSUMED`; unused approval dead after `CANCEL_REQUESTED`.
- `AssuranceLevel = VIEW_ONLY|STEER_SAFE|APPROVE_SAFE|APPROVE_SENSITIVE|LOCAL_STRONG`; channel `RESPOND` needs ≥`STEER_SAFE` + granted `RESPOND`; v1 sender must be the Workspace owner.
- `DelegationProfile` restricts, never authorizes; child Attempts get fresh grants; parent grants/approvals/leases/native credentials never flow; preflight never pre-authorizes an unknown digest.
- Coworker defaults `STANDARD_TRUST_POLICY|REQUIRE_OWNER_APPROVAL|HANDOFF_TO_OWNER` may only add friction; `ActionBatch` members independent.
- `SecretRef.provider_ref` opaque; `UserRequest` is never a credential channel; MCP URL-mode URLs stay encrypted in the source Runtime. 11-item threat model.

### docs/V1-INTEGRATED-TERMINAL-LLD.md (131)
- "normative TARGET, not a shipped terminal implementation"; Workbench hosts UI only, "does not own a privileged shell".
- Profiles: Windows `pwsh`/Windows PowerShell/`cmd.exe`/WSL/Git Bash/MSYS2/Cygwin; Linux `bash|zsh|fish|sh/dash|pwsh`/container; macOS `zsh|bash|fish|pwsh`; custom exact executable + argv.
- `TerminalProfile.defaultCwdPolicy = WORKSPACE_ROOT|TASK_WORKTREE|ENVIRONMENT|HOME|CUSTOM`; `TerminalSessionProjection.ownerKind = USER_MANUAL|TASK_ATTEMPT|PROVIDER_PRIVATE`, `shellProcessState = STARTING|ACTIVE|EXITED|RECONNECTING|LOST|DETACHED|STOPPING`; `TerminalOutputFrame.kind = DATA|RESIZE_ACK|EXIT|HEALTH|GAP`.
- PTY: Linux/macOS PTY, Windows **ConPTY**; argv-array spawn, process-group ownership, one broker per stdin/resize; reconnect only same incarnation else `LOST`; "command replay is never automatic".
- OSC 633 shell integration; escape markers are "UI hints, NEVER proof of Task/Effect success"; 11 proposed functions (`createTerminalSession`, `requestTerminalTakeover`, `stopTerminalProcess`, `deriveCommandMarks`…) unpromoted; 10-row scenario table; VS Code sources.

### docs/V1-UI-SCREEN-CONTRACT.md (130)
- Normative UX **TARGET**; 2026-10-31 reset provenance: executable UI tree gone, 10-22/24/25 "current implementation" notes are pre-reset history.
- Historical gaps named: turn route "always returns 503" with disabled Send; no conversation mutate route; memory routes unimplemented; work controls target-only; no Browser/Computer/Terminal provider; no PDF/Office generation; no streamed frames; no global search.
- 10-surface sidebar table (New Chat, Recent Chats & Search, Coworkers, Needs You, Work, Automations, Library, Discover, Agents, Settings); three-pane shell, dock/undock/collapse, drawer+focus trap on narrow widths, layout per Workspace only.
- Composer `[+][Agent][Model?][Reasoning?][Native / @][Send]`; Agent explicit when >1 lead-eligible; conflicts invalidated, never silently dropped; `Run as Task`/`Schedule`/`Save as Routine` are reviewed flows.
- Discover: LiteSPM authoritative; tabs Explore / Installed-native / Managed / Connections; labels `installable|discovery-only|unsupported for this host|unavailable/unknown`; plan-first, agent cannot self-approve; Installed/Authenticated/Assigned/Granted/Activated/Invoked distinct.
- Artifact matrix Text/Code/PDF/DOCX-XLSX-PPTX/Image/Interactive app, each requiring the named real capability.
- Starter flows **Weekday codebase review** (09:00 weekdays) and **Weekly project briefing** (Mondays); save creates a **paused** Automation; due occurrence yields one `READY` Task only. 12-state global matrix; 2026-10-25 Chat→Task promotion addendum.

### docs/WORLD-RESOURCES.md (529)
- World Index = permission-scoped factual catalog, not memory/semantic RAG/OCR/whole-machine monitor; fields owned by `DATA-MODEL.md`.
- Served: `WorkspaceRoot` via native folder selection, list/pause/resume/revoke, startup identity revalidation, **Linux-only** metadata census; `GET /v1/resources/search` is not root-aware; root-aware content search + watcher are TARGET.
- Scan limits: ≤**1,000** entries, depth **32**, **1 MiB** locator bytes, **8 s** deadline (stop at **7 s**), ≤**2** concurrent scans, `openat2` beneath-root/no-symlink; metadata only, no `ResourceRevision`; partial scan never tombstones.
- `ResourceRef` URI `resource://<workspace-id>/<resource-id>@<revision-id?>`; `locator_ref_id` joins Runtime-local binding; only `STRONG`/`PROVIDER_SCOPED` produce `identity_digest`; `WEAK` never dedupes.
- Revision DAG acyclic; `current_revision_id` only with one head; multi-head = `CONFLICTED`, unpinned = `RESOURCE_CONFLICT`; upload needs `If-Match` + full head set; merge names every head.
- Text index: ≤**1 MiB**, ≤**20,000** terms of ≤**128** chars, encrypted `RESOURCE_INDEX`, HMAC-SHA256 tokens only; ≤**200** snapshots / **64 MiB** per request; `NOT_INDEXABLE` on bad input; no plaintext fallback. Legacy `ON_DEMAND_CONTENT`: 20 candidates/1 MiB/8 MiB.
- Windows locator `windows-path-u16le-b64:` ≤**16,896** UTF-16 code units; overlap admission across ACTIVE/PAUSED/UNAVAILABLE, freed only by REVOKED; focused tests pass on **Windows 11 build 26200** but packaged startup/scan/keyring unqualified; macOS/Windows scans fail closed.
- ZIP: `GET /v1/capabilities/zip-intake` + Linux-only `POST /v1/resources/{resource_id}/archive/preview` (Bubblewrap Python CLI), metadata only, ≤2 previews else `429 RATE_LIMITED`; extraction deferred.

### docs/adr/0014-versioned-worker-profiles-and-bounded-selection.md (26)
- ADR-0014 "Version Worker Profiles and Bound Selection" — **Accepted for vNext**, 2026-10-05.
- Decision: worker roles are immutable `DelegationProfile` revisions on an `AgentBinding`; discovery, binding authorization, lead eligibility and worker enablement stay distinct; **filter hard constraints first, then rank** under explicit policy.
- "Unknown cost is not zero"; verification + bounded new-Attempt escalation decide adequacy; explicit `REQUIRE` never substitutes.
- Consequence: one installation can be lead + N workers; model/session options adapter-owned; performance projections grant no authority.

### docs/adr/0017-deadline-sensitive-is-best-effort.md (25)
- ADR-0017 "Deadline-Sensitive Work Is Best-Effort" — **Accepted for vNext**, 2026-10-05.
- Decision: `DEADLINE_SENSITIVE` buys prioritized placement + preflight only; **hard realtime is not a product contract**.
- Fallback ladder structured API → structured browser → accessibility browser → screen interaction, only for the same authorized operation; `ActionBatch` keeps per-operation Effect/Evidence identity.
- Consequence: UI reports readiness/blockers honestly, cannot promise deadline completion; unsuitable fallback stops for review.

### docs/adr/0024-optional-chat-first-coworkers.md (35)
- ADR-0024 "Optional chat-first Coworkers with bounded proactive authority" — **Accepted target, 2026-10-10. Implementation, schema migration and owner acceptance remain pending.**
- 8 decisions: optional Workspace-scoped identity, no forced default; Conversation primary; native reasoning native; unlimited *configured* connections with progressive activation; `StandingResponsibility` groups version-pinned revisions (`TriggerCoordinator` due, `TaskService` execute, `Trust` authorize); automatic scoped memory only from eligible conversations; desktop continuity only with enabled background Runtime; versioned rollout.
- Rejected: mandatory Coworker, model-controlled "Goal Keeper", unlimited MCP processes, per-tool permission wizard, memory from every connector, faking cloud/native output.
- Migration: `Workspace.primary_coworker_id` kept as legacy Task-origin preference; Conversation rows migrate with **nullable owner**; expected-version + RequestId + same-Workspace FKs + durable outbox; "No active provider dispatch is enabled merely by this ADR".

### docs/adr/0025-agent-module-registry-and-native-configuration.md (59)
- ADR-0025 "Agent Modules Own Agent-Specific Lifecycle and Native Configuration" — **Accepted for vNext**, 2026-10-10.
- Decision: each agent is an independently versioned `AgentModule` = `AgentLifecycleAdapter` + `AgentAdapter` + optional `AgentCapabilityBridge`; everything agent-specific projected through a time-bounded `AgentControlDescriptor`.
- "LiteCowork does not create a global provider/model catalog"; secret-bearing config is native-owned or written directly to `SecretStore`, ordinary `AgentBinding` config stays non-secret.
- ACP Registry metadata is discovery only: "registry presence never proves installation, authentication, capability support, readiness or authorization"; cannot supply commands or privileged auth actions.
- Consequence: original-harness capability retained; updates isolate to one module + fixtures; Core stays brand-agnostic; install/auth/config readiness separate; per-agent real-provider qualification required.

## Cross-cutting observations
- Uniform doc shape: normative target + explicit served/current boundary + dated addenda (2026-10-12, 10-24, 10-25, 10-31). The 2026-10-31 reset header is the only reliable currency cutoff.
- Most repeated invariant: **discovery/listing ≠ readiness**, and installed ≠ connected ≠ authenticated ≠ assigned ≠ granted ≠ activated ≠ invoked (AGENT-CONTROL §2, CONTRACT-INDEX, UI contract Discover, TRUST, WORLD-RESOURCES, audit §4.9).
- Second: **opacity of secrets/handles** — secret bytes, native session handles, PTY/OS handles, provider URLs, ingress cursors, fencing credentials, locators stay Runtime-local, incarnation-scoped, excluded from events/snapshots/projections/logs/backups.
- Third: **epoch/fence everywhere** — `ExecutionLease.epoch`, EnvironmentControlLease owner epoch, `host_epoch`, `claim_epoch`, `control_version`; stale claimants rejected, never resolved by wall clock.
- Numeric contracts spread per domain (30 s skew margin, 1,000 entries/32 depth/8 s scan, 64 props/20,000 terms/64 MiB, 148 events/131 codes/229 operations/135 tables). Schema changes must update the audit's reconciliation counts.
- Acceptance ≠ implementation is deliberate: 25 ADRs, 0024 "Accepted target" pending; STATE-MACHINES records the SQLite Effect writer still rejecting `STARTED`.
- Duplication risk: agent install/auth/descriptor machines stated in both AGENT-CONTROL §8 and STATE-MACHINES; channel host drain/fence stated at length in CHANNELS, RUNTIME-MESH and a third time in STATE-MACHINES — consistent today, three copies of one authority.
- **Defect list for owner:** (1) ENVIRONMENTS stray archive path in `IsolationAttestation`; (2) audit link-count drift 143/863 vs 137/877; (3) COMPETITIVE-RESEARCH title date older than its 2026-11-01 section; (4) V1-UI-SCREEN-CONTRACT present-tense 10-24/25 claims coexist with the 10-31 "no UI tree" statement; (5) ROUTINES describes `POST /v1/routines/{id}/run` with served-level wording without a status label, while the audit calls Routines partially served.
- No file contradicts the authority order; ADR-0024/0025 declare themselves subordinate to `ARCHITECTURE.md` and owner contracts.

## Coverage check
- All 18 listed files read to their final line. `STATE-MACHINES.md` (1,427) and `COMPETITIVE-RESEARCH.md` (608) paged in overlapping ranges (1-440/441-880/881-1320/1321-end; 1-300/299-608).
- `AGENT-CONTROL.md` (1,313) read in full; first pass hit an output-size truncation inside the last section, tail re-read from line 1120 and heading list (`## 1`–`## 19` + 4 addenda) confirms no skipped section.
- No file skipped or unreadable. Line/byte totals match the plan (6,576 lines, 455,100 bytes).
