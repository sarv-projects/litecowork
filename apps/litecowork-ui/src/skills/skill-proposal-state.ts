// The owner-review projection of one SkillProposal.
//
// A draft never enables itself. This module decides, from the stored record
// alone, whether an owner may approve or reject a proposal and what the owner
// is told about the outcome. It never infers authority from a proposal's
// existence, its declared capabilities, or its source.
//
// Authority: docs/LEARNING.md section 9, docs/CAPABILITY-FABRIC.md.

import type { SkillProposal } from "./skill-proposal-api.ts";

export type SkillProposalDecision = "approve" | "reject";

export type SkillProposalReviewState = {
  proposalId: string;
  title: string;
  status: SkillProposal["status"];
  redactionStatus: SkillProposal["redaction_status"];
  /** True when the owner may record an approval. */
  canApprove: boolean;
  /** True when the owner may record a rejection. */
  canReject: boolean;
  /** The plain reason an approval is unavailable. Empty when it is available. */
  blockedReason: string;
  /** The plain reason the proposal cannot be used yet. */
  note: string;
  /** True when the proposal drifted and needs a fresh review. */
  drifted: boolean;
};

const DRIFT_BLOCKER = "SKILL_DRIFT_DETECTED";
const PACKAGE_BLOCKER = "LITESPOM_PACKAGE_SERVICE_NOT_QUALIFIED";

export function skillProposalReviewState(proposal: SkillProposal): SkillProposalReviewState {
  const drifted = proposal.drift !== null;
  const title = proposal.placeholders.length > 0
    ? `Skill proposal ${proposal.skill_proposal_id}`
    : `Skill proposal ${proposal.skill_proposal_id}`;
  if (proposal.status === "REJECTED" || proposal.status === "PUBLISHED") {
    return {
      proposalId: proposal.skill_proposal_id,
      title,
      status: proposal.status,
      redactionStatus: proposal.redaction_status,
      canApprove: false,
      canReject: false,
      blockedReason: "This proposal is closed.",
      note: proposal.status === "PUBLISHED"
        ? "LiteSPM published this proposal."
        : "The owner rejected this proposal.",
      drifted,
    };
  }
  if (proposal.status === "APPROVED") {
    return {
      proposalId: proposal.skill_proposal_id,
      title,
      status: proposal.status,
      redactionStatus: proposal.redaction_status,
      canApprove: false,
      canReject: false,
      blockedReason: "This proposal is already approved.",
      note: "Publication stays with LiteSPM and is not qualified yet.",
      drifted,
    };
  }
  if (proposal.status === "DRAFT") {
    return {
      proposalId: proposal.skill_proposal_id,
      title,
      status: proposal.status,
      redactionStatus: proposal.redaction_status,
      canApprove: false,
      canReject: false,
      blockedReason: "This proposal has not entered review yet.",
      note: "The captured procedure has not been shown to the owner.",
      drifted,
    };
  }
  if (proposal.redaction_status === "FAILED") {
    return {
      proposalId: proposal.skill_proposal_id,
      title,
      status: proposal.status,
      redactionStatus: proposal.redaction_status,
      canApprove: false,
      canReject: true,
      blockedReason: "The captured procedure still carries forbidden content.",
      note: "Reject this proposal and capture the workflow again.",
      drifted,
    };
  }
  if (proposal.redaction_status === "PENDING") {
    return {
      proposalId: proposal.skill_proposal_id,
      title,
      status: proposal.status,
      redactionStatus: proposal.redaction_status,
      canApprove: false,
      canReject: true,
      blockedReason: "The captured procedure has not been checked for secrets.",
      note: "Review it once the redaction check has run.",
      drifted,
    };
  }
  if (drifted) {
    return {
      proposalId: proposal.skill_proposal_id,
      title,
      status: proposal.status,
      redactionStatus: proposal.redaction_status,
      canApprove: false,
      canReject: true,
      blockedReason: `The procedure changed since it was captured (${DRIFT_BLOCKER}).`,
      note: "Capture the workflow again and review the new proposal.",
      drifted,
    };
  }
  return {
    proposalId: proposal.skill_proposal_id,
    title,
    status: proposal.status,
    redactionStatus: proposal.redaction_status,
    canApprove: true,
    canReject: true,
    blockedReason: "",
    note: `Approving records your decision only. Publication stays with LiteSPM (${PACKAGE_BLOCKER}).`,
    drifted,
  };
}

/**
 * The outcome of an owner decision.
 *
 * An approval never installs a package, never activates a Skill, and never
 * grants authority. The publication blocker is reported plainly.
 */
export function skillProposalDecisionOutcome(
  decision: SkillProposalDecision,
): { activatesSkill: false; installsPackage: false; grantsAuthority: false; message: string } {
  if (decision === "reject") {
    return {
      activatesSkill: false,
      installsPackage: false,
      grantsAuthority: false,
      message: "The proposal was rejected. Nothing was installed or activated.",
    };
  }
  return {
    activatesSkill: false,
    installsPackage: false,
    grantsAuthority: false,
    message: `Approved for review only. Nothing was installed or activated. Publication stays with LiteSPM (${PACKAGE_BLOCKER}).`,
  };
}

/**
 * The declared capabilities a proposal asks for.
 *
 * A proposal's declared capabilities are a request, never authority. This
 * helper exists so the UI can say so next to the list.
 */
export function declaredCapabilityNotice(proposal: SkillProposal): string {
  if (proposal.declared_capabilities.length === 0) {
    return "This procedure asks for no capability.";
  }
  return `This procedure asks for ${proposal.declared_capabilities.length} capability reference(s). Asking is not being granted.`;
}
