# LiteCowork implementation plan

Status: planned; no application implementation or product qualification is claimed.
Baseline: `8af59a5b9be0a00b12849caf40eac287527ac43e`. Prepared 2026-10-05.

This is the delivery plan for coding agents with one owner reviewing the work. The
architecture and domain documents remain contract authority. This folder owns execution
ordering, story acceptance, evidence, research references, and release gates.

## Read in order

1. [Scope and release gates](SCOPE.md)
2. [Current contract audit and optimizations](AUDIT.md)
3. [Stack recommendations and qualification spikes](STACK.md)
4. [Agent development and Agile process](PROCESS.md)
5. [Roadmap](ROADMAP.md), then the dependency-ready [story backlog](backlog.json)
6. [UI delivery](UI.md) and [file/folder/ZIP retrieval delivery](RAG.md)
7. [Code, implementation, and real-work testing](TESTING.md)
8. [Practical Cowork replacement assessment](WORKFLOWS.md)
9. [Research and repository reference catalogue](SOURCES.md)
10. [Production operations and release checklist](RELEASE.md)

[Contract coverage](coverage.csv) assigns every existing flow, benchmark, and authority
document to a delivery story. Assignment is planning coverage, not a passed test.
[Baseline inventory](audit-inventory.csv) records the files examined structurally, their
sizes, and digests. [Machine inventory](machine-inventory.json) enumerates API operations,
event payload definitions, and SQL objects; stories must refine those into executable
contract tests before their domain exits.

Run `python3 scripts/validate_implementation_plan.py` alongside the architecture validator.
The checks prove document/backlog integrity; they do not prove the product works.
