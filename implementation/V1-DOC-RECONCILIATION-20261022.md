# V1 Documentation Correction Pass — 2026-10-22

**Scope:** Canonical LiteCowork product/HLD/LLD and implementation traceability. This is a **documentation PR**, not native agent/Task/LiteSPM runtime completion. All release-blocking unimplemented code remains blocked. The [current ten-area product contract](../docs/V1-PRODUCT-RELEASE-CONTRACT.md) is the target; the [acceptance matrix](V1-PRODUCT-ACCEPTANCE-MATRIX.md) records reality.

## Canonical priorities

1. Product = local-first/chat-first; a durable Task is central to managed **execution**, not to simple Conversation. No mandatory Coworker.
2. LiteCowork manages Tasks/Coworkers/workspaces/permissions/evidence; LiteSPM manages new **and eligible pre-existing** package lifecycles; the native harness owns its auth/models/sessions. Do not add a second global model router or hidden agent planner.
3. HorizonCode is a separately built first-party native chief/worker **candidate** with the same AgentModule qualification standards.
4. Free local core without a mandatory LiteCowork subscription/account is a product requirement; no assumption third-party providers are free.
5. Ten desktop product areas inside V1; cloud LiteCowork Runtime, multi-device/teamwork inside V2. Hosted agents accessed from a local app are independently qualified V1 candidates.

## Reconciled authoritative files

| Scope | New/updating source |
|---|---|
| Product/HLD | `ARCHITECTURE.md`, `README.md`, `docs/PRODUCT.md`, `docs/V1-PRODUCT-RELEASE-CONTRACT.md` |
| UX | `docs/EXPERIENCE.md` (legacy Home archived), `docs/V1-UI-SCREEN-CONTRACT.md`, `docs/DESKTOP-SHELL-LLD.md`, `docs/NATIVE-COMPOSER-LLD.md` |
| Package/capabilities | `docs/LITESPM-LITECOWORK-INTEROP-LLD.md`, `docs/CAPABILITY-FABRIC.md`, `docs/CONNECTORS-WORKFLOWS.md` |
| Native agents | `docs/V1-HORIZONCODE-NATIVE-AGENT-LLD.md`, `docs/AGENT-CONTROL.md`, `implementation/AGENT-PORTFOLIO-2026-10-21.md` |
| Runtime/product proof | `implementation/V1-PRODUCT-ACCEPTANCE-MATRIX.md`, `implementation/SCOPE.md`, `implementation/QUALIFICATION-MATRIX.md`, `implementation/RELEASE.md` |
| Learning/modules | `docs/LEARNING.md` and `docs/CONNECTORS-WORKFLOWS.md` target file layout corrected |
| Historical transition | `implementation/archive/HOME-LEGACY-UX-20261022.md` |

## What is *not* resolved by this documentation pass

- Native Send disabled in `apps/litecowork-ui/src/conversations/ConversationsPage.tsx` until `conversation_operator.rs` admits real native AgentSessions.
- Attempt admissions/leases fail closed in `crates/storage-sqlite/src/execution.rs` until proof writers exist.
- LiteSPM's versioned machine contract, local client, cross-agent host/kind qualification and ConnectorPackage/credential boundary are unqualified.
- Browser/Computer/Terminal and office editing require actual provider/OS tests; a renderer does not imply native automation.
- `INCOMPLETE` Task settlement to Automation occurrence failure in `domain-responsibility/src/occurrences.rs` needs recoverability semantics; cursor retirement and notification delivery acceptance remain.
- Local Coworker learning/memory and heterogeneous workers need producer/invocation/verified end-to-end evidence.
- No CODE + SYSTEM + USER release matrix certifies the ten real product journeys. Existing `backlog.json` stories remain as-is; no blanket completion claims.

## Further verified source-status corrections

- Older codebase audit originally reported a quiet-hours parser-shape mismatch; current `crates/domain-responsibility/src/notifications.rs` accepts `timezone`, `weekdays`, `start_local_time`, `end_local_time`. Do not resurrect the obsolete defect.
- A `notification_deliveries` PENDING writer now exists in `crates/storage-sqlite/src/notification.rs`, which is distinct from proof of actual OS delivery and settled attempts.
- LiteSPM `adopt` CLI and an RPC handler are present in its current source while parts of LiteSPM `STATUS.md` / ARCH/16 remain stale; that documentation needs a companion change upstream. Adoption remains narrower than universal skills/plugins support.
- The source-derived `implementation/served-route-inventory.json` must remain the truth for mounted versus target-only routes. Do not manually change route counts from old audits.
- Some older source-file references in E10 and E13 review packets described illustrative/nonexistent paths. These are now labelled/linked, without creating fake crates.
- `implementation/CURRENT-RUN.md` is chronological history. Recent status should refer to this correction pass and current tests, not treat every old audit's original finding as still open.

## Required follow-on PR acceptance

For any *code* PR: exact source owner and relevant epic, schema/route status, negative tests, idempotent mutation, crash/retry/reconciliation, platform/agent build provenance, and CODE + SYSTEM + USER evidence before “qualified” UI release label. Explicitly state BLOCKED for external providers or OS hardware that cannot be tested. No approval/permission is inferred from a package install or an agent catalog listing.
