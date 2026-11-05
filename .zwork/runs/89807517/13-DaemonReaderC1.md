# Daemon Reader C1 — run 89807517 participant 13

**Bucket daemon-01** — 13 files read fully + `operator.rs` 1–7500/9525; ~14,000 of 21,460 lines. Digest: [reading/digest-daemon-01.md](reading/digest-daemon-01.md)

Top findings:
1. **No HTTP listener.** `operator.rs` is an authenticated local IPC frame server (Unix peer-UID / Windows SID) driving `axum::Router` via `oneshot`; auth is a private `AuthenticatedLocalPeer` extension, unforgeable by header. `/v1/operator/readiness` reports `api_contract_version: 1`.
2. **Deliberate permanent stubs:** `GET /v1/capabilities/zip-intake` always `UNAVAILABLE/OPAQUE_RESOURCE_ONLY`; `/v1/coworkers/{id}/presence` always 503; computer-use provider probe can never return `Available`; planning readiness hard-codes `dispatch_available:false`.
3. **Evidence discipline is real:** OpenCode offers are `compatible:false` (DEGRADED max), Codex conversation eligibility is hard-coded `NOT_ELIGIBLE` with six named blockers, all sanitized projections strip paths/secrets/model payloads.
4. **Two authz tiers:** `authorized_workspace` (owner + ACTIVE RuntimeWorkspaceBinding) vs `authorized_data_workspace` (owner only, stays readable after archive).
5. **Drift risk:** three If-Match parsers with different floors (0 allowed for notifications, >0 elsewhere), duplicated cursor codecs, and several handlers calling SQLite on the async thread.

Not covered (deadline): `user_request_operator.rs` and six `*_tests.rs` files — stated in Coverage check.