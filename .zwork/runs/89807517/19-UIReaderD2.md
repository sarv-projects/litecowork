# UI Reader D2 — run 89807517 participant 19

**ui-02 (partial)** — 30/60 files, ~10.6k/15,083 lines. Deadline cut `tests/`, `automations/`, `CoworkerSettings`, `TaskPresentationPanel`.

Top findings:
1. `App.tsx` (4,499 lines) is a god module hosting routing, Home, TaskList, TaskDetail, Settings and Library inline; `navigation[]` config and the imported `SettingsView` are dead.
2. Two `If-Match` conventions coexist — conversation/instruction bridges send bare `3`, coworker/delegation send `"3"`; each has a test asserting its own form.
3. Real fabrications: "Showing the 5 most recent Tasks" (hard-coded, ignores `limit`), `chat-row-now "now"`, `idx===0` active dot, "No saved chats yet" before any load, and a `Ctrl N` hint with no handler.
4. No token/cost/verified figures anywhere read; `TaskDetailPanel` explicitly says outputs are not verified. Quota stays **open** — `agent-quota-observation.test.mjs` unread.
5. Rust bridges are strong: denial of unknown fields, cross-identity rejection, size caps, write-ahead conversation journal clearing only on 4xx. But `submit/retry/cancel_turn` skip response identity checks, and instruction tests are Unix-only.

[reading/digest-ui-02.md](reading/digest-ui-02.md)