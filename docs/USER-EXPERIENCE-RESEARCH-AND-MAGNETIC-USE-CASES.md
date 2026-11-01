# User Experience Research & Magnetic Use Cases

**Status:** Canonical user-journey & sentiment research  
**Date:** 2026-11-01  
**Sources:** Live community sentiment & workflow reports from Reddit (`r/ClaudeAI`, `r/singularity`, `r/AI_Agents`, `r/Cursor`, `r/OpenClaw`), X/Twitter tech community, YouTube developer breakdowns, and competitor audits (Claude Cowork, Claude Computer Use, Claude Code, Grok, Manus 2.0, OpenWork, Cursor, Windsurf).

---

## 1. Executive Summary & Market Sentiment

Modern AI agent users have moved past autocomplete and simple Q&A. They demand **agentic, multi-step execution**. However, existing market solutions suffer from painful structural flaws:

| Platform | What Users Love (Viral Workflows) | What Users Hate (Pain Points & Frustrations) | LiteCowork Opportunity & Solution |
|---|---|---|---|
| **Claude Cowork** | Agentic chores on local folders, spreadsheets, cross-device monitoring. | "Past chats" blindspot (zero persistent memory across chats); environment file corruption and lag. | **Coworkers with Scoped Memory** ([ADR-0024](adr/0024-optional-chat-first-coworkers.md)) + local SQLite immutable history. |
| **Claude Computer Use** | True OS mouse/browser control; automated data entry and lead scraping. | **Massive security anxiety** (prompt injection on bare metal, fear of deleting files/passwords). | **Isolated Sandbox + Workbench Human Takeover**: human can take over browser/mouse instantly; sensitive commands strictly mediated. |
| **Claude Code** | Generational leap in architectural reasoning, CLI speed, custom skills/hooks. | Clunky terminal-only UX; reviewing diffs in terminal is painful; annoying repetitive permission prompts. | **Desktop Shell with Contextual Workbench**: Claude Code reasoning inside a clean GUI with visual diffs & terminal PTY. |
| **Cursor / Windsurf** | Visual diffs (`Cmd+K`), multi-file composer, familiar IDE. | Brutal token burn / subscription costs; hallucination loops on large codebases. | **BYOA Model Control**: Bring your own keys/subscriptions (Claude, DeepSeek, Codex, Ollama); zero markup, zero token extortion. |
| **Grok (xAI)** | 24/7 background social/market pulse, real-time X news monitoring. | Locked to xAI models; lack of local desktop and file execution capabilities. | **Automations & Routines**: 24/7 background cron/event monitoring running locally via `litecoworkd`. |
| **Manus AI** | Boardroom-ready slide decks, autonomous ad scraping, hub-and-spoke SEO generation. | Expensive closed cloud; user cannot inspect raw code or edit individual pieces without regeneration. | **Interactive Artifact Runtime**: outputs live editable React/HTML5 apps, spreadsheets, and documents directly to disk. |
| **OpenAI Dots / OpenDot** | Always-on background digital coworker, morning briefings, multi-task orchestration. | **Abuse caps & hard lockouts** ("Dot is on a break"); dynamic layout stall-outs; high cost. | **Self-hosted local container/CLI execution**: zero rate-limit lockouts, local Ollama/OpenRouter options, human-in-the-loop gates. |
| **Grok Bot (xAI)** | 24/7 background social/market pulse, virtual company multi-agent teams. | **Terrifying runaway automation loops**; **silent missed overnight routines**; astronomical token burn from inter-agent chatter; ghost background pollers. | **Fenced Execution Leases + Monotonic Occurrence Slots** ([ADR-0010](adr/0010-revision-independent-occurrences.md)): no runaway loops, no ghost polling, no silent missed routines. |
| **Meta Muse** | Deep cross-app integration (clearing spam, cross-referencing receipts, daily plans). | **Privacy nightmare** (unprompted scanning of private texts/DMs); **"Verification Trap"** (hallucinated details force users to double-check everything). | **100% Local-First Privacy**: zero data harvesting. **Verified Truth Receipts** ([ADR-0007](adr/0007-reported-observed-verified.md)) prevent hallucinated completions. |
| **OpenWork / OpenClaw** | WhatsApp/Slack-to-desktop control; true local-first privacy. | Complex command-line setup; fragile ad-hoc integrations without formal transaction/state-machine guarantees. | **Polished Tauri Desktop + State-Machine Guarantees**: rock-solid SQLite transactions and one-click channel setup. |

---

