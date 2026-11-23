# Data Model - Release Engineering and V1 Qualification

**Status:** IN_REVIEW - not owner-accepted; authorizes no implementation; not a claim of implementation or qualification.
**Feature:** 019 | **Authority:** ACCEPTANCE-AND-E2E-PLAN sections 6 and 7, TESTING, SECURITY, V1 release contract section 7.

This package owns release metadata, not product data. It adds no table to the daemon database and no DomainEvent. Its records live as JSON files validated by schemas promoted in T02 (`docs/schemas/release-evidence-bundle.schema.json`, `docs/schemas/release-matrix-row.schema.json`, both new and PROPOSED) and, for the updater, a small local file in the desktop app data directory.

## 1. Records

| Record | Identity | Fields (PROPOSED) | Owner | Persistence |
|---|---|---|---|---|
| ReleaseRow | `row_id` such as `windows-x64-opencode-v2.0.32` | os, os_min_build, arch, target_triple, desktop_runtime (WebView2 / WebKitGTK version), agent, agent_version, auth_mode, protocol, priority (1 first, 2 second), counted (bool) | Release Engineering | `scripts/v1_release_matrix_rows.json` (new) |
| QualificationCell | (`row_id`, `cell_id`) where `cell_id` is an adopted V1 E2E id (E2E-01..E2E-12, E2E-14..E2E-17) or a capability name | state, evidence_bundle_digest, reason, command, observed_at, host_fingerprint | QA | `docs/release/QUALIFICATION-MATRIX.json` (generated); conditional E2E-13 is not a V1 cell |
| EvidenceBundle | SHA-256 of the canonical bundle JSON | journey_id, requirement_ids, owner_contracts, repo_sha, dirty, os_build, arch, packaging_mode, agent_versions, auth_mode, protocol, code_commands[], system_trace (request_ids, correlation_ids, event_sequences, receipt_refs), user_artifacts[] (digest, kind, storage_ref), negative_cases[] (case, outcome), limitations[], reviewer, accepted_at | QA, Product | Redacted index in `docs/release/evidence/INDEX.md`; full bundle in owner storage |
| UserSignoff | (`row_id`, `journey_id`, `bundle_digest`) | reviewer, date, decision (ACCEPT, PARTIAL, REJECT), blocked_or_unknown[] | Product | Inside the bundle and the index |
| UpdateManifest | (`channel`, `version`) | version, pub_date, platforms{target: {url, signature}}, notes_digest | Release Engineering | Static HTTPS file, signed |
| UpdateAttempt | `request_id` (UUID) | from_version, to_version, state, started_at, last_transition_at, refusal_reasons[] | Desktop Shell | App data directory JSON, single record |
| SigningRecord | artifact digest | artifact_name, signer_thumbprint (public), timestamp_authority, signed_at, verify_result | Security | Release job artifact |
| LicensingDecision | ADR number | licence, notices_digest, bundled_components, excluded_components | Product | `docs/adr/` |
| ReleaseDecision | release tag | decision (GO, NO_GO), gate_output_digest, missing[], owner, date | Product | `docs/release/V1-GO-NO-GO.md` |

## 2. Lifecycles

QualificationCell:

```text
NOT_RUN -> RUNNING -> PASSED | FAILED | BLOCKED
NOT_RUN -> UNAVAILABLE   (capability visibly unsupported on this row, with reason)
any     -> NOT_COUNTED   (run happened under WSL2, a container or a CI image)
PASSED  -> STALE         (a later commit changes an input the bundle covered)
```

PASSED requires a schema-valid bundle observed on the row host. QUALIFIED is a property of a journey cell with PASSED plus USER sign-off ACCEPT.
Before a bundle can count, its OS minimum/build, architecture, declared agent and version, authentication mode, negotiated protocol, packaged mode and clean-checkout state must match the `ReleaseRow`. A mismatch is BLOCKED and remains linked only as diagnostic evidence; it is not transferable to another row. The current OpenCode v2 row protocol is `opencode-v2-http-sse`; the ACP route is a separate adapter and cannot be claimed by this row until it is the route actually exercised.

UpdateAttempt:

```text
IDLE -> CHECKING -> UP_TO_DATE | AVAILABLE | CHECK_FAILED
AVAILABLE -> GATING -> REFUSED (NotQuiescent or gate unavailable)
GATING -> DOWNLOADING -> VERIFYING -> STOPPING_RUNTIME -> HANDOFF -> COMPLETED
any active -> FAILED (signature, download, stop timeout, installer exit code)
```

Only one active UpdateAttempt exists. On launch, an attempt left in DOWNLOADING, VERIFYING or STOPPING_RUNTIME is settled FAILED with reason `INTERRUPTED`; an attempt left in HANDOFF is settled COMPLETED if the running version equals `to_version`, else FAILED.

ReleaseDecision: `OPEN -> NO_GO | GO`; a GO is bound to one release tag and is invalidated by any new commit on that tag.

## 3. Retention and privacy

Bundles keep digests of screenshots and logs, not the content, in the repository. Logs are redacted before inclusion and secret-scanned. UpdateAttempt holds versions and reason codes only. No credential, token, prompt or user content is stored in any record above.

## 4. Migration

None for the daemon. Schema files are versioned by `$id` and a `schema_version` field; a breaking change creates a new schema file.
