# Tasks - Agent Relay on One Conversation

**Feature:** 004 | **Status:** ACCEPTED/LOCKED - implementation authorized; tasks not started unless marked.
**Spec revision:** 1.0 | **Release target:** V1 (release matrix rows: Windows x64 + OpenCode v2.0.32 first; Linux x64 second)

## Current baseline

Verified against source on 2026-11-13 (crosswalk only; no source behavior changed). Served: `PATCH /v1/conversations/{conversation_id}/agent-binding` (`setConversationAgentBinding`) in `apps/litecoworkd/src/conversation_operator.rs`, Tauri `set_conversation_agent_binding` in `apps/litecowork-ui/src-tauri/src/conversation_bridge.rs`, and `conversation.turn.created.v2` pinning `agent_binding_id`. CODE: `build_conversation_context_package` and `CONVERSATION_CONTEXT_MAX_BYTES` in `crates/storage-core/src/conversation.rs`, read by `load_conversation_context_for_turn` in `crates/storage-sqlite/src/conversations.rs`; the supervisor calls Feature 001's `materialize_conversation_resources` before `encode_conversation_context_prompt` (256 KiB; the direct encoder rejects unmaterialized refs); it has one OpenCode dispatch arm. Desktop parsing/rendering retains producer identity, but picker choices do not require a fresh descriptor or auth/offer qualification; `deriveConversationAgentSurface` has no production caller. Gaps: no Claude Code arm (B-16); no relay truncation receipt; orphan-row and COUNT/SUM scans over full Conversation history before the 1,024-row fetch; Workspace instruction preamble and ArtifactVersionRef materialization missing; no descriptor-freshness gate; no A→B→A provider or recovery qualification. See the 2026-11-13 current cross-check report.

## 004-T01 - Promote portable-context bounds, truncation receipt and relay errors

- **Status:** NOT STARTED
- **Depends on:** NONE
- **Cross-package prerequisites:** Feature 001's B-07 adapter budget confirmation; owner resolution of A-66/A-96 for AC-007 and the durable receipt's identity/association/storage; canonical Workspace instruction revision and `ArtifactVersionRef` owner wiring from `docs/CONTEXT.md`, `docs/SCHEMAS.md` and Feature 007. Do not choose a receipt placement or migration by implementation preference.
- **Owner:** Conversation, Agent Runtime, API/schema owner.
- **Allowed paths:** `docs/CONTEXT.md`, `docs/SCHEMAS.md`, `docs/API.md`, `docs/schemas/operator-api.openapi.yaml`, `docs/schemas/error-codes.schema.json`, `docs/schemas/sqlite-vNN.sql` (new, only if the receipt is durable; v48 or the next free number because v35-v47 are provisionally reserved after wired v34), `apps/litecowork-ui/src/api/operator-api.ts` (regenerated), this package.
- **Contract:** SPEC §5 B-15, §7.2 (`PortableRelayExport`, `PortableContextMaterializer`, `RelayExportError`), AC-002, AC-007, AC-008, AC-011.
- **Change intent:** Promote the accepted bounds (1,024 messages, 4 MiB package, the smaller of 256 KiB and the adapter's declared text budget), the allowed reference kinds (`ResourceRef` and the canonical exact-version `ArtifactVersionRef` corresponding to B-15's durable ArtifactRef wording), the exclusion list (`SYSTEM_NOTICE`, drafts, native handles, credentials, harness-private transcripts), the truncation receipt shape, and `RelayExportError` codes mapped to canonical error codes. A-66 requires owner review to align AC-007 with B-15; A-96 requires the derived-package/atomic-record wording to be reconciled. Do not choose whether the receipt is separate durable storage or part of the turn/admission receipt, or allocate a migration, until the Conversation/Storage owners approve its exact identity and transaction association.
- **Acceptance:**
  - [ ] One normative home per bound and per error; OpenAPI/JSON Schema match; `api:check` passes.
  - [ ] No relay-only route or event is introduced.
  - [ ] SPEC §7.2 entries promoted are relabelled in a new SPEC revision with owner approval.
