# Coworker, Goal, and Automation domain boundary

This crate implements typed Coworker and Automation commands plus the passive Goal
domain boundary. Current source includes SQLite-backed Coworker and Automation definition
persistence, authenticated Coworker/Goal/Automation Operator routes, a Goal SQLite adapter
and bounded progress projection, and desktop settings/Goal/Automation source slices.
Automation activation/recurring trigger hosting is deliberately incomplete: the local
path supports stored definitions and an explicit one-shot ManualTrigger Task admission,
not a qualified recurring/provider TriggerCoordinator. Coworker presence has a route/UI
shape but remains unavailable until its committed activity/runtime projections exist.
Current integration status is tracked in `implementation/CURRENT-RUN.md`; source presence
and passing unit tests do not satisfy system or owner acceptance gates.

Authority: [RESPONSIBILITIES](../../docs/RESPONSIBILITIES.md),
[AUTOMATION](../../docs/AUTOMATION.md), [DATA-MODEL](../../docs/DATA-MODEL.md),
[STATE-MACHINES](../../docs/STATE-MACHINES.md), [SCHEMAS](../../docs/SCHEMAS.md),
[E08](../../implementation/epics/E08.md), [E09](../../implementation/epics/E09.md).

## Implemented boundary

- Coworker and Automation mutable heads plus immutable canonical revision values.
  Revision content serializes as flat canonical fields, not a nested `definition`.
- Owner command preparation and transaction ports for creation, immutable append,
  expected-version status changes, atomic event writes and RequestId replay.
- Coworker pause/archive/resume rules, including primary selection and outstanding
  responsibility guards. Owner-origin Tasks can select a paused Coworker;
  proactive work requires ACTIVE. Archived identities cannot seed new work.
- Automation creation is PAUSED; enabling is a separate owner command. DISABLED
  is terminal. Lifecycle changes do not rewrite revisions or existing Tasks.
- Independent ANY trigger identities, unique trigger IDs and placement checks;
  retained IDs reject changed logical sources. Five V1 definition variants are
  represented, without claiming any trigger provider is implemented or qualified.
- Exact v2 occurrence key encoding and schedule/one-shot/manual/stable webhook/
  connector delivery identities. Revision edits are absent from occurrence keys.
- A separate private test-command identity pins an exact Automation revision. It
  does not create an occurrence and cannot mutate a cursor. This fingerprint is
  an internal proposed Task admission seam, not a new public transport contract.
- Passive Goal identity/revision/status types, reference-validation port, versioned
  create/revise/status commands, event drafts, idempotency fingerprint, and explicit
  lifecycle transitions. Goal mutations cannot create Tasks; the adapter must validate
  linked same-Workspace Task/Routine revisions and persist the event/revision atomically.

Context memory confirmation, profile uniqueness, positive revisions/concurrency,
recurrence metadata and policy bounds have local checks. References and schemas
owned by Resource, Agent, Budget and Trust boundaries remain obligations of the
transaction port; an object value is not proof of validity or authorization.

## Production integration seams

`ResponsibilityStore::transaction` accepts an owned, deterministic, repeatable
`Send + 'static` decision callback. The SQLite adapter evaluates it during
preparation, releases the SQL transaction to commit/verify the immutable aggregate
state blob, then evaluates it against fresh final transaction state. The exact
mutation and canonical snapshot must match before commit. Runtime event metadata
and the command clock remain fixed between phases. Other adapters must enforce the
same state-reference boundary.

`ResponsibilityStore::transaction` must enforce current Workspace ownership,
exact RequestId command reuse, Workspace isolation, rollback, durable CAS,
immutable append, one event envelope and the stored command response atomically.
It must never acknowledge an in-memory result as durable. Authentication must
precede replay so a revoked principal cannot recover an old response.

`ResponsibilityTransaction` validates references and lifecycle facts inside that
same write transaction. The adapter must reject cross-Workspace reads, validate
pinned avatar/Routine/Coworker revisions and Agent/profile state, authenticate
trigger providers, and validate all nested owning schemas including limits,
recurrence grammar, timestamps/timezones, filters, budget ceilings and error codes.
The adapter must recheck any resume preflight observations for freshness before
commit. External probes happen before the transaction; immutable observation
references and authoritative local facts are checked inside it.

