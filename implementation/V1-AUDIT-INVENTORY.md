# V1 First-Party Markdown Mechanical Inventory

This script read every line of every included first-party Markdown file and checked literal Markdown relative link targets. **Mechanical coverage does not imply that each statement was semantically audited, implemented or runtime-qualified.**

- Files read: 173
- Lines read: 59296
- Bytes read: 4164770
- Broken relative links: 0
- Excluded: dependency/cache/output worktrees and generated audit reports; CSV inventory includes SHA-256 for every read Markdown source.

## Per-file line and topical inventory

| File | Lines | SHA-256 prefix | Keyword categories |
|---|---:|---|---|
| AGENTS.md | 289 | dd868672c157 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| ARCHITECTURE.md | 780 | 5f6c95f1c1f1 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| CONTRIBUTING.md | 38 | 709b7f2d6cad | rendering;trust/security |
| GLOSSARY.md | 292 | e76323588a46 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| README.md | 156 | 5c54bf5c560f | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| apps/litecowork-ui/README.md | 77 | b2f39f920056 | browser/computer;shell/settings/motion;workflow/recovery |
| apps/litecowork-ui/src/artifacts/README.md | 155 | bed0b3660621 | shell/settings/motion;rendering;workflow/recovery;trust/security |
| apps/litecowork-ui/src/presentation/README.md | 32 | c868572e9239 | browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| assets/host-instructions/presentation-policy.md | 12 | 7a598b60ebb7 | workflow/recovery |
| assets/host-skills/rich-response-design/SKILL.md | 111 | 7f6287aab3db | shell/settings/motion;rendering;workflow/recovery;trust/security |
| capabilities/zip_intake/README.md | 104 | c8bbbbd824a5 | workflow/recovery;trust/security |
| crates/domain-responsibility/README.md | 141 | 0e9811b3c3f5 | shell/settings/motion;workflow/recovery;trust/security |
| docs/AGENT-CONTROL.md | 1176 | fcbe8c0cd71b | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/AGENT-FABRIC.md | 414 | d3c212797a1d | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/API.md | 1764 | dcc95193ce74 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/ARTIFACTS-EVIDENCE.md | 338 | efbf03d07aa7 | shell/settings/motion;workflow/recovery;trust/security |
| docs/AUTOMATION.md | 491 | 0beb58238ce6 | workflow/recovery;trust/security |
| docs/BENCHMARKS.md | 683 | 608779fb8632 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/CAPABILITY-FABRIC.md | 375 | 92edd06d3098 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/CAPABILITY-INVOCATIONS.md | 467 | 5c436b77b83d | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/CHANNELS.md | 274 | 4e497119011f | workflow/recovery;trust/security |
| docs/CHROME-BROWSER-LLD.md | 176 | 397c081872e4 | browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/COMPETITIVE-RESEARCH.md | 236 | b7ad0c8efcb6 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/CONNECTORS-WORKFLOWS.md | 825 | 70815aa25dbd | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/CONTEXT.md | 210 | 1e502103010c | agent/composer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/COVERAGE-MATRIX.md | 144 | 32f7e58bed02 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/COWORKER-FLOWS.md | 166 | 201dbdd63017 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/COWORKERS-TARGET.md | 444 | 42b3c7f5b33f | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/DATA-MODEL.md | 2546 | dedd484d1c97 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/DELEGATION.md | 802 | de5562921603 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/DESIGN-SYSTEM.md | 456 | 4d6c647ce42d | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/DESKTOP-SHELL-LLD.md | 267 | 098d67ed6982 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/ENVIRONMENTS.md | 430 | 06f571bab1a4 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/EVENTS.md | 855 | 1332bbf56349 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/EXPERIENCE.md | 1087 | a9c7f80e3fd1 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/FAILURE-RECOVERY.md | 208 | 4292dffbbbe7 | agent/composer;browser/computer;rendering;workflow/recovery;trust/security |
| docs/FLOWS.md | 4420 | 17ba123ab1bf | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/HOST-GUIDANCE.md | 138 | 39a1fd4833e3 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/IMPLEMENTATION.md | 226 | 01c86f6c01f0 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/LEARNING.md | 644 | 55daeed607e8 | agent/composer;shell/settings/motion;workflow/recovery;trust/security |
| docs/LOCAL-OPERATOR-IPC.md | 420 | bef5225f50d2 | agent/composer;workflow/recovery;trust/security |
| docs/MOTION.md | 256 | 95016d7f546f | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery |
| docs/NATIVE-COMPOSER-LLD.md | 178 | c582a2dfce66 | agent/composer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/NETWORK-SECURITY.md | 306 | 86e915bdd6a8 | browser/computer;rendering;workflow/recovery;trust/security |
| docs/OBSERVABILITY.md | 241 | 3e0faa25f032 | browser/computer;rendering;workflow/recovery;trust/security |
| docs/PRESENTATION-RUNTIME.md | 505 | aea8b72f539a | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/PRODUCT.md | 236 | bc75bc1dff7b | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/PROTOCOLS.md | 182 | c21f8ea7b8fd | agent/composer;browser/computer;rendering;workflow/recovery;trust/security |
| docs/REFERENCE-IMPLEMENTATIONS.md | 318 | c83e50f05244 | agent/composer;browser/computer;rendering;workflow/recovery;trust/security |
| docs/RENDERING-CONFORMANCE-LLD.md | 119 | 113baa60f8c2 | browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/RESPONSIBILITIES.md | 486 | eac7fa14beab | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/RICH-RESPONSE.md | 290 | 83bbe1009319 | shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/ROUTINES.md | 221 | 7f5b05dfb2fa | agent/composer;workflow/recovery;trust/security |
| docs/RUNTIME-LIFECYCLE.md | 580 | 20e20cb3b196 | agent/composer;browser/computer;workflow/recovery;trust/security |
| docs/RUNTIME-MESH.md | 744 | 2c05757095d5 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/SCHEMAS.md | 1437 | f92fa6b38efa | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/SECURITY.md | 174 | fa030468484c | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/SERVICES.md | 1547 | 698fb946131d | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/SOURCE-RECONCILIATION.md | 153 | e6839158941b | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/STATE-MACHINES.md | 1373 | ac343c56eb2d | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/STORAGE.md | 1071 | 29e1b214f9dd | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/TASK-RUNTIME.md | 745 | 661c26243ca0 | agent/composer;workflow/recovery;trust/security |
| docs/TESTING.md | 454 | 50174de3ae35 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/TRUST.md | 317 | ab477965b658 | agent/composer;browser/computer;workflow/recovery;trust/security |
| docs/V1-AGENT-DISCOVERY-AND-CATALOG-LLD.md | 114 | d8cd93ef7ae0 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/V1-AGENT-MEMORY-FAST-PATH-LLD.md | 104 | c1725285e9f2 | agent/composer;shell/settings/motion;workflow/recovery;trust/security |
| docs/V1-DESIGN-INDEX.md | 83 | 53d7ef21a0ea | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/V1-END-TO-END-FLOW-AND-RACE-CONTRACTS.md | 70 | 081405034fad | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/V1-FUNCTION-CONTRACT-LEDGER.md | 245 | de37d49b8aa2 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/V1-FUNCTION-FILE-CATALOG.md | 465 | bcdc5c725ced | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/V1-INTEGRATED-TERMINAL-LLD.md | 131 | 5e7ac68f5bc0 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/V1-REFERENCE-OMNIROUTE-OMNIRUSH-TERMINAL.md | 63 | a002bab93eb3 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/V1-SETTINGS-CONTRACT-LEDGER.md | 215 | e09682e2de63 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/V1-TELEMETRY-USAGE-RESILIENCE-LLD.md | 172 | 04b79e5507a7 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/V1-UI-ACTION-STATE-LEDGER.md | 57 | feb6e9c52f3c | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/V1-WORKFLOW-EVENT-LLD.md | 116 | 37d4d68f0658 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/WORLD-RESOURCES.md | 390 | 17445d587bc3 | browser/computer;workflow/recovery;trust/security |
| docs/adr/0001-durable-task-core.md | 23 | 8fe458784bec | workflow/recovery |
| docs/adr/0002-task-and-attempt-identity.md | 21 | 83d4b175418a | agent/composer;browser/computer;workflow/recovery |
| docs/adr/0003-litespm-is-independent.md | 30 | e554a0d25448 | workflow/recovery;trust/security |
| docs/adr/0004-runtime-and-environment-are-distinct.md | 21 | 359ffb5d539b | browser/computer;workflow/recovery |
| docs/adr/0005-replicate-domain-state-not-databases.md | 21 | 3428312c13b9 | agent/composer;workflow/recovery |
| docs/adr/0006-conversation-task-separation.md | 16 | 92a5f0a08431 | workflow/recovery |
| docs/adr/0007-reported-observed-verified.md | 16 | 8c15daf850e7 | workflow/recovery |
| docs/adr/0008-no-generic-provider-layer.md | 23 | 6c1f92c12c72 | agent/composer;shell/settings/motion;trust/security |
| docs/adr/0009-attempt-free-lead-planning.md | 16 | 6041ee56f87a | workflow/recovery |
| docs/adr/0010-revision-independent-occurrences.md | 16 | d5becc7f733d | workflow/recovery |
| docs/adr/0011-atomic-plan-acceptance.md | 30 | 6f9b78226189 | shell/settings/motion;workflow/recovery |
| docs/adr/0012-native-harness-integrity.md | 35 | 90b1ffcd4df9 | agent/composer;workflow/recovery;trust/security |
| docs/adr/0013-host-delegation-uses-accepted-plan-steps.md | 25 | fec8da7253f7 | workflow/recovery;trust/security |
| docs/adr/0014-versioned-worker-profiles-and-bounded-selection.md | 26 | 6d56c993d253 | agent/composer;workflow/recovery;trust/security |
| docs/adr/0015-warmth-is-operational-and-sharing-is-separate.md | 24 | d33dad274f71 | browser/computer;workflow/recovery;trust/security |
| docs/adr/0016-coworker-goal-and-suggestion-authority-boundary.md | 25 | de6aefd4ae3c | workflow/recovery;trust/security |
| docs/adr/0017-deadline-sensitive-is-best-effort.md | 25 | e75f3980fa2f | browser/computer;shell/settings/motion;workflow/recovery |
| docs/adr/0018-context-content-is-resource-backed-and-provider-pluggable.md | 29 | 670ddfc6d2f5 | workflow/recovery;trust/security |
| docs/adr/0019-credential-egress-and-audit-boundaries.md | 25 | 5b93f069c10c | workflow/recovery;trust/security |
| docs/adr/0020-runtime-identity-is-installation-scoped.md | 50 | d87e26cf0229 | workflow/recovery;trust/security |
| docs/adr/0021-device-signing-identity.md | 54 | 2b51dfb39fb6 | workflow/recovery;trust/security |
| docs/adr/0022-rich-presentation-is-optional.md | 33 | 7104d5c575e3 | shell/settings/motion;rendering;trust/security |
| docs/adr/0023-host-guidance-is-not-capability-authority.md | 27 | 837ea5dd936b | agent/composer;workflow/recovery;trust/security |
| docs/adr/0024-optional-chat-first-coworkers.md | 35 | 5a6cde423b5c | agent/composer;shell/settings/motion;workflow/recovery;trust/security |
| docs/adr/0025-agent-module-registry-and-native-configuration.md | 59 | 79ff69084cb0 | agent/composer;workflow/recovery;trust/security |
| docs/adr/README.md | 39 | 54afa5933f64 | agent/composer;workflow/recovery;trust/security |
| implementation/AGENT-MODULE-AUDIT.md | 524 | bad51ed44a90 | agent/composer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/ARCHITECTURE-COVERAGE.md | 177 | a39bebdf356f | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/AUDIT.md | 155 | a55491d42453 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/COVERAGE.md | 33 | de61a91ec685 | rendering |
| implementation/CURRENT-RUN.md | 7172 | f57fa4b185aa | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/DOC-AUDIT-2026-10-10.md | 173 | ff27abd7e23a | shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/NEXT-SESSION-HANDOFF-2026-10-13.md | 363 | 9c871129dcc1 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/PROCESS.md | 99 | 9ec104a4a710 | shell/settings/motion;workflow/recovery;trust/security |
| implementation/RAG.md | 120 | eebd30eaeeb7 | shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/README.md | 81 | 7987a25382ba | agent/composer;browser/computer;workflow/recovery |
| implementation/RELEASE.md | 77 | 18df5c3b605e | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/ROADMAP.md | 175 | fbe5a1fd3ced | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/SCOPE.md | 132 | 3511d2336ba9 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/SOURCES.md | 134 | 2c70d7a44f46 | browser/computer;rendering;workflow/recovery;trust/security |
| implementation/STACK.md | 140 | c866f1d4b5f2 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/TESTING.md | 181 | 9643bb926700 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/UI.md | 776 | b6645ba0e06d | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/V1-CONTRACT-CHECK.md | 42 | 7132c1c7ed77 | agent/composer;browser/computer |
| implementation/V1-CONTRACT-RECONCILIATION-2026-10-13.md | 59 | a3b6be9662e0 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/V1-DESIGN-COVERAGE.md | 226 | d269d2d96e60 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/V1-DESIGN-DRYRUN-AND-RECONCILIATION-2026-10-13.md | 116 | 4836d42b5f95 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/V1-DESIGN-HANDOFF-2026-10-12.md | 71 | a2ed19f4b45c | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/V1-EVERY-MD-AUDIT-2026-10-12.md | 179 | 49fd6f683de5 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/V1-IMPLEMENTATION-REAUDIT-2026-10-13.md | 186 | 874ceb3eaded | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/V1-OBSERVABILITY-AGENT-MEMORY-TERMINAL-UAT.md | 81 | 744ac1294a31 | browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/V1-SEMANTIC-MD-FINDINGS-2026-10-12.md | 73 | 716f85e04ed2 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/V1-SERVED-CAPABILITY-INVENTORY.md | 60 | 3340bfeff2ba | agent/composer;workflow/recovery;trust/security |
| implementation/WORKFLOWS.md | 68 | 506e29243c5f | browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/epics/E01.md | 701 | c201ed250b83 | agent/composer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/epics/E02.md | 439 | 71bb2e66ea4a | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/epics/E03.md | 428 | 53afeafbcb78 | agent/composer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/epics/E04.md | 134 | 27e02ac37fb0 | workflow/recovery;trust/security |
| implementation/epics/E05.md | 127 | 7a25662952e3 | workflow/recovery;trust/security |
| implementation/epics/E06.md | 111 | eaa544603998 | agent/composer;workflow/recovery;trust/security |
| implementation/epics/E07.md | 221 | 1be0f4b3c765 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/epics/E08.md | 470 | cd5f5842a9f1 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/epics/E09.md | 207 | ee6307af4afe | shell/settings/motion;workflow/recovery;trust/security |
| implementation/epics/E10.md | 62 | 3bce19610b78 | browser/computer;workflow/recovery;trust/security |
| implementation/epics/E11.md | 98 | 2e45acbcb389 | browser/computer;workflow/recovery;trust/security |
| implementation/epics/E12.md | 44 | 0519a349a2ae | workflow/recovery;trust/security |
| implementation/epics/E13.md | 88 | e2a9f352db78 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/licensingandpkacgingresearch.md | 196 | 2e29b84d4b59 | agent/composer;workflow/recovery;trust/security |
| implementation/reviews/E01-S01.md | 115 | 52e28753cd56 | rendering;workflow/recovery;trust/security |
| implementation/reviews/E01-S02-BUSY-RETRY-2026-10-13.md | 80 | 77e30693e494 | workflow/recovery;trust/security |
| implementation/reviews/E01-S02.md | 528 | 241368b82db7 | rendering;workflow/recovery;trust/security |
| implementation/reviews/E01-S03.md | 665 | 7d1abd909e91 | shell/settings/motion;workflow/recovery;trust/security |
| implementation/reviews/E01-S04.md | 1412 | 8cec448718ec | agent/composer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/reviews/E02-S01.md | 199 | 51bbb5fc91f5 | browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/reviews/E02-S02-TAURI-IPC-2026-10-13.md | 86 | 316d19024d46 | shell/settings/motion;rendering;trust/security |
| implementation/reviews/E02-S02.md | 738 | 2e4db2f170ee | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/reviews/E02-S03.md | 389 | 4f47c428415d | browser/computer;workflow/recovery;trust/security |
| implementation/reviews/E02-S04.md | 100 | 986bf7e302e8 | browser/computer;shell/settings/motion;workflow/recovery |
| implementation/reviews/E03-S01.md | 100 | f153dc715714 | agent/composer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/reviews/E03-S03.md | 108 | e99d7236399a | agent/composer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/reviews/E05-S01.md | 76 | 4fea0770ff9a | shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/reviews/E07-S00.md | 91 | 7b1cc7765f6e | shell/settings/motion;workflow/recovery;trust/security |
| implementation/reviews/E07-S01.md | 82 | 9c6610218f90 | shell/settings/motion;workflow/recovery;trust/security |
| implementation/reviews/E08-S02.md | 106 | 0bb912438308 | shell/settings/motion;workflow/recovery;trust/security |
| implementation/reviews/E08-S03.md | 116 | 0fb658b225c9 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/reviews/E08-S04.md | 84 | 1e935f47736a | shell/settings/motion;workflow/recovery;trust/security |
| implementation/reviews/E08-S07.md | 96 | 7c4aec63e15a | shell/settings/motion;rendering |
| implementation/reviews/E08-S08.md | 131 | 20740c259173 | browser/computer;shell/settings/motion;rendering;workflow/recovery |
| implementation/reviews/E09-S02.md | 93 | cc6f0bac35de | shell/settings/motion;workflow/recovery |
| implementation/reviews/E09-S05.md | 92 | efcc433183e3 | shell/settings/motion;workflow/recovery;trust/security |
| implementation/reviews/SP04-HANDOVER-REVIEW.md | 110 | 8917518969a7 | agent/composer;workflow/recovery;trust/security |
| implementation/spikes/SP02.md | 174 | 70efa57d9d45 | workflow/recovery;trust/security |
| implementation/spikes/SP03.md | 100 | 956e9125e1a7 | workflow/recovery;trust/security |
| implementation/spikes/SP04.md | 474 | dc154675dbcf | agent/composer;workflow/recovery;trust/security |
| implementation/spikes/SP05.md | 119 | 5a3dfa04c67a | agent/composer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/spikes/sp02-driver-compare/README.md | 62 | 12af8c71a036 | shell/settings/motion |

## Broken local targets

None detected by this limited Markdown link parser.

## Semantic/source qualification still required

The normative target docs and implementation/V1-DESIGN-COVERAGE.md require owner-specific CODE, SYSTEM and USER evidence. Keyword categories do not certify intent or correctness, and link checking is not an external URL status check.
