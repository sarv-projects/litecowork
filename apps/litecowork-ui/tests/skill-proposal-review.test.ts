import assert from "node:assert/strict";
import { test } from "node:test";

const { decodeSkill, decodeSkillProposal, decodeSkillProposalPage } = await import("../src/skills/skill-proposal-api.ts");
const {
  declaredCapabilityNotice,
  skillProposalDecisionOutcome,
  skillProposalReviewState,
} = await import("../src/skills/skill-proposal-state.ts");

const WORKSPACE = "workspace-1";

function baseProposal(overrides: Record<string, unknown> = {}) {
  return {
    skill_proposal_id: "sp-1",
    workspace_id: WORKSPACE,
    source: "DEMONSTRATION",
    source_task_id: "task-1",
    source_artifact_id: "artifact-1",
    draft_resource_ref: { workspace_id: WORKSPACE, resource_id: "res-1", revision_id: null },
    draft_digest: `sha256:${"b".repeat(64)}`,
    redaction_status: "PASSED",
    status: "REVIEW",
    approved_by: null,
    published_capability_ref: null,
    placeholders: [],
    declared_capabilities: [],
    drift: null,
    created_at: "2026-10-20T10:00:00Z",
    updated_at: "2026-10-20T10:00:00Z",
    version: 1,
    ...overrides,
  };
}

test("a proposal from another Workspace is refused", () => {
  assert.throws(() => decodeSkillProposal(baseProposal({ workspace_id: "workspace-other" }), WORKSPACE), /another Workspace/);
});

test("a proposal whose draft belongs to another Workspace is refused", () => {
  assert.throws(
    () =>
      decodeSkillProposal(
        baseProposal({ draft_resource_ref: { workspace_id: "workspace-other", resource_id: "res-1", revision_id: null } }),
        WORKSPACE,
      ),
    /another Workspace/,
  );
});

test("a proposal with a malformed draft digest is refused", () => {
  assert.throws(() => decodeSkillProposal(baseProposal({ draft_digest: "nope" }), WORKSPACE), /invalid SkillProposal digest/);
});

test("an unsupported proposal status is refused rather than defaulted", () => {
  assert.throws(() => decodeSkillProposal(baseProposal({ status: "MAYBE" }), WORKSPACE), /unsupported SkillProposal status/);
});

test("a proposal that is in review with a passed redaction can be approved", () => {
  const state = skillProposalReviewState(decodeSkillProposal(baseProposal(), WORKSPACE));
  assert.equal(state.canApprove, true);
  assert.equal(state.canReject, true);
  assert.equal(state.blockedReason, "");
});

test("a proposal whose redaction failed can never be approved", () => {
  const state = skillProposalReviewState(decodeSkillProposal(baseProposal({ redaction_status: "FAILED" }), WORKSPACE));
  assert.equal(state.canApprove, false);
  assert.equal(state.canReject, true);
  assert.match(state.blockedReason, /forbidden content/);
});

test("a proposal whose redaction has not run can never be approved", () => {
  const state = skillProposalReviewState(decodeSkillProposal(baseProposal({ redaction_status: "PENDING" }), WORKSPACE));
  assert.equal(state.canApprove, false);
  assert.match(state.blockedReason, /not been checked for secrets/);
});

test("a draft proposal cannot be approved before it enters review", () => {
  const state = skillProposalReviewState(decodeSkillProposal(baseProposal({ status: "DRAFT" }), WORKSPACE));
  assert.equal(state.canApprove, false);
  assert.equal(state.canReject, false);
});

test("an approved proposal is closed for further review", () => {
  const state = skillProposalReviewState(decodeSkillProposal(baseProposal({ status: "APPROVED" }), WORKSPACE));
  assert.equal(state.canApprove, false);
  assert.equal(state.canReject, false);
  assert.match(state.note, /LiteSPM/);
});

test("a rejected proposal is terminal", () => {
  const state = skillProposalReviewState(decodeSkillProposal(baseProposal({ status: "REJECTED" }), WORKSPACE));
  assert.equal(state.canApprove, false);
  assert.equal(state.canReject, false);
});

