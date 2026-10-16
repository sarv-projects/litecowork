# LiteCowork Agent Portfolio — 2026-10-16

## Purpose and scope

This document records the owner-directed high-value portfolio of 15 local agent
profiles for the desktop product. It is a product priority roster, **not an empirically
ranked list of the 15 most-used agents**; no comparable usage dataset was available for
that claim.

This records the owner's direction to pursue a 15-agent V1 portfolio and expands the
four agents explicitly named for qualification in [`SCOPE.md`](SCOPE.md): Claude Code,
Codex CLI, OpenCode, and Cline CLI. SCOPE and E03-S06–S11 now make this roster the
formal implementation and qualification target. Recording a profile here does not say
that its adapter is implemented, that its release gate is complete, or that it is
supported.

## Status vocabulary

- **Discovered:** LiteCowork has read bounded local installation/version information.
  This does not establish protocol support, authentication, a usable model, or a
  session.
- **Profile observed:** a bounded, owner-triggered probe has recorded sanitized agent
  metadata and readiness observations. A catalog observation does not establish
  inference entitlement or Task execution.
- **Module implemented:** trusted LiteCowork code owns the agent's versioned launch,
  authentication/configuration, runtime, capabilities, and lifecycle contract. A
  registry record or executable path is not a module.
- **Protocol handshake:** a well-formed handshake proves only that the two sides can
  negotiate that protocol version and exchange its initial metadata. It does not prove
  session creation, policy enforcement, effects, cancellation, or recovery.
- **Qualified support:** the exact agent/module/version/OS combination has passed its
  required CODE, SYSTEM, and owner USER acceptance evidence. Claims are limited to the
  capabilities actually tested.

## Source-backed LiteCowork status

Current source has bounded Codex and OpenCode installation/probe paths. The local
inventory also observes Cline's matching npm package manifest or uses a bounded
`--version` fallback; it starts discovery for these three agents in
[`apps/litecoworkd/src/agents/mod.rs`](../apps/litecoworkd/src/agents/mod.rs); the
profile-probe Operator route accepts only `CODEX` and `OPENCODE` in
[`apps/litecoworkd/src/operator.rs`](../apps/litecoworkd/src/operator.rs). Their probes
observe limited catalog/readiness data. Codex App Server and OpenCode Server transport
components also exist, but there is no generic ACP client or complete shared session
adapter wired through durable Task/Attempt admission, Trust, Environment, Effect and
recovery semantics. The current source therefore does **not** implement or qualify a
15-agent session portfolio.

The source-backed status by roster member is:

- **Codex CLI:** bounded version discovery and owner-triggered profile/catalog
  observation exist. Codex App Server components exist, but the complete durable Task
  session adapter and qualified support do not.
- **OpenCode:** bounded version discovery and provider/model catalog observation exist;
  version-specific Server transport components also exist. A complete durable Task
  session adapter and qualified support do not.
- **Cline CLI:** bounded installation inventory exists. It does not inspect native
  authentication, initialize ACP, create an AgentProfile, or qualify a session adapter.
- **Claude Code and the remaining 11 entries:** no dedicated local discovery/probe
  module or qualified session adapter was found in the current agent source. No
  qualified support evidence was found.

[`E03-S06`](epics/E03.md) remains the planned AgentModule registry and ACP discovery
slice. [`E03-S11`](epics/E03.md) requires versioned conformance evidence for launch-
critical modules. The architecture explicitly forbids treating ACP registry presence
or an installed binary as support ([`AGENT-CONTROL.md`](../docs/AGENT-CONTROL.md)).

