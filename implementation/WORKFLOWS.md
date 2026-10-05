# Cowork replacement assessment

Research checked 2026-10-05. This is a prospective capability assessment: LiteCowork
currently has contracts, not a working replacement. Vendor examples indicate demand,
not proof our implementation succeeds. Completion requires the real-use tests below.

Claude documents folder/file work and structured deliverables; its usage analysis reports
actual aggregated product sessions, although that sample is vendor-selected and cannot
establish all-user prevalence. [Getting started](https://support.claude.com/en/articles/13345190-get-started-with-claude-cowork),
[usage study](https://claude.com/blog/how-people-are-using-claude-cowork).
Muse describes background goals, browser actions, rich artifacts, editable memory,
identity, structured approvals and restrained proactive notifications. It gives personal
email/calendar/shopping examples. Those are vendor-reported behavior and anecdotes, not
our independent test results. [Muse design](https://introducing.muse.ai/).
ChatGPT documents file previews and iterative artifact work.
[File viewer](https://learn.chatgpt.com/docs/artifacts-viewer?surface=app).

## Workflow coverage and laptop replacement

| Workflow / evidence | LiteCowork implementation path | Release acceptance | External constraints / verdict after implementation |
|---|---|---|---|
| Organize messy folders / Claude | Resource index + bounded agent + mediated rename/move | U01, exact manifest/digest compare | Can replace locally; system folders/symlinks need explicit scope |
| Receipts to expense workbook / Claude | OCR/table extraction + office provider + verifier | U02, gold totals/currency/formulas | Can replace; OCR/model accuracy, office license/fidelity must qualify |
| Research many PDFs and web / Claude | RAG + scoped browser/research + cited report Artifact | U03, resolve source citations | Can replace; paywalls/available sources and model grounding constrain |
| Transcript/email meeting preparation / Claude | Inputs/connectors + deterministic source trace + document | U04/U06 | Can replace; account permissions and connector schemas apply |
| Forecast/spreadsheet/slides / Claude | Office capability + independent recalc/render checks | U05 | Can replace supported formats; macros/native-app layout may differ |
| Code review/refactor / native coding harness | Native lead + cheap workers/worktrees + tests/diff verification | U07/U08 | Can replace covered agent tasks; provider auth/quota/native features remain upstream |
| Personal obligations/calendar / Muse examples | Read connector, extracted proposal, human-approved mutation | U06/U09 | Mostly replace; calendar/timezone correctness and account access required |
| Browse/form/reservation preparation / Muse | API→DOM→accessibility→computer, takeover/approval | U10/U11 | Prepare and hand off final sensitive action; no CAPTCHA or site restriction bypass |
| Spending tracker/dashboard / Muse artifact pattern | Data capability + sandboxed renderer + versioned Artifact | U12 | Can replace supported data; sources must refresh under policy |
| Scheduled research/site change review / persistent assistants | Routine/Automation/occurrence/health with bounded producer | U13/U14 | Can replace; laptop must be awake for local-only inputs; cloud eligible otherwise |
| Editable profile/continuity / Muse pattern | ContextDocument revisions + scoped retrieval + durable Task | U15/U16 | Replace explicit continuity; no invisible autonomous memory proposal V1 |
| Learn repeated browser work / Teach-a-task pattern | Bounded semantic demonstration→reviewed SkillProposal→LiteSPM | U17 | Can replace qualified sites; drift and package contract can block publishing |
| Rich output refinement / ChatGPT file work | Workbench renderer + immutable versions and conflict-safe edits | U05/U12 | Can replace renderer-qualified artifacts; not arbitrary vendor-private app semantics |
| Background cloud work / Cowork category | Same Task→new cloud Attempt after reconciliation/fencing | U18/U19 | Laptop may close for portable work; local apps never teleport |
| Remote workstation / our release requirement | Paired headless Runtime + explicit resource/environment placement | U20 | Can replace eligible remote tasks; network/auth/OS provider limits apply |

## Conclusion and claim threshold

The architecture can cover most laptop file, research, coding, document, scheduling and
browser-preparation workflows after these stories are implemented and verified. It cannot
promise the same output quality as a proprietary model, universal site access, unlimited
subscription quota, hard realtime, or unattended financial authorization. User review and
provider-specific capability matrices determine supported replacement, not feature lists.

Measure each workflow: correct verified result, human interventions, source fidelity,
known cost/unknown usage, latency, recovery, permission safety and output usability.
Compare a baseline native harness/local app and LiteCowork on the same fixtures where
permitted; no marketing efficiency multiplier until measured. Keep local-model comparison
separate from premium-provider comparison. Real private workflows run in owner-approved
test accounts and private datasets; published evidence remains sanitized.