- **Command:** `python3 .specify/scripts/validate_feature_package.py` ; `python3 scripts/audit_v1_contracts.py` ; `pnpm --dir apps/litecowork-ui run api:check`
- **Evidence:** reviewed docs/schema diff; command output in CURRENT-RUN.md.
- **Stop:** stop if a bound would need to differ from the Feature 001 package bound without a 001 revision.

## 004-T02 - Preserve producer identity through the desktop message types

- **Status:** PARTIAL (desktop type/parser, malformed-field rejection, legacy null producer, older-page attribution retention and immutable-stream producer drift are covered by `conversation-view.test.ts`; A→B→A transcript, storage pass-through evidence and native provider handles exclusion remain open).
- **Depends on:** NONE
- **Cross-package prerequisites:** NONE (no new contract; can run in Wave 1).
- **Owner:** Desktop UI, Conversation.
- **Allowed paths:** `apps/litecowork-ui/src/conversations/conversation-view.ts`, `apps/litecowork-ui/src/conversations/conversation-stream.ts` (merge of streamed messages only), `apps/litecowork-ui/src-tauri/src/conversation_bridge.rs` (tests only, asserting pass-through), `apps/litecowork-ui/tests/conversation-view.test.ts`, `apps/litecowork-ui/tests/conversation-stream.test.ts`, `crates/storage-sqlite/src/conversations.rs` (provenance test only).
- **Contract:** SPEC §4, §7.2 `ConversationMessageView`, AC-014, AC-012; docs/schemas/operator-api.openapi.yaml `ConversationMessage`.
- **Change intent:** Add `agent_binding_id: string | null` and `agent_session_id: string | null` to `ConversationMessageView`; in `parseConversationSnapshot` accept `null` or a bounded id matching the existing id rule and throw "Conversation history contains an invalid message." otherwise; carry both through `mergeConversationStreamSnapshot` and `sameCommittedMessage` so a stream merge never erases them. Add a storage test that a committed AGENT message keeps `source_agent_binding_id` after later turns on other bindings.
- **Acceptance:**
  - [ ] AC-014: a snapshot with producer fields round-trips into `ConversationMessageView`; a malformed id is rejected, not dropped.
  - [ ] AC-012: after A, B, A turns, each committed AGENT message keeps its own producer binding.
  - [ ] Negative: a USER or SYSTEM_NOTICE message with null producer parses; a native handle-like value never appears in the type.
  - [ ] Pagination: an older page loaded after a newer one keeps producer fields on both.
- **Command:** `pnpm --dir apps/litecowork-ui test` ; `cargo test -p litecowork-ui --locked conversation_bridge` ; `cargo test -p storage-sqlite --locked conversation_message_provenance` (filter to be added by this task)
- **Evidence:** test names; docs/IMPLEMENTATION-STATUS.md 004 row "producer identity parsed".
- **Stop:** stop if the Operator response shape differs from the OpenAPI `ConversationMessage` schema.

## 004-T03 - Portable relay export with filtering, bounds and truncation receipt

