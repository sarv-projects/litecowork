# Licensing and Packaging Research

Status: **Deferred. Do not make or implement a licensing/packaging decision yet.**

Decision point: revisit this document only after LiteCowork V1 has been developed,
implemented, tested, and is ready for beta release. Until then, continue building against
the current architecture and repository boundaries. This note records options and claims
to investigate; it is not legal advice, an approved license, a packaging design, or a
product commitment.

The filename preserves the spelling requested in the project conversation. The intended
topic is licensing and packaging research.

## Research question

How should LiteCowork distribute its desktop application, agent adapters, local Runtime,
cloud services, and future enterprise features so that developers can inspect and adopt
the product while the project can sustain commercial development?

Any eventual decision must account for the product's actual architecture, dependency
licenses, distribution model, provider terms, contributor policy, and the trust users need
to give a local agent runtime access to their files and connected services.

## Proposal recorded for later evaluation

### 1. Source-available or non-permissive licensing

The proposal recommends avoiding standard permissive licenses such as MIT or Apache-2.0
because others could commercialize the code. It suggests evaluating source-available or
“fair-code” models, including:

- **Business Source License (BSL/BUSL):** the proposal describes public, inspectable source
  with free individual, testing, or non-commercial local use and a commercial threshold
  such as organization seat count. It also describes a change date after which each
  version converts to a specified open-source license, for example MIT or Apache-2.0.
- **Elastic License 2.0 (ELv2) or PolyForm Shield:** the proposal describes allowing
  inspection, modification, and local use while restricting offering the software as a
  managed service and removing copyright notices.

The proposal names MariaDB, CockroachDB, and HashiCorp as examples associated with BSL.
Those examples, current license versions, exact permissions/restrictions, conversion
terms, compatibility, and suitability for LiteCowork have **not been researched or
verified**. “Free,” “open source,” “source available,” and “non-commercial” must not be
used interchangeably in user-facing license descriptions.

Research before deciding:

- Compare the exact current BSL/BUSL, ELv2, and PolyForm Shield texts and their permitted
  use, restricted use, change dates, change licenses, and enforcement requirements.
- Decide whether a seat-based commercial restriction is understandable, enforceable, and
  compatible with the intended individual, company, and hosted-use scenarios.
- Audit all direct and transitive dependency licenses, generated assets, bundled agent
  tools, and contribution terms against each candidate license.
- Determine whether a contributor license agreement, developer certificate of origin,
  trademark policy, or separate commercial agreement would be needed.
- Obtain qualified legal review before publishing or changing a license.

### Suggested public/private boundary to evaluate

The proposal suggests a source-available/open-source distribution repository for the
desktop shell, UI, public CLI, and agent adapters, with a separately licensed private
core for context compression, state reconciliation, rollback, verification receipts,
and memory-graph logic. It further suggests placing these in separate
`litecowork-harness/` and `litecowork-core/` directories or repositories, connected over
local IPC/RPC.

This is a proposal to assess, **not an approved architecture or repository split**. It
conflicts with the current architecture if it moves durable Task state, authorization,
Effects, Evidence, verification, or recovery outside the documented LiteCowork owner
boundary. A compiled Rust/Go process and an IPC boundary do not by themselves protect
trade secrets or make the system secure. Before considering a split, evaluate whether
it creates a real operational or commercial benefit, the maintenance/API burden, local
tampering and threat-model implications, user trust and inspectability, and whether the
same product can be packaged from one repository without hiding or duplicating Core
responsibilities.

### 2. Local application with optional cloud value

The proposal recommends keeping local execution useful while offering optional paid cloud
features, including:

- encrypted synchronization/relay between a user's devices;
- hosted agent/model access with billing, context caching, and rate-limit management;
- managed delivery, security/compliance operations, and enterprise capabilities.

The proposal's suggested business moat is that a fork can copy local UI code but would
need to build and operate comparable hosted infrastructure to provide the cloud service.
This is a **business hypothesis**, not a proven moat or a reason to weaken local
functionality.

Evaluate later:

- Which cloud capabilities provide durable user value and are expensive to operate well?
- Can encrypted sync meet the documented zero-knowledge/privacy claim, and what metadata
  would remain visible to the relay?
- What are the actual costs, provider terms, quota constraints, support obligations, and
  margins for hosted agent/model billing?
- Which features belong in local V1, optional cloud plans, or later enterprise plans?
- How can the local product remain useful without a subscription or network connection?

### 3. Community, distribution, and execution speed

