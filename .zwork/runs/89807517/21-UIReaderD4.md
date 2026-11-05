# UI Reader D4 — run 89807517 participant 21

Digest written to [reading/digest-ui-04.md](reading/digest-ui-04.md).

**Bucket ui-04** — 13 files read fully + `lib.rs` ~91% of 60 listed; ~7,600 lines. Declared scope deviation: the plan omits `src-tauri/src/lib.rs`, which the bridge brief requires (6,133 lines, `generate_handler!` hub).

Top findings:
1. **Auth is OS IPC, not HTTP/token** — `operator_ipc` framed protocol, one connection per request; Unix `SO_PEERCRED` uid match, Windows named-pipe DACL; every call gated on `GET /v1/operator/readiness` (`SERVING`, `api_contract_version == 1`).
2. **105 registered commands**; spawn is `litecoworkd run --data-dir`, resolved by `is_file()` only (no signature check); Linux uses `systemd_user_service` instead, and an unverified unit forces `DEGRADED` + `LINUX_SYSTEMD_SERVICE_UNVERIFIED`.
3. **Weakest seam:** `DelegationProfileResponse`/`AutomationResponse` return raw `status`/`content_type`/`body_base64` to the WebView with no `is_success()` or typed decode — the only unvalidated daemon-body path found. No credential/handle leak observed.
4. **Strong:** `add_workspace_root` HMAC-scopes idempotency over path bytes (base64 to daemon only); response-echo checks (`version+1`, exact `input_refs`, plan/step identity); readiness decoder *refuses* any `planning_started=true` claim.
5. **One durable native artifact:** `conversation-create-recovery/pending.v1.json` (0700/0600, flock, atomic rename, symlink fail-closed). Open risk: no CSP in `index.html` — unverified, check `tauri.conf.json`.

46 listed files (large CSS, `check-rendered-ui.mjs`, `harness.tsx`, most UI pages) were not read — itemized in Coverage check.