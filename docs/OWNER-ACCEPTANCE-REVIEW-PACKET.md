# Owner acceptance review packet

**Status:** ACCEPTANCE RECORDED for the dated decisions below. The owner (sarv-projects) answered "accept all" on 2026-11-09. Every row in Parts 1-3 is ACCEPT; each "ACCEPT (scoped)" caveat is a binding rider, and the three NEEDS YOU rows are accepted with the riders written in their Decision cells. Part 4 lists later decisions; Part 6 records the Feature 016 revision 0.4 scope proposal, the Saved View product direction and its remaining OPEN/BLOCKED gates.
**Assembled:** 2026-11-08, after the doc audit, the doc+spec upgrade pass and the A-24/A-26 code fixes. **Decided:** 2026-11-09.
**Effect:** packages 001-012 moved to ACCEPTED/LOCKED and implementation is authorized within each package's TASKS.md. Each resolution's full text lives at its owner (register row or specs/NNN §5); this packet summarizes and links, it does not restate authority.

**Dated update (2026-11-20):** Part 4's B-19 follow-up row below accurately records that the owner had not selected a replacement recipe on 2026-11-10. The product owner later USER-DELEGATED GitHub App user-to-server device flow for internal implementation planning; Feature 006 revision 1.1 remains IN_REVIEW pending formal Connectors/Security ratification. This update preserves the historical packet answer and does not mark the package ACCEPTED/LOCKED again or qualify a provider journey. See A-82 and Feature 006 T02.

Legend for my recommendation: **ACCEPT** = adopt as written; **ACCEPT (scoped)** = adopt but note a caveat; **NEEDS YOU** = a genuine product/strategy call I should not pre-bias.

---

## Part 1 — Package 001 (Conversation Session Supervisor) — the implementable one

Feature 001's baseline was accepted and locked on 2026-11-09. This section preserves the original decision packet and the owner's B-01..B-10 resolutions; read the current package status and active task evidence in [Feature 001 TASKS](../specs/001-conversation-session-supervisor/TASKS.md). The accepted baseline does not imply task completion or provider qualification.

