# Feature 010 current package and source cross-check — 2026-11-13

## Scope and evidence

This report records a line-by-line read of all 11 files in `specs/010-automations-recurring-host/`, followed by an independent read-only re-audit against the current Rust, React, SQLite, OpenAPI, event, data-model and UI contracts. The repository was on `main` at `d6c5819ca61a96f9fed0b39e1cbe10e511256358` when this continuation began, with pre-existing dirty changes from earlier authorized documentation/source cross-checks. The worktree still has one checkout. This report is a working-tree cross-check, not a claim that the entire repository or all active source has been reread.

The package has 1,322 physical lines. The following SHA-256 values refer to its current post-edit bytes:

| Package file | Lines | SHA-256 |
|---|---:|---|
| `README.md` | 33 | `b3c62cd35a455d6c2303345dd9efaa2eda93df9f7a2f6dbb8a7f52cd89c60a36` |
| `SPEC.md` | 411 | `ebbc6c7ed57e3c88e3d452f41d765dd43a7696627469b8dccce45d7e741f313d` |
| `PLAN.md` | 51 | `b2a3bdaa1d87bd99b9e105cf286b2034683b9b3c8e0fed3e149978067ccd68e9` |
| `TASKS.md` | 394 | `7a56db7ceb019a37dfce8f41b910b6c4a04de2758045270bcc85bc2c9fcce6f9` |
| `DATA-MODEL.md` | 45 | `c263864396763c8b4dac7b3acc98a7ee0864e09715919454c839c10fb9354c10` |
| `RESEARCH.md` | 128 | `fdac43c0587d07a4a73c5c767210da2eef9c7035908677fc43e5c12b2d2edf93` |
| `WIRING.yaml` | 171 | `93f64d561e8b74462d1b57d34dd0c616bb5a7c0d5d5fbef39cc66fdf48c8d084` |
| `contracts/README.md` | 9 | `24741dab3370e445f2a34e24d907fec21c72722cf3cad00c6b58618980baf307` |
| `contracts/api-events.md` | 18 | `ac8357f2fb960c77f45f2ec096147338e673815fcb68be3072d78ac256fe5568` |
| `contracts/errors-recovery.md` | 20 | `2f85cb81c4a6fdecc0d3cf116cb5db761c3f98757da7bc4d05e56a6247a2ee1a` |
| `contracts/ui-behavior.md` | 42 | `f222c0b0cef5b47c518e902006e29bf36966deb7414c0731f67d69eea3acca89` |

## Independent re-audit findings and corrections

The independent reviewer found that the first T02 Pause/Resume copy correction still did not match the source semantics. PAUSED status alone does not block Manual Run, but Run once is offered only when the revision has exactly one Manual trigger, the Automation is not disabled, the pinned Routine is active, inputs pass validation at submission, and no request is busy or unresolved for the selected Automation. An existing overdue schedule cursor is preserved and processed at the next host poll under its misfire policy. The Automation page now scopes its initial PAUSED-save confirmation to recurring schedule hosting, pause feedback to scheduled Task admission and existing Tasks, and leaves Manual Run availability to its action-specific controls. Resume explains that overdue slots follow the configured misfire policy at the next host poll and may create READY Tasks; the resume command itself creates no Task. The regression test pins these boundaries.

