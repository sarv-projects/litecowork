import { useEffect, useState } from "react";
import type { SkillApi, SkillProposal, SkillProposalStatus } from "./skill-proposal-api.ts";
import { declaredCapabilityNotice, skillProposalDecisionOutcome, skillProposalReviewState } from "./skill-proposal-state.ts";
import "./skill-proposals-page.css";

type Props = {
  api: SkillApi;
};

type LoadState =
  | { kind: "loading" }
  | { kind: "ready"; proposals: SkillProposal[] }
  | { kind: "empty" }
  | { kind: "error"; message: string };

const STATUS_FILTERS: { value: SkillProposalStatus | ""; label: string }[] = [
  { value: "", label: "All" },
  { value: "REVIEW", label: "Needs review" },
  { value: "DRAFT", label: "Drafts" },
  { value: "APPROVED", label: "Approved" },
  { value: "REJECTED", label: "Rejected" },
];

function freshRequestId(): string {
  const randomUUID = globalThis.crypto?.randomUUID;
  if (!randomUUID) throw new Error("This desktop session cannot create secure request identities.");
  return randomUUID.call(globalThis.crypto);
}

export function SkillProposalsPage({ api }: Props) {
  const [state, setState] = useState<LoadState>({ kind: "loading" });
  const [status, setStatus] = useState<SkillProposalStatus | "">("");
  const [notice, setNotice] = useState<string>("");
  const [busyId, setBusyId] = useState<string | null>(null);

  useEffect(() => {
    const controller = new AbortController();
    setState({ kind: "loading" });
    api
      .proposals(status === "" ? undefined : status, undefined, controller.signal)
      .then(page => setState(page.items.length === 0 ? { kind: "empty" } : { kind: "ready", proposals: page.items }))
      .catch(error => {
        if (controller.signal.aborted) return;
        setState({ kind: "error", message: error instanceof Error ? error.message : "The Skill proposals could not be read." });
      });
    return () => controller.abort();
  }, [api, status]);

  const decide = async (proposal: SkillProposal, decision: "approve" | "reject") => {
    const review = skillProposalReviewState(proposal);
    if (decision === "approve" && !review.canApprove) {
      setNotice(review.blockedReason);
      return;
    }
    setBusyId(proposal.skill_proposal_id);
    setNotice("");
    try {
      const requestId = freshRequestId();
      const updated = decision === "approve"
        ? await api.approve(proposal.skill_proposal_id, proposal.version, requestId)
        : await api.reject(proposal.skill_proposal_id, proposal.version, requestId);
      setState(current => (current.kind === "ready"
        ? {
            kind: "ready",
            proposals: current.proposals.map(item => (item.skill_proposal_id === updated.skill_proposal_id ? updated : item)),
          }
        : current));
      setNotice(skillProposalDecisionOutcome(decision).message);
    } catch (error) {
      setNotice(error instanceof Error ? error.message : "The decision could not be recorded.");
    } finally {
      setBusyId(null);
    }
  };

  return (
    <section className="skill-proposals-page" aria-label="Skill proposals">
      <header className="skill-proposals-page__header">
        <h2>Skill proposals</h2>
        <p className="skill-proposals-page__hint">
          A proposal is a draft. Approving it records your decision only: nothing is installed, activated, or granted.
        </p>
        <label className="skill-proposals-page__filter">
          <span>Status</span>
          <select value={status} onChange={event => setStatus(event.target.value as SkillProposalStatus | "")}>
            {STATUS_FILTERS.map(filter => (
              <option key={filter.value || "all"} value={filter.value}>
                {filter.label}
              </option>
            ))}
          </select>
        </label>
      </header>

      {notice !== "" && <p className="skill-proposals-page__notice">{notice}</p>}
      {state.kind === "loading" && <p className="skill-proposals-page__status">Loading Skill proposals…</p>}
      {state.kind === "error" && <p className="skill-proposals-page__status skill-proposals-page__status--error">{state.message}</p>}
      {state.kind === "empty" && <p className="skill-proposals-page__status">No Skill proposal is waiting.</p>}

      {state.kind === "ready" && (
        <ul className="skill-proposals-page__list">
          {state.proposals.map(proposal => {
            const review = skillProposalReviewState(proposal);
            return (
              <li key={proposal.skill_proposal_id} className="skill-proposal">
                <div className="skill-proposal__body">
                  <span className="skill-proposal__title">{review.title}</span>
                  <span className="skill-proposal__meta">
                    {proposal.source} · redaction {proposal.redaction_status} · {proposal.status}
                  </span>
                  <span className="skill-proposal__note">{review.note}</span>
                  {review.blockedReason !== "" && <span className="skill-proposal__blocked">{review.blockedReason}</span>}
                  <span className="skill-proposal__note">{declaredCapabilityNotice(proposal)}</span>
                  {proposal.placeholders.length > 0 && (
                    <span className="skill-proposal__note">
                      {proposal.placeholders.length} task-specific value(s) were replaced by typed placeholders.
                    </span>
                  )}
                </div>
                <div className="skill-proposal__actions">
                  <button
                    type="button"
                    disabled={!review.canApprove || busyId === proposal.skill_proposal_id}
                    onClick={() => void decide(proposal, "approve")}
                  >
                    Approve
                  </button>
                  <button
                    type="button"
                    disabled={!review.canReject || busyId === proposal.skill_proposal_id}
                    onClick={() => void decide(proposal, "reject")}
                  >
                    Reject
                  </button>
                </div>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