Updating an enabled Automation must atomically carry retained cursors to the new
revision. Host replacement requires provider-supported transfer or bounded rescan
and exclusive fenced ownership. Source replacements use new trigger IDs. Removed
triggers cannot discard pending occurrences. Pause/disable stops claims while
retaining or explicitly settling unmaterialized occurrences; materialized Tasks
continue. No network call belongs inside the transaction.

TriggerCoordinator integration is deliberately pending: atomic delivery receipt,
cursor, claim epoch, occurrence and ordinary Task link; payload-digest conflict
checks; expiry/fencing; misfire, overlap and cancellation settlement. This crate
never simulates these with a persistence map. Webhook dedupe without stable delivery
IDs requires a qualified bounded observation/window provider and is not supplied.

TaskService must validate current Coworker status at origin admission, pin the exact
selected revision in Task provenance, and apply normal lead/Trust/budget/dependency
rules. Exact-revision tests must reserve TestRunIdentity with one ordinary Task
atomically, enforce input/auth/Effect rules, and keep occurrence/cursor state intact.
No test means dry run, grants new authority, or implicitly enables scheduling.

Workspace primary selection and Coworker/Goal/Automation Operator/UI slices now exist in
partial form. Truthful Coworker presence and complete health projections remain pending;
the current presence route fails explicitly when its required committed Task,
UserRequest, Attempt, and Runtime projections are unavailable.

## Validation status

Unit tests cover canonical vectors/UTF-8 framing, typed codecs, pause/archive/resume
guards, terminal disable, stale versions/overflow, source identity, and separate test-run
identity. Later repository verification on 2026-10-09 ran `cargo test --workspace` and
recorded all 21 `domain-responsibility` tests passing; the same run also exercised the
current workspace storage/daemon test suites. This is CODE-level evidence only: provider
trigger hosting, restart/system behavior, supported-OS qualification, and owner acceptance
remain open. Automation definition persistence is implemented; recurring/provider trigger
coordination and production occurrence settlement are not.

## Coworker SQLite adapter

`SqliteCoworkerStore::new` takes the existing `SqliteWorkspaceStore` and one
Runtime-generated `CoworkerEventContext`; `ResponsibilityService::execute` takes
transport-authenticated owner scope and a typed Coworker command. Read methods
require current active-Workspace ownership and provide exact current revision
and bounded keyset pagination. Automation create/revise/read persistence supports the
current definition contract; enabling/active hosting fails closed for unsupported
non-Manual triggers or missing qualified TriggerHost/reconciliation support.

Preparation and final SQL transactions both check current ownership and exact
RequestId reuse before evaluating the domain callback. Between them the adapter
commits and reads back the encrypted content-addressed state blob through the
existing BlobStore. The final transaction reruns CAS, reference and lifecycle
checks and rejects a changed mutation or canonical snapshot. Head, immutable
revision, origin sequence, state reference, event, and replay response commit
together. A failed transaction can leave an unreferenced immutable blob for the
existing delayed garbage-collection boundary.

The adapter validates same-Workspace lead/profile/failover state, pinned active
avatar Resource revision, and complete delegation budget policy shape. Archive
checks match canonical SQL: a primary identity, any non-disabled owned Automation,
any unsettled owned occurrence, or any nonterminal originating Task blocks archive.
Primary selection stays a separate Workspace command. Resume returns
`ReconciliationRequired` while a retained Automation or occurrence needs trigger
reconciliation, because that provider path is intentionally absent from this slice.
Existing Tasks and immutable historical revisions remain unchanged.

Concrete SQLite test source covers restart/readback, immutable prior revision,
state-blob content, command replay/conflict, stale version, owner isolation, pause,
primary/archive guards, and authorization before replay. Later workspace test execution
passed the integrated Rust suite, but real provider/TriggerCoordinator behavior,
platform/system qualification, and owner review remain pending; no story completion claim
is made.
