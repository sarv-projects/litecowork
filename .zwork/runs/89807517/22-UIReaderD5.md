# UI Reader D5 — run 89807517 participant 22

**Bucket ui-05** — `apps/litecowork-ui` tests, bridges, config. 59/59 files: 57 fully read + `Cargo.lock` (generated) and `icon.ico` (binary) characterized; ~11,750 lines / ~582 KB of ~16.5k lines / 803 KB.

Top findings:
1. **Codegen is real and gated**: `api:check` → `scripts/generate-operator-api-types.mjs` (`openapi-typescript 7.13.0`) runs inside `build` *before* `tsc --noEmit` and `vite build`. But `tsconfig.json` `include: ["src"]` — tests are never type-checked.
2. **19 test files, ~4.7k lines**, pure `node:test` (no vitest/jest); 3 use a real headless-Chrome CDP harness driving mounted React with a faked `__TAURI_INTERNALS__.invoke` promise ledger.
3. **Missing-Chrome policy is inconsistent**: `needs-you-page-interaction` hard-fails, `task-attention-page-pagination` skips — so `pnpm test` fails on a Chrome-less machine.
4. **`SettingsView.tsx` outruns its implementation**: 5/9 categories are non-persisted `useState`; "Clear All Data" only sets a toast; static badges claim "Windows Job Object Active"/"Service Integrated".
5. Bridges are genuinely fail-closed: allow-listed operations, `deny_unknown_fields`, `no-store` enforcement, digest-pinned atomic Save-As, chunk-by-chunk SHA-256 upload resume.

Digest: [reading/digest-ui-05.md](reading/digest-ui-05.md)