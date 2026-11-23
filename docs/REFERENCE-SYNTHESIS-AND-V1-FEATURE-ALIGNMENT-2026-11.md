# Reference synthesis, LiteSPM-as-a-service, ACP, and V1 feature alignment (2026-11-09)

> **Historical research note:** This dated feature map records the baseline at its review date. Its Feature 006 row predates the GitHub permission review and still says the client-secret finding is open; the current delegated direction and implementation state are in [Feature 006](../specs/006-golden-connection-capability-write/README.md) and alignment register A-82. Its Feature 008 row describes revision 1.0/B-32 and the earlier browser-only Workbench decision. The owner-requested selected-screen Computer design is tracked separately as Feature 008 revision 1.1, IN_REVIEW, in [the current Workspace experience revision](WORKSPACE-EXPERIENCE-REVISION-2026-11.md) and [Computer observation/control LLD](COMPUTER-OBSERVATION-AND-CONTROL-LLD.md). Neither this historical row nor the new proposal is implementation or qualification evidence.

**Status:** research and planning synthesis. Not an owner contract. Authority stays with ARCHITECTURE.md, the owner docs and the accepted feature packages. Everything new here was a proposed task against an existing package (Section 8) for owner review.
**Outcome (2026-11-09):** the owner accepted the ACP direction (register [A-54](CONTRADICTION-AND-ALIGNMENT-REGISTER.md)) and approved folding the proposed tasks P-11..P-19 into their owning packages, including the LOCKED packages (register [A-55](CONTRADICTION-AND-ALIGNMENT-REGISTER.md), recorded in [OWNER-ACCEPTANCE-REVIEW-PACKET.md](OWNER-ACCEPTANCE-REVIEW-PACKET.md) Part 5). The Section 8 items below are therefore accepted package tasks now, not open proposals; their final task ids and the capability-by-capability verdict are in [V1-PRODUCT-FEATURE-COVERAGE.md](V1-PRODUCT-FEATURE-COVERAGE.md) and [SPEC-TASK-TRACEABILITY.md](SPEC-TASK-TRACEABILITY.md). The "proposed P-NN" wording below is preserved as the original proposal record.
**Revision:** branch `docs/audit-competitor-superset` at 06620c7.
**Inputs:** the consolidated LiteCowork Final Product Feature List (9 Nov 2026, 17 capabilities), the [LiteSPM product site](https://lite-spm-nine.vercel.app/), the [Agent Client Protocol overview](https://agentclientprotocol.com/protocol/overview), and nine reference repositories cloned read-only into `repo_take/` (research only, never imported as source per AGENTS.md section 7): [getpaseo/paseo](https://github.com/getpaseo/paseo), [elie222/rakazo](https://github.com/elie222/rakazo), [prasenjeet-symon/ogcode](https://github.com/prasenjeet-symon/ogcode), [0xsline/awesome-deepseek-harness](https://github.com/0xsline/awesome-deepseek-harness), [hotovo/aider-desk](https://github.com/hotovo/aider-desk), [zhayujie/CowAgent](https://github.com/zhayujie/CowAgent), [CherryHQ/cherry-studio](https://github.com/CherryHQ/cherry-studio), [ValueCell-ai/ClawX](https://github.com/ValueCell-ai/ClawX), [LePro10/NeuralAgent](https://github.com/LePro10/NeuralAgent).

## 1. The one decision that reframes everything: adopt ACP as the native-agent transport

The Final Product Feature List asks for streaming responses, structured tool activity, native questions and confirmations, slash commands, session resume, live diffs, agent provenance, and a real coding cockpit (capabilities 1, 2, 3). The current build cannot deliver any of these, because the supervisor parses one OpenCode server's SSE by guessing top-level JSON keys and flattens everything to a single TEXT block (register A-04, A-52; audit R-01/R-04).

The reference repos point to a single, decisive fix: **standardize the native-agent boundary on the Agent Client Protocol (ACP)** and treat per-provider HTTP (like OpenCode's server API) as one adapter behind it, not as the contract.

Why ACP is the right spine, with evidence:

- **ACP already carries every structured event the feature list needs.** Its `session/update` notification streams message chunks, agent thought chunks, tool calls and updates, plans, available-commands updates, mode changes and usage. Its client methods are exactly LiteCowork's trust surface: `session/request_permission` (the Needs You inbox, capability 15), `fs/read_text_file` and `fs/write_text_file` (mediated file effects, capability 17), and `terminal/create`, `terminal/output`, `terminal/wait_for_exit`, `terminal/kill`, `terminal/release` (the real PTY cockpit, capability 3). Source: [ACP overview](https://agentclientprotocol.com/protocol/overview).
- **ACP is how the ecosystem already ships.** Paseo's catalog lists 40-plus ACP coding agents behind one adapter, each just a launch command plus `--acp`: Claude (`claude-code-acp`), Codex (`codex acp`), Cursor (`cursor-agent acp`), Gemini, Cline (`cline --acp`), Copilot CLI, Grok, Kimi, Goose, Auggie and more (`repo_take/paseo/packages/app/src/data/acp-provider-catalog.ts:1-410`). This is the clean, qualified version of LiteCowork's Agent Registry (capability 2) and Agent Relay (capability 1). It directly supplies the "other qualified native agent harnesses" line.
- **A production ACP client already exists to learn the exact mapping from.** Paseo's `acp-agent.ts` (4,389 lines) maps every `sessionUpdate` kind to a typed timeline item (`:3233-3306`): `agent_message_chunk` to assistant message, `agent_thought_chunk` to a separate reasoning item (never concatenated into the answer, which fixes A-52's leak risk), `tool_call`/`tool_call_update` to tool cards, `plan` to a plan timeline (`mapPlanToTimeline`, `:3954`), `current_mode_update`, `available_commands_update` to slash commands, and `usage_update` to usage. It maps stop reasons (`cancelled`, `end_turn`, `max_tokens`, `max_turn_requests`, `refusal`) to turn completion/cancellation (`:3512-3524`) and content blocks (text, image, resource_link, resource, audio) to typed content (`:3865-3903`). It also handles the failure modes LiteCowork lacks: the child exiting before `initialize` replies, draining stderr into the error, and tree-kill on disconnect (`:143-180`).

This does not weaken any invariant. ACP agents still run as native subprocesses that own their reasoning and auth; LiteCowork stays the Client that owns permission, files, terminals and evidence. It is the exact boundary AGENTS.md section 2 describes, expressed in a standard the agents already speak.

**Recommendation.** Feature 002's "Adapter 1.0" (002-T03) and Feature 001's adapter trait (001-T04) should be defined as an **ACP client** with a per-provider launch catalog, with the existing OpenCode v2 HTTP path kept as one non-ACP adapter until OpenCode's own ACP route is qualified. This makes Claude Code, Codex, Gemini, Cline and Cursor reachable through one qualified path instead of one hand-written adapter per vendor, and it is what turns capabilities 1, 2 and 3 from aspiration into a bounded implementation. Proposed task P-11.

## 2. LiteSPM as a first-party service (capability 7)

The LiteSPM site confirms the product the Final List assumes: an npm-installed local service (`npm i -g litespm`) that is the package manager for AI-agent capabilities (MCP servers, skills, plugins) across 50 agent hosts, with a `/marketplace` chat command, a windowed marketplace, previews, pre-write backups, a rollback ledger, and byte-exact `litespm restore <installId>`. Its stated model matches LiteCowork's ownership split almost exactly:

| LiteSPM site statement | LiteCowork mapping | Status today |
|---|---|---|
| "Install once, use everywhere"; one local service per machine shared across agents | One local LiteSPM service per OS user; LiteCowork is a client, not a second package DB (AGENTS.md section 6) | Feature 013 B-36 (pin version, transport, peer auth) |
| Previews a diff, backs up, logs to an undo ledger before any write | 013 plan then digest-approval then apply then receipt then reconcile | 013-T07..T09 |
| "Installable" requires a verified runtime contract (remote URL or command+args); community packages are "Discovery only" | 013 runtime-verified listings only; Discover `checked_at` must be a real timestamp, not a correlation id (audit of 013) | 013-T06, 013-T15 |
| `litespm install remove` strips only nodes LiteSPM wrote; manual edits preserved | LiteSPM owns package lifecycle; LiteCowork owns use authorization, scope, invocation and evidence | 013-T10, 013-T11, 013-T14 |
| Keys and configs stay local; no account, no credential proxy | Local-first; tokens never enter agent prompts (capability 8, AGENTS.md section 6) | 006 Connections, kept separate |
| `/marketplace` in an agent's chat; agent prepares installs for user approval | Agent-facing marketplace; an agent cannot self-approve | 013-T13, 013-T14 |

Two concrete pulls for Feature 013:

1. **Pin the bridge from LiteSPM's published contract, never inferred.** The site advertises an npm package and a marketplace bridge, but the exact wire protocol, version and peer-auth are not on the marketing page. AGENTS.md section 6 forbids inferring RPC names from CLI output. B-36 must pin the real published bridge contract (its npm package internals or a documented protocol doc), recorded in `specs/013 RESEARCH.md`, and reconcile the three conflicting pins the audit found (9f42b25d / 111fd9b8 / f77a3a3). The `npmjs.com/package/litespm` page returned 403 to automated fetch; the pin must be captured from the installed package or the project's own protocol doc at implementation time.
2. **Mirror the safety contract exactly.** Preview diff, pre-write backup, transactional single write, byte-exact restore, and "installable vs discovery-only" are already LiteCowork's model; 013 should state them in the same words so the two services cannot drift.

Cherry Studio shows the client-side half worth borrowing: an `McpCatalogService` that single-flights tool-cache warms per server so concurrent sessions do not each open a connection, with a failed-tools backoff (`repo_take/cherry-studio/src/main/ai/mcp/McpCatalogService.ts:24-40`). LiteCowork's 013 Discover read-through (013-T06) should adopt the same single-flight and backoff shape.

## 3. The real coding cockpit and terminal (capability 3)

The feature list is emphatic that the terminal must be real PTY sessions, not a mock, with ConPTY on Windows, persistence across tab switches, and preserved child processes. The current Workbench Terminal is an honest "unavailable" notice (correct, register A-51) and the Browser iframe's sandbox is self-cancelling (audit U-02/A-53). ACP gives the agent-driven half for free (`terminal/create` and friends). For user-owned terminals and process lifecycle, the references give two concrete models:

- **NeuralAgent** is the closest architectural twin: a Rust daemon (`crates/na-daemon`) that is the source of truth, with discrete modules for `pty.rs`, `git.rs`, `files.rs`, `permissions.rs`, `policy.rs`, `secrets.rs`, `usage.rs`, `connectors/`, `mcp.rs`, and an `autopilot/` turn engine (`turn.rs`, `perm.rs`, `sched.rs`). Its `na-protocol` crate is one shared wire-types file with a `PROTOCOL_VERSION` and an explicit `DAEMON_FEATURES` capability list that clients check before sending newer messages, and TypeScript types are generated from the Rust with `cargo test -p na-protocol` (`repo_take/NeuralAgent/crates/na-protocol/src/lib.rs:1-35`). This is a strong template for two things LiteCowork needs: splitting the 24k-line `storage-sqlite/lib.rs` and 9.5k-line `operator.rs` into owned modules (audit X-02), and generating the TS operator types from Rust instead of hand-maintaining a 13.8k-line `operator-api.ts`.
- **Paseo** ships a real terminal as a native module with a worker and a typed `contract.ts` (`repo_take/paseo/packages/app/modules/paseo-terminal/contract.ts`), documented for performance and a worker thread (`docs/terminal-worker.md`, `docs/terminal-performance.md`). It is the pattern for persistent sessions that survive tab switches, which is Feature 008's B-23 requirement.

Neither replaces LiteCowork's accepted B-23 design (daemon-owned PTY/ConPTY under an admitted Environment with a containment receipt). They confirm the shape and give a concrete module layout. Proposed note on 008-T08..T12.

## 4. Repo-by-repo: what to take, what to leave

| Repo | What it is | Take | Leave |
|---|---|---|---|
| **paseo** (TS, Expo/React, ~6,700 files) | Multi-agent ACP desktop/mobile client | The ACP client (`acp-agent.ts`), the `session/update` to timeline mapping, the 40-agent launch catalog, the capability-negotiation adapter boundary (`connection.ts:1-180`), the terminal worker module, the `assertChangesAreLive` option-control discipline | Mobile/Expo surface; its own cloud sync |
| **NeuralAgent** (Rust+Tauri, ~1,100 files) | Local daemon agent with remote relay | Daemon module decomposition; `na-protocol` single wire-types file with `PROTOCOL_VERSION` + feature flags + generated TS types; separate `permissions.rs`/`policy.rs`/`secrets.rs` | E2E relay and computer-vision autopilot (that is V2 remote scope and desktop control) |
| **aider-desk** (TS/Electron, ~1,900 files) | Coding-agent desktop with an ADR process | Its ADR discipline (`docs/adr/platform-services/0035-pty-terminal-integration.md`), the ACP CLI command surface (`cli/commands/acp.ts`), the extension loader with install registry | Aider-specific model/provider assumptions |
| **cherry-studio** (TS/Electron, ~13k files) | Mature multi-provider chat desktop | `McpCatalogService` single-flight + backoff; the lifecycle service container (`@Injectable`, `@ServicePhase`, `@DependsOn`); provider `ExtensionRegistry` | Its breadth of provider integrations (out of BYOA scope) |
| **ClawX** (TS/Electron, ~1,200 files) | Gateway-mediated agent desktop | The gateway pattern with `agent-database-admission.ts`, `capability-monitor.ts`, `config-delivery.ts`; a manifest-driven builtin extension loader (`electron/extensions/loader.ts:27-70`) | Its specific agent bundle |
| **rakazo** (TS, ~4,900 files) | Self-hosted persistent AI teammates (web/desktop/mobile) | Its Coworker product shape (persistent teammates with memory, routines, apps, a computer) validates LiteCowork capability 5; routine and memory UX reference | Shared/team computers and mobile (V2) |
| **CowAgent** (Python, ~1,700 files) | Multi-channel chatbot (Discord, Feishu, DingTalk, QQ) | The channel-adapter factory pattern (`channel/channel_factory.py`) as a reference only, for future connector breadth | Python runtime; chat-bot framing is not BYOA |
| **ogcode** (Go, ~700 files) | Remote-reachable coding agent with a control plane | Its control-plane auth and tunnel/session model (`controlplane/internal/master/{auth,sessions,tunnel}.go`) is a V2 remote-control reference only | Everything else; remote exposure is explicitly post-V1 |
| **awesome-deepseek-harness** (docs only, 10 files) | A catalog list, not a plugin architecture | The curated capability-catalog idea for Discover | There is no plugin-based architecture here to borrow; it is a `CATALOG.md` link list. The earlier plan note that cited its "plugin-based architecture" overstated it. |

Correction to record: the steering asked to study awesome-deepseek-harness "esp its plugin based architecture". On inspection it is a 10-file curated catalog (`CATALOG.md` plus scripts), not a plugin framework. The genuine plugin and extension patterns worth taking are ClawX's manifest loader and cherry-studio's lifecycle service container, noted above.

## 5. Feature-list coverage against the 20 packages

Every capability in the Final Product Feature List maps to an accepted or in-review package. No capability is unowned. The gaps are depth, not scope.

| # | Capability | Package(s) | Coverage verdict |
|---|---|---|---|
| 1 | Universal Chat + Agent Relay | 001, 004, 014, 012 | Owned. Relay needs the ACP spine (P-11) and the rendering fixes (audit P-01..P-04). |
| 2 | Bring Your Own Agent, Registry, Matchmaker, Upgrade Shield | 002 | Owned. Registry should become an ACP launch catalog (P-11). Matchmaker ("which agent for this job") is not an explicit task; proposed P-12. |
| 3 | Coding cockpit, interactive terminal, Git/worktree | 008, 005, plus ACP terminal | Owned for terminal (008 B-23) and worktrees (005). Git worktree UI and PR ops are thin; proposed P-13. |
| 4 | Durable Tasks, multi-agent delegation, Compare Agents, Mission Control | 005, 015 | Owned. Compare Agents and the Mission Control canvas are not explicit tasks; proposed P-14 (both are post-first-workflow per the list's own "prove one bounded workflow first"). |
| 5 | Persistent Coworkers | 009, 020 | Owned. |
| 6 | Computer interaction and Workbench | 008 | Owned. Computer use deferred (B-32). Activity pane is 008-T02/T14. |
| 7 | LiteSPM and native Marketplace | 013 | Owned. Section 2 pins and safety contract. |
| 8 | Connections and app integrations | 006 | Owned. First read + reviewed write (B-19/B-20); client_secret finding open. |
| 9 | Local Knowledge, Resources, RAG, Context Debugger | 003 | Owned. Context Debugger ("what the agent actually received") is thin; proposed P-15. |
| 10 | Routines and Automations | 010 | Owned. |
| 11 | Memory and Knowledge Learning | 009, 020 | Owned. The list agrees with A-20/A-34: keep automatic memory review-gated (accepted). |
| 12 | Library and Artifacts | 007 | Owned. One DOCX/XLSX route is B-21 (currently Markdown-first; Office is deferred, matching the list). |
| 13 | Intelligent UI, 5 presentation layers, Interactive Artifacts | 016 | Owned. Rust/TS validator drift and the missing publisher are 016-T03/T04/T08; audit P-03. |
| 14 | Mission Control + persistent Workspace Dashboard | 015, plus new | Partly owned. The pinnable Workspace Dashboard is not owned; proposed P-16. |
| 15 | Needs You inbox | 005, 006, 009, 020 feed it; UI exists | Owned, but the page is currently unreachable from the sidebar (audit U-01). Proposed P-05. |
| 16 | Media, voice, multimodal | 003 (image), later | Partial. Image upload/understanding only where the ACP provider supports `prompt.image` (paseo negotiates this, `connection.ts:78`). Audio/transcription and video are V1.x per the list; record as scope, not a V1 gate. |
| 17 | Desktop Runtime, Settings, security, portability, evidence, commercial | 011, 017, 018, 019 | Owned. Settings honesty is 011-T06; backup leak and portability are 017; REPORTED/OBSERVED/VERIFIED is the evidence vocabulary throughout. |

Two scope confirmations the list makes that match accepted decisions: automatic memory stays review-gated (capability 11 vs A-20/A-34, accepted), and the commercial model is local-first with no mandatory subscription and no token markup (capability 17 vs B-44, which 019-T13 records as an ADR).

## 6. The architecture and design problems this consolidates

The three prior audit documents ([UI-RENDERING-SETTINGS-ARCHITECTURE-AUDIT-2026-11.md](UI-RENDERING-SETTINGS-ARCHITECTURE-AUDIT-2026-11.md), the docs re-audit, the register) already name the defects. The references tell us the fix is largely structural, not more surface:

1. **Agent boundary (highest impact).** Replace per-vendor SSE guessing with an ACP client. Fixes A-04, A-52, R-01, R-04 and unlocks capabilities 1, 2, 3, 15 at once. P-11.
2. **Streaming and activity.** ACP `session/update` chunks become `turn.delta` and `turn.activity` frames the UI reducer already supports. Fixes R-03 and powers the cowork plan/activity view (audit C-1/C-2, P-01/P-02).
3. **Rendering.** Per-block Markdown degradation and a highlighter (audit P-03) so answers stop collapsing to raw text (R-05/R-06).
4. **Decompose the god files.** Use NeuralAgent's module and generated-types pattern to split `storage-sqlite/lib.rs`, `operator.rs`, `src-tauri/lib.rs` and `App.tsx` (audit X-02, P-07).
5. **One source of truth for wire types.** Generate `operator-api.ts` from Rust (NeuralAgent `na-protocol` pattern) to kill the hand-maintained 13.8k-line file and the drift between Rust and TS validators (audit X-04, finding 016).
6. **LiteSPM contract pinned from the published bridge** (Section 2), not inferred.

## 7. Rust vs TypeScript placement (consolidated)

Consistent with the per-package optimization notes added on 2026-11-09 and confirmed by NeuralAgent:
- **Rust (daemon):** the ACP client and event normalization; terminal/PTY ownership; the single rich-presentation validator; diffs; permission and policy gates; secrets; the wire-type source of truth with generated TS.
- **TypeScript (UI):** pure reducers and registered renderer components only; the ACP provider launch catalog as data; no second validator authority (generate the parity check from schemas).

## 8. Proposed tasks for owner review (accepted 2026-11-09, register A-55)

Packages 001, 002, 003, 004, 005, 008, 011 and 015 are ACCEPTED/LOCKED, so these need an owner-approved TASKS revision; nothing here edits a locked file. Packages 013-020 are IN_REVIEW and can take additions now.

| ID | Package | Proposed task | Why |
|---|---|---|---|
| P-11 | 002 (adapter) + 001-T04 | Define the native-agent adapter as an **ACP client** (initialize, session/new, session/prompt, session/update, session/cancel; client-side request_permission, fs, terminal) with a per-provider launch catalog; keep OpenCode v2 HTTP as one non-ACP adapter until its ACP route qualifies. Pin the ACP SDK version in RESEARCH.md. | Delivers capabilities 1, 2, 3, 15 through one qualified path; fixes A-52. |
| P-12 | 002 | Agent Matchmaker: given a job, rank eligible bound agents by declared capability with a stated reason; user still chooses. | Feature-list capability 2. |
| P-13 | 008 | Git worktree view and changed-file/aggregate diff from real VCS, plus user-invoked GitHub PR ops behind 006. | Capability 3 coding cockpit. |
| P-14 | 015 | Mission Control canvas (delegation tree, step status, live tool activity, approvals, evidence) bound to real runtime records, after the first bounded workflow is proven; and an explicitly-requested Compare Agents read-only fan-out. | Capability 4 and 14; the list itself defers swarms. |
| P-15 | 003 | Context Debugger surface: show exactly what entered model context (attachments, retrieved passages with source revisions, skills, limits). | Capability 9. |
| P-16 | 011 | Persistent Workspace Dashboard with pinnable widgets (Task status, automation status, Coworker activity, recent artifacts). | Capability 14. |
| P-17 | 013 | Pin the LiteSPM bridge from its published package/protocol (reconciling the three SHAs), and mirror its preview/backup/transaction/byte-exact-restore safety contract word for word; adopt single-flight + backoff for Discover read-through. | Capability 7; Section 2. |
| P-18 | 018/019 | Generate `operator-api.ts` and the TS validators from the Rust wire types (NeuralAgent pattern); add a drift gate. | Fixes audit X-04 and finding 016; one source of truth. |
| P-19 | 011/019 | Decompose the four god files into owned modules behind the existing boundaries, no behavior change. | Audit X-02. |

## 9. Method and limits

Nine repos cloned shallowly into `repo_take/` (research only). Claims cite exact files and lines read: paseo `acp-agent.ts:143-180, 3233-3306, 3505-3535, 3865-3903`, `acp-provider-catalog.ts:1-120`, `acp-internal/connection.ts:1-180`; NeuralAgent `na-protocol/src/lib.rs:1-50` and the `na-daemon` module listing; aider-desk `cli/commands/acp.ts:1-90`; cherry-studio `McpCatalogService.ts:1-30`; ClawX `extensions/loader.ts:1-70`; ogcode controlplane listing; rakazo and CowAgent READMEs. The ACP shape is from the published protocol overview. The LiteSPM model is from its product site; the npm page returned 403 to automated fetch, so the exact bridge wire contract must be pinned from the installed package at 013 implementation time, not from marketing copy. No reference code was copied into LiteCowork and no app source was changed by this document.
