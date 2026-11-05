# Doc Reader A6 — run 89807517 participant 5

**Bucket docs-06** — 19/19 files read, 5,928 lines, 445 KB.

**Top 5 findings**
1. All UI is TARGET: V1-DESIGN-INDEX, SEMANTIC-SERVED-UI-MAP ("SERVED is not claimed") and the BLOCKED acceptance matrix agree; no `operationId` exists for any control.
2. 187 settings controls are enumerated once, with only 12 `LOCAL_ONLY` (e.g. `setReducedMotion`, `setDensity`, `selectWorkspace`); everything else TARGET/NATIVE.
3. IPC is the tightest contract: allowlisted headers (`X-Workspace-ID`, `Idempotency-Key`, `If-Match`, `Content-Range`, `X-Chunk-SHA256`), reserved `POST /__litecowork_local/workspace-root-selection`, stream type `application/vnd.litecowork.operator-stream+json`, limits 16 KiB header / 100 MiB request / 10 MiB response / 8 conns / 2 streams.
4. Four conflicting Task-state vocabularies exist; only the 13-state `READY…CANCELLED` table is canonical, and `Planning` must stay presentation-only.
5. Drift: ZCODE-UI-INSPIRATION-AUDIT §8 claims a shipped `SettingsView.tsx` mapping all 187 controls, contradicting its own scope and the reset status — pre-reset "Implementation status 2026-10-10/22" blocks elsewhere are historical only.

Digest: [reading/digest-docs-06.md](reading/digest-docs-06.md)