## 2. The "Daily Stuff" Reality: Unglamorous Chores That MUST Work Flawlessly

Research on Reddit proves that users don't just want AI to write entire software apps or build trading bots. **They want their daily, unglamorous chores to work reliably without supervision.** When these daily basics break, users lose trust immediately.

Here is the benchmark matrix for everyday operations:

### 1. The "Clean Morning Briefing" (Daily Routine)
* **What real users expect:** At 8:00 AM, wake up to a consolidated 1-page digest:
  - Calendar agenda with conflicting invites flagged.
  - Priority unread GitHub PRs or critical client emails.
  - Weather, local transit, or industry breaking news.
* **Why Dots/Grok fail on Reddit:** Grok silently skips overnight runs; Dots hit rate limits at 7:55 AM; Muse pulls private family DMs into the business summary.
* **LiteCowork Guarantee:** `litecoworkd` runs as an OS background daemon. Monotonic occurrence tracking ensures **zero skipped schedules**. If the laptop was in sleep mode, `CATCH_UP` policy runs the moment the lid opens.

### 2. The "Messy Downloads Folder & Receipt Sorter"
* **What real users expect:** *"Sort my Downloads folder from the past 7 days into Documents, Invoices, Images, and Code Archives. Extract date and vendor for invoices into a CSV."*
* **Why others fail:** Agents delete files accidentally, overwrite identically named files, or hallucinate CSV data without opening PDFs.
* **LiteCowork Guarantee:** File movements are governed by `Effect` transactions. LiteCowork generates a preview diff in the *Files Workbench* before moving files, with zero silent deletions allowed (`NO_DESTRUCTIVE_UNVERIFIED_EFFECTS`).

### 3. The "Email & Message Triage" (Without Inadvertent Reply)
* **What real users expect:** *"Draft replies to my 5 urgent client emails, but DO NOT send them until I click Send."*
* **Why Muse/Grok fail on Reddit:** Muse takes liberties and sends messages unprompted or shares private contact info; Grok loops on draft generation.
* **LiteCowork Guarantee:** **Approval Fencing**. The outbound send capability is locked behind an explicit `APPROVAL_REQUIRED` ticket. Drafts are placed in the **Needs You** queue for 1-click human review.

### 4. The "Quick Scratchpad & Calculation"
* **What real users expect:** Quick math, currency conversions, timezone translations, and markdown table transformations directly in chat without spinning up heavy agents.
* **LiteCowork Guarantee:** Ordinary chat mode has zero overhead. No Task or Attempt is spun up. Fast local responses in milliseconds.

---

---

## 3. Additional Platform Research: Perplexity, Notion AI, Replit Agent, Devin

| Platform | Daily Workflow | Top Pain Point (Reddit) | LiteCowork Advantage |
|---|---|---|---|
| **Perplexity** | Fast web-backed research, daily fact-checking, topic monitoring. | Feature stagnation; Pro limits on Deep Research; competitors catching up fast. | Deep Research via **WorkspaceRoot + ResourceTextIndex** is 100% local — no limits. |
| **Notion AI** | Meeting notes, database automation, workspace agents. | **Credit-burn pricing shock**: complex workflows burn through credits at alarming rates. Severe lag on large databases. | BYOA model means zero per-request credit tax; local SQLite queries are sub-millisecond. |
| **Replit Agent** | Text-to-app code generation, cloud deployment. | **"Pay-to-fix-mistakes" cost trap**: burns hundreds of dollars having it fix its own bugs. | **Attempt isolation + Verified Truth Receipt**: agent cannot mark itself complete until exit codes and file hashes validate. Cost stays in user's own API key. |
| **Devin** | Autonomous ticket-to-code engineering. | **"Unruly toddler" management overhead**: requires heavy babysitting; produces off-spec tech debt. | **Plan-first + atomic Step acceptance**: users review and approve the full plan before any execution begins. |

---

## 4. The 4 Non-Negotiable Cross-Platform User Demands (Reddit Consensus 2025–2026)

Research across `r/LocalLLM`, `r/AI_Agents`, `r/ClaudeCode`, and `r/Artificial` reveals **four universal demands** that every user — regardless of skill level — treats as hard release gates:

### Demand 1: Undo / Rollback (The Safety Net)
* **The Problem on Reddit:** Agents delete "unnecessary" files, overwrite production configs, or alter databases silently. Git doesn't always catch uncommitted local file operations. *"Starting fresh means losing hours of work."*
* **LiteCowork Response:** Every consequential file or system operation is an `Effect` record. The Workbench *Files* pane shows a diff **before** any file is moved, renamed, or deleted. Filesystem effects carry content-addressed pre-operation snapshots enabling 1-click rollback. Uncommitted destructive mutations are physically blocked.

