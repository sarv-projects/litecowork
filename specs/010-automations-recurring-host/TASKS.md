# Tasks - Automations and Recurring Host

**Feature:** 010 | **Status:** ACCEPTED/LOCKED - implementation authorized; tasks not started unless marked.
**Spec revision:** 1.0 | **Release target:** V1 (release matrix rows: Windows x64 + OpenCode v2.0.32 first; Linux x64 second)
**Owners:** Automation, Task Runtime, Runtime lifecycle, Desktop UI. Owner acceptance: 2026-11-09 via docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md (B-26, B-27, A-35).

Renumbering note: the previous T05 (external WorkflowBackend Routine mapping) is now T18 and the previous T06 (direct provider-graph authoring evaluation) is now T19. Both are outside the V1 release gate. docs/ACCEPTANCE-AND-E2E-PLAN.md still cites the old numbers; the orchestrator updates that reference.

## Current baseline

Rechecked against the active checkout on 2026-11-13; source checkpoint before this documentation/code update: `d6c5819ca61a96f9fed0b39e1cbe10e511256358`. Full package/source ranges and hashes are in [the current cross-check](../../docs/audit/FEATURE-010-CURRENT-CROSSCHECK-2026-11-13.md).

- **Operator:** `apps/litecoworkd/src/automation_operator.rs` (3,071 lines) mounts `/v1/automations` (list/create), `/{automation_id}` (get/revise), `/revisions`, `/occurrences`, `/health`, `/pause`, `/resume`, `/disable`, `/run`, `/test-runs` and `/v1/automations/host/settle`. T02 corrected the stale module header and user-facing save/pause/resume status copy at CODE level; neither change qualifies the packaged journey. `apps/litecoworkd/src/routine_operator.rs` mounts `/v1/routines`, `/{routine_id}`, `/revisions`, `/run`, `/archive`, `/health`.
- **TriggerHost:** `AutomationTriggerHostWorker` (same file, started from `apps/litecoworkd/src/operator.rs`) runs `run_trigger_host_loop` on a named thread every `TRIGGER_HOST_POLL_INTERVAL` (30 s). `settle_one_trigger` calls `plan_host_settlement` (`crates/domain-responsibility/src/trigger_host.rs`), then `materialize_scheduled_slot`, which returns the original Task through `occurrence_for_slot` on a repeated slot. A materialization error becomes a deferral blocker and the cursor keeps naming the slot. `SlotOutcomeReason` has `Missed`, `OverlapSkipped`, `OverlapQueued`, `OverlapCancelOld` and no Coworker reason.
- **Triggers:** `TriggerDefinition` in `crates/domain-responsibility/src/models.rs` has `Schedule` (`RecurrenceFormat::Cron5` or `Rfc5545Rrule`, IANA `timezone`, `misfire_policy`, `ambiguous_local_time`, `nonexistent_local_time`), `OneShot`, `Webhook`, `ConnectorEvent`, `Manual`. `CONNECTOR_EVENT` exists as a target kind, but event-trigger hosting is not served. `locally_reconcilable_trigger` in `crates/storage-sqlite/src/coworkers.rs` allows only Manual and parseable Schedule triggers to be enabled. Schedule parsing is `ScheduleTrigger` in `crates/domain-responsibility/src/schedule.rs` (`schedule_tests.rs`, 24 tests).
- **Storage:** `crates/storage-sqlite/src/automation_host.rs` (`list_due_schedules`, `occurrence_for_slot`, `record_skipped_occurrence`, `settle_started_occurrences_from_task_outcomes`, `advance_cursor`, `list_occurrence_page`), tests in `automation_host_tests.rs` such as `a_repeated_settlement_of_the_same_slot_never_creates_a_second_task`, `a_misfire_records_a_bounded_skipped_occurrence_and_advances_the_cursor` and `the_cursor_advance_is_monotonic_and_never_rewinds_on_a_backwards_clock`. `validate_coworker_provenance` in `automation_admission.rs` rejects a non-ACTIVE Coworker at Task admission.
- **Notifications:** `record_automation_outcome_delivery_in_transaction` in `crates/storage-sqlite/src/notification.rs` applies `decide_notification` and inserts only `PENDING` delivery rows. No production claim/presenter/settlement path moves a row out of `PENDING`. The Notifications Operator serves reads and per-event-class preference writes only. The current SQLite v1 schema already allows `PENDING`, `SENDING`, `SENT`, `FAILED`, `AMBIGUOUS` and `SUPPRESSED`, and already has attempt Runtime/epoch/count, `next_attempt_at`, `last_error_code` and `settled_at`; it has no delivery-row version for the proposed `If-Match` settle contract. The canonical `notification.delivery.changed.v1` event/payload and state names also already exist. Quiet-hours save validation checks the shape, weekdays and time format but does not prove the timezone is a valid IANA zone; evaluation treats timezone parse failure as outside quiet hours. Suppressed decisions currently write no row, contrary to AC-007. The preference is attached to one event class at a time; the accepted Workspace-wide enabled/class-selection UI does not have an atomic multi-class write contract. See A-97/A-98.
- **UI:** `apps/litecowork-ui/src/automations/AutomationsPage.tsx` reads only the first occurrence page (`setOccurrences(page.items)`, cursor discarded; Load more exists for definitions only). Pause copy scopes the effect to scheduled Task admission and leaves the action-specific Manual Run availability to the run controls; Resume copy says overdue slots are processed at the next host poll under their configured misfire policy. `AutomationEditor.tsx` edits schedule fields. `apps/litecowork-ui/src/notifications/NotificationsPage.tsx` displays quiet hours with `quietHoursText` and saves preserve the stored value; there is no quiet-hours control. Tests: `tests/automation-api.test.ts`, `tests/automation-setup.test.ts`, `tests/notification-api.test.ts`.
- **Task execution:** occurrences create READY Tasks only; Feature 005 execution is not qualified and nothing claims those Tasks.

Gaps to V1: cron grammar and catch-up bound are not pinned in the owner contract; CONNECTOR_EVENT is a target kind but event-trigger hosting is not served; no delivery after `PENDING`; SUPPRESSED is never recorded; no quiet-hours control; Coworker pause is handled as a retry loop; occurrence pagination is missing; no execution handoff; and neither release row has CODE/SYSTEM/USER qualification. The stale header and save/pause/resume copy are corrected at CODE level by T02.

## 010-T01 - Promote accepted Automation and notification contracts into owner docs and schemas

