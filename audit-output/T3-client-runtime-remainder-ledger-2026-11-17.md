# T3 `packages/client-runtime` Remainder Ledger

Pinned upstream checkout: `222288b00f9ed392e1e7352ad929c9ae9ac10cef`. This is a separate audit artifact; it does not modify LiteCowork source or the central audit report.

## Exact remainder inventory

Derived as the 420 tracked package paths from pinned `git ls-files` minus the central audit's reconciled G1–G15 282-path manifest. The manifest subtraction was validated against each file's physical line count. The known baseline is 282 files / 79,281 lines; the exact remainder is 138 files / 34,041 lines. File counts use physical text lines, including final unterminated lines.

| Path | Physical lines | Read status |
|---|---:|---|
| `packages/client-runtime/src/pullRequestChecks.test.ts` | 62 | Read complete 2026-11-17 |
| `packages/client-runtime/src/pullRequestChecks.ts` | 30 | Read complete 2026-11-17 |
| `packages/client-runtime/src/pullRequestDiffFold.test.ts` | 41 | Read complete 2026-11-17 |
| `packages/client-runtime/src/pullRequestDiffFold.ts` | 25 | Read complete 2026-11-17 |
| `packages/client-runtime/src/pullRequestEditing.test.ts` | 249 | Read complete 2026-11-17 |
| `packages/client-runtime/src/pullRequestEditing.ts` | 120 | Read complete 2026-11-17 |
| `packages/client-runtime/src/pullRequestLinkInput.test.ts` | 205 | Read complete 2026-11-17 |
| `packages/client-runtime/src/pullRequestLinkInput.ts` | 122 | Read complete 2026-11-17 |
| `packages/client-runtime/src/pullRequestMarkdown.test.ts` | 405 | Read complete 2026-11-17 |
| `packages/client-runtime/src/pullRequestMarkdown.ts` | 371 | Read complete 2026-11-17 |
| `packages/client-runtime/src/pullRequestProjectFilter.ts` | 52 | Read complete 2026-11-17 |
| `packages/client-runtime/src/pullRequestQuery.test.ts` | 162 | Read complete 2026-11-17 |
| `packages/client-runtime/src/pullRequestQuery.ts` | 160 | Read complete 2026-11-17 |
| `packages/client-runtime/src/pullRequestReactions.test.ts` | 96 | Read complete 2026-11-17 |
| `packages/client-runtime/src/pullRequestReactions.ts` | 112 | Read complete 2026-11-17 |
| `packages/client-runtime/src/pullRequestReply.ts` | 20 | Read complete 2026-11-17 |
| `packages/client-runtime/src/pullRequestReviewers.test.ts` | 131 | Read complete 2026-11-17 |
| `packages/client-runtime/src/pullRequestReviewers.ts` | 101 | Read complete 2026-11-17 |
| `packages/client-runtime/src/queuedRunEdit.test.ts` | 293 | Read complete 2026-11-17 |
| `packages/client-runtime/src/queuedRunEdit.ts` | 216 | Read complete 2026-11-17 |
| `packages/client-runtime/src/relay/cloudAccountService.test.ts` | 244 | Read complete 2026-11-17 |
| `packages/client-runtime/src/relay/cloudAccountService.ts` | 213 | Read complete 2026-11-17 |
| `packages/client-runtime/src/relay/discovery.test.ts` | 720 | Read complete 2026-11-17 |
| `packages/client-runtime/src/relay/discovery.ts` | 406 | Read complete 2026-11-17 |
| `packages/client-runtime/src/relay/dpopProof.test.ts` | 78 | Read complete 2026-11-17 |
| `packages/client-runtime/src/relay/dpopProof.ts` | 111 | Read complete 2026-11-17 |
| `packages/client-runtime/src/relay/errorPresentation.test.ts` | 47 | Read complete 2026-11-17 |
| `packages/client-runtime/src/relay/errorPresentation.ts` | 109 | Read complete 2026-11-17 |
| `packages/client-runtime/src/relay/index.ts` | 6 | Read complete 2026-11-17 |
| `packages/client-runtime/src/relay/managedRelay.test.ts` | 458 | Read complete 2026-11-17 |
| `packages/client-runtime/src/relay/managedRelay.ts` | 1,004 | Read complete 2026-11-17 |
| `packages/client-runtime/src/relay/managedRelayState.test.ts` | 279 | Read complete 2026-11-17 |
| `packages/client-runtime/src/relay/managedRelayState.ts` | 426 | Read complete 2026-11-17 |
| `packages/client-runtime/src/remotePerformance.bench.ts` | 141 | Read complete 2026-11-17 |
| `packages/client-runtime/src/reviewGuideTour.test.ts` | 188 | Read complete 2026-11-17 |
| `packages/client-runtime/src/reviewGuideTour.ts` | 194 | Read complete 2026-11-17 |
| `packages/client-runtime/src/rpc/client.test.ts` | 621 | Read complete 2026-11-17 |
| `packages/client-runtime/src/rpc/client.ts` | 566 | Read complete 2026-11-17 |
| `packages/client-runtime/src/rpc/http.ts` | 172 | Read complete 2026-11-17 |
| `packages/client-runtime/src/rpc/index.ts` | 4 | Read complete 2026-11-17 |
| `packages/client-runtime/src/rpc/protocol.ts` | 12 | Read complete 2026-11-17 |
| `packages/client-runtime/src/rpc/session.test.ts` | 1,108 | Read complete 2026-11-17 |
| `packages/client-runtime/src/rpc/session.ts` | 478 | Read complete 2026-11-17 |
| `packages/client-runtime/src/scheduledTaskTimeZone.test.ts` | 12 | Read complete 2026-11-17 |
| `packages/client-runtime/src/scheduledTaskTimeZone.ts` | 23 | Read complete 2026-11-17 |
| `packages/client-runtime/src/scheduledTaskWebhook.ts` | 134 | Read complete 2026-11-17 |
| `packages/client-runtime/src/secretRequest.test.ts` | 133 | Read complete 2026-11-17 |
| `packages/client-runtime/src/secretRequest.ts` | 237 | Read complete 2026-11-17 |
| `packages/client-runtime/src/sourceControlClients.test.ts` | 415 | Read complete 2026-11-17 |
| `packages/client-runtime/src/sourceControlClients.ts` | 66 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/checkpointDiff.ts` | 25 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/commandOutput.test.ts` | 24 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/commandOutput.ts` | 37 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/entities.test.ts` | 598 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/entities.ts` | 125 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/models.ts` | 407 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/orchestration.ts` | 264 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/orchestrationV2Projection.test.ts` | 895 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/orchestrationV2Projection.ts` | 467 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/orchestrationV2TestFixtures.ts` | 110 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/outdatedServerUpdate.ts` | 90 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/providerInstanceDisplay.test.ts` | 85 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/providerInstanceDisplay.ts` | 92 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/review.ts` | 134 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/serverUsage.test.ts` | 335 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/shell-sync.test.ts` | 1,681 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/shell.ts` | 903 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/shellCommands.ts` | 16 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/shellPullRequests.ts` | 84 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/shellReducer.test.ts` | 432 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/shellReducer.ts` | 272 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/shellSnapshotHttp.ts` | 160 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/snapshots.ts` | 20 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/sourceControl.ts` | 105 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/steadyShells.test.ts` | 338 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/steadyShells.ts` | 217 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/subagentDisplay.test.ts` | 52 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/subagentDisplay.ts` | 160 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/terminal.test.ts` | 431 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/terminal.ts` | 457 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/terminalOutput.ts` | 358 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/terminalSession.test.ts` | 395 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/terminalSession.ts` | 249 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/threadCheckpoints.test.ts` | 257 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/threadCheckpoints.ts` | 91 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/usage.test.ts` | 207 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/usage.ts` | 96 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/usageAccess.test.ts` | 66 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/usageAccess.ts` | 42 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/usageProgress.test.ts` | 120 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/usageProgress.ts` | 86 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/vcs.test.ts` | 890 | Read before this tranche |
| `packages/client-runtime/src/state/vcs.ts` | 401 | Read before this tranche |
| `packages/client-runtime/src/state/vcsAction.test.ts` | 591 | Read before this tranche |
| `packages/client-runtime/src/state/vcsAction.ts` | 598 | Read before this tranche |
| `packages/client-runtime/src/state/vcsCommandScheduler.ts` | 13 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/vcsRef.ts` | 9 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/vcsRefInvalidation.ts` | 81 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/vcsStatusRevalidation.test.ts` | 93 | Read complete 2026-11-17 |
| `packages/client-runtime/src/state/vcsStatusRevalidation.ts` | 72 | Read complete 2026-11-17 |
| `packages/client-runtime/src/t3ToolSummary.test.ts` | 100 | Read complete 2026-11-17 |
| `packages/client-runtime/src/t3ToolSummary.ts` | 489 | Read complete 2026-11-17 |
| `packages/client-runtime/src/textPaste.test.ts` | 12 | Read complete 2026-11-17 |
| `packages/client-runtime/src/textPaste.ts` | 82 | Read complete 2026-11-17 |
| `packages/client-runtime/src/threadPullRequestCompatibility.test.ts` | 85 | Read complete 2026-11-17 |
| `packages/client-runtime/src/threadPullRequestCompatibility.ts` | 57 | Read complete 2026-11-17 |
| `packages/client-runtime/src/threadPullRequests.test.ts` | 487 | Read complete 2026-11-17 |
| `packages/client-runtime/src/threadPullRequests.ts` | 28 | Read complete 2026-11-17 |
| `packages/client-runtime/src/threadReference.test.ts` | 115 | Read complete 2026-11-17 |
| `packages/client-runtime/src/threadReference.ts` | 69 | Read complete 2026-11-17 |
| `packages/client-runtime/src/userMessage.test.ts` | 236 | Read complete 2026-11-17 |
| `packages/client-runtime/src/userMessage.ts` | 139 | Read complete 2026-11-17 |
| `packages/client-runtime/src/voice-input/controller.test.ts` | 911 | Read complete 2026-11-17 |
| `packages/client-runtime/src/voice-input/controller.ts` | 706 | Read complete 2026-11-17 |
| `packages/client-runtime/src/voice-input/environment.ts` | 313 | Read complete 2026-11-17 |
| `packages/client-runtime/src/voice-input/index.ts` | 41 | Read complete 2026-11-17 |
| `packages/client-runtime/src/voice-input/stream.test.ts` | 108 | Read complete 2026-11-17 |
| `packages/client-runtime/src/voice-input/stream.ts` | 172 | Read complete 2026-11-17 |
| `packages/client-runtime/src/voice-input/transcription.ts` | 54 | Read complete 2026-11-17 |
| `packages/client-runtime/src/webhookAddress.ts` | 32 | Read complete 2026-11-17 |
| `packages/client-runtime/src/work-log/commandLabel.test.ts` | 169 | Read complete 2026-11-17 |
| `packages/client-runtime/src/work-log/commandLabel.ts` | 1,435 | Read complete 2026-11-17 |
| `packages/client-runtime/src/work-log/embeddedScripts.test.ts` | 162 | Read complete 2026-11-17 |
| `packages/client-runtime/src/work-log/embeddedScripts.ts` | 577 | Read complete 2026-11-17 |
| `packages/client-runtime/src/work-log/itemDetail.test.ts` | 184 | Read complete 2026-11-17 |
| `packages/client-runtime/src/work-log/itemDetail.ts` | 298 | Read complete 2026-11-17 |
| `packages/client-runtime/src/work-log/presentation.test.ts` | 206 | Read complete 2026-11-17 |
| `packages/client-runtime/src/work-log/presentation.ts` | 694 | Read complete 2026-11-17 |
| `packages/client-runtime/src/work-log/scrollAnchor.test.ts` | 17 | Read complete 2026-11-17 |
| `packages/client-runtime/src/work-log/scrollAnchor.ts` | 33 | Read complete 2026-11-17 |
| `packages/client-runtime/src/work-log/toolPresentation.test.ts` | 80 | Read complete 2026-11-17 |
| `packages/client-runtime/src/work-log/toolPresentation.ts` | 143 | Read complete 2026-11-17 |
| `packages/client-runtime/src/work-log/userInput.test.ts` | 39 | Read complete 2026-11-17 |
| `packages/client-runtime/src/work-log/userInput.ts` | 46 | Read complete 2026-11-17 |
| `packages/client-runtime/src/workspaceBasenameLookup.ts` | 237 | Read complete 2026-11-17 |
| `packages/client-runtime/src/worktreeSetup.ts` | 23 | Read complete 2026-11-17 |
| `packages/client-runtime/src/zip/boundedZip.ts` | 78 | Read complete 2026-11-17 |
| `packages/client-runtime/src/zip/zipDirectory.ts` | 75 | Read complete 2026-11-17 |

## Read tranche 1 — pull request checks, diff folds and editing

Read all six listed files line by line. The check projection keeps detail fields stable while replacing checks only when the timestamp/evidence says they are at least as fresh; merged status is sticky, with empty/running checks refreshed faster. Diff fold state combines per-file explicit choices over global expand/fold and viewed state. Pull request editability follows host-advertised capabilities and viewer/thread permissions: explicit host answers override fallback author/merge checks; comment kinds exclude review summaries. Maintainer-edit controls require an open request, all capability/permission gates and a defined host value. Description/maintainer drafts are local optimistic projections keyed to PR URL and host observation time; overlapping save counters preserve the last desired body, failed ambiguous saves keep the draft until a later host read. Tests cover timestamp ordering, per-file fold exceptions, capability/permission denial, overlapping saves, failure and host-read reconciliation. This is T3 client-runtime source/test reading only; tests were not run.

## Read tranche 2 — pull request link input, Markdown and query

Read all seven listed files line by line; physical counts from `Get-Content` matched the inventory (1,477 lines total). Link input delegates provider-specific URL and checkout-command grammar to the source-control registry, canonicalizes GitCafe routes, bounds bare-number resolution to the current thread repository, and requires an environment project capable of reading a full external URL. Project filtering deduplicates canonical repositories within an Environment, favors an explicitly selected Project, and disambiguates duplicate titles by environment/path/id.

Markdown handling preserves body offsets for edits, lifts only recognized HTTP(S) media, respects fenced/indented code, and bounds video/table scans. Table lifting is limited to structurally representable media tables; nested tables, captions, text outside cells, spans and malformed shapes stay as Markdown. Reference definitions are carried to split Markdown segments so links continue to resolve across lifted media; GitHub-style task ticks retain write-back offsets. Provider-specific media and issue-reference links come from the source-control registry. Query parsing recognizes a bounded set of typed filters, preserves unknown qualifiers as namespaced label filters or plain text as appropriate, ignores GitHub scope/order qualifiers owned by the list, and reconciles typed draft filters with menu state. Tests demonstrate intended behavior but were not run.

Relevance to LiteCowork's BYOA/provider-UI review is indirect: the design demonstrates provider-owned source-control grammar behind a shared registry, with generic rendering and bounded parsing at the UI layer. These files do not establish ACP capabilities, agent UI trees, subagent feeds, or any LiteCowork qualification. No source or tests in the T3 checkout were modified or run.

## Read tranche 3 — pull request reactions, reviewers and queued edits

Read all seven listed files line by line; physical counts from `Get-Content` matched the inventory (969 lines total). Reactions are gated by advertised reaction capability, with one provider/comment-kind exception, and optimistic pending state overlays only the viewer's choice until host detail catches up; the display order, tooltip actor/count wording, and signature are deterministic. Reply freshness compares merged-state stickiness, host update time, then observed time. Reviewer selection merges the loaded page with host search while deduplicating by kind/id, avoids showing stale debounce results, and explicitly reports searchable, unsearchable, failed, or truncated candidate sets rather than implying a complete list.

Queued-run editing has one watcher per thread/edit identity. Unknown run state waits; a run leaving the queued state delivers the latest edit once. Save claims pause exit observation, disallow parallel saves, and remain claimed across edit replacement/end so an older server-arriving save cannot race a newer one. On failed save the current edit resumes watching; deleted-thread state ends it, while archive/absence from shell does not falsely count as deletion. A changed edit can move into the ordinary draft only if that draft is empty, preventing hidden overwrite. Tests were read but not run; findings describe T3 code only, without implying LiteCowork integration or qualification.

## Read tranche 4 — relay account, proof, discovery and error handling

Read all nine listed files line by line; physical counts from `Get-Content` matched the inventory (1,934 lines total). Cloud account observation deduplicates repeated auth-provider emissions, synchronously deactivates credentials during account switch/sign-out, serializes cleanup and activation, uses a generation fence against stale async setup, cleans up on an initially signed-out observation, retries failed activation on the next same-account emission, and detaches replaced auth sources. DPoP proof creation normalizes the target URL, uppercases HTTP method, generates UUID/time claims, binds the optional access-token hash, signs ES256 with platform-supplied P-256 key material, and caches a successful key load but retries failed loads.

Relay discovery publishes a listed environment row before parallel status calls settle, validates status identity/endpoint against its listed record, protects state writes with account generations, coalesces list/foreground refresh work, retains rows on offline connectivity or failed foreground recheck, distinguishes unknown from unregistered until a successful list, resets state on credential changes/sign-out, and deduplicates repeated offline warnings. The error presenter maps typed relay conditions into user guidance, including conservative clock-skew wording when the DPoP cause is absent, explicit 504 recovery text, connection/link failures and released tunnels. These are T3 client-runtime code/test observations; tests were not run and no LiteCowork integration/qualification follows.

## Read tranche 5 — managed relay transport and account-scoped state

Read all four listed files line by line; physical counts from `Get-Content` matched the inventory (2,167 lines total). The client validates/normalizes secure HTTPS relay origins before constructing a generated API client; assigns an explicit timeout to relay calls; preserves typed protected errors, HTTP status and trace IDs; and separates ordinary bearer calls from DPoP-authorized endpoints. DPoP access tokens are scoped to account, client, relay origin, key thumbprint, granted scopes and expiry; scope grants must exactly match the request. A synchronized cache removes expired entries, coalesces misses, can persist tokens through an injected store, and invalidates/re-exchanges a rejected token for one retry. Tests cover concurrent scope misses, cache reuse across runtimes/session-token rotation, one stale-token refresh, invalid subject-token persistence denial, and 504 error presentation; tests were not run.

Managed relay UI state owns a stable account session whose Clerk-token reader can be replaced during same-account auth refresh. Concurrent reads coalesce, successful JWTs cache until expiry with skew, slow reads are replaced after four seconds and at most three attempts, and token-provider replacement fences stale reads from filling current cache. Account identity is checked before mutations; environment unlink and T3 Chat link/unlink are mediated through that session. Query atoms are account-keyed, stale-while-revalidate with idle TTL, publish start/success/failure events, and log each shared listing failure once. These patterns are useful BYOA-adjacent auth/session mechanics, but this service talks to T3 Connect relay, not native provider sign-in or ACP. It does not qualify LiteCowork's BYOA authentication, credential submission, or provider binding path.

## Read tranche 7 — environment RPC, WebSocket sessions and HTTP error mapping

Read all seven listed files line by line; physical counts from `Get-Content` matched the inventory (2,961 lines total). RPC client routes protected calls through a fail-closed permission guard when scopes are required, binds calls/subscriptions to active session identity, exposes request/subscription observers, and distinguishes finite stream commands from durable subscriptions. Durable streams switch with session replacement, tag buffered events with their producer session, retry transport and decode failures with capped exponential backoff, allow handled expected failures to retry except authorization denials, and park reported defects until a replacement or explicit resubscribe. Finite search streams do not restart on the same session. VCS capability-dependent refresh is decided and executed on one session.

The session factory owns one WebSocket per scoped session, adds optional multipart UTF-8 frame transport, sets a 15-second liveness timeout and disables lower-layer reconnect so the supervisor owns replacement. Readiness requires the initial config; config must match the expected Environment. It replays an authoritative config snapshot plus retained theme/source events, uses revision gaps to recover from a bounded sliding event buffer, tracks server clock, and exposes closure/probe/inbound-frame evidence. HTTP helpers normalize remote API base origins and map typed/common failures, invalid response schemas, undeclared HTTP statuses and request timeouts. Tests exercise large chunked config, scope closure, stream resubscribe and backoff, producer identity across session switches, event replay after slow consumers miss deltas/deletions, mismatched environment denial, clock handling and future config members. Tests were read but not run; these are T3-only source capabilities, not LiteCowork implementation evidence.

## Read tranche 8 — schedules, webhooks, secret requests and source-control registry

Read all seven listed files line by line; physical counts from `Get-Content` matched the inventory (1,020 lines total). Scheduled-task timezone labels use generic zone names without a run instant and DST-aware short names for a specific instant; unsupported zones return no label. Webhook form state parses max delivery age distinctly as unlimited/valid/invalid, gives filter rows local-only IDs, round-trips supported filter operations through the server's schema, enforces count limits, and detects whether filters changed so title-only edits omit the field and do not overwrite concurrent server changes.

Secret-request presentation differentiates local from inherited pending requests (only local rows are answerable), states privacy according to delivery mode, allows only HTTP(S) links, prefers an explicit agent URL, and extracts a fallback reason link only when there is exactly one distinct page. Configured secret-provider references are offered only when the host enables reference answers and a known provider config decodes; the value must match provider-specific syntax. Unknown failures use generic UI copy to avoid echoing secret-bearing payloads. Source-control clients delegate host URL, remote, repository identity and reference grammar to one registry; tests cover several forges, enterprise/self-hosted authorities, Azure URL canonicalization and safe rejection of ambiguous/malformed links. App sign-in target derivation is available only for remotes whose registered host definition advertises app sign-in on that host. No T3 tests were run; source-control sign-in target derivation is an adapter capability and does not itself prove an authenticated user bind/send flow.

## Read tranche 6 — remote performance harness and review guide diff excerpts

Read all three listed files line by line; physical counts from `Get-Content` matched the inventory (523 lines total). The benchmark source defines in-memory HTTP fixtures for descriptor lookup, WebSocket ticket issuance, bounded snapshots, and 200 projection updates at 100/1,000 loaded messages; these are benchmark definitions, not executed results. Review-guide utilities clamp tour navigation, collect referenced files, map patch content to old/new line numbers, include paired replaced lines at excerpt edges, rebuild independent patch hunks, preserve final-newline and CRLF behavior, and decline synthetic expansion when an excerpt crosses source hunks. Test code covers those cases, but neither tests nor benchmarks were run.

## Read tranche 10 — orchestration event projection and provider display

Read all seven listed files line by line; physical counts from `Get-Content` matched pinned HEAD (1,873 lines total). The event reducer scopes events to the current Thread, handles thread/run/attempt/provider-session/provider-thread/provider-turn/message/plan/turn-item/checkpoint/context entities, retains omitted token-use and deduped message/subagent bodies, and preserves deferred-work cursors. It maintains a visible-turn projection with authoritative ordinals, stable unchanged row identity, localized item replacement, hidden queued user inputs until dispatch, inherited-row preservation, and visibility updates when run/attempt state changes. Partial timelines use a watermark/window policy that retains in-flight run items but drops missing older completed history; leaving queue restores only items actually received. Tests also assert that streamed replacement avoids full row/run/ordering scans at representative collection sizes; tests were read, not run.

Provider display logic resolves labels from provider metadata/instance identity, normalizes only six-digit hex accents, computes a readable black/white foreground by relative luminance, and decides whether identity badges are needed while distinguishing ACP registry agent IDs and historical disabled entries. Outdated server update commands are Environment-scoped and single-flight, expose stages, and clear versus retain a failed stage on success/interruption/failure. Review data uses bounded patch-read concurrency, stale preview revision invalidation, tracked-only incomplete preview fallback for older servers, and single-flight file-content reads keyed by Environment and diff identity. These are T3 client-runtime behaviors only; no tests were executed and no LiteCowork integration/qualification is implied.

## Read tranche 9 — client state projection and streamed command output

Read all seven listed files line by line; physical counts from `Get-Content` matched the pinned inventory (1,480 lines total). Scoped project/thread keys encode Environment identity as part of the key and parse with typed invalid-key errors; project-reference collections use schema-decoded JSON tuples, and equality helpers preserve ordered collection semantics. Client shell projection keeps the native thread projection nested, strips volatile streaming fields from structural views for identity stability, and derives run/runtime summaries, activity timing, provider-history stack, pending question/approval state, background-work idle parking, and subagent display title. The tests exercise lifecycle precedence among active/terminal runs, pending asynchronous questions, background work and stale shell status, plus projection identity and display joins for pending native request items.

Orchestration query wiring scopes atoms/subscriptions by Environment and account identity, conditionally falls back to legacy metadata only when the server advertises the needed capability, and binds command output and search/diff queries to the active RPC session. Command output uses bounded full-tail replacement frames for initial/final/behind-client recovery and append frames for incremental output; the client shares terminal normalization rules across frame boundaries, including ANSI escape state and carriage-return redraw. Settled command output ends its subscription and retains final cached output. Stop/force-stop and destructive worktree deletion flows read fresh state and confirmation details before acting. Checkpoint diff target/state shapes explicitly include Environment, Thread, turn range, whitespace mode and optional cache scope. Test files were read only; no T3 tests or benchmarks were run. These findings are T3 client-runtime patterns, not LiteCowork implementation or qualification evidence.

## Read tranche 11 — usage refresh and shell stream synchronization tests

Read both listed files line by line; physical counts from `Get-Content` matched pinned HEAD (2,016 lines total). Usage-state tests use a controlled RPC/config harness to demonstrate that a settings/pricing change interrupts and restarts an in-flight read, cached figures remain visible through an awaited refresh failure, failed provisional saved reads fall through to a full read, and a later refresh requests exact current figures instead of provisional ones. This is test intent/evidence in source only; the tests were not run.

Shell synchronization tests cover bounded stream batches and event cursor advancement, ignored future event kinds, sequence-aware removals and authoritative reset generations, unchanged write suppression, debounced persistence, retries after save failure, flush on disconnect/scope teardown, and protection from older in-flight saves overwriting same-sequence enrichment. Cold start tests specify when a cached shell may resume (matching server version/database migration/omission set and freshness), when HTTP reload is needed, capability-gated shell omissions, reconnect cursor reuse, migration-triggered reload, credential rotation without reload, and race handling when connection/session loads overlap or finish late. They also cover defect recovery/resubscription with cursor continuity and `snapshot-required` fallback from warm/cold stream to HTTP. These are T3 tests read but not executed; they do not establish LiteCowork behavior.

## Read tranche 12 — shell snapshot lifecycle and HTTP resume path

Read five files line by line; physical counts from `Get-Content` matched pinned HEAD (1,183 lines total): shell.ts (903), shellCommands.ts (16), shellPullRequests.ts (84), shellSnapshotHttp.ts (160), and snapshots.ts (20). The shell owner maintains cached/synchronizing/live states, authoritative generations and removal cursors; it validates cache-resume identity against server version, migration, shell omission capabilities and a 12-hour age window. It coordinates early HTTP loads with live socket sessions, single-flight/replacement-safe connection loads, short grace for resumable rows, sequence/fallback cursor construction, HTTP/socket agreement on omitted children, and `snapshot-required` reloads. Stream errors stay user-safe and defects retry on backoff; wakeups request resubscription.

Shell event writes serialize against deferred PR enrichment, reject events from replaced sessions, maintain monotonic cursors except authoritative resets, and only advance generation on authoritative snapshots. Cache writes are bounded/coalesced, avoid rewriting unchanged rows while retaining cursor state, retry failed writes, and flush stable snapshots on disconnect/teardown. Deferred PR links decode in batches after yielding to the host, are schema-validated and logged/discarded on invalid shape, and only fill rows belonging to that exact snapshot so older async results cannot overwrite newer rows. The HTTP loader uses the shared authenticated Environment HTTP boundary, DPoP and remote authorization when available, typed schema/common-error handling, protocol headers and a 20-second timeout; failure falls back to the socket snapshot. The direct open-in-editor RPC is Environment-scoped. These are T3 client-runtime source observations; no tests were run and no LiteCowork behavior or qualification is implied.

## Read tranche 13 — shell reduction, multi-environment list stability and subagent display

Read seven files line by line; physical counts from `Get-Content` matched pinned HEAD (1,576 lines total): shellReducer.test.ts (432), shellReducer.ts (272), sourceControl.ts (105), steadyShells.test.ts (338), steadyShells.ts (217), subagentDisplay.test.ts (52), and subagentDisplay.ts (160). Shell reducers ignore stale sequences, retain repository identity only while workspace roots match, preserve thread/project object identity for equal rows, and separate metadata-only repository-identity enrichment from authoritative snapshots so a lower-sequence enrichment cannot regress thread/project structure or resurrect rows. Stream updates keep legacy archive deltas out of the normal bounded shell, preserve explicit link deletion, and batch deltas with lazy indexes; tests compare batched reduction with sequential reduction and check reference identity.

The steady-shell projection holds server updates during reconnect for multi-Environment thread lists, synchronizes row release within a 2-second window, releases on timeout/disconnect/failure, follows a live Environment after user row actions, and keeps point reads live while stabilizing cross-Environment lists. Tests cover reconnect replay, cache arrival, user actions, enable/disable removal, and timer wakeup; tests were not run. Source-control atoms expose provider discovery/app-access/repository lookup, star checks, clone start/cancel/retry serialized per Environment, sign-in start/wait/cancel independently of clone queues, clone progress subscription, and repository publishing serialized with VCS invalidation. Subagent presentation derives concise status/group text, model and project/worktree metadata, safe 280-character previews, Unicode-aware truncation, and sanitized card details. No T3 tests were executed; these patterns do not establish LiteCowork integration or qualification.

## Read tranche 14 — terminal attachment lifecycle and bounded output

Read three files line by line; physical counts from `Get-Content` matched pinned HEAD (1,246 lines total): terminal.ts (457), terminal.test.ts (431), and terminalOutput.ts (358). Terminal RPC atoms distinguish observing from attaching, detach immediately when an attached viewer unmounts, serialize lifecycle commands per Environment/Thread, and coalesce resize commands to the latest size per terminal session. Lifecycle transitions track whether the user has already observed output and whether the shell was running; an attached shell exit closes/reports once, a shell ending while detached may reopen on explicit return, and a read-only observation prevents a later grant from unexpectedly reopening old history. Operate permission is checked immediately before resize/write/open/close actions. Initial launch input is sent once only after a measured grid resize succeeds; failed resize retries once unless superseded, and each new output generation replays the grid. Tests cover revocation, detachment, exit/reopen, regrant and resize/input races, but were not run.

Terminal output is retained in bounded UTF-8-aware chunks and byte accounting, with monotonic code-unit offsets, generation/reset cursors, capped chunk count/compaction and parser state at a trimmed boundary. Partial escape sequences are skipped correctly on replay; display-only clear preserves emulator modes/history while marking a new text start. Readers receive reset for new generation/reset or lost history and append for unread suffixes; incomplete terminal sequences are filtered consistently for both emulator replay and plain text. These behaviors are T3 source observations only; no tests were run and no LiteCowork provider or terminal qualification is implied.

## Read tranche 15 — terminal state reduction and checkpoint summaries

Read four files line by line; physical counts from `Get-Content` matched pinned HEAD (992 lines total): terminalSession.ts (249), terminalSession.test.ts (395), threadCheckpoints.ts (91), and threadCheckpoints.test.ts (257). Terminal attach reducers distinguish metadata summaries from authoritative live attach status, preserve output while updating shared PTY grid, increment attachment generation on reattach so renderers replay their measured grid, and advance lifecycle version on restart. Snapshot/output/clear/exit/closed/error/activity events update only their owned state; metadata applies snapshot/upsert/remove by thread+terminal, and combined state chooses attach status once attach evidence exists. Tests cover Unicode and byte-tail retention, escape-sequence truncation, delayed renderer cursor resets/appends, coalesced unread output, bounded chunk inspections, clear preserving program modes, reconnect generation and restart behavior. Tests were read but not run.

Checkpoint summaries filter records without a run or turn ordinal, preserve checkpoint order/status/file references, convert capture time to ISO, and associate each checkpoint to the last assistant message in array order for that run (not timestamp order). A reverse cursor indexes only requested runs, avoids scanning unrelated older history after requested tails are found, does not read message bodies, and returns null where no assistant ID exists. File totals use server totals when supplied or sum local prefix entries as a fallback. Tests cover duplicate/interleaved runs, replacements without stale ID retention and linear visitation bounds; tests were not run. These are T3 client-runtime observations only.

## Read tranche 16 — usage access, refresh and progress states

Read six files line by line; physical counts from `Get-Content` matched pinned HEAD (617 lines total): usage.ts (96), usage.test.ts (207), usageAccess.ts (42), usageAccess.test.ts (66), usageProgress.ts (86), and usageProgress.test.ts (120). Usage reads apply each Environment's own configured model filter. Manual refresh starts pricing refresh and then replaces/reruns the usage query, stops waiting for scans when that Environment disconnects, and awaits connected scans; limits refresh coalesces per Environment and applies a five-minute automatic cooldown. Tests cover price success/failure, per-Environment filter isolation, disconnect and concurrent/duplicate limit refresh behavior; tests were not run.

Diagnostics/usage access is gated specifically by the diagnostics-read session grant; pending is limited to connection phases that can complete session verification, known grants can retain cached summaries offline, and denial is distinguished from access-check failure. Progress projection separates inactive/loading/stale/ready/partway, labels waiting Environments or refreshing provider sources, and dims only when none of the available Environments has answered. These are T3 client-runtime patterns, not LiteCowork authorization/product evidence; tests were not run.

## Read tranche 17 — VCS refresh coordination, tool summaries and text paste

Read nine files line by line and checked each file's physical line count against the pinned Git blob at `222288b00f9ed392e1e7352ad929c9ae9ac10cef` (951 lines total): vcsCommandScheduler.ts (13), vcsRef.ts (9), vcsRefInvalidation.ts (81), vcsStatusRevalidation.test.ts (93), vcsStatusRevalidation.ts (72), t3ToolSummary.test.ts (100), t3ToolSummary.ts (489), textPaste.test.ts (12), and textPaste.ts (82). The VCS command scheduler serializes operations by Environment and working directory; VCS ref cache invalidation increments an Environment-scoped revision and takes a partitioned persistence lock so invalidated snapshots cannot be rehydrated or repersisted from stale refreshes. Status refresh is shared per Environment/checkout, wakes on debounced host signals and reconnects, and unsubscribes/cancels a pending debounce when the last observer leaves. Tests encode these cases but were not run.

The T3 tool-summary mapper parses bounded nested MCP-style result envelopes, distinguishes transport-complete calls from embedded provider errors, reports child task failures/timeouts separately from tool-call failure, and emits user-facing summaries for many T3 orchestration actions. Its tests cover envelope variants, PR batch counts, failed/unfinished sends and child outcomes. Text paste uses platform-aware Ctrl/Cmd+Shift+V detection, byte-sensitive attachment thresholds, collision-free names and selection/length helpers. This is presentation and input normalization in T3; it does not define LiteCowork's ACP protocol, canonical RichPresentation contract, permissions or provider qualification. No T3 tests were run.

## Read tranche 18 — thread pull-request and message presentation

Read eight files line by line; physical counts matched the ledger and pinned checkout at `222288b00f9ed392e1e7352ad929c9ae9ac10cef` (1,216 lines total): threadPullRequestCompatibility.test.ts (85), threadPullRequestCompatibility.ts (57), threadPullRequests.test.ts (487), threadPullRequests.ts (28), threadReference.test.ts (115), threadReference.ts (69), userMessage.test.ts (236), and userMessage.ts (139). Thread pull-request linking remains disabled until per-Environment capabilities load; mutations preserve host ports and test server-state eligibility before enabling watches. The shared pull-request projection handles legacy Forgejo/Azure identity normalization, deterministic open/terminal selection, native or derived stack order, branched/cyclic/reused-branch ambiguity, aggregate state, and search terms; it avoids guessing a chain when identity/order is ambiguous. Tests were read, not run.

Copying a thread reference produces both a hosted URL and a scoped clipboard context chip; the currently open PR panel URL takes precedence, then projected PR links and legacy fallback. User-message presentation keeps guest attribution separate from the agent-facing stored text, distinguishes user/agent/server/automation messages, removes legacy scheduled-task wrapper text only for recognized scheduler messages, hides synthetic attachment-only bootstrap prompts, and groups adjacent own messages while identifying continued steer messages. File-only cards appear only when every attachment is a non-media file and the body contains only valid bound file references. These T3 client display and context clipboard patterns are not LiteCowork ACP/provider contracts; tests were not run.

## Read tranche 19 — voice input, streaming transcription and webhook address display

Read eight files line by line; physical counts from the pinned checkout at `222288b00f9ed392e1e7352ad929c9ae9ac10cef` total 2,337 lines: controller.test.ts (911), controller.ts (706), environment.ts (313), index.ts (41), stream.test.ts (108), stream.ts (172), transcription.ts (54), and webhookAddress.ts (32). Voice capture checks support and permission before microphone use, caps capture at five minutes, owns one active recorder/transcription operation, keeps stale composer owners from receiving transcripts, and handles cancellation, app backgrounding, interruption, preparation, transcription, post-processing, retained-audio retry and cleanup. Draft insertion maps the original UTF-16 selection through concurrent edits, protects surrogate pairs, and applies language-sensitive word spacing. Streaming audio uses an acknowledged two-frame window, bounded byte/chunk queues, consent checks before send/receive, revision checks, finish/drain ordering and a 120-second response deadline; environment HTTP calls use authenticated scoped requests, redirect rejection, DPoP/remote authorization where available, redacted secret values and explicit long-operation timeouts. Test files were read but not run.

Webhook address display distinguishes a public URL, a local-only loopback URL and an unlinked relative path. These are T3 voice/webhook product choices and cannot be imported as LiteCowork capabilities or consent policy without its owner contracts; no LiteCowork speech or webhook qualification is implied.

## Read tranche 20 — work-log command, tool and item presentation

Read 14 files line by line; physical counts matched the ledger and pinned checkout at `222288b00f9ed392e1e7352ad929c9ae9ac10cef` (4,083 lines total): commandLabel.test.ts (169), commandLabel.ts (1,435), embeddedScripts.test.ts (162), embeddedScripts.ts (577), itemDetail.test.ts (184), itemDetail.ts (298), presentation.test.ts (206), presentation.ts (694), scrollAnchor.test.ts (17), scrollAnchor.ts (33), toolPresentation.test.ts (80), toolPresentation.ts (143), userInput.test.ts (39), and userInput.ts (46). The work log selects friendly labels/icons/group summaries from structured T3 tool identity, status and bounded output; distinguishes tool failure from child-task failure and reasoning text, suppresses successful entries according to visibility settings, and separates compact live activity from detailed output fetches. Detail rendering preserves significant structured/image output, protects large JSON numbers from lossy pretty-printing, and avoids displaying provider UI wrapper metadata as the tool result. Computer/browser surface labels are normalized only from known surface/source metadata or a narrow known T3 tool-code pattern; URL/icon and workspace image references are length/protocol/path checked. User-input answer history removes the structured provider context envelope for display while preserving literal user-typed XML.

Command display helpers parse shell/PowerShell command text without executing it, infer a program label conservatively across bounded wrappers, segments and nesting, and decline to guess on malformed quoting; they also expose control characters visibly. Embedded-script highlighting uses a parser and original-source offsets to syntax-color known inline scripts/heredocs (including nested shell and SQL cases) while leaving unsupported or malformed bodies alone. Scroll anchoring resolves the actual row from pixel offset rather than trusting a virtualizer range that can lag during fling. Tests were read but not run. These are T3 presentation mechanisms, not LiteCowork ACP, universal UI-tree or provider qualification evidence.

## Read tranche 21 — workspace file-link lookup, setup status and bounded ZIP preview

Read the final four files line by line; physical counts matched the pinned checkout at `222288b00f9ed392e1e7352ad929c9ae9ac10cef` (413 lines total): workspaceBasenameLookup.ts (237), worktreeSetup.ts (23), boundedZip.ts (78), and zipDirectory.ts (75). Workspace file-link lookup accepts only a unique match across basename, repeated project prefixes or suffix paths; it treats Windows case folding separately, limits cross-directory discovery to four roots and filesystem metadata discovery to the contract batch limit, and lets the newest panel lookup supersede older calls. It infers agent working directories only from literal absolute `cd` operands before the cited reply, with shell parsing safeguards and a bounded set. Worktree setup projection prefers the highest live/recorded sequence, keeps active/failure state visible, and dismisses successful completion after the agent starts or a follow-up arrives.

ZIP preview performs central-directory validation before JSZip allocation, limits total input to 50 MiB, entries to 2,048, paths to 1,024 characters, each expanded file to the passed entry limit, expanded total to the configured/default limit, compression ratio to 200 (with an optional floor), and XML/markup limits; unsupported or inconsistent ZIP64 layouts are rejected. These are T3 preview controls and UX, not LiteCowork workspace-root authorization or artifact-import contracts. No tests were run. This completes all 138 rows in this exact remainder ledger (34,041 listed physical lines, including four vcs rows already covered by central G16); the central and remainder manifests therefore cover 420/420 unique client-runtime package paths when deduplicated.
