# Feature 003 current source cross-check — 2026-11-13

## Scope and evidence boundary

All 11 current files in `specs/003-workspace-roots-context-ingestion/` were read line by line in this continuation and checked against their linked owner contracts, current route/schema inventory, and relevant Rust/Tauri/React paths below. The package contains 1,090 physical lines after this continuation's edits. The exact-byte SHA-256 manifest is included below.

The implementation source checkpoint is `6adadab539a62ca1c8b70621c7f88e1785cfe8c2`; the documentation baseline at the start of this cross-check is `d6c5819ca61a96f9fed0b39e1cbe10e511256358`. Source review is bounded to the listed code ranges and call sites; this is not a full reread of the larger Operator, Tauri, Conversation or UI source files. No Feature 003 product test, packaged desktop journey, Windows root operation, provider session or platform qualification was run for this report. Existing test results remain as dated in TASKS.md and are not re-claimed as freshly run.

## Findings verified against current source

1. **Root creation and lifecycle:** the public root lifecycle handlers are mounted in the daemon. Desktop Add uses a native folder picker and a private local-only selection route; the daemon opens and revalidates the selected directory, then commits the Resource/root/location/private-binding records through `WorkspaceRootService`. The implementation is cfg-limited to Linux/macOS; the Windows fallback returns 503. The UI Add control is still enabled until after the picker returns, so its platform behavior does not meet AC-015.
2. **Root scan is metadata census only:** the scan handler rejects every non-Linux target with `SCAN_UNSUPPORTED_PLATFORM`. Linux traversal is bounded to 1,000 entries, depth 32, a 1 MiB relative-locator budget, two concurrent scans and an 8-second total deadline (traversal ends at 7 seconds to reserve the identity recheck). It records membership and observations; it does not read file contents, create content revisions or populate the Resource text index. A successful scan therefore does not make a file attachable or retrievable.
3. **Retrieval is separate and read-only:** `POST /v1/resources/retrieval` performs authorized local lexical retrieval over indexed Resources and returns citations tied to exact revisions/digests or an explicit insufficient-evidence/provider-unavailable outcome. It does not persist a receipt or ContextAttachment, and no production Conversation Send call site invokes it. The retrieval decoder helper has no production UI call site. The route's focused unit tests are present in source, but were not rerun in this continuation.
4. **Current picker differs from the accepted interaction:** `LibraryPage.tsx` owns the Persistent folders section; `App.tsx` mounts Library but is not the root-control owner. In an existing Conversation, the **Add context or file** button loads only the first `list_resources` page, filters same-Workspace attachable text Resources (including the UI's 128 KiB cap), and sends revision-pinned `resourceRefs`. It does not open on `@`, support first-message attachments, provide bounded search/pagination or the accepted labelled listbox/keyboard/result-count behavior, or expose files merely observed by a root metadata scan. Feature 001's dispatch-time materializer rechecks and materializes eligible text.
5. **A-65 is a real transaction and replay-boundary conflict:** Feature 001 atomically commits the USER message, OPEN Turn, UNCLAIMED dispatch intent and idempotency receipt. Feature 003's target standalone attachment transaction requires a `turn_id` that does not exist until that atomic admission, while T10 places the turn-bound receipt before dispatch. A separate attachment transaction would break the current admission boundary. T10's acceptance that the same idempotency key never runs retrieval twice is also broader than the proposed read-only retrieval-before-admission sequence, which can repeat after a pre-commit crash or concurrent in-flight retry unless a durable gate is approved. The owners must define which retry cases the guarantee covers. The atomic-candidate sequence remains a proposal; do not claim exactly-once retrieval or weaken the outbox.
6. **A-69 is a separate unresolved shape conflict:** SPEC §7.2 requires `retrieval_receipt_id` on `CommitContextAttachment` and `root_id` on `ContextUsedEntry`; DATA-MODEL allows the former to be absent for explicit ResourceRefs and the latter for uploaded Resources. The register now clarifies one possible shape for owner review, without accepting it. The issue blocks T01 schema promotion as well as T09/T10 implementation. Do not allocate a DDL/API shape or write the allocated v35 migration before owner resolution.

## Current task and qualification posture

| Task | Current status confirmed in TASKS.md |
|---|---|
| T01 | BLOCKED on A-69 owner decision |
| T02 | PARTIAL; dated Linux lifecycle storage evidence only |
| T03 | Windows implementation DEFERRED by owner; not started |
| T04 | PARTIAL Linux path; Windows portion deferred and unexercised |
| T05–T06 | Windows implementation/denial work DEFERRED by owner; not started |
| T07 | NOT STARTED; no content ingestion from WorkspaceRoots |
| T08 | PARTIAL; actual Persistent folders UI owner is `LibraryPage.tsx` |
| T09–T10 | BLOCKED on A-65 and A-69 |
| T11 | PARTIAL; current first-page button picker only |
| T12–T18 | NOT STARTED, including Context Used, failure/recovery suite, complete operation traces, E2E-03 and package verification |

The package's accepted status is a target approval, not runtime evidence. Current source has no durable ContextAttachment/retrieval-receipt record or Context Used projection. Linux root lifecycle storage evidence does not qualify root ingestion, grounded Send, packaged recovery, or any Windows operation. The global documentation/source audit remains OPEN.

## Feature 003 package line/hash manifest

SHA-256 is over exact file bytes after this continuation's documentation corrections.

| File | Lines | SHA-256 |
|---|---:|---|
| `specs/003-workspace-roots-context-ingestion/README.md` | 36 | `521d0b1ac3b498d5feef88b173cd518da02a04548b63ee5b1e3e5e9d7a72d548` |
| `specs/003-workspace-roots-context-ingestion/SPEC.md` | 341 | `3b8c44c504f2ef14311f0a5a55b3e9ba055e9ab4dba76c885dcf72fa4482fa6f` |
| `specs/003-workspace-roots-context-ingestion/PLAN.md` | 43 | `0a66d755d261b4e1c9219a265513acd518504e6328b9d53999ab21748318c13e` |
| `specs/003-workspace-roots-context-ingestion/TASKS.md` | 378 | `4c29cc769ae2516bc28b703ddbfead7b3d059dc810d910b94f94d6819cd6bde2` |
| `specs/003-workspace-roots-context-ingestion/DATA-MODEL.md` | 36 | `d17eda11c45cf1e12161b20b12b30bfdd0ab0beac09e3ff0c38db078ec706ba1` |
| `specs/003-workspace-roots-context-ingestion/RESEARCH.md` | 66 | `0536afa48b99ab9c08013a8b5aa1d42b3d6766cae7ef74058425ffdd531552d8` |
| `specs/003-workspace-roots-context-ingestion/WIRING.yaml` | 130 | `9fbc333f7a11cfeb7153d33fef5ae9f1b84ddf9c6ab0941fdb9c1fc5a9d8099e` |
| `specs/003-workspace-roots-context-ingestion/contracts/README.md` | 9 | `5eec3c904cd05b4d9de68f96e0d1c224c90479c85016796d77ebfbef11a71271` |
| `specs/003-workspace-roots-context-ingestion/contracts/api-events.md` | 11 | `815c53e7fbeb742921fd29b1dc4bfd6272fc87769de719f5cf291ceddad7e677` |
| `specs/003-workspace-roots-context-ingestion/contracts/errors-recovery.md` | 9 | `23b50b52397b4c9aa2fc0464eb2034ab57a2e8a773a0e0a97d809cac529fd5b8` |
| `specs/003-workspace-roots-context-ingestion/contracts/ui-behavior.md` | 31 | `56f9ca5bf5f42d487fbd56d9c3ae38f80611d123986736d5813ed8c2bb6482a8` |

## Source cross-check ledger

The listed ranges were inspected for the findings above. Hashes pin the current source bytes; unlisted lines of these large files are not claimed as reread.

| Source | Checked lines | SHA-256 |
|---|---|---|
| `apps/litecoworkd/src/operator.rs` | 1600–1621, 1700–1725, 6526–6735 | `37402613e8dfd81fbcb0b9b045a44e6a100fadfc466736b07a17caa730d6e46d` |
| `apps/litecoworkd/src/workspace_root_scan_operator.rs` | 35–56, 391–470, 548–638, 904–955 | `6816e1810e20b80dfe915b025485d16b55607a25db4b19f3c18f5f44fad63062` |
| `apps/litecoworkd/src/rag_retrieval_operator.rs` | 1–37, 106–249, 638–793 | `1bf74c578d5586965c923d875aeb0fe8ff8c7c4cb0044ed1ad7ded6c47f0251b` |
| `apps/litecowork-ui/src-tauri/src/lib.rs` | 1760–1947, 5458–5468 | `b2126578c033986988fc0512b4c232e4702662985067e76f31fd56c45aa39080` |
| `apps/litecowork-ui/src/library/LibraryPage.tsx` | 110–145, 540–650 | `09dbc69ff92c7fcadaf8a0f836fcf2e0eb2ce2af081a150e15a2cfba2a9a026d` |
| `apps/litecowork-ui/src/conversations/ConversationsPage.tsx` | 801–860, 1167–1210 | `e6f40a87562efbddd0a0bcccb9df02d2fa7fe9ba52789c72824d357966f9dea4` |
| `apps/litecowork-ui/src/conversations/conversation-attachments.ts` | 1–61 | `d7c418286ad6fb60fc546bdd04a74308a6aab77f8274e437457d84f4dc53e1cd` |
| `apps/litecoworkd/src/conversation_operator.rs` | 1378–1428 | `f97b7ff3f8e61b53b09d30dad61e7a64cd35e1106d88a668eecef35f17e614de` |
| `apps/litecoworkd/src/agents/conversation_prompt.rs` | 145–202 | `9803004b9cc4953aaf4fd16a702cbc7c8ab1041bae8ee371be06be9e4cb4c667` |
| `apps/litecowork-ui/src/resources/retrieval-grounding.ts` | 1–279; repository search for production imports/call sites | `8e768ed1fe4b786aaa2b3feac286e6dbab9e9b7b4686192af011abed21c3a004` |
| `crates/storage-sqlite/src/lib.rs` | 220–260 | `5d3c1d07c1f365543168dab379fbf784121e3ba48c31a26a9c7bbd0d59fc6944` |
| `docs/LITECOWORK-BUILD-TASKS.md` | 215–230 | `d88e8fae80ed102040f86199f5d810618623ccfc256cc7958852b8b36e3ccc9a` |

## Remaining work and safe sequence

1. Obtain one owner-approved decision for A-69 and a cross-owner A-65 admission/replay contract, including same-key concurrent and pre-commit-crash behavior; reconcile Feature 001/003 API, data, event, wiring and recovery contracts together.
2. Only then promote the schema and decide whether the allocated migration v35 is still needed. Preserve the current Feature 001 atomic USER/Turn/outbox/idempotency boundary.
3. Implement root ingestion/indexing and platform availability through the Resource and WorkspaceRoot owners; keep Windows unavailable until native Windows traversal, identity, race and denial evidence passes.
4. Implement the accepted composer picker and first-message path; use exact revision-pinned ResourceRefs and reauthorize at dispatch.
5. Add durable context receipt/attachment projections and Context Used from committed records only; then add restart, denial, duplicate replay, cancellation and revoke-race tests.
6. Complete the cross-layer operation traces and packaged E2E-03 on each release row with exact OS/build/provider/auth evidence and owner USER review.

Do not treat this audit report or its structural validation as owner approval, implementation evidence, or release qualification.