- **Status:** PARTIAL - owner acceptance of B-26, B-27 and A-35 recorded 2026-11-09 (SPEC revision 1.0). The exact CRON_5 grammar/catch-up cap and `COWORKER_PAUSED` slot token are not promoted. `notification.delivery.changed.v1`, the delivery state names, `SUPPRESSED`, the current delivery attempt fields and encrypted `automation_trigger_bindings` already exist in canonical event/state/schema contracts or SQLite v1. What remains absent is the served claim/settle route and an exact delivery CAS/lease version contract; do not duplicate existing status/cursor columns. The reserved v44 migration has no approved exact delta yet (A-98).
- **Depends on:** NONE
- **Cross-package prerequisites:** NONE. Feature 006 reviews the CONNECTOR_EVENT polling contract; Feature 011 reviews the Settings link to the quiet-hours control (A-35).
- **Owner:** Automation, Channels, API, Events and Storage owners; Product resolves remaining trigger details. CONNECTOR_EVENT itself is owner-confirmed by A-62.
- **Allowed paths:** `docs/AUTOMATION.md`, `docs/CHANNELS.md`, `docs/API.md`, `docs/EVENTS.md`, `docs/STORAGE.md`, `docs/STATE-MACHINES.md`, `docs/V1-SETTINGS-CONTRACT-LEDGER.md`, `docs/schemas/operator-api.openapi.yaml`, `docs/schemas/domain-event.schema.json`, `docs/schemas/error-codes.schema.json`, `docs/schemas/sqlite-v44.sql` (provisional reservation; create only the owner-approved additive delta, otherwise release the reservation; use the next free number if an earlier migration merges), this package.
- **Contract:** SPEC sections 5, 7.2, 15; AC-013, AC-014, AC-015, AC-016; SPECIFICATION-STANDARD section 7.
- **Change intent:** in one reviewed change promote only missing contract details: the exact CRON_5 grammar, `recurrence_semantics_version` meaning and operational catch-up cap; slot outcome `COWORKER_PAUSED`; the CONNECTOR_EVENT cursor/identity/payload-digest contract using the already-defined encrypted Runtime-local cursor owner; the LOCAL_POLL claim and settle routes and their lease/CAS fences; missing typed errors `CRON_GRAMMAR_UNSUPPORTED`, `TIMEZONE_INVALID`, `NOTIFICATION_LEASE_EXPIRED`; and enum validation for delivery transition state tokens in the event schema. Reuse the existing `notification.delivery.changed.v1`, existing event envelope, status vocabulary, attempt fields and encrypted cursor table unless the owner approves an explicit additive schema change. Event names/payload shapes exist, but current JSON Schema permits arbitrary nonempty `from`/`to` strings. CONNECTOR_EVENT is already owner-confirmed by A-62.
- **Acceptance:**
  - [x] B-26, B-27 and A-35 owner acceptance recorded (SPEC section 5).
  - [ ] Every promoted name exists once in its owner doc and machine schema; no parallel definition remains here.
  - [ ] If the owner-resolved claim/settle contract requires additive DDL, the next immutable `sqlite-vNN.sql` is created, older migrations remain byte-identical, and the fingerprint test passes. If no DDL is required, record the owner's decision and release the provisional v44 reservation without creating a migration.
  - [ ] Negative: schema validation rejects a delivery settle payload with an unknown status, a `notification.delivery.changed.v1` transition with an unknown `from`/`to` token, and a trigger with a fixed UTC offset.
- **Command:** `python3 .specify/scripts/validate_feature_package.py` and `python3 scripts/validate_architecture.py` and `cargo test -p storage-sqlite --locked schema` (fingerprint filter to be added by this task if absent)
- **Evidence:** PR link and command output in Completion evidence; register rows B-26, B-27, A-35 closed by the orchestrator.
- **Stop:** the CONNECTOR_EVENT kind is owner-confirmed (A-62). T09 remains BLOCKED on qualification of the Feature 006 Connection event source and dispatch path; the accepted kind does not satisfy that prerequisite.

## 010-T02 - Correct stale host claims and Automation status copy (A-29)

