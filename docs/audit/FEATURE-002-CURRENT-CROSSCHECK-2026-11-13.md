# Feature 002 current source cross-check — 2026-11-13

## Scope and evidence boundary

All 11 current files in `specs/002-native-agent-install-auth-binding/` were read line by line in this continuation and compared with their accepted owner contracts, the Feature 011 screen contract, current OpenAPI, and the active Rust/Tauri/React paths listed below. The reviewed package contains 1,406 physical lines; exact byte hashes are recorded in the manifest. Source inspection was bounded to the directly relevant implementations and call sites. It is not a full reread of the 24,000-line storage file, the 9,800-line Operator file, or all other repository sources.

Implementation source checkpoint before the follow-up correction: `6adadab539a62ca1c8b70621c7f88e1785cfe8c2`. Documentation baseline at the start of this package review: `d6c5819ca61a96f9fed0b39e1cbe10e511256358`. After the bounded source review, the catalog incorrectly used the Conversation-only `profile_admission_supported` field to report Task lead eligibility. The T01-authorized React UI and its existing test were corrected; the focused catalog test file passes all 8 tests. No Rust, Tauri, schema or migration behavior changed. No provider sign-in, credential submission, packaged desktop journey or provider qualification was performed. This report does not qualify Feature 002 or close the repository-wide audit.

## Findings and corrections

1. **OpenCode offer semantics:** `opencode_offer_admission` returns a compatible `STARTABLE` offer only for CLI 2.0.32 with a complete probe and observed host stop. The registered executable identity is shared with dispatch. That exact offer permits creation of a disabled binding under accepted AC-013; it does not prove authentication, enabled binding readiness, or qualified Conversation Send. The old global `docs/DATA-MODEL.md` sentence said offers always remain incompatible until model selection/execution qualify. It has been corrected to distinguish offer compatibility from authenticated/qualified use.
2. **Descriptor producer:** the trusted module registry currently registers OpenCode only. Descriptor GET/refresh is served for profiles selecting that module and reuses the table from migration 16. Module-less profiles fail closed with 503. The three-per-minute refresh budget is process-local and resets at restart; session-options and harness-capability routes remain target-only. The remaining F13 registry listing/assessment calls are not route-wired.
3. **Binding gate:** create uses a fresh compatible Runtime offer and creates a disabled binding. Enable repeats the offer check and enforces descriptor revision only when a trusted module is selected; legacy module-less profiles still bypass the descriptor requirement. The limitation is already recorded in Feature 002 and A-31; it remains unresolved against accepted AC-004 and must not be called universally descriptor-gated.
4. **Native sign-in:** the OpenCode manifest and descriptor declare no native auth method or native-config target. The authenticated ID-only Tauri commands and Operator routes exist, but production uses `UnqualifiedLauncher`/`UnqualifiedOpener`, which fail closed. Auth observations are in-memory and read back as UNKNOWN after expiry/restart. There is no successful sign-in journey.
5. **Credentials:** `submitAgentCredential` is mounted, but production injects `NoAgentDescriptorSource`; even a descriptor-backed test route remains disabled until replay-safe cross-store recovery is available. The current path returns 503 without writing SecretStore. No credential-entry UI or Tauri credential-submission command exists. The Feature 002 T14 UI/API trace omitted the secure-key row and its no-control/no-bridge facts; the row is now explicit and names the existing fail-closed route and test.
6. **Settings trace:** descriptor refresh is a real ID-only Tauri command (`refresh_agent_control_descriptor`); the prior trace put the HTTP route in the Tauri-command column. That mapping is corrected. The Settings panel displays only descriptor-declared controls. No install/update route or action, secure-key form, model-options bridge, or Usage projection is active.
7. **Second provider and ACP:** Claude Code remains inventory-only; the local help/version observation is dated 2026-11-11 and is not a protocol/auth/cancellation fixture. The ACP client is bounded code exercised by fixtures but has no provider launch/Conversation dispatch arm; the published SDK/protocol pin remains absent. Matchmaker ranking is advisory code only, using keyword extraction, with no Operator route or UI. These components do not make a second agent path available.
8. **Plan and navigation alignment:** Feature 002 PLAN incorrectly listed T15–T17 as its final qualification/documentation/VERIFIED sequence, although TASKS defines T17/T18 as ACP/Matchmaker and T19 as owner-verified completion. The plan now follows those stable task IDs and dependencies. The screenshot-specific research text that listed every surface as a permanent sidebar entry is also superseded; it now links to V1-UI-SCREEN-CONTRACT §1 and preserves the compact four-destination shell.
9. **Conversation versus Task admission copy:** the OpenCode probe's `profile_admission_supported` is explicitly Conversation-only in Rust and T01. The catalog previously rendered it under “Task lead eligibility,” so the exact 2.0.32 compatible fixture misleadingly reported Task support. The row now names Conversation profile admission and the separate Task row always reports Task admission unsupported; the focused UI test covers both facts with a compatible fixture.

