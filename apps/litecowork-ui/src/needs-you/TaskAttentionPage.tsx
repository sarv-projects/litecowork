import { useCallback, useEffect, useLayoutEffect, useReducer, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  EMPTY_NEEDS_YOU_MOTION_BASELINE,
  isValidNeedsYouPage,
  mergeNeedsYouPage,
  nextNeedsYouMotionBaseline,
  taskAttentionLoadReducer,
  type NeedsYouItemView,
  type NeedsYouMotionBaseline,
  type NeedsYouPage,
  type NeedsYouStatus,
} from "./task-attention-state";
import { UserRequestDetail } from "./UserRequestDetail";
import { ApprovalDetail } from "./ApprovalDetail";
import { NativePermissionCard } from "./NativePermissionCard";
import {
  applyNativePermissionReceipt,
  parseWorkspacePermissionSnapshot,
  type NativePermissionDecision,
  type NativePermissionState,
  type WorkspacePermissionItem,
} from "./native-permission";
import "./task-attention-page.css";

type TaskStatus = "WAITING_USER" | "NEEDS_USER" | "BLOCKED";
type TaskSummary = {
  taskId: string;
  status: TaskStatus;
  objective: string;
  createdAt: string;
  updatedAt: string;
};
type TaskPage = { items: TaskSummary[]; nextCursor: string | null };
type StatusPage = { status: TaskStatus; nextCursor: string | null };
type StatusPageOutcome =
  | { status: TaskStatus; ok: true; page: TaskPage }
  | { status: TaskStatus; ok: false };

/** Synchronously fences same-render duplicate clicks and completions from old refreshes. */
export class TaskAttentionPaginationGate {
  private generation = -1;
  private active = false;

  invalidate(generation: number): void {
    this.generation = generation;
    this.active = false;
  }

  begin(generation: number): boolean {
    if (generation !== this.generation || this.active) return false;
    this.active = true;
    return true;
  }

  finish(generation: number): boolean {
    if (generation !== this.generation || !this.active) return false;
    this.active = false;
    return true;
  }
}

function isValidStatusPage(status: TaskStatus, page: TaskPage, previousCursor: string | null): boolean {
  return Boolean(page && Array.isArray(page.items)
    && (page.nextCursor === null || (typeof page.nextCursor === "string" && page.nextCursor.length > 0 && page.nextCursor !== previousCursor))
    && page.items.every(task => task && task.status === status && Boolean(task.taskId) && Boolean(task.objective)));
}

/** Applies only complete, valid status pages so one failed status cannot discard sibling progress. */
export function mergeTaskAttentionPages(
  items: TaskSummary[],
  pages: StatusPage[],
  outcomes: StatusPageOutcome[],
): { items: TaskSummary[]; pages: StatusPage[]; failed: boolean } {
  const successful = new Map<TaskStatus, TaskPage>();
  let failed = false;
  for (const outcome of outcomes) {
    const previousCursor = pages.find(page => page.status === outcome.status)?.nextCursor ?? null;
    if (!outcome.ok || !isValidStatusPage(outcome.status, outcome.page, previousCursor)) {
      failed = true;
      continue;
    }
    successful.set(outcome.status, outcome.page);
  }

  const mergedItems = [...items];
  const seen = new Set(items.map(item => item.taskId));
  for (const page of successful.values()) {
    for (const task of page.items) {
      if (seen.has(task.taskId)) continue;
      seen.add(task.taskId);
      mergedItems.push(task);
    }
  }

  const mergedPages = pages.map(current => {
    const page = successful.get(current.status);
    return page ? { status: current.status, nextCursor: page.nextCursor } : current;
  });
  return { items: mergedItems, pages: mergedPages, failed };
}

export function TaskAttentionLoadMoreButton({
  loading,
  disabled,
  onLoadMore,
}: {
  loading: boolean;
  disabled: boolean;
  onLoadMore: () => void;
}) {
  return <>
    <button type="button" className="quiet-button task-attention-more" onClick={onLoadMore} disabled={disabled} aria-busy={loading}>
      Load more
    </button>
    {loading && <p role="status">Loading more Tasks…</p>}
  </>;
}

const PAGE_SIZE = 100;

