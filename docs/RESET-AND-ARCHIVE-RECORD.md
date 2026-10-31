# LiteCowork — Reset and Archive Record

**Date:** 2026-10-30  
**Main baseline:** sarv-projects/litecowork, branch main, commit c7e9c05ab09563a554515bdd7e0e7d86c99fffa0  
**Pre-reset main working tree:** clean; main was aligned with origin/main.  
**Purpose:** archive the previous implementation snapshot intact and make the active repository a docs-first specification/architecture workspace.

## 1. Source material processed

The following user-provided Markdown sources were read in full for line structure and content, then reconciled:
- LiteCowork V1 Final Master Specification Revision 9 — 11,592 lines.
- LiteCowork V1 Final Implementation Blueprint V7 UI Audit — 8,906 lines.
- LiteCowork V7 Deep Competitive Engineering Audit — 988 lines.
- LiteCowork Activepieces/Nango Repository Deep Dive — 236 lines.

Total: 21,722 lines. Revision 9 is the current umbrella authority. It incorporates the older V7 specification and adds cross-source decisions, first-class LiteSPM integration and the Blaze coding-cockpit analysis. Existing owner contracts, accepted ADRs and docs/schemas remain authoritative in their domains.

## 2. Preserved active documentation

The following remain in the active tree:
- Root ARCHITECTURE.md, README.md, CONTRIBUTING.md and GLOSSARY.md.
- The complete docs/ owner-contract, ADR and schema tree.
- The new strategy, sequencing, UI, E2E, LiteSPM and connector documents indexed by docs/README.md.

The former archive/legacy-implementation-c7e9c05/ folder contained historical documentation/evidence rather than active source code. It was moved intact to:

docs/archive/legacy-implementation-c7e9c05/

The prior root AGENTS.md is retained as:

docs/archive/legacy-implementation-c7e9c05/AGENTS-legacy-2026-10-30.md

Its old implementation paths are historical and must not be followed as current instructions without reconciliation.

## 3. Local code archive

The former source/build/tooling snapshot was moved, not deleted, to:

archives/legacy-codebase-c7e9c05/

This local archive contains the old apps, crates, capabilities, assets, scripts, tests, Rust/Python/toolchain files, UI mock, caches, agent/dev tooling and other non-document root material. It is ignored by the root .gitignore and is intentionally not part of a future documentation commit.

The registered dirty worktree:

- Branch: feature/conversation-session-supervisor
- Commit: 3052341473e964bececf7f34c9d45923848e5218
- Archived path: archives/legacy-codebase-c7e9c05/.worktrees/conversation-session-supervisor

Its uncommitted changes and untracked spec/plan work are preserved. It was moved with git worktree move; do not reset, delete or prune it without an explicit recovery decision.

## 4. Git and safety status

- The main branch history was not rewritten.
- No commit or push was performed as part of this reset.
- Tracked source files moved out of their old paths will appear as deletions in git status until the owner decides whether to commit the docs-first state. The ignored local archive remains present on disk.
- The archive should not be staged or pushed accidentally.
- No credentials or native agent private profiles were intentionally moved out of the repository because they were not part of the tracked source tree.

## 5. Evidence caveat

The supplied c7e9c05 product acceptance and competitive audit material marked all required V1 user journeys BLOCKED at that historical baseline. Those reports remain historical evidence, not current live status. The new docs require a fresh baseline from the code that will be reconstructed/implemented next. No journey is upgraded to CODE, SERVED, INTEGRATED or QUALIFIED by this documentation reset.

## 6. Next working posture

Treat docs/README.md as the documentation entry point. Read root ARCHITECTURE.md, docs/PRODUCT.md, the V1 Product Release Contract and the relevant owner contracts before any new implementation. Follow docs/IMPLEMENTATION-MASTER-PLAN.md and docs/ACCEPTANCE-AND-E2E-PLAN.md. Recover old source only when a specific slice justifies it; do not restore the whole archive by default.
