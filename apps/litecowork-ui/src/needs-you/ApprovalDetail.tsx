import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  clearPendingApprovalDecision,
  getOrCreatePendingApprovalDecision,
  isApprovalDetailFor,
  type ApprovalDecision,
  type ApprovalDetailView,
} from "./approval-state";
import "./approval-detail.css";

function displayTime(value: string | null): string {
  if (value === null) return "No expiry recorded";
  const parsed = Date.parse(value);
  return Number.isFinite(parsed) ? new Date(parsed).toLocaleString() : "Time unavailable";
}

function riskLabel(risk: ApprovalDetailView["risk"]): string {
  switch (risk) {
    case "SAFE": return "Lower risk";
    case "SENSITIVE": return "Sensitive";
    case "HIGH_IMPACT": return "High impact";
  }
}

function statusLabel(status: ApprovalDetailView["status"]): string {
  switch (status) {
    case "PENDING": return "Waiting for your decision";
    case "APPROVED": return "Approved";
    case "DENIED": return "Denied";
    case "EXPIRED": return "Expired";
    case "CANCELLED": return "Cancelled";
  }
}

export function ApprovalDetail({
  workspaceId,
  approvalId,
  operatorReady,
  refreshToken = 0,
  onBack,
  onResolved,
}: {
  workspaceId: string;
  approvalId: string;
  operatorReady: boolean;
  refreshToken?: number;
  onBack: () => void;
  onResolved: () => void;
}) {
  const [loaded, setLoaded] = useState<{ workspaceId: string; approvalId: string; value: ApprovalDetailView } | null>(null);
  const detail = loaded?.workspaceId === workspaceId && loaded.approvalId === approvalId ? loaded.value : null;
  const [loading, setLoading] = useState(false);
  const [acting, setActing] = useState(false);
  const [failure, setFailure] = useState<"offline" | "load" | "resolve" | null>(null);
  const [retryVersion, setRetryVersion] = useState(0);
  const generation = useRef(0);
  const title = useRef<HTMLHeadingElement>(null);

  useLayoutEffect(() => {
    title.current?.focus();
  }, [workspaceId, approvalId]);

  useLayoutEffect(() => {
    generation.current += 1;
    setLoaded(current => current?.workspaceId === workspaceId && current.approvalId === approvalId ? current : null);
  }, [workspaceId, approvalId, operatorReady, retryVersion, refreshToken]);

  useEffect(() => {
    const current = generation.current;
    let active = true;
    if (!workspaceId || !approvalId) {
      setLoaded(null);
      setLoading(false);
      setFailure("load");
      return () => { active = false; };
    }
    if (!operatorReady) {
      setLoading(false);
      setFailure("offline");
      return () => { active = false; };
    }
    setLoading(true);
    setFailure(null);
    void invoke<unknown>("get_approval_detail", { workspaceId, approvalId }).then(value => {
      if (!active || generation.current !== current) return;
      if (!isApprovalDetailFor(workspaceId, approvalId, value)) throw new Error("Approval detail did not match this Workspace and approval.");
      const checked = value as ApprovalDetailView;
      setLoaded({ workspaceId, approvalId, value: checked });
      setFailure(null);
      if (checked.status !== "PENDING") {
        try { clearPendingApprovalDecision(window.sessionStorage, workspaceId, approvalId); } catch {}
      }
    }).catch(() => {
      if (!active || generation.current !== current) return;
      setFailure("load");
    }).finally(() => {
      if (active && generation.current === current) setLoading(false);
    });
    return () => { active = false; };
  }, [workspaceId, approvalId, operatorReady, retryVersion, refreshToken]);

  const retry = () => setRetryVersion(value => value + 1);
  const resolve = async (decision: ApprovalDecision) => {
    if (!detail || detail.status !== "PENDING" || !operatorReady || acting) return;
    setActing(true);
    setFailure(null);
    try {
      const pending = getOrCreatePendingApprovalDecision(window.sessionStorage, workspaceId, detail, decision);
      const updated = await invoke<unknown>("resolve_approval", {
        workspaceId,
        approvalId,
        expectedVersion: pending.expectedVersion,
        decision: pending.decision,
        requestId: pending.requestId,
        confirmationDigest: detail.actionDigest,
      });
      if (!isApprovalDetailFor(workspaceId, approvalId, updated)) throw new Error("Approval response did not match this Workspace and approval.");
      const resolved = updated as ApprovalDetailView;
      if (resolved.status !== (decision === "APPROVE" ? "APPROVED" : "DENIED") || resolved.version <= detail.version) {
        throw new Error("Approval response did not confirm the requested decision.");
      }
      clearPendingApprovalDecision(window.sessionStorage, workspaceId, approvalId);
      setLoaded({ workspaceId, approvalId, value: resolved });
      onResolved();
    } catch {
      // Preserve the exact request id and decision so a retry after a lost response is idempotent.
      setFailure("resolve");
      setRetryVersion(value => value + 1);
    } finally {
      setActing(false);
    }
  };

  const showingSavedDetails = failure === "offline" && detail !== null;
  const canResolve = operatorReady && !loading && !acting && failure === null && detail?.status === "PENDING";

  return <section className="approval-detail" aria-labelledby="approval-detail-title" aria-busy={loading || acting}>
    <div className="approval-detail-heading">
      <div>
        <p className="approval-detail-context">Approval for a Task</p>
        <h2 id="approval-detail-title" ref={title} tabIndex={-1}>Review this action</h2>
      </div>
      <button type="button" className="quiet-button" onClick={onBack}>Back to Needs You</button>
    </div>
    {loading && <p className="approval-detail-notice" role="status">{detail ? "Checking the latest approval status. Showing the saved details until that check finishes." : "Loading approval details…"}</p>}
    {showingSavedDetails && <p className="approval-detail-notice" role="status">The local Runtime is offline. These saved details are read-only until the approval can be checked again.</p>}
    {failure === "load" && <div className="approval-detail-error" role="alert"><p>Approval details could not be checked in this Workspace.</p><button type="button" className="quiet-button" onClick={retry} disabled={!operatorReady}>Try again</button></div>}
    {failure === "resolve" && <div className="approval-detail-error" role="alert"><p>The decision was not confirmed. Refresh to check its saved status, then retry the same choice if it is still waiting.</p><button type="button" className="quiet-button" onClick={retry} disabled={!operatorReady}>Refresh status</button></div>}
    {detail && <>
      <div className="approval-detail-status"><span>{statusLabel(detail.status)}</span><span className={`approval-risk risk-${detail.risk.toLowerCase()}`}>{riskLabel(detail.risk)}</span></div>
      <section className="approval-detail-action" aria-labelledby="approval-action-heading">
        <h3 id="approval-action-heading">What will happen</h3>
        <p>{detail.actionSummary}</p>
      </section>
      <dl className="approval-detail-facts">
        <div><dt>Requested for</dt><dd>{detail.kind.replaceAll("_", " ").toLowerCase()}</dd></div>
        <div><dt>Target Resource</dt><dd>{detail.targetRef.resourceId}{detail.targetRef.revisionId ? ` · revision ${detail.targetRef.revisionId}` : " · current target"}</dd></div>
        <div><dt>Required assurance</dt><dd>{detail.requiredAssurance.replaceAll("_", " ").toLowerCase()}</dd></div>
        <div><dt>Expires</dt><dd>{displayTime(detail.expiresAt)}</dd></div>
      </dl>
      <p className="approval-detail-boundary">Your choice records this exact-action decision only. It does not grant a reusable capability or run the action by itself.</p>
      {detail.status === "PENDING" && <div className="approval-detail-actions">
        <button type="button" className="quiet-button" onClick={() => void resolve("DENY")} disabled={!canResolve} aria-busy={acting}>Deny action</button>
        <button type="button" className="primary-button" onClick={() => void resolve("APPROVE")} disabled={!canResolve} aria-busy={acting}>Approve this action</button>
        {!operatorReady && <span>The local Runtime must be connected before you can decide.</span>}
      </div>}
    </>}
  </section>;
}
