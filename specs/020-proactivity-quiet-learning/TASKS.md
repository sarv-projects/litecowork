# Tasks - Proactivity and Quiet Learning

**Feature:** 020 | **Status:** IN_REVIEW - not formally owner-accepted. By repository-owner delegation on 2026-11-20, only the isolated pure-gate repair in T03 may proceed; this does not accept the package or unblock dependent product work. Tasks not started unless marked.
**Spec revision:** 0.2 | **Release target:** V1 (release matrix rows: Windows x64 + OpenCode v2.0.32 first; Linux x64 second)
**Owners:** Responsibility domain, Coworker, Storage, Operator/API, Desktop UI, Security/Trust, Product.

## Current baseline

Historical baseline verified on branch `docs/audit-competitor-superset` (HEAD 234c564); it is not current checkout evidence. See CURRENT-RUN.md and docs/IMPLEMENTATION-STATUS.md for the current source state.

- **Suggestions (SERVED, no producer):** `apps/litecoworkd/src/suggestions_operator.rs` mounts `GET /v1/suggestions` (runs `SuggestionService::settle_expired` first), `POST /v1/suggestions/{suggestion_id}/resolve`, `/accept-task`, `/snooze`, `GET /v1/workspaces/{workspace_id}/suggestion-preferences` and `PUT .../{kind}`; merged in `apps/litecoworkd/src/operator.rs`. Domain `crates/domain-responsibility/src/suggestions.rs` ("proposal production remains separate"); storage `crates/storage-sqlite/src/suggestions.rs` (`SqliteSuggestionStore`, `create_task_from_suggestion`); schema `docs/schemas/sqlite-v1.sql` table `suggestions`, `uq_suggestions_open_dedupe`, `idx_suggestions_dismissal_cooldown`, `suggestion_preferences`. No production code inserts a Suggestion; `suggestion.proposed.v1` is in `docs/schemas/domain-event.schema.json` with no producer.
- **Suggestion UI:** `apps/litecowork-ui/src-tauri/src/suggestions_bridge.rs` (`list_suggestions`, `suggestion_owner_action` for dismiss/snooze/unsnooze, `accept_suggestion_task`, `list_suggestion_preferences`, `set_suggestion_preference`); `apps/litecowork-ui/src/suggestions/SuggestionsPage.tsx` ("Ideas"), `suggestion-api.ts`, `desktop-suggestion-api.ts`, `suggestion-request-scope.ts`; tests `apps/litecowork-ui/tests/desktop-suggestion-api.test.ts`, `suggestion-request-scope.test.ts`.
- **Goals (SERVED):** `apps/litecoworkd/src/goal_operator.rs` (`/v1/goals`, `/v1/goals/{goal_id}`, `/revisions`, `/status`), `crates/domain-responsibility/src/goals.rs` (`GoalService`, `GoalProgressProjection`), `apps/litecowork-ui/src/goals/GoalsPage.tsx`, Tauri `goal_bridge::goal_request`.
- **Needs You (SERVED, read-only):** `apps/litecoworkd/src/needs_you_operator.rs` (`GET /v1/needs-you`, `no-store` middleware), `crates/storage-core/src/needs_you.rs`, tests `apps/litecoworkd/src/needs_you_operator_tests.rs`; UI `apps/litecowork-ui/src/needs-you/TaskAttentionPage.tsx` and `needs-you-*` tests.
- **Learning (CODE, no production caller):** canonical gates in `crates/domain-responsibility/src/learning_gates.rs` (34 tests); `crates/storage-core/src/coworker_learning.rs` and crate-root exports retain compatibility. `redact_sensitive_content` is reused by `crates/storage-core/src/skill.rs`. No learning table, service, route, event producer or UI. `apps/litecowork-ui/src/coworkers/CoworkerSettings.tsx` discloses that quiet learning is inactive.
- **Quiet hours (Feature 010 domain):** `crates/domain-responsibility/src/notifications.rs` `QuietHours::{parse, contains}`; `notification_preferences.quiet_hours_json` per Workspace and event class; `NotificationEventClass` has no Suggestion or learning class.
- **Migrations:** the current chain includes `docs/schemas/sqlite-v34.sql`; v44 is the current provisional allocation in docs/LITECOWORK-BUILD-TASKS.md and must still be rechecked if another migration is wired before T02.

Gaps to V1: no producer and no `propose`; no quiet-hours behavior; no learning pipeline; Goals/Needs You conformance work remains; no qualification. T03 closes the pure-gate defects only and does not reduce or complete the full feature scope.

## 020-T01 - Owner acceptance of SPEC, D-01 and blocker B-45

- **Status:** BLOCKED (D-01 and B-45 accepted as proposed 2026-11-10; pending formal owner acceptance of SPEC revision 0.2; owner: repository owner, sarv-projects. The delegated T03 direction does not change this status).
- **DEFERRED/BLOCKED note:** Why: whole-SPEC acceptance pending although D-01 and B-45 are accepted. Unblocks when: owner reviews and accepts the SPEC. Test later at: decision only.
- **Depends on:** NONE
- **Cross-package prerequisites:** NONE
- **Owner:** Product (repository owner), Responsibility domain, Security.
- **Allowed paths:** `specs/020-proactivity-quiet-learning/` (status, revision, decision rows only).
- **Contract:** SPEC section 5; SPECIFICATION-STANDARD section 7; AC-010, AC-021.
- **Change intent:** record the owner decision on D-01 and B-45, move SPEC and WIRING to ACCEPTED/LOCKED with an owner-acceptance line, bump revision to 1.0. The orchestrator updates the register row for B-45.
- **Acceptance:**
  - [ ] D-01 and B-45 rows say ACCEPTED (date) or carry the owner's amendment.
  - [ ] SPEC and WIRING status match; validator passes.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** owner decision link in Completion evidence.
