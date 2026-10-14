import { useEffect, useMemo, useReducer, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { taskAttentionLoadReducer } from "./task-attention-state";
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

const ATTENTION_STATUSES: readonly TaskStatus[] = ["WAITING_USER", "NEEDS_USER", "BLOCKED"];
const PAGE_SIZE = 100;

function statusLabel(status: TaskStatus): string {
  switch (status) {
    case "WAITING_USER": return "Waiting for your input";
    case "NEEDS_USER": return "Needs your decision";
    case "BLOCKED": return "Blocked";
  }
}

function displayTime(value: string): string {
  const time = Date.parse(value);
  return Number.isFinite(time) ? new Date(time).toLocaleString() : "Time unavailable";
}

/** A factual task-status view, not the full Approval/UserRequest Needs You inbox. */
export function TaskAttentionPage({
  workspaceId,
  operatorReady,
  onOpenTask,
}: {
  workspaceId: string;
  operatorReady: boolean;
  onOpenTask: (taskId: string) => void;
}) {
  const [items, setItems] = useState<TaskSummary[]>([]);
  const [pages, setPages] = useState<StatusPage[]>([]);
  const [loadedWorkspace, setLoadedWorkspace] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [loadState, dispatchLoadState] = useReducer(taskAttentionLoadReducer, { generation: 0, loadingMore: false });
  const loadingMore = loadState.loadingMore;
  const [refreshToken, setRefreshToken] = useState(0);
  const [failure, setFailure] = useState<string | null>(null);
  const generation = useRef(0);
  const paginationGate = useRef(new TaskAttentionPaginationGate());
  const visibleItems = loadedWorkspace === workspaceId ? items : [];
  const sortedItems = useMemo(() => [...visibleItems].sort((a, b) => {
    const timeOrder = Date.parse(b.updatedAt) - Date.parse(a.updatedAt);
    return Number.isFinite(timeOrder) && timeOrder !== 0 ? timeOrder : a.taskId.localeCompare(b.taskId);
  }), [visibleItems]);

  useEffect(() => {
    const current = ++generation.current;
    paginationGate.current.invalidate(current);
    dispatchLoadState({ type: "invalidate", generation: current });
    let active = true;
    setPages([]);
    setFailure(null);
    if (!workspaceId) {
      setItems([]);
      setLoadedWorkspace(null);
      setBusy(false);
      return () => { active = false; };
    }
    if (!operatorReady) {
      setBusy(false);
      setFailure("The local Runtime is unavailable. Previously loaded Tasks remain visible only for this Workspace.");
      return () => { active = false; };
    }

    setBusy(true);
    void Promise.all(ATTENTION_STATUSES.map(async status => ({
      status,
      page: await invoke<TaskPage>("list_tasks", { workspaceId, status, cursor: null, limit: PAGE_SIZE }),
    }))).then(results => {
      if (!active || generation.current !== current) return;
      const combined = new Map<string, TaskSummary>();
      for (const result of results) {
        if (!isValidStatusPage(result.status, result.page, null)) {
          throw new Error("Task status results did not include a valid advancing page cursor.");
        }
        for (const task of result.page.items) {
          if (task.status !== result.status || !task.taskId || !task.objective) {
            throw new Error("Task status results did not match the requested Workspace query.");
          }
          combined.set(task.taskId, task);
        }
      }
      setItems([...combined.values()]);
      setPages(results.map(result => ({ status: result.status, nextCursor: result.page.nextCursor })));
      setLoadedWorkspace(workspaceId);
      setFailure(null);
    }).catch(error => {
      if (!active || generation.current !== current) return;
      setFailure(typeof error === "string" ? error : error instanceof Error ? error.message : "Task attention could not be refreshed.");
    }).finally(() => {
      if (active && generation.current === current) setBusy(false);
    });
    return () => { active = false; };
  }, [workspaceId, operatorReady, refreshToken]);

  const loadMore = async () => {
    if (!operatorReady || pages.every(page => page.nextCursor === null)) return;
    const current = generation.current;
    if (!paginationGate.current.begin(current)) return;
    dispatchLoadState({ type: "start-more", generation: current });
    setFailure(null);
    try {
      const pending = pages.filter(page => page.nextCursor !== null);
      const results = await Promise.all(pending.map(async entry => ({
        status: entry.status,
        ...(await invoke<TaskPage>("list_tasks", {
          workspaceId,
          status: entry.status,
          cursor: entry.nextCursor,
          limit: PAGE_SIZE,
        }).then(page => ({ ok: true as const, page })).catch(() => ({ ok: false as const }))),
      })));
      if (generation.current !== current) return;
      const merged = mergeTaskAttentionPages(items, pages, results);
      setItems(merged.items);
      setPages(merged.pages);
      if (merged.failed) setFailure("Some Task status pages could not be loaded. Load more to retry them.");
    } catch (error) {
      if (generation.current === current) setFailure(typeof error === "string" ? error : "More Tasks could not be loaded.");
    } finally {
      if (paginationGate.current.finish(current)) {
        dispatchLoadState({ type: "finish-more", generation: current });
      }
    }
  };

  const hasMore = pages.some(page => page.nextCursor !== null);
  return <div className="page-content subpage-content task-attention-page">
    <div className="eyebrow">WORKSPACE TASKS</div>
    <div className="task-attention-heading">
      <div><h1>Needs You</h1><p>Tasks saved as waiting for input, needing a decision, or blocked.</p></div>
      <button type="button" className="quiet-button" onClick={() => setRefreshToken(value => value + 1)} disabled={!operatorReady || busy}>Refresh</button>
    </div>
    <p className="task-attention-scope-note">This view uses persisted Task status. Approval and UserRequest inbox actions are not connected here; opening a Task does not resolve its blocker.</p>
    {failure && <p className="task-attention-notice" role="status">{loadedWorkspace === workspaceId && visibleItems.length > 0 ? "Showing the last saved Task list. " : ""}{failure}</p>}
    {!workspaceId ? <section className="task-attention-empty"><h2>Select a Workspace</h2><p>Choose or create a Workspace before reviewing saved Tasks.</p></section>
      : busy && visibleItems.length === 0 ? <p className="task-attention-empty" role="status">Loading Tasks that may need attention…</p>
      : failure && visibleItems.length === 0 ? <section className="task-attention-empty"><h2>Task attention is unavailable</h2><p>Refresh when the local Runtime is available. No inbox state is inferred from an unavailable response.</p></section>
      : sortedItems.length === 0 ? <section className="task-attention-empty"><h2>No waiting or blocked Tasks</h2><p>Tasks in other states are available in Work. Approval and UserRequest actions are not part of this view yet.</p></section>
      : <ul className="task-attention-list">{sortedItems.map(task => <li key={task.taskId}>
        <button type="button" className="task-attention-row" onClick={() => onOpenTask(task.taskId)}>
          <span className={`task-attention-marker status-${task.status.toLowerCase()}`} aria-hidden="true" />
          <span className="task-attention-copy"><strong>{task.objective}</strong><span>{statusLabel(task.status)} · Last updated {displayTime(task.updatedAt)}</span></span>
          <span aria-hidden="true">→</span>
        </button>
      </li>)}</ul>}
    {hasMore && <TaskAttentionLoadMoreButton
      loading={loadingMore}
      disabled={!operatorReady || loadingMore}
      onLoadMore={() => void loadMore()}
    />}
  </div>;
}
