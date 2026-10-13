# Every Markdown File â€” Current Working Tree Audit (2026-10-12)

Exhaustive file and line ingestion, digest, heading, code fence and literal relative Markdown link check. NOT a manual semantic proof or runtime qualification. Flags indicate review candidates, not proven defects.

- Files: 142
- Lines: 53544
- Bytes: 3528376
- Broken relative links: 0
- Unclosed fences: 0

## Per-file inventory

| File | Lines | Hash prefix | Main heading | Review flags |
|---|---:|---|---|---:|
| AGENTS.md | 269 | 15d0cb622c61 | LiteCowork Repository Instructions | 2 |
| apps/litecowork-ui/README.md | 61 | 124fd9fb8b92 | LiteCowork desktop shell | 0 |
| apps/litecowork-ui/src/artifacts/README.md | 155 | bed0b3660621 | Artifact Library and Workbench source boundary | 0 |
| apps/litecowork-ui/src/presentation/README.md | 32 | c868572e9239 | Desktop Presentation Runtime | 0 |
| ARCHITECTURE.md | 780 | 5f6c95f1c1f1 | LiteCowork Architecture | 0 |
| assets/host-instructions/presentation-policy.md | 12 | 7a598b60ebb7 | LiteCowork Presentation Policy | 0 |
| assets/host-skills/rich-response-design/SKILL.md | 111 | 7f6287aab3db | Rich Response Design | 0 |
| capabilities/zip_intake/README.md | 104 | c8bbbbd824a5 | Bounded ZIP intake capability slice | 0 |
| CONTRIBUTING.md | 38 | 709b7f2d6cad | Contributing to LiteCowork | 0 |
| crates/domain-responsibility/README.md | 141 | 0e9811b3c3f5 | Coworker, Goal, and Automation domain boundary | 0 |
| docs/adr/0001-durable-task-core.md | 23 | 8fe458784bec | ADR-0001: Durable Task Core Around External Agents | 0 |
| docs/adr/0002-task-and-attempt-identity.md | 21 | 83d4b175418a | ADR-0002: A Task Continues Through New Attempts | 0 |
| docs/adr/0003-litespm-is-independent.md | 30 | e554a0d25448 | ADR-0003: Keep LiteSPM Independent | 0 |
| docs/adr/0004-runtime-and-environment-are-distinct.md | 21 | 359ffb5d539b | ADR-0004: Runtime and Environment Are Separate | 0 |
| docs/adr/0005-replicate-domain-state-not-databases.md | 21 | 3428312c13b9 | ADR-0005: Replicate Domain State, Not Database Files | 0 |
| docs/adr/0006-conversation-task-separation.md | 16 | 92a5f0a08431 | ADR-0006: Conversation and Task Are Separate | 0 |
| docs/adr/0007-reported-observed-verified.md | 16 | 8c15daf850e7 | ADR-0007: Preserve Evidence Levels | 0 |
| docs/adr/0008-no-generic-provider-layer.md | 23 | 6c1f92c12c72 | ADR-0008: Keep Protocol Boundaries Explicit | 0 |
| docs/adr/0009-attempt-free-lead-planning.md | 16 | 6041ee56f87a | ADR-0009: Initial Planning Does Not Require an Attempt | 0 |
| docs/adr/0010-revision-independent-occurrences.md | 16 | d5becc7f733d | ADR-0010: Deduplicate Automation Occurrences Across Revisions | 0 |
| docs/adr/0011-atomic-plan-acceptance.md | 30 | 6f9b78226189 | ADR-0011: Accept Plans and Materialize Steps Atomically | 0 |
| docs/adr/0012-native-harness-integrity.md | 35 | 90b1ffcd4df9 | ADR-0012: Preserve Native Agent Harnesses | 0 |
| docs/adr/0013-host-delegation-uses-accepted-plan-steps.md | 25 | fec8da7253f7 | ADR-0013: Host Delegation Uses Accepted Plan Steps | 0 |
| docs/adr/0014-versioned-worker-profiles-and-bounded-selection.md | 26 | 6d56c993d253 | ADR-0014: Version Worker Profiles and Bound Selection | 0 |
| docs/adr/0015-warmth-is-operational-and-sharing-is-separate.md | 24 | d33dad274f71 | ADR-0015: Warmth Is Operational and Sharing Is Separate | 0 |
| docs/adr/0016-coworker-goal-and-suggestion-authority-boundary.md | 25 | de6aefd4ae3c | ADR-0016: Coworkers, Goals, and Suggestions Do Not Own Execution | 0 |
| docs/adr/0017-deadline-sensitive-is-best-effort.md | 25 | e75f3980fa2f | ADR-0017: Deadline-Sensitive Work Is Best-Effort | 0 |
| docs/adr/0018-context-content-is-resource-backed-and-provider-pluggable.md | 29 | 670ddfc6d2f5 | ADR-0018: Context Is Resource-Backed and Provider-Pluggable | 0 |
| docs/adr/0019-credential-egress-and-audit-boundaries.md | 25 | 5b93f069c10c | ADR-0019: Separate Policy, Approval, Credentials, Egress, and Audâ€¦ | 0 |
| docs/adr/0020-runtime-identity-is-installation-scoped.md | 50 | d87e26cf0229 | ADR-0020: Runtime Identity Is Installation-Scoped | 0 |
| docs/adr/0021-device-signing-identity.md | 54 | 2b51dfb39fb6 | ADR-0021: Ed25519 Device Signing Identity | 0 |
| docs/adr/0022-rich-presentation-is-optional.md | 33 | 7104d5c575e3 | ADR-0022: Rich Presentation is an optional rendering enhancement | 0 |
| docs/adr/0023-host-guidance-is-not-capability-authority.md | 27 | 837ea5dd936b | ADR-0023: Built-in Host guidance is not capability authority | 0 |
| docs/adr/0024-optional-chat-first-coworkers.md | 35 | 5a6cde423b5c | ADR 0024 — Optional chat-first Coworkers with bounded proactive aâ€¦ | 0 |
| docs/adr/0025-agent-module-registry-and-native-configuration.md | 59 | 79ff69084cb0 | ADR-0025: Agent Modules Own Agent-Specific Lifecycle and Native Câ€¦ | 0 |
| docs/adr/README.md | 28 | 5fbbf6d2842a | Architecture Decision Records | 0 |
| docs/AGENT-CONTROL.md | 1171 | 003132c12568 | Agent Registry, Configuration, Authentication, and Native Surfaceâ€¦ | 1 |
| docs/AGENT-FABRIC.md | 414 | d3c212797a1d | Agent Fabric LLD | 0 |
| docs/API.md | 1746 | 96253a808f34 | Operator API | 2 |
| docs/ARTIFACTS-EVIDENCE.md | 338 | 5e41fe2127e1 | Artifacts, Effects, Evidence and Verification | 0 |
| docs/AUTOMATION.md | 491 | 0beb58238ce6 | Automation LLD | 0 |
| docs/BENCHMARKS.md | 683 | 608779fb8632 | LiteCowork Real-Work Benchmark | 2 |
| docs/CAPABILITY-FABRIC.md | 375 | 92edd06d3098 | Capability Fabric | 0 |
| docs/CAPABILITY-INVOCATIONS.md | 467 | 5c436b77b83d | Durable Capability Invocations | 0 |
| docs/CHANNELS.md | 274 | 4e497119011f | Messaging Channels LLD | 0 |
| docs/CHROME-BROWSER-LLD.md | 175 | d7ca36c12160 | Chrome Browser, Profile Connection, Side Browser and Takeover LLDâ€¦ | 0 |
| docs/COMPETITIVE-RESEARCH.md | 236 | b7ad0c8efcb6 | Competitive Product Research (Snapshot: 2026-10-10; earlier checkâ€¦ | 0 |
| docs/CONNECTORS-WORKFLOWS.md | 825 | 70815aa25dbd | Connectors, Authentication, Workflow Backends, and Event Relay | 0 |
| docs/CONTEXT.md | 206 | ba035836e7e3 | Workspace and Agent Context | 0 |
| docs/COVERAGE-MATRIX.md | 144 | 32f7e58bed02 | Architecture Coverage Matrix | 0 |
| docs/COWORKER-FLOWS.md | 166 | 31d5afa31193 | Coworker target interaction flows, failure paths and acceptance | 0 |
| docs/COWORKERS-TARGET.md | 448 | fa472ad64a19 | LiteCowork Coworkers — Consolidated Product, UX & Architecture Spâ€¦ | 0 |
| docs/DATA-MODEL.md | 2546 | dedd484d1c97 | Data Model | 0 |
| docs/DELEGATION.md | 799 | c144bbd07806 | Delegation and Worker Selection | 0 |
| docs/DESIGN-SYSTEM.md | 456 | 4d6c647ce42d | Design System | 0 |
| docs/DESKTOP-SHELL-LLD.md | 252 | 9e59154fd57e | Desktop Shell, Settings, Sidebars, Interaction, Animation LLD (V1â€¦ | 0 |
| docs/ENVIRONMENTS.md | 426 | 8cc145be07ba | Environment Fabric LLD | 0 |
| docs/EVENTS.md | 855 | 1332bbf56349 | Event Journal and Event Schemas | 0 |
| docs/EXPERIENCE.md | 1087 | a9c7f80e3fd1 | Experience Architecture | 0 |
| docs/FAILURE-RECOVERY.md | 207 | b99f325487fd | Failure and Recovery | 0 |
| docs/FLOWS.md | 4421 | d5d222203016 | End-to-End Flows | 0 |
| docs/HOST-GUIDANCE.md | 138 | 39a1fd4833e3 | Host Guidance and Built-in Skills | 0 |
| docs/IMPLEMENTATION.md | 226 | 01c86f6c01f0 | Implementation Architecture | 0 |
| docs/LEARNING.md | 641 | 57a239d88d64 | Coworker Learning, Memory, Instruction, and Skill Revision | 0 |
| docs/LOCAL-OPERATOR-IPC.md | 420 | bef5225f50d2 | Local Operator IPC | 0 |
| docs/MOTION.md | 256 | 95016d7f546f | Motion and Transition System | 0 |
| docs/NATIVE-COMPOSER-LLD.md | 178 | c582a2dfce66 | Native Chat, Composer, and Agent-Parity LLD (V1) | 0 |
| docs/NETWORK-SECURITY.md | 298 | ae491df0f42c | Network, IPC, and Resource Security | 0 |
| docs/OBSERVABILITY.md | 237 | dcbe4355adf0 | Observability Architecture | 0 |
| docs/PRESENTATION-RUNTIME.md | 505 | aea8b72f539a | Presentation Runtime | 0 |
| docs/PRODUCT.md | 236 | bc75bc1dff7b | Product Contract | 0 |
| docs/PROTOCOLS.md | 182 | c21f8ea7b8fd | Protocol Contracts | 0 |
| docs/REFERENCE-IMPLEMENTATIONS.md | 318 | 48e6434cbf0d | Reference Implementations and Upstream Design Sources | 1 |
| docs/RENDERING-CONFORMANCE-LLD.md | 119 | 113baa60f8c2 | Central Conversation Rendering, Workbench Viewers and Native Outpâ€¦ | 0 |
| docs/RESPONSIBILITIES.md | 483 | 54e013870023 | Coworkers, Goals, Suggestions, and Personal Context | 0 |
| docs/RICH-RESPONSE.md | 290 | 83bbe1009319 | Rich Response Contract | 0 |
| docs/ROUTINES.md | 221 | 7f5b05dfb2fa | Routines | 0 |
| docs/RUNTIME-LIFECYCLE.md | 576 | 33f0a0f0bd70 | Runtime and Worker Lifecycle | 0 |
| docs/RUNTIME-MESH.md | 744 | 2c05757095d5 | Runtime Mesh LLD | 0 |
| docs/SCHEMAS.md | 1437 | 7760050dad47 | Shared Schemas and Error Taxonomy | 1 |
| docs/SECURITY.md | 174 | fa030468484c | Security Architecture | 0 |
| docs/SERVICES.md | 1542 | 440307f7ee71 | Services and Ownership | 0 |
| docs/SOURCE-RECONCILIATION.md | 148 | 3ebcdf8b1976 | Architecture Source Reconciliation | 0 |
| docs/STATE-MACHINES.md | 1373 | ac343c56eb2d | State Machines and Transition Ownership | 0 |
| docs/STORAGE.md | 1054 | f945a94263a0 | Storage LLD | 0 |
| docs/TASK-RUNTIME.md | 721 | 0a8187f9beaa | Task Runtime LLD | 0 |
| docs/TESTING.md | 454 | 50174de3ae35 | Test Architecture | 1 |
| docs/TRUST.md | 317 | ab477965b658 | Trust, Authorization, Approvals and Secrets | 0 |
| docs/V1-DESIGN-INDEX.md | 72 | 53bb819d2c82 | LiteCowork Desktop V1 — Detailed Design Authority Index | 0 |
| docs/V1-FUNCTION-FILE-CATALOG.md | 366 | cb93e1559a5a | V1 Implementation Function, File and Contract Catalog | 0 |
| docs/V1-WORKFLOW-EVENT-LLD.md | 112 | c9b3ce351b1c | V1 Workflow, Cross-Agent Handoff, Automation, Events and Recoveryâ€¦ | 0 |
| docs/WORLD-RESOURCES.md | 385 | 93457c6de7cd | Workspace Resources and Deterministic Search | 0 |
| GLOSSARY.md | 292 | e76323588a46 | LiteCowork Glossary | 0 |
| implementation/AGENT-MODULE-AUDIT.md | 524 | bad51ed44a90 | Agent Module Current-Source Audit and Refactor Map | 0 |
| implementation/ARCHITECTURE-COVERAGE.md | 167 | 525153651755 | Architecture and contract coverage audit | 0 |
| implementation/AUDIT.md | 143 | 10432f8e586a | Current architecture audit and development refinements | 0 |
| implementation/COVERAGE.md | 33 | de61a91ec685 | Coverage map | 0 |
| implementation/CURRENT-RUN.md | 6223 | 2c25d363a3b0 | Current run and agent handoff | 4 |
| implementation/DOC-AUDIT-2026-10-10.md | 173 | ff27abd7e23a | Full Markdown Architecture Audit — 2026-10-10 | 0 |
| implementation/epics/E01.md | 666 | bbbabe7dbabd | E01 — Development foundation | 0 |
| implementation/epics/E02.md | 416 | 3ea7b052fefd | E02 — Desktop shell and resources | 0 |
| implementation/epics/E03.md | 424 | 311b14562c39 | E03 — Full native agent and durable work | 0 |
| implementation/epics/E04.md | 134 | 27e02ac37fb0 | E04 — Trust, capabilities and effects | 0 |
| implementation/epics/E05.md | 127 | 7a25662952e3 | E05 — Heterogeneous workforce and cost | 0 |
| implementation/epics/E06.md | 111 | eaa544603998 | E06 — Documents, RAG and local models | 0 |
| implementation/epics/E07.md | 217 | 64dcf285b70e | E07 — Environments, browser and office work | 0 |
| implementation/epics/E08.md | 466 | 8fa5ab57b5f0 | E08 — Coworker, goals, context and product UX | 0 |
| implementation/epics/E09.md | 207 | ee6307af4afe | E09 — Responsibilities, scheduling and notifications | 0 |
| implementation/epics/E10.md | 62 | 3bce19610b78 | E10 — Reusable skills and capability UI | 0 |
| implementation/epics/E11.md | 98 | 2e45acbcb389 | E11 — Cloud continuation and messaging | 0 |
| implementation/epics/E12.md | 44 | 0519a349a2ae | E12 — Remote Runtime | 0 |
| implementation/epics/E13.md | 84 | 27ee7ea1a15e | E13 — Desktop/local V1 production release qualification | 0 |
| implementation/licensingandpkacgingresearch.md | 196 | 2e29b84d4b59 | Licensing and Packaging Research | 0 |
| implementation/PROCESS.md | 95 | a4c0d040f965 | Agile process for coding agents and one reviewer | 0 |
| implementation/RAG.md | 120 | eebd30eaeeb7 | File, folder, ZIP and local-model delivery | 0 |
| implementation/README.md | 69 | dbecc67ef2e3 | LiteCowork implementation plan | 1 |
| implementation/RELEASE.md | 73 | a627516f78cf | Production operations and release gates | 0 |
| implementation/reviews/E01-S01.md | 75 | e87fb6748601 | E01-S01 implementation review packet | 0 |
| implementation/reviews/E01-S02.md | 433 | f1ef62e54d7f | E01-S02 review packet — transactional persistence | 0 |
| implementation/reviews/E01-S03.md | 603 | 22dc0b9fa230 | E01-S03 review packet — bounded Runtime shutdown | 0 |
| implementation/reviews/E01-S04.md | 1312 | 5036ee36bc18 | E01-S04 review packet — authenticated Operator boundary | 1 |
| implementation/reviews/E02-S01.md | 149 | 99ee608d2ead | E02-S01 review packet — responsive desktop shell slice | 0 |
| implementation/reviews/E02-S02.md | 612 | c8e0e0cf6277 | E02-S02 review packet — Workspace policy and instructions | 0 |
| implementation/reviews/SP04-HANDOVER-REVIEW.md | 110 | 8917518969a7 | SP04 handover review and production follow-through | 0 |
| implementation/ROADMAP.md | 171 | a26e25378894 | Delivery roadmap | 8 |
| implementation/SCOPE.md | 128 | 1d7cc4026807 | Release scope | 0 |
| implementation/SOURCES.md | 134 | 2c70d7a44f46 | Research sources and repository reference guide | 0 |
| implementation/spikes/SP02.md | 174 | 70efa57d9d45 | SP02 — SQLite adapter qualification | 0 |
| implementation/spikes/sp02-driver-compare/README.md | 62 | 12af8c71a036 | SP02 SQLite driver comparison | 0 |
| implementation/spikes/SP03.md | 100 | 956e9125e1a7 | SP03 — Authenticated local Operator transport | 0 |
| implementation/spikes/SP04.md | 474 | dc154675dbcf | SP04 — Native coding-agent handover and safe-switch PoC | 0 |
| implementation/spikes/SP05.md | 119 | 5a3dfa04c67a | SP05 — Local model provider and native-harness qualification | 0 |
| implementation/STACK.md | 140 | c866f1d4b5f2 | Stack recommendations and qualification decisions | 0 |
| implementation/TESTING.md | 174 | d5276bf6f0a9 | Tests and acceptance evidence | 0 |
| implementation/UI.md | 772 | 705eea3e3a31 | Desktop UI delivery, control inventory and interaction acceptance | 1 |
| implementation/V1-AUDIT-INVENTORY.md | 160 | be17cf9ace43 | V1 First-Party Markdown Mechanical Inventory | 0 |
| implementation/V1-CONTRACT-CHECK.md | 40 | 31e7b35901a9 | V1 Contracts vs Actual OpenAPI and Backlog | 0 |
| implementation/V1-DESIGN-COVERAGE.md | 226 | d269d2d96e60 | V1 Implementation-Ready Design Coverage and Qualification Matrix | 0 |
| implementation/V1-DESIGN-HANDOFF-2026-10-12.md | 67 | 0386d06ed906 | V1 Detailed Design Handoff — 2026-10-12 | 0 |
| implementation/V1-SEMANTIC-MD-FINDINGS-2026-10-12.md | 54 | 83566d82e309 | LiteCowork V1 — Cross-Document Semantic Markdown Findings | 0 |
| implementation/WORKFLOWS.md | 68 | 07fd4d0dbc1c | Cowork replacement assessment | 0 |
| README.md | 156 | 5c54bf5c560f | LiteCowork | 0 |

## Broken relative links
No literal-link missing targets.

## Unclosed code fences
No unclosed fences.

## Candidate terms needing semantic review
- AGENTS.md: 125: - Prefer deep typed interfaces and real adapters over broad placeholder layers. / 247: - Contract freeze first: agree request/response, status/event and reducer fixture, then immediately implement the real producer AN
- docs/AGENT-CONTROL.md: 473: its configuration is no longer an untyped empty placeholder.
- docs/API.md: 1352: digest. Missing comparison data, a revisionless Resource, `UNKNOWN`, or `PLACEHOLDER` / 1359: are `AVAILABLE`, `OFFLINE`, `PLACEHOLDER`, `REVOKED`, `UNKNOWN`, or `UNAVAILABLE`. This is
- docs/BENCHMARKS.md: 8: Read school/work email, identify obligations, update calendar/todo through capabilities, produce daily brief. Test duplicate-messa / 519: - streamed/cloud placeholder file not materialized
- docs/REFERENCE-IMPLEMENTATIONS.md: 43: ; `docs/architecture.md` ; https://github.com/CopilotKit/OpenIntelligentUI/blob/f6e4388b26a64b9a0714943b08a1ce622b924eec/docs/arch
- docs/SCHEMAS.md: 342: ResourceLocationAvailability = AVAILABLE ; OFFLINE ; PLACEHOLDER ; REVOKED ; UNKNOWN ; UNAVAILABLE
- docs/TESTING.md: 333: never a placeholder for missing telemetry. Exact/estimated COST observations and cost
- implementation/CURRENT-RUN.md: 1786: `implementation/ROADMAP.md` now contains an evidence-based done/not-done matrix. Important gaps remain NOT DONE: provider-backed c / 3890: registration. There is no placeholder-key path; missing or mismatched credentials fail / 4824: no such shortcut or placeholder UI was added. This source-only audit ran no checks. / 4855: available for original-byte export; no in-process parser shortcut or placeholder extraction
- implementation/README.md: 42: The reviewed user requirements are preserved in [Coworker target](../docs/COWORKERS-TARGET.md) and detailed controls/tests in [UI]
- implementation/reviews/E01-S04.md: 502: `STALE`; missing comparison data, a revisionless Resource, `UNKNOWN`, and `PLACEHOLDER`
- implementation/ROADMAP.md: 76: ; Provider-backed chat send ; No qualified desktop native turn admission ; NOT DONE ; E03-S01/S02 ; / 78: ; Coworker-owned Conversation + last-active ; No Conversation-owner field/service/route; unrelated environment/activity fields do  / 80: ; Coworker capability assignment ; Capability fabric exists; no CoworkerCapabilityAssignment type/schema/source ; NOT DONE ; E10/E / 82: ; Task-time Connect + resume ; UserRequest/capability pieces exist; no full Coworker repair flow ; NOT DONE ; E04/E10 ; / 84: ; Automatic quiet private memory ; No MemoryCandidate or CoworkerMemoryPolicy implementation found ; NOT DONE ; E08-S03 ; / 85: ; StandingResponsibility group ; No storage/API/domain type found ; NOT DONE ; E09-S01/S02 ; / 87: ; Scheduled/provider/event TriggerHost ; Only manual pinned local TriggerHost path observed ; NOT DONE ; E09-S02 ; / 88: ; Proactive deterministic watch/no-change ; Architecture only ; NOT DONE ; E09-S02/provider ;
- implementation/UI.md: 173: ; Backend API unavailable ; Honest unavailable label and explanatory disabled control ; No placeholder mutations or synthetic chat

## Limits
This detects mechanical/link defects and indexes candidate language. It does not establish cross-file semantic agreement, verified HLD/LLD, served API routes, complete functions or working UI/browser. Those require authority mapping, real source review and tests.
