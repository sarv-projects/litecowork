# Feature 004 current package and source cross-check — 2026-11-13

## Scope and evidence boundary

All 11 files in `specs/004-agent-relay/` and its `contracts/` directory were read line by line and checked against the current Conversation implementation, Feature 001 dispatch/context ownership, Feature 007 Artifact read ownership, canonical context schemas and desktop UI path. This report pins exact package bytes and the bounded code ranges reviewed. It does not claim a line-by-line read of every line in the larger Rust, Tauri, React or schema files.

Implementation source checkpoint: `6adadab539a62ca1c8b70621c7f88e1785cfe8c2`. The package has 945 physical lines across 11 files. SHA-256 hashes below cover exact file bytes. Feature 004 source behavior was not changed in this cross-check.

## Findings verified against the current checkout

1. **Binding selection is served, but relay is not.** `PATCH /v1/conversations/{conversation_id}/agent-binding` is composed and uses Workspace authorization, `If-Match` and an idempotency key; it changes the binding for future turns. The existing submit route remains Feature 001-owned. No relay-only endpoint was added.
2. **The current supervisor has one Conversation adapter.** `execute_agent_turn` normalizes provider identity through `NativeProviderKey` but has an OpenCode arm only. Claude Code has no Conversation dispatch arm (004-T05 NOT STARTED). Codex App Server context code remains outside this supervisor path. The picker filters by current dispatch-arm support, but does not gate on fresh descriptor/auth readiness; `deriveConversationAgentSurface` has no production caller.
3. **Producer identity is retained and displayed at CODE level.** The Operator message projection includes nullable LiteCowork `agent_binding_id` and `agent_session_id`; the desktop parser validates and preserves them, and committed AGENT messages render the label from the stored binding id with an unknown-agent fallback. This is not a qualified A→B→A journey or proof of a real second adapter.
4. **The context package is derived and producer-aware.** `build_conversation_context_package` uses the committed message prefix through the exact USER message revision, orders by event revision, excludes `SYSTEM_NOTICE`, and preserves the prior AGENT producer binding. The focused builder test confirms A→B→A producer attribution and does not serialize native session handles.
5. **Returned rows are capped, but total history work is not.** `load_conversation_context_for_turn` first checks all Conversation messages for orphan append events and computes COUNT/SUM aggregates over the committed prefix, then fetches at most 1,024 newest rows. Because the full-prefix scans precede that LIMIT, current code has not demonstrated AC-011's bounded-work requirement. There is no >1,024-message loader test proving the complete query path.
6. **Resource support is partial and has an existing owner.** Feature 001 T14 reauthorizes attached ResourceRefs and materializes eligible text before prompt encoding. Binary and WorkspaceRoot content are not materialized by that path. Feature 004 must reuse the Resource owner boundary rather than create a duplicate materializer.
7. **Two accepted B-15 input kinds are missing from the current Conversation package.** The pinned Workspace instruction preamble described by B-15 and `docs/CONTEXT.md` is not assembled. Canonical `ArtifactVersionRef` exists, and Feature 007 already exposes exact-version metadata/content reads, but the Conversation relay has no typed ArtifactVersionRef input or scoped exact-version materializer calling that owner. Feature 004's illustrative `ArtifactRef` spelling in SPEC §7 must be reconciled to the canonical type without creating a parallel type.
8. **The B-07 window value is not a B-15 relay receipt.** `window_conversation_context` returns an in-memory `ContextReduction`; the supervisor binds it as `_context_reduction` and discards the value. The prompt encoder can add a model-visible omission notice, but the reduction is neither persisted nor displayed as a user-facing receipt in the desktop UI. B-15's refusal with an audited receipt remains unimplemented. A-66 leaves the receipt's transaction/storage placement open.
9. **There is a WIRING/DATA-MODEL ambiguity.** WIRING lists `portable_context_package` as an `atomic_record` in `tx-export`, while DATA-MODEL says the package is derived and not separately persisted; SPEC also requires a durable receipt before dispatch. A-96 records the owner question. No table, migration or receipt location is inferred.
10. **No integration or qualification evidence exists for relay.** The focused Rust builder test passed 1/1 and the focused Conversation snapshot UI suite passed 24/24. Neither exercises the real SQLite-to-desktop/provider path. No second-provider run, packaged desktop journey, recovery/denial USER evidence or OS qualification was performed.

## Open owner items

