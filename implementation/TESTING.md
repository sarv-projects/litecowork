# Tests and acceptance evidence

The architecture [test contract](../docs/TESTING.md) and [benchmarks](../docs/BENCHMARKS.md)
remain authority. Each story has CODE, SYSTEM and USER test IDs; these are planned cases,
not existing executable tests. Assigned flow/benchmark scenarios must become executable
suites with exact expected states/events/Effects/Evidence. Test stubs cannot count as pass.

## Three levels

| Level | Tools / scope | Required evidence |
|---|---|---|
| Code | Unit/property, API/event/schema contract, SQL/migration, deterministic integration | Positive/negative guards, transaction atomicity, legal transitions, RequestId/revision behavior |
| Implementation | Actual native daemon/UI/adapter/provider/OS/install + fault/load | Versioned process protocol behavior, permissions/egress, restart/kill/partition, real render/inference |
| Real user work | Owner performs useful task against fixed inputs and oracle | Usable Artifact, source/criteria truth, interventions, cost, recovery, explicit owner acceptance |

A domain's fake provider tests verify coordination; they cannot qualify the upstream
provider. Use sanitized captured protocols for repeatable regressions and real adapter
smoke suites for changing upstream behavior. Pin adapter/provider versions in evidence.
Model-dependent tests separate deterministic invariants from stochastic quality, run
repeated trials with declared seeds/model/options where supported, and report distribution.

## Case record format

```text
Case ID; story/flow/benchmark IDs; build/OS/adapter/provider versions
Fixture manifest and digests; grants/policy/placement; model/hardware/config
Initial state; actions; injected fault phase; expected states/events/outputs
Observed state/event/Effect counts; Evidence and Artifact refs
Assertions; cost/latency/interventions; redacted logs/screenshots
PASS / FAIL / BLOCKED / scoped NOT_APPLICABLE; owner decision and rerun reason
```

## Executable real-work corpus

Synthetic baseline fixtures should be committed; private account runs never are. Numbers
below define fixture oracles, not model success/performance claims.

| ID | Fixture and action | Oracle / negative extension | Main stories |
|---|---|---|---|
| U01 | 100 files incl duplicates; organize into reviewed folders | Digest multiset unchanged, no outside-root move; stale file edit aborts | E02-S03, E07-S05 |
| U02 | 12 receipts, two currencies, known subtotal/tax | Exact per-currency totals and source rows; unreadable receipt flagged, no invented value | E06-S01, E07-S04 |
| U03 | 20 PDFs + approved web sources; answer gold questions | Every factual source citation resolves correct span; unanswerable questions explicit | E06-S02 |
| U04 | Meeting transcript with 8 agreed actions and 2 rejected proposals | Correct owner/date/source for agreed items; rejected proposals not assigned | E07-S04 |
| U05 | Forecast workbook with known formulas + slides/PDF | Recalculated gold cells, preserved formulas; rendered pages legible; no missing chart | E07-S04, E08-S07 |
| U06 | Test mailbox 30 messages, 4 obligations and forged instructions | Correct brief/proposed calendar entries; forged email cannot grant/send | E04-S01, E09-S01 |
| U07 | Repo with failing test and known bug; code fix | Regression suite passes, minimal diff, no secret/config rewrite; worker kill recover | E03-S04, E05-S02 |
| U08 | Two worker edits in isolated worktrees | No shared-checkout race; merge conflict explicit; independent verifier determines success | E05-S02/S04 |
| U09 | Calendar DST boundary and duplicate incoming event | One authorized event at correct timezone; duplicate delivery doesn't create second | E04-S03, E09-S02 |
| U10 | Controlled browser form and login handoff | Fields exact; credentials absent from prompt/artifact; final sensitive submit gated | E07-S02 |
| U11 | Take over mid-form, user changes value | Old-epoch input rejected; fresh agent observation uses new value; one control actor | E07-S02 |
| U12 | Spending CSV + interactive dashboard + edit | Gold aggregates, isolated preview no network/credential escape; new version not overwrite | E08-S07 |
| U13 | Weekly report schedule incl restart/DST/edit | Correct logical occurrences, no cursor reset, explicit misfire policy and pinned revisions | E09-S02 |
| U14 | Site monitor unchanged then changed source | No LLM polling when deterministic no-change; single useful authorized notification | E09-S02/S04 |
| U15 | Context edited concurrently then revoked/deleted | Conflict and explicit merge; zero revoked fresh retrieval; exact purge receipts before DELETED | E08-S03 |
| U16 | Goal with 3 Tasks incl unverified completion claim | Progress uses only valid verified outcomes; suggestion cannot execute itself | E08-S02 |
| U17 | Teach 8 semantic browser actions incl private login | Login capture paused/redacted; draft reviewed/tested; selector drift yields repair proposal | E10-S02 |
| U18 | Cloud-eligible report with portable inputs; close laptop | New cloud Attempt/epoch with same Task and verified output; no DB/session teleport | E11-S03 |
| U19 | Cloud Task needs a local nonportable office app | Explicit waiting/dependency; no false continuing/completed status | E11-S05 |
| U20 | Remote repo task, host reboot during execution | Correct remote lease/recovery; no stale result commit; eligible resources verified | E12-S02 |
| U21 | ZIP traversal/bomb/corrupt PDF corpus | No extraction escape; bounded resource use; per-file errors visible | E06-S01 |
| U22 | Airplane-mode local-model knowledge/code task | Zero external network; quality against gold; cancel/OOM clean and actionable | E06-S04 |
| U23 | Provider send succeeds but ACK lost | Effect ambiguous→reconcile, one external message; no blind resend | E04-S03 |
| U24 | Premium-only vs cheap workforce same task corpus | Report success/usage/known cost/time/retries/rescues; unknown units explicit | E05-S05 |
| U25 | Clean install, active Task upgrade, encrypted restore elsewhere | New Runtime identity; no restored lease; pinned revisions/tombstones intact | E13-S01/S04 |
| U26 | Keyboard/screen-reader/reduced-motion approval/task flow | Correct focus/semantic announcements, accessible control, real event-driven status | E08-S05 |
| U27 | Host cold/warm/config changed/hold evicted | No model token use for prewarm; fresh admission checks; warm loss affects latency only | E07-S03 |
| U28 | ActionBatch first action succeeds, third fails | Individual invocation/effect ordinal mapping; reconcile partial work, no transactional rollback fiction | E07-S03 |
| U29 | Channel duplicate/forged/late reply to UserRequest | Identity/assurance/correlation enforced; weak channel cannot approve high-risk action | E11-S04 |
| U30 | Large row dataset, partition/merge under memory bound | Gold count/checksum/statistics; no truncated output reported successful; retries bounded | E07-S04, E13-S02 |
| U31 | Reassign a channel while one receipt is processing and the source lease is near expiry | New claims stop at DRAINING; only the original live claim settles; reassignment waits for proof or expiry plus skew; v2 history records release basis and continuity provenance | E11-S04 |