T02 remains **PARTIAL (CODE)**. The focused Automation UI test passes 14/14 and `cargo test -p litecoworkd --locked automation_route_tests` passes 2/2. The completed full `pnpm test` run reports 626 passed, 5 failed and 5 skipped of 636. Three browser tests timed out starting Chrome DevTools (`Runtime.enable`): mounted Needs You page (#304), Needs You request detail (#314), and mounted Resource editor race (#382). Two dependent Needs You request-detail tests (#315–316) failed when their browser state was unavailable. Automation tests passed within this run. It is not recorded as a full-suite pass. T02 only closes the truthful-copy portion of AC-019; occurrence-history pagination remains open under T11.

The reviewer independently confirmed these owner-level alignment gaps:

- **A-97 — quiet-hours ownership and atomicity:** accepted Feature 010 describes Workspace-level enabled/class selection, but current `QuietHours` is stored on one versioned per-event-class preference and the API writes one class per request. Do not build a multi-class editor until owners define one canonical, version-fenced atomic save contract.
- **A-98 — schema/migration and settlement fencing:** SQLite v1 already contains delivery states, attempt fields, and encrypted Runtime-local `automation_trigger_bindings`; it does not contain a delivery row version. The event name, envelope and payload shape exist, but the event JSON Schema permits arbitrary nonempty `from` and `to` strings rather than constraining them to delivery states. T01 must add or trace status-token validation and define the exact settle fence before reserving DDL. Release provisional v44 if the owner decision requires no migration.
- **A-99 — suppressed outcome conflict:** higher-authority `docs/DATA-MODEL.md` says a suppressed outcome writes no row, while accepted Feature 010 AC-007 requires a visible persisted `SUPPRESSED` row. Current code follows the global data model. Resolve in the owning domain before implementing AC-007; no owner semantics were silently selected here.
- **WIRING transaction boundary:** source writes a pending notification row in the same Immediate SQLite transaction as the occurrence outcome and its domain event. WIRING now models this as one transaction. Target claim/settle uses the canonical `notification_deliveries` row plus `domain_events`; it no longer invents claim/event tables. Quiet-hours atomic records remain unresolved under A-97. Connector payload digest is a field on `automation_occurrences`, not a separate record.
- **Task/status alignment:** corrected stale T02 source/UI baseline statements; distinguished the accepted CONNECTOR_EVENT kind from unserved event-trigger hosting; updated T01's stop condition to the qualified Feature 006 Connection dependency; corrected the T19 graph-authoring reference and explicit DEFERRED state; made the possible v44 migration conditional on owner-approved DDL.
- **Cross-document alignment:** refreshed AUTOMATION.md, FLOWS.md, the current SDD register, feature/UI coverage, the served UI map, implementation status, the contradiction register and dated audit language. Historical archives remain historical.

## Source facts checked

- `apps/litecoworkd/src/operator.rs` starts `AutomationTriggerHostWorker`; `automation_operator.rs` polls persisted local trigger cursors every 30 seconds. It materializes ordinary READY Tasks only; it does not execute Task bodies.
- `crates/storage-sqlite/src/coworkers.rs` preserves an existing cursor deadline when resuming; a missing schedule cursor is initialized separately. `crates/domain-responsibility/src/schedule.rs` applies the configured SKIP, RUN_ONCE_WHEN_AVAILABLE or CATCH_UP_BOUNDED policy to due slots.
- Manual Run is a separate owner-triggered one-shot path. PAUSED status alone does not block it, but the UI offers it only for exactly one Manual trigger, a non-disabled Automation, an active pinned Routine, inputs that pass validation at submission, and no busy or unresolved request for that Automation. Scheduled admission checks current status/version. There is a narrow host snapshot race that may record skipped history after pause, but scheduled Task admission rechecks status/version.
- `crates/storage-sqlite/src/automation_host.rs` commits skipped occurrence state/events and any notification decision in one transaction; terminal occurrence settlement similarly writes its occurrence event and notification decision in one transaction.
- `crates/storage-sqlite/src/notification.rs` writes only PENDING deliveries on an eligible decision; suppression writes no delivery row; no production claim/presenter/settlement worker advances statuses. Invalid timezone text is accepted by the storage shape check and parse failure is treated as outside quiet hours.
- Notifications preferences are per event class; the current UI displays quiet hours without editing them, and the accepted class-selection control has no atomic multi-class API.
- `AutomationsPage.tsx` exposes only the first occurrence-history page and discards `next_cursor`; definition pagination does have Load more.
- External WorkflowBackendAdapter and event-trigger hosting are target-only. CONNECTOR_EVENT was owner-confirmed by A-62; T09 is blocked on a qualified Feature 006 Connection event source, not on choosing the event kind.

The independent reviewer also checked the relevant canonical contract facts in `docs/schemas/sqlite-v1.sql` (tables `automation_occurrences`, `automation_trigger_bindings`, `notification_deliveries`, `domain_events`), `docs/schemas/domain-event.schema.json`, `docs/DATA-MODEL.md`, `docs/AUTOMATION.md`, Feature 010 TASKS/SPEC and the active Operator/Storage/UI source. This is source and contract evidence, not OS/provider/runtime qualification.

## Task state at this cross-check

| Task | State | Current evidence / dependency |
|---|---|---|
| T01 | PARTIAL | Missing grammar/catch-up and slot/error details, status-token validation, claim/settle route/fence; A-97/A-98/A-99 constrain dependent work |
| T02 | PARTIAL (CODE) | Correct header/copy; focused UI 14/14 and daemon route 2/2; full UI suite has separate CDP timeout; only truthful-copy portion of AC-019 |
| T03 | PARTIAL | Schedule/DST/misfire code and unit evidence exist; owner grammar/cap and qualified platform journey remain |
| T04 | PARTIAL | Idempotent repeated-slot path exists; all concurrent/digest-conflict cases not proven |
| T05 | PARTIAL | Non-ACTIVE Coworker becomes blocker/retry; no persisted Coworker-paused skip |
| T06 | NOT STARTED | PENDING row producer exists; no sender/claim/status transitions |
| T07 | PARTIAL | Suppression decision and preference value exist; A-99, A-97, timezone fail-open, no editor |
| T08 | BLOCKED | Feature 005 Task execution is not qualified |
| T09 | BLOCKED | CONNECTOR_EVENT accepted; qualified Feature 006 Connection source absent |
| T10 | NOT STARTED | Close/sleep/Runtime stop matrix absent |
| T11 | PARTIAL | Definition Load more exists; occurrence history is first-page-only |
| T12 | PARTIAL | Cursor monotonicity/repeated settlement tests exist; crash/lease/delivery restart cases incomplete |
| T13 | PARTIAL | Some Routine input/security checks exist; socket and untrusted-event containment tests absent |
| T14 | NOT STARTED | Cross-layer trace incomplete |
| T15 | BLOCKED | Requires T09, Feature 005 Linux execution, and B-35 Windows Environment provider |
| T16 | NOT STARTED | Status/inventory synchronization remains |
| T17 | NOT STARTED | Feature package is not VERIFIED for V1 |
| T18 | BLOCKED, outside V1 release gate | Requires qualified Feature 005/006 and external WorkflowBackendAdapter |
| T19 | DEFERRED | Owner-confirmed outside V1; no implementation authorization |

## Next work order

1. Obtain owner resolution for A-97 and A-99 before building the quiet-hours editor or SUPPRESSED persistence path. Resolve A-98's CAS/lease fence and exact DDL before producing a migration; add enum validation for delivery transitions.
2. Promote only the missing owner contracts and schemas through T01; retain the existing v1 status, attempt and encrypted cursor fields.
3. Rerun the full UI suite in a healthy mounted-browser environment; T02 remains PARTIAL until its declared suite requirement is green. Continue Feature 010 in dependency order rather than treating code foundations as qualification.
4. Implement the remaining V1 tasks only when their owning prerequisites are qualified: T09 with Feature 006, T08/T15 with Feature 005 and the platform-specific B-35 decision.

No packaged Automation journey, real notification delivery, external event source, native Windows/Linux qualification or owner USER evidence was produced in this cross-check.