| ID | Decision needed | Proposed resolution (summary) | My rec | Trade-off / why | Decision |
|---|---|---|---|---|---|
| B-01 | Concurrent-send policy per Conversation | Strict single-flight turn lane; 2nd submit → `CONVERSATION_TURN_IN_PROGRESS` 409; composer disabled while active | **ACCEPT** | Simplest correct model; eliminates HLC-ordering/correlation races. Cost: no parallel turns in one Conversation (fine for chat). | **ACCEPT** (2026-11-09) |
| B-02 | First GA OS + harness + adapter + auth | Windows 11/10 `x86_64-pc-windows-msvc` + Claude Code & OpenCode/Codex CLI; Adapter 1.0 framed subprocess; harness-managed native pass-through auth | **NEEDS YOU** | This is the single biggest strategic call — it sets the first qualified platform and harness. Windows-first matches the existing test history; but the only path that actually *runs today* is OpenCode (the audit found Codex/Claude Code not wired to the supervisor). Consider **ACCEPT but name OpenCode as the first route to qualify** so the spec matches reality, with Claude Code/Codex as fast-follows. | **ACCEPT with rider:** Windows 11/10 x64 first row; OpenCode v2.0.32 is the first route to qualify; Claude Code second; Codex later; Linux x64 second row; WSL2 never counts |
| B-03 | Readiness handshake proving session identity | Three-phase: version probe → create-session → atomic SQLite CAS activation; prompt dispatched only after activation commits | **ACCEPT** | Cleanly separates host-process presence from turn readiness (a core invariant). No real downside. | **ACCEPT** (2026-11-09) |
| B-04 | Atomic final-response commit | `commit_conversation_agent_response` IMMEDIATE txn: message + turn SETTLED + session CLOSED + 2 events + dedup receipt, all-or-nothing | **ACCEPT (scoped)** | Correct shape and already partly coded. Caveat the audit found: add explicit **retry-ordinal + stable provider-response identity** fields (currently missing) before calling it complete — that's task 001-T02/T06. | **ACCEPT (rider):** add retry ordinal and stable provider-response identity (001-T02, 001-T06) |
| B-05 | Streaming resume vs gaps | Best-effort transient `TURN_DELTA` frames; on gap, discard buffer and REST-resync the committed transcript; never replay transient frames | **ACCEPT** | Keeps durable truth in storage, stream as projection (invariant-aligned). | **ACCEPT** (2026-11-09) |
| B-06 | Conversation-scoped UserRequest continuation | Commit immutable response → WAITING_USER → WAITING_DEPENDENCY; reconcile provider input key; fresh session only after acceptance; never creates Task/Attempt | **ACCEPT** | Preserves Conversation≠Task separation. | **ACCEPT** (2026-11-09) |
| B-07 | Context reduction for tight budgets | Pinned preamble + selected refs + recent contiguous messages to 80% of budget; explicit truncation receipt in prompt | **ACCEPT (scoped)** | Reasonable deterministic window. Note open question O-05: a pinned harness may accept less than the 4 MiB/1,024-message package; 256 KiB in API.md is a different cap. Confirm the first adapter's real budget during 001-T05. | **ACCEPT (rider):** measure the first adapter budget in 001-T05 |
| B-08 | Attachment/resource materialization | Text inline with checksummed headers; binary as verified read-only WorkspaceRoot paths via `--file`; reject arbitrary external paths | **ACCEPT** | Enforces Workspace boundary / prevents path traversal. | **ACCEPT** (2026-11-09) |
| B-09 | Process ownership + idle shutdown | Job Object (Win)/cgroup (Linux); 10-min idle TTL; graceful term → 5s → kill; never kill shared external processes | **ACCEPT (scoped)** | Good default. Caveat per invariant: a Job Object is **not** a full filesystem/network/credential sandbox — don't let this row imply isolation (that's B-17/RV-05). | **ACCEPT (rider):** Job Object is containment only, not a sandbox |
| B-10 | Usage/cost attribution | Provider-reported tokens if present, else byte heuristic (`bytes/4`) marked `ESTIMATED_LOCAL`; never block settlement on missing telemetry | **ACCEPT** | Truthful, non-fabricated accounting; display never fabricates $0.00/0 tokens. | **ACCEPT** (2026-11-09) |

**001 open conformance items (not blockers, but gate VERIFIED):** O-01 first route must match source (only OpenCode runs today), O-03 single-flight not yet enforced in storage, O-04 final-txn provenance/rollback tests incomplete. These are implementation tasks, not acceptance decisions.

---

## Part 2 — Cross-cutting owner decisions (register A-xx)

Full text: [CONTRADICTION-AND-ALIGNMENT-REGISTER.md](CONTRADICTION-AND-ALIGNMENT-REGISTER.md).

| ID | Decision needed | Proposed resolution (summary) | My rec | Trade-off | Decision |
|---|---|---|---|---|---|
| A-18 | ContextDocument create kinds | Narrow OpenAPI *create* enum to the 2 validator-accepted kinds (PERSONAL_PROFILE, WORKSPACE_NOTES); keep all 4 in record/read/policy as TARGET-pending | **ACCEPT** | Makes the machine contract match the validator today; the 2 deferred kinds stay target values. No downside. | **ACCEPT** (2026-11-09) |
| A-20 / A-34 | The single learning vocabulary | Choose `LearningCandidate` as the one candidate-lifecycle vocabulary (owner LEARNING.md); keep `MemoryRecordMetadata.status` as the distinct committed-record lifecycle; supersede `MemoryCandidate` | **ACCEPT** | Ends a 3-way vocabulary collision with one owner. `LearningCandidate` is the richer/more-expressive set. Unblocks 009/B-24. | **ACCEPT** (2026-11-09) |
| A-35 | Settings screen ownership | Settings→Agents **screen** owned by Feature 011 (002 links); quiet-hours control owned by Feature 010 (011 links) | **ACCEPT** | One owner per screen. Matches where each capability actually lives. Unblocks 002/010/011 from leaving DRAFT. | **ACCEPT** (2026-11-09) |
| A-37 | delegation.schema `on_exhaustion` | Keep the schema (it matches accepted DELEGATION target); mark the field TARGET-not-yet-wired; packets omitting it are valid until the dispatcher exists | **ACCEPT** | Schema-ahead-of-code by design; honest TARGET label. Wiring is a 004/005 step. | **ACCEPT** (2026-11-09) |

