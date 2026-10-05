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
| [E11](epics/E11.md) | Cloud continuation and messaging / G3 | E10 | Close laptop for cloud-eligible work; local-app Task explicitly waits. |
| [E12](epics/E12.md) | Remote Runtime / G4 | E11 | Execute code task remotely, disconnect mid-run and reconcile on reconnect. |
| [E13](epics/E13.md) | Production release qualification / G5 | E12 | Install/update/uninstall on supported OS and execute incident drill. |

## First implementation cycle

Start E01-S01 with SP01–SP04 reports, selecting a real first adapter. Then E01-S02–S04:
one durable Workspace transaction and an authenticated client with replay. Next E02:
installable shell and Resource intake. E03/E04 produce the first verified local task and
kill/restart recovery. G1 requires that whole behavior, not a page of successful mocks.

E05–E10 complete local workforce, RAG, local models, browser/office, persistent identity,
context, responsibility, rich outputs and reusable skills. Early Coworker naming can be
introduced in E02 as UI preparation, but E08 owns durable identity/allowlist semantics.
E06 is independent of E05 after E04; choose according to owner review bandwidth.

E11 is deliberately after desktop completion: cloud deployment/pairing/handoff/channel.
E12 uses the same Runtime on a remote machine. E13 closes production evidence and release.
Tests, backup primitives, diagnostics and security are developed continuously; E13 is
qualification/soak, not the first time they are implemented. Every earlier story includes
its actual failure paths and operational requirements.

## Scope-preserving decomposition

A story can contain multiple implementation tasks. Split into IDs such as E06-S02-A,
retain parent acceptance, add explicit dependency edges in backlog, and update validator
support if a new ID shape is introduced. Do not call an epic done merely because its
first representative screen exists. All finalized APIs/transitions and assigned scenarios
must be accounted for by the epic's exit. Integrated provider breadth remains an explicit
qualification matrix; a feature advertised unavailable cannot meet a required release gate.
