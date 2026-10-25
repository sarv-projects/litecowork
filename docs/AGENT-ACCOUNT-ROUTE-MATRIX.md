# Agent and Account Route Qualification Matrix

**Status:** Normative V1 contract. This document defines which agent/model/account routes LiteCowork may present and launch; it does not grant access to any model or account.

## Product rule

“Use all models” means: expose every model that is actually selectable through a supported, qualified AgentBinding and the user's own eligible authentication route. It does not mean a universal model router, subscription proxy, credential transfer, or a promise that every vendor model can be used through every harness.

The composer selects **Agent → that agent's model/reasoning/session options**. A model name is meaningful only inside its agent/provider route. Switching agents starts or resumes that agent's own session according to its adapter; it never silently replays private native history or changes the native agent's provider settings.

Direct Conversation turns can use the selected lead's native subagents only. LiteCowork-hosted workers from other AgentBindings require an admitted durable Task with a planned Step/Attempt; the user explicitly chooses Run as Task or confirms a Task proposal. A plain Send never creates hidden cross-agent worker work. This keeps child model/auth routing, leases, grants and results under the Task contracts.

## Route classes

| Route | V1 treatment | Account and execution owner |
|---|---|---|
| User-selected native lead agent | Supported only for a qualified AgentModule, current control descriptor, eligible OS/runtime, and usable native auth | Native agent and its provider account |
| Native subagent of the lead | Agent-owned behavior; use only when the harness supports it. Child visibility, model control, limits and cancellation are reported at the adapter's actual guarantee level | Selected native harness |
| LiteCowork-hosted worker on another AgentBinding | Supported only after a separate worker profile is configured and the worker route independently passes admission | Each worker's own harness, model configuration, auth and provider entitlement |
| Provider API route configured inside a supported agent | Supported only through that agent's documented setup and the provider's allowed route | Native agent configuration and the configured provider |
| Vendor-hosted task/service | Conditional: only a published, qualified provider integration with an explicit data, auth, terms, result, cancellation and recovery contract. It is not implied by local desktop V1 | Vendor-hosted agent/provider; LiteCowork owns only its mapped Task integration |
| Arbitrary subscription-cookie reuse, credential copying, account pooling, hidden proxying, or terms-unclear route | Unsupported; fail closed and explain how to configure a supported route | No LiteCowork route |

A local desktop UI may launch an agent that itself communicates with a vendor cloud. That is different from LiteCowork's own cloud Runtime or cross-device continuation. The latter remain post‑V1.

## Route qualification record

Every route is qualified by the exact tuple:

(AgentModule revision, harness/version, OS/architecture, provider route, auth method, model descriptor revision, feature set, account/plan eligibility, region, execution location)

The qualification record also includes:

- output delivery modes and exact output types the selected route can return; an agent/model catalog entry does not prove that Artifact type can be delivered;
- primary evidence URL/document, terms or policy clause, reviewer, review date, and recheck date;
- who authenticates, where the secret is stored, and which process can read it;
- provider and LiteCowork data handling, retention, training controls, and region where established;
- model IDs and supported inputs/tools/options, not just display names;
- rate limits, plan quotas, concurrency, cost source and whether estimates are preflight or post-run;
- interactive/background permission, local-filesystem or browser access, OS dependencies and required user-visible approvals;
- session resume, streaming, steering, cancellation, timeout, crash recovery, result/evidence, and redaction guarantees;
- unsupported cases and a user-readable reason.

Status is QUALIFIED, CONDITIONAL, STALE, REVOKED, or UNQUALIFIED. Only QUALIFIED routes are offered as ready. A conditional route must name its unmet condition and cannot be launched as if ready. A stale or revoked descriptor blocks new admission; already admitted Tasks retain their pinned evidence and settle under ordinary cancellation/reconciliation policy.

A route is not qualified merely because a model appears in a web catalog, a LiteSPM listing, a native config file, or a vendor marketing page. Do not scrape UI output or infer endpoint/auth semantics.

## Output capability and interactive apps

Output support is qualified independently from model selection, input support, and agent branding. `output.semantic_text`, `output.file_artifact`, `output.rich_presentation`, and `output.interactive_app` are separate descriptor observations. A route advertises an output as routable only when its exact adapter/version reports SUPPORTED; UNKNOWN, NOT_EXPOSED and missing values are not routable. UNSUPPORTED is an explicit negative observation. These flags describe delivery, not content quality, safety, correctness, entitlement, or execution authority.

An interactive app candidate uses the exact selected AgentBinding, model/options, account route, and session/Task policy already chosen for the request. LiteCowork never switches to another agent or model to make an app. A candidate must pass the versioned package validator and ArtifactStore publication path before the Workbench offers it. The semantic answer remains independently useful, and package/validation failure falls back to ordinary text/source output. The app is inert until its owner explicitly selects Run in the Workbench; artifact arrival never auto-runs code. If the descriptor does not include `output.interactive_app`, the UI does not promise app delivery and does not infer it from file or rich-presentation support. See [Interactive Artifact Runtime](INTERACTIVE-ARTIFACT-RUNTIME.md).

## Model selection and delegation