---

## Part 3 — Packages 002–012 blockers (each gates its package leaving DRAFT)

Each resolution's full text is in the named `specs/NNN §5`. Accept to let that package advance.

| ID | Pkg | Decision | Proposed resolution (summary) | My rec | Trade-off | Decision |
|---|---|---|---|---|---|---|
| B-11 | 002 | Additional AgentModules after first route | Qualify exactly one more module matching the B-02 pair | **ACCEPT (scoped)** | Depends on the B-02 call; keep in lockstep. | **ACCEPT (rider):** second module is Claude Code CLI (002-T12) |
| B-12 | 002 | Descriptor freshness TTL / probe budget | 15-min per-module TTL, 3/min probe budget, module-overridable | **ACCEPT** | Sensible defaults; overridable. | **ACCEPT** (2026-11-09) |
| B-13 | 003 | First ingestion formats | Qualify the allowlisted UTF-8 lexical text set first | **ACCEPT** | Smallest safe surface; formats added later. | **ACCEPT** (2026-11-09) |
| B-14 | 003 | Watcher vs explicit rescan | Manual rescan is GA default; no V1 file watcher | **ACCEPT** | Avoids OS-notification complexity/races in V1. Cost: no live auto-refresh (acceptable). | **ACCEPT (rider):** the root scanner must also qualify on Windows (003-T05) |
| B-15 | 004 | Portable context size/reduction for relay | Reuse 001 package bounds + portability filter + audited truncation | **ACCEPT** | No private-transcript migration (invariant-safe). | **ACCEPT** (2026-11-09) |
| B-16 | 004 | Second qualified route + relay semantics | Second module = the B-02 pair complement; fresh session per turn | **ACCEPT (scoped)** | Tied to B-02. | **ACCEPT (rider):** tied to the B-02 rider; Claude Code is the complement |
| B-17 | 005 | First Environment provider | Linux **bubblewrap** with all four isolation dimensions ENFORCED | **NEEDS YOU** | Strategy: this picks the first real sandbox. Bubblewrap (Linux) is a solid first target and contradicts nothing, but it means the first *Task-execution* platform is Linux while the first *chat* platform (B-02) is Windows. Confirm you're comfortable with that split, or ask for a Windows-first Environment instead. | **ACCEPT with rider:** Linux bubblewrap first; Task execution and terminal UNAVAILABLE on Windows until B-35 qualifies |
| B-18 | 005 | First mediated file-op class | Atomic whole-file replace, ≤1 MiB, confined path, temp-rename, verifier | **ACCEPT** | Smallest provable Effect with rollback. | **ACCEPT** (2026-11-09) |
| B-19 | 006 | First Connection/provider | GitHub public PKCE loopback client; `api.github.com` allowlist | **ACCEPT** | No embedded secret (PKCE); well-understood API. | **ACCEPT; then BLOCKED by finding:** GitHub requires `client_secret` for web-flow token exchange and revoke, so 006-T02 needs a new owner choice (Part 4) |
| B-20 | 006 | First capability write | Create an issue comment (WRITE class); UNKNOWN blocks + reconciles | **ACCEPT** | Low-blast-radius first write with idempotency. | **ACCEPT** (2026-11-09) |
| B-21 | 007 | First professional Artifact format | **Markdown** `text/markdown`, deterministic in-process serializer, byte-stable round-trip; **no Office engine** in first slice | **ACCEPT** | Pragmatic: proves the Artifact pipeline without an Office/PDF engine. Cost: the headline PPTX/DOCX/XLSX parity (gap-matrix #3) is deferred — intentional. | **ACCEPT** (2026-11-09) |
| B-22 | 008 | First real browser mode | `MANAGED_CHROMIUM` isolated profile; ambient-CDP discovery & default-profile copy DISABLED; attach-existing later | **ACCEPT** | Security-first (the browser-use adapter rule). Cost: no "use my logged-in Chrome" in first slice. | **ACCEPT** (2026-11-09) |
| B-23 | 008 | First real terminal provider | One daemon-owned PTY/ConPTY per tab under an admitted Environment; containment receipt; ordered receipts; timeout-as-receipt; no startup-script exec during discovery | **ACCEPT** | Directly fixes the fabricated-terminal defect; borrows OpenHands receipt doctrine. | **ACCEPT** (2026-11-09) |
| B-24 | 009 | Minimal Coworker memory record | Owner-authored Coworker-scoped ContextDocument using `MemoryRecordMetadata.status`; automatic `LearningCandidate` pipeline deferred; validator accepts `COWORKER_NOTES` | **ACCEPT** | Depends on A-34 ACCEPT. Ships manual memory first, auto-learning later. | **ACCEPT** (2026-11-09) |
| B-25 | 009 | Quick Create Coworker binding | Starts UNBOUND until Feature 002; Coworker + first-chat are separate idempotent commits | **ACCEPT** | Clean separation; safe retry. | **ACCEPT** (2026-11-09) |
| B-26 | 010 | First Automation trigger type | Cron-in-named-timezone first; `RUN_ONCE_WHEN_AVAILABLE` default; stored-TZ DST; bounded catch-up | **ACCEPT** | Standard, honest schedule semantics. | **ACCEPT** (2026-11-09) |
| B-27 | 010 | Notification channel/guarantees | Single LOCAL_POLL channel; `PENDING` delivery row distinct from Task outcome; quiet-hours suppression; no webhook; no delivery guarantee | **ACCEPT** | Fixes the "notifications inserted but never sent" defect honestly. | **ACCEPT** (2026-11-09) |
| B-28 | 011 | First GA OS + security profile | Windows 11/10 `x86_64-pc-windows-msvc` first (named-pipe DACL); Linux 0600/0700 socket second; WSL2 never counts | **NEEDS YOU** | Must agree with B-02. If you pick Windows-first for chat, accept this as-is. | **ACCEPT with rider:** same as B-02 |
| B-29 | 011 | Which Settings groups enabled in V1 | Only served-owner groups interactive (Workspaces, Runtime reads, Agents via 002); others visibly unavailable | **ACCEPT** | Honest; no fake settings. | **ACCEPT** (2026-11-09) |
| B-30 | 007 | Artifact create authority | One `POST /v1/artifacts` create, 3 source classes, one IMMEDIATE txn | **ACCEPT** | Needed for 007; atomic. | **ACCEPT** (2026-11-09) |
| B-31 | 007 | Durable export receipt | `ArtifactExportReceipt` + `artifact.exported.v1` distinct from native Save As; destination class not raw path | **ACCEPT** | Distinguishes real export from local save. | **ACCEPT; class pending:** confirm the local-export receipt class (007-T06, Part 4) |
| B-32 | 008 | Computer-use milestone | **Defer** from first V1 milestone; keep surface explicitly UNAVAILABLE (no disabled-but-ready control) | **ACCEPT** | Scopes V1 down sensibly; computer-use is the riskiest surface. | **ACCEPT** (2026-11-09) |
| B-33 | 011 | V1 Settings acceptance surface | The honestly-mapped served controls within the mounted 4 categories are the acceptance surface; the 16-category IA is authored target, not an assertion | **ACCEPT** | Resolves a 2-orders-of-magnitude mismatch between contract and shipped UI toward honesty. | **ACCEPT** (2026-11-09) |
| B-34 | 012 | Co-edit / take-control owner model | Two bounded Conversation affordances over the 001 turn authority (take-control via cancel/quiescence + `OWNER_TOOK_CONTROL` event; `OWNER_SIDE_NOTE` side-chat excluded from prompt); no new Task type | **ACCEPT (scoped)** | Depends on the 001 Stop/quiescence fix (A-25) being real before it can work; cites RV-14 as research, not fact. Accept the *model*; implementation waits on 001. | **ACCEPT (rider):** model accepted; implementation waits on 001-T07 |

---

## Part 4: decisions raised after the 2026-11-09 acceptance (decided 2026-11-10 where marked)

These came out of the V1 task planning on 2026-11-09. On 2026-11-10 the owner (sarv-projects) accepted the rows marked ACCEPT as proposed, deferred the LiteSPM rows (B-36, B-37 and all Feature 013 tasks including 013-T20), the Windows rows (B-35 and the Windows-only task rows, for later laptop testing) and the release-dependent rows (B-43, B-44), and left B-42 and the B-19 follow-up open and BLOCKED. Recorded as register [A-62](CONTRADICTION-AND-ALIGNMENT-REGISTER.md). The packages 013-020 stay IN_REVIEW: accepting a decision row does not accept the package SPEC as a whole.

| ID | Pkg | Decision needed | Proposed resolution (summary) | Decision |
|---|---|---|---|---|
| B-35 | 005 | Windows Environment provider | AppContainer + restricted token + Job Object + WFP egress filtering; until it qualifies, Windows Task execution and terminal stay UNAVAILABLE | **DEFERRED** (2026-11-10): Windows parts wait for later laptop testing; Windows Task execution and terminal stay UNAVAILABLE |
| B-36 | 013 | LiteSPM bridge pin, transport, peer auth | Pin one LiteSPM release; reconcile the three conflicting pins (9f42b25d, 111fd9b8, f77a3a3) | **DEFERRED** (2026-11-10): LiteSPM parts are deferred by the owner; not decided |
| B-37 | 013 | First package kind and host for plan/apply | `skill` installed by copy, runtime-verified listings only | **DEFERRED** (2026-11-10): LiteSPM parts are deferred by the owner; not decided |
| B-38 | 014 | Conversation delete semantics | Tombstone with immediate fenced purge, no V1 restore window | **ACCEPT as proposed** (2026-11-10) |
| B-39 | 015 | First delegation pair | OpenCode v2.0.32 lead, Claude Code CLI worker; Environment sharing rule | **ACCEPT as proposed** (2026-11-10) |
| B-40 | 016 | First Intelligent UI component | `SAFE_CALCULATOR`; Interactive Artifact Runtime stays PREVIEW_ONLY | **ACCEPT as proposed** (2026-11-10) |
| B-41 | 017 | Backup scope, encryption, restore-over-live | One Workspace per container with a complete table map | **ACCEPT as proposed** (2026-11-10) |
| B-42 | 018 | Support-bundle contents | Deny by default; previewed allowlisted categories | **OPEN, BLOCKED:** not decided in this pass |
| B-43 | 019 | Signing identity and update channel | Authenticode-signed installers and binaries; signed Linux checksums | **DEFERRED** (2026-11-10): release dependent |
| B-44 | 019 | Licensing and packaging | Record as an ADR before any release build | **DEFERRED** (2026-11-10): release dependent |
| B-45 | 020 | First learning source and auto-commit policy | Owner messages and feedback in Coworker-owned Conversations; gate fix first | **ACCEPT as proposed** (2026-11-10) |
| D-01 | 020 | First Suggestion producers | Deterministic GOAL_NEXT_STEP and REPEATED_TASK, no model call | **ACCEPT as proposed** (2026-11-10) |
| B-19 follow-up | 006 | GitHub OAuth recipe after the `client_secret` finding | Choose device flow, an embedded non-confidential secret, or another provider | **OPEN, BLOCKED:** not decided in this pass; 006-T02 stays blocked |
| B-31 follow-up | 007 | Local-export receipt class | Confirm the destination class for the first export receipt | **ACCEPT as proposed** (2026-11-10): the class is the owner-selected local file destination chosen through the native picker, recorded as class only with no raw path, enum value `LOCAL_OWNER_SELECTED_FILE`; 007-T06 Product gate closed |
| 010 event kind | 010 | Event-trigger kind after cron | Confirm CONNECTOR_EVENT (010-T09) | **ACCEPT as proposed** (2026-11-10): CONNECTOR_EVENT over LOCAL_POLL; 010-T09 stays BLOCKED on a qualified Feature 006 Connection |
| 010-T19 | 010 | Direct provider-graph authoring evaluation | Keep deferred and out of V1; needs an ADR later | **ACCEPT as proposed** (2026-11-10): stays deferred, out of V1, ADR later |
| E2E ids | 014, 015, 018, 020 | New golden journeys | Adopt proposed E2E-14..E2E-17 in ACCEPTANCE-AND-E2E-PLAN.md | **ACCEPT as proposed** (2026-11-10): E2E-14..E2E-17 adopted; E2E-16 qualification still waits on B-42 |

Accepting 013-020 also needs the owner to accept each package SPEC as a whole (each package's T01).

## Part 5: 2026-11-09 approval to fold the proposed P-tasks into their owning packages

On 2026-11-09 the owner (sarv-projects) directed "update all docs end to end" and approved folding the proposed tasks P-05 and P-11..P-19 (with the P-00..P-09 rendering and shell set) into their owning packages, including the LOCKED packages, under the Agent Client Protocol (ACP) native-agent transport direction. The ACP direction is authored once at register [A-54](CONTRADICTION-AND-ALIGNMENT-REGISTER.md); this approval is recorded as register [A-55](CONTRADICTION-AND-ALIGNMENT-REGISTER.md). The proposed-task sources are [UI-RENDERING-SETTINGS-ARCHITECTURE-AUDIT-2026-11.md](UI-RENDERING-SETTINGS-ARCHITECTURE-AUDIT-2026-11.md) (P-00..P-09) and [REFERENCE-SYNTHESIS-AND-V1-FEATURE-ALIGNMENT-2026-11.md](REFERENCE-SYNTHESIS-AND-V1-FEATURE-ALIGNMENT-2026-11.md) (P-11..P-19).

This approval does not re-open any LOCKED B-decision. It adds the owner-approved depth tasks inside each package's TASKS.md, renumbers each affected package's "Package VERIFIED for V1" task, and keeps one normative owner per entity. The single answer to "are all 17 capabilities covered" is [V1-PRODUCT-FEATURE-COVERAGE.md](V1-PRODUCT-FEATURE-COVERAGE.md): all 17 are owned; the remaining gaps are depth, not scope.

| Proposed task | Capability / defect | Destination package | Final task id and name |
|---|---|---|---|
| P-00 | Capture a redacted OpenCode v2.0.32 turn fixture | 001 | 001-T23 Capture a redacted OpenCode v2.0.32 turn fixture |
| P-01 | OpenCodeEventNormalizer with correlated internal events | 001 | 001-T24 OpenCodeEventNormalizer with correlated internal events |
| P-02 | Transient turn.activity frames, REPORTED activity records and UI cards | 001 | 001-T25 Transient turn.activity frames and REPORTED activity records with UI cards |
| P-04 | Stick-to-bottom autoscroll and new-messages indicator | 001 | 001-T26 Stick-to-bottom autoscroll and new-messages indicator |
| P-09 | Native permission prompts inline and in Needs You | 001 (with 002) | 001-T27 Surface native permission prompts inline and in Needs You |
| P-11 | Native-agent adapter as an ACP client (per-provider launch catalog) | 002 (consumed by 001) | 002-T17 Define the native-agent adapter as an ACP client |
| P-12 | Agent Matchmaker ranking for a job | 002 | 002-T18 Agent Matchmaker ranking for a job |
| P-15 | Context Debugger (what entered model context) | 003 | 003-T17 Context Debugger surface |
| P-06 | Remove allow-same-origin from the Browser preview iframe and add a guard test | 008 | 008-T19 Remove allow-same-origin from the Browser preview iframe and add a guard test |
| P-13 | Git worktree view, VCS diff and user-invoked GitHub PR ops | 008 | 008-T20 Git worktree view, VCS diff and user-invoked GitHub PR ops |
| P-05 | Shell reachability and truthfulness | 011 | 011-T19 Shell reachability and truthfulness |
| P-07 + P-19 | Decompose App.tsx and the Rust god files behind existing boundaries | 011 | 011-T20 Decompose App.tsx and the Rust god files behind existing boundaries |
| P-08 | Merge SettingsPage into SettingsView with precedence notes | 011 | 011-T21 Merge SettingsPage into SettingsView with precedence notes |
| P-16 | Persistent Workspace Dashboard with pinnable widgets | 011 | 011-T22 Persistent Workspace Dashboard with pinnable widgets |
| P-17 | Pin the LiteSPM bridge and mirror its safety contract | 013 | 013-T20 Pin the LiteSPM bridge and mirror its safety contract |
| P-14 | Mission Control canvas and read-only Compare Agents fan-out | 015 | 015-T20 Mission Control canvas and read-only Compare Agents fan-out |
| P-03 | Per-block Markdown degradation with code and TEXT_SLICE rendering | 016 | 016-T18 Per-block Markdown degradation with code and TEXT_SLICE rendering |
| P-18 | Generate operator-api.ts and TS validators from the Rust wire types with a drift gate | 018 | 018-T18 Generate operator-api.ts and TS validators from the Rust wire types with a drift gate |

The renumbered "Package VERIFIED for V1" tasks are 001-T28, 002-T19, 003-T18, 008-T21, 011-T23, 013-T21, 015-T21, 016-T19 and 018-T19; 019-T23 (the V1 go/no-go) re-synced its VERIFIED-id list to those ids. Packages not in this table keep their prior final task id.

## Summary

- Accepted 2026-11-09: Parts 1-3 in full, with the riders in the Decision cells; and Part 5 (fold the proposed P-tasks P-00..P-09 and P-11..P-19 into their owning packages under the ACP direction, register A-55).
- Part 4 decided 2026-11-10 (A-62): B-38, B-39, B-40, B-41, B-45, D-01, the B-31 class, CONNECTOR_EVENT, E2E-14..E2E-17 and the 010-T19 deferral are accepted as proposed. Deferred: LiteSPM (B-36, B-37, all of 013), Windows rows (B-35 and Windows-only tasks, for later laptop testing) and B-43/B-44 (release dependent). B-42 remained open; the B-19 OAuth recipe was open and blocked at that dated checkpoint, then received USER-DELEGATED direction on 2026-11-20 as described in the dated update above. Feature 006 revision 1.1 remains IN_REVIEW pending formal Connectors/Security ratification.
- The delivery order is in [LITECOWORK-BUILD-TASKS.md](LITECOWORK-BUILD-TASKS.md) "V1 release waves".

## Part 6 — Feature 016 revision 0.4 intelligent workspace scope (proposed 2026-11-17; Saved View direction decided, whole-scope decision pending)

The owner requested a broader product scope and selected “Revise scope before implementation.” On 2026-11-17 the owner further decided: **“Keep Saved Views in V1; amend Feature 007 B-21.”** This accepts the product direction for Saved Views, not the whole Feature 016 revision or formal qualification. On 2026-11-24 the user delegated implementation direction A-84 for the complete current proposal, including a separate host-owned Ask-a-follow-up action. The Feature 007 revision 1.1 Saved View contract remains under cross-owner review and allows an explicit, unchecked-by-default numeric snapshot in immutable ArtifactVersion content. The calculator component remains transient and non-submitting under B-40. A follow-up may include selected values only as visible ordinary composer text after exact preview and explicit Send; no hidden provider payload is added. Feature 016 remains IN_REVIEW. See [Feature 016 SPEC §20](../specs/016-rich-presentation-intelligent-ui/SPEC.md), [Feature 007 Saved View contract](../specs/007-professional-artifacts/contracts/saved-view.md), [A-84](CONTRADICTION-AND-ALIGNMENT-REGISTER.md) and [A-103](CONTRADICTION-AND-ALIGNMENT-REGISTER.md).

| ID | Decision needed | Proposal for review | Owners required | Decision |
|---|---|---|---|---|
| B-46 (proposed) | Does V1 allow device-local remembered inputs and ordinary-turn state sharing while preserving B-40's component boundary? | USER-DIRECTION RECORDED 2026-11-24 under A-84 for the follow-up behavior: keep components local/non-submitting; values are omitted by default; an explicit host action may place selected declared values into a visible editable ordinary composer draft after exact preview; only existing Conversation Send dispatches. Device-local Remember/Forget remains a separate operation requiring consent, exact scope, quota, receipts, deletion fencing, retention/backup and recovery. Saved View numeric snapshots remain a separate explicit Artifact action. No autosave, model-triggered action, hidden metadata, component callback, generic function or event/receipt input values. | Product, Presentation Runtime, Security, Accessibility, Privacy, Storage, Conversation; repository owner | **FOLLOW-UP DIRECTION RECORDED; CROSS-OWNER RATIFICATION OPEN; REMEMBER/Forget CONTRACT OPEN** |
| T25 / A-103 | Can Feature 007 own a non-executable Saved View in its ArtifactVersion/Library model? | **OWNER DIRECTION RECORDED 2026-11-17; implementation detail USER-DELEGATED 2026-11-24:** Keep Saved Views in V1; amend Feature 007 B-21. The drafted contract defines one Saved View Artifact media type, exact source lineage, semantic fallback, ArtifactVersion/ResourceRevision pairing, atomic create receipt, exact-version reopen, explicit update from a new committed same-Conversation presentation, archive-only removal, Conversation deletion race, backup, retention, conflict and recovery. A-84 adds an optional explicit numeric input snapshot inside immutable ArtifactVersion content, not an event, receipt field, second table or agent input. No hard Delete, export of internal JSON, or permanent navigation destination. See `specs/007-professional-artifacts/contracts/saved-view.md`; cross-owner review and OpenAPI/schema promotion remain pending. | Feature 007 Artifact/Library, Presentation Runtime, Storage, Conversation, Privacy/retention, Backup, Product; repository owner | **USER DIRECTION RECORDED; technical contract IN_REVIEW** |
| T01 | Does the owner accept Feature 016 revision 0.4 as a whole for V1? | Record the exact included ACs, riders, dependencies, release rows and owner contracts. A-84 directs implementation of the complete current proposal but does not count as package acceptance; no unreviewed criterion becomes ACCEPTED/LOCKED by association. Saved Views remain in scope under the Feature 007 owner and its immutable Artifact lifecycle. | Product, Presentation Runtime, Security, cross-owner reviewers; repository owner | **FORMAL ACCEPTANCE OPEN; user-directed implementation may proceed in scoped tasks** |

Part 6 does not record formal package acceptance or qualification. A-84 is explicit authorization to implement the complete current proposal through scoped active task work and canonical contract updates; it does not permit claiming ACCEPTED/LOCKED, cross-owner ratification, external protocol facts or qualification. The Saved View schema remains TARGET until its owner contract and API path are promoted and implemented. The ordinary follow-up draft uses the existing Conversation Send contract; device-local Remember/Forget still needs a separately promoted storage and recovery contract.