function kindLabel(kind: NeedsYouItemView["kind"]): string {
  switch (kind) {
    case "APPROVAL": return "Approval";
    case "USER_REQUEST": return "User request";
    case "TASK_BLOCKER": return "Task blocker";
  }
}

function displayTime(value: string | null): string {
  if (value === null) return "Not recorded";
  const time = Date.parse(value);
  return Number.isFinite(time) ? new Date(time).toLocaleString() : "Time unavailable";
}

export function NeedsYouFilter({ status, onChange }: {
  status: NeedsYouStatus;
  onChange: (status: NeedsYouStatus) => void;
}) {
  return <fieldset className="needs-you-filter">
    <legend>Show Needs You items</legend>
    <label><input type="radio" name="needs-you-status" value="OPEN" checked={status === "OPEN"} onChange={() => onChange("OPEN")} />Open</label>
    <label><input type="radio" name="needs-you-status" value="RESOLVED" checked={status === "RESOLVED"} onChange={() => onChange("RESOLVED")} />Resolved history</label>
  </fieldset>;
}

export function NeedsYouItemCard({ item, onOpenTask, onOpenUserRequest, onOpenApproval, entering = false }: {
  item: NeedsYouItemView;
  onOpenTask: (taskId: string) => void;
  onOpenUserRequest: (requestId: string, trigger: HTMLButtonElement) => void;
  onOpenApproval: (approvalId: string, trigger: HTMLButtonElement) => void;
  entering?: boolean;
}) {
  const canOpenTask = item.kind === "TASK_BLOCKER" && item.taskId !== null;
  return <article className={`needs-you-card${entering ? " needs-you-card-entering" : ""}`}>
    <div className="needs-you-card-meta">
      <span className={`needs-you-kind kind-${item.kind.toLowerCase()}`}>{kindLabel(item.kind)}</span>
      <span className={`needs-you-status status-${item.status.toLowerCase()}`}>{item.status === "OPEN" ? "Open" : "Resolved"}</span>
    </div>
    <h2>{item.title}</h2>
    <p className="needs-you-summary">{item.safeSummary}</p>
    <p className="needs-you-timestamps">
      Created {displayTime(item.createdAt)}
      {item.status === "RESOLVED" ? ` · Resolved ${displayTime(item.resolvedAt)}` : ""}
    </p>
    {canOpenTask
      ? <div className="needs-you-card-action">
        <button type="button" className="quiet-button" onClick={() => onOpenTask(item.taskId!)}>Open current Task</button>
        <span>Opening the Task does not resolve this blocker.</span>
      </div>
        : item.kind === "TASK_BLOCKER"
          ? <p className="needs-you-action-note">Task details are unavailable for this item.</p>
          : item.kind === "USER_REQUEST" && item.userRequestId
            ? <div className="needs-you-card-action">
              <button type="button" className="quiet-button" onClick={event => onOpenUserRequest(item.userRequestId!, event.currentTarget)}>View request details</button>
              <span>Review only. Opening this request does not answer or dismiss it.</span>
            </div>
            : item.kind === "APPROVAL" && item.approvalId
              ? <div className="needs-you-card-action">
                <button type="button" className="quiet-button" onClick={event => onOpenApproval(item.approvalId!, event.currentTarget)}>{item.status === "OPEN" ? "Review decision" : "View decision"}</button>
                <span>{item.status === "OPEN" ? "Review the exact action and target before deciding." : "This saved decision is read-only."}</span>
              </div>
              : <p className="needs-you-action-note">Approval details are unavailable for this item.</p>}
  </article>;
}

function NeedsYouLoadMoreButton({ loading, disabled, onLoadMore }: {
  loading: boolean;
  disabled: boolean;
  onLoadMore: () => void;
}) {
  return <div className="needs-you-load-more">
    <button type="button" className="quiet-button task-attention-more" onClick={onLoadMore} disabled={disabled} aria-busy={loading}>Load more</button>
    {loading && <p role="status">Loading more Needs You items…</p>}
  </div>;
}

