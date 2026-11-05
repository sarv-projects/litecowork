# Digest archive-01

Files: 6 opened / 1 listed in plan | Lines read: ~120 of 7,394 in `coverage.csv` (sampled by offset, not linear) | Bytes: coverage.csv 1,423,741 bytes (all other files partially read)

**Plan/brief mismatch (recorded, not resolved):** `reading/plan/archive-01.txt` lists exactly **one** file — `docs/archive/legacy-implementation-c7e9c05/coverage.csv  1423741  7394`. The brief names ~10 artifacts (machine-inventory.json, audit-inventory.csv, V1-MARKDOWN-INVENTORY.csv, V1-EVERY-MD-AUDIT, served-route-inventory.json, backlog.json, V1-CROSS-CONTRACT-GUARD.json, V1-DEEP-SEMANTIC-SUMMARY.json, spikes). All of those exist in the same archive directory, but only coverage.csv is in the plan. I read coverage.csv plus the four derived/summary files that give its exact counts; the rest are **unread** (see Coverage check).

## Per-file notes

### docs/archive/legacy-implementation-c7e9c05/coverage.csv (7,394 lines)
- Generated traceability ledger (not prose). Owning layer: documentation/validation tooling, produced by `scripts/validate_implementation_coverage.py --write`.
- Header: `kind,contract_or_scenario,primary_story,coverage_note` → **7,393 data rows**.
- Rows are **sorted by `kind`**, so blocks are contiguous and countable by offset probing. Block sequence actually observed: `API_OPERATION` → `ARCH_DOC` → `BACKLOG_STORY` → `BENCHMARK` → `DOC_SECTION` → `ERROR_CODE` → `EVENT_PAYLOAD` → `EVENT_TYPE` → `FLOW` → `OPENAPI_SCHEMA` → `PLAN_SECTION` → `SHARED_SCHEMA` → `SQL_CHECK` → `SQL_COLUMN` → `SQL_FOREIGN_KEY` → `SQL_INDEX` → (`SQL_TABLE`, `SQL_TRIGGER`, `SQL_VIEW`) → `USER_WORKFLOW`.
- `BACKLOG_STORY` was not anticipated in the brief's vocabulary — it is a real kind (line 342 = `E01-S01`).
- Kind distribution, reconciled to `ARCHITECTURE-COVERAGE.md` totals and confirmed at block boundaries (probe-confirmed boundary pairs in bold):

| kind | rows | first row | evidence |
|---|---|---|---|
| API_OPERATION | 229 | 2 | **230 = API_OPERATION, 231 = ARCH_DOC** |
| ARCH_DOC | 111 | 231 | **341 = ARCH_DOC, 342 = BACKLOG_STORY** |
| BACKLOG_STORY | 68 | 342 | matches backlog.json story count |
| BENCHMARK | 91 | 410 | 432/433 in BENCHMARK block; doc total 91 |
| DOC_SECTION | 1,893 | 501 | 1000/1500/2000/2100/2325 all DOC_SECTION |
| ERROR_CODE | 131 | 2,394 | **2456 = ERROR_CODE** |
| EVENT_PAYLOAD | 147 | 2,525 | **2603 = EVENT_PAYLOAD** |
| EVENT_TYPE | 148 | 2,672 | **2751 = EVENT_TYPE** |
| FLOW | 144 | 2,820 | **2895 = FLOW** |
| OPENAPI_SCHEMA | 405 | 2,964 | 3300 = OPENAPI_SCHEMA |
| PLAN_SECTION | 1,158 | 3,369 | 3800/4000/4458 = PLAN_SECTION |
| SHARED_SCHEMA | 271 | 4,527 | **4729 = SHARED_SCHEMA** |
| SQL_CHECK | 348 | 4,798 | 5000/5200 = SQL_CHECK |
| SQL_COLUMN | 1,172 | 5,146 | 5500/6000 = SQL_COLUMN |
| SQL_FOREIGN_KEY | 475 (claim) | 6,318 | 6600/6900 = SQL_FOREIGN_KEY |
| SQL_INDEX | 76 (claim) | ~6,793 | **7000 = SQL_INDEX** |
| SQL_TABLE | 107 (claim) | ~6,869 | not probed |
| SQL_TRIGGER | 140 (claim) | ~7,234 | **7300 = SQL_TRIGGER** |
| SQL_VIEW | 1 (claim) | ~7,374 | not probed |
| USER_WORKFLOW | ~20 | — | 7393/7394 = USER_WORKFLOW (U19, U20) |