test("a drifted proposal needs a fresh capture and review", () => {
  const state = skillProposalReviewState(
    decodeSkillProposal(
      baseProposal({
        drift: {
          expected_manifest_digest: `sha256:${"c".repeat(64)}`,
          observed_manifest_digest: `sha256:${"d".repeat(64)}`,
          drifted_resources: ["ui://app"],
          detected_at: "2026-10-20T11:00:00Z",
        },
      }),
      WORKSPACE,
    ),
  );
  assert.equal(state.canApprove, false);
  assert.equal(state.drifted, true);
  assert.match(state.blockedReason, /SKILL_DRIFT_DETECTED/);
});

test("approving a proposal never activates a Skill, installs a package, or grants authority", () => {
  const outcome = skillProposalDecisionOutcome("approve");
  assert.equal(outcome.activatesSkill, false);
  assert.equal(outcome.installsPackage, false);
  assert.equal(outcome.grantsAuthority, false);
  assert.match(outcome.message, /LiteSPM/);
});

test("rejecting a proposal reports that nothing was installed or activated", () => {
  const outcome = skillProposalDecisionOutcome("reject");
  assert.equal(outcome.activatesSkill, false);
  assert.equal(outcome.installsPackage, false);
  assert.equal(outcome.grantsAuthority, false);
  assert.match(outcome.message, /rejected/);
});

test("declared capabilities are reported as a request, never as a grant", () => {
  const proposal = decodeSkillProposal(baseProposal({ declared_capabilities: ["connector.invoice.read"] }), WORKSPACE);
  const notice = declaredCapabilityNotice(proposal);
  assert.match(notice, /Asking is not being granted/);
});

test("a proposal with no declared capability says so plainly", () => {
  const proposal = decodeSkillProposal(baseProposal(), WORKSPACE);
  assert.match(declaredCapabilityNotice(proposal), /asks for no capability/);
});

test("a proposal page refuses a proposal from another Workspace", () => {
  assert.throws(
    () => decodeSkillProposalPage({ items: [baseProposal({ workspace_id: "workspace-other" })], next_cursor: null }, WORKSPACE),
    /another Workspace/,
  );
});

test("a Skill definition keeps its capability reference and never inlines its content", () => {
  const skill = decodeSkill(
    {
      skill_id: "skill-1",
      workspace_id: WORKSPACE,
      kind: "LOCAL_PROCEDURE",
      status: "DRAFT",
      capability_ref: {
        capability_id: "local:skill-1",
        identity_kind: "LOCAL",
        source: "litecowork:local",
        package_version: null,
        digest: `sha256:${"e".repeat(64)}`,
        component: "local:skill-1",
      },
      current_revision: 1,
      revision: {
        revision: 1,
        title: "Invoice cleanup",
        summary: "Clean up an invoice",
        manifest_digest: `sha256:${"f".repeat(64)}`,
        content_digest: `sha256:${"0".repeat(64)}`,
        content_ref: "res-1",
        declared_capabilities: [],
        declared_requirements: [],
        created_at: "2026-10-20T10:00:00Z",
        created_by: "owner-1",
      },
      created_at: "2026-10-20T10:00:00Z",
      updated_at: "2026-10-20T10:00:00Z",
      version: 1,
    },
    WORKSPACE,
  );
  assert.equal(skill.status, "DRAFT");
  assert.equal(skill.revision?.content_ref, "res-1");
  assert.equal(Object.prototype.hasOwnProperty.call(skill.revision ?? {}, "content"), false);
});

test("a Skill definition with a malformed capability digest is refused", () => {
  assert.throws(
    () =>
      decodeSkill(
        {
          skill_id: "skill-1",
          workspace_id: WORKSPACE,
          kind: "LOCAL_PROCEDURE",
          status: "DRAFT",
          capability_ref: {
            capability_id: "local:skill-1",
            identity_kind: "LOCAL",
            source: "litecowork:local",
            package_version: null,
            digest: "nope",
            component: "local:skill-1",
          },
          current_revision: 1,
          revision: null,
          created_at: "2026-10-20T10:00:00Z",
          updated_at: "2026-10-20T10:00:00Z",
          version: 1,
        },
        WORKSPACE,
      ),
    /invalid Skill digest/,
  );
});