## Owner real-world user session suite

Synthetic cases keep regressions repeatable; they do not satisfy real-world acceptance.
Before G2, G3, G4 and G5, the owner performs the required product tasks with the installed
build using owner-approved real repositories/files, realistic mixed document folders,
qualified test accounts and non-production external destinations. At minimum each
finalized user workflow gets one real session: inspect/fix/review an actual coding repo;
organize a chosen local folder; analyze a realistic PDF/receipt/spreadsheet bundle; produce
and revise a research/document artifact; use a permitted connector and review an approved
write in a sandbox account; prepare a browser action then take over; run the configured
routine twice including one restart; continue portable work in cloud with the desktop
closed; and run one eligible Task on a paired remote Runtime. Include a locally hosted model
path on hardware the owner expects to use.

For each session the owner records the original task request, intended resource scope,
expected output or manual comparison oracle, elapsed time, blockers/interventions, verified
criteria, artifact usability, authority decisions, cost/unknown usage and any failed or
unsupported step. Compare generated outputs to source data and the baseline native app or
manual result where possible. A coding agent's self-assessment is not acceptance. Private
files/accounts/screenshots stay under owner control; commit only synthetic/redacted evidence
summaries and fixture hashes. Repeat the session after fixes and on each claimed OS/provider
combination. The owner can stop a risky external action; this suite never uses real payment
or irreversible-send accounts to satisfy a test.

## System failure matrix

Inject faults before admission, after aggregate+event commit, after external request, before
ACK, during verification, during lease renewal and after checkpoint. Kill agent, capability
host, daemon and browser separately. Inject network partition, duplicate/reordered events,
Hub loss, disk full/corruption, stale config/auth, revoked grant/source, sleep/wake/clock skew.
Assert no duplicate consequential Effect, no stale fence accepted, no half transaction,
no false completed/verified UI, no secret payload, bounded retries and actionable blocker.

State-machine properties enumerate legal/illegal transitions for every schema enum.
API tests cover all inventoried operations: authentication/scope, required fields, version
conflict, replay, pagination/cursor, typed errors and content secrecy. SQL tests cover
contract triggers/indexes/foreign keys and migration history. Event tests verify aggregate
owner/version/payload/replication class; ephemeral token/PID data never becomes domain truth.

## Platform, performance and coverage

Matrix: Linux reference OS, then declared Windows/macOS installers; native harnesses and
local engine models; local/cloud/remote; internet/airplane/offline; cold/warm; keyboard and
screen reader. Publish unavailable cells and blockers. No successful Linux mock labels
macOS native computer use supported. Use actual OS runners/devices and owner test accounts.

Existing B13/B14 mobile interaction clauses: V1 validates channel/cross-device domain
behavior through desktop/second desktop and supported human channel. Native mobile UI
rendering remains explicitly out of scope; record that variant, never claim the unrun
mobile original passed. All other scenario assertions remain. The owner accepts scoped
NOT_APPLICABLE only for explicitly deferred client/platform integrations, not a broken
required feature or missing provider authority.

Measure daemon idle/startup, event-to-UI lag, task admission/recovery, source/index/query,
provider start, model generation, memory/VRAM, queues, steady-state soak and cost separately.
Freeze numeric targets after SP01–SP07 on named hardware/corpus; production gate cannot
pass with performance targets still blank. Safety invariants above are categorical.
Coverage counts are bookkeeping; use fault oracles, mutation tests for critical guards
and escaped-defect trends to judge test effectiveness.

## CI progression

Every PR: architecture+plan checks, formatting/types/build, unit/property/contract/SQLite,
frontend components and local integration. Nightly: real native smoke, controlled browser,
recovery matrix and retrieval corpus. Release: installers/upgrades on supported OS,
cloud/remote fault drill, full real-use corpus, accessibility, dependency/license scan,
benchmark comparison and owner review. Quarantined flaky tests need issue/owner/expiry and
cannot hide a release-critical failure. Paid provider runs are budgeted and never use
production send/payment accounts without the owner's test authorization.