- **A-95 — accepted portable input wiring:** trace Workspace instruction revision, selected ResourceRefs, canonical exact-version ArtifactVersionRefs and their authorization/materialization, receipts, UI state and recovery. Feature 007 exact-version read support exists; the missing work is its authorized Conversation dispatch integration and naming alignment, not a new Artifact API.
- **A-96 — export transaction record:** reconcile WIRING's `atomic_records` label with the derived-package rule and identify the owner/identity/transaction of the durable refusal/truncation receipt. Keep A-66 open for the exact receipt placement decision.
- **AC-011:** remove or finitely bound the orphan/count/size full-prefix scans, or define an owner-approved bound, and add a large-history query-count test before calling portable export bounded.

These remain documentation/owner questions. The accepted B-15/B-16 product decisions have not been narrowed or rewritten.

## Current task state

| Task | Status | Current gap |
|---|---|---|
| 004-T01 | NOT STARTED | Promote bounds/errors/receipt after A-66/A-96 and cross-owner input decisions; no schema promotion yet |
| 004-T02 | PARTIAL | Parser/UI evidence exists; storage pass-through and cross-layer A→B→A evidence remain |
| 004-T03 | PARTIAL | Builder/text refs exist; full-prefix scans, missing instruction/artifact inputs, adapter budget and B-15 receipt remain |
| 004-T04 | PARTIAL | Future-turn binding switch and some admission checks exist; single-flight/fresh-descriptor admission gate remains |
| 004-T05 | NOT STARTED | Claude Code Conversation adapter absent |
| 004-T06 | PARTIAL | Current adapter filtering exists; fresh descriptor/auth/offer projection gate absent |
| 004-T07 | PARTIAL | Stored producer label exists; complete picker states and rendered acceptance journey remain |
| 004-T08 | NOT STARTED | Cross-provider failure/restart recovery tests absent |
| 004-T09 | NOT STARTED | Full UI→Tauri→Operator→owner→transaction→event→projection→restart trace absent |
| 004-T10 | NOT STARTED | No packaged Windows/Linux E2E-02 qualification |
| 004-T11 | NOT STARTED | Full ledger/docs verification not complete |
| 004-T12 | NOT STARTED | Package is not VERIFIED |

## Feature package line/hash manifest

| File | Lines | SHA-256 |
|---|---:|---|
| `specs/004-agent-relay/README.md` | 34 | `878de269b4d922a649564858954ce79f8ace0b30515861b288a8df310282cc13` |
| `specs/004-agent-relay/SPEC.md` | 329 | `6bd00b11bd19ec8092f4d1d96119ab6e8ab9a43876316e07bd24dba31b758569` |
| `specs/004-agent-relay/PLAN.md` | 44 | `188a98e46525d273e27dfc5c73cb1aef1da7baebd0d18cac81d8365b9ec6fc33` |
| `specs/004-agent-relay/TASKS.md` | 239 | `259cf87b5e771d6cd9ef0759f3ef9e8ba63a5b90a905d430a2fb8f8436a1ee0f` |
| `specs/004-agent-relay/DATA-MODEL.md` | 50 | `c9f509ba82969a6265aa52e2165e8f7db5beef11832a6b9f159473c8714c5b6e` |
| `specs/004-agent-relay/RESEARCH.md` | 80 | `955f4fdeccea523c3a105b983f2e72b8c0e5d888d335b60202b653f1c863605f` |
| `specs/004-agent-relay/WIRING.yaml` | 72 | `c0bed7352df29e709946a343da87c6976a819835faa00d1837e35819ccaa83fc` |
| `specs/004-agent-relay/contracts/README.md` | 9 | `dcd3f2509e555ea04e9df04adbe7785751be4a25061fd6f5ea3b26bb14794e31` |
| `specs/004-agent-relay/contracts/api-events.md` | 27 | `ec01acbfe47c71c6f94037acfe1d9c46668cd64aa44c0ee17ad3596a11156152` |
| `specs/004-agent-relay/contracts/errors-recovery.md` | 24 | `b3f6ceffed29339c3cce397544a86cd2593014d23721809ce0a225fadc426ef6` |
| `specs/004-agent-relay/contracts/ui-behavior.md` | 37 | `e1db762b22f8b05d885cfbca92c1775d9c7f7f4e01da72ef9392976da8271606` |

## Bounded source and owner-contract ledger

Hashes pin exact current bytes for the listed files; only the stated ranges were read for this feature cross-check.