- **Stop:** owner rejects B-45; learning tasks stay BLOCKED and the UI keeps the "not available" disclosure.

## 020-T02 - Promote accepted interfaces into owner docs and schemas

- **Status:** NOT STARTED.
- **Depends on:** T01
- **Cross-package prerequisites:** Feature 010 review of the new notification event classes; Feature 009 review of learned-memory storage as Coworker-scoped ContextDocument revisions.
- **Owner:** Responsibility, Storage, API, Events owners.
- **Allowed paths:** `docs/RESPONSIBILITIES.md`, `docs/LEARNING.md`, `docs/API.md`, `docs/EVENTS.md`, `docs/SCHEMAS.md`, `docs/STORAGE.md`, `docs/ACCEPTANCE-AND-E2E-PLAN.md`, `docs/schemas/operator-api.openapi.yaml`, `docs/schemas/error-codes.schema.json`, `docs/schemas/domain-event.schema.json`, `docs/schemas/sqlite-v29.sql` (new; use the next free number at implementation time), this package.
- **Contract:** SPEC sections 5, 7.2, 8; LEARNING sections 18, 19; AC-001, AC-008.
- **Change intent:** in one reviewed change: extend `SuggestionService.propose` outcomes in RESPONSIBILITIES with SUPPRESSED_DUPLICATE, SUPPRESSED_BUDGET and SUPPRESSED_SCOPE plus the D-01 budgets and the quiet-hours push rule; change the LEARNING vocabulary note from PROPOSED to accepted (A-20/A-34, 2026-11-09) and record B-45; promote the LEARNING section 18 routes and the feedback route into OpenAPI; add `learning.candidate.created.v1`, `learning.candidate.status.changed.v1`, `learning.memory.committed.v1`, `learning.memory.revoked.v1`, `coworker.learning_policy.revised.v1` payload branches (ids, digests, reason codes only); add error codes for unsupported policy mode, stale candidate, Coworker not active and source ineligible; add the migration for `learning_candidates`, `coworker_learning_policies`, `memory_records`, `learning_work_items`, `suggestion_producer_cursors`; add a proposed learning journey to ACCEPTANCE (Coworker quiet learning, review, undo, restart). Every name is labelled "accepted target interface (not yet in source)" until its task lands.
- **Acceptance:**
  - [ ] Each promoted name exists once in its owner doc and machine schema.
  - [ ] Event schema rejects a learning payload containing a `body`, `text` or `value` field.
  - [ ] Migration is the next immutable `sqlite-vNN.sql`; older migrations byte-identical; fingerprint test passes.
- **Command:** `python3 .specify/scripts/validate_feature_package.py` and `python3 scripts/validate_architecture.py` and `cargo test -p storage-sqlite --locked migration`
- **Evidence:** PR link; command output.
- **Stop:** a name conflicts with an existing owner definition; resolve at the owner.

## 020-T03 - Fix the deterministic learning gate defects (Rust)

- **Status:** DONE (CODE; narrow implementation direction delegated by repository owner on 2026-11-20; package remains IN_REVIEW and T01 remains formally BLOCKED).
- **Depends on:** T01
- **Cross-package prerequisites:** NONE
- **Owner:** Responsibility, Security.
- **Allowed paths:** `crates/domain-responsibility/src/learning_gates.rs` (canonical pure gate implementation and tests), `crates/domain-responsibility/src/lib.rs` (module/re-exports), `crates/storage-core/src/coworker_learning.rs` (compatibility re-export), `crates/storage-core/src/lib.rs` (compatibility re-exports only), `crates/storage-core/src/skill.rs` (only if the redactor signature changes), `specs/020-proactivity-quiet-learning/{README.md,SPEC.md,PLAN.md,TASKS.md,RESEARCH.md}`, `docs/LEARNING.md`, `docs/IMPLEMENTATION-STATUS.md`, `CURRENT-RUN.md`, and `docs/audit/EXTERNAL-AI-AUDIT-DISPOSITION-2026-11-19.md`.
- **Contract:** SPEC section 7.1; LEARNING sections 6, 16, 19; AC-010, AC-011.
- **Change intent:** make `source_kind_matches_channel` a strict one-kind-per-channel mapping (`OwnerMessage` to `UserExplicit` or `UserCorrection`, `OwnerFeedback` to `UserFeedback` or `UserCorrection`, `OwnerDemonstration` to `Demonstration`, `VerifiedTaskOutcome` to `TaskOutcome` or `RepeatedPattern`); remove `LearningSourceChannel::permitted_source_kind` or change it to return `Option<LearningSourceKind>` with `None` for untrusted channels; add `LearningDisposition::as_str` returning `AUTO_COMMIT`, `NEEDS_REVIEW`, `REJECTED`, `DEDUPLICATED` and use it in `learning_candidate_event_payload` instead of `format!("{:?}")`. Decide F-04 (keep or move to `domain-responsibility`) and record it; a move keeps one definition with a re-export.
- **Acceptance:**
  - [x] Regression: a `USER_EXPLICIT` candidate from `VERIFIED_TASK_OUTCOME` is REJECTED with `SOURCE_KIND_CHANNEL_MISMATCH` (AC-010).
  - [x] Event payload `disposition` values are the four stable tokens.
  - [x] All 30 original tests and the added source-mapping/token regressions pass (34 tests); existing secret-redaction fixtures remain blocked (AC-011).
