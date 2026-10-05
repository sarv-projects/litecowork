# LiteCowork implementation plan

Status: planned; no application implementation or product qualification is claimed.
Baseline: `8af59a5b9be0a00b12849caf40eac287527ac43e`. Prepared 2026-10-05.

This is the delivery plan for coding agents with one owner reviewing the work. The
architecture and domain documents remain contract authority. This folder owns execution
ordering, story acceptance, evidence, research references, and release gates.

## Read in order

1. [Current work and agent handoff](CURRENT-RUN.md)
2. [Scope and release gates](SCOPE.md)
3. [Current contract audit and optimizations](AUDIT.md)
4. [Stack recommendations and qualification spikes](STACK.md)
5. [Agent development and Agile process](PROCESS.md)
6. [Roadmap](ROADMAP.md), then the dependency-ready [story backlog](backlog.json)
7. [UI delivery](UI.md) and [file/folder/ZIP retrieval delivery](RAG.md)
8. [Code, implementation, and real-work testing](TESTING.md)
9. [Practical Cowork replacement assessment](WORKFLOWS.md)
10. [Research and repository reference catalogue](SOURCES.md)
11. [Production operations and release checklist](RELEASE.md)

[Architecture coverage](ARCHITECTURE-COVERAGE.md) lists every tracked architecture source,
heading, ADR, flow, benchmark, story, and enumerated machine-contract object with a primary
implementation guardian. This is traceability, not proof of implementation. The
[baseline audit inventory](audit-inventory.csv) records the source files, sizes, and digests
examined. The generated [machine inventory](machine-inventory.json) is checked against the
current API, schemas, events, and SQLite contract.

Run `python3 scripts/validate_implementation_plan.py` alongside the architecture validator.
This also runs the coverage checker. The checks prove contract and backlog consistency; they
do not prove that the product works. The owner must implement and review each story's
CODE, SYSTEM, and USER evidence before accepting it.