- The admitted Conversation turn or Task pins its selected AgentBinding, exact selected model/options, the descriptor digest, and the route-qualification evidence revision used at admission.
- Native child model policy is separate from routing policy: AGENT_DEFAULT or INHERIT_PARENT_IF_SUPPORTED. The adapter descriptor reports whether the harness supports parent inheritance and how it reports child models. “Inherit parent” means the native harness receives the exact parent model only when it advertises and enforces that behavior. Otherwise the choice is unavailable; no silent fallback is allowed.
- A host-delegated worker uses its own pinned DelegationProfile revision, AgentBinding, model/options and authentication. Its default is the profile's explicit agent-owned selection. If the owner requests a parent-model match, the exact model/provider identity must be supported by that worker route and separately authorized by the worker's own configured account; otherwise admission fails with a typed unsupported/ineligible reason.
- Never pass parent cookies, refresh tokens, API keys, native session handles, subscription entitlements or hidden provider settings to a child agent.
- A LiteCowork native child remains opaque harness-owned activity. Only a host-delegated worker becomes a LiteCowork child Attempt and receives a scoped lease/grant set.
- Native model reporting may be EXACT, AGENT_REPORTED, UNKNOWN, or NOT_EXPOSED. UI and usage ledgers preserve that provenance and never claim exact cost when only estimated/reported usage exists.

## Admission, switching and failure behavior

Admission rechecks descriptor freshness, OS/process availability, user auth state, account/plan eligibility where observable, model compatibility, quota/concurrency, worker allowlist, trust/grant scope, location and budget. If the selected route fails, preserve the draft and show the exact blocker. Do not silently switch agent, model, account, region, execution location or subscription route. User-approved retry may use another route only after a new preview describes the differences and creates the appropriate new session/Attempt.

On a provider/model switch within a conversation, the adapter determines whether the change is live or requires a new AgentSession. The UI shows the boundary and what context is sent. Native private session state remains with its agent; only user-selected Conversation content, authorized Resources and explicit Task checkpoints may cross the boundary.

## User-visible status

Show the selected agent and its model together, with a route/account indicator when it affects eligibility or data handling. Distinguish:

- installed vs discovered vs bound vs authenticated vs ready;
- model catalog vs usable model on this route;
- native tool/subagent activity vs LiteCowork-hosted worker activity;
- exact vs agent-reported vs estimated usage;
- local Agent execution that calls a vendor service vs LiteCowork cloud continuation.

Settings → Agents owns native setup/auth and route readiness. Settings → Subagents owns eligible worker profiles and delegation defaults. LiteSPM owns package discovery/lifecycle only; it does not qualify an AgentBinding, account, model route, or delegated worker.

## Release gate

Before offering a route, require automated descriptor conformance plus human-reviewed route evidence. Re-review after terms/policy, product plan, provider API, authentication, OS, feature, region or data-handling changes. Unknown answers stay unknown and block only the dependent route; they must not be filled by assumption.


## Vendor account routes rechecked — 2026-10-25

This is route qualification evidence, not a grant of access or an inference that an account, plan, or model is eligible.

### OpenAI Sign in with ChatGPT

OpenAI now documents Sign in with ChatGPT (SIWC) as a supported way for an application to let users connect eligible ChatGPT plans, subject to the SIWC Terms. Requests must originate from the user's local runtime or a remote runtime the user controls; requests must arise from user activity or an expressly authorized background process, with express consent before background use. SIWC usage is limited to the connected application, cannot be exposed as a general-purpose API, carries plan eligibility/usage limits, and cannot be pooled, transferred, resold, or used for another user's requests. SIWC authentication tokens must come from the supported flow; persistent storage must remain local and under user control. The terms also require the connected service to be usable without charging the user or requiring a paid LiteCowork tier.

Therefore, a native Codex installation or a catalog entry is not a blanket authorization to route arbitrary LiteCowork requests through a ChatGPT subscription. Any SIWC path must be an explicit, separately qualified product route that meets the current SIWC Terms, consent, disclosure, security, plan, and quota rules. Do not ask for ChatGPT passwords or cookies.

### Anthropic Claude Code and Agent SDK

Anthropic distinguishes embedding its Agent SDK from installing and running the published Claude Code harness. Developers building Claude integrations, including Agent SDK products, should use API-key authentication through Claude Console or a supported cloud provider; third-party apps must not offer Claude.ai login through the SDK or collect, store, or intermediate Claude.ai credentials/session tokens. Separately, Anthropic says a product may install and run the unmodified Claude Code binary if the developer agrees to its Commercial Terms and follows its conditions; the end user must authenticate with their own eligible Anthropic API key, Claude plan credentials, or supported third-party inference credential, and usage must be billed under that user's own agreement. Do not modify the binary, remove its authentication methods, or pay for, resell, or intermediate users' Claude usage.

LiteCowork must qualify these as distinct route types. “Use Claude Code as Claude Code” means launching the unmodified user-authenticated harness under the applicable commercial terms; it does not authorize embedding subscription OAuth in a LiteCowork-owned SDK client or forwarding subscription credentials to other agents.

### Sources checked

- OpenAI, [Sign in with ChatGPT Terms](https://openai.com/policies/sign-in-with-chatgpt-terms/) (published 2026-09-29), and [Service Terms](https://openai.com/policies/service-terms/).
- Anthropic, [Claude Code legal and compliance](https://code.claude.com/docs/en/legal-and-compliance) and [Agent SDK overview](https://code.claude.com/docs/en/agent-sdk/overview), checked 2026-10-25.
