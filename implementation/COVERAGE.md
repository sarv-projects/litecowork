# Coverage map

The generated [architecture and contract coverage audit](ARCHITECTURE-COVERAGE.md) is the
human-readable report. The [coverage CSV](coverage.csv) maps every tracked architecture
Markdown file and heading, every ADR, every numbered flow and benchmark, all backlog stories,
and enumerated machine-contract objects to a primary implementation guardian.

The [machine inventory](machine-inventory.json) and
[`validate_implementation_coverage.py`](../scripts/validate_implementation_coverage.py)
enumerate and digest-pin the current OpenAPI operations/components, shared schema names,
error codes, event types/payloads, and SQLite objects/fields/constraints. The checker also
compares the prose API route inventory with OpenAPI and confirms that public schemas are
reachable from an operation.

Run `python3 scripts/validate_implementation_plan.py`; it runs the coverage checker too.
The explicit checker command is
`python3 scripts/validate_implementation_coverage.py`. After reviewing an architecture
change, regenerate with `python3 scripts/validate_implementation_coverage.py --write` and
review the generated diff.

These checks prove traceability and contract inventory consistency. They do not prove that
features work. Each planned story still requires its CODE, SYSTEM, and owner USER case to be
implemented and pass with real providers and target environments where applicable. See
[what remains uncovered](ARCHITECTURE-COVERAGE.md#not-covered-by-this-plan-yet).


## Target docs added after the 2026-10-10 generated coverage snapshot

docs/REFERENCE-IMPLEMENTATIONS.md, docs/CONNECTORS-WORKFLOWS.md and docs/LEARNING.md are
architecture inputs added by the final connector/learning research pass. They do not create
executable API/SQL/event objects yet. The next implementation story that promotes one must
regenerate machine/coverage inventory; until then the generated CSV remains a snapshot of
current canonical machine contracts, not a reason to pretend target routes are shipped.