- **UNRESOLVED arithmetic discrepancy (important):** summing the report's counts for kinds through `SQL_COLUMN` consumes rows 2–6,317 exactly (6,316 rows), which is self-consistent with every probe. That leaves 1,077 rows for 6,318–7,394, but the report's tail counts (475+76+107+140+1+68 unique constraints+~20 workflows) total only ~887. **~190 rows are unaccounted for**; either `SQL_FOREIGN_KEY` rows exceed the "475 foreign-key columns" claim (the CSV row is a *constraint* `table#FK1.0 col->table.col`, not a column), or an unprobed kind (plausibly `SQL_CONSTRAINT`/`SQL_UNIQUE_CONSTRAINT`) sits between `SQL_COLUMN` and `SQL_FOREIGN_KEY`. Not resolvable before deadline.
- `SQL_INDEX` at row 7000 reference: `SQL_INDEX,idx_conversation_sessions,E03-S01,index on agent_sessions; DDL sha256:391174f3cbb79be8408b73c1f9e213c71100bdb284f46fd9e5d7e69b888968bc.`
- Digest-pinning is the mechanical content: `operation sha256:`, `schema sha256:`, `section sha256:`, `DDL sha256:`, `expression sha256:`, `row sha256:`, `file sha256:`. This is a **drift detector**, never behavioral evidence.

### docs/archive/legacy-implementation-c7e9c05/ARCHITECTURE-COVERAGE.md (full read)
- The generator's human-readable report and the source of the authoritative totals: 111 source Markdown docs / 1,893 heading sections; 108 implementation-plan docs / 1,158 plan sections; **68 backlog stories**; 144 flows; 91 benchmarks; 229 OpenAPI operations; 405 component schemas; **0 components unreachable** from an Operator operation; 131 error codes; 271 shared IDs/value definitions; 148 event types; 147 typed payload schemas; 107 SQLite tables, 1,172 columns, 475 foreign-key columns, 68 unique constraints, 348 checks, 1 view, 140 triggers, 76 indexes.
- Also a full owner table: 88 architecture docs + 25 ADRs (`docs/adr/0001`…`0025`) each mapped to a planning guardian story (`E01-S01`…`E13-S03`).
- Self-disclaiming, consistent with posture: "This coverage index does not establish implementation or qualification… A digest is not a semantic test… Mock results cannot close those gates."
- Lists deferred scope: Team/RBAC, native mobile/web, voice, marketplace/template sharing, messaging gateways.

### docs/archive/legacy-implementation-c7e9c05/COVERAGE.md
- Short index. Names the checker: `scripts/validate_implementation_coverage.py`, `scripts/validate_implementation_plan.py`, and `--write`.
- Connects the ledger to its own limits: "These checks prove traceability and contract inventory consistency. They do not prove that features work."
- Notes a post-snapshot drift: `docs/REFERENCE-IMPLEMENTATIONS.md`, `docs/CONNECTORS-WORKFLOWS.md`, `docs/LEARNING.md` were added **after** the 2026-10-10 snapshot and are not represented as executable objects.

### docs/archive/legacy-implementation-c7e9c05/backlog.json (2,235 lines; status field fully enumerated)
- Keys: `schema_version: 1`, `baseline: 8af59a5b9be0a00b12849caf40eac287527ac43e`, `stories[]`.
- Per-story fields (exact): `id`, `epic`, `title`, `gate`, `status`, `depends_on`, `authorities`, `implementation`, `acceptance`, `negative_cases`, `demo`, `tests`, `sources`.
- **68 stories. Status distribution (complete, every `"status":` line enumerated): `IN_PROGRESS` 40, `PLANNED` 23, `IN_REVIEW` 5, DONE/VERIFIED 0.**
- Story IDs run `E01-S01` … `E13-S03`; gates `G0`+. Test IDs are triples per story: `<STORY>-CODE`, `-SYSTEM`, `-USER` (e.g. `E01-S01-CODE`).
- Representative rows (verbatim):
  - `"id": "E01-S01"` / `"title": "Contract/toolchain baseline"` / `"gate": "G0"` / `"status": "IN_REVIEW"` — acceptance: "A clean checkout runs scripts/check.sh with exact pinned versions… **No operational Runtime behavior is claimed.**"
  - `"status": "PLANNED"` at lines 366, 399, 467, 503, 539, 576, 615, 746, 808, 972, 1357, 1483, 1517, 1738, 1769, 1800, 1831, 1862, 1893, 1922, 2075, 2188.
  - `"status": "IN_REVIEW"` only at 10, 1951, 1982, 2013, 2044.