- **Command:** `cargo test -j 1 -p domain-responsibility --locked learning_gates -- --nocapture`; `cargo test -j 1 -p storage-core --locked skill -- --nocapture`.
- **Evidence:** native Windows: domain gate tests **34/34 passed**; storage-core Skill consumer tests **50/50 passed**. The red-first storage-core run failed the new spoofing regression as expected (`AutoCommit` vs `Rejected`); after the fix the moved gate suite passed. CODE only; no learning pipeline or qualification.
- **Stop:** a change would widen eligibility; revert.

## 020-T04 - SuggestionService.propose and the admission transaction

- **Status:** NOT STARTED (no `propose`; schema indexes for dedupe and cooldown exist).
- **Depends on:** T01, T02
- **Cross-package prerequisites:** NONE
- **Owner:** Responsibility, Storage.
- **Allowed paths:** `crates/domain-responsibility/src/suggestions.rs`, `crates/domain-responsibility/src/lib.rs`, `crates/domain-responsibility/src/tests.rs`, `crates/storage-sqlite/src/suggestions.rs`, `crates/storage-sqlite/src/suggestion_propose_tests.rs` (new).
- **Contract:** SPEC sections 5 (L-03, D-01), 7.2, 7.3; RESPONSIBILITIES "Operator API and services"; AC-001, AC-002, AC-004, AC-007.
- **Change intent:** add `SuggestionCandidate`, `ProposeOutcome`, the `SuggestionProposalStore` port and `SuggestionService::propose`; implement `SqliteSuggestionStore::propose` as one IMMEDIATE transaction checking Workspace active, Coworker ACTIVE when scoped, registered producer id, pinned source/Goal revision visibility, reason and spec size bounds, expiry within 1 h to 30 days, mute, 30-day dismissal cooldown (`idx_suggestions_dismissal_cooldown`), open dedupe key (`uq_suggestions_open_dedupe`), producer 24-hour budget and the 20-open cap; compute `kind` from `proposed_action` and `dedupe_key` from normalized action, target and pinned refs; append `suggestion.proposed.v1` and a receipt keyed by producer request id.
- **Acceptance:**
  - [ ] One test per outcome: Created, SuppressedMuted, SuppressedCooldown, SuppressedDuplicate, SuppressedBudget, SuppressedScope (AC-001).
  - [ ] Suppression writes no row, event or receipt with candidate text (AC-002).
  - [ ] Budget boundaries at 2/3 per producer and 19/20/21 open (AC-004).
  - [ ] Concurrent same-key proposals leave one PROPOSED row; mute racing propose leaves no visible new row (AC-007).
  - [ ] Exact replay of a producer request id returns the original outcome.
- **Command:** `cargo test -p storage-sqlite --locked suggestion_propose` (filter to be added by this task) and `cargo test -p domain-responsibility --locked suggestion`
- **Evidence:** test output.
- **Stop:** a check needs an external call inside the transaction; move it before the transaction.

## 020-T05 - Deterministic producers and restart-safe cursor (D-01)

- **Status:** NOT STARTED.
- **Depends on:** T01, T04
- **Cross-package prerequisites:** Feature 005 Task creation events and owner-created Task intent digest; Feature 009 Coworker status for scoped producers.
- **Owner:** Responsibility, Runtime.
- **Allowed paths:** `apps/litecoworkd/src/suggestion_producers.rs` (new), `apps/litecoworkd/src/suggestion_producer_worker.rs` (new), `apps/litecoworkd/src/runtime.rs` (worker start only), `crates/storage-sqlite/src/suggestion_producer_cursor.rs` (new), adjacent tests.
- **Contract:** SPEC section 5 (D-01), 6, F-01; AC-001, AC-016, AC-017.
- **Change intent:** `SuggestionProducer` trait and registry with `GOAL_NEXT_STEP` and `REPEATED_TASK`; a bounded worker reads the event journal after a persisted cursor (`suggestion_producer_cursors`), calls producers outside any transaction, calls `SuggestionService::propose`, then advances the cursor in its own transaction after each batch. Producers read only committed state and copy only owner-authored text into `reason` and `proposed_task_spec`.
- **Acceptance:**
  - [ ] Goal with no nonterminal linked Task for 7 days yields one TASK_OPPORTUNITY pinned to the Goal revision; a revised Goal yields a new dedupe key.
  - [ ] Three equal-intent owner Tasks in 14 days yield one AUTOMATION_OPPORTUNITY with `OPEN_AUTOMATION_EDITOR`.
  - [ ] Crash after propose and before cursor advance does not create a duplicate (AC-016).
  - [ ] PAUSED or ARCHIVED Coworker gets no scoped proposal (AC-017).
  - [ ] Negative: an unregistered producer id is rejected (AC-001).
