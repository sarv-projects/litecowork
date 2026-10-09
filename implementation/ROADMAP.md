# Delivery roadmap

The dependency graph in [backlog](backlog.json) is authoritative for ready work.
No invented calendar schedule; one-week planning/review windows follow [process](PROCESS.md).

| Epic | Increment and gate | Dependencies | First owner demo |
|---|---|---|---|
| [E01](epics/E01.md) | Development foundation / G0 | none | Use desktop client and adversarial HTTP client against same daemon. |
| [E02](epics/E02.md) | Desktop shell and resources / G1 | E01 | Create two blockers, resolve one, replay event and observe unchanged inbox. |
| [E03](epics/E03.md) | Full native agent and durable work / G1 | E02 | Run native-feature suite outside and inside host and report actual differences. |
| [E04](epics/E04.md) | Trust, capabilities and effects / G1 | E03 | Produce diff/report, independently check it and show verified versus reported status. |
| [E05](epics/E05.md) | Heterogeneous workforce and cost / G2 | E04 | Run same code task corpus with baseline and delegated policy. |
| [E06](epics/E06.md) | Documents, RAG and local models / G2 | E04 | Drag a ZIP, inspect readiness and citation, attempt disallowed cloud task. |
| [E07](epics/E07.md) | Environments, browser and office work / G2 | E05, E06 | Operate permitted local app with keyboard takeover and recorded real activity. |
| [E08](epics/E08.md) | Coworker, goals, context and product UX / G2 | E05, E06 | Owner completes task/approval/takeover solely by keyboard and reduced motion. |
| [E09](epics/E09.md) | Responsibilities, scheduling and notifications / G2 | E07, E08 | Complete Task with app closed; reopen correct Artifact from notification. |
| [E10](epics/E10.md) | Reusable skills and capability UI / G2 | E09 | Teach sandbox browser task, review/test draft, approve package publication when qualified. |
| [E13](epics/E13.md) | Desktop/local V1 production release / G3 | E10 | Install/update/uninstall on supported desktop OS and execute incident drill. |
| [E11](epics/E11.md) | Post-V1 cloud continuation and messaging | E13 | Close laptop for cloud-eligible work; local-app Task explicitly waits. |
| [E12](epics/E12.md) | Post-V1 Remote Runtime | E11 | Execute code task remotely, disconnect mid-run and reconcile on reconnect. |

## First implementation cycle

Start with E01-S01: pin Rust/Python development tools, build the minimal `litecoworkd`
help/version executable, and make the local/CI contract gate reproducible. This is a
build foundation, not an operational daemon. Then implement E01-S02 storage while running
SP02, E01-S03 lifecycle, and E01-S04 authenticated transport while running SP03. Run SP01
with E02-S01 before committing to the desktop shell; run SP04 alongside E03-S01 before
selecting the first native adapter. G1 still requires a complete verified local task, not
a successful shell page or a set of mocks.

E05–E10 complete local workforce, RAG, local models, browser/office, persistent identity,
context, responsibility, rich outputs and reusable skills. Early Coworker naming can be
introduced in E02 as UI preparation, but E08 owns durable identity/allowlist semantics.
E06 is independent of E05 after E04; choose according to owner review bandwidth.

E08 is intentionally split into reviewable UI increments: E08-S04 establishes truthful
Work/activity projections; E08-S06 adds typed PresentationItems and reconnect-safe
transient streaming; E08-S07 delivers the Artifact Workbench and immutable version
history; E08-S05 validates accessibility and nontechnical/technical end-to-end journeys
across those surfaces. Context intake and source readiness remain E06-S05; context
editing/revocation remains E08-S03. E08-S08 completes semantic-first optional
RichPresentation, bounded Host Guidance, safe presentation compilation, and exact-version
deliverable rendering after the semantic Conversation path and Artifact bindings exist.
These are staged implementations of the finalized desktop/local V1 feature set, not scope
cuts. E08-S08 does not pull Cloud continuation or Remote Runtime into the V1 critical path.

E11 and E12 are post-V1: cloud deployment/pairing/handoff/channel, then the same Runtime
on a remote machine. E13 closes desktop/local V1 production evidence and release before
either post-V1 program is scheduled. Tests, local backup primitives, diagnostics and
security are developed continuously; E13 is qualification/soak, not the first time they
are implemented. Every earlier local story includes its actual failure paths and
operational requirements.

## Scope-preserving decomposition

A story can contain multiple implementation tasks. Split into IDs such as E06-S02-A,
retain parent acceptance, add explicit dependency edges in backlog, and update validator
support if a new ID shape is introduced. Do not call an epic done merely because its
first representative screen exists. All finalized APIs/transitions and assigned scenarios
must be accounted for by the epic's exit. Integrated provider breadth remains an explicit
qualification matrix; a feature advertised unavailable cannot meet a required release gate.

## Reconciled Coworker implementation sequence

E08-S01/04 first establish optional chat-first ownership with real Conversation creation/list/last-active and honest provider-disabled states, Quick Create, and preserved Workbench context. E04/E10 enable qualified connection assignments; E08-S03 gains deterministic scoped memory candidate eligibility and qualified extraction, independently of proactive actions. E09 then enables reviewed standing responsibilities, trigger outbox/recovery, background service, missing-connection/Needs You and run history using the existing Task engine. This target modifies feature acceptance, not the actual dependency-ready status; backlog and machine-level contract changes require corresponding verified story updates.