## Current task and release posture

- T01, T03 and T04 are CODE-COMPLETE; T02, T05–T07, T09–T14 and T16–T18 are PARTIAL.
- T08 is BLOCKED on an owner-approved SecretStore/SQLite replay and crash-reconciliation contract. Migration numbering is not the blocker; v34 remains provisional only if reviewed DDL is needed.
- T12 is PARTIAL: CLI help advertises a stream-json/auth surface, but no safe-stop, auth-mode, or redacted real stream fixture is recorded.
- T15 remains DEFERRED by the owner for native packaged Windows/Linux qualification. T19 is NOT STARTED.
- No module is QUALIFIED. OpenCode's offer is bindable in CODE for the exact tuple, but real sign-in, packaged binding/send, stop/retry, restart/recovery, and owner USER evidence remain absent. Claude Code is not a second dispatch path.

## Feature 002 package line/hash manifest

SHA-256 is over exact file bytes after this review's corrections.

| File | Lines | SHA-256 |
|---|---:|---|
| `specs/002-native-agent-install-auth-binding/README.md` | 40 | `855e41bcfd9f2d42e9fc8b9f6c56d7d0e3878d385711690d3d25fb7a2691ca5f` |
| `specs/002-native-agent-install-auth-binding/SPEC.md` | 468 | `1c624639dd9fd06a4a5f2efd523bbe28b6162ba9f90b7102d05623cb0409d712` |
| `specs/002-native-agent-install-auth-binding/PLAN.md` | 54 | `5ed090245ca7b584d59abc0895d720ab59a27d0afc24b0e55b8413fca7850640` |
| `specs/002-native-agent-install-auth-binding/TASKS.md` | 401 | `8d9ece19816fd0e517374ac1510b0157523f9ac0fd7660cf30f5be05dc6a3586` |
| `specs/002-native-agent-install-auth-binding/DATA-MODEL.md` | 44 | `6abe39c9aac9b6c9c7784d9f0e926e678952f69525efb1545585cd5d934e292f` |
| `specs/002-native-agent-install-auth-binding/RESEARCH.md` | 110 | `a3364d3d9bfc4a1ff5e7b1c3191f16716aedc87b51769cef5838fd586dcceb67` |
| `specs/002-native-agent-install-auth-binding/WIRING.yaml` | 134 | `698416f08baf10965c9ee41c07f745f0ec5fd4f8483b9b6da035c0e40628f771` |
| `specs/002-native-agent-install-auth-binding/contracts/README.md` | 9 | `650e7a2f3b6ee610b458ff550a8c1e02ae54ff89fcb8c1ed2d7885ca46754e42` |
| `specs/002-native-agent-install-auth-binding/contracts/api-events.md` | 28 | `aebadc0e358e1fceb210b8184cc9150234a2708ba390396c73be60f63987e266` |
| `specs/002-native-agent-install-auth-binding/contracts/errors-recovery.md` | 9 | `58b054462532f74a79b55c11a1a44db5b56b73852496c76378f9e0202a5e171a` |
| `specs/002-native-agent-install-auth-binding/contracts/ui-behavior.md` | 109 | `f2832195b20f903968fef995c1d4dd174f0f8600b37ec720b51b60acec3f8800` |

## Source cross-check ledger

Hashes pin each inspected file at this source checkpoint. The checked ranges establish the listed findings only; unlisted lines of large files are not claimed as reread.

