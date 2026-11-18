# LiteSPM pre-launch product, website, UX, and engineering audit

**Audit date:** 2026-11-18  
**Target site:** https://lite-spm-nine.vercel.app/  
**Target source:** https://github.com/sarv-projects/LiteSPM  
**Source revision reviewed:** f593c89044009eec0952ab6286413219f44caec3  
**Verdict:** **NOT READY for broad launch**

## Executive summary

LiteSPM has a clear developer problem to solve: people configure MCP servers, skills, and plugins separately for each agent host. The website communicates a compelling local workflow—preview changes, preserve prior files, and undo them—and the catalog pages expose publisher, source, installation state, and relevant permissions. The docs also explain approval, backups, local credential handling, and recovery. These are meaningful strengths.

The launch decision is held by four issues:

1. The latest public source revision has a failing GitHub Actions run. The static-check job and Windows npm job both fail the npm package contents assertion because the package currently includes test_npm.js while the test expects it to be absent. That run also skipped release-fixture tests and reported four moderate npm dependency advisories. [Latest CI run](https://github.com/sarv-projects/LiteSPM/actions/runs/44139044863)
2. The default litespm sync path writes detected agent config files without a preview or an approval prompt. It is explicitly described as fully automatic and defaults to yes. That conflicts with the site's blanket statement that nothing is written without preview and with the documented human approval flow. [sync.go at audited revision](https://github.com/sarv-projects/LiteSPM/blob/f593c89044009eec0952ab6286413219f44caec3/cmd/litespm/sync.go#L84)
3. The site and source promote an in-agent marketplace path associated with 0.6.1, while npm currently serves 0.6.0. The docs disclose this difference, but a visitor can still encounter the in-agent activation promise before seeing the release caveat. [Agent compatibility docs](https://lite-spm-nine.vercel.app/docs/agent-compatibility), [npm package](https://www.npmjs.com/package/litespm)
4. Follow-up checks improved the evidence but did not qualify the full journey. The homepage opened in the in-app browser and its desktop first fold was visually reviewed; deeper page navigation and DOM inspection timed out. A checksum-verified disposable Ubuntu WSL distro successfully installed the published npm package and launched the Linux CLI. No real agent host or authenticated session was present, and WSL does not establish native Windows/macOS or desktop behavior. Approval/denial, real host mutation, rollback, mobile layout, and accessibility remain unverified.

These issues are fixable. Until they are addressed and the exact released package passes a native first-value test, the site should be treated as a preview rather than a launch-ready acquisition surface.

## Scope and method

The full user-provided audit brief was read. Work followed its user-first order: inspect the public product and route behavior first, then inspect the matching public source, current release signals, competition, trust/security claims, and available QA evidence. No product source was edited and no changes were made to the public site.

The audit target is the public LiteSPM project, not the LiteCowork checkout that hosts this report. The LiteCowork checkout already contained extensive unrelated changes and audit data. Those files and existing audit-output entries were preserved. Only the two new LiteSPM audit artifacts named at the end of this report were added. The public site does not expose its deployed source SHA, so the exact relationship between the live site build and the public main revision reviewed here could not be independently confirmed.

### Evidence labels used here

- **LIVE:** Read-only HTTP response from the public site or public registry.
- **SOURCE:** Static inspection of the public LiteSPM source at the SHA above.
- **CI:** GitHub Actions output for that exact source revision.
- **INFERENCE:** A reasoned product or audience conclusion, not a tested user outcome.
- **UNVERIFIED:** The required browser, installed application, or real agent behavior was not exercised.

### Coverage status

| Audit area | Status | Evidence |
|---|---|---|
| Full audit brief intake | Complete | Full pasted brief read before assessment |
| First impression and audience framing | Partial | Homepage SSR text and page structure; persona responses are inferences |
| Public route inventory | Complete for sitemap | All 571 sitemap URLs fetched |
| Search and not-found behavior | Partial, read-only | Search API, zero-results query, unknown package and unknown route |
| Browser, mobile, keyboard, assistive technology | Partial | In-app browser opened the homepage and a desktop first-fold screenshot was reviewed; deeper DOM/navigation timed out; mobile and accessibility remain unverified |
| Product source review | Partial | Public main revision and selected Go, Next.js, npm, catalog, and workflow files |
| Release/CI review | Complete for latest run reviewed | Failed npm packaging assertion, skipped checks, other job statuses |
| Published CLI install and host workflow | Partial | Published 0.6.0 npm postinstall and Linux CLI smoke passed in a fresh checksum-verified WSL distro; no real agent host, desktop keyring, or native Windows/macOS evidence |
| Competitor scan | Partial | Official competitor sites and repositories; no customer interviews or paid-market data |
| External package/bundle provenance | Partial | Full bundle manifest and structural checker inspected; individual upstream packages not independently audited |

## Product and website readout

### What the product appears to be

LiteSPM is a local CLI and discovery website for installing MCP servers, skills, and plugins across multiple AI coding-agent hosts. The homepage claims 52 agent hosts, with 47 remote-capable, and describes a marketplace of more than 69,000 listings. The live search API returned 69,311 unique listings and 380 results for a PostgreSQL query. The website also advertises 81 skill targets.

The strongest product promise is not simply “more AI tools.” It is that a developer can discover a package once, see where and how it will be applied, preserve the previous host config, and undo LiteSPM-managed changes. The homepage's “preview, backup, and byte-exact undo” story is concrete and memorable. [Homepage](https://lite-spm-nine.vercel.app/), [install flow](https://lite-spm-nine.vercel.app/docs/install-flow)

### First-impression simulations

These are content-level inferences based on the server-rendered page. They are not observed interviews or tested user sessions.

| Audience | Likely reaction | Friction to address |
|---|---|---|
| AI-curious, nontechnical social visitor | The headline “Install AI tools” sounds broad, but MCP, npm, coding-agent hosts, and terminal commands quickly establish a developer product. | Say early that LiteSPM is for people using coding-agent CLIs/IDEs and requires a terminal and a supported Node version. Avoid attracting people who expect a consumer AI app. |
| Beginner vibe coder | A single catalog and an undo promise sound useful, and curated bundles offer a path to a working set. | The first visible workflow is still a shell install; Node 22.22.2+ is not prominent in the hero. A short real install/undo demo would reduce uncertainty. |
| Intermediate developer | Cross-host config normalization, clear installability, source links, and backup/undo are relevant benefits. | Show one end-to-end example with actual diff, exact affected host file, and post-install verification. Clarify which features work in the currently published version. |
| Senior or security-conscious developer | Publisher/source visibility and local credential claims are promising. | The automatic sync command and preview/approval language currently conflict. An explicit explanation of trust boundaries and the limits of package review is needed. |
| Package maintainer | Publishing docs and a catalog path are visible and useful. | The publish form and its error/empty/success states were not exercised in a browser. |
| Mobile visitor, screen-reader user, keyboard-only visitor | No conclusion is possible from this audit. | Complete a real browser accessibility and responsive pass before launch. |

### Main funnel

The homepage presents marketplace browsing and a quickstart, then the marketplace provides search and filters. Package detail pages expose install commands and package context. This is a sensible browse-to-install sequence. Bundles offer curated starting points, and publishing documentation addresses supply-side growth.

The most important funnel break is version truth: the in-agent marketplace is described in a way that can be read as available now, while the released npm version is still 0.6.0 and the in-agent feature is associated with 0.6.1. Keep the path clearly labeled as preview/upcoming until the version actually available to visitors contains it, or publish and qualify the release before promoting it.

## Live website and route results

### Availability and crawl

The sitemap contained 571 URLs. All 571 returned HTTP 200 at audit time. Every URL had a title, description, and canonical reference. The route inventory breaks down as:

| Route group | Count |
|---|---:|
| Package details | 500 |
| Agent details | 52 |
| Docs | 12 |
| Other top-level pages | 7 |
| **Total** | **571** |

Read-only functional probes also returned expected behavior:

| Probe | Result |
|---|---|
| Homepage | HTTP 200 |
| Search API with PostgreSQL query | HTTP 200; 380 matches reported |
| Search API with a unique no-match query | HTTP 200; zero results |
| Nonexistent package route | HTTP 404 |
| Unknown site route | HTTP 404 |

This establishes that key public endpoints respond. It does not establish that the browser UI hydrates correctly, that all filters work, or that a real install succeeds.

### SEO, sharing, and crawlability

The site has good baseline SEO coverage: titles, descriptions, canonical links, robots.txt, and a sitemap are present. The sitemap currently exposes the top 500 ranked installable package routes out of 69,311 catalog listings—about 0.72% of the catalog. This may be a deliberate quality cap, but document the rationale and a path for quality-controlled expansion.

The route scan found:

- 53 URLs without a server-rendered H1: /insights and all 52 agent detail URLs.
- Only 1 of 571 pages has an Open Graph image.
- 69 exact-title groups cover 276 routes. Repeated examples include “Skill Creator — LiteSPM” on 20 routes, “Code Review — LiteSPM” on 17, and “Frontend Design — LiteSPM” on 15. These appear to be distinct catalog entries; include publisher or another distinguishing package identity in titles and social previews.
- All sitemap lastmod values are identical: 2026-11-05T09:09:20Z. Source derives that value from the catalog release date or a fixed fallback instead of per-page update dates. Revisit whether a lastmod should be emitted when the true page modification date is not known. [Sitemap source](https://github.com/sarv-projects/LiteSPM/blob/f593c89044009eec0952ab6286413219f44caec3/web/src/app/sitemap.ts#L23)
- The package detail “Link” control copies a legacy hash URL of the form /#/package/… while clean server-rendered package pages use /package/…. Share bots following the hash URL receive the homepage document and metadata. [Package detail source](https://github.com/sarv-projects/LiteSPM/blob/f593c89044009eec0952ab6286413219f44caec3/web/src/components/litespm/views/package-detail.tsx#L319)
- Per-page metadata sets Open Graph title and description, but not an image. Since the page metadata overrides the layout default, the live route scan shows no share image on the 570 deep routes. Add the generated image to route-level metadata or a suitable shared default. [Page metadata source](https://github.com/sarv-projects/LiteSPM/blob/f593c89044009eec0952ab6286413219f44caec3/web/src/app/%5B...slug%5D/page.tsx#L88)
- Package JSON-LD assigns every package a cross-platform Windows/macOS/Linux operating system and a zero-dollar offer. External packages can have host, platform, licensing, or service constraints. Only publish verified listing-specific values; otherwise omit these fields. [Structured data source](https://github.com/sarv-projects/LiteSPM/blob/f593c89044009eec0952ab6286413219f44caec3/web/src/app/%5B...slug%5D/page.tsx#L245)

### Security headers and trust content

The homepage response included HSTS, X-Content-Type-Options: nosniff, Referrer-Policy: strict-origin-when-cross-origin, X-Frame-Options: SAMEORIGIN, and a restrictive Permissions-Policy. The CSP is present but permits unsafe-inline and unsafe-eval; this is a hardening opportunity, not evidence of an exploitable issue. Access-Control-Allow-Origin is wildcard; confirm that public API responses are intended to be readable cross-origin and never contain credentialed or private data.

The site prominently says no account is required and tokens stay in an OS vault or environment references. Its security docs describe human approval and backups. The current source also has a signed-binary install design with a pinned signing identity. These are good trust signals. Their runtime behavior was not independently exercised here. [Security docs](https://lite-spm-nine.vercel.app/docs/security), [npm package source](https://github.com/sarv-projects/LiteSPM/tree/f593c89044009eec0952ab6286413219f44caec3/npm)

The public footer points to GitHub, npm, and issues, but no dedicated Privacy or Terms page was found in the inspected navigation. The site says it sends nothing to itself; make the data collection, telemetry, contact, and support position easy to find so that claim is auditable.

## Product behavior and engineering findings

### High priority: default sync mutates host configuration without approval

The sync help text calls the command fully automatic; argument parsing sets yes=true by default. When an existing agent config is found, the code plans setup and then immediately calls ApplySetup. No preview result or human approval gate is required on that path. This is a concrete mismatch with the site-wide message that nothing is written without preview and the install-flow documentation describing a human approve step. [sync.go: default options](https://github.com/sarv-projects/LiteSPM/blob/f593c89044009eec0952ab6286413219f44caec3/cmd/litespm/sync.go#L84), [sync.go: ApplySetup path](https://github.com/sarv-projects/LiteSPM/blob/f593c89044009eec0952ab6286413219f44caec3/cmd/litespm/sync.go#L337), [install flow promise](https://lite-spm-nine.vercel.app/docs/install-flow)

**Recommendation:** Make preview the default and require explicit opt-in for unattended writes. If fully automatic sync is retained, distinguish it prominently from the approval-based install flow, list exact host files and operations before confirmation, and ensure the default command cannot silently expand its scope to newly detected hosts.

### High priority: sync can report success after partial failures

The implementation records setup planning/application errors, but the caller only returns a nonzero exit code when executeSync itself returns an error. The routine returns a report and nil at its end. Skill copy errors are ignored; ledger.Add errors are discarded; the reconciled count is based on the number of source skills rather than successful destination copies; and the final text says all configurations and capabilities are in sync. A permission error or failed copy can therefore yield a successful exit and a green all-in-sync message despite incomplete state. This was established by source inspection, not by injecting a filesystem failure. [sync.go: exit handling](https://github.com/sarv-projects/LiteSPM/blob/f593c89044009eec0952ab6286413219f44caec3/cmd/litespm/sync.go#L121), [sync.go: ignored copy/ledger errors and success output](https://github.com/sarv-projects/LiteSPM/blob/f593c89044009eec0952ab6286413219f44caec3/cmd/litespm/sync.go#L403)

**Recommendation:** Return nonzero for any failed host or skill operation, preserve partial success explicitly in JSON and human output, count completed operations, surface ledger persistence failures, and add fault-injection tests for permission denied, mid-copy interruption, and restart/retry.

### High priority: current CI is red

At the latest reviewed main SHA, the GitHub Actions run failed in the static-check job and the Windows npm job. Both fail the “npm pack ships exactly the allowlist” assertion because test_npm.js appears in the actual package tarball. The source package.json includes test_npm.js in its files list, creating a direct mismatch between the package manifest and the test's intended allowlist. Resolve that contract deliberately and rerun the full workflow.

Additional CI evidence:

- The Windows npm wrapper suite ran but one package contents assertion failed; several real release fixture and network namespace tests were skipped.
- The static-check job ran Go formatting, vet, shell/Python checks, then failed in the same npm wrapper assertion.
- The Windows and Ubuntu Go engine jobs passed; the web application build passed.
- The macOS Go job was cancelled, so cross-platform green status is not demonstrated.
- npm install logged four moderate severity dependency advisories. The log does not provide enough detail to classify or accept them; retrieve the audit detail and triage each advisory before release.

These CI results are a release gate, not proof that the published 0.6.0 binary is broken. They are evidence that the current source revision is not green and its packaging/integration test contract is unresolved. [Workflow run](https://github.com/sarv-projects/LiteSPM/actions/runs/44139044863), [package.json at audited revision](https://github.com/sarv-projects/LiteSPM/blob/f593c89044009eec0952ab6286413219f44caec3/npm/package.json)

### High priority: source and published version differ

The latest reviewed source package declares 0.6.1, while the npm registry's latest package is 0.6.0. The docs themselves say the in-agent marketplace is a 0.6.1 feature and that npm serves 0.6.0. Keep the feature clearly labeled as upcoming or release 0.6.1 before presenting it as usable. Also update the versioned installation and compatibility guidance in one coordinated release.

The latest public GitHub release v0.6.0 has platform binary assets, SHA256 checksums, Sigstore bundles, and an SBOM. That is a positive release artifact pattern, but a local installation and signature verification were not run as part of this audit. [v0.6.0 release](https://github.com/sarv-projects/LiteSPM/releases/tag/v0.6.0), [npm package](https://www.npmjs.com/package/litespm), [compatibility documentation](https://lite-spm-nine.vercel.app/docs/agent-compatibility)

### Bundle experience and catalog quality

The source contains 21 curated bundles with 151 total package entries. The bundle checker resolves entries against the same catalog deduplication and installability rules used by the website; this is a strong stale-data guard. Examples include Full-stack web dev (7 items), Office documents (8), Browser testing and QA (6), Application security & vulnerability audit (8), and Programmatic Video & Motion UI (7). The complete manifest is in [bundles.json](https://github.com/sarv-projects/LiteSPM/blob/f593c89044009eec0952ab6286413219f44caec3/web/data/bundles.json), and the structural/installability gate is [check-bundles.mjs](https://github.com/sarv-projects/LiteSPM/blob/f593c89044009eec0952ab6286413219f44caec3/web/scripts/check-bundles.mjs).

The bundle picker is an editorial strength, but the data checker proves only catalog presence and installability. This audit did not independently validate all 151 upstream projects, licenses, maintenance status, permissions, or publisher trust. Bundles also copy multiple install commands because the CLI takes one ID per command; a 16-item bundle can require 16 separate install commands and their individual review steps. Make that item-by-item behavior clear and consider a single grouped plan/preview/approval workflow.

A specific onboarding defect is present: bundle details state Node.js 20+ while the current package engine requires Node 22.22.2+ or the stated newer supported lines. A beginner following the bundle text can install Node 20 and then fail the actual CLI prerequisite. Align the bundle requirement with the published npm package and verify the supported lines. [Bundle prerequisite source](https://github.com/sarv-projects/LiteSPM/blob/f593c89044009eec0952ab6286413219f44caec3/web/src/components/litespm/views/bundles.tsx#L182), [npm engine declaration](https://github.com/sarv-projects/LiteSPM/blob/f593c89044009eec0952ab6286413219f44caec3/npm/package.json)

### Website rendering and app behavior

The sitemap and route metadata are server-rendered, but all agent detail pages and the insights page lack an H1 in their initial HTML. Source inspection shows the agent detail view starts in a loading state and fetches its content after client hydration. This weakens direct-link first impressions and makes crawlers and assistive technology depend on JavaScript for core page identity. Server-render a useful title and summary, then hydrate richer data; ensure loading, unavailable, and error states remain distinguishable.

The installation docs are unusually concrete about plan, preview, approve, backup/apply, and restore. The product should preserve that clarity in every command that can mutate host config, including sync. The docs mention doctor, self-update, recovery and exact-byte restore, but these user journeys were not executed.

### Release and install qualification gap

The follow-up used an imported, checksum-verified [Ubuntu 24.04.5 WSL image](https://releases.ubuntu.com/noble/) in a separate temporary distro; its hash matched Microsoft's [WSL distribution manifest](https://github.com/microsoft/WSL/blob/master/distributions/DistributionInfo.json). Distro-local settings disabled Windows drive automount and Windows process interop. Node v22.23.4 was downloaded from the official Node release and passed its [published SHA-256 check](https://nodejs.org/dist/v22.23.4/SHASUMS256.txt). In that environment:

- npm install --global litespm@0.6.0 completed successfully, including its postinstall step.
- litespm --version launched and reported LiteSPM v0.6.0, Protocol 2026-07-28, linux/amd64, go1.26.9.
- litespm --help completed and listed the expected discover, manage, recover, configure, and system command groups.
- litespm doctor ran and returned exit 70: 5 checks passed, 4 warnings, and 2 failures. The failures were missing CONFIG_ROOT/RUNTIME_ROOT directories and no FreeDesktop Secret Service/secret-tool. The fresh headless WSL image had no host registrations, so the doctor result does not establish a defect in a supported desktop environment.

The homepage screenshot showed the v0.6.0-on-npm and 52-host badge, the main install/undo message, the navigation, terminal install command, and product illustration. The visible first fold is coherent and legible at the captured desktop size.

The temporary LiteSPM WSL distro was unregistered after the smoke test. The downloaded Ubuntu base image remains in the Windows Temp folder because the local execution policy blocked deleting that verified image file; its SHA-256 is bb415d824822c4b878125729af451a5d18fb13d1cf5cbed9a7393ad64ac6039e.

This is a limited Linux-on-WSL package smoke test, not native Linux desktop or Windows qualification. These items remain unverified:

- Native Windows/macOS installation and real desktop keyring integration.
- First-run setup with a real supported host and actual host detection.
- Preview, approval, denial, apply, and exact-byte restore against a real host config.
- A real agent session seeing and invoking an installed MCP server or skill.
- Upgrade, self-update, uninstall, duplicate execution, offline behavior, permissions failure, and recovery after interruption.
- Secret storage and token delivery at runtime.

Before launch, run the full flow in disposable environments matching the declared OS, Node, agent, and authentication support. Keep the evidence tied to exact binary and source revisions.

## Security and privacy assessment

### Positive evidence

- The site says credentials remain local and are stored in an OS vault or environment references, and that they are not sent to the site.
- Security docs describe explicit approval and backups.
- The npm installer source pins the expected signer identity and issuer and requires signature verification by default.
- The v0.6.0 release publishes signatures, checksums, and an SBOM.
- Standard transport and browser security headers were present in the homepage response.
- Package detail content describes certain inferred MCP permission categories as an aid rather than claiming to be a security audit. That caveat is valuable.

### Gaps and actions

- Sync's default behavior is incompatible with the product's preview/approval promise.
- The actual signed install, verification, credential vault, and post-install host behavior were not exercised.
- Latest CI shows four moderate npm advisories without detailed triage and skips release-fixture tests because fixtures were unavailable.
- CSP permits unsafe-inline and unsafe-eval. Tighten it where feasible; treat this as hardening, not a confirmed vulnerability.
- Wildcard CORS should be reviewed against the intended public API surface.
- The website does not surface a dedicated privacy/terms page in its inspected footer. State whether website analytics, catalog queries, or CLI telemetry exist, and expose a contact/reporting path.
- Do not imply that ranking or metadata makes arbitrary third-party packages safe. Continue distinguishing source/availability verification from security review.

## Competitive position

LiteSPM enters an active “package manager for agent context/tools” category. “Install once across agents” is not unique on its own.

| Product | Publicly described overlap | Strategic implication for LiteSPM |
|---|---|---|
| Microsoft APM | Project-level manifest and lockfile for skills, prompts, plugins, instructions, and MCP servers; content hashes, cross-harness projection, policy checks, and governance. | Emphasize local user-level cross-host configuration and reversible, visible config changes; compare honestly on project reproducibility and policy. |
| Vercel Skills CLI and skills.sh | Open-source CLI for discovering/installing skills; shareable packs; popularity ranking uses anonymous install telemetry, and docs caution that listing does not guarantee quality/security. | Distinguish LiteSPM's combined MCP/skills/plugin catalog and local change/restore flow, and be explicit about LiteSPM's telemetry position. |
| skillpkg SPM | Publicly positions itself as a package manager for agent skills and includes MCP in its repository topics and description. | Avoid generic “npm for agent tools” copy; demonstrate host coverage, install transaction, provenance and rollback. |
| Camunda SPM CLI | Project-scoped external skills as dependencies. | Explain the difference between repo-owned dependencies/lockfiles and LiteSPM's local host setup/catalog approach. |
| PSPM | Publicly positions itself as a package manager for AI-agent skills. Its site did not yield readable content during this source pass. | Treat as a competitor to validate in a later hands-on comparison; no detailed capability comparison is asserted here. |

Sources: [Microsoft APM docs](https://microsoft.github.io/apm/), [Skills CLI docs](https://www.skills.sh/docs), [skillpkg/spm](https://github.com/skillpkg/spm), [Camunda SPM CLI](https://github.com/camunda/spm-cli), [PSPM](https://pspm.dev/).

**Recommended positioning:** “A local, reversible way to install and manage MCP servers, skills, and plugins across the coding agents you already use.” Lead with the visible plan/preview/backup/undo experience. Explicitly name the developer audience and supported-host boundary. Do not claim unique universal compatibility or imply that every catalog listing has been security-reviewed.

## Prioritized issue register

Priority definitions: **P1** means resolve or explicitly accept before broad public launch. **P2** means high-value pre-launch polish or a documented follow-up. No confirmed exploitable security vulnerability was established in this pass.

| ID | Priority | Finding | Required outcome |
|---|---|---|---|
| LC-01 | P1 | Latest CI is red on npm package contents assertion in static and Windows npm jobs. | Align npm package manifest and allowlist expectation; rerun all OS, npm, web, and release-fixture checks; no cancelled required platform job. |
| LC-02 | P1 | Default sync can write agent configuration without preview or human approval. | Make preview/approval default, or isolate explicit unattended mode and change public promise; add denial and scope tests. |
| LC-03 | P1 | Sync can return exit 0 and claim success despite failed config/skill operations or ledger writes. | Count only completed work, surface each failure, preserve partial status, and return nonzero for incomplete runs. |
| LC-04 | P1 | Public site promotes an in-agent capability associated with 0.6.1 while npm latest is 0.6.0. | Gate copy and CTA to the actually published/qualified version; update docs and release notes together. |
| LC-05 | P1 | Published Linux CLI smoke passed in WSL, but no native Windows/macOS or real-agent install/restore evidence exists. | Complete the exact released package journey in disposable supported OS/agent environments, including denial and recovery. |
| LC-06 | P1 | Bundle prerequisite says Node 20+ while npm requires Node 22.22.2+ or newer listed lines. | Align all visible prerequisites with package engines and test a clean setup with each supported Node line. |
| LC-07 | P2 | 53 sitemap pages lack server-rendered H1; agent/insights content waits on client fetch. | Server-render page identity and useful fallback content; keep accessible loading and error states. |
| LC-08 | P2 | Only homepage has an Open Graph image; package sharing copies hash URLs. | Use canonical package paths and provide share images for deep routes. |
| LC-09 | P2 | 276 routes fall into 69 repeated title groups; package JSON-LD overclaims free/cross-platform. | Use package-and-publisher-specific titles; emit only verified structured data. |
| LC-10 | P2 | Sitemap caps listings at 500 and assigns one identical lastmod date. | Document the cap and quality rule; use accurate modification dates or omit them when unknown. |
| LC-11 | P2 | Headline is broader than actual developer audience; Node/terminal prerequisite is not prominent; [README says “80+ AI coding agents”](https://github.com/sarv-projects/LiteSPM/blob/f593c89044009eec0952ab6286413219f44caec3/README.md#L9) while the site distinguishes 52 MCP hosts and 81 skill targets. | State intended user, required tooling, supported host boundary, and first-success steps above the fold; reconcile counts and terms across README and website. |
| LC-12 | P2 | Bundle cards install one package per command; 151 upstream entries were not individually reviewed in this audit. | Make per-item approval friction explicit; consider grouped planning; publish the scope and limits of catalog validation. |
| LC-13 | P2 | Site is on a generic vercel.app hostname; no dedicated privacy/terms page was found in inspected navigation. | Before broad promotion, use the intended brand domain and make privacy, telemetry, contact, and support details easy to find. |
| LC-14 | P2 | CSP allows unsafe-inline and unsafe-eval; wildcard CORS and four moderate npm advisories need review. | Tighten browser policy where practical, confirm CORS scope, and triage advisories with recorded disposition. |

## Launch gates and acceptance evidence

### Must pass before broad public launch

1. Latest main/release CI is green for static, npm packaging, web build, Go tests on Windows/macOS/Linux, and signed release fixture/integration paths. No required platform is cancelled.
2. Published npm package version equals the version and feature set marketed on the website. The in-agent marketplace claim is either available and tested or clearly marked upcoming.
3. A fresh disposable Windows, macOS, and Linux environment completes install, signature verification, setup, package discovery, preview, approval, install, real agent invocation, remove, and byte-exact restore.
4. Denial leaves host config unchanged. Duplicate replay does not create duplicates. Timeout, network failure, permission denial, and interruption produce truthful partial/unknown states and recover without false success.
5. Default sync cannot modify a newly detected host without a visible preview and an explicit authorization path.
6. Bundle prerequisites match supported Node versions; every bundle command and package source gets a real successful path test.
7. Direct deep links have meaningful server-rendered titles, H1s, canonicals, and social previews, and the copied URL is canonical.
8. A browser pass covers desktop and mobile layouts, keyboard operation, basic screen-reader labels, validation/errors, empty/loading states, and the publish workflow.

### Go/no-go table

| Area | Status | Reason |
|---|---|---|
| Site reachability and public route availability | PASS | 571/571 sitemap routes returned HTTP 200 |
| Search API and empty-result response | PASS | Query and no-result probes returned valid JSON |
| Unknown route semantics | PASS | Unknown package and unknown route returned HTTP 404 |
| Baseline title/description/canonical coverage | PASS | Present on all 571 sitemap URLs |
| Product story and trust information | PARTIAL | Good reversible workflow message; sync behavior and version messaging conflict |
| Latest source CI | FAIL | Two jobs fail the same npm packaging assertion; macOS job cancelled |
| npm/source release parity | FAIL | Source declares 0.6.1; registry latest is 0.6.0 |
| Published npm install and Linux CLI startup | PASS in WSL | Published 0.6.0 installed with Node 22.23.4 and launched in a fresh WSL distro |
| Install and undo with a real agent | UNVERIFIED | No real supported host or authenticated session in the disposable distro |
| Approval/denial/recovery workflow | UNVERIFIED | Not exercised in a real installation |
| Sync safety and truthful failure reporting | FAIL by source inspection | Default apply path skips approval; errors can be hidden by zero exit/final success text |
| SEO/social deep links | FAIL / PARTIAL | Deep pages lack OG image, 53 lack H1, package copy link uses legacy hash path |
| Desktop first impression | PARTIAL | In-app browser screenshot reviewed; deep page DOM/navigation timed out |
| Mobile, keyboard, screen reader | UNVERIFIED | Responsive and accessibility interactions could not be completed |
| Bundle data structure | PARTIAL | Checker enforces catalog existence/installability; upstream quality not individually audited |
| Privacy/support clarity | PARTIAL | Local-data claims exist; privacy/terms contact details are not readily surfaced |

## Recommended sequence

### Gate 1 — safety and release truth

- Fix the sync approval and error-accounting behavior.
- Resolve the npm allowlist/package contents mismatch.
- Triage the npm advisories and rerun full CI with release fixtures available.
- Publish only the version that passes the native install and rollback matrix.
- Match all in-agent marketplace copy to the published version.

### Gate 2 — first-run success

- Fix the Node prerequisite in bundle details and quickstart.
- Add a short proof demo showing the preview diff, approval, actual host result, and undo.
- Test one representative host from each supported OS and document any host-specific limits.
- Add fault injection for no-write denial, partial writes, permissions, network timeout, duplicate replay, and restore after interruption.

### Gate 3 — public discovery and trust

- Correct package share URLs and add deep-route social images.
- Server-render page identity for agent and insights details; improve duplicate metadata.
- Correct structured data and sitemap freshness.
- Surface telemetry/privacy/support policy and move to the intended domain.
- Decide whether to group a bundle into one coherent plan/approval or clearly show one install/approval per item.

## Source and evidence links

- [LiteSPM homepage](https://lite-spm-nine.vercel.app/)
- [Getting started](https://lite-spm-nine.vercel.app/docs/getting-started)
- [Agent compatibility](https://lite-spm-nine.vercel.app/docs/agent-compatibility)
- [Install flow](https://lite-spm-nine.vercel.app/docs/install-flow)
- [Security docs](https://lite-spm-nine.vercel.app/docs/security)
- [Methodology](https://lite-spm-nine.vercel.app/docs/methodology)
- [LiteSPM source repository](https://github.com/sarv-projects/LiteSPM)
- [Latest CI run reviewed](https://github.com/sarv-projects/LiteSPM/actions/runs/44139044863)
- [Latest tagged release reviewed](https://github.com/sarv-projects/LiteSPM/releases/tag/v0.6.0)
- [npm package and current registry version](https://www.npmjs.com/package/litespm)

## Companion artifact

The companion CSV contains the per-URL status, title, description, H1 text, HTML byte count, canonical presence, and Open Graph image presence for every URL in the sitemap fetched on 2026-11-18:

- audit-output/litespm-route-audit-2026-11-18.csv

The crawl is a point-in-time HTTP inventory. A screenshot of the desktop first fold was reviewed, but this is not a substitute for full-page/mobile browser review, Lighthouse/Web Vitals, accessibility testing, real-agent install and restore, or user interviews.
