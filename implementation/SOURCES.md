# Research sources and repository reference guide

Snapshot checked 2026-10-06. This is a navigation list, not permission to copy code or a
claim that every rolling page remains accurate. Before implementation, open the current
upstream document/repository at the version in use, record access date and applicable
license, and confirm the described feature is shipped. Vendor examples are vendor-reported;
our parity requires the real-use cases in [WORKFLOWS](WORKFLOWS.md) and [TESTING](TESTING.md).

## Directly checked for this plan

| ID | Source and type | URL | Where/why to consult |
|---|---|---|---|
| R-RUST-RELEASES | Official Rust release notes | [https://doc.rust-lang.org/stable/releases.html](https://doc.rust-lang.org/stable/releases.html) | Check stable toolchain and component availability before updating `rust-toolchain.toml`. |
| R-RUSTUP | Official rustup installation guide | [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install) | Install rustup before using the repository-pinned compiler/toolchain components. |
| R-PYTHON-RELEASES | Official Python release pages | [https://www.python.org/downloads/](https://www.python.org/downloads/) | Check supported/security-maintenance release before updating `.python-version`. |
| R-UV-INSTALL | Official uv installer docs | [https://docs.astral.sh/uv/getting-started/installation/](https://docs.astral.sh/uv/getting-started/installation/) | Install the exact repository-pinned uv version on developer machines. |
| R-UV-GITHUB | Official uv GitHub Actions guide | [https://docs.astral.sh/uv/guides/integration/github/](https://docs.astral.sh/uv/guides/integration/github/) | Locked Python tool environment, pinned uv action, cache and CI setup. |
| R-UV-LOCK | Official uv project/lock guidance | [https://docs.astral.sh/uv/concepts/projects/](https://docs.astral.sh/uv/concepts/projects/) | Reproducible validator dependency lock and `--locked` behavior. |
| R-TAURI | Framework docs — Tauri 2 official docs | [https://v2.tauri.app/start/](https://v2.tauri.app/start/) | Desktop framework fit, packaging, webview variance and IPC tests. Do not infer app performance from sample size. |
| R-SQLITE | Official database docs — SQLite FTS5 | [https://www.sqlite.org/fts5.html](https://www.sqlite.org/fts5.html) | Deterministic full-text candidate; compare indexes/corpus before vector addition. |
| R-DOCLING | Official parser docs — Docling | [https://docling-project.github.io/docling/](https://docling-project.github.io/docling/) | Evaluate extraction/table/layout/OCR limits, licensing, resource isolation. |
| R-OLLAMA | Official local inference docs — Ollama API | [https://docs.ollama.com/api/introduction](https://docs.ollama.com/api/introduction) | Candidate local model backend only; qualify full harness separately. |
| R-LMSTUDIO | Official local inference docs — LM Studio developer docs | [https://lmstudio.ai/docs/developer](https://lmstudio.ai/docs/developer) | Candidate local model backend only; verify hardware and tool use. |
| R-ANYTHINGLLM | Product implementation reference — AnythingLLM document/RAG docs | [https://docs.anythingllm.com/chatting-with-documents/introduction](https://docs.anythingllm.com/chatting-with-documents/introduction) | Study workspace-scoped document ingestion and retrieval UX; not Core authority. |
| R-JAN | Jan desktop/agent docs — official local AI app | [https://www.jan.ai/docs](https://www.jan.ai/docs) | Examine Projects/files, Cowork preview, local endpoint, separate agent interfaces; do not assume its models equal coding-agent quality. |
| R-GPT4ALL | GPT4All LocalDocs | [https://docs.gpt4all.io/gpt4all_desktop/localdocs.html](https://docs.gpt4all.io/gpt4all_desktop/localdocs.html) | Compare local folder indexing, on-device embeddings, readiness and local inference experience. |
| R-OPENWEBUI | Product implementation reference — Open WebUI RAG docs | [https://docs.openwebui.com/features/chat-conversations/rag/](https://docs.openwebui.com/features/chat-conversations/rag/) | Study folder/file RAG, citations, hybrid search and the difference between pre-injected context and retrieval tools. |
| R-LLAMACPP | GitHub implementation — llama.cpp repository | [https://github.com/ggml-org/llama.cpp](https://github.com/ggml-org/llama.cpp) | Local inference runtime reference, hardware/backend and server API; no direct Core dependency. |
| R-CODEX | GitHub native harness — OpenAI Codex App Server | [https://github.com/openai/codex/tree/main/codex-rs/app-server](https://github.com/openai/codex/tree/main/codex-rs/app-server) | Read app-server protocol/current README and sample clients before Codex adapter changes. |
| R-CODEX-DOCS | Official engineering article — OpenAI Codex harness article | [https://openai.com/index/unlocking-the-codex-harness/](https://openai.com/index/unlocking-the-codex-harness/) | Rationale for full harness integrations; use current App Server contract for implementation. |
| R-CLAUDE | Official adapter docs — Claude Code subagent docs | [https://code.claude.com/docs/en/sub-agents](https://code.claude.com/docs/en/sub-agents) | Native worker lifecycle/config, per-profile model options and current constraints; recheck current release. |
| R-CLAUDE-USE | Official product docs — Claude Cowork help/use | [https://support.claude.com/en/articles/13345190-get-started-with-claude-cowork](https://support.claude.com/en/articles/13345190-get-started-with-claude-cowork) | Folder workflow, deliverables, actual current feature/account rollout limits. |
| R-CLAUDE-MERGE | Official product docs — Claude Cowork/chat rollout | [https://support.claude.com/en/articles/16761823-claude-cowork-and-chat-are-one-claude](https://support.claude.com/en/articles/16761823-claude-cowork-and-chat-are-one-claude) | Time-sensitive rollout; verify account-specific local/cloud behavior before claims. |
| R-CLAUDE-STUDY | Vendor-reported product analytics — Claude Cowork usage report | [https://claude.com/blog/how-people-are-using-claude-cowork](https://claude.com/blog/how-people-are-using-claude-cowork) | Use as vendor-selected session evidence, not independently sampled population research. |
| R-MUSE | Vendor product-design account — How We Designed Muse | [https://introducing.muse.ai/](https://introducing.muse.ai/) | Study proactive background work, goals, Ideas, identity, rich outputs and approval UI. Examples are vendor-reported. |
| R-MUSE-SAFETY | Meta AI research/vendor technical account — Muse agent safety architecture | [https://research.meta.ai/blog/security-and-safety-for-ai-agents-our-approach-with-muse](https://research.meta.ai/blog/security-and-safety-for-ai-agents-our-approach-with-muse) | Study credential boundary and constrained browser representations; do not assume equivalent implementation. |
| R-CHATGPT-WORK | Official product docs — ChatGPT Work | [https://learn.chatgpt.com/docs/web](https://learn.chatgpt.com/docs/web) | Research/files/apps/output workflows; rollout and surface may change. |
| R-CHATGPT-FILES | Official product docs — ChatGPT file artifacts | [https://learn.chatgpt.com/docs/artifacts-viewer?surface=app](https://learn.chatgpt.com/docs/artifacts-viewer?surface=app) | Study document/spreadsheet/slide previews and iterative artifact interaction. |
| R-OPENCLAW | GitHub implementation — OpenClaw project | [https://github.com/openclaw/openclaw](https://github.com/openclaw/openclaw) | Study daemon/gateway ownership, durable schedules, message ingress, plugins, secrets, deployment. License and current code must be checked before reuse. |
| R-OPENCLAW-DOCKER | Official deployment docs — OpenClaw Docker install | [https://docs.openclaw.ai/install/docker](https://docs.openclaw.ai/install/docker) | Reference container/volume/service update flows for later LiteCowork cloud deployment. |
| R-OPENCLAW-GATEWAY | Official operations docs — OpenClaw Gateway runbook | [https://docs.openclaw.ai/gateway](https://docs.openclaw.ai/gateway) | Study gateway health/config/recovery boundaries; not a LiteCowork protocol contract. |
| R-OPENCLAW-CHANNELS | Official channel integration docs — OpenClaw channel docs | [https://docs.openclaw.ai/channels](https://docs.openclaw.ai/channels) | Future gateway/channel patterns incl access control, receipts, delivery, approvals; not a V1 commitment to every channel. |
| R-OPENHANDS | GitHub implementation — OpenHands project | [https://github.com/OpenHands/OpenHands](https://github.com/OpenHands/OpenHands) | Study agent/runtime separation, workspaces, integrations and deployment; verify exact implementation version. |
| R-GOOSE | GitHub implementation — Goose project | [https://github.com/block/goose](https://github.com/block/goose) | Study native extensions, recipes and harness composition. |
| R-OPENCODE | GitHub implementation — OpenCode project | [https://github.com/anomalyco/opencode](https://github.com/anomalyco/opencode) | Study server/session/agent configuration before adapter; official docs also at https://opencode.ai/docs/server/. |
| R-CLINE | Official adapter docs — Cline docs | [https://docs.cline.bot/cline-overview](https://docs.cline.bot/cline-overview) | Study supported host/extension interface and worker capabilities; qualify independently. |
| R-ACPSPEC | Protocol specification — Agent Client Protocol | [https://agentclientprotocol.com/protocol/overview](https://agentclientprotocol.com/protocol/overview) | Use only where actual adapter features survive; not default replacement for native App Servers. |
| R-MCP | Protocol specification — Model Context Protocol | [https://modelcontextprotocol.io/specification/2025-11-25](https://modelcontextprotocol.io/specification/2025-11-25) | Check current stable protocol plus repository-pinned schema; version skew and unsafe direct paths matter. |
| R-OPENHANDS-ACP | GitHub implementation docs — OpenHands ACP guide | [https://github.com/OpenHands/docs/blob/main/openhands/usage/agent-canvas/acp-agents.mdx](https://github.com/OpenHands/docs/blob/main/openhands/usage/agent-canvas/acp-agents.mdx) | Study a backend hosting an external agent; do not collapse LiteCowork attempts into ACP sessions. |
| R-OPENWORKER | GitHub implementation — OpenWorker | [https://github.com/andrewyng/openworker](https://github.com/andrewyng/openworker) | Study local coworker and approval-gated automation examples, verify service lifecycle. |
| R-OPENCOWORK | GitHub project/issue — OpenCowork | [https://github.com/OpenCoworkAI/open-cowork](https://github.com/OpenCoworkAI/open-cowork) | Inspect only issue/project status; proposals are not shipped capability. |
| R-AGENTZERO | GitHub implementation — Agent Zero | [https://github.com/agent0ai/agent-zero](https://github.com/agent0ai/agent-zero) | Study projects/files/scheduled tasks; check security and version before adaptation. |
| R-MANUS | Official help page — Manus Cloud Computer | [https://help.manus.im/en/articles/15392111-what-is-the-cloud-computer](https://help.manus.im/en/articles/15392111-what-is-the-cloud-computer) | Compare persistent cloud computer versus temporary environment and cost. |
| R-CURSOR | Official docs — Cursor Cloud Agent | [https://cursor.com/docs/cloud-agent/automations](https://cursor.com/docs/cloud-agent/automations) | Study scheduled/event automation and self-hosted computer use limits. |
| R-ZAPIER | Official docs — Zapier schedule triggers | [https://help.zapier.com/hc/en-us/articles/8496276333453-Schedule-Zaps-to-run-at-specific-intervals](https://help.zapier.com/hc/en-us/articles/8496276333453-Schedule-Zaps-to-run-at-specific-intervals) | Study trigger identity/approval flow; do not infer exactly-once external Effects. |
| R-N8N | Official docs — n8n Schedule Trigger | [https://docs.n8n.io/integrations/builtin/core-nodes/n8n-nodes-base.scheduletrigger/](https://docs.n8n.io/integrations/builtin/core-nodes/n8n-nodes-base.scheduletrigger/) | Study explicit timezones/published workflow schedule semantics. |
| R-COPILOT-HOOKS | Official docs — GitHub Copilot hooks | [https://docs.github.com/en/copilot/concepts/agents/hooks](https://docs.github.com/en/copilot/concepts/agents/hooks) | Compare native hook ownership and boundaries. |
| R-DOTS | Official help page — OpenAI Dot getting started | [https://help.openai.com/en/articles/20001530-getting-started-with-your-dot](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot) | Persistent identity/context product reference; verify availability/account status before implementation. |


## Existing architecture references retained

The links below already appear in `docs/COMPETITIVE-RESEARCH.md`; they are preserved as
inherited pointers, not newly verified unless also listed above. Check availability and
context before using. GitHub references are research/navigation only: never copy source
without license/security review and a specific need. Reference no archived local bundles.

- [https://cursor.com/docs/cloud-agent/capabilities](https://cursor.com/docs/cloud-agent/capabilities)
- [https://docs.devin.ai/automation-templates/weekly-dependency-updates](https://docs.devin.ai/automation-templates/weekly-dependency-updates)
- [https://docs.openclaw.ai/automation/cron-jobs/how-it-works](https://docs.openclaw.ai/automation/cron-jobs/how-it-works)
- [https://docs.openclaw.ai/automation/cron-jobs/schedules](https://docs.openclaw.ai/automation/cron-jobs/schedules)
- [https://docs.openhands.dev/api-reference/get-conversation](https://docs.openhands.dev/api-reference/get-conversation)
- [https://docs.openhands.dev/openhands/usage/run-openhands/github-action](https://docs.openhands.dev/openhands/usage/run-openhands/github-action)
- [https://github.com/OpenCoworkAI/open-cowork/issues/304](https://github.com/OpenCoworkAI/open-cowork/issues/304)
- [https://github.com/OpenHands/docs/blob/main/openhands/usage/agent-canvas/overview.mdx](https://github.com/OpenHands/docs/blob/main/openhands/usage/agent-canvas/overview.mdx)
- [https://github.com/aaif-goose/goose/blob/main/documentation/docs/getting-started/using-extensions.md](https://github.com/aaif-goose/goose/blob/main/documentation/docs/getting-started/using-extensions.md)
- [https://github.com/aaif-goose/goose/blob/main/documentation/docs/goose-architecture/goose-architecture.md](https://github.com/aaif-goose/goose/blob/main/documentation/docs/goose-architecture/goose-architecture.md)
- [https://github.com/aaif-goose/goose/blob/main/documentation/docs/guides/recipes/recipe-reference.md](https://github.com/aaif-goose/goose/blob/main/documentation/docs/guides/recipes/recipe-reference.md)
- [https://github.com/agent0ai/agent-zero/blob/main/docs/guides/usage.md](https://github.com/agent0ai/agent-zero/blob/main/docs/guides/usage.md)
- [https://github.com/andrewyng/openworker/blob/main/coworker/automation/tools.py](https://github.com/andrewyng/openworker/blob/main/coworker/automation/tools.py)
- [https://help.openai.com/en/articles/10291617-scheduled-tasks-in-chatgpt](https://help.openai.com/en/articles/10291617-scheduled-tasks-in-chatgpt)
- [https://help.openai.com/en/articles/20001275-chatgpt-work-and-codex](https://help.openai.com/en/articles/20001275-chatgpt-work-and-codex)
- [https://help.openai.com/en/articles/20001540-creating-and-managing-team-tasks-in-chatgpt](https://help.openai.com/en/articles/20001540-creating-and-managing-team-tasks-in-chatgpt)
- [https://help.openai.com/en/articles/20001548-agent-security-and-local-work-sync-in-chatgpt](https://help.openai.com/en/articles/20001548-agent-security-and-local-work-sync-in-chatgpt)
- [https://help.zapier.com/hc/en-us/articles/38731463206029-Request-approval-to-keep-your-workflow-running-with-Human-in-the-Loop](https://help.zapier.com/hc/en-us/articles/38731463206029-Request-approval-to-keep-your-workflow-running-with-Human-in-the-Loop)
- [https://help.zapier.com/hc/en-us/articles/8496260269965-How-Zapier-handles-duplicate-data-in-Zap-workflows](https://help.zapier.com/hc/en-us/articles/8496260269965-How-Zapier-handles-duplicate-data-in-Zap-workflows)
- [https://help.zapier.com/hc/en-us/articles/8496288648461-Schedule-Zaps-to-run-at-specific-intervals](https://help.zapier.com/hc/en-us/articles/8496288648461-Schedule-Zaps-to-run-at-specific-intervals)
- [https://prod.cursor.com/docs/cloud-agent/self-hosted/computer-use](https://prod.cursor.com/docs/cloud-agent/self-hosted/computer-use)
- [https://support.claude.com/en/articles/13854387-schedule-recurring-tasks-in-claude-cowork](https://support.claude.com/en/articles/13854387-schedule-recurring-tasks-in-claude-cowork)
- [https://support.claude.com/en/articles/14479288-claude-cowork-architecture-overview](https://support.claude.com/en/articles/14479288-claude-cowork-architecture-overview)
- [https://support.claude.com/en/articles/15520349-use-claude-cowork-on-web-desktop-and-mobile](https://support.claude.com/en/articles/15520349-use-claude-cowork-on-web-desktop-and-mobile)


## Repository review checklist

For a requested implementation reference, record: repo/commit; applicable license;
architecture path; tests/security review performed; semantics adopted; difference from
LiteCowork contracts; explicit decision not to reuse where appropriate. Never let a
reference override Task/effect/trust/fencing contracts. OpenClaw receives special future
attention for cloud deployment, gateway operations and channel integration, while its
agent, task store and authority model remain independent.