### Demand 2: "Show Your Work" (Transparent Reasoning Chain)
* **The Problem on Reddit:** Users in legal, financial, and engineering workflows describe AI outputs as *"structural gaslighting"* — polished and confident but wrong. They want an append-only action log with exact tool-call inputs and outputs, not a final polished summary.
* **LiteCowork Response:** The **Workbench Activity** pane shows every tool call, agent decision, Effect record, and approval gate in real time. An immutable Evidence ledger attached to each Attempt records every input, output, exit code, and file digest. This is mandatory, not optional.

### Demand 3: True Offline-First (Zero Cloud Tax)
* **The Problem on Reddit:** *"I'm tired of paying a silent monthly tax per request, uploading my confidential legal code to corporate servers, and having my workflow die the moment my VPN drops."* The local-LLM / Ollama community on Reddit has exploded because of this.
* **LiteCowork Response:** This is the core invariant. `litecoworkd` runs fully offline. WorkspaceRoot indexing is local. Secrets never leave the machine. Provider costs are user-owned API keys. Using Ollama/local models: zero cloud dependency at all.

### Demand 4: Be a Tool, Not a Boss (Anti-Paternalism)
* **The Problem on Reddit:** `r/ClaudeCode` has entire threads venting about AI assistants refusing to run edge-case code, inserting unsolicited security lectures, or adding repetitive confirmation dialogs to read-only operations. *"Be a tool, not an HR manager."*
* **LiteCowork Response:** Mediation is scoped to genuinely consequential **mediated effects** only (ADR-0019). Read-only operations and local queries execute at full speed with zero approval overhead. Destructive external mutations require gates. No opinionated behavioral guardrails on ordinary work.

---

## 5. The First-Run Onboarding Standard (First 60 Seconds Critical)

Reddit research on agent onboarding identifies the **#1 drop-off cause**: the blank prompt box + lengthy setup wizard. The community consensus is: *"Show, don't ask."*

| Anti-Pattern (What Others Do) | LiteCowork First-Run Standard |
|---|---|
| Long setup questionnaire before first value. | Detect installed native agents (Claude Code, Codex, Ollama) automatically. Launch a **pre-seeded demo conversation** in 1 click. |
| User must re-explain their setup every session. | **Workspace + Coworker Scoped Memory** persists identity, project context, and instructions permanently. Zero Groundhog Day loops. |
| Silent failure on first automation run. | **Loud, typed, actionable failure states** everywhere: never show a spinner without a timeout, a broken state without a repair path, or a silent skip without an audit log entry. |
| Blank "What would you like to do?" box. | **Smart Conversation Starter Suggestions** derived from detected WorkspaceRoots (e.g., *"I see a Rust workspace here — want me to run clippy and summarize issues?"*). |

---

## 6. Persona-Specific Viral Workflows (Freelancers, Lawyers, Students, Researchers)

Beyond power-users and developers, research on Reddit and YouTube shows massive demand from non-technical professional personas:

### The Freelancer
* **Trigger:** *"Find new gigs matching my skills, draft customized proposals, parse my invoice PDFs, and remind me to follow up after 3 days."*
* **The Flow:** Lead scraper Routine (Automation) → draft proposal Coworker → invoice parser in Files Workbench → follow-up Routine triggered by Calendar channel event.
* **Why LiteCowork:** Everything stays local. No SaaS workflow builder subscription. Client invoice PDFs never touch a vendor server.

### The Lawyer / Legal Professional
* **Trigger:** *"Read this 200-page contract, flag any indemnification, governing law, and non-compete clauses, and produce a risk matrix."*
* **The Flow:** WorkspaceRoot ingestion → local ResourceTextIndex semantic search → structure-aware extraction prompt → formatted risk matrix artifact in Library.
* **Why LiteCowork:** Confidential documents stay on local disk. Explicit citations back to page/line in the source document (grounded retrieval).

### The Student / Researcher
* **Trigger:** *"Here are 15 PDFs from my research. Build me a structured literature review matrix, flag contradictions between papers, and generate flashcards."*
* **The Flow:** Batch WorkspaceRoot ingest → vector + lexical ResourceTextIndex → cluster-by-methodology prompt → exportable Markdown/XLSX artifact.
* **Why LiteCowork:** University confidential research never hits OpenAI/Anthropic servers. Grounded source citations prevent hallucinated references.

