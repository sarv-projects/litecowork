# Implementation Plan - Rich Presentation, Intelligent UI and Generated Media

**Feature:** 016 | **Spec revision:** 0.2 (proposed)
**Status:** IN_REVIEW - authored for owner review; does not authorize implementation until owner acceptance (TASKS T01).

## 1. Delivery strategy

Fix the foundation before adding features. First make one Rust validator the authority and prove the TypeScript mirror agrees with it on a shared corpus; then add the restricted calculation AST, the chart and the one Intelligent UI component; then the production binder, the disable preference, media ingestion and the inert interactive preview; then security gates, recovery, performance and qualification. Wave 2 in the V1 delivery plan. Both release rows can qualify because nothing here needs an Environment provider.

### Language and module placement (optimization)

| Concern | Placement | Reason |
|---|---|---|
| Bounds, allowlist, canonical bytes, digests, CalcAst validation | Rust, new `crates/domain-presentation` | Reject bad documents before they persist; one definition shared by storage and any future publisher; no dependency on SQLite |
| Defensive re-validation in the WebView | TypeScript `rich-presentation.ts` (existing) | Defence in depth at the render boundary; same corpus as Rust |
| Calculator evaluation | TypeScript `calc-reducer.ts` | Runs on view-local state the daemon never sees |
| Chart geometry | TypeScript `chart-model.ts` (pure) + React SVG | No chart library; testable without a DOM |
| Media sniffing and hashing | Rust `media_ingestion.rs` | Bytes never reach the WebView unvalidated |

Existing `serde_json`, `serde_json_canonicalizer` and `sha2` cover the Rust needs; no new crates beyond the workspace member. No new npm dependency.

## 2. Dependency graph

```mermaid
flowchart LR
  T01[T01 baseline acceptance] --> T02[T02 contracts]
  T02 --> T03[T03 Rust validator crate]
  T03 --> T04[T04 corpus parity]
  T03 --> T05[T05 CalcAst + reducer]
  T04 --> T06[T06 chart]
  T05 --> T07[T07 SAFE_CALCULATOR]
  T06 --> T07
  T05 --> T08[T08 binder]
  T04 --> T09[T09 disable preference]
  T02 --> T10[T10 media Artifacts]
  T01 --> T11[T11 inert interactive preview]
  T07 --> T12[T12 no-exec gate]
  T08 --> T13[T13 recovery]
  T10 --> T13
  T07 --> T14[T14 performance]
  T12 & T13 & T14 --> T15[T15 traces]
  T15 --> T16[T16 E2E-11 both rows]
  T16 --> T17[T17 docs]
  T17 --> T19[T19 baseline VERIFIED]
  T06 --> T18[T18 Markdown degradation]
  T18 --> T19
  T01 --> T25[T25 extension owner decision]
  T25 --> T20[T20 ingress and intent contract]
  T20 --> T21[T21 shared Canvas controller]
  T21 --> T22[T22 optional persistence]
  T21 --> T23[T23 explicit new-turn modification]
  T22 & T23 --> T24[T24 extension qualification]
  F001[Feature 001 final commit] -.-> T08
  F007[Feature 007 Artifact create] -.-> T10
  F011[Feature 011 preferences, CSP] -.-> T09
```

## 3. Phases and exit gates

| Phase | Tasks | Exit gate |
|---|---|---|
| 0 Baseline acceptance | T01 | Owner decision covers AC-001..AC-019 only; extension gate is T25 |
| 1 Contracts | T02 | Schema v2 profile, calc-ast schema, errors, events promoted |
| 2 Validation | T03-T05 | Rust and TypeScript agree on the corpus; drift closed |
| 3 Components | T06-T07 | Chart and calculator accessible and tested |
| 4 Producer and preference | T08-T09 | Production publish after commit; disable path |
| 5 Media and interactive posture | T10-T11 | Provenance Artifacts; inert preview |
| 6 Hardening | T12-T14 | No-exec gate, recovery and performance green |
| 7 Trace and qualification | T15-T16 | E2E-11 USER evidence on Windows x64 and Linux x64 |
| 8 Docs and baseline VERIFIED | T17-T19 | Status rows match evidence |

## 4. Scope and file ownership (candidate paths)

