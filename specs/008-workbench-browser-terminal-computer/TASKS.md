# Tasks - Workbench Browser Terminal and Computer

**Feature:** 008 | **Status:** DRAFT - do not execute before SPEC is ACCEPTED/LOCKED.

**Spec revision:** 0.2

## 008-T01 - Lock browser mode and shell profile

- **Status:** BLOCKED
- **Depends on:** NONE
- **Owner:** Product and Security
- **Allowed paths:** this package
- **Contract:** SPEC §5
- **Change intent:** Resolve B-22, B-23 and B-32 before any provider or UI implementation.
- **Acceptance:** Product/Security/Browser owner pin one exact browser provider, distribution, version, visible-surface adapter and managed-profile/consent/network/lifecycle route (B-22); Environments/Security pin one real PTY/ConPTY provider, shell executable/profile, allowed argv/env/cwd, Environment process-tree containment and stop/recovery contract (B-23); Product explicitly includes or defers Computer use and names exact provider/platform permission/action scope if included (B-32). No browser/terminal/computer readiness is inferred from enums or UI badges.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 008-T02 - Browser epoch takeover return

- **Status:** NOT STARTED
- **Depends on:** T01 and accepted B-22 provider contract
- **Owner:** Browser provider and Trust
- **Allowed paths:** browser gateway
- **Contract:** CHROME-BROWSER-LLD
- **Change intent:** Consent, epoch, takeover, stale reject.
- **Acceptance:** On the declared OS/build, a real version-pinned browser provider launches/attaches only under approved profile policy; live view and provider session identities are distinct; exact session/tab generation, principal, control epoch and lease are checked at every action; Takeover fences/discards queued agent inputs atomically; Return requires explicit confirmation and a fresh observation; provider loss/restart becomes RECOVERING/LOST/AMBIGUOUS without replaying stale actions; consent is revocable and profile secrets are not copied.
- **Command:** `cargo test -p litecoworkd --locked browser_epoch`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 008-T03 - Terminal bound to Environment

- **Status:** NOT STARTED
- **Depends on:** T01 and accepted B-23 provider/shell contract
- **Owner:** Environments
- **Allowed paths:** pty host
- **Contract:** V1-INTEGRATED-TERMINAL-LLD
- **Change intent:** Discovery without startup execution; one process tree per tab.
- **Acceptance:** Real PTY/ConPTY process is spawned under the admitted Environment with exact shell/profile/argv/cwd/allowlisted environment, process identity and containment receipt; stdout/stderr framing, resize, bounded scrollback/backpressure and stdin are connected to Tauri; tab switch/detach/reattach does not respawn or kill the process; explicit Stop terminates its owned process tree; UI close, Runtime stop and crash yield exact QUIESCENT or LOST/UNKNOWN evidence; no UI claim of continuation without a surviving process.
- **Command:** `cargo test -p litecoworkd --locked terminal_environment_bind`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 008-T04 - Workbench views

- **Status:** NOT STARTED
- **Depends on:** T02-T03 and accepted B-32 decision
- **Owner:** Desktop UI
- **Allowed paths:** right workbench
- **Contract:** DESKTOP-SHELL-LLD and ui-behavior
- **Change intent:** Show real availability only.
- **Acceptance:** Browser availability is distinct from mode/session/view; iframe preview is never labeled as a qualified Chrome/agent browser. Terminal shows only provider-backed shell/process state and never prints fabricated exit codes, containment or quotas. Files are explicit Resource/authorized filesystem objects. Diffs are derived from actual immutable revisions or VCS change sets; Activity is sourced from committed events/attempt/process observations or is clearly deferred. Computer-use controls exist only when B-32 provider and permissions are qualified; otherwise no simulated buttons/cursor.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.
