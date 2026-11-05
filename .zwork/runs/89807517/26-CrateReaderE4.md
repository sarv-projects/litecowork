# Crate Reader E4 — run 89807517 participant 26

Bucket crates-04, 20 of 28 files read (~9,016 of 20,817 lines); deadline cut the last 8 storage-sqlite test/impl files, declared in the digest's coverage section.

Top findings:
1. The brief's three containment crates are absent from this bucket — no DACL, job-object, namespace or process-tree fencing appears in any file read. Containment here is Unix socket confinement (`0o700` dir, `0o600` socket, inode-identity rebind refusal, peer-credential capture, `MAX_CONCURRENT_CONNECTIONS`) plus DB-level epoch/binding fences.
2. `capability_dispatch.rs` self-declares it fails closed on the current schema (`sqlite-v15.sql` guards, `implementation/reviews/E04-S03.md`) — dispatch admission is written but not serviceable.
3. Real defect in `skill.rs::build_app_frame`: emits `resource-src`, which is not a CSP directive, so declared `resource_domains` are never selectively allowed; tests only string-match, hiding the mismatch.
4. Fail-open in `notifications::decide_notification` — `quiet.contains(...).unwrap_or(false)` treats a bad timezone as "not quiet hours" and delivers.
5. `windows_credential_manager.rs` is the only native Windows security test: `#[ignore]`, env-gated, exact-account cleanup, never wildcard deletes — QUALIFIED-candidate only.

Digest: [reading/digest-crates-04.md](reading/digest-crates-04.md)