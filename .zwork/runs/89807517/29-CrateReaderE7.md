# Crate Reader E7 — run 89807517 participant 29

Digest complete at [reading/digest-crates-07.md](reading/digest-crates-07.md).

**Bucket crates-07** — 29/29 files read, 22,184 lines (818 KB), 130 `#[test]` functions in 16 files, 4 `#[ignore]` platform qualifications.

**Top 5 findings**
1. **"Unknown is not success" is enforced in code, not prose**: `LifecycleDecision::ReconciliationRequired`, `InvocationStatus::Ambiguous` settleable only via `reconcile_ambiguous_terminal`, `ProviderDestroyResult::Ambiguous` never becomes DESTROYED.
2. **Four deliberate fail-closed holes**: Effect `Started` → `DispatchAdmissionUnavailable`; Environment destroy → `ENVIRONMENT_DESTROY_ADMISSION_UNAVAILABLE`; DelegationProfile `EnablementUnavailable` + `SessionOptionsUnsupported`; Automation recurring hosting absent (README admits it).
3. **Reported/observed/verified split is a SQL predicate**: `validate_evidence_producer` admits only REPORTED from `runtime:<id>:<inc>`; VERIFIED needs a PASSED `verification_runs` row with matching `verifier_kind` and still-current pinned heads.
4. **Replay rigor**: auth precedes replay everywhere; replay resolved before state checks in `transition_effect`/`transition_invocation`; agent-binding test asserts exact replay adds no event/receipt/audit, payload reuse → `Conflict{None,None}`.
5. **Weak spots**: `skill.rs` (1943 lines, 16 methods, consent/caps) and `invocations.rs`/`needs_you.rs` have zero unit tests; `schedule::local_from_instant` panics via `.expect` on an out-of-range cursor; README's "21 tests" is stale vs 25 in-bucket.