### The Content Creator / Marketer
* **Trigger:** *"Monitor my niche on Reddit and YouTube for the past week, find the top 10 viral posts, identify common pain points, and draft 5 scroll-stopping LinkedIn posts in my tone."*
* **The Flow:** Scheduled Automation (weekly) → Workbench Browser scraping routine → Coworker with stored tone-of-voice rules → reviewed drafts in Library.
* **Why LiteCowork:** Personal voice and strategy stored privately in a Coworker; drafts held for human approval before any publish.

---

## 7. The 6 Magnetic "Killer Workflows" for LiteCowork

These are the exact high-delight user workflows engineered to make developers, founders, and knowledge workers fall in love with LiteCowork:

### Workflow 1: The "Architect & Speed Coder" (Heterogeneous BYOA)
* **User Trigger:** *"Refactor our legacy payment webhook handler to use Stripe idempotent events, update Prisma models, and write full test coverage."*
* **The Magic:**
  * **Planning:** The user chooses **Claude 3.7 Sonnet** as the *Chief Architect* to read the repo, analyze dependencies, and write an immutable, bulletproof Step Plan.
  * **Execution:** Once approved, repetitive boilerplate and unit test generation are dispatched to **DeepSeek V3 / OpenAI Codex** via LiteCowork worker profiles.
  * **Review:** The Chief Agent reviews the generated diffs, runs tests in the *Workbench Terminal*, and presents a unified verified diff.
* **Why Users Love It:** Slashes API costs by 70%, achieves superior code quality, and uses the exact models the user already pays for without vendor lock-in.

---

### Workflow 2: Deep Market Recon with One-Click Human Takeover
* **User Trigger:** *"Scrape the top 10 competitors in our space from G2 and LinkedIn, compile their features and pricing into an interactive matrix, and highlight our pricing advantage."*
* **The Magic:**
  * The agent opens the **Workbench Browser** and starts extracting data.
  * A competitor site displays a Cloudflare captcha or requires a 2FA login.
  * Instead of failing or crashing (as other agents do), the agent pauses and notifies the user with a gentle bell.
  * The user clicks **`Take Over`** in the Workbench, solves the captcha in 5 seconds, and clicks **`Return to Agent`**.
  * The agent smoothly resumes data extraction without losing context or session cookies.
  * The final output is rendered in the Workbench as an **Interactive Artifact**: a live, sortable, filtering pricing dashboard.
* **Why Users Love It:** Never gets stuck. Blends autonomous scraping with painless human-in-the-loop takeover.

---

### Workflow 3: The "Chat-to-Routine" 24/7 Assistant (Zero Cloud Cost)
* **User Trigger:** In ordinary chat, a user asks: *"Check my git repositories for open dependency vulnerabilities and unmerged stale PRs, and give me a summary."*
* **The Magic:**
  * The agent delivers an outstanding report.
  * Right below the message, an action pill illuminates: **`Save as Routine`**.
  * The user clicks it: an editable modal previews:
    * Trigger: *Every weekday at 8:30 AM*
    * Execution: *Local `litecoworkd` daemon*
    * Notification: *Desktop push notification + 'Needs You' queue*
  * The user clicks **`Enable Automation`**.
* **Why Users Love It:** No Zapier, no complex cron files, no monthly cloud SaaS bill. Any great prompt becomes a permanent personal assistant in 2 clicks.

---

### Workflow 4: The "Safe Terminal & Sandbox" Full-Stack Sprint
* **User Trigger:** *"Clone this GitHub repo, set up the Docker containers, run database migrations, fix any failing tests, and launch the dev server."*
* **The Magic:**
  * The agent runs inside an isolated **Environment** with strict read/write boundaries.
  * Terminal PTY commands stream live into the *Workbench Terminal* pane with colorized output.
  * Dangerous commands (e.g. `rm -rf`, modifying system PATH, credential file reads) trigger an explicit **Needs You** permission dialog showing exact command arguments.
  * Safe commands (installing packages, running test suites) execute with high speed.
* **Why Users Love It:** Total confidence. Users get the speed of autonomous terminal agents (like Claude Code or Aider) without the terrifying risk of bricking their machine or leaking secrets.

---