| Source | Checked lines | SHA-256 |
|---|---|---|
| `apps/litecoworkd/src/agents/native_provider.rs` | 53–99, 200–260 | `cf19da5862421ad8eb056fc33fbf081297e63f2e0cf221e3f43f7830917f7eea` |
| `apps/litecoworkd/src/agents/opencode_profile_probe.rs` | 72–95 | `234551dbcbba1311439c45428aa33b934646c5701e7335b9dd3c16f26a7991fe` |
| `apps/litecoworkd/src/operator.rs` | 7475–7489, 7730–7848 | `37402613e8dfd81fbcb0b9b045a44e6a100fadfc466736b07a17caa730d6e46d` |
| `apps/litecoworkd/src/agents/module_registry.rs` | 1–120 | `8441f026a85892dbe0762cec52f5c73a79c2a5a64d226c3894f28bcf3524805d` |
| `apps/litecoworkd/src/agents/modules/mod.rs` | 1–19 | `7afd45e0509596e6c020db84358c0ce12f4d58d936f680eee83fc99c9b654ea3` |
| `apps/litecoworkd/src/agents/modules/opencode.rs` | 30–115 | `134d4c006ce4c9d2faf13b60886973edf02f354750bcd5be063a07409a61d59d` |
| `apps/litecoworkd/src/agents/descriptor_service.rs` | 75–190 | `5784c81340e54782b779924040a7d613f8468279c1c2e2160d2912b0bd713d89` |
| `apps/litecoworkd/src/agent_control_operator.rs` | 66–210 | `1d9068e3c39acc5435d76a82603bb749d7bb44a874e9502e2b33b6df3bd693a6` |
| `apps/litecoworkd/src/agent_auth_operator.rs` | 1–141 | `5fa09fdc1db49aa754c9ebf76872793290582c10919e32400f4ebbe3efb462ab` |
| `apps/litecoworkd/src/agents/auth_service.rs` | 24–199 | `831894c047721fa9116ce8a079d0426752cabda71c8f4c7469e91c01068904b4` |
| `apps/litecoworkd/src/agent_credentials_routes.rs` | 49–223, 420–444 | `7037e3c063173acfc6cb12078fa8940cde4dbe80329f5b0609062e22d5544323` |
| `apps/litecoworkd/src/agent_credentials_operator.rs` | 150–260 | `f2f0ad1015515b16e65684099560afc91ee70279709a47e8be402fb4fbc1c56a` |
| `apps/litecoworkd/src/agent_native_config_operator.rs` | 44–115 | `c4b24714dd302e6e6c97f903e67fa6f9025ed7243878062d8d3715df53c318d5` |
| `crates/storage-sqlite/src/lib.rs` | 18986–19060 | `5d3c1d07c1f365543168dab379fbf784121e3ba48c31a26a9c7bbd0d59fc6944` |
| `apps/litecowork-ui/src-tauri/src/agent_control_bridge.rs` | 30–214 | `b074385571a902d63a24e5c1b9f3efb07d4efaadfc052ea7801505730fdfd63e` |
| `apps/litecowork-ui/src-tauri/src/agent_binding_bridge.rs` | 1–129 | `791369daf72c6127a14f09a77e9d6d17a346b4c1c7f4a0f3ed6d7faaea596580` |
| `apps/litecowork-ui/src/agents/AgentControlDescriptorPanel.tsx` | 25–140 | `e0ac94699a619bff0f8f58e255911578c2cb022b672d7b4f6becc5098c0826b1` |
| `apps/litecowork-ui/src/agents/AgentCatalogSettings.tsx` | 780–931 | `915ed7f529bac8926b7c34f570af041c94f71e0bdbdeef327141d224bf3f73b1` |
| `apps/litecowork-ui/tests/agent-catalog-settings.test.mjs` | compatible OpenCode fixture and test | `ab1ab19a50c8c534603c776de33896c893ba1de3e87d55b6c7b55a770ac73e64` |
| `apps/litecoworkd/src/agents/acp_client.rs` | 1–130; repository search for dispatch call sites | `be3727f2e02c8da145430267dc2116d5fa08364c608fdcb398df0baa18528ef2` |
| `apps/litecoworkd/src/agents/matchmaker.rs` | 1–160; repository search for route/UI call sites | `b9455a5ba1138d7d3185ab03395858fb66a78e73b054c5972b8abd35e5f360dd` |
| `docs/schemas/operator-api.openapi.yaml` | 72, 1947, 2087–2100 | `3a614df67fc65e75e4e4ae9dbf9de898521a845d4bfc38b0a2bb88632329342f` |
| `docs/DATA-MODEL.md` | 657–678 | `92d6d36654272306d9b22607a076d501541d4313cae3250df897311e8307746a` |
| `docs/V1-UI-SCREEN-CONTRACT.md` | 12–32, 301–325 | `ea73e8f4737eb7528fce875bff2136fea5f33aff0f6ff3eaf002ee303afc2b56` |

## Remaining work

- Resolve the module-less descriptor gate against accepted AC-004; preserve A-31 and do not describe the current backend as universally descriptor-gated.
- Resolve the T08 owner decision for idempotent SecretStore writes and SQLite receipt recovery before enabling a secure-key form or any credential mutation.
- Complete native auth method/launcher and durable auth-observation semantics only for a module with pinned provider evidence.
- Pin the ACP SDK/protocol version or revise T17 through the owner process; the current custom client code is not a qualified provider.
- Complete session-options projection and the admission revalidation/selected-model flow before mounting a model/effort picker.
- Qualify OpenCode and the Claude Code second module on native packaged Windows and Linux, then prove denial, replay, stop/retry, restart/recovery and owner USER evidence. T15 is owner-deferred and T19 remains not started.
- Continue the global all-document/all-source semantic audit; it remains OPEN.
