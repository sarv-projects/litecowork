# E03-S01 Agent Catalog truthful status CODE slice — 2026-10-16

**Status:** CODE evidence for a narrow settings projection correction. E03-S01 remains
`IN_PROGRESS`; packaged SYSTEM, provider qualification, supported-OS, and owner USER gates
remain open.

## Outcome

The Agent Catalog now presents three distinct facts without collapsing them into a single
“ready” badge:

- Runtime offer compatibility for creating a Workspace binding;
- the Runtime’s observed process readiness;
- Codex Conversation safety eligibility and its blockers.

A current incompatible offer explains that binding is unavailable. A missing or expired
offer still directs the user to refresh. The Workspace default is described as a saved
lead preference; the UI says that current compatible-offer and session-specific
authorization checks happen again before work starts. The copy also states lead
precedence: an explicit lead wins, a Coworker’s lead setting precedes the Workspace
default for Coworker work, and the Workspace choice is the fallback. The clear-default
prompt says existing explicit and Coworker lead settings remain in place. The list labels
lead eligibility as saved settings, rather than implying that an enabled binding can
currently admit work.
Codex Conversation blockers stay visible without opening diagnostic details.

The owning contracts keep these states separate: [Agent Control](../../docs/AGENT-CONTROL.md),
[Agent Fabric](../../docs/AGENT-FABRIC.md), and [E03-S01](../epics/E03.md#e03-s01-harness-qualification-and-bindings).
OpenCode remains offer-incompatible under its explicit qualification rule. Codex offer
compatibility was not changed: its unqualified Conversation safety controls deny that
specific use, while the Workspace-binding offer remains a separate contract state. No API,
event, schema, or Rust behavior changed in this slice.

## CODE verification

Environment: Linux x86_64/WSL2, Node `24.21.0`, pnpm `12.10.1`, Vite `6.4.4`, and Chrome
`149.0.7827.200`.

From `apps/litecowork-ui/`:

```sh
PATH=/home/sarvesh/.nvm/versions/node/v24.21.0/bin:$PATH node --test --test-reporter=spec tests/agent-binding-disable.test.mjs tests/agent-catalog-settings.test.mjs
PATH=/home/sarvesh/.nvm/versions/node/v24.21.0/bin:$PATH corepack pnpm@12.10.1 test
PATH=/home/sarvesh/.nvm/versions/node/v24.21.0/bin:$PATH LITECOWORK_RENDERED_CHECK_GROUP=agent-binding-ui corepack pnpm@12.10.1 run test:browser
PATH=/home/sarvesh/.nvm/versions/node/v24.21.0/bin:$PATH corepack pnpm@12.10.1 run build
PATH=/home/sarvesh/.nvm/versions/node/v24.21.0/bin:$PATH corepack pnpm@12.10.1 run api:generate
```

Results after the final copy update: focused AgentBinding/catalog tests **14/14 passed**;
the full UI helper suite **275/275 passed**;
the rendered Chrome AgentBinding group passed its Workspace-switch/default-clear and exact
Disable-action check **1/1**; production build passed generated API freshness, TypeScript,
and Vite. The focused browser group is interaction evidence for the existing binding
controls; it does not exercise a provider-backed session or the newly changed status copy
with a live Runtime.

Test-first runs exposed missing independent status text and the prior overclaim
“eligible for new work.” One first focused run had a test expectation mismatch: its
six-blocker Codex fixture did not exercise the empty-blocker fallback. The assertion was
corrected to match the fixture. A read-only adversarial review then found that the
saved-preference copy omitted explicit/Coworker lead precedence. New assertions first
failed against that copy; after the source and clear-default prompt described the fallback
order, focused AgentBinding/catalog tests passed **14/14**. The full suite also found an
older copy assertion, which was updated. The first production build reported stale
generated API types against the broader already-dirty OpenAPI source; `api:generate` brought
the current working-tree generated client up to that schema, then the build passed. That
generated client diff includes unrelated owner contract changes and is not part of this UI
slice; the generated file remains unstaged.

`git diff --check` passed. The browser script cleaned up its temporary Chrome profile and
process. No worktree, branch, or temporary build directory was created. Existing Cargo,
Tauri, `node_modules`, and Vite output caches were retained for continued V1 work.

## Pending evidence and limitations

Owner USER acceptance must confirm that a nontechnical user can distinguish binding
compatibility, process readiness, saved lead preference, and Conversation safety. Real
provider and packaged desktop tests must verify the server projections and admission gates
against qualified installations. This slice does not qualify Codex Conversation sessions,
OpenCode binding, Task planning, authentication, model entitlement, or native execution.
