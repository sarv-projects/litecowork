# Current architecture audit and development refinements

## Evidence and limits

Baseline is the current tracked repository, not the pasted historic hashes. This pass
inventories all tracked root documentation, all `docs/` authorities/ADRs/machine contracts,
and validation scripts. Markdown headings and relevant contract bodies were cross-checked;
large JSON/OpenAPI/SQL contracts are structurally enumerated and validated. This is not a
claim that every line was manually understood or that a green validator proves every
semantic property. The inventory records exact baseline digests so later reviewers can
identify what changed. Application behavior cannot be assessed because no app exists yet.

[Inventory](audit-inventory.csv), [machine objects](machine-inventory.json), and
[coverage](coverage.csv) are the reproducible review map. The code intelligence index is
local/ignored and syntax/regex based here; it does not provide compiler semantic evidence.
Every story requires a deeper reread of its owning contracts and exact machine definitions.

## Findings and decisions

| Finding | Development refinement | Work |
|---|---|---|
| Durable core is coherent, but documentation size is much larger than current implementation | Preserve identities/transaction/fencing contracts; build one observable vertical slice at a time | E01–E03 |
| Existing early stage contains too many infrastructure deliverables | Keep acceptance scope but split into reviewable stories; ship desktop alpha before cloud | ROADMAP and E01–E10 |
| ACP-first text could imply harness features must fit ACP | Qualify native supported interfaces first, use ACP only when faithful | SP04 / E03 |
| Semantic RAG deliberately outside Core, but file/folder/ZIP experience needs delivery details | Integrate Resource-backed parser/retrieval provider with exact revision/citation/delete contracts | E06 / RAG |
| Cloud/remote appear before local responsibility/office feature completeness in old sequence | Reorder delivery to complete desktop workflows, then cloud, then remote | E11–E12 |
| Existing benchmark mobile scenarios conflict with desktop-only release client scope | Execute same domain/channel continuity with desktop/second desktop and source channel; native-mobile rendering deferred explicitly | coverage variants in TESTING |
| LiteSPM actual wire authority still unavailable | Contract fixture tests plus real integration dependency; never claim production package conformance from fixtures | E04-S04 |
| Motion/status rules are mature | Implement from committed projection states and validate actual-event sequencing | E02 / E08 |
| Broad provider promises risk being untestable | Publish supported-feature matrices by exact adapter/OS/provider version | E03/E07/E13 |
| Green architecture CI is specification evidence only | Add plan integrity checks plus code, system, real-use evidence gates | E01/E13 |
| Competitor documentation changes rapidly | Separate retrieved vendor evidence, vendor examples, inherited links and our proposed parity | SOURCES / WORKFLOWS |

The prior competitive snapshot incorrectly described 2026-10-06 as two days after
2026-10-05. Correct the arithmetic and treat any rollout statement as date-sensitive,
not proof of account availability. External product behavior is not a normative dependency.

## Coverage discipline

All current F00–F78 and B01–B74 headings are assigned to stories. Coverage assignment
selects a primary owner by the concern in the actual title; an implementer must expand
cross-domain dependencies and execute every assigned scenario, not just its headline.
All normative docs and machine files have primary owners. A primary story owning API or
SQL contracts is a cross-cutting guardian; each domain story still implements its own
operations/constraints/event payloads. The machine inventory is the detailed checklist
for that expansion. New operations/events/flows/benchmarks must update coverage.

## Remaining evidence-dependent choices

Exact dependency versions, first adapter, SQLite driver, retrieval index, OCR engine,
local model/hardware requirements, OS support, cloud vendor, signing credentials and
numeric SLOs must be established through the listed spikes. Recommendations are concrete,
but those choices are not represented as already benchmarked architecture facts.
No new autonomous planner, memory database inside Core, or goal-execution authority is added.