- **Status:** PARTIAL (source cross-check 2026-11-13: `build_conversation_context_package` orders by event revision, excludes `SYSTEM_NOTICE`, carries `source_agent_binding_id`, and applies 1,024-message/4 MiB windows; the A→B→A builder test proves producer attribution and excludes native session handles. Feature 001 T14 reauthorizes ResourceRefs and materializes eligible text. The SQLite loader limits its message fetch to 1,024 rows but first scans for orphan messages and runs full-prefix COUNT/SUM queries, so AC-011's bounded-work guarantee remains unproven. Workspace instruction preamble and ArtifactVersionRef inputs are not assembled by the current Conversation path. Relay still lacks its B-15 refuse-with-receipt path and adapter-budget integration.)
- **Depends on:** T01
- **Cross-package prerequisites:** Feature 001 T14 owns eligible text ResourceRef authorization/materialization before dispatch. Map Workspace instruction revision pinning to the canonical Context owner. Feature 007 already owns exact ArtifactVersion reads; T03 must map the canonical `ArtifactVersionRef` to that existing authorization/read path without inventing a parallel type or duplicate Artifact API. Feature 003 is still required for WorkspaceRoot content ingestion; relay T03 must not duplicate the materializer or treat binary/root refs as supported.
- **Owner:** Conversation, Storage.
- **Allowed paths:** `crates/storage-core/src/conversation.rs`, `crates/storage-sqlite/src/conversations.rs` (`load_conversation_context_for_turn`), `apps/litecoworkd/src/agents/conversation_prompt.rs`, related unit tests in those files.
- **Contract:** SPEC §5 B-15, §7, §10, AC-002, AC-007, AC-008, AC-011.
- **Change intent:** Implement `PortableContextMaterializer::export_for_relay` over the existing builder and take the target adapter text budget as input. Apply B-15 exactly: if the portable package exceeds the effective bound, refuse dispatch and return an audited truncation receipt describing the omitted committed range and applicable bound; never send a silently shortened history. Define the receipt's durable association in T01 before implementation. Preserve Feature 001's same-Workspace ResourceRef authorization/materialization boundary and map refusals to canonical relay errors; assert no private field is present (`PrivateFieldPresent`). Read the prefix with one bounded, index-ordered query capped at 1,024 rows instead of loading full history.
- **Acceptance:**
  - [ ] AC-002: the export for B contains committed messages and allowed refs only; no A session id, native handle or transcript.
  - [ ] B-15: an over-budget history is refused with an audited truncation receipt, and no provider dispatch occurs; never silently shorten the package. AC-007 wording is tracked under A-66 for owner reconciliation before T01 promotion.
  - [ ] AC-008: reusing Feature 001's materializer rejects cross-Workspace refs; credentials and handles never reach the encoder.
  - [ ] AC-011: building over a 10,000-message Conversation reads at most 1,024 rows (query-count assertion).
- **Command:** `cargo test -p storage-core --locked conversation_context` ; `cargo test -p storage-sqlite --locked portable_context_export` (filter to be added by this task) ; `cargo test -p litecoworkd --locked conversation_prompt`
- **Evidence:** `cargo test -p storage-core --locked relay_context_preserves_each_producer_across_a_to_b_to_a` (1 passed, 115 filtered); test name and CODE limits in CURRENT-RUN.md.
- **Stop:** stop if the export would need any field from a native session record.

## 004-T04 - Relay admission: fresh revalidation, no fallback, single-flight, no Task writes

- **Status:** PARTIAL (`setConversationAgentBinding` commits future-turn selection with CAS and idempotency; admission checks a fresh compatible offer through `resolve_conversation_session_host_in_transaction`; single-flight and the descriptor gate are not enforced).
- **Depends on:** T03
- **Cross-package prerequisites:** Feature 001 single-flight in storage and durable dispatch outbox; Feature 002 fresh-descriptor gate on enable and admission.
- **Owner:** Conversation, Agent Runtime.
- **Allowed paths:** `crates/storage-sqlite/src/conversations.rs` (turn admission only), `apps/litecoworkd/src/conversation_operator.rs` (submit path validation only), `apps/litecoworkd/src/conversation_operator_tests.rs`, `apps/litecoworkd/tests/conversation_agent_relay.rs` (new).
- **Contract:** SPEC §5 B-16, §7, §11, AC-001, AC-003, AC-004, AC-006, AC-009.
- **Change intent:** At turn admission, revalidate the turn's pinned binding (enabled, lead-eligible, fresh offer, fresh descriptor once Feature 002 serves it) inside the admission transaction; on failure settle the request as rejected with the canonical unavailable code and keep the draft; never substitute the previous binding or Workspace default. Rely on Feature 001's single-flight for concurrent submits and add relay-specific tests. Assert no Task, Attempt, DelegationProfile, Environment, lease, grant or `task.lead_agent.changed.v1` row is written.
- **Acceptance:**
  - [ ] AC-001: switching A to B then submitting admits B only after revalidation.
  - [ ] AC-004: a stale or disabled B fails admission, zero native dispatch to A, draft preserved.
  - [ ] AC-009: an A turn still RUNNING makes a B submit fail with the Feature 001 in-progress code.
  - [ ] AC-006: Task/Attempt/lease/grant row counts are unchanged across A→B→A.
  - [ ] AC-003: returning to A shows B's committed messages in history.
  - [ ] Race: a binding switch committed while a submit is in flight affects only the next turn (pinned binding wins).
- **Command:** `cargo test -p litecoworkd --locked conversation_agent_relay` (filter to be added by this task) ; `cargo test -p storage-sqlite --locked conversation_turn_admission` (filter to be added by this task)
- **Evidence:** test names; docs/IMPLEMENTATION-STATUS.md relay admission row.
- **Stop:** stop if admission would need a network or process call inside the transaction.

## 004-T05 - Claude Code dispatch arm in AgentSessionSupervisor (B-16)

- **Status:** NOT STARTED (`execute_agent_turn` has one `"opencode"` arm and an `other =>` unsupported arm).
- **Depends on:** T03
- **Cross-package prerequisites:** Feature 002 OpenCode binding fix and `NativeProviderKey` (Wave 1); Feature 002 Claude Code CLI second module with pinned version, transport and auth (Wave 4); Feature 001 final-response commit with retry ordinal and provider-response identity (B-04 caveat) and native stop/quiescence.
- **Owner:** Agent Runtime, Conversation.
- **Allowed paths:** `apps/litecoworkd/src/agents/supervisor.rs`, `apps/litecoworkd/src/agents/claude_code.rs` (new adapter), `apps/litecoworkd/src/agents/mod.rs`, `apps/litecoworkd/src/agents/conversation_prompt.rs` (adapter budget input only), `apps/litecoworkd/tests/claude_code_conversation.rs` (new).
- **Contract:** SPEC §5 B-16, §7, §7.2 dispatch match, §10, AC-013, AC-002, AC-010; Feature 002 B-11; docs/AGENT-CONTROL.md.
- **Change intent:** Refactor `execute_agent_turn` to match `NativeProviderKey` and move the existing body into `run_opencode_turn`. Add `run_claude_code_turn`: resolve the executable from the binding's registered `endpoint_ref`, start the CLI in the non-interactive streaming mode pinned by the Feature 002 fixture with a cleared environment allow list (as `OpenCodeEnvironment::from_current_process` does), a new native session every turn and no resume flag, the Workspace root as working directory, the portable prompt on stdin, bounded stdout framing behind the Adapter 1.0 framed transport contract, process lifetime under Job Object (Windows) or process group (Linux) as lifetime control only, never an isolation claim. Normalize output as untrusted, then activate the session and commit through `commit_conversation_agent_response` exactly like the OpenCode arm. Unknown keys and Codex keep failing closed.
- **Acceptance:**
  - [ ] AC-013: a Claude Code binding turn creates a fresh AgentSession and native session and commits one AGENT message with `source_agent_binding_id` = the Claude Code binding, on Windows x64 and Linux x64.
  - [ ] AC-013 negative: Codex and unknown keys fail the turn with the unsupported reason and no native spawn; no fallback to OpenCode.
  - [ ] AC-002: the stdin payload contains only the portable prompt (captured in test).
  - [ ] Timeout-after-dispatch and malformed output settle the turn FAILED with the session marked lost; no partial message committed.
  - [ ] Security: no credential, cookie or native session handle is written to logs or projections.
- **Command:** `cargo test -p litecoworkd --locked claude_code_conversation` (filter to be added by this task) ; `cargo test -p litecoworkd --locked supervisor`
- **Evidence:** test names; per-OS transcript in CURRENT-RUN.md with CLI version.
- **Stop:** stop if the CLI cannot run non-interactively without credential copying, or if a fresh native session per turn cannot be guaranteed.

## 004-T06 - Picker gated on dispatchability and descriptor freshness

- **Status:** PARTIAL (`buildConversationAgentChoices` also filters provider keys to the current OpenCode supervisor arm and has an unsupported-provider regression case; no fresh-descriptor/current-offer/auth gating, and `deriveConversationAgentSurface` still has no production caller).
- **Depends on:** T05
- **Cross-package prerequisites:** Feature 002 descriptor producer and composer option projection (shared picker gating).
- **Owner:** Desktop UI, Agent Runtime.
- **Allowed paths:** `apps/litecowork-ui/src/conversations/conversation-agent-selection.ts`, `apps/litecowork-ui/src/conversations/conversation-agent-surface.ts`, `apps/litecowork-ui/src/conversations/ConversationsPage.tsx` (picker region only), `apps/litecowork-ui/tests/conversation-agent-selection.test.ts`, `apps/litecowork-ui/tests/conversation-agent-surface.test.ts`.
- **Contract:** contracts/ui-behavior.md "Target flows and states", AC-015, AC-004.
- **Change intent:** Feed the picker from the daemon's per-binding projection (dispatchable provider + descriptor freshness) and wire `deriveConversationAgentSurface` into production, reduced to rendering that projection. Non-selectable bindings render disabled with reason text ("No Conversation adapter for this agent", "Descriptor stale - refresh", "Sign-in required").
- **Acceptance:**
  - [ ] AC-015: a Codex binding and a stale-descriptor binding are visible but disabled with reasons; an OpenCode or Claude Code binding with a fresh descriptor is selectable.
  - [ ] AC-004: selecting a binding that turns stale before Send results in rejection with the draft kept.
  - [ ] Accessibility: disabled options expose `aria-disabled` plus the reason as description; keyboard navigation skips nothing silently.
- **Command:** `pnpm --dir apps/litecowork-ui test`
- **Evidence:** UI test names.
- **Stop:** stop if the projection would have to be computed in TypeScript from raw descriptors.

## 004-T07 - Relay UI states and producer label

- **Status:** PARTIAL (committed AGENT messages now render a label resolved from their stored binding, with an unknown-agent fallback; the existing binding-save notice already explains that a running reply keeps its agent. Full picker states, rendered accessibility checks and acceptance tests remain open).
- **Depends on:** T02, T06
- **Cross-package prerequisites:** Feature 011 app-wide keyboard and focus baseline.
- **Owner:** Desktop UI.
- **Allowed paths:** `apps/litecowork-ui/src/conversations/ConversationsPage.tsx` (transcript message header and picker notice), `apps/litecowork-ui/src/conversations/conversations-page.css`, `apps/litecowork-ui/tests/conversation-relay-ui.test.ts` (new).
- **Contract:** contracts/ui-behavior.md, AC-005, AC-004, AC-014.
- **Change intent:** Render on each committed AGENT message a producer label resolved from its stored `agent_binding_id` to the binding/profile display name, falling back to a neutral "Unknown agent" plus the binding id; never use the current picker value. Show the switch notice "Applies to your next message; the running reply keeps its agent". Show loading, empty, error, denied, offline, stale and recovery states for the picker and transcript.
- **Acceptance:**
  - [ ] AC-005: after reload, three turns A, B, A show OpenCode, Claude Code, OpenCode labels.
  - [ ] A deleted or disabled binding still labels its historical messages (neutral fallback).
  - [ ] Accessibility: the label is text (not color only), part of the message's accessible name, and readable by screen readers; contrast meets the 011 baseline.
- **Command:** `pnpm --dir apps/litecowork-ui test` ; `pnpm --dir apps/litecowork-ui run build`
- **Evidence:** UI test names; V1-UI-ACTION-STATE-LEDGER rows updated by T11.
- **Stop:** stop if the label would need data not present in the committed message.

## 004-T08 - Relay failure and recovery

- **Status:** NOT STARTED
- **Depends on:** T04, T05
- **Cross-package prerequisites:** Feature 001 durable dispatch outbox, startup sweep, UNKNOWN reconciliation and effective Stop with observed quiescence.
- **Owner:** Conversation, Agent Runtime.
- **Allowed paths:** `apps/litecoworkd/tests/conversation_agent_relay.rs`, `apps/litecoworkd/tests/claude_code_conversation.rs`, plus fixes inside files owned by T03-T05.
- **Contract:** SPEC §11, §12, contracts/errors-recovery.md, AC-010, AC-004, AC-009.
- **Change intent:** Exercise: daemon restart with a B turn RUNNING (reload committed only, session lost, no A replay, no uncommitted B output); B partial stream then switch back to A; timeout after dispatch on B reconciled through the 001 outbox as UNKNOWN then settled; duplicate submit replay returns the original turn; Stop on B racing a binding switch; Claude Code process killed mid-turn.
- **Acceptance:**
  - [ ] AC-010: after restart only committed turns load; A's prior session is never reused.
  - [ ] UNKNOWN outcomes are shown as unknown until reconciled; never as success.
  - [ ] Duplicate replay creates no second turn or message.
- **Command:** `cargo test -p litecoworkd --locked conversation_agent_relay` ; `python3 scripts/v1_resilience_matrix.py`
- **Evidence:** recovery report in CURRENT-RUN.md.
- **Stop:** stop if recovery depends on the native process still being alive.

## 004-T09 - Cross-layer trace for relay operations

- **Status:** NOT STARTED
- **Depends on:** T07, T08
- **Cross-package prerequisites:** Resolve A-95/A-96 with the Conversation, Workspace Context, Artifact and Storage owners before claiming the trace is complete; use the canonical Feature 007 exact-version read contract and identify the durable receipt owner/transaction without adding a duplicate schema.
- **Owner:** Conversation, Desktop UI.
- **Allowed paths:** this package (`contracts/ui-behavior.md`, `WIRING.yaml`), `docs/V1-UI-ACTION-STATE-LEDGER.md` (relay rows).
- **Contract:** AGENTS.md §4; AC-001, AC-005, AC-013, AC-015.
- **Change intent:** For Switch agent, Send on B, Return to A and Reload history, record control -> Tauri command (`set_conversation_agent_binding`, turn submit) -> Operator route -> authorization -> ConversationService -> transaction/CAS -> event (`conversation.agent_binding.changed.v1`, `conversation.turn.created.v2`, settlement) -> projection -> restart -> evidence. Replace the generic WIRING nodes (`owner`, `harness_b`) with named owners and the Claude Code edge.
- **Acceptance:**
  - [ ] Every relay control has a complete trace with real symbol names and a test id.
  - [ ] WIRING edges name the real services and keep `external_calls_inside: false`.
- **Command:** `python3 .specify/scripts/validate_feature_package.py` ; `python3 scripts/audit_served_routes.py`
- **Evidence:** trace tables in contracts/ui-behavior.md.
- **Stop:** stop if a trace step cannot be named from source.

## 004-T10 - E2E-02 qualification on the release matrix

- **Status:** NOT STARTED
- **Depends on:** T09
- **Cross-package prerequisites:** Feature 002 qualification of both modules; Feature 001 E2E-01; Feature 019 packaged installer and release matrix.
- **Owner:** Conversation, Product (owner USER evidence).
- **Allowed paths:** evidence only: `CURRENT-RUN.md`, `docs/IMPLEMENTATION-STATUS.md` (004 rows).
- **Contract:** docs/ACCEPTANCE-AND-E2E-PLAN.md E2E-02; AC-001 to AC-015.
- **Change intent:** On the packaged desktop, Windows x64: OpenCode 2.0.32 turn, switch to Claude Code, turn, switch back, turn, reload, restart. Repeat on Linux x64. Include denial (stale B, unsupported binding), duplicate replay, timeout-after-dispatch and restart. WSL2 is not evidence.
- **Acceptance:**
  - [ ] CODE: all task commands green at one recorded revision.
  - [ ] SYSTEM: packaged run per row with real harnesses.
  - [ ] USER: owner performs E2E-02 per row and records pass/fail.
  - [ ] Evidence shows no private transcript or credential transfer (export capture).
- **Command:** `python3 scripts/v1_platform_matrix.py` ; `cargo test -p litecoworkd --locked conversation_agent_relay`
- **Evidence:** evidence bundle per row (build, OS, both harness versions, auth routes).
- **Stop:** stop if either harness is not qualified on the row under Feature 002.

## 004-T11 - Documentation and ledger sync

- **Status:** NOT STARTED
- **Depends on:** T10
- **Cross-package prerequisites:** orchestrator-owned register and traceability updates.
- **Owner:** Conversation, documentation owner.
- **Allowed paths:** `docs/IMPLEMENTATION-STATUS.md`, `docs/schemas/served-route-inventory.json` (regenerated), `docs/V1-UI-ACTION-STATE-LEDGER.md`, this package.
- **Contract:** AGENTS.md §1 item 6; SPEC §4.
- **Change intent:** Update SPEC §4 and IMPLEMENTATION-STATUS to the evidence level recorded in T10, regenerate the served-route inventory, update UI ledgers, and ask the orchestrator to close B-15/B-16 register rows as implemented.
- **Acceptance:**
  - [ ] No claim exceeds T10 evidence; the inventory matches source.
- **Command:** `python3 scripts/audit_served_routes.py` ; `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** diffs of status and inventory.
- **Stop:** stop if a doc would claim a higher level than recorded.

## 004-T12 - Package VERIFIED for V1

- **Status:** NOT STARTED
- **Depends on:** T01-T11
- **Cross-package prerequisites:** Feature 001 and Feature 002 VERIFIED records for shared rows.
- **Owner:** Product (owner sign-off), Conversation.
- **Allowed paths:** this package (status lines), `CURRENT-RUN.md`.
- **Contract:** AGENTS.md §3 lifecycle; AC-001 to AC-015.
- **Change intent:** Assemble the evidence bundle (per-task revision, command, result, OS, harness versions, auth routes, reviewer), move status ACCEPTED/LOCKED -> IMPLEMENTING -> VERIFIED and record owner sign-off.
- **Acceptance:**
  - [ ] Every AC-001 to AC-015 maps to a passing recorded test and, where user-visible, a USER record.
  - [ ] Owner sign-off recorded with date and revision.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** evidence bundle index in CURRENT-RUN.md.
- **Stop:** stop if any release-matrix row lacks USER evidence.

## Dependency order

T01 and T02 start in parallel (T02 needs no new contract). T01 -> T03 -> T04 and T03 -> T05. T05 -> T06. T02 + T06 -> T07. T04 + T05 -> T08. T07 + T08 -> T09 -> T10 -> T11 -> T12.

## Cross-package dependencies

Cross-package task numbers are being re-authored in parallel, so this table names the owning package and outcome instead of a task number.

| This task | Other package (outcome) | Reason |
|---|---|---|
| T01 | Feature 001 (context dispatch contract, B-07 budget) | The relay reuses 001's package bounds. |
| T03 | Feature 001 (dispatch-time text ResourceRef materializer); Feature 003 (WorkspaceRoot ingestion) | Reuse 001's authorization/materialization and cross-Workspace denial; 003 is needed only for root-file admission/indexing, not to create a duplicate Resource materializer. |
| T04 | Feature 001 (single-flight, outbox), Feature 002 (fresh-descriptor gate) | Admission semantics are owned there. |
| T05 | Feature 002 (OpenCode binding fix, `NativeProviderKey`, Claude Code module), Feature 001 (final-response commit, stop/quiescence) | The arm needs a pinned module and a durable commit path. |
| T06 | Feature 002 (descriptor projection, composer option gating) | Shared picker gating. |
| T07 | Feature 011 (keyboard and focus baseline) | App-wide accessibility baseline. |
| T08 | Feature 001 (outbox, startup sweep, UNKNOWN reconciliation, Stop) | Recovery is 001-owned; 004 adds relay cases. |
| T10 | Feature 002 (both modules qualified), Feature 019 (release matrix, installer) | Packaged E2E-02 rows. |

## Completion evidence

For each task record in CURRENT-RUN.md: task id, git revision, exact command(s), result (pass/fail with test counts), OS and build (Windows 11/10 x86_64-pc-windows-msvc or Linux x86_64), harness versions (OpenCode 2.0.32 and the pinned Claude Code CLI version), auth routes, reviewer and date. A task is DONE only when its acceptance boxes are checked against that record. Evidence levels follow AGENTS.md §5.
