# Doc Reader A2 — run 89807517 participant 1

I ran out of time on the write; here is the state.

**Bucket:** docs-02 — 17 files read / 17 listed, 5,836 lines, 455,106 bytes (444.4 KiB).

**Top 5 findings**
1. `docs/API.md` is the bucket's only real served-vs-TARGET ledger: mounted routes vs "declared in OpenAPI, no handler" (turn `messages`/`usage`, all task control routes, `plan-revisions` POST, agent auth/config writes, `runtime-lifecycle` stop trio, generic workspace-root create/PATCH). Conversation Send settles `OPEN → FAILED` with `503 DEPENDENCY_UNAVAILABLE`.
2. Transport law is consistent across five files: `Idempotency-Key`→RequestId, `If-Match`→`version`, `X-Workspace-ID` scope, IPC `request_id` correlation-only; sole no-key route is `POST /v1/automations/host/settle` (identity = trigger + slot instant).
3. `VIABILITY-VS-COMPETITORS` labels RV-02/RV-04/RV-09/RV-10 "LOCKED DESIGN" (GA tuple, browser CDP, 229 operations, 60 s drain with `AMBIGUOUS` effects), directly contradicting `AGENT-FABRIC`, `CHROME-BROWSER-LLD`, `API.md` and `RUNTIME-LIFECYCLE`; owner docs win, so that table is drift.
4. OpenCode is pinned to profiles 2.0.26/2.0.28/2.0.32 with `/api/info` version+PID gates and a required `server listening on http://127.0.0.1:<port>` stdout line before any Basic credential — versus VIABILITY's "OpenCode CLI v1.0.0 / Claude Code CLI".
5. `COMPLETE-ACTIVE-CORPUS-INVENTORY` self-declares stale counts (pre-`specs/002`–`011`) and disagrees with the plan on two files (496 vs 497; 165 vs 163); 187 settings controls / 30 groups is the settings ledger figure.

**Digest path:** not written. The `write` to `reading/digest-docs-02.md` was cancelled at the deadline, and follow-up `ls`/`find` calls were also cancelled, so I cannot confirm the file exists — treat it as absent and re-run the write. All 17 files were fully read (API.md paged 1–2388 in four reads; the settings ledger's truncated first read was completed from line 180 to EOF), so the analysis above needs no re-reading.