- **Status:** PARTIAL (CODE, 2026-11-13). The module header and truthful save/pause/resume copy are corrected and pinned by a regression guard. The focused UI test passes 14/14 and the daemon route-filter passes 2/2. The completed full UI suite reports 626 passed, 5 failed and 5 skipped of 636: three browser/CDP startup timeouts (Needs You #304 and #314; Resource editor #382) and two follow-on Needs You browser-state failures (#315–316). Automation tests passed within the full run. This closes only T02's truthful-message subcriterion of AC-019; T11 still owns missing occurrence-history pagination.
- **Depends on:** NONE
- **Cross-package prerequisites:** NONE
- **Owner:** Automation, Desktop UI.
- **Allowed paths:** `apps/litecoworkd/src/automation_operator.rs` (module doc comment only), `apps/litecowork-ui/src/automations/AutomationsPage.tsx`, `apps/litecowork-ui/tests/automation-api.test.ts`.
- **Contract:** SPEC section 19a; register A-29; the truthful-message subcriterion of AC-019. AC-019 also covers occurrence history pagination, which remains with T11.
- **Change intent:** rewrite the module header to say the local `AutomationTriggerHostWorker` polls every 30 s and `/resume` is served for locally supported Schedule hosting; Manual Run is an owner-triggered one-shot command, not host polling. Save and pause copy says recurring schedule hosting is off without implying Manual Run is unconditionally available. Resume copy says overdue schedule slots follow the configured misfire policy at the next host poll and may create READY Tasks; the resume action itself does not synchronously create a Task. Manual Run eligibility remains controlled by the exact trigger, Routine, input and in-flight request gates.
- **Acceptance:**
  - [x] T02 truthful-message subcriterion of AC-019: no UI string or module comment says recurring hosting or `/resume` is unavailable while the worker runs.
  - [x] A focused UI regression test pins the corrected save, Pause and Resume messages.
  - [x] Daemon route-filter command passes 2/2.
  - [ ] Full UI suite passes (current run has three mounted-browser CDP startup timeouts and two dependent Needs You browser-state failures); the broader AC-019 pagination subcriterion remains open under T11.
- **Command:** `pnpm --dir apps/litecowork-ui test` and `cargo test -p litecoworkd --locked automation_route_tests`
- **Evidence:** test names in Completion evidence.
- **Stop:** NONE expected; text-only change.

## 010-T03 - Qualify cron in a named timezone with the accepted misfire policy (B-26)

- **Status:** PARTIAL - `ScheduleTrigger` parsing, DST rules and misfire handling exist with tests in `crates/domain-responsibility/src/schedule_tests.rs` and `crates/storage-sqlite/src/automation_host_tests.rs` (`a_cron_schedule_trigger_parses_into_an_executable_plan`, `a_misfire_records_a_bounded_skipped_occurrence_and_advances_the_cursor`). The grammar is not pinned in the owner contract and save-time validation of timezone and grammar is not proven against it.
- **Depends on:** T01
- **Cross-package prerequisites:** NONE
- **Owner:** Automation domain, Storage.
- **Allowed paths:** `crates/domain-responsibility/src/schedule.rs`, `crates/domain-responsibility/src/schedule_tests.rs`, `crates/domain-responsibility/src/trigger_host.rs`, `crates/domain-responsibility/src/trigger_host_tests.rs`, `crates/domain-responsibility/src/models.rs`, `crates/storage-sqlite/src/automation_host.rs`, `crates/storage-sqlite/src/automation_host_tests.rs`.
- **Contract:** SPEC sections 5 (B-26), 7a, 11; AUTOMATION.md ScheduleTrigger; AC-003, AC-004, AC-010, AC-012, AC-013.
- **Change intent:** validate at save that `recurrence_format` is `CRON_5` within the T01 grammar and `timezone` is a known IANA name, returning `CRON_GRAMMAR_UNSUPPORTED` / `TIMEZONE_INVALID`. Keep `RFC5545_RRULE` rejected for enable until separately qualified. Default `misfire_policy` to `RUN_ONCE_WHEN_AVAILABLE` in the editor payload. Add tests across a spring-forward and fall-back transition in at least two zones, a timezone-rule change after a claimed occurrence, and a sleep gap of several days with the catch-up bound.
- **Acceptance:**
  - [ ] AC-013: an unknown timezone, `+05:30`-style offset or out-of-grammar expression is rejected before any cursor exists.
  - [ ] AC-004: next fire follows the stored zone across DST; EARLIER/LATER and SKIP/NEXT_VALID behave as declared.
  - [ ] AC-003 and AC-012: a multi-day gap yields one misfire-recovered occurrence (or the bounded count) and never a success claim.
  - [ ] AC-010: save creates PAUSED; resume fires nothing at enable time.
- **Command:** `cargo test -p domain-responsibility --locked schedule` and `cargo test -p storage-sqlite --locked automation_host`
- **Evidence:** test names in Completion evidence.
- **Stop:** the grammar in source and in AUTOMATION.md disagree; fix the owner first.

## 010-T04 - Harden occurrence identity, dedupe and conflict

- **Status:** PARTIAL - `occurrence_for_slot` returns the original Task for a repeated slot and `a_reused_task_request_key_with_a_changed_payload_is_a_conflict` exists. Concurrent settlement from two passes and `OCCURRENCE_CONFLICT` for a stable delivery id with a new digest are not proven for every trigger kind.
- **Depends on:** T01
- **Cross-package prerequisites:** NONE
- **Owner:** Automation, Storage.
- **Allowed paths:** `crates/storage-sqlite/src/automation_host.rs`, `crates/storage-sqlite/src/automation_admission.rs`, `crates/storage-sqlite/src/automation_host_tests.rs`, `crates/domain-responsibility/src/occurrences.rs`, `crates/domain-responsibility/src/occurrence_tests.rs`.
- **Contract:** SPEC sections 6, 7a; AUTOMATION.md occurrence lifecycle; AC-001, AC-002, AC-009.
- **Change intent:** guarantee one occurrence per `(automation_id, trigger_id, occurrence_key)` with a unique index and in-transaction lookup; map a key reuse with a different payload digest to `OCCURRENCE_CONFLICT`; make Manual Run and scheduled run share the same idempotency rules. Add a two-connection race test.
- **Acceptance:**
  - [ ] AC-001: one observed slot yields exactly one Task linked to the occurrence.
  - [ ] AC-002: a repeated or concurrent settlement returns the original Task.
  - [ ] AC-009: reused delivery id with another digest returns `OCCURRENCE_CONFLICT` and writes nothing.
- **Command:** `cargo test -p storage-sqlite --locked automation_host` and `cargo test -p domain-responsibility --locked occurrence`
- **Evidence:** test names in Completion evidence.
- **Stop:** the unique index cannot be added without rewriting existing rows; plan a forward migration first.

## 010-T05 - Recheck Coworker PAUSED at occurrence settlement

- **Status:** PARTIAL - admission refuses a non-ACTIVE Coworker (`validate_coworker_provenance`), but `settle_one_trigger` turns that into a blocker and retries the slot every pass; no skip is recorded and the cursor does not advance.
- **Depends on:** T01, T04
- **Cross-package prerequisites:** Feature 009 Coworker pause and resume reconciliation (009 owns status semantics and resume; this task owns the TriggerHost behavior).
- **Owner:** Automation, Responsibility domain.
- **Allowed paths:** `crates/domain-responsibility/src/trigger_host.rs`, `crates/domain-responsibility/src/trigger_host_tests.rs`, `crates/storage-sqlite/src/automation_host.rs`, `crates/storage-sqlite/src/automation_host_tests.rs`, `apps/litecoworkd/src/automation_operator.rs` (`settle_one_trigger`, `skip_reason_token`).
- **Contract:** SPEC sections 7a, 11; RESPONSIBILITIES.md; AC-018.
- **Change intent:** load the pinned Coworker status with the due schedule (`list_due_schedules` already reads `coworker_id`) and evaluate `check_origin_status(status, WorkOrigin::Proactive)` before planning. For PAUSED, emit `HostSlotDecision::Skip` with a new `SlotOutcomeReason::CoworkerPaused`, write one skipped occurrence with reason `COWORKER_PAUSED` through `record_skipped_occurrence`, and advance the cursor. Keep the admission-time check as the transactional backstop for a pause that commits between planning and admission.
- **Acceptance:**
  - [ ] AC-018: a due slot for a PAUSED Coworker records one skip, creates no Task and is not retried on the next pass.
  - [ ] Race: pause committed after planning still yields no Task and no duplicate skip.
  - [ ] Health shows the skip count; no `OCCURRENCE_TASK_MISSING` or repeated blocker appears.
- **Command:** `cargo test -p storage-sqlite --locked coworker_paused` (filter to be added by this task) and `cargo test -p litecoworkd --locked trigger_host`
- **Evidence:** test names in Completion evidence.
- **Stop:** the skip would hide an ARCHIVED Coworker; ARCHIVED must stay a typed blocker.

## 010-T06 - Deliver LOCAL_POLL notifications with status separate from Task outcome (B-27)

- **Status:** NOT STARTED. `PENDING` rows are inserted; nothing claims, presents or settles them.
- **Depends on:** T01
- **Cross-package prerequisites:** Feature 011 Runtime incarnation (lease owner identity) and packaged shell; Feature 019 dependency audit if an OS notification plugin is added.
- **Owner:** Channels, Notifications, Desktop UI.
- **Allowed paths:** `crates/storage-sqlite/src/notification.rs`, `crates/storage-sqlite/src/notification_tests.rs`, `crates/domain-responsibility/src/notifications.rs`, `apps/litecoworkd/src/notification_operator.rs`, `apps/litecowork-ui/src-tauri/src/notification_bridge.rs`, `apps/litecowork-ui/src/notifications/notification-api.ts`, `apps/litecowork-ui/src/notifications/desktop-notification-api.ts`, `apps/litecowork-ui/src/notifications/notification-presenter.ts` (new), `apps/litecowork-ui/tests/notification-api.test.ts`.
- **Contract:** SPEC section 5 (B-27), 7.2 `NotificationDelivery`; CHANNELS.md; AC-006, AC-015.
- **Change intent:** add `claim_notification_deliveries` (lease a bounded batch of `PENDING` rows to this Runtime incarnation, set `SENDING`, `attempt_count + 1`, `next_attempt_at` as lease expiry) and `settle_notification_delivery` (`SENT`, `FAILED` with `last_error_code`, or `AMBIGUOUS`; If-Match on the row version), each writing `notification.delivery.changed.v1`. A startup sweep marks rows left in `SENDING` by a previous incarnation `AMBIGUOUS`. Serve the T01 routes and add `claim` and `settle` operations to `notification_request`. The first presenter is in-app (a notification tray in the desktop shell); an OS toast is optional and needs a reviewed plugin dependency. Task and occurrence rows are never touched by these transactions.
- **Acceptance:**
  - [ ] AC-015: claim then settle moves `PENDING -> SENDING -> SENT`; a presenter error settles `FAILED`; restart during `SENDING` yields `AMBIGUOUS`, never `SENT`.
  - [ ] AC-006: a Task can be COMPLETED while its delivery is `PENDING`, `FAILED` or `AMBIGUOUS`; neither status changes the other.
  - [ ] Concurrency: two claimers never lease the same row; an expired lease returns `NOTIFICATION_LEASE_EXPIRED` on settle.
  - [ ] Delivery bodies in events carry ids and classes only, no Task text.
- **Command:** `cargo test -p storage-sqlite --locked notification` and `cargo test -p litecoworkd --locked notification` and `pnpm --dir apps/litecowork-ui test`
- **Evidence:** test names in Completion evidence.
- **Stop:** delivery status would be derived from or written into the Task record.

## 010-T07 - Record SUPPRESSED deliveries and serve the quiet-hours control surface (A-35)

- **Status:** PARTIAL - `QuietHours` parsing and `decide_notification` suppression exist; `set_notification_preference` accepts the value. Current storage validation checks a nonempty timezone string, weekday set and local-time syntax, but does not reject an unknown IANA zone. If invalid data is already stored, timezone parsing fails in `contains` and the decision path currently treats that as outside quiet hours. Suppression writes no row; Feature 010 AC-007 requires a visible `SUPPRESSED` row, but higher-authority `docs/DATA-MODEL.md` says a suppressed outcome writes nothing (A-99). `NotificationsPage.tsx` has no quiet-hours editor, and the accepted Workspace-wide class selector has no atomic save/API contract across per-event-class preference records (A-97).
- **Depends on:** T01, T06
- **Cross-package prerequisites:** Feature 011 Settings shell entry that links to this control (A-35); Feature 020 proactive suggestion policy consumes the same quiet-hours contract.
- **Owner:** Notifications, Desktop UI.
- **Allowed paths:** `crates/storage-sqlite/src/notification.rs`, `crates/storage-sqlite/src/notification_tests.rs`, `crates/domain-responsibility/src/notifications.rs`, `crates/domain-responsibility/src/notification_tests.rs`, `apps/litecowork-ui/src/notifications/NotificationsPage.tsx`, `apps/litecowork-ui/src/notifications/quiet-hours-form.ts` (new), `apps/litecowork-ui/tests/quiet-hours-form.test.ts` (new).
- **Contract:** SPEC section 5 quiet-hours normative table; AC-007, AC-016.
- **Change intent:** when `decide_notification` returns a quiet-hours suppression, insert the row with status `SUPPRESSED` and the suppress reason instead of returning `None`; other suppressions (policy off, dedupe) keep their current behavior unless T01 says otherwise. Add a quiet-hours editor on the Notifications page: enabled toggle, start and end local time, IANA timezone picker, weekdays and suppressed classes, saved through the existing `PUT /v1/notification-preferences/{event_class}` with If-Match. Input shaping lives in `quiet-hours-form.ts`; validity stays in Rust.
- **Acceptance:**
  - [ ] AC-007: a delivery inside the window is stored `SUPPRESSED`, visible in history, not silently dropped.
  - [ ] AC-016: a stale version conflicts with a reload prompt; an invalid zone, empty weekdays or malformed time is rejected by the daemon; a saved window affects later deliveries only.
  - [ ] Keyboard and screen reader can operate every control; errors are announced.
- **Command:** `cargo test -p storage-sqlite --locked notification` and `cargo test -p domain-responsibility --locked notification` and `pnpm --dir apps/litecowork-ui test`
- **Evidence:** test names in Completion evidence.
- **Stop:** the UI would need to decide suppression itself.

## 010-T08 - Hand occurrence Tasks to execution only through Feature 005

- **Status:** BLOCKED - Feature 005 Task execution is not qualified (owner: Task Runtime). Occurrences create READY Tasks and `settle_started_occurrences_from_task_outcomes` already settles occurrences from terminal Task outcomes.
- **DEFERRED/BLOCKED note:** Why: Feature 005 Task execution is not qualified. Unblocks when: Feature 005 admission and execution qualified. Test later at: native Linux, then the owner's native Windows x64 laptop (release matrix row 1); WSL2 and containers never count.
- **Depends on:** T04
- **Cross-package prerequisites:** Feature 005 qualified Task admission and execution on the Linux row (Windows row after the B-35 Environment provider).
- **Owner:** Automation, Task Runtime.
- **Allowed paths:** `apps/litecoworkd/src/automation_operator.rs` (handoff call after materialization), `crates/storage-sqlite/src/automation_host.rs`, `crates/storage-sqlite/src/automation_host_tests.rs`, `apps/litecowork-ui/src/automations/AutomationsPage.tsx` (history labels).
- **Contract:** SPEC sections 5 LOCKED ("TriggerHost observes; ordinary Task admission selects the execution Runtime"), 7; TASK-RUNTIME.md; AC-001, AC-017.
- **Change intent:** after a slot materializes a READY Task, call the 005 admission entry point outside the occurrence transaction when, and only when, 005 reports execution readiness for this Workspace and row. Until then history shows "Task created, not executed on this device". The occurrence settles only from the terminal Task outcome per section 7a.
- **Acceptance:**
  - [ ] AC-017: with 005 unavailable, Tasks stay READY and the UI says not executed; with 005 available, admission goes through 005 and the TriggerHost never runs a Task body.
  - [ ] AC-001: one occurrence still maps to one Task across handoff retries.
  - [ ] A handoff failure leaves the occurrence STARTED and visible, not COMPLETED.
- **Command:** `cargo test -p litecoworkd --locked automation_execution_handoff` (filter to be added by this task)
- **Evidence:** test names; row-specific note in IMPLEMENTATION-STATUS.md.
- **Stop:** the handoff would bypass 005 admission or run inside the occurrence transaction.

## 010-T09 - Add the event-trigger kind after cron (CONNECTOR_EVENT over LOCAL_POLL)

- **Status:** BLOCKED - the kind is owner-confirmed (CONNECTOR_EVENT over LOCAL_POLL, sarv-projects, 2026-11-10); still needs a qualified Feature 006 Connection (owner: Capability Fabric). `TriggerDefinition::ConnectorEvent` exists; `locally_reconcilable_trigger` refuses it.
- **DEFERRED/BLOCKED note:** Why: needs a qualified Feature 006 Connection; CONNECTOR_EVENT is owner-confirmed. Unblocks when: Feature 006 Connection (needs the B-19 follow-up decision). Test later at: a real connector account on native Linux or the owner's native Windows x64 laptop (release matrix row 1); WSL2 and containers never count.
- **Depends on:** T01, T03, T04
- **Cross-package prerequisites:** Feature 006 Connection broker with a polling read for provider events.
- **Owner:** Automation, Capability Fabric.
- **Allowed paths:** `crates/domain-responsibility/src/models.rs`, `crates/domain-responsibility/src/trigger_host.rs`, `crates/domain-responsibility/src/trigger_host_tests.rs`, `crates/storage-sqlite/src/coworkers.rs` (`locally_reconcilable_trigger`), `crates/storage-sqlite/src/automation_host.rs`, `crates/storage-sqlite/src/automation_host_tests.rs`, `apps/litecoworkd/src/automation_operator.rs`, `apps/litecowork-ui/src/automations/AutomationEditor.tsx`.
- **Contract:** SPEC sections 3 (no public webhooks), 7.2 `ConnectorEventObservation`, 10; AUTOMATION.md event-trigger delivery section; AC-009, AC-011, AC-014.
- **Change intent:** allow enabling a CONNECTOR_EVENT trigger when its Connection is ACTIVE and qualified. On each pass, read new provider events through the 006 broker after the stored cursor, compute `OccurrenceIdentity` from `trigger_id` and `provider_event_id`, commit one occurrence and Task per event, store the payload digest, and advance the cursor in the same transaction as the occurrence. Payload fields are untrusted data and are passed only as bound Routine inputs. No listener socket is opened.
- **Acceptance:**
  - [ ] AC-014: one provider event yields one occurrence; a re-read event returns the original.
  - [ ] AC-009: the same `provider_event_id` with another digest returns `OCCURRENCE_CONFLICT`.
  - [ ] AC-011: no inbound port is opened (socket inventory check); payload text cannot change grants.
  - [ ] Connection revoked or unauthenticated: the trigger shows blocked with a repair link and creates nothing.
- **Command:** `cargo test -p storage-sqlite --locked connector_event_trigger` (filter to be added by this task) and `cargo test -p domain-responsibility --locked trigger_host`
- **Evidence:** test names in Completion evidence.
- **Stop:** the only available event source would require an inbound listener.

## 010-T10 - Window close, Runtime stop and sleep matrix

- **Status:** NOT STARTED. The worker runs while the daemon runs; no test covers window close versus Runtime stop or OS sleep.
- **Reference (DeepSeek Harness, MIT, pinned `fcc0ad46`, no code copied yet):** upstream [deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness/tree/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159), reuse map [DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md](../../docs/DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md).
  - **P-31 QuiescenceReport (one "interruptible work" predicate; unknown means busy; lock then learn; inspect once).** Class: **ADAPT**. Upstream permalinks: [src/host-process.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/host-process.ts), [src/quit-confirmation.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/quit-confirmation.ts). Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 3 and 14 (row P-31). Why: LiteCowork describes the same predicate in three places (T12, T13, T14 of package 011). Upstream shows the same facts must back stop, update, quit and keep-awake, and that unknown resolves to busy. The inputs differ (admitted turns, admitted Attempts, Terminal and Browser sessions, armed Automations, pending approvals, not Electron tasks), so it is an ADAPT, not a port. Reuse: Pattern only, reimplement. No upstream code or text is copied or imported (AGENTS.md section 7; upstream is Electron and TypeScript, LiteCowork is Tauri, Rust and React).
- **Depends on:** T03, T05
- **Cross-package prerequisites:** Feature 011 Runtime lifecycle, stop/quiescence and keep-awake.
- **Owner:** Runtime lifecycle, Automation.
- **Allowed paths:** `apps/litecoworkd/src/automation_operator.rs` (`trigger_host_worker_tests`), `apps/litecoworkd/tests/automation_recurring_host.rs` (new), `crates/storage-sqlite/src/automation_host_tests.rs`.
- **Contract:** SPEC sections 5 LOCKED (window close vs sleep), 11, 12; RUNTIME-LIFECYCLE.md; AC-003, AC-005.
- **Change intent:** an integration test that runs the daemon with a short poll interval, closes the desktop client (Runtime stays up) and checks slots still settle; stops the Runtime and checks no occurrence claims a run that did not start; simulates a clock jump to stand in for sleep and checks the misfire record. A manual SYSTEM run covers real OS sleep on each release row.
- **Acceptance:**
  - [ ] AC-005: window close keeps observing; Runtime stop produces no false STARTED or COMPLETED.
  - [ ] AC-003: sleep across a fire time records misfire or skip per policy.
- **Command:** `cargo test -p litecoworkd --test automation_recurring_host --locked` (test to be added by this task)
- **Evidence:** test output plus SYSTEM sleep notes per row.
- **Stop:** the result depends on WSL2 behavior; WSL2 is not evidence for either row.

## 010-T11 - Automation history and editor UI states

- **Status:** PARTIAL - `AutomationsPage.tsx` lists definitions with Load more, shows health and the first occurrence page; `AutomationEditor.tsx` edits triggers. History has no pagination and no delivery status column.
- **Depends on:** T02, T03, T06
- **Cross-package prerequisites:** Feature 011 keyboard and focus baseline.
- **Owner:** Desktop UI.
- **Allowed paths:** `apps/litecowork-ui/src/automations/AutomationsPage.tsx`, `apps/litecowork-ui/src/automations/AutomationEditor.tsx`, `apps/litecowork-ui/src/automations/automation-api.ts`, `apps/litecowork-ui/src/automations/desktop-automation-api.ts`, `apps/litecowork-ui/src/automations/automation-history-state.ts` (new), `apps/litecowork-ui/src/automations/automations-page.css`, `apps/litecowork-ui/tests/automation-history-state.test.ts` (new), `specs/010-automations-recurring-host/contracts/ui-behavior.md`.
- **Contract:** contracts/ui-behavior.md; docs/V1-UI-ACTION-STATE-LEDGER.md; AC-008, AC-010, AC-019.
- **Change intent:** history rows show occurrence status, Task status and delivery status in separate labelled columns, with skip reasons (`MISFIRE`, `OVERLAP`, `COWORKER_PAUSED`) in plain words. Add history Load more using `next_cursor` with request-generation guards like the definition list. Editor: CRON_5 field with a next-five-fires preview computed by the daemon, IANA timezone picker, misfire default, and the event-trigger option shown unavailable until T09. States: loading, empty, error, denied, offline (Runtime down), stale (version conflict) and recovery for every action.
- **Acceptance:**
  - [ ] AC-008: a `PENDING` delivery never reads as delivered; all three statuses are separate accessible text.
  - [ ] AC-019: history pages through every occurrence without duplicates across a concurrent new occurrence.
  - [ ] AC-010: the save flow shows PAUSED and Resume explains that nothing fires immediately.
- **Command:** `pnpm --dir apps/litecowork-ui test` and `pnpm --dir apps/litecowork-ui run build`
- **Evidence:** test names; UI ledger rows updated in T16.
- **Stop:** a control has no owner route.

## 010-T12 - Failure and recovery: crash, replay, lease expiry and races

- **Status:** PARTIAL - cursor monotonicity and repeated settlement are tested in `automation_host_tests.rs`; crash between occurrence commit and Task, CLAIMED lease expiry and restart during delivery are not.
- **Depends on:** T04, T05, T06
- **Cross-package prerequisites:** Feature 011 Runtime restart.
- **Owner:** Automation, Storage, Notifications.
- **Allowed paths:** `crates/storage-sqlite/src/automation_host_tests.rs`, `crates/storage-sqlite/src/notification_tests.rs`, `apps/litecoworkd/tests/automation_recurring_host.rs`.
- **Contract:** SPEC sections 11, 12; contracts/errors-recovery.md; AC-002, AC-003, AC-009, AC-015, AC-018.
- **Change intent:** kill the daemon after occurrence commit and before Task admission, during a delivery lease and during a pause race; replay duplicate settle and run requests; expire a CLAIMED lease before materialization and check it returns to PENDING once.
- **Acceptance:**
  - [ ] AC-002 and AC-009: replays return originals; conflicts write nothing.
  - [ ] AC-003: restart replays the cursor without double-creating.
  - [ ] AC-015: a lease interrupted by restart settles AMBIGUOUS.
  - [ ] AC-018: pause during a pass yields at most one skip.
- **Command:** `cargo test -p litecoworkd --test automation_recurring_host --locked recovery` and `cargo test -p storage-sqlite --locked automation_host`
- **Evidence:** test output in Completion evidence.
- **Stop:** a recovery path infers success from an intent.

## 010-T13 - Security negatives: no grant, no inbound port, untrusted payloads

- **Status:** PARTIAL - Routine run rejects bindings beyond 64 and grants nothing (`apps/litecoworkd/src/routine_run_operator_tests.rs`); no socket inventory or event-payload containment test exists.
- **Depends on:** T09
- **Cross-package prerequisites:** Feature 019 security hardening checks (IPC and secret scan).
- **Owner:** Security, Automation.
- **Allowed paths:** `apps/litecoworkd/tests/automation_recurring_host.rs`, `apps/litecoworkd/src/routine_run_operator_tests.rs`, `crates/storage-sqlite/src/automation_host_tests.rs`.
- **Contract:** SPEC sections 10, 17; TRUST.md; AC-011.
- **Change intent:** assert that saving or enabling any Automation or Routine creates no CapabilityGrant or Approval, that the daemon opens no listening socket beyond the Operator IPC endpoint while triggers run, that event payload strings reach Tasks only as bound inputs, and that no secret or Connection token appears in revisions, events or logs.
- **Acceptance:**
  - [ ] AC-011: zero grants, zero approvals, zero new listeners.
  - [ ] `python3 scripts/v1_secret_scan.py` reports nothing for fixtures written by these tests.
- **Command:** `cargo test -p litecoworkd --test automation_recurring_host --locked security` and `python3 scripts/v1_secret_scan.py`
- **Evidence:** test output in Completion evidence.
- **Stop:** any listener or grant appears.

## 010-T14 - Cross-layer trace for each Automation operation

- **Status:** NOT STARTED.
- **Depends on:** T06, T07, T09, T11
- **Cross-package prerequisites:** NONE
- **Owner:** Automation and Desktop UI reviewers.
- **Allowed paths:** this package (`contracts/ui-behavior.md`, `contracts/api-events.md`, `WIRING.yaml`).
- **Contract:** AGENTS.md section 4; AC-008.
- **Change intent:** for Save, Resume, Pause, Run now, Create test Task, history paging, quiet-hours edit and notification claim/settle, record user outcome, exact control, typed UI command, Tauri operation, Operator route, authorization, service owner, transaction/version/fence, event/receipt, projection, restart behavior and the proving test. Add matching WIRING edges for delivery and the event trigger.
- **Acceptance:**
  - [ ] Every operation has a complete row; no control lacks a route and no route lacks a control.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** the trace table in contracts/ui-behavior.md.
- **Stop:** a link in the chain is missing in source.

## 010-T15 - Qualify E2E-10 (scheduled and event Automation) on the release matrix rows

- **Status:** BLOCKED - the event half needs T09 (Feature 006 Connection); Task execution needs Feature 005 on the Linux row and the B-35 Windows Environment provider on the Windows row (owner: Task Runtime).
- **DEFERRED/BLOCKED note:** Why: E2E-10 needs T08 and T09 and Task execution. Unblocks when: T08, T09, Feature 005. Test later at: the owner's Windows laptop and native Linux.
- **Depends on:** T08, T10, T12, T13, T14
- **Cross-package prerequisites:** Feature 005 Task execution; Feature 006 Connection; Feature 011 packaged desktop and keep-awake; Feature 019 release matrix.
- **Owner:** Product, QA, Automation.
- **Allowed paths:** this package; evidence paths named by Feature 019.
- **Contract:** ACCEPTANCE-AND-E2E-PLAN E2E-10; SPEC section 15; AC-001 to AC-019.
- **Change intent:** run the packaged desktop journey: create a cron Automation in a named zone and an event Automation, resume both, observe one occurrence and one Task each, see delivery reach SENT (or FAILED/AMBIGUOUS truthfully), quiet hours suppress a class, pause a Coworker and see a recorded skip, sleep and restart and see misfire handling. On the Windows row, Task execution shows UNAVAILABLE until B-35 qualifies; the occurrence, Task identity and notification parts still qualify there.
- **Acceptance:**
  - [ ] E2E-10 passes per row with denial and recovery, with the Windows execution limit stated.
  - [ ] Owner USER sign-off per row.
- **Command:** `cargo test -p litecoworkd --test automation_recurring_host --locked` and `pnpm --dir apps/litecowork-ui test`
- **Evidence:** evidence bundle path per row.
- **Stop:** a row lacks USER evidence; that row stays unqualified.

## 010-T16 - Synchronize status documents and inventories

- **Status:** NOT STARTED.
- **Depends on:** T15
- **Cross-package prerequisites:** NONE
- **Owner:** Documentation owner, orchestrator.
- **Allowed paths:** `docs/IMPLEMENTATION-STATUS.md`, `docs/V1-UI-ACTION-STATE-LEDGER.md`, `docs/V1-SETTINGS-CONTRACT-LEDGER.md`, `docs/SEMANTIC-SERVED-UI-MAP.md`, `docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md`, `docs/ACCEPTANCE-AND-E2E-PLAN.md` (T18/T19 renumbering reference), generated served-route inventory, this package.
- **Contract:** AGENTS.md section 4 steps 5-6.
- **Change intent:** update Automation and Notification rows to the evidence reached, regenerate the served-route inventory with `scripts/audit_served_routes.py`, update UI and Settings ledgers for the quiet-hours control and history columns, and close register rows B-26, B-27, A-29 and A-35.
- **Acceptance:**
  - [ ] Inventory lists claim and settle routes as served; nothing else changes class without evidence.
- **Command:** `python3 scripts/audit_served_routes.py` and `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** diff of the regenerated inventory.
- **Stop:** a status row would claim more than recorded evidence.

## 010-T17 - Package VERIFIED for V1

- **Status:** NOT STARTED.
- **Depends on:** T01-T16
- **Cross-package prerequisites:** Feature 019 evidence bundle process and release qualification matrix.
- **Owner:** Product (sign-off), Automation, Notifications, Security.
- **Allowed paths:** this package; `docs/IMPLEMENTATION-STATUS.md`.
- **Contract:** SPEC sections 15-16; AC-001 to AC-019; SPECIFICATION-STANDARD lifecycle.
- **Change intent:** assemble the evidence bundle; move the package ACCEPTED/LOCKED to IMPLEMENTING at the first implementation merge and to VERIFIED after owner sign-off on both release rows. T18 and T19 are outside this gate.
- **Acceptance:**
  - [ ] Canonical command and T15 pass on Windows x64 + OpenCode v2.0.32 (execution limit stated) and on Linux x64.
  - [ ] SPEC and WIRING status equal VERIFIED and the validator passes.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** evidence bundle path referenced from IMPLEMENTATION-STATUS.md.
- **Stop:** any row lacks USER evidence.

## 010-T18 - Qualified external WorkflowBackend Routine mapping (not a V1 release gate; formerly T05)

- **Status:** BLOCKED - depends on qualified Feature 005 Task admission, Feature 006 CapabilityInvocation and a WorkflowBackendAdapter, none present (owners: Task Runtime, Capability Fabric).
- **DEFERRED/BLOCKED note:** Why: not a V1 release gate; needs Feature 005 admission, Feature 006 invocation and a WorkflowBackendAdapter. Unblocks when: post-V1 decision plus those features. Test later at: not scheduled.
- **Depends on:** T08
- **Cross-package prerequisites:** Feature 005 and Feature 006 qualified; CONNECTORS-WORKFLOWS owner adapter.
- **Owner:** Automation, Capability Fabric, Task Runtime, Desktop UI.
- **Allowed paths:** Routine-to-workflow reference mapping, adapter integration, invocation projection and tests; the provider adapter remains owned by CONNECTORS-WORKFLOWS.
- **Contract:** SPEC section 19, ROUTINES.md, CONNECTORS-WORKFLOWS.md, TASK-RUNTIME.md.
- **Change intent:** allow a Routine to pin a discovered external workflow reference, validate mapped inputs and invoke it as one bounded CapabilityInvocation inside the admitted Task.
- **Acceptance:**
  - [ ] Provider/Connection identity and workflow reference are pinned; input validation happens before invocation.
  - [ ] External graph state stays provider-owned; provider step observations create no LiteCowork Steps.
  - [ ] Timeout or unknown result reconciles before retry; Trust and Effect policy cover declared effect classes.
- **Command:** owner-specific adapter and Task/Capability integration tests after those owners are ACCEPTED/LOCKED.
- **Evidence:** not applicable until unblocked.
- **Stop:** do not create a Core WorkflowGraph schema, canvas, local graph revision/run store or second Task/Attempt state machine.

## 010-T19 - Direct LiteCowork Coworker provider-graph authoring evaluation (deferred, not V1; formerly T06)

- **Status:** DEFERRED - owner confirmed on 2026-11-10 that it stays outside V1; no implementation authorization unless a later owner ADR changes scope.
- **Deferred note:** Why: owner confirmed on 2026-11-10 that this stays out of V1 (A-62). Reconsider only through a later owner ADR. Test later at: not scheduled.
- **Depends on:** T18
- **Cross-package prerequisites:** provider-specific workflow authoring API; explicit owner ADR.
- **Owner:** Product, Capability Fabric, Trust, Automation.
- **Allowed paths:** research and provider adapter proposal only until owner promotion.
- **Contract:** SPEC section 19, CONNECTORS-WORKFLOWS.md, TRUST.md.
- **Change intent:** evaluate whether LiteCowork may expose a selected provider's graph-mutation MCP/API tools to a Coworker with narrower authority than workflow invocation. Users already using the provider's own assistant inside that provider is out of scope here.
- **Acceptance:**
  - [ ] No V1 acceptance. A future proposal must include provider revision/digest, schema validation, draft preview/diff, separate read/draft/mutate/test/publish/run/delete scopes, owner publish confirmation, idempotency, ambiguous publish reconciliation, audit receipt and provider-specific security tests.
- **Command:** none until the owner ADR and machine/API contracts are promoted.
- **Evidence:** not applicable.
- **Stop:** no generic agent tool that writes or publishes arbitrary external workflows; no in-Core graph model or code-execution node.

## Implementation and optimization notes (Rust and TypeScript)

Findings from reading the source, folded into the tasks above. They add no scope.

- **Fixed 30 s poll (Rust, T03/T10):** `run_worker_loop` sleeps the full interval regardless of the next due slot, so a slot can fire up to 30 s late and idle passes still open a transaction per hosted Workspace. Sleeping until `min(next_scheduled_at, 30 s)` and waking on the existing stop channel (or a second wake message on Resume) cuts latency and idle work without adding a scheduler.
- **Paused-Coworker retry loop (Rust, T05):** every pass re-runs a full admission transaction that is guaranteed to fail. One recorded skip plus a cursor advance removes it.
- **Settlement scan (Rust/SQLite, T12):** `settle_started_occurrences_from_task_outcomes` selects `WHERE workspace_id = ?1 AND status = 'STARTED'`; the existing indexes are `(automation_id, status, created_at)` and `(status, claim_expires_at)`. Check the query plan and add `(workspace_id, status, created_at)` in the T01 migration if it scans.
- **Delivery claim in one statement (Rust/SQLite, T06):** lease rows with a single `UPDATE ... WHERE delivery_id IN (SELECT ... WHERE status = 'PENDING' ORDER BY created_at LIMIT ?)` inside the writer transaction; `idx_notification_due (status, next_attempt_at)` already supports it.
- **Module size (Rust, optional with T02):** `automation_operator.rs` holds routes, the TriggerHost worker and settlement logic in 3,071 lines. Moving the worker and `settle_*` functions to `automation_host_operator.rs` is allowed by the FLEXIBLE file-layout rule and makes T05/T09 reviews smaller; do it only alongside a task that touches that code.
- **UI polling (TypeScript, T06):** poll claim with backoff and pause while the window is hidden; if the Feature 011 Operator stream carries `notification.delivery.changed.v1`, use it as the wake signal and keep the poll as fallback. Pure modules (`notification-presenter.ts`, `quiet-hours-form.ts`, `automation-history-state.ts`) keep logic testable with `node --test`. Suppression, timezone math and next-fire computation stay in Rust; the UI never recomputes them.

## Dependency order

T02 is independent. T01 -> T03 and T04 and T06. T04 -> T05 (also needs T01). T06 -> T07. T04 -> T08 (BLOCKED on 005). T01, T03, T04 -> T09 (BLOCKED on 006). T03, T05 -> T10. T02, T03, T06 -> T11. T04, T05, T06 -> T12. T09 -> T13. T06, T07, T09, T11 -> T14. T08, T10, T12, T13, T14 -> T15 -> T16 -> T17. T08 -> T18 -> T19 (outside the V1 gate).

## Cross-package dependencies

| This task | Other package task | Reason |
|---|---|---|
| T05 | Feature 009 Coworker pause fence and resume reconciliation | 009 owns Coworker status semantics; 010 owns the TriggerHost skip |
| T06, T10, T12 | Feature 011 Runtime incarnation, lifecycle, stop/quiescence and keep-awake | Leases and restart sweeps need the incarnation identity; sleep behavior is a Runtime concern |
| T06 | Feature 019 dependency audit | Any OS notification plugin is a new dependency |
| T07 | Feature 011 Settings shell link (A-35); Feature 020 proactive suggestion policy | 011 links to this control; 020 consumes the same quiet-hours contract |
| T08, T15, T18 | Feature 005 Task admission and execution (Linux row first, B-35 for Windows) | Occurrence Tasks execute only through 005 |
| T09, T15, T18 | Feature 006 Connection broker and CapabilityInvocation | CONNECTOR_EVENT reads provider events through a qualified Connection |
| T13 | Feature 019 security hardening | Secret scan and IPC negatives run in the release pipeline |
| T15, T17 | Feature 019 release matrix and evidence bundle | Qualification is per release row |

Cross-package tasks are named by outcome because the other packages are being renumbered in the same release-planning pass; the orchestrator resolves them to IDs in the traceability index.

## DeepSeek Harness proposals (PROPOSED, owner review required)

Status: PROPOSED. None of the items below is an accepted task. They come from the DeepSeek Harness studies ([desktop deep-dive](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) register A-63, proposals P-31 to P-46; [plugin deep-dive](../../docs/DEEPSEEK-HARNESS-PLUGIN-ARCHITECTURE-DEEP-DIVE-2026-11.md) register A-58). Owner review is required before any item becomes a numbered task in this package. They are labelled `PROP-Pnn` so they do not collide with the stable task IDs that `.specify/scripts/validate_feature_package.py` checks. On acceptance: assign the next free task ID in this file, copy the upstream permalink, class and reuse statement into the task, and update the [reuse map](../../docs/DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md). Upstream pin `fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159` (MIT, Copyright (c) 2026 DeepSeek). No upstream code is imported (AGENTS.md section 7).

### Proposals owned elsewhere that touch this package (pointer only)

- **PROP-P31 (P-31) QuiescenceReport (one "interruptible work" predicate; unknown means busy; lock then learn; inspect once)** [PROPOSED, not accepted, owner review required]: row owned by package 011; class **ADAPT**; would touch here: T12, T13, T14 of package 011; T11 of package 019; T10 of package 010. Upstream permalinks: [src/host-process.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/host-process.ts), [src/quit-confirmation.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/quit-confirmation.ts). Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 3 and 14 (row P-31). Reuse: Pattern only, reimplement. No upstream code or text is copied or imported (AGENTS.md section 7; upstream is Electron and TypeScript, LiteCowork is Tauri, Rust and React).


## Completion evidence

For each task append a row when it closes: task, code revision (SHA), exact command, result (pass/fail with counts), OS and build (Windows 11/10 x64 or Linux x64 distribution), harness and version (OpenCode v2.0.32 where a Conversation or agent is involved), auth route, evidence class reached (CODE, SYSTEM, USER), report path and reviewer. Statuses above are source-audit classifications at the date of this revision, not qualification.

| Task | Revision | Command | Result | OS/build | Harness/version | Class | Reviewer |
|---|---|---|---|---|---|---|---|
| T02 | `d6c5819` + working tree | `node --experimental-transform-types --test --test-concurrency=1 tests/automation-api.test.ts`; `pnpm test`; `cargo test -p litecoworkd --locked automation_route_tests` | focused Automation UI test PASS 14/14; full UI suite 626 passed / 5 failed / 5 skipped of 636 (three Chrome DevTools startup timeouts, two dependent Needs You browser-state failures); daemon route-filter PASS 2/2 | Windows working tree | n/a | CODE | repository owner |
