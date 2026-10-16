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
| Scheduled research/site change review / persistent assistants | Routine/Automation/occurrence/health with bounded producer | U13/U14 | Desktop V1 requires the qualified local Runtime awake; no promised cloud fallback before E11 |
| Editable context continuity / Muse pattern | Owner-authored ContextDocument revisions, scoped retrieval, and revocation/deletion through ResourceService | U15 | Concurrent edit conflict requires explicit resolution; revoked content is excluded from new retrieval; deletion completes only after exact purge receipts. U15 does not test automatic learning. |
| Goal progress and non-executable suggestions | Goal projection from verified Task outcomes | U16 | Unverified completion does not advance progress; a suggestion cannot execute itself. This is E08-S02, not a memory case. |
| Quiet private learning target | Eligible source -> LearningCandidate -> policy-gated Coworker-private MemoryRecord, with provenance, owner visibility/opt-out, conflict and revocation controls | UIV1-120–UIV1-127 (supplemental target cases; see [Learning](../docs/LEARNING.md) and [V1 design coverage](V1-DESIGN-COVERAGE.md)) | Accepted E08-S03 target; U15/U16 do not cover this behavior. LearningCandidate/MemoryRecord automatic-learning behavior remains unimplemented; no implementation or acceptance claim. |
| Learn repeated browser work / Teach-a-task pattern | Bounded semantic demonstration→reviewed SkillProposal→LiteSPM | U17 | Can replace qualified sites; drift and package contract can block publishing |
| Rich output refinement / ChatGPT file work | Workbench renderer + immutable versions and conflict-safe edits | U05/U12 | Can replace renderer-qualified artifacts; not arbitrary vendor-private app semantics |
| Background cloud work / Cowork category (**post-V1 E11**) | Same Task→new cloud Attempt after reconciliation/fencing | U18/U19 | Not a desktop V1 release promise; laptop-close continuation only after qualified E11 |
| Remote workstation (**post-V1 E12**) | Paired headless Runtime + explicit resource/environment placement | U20 | Not a desktop V1 release promise; eligible remote tasks require E12 qualification |

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


## Added 2026-10-10 integration and learning acceptance workflows

| Workflow | Architecture path | Acceptance |
|---|---|---|
| Connect first-party app locally | ConnectorPackage -> PKCE/loopback -> SecretStore -> Connection | token absent from logs/events; refresh/revoke; scopes do not silently widen |
| Event while laptop asleep | provider/EventRelay hint -> TriggerCoordinator -> authorized fetch -> occurrence | duplicate/reorder/offline replay, revoked account, prompt injection, misfire |
| Existing external workflow | Task Step -> CapabilityInvocation -> WorkflowBackend -> Effect/Evidence | TEST vs PRODUCTION, timeout-after-effect, cancellation ambiguity, outage |
| Continue scheduled run | occurrence -> Task -> unique follow-up Conversation | duplicate Continue same Conversation; no cursor change; resume or honest rebuild |
| Correct Coworker once | correction -> LearningCandidate -> memory/instruction path | third-party content cannot auto-learn; conflicts/revoke/temporary chat |
| Successful procedure to Skill | work -> redaction -> SkillProposal -> isolated tests -> LiteSPM | no secrets/task IDs; no automatic publication; drift requalification |
| Start Code task | New preset -> same Conversation/Task + coding defaults | no separate history/store/mode; agent switch preserves native controls |

## V1 detailed design integration — 2026-10-12

Cross-workflow Task/Automation/Effect/NeedsYou/browser control event and recovery logic is expanded in ../docs/V1-WORKFLOW-EVENT-LLD.md; function ownership in ../docs/V1-FUNCTION-FILE-CATALOG.md, USER evidence in V1-DESIGN-COVERAGE.md. UI status labels must be grounded in exact persisted state; separate trigger enabled from Task execution.
