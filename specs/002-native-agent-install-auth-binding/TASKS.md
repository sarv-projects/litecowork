[Reading 58 lines from start (total: 58 lines, 0 remaining)]

# Tasks - Native Agent Install, Auth and Binding

**Feature:** 002 | **Status:** DRAFT - owner decisions and contract promotion required before implementation.
**Spec revision:** 0.3

## 002-T01 - Lock the first exact agent route
- **Status:** BLOCKED
- **Depends on:** NONE
- **Owner:** Product and Agent Runtime
- **Allowed paths:** this package and canonical AGENT-CONTROL owner docs after decision
- **Contract:** SPEC §5, §7, §15
- **Change intent:** Decide first GA OS/harness/version/auth tuple, descriptor TTL/probe budget, lifecycle installer ownership and later agents.
- **Acceptance:** AC-001–AC-007; owner records the exact tested executable, OS/Runtime, auth and session semantics. No catalog-wide claim from one route.
- **Stop:** no provider/session qualification from metadata or profile-only probe.
**Command:** `python .specify/scripts/validate_feature_package.py specs/002-native-agent-install-auth-binding`

## 002-T02 - Catalog and installation observations
- **Status:** PARTIAL SOURCE / NOT RELEASE QUALIFIED
- **Depends on:** T01
- **Owner:** Agent Runtime
- **Allowed paths:** registry/lifecycle service, OpenAPI, schemas, tests
- **Contract:** docs/V1-AGENT-DISCOVERY-AND-CATALOG-LLD.md and AGENT-CONTROL
- **Change intent:** Keep distribution metadata separate from module authority; complete bounded Runtime-local detection and trusted install preview/receipt/reconcile.
- **Acceptance:** AC-001, AC-002 and bounded discovery test matrix; exact candidate path identity, source/license/digest, OS/arch and Runtime incarnation; preserve old working version; no arbitrary shell command.
- **Current gap:** Registry and inventory observations exist; install/update/repair handlers do not. ACP metadata is read-only.
- **Stop:** no installation action until exact module-owned installer and signature/source policy are qualified.
**Command:** `python .specify/scripts/validate_feature_package.py specs/002-native-agent-install-auth-binding`

## 002-T03 - Native profile/auth/config and descriptor
- **Status:** PARTIAL SOURCE / NOT RELEASE QUALIFIED
- **Depends on:** T01–T02
- **Owner:** Agent Runtime and Security/Trust
- **Allowed paths:** AgentModule adapter, credential broker, descriptor service, storage/tests
- **Contract:** AGENT-CONTROL, SECURITY, SPEC AC-003/004/006
- **Change intent:** Implement native auth observations and secure slots, exact descriptor producer, native config target allowlist and readiness handshake.
- **Acceptance:** AC-003, AC-004, AC-006; secrets never enter WebView/logs/events; cancelled auth preserves previous state; stale descriptor blocks new binding admission.
- **Current gap:** Control-descriptor and credential routes return 503; native auth-begin and native-config open target-only.
- **Stop:** no SecretStore mutation absent exact replay-safe credential contract.
**Command:** `python .specify/scripts/validate_feature_package.py specs/002-native-agent-install-auth-binding`

## 002-T04 - Workspace binding and native Conversation route
- **Status:** PARTIAL SOURCE / NOT RELEASE QUALIFIED
- **Depends on:** T01–T03 and Feature 001
- **Owner:** Agent Runtime, Conversation Runtime, Storage
- **Allowed paths:** binding/admission and exact adapter tests
- **Contract:** AGENT-CONTROL, Feature 001, STATE-MACHINES
- **Change intent:** Make binding enablement require fresh descriptor and qualify the exact end-to-end Send route.
- **Acceptance:** AC-004/005/006; one native user journey including install/detect, auth, descriptor, binding, Send, stream, stop, retry and restart.
- **Current gap:** Codex binding is not dispatched by current supervisor; OpenCode profile offer is intentionally incompatible. Current Conversation execution is not generally agent-ready.
- **Stop:** a profile or enabled binding alone never enables Send.
**Command:** `python .specify/scripts/validate_feature_package.py specs/002-native-agent-install-auth-binding`

## 002-T05 - Final Agent Settings UI and profile details
- **Status:** NOT STARTED
- **Depends on:** T01–T04 and Feature 011
- **Owner:** Desktop UI
- **Allowed paths:** agent catalog/profile components and tests
- **Contract:** SPEC V1 UI lifecycle contract; contracts/ui-behavior.md
- **Change intent:** Deliver catalogue, installed list, detail sections, independent chips, real actions and honest unavailable states.
- **Acceptance:** all listed control states, responsive list/detail navigation, keyboard/screen-reader tests, state-race tests, actual receipt/observation binding.
- **Current gap:** Current UI is catalogue/probe/binding screen, not complete install/auth/profile experience.
- **Stop:** no fake Install/Sign in/Test/Ready, and no dead action.

[executed on device: DESKTOP-SARV (a709d51c-bcd7-4e97-a12f-b3b065c93231)]
**Command:** `python .specify/scripts/validate_feature_package.py specs/002-native-agent-install-auth-binding`

[executed on device: DESKTOP-SARV (a709d51c-bcd7-4e97-a12f-b3b065c93231)]

## 002-T06 - Descriptor-bound model and effort picker

- **Status:** BLOCKED / AFTER EXACT NATIVE ROUTE
- **Depends on:** T01–T04, Feature 001 exact provider route, and Feature 011 Settings/composer shell
- **Owner:** Agent Runtime, Desktop UI
- **Allowed paths:** AgentControlDescriptor producer/validator, model option projection, profile/composer picker and tests
- **Contract:** AGENT-CONTROL.md §14.4, V1-AGENT-DISCOVERY-AND-CATALOG-LLD.md, contracts/ui-behavior.md
- **Change intent:** Expose a compact searchable model/effort picker scoped to the exact agent profile/account and current descriptor revision.
- **Acceptance:** model/options are adapter-declared; no cross-provider mixing; stale descriptor rejects selection and preserves draft; provider model ID plus descriptor revision is revalidated on admission; changes affect future admissions only; a fully qualified native Conversation/Task path consumes the selected option.
- **Command:** package validator plus exact AgentControlDescriptor and native Conversation/Task integration tests after the route is qualified.
- **Stop:** no global router, inferred effort values, model selection without a compatible dispatcher, or silent native config mutation.