/** Workspace-scoped projection of persisted approval, request, and Task blocker rows. */
export function TaskAttentionPage({
  workspaceId,
  operatorReady,
  onOpenTask,
}: {
  workspaceId: string;
  operatorReady: boolean;
  onOpenTask: (taskId: string) => void;
}) {
  const [selectedUserRequest, setSelectedUserRequest] = useState<{ workspaceId: string; requestId: string } | null>(null);
  const [selectedApproval, setSelectedApproval] = useState<{ workspaceId: string; approvalId: string } | null>(null);
  const itemOpener = useRef<HTMLButtonElement | null>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const [status, setStatus] = useState<NeedsYouStatus>("OPEN");
  const [items, setItems] = useState<NeedsYouItemView[]>([]);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [loadedWorkspace, setLoadedWorkspace] = useState<string | null>(null);
  const [loadedStatus, setLoadedStatus] = useState<NeedsYouStatus | null>(null);
  const [enteringItemIds, setEnteringItemIds] = useState<ReadonlySet<string>>(() => new Set<string>());
  const motionBaseline = useRef<NeedsYouMotionBaseline>(EMPTY_NEEDS_YOU_MOTION_BASELINE);
  const [busy, setBusy] = useState(Boolean(workspaceId));
  const [loadState, dispatchLoadState] = useReducer(taskAttentionLoadReducer, { generation: 0, loadingMore: false });
  const loadingMore = loadState.loadingMore;
  const [refreshToken, setRefreshToken] = useState(0);
  const [failure, setFailure] = useState<"refresh" | "more" | null>(null);
  const [livePermissionSnapshot, setLivePermissionSnapshot] = useState<{ workspaceId: string; items: WorkspacePermissionItem[] } | null>(null);
  const [livePermissionLoading, setLivePermissionLoading] = useState(false);
  const [livePermissionError, setLivePermissionError] = useState(false);
  const [livePermissionActions, setLivePermissionActions] = useState<Record<string, NativePermissionDecision>>({});
  const livePermissionWorkspace = useRef(workspaceId);
  livePermissionWorkspace.current = workspaceId;
  const livePermissionGeneration = useRef(0);
  const livePermissionActionLock = useRef(new Set<string>());
  const generation = useRef(0);
  const previousWorkspace = useRef(workspaceId);
  const paginationGate = useRef(new TaskAttentionPaginationGate());
  const visibleItems = loadedWorkspace === workspaceId && loadedStatus === status ? items : [];

  const refreshLivePermissions = useCallback(async (requestedWorkspaceId = workspaceId) => {
    if (!requestedWorkspaceId || !operatorReady) return null;
    const generation = ++livePermissionGeneration.current;
    setLivePermissionLoading(true);
    setLivePermissionError(false);
    try {
      const value = await invoke<unknown>("list_workspace_live_conversation_permissions", { workspaceId: requestedWorkspaceId });
      const parsed = parseWorkspacePermissionSnapshot(value, requestedWorkspaceId);
      if (!parsed) throw new Error("Workspace permission snapshot did not match its contract");
      if (livePermissionGeneration.current === generation && livePermissionWorkspace.current === requestedWorkspaceId) {
        setLivePermissionSnapshot(parsed);
        setLivePermissionActions({});
        setLivePermissionError(false);
      }
      return parsed;
    } catch {
      if (livePermissionGeneration.current === generation && livePermissionWorkspace.current === requestedWorkspaceId) setLivePermissionError(true);
      throw new Error("Live agent requests could not be checked");
    } finally {
      if (livePermissionGeneration.current === generation && livePermissionWorkspace.current === requestedWorkspaceId) setLivePermissionLoading(false);
    }
  }, [operatorReady, workspaceId]);

  useEffect(() => {
    if (!workspaceId || !operatorReady) {
      setLivePermissionSnapshot(null);
      setLivePermissionLoading(false);
      setLivePermissionError(false);
      return;
    }
    void refreshLivePermissions(workspaceId).catch(() => {});
    const refreshIfVisible = () => {
      if (document.visibilityState === "visible") void refreshLivePermissions(workspaceId).catch(() => {});
    };
    window.addEventListener("focus", refreshIfVisible);
    document.addEventListener("visibilitychange", refreshIfVisible);
    return () => {
      livePermissionGeneration.current += 1;
      window.removeEventListener("focus", refreshIfVisible);
      document.removeEventListener("visibilitychange", refreshIfVisible);
    };
  }, [operatorReady, refreshLivePermissions, workspaceId]);

  const replyToLivePermission = useCallback(async (item: WorkspacePermissionItem, decision: NativePermissionDecision) => {
    const token = item.promptToken;
    if (!operatorReady || livePermissionError || livePermissionLoading || livePermissionActionLock.current.has(token)
      || item.state !== "PENDING" || livePermissionWorkspace.current !== workspaceId) return;
    livePermissionActionLock.current.add(token);
    setLivePermissionActions(current => ({ ...current, [token]: decision }));
    try {
      const receipt = await invoke<unknown>("reply_conversation_permission", {
        workspaceId,
        conversationId: item.conversationId,
        turnId: item.turnId,
        promptToken: token,
        decision,
        idempotencyKey: crypto.randomUUID(),
      });
      const prior: NativePermissionState = {
        lastSequence: item.attemptSequence,
        needsRefresh: false,
        prompts: { [token]: item },
      };
      const reconciled = applyNativePermissionReceipt(prior, receipt);
      if (reconciled.needsRefresh || reconciled === prior) {
        await refreshLivePermissions(workspaceId);
      } else {
        const updated = reconciled.prompts[token];
        if (!updated || updated.state === "ANSWERED" || updated.state === "SESSION_ENDED") {
          await refreshLivePermissions(workspaceId);
        } else {
          setLivePermissionSnapshot(current => current?.workspaceId !== workspaceId ? current : {
            ...current,
            items: current.items.map(candidate => candidate.promptToken === token
              ? { ...candidate, ...updated, attemptSequence: reconciled.lastSequence }
              : candidate),
          });
        }
      }
    } catch {
      try {
        const refreshed = await refreshLivePermissions(workspaceId);
        if (refreshed && !refreshed.items.some(candidate => candidate.promptToken === token)) {
          setLivePermissionSnapshot(current => current?.workspaceId !== workspaceId ? current : {
            ...current,
            items: current.items.map(candidate => candidate.promptToken === token
              ? { ...candidate, state: "OUTCOME_UNKNOWN", decision }
              : candidate),
          });
        }
      } catch {
        setLivePermissionSnapshot(current => current?.workspaceId !== workspaceId ? current : {
          ...current,
          items: current.items.map(candidate => candidate.promptToken === token
            ? { ...candidate, state: "OUTCOME_UNKNOWN", decision }
            : candidate),
        });
      }
    } finally {
      livePermissionActionLock.current.delete(token);
      setLivePermissionActions(current => {
        const next = { ...current };
        delete next[token];
        return next;
      });
    }
  }, [livePermissionError, livePermissionLoading, operatorReady, refreshLivePermissions, workspaceId]);

  useLayoutEffect(() => {
    if (previousWorkspace.current !== workspaceId) {
      previousWorkspace.current = workspaceId;
      itemOpener.current = null;
      setSelectedUserRequest(null);
      setSelectedApproval(null);
    }
    const current = ++generation.current;
    paginationGate.current.invalidate(current);
    dispatchLoadState({ type: "invalidate", generation: current });
  }, [workspaceId, status, operatorReady, refreshToken]);

  /** Records which committed items newly appeared, so only a genuinely new card animates. */
  const recordMotion = useCallback((scopeKey: string, rendered: readonly NeedsYouItemView[]) => {
    const previous = motionBaseline.current;
    const motion = nextNeedsYouMotionBaseline(
      previous,
      scopeKey,
      rendered.map((item) => item.itemId),
    );
    motionBaseline.current = motion.baseline;
    if (previous.scopeKey !== scopeKey) {
      // A new scope renders the current state without replaying a transition.
      setEnteringItemIds(new Set<string>());
      return;
    }
    // An emphasis already running is not cut short by a refresh that returns the same card.
    setEnteringItemIds((current) => (
      motion.entering.length === 0
        ? current
        : new Set([...current, ...motion.entering])
    ));
  }, []);

  useEffect(() => {
    const current = generation.current;
    let active = true;
    setFailure(null);
    if (!workspaceId) {
      setItems([]);
      setNextCursor(null);
      setLoadedWorkspace(null);
      setLoadedStatus(null);
      setBusy(false);
      return () => { active = false; };
    }
    if (!operatorReady) {
      setBusy(false);
      setFailure("refresh");
      return () => { active = false; };
    }

    setBusy(true);
    void invoke<NeedsYouPage>("list_needs_you", {
      workspaceId,
      status,
      cursor: null,
      limit: PAGE_SIZE,
    }).then(page => {
      if (!active || generation.current !== current) return;
      if (!isValidNeedsYouPage(status, page, null)) throw new Error("Needs You response did not match the requested status or page contract.");
      setItems(page.items);
      setNextCursor(page.nextCursor);
      setLoadedWorkspace(workspaceId);
      setLoadedStatus(status);
      setFailure(null);
      recordMotion(`${workspaceId}|${status}`, page.items);
    }).catch(() => {
      if (!active || generation.current !== current) return;
      setFailure("refresh");
    }).finally(() => {
      if (active && generation.current === current) setBusy(false);
    });
    return () => { active = false; };
  }, [workspaceId, status, operatorReady, refreshToken]);

  const loadMore = async () => {
    if (!operatorReady || nextCursor === null || !workspaceId) return;
    const current = generation.current;
    if (!paginationGate.current.begin(current)) return;
    dispatchLoadState({ type: "start-more", generation: current });
    setFailure(null);
    try {
      const previousCursor = nextCursor;
      const page = await invoke<NeedsYouPage>("list_needs_you", {
        workspaceId,
        status,
        cursor: previousCursor,
        limit: PAGE_SIZE,
      });
      if (generation.current !== current) return;
      if (!isValidNeedsYouPage(status, page, previousCursor)) throw new Error("Needs You cursor page did not advance or did not match its contract.");
      const merged = mergeNeedsYouPage(items, page);
      setItems(merged.items);
      setNextCursor(merged.nextCursor);
      recordMotion(`${workspaceId}|${status}`, merged.items);
    } catch {
      if (generation.current === current) setFailure("more");
    } finally {
      if (paginationGate.current.finish(current)) {
        dispatchLoadState({ type: "finish-more", generation: current });
      }
    }
  };

  const hasMore = loadedWorkspace === workspaceId && loadedStatus === status && nextCursor !== null;
  const showingSavedItems = visibleItems.length > 0;
  const refreshingSavedItems = busy && showingSavedItems && failure === null;
  const failureText = failure === "more"
    ? "More Needs You items could not be loaded. Existing items remain available; retry when the local Runtime is ready."
    : showingSavedItems
      ? `Needs You could not be refreshed. Showing the last saved ${status === "OPEN" ? "open" : "resolved"} items for this Workspace.`
      : "Needs You is unavailable. The current list could not be checked, so no empty state is inferred.";

  return <div className="page-content subpage-content task-attention-page needs-you-page">
    <div className="eyebrow">WORKSPACE INBOX</div>
    <div className="task-attention-heading">
      <div><h1 ref={heading} tabIndex={-1}>Needs You</h1><p>Review open requests and blockers, or browse resolved history.</p></div>
      <button type="button" className="quiet-button" onClick={() => setRefreshToken(value => value + 1)} disabled={!operatorReady || busy}>Refresh</button>
    </div>
    <p className="task-attention-scope-note">User request responses and dismissals are saved against the current request version. Task blockers open the current Task. Approval decisions are checked against the current Workspace, target, and version before they are saved.</p>
    {selectedUserRequest?.workspaceId === workspaceId && <UserRequestDetail
      workspaceId={selectedUserRequest.workspaceId}
      requestId={selectedUserRequest.requestId}
      operatorReady={operatorReady}
      refreshToken={refreshToken}
      onResolved={() => setRefreshToken(value => value + 1)}
      onBack={() => {
        const opener = itemOpener.current;
        setSelectedUserRequest(null);
        requestAnimationFrame(() => {
          if (opener?.isConnected) opener.focus();
          else heading.current?.focus();
        });
      }}
    />}
    {selectedApproval?.workspaceId === workspaceId && <ApprovalDetail
      workspaceId={selectedApproval.workspaceId}
      approvalId={selectedApproval.approvalId}
      operatorReady={operatorReady}
      refreshToken={refreshToken}
      onBack={() => {
        const opener = itemOpener.current;
        setSelectedApproval(null);
        requestAnimationFrame(() => {
          if (opener?.isConnected) opener.focus();
          else heading.current?.focus();
        });
      }}
      onResolved={() => {
        setSelectedApproval(null);
        setSelectedUserRequest(null);
        setRefreshToken(value => value + 1);
      }}
    />}
    <NeedsYouFilter status={status} onChange={setStatus} />
    <section className="needs-you-live-permissions" aria-labelledby="needs-you-live-permissions-title">
      <div className="needs-you-live-permissions-heading">
        <div>
          <h2 id="needs-you-live-permissions-title">Live agent requests</h2>
          <p>These requests exist only while their native agent session is active.</p>
        </div>
        <button type="button" className="quiet-button" onClick={() => void refreshLivePermissions().catch(() => {})}
          disabled={!workspaceId || !operatorReady || livePermissionLoading} aria-busy={livePermissionLoading}>
          {livePermissionLoading ? "Checking…" : "Refresh live requests"}
        </button>
      </div>
      {!workspaceId ? <p className="needs-you-live-permissions-state">Select a Workspace to check for live agent requests.</p>
        : livePermissionLoading && livePermissionSnapshot?.workspaceId !== workspaceId
          ? <p className="needs-you-live-permissions-state" role="status">Checking live agent requests…</p>
          : livePermissionError
            ? <p className="needs-you-live-permissions-state" role="alert">Live agent requests could not be checked. The last list is not actionable until refreshed.</p>
            : (livePermissionSnapshot?.workspaceId !== workspaceId || livePermissionSnapshot.items.length === 0)
              ? <p className="needs-you-live-permissions-state">No live agent requests.</p>
              : <div className="needs-you-live-permissions-list">{livePermissionSnapshot.items.map(item => (
                <NativePermissionCard
                  key={item.promptToken}
                  prompt={item}
                  conversationTitle={item.conversationTitle}
                  disabled={!operatorReady || livePermissionError || livePermissionLoading || item.state !== "PENDING"}
                  refreshDisabled={!operatorReady || livePermissionLoading}
                  sendingDecision={livePermissionActions[item.promptToken] ?? null}
                  onDecision={decision => void replyToLivePermission(item, decision)}
                  onRefresh={() => void refreshLivePermissions().catch(() => {})}
                />
              ))}</div>}
    </section>
    {failure && <p className="task-attention-notice" role="status">{failureText}</p>}
    {refreshingSavedItems && <p className="task-attention-notice" role="status">Checking the latest Needs You items. Showing the last saved {status === "OPEN" ? "open" : "resolved"} items for this Workspace until refresh completes.</p>}
    {!workspaceId ? <section className="task-attention-empty"><h2>Select a Workspace</h2><p>Choose or create a Workspace before reviewing Needs You items.</p></section>
      : busy && visibleItems.length === 0 ? <p className="task-attention-empty" role="status">Loading {status === "OPEN" ? "open" : "resolved"} Needs You items…</p>
      : failure === "refresh" && visibleItems.length === 0 ? <section className="task-attention-empty"><h2>Needs You is unavailable</h2><p>Refresh when the local Runtime is available. No inbox state is inferred from an unavailable response.</p></section>
      : visibleItems.length === 0 ? <section className="task-attention-empty"><h2>{status === "OPEN" ? "No open Needs You items" : "No resolved history"}</h2><p>{status === "OPEN" ? "There are no open approvals, requests, or Task blockers in this Workspace." : "Resolved items will appear here while their source records remain available."}</p></section>
      : <ul className="task-attention-list needs-you-list">{visibleItems.map(item => <li key={item.itemId}>
        <NeedsYouItemCard
          item={item}
          entering={enteringItemIds.has(item.itemId)}
          onOpenTask={onOpenTask}
          onOpenUserRequest={(requestId, trigger) => {
            itemOpener.current = trigger;
            setSelectedUserRequest({ workspaceId, requestId });
          }}
          onOpenApproval={(approvalId, trigger) => {
            itemOpener.current = trigger;
            setSelectedApproval({ workspaceId, approvalId });
          }}
        />
      </li>)}</ul>}
    {hasMore && <NeedsYouLoadMoreButton
      loading={loadingMore}
      disabled={!operatorReady || loadingMore || busy}
      onLoadMore={() => void loadMore()}
    />}
  </div>;
}