The ACP Registry project's protocol adaptation matrix is useful discovery research, not
LiteCowork product evidence. Checked 2026-10-16, the pinned registry commit below contains
a matrix generated 2026-10-15: 36 agents probed and 35 successful `initialize` handshakes.
Those results do not establish LiteCowork's installation, auth, isolation, cancellation,
or Task integration. The matrix probes `session/stop`, which is not the standard ACP
`session/cancel`; its method summary does not test `session/cancel` and must not be read
as evidence either for or against an agent's cancel behavior. Manifest versions also
differ from the matrix probes for Claude ACP (0.91.0 vs 0.90.0), Cline (3.0.72 vs
3.0.71), and OpenCode (1.19.2 vs 1.19.0); their matrix capabilities are not version-exact
evidence for the pinned manifests. The Qwen registry entry is version 0.25.2 while the
matrix probes 0.25.1, so its capabilities are likewise not version-exact. [Pinned
registry commit and matrix](https://github.com/agentclientprotocol/registry/tree/4d1729af836732b46840080920912dac7d5a2935), [matrix snapshot](https://github.com/agentclientprotocol/registry/blob/4d1729af836732b46840080920912dac7d5a2935/.protocol-matrix/latest.md)

Rows 11–15 were checked against vendor documentation and this immutable Registry
snapshot on 2026-10-16. “Native ACP” below means the vendor documents an ACP launch mode;
it is not LiteCowork support or proof that the pinned Registry command is current. Keep
release, manifest, and matrix versions distinct where they disagree. In particular, the
active Kimi Code CLI is a different project from the archived Kimi CLI entry, and the
Factory manifest, matrix, and current CLI reference do not agree on a launch version or
ACP output name.

## Priority roster and adapter paths

The preferred paths below identify plausible integration seams. ACP standardizes the
communication channel between an agent and its client; that does not provide LiteCowork's
policy, lifecycle, or Task guarantees ([ACP introduction](https://agentclientprotocol.com/get-started/introduction)).
The generic ACP client is a **reuse hypothesis** until LiteCowork implements and verifies
it. Even with that client, every agent requires a trusted AgentModule for launch/version
rules, auth and native configuration ownership, capability and option descriptors,
platform support, and CODE/SYSTEM/USER qualification.

| # | Agent profile | Proposed adapter path | Upstream evidence and qualification notes |
|---:|---|---|---|
| 1 | Claude Code | Profile candidate: native `claude` CLI (interactive; `claude -p "query"` for noninteractive use). Native ACP module is not established; assess Claude Agent SDK separately. | Anthropic's CLI docs checked 2026-10-16 document interactive, print, continue, and resume modes but no native ACP launch. Registry ID `claude-acp` is the separate `@agentclientprotocol/claude-agent-acp` SDK-based bridge, version 0.91.0, not native CLI parity. Its pinned README notes that background child processes may continue after cancellation, so a bridge cancel response cannot establish quiescence. Native CLI auth status/login does not prove the bridge's SDK auth state. Safe now: a curated profile; adapter work must name the SDK/bridge as a separate module. Unqualified for LiteCowork sessions. Sources checked 2026-10-16: [Anthropic CLI](https://docs.anthropic.com/en/docs/claude-code/cli-usage), [Claude Agent SDK overview](https://platform.claude.com/docs/en/cli-sdks-libraries/overview), [pinned bridge README, c0af592d252e6bd072b53407f618a9dd4cd02797](https://github.com/agentclientprotocol/claude-agent-acp/blob/c0af592d252e6bd072b53407f618a9dd4cd02797/README.md), [pinned Registry entry, commit 4d1729af836732b46840080920912dac7d5a2935](https://github.com/agentclientprotocol/registry/blob/4d1729af836732b46840080920912dac7d5a2935/claude-acp/agent.json) |
| 2 | Codex CLI | Profile candidate; investigate native `codex app-server` and the separate `codex-acp` bridge as distinct module paths. | OpenAI documents native App Server launch as `codex app-server`, using stdio newline-delimited JSON / JSON-RPC 2.0 for initialize, thread/turn lifecycle, streamed events, and approval requests. OpenAI marks the command and WebSocket transport experimental and not supported for production workloads; WebSocket is unsupported. Registry ID `codex-acp` is `@agentclientprotocol/codex-acp` 2.2.2, a distinct ACP translation bridge that starts App Server. Preserve native auth/config and approval/sandbox semantics; model catalog is not entitlement. Safe now: profile only; native App Server and ACP bridge are separate adapter candidates. Unqualified for LiteCowork sessions. Sources checked 2026-10-16: [Codex as a platform](https://developers.openai.com/blog/codex-as-a-platform), [App Server and account/catalog caveats](https://developers.openai.com/siwc/token-sharing-open-source/codex-app-server), [pinned bridge README, 202e66e63343ec4f5a5e35ab590251d813d129ca](https://github.com/agentclientprotocol/codex-acp/blob/202e66e63343ec4f5a5e35ab590251d813d129ca/README.md), [pinned Registry entry, commit 4d1729af836732b46840080920912dac7d5a2935](https://github.com/agentclientprotocol/registry/blob/4d1729af836732b46840080920912dac7d5a2935/codex-acp/agent.json) |
| 3 | OpenCode | Profile candidate; direct native ACP module (`opencode acp`) and the existing version-pinned OpenCode Server path are distinct adapter options. | Vendor docs checked 2026-10-16 document `opencode acp` over stdio as native ACP. Registry ID `opencode` is the native project entry, version 1.19.2 (binary distribution argument `acp`), not a third-party bridge. ACP use follows native config/tools, but `/undo` and `/redo` are unsupported; native MCP/plugins and tool execution need mediation before governed Task use. Preserve native auth/configuration, including its local auth store. Safe now: profile and adapter candidate only. Unqualified for LiteCowork sessions; current LiteCowork probe/server components are not session qualification. Sources checked 2026-10-16: [ACP support](https://opencode.ai/docs/acp/), [CLI reference](https://opencode.ai/docs/cli/), [pinned Registry entry, commit 4d1729af836732b46840080920912dac7d5a2935](https://github.com/agentclientprotocol/registry/blob/4d1729af836732b46840080920912dac7d5a2935/opencode/agent.json) |
| 4 | Cline CLI | Profile candidate; direct native ACP module (`cline --acp`) over stdio. | Vendor source pinned at commit `01b8a9724da905423e636396d7fb4acdf47796bb` documents `cline --acp`; Registry ID `cline` records package version 3.0.72 and the same native launch mode. The default file-edit and command prompts are handled by the ACP client; `--auto-approve true` disables those prompts and is not a safe substitute for LiteCowork Trust. Auth may use Cline sign-in/subscription, provider configuration, or `CLINE_API_KEY`; preserve native ownership and verify it per module. Safe now: profile and direct native ACP adapter candidate. Unqualified pending auth, permission, cancellation/quiescence, and provider evidence. Sources checked 2026-10-16: [pinned Cline ACP docs, commit 01b8a9724da905423e636396d7fb4acdf47796bb](https://github.com/cline/cline/blob/01b8a9724da905423e636396d7fb4acdf47796bb/docs/usage/acp.mdx), [pinned Registry entry, commit 4d1729af836732b46840080920912dac7d5a2935](https://github.com/agentclientprotocol/registry/blob/4d1729af836732b46840080920912dac7d5a2935/cline/agent.json) |
| 5 | Gemini CLI | Profile candidate; direct native ACP module (`gemini --acp`) over stdio. | Google source pinned at commit `b25669d84e1a404e30c176d779e732e25dafcec0` documents native ACP JSON-RPC, session creation/loading, prompting, cancellation, and an ACP file service proxied by the client. Registry ID `gemini` records package version 0.64.0 and `--acp`. Native auth may come from cached browser sign-in, environment credentials, or Vertex AI setup; `.env` loading and user/project/system configuration mean the module must control the child environment and preserve native config ownership. The CLI also has prompt, auto-edit, and YOLO approval modes; do not equate those with Core Trust. Safe now: profile and direct native ACP adapter candidate. Unqualified pending scoped file access, auth, cancellation/quiescence, and OS/provider evidence. Sources checked 2026-10-16: [pinned ACP docs, commit b25669d84e1a404e30c176d779e732e25dafcec0](https://github.com/google-gemini/gemini-cli/blob/b25669d84e1a404e30c176d779e732e25dafcec0/docs/cli/acp-mode.md), [pinned auth docs](https://github.com/google-gemini/gemini-cli/blob/b25669d84e1a404e30c176d779e732e25dafcec0/docs/get-started/authentication.mdx), [pinned configuration docs](https://github.com/google-gemini/gemini-cli/blob/b25669d84e1a404e30c176d779e732e25dafcec0/docs/reference/configuration.md), [pinned Registry entry, commit 4d1729af836732b46840080920912dac7d5a2935](https://github.com/agentclientprotocol/registry/blob/4d1729af836732b46840080920912dac7d5a2935/gemini/agent.json) |
| 6 | GitHub Copilot CLI | Native CLI ACP server: `copilot --acp` (stdio preferred); not the separate Copilot language-server ACP entry | **Public preview** as of 2026-10-16. The immutable registry record at commit `4d1729a` identifies `github-copilot-cli` 1.0.98 and `@github/copilot@1.0.98 --acp`; its matrix row reports `initialize`, `loadSession`, and `session/list`. GitHub login or configured BYOK providers are used. Tool filters and reasoning effort are launch-time server settings applied to every session that connects; ACP `session/new` cannot set them per session. Isolate server processes by Task/configuration, or enforce a least-privilege shared setting. Test permission denial and cancellation rather than inferring them from the handshake. [Pinned registry entry](https://github.com/agentclientprotocol/registry/blob/4d1729af836732b46840080920912dac7d5a2935/github-copilot-cli/agent.json), [GitHub ACP docs](https://docs.github.com/en/copilot/reference/copilot-cli-reference/acp-server) |
| 7 | Qwen Code | Native CLI ACP mode: `qwen --acp`; registry launch also enables `--experimental-skills` | `--acp` is documented as **stable** as of 2026-10-16. The pinned registry entry specifies package 0.25.2, while its 2026-10-15 protocol matrix probes 0.25.1; do not treat that handshake as version-exact evidence for 0.25.2. The entry's experimental-skills opt-in must remain an explicit module choice. Qwen OAuth was discontinued on 2026-04-15; current setup uses ModelStudio API/Coding Plan, third-party, or custom providers. Preserve native settings and qualify approval modes, sandbox policy, tool denial, cancellation, and restart/reload. The CLI's `--worktree` option cannot be combined with ACP. [Pinned registry entry](https://github.com/agentclientprotocol/registry/blob/4d1729af836732b46840080920912dac7d5a2935/qwen-code/agent.json), [Qwen v0.25.2 auth](https://github.com/QwenLM/qwen-code/blob/v0.25.2/docs/users/configuration/auth.md), [ACP flag and settings](https://github.com/QwenLM/qwen-code/blob/v0.25.2/docs/users/configuration/settings.md), [worktree constraint](https://github.com/QwenLM/qwen-code/blob/v0.25.2/docs/users/features/worktree.md) |
| 8 | Kiro CLI | Native CLI ACP server: V2 `kiro-cli acp`; V3 `kiro-cli acp --agent-engine=v3` | No Kiro agent entry appears in the pinned ACP Registry snapshot above. As of 2026-10-16, CLI 2.30.0 is gradually rolling V3 out before CLI 3.0; qualify V2 and V3 separately. Both negotiate ACP protocol version 1, so that field cannot identify the server generation. V3 changes auth, permission, session/configuration, and event contracts; it uses CLI login by default or client-owned token requests when explicitly configured. Register update handlers before session load and resend client-owned inputs on load. Keep tokens and OAuth URLs out of logs. [Kiro ACP docs](https://kiro.dev/docs/cli/acp/), [V3 migration](https://kiro.dev/docs/cli/v3/acp-migration/), [authentication](https://kiro.dev/docs/cli/authentication/), [2.30.0 rollout notes](https://kiro.dev/changelog/) |
| 9 | Goose | Native CLI ACP server: `goose acp` over stdio; do not substitute the separate HTTP/WebSocket `goose serve` path | The pinned registry entry provides Goose 1.54.0 platform binaries and SHA-256 values; the matrix reports `loadSession` and `session/list`, but its snapshot does not test standard cancellation. Goose uses the system keyring by default; on macOS, an unattended Keychain prompt can block indefinitely. `GOOSE_DISABLE_KEYRING=1` requires a separately configured provider credential and must not silently replace native auth/configuration. Qualify permission denial, cancel with pending approval, restart/replay, and child-process cleanup. [Pinned registry entry](https://github.com/agentclientprotocol/registry/blob/4d1729af836732b46840080920912dac7d5a2935/goose/agent.json), [Goose 1.54.0 ACP command and keyring caveat](https://github.com/aaif-goose/goose/blob/v1.54.0/documentation/docs/guides/goose-cli-commands.md), [Goose ACP client guide](https://block.github.io/goose/docs/guides/using-goose-in-acp-clients/) |
| 10 | Mistral Vibe | Native dedicated ACP executable `vibe-acp` (not an ACP wrapper around another agent) | The pinned registry entry specifies Vibe 2.26.1 binaries for macOS and Linux x86-64/ARM64 and Windows x86-64; no Windows ARM64 binary is listed. Vibe uses its native auth, engine, and customization: credentials are stored under `~/.vibe`, and user/project `.vibe` configuration, agents, skills, and MCP servers are shared with CLI/VS Code. Preserve that configuration and trusted-folder behavior. The matrix reports `loadSession`, `session/list`, and `session/fork`; separately qualify tool approvals, cancel/stop, and crash/reload recovery. [Pinned registry entry](https://github.com/agentclientprotocol/registry/blob/4d1729af836732b46840080920912dac7d5a2935/mistral-vibe/agent.json), [Vibe ACP setup](https://docs.mistral.ai/vibe/code/use-vibe-in-other-ides), [Vibe authentication and installation](https://docs.mistral.ai/getting-started/quickstarts/vibe-code/install-cli), [2.26.1 release](https://github.com/mistralai/mistral-vibe/releases/tag/v2.26.1) |
| 11 | Cursor CLI | Vendor-documented native ACP (`agent acp`, stdio); native `stream-json` is a separate CLI mode | Cursor documents ACP sessions, `session/cancel`, agent/plan/ask modes, and once/always/reject permission choices. An unanswered permission, `cursor/ask_question`, or `cursor/create_plan` request blocks progress. Native auth is `cursor_login`, `agent login`, API key, or auth-token environment configuration. Registry pin `2026.10.14` launches `cursor-agent acp`, so reconcile that executable name with the vendor-doc command; it lists macOS/Linux/Windows x64 and ARM64. Preserve project/user MCP config; team MCP is unsupported. Native approval is not a Core Trust grant. [Cursor ACP docs](https://cursor.com/docs/cli/acp), [permissions](https://cursor.com/docs/cli/reference/permissions), [pinned Registry entry](https://github.com/agentclientprotocol/registry/blob/4d1729af836732b46840080920912dac7d5a2935/cursor/agent.json) |
| 12 | Kimi Code CLI | Vendor-documented native `kimi acp` over stdio | The Registry's `kimi` 1.52.0 entry points to archived [`MoonshotAI/kimi-cli`](https://github.com/MoonshotAI/kimi-cli) (archived 2026-09-23); its platform list must not be applied to today's separate `MoonshotAI/kimi-code`. Active Kimi Code release 2.2.0 (2026-10-14), pinned at commit `472192c34c1efad89a369870eff0da0b708ce9bb`, documents ACP 0.23, 11/11 agent session methods and 10/11 client reverse-RPC methods. `session/fork` ignores requested cwd, additional directories, and MCP servers. Preserve `kimi login` and `.kimi-code` native state; missing auth returns `authRequired`. Qualify the current release's platform support separately. [Archived Registry entry](https://github.com/agentclientprotocol/registry/blob/4d1729af836732b46840080920912dac7d5a2935/kimi/agent.json), [Kimi Code 2.2.0 release](https://github.com/MoonshotAI/kimi-code/releases/tag/%40moonshot-ai%2Fkimi-code%402.2.0), [pinned ACP reference](https://github.com/MoonshotAI/kimi-code/blob/472192c34c1efad89a369870eff0da0b708ce9bb/docs/en/reference/kimi-acp.md), [pinned IDE guide](https://github.com/MoonshotAI/kimi-code/blob/472192c34c1efad89a369870eff0da0b708ce9bb/docs/en/guides/ides.md) |
| 13 | JetBrains Junie CLI | Vendor-documented native `junie --acp true` | Registry/matrix pin 3659.4.0; the manifest lists macOS/Linux/Windows x64 and ARM64. Native auth may be JetBrains account, Junie API token, BYOK key, or custom endpoint. Headless ACP cannot prompt for project trust: Junie treats the project as trusted and loads project MCP servers, hooks, agents, skills, and guidelines. Sandbox is EAP, off by default, limited to macOS/Linux, and not a hardened VM; Windows has no sandbox. The matrix reports load/list/fork/resume, not standard cancel. [Headless project-trust behavior](https://junie.jetbrains.com/docs/junie-headless.html), [Junie ACP docs](https://junie.jetbrains.com/docs/junie-cli-acp.html), [CLI docs](https://junie.jetbrains.com/docs/junie-cli.html), [parameters and sandbox](https://junie.jetbrains.com/docs/parameters.html), [pinned Registry entry](https://github.com/agentclientprotocol/registry/blob/4d1729af836732b46840080920912dac7d5a2935/junie/agent.json) |
| 14 | Factory Droid | **Launch contract unresolved**; do not yet treat as a verified native ACP mode | Registry entry pins `droid@0.243.0 exec --output-format acp-daemon`, while the matrix probes 0.242.0. Factory IDE docs use `droid exec --output-format acp`; the current CLI reference lists text/json/stream-json/stream-jsonrpc, not either ACP name. Reconcile exact package, command, and session lifecycle before implementation claims. Native auth is browser `/login` or `FACTORY_API_KEY`; preserve native permission/autonomy settings and note `cloudSessionSync=true` by default. [IDE integration docs](https://docs.factory.com/ide-integrations), [current CLI reference](https://docs.factory.com/droid-cli/cli-reference), [settings](https://docs.factory.com/droid-cli/settings), [pinned Registry entry](https://github.com/agentclientprotocol/registry/blob/4d1729af836732b46840080920912dac7d5a2935/factory-droid/agent.json) |
| 15 | Auggie CLI | Vendor-documented native `auggie --acp` | Registry package and matrix pin `@augmentcode/auggie` 0.36.0; Augment says ACP does not expose every interactive CLI feature. The matrix reports load/list only, not resume or standard cancel. Requires Node 20+; documented targets are macOS, Linux, and Windows under WSL, not native Windows. Preserve `auggie login`; auth JSON (`AUGMENT_SESSION_AUTH` / `--augment-session-json`) is secret material tied to an individual. Auggie automatically indexes the current workspace or the selected workspace root for Augment's context engine; disclose this data boundary and qualify it. Enterprise service-account availability elsewhere is not evidence that ACP accepts it. [Auggie ACP docs](https://docs.augmentcode.com/cli/acp/agent), [workspace context and indexing](https://docs.augmentcode.com/cli/setup-auggie/workspace-context), [authentication](https://docs.augmentcode.com/cli/setup-auggie/authentication), [installation/platforms](https://docs.augmentcode.com/cli/setup-auggie/install-auggie-cli), [pinned Registry entry](https://github.com/agentclientprotocol/registry/blob/4d1729af836732b46840080920912dac7d5a2935/auggie/agent.json) |

## Reuse boundary and required evidence

A shared ACP client may reuse JSON-RPC framing, request correlation, event parsing, and
common ACP method handling. It does not replace the per-agent module. The module must
define an allowlisted executable and argument vector; exact version/protocol generation;
credential ownership and auth handoff; configuration and session-option schemas; model
and reasoning behavior; image/file/reference/command support; permission requests;
resume/cancel/close semantics; and supported OS/architecture. Registry metadata is
descriptive and untrusted: it must never supply arbitrary commands, auth callbacks,
credential slots, or privileged setup actions.

Modules must also declare configuration scope. Copilot's ACP launch options apply to
every session on that server; Goose, Qwen Code, and Vibe can reuse native keyring,
environment, user, or project configuration. Preserve each agent's native auth and
configuration ownership, isolate process-wide options by Task where needed, and never
copy credentials or settings between agents or workspaces.

ACP is an agent/editor interaction protocol, not a replacement for LiteCowork's Task,
Trust, Environment, Effect, Artifact, Evidence, lease, and recovery boundaries. Before a
module can be used for a Task, its direct effects must be mediated by LiteCowork or its
provider must enforce an equivalent contract. A process ending is not proof that its
descendants stopped writing. If an agent's tool execution or cancellation path cannot
meet those guarantees, keep it unavailable for that use rather than presenting a chat
handshake as integrated support.

Upstream discovery evidence is time-bounded: the Registry snapshot above was generated
2026-10-15 and checked 2026-10-16; vendor sources and rows 11–15 were checked
2026-10-16. Preserve the immutable Registry commit, exact package/release pin, and
vendor-source commit where available. Do not transfer evidence across product identities
or versions (including archived versus active Kimi, or Factory's 0.242.0/0.243.0
mismatch). Recheck mutable vendor documentation and the installed agent version before
qualification.

For each agent/version/OS claimed as supported, E03 conformance must record actual
installation and update behavior; native sign-in/configuration; model and session
options; text/image/file/resource inputs; commands and references; extensions and native
subagents; permission denial; cancel/resume/stop and process-tree behavior; stale
descriptor handling; crash/restart and replay; secret redaction; and owner-reviewed
native-versus-LiteCowork results. Capabilities without evidence remain explicitly
unsupported, unqualified, or provider-dependent.
