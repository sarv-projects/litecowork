# V1 served capability inventory — initial scope

Initial review: 2026-10-13. Route-specific evidence corrections checked against pushed
`5fb33ea` on 2026-10-14. This remains a scoped inventory of audited surfaces, **not a
count of all served routes or implemented features**. An OpenAPI
operation is a target contract. A mounted handler is evidence only that a request can
reach that handler; it does not prove native provider integration or release qualification.
The OpenAPI server base is `/v1`; table paths show the resolved daemon path
([spec](../docs/schemas/operator-api.openapi.yaml#L1)).

## Router and transport boundary

[`build_operator_router`](../apps/litecoworkd/src/operator.rs#L1349) composes **18
merged route factories** plus inline registrations. One additional native folder-selection
route is explicitly private to authenticated local IPC and excluded from OpenAPI
([`operator.rs`](../apps/litecoworkd/src/operator.rs#L1384)). OpenAPI and structural
coverage counts do not enumerate this Axum composition. The structural contract report
counts OpenAPI operations from YAML and checks catalog entries against that spec; it does
not parse the daemon router ([`audit_v1_contracts.py`](../scripts/audit_v1_contracts.py#L20)).

[`OperatorServer::start`](../apps/litecoworkd/src/operator.rs#L1109) derives and binds
the local endpoint, then serves this router through authenticated IPC; the desktop
Runtime does not mount an HTTP listener ([`operator.rs`](../apps/litecoworkd/src/operator.rs#L1384)).
Unix source checks peer credentials and the expected UID
([`operator.rs`](../apps/litecoworkd/src/operator.rs#L346), [`accept` peer check](../apps/litecoworkd/src/operator.rs#L405), [`unix.rs`](../crates/operator-ipc/src/unix.rs#L647)). Windows source requires a bound
logon-session SID and uses the Windows named-pipe endpoint
([`operator.rs`](../apps/litecoworkd/src/operator.rs#L367),
[`windows.rs`](../crates/operator-ipc/src/windows.rs#L1)). Unix lifecycle-test source exists
([`unix_endpoint_lifecycle.rs`](../crates/operator-ipc/tests/unix_endpoint_lifecycle.rs#L60));
these source paths and test files do not establish OS release qualification: supported-host SYSTEM and
owner USER evidence remain **PENDING**.

Evidence labels below are separate: **OpenAPI** is target presence; **mounted** names the
current composed route/handler; **provider** describes the implemented boundary;
**transport/OS** is not inferred from a unit route test; **CODE**, **SYSTEM**, and
**OWNER USER** record distinct evidence states. `PENDING` means this inventory did not
establish the required evidence.

## Representative audited operations

| Operation | OpenAPI status | Mounted handler in daemon | Provider integration | Transport / OS evidence | CODE evidence | SYSTEM evidence | OWNER USER |
|---|---|---|---|---|---|---|---|
| `GET /v1/conversations` | Target: `listConversations` ([OpenAPI](../docs/schemas/operator-api.openapi.yaml#L333)) | **Mounted:** `conversation_operator::routes` → `list_conversations` ([source](../apps/litecoworkd/src/conversation_operator.rs#L42)) | SQLite-backed Conversation catalog read only. No native turn provider is implied. | Local authenticated IPC source; host qualification **PENDING**. | Route-specific CODE evidence **PENDING** in this review. | **PENDING** | **PENDING** |
| `POST /v1/conversations` | Target: `createConversation` ([OpenAPI](../docs/schemas/operator-api.openapi.yaml#L333)) | **Mounted:** `conversation_operator::routes` → `create_conversation` ([source](../apps/litecoworkd/src/conversation_operator.rs#L42)) | Durable SQLite Conversation record/event/receipt path; no AgentSession or turn execution provider. The module documents turn dispatch as absent ([source](../apps/litecoworkd/src/conversation_operator.rs#L1)). | Local authenticated IPC source; host qualification **PENDING**. | CODE: `conversation_create_replays_receipt_after_archive_and_rejects_fresh_create` uses real SQLite and an isolated authenticated route ([test](../apps/litecoworkd/src/operator_ipc_tests.rs#L892)); full composed router, packaged IPC, and native turn execution are not established by this test. | **PENDING** | **PENDING** |
| `GET /v1/conversations/{conversation_id}/presentation` | Target: `getConversationPresentation` ([OpenAPI](../docs/schemas/operator-api.openapi.yaml#L375)) | **Mounted:** `presentation_operator::routes` → `get_conversation_presentation` ([source](../apps/litecoworkd/src/presentation_operator.rs#L29)) | Bounded persisted SQLite presentation snapshot; not a live agent or turn-generation provider. | Local authenticated IPC source; host qualification **PENDING**. | Read tests exist, including `owner_reads_semantic_conversation_snapshot_with_optional_presentation_ref` ([tests](../apps/litecoworkd/src/presentation_operator.rs#L3771)); full desktop/provider flow **PENDING**. | **PENDING** | **PENDING** |
| `POST /v1/tasks` | Target: `createTask` ([OpenAPI](../docs/schemas/operator-api.openapi.yaml#L532)) | **Mounted:** `build_operator_router` → `create_task` ([source](../apps/litecoworkd/src/operator.rs#L1408), [handler](../apps/litecoworkd/src/operator.rs#L3520)) | Durable Task creation and receipt through Core/SQLite. It creates a `READY` envelope; it does not dispatch planning or execution ([handler](../apps/litecoworkd/src/operator.rs#L3517)). | Local authenticated IPC source; host qualification **PENDING**. | CODE: a Task route subcase within `instruction_create_route_commits_first_revision_and_replays_after_advance` creates with a 200-byte key, clears the default AgentBinding, retries the exact request, denies a foreign owner, and asserts one Task row/event ([test](../apps/litecoworkd/src/operator_ipc_tests.rs#L1888)). It uses real SQLite and an isolated route, not full-router composition, packaged IPC, or agent execution. | **PENDING** | **PENDING** |
| `GET /v1/tasks/{task_id}/plan-revisions` | Target: `listPlanRevisions` ([OpenAPI](../docs/schemas/operator-api.openapi.yaml#L748)) | **Mounted:** `build_operator_router` → `list_task_plan_revisions` ([source](../apps/litecoworkd/src/operator.rs#L1418), [handler](../apps/litecoworkd/src/operator.rs#L4351)) | SQLite plan-history read only. | Local authenticated IPC source; host qualification **PENDING**. | Route-specific CODE evidence **PENDING** in this review. | **PENDING** | **PENDING** |
| `POST /v1/tasks/{task_id}/plan-revisions` | Target: `submitTaskPlan` ([OpenAPI](../docs/schemas/operator-api.openapi.yaml#L761)) | **TARGET ONLY:** no POST registration in the composed router; only GET is mounted ([source](../apps/litecoworkd/src/operator.rs#L1418)). | `TaskService::submit_initial_plan` and atomic SQLite acceptance code exist ([service](../crates/domain-task/src/lib.rs#L570), [storage](../crates/storage-sqlite/src/lib.rs#L2697)), but no Operator producer-authenticated route or agent invocation is integrated. Native planning remains fail-closed ([source](../apps/litecoworkd/src/agents/planning.rs#L60)). | No served POST to qualify; planner/OS qualification **PENDING**. | Domain/storage code is not route evidence; integrated submit CODE **PENDING**. The existing dispatch-gate test checks blockers, not plan submission ([test](../apps/litecoworkd/src/agents/planning.rs#L108)). | **PENDING** | **PENDING** |
| `GET /v1/needs-you` | Target: `listNeedsYou` ([OpenAPI](../docs/schemas/operator-api.openapi.yaml#L3391)) | **Mounted:** `needs_you_operator::routes` → `list_needs_you`, merged into the composed Operator router ([composition](../apps/litecoworkd/src/operator.rs#L1428), [handler](../apps/litecoworkd/src/needs_you_operator.rs#L29)). | Read-only SQLite projection over persisted Approval/UserRequest records and current `BLOCKED`/`NEEDS_USER` Task blockers; linked blockers deduplicate to their source. No owning-resource action or notification provider is connected ([storage](../crates/storage-sqlite/src/needs_you.rs#L243), [UI](../apps/litecowork-ui/src/needs-you/TaskAttentionPage.tsx#L135)). | Authenticated local Unix IPC path is exercised with framed request/response CODE evidence; packaged desktop host qualification **PENDING**. | CODE: storage projection 7/7 and full crate 224/224; route 5/5 including authenticated Unix IPC; Tauri bridge 3/3; mounted Chrome page 2/2; full UI 226/226; API generation/build pass. Exact evidence in [E02-S04 review](reviews/E02-S04.md#read-only-source-projection-code-checkpoint--2026-10-14). | **PENDING** | **PENDING** |
| `GET /v1/tasks/{task_id}/effects`, `GET /v1/approvals`, `GET/POST /v1/workspaces/{workspace_id}/environments`, and `POST /v1/workspaces/{workspace_id}/environments/provision-preview` | Target operations exist in OpenAPI ([Effects](../docs/schemas/operator-api.openapi.yaml#L851), [Approvals](../docs/schemas/operator-api.openapi.yaml#L865), [Environments](../docs/schemas/operator-api.openapi.yaml#L3243), [provision preview](../docs/schemas/operator-api.openapi.yaml#L3271)). | **TARGET ONLY** in the current daemon composition: no matching Effect, Approval, or Environment handlers are registered in `build_operator_router` or its merged factories ([composition](../apps/litecoworkd/src/operator.rs#L1361)). | No integrated Trust admission, Environment provider, Invocation dispatch, or Effect reconciliation path. The owning contract says dispatch is unavailable until authoritative admission and provider reconciliation exist ([CAPABILITY-INVOCATIONS](../docs/CAPABILITY-INVOCATIONS.md#L85), [dispatch gate](../docs/CAPABILITY-INVOCATIONS.md#L130)). | No corresponding served operations to qualify; OS/provider qualification **PENDING**. | Target API/provider CODE **PENDING**; schemas and storage primitives do not establish integrated dispatch. | **PENDING** | **PENDING** |

## Evidence limits

The inventory records representative routes and audited absences only. It does not claim
an exhaustive route list, derive a served-route count, or equate handlers, schemas,
SQLite/domain methods, mocked fixtures, or transport source with integrated providers or
V1 acceptance. The OpenAPI machine-route count remains a target-contract count. Full
source-derived route enumeration and per-operation qualification evidence remain open. Test
links identify existing test definitions; this review did not execute them. Link and
whitespace checks on this document are not runtime evidence.
