# UI Reader D1 — run 89807517 participant 18

**Bucket ui-01** — 51/51 files read, ~6,850 of 19,632 text lines (all hand-written files in full; `operator-api.ts` sampled structurally), 803 KB.

**Top findings**
1. `src/api/operator-api.ts` is **typings-only** (openapi-typescript: `paths` / `components` / `operations` / `$defs`) — no fetch, no client, no auth-header injection. "Generated API client" ≠ transport.
2. Transport is **Tauri IPC**: `coworker_request`, `goal_request`, `get_task_presentation`, `get_task_progress`, plus 9 agent-catalog commands; `X-Workspace-ID` is a typed OpenAPI parameter, enforced in adapters, not in a generated client.
3. Error mapping is type-level: every operation `default: responses["Error"]` → `ApiError{code, message, retryable, correlation_id}` with a ~150-code enum; UI parsers strip the `[CODE; retryable=…; reference=…]` suffix.
4. Consistent fail-closed layering: pure `create*Api(workspaceId, transport)` + strict decoders (sha256, enum allowlists, cross-workspace rejection) behind desktop adapters; strongest is `appendTextVersion` recomputing the submitted digest.
5. No mock data; two unledgered product-surface claims: "Worker execution is not available in this Runtime." and `SENDING_NOT_AVAILABLE` (chat sending unavailable) — relevant to the chat-first V1 promise. `src/App.tsx` is *not* in this bucket (shell evidence only indirect, via the CDP script and a source-grepping test).

Digest: [reading/digest-ui-01.md](reading/digest-ui-01.md)