### Workflow 5: The "Persistent Specialist Coworker" (True Project Memory)
* **User Trigger:** The user creates a named specialist: *"SaaS Growth Marketer"*.
* **The Magic:**
  * Unlike standard ChatGPT or Claude where memory is fuzzy or hidden in vague system prompts, the Coworker has **Scoped Memory**:
    * Target audience personas
    * Tone of voice rules
    * Specific attached knowledge files (pitch decks, past campaigns)
  * The user can open 10 different chats over 3 months with this Coworker; it maintains perfect continuity.
  * The user can inspect, edit, or revoke any single memory fact with one click in the Coworker Library.
* **Why Users Love It:** Solves the #1 complaint on Reddit regarding Claude Cowork ("it forgets past sessions and lacks long-term context").

---

### Workflow 6: The "Boardroom Deliverable" Studio
* **User Trigger:** *"Analyze our Q3 sales CSV and write an executive briefing with charts and a financial forecast model."*
* **The Magic:**
  * LiteCowork does not just output markdown text or raw code.
  * It creates two durable deliverables in the **Library**:
    1. A formatted, print-ready PDF/Markdown report with executive takeaways.
    2. A live, single-file Interactive Web App (React + Chart.js) rendered directly in the Workbench, allowing stakeholders to adjust growth rate sliders and watch projections recalculate in real time.
* **Why Users Love It:** Produces client-ready, interactive deliverables that look like they took a team of analysts days to build.

---

## 8. Full Implementation Qualification Checklist (Incoming Codebase)

When the tar file is unpacked, we will verify all of these user-facing flows end-to-end:

### Killer Workflows
- [ ] **UC-BYOA-01:** Detect installed Claude CLI / Codex / Ollama and switch models mid-draft with zero state loss.
- [ ] **UC-WORKBENCH-01:** Split-pane Workbench responsiveness (Files diff, Terminal PTY, Browser, and Artifact preview).
- [ ] **UC-TAKEOVER-01:** Live browser navigation with seamless human takeover and return.
- [ ] **UC-ROUTINE-01:** One-click `Save as Routine` from conversation message to durable `AutomationOccurrence`.
- [ ] **UC-MEMORY-01:** Coworker scoped memory inspection, fact deletion, and cross-chat continuity.
- [ ] **UC-ARTIFACT-01:** Interactive artifact mounting inside the isolated iframe sandbox with zero permission leakage.
- [ ] **UC-TRUTH-01:** Verification gate: terminal exit codes and file digests match the reported completion state.

### Daily Stuff (Must Work Reliably)
- [ ] **UC-DAILY-01:** Scheduled morning briefing fires at correct time; missed schedule during sleep triggers catch-up on wake.
- [ ] **UC-DAILY-02:** File sort + PDF invoice extraction previews diff before moving, produces correct CSV, zero silent deletion.
- [ ] **UC-DAILY-03:** Email draft held in Needs You queue; no send without explicit user confirmation click.
- [ ] **UC-DAILY-04:** Quick scratchpad (math, regex, JSON) responds in under 2 seconds with zero Task overhead.
- [ ] **UC-DAILY-05:** Stop button terminates running agent immediately via lease epoch invalidation; no ghost background polling.

### Universal User Demands
- [ ] **UC-ROLLBACK-01:** File move/rename/delete previews diff before execution; 1-click rollback restores pre-op state.
- [ ] **UC-TRANSPARENCY-01:** Activity pane shows every tool call with exact inputs, outputs, and exit codes in real time.
- [ ] **UC-OFFLINE-01:** WorkspaceRoot, Coworker memory, and ordinary chat work with network fully disabled.
- [ ] **UC-TOOL-01:** Read-only operations execute with zero confirmation dialogs; only mediated destructive effects trigger approval gates.

### Onboarding
- [ ] **UC-ONBOARD-01:** First run detects installed native agents within 30 seconds; pre-seeded demo loads immediately — no blank box.
- [ ] **UC-ONBOARD-02:** Workspace memory and Coworker context survive daemon restart intact.
- [ ] **UC-ONBOARD-03:** First automation failure produces loud, typed, actionable error — never a silent skip.

### Professional Personas
- [ ] **UC-LEGAL-01:** 200-page PDF contract extracted locally with grounded page/line citation references; no external server upload.
- [ ] **UC-RESEARCH-01:** 15-PDF literature review matrix with contradiction detection; all local with source citations.
- [ ] **UC-FREELANCE-01:** Lead scraper → proposal draft → invoice parse → follow-up reminder full pipeline runs as Automation.
- [ ] **UC-CREATOR-01:** Weekly content digest → tone-matched draft pipeline with Coworker voice rules, draft approval gate.