- **Command:** `cargo test -p litecoworkd --locked suggestion_producer` (filter to be added by this task)
- **Evidence:** test output.
- **Stop:** a producer needs a model call; that is out of D-01 scope.

## 020-T06 - Quiet hours on Suggestion and memory push surfaces

- **Status:** NOT STARTED (QuietHours domain type exists in Feature 010's domain; no Suggestion or learning event class).
- **Depends on:** T01, T04
- **Cross-package prerequisites:** 010-T07 (quiet-hours control surface, A-35) plus new Feature 010 notification event classes; 011-T10 (Settings Notifications entry hosting Feature 010's quiet-hours component).
- **Owner:** Responsibility, Desktop UI; Feature 010 for delivery.
- **Allowed paths:** `crates/domain-responsibility/src/suggestions.rs` (projection helper only), `apps/litecoworkd/src/suggestions_operator.rs` (projection field only), `apps/litecowork-ui/src/App.tsx` (Home prominent idea slot only), `apps/litecowork-ui/src/suggestions/suggestion-api.ts`, `apps/litecowork-ui/tests/suggestion-quiet-hours.test.ts` (new).
- **Contract:** SPEC sections 5 (B-45 item 5), 12; COWORKER-FLOWS CF14; AC-005.
- **Change intent:** read the Workspace's quiet-hours window through Feature 010's contract and `QuietHours::contains`; add a projection flag `push_eligible` on listed Suggestions so the Home prominent idea and any notification request skip new proposals during the window; request one digest notification after the window through Feature 010's delivery path; memory-commit notices follow the same rule. This package defines no quiet-hours editor.
- **Acceptance:**
  - [ ] During quiet hours: no Home prominent idea for a new proposal, no notification; Ideas page still lists it (AC-005).
  - [ ] After the window: at most one digest per Workspace.
  - [ ] Invalid or missing quiet hours: behaves as no window, never as always-quiet.
- **Command:** `cargo test -p domain-responsibility --locked quiet` and `pnpm --dir apps/litecowork-ui test`
- **Evidence:** clock-controlled test output.
- **Stop:** quiet-hours semantics would be redefined here; link to Feature 010 instead.

## 020-T07 - Ideas page and Home idea completion

- **Status:** PARTIAL - `SuggestionsPage.tsx` has visibility tabs, accept/dismiss/snooze/unsnooze, preference mute with unresolved-mutation recovery and a live region; Home and drawer limits and the provenance card content are not verified.
- **Depends on:** T01, T04, T06
- **Cross-package prerequisites:** NONE
- **Owner:** Desktop UI.
- **Allowed paths:** `apps/litecowork-ui/src/suggestions/`, `apps/litecowork-ui/src/App.tsx` (Home idea slot only), `apps/litecowork-ui/tests/suggestions-page.test.mjs` (new), `apps/litecowork-ui/tests/desktop-suggestion-api.test.ts`.
- **Contract:** RESPONSIBILITIES "Suggestions" projection limits; [contracts/ui-behavior.md](contracts/ui-behavior.md); AC-003, AC-006, AC-022.
- **Change intent:** card shows reason, pinned sources, what acceptance creates (READY Task or editor), permissions that may be required and "This does not start work"; Home shows at most one prominent idea and the drawer at most three; states loading, empty ("No ideas right now"), error, offline, stale (version conflict refresh), denied; accept TASK navigates to the READY Task without starting it.
- **Acceptance:**
  - [ ] Card content matches AC-022 for each kind.
  - [ ] Accepting TASK shows the READY Task and no planning indicator (AC-003).
  - [ ] Snoozed items hidden until time; expiry wins (AC-006).
  - [ ] Keyboard and screen-reader checks pass for every control.
- **Command:** `pnpm --dir apps/litecowork-ui test`
- **Evidence:** test output; screenshots per state.
- **Stop:** the UI would infer authority from a Suggestion; remove that path.

## 020-T08 - Learning storage with CAS, idempotency and Resource-backed payloads

- **Status:** BLOCKED (B-45 pending owner; owner: repository owner).
- **DEFERRED/BLOCKED note:** Why: blocked on T01 SPEC acceptance and the migration v44 allocation. Unblocks when: T01. Test later at: any Linux host (storage tests).
- **Depends on:** T01, T02, T03
- **Cross-package prerequisites:** 009-T04 (COWORKER_NOTES ContextDocument re-widen, A-18/B-24 rider) for memory text; Feature 003 Resource revision commit and purge.
- **Owner:** Storage, Responsibility.
- **Allowed paths:** `crates/storage-core/src/learning_store.rs` (new port), `crates/storage-core/src/lib.rs`, `crates/storage-sqlite/src/learning.rs` (new), `crates/storage-sqlite/src/learning_tests.rs` (new), `crates/storage-sqlite/src/lib.rs` (module registration).
- **Contract:** SPEC section 8; LEARNING sections 3, 7, 20; DATA-MODEL; AC-008, AC-016.
- **Change intent:** port `LearningStore` with `insert_candidate`, `transition_candidate(expected_version)`, `commit_memory(candidate_id, expected_candidate_version, expected_policy_revision, ...)`, `revoke_memory(expected_version)`, `revise_policy(expected_revision)`, `list_candidates`, `list_memory`, `enqueue_work_item`, `claim_work_item`; every mutation IMMEDIATE with an idempotency receipt and events from T02; the candidate row stores `proposed_payload_ref` and `redaction_report_ref` only; a unique index on (coworker_id, normalized memory key, source lineage) for ACTIVE memory prevents duplicates; candidate state transitions enforced in SQL triggers as well as code.
- **Acceptance:**
  - [ ] Illegal transitions (terminal to any, NEW to COMMITTED) fail with no write.
  - [ ] Stale version, wrong Workspace, wrong Coworker and foreign principal fail with no write.
  - [ ] Duplicate commit of the same normalized key and lineage returns the existing record (AC-016).
  - [ ] Event payloads contain no candidate text (AC-008).
  - [ ] Fault injection after each insert rolls back everything.
- **Command:** `cargo test -p storage-sqlite --locked learning` (filter to be added by this task)
- **Evidence:** test output.
- **Stop:** the payload would be stored inline; keep it Resource-backed.

## 020-T09 - LearningService and LearningClassifier port (B-45)

- **Status:** BLOCKED (B-45 pending owner; Feature 009 Coworker-owned Conversation admission absent, owner Coworker domain).
- **DEFERRED/BLOCKED note:** Why: blocked on T01 and on Feature 009 Coworker-owned Conversation admission. Unblocks when: T01 and Feature 009. Test later at: native Linux with a real agent.
- **Depends on:** T01, T03, T08
- **Cross-package prerequisites:** 009-T02 (immutable Coworker origin on Conversations); Feature 001 settled-turn commit as checkpoint; Feature 001 qualified OpenCode v2.0.32 route if the optional extractor uses the native lead.
- **Owner:** Responsibility, Agent Runtime (extractor adapter), Security.
- **Allowed paths:** `crates/domain-responsibility/src/learning.rs` (new), `crates/domain-responsibility/src/learning_tests.rs` (new), `crates/domain-responsibility/src/lib.rs`, `apps/litecoworkd/src/learning_worker.rs` (new), `apps/litecoworkd/src/learning_classifier.rs` (new), `apps/litecoworkd/src/runtime.rs` (worker start only).
- **Contract:** SPEC sections 5 (B-45), 6 (sequence), 7.2; LEARNING sections 6, 11, 16, 21; AC-009, AC-010, AC-011, AC-012, AC-015.
- **Change intent:** `LearningService::record_owner_action` (Remember this, Correct), `observe_settled_turn`, `evaluate_and_settle`, `approve`, `dismiss`, `revoke_memory`, `revise_policy`; the worker claims bounded work items after turn settlement, writes payload and redaction report Resources first, then inserts the candidate, evaluates with `evaluate_learning_candidate` and commits with the in-transaction recheck. `LearningClassifier` returns `ProposedMemory` values marked `MODEL_OUTPUT`; CONFIRMED only when the cited span is a verbatim substring of the committed owner message and matches the V1 English lexicon; otherwise INFERRED. Classifier calls have a deadline and a per-Coworker rate limit; failure skips learning.
- **Acceptance:**
  - [ ] Confirmed preference under AUTO_CONFIRMED_PRIVATE commits quietly; under REVIEW_ALL stays NEEDS_REVIEW (LEARNING tests 1, 2).
  - [ ] Policy revised, Coworker archived or memory edited between evaluation and commit: no commit, NEEDS_REVIEW or SUPERSEDED with reason (AC-009).
  - [ ] Web/email/tool/model/worker/connector content and TEMPORARY or non-Coworker Conversations never auto-commit (AC-010).
  - [ ] Forbidden content never reaches a Resource; review-required goes to NEEDS_REVIEW (AC-011).
  - [ ] Instruction, Skill, Routine and Automation candidates always NEEDS_REVIEW (AC-012).
  - [ ] Classifier timeout or outage: Conversation turn settles normally; skipped count increments (AC-015).
- **Command:** `cargo test -p domain-responsibility --locked learning` and `cargo test -p litecoworkd --locked learning_worker` (filters to be added by this task)
- **Evidence:** test output.
- **Stop:** learning work appears on the Send path or blocks settlement; move it after commit.

## 020-T10 - Learning Operator routes, authorization and Tauri bridge

- **Status:** BLOCKED (B-45 pending owner).
- **DEFERRED/BLOCKED note:** Why: blocked on T01. Unblocks when: T01. Test later at: any Linux host.
- **Depends on:** T01, T02, T09
- **Cross-package prerequisites:** NONE
- **Owner:** Operator/API, Desktop Shell.
- **Allowed paths:** `apps/litecoworkd/src/learning_operator.rs` (new), `apps/litecoworkd/src/learning_operator_tests.rs` (new), `apps/litecoworkd/src/operator.rs` (route merge only), `apps/litecowork-ui/src-tauri/src/learning_bridge.rs` (new), `apps/litecowork-ui/src-tauri/src/lib.rs` (command registration only), `docs/schemas/served-route-inventory.json` (regenerated).
- **Contract:** SPEC section 7.2; LEARNING section 18; API; AC-014, AC-021.
- **Change intent:** mount the promoted learning routes and the message feedback route behind `authenticate`, Workspace and Coworker scope, `Idempotency-Key` and `If-Match`; responses `no-store`; reject `AUTO_CONFIRMED_AND_LOW_RISK_INFERRED_PRIVATE`; a missing policy reads as REVIEW_ALL. Tauri commands mirror `suggestions_bridge.rs` validation (bounded ids, fixed operation set, no caller-chosen URL or Workspace authority).
- **Acceptance:**
  - [ ] Unauthenticated, wrong Workspace, wrong Coworker and stale version requests fail with typed errors and no write.
  - [ ] Inferred auto mode rejected; missing policy reads REVIEW_ALL (AC-021).
  - [ ] Revoke returns REVOKED and later context assembly excludes it (AC-014).
  - [ ] Served-route inventory regenerated and matches.
- **Command:** `cargo test -p litecoworkd --locked learning_operator` (filter to be added by this task) and `cargo test --manifest-path apps/litecowork-ui/src-tauri/Cargo.toml --locked learning` and `python3 scripts/audit_served_routes.py`
- **Evidence:** test output; inventory diff.
- **Stop:** a route would return candidate text in a list response beyond the bounded preview.

## 020-T11 - Learning review list, Remembered notice, policy and memory controls

- **Status:** BLOCKED (B-45 pending owner); `CoworkerSettings.tsx` shows the inactive-learning disclosure today.
- **DEFERRED/BLOCKED note:** Why: blocked on T01. Unblocks when: T01. Test later at: a rendered browser run (none available here).
- **Depends on:** T01, T10, T06
- **Cross-package prerequisites:** 009-T09 (Coworker chats and Memory & Knowledge UI host) and Coworker onboarding (`HomeCoworkerOnboarding.tsx`); Feature 001 message actions slot in the Conversation view.
- **Owner:** Desktop UI.
- **Allowed paths:** `apps/litecowork-ui/src/learning/` (new), `apps/litecowork-ui/src/coworkers/CoworkerSettings.tsx` (learning policy section only), `apps/litecowork-ui/src/coworkers/HomeCoworkerOnboarding.tsx` (learning choice step only), `apps/litecowork-ui/src/conversations/ConversationsPage.tsx` (Remember this action and Remembered notice slot only), `apps/litecowork-ui/tests/learning-*.test.mjs` (new).
- **Contract:** LEARNING section 17; [contracts/ui-behavior.md](contracts/ui-behavior.md); AC-013, AC-014.
- **Change intent:** Learning review list in the Coworker view (Needs review with Review diff or Approve and Dismiss); "Remembered: ... [View]" notice with View (source, stored value, scope, certainty), Undo and Correct; "Remember this" on owner messages in Coworker Conversations only; policy control with OFF, REVIEW_ALL, AUTO_CONFIRMED_PRIVATE and the LEARNING section 4 explanation; memory list with Revoke. Pure state reducers in `learning-state.ts` for testability.
- **Acceptance:**
  - [ ] Loading, empty, error, denied, offline, stale and unavailable states render specific reasons (AC-013).
  - [ ] No modal on commit; notice announced through a polite live region; Undo works by keyboard (AC-013).
  - [ ] Undo and Revoke move the memory to REVOKED and the next reply no longer uses it (AC-014).
  - [ ] Remember this is absent in TEMPORARY and non-Coworker Conversations.
- **Command:** `pnpm --dir apps/litecowork-ui test`
- **Evidence:** test output; screenshots.
- **Stop:** a control has no served owner route; render it unavailable instead.

## 020-T12 - Goals and Needs You owner conformance

- **Status:** PARTIAL - routes, services, UI and Needs You tests exist; no negative test that Goal commands create no Task or schedule; no explicit no-mutation assertion for Needs You.
- **Depends on:** T01
- **Cross-package prerequisites:** NONE
- **Owner:** Responsibility, Operator/API.
- **Allowed paths:** `apps/litecoworkd/src/goal_operator.rs` (tests module only), `apps/litecoworkd/src/goal_operator_tests.rs` (new), `apps/litecoworkd/src/needs_you_operator_tests.rs`, `crates/domain-responsibility/src/goals.rs` (tests only).
- **Contract:** ADR-0016; RESPONSIBILITIES "Goals"; AC-018, AC-019.
- **Change intent:** add tests that every Goal command (create, revise, status change) leaves Task, Automation, occurrence and AgentSession tables unchanged and that progress counts only verified linked outcomes; add a Needs You test that listing changes no Approval, UserRequest or TaskBlocker row and that error responses also carry `no-store`.
- **Acceptance:**
  - [ ] Goal commands create, schedule or complete nothing (AC-018).
  - [ ] Needs You is read-only and `no-store` on success and error (AC-019).
- **Command:** `cargo test -p litecoworkd --locked goal_operator` (filter to be added by this task) and `cargo test -p litecoworkd --locked needs_you`
- **Evidence:** test output.
- **Stop:** NONE beyond test failure.

## 020-T13 - Failure, race and recovery suite

- **Status:** NOT STARTED.
- **Depends on:** T05, T09
- **Cross-package prerequisites:** 014-T13 (cross-package delete fences) for source messages; Feature 009 Coworker pause/archive.
- **Owner:** Responsibility, Storage, QA.
- **Allowed paths:** `apps/litecoworkd/tests/proactivity_quiet_learning.rs` (new canonical target), `crates/storage-sqlite/src/learning_tests.rs`, `crates/storage-sqlite/src/suggestion_propose_tests.rs`.
- **Contract:** SPEC section 11; LEARNING section 21; FAILURE-RECOVERY; AC-015, AC-016, AC-017.
- **Change intent:** create the canonical integration target and cover: daemon kill between Resource write and candidate insert (orphan Resource swept); between evaluation and commit; producer crash before cursor advance; classifier timeout; queue full; owner correction during extraction (stale candidate SUPERSEDED); source message deleted before commit (REJECTED); Coworker paused or archived mid-flight; clock jump at expiry; duplicate feedback request replay.
- **Acceptance:**
  - [ ] Every case ends in a documented state with no duplicate Suggestion or MemoryRecord (AC-016).
  - [ ] Paused or archived Coworker: no commit, history readable (AC-017).
  - [ ] Learning failures never change Conversation or Task outcomes (AC-015).
- **Command:** `cargo test -p litecoworkd --test proactivity_quiet_learning --locked -- --nocapture`
- **Evidence:** test output.
- **Stop:** a case loses an owner's explicit Remember action; fix before continuing.

## 020-T14 - Cross-layer trace for each user-visible operation

- **Status:** NOT STARTED.
- **Depends on:** T07, T11, T12
- **Cross-package prerequisites:** NONE
- **Owner:** Responsibility, Desktop UI.
- **Allowed paths:** `specs/020-proactivity-quiet-learning/contracts/ui-behavior.md`, `specs/020-proactivity-quiet-learning/WIRING.yaml`.
- **Contract:** AGENTS.md section 4; AC-003, AC-013.
- **Change intent:** for Accept idea, Dismiss, Snooze, Mute kind, Remember this, Undo, Correct, Approve/Dismiss candidate, Change learning policy, Revoke memory, Create/revise Goal and Open Needs You item, record outcome, exact control, Tauri command, Operator route, authorization, service owner, transaction/version fence, aggregate/event/receipt, projection, restart/recovery and evidence test.
- **Acceptance:**
  - [ ] Every operation has a complete row; no route without UI and no UI without route.
  - [ ] WIRING edges marked served only where evidence exists.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** updated ui-behavior.md.
- **Stop:** a hop is missing; open a task rather than documenting it as present.

## 020-T15 - Qualification on the release matrix rows

- **Status:** NOT STARTED.
- **Depends on:** T13, T14
- **Cross-package prerequisites:** 019-T14 (row-aware release matrix), 019-T15 (evidence bundle process and USER sign-off); Feature 009 E2E-09 on the same row; Feature 001 qualified Send route on the same row.
- **Owner:** QA, Product (USER), Responsibility.
- **Allowed paths:** `apps/litecoworkd/tests/proactivity_quiet_learning.rs`, `scripts/v1_perf_probe.py` (learning and propose probes only), `docs/release/evidence/INDEX.md`.
- **Contract:** ACCEPTANCE sections 2, 4, 5 and the proposed learning journey from T02; BENCHMARKS B18, B54, B55, B61, B62, B65, B72; AC-005, AC-009, AC-020.
- **Change intent:** on the packaged desktop on Windows x64 + OpenCode v2.0.32 and then Linux x64: produce a real Suggestion from a Goal, accept it into a READY Task, mute and snooze; in a Coworker Conversation say an explicit preference, observe the quiet Remembered notice, restart, confirm the next reply uses it, Undo, confirm it is gone; send third-party pasted content that says "remember this" and confirm no memory; run during quiet hours; measure propose and evaluation latency and Send latency with learning enabled versus disabled.
- **Acceptance:**
  - [ ] Journey passes on each row with owner USER sign-off and recorded denial cases.
  - [ ] Quiet hours suppress push surfaces on each row (AC-005).
  - [ ] Recheck race reproduced on the packaged build (AC-009).
  - [ ] Latency budgets met or misses reported with cause (AC-020).
- **Command:** `cargo test -p litecoworkd --test proactivity_quiet_learning --locked -- --nocapture` and `uv run --locked python scripts/v1_evidence_bundle.py validate docs/release/evidence/INDEX.md`
- **Evidence:** bundle per row; matrix cell.
- **Stop:** the journey passes only under WSL2 or a container; it does not count.

## 020-T16 - Documentation and ledger sync

- **Status:** NOT STARTED.
- **Depends on:** T10, T14
- **Cross-package prerequisites:** NONE (orchestrator owns shared ledgers; this task supplies edits for review).
- **Owner:** Documentation owners, Responsibility.
- **Allowed paths:** `docs/IMPLEMENTATION-STATUS.md`, `docs/LEARNING.md` (checkpoint paragraph), `docs/RESPONSIBILITIES.md` (implementation boundary), `docs/schemas/served-route-inventory.json`, `docs/V1-UI-ACTION-STATE-LEDGER.md`, `CURRENT-RUN.md`, this package.
- **Contract:** AGENTS.md sections 4, 5; SPEC section 4; AC-019.
- **Change intent:** update the LEARNING and RESPONSIBILITIES current-implementation paragraphs, the Area 7 and Area 8 rows with exact evidence class per row, regenerate the served-route inventory, close the B-45 register row when accepted and evidenced.
- **Acceptance:**
  - [ ] `python3 scripts/audit_served_routes.py` matches the committed inventory.
  - [ ] No row claims QUALIFIED without the T15 bundle for that row.
- **Command:** `python3 scripts/audit_served_routes.py` and `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** diff of status documents.
- **Stop:** evidence and claim disagree; downgrade the claim.

## 020-T17 - Package VERIFIED for V1

- **Status:** NOT STARTED.
- **Depends on:** T01-T16
- **Cross-package prerequisites:** Feature 019 release qualification sign-off.
- **Owner:** Product (sign-off), Responsibility, Security.
- **Allowed paths:** this package; `docs/IMPLEMENTATION-STATUS.md`.
- **Contract:** SPEC sections 14, 15; AC-001 to AC-022; SPECIFICATION-STANDARD lifecycle.
- **Change intent:** assemble the evidence bundle, move SPEC/WIRING ACCEPTED/LOCKED to IMPLEMENTING at the first implementation merge and to VERIFIED after owner sign-off on both rows.
- **Acceptance:**
  - [ ] Canonical command passes on Windows x64 + OpenCode v2.0.32 and on Linux x64.
  - [ ] Owner USER sign-off for the Suggestion and quiet-learning journeys on each row, including denial and recovery.
  - [ ] SPEC and WIRING status equal VERIFIED and the validator passes.
- **Command:** `cargo test -p litecoworkd --test proactivity_quiet_learning --locked -- --nocapture` and `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** bundle path referenced from IMPLEMENTATION-STATUS.md.
- **Stop:** any row lacks USER evidence; that row stays unqualified.

## Implementation and optimization notes (Rust and TypeScript)

Findings from reading the source, folded into the tasks above. They do not add scope.

- **Keep authority in Rust (T04, T05, T09):** producers, budgets, quiet-hours checks, eligibility and the commit recheck belong in `domain-responsibility`/`storage-sqlite`/`litecoworkd`. The TypeScript side keeps only pure view reducers (`suggestion-api.ts` already decodes defensively; add `learning-state.ts`). No eligibility or certainty logic in the WebView.
- **Pure gates live in a storage port crate (T03, F-04):** `coworker_learning.rs` is domain logic with no storage code. Moving it to `crates/domain-responsibility/src/learning_gates.rs` (with a `storage-core` re-export for `skill.rs`) puts it beside `SuggestionService` and the future `LearningService` and avoids a storage-core to domain dependency inversion. Optional; one definition either way.
- **Debug strings on the wire (T03):** `format!("{:?}", evaluation.disposition)` ties an event field to Rust `Debug` output; an explicit `as_str` keeps the wire stable across refactors.
- **Dead, unsafe helper (T03):** `permitted_source_kind` is unused and returns the most privileged kind for untrusted channels; removing it prevents a future caller from widening eligibility.
- **Budget queries use existing indexes (T04):** the open-cap and cooldown checks can use `idx_suggestions_workspace_status_created` and `idx_suggestions_dismissal_cooldown`; a producer-budget check needs one more partial index on (`workspace_id`, `proposed_by`, `created_at`) rather than a counter table.
- **Event cursor over polling tables (T05):** consuming the event journal after a persisted cursor costs one indexed range read per batch and gives restart safety, instead of rescanning Goals and Tasks on a timer.
- **Bridge thread use (T10):** `suggestions_bridge.rs` runs each call in `spawn_blocking` with a blocking client; the new `learning_bridge.rs` should reuse that validated pattern (bounded ids, fixed operation set, bounded response read) rather than add a second client style.

## Dependency order

T01 -> T02, T03, T12. T02 -> T04 -> T05 and T06. T06 + T04 -> T07. T02 + T03 -> T08 -> T09 -> T10. T10 + T06 -> T11. T05 + T09 -> T13. T07, T11, T12 -> T14. T13 + T14 -> T15. T10 + T14 -> T16. T01-T16 -> T17. Suggestion work (T04 to T07) needs only T01 and T02; learning work (T08 to T11) also needs B-45 accepted.

## Cross-package dependencies

| This task | Other package | Reason |
|---|---|---|
| T05 | Feature 005 Task creation events; Feature 009 Coworker status | Producer inputs and scope fence |
| T06 | Feature 010 quiet hours and event classes; Feature 011 Notifications entry | Quiet hours owned by 010 (A-35); 011 hosts the entry |
| T08 | Feature 009 COWORKER_NOTES re-widen; Feature 003 Resource revisions and purge | Learned memory text is a Coworker-scoped Resource revision |
| T09 | Feature 009 Coworker-owned Conversations; Feature 001 settled turns and qualified route | B-45 source and checkpoint; optional extractor route |
| T11 | Feature 009 Memory view and onboarding; Feature 001 message actions slot | UI hosts |
| T13 | Feature 014 delete fences; Feature 009 pause/archive | Recovery cases |
| T15 | Feature 019 matrix and evidence bundles; Features 001 and 009 qualified on the same row | Row-scoped qualification |
| T17 | Feature 019 sign-off | Package VERIFIED |
| Inbound | Feature 019 V1 go/no-go | Consumes this package's VERIFIED task |

## Completion evidence

For each task append a row when it closes: task, code revision (SHA), exact command, result (pass/fail with counts), OS and build (Windows 11/10 x64 or Linux x64 distribution), harness and version (OpenCode v2.0.32) where the extractor or Send route is involved, evidence class reached (CODE, SYSTEM, USER), bundle digest and reviewer. No commit or push is implied by task completion. Statuses above are source-audit classifications at the date of this revision, not qualification.

| Task | Revision | Command | Result | OS/build | Harness/version | Class | Reviewer |
|---|---|---|---|---|---|---|---|
