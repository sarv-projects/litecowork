# UI behavior - Golden Connection and One Capability Write

Behavior, not pixels. Screen inventory: [V1-UI-SCREEN-CONTRACT.md](../../../docs/V1-UI-SCREEN-CONTRACT.md). Control classes: [V1-SETTINGS-CONTRACT-LEDGER.md](../../../docs/V1-SETTINGS-CONTRACT-LEDGER.md) and [SEMANTIC-SERVED-UI-MAP.md](../../../docs/SEMANTIC-SERVED-UI-MAP.md). Action states: [V1-UI-ACTION-STATE-LEDGER.md](../../../docs/V1-UI-ACTION-STATE-LEDGER.md).

## Target flow and state distinctions

1. Discover catalog availability, host compatibility, package installation and authentication state are distinct observations. Selecting a package never creates a Connection or grants capability authority.
2. The owner chooses the one supported provider and completes public OAuth/PKCE. The UI shows requested scopes and destination account hint without claiming authenticated identity from an account-display string alone.
3. Connection health/status is displayed separately from capability assignment and grants. Connection revoke/reauth does not delete historical Invocations/Effects.
4. The owner assigns a compatible capability and grants exact read/write scope. The selected native agent sees only an opaque capability reference, never token bytes.
5. A read operation displays its source/receipt. A consequential write displays the exact target/action digest, operation class, scopes, approval expiry and reconciliation semantics before approval.
6. Needs You approval confirms the exact action digest. Approve is a decision state, not a dispatch. The user-visible outcome changes only after the mediated Invocation/Effect is reconciled and any required independent verification is committed.
7. Revoke immediately fences later invocation attempts; offline/expired/unknown/ambiguous states remain truthful and block new actions.

## Current implementation crosswalk (not the target)

- DiscoverPage currently provides Explore, Installed/native, Managed and Connections tabs. Explore lists Workspace-local skills and says public package discovery is unavailable. Installed/native shows only bounded Runtime-reported capability hosts, not a complete native inventory.
- Managed explicitly says LiteSPM install/adopt/repair/update/unmanage flows are not qualified. Connections explicitly says account Connections cannot be read. No account sign-in or Connection details surface is rendered.
- Discover row action buttons are disabled: `connectActionOutcome()` returns `allowed=false` even for otherwise available rows. Workspace-local skill Open/Run is unavailable, and package Connect/Use is unavailable pending LiteSPM contract qualification.
- The backend connect-or-install route returns 503; its test route returns UNKNOWN without contacting a provider. UI does not simulate a successful connection or grant.
- Needs You ApprovalDetail is real: it loads an owner-scoped Approval, shows exact action/target/risk/expiry, requires exact digest on APPROVE, preserves the same idempotent decision request after uncertain response and refreshes state. Its copy correctly says approval does not execute the action or create a reusable grant.
- No connected account list/detail, OAuth start/callback, scope review, connection revoke, provider read/write picker, CapabilityGrant authoring for a connector, or actual Invocation dispatch/reconciliation UI exists.

## Required rendered regression cases after implementation

- Cancelled OAuth leaves no Connection; callback state/PKCE replay fails; token bytes never enter React state, event payloads, logs or agent context.
- Connection token custody commit failure purges newly written secrets and does not show CONNECTED; refresh stampede serializes; invalid_grant becomes REAUTH_REQUIRED; revocation fences refresh and invocation.
- Catalog listing, package install, Connection status, capability assignment, grant and Invocation are shown as separate states.
- Provider not qualified, missing scopes, expired Connection, revoked grant, stale approval, unavailable Runtime and denied Trust each disable the relevant action with a stable reason.
- APPROVE with stale digest/version is rejected; an exact decision replay is idempotent; approval alone does not show the external action as completed.
- Timeout after dispatch becomes AMBIGUOUS; no retry occurs until provider/client idempotency and exact-target reconciliation prove the basis.
- Revoking the Connection blocks later read/write calls while preserving historical Approval, Invocation, Effect and Evidence history.

## Validation

Source-level tests or a mounted Approval route do not prove a live connector. Qualification requires the chosen provider's real auth, read, consequential write, exact approval, reconciliation, revoke and restart USER path on the declared OS.