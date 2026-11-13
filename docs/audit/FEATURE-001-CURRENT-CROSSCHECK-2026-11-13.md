# Feature 001 current source cross-check — 2026-11-13

## Scope and evidence boundary

This supplement records the current Feature 001 source cross-check after the 2026-11-13 handoff. The implementation source checkpoint is `6adadab539a62ca1c8b70621c7f88e1785cfe8c2`; the documentation baseline at the start of this continuation was `d6c5819ca61a96f9fed0b39e1cbe10e511256358`. The Feature 001 package had a complete semantic read in the preceding audit; this continuation rechecked the changed package statements and traced them to the current source/schema lines below. The manifest pins the current package bytes after those corrections.

This is a bounded module review. It does not complete the repository-wide all-document/all-source audit and does not qualify a native provider journey. No Rust, Tauri, database, or React behavior changed in this supplement.

## Findings verified against source

1. **OpenCode offer readiness:** `opencode_offer_admission` can produce a compatible `STARTABLE` offer only for CLI 2.0.32, a complete probe, and an observed host stop. This supports a bindable offer in CODE; it proves neither authentication nor a working packaged Send route. The prior claims that the offer is always `compatible=false` were stale in current Feature 001/004/status prose. The 2026-11-09 historical baseline in Feature 001 TASKS remains labelled historical.
2. **Stop propagation:** Operator Stop calls `request_cancel`; the signal is consumed by the OpenCode `drive_turn` cancellation closure, which invokes `abort_session` and waits for quiescence. The distinct `AgentAdapter::interrupt` method is not the call path used by Stop. Completion-versus-interrupt ambiguity remains unresolved, so T07 stays PARTIAL.
3. **Send eligibility:** the UI checks a nonempty draft, Workspace presence, Operator readiness, a selectable same-Workspace enabled lead binding with an OpenCode dispatch arm, and no occupying turn. It does not check descriptor freshness, auth, native session readiness, exact OS/provider/auth tuple, or qualification. T19 is accurately described as the dispatch-eligibility guard; T09 still owns the disabled-until-qualified requirement.
4. **Response commit status mismatch (A-93):** storage accepts `COMPLETED`, `FAILED`, `CANCELLED`, `WAITING_USER`, and `WAITING_DEPENDENCY`; the transaction writes `settled_at` and closes the active AgentSession for all accepted statuses. The production supervisor caller passes only `COMPLETED`. Locked B-04 lists terminal outcomes, while AC-017/018 own the separate UserRequest continuation lifecycle. T02 is PARTIAL pending Conversation/Storage owner resolution; the unused waiting status support is not continuation evidence.
5. **TURN_DELTA correlation mismatch (A-94):** Feature 001 T13 and `contracts/api-events.md` require AgentSession identity in each transient delta, but `SPEC.md` §7.6, `docs/API.md`, `operator-stream.schema.json`, and `TurnFramePublisher` omit it. The UI currently fences by turn/retry/sequence. T13 is PARTIAL pending owner resolution; schema, producer, bridge, UI, and tests must move together if the session field remains required.
6. **Locked SPEC internal contradiction (A-92):** Feature 001 SPEC records B-02/B-03 as accepted, then §7.4 says no first harness contract is locked until those decisions. Its revision 1.1 note lists AC-020..026 while AC-027..031 and their folded P-task scope are also present. The locked SPEC was not silently rewritten; a reviewed owner revision is required.

## Cross-package corrections

The current status correction was propagated to Feature 001 TASKS/RESEARCH/WIRING, the Feature 001 current-evidence row, Feature 004's current provider-dispatch crosswalk and UI/API/error/research notes, `docs/IMPLEMENTATION-STATUS.md`, `docs/V1-AGENT-DISCOVERY-AND-CATALOG-LLD.md`, and the contradiction register. Historical snapshots and archives were left unchanged. Feature 004's full current package/source cross-check is recorded separately in `FEATURE-004-CURRENT-CROSSCHECK-2026-11-13.md`.

The OpenCode-compatible-offer fix does not make Conversation qualified. Authentication, packaged bind/send, stop/retry, restart/recovery, denial behavior, and owner USER evidence remain open. A second qualified dispatch path is still absent, so A→B→A relay remains unavailable.

## Feature 001 package line/hash manifest

All 11 current Feature 001 package files, 1,814 physical lines total. SHA-256 is over exact file bytes. The full semantic read is recorded in the preceding module audit; this continuation reread modified claims against the source ranges below.