| Source | Checked ranges | SHA-256 |
|---|---|---|
| `crates/storage-core/src/conversation.rs` | 600–748; 811–981 (A→B→A test) | `f2e48abece409f3d7de282c92f1a1130679c2e2b1e54cacf2ee7337c89c124d0` |
| `crates/storage-sqlite/src/conversations.rs` | 1445–1645 (orphan/count scans and capped row fetch) | `871fc0283cbaedc5b86e672d52edee419159b33d752c60e158d6bfd3c4df25c4` |
| `apps/litecoworkd/src/agents/supervisor.rs` | 800–830; 971–1007 | `fcde4daf3be04a5904e3975450ad42c2b073e88a6b26dcbbd4a80fcc0a68bca0` |
| `apps/litecoworkd/src/agents/conversation_prompt.rs` | 60–210 | `9803004b9cc4953aaf4fd16a702cbc7c8ab1041bae8ee371be06be9e4cb4c667` |
| `apps/litecoworkd/src/agents/resource_materializer.rs` | 154–235 | `f867c631de1a6d2ad60d36e53046bb006691920a472a646a47a7b584fadc65f1` |
| `apps/litecowork-ui/src/conversations/conversation-agent-selection.ts` | 1–70 | `a019773b16629aa026cdbe36bc188a25ba0ca4d48f2c164dd0e2910c7c6a476d` |
| `apps/litecowork-ui/src/conversations/conversation-agent-surface.ts` | 1–164; repository call-site search | `8f2f98e307621a4c7ecc99737de13b6618ab672225e4cf1e7d2959ab843e724d` |
| `apps/litecowork-ui/src/conversations/conversation-view.ts` | 330–380 | `f4709449f392fca11a47bb5a50de1b0571a874862150cfb7d3102948c93fc8bf` |
| `apps/litecowork-ui/src/conversations/ConversationsPage.tsx` | 48–58; 610–644; 1208–1220 | `e6f40a87562efbddd0a0bcccb9df02d2fa7fe9ba52789c72824d357966f9dea4` |
| `apps/litecoworkd/src/conversation_operator.rs` | 131–166; 283–360 | `f97b7ff3f8e61b53b09d30dad61e7a64cd35e1106d88a668eecef35f17e614de` |
| `docs/schemas/operator-api.openapi.yaml` | 6896–6910 | `3a614df67fc65e75e4e4ae9dbf9de898521a845d4bfc38b0a2bb88632329342f` |
| `apps/litecoworkd/src/artifact_operator.rs` | 131–143; 700–732 | `927dd5c48c3b3129d36b0921a3639393ef7df6626ba2e5b49555bb250db3b960` |
| `crates/storage-sqlite/src/artifacts.rs` | 353–400; 1885–1904 | `b874a94b46e4b9c945f067591abbe9b830521e9dc43f455d1d506fed6525cf1b` |
| `docs/CONTEXT.md` | 35–67 | `deae265133353676b56fbe6fe25ff4bac7e1028437f4e1fe2d6d951ec73dc86c` |
| `docs/SCHEMAS.md` | 580–598; 865–930 | `d7c2f5545cf6c578efa0d0bf389daff978212ae0593b4d558a6d0c78dab44c98` |
| `specs/001-conversation-session-supervisor/TASKS.md` | 101–104 | `45b63258496595f0c31cf0446d4351a3f6f5e2654f9c0beedcafd7a0d6415512` |
| `specs/007-professional-artifacts/contracts/api-events.md` | 7–18 | `43d8d4df3de015f5d666db614ce9d4443af873021d5e09181ae6a8903b277f9f` |

## Validation record

- `cargo test -p storage-core --locked relay_context_preserves_each_producer_across_a_to_b_to_a` — PASS, 1 passed, 115 filtered.
- `node --experimental-transform-types --test --test-concurrency=1 tests/conversation-view.test.ts` from `apps/litecowork-ui` — PASS, 24 passed.
- No Feature 004 integration test, provider session, packaged desktop journey, restart/denial E2E, or OS qualification was run.

## Remaining sequence

1. Conversation/Context/Artifact/Storage owners resolve A-95/A-96 and A-66; confirm canonical instruction and ArtifactVersionRef inputs, receipt identity/storage and migration need.
2. Feature 001 confirms actual adapter budget and the owner-approved receipt ordering; 004-T01 promotes only the resolved canonical contracts.
3. Replace all full-prefix query work with bounded/indexed access; test a large Conversation and the exact event-revision boundary; implement the target refusal receipt.
4. Integrate the exact-version ArtifactVersionRef read using Feature 007's existing authorization/store path; add pinned Workspace instruction revision; no duplicate Artifact or Resource schema.
5. Complete and qualify Claude Code under Feature 002, add its supervisor adapter, gate picker/admission on fresh dispatchability, and prove denial, replay, timeout, cancellation, restart and recovery in packaged E2E-02 on each declared OS row.
6. Complete the UI/API/wiring trace and record owner USER evidence before 004-T12 VERIFIED.
