# V1 First-Party Markdown Mechanical Inventory

This script read every line of every included first-party Markdown file and checked literal Markdown relative link targets. **Mechanical coverage does not imply that each statement was semantically audited, implemented or runtime-qualified.**

- Files read: 193
- Lines read: 69345
- Bytes read: 4917938
- Broken relative links: 0
- Excluded: dependency/cache/output worktrees and generated audit reports; CSV inventory includes SHA-256 for every read Markdown source.

## Per-file line and topical inventory

| File | Lines | SHA-256 prefix | Keyword categories |
|---|---:|---|---|
| AGENTS.md | 295 | 0a77bf1d580d | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| ARCHITECTURE.md | 780 | 5f6c95f1c1f1 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| CONTRIBUTING.md | 38 | 709b7f2d6cad | rendering;trust/security |
| GLOSSARY.md | 292 | e76323588a46 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| README.md | 156 | 5c54bf5c560f | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| apps/litecowork-ui/README.md | 77 | b2f39f920056 | browser/computer;shell/settings/motion;workflow/recovery |
| apps/litecowork-ui/src/artifacts/README.md | 155 | bed0b3660621 | shell/settings/motion;rendering;workflow/recovery;trust/security |
| apps/litecowork-ui/src/presentation/README.md | 32 | c868572e9239 | browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| assets/host-instructions/presentation-policy.md | 12 | 7a598b60ebb7 | workflow/recovery |
| assets/host-skills/rich-response-design/SKILL.md | 111 | 7f6287aab3db | shell/settings/motion;rendering;workflow/recovery;trust/security |
| capabilities/zip_intake/README.md | 120 | e692d1f9e057 | workflow/recovery;trust/security |
| crates/domain-responsibility/README.md | 141 | 0e9811b3c3f5 | shell/settings/motion;workflow/recovery;trust/security |
| docs/AGENT-CONTROL.md | 1212 | e0bd409a3063 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/AGENT-FABRIC.md | 445 | dec95680c00a | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/API.md | 2016 | ab61f4f92868 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/ARTIFACTS-EVIDENCE.md | 355 | 005fa9ba7aa7 | shell/settings/motion;workflow/recovery;trust/security |
| docs/AUTOMATION.md | 491 | 0beb58238ce6 | workflow/recovery;trust/security |
| docs/BENCHMARKS.md | 683 | 608779fb8632 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/CAPABILITY-FABRIC.md | 394 | 2795b7e55fd1 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/CAPABILITY-INVOCATIONS.md | 467 | 5c436b77b83d | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/CHANNELS.md | 274 | 4e497119011f | workflow/recovery;trust/security |
| docs/CHROME-BROWSER-LLD.md | 176 | 397c081872e4 | browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/COMPETITIVE-RESEARCH.md | 347 | 7f054a1e704c | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/CONNECTORS-WORKFLOWS.md | 825 | 70815aa25dbd | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/CONTEXT.md | 238 | e2103ca01056 | agent/composer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/COVERAGE-MATRIX.md | 144 | 4861bb9d6a71 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/COWORKER-FLOWS.md | 166 | 201dbdd63017 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/COWORKERS-TARGET.md | 444 | 42b3c7f5b33f | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/DATA-MODEL.md | 2596 | 8d418cd884ee | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/DELEGATION.md | 802 | de5562921603 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/DESIGN-SYSTEM.md | 456 | 4d6c647ce42d | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/DESKTOP-SHELL-LLD.md | 267 | 098d67ed6982 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/ENVIRONMENTS.md | 430 | 06f571bab1a4 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/EVENTS.md | 897 | 201a28cdf187 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/EXPERIENCE.md | 1093 | eebd6d41765e | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/FAILURE-RECOVERY.md | 208 | 4292dffbbbe7 | agent/composer;browser/computer;rendering;workflow/recovery;trust/security |
| docs/FLOWS.md | 4491 | 39d25e88c879 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/HOST-GUIDANCE.md | 138 | 39a1fd4833e3 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/IMPLEMENTATION.md | 226 | 01c86f6c01f0 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/LEARNING.md | 644 | 55daeed607e8 | agent/composer;shell/settings/motion;workflow/recovery;trust/security |
| docs/LOCAL-OPERATOR-IPC.md | 425 | 6fbac4213425 | agent/composer;workflow/recovery;trust/security |
| docs/MOTION.md | 256 | 95016d7f546f | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery |
| docs/NATIVE-COMPOSER-LLD.md | 178 | c582a2dfce66 | agent/composer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/NETWORK-SECURITY.md | 308 | ce88bd9a88eb | browser/computer;rendering;workflow/recovery;trust/security |
| docs/OBSERVABILITY.md | 241 | 3e0faa25f032 | browser/computer;rendering;workflow/recovery;trust/security |
| docs/PRESENTATION-RUNTIME.md | 505 | aea8b72f539a | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/PRODUCT.md | 236 | bc75bc1dff7b | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/PROTOCOLS.md | 182 | c21f8ea7b8fd | agent/composer;browser/computer;rendering;workflow/recovery;trust/security |
| docs/REFERENCE-IMPLEMENTATIONS.md | 450 | 807ccb93f58a | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/RENDERING-CONFORMANCE-LLD.md | 119 | 113baa60f8c2 | browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/RESPONSIBILITIES.md | 486 | eac7fa14beab | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/RICH-RESPONSE.md | 290 | 83bbe1009319 | shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/ROUTINES.md | 224 | ea5b09128326 | agent/composer;workflow/recovery;trust/security |
| docs/RUNTIME-LIFECYCLE.md | 589 | f3ae333e28b5 | agent/composer;browser/computer;workflow/recovery;trust/security |
| docs/RUNTIME-MESH.md | 744 | 2c05757095d5 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| docs/SCHEMAS.md | 1529 | 5be26d9d9a19 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/SECURITY.md | 174 | fa030468484c | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/SERVICES.md | 1547 | 92690c4cb268 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/SOURCE-RECONCILIATION.md | 153 | e6839158941b | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/STATE-MACHINES.md | 1405 | 63f542e94f7e | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/STORAGE.md | 1281 | 7777a38a8d95 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| docs/TASK-RUNTIME.md | 754 | 29f42a6c2d3f | agent/composer;workflow/recovery;trust/security |
| docs/TESTING.md | 460 | 029cdb3bc702 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
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
| docs/WORLD-RESOURCES.md | 525 | a285137a6acf | agent/composer;browser/computer;workflow/recovery;trust/security |
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
| implementation/AGENT-MODULE-AUDIT.md | 532 | 9507efb440a7 | agent/composer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/AGENT-PORTFOLIO-2026-10-16.md | 156 | c3c53ff30e90 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/ARCHITECTURE-COVERAGE.md | 177 | fb89defa3342 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/AUDIT.md | 155 | a55491d42453 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/COVERAGE.md | 33 | de61a91ec685 | rendering |
| implementation/CURRENT-RUN.md | 9122 | 0556f6f52610 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/DOC-AUDIT-2026-10-10.md | 173 | ff27abd7e23a | shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/NEXT-SESSION-HANDOFF-2026-10-13.md | 316 | 938e000f5933 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/NEXT-SESSION-HANDOFF-2026-10-20.md | 149 | 370c7e6d28ea | browser/computer;rendering;workflow/recovery;trust/security |
| implementation/PROCESS.md | 99 | 9ec104a4a710 | shell/settings/motion;workflow/recovery;trust/security |
| implementation/RAG.md | 120 | eebd30eaeeb7 | shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/README.md | 85 | 37326cf30b73 | agent/composer;browser/computer;workflow/recovery |
| implementation/RELEASE.md | 77 | 18df5c3b605e | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/ROADMAP.md | 175 | fbe5a1fd3ced | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/SCOPE.md | 137 | 1f431913023b | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/SOURCES.md | 134 | 7cc8ad920d2f | browser/computer;rendering;workflow/recovery;trust/security |
| implementation/STACK.md | 140 | c866f1d4b5f2 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/TESTING.md | 181 | 9643bb926700 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/UI.md | 779 | 18fe6565ca99 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/V1-CONTRACT-CHECK.md | 42 | c6d0c53f2ccb | agent/composer;browser/computer |
| implementation/V1-CONTRACT-RECONCILIATION-2026-10-13.md | 59 | a3b6be9662e0 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/V1-DESIGN-COVERAGE.md | 226 | d269d2d96e60 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/V1-DESIGN-DRYRUN-AND-RECONCILIATION-2026-10-13.md | 116 | 4836d42b5f95 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/V1-DESIGN-HANDOFF-2026-10-12.md | 71 | a2ed19f4b45c | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/V1-EVERY-MD-AUDIT-2026-10-12.md | 179 | 49fd6f683de5 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/V1-IMPLEMENTATION-REAUDIT-2026-10-13.md | 292 | 1b935ece7feb | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/V1-OBSERVABILITY-AGENT-MEMORY-TERMINAL-UAT.md | 81 | 744ac1294a31 | browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/V1-SEMANTIC-MD-FINDINGS-2026-10-12.md | 73 | 716f85e04ed2 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/V1-SERVED-CAPABILITY-INVENTORY.md | 64 | ea4f34c1f61a | agent/composer;browser/computer;workflow/recovery;trust/security |
| implementation/WORKFLOWS.md | 70 | 8c37420bf900 | browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/epics/E01.md | 738 | 0b7541d7ce47 | agent/composer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/epics/E02.md | 568 | 6e4598664f81 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/epics/E03.md | 479 | 957b5b2b2da6 | agent/composer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/epics/E04.md | 134 | 27e02ac37fb0 | workflow/recovery;trust/security |
| implementation/epics/E05.md | 184 | 27391136ba4d | shell/settings/motion;workflow/recovery;trust/security |
| implementation/epics/E06.md | 113 | 62925d7e4282 | agent/composer;workflow/recovery;trust/security |
| implementation/epics/E07.md | 224 | 96df60895aa1 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/epics/E08.md | 495 | 3b2070e99e71 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/epics/E09.md | 207 | ee6307af4afe | shell/settings/motion;workflow/recovery;trust/security |
| implementation/epics/E10.md | 62 | 3bce19610b78 | browser/computer;workflow/recovery;trust/security |
| implementation/epics/E11.md | 98 | 2e45acbcb389 | browser/computer;workflow/recovery;trust/security |
| implementation/epics/E12.md | 44 | 0519a349a2ae | workflow/recovery;trust/security |
| implementation/epics/E13.md | 88 | e2a9f352db78 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/licensingandpkacgingresearch.md | 196 | 2e29b84d4b59 | agent/composer;workflow/recovery;trust/security |
| implementation/reviews/E01-S01.md | 140 | a09e0a006d43 | rendering;workflow/recovery;trust/security |
| implementation/reviews/E01-S02-BUSY-RETRY-2026-10-13.md | 80 | 77e30693e494 | workflow/recovery;trust/security |
| implementation/reviews/E01-S02.md | 632 | fa9542f7a1d4 | rendering;workflow/recovery;trust/security |
| implementation/reviews/E01-S03.md | 775 | b91158988372 | shell/settings/motion;workflow/recovery;trust/security |
| implementation/reviews/E01-S04.md | 1578 | c233684d8593 | agent/composer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/reviews/E02-S01.md | 387 | 9b8d9e5adbb2 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/reviews/E02-S02-TAURI-IPC-2026-10-13.md | 86 | 316d19024d46 | shell/settings/motion;rendering;trust/security |
| implementation/reviews/E02-S02.md | 835 | 8679a9672be1 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/reviews/E02-S03.md | 1790 | f007d56926bd | browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/reviews/E02-S04.md | 428 | 7e0de77bf531 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/reviews/E03-S01-agent-catalog-truthful-status-2026-10-16.md | 80 | 2bc00031d878 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery |
| implementation/reviews/E03-S01.md | 595 | 7103703d1206 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/reviews/E03-S02.md | 203 | 88a37ea85649 | agent/composer;browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/reviews/E03-S03.md | 108 | e99d7236399a | agent/composer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/reviews/E03-S06.md | 155 | 8c0745de8b00 | agent/composer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/reviews/E03-S08.md | 112 | 9ef82c02f5bc | agent/composer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/reviews/E03-S09.md | 78 | fea18c94b55e | agent/composer;workflow/recovery;trust/security |
| implementation/reviews/E03-S10.md | 63 | 3f68d1c474f3 | agent/composer;workflow/recovery;trust/security |
| implementation/reviews/E04-S01.md | 203 | dfb47dc65c93 | workflow/recovery;trust/security |
| implementation/reviews/E04-S03.md | 117 | 6ef4916c35a8 | workflow/recovery;trust/security |
| implementation/reviews/E04-S05.md | 216 | 59b8ac8fc3be | workflow/recovery;trust/security |
| implementation/reviews/E05-S01.md | 174 | 49b0a7a13b22 | shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/reviews/E05-S02.md | 265 | 96d942cd6cb6 | workflow/recovery;trust/security |
| implementation/reviews/E05-S03.md | 210 | 37e25a6d77ca | agent/composer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/reviews/E05-S04.md | 129 | fd79e3643588 | shell/settings/motion;workflow/recovery;trust/security |
| implementation/reviews/E05-S05.md | 70 | eab3f3f90e29 | workflow/recovery;trust/security |
| implementation/reviews/E06-S01.md | 214 | 77d4319f126f | browser/computer;shell/settings/motion;workflow/recovery;trust/security |
| implementation/reviews/E07-S00.md | 208 | dd55c6844e71 | shell/settings/motion;workflow/recovery;trust/security |
| implementation/reviews/E07-S01.md | 82 | 9c6610218f90 | shell/settings/motion;workflow/recovery;trust/security |
| implementation/reviews/E07-S02.md | 339 | c39c407041fc | browser/computer;workflow/recovery;trust/security |
| implementation/reviews/E08-S01.md | 91 | fa8b4073f2ab | agent/composer;browser/computer;shell/settings/motion;workflow/recovery |
| implementation/reviews/E08-S02.md | 200 | 31b088fa870b | shell/settings/motion;workflow/recovery;trust/security |
| implementation/reviews/E08-S03.md | 171 | f5ecbdf590d5 | agent/composer;browser/computer;shell/settings/motion;rendering;workflow/recovery;trust/security |
| implementation/reviews/E08-S04.md | 138 | cf454e755c84 | shell/settings/motion;workflow/recovery;trust/security |
| implementation/reviews/E08-S05.md | 43 | 0a3827edcf26 | browser/computer;shell/settings/motion;rendering |
| implementation/reviews/E08-S06.md | 82 | c96f52946611 | agent/composer;browser/computer;rendering;workflow/recovery;trust/security |
| implementation/reviews/E08-S07.md | 134 | 13920b5203b2 | shell/settings/motion;rendering |
| implementation/reviews/E08-S08.md | 157 | b4b8deeebd2d | browser/computer;shell/settings/motion;rendering;workflow/recovery |
| implementation/reviews/E09-S02.md | 93 | cc6f0bac35de | shell/settings/motion;workflow/recovery |
| implementation/reviews/E09-S05.md | 111 | b4b5b4ff18ad | shell/settings/motion;workflow/recovery;trust/security |
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