| File | Lines | SHA-256 |
|---|---:|---|
| `specs/001-conversation-session-supervisor/README.md` | 31 | `18ba16d73cd8eb7a717bff5c663d337eddfaba5e16f2efc1b771fd5e9f25ef0b` |
| `specs/001-conversation-session-supervisor/SPEC.md` | 378 | `4bd86dd44f8547dd8dcd2e765741b56945b81157b18b3310b5055f837f10dfac` |
| `specs/001-conversation-session-supervisor/PLAN.md` | 117 | `e3a4d0fb3027cacce75286207a03e149f34532a694b7b50699eeae524f9c01b5` |
| `specs/001-conversation-session-supervisor/TASKS.md` | 561 | `b40b755e0d4a119e18c5e88b6f660aabb03bcfd21d1f4bb690d721813c0ea849` |
| `specs/001-conversation-session-supervisor/DATA-MODEL.md` | 79 | `1745cf92da06d41b30a3829a43bd5e5f7abd43c4a2e927983c56254fa392e88e` |
| `specs/001-conversation-session-supervisor/RESEARCH.md` | 134 | `895298426d7825a8a09068f3d3b8e752622b43a803678f49cdbaedfd2725b5b6` |
| `specs/001-conversation-session-supervisor/WIRING.yaml` | 254 | `015d799308106e42e9a166408a1628694838067a1547850f170bfd89d37b0ae8` |
| `specs/001-conversation-session-supervisor/contracts/README.md` | 12 | `db996aef02f977b4f33446470050accb843a7e9fc2ea323c9393a9eb8e7a737f` |
| `specs/001-conversation-session-supervisor/contracts/api-events.md` | 63 | `303d60c1969723285a99167dfcac2afe87d8e35ac6bb50e432467778ee9de575` |
| `specs/001-conversation-session-supervisor/contracts/errors-recovery.md` | 78 | `47de1c535b262d4f804ba14eb166f7ce267f325acad83788fd66d93bfc64bb59` |
| `specs/001-conversation-session-supervisor/contracts/ui-behavior.md` | 107 | `8f503bf4c54b846686e5bbd500d45b6c9f9e299ec0361cd4ed76e666898d3ad4` |

## Source cross-check ledger

The listed ranges were inspected for the findings above. Hashes pin the exact current source/schema bytes; unlisted lines in those large files are not claimed as a full source reread.

| Source | Checked lines | SHA-256 |
|---|---|---|
| `apps/litecoworkd/src/agents/native_provider.rs` | 53–99, 200–260 | `cf19da5862421ad8eb056fc33fbf081297e63f2e0cf221e3f43f7830917f7eea` |
| `apps/litecoworkd/src/operator.rs` | 7475–7489 | `37402613e8dfd81fbcb0b9b045a44e6a100fadfc466736b07a17caa730d6e46d` |
| `apps/litecoworkd/src/agents/opencode_profile_probe.rs` | 72–95 | `234551dbcbba1311439c45428aa33b934646c5701e7335b9dd3c16f26a7991fe` |
| `apps/litecoworkd/src/conversation_operator.rs` | 1133, 1201 | `f97b7ff3f8e61b53b09d30dad61e7a64cd35e1106d88a668eecef35f17e614de` |
| `apps/litecoworkd/src/agents/supervisor.rs` | 163, 1085–1095, 1171 | `fcde4daf3be04a5904e3975450ad42c2b073e88a6b26dcbbd4a80fcc0a68bca0` |
| `apps/litecoworkd/src/agents/adapter.rs` | 601–638, 728–739 | `a725fe6874dc406840e51f909fb625cbb9743a80839bb9328803e9490df4db50` |
| `apps/litecoworkd/src/transient_frames.rs` | 76–80, 91–109, 139–159 | `4c53ed8c83dbc4632c6c510d57667a361c177fc4040c8a9b33c1ae725f1d6e3a` |
| `crates/storage-sqlite/src/conversations.rs` | 1026–1037, 1255–1274, 1314–1320 | `871fc0283cbaedc5b86e672d52edee419159b33d752c60e158d6bfd3c4df25c4` |
| `apps/litecowork-ui/src/conversations/conversation-agent-selection.ts` | 21–48 | `a019773b16629aa026cdbe36bc188a25ba0ca4d48f2c164dd0e2910c7c6a476d` |
| `apps/litecowork-ui/src/conversations/conversation-turn-state.ts` | 44–57 | `0cd587a0c8e865b1b0977bc0a133982a2b2fb2c706dee9682f7dd06d69697522` |
| `apps/litecowork-ui/src/conversations/ConversationsPage.tsx` | 782–797, 813–817 | `e6f40a87562efbddd0a0bcccb9df02d2fa7fe9ba52789c72824d357966f9dea4` |
| `docs/schemas/operator-api.openapi.yaml` | 1894, 7027–7036, 7475–7485 | `3a614df67fc65e75e4e4ae9dbf9de898521a845d4bfc38b0a2bb88632329342f` |
| `docs/schemas/operator-stream.schema.json` | 485–529 | `783bb8f79b983db5041307f83c91ff5b34ae62f1199da2894e425f92707e6f10` |

## Remaining work

- Resolve A-92 through a reviewed Feature 001 SPEC revision; do not edit the accepted contract by inference.
- Resolve A-93 with Conversation/Storage owners, then complete the authorized T02 storage path and its negative/recovery evidence.
- Resolve A-94 with Conversation/Presentation/API owners; if AgentSession remains in the delta contract, update schema, producer, stream path, UI and tests in one reviewed change.
- Continue the global semantic audit across the remaining feature packages, global owner docs, and active Rust/Tauri/React source. The global audit remains OPEN.
- Complete T09 packaged Windows and Linux provider journeys and T28 owner verification; documentation changes do not count as qualification.