The proposal argues that community, developer distribution, reputation, and founder
execution speed may be more defensible than code alone. It suggests building publicly,
encouraging community participation, and responding quickly to user feedback. It also
claims that a copied product would face a “clone penalty” and that a solo founder can ship
faster than larger companies.

Treat these as hypotheses. GitHub stars, community size, contributor activity, user
retention, release cadence, and support load should be measured rather than assumed. A
license is not a substitute for product quality, trusted security practices, sustainable
maintenance, or a real distribution channel. The source conversation's specific claims
of reaching 5,000 GitHub stars, building an active Discord, shipping “10x” faster, and
clones being distrusted by developers are unverified examples, not forecasts or launch
targets. Evaluate community investment against actual adoption, retention, contribution,
support, and distribution evidence.

## Architecture boundary to preserve while this is deferred

Do not move LiteCowork's durable Task truth into an opaque engine solely to create a
licensing boundary. The current architecture assigns LiteCowork responsibility for
durable Tasks, coordination, mediated Trust, Effects, Evidence, verification, and recovery;
external agents retain their native harnesses and private state. Packaging/licensing may
change how these pieces are distributed, but any future proposal must preserve the
documented ownership and authority invariants or explicitly revise them through the normal
architecture process.

In particular:

- Do not claim LiteCowork can intercept or roll back every native-agent shell command or
  external side effect. Only mediated or equivalently enforced operations can receive the
  corresponding LiteCowork authorization, audit, idempotency, and reconciliation
  guarantees.
- Do not treat a private binary as a security guarantee. The local Runtime handles
  sensitive workspace data and authority; closed-source distribution has trust,
  inspectability, support, and threat-review tradeoffs.
- Do not use “open-core” as a reason to split repositories or duplicate the local
  execution engine before a measured product or operational need exists.
- Keep public protocols, schemas, and adapter extension points stable and documented if
  community adapters are part of the eventual distribution strategy.

## Handoff and desktop stack assumptions

The referenced conversation asks how a switch from a stateless cloud agent such as Codex
to a local-first agent such as OpenCode should preserve progress. LiteCowork's current
architecture answer is:

1. Durable Task, Plan/Steps, committed artifacts, Evidence, decisions, and unresolved
   Effects remain the source of truth.
2. Reconcile unresolved Effects and fence the old lease before replacement work is
   admitted.
3. Create a bounded handoff projection from durable state; do not copy hidden reasoning,
   private native transcripts, or provider session handles.
4. Start a new native AgentSession/Attempt on the selected Runtime. A live process or
   provider session is not assumed to migrate between agents or devices.

The current implementation stack is Tauri with a Rust desktop/native layer and React with
TypeScript for the UI, backed by the `litecoworkd` local Runtime. Reevaluate this only if V1
implementation evidence identifies a concrete limitation.

## Sources and claims to verify later

The source material that prompted this note supplied these starting references:

- Claude Code repository: <https://github.com/anthropics/claude-code>
- OpenCode product site: <https://opencode.ai>
- OpenCode repository link supplied in the conversation: <https://github.com/opencode-ai/opencode>
- Eigent comparison article: <https://www.eigent.ai/blog/claude-cowork-vs-codex>
- Substack comparison article: <https://ramosmarcs.substack.com/p/choosing-between-cowork-or-codex>
- CoddyKit/OpenWork article: <https://www.coddykit.com/pages/blog-detail?id=512990&slug=openwork-the-open-source-alternative-to-claude-cowork-with-20-000-github-stars>
- BSL overview: <https://mariadb.com/bsl11/>
- Elastic License 2.0: <https://www.elastic.co/licensing/elastic-license>
- PolyForm Shield: <https://polyformproject.org/licenses/shield/1.0.0/>

These links are research leads from the supplied conversation, not endorsements or
verification that a source is current, authoritative, or accurately represented. Recheck
official license texts and primary project/company sources at the decision point. Treat
market-size, adoption, star-count, “clone penalty,” and competitor capability claims as
time-sensitive and independently verify them.

## Decision checklist for the V1 beta review

Do not close this question until the owner has reviewed:

- intended users and permitted commercial/non-commercial use cases;
- exact license choice and version, including any change date and change license;
- dependency and asset license compatibility;
- repository and package split, if any, with a concrete maintenance reason;
- open versus commercial feature boundary without moving durable Task/Trust truth out of
  its documented owner;
- local-only usability and optional cloud service economics;
- contribution, trademark, security disclosure, and support policies;
- legal review and beta-user comprehension of the license terms.

Until that review, implementation should continue without changing the repository license,
creating a private-core split, or promising a particular commercial model.