| Area | Paths |
|---|---|
| Rust validator | `crates/domain-presentation/` (new), `Cargo.toml` members |
| Storage | `crates/storage-sqlite/src/rich_presentations.rs`, `crates/storage-core/src/rich_presentation.rs` |
| Daemon | `apps/litecoworkd/src/presentation_binder.rs`, `media_ingestion.rs` (new), `agents/supervisor.rs` (one call each) |
| Desktop | `apps/litecowork-ui/src/presentation/` (`rich-presentation.ts`, `RichResponseView.tsx`, new `ChartBlock.tsx`, `chart-model.ts`, `SafeCalculatorBlock.tsx`, `calc-reducer.ts`), `src/artifacts/ArtifactWorkbench.tsx` |
| Contracts | owner docs and schemas listed in TASKS T02 |

## 5. Persistence, migration, rollout

No new table. The existing `rich_presentations` table and blob purpose `RICH_PRESENTATION` are reused; media provenance rides on Feature 007 ArtifactVersion records. Lowering the document cap from 1 MiB to 512 KiB affects only new publishes; no production rows exist (no producer today). Rollout: the binder is off until T08 lands; renderer contract version 2 is ignored by an older desktop, which falls back to semantic text.

## 6. Operational risks

| Risk | Mitigation |
|---|---|
| Validators drift again | Shared corpus run by both test suites in CI (Feature 019) |
| Chart misrepresents data | Chart only from the bound semantic table; data table always available |
| Calculator mistaken for an action | No submit, no bridge call, view-local state, visible fallback text |
| Image bytes carry an exploit | Magic-byte sniff, size and dimension bounds, no SVG, display through authorized read |
| CSP loosened for images | Feature 011 owns the change; Feature 019 audits it |
| Interactive app pressure | PREVIEW_ONLY until a new accepted revision after E08-S09 evidence |

## 7. Validation command matrix

| Layer | Command |
|---|---|
| Docs | `python3 .specify/scripts/validate_feature_package.py` |
| Rust validator | `cargo test -p domain-presentation --locked` |
| Storage | `cargo test -p storage-sqlite --locked rich_presentation` |
| Daemon | `cargo test -p litecoworkd --locked presentation_binder` |
| Desktop | `pnpm --dir apps/litecowork-ui test` and `pnpm --dir apps/litecowork-ui run build` |
| Feature | `cargo test -p litecoworkd --test rich_presentation_intelligent_ui --locked -- --nocapture` |

## 8. Stop and rollback criteria

Stop if any path executes model content, a rendered element changes host state, the binder runs inside the final-commit transaction, a URL is fetched for media, or Run appears for an interactive Artifact. Roll back by disabling the binder call; stored presentations stay readable and the desktop falls back safely.

## 9. Handoff

Feature 007 consumes the media Artifacts; a later revision (after E08-S09) may open the Interactive Artifact Runtime.

## 10. Revision 0.2 gated extension sequence

[Pipeline LLD](../../docs/ACP-PRESENTATION-PIPELINE-LLD.md) is the implementation decomposition target. T25 is the separate owner decision for AC-020..AC-028; T01's B-40/baseline acceptance cannot approve it. T25 must also explicitly defer or separately scope the A-103 Saved View proposal; T22 local input persistence is not a Library save. T20 resolves exact provider ingress, research pins, calculation intent, metrics and canonical promotion after T01/T02/T25. T21 depends on T04/T05/T07/T08/T20 and Feature 011 Workbench focus/navigation, and introduces one controller for inline/Workbench Canvas without persistence. T22 is a separately reviewed persistence slice: consent/privacy/backup/deletion/quota, migration allocation, authenticated typed commands and expected-revision/receipt transactions. T23 is independent of T22 and Feature 001 new-turn dispatch; it previews a selected in-memory snapshot and sends it only after explicit normal Conversation Send, then adds new-message modification lineage. T24 depends on T22/T23 and performs hostile/recovery/accessibility/packaged evidence. T19 baseline verification is not evidence of extension completion; extension acceptance requires all additional ACs and owner sign-off.

No persistence rollout begins under the existing no-table baseline paragraph: that statement applies only to original B-40 scope. T22 must allocate the migration through the global allocation owner, promote API/event/schema/UI trace contracts together, and provide backup/deletion/rollback handling before a write endpoint or UI switch can ship. Flags default off and cannot bypass acceptance. Rollback disables extension commands, retains semantic messages and immutable presentations, and preserves authorized saved data until explicit retention/deletion policy applies. Never erase user data as rollback.