### docs/archive/legacy-implementation-c7e9c05/served-route-inventory.json (1,303 lines; header read)
- Keys: `version: 1`, `served[]`; per-record fields `method`, `path`, `openapi_path`, `operation_id`.
- Concrete identifiers: `GET /v1/agent-bindings` → `listAgentBindings`; `GET /v1/agent-bindings/{agent_binding_id}` → `getAgentBinding` (openapi `{agentBindingId}` — naming normalization `snake_case`→`camelCase`); `GET /v1/agent-bindings/{agent_binding_id}/quota-observation` → `getAgentQuotaObservation`; `GET /v1/agent-installations` → `listLocalAgentInstallations`; `GET /v1/agent-profiles`.
- **SERVED vs TARGET split NOT determined** — I never reached a `target[]` key or a per-entry status flag before the deadline. Record shape as read has no status field; if TARGET lives in a sibling key it is unread. Do not treat ~217 (1,303 lines ÷ 6) as a count.

## Cross-cutting observations
- The archive's own framing already matches the current docs-first posture: inventory/digest checks are explicitly declared non-evidence, and `E01-S01` promises no Runtime behavior. The contradiction is **latent, in the numbers**: a 229-operation OpenAPI surface, 148 event types and 107 SQLite tables are enumerated as "current canonical machine contracts", which reads as a shipped system unless the reader keeps the disclaimer in view.
- Every row's `primary_story` is a *planning guardian*, not an implementer — `DOC_SECTION` rows say "primary implementation guardian only". So coverage.csv cannot answer "who implemented this" and must not be cited as implementation coverage.
- `coverage_note` vocabulary is boilerplate by kind (`operation sha256:`, `section sha256:`, `type=TEXT; not_null=true; default=None; pk_order=0`, `on_update=NO ACTION; on_delete=NO ACTION; match=NONE`, `guard on workspaces`). Useful as a schema fingerprint, not as semantics.
- `PLAN_SECTION` rows point at `implementation/...` paths (e.g. `implementation/epics/E13.md`, `implementation/reviews/E07-S05.md`, `implementation/NEXT-SESSION-HANDOFF-2026-10-13.md`) that are not part of this docs checkout — stale pointers after the reset.
- Schema inventory counts SQLite **columns, FKs, checks, triggers, indexes** but the OpenAPI side is pinned only at whole-component granularity ("Not every machine-readable field is an independent CSV row"), so field-level contract coverage is asymmetric.

## Coverage check
- Read fully: `ARCHITECTURE-COVERAGE.md`, `COVERAGE.md`, `reading/plan/archive-01.txt`.
- Read partially: `coverage.csv` (~40 of 7,394 lines via offset probes; distribution derived from block-contiguity + report totals, tail block unverified), `backlog.json` (all `"status"` lines; not all prose), `served-route-inventory.json` (first 5 records only).
- **Not read at all** (in brief, absent from plan): `machine-inventory.json`, `audit-inventory.csv`, `V1-MARKDOWN-INVENTORY.csv`, `V1-EVERY-MD-AUDIT-2026-10-12.csv`, `V1-CROSS-CONTRACT-GUARD.json`, `V1-DEEP-SEMANTIC-SUMMARY.json`, `spikes/*` (SP02–SP05.md, `agent_switching_poc/`, `sp02-driver-compare/`).
- Blocked by: no shell/Python tool in this session (`tool_search` for code execution returned 0 tools), so CSV aggregation was done by bisecting sorted blocks; run hit its deadline during tail bisection.
- Suggested next step: give this bucket the JSON/CSV files explicitly in the plan, and provide a Python execution path — per-kind counts, SERVED-vs-TARGET split, and the ~190-row tail gap are all one `pandas.value_counts()` away.
