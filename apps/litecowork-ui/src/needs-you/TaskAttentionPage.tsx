import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
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
  const [loadingMore, setLoadingMore] = useState(false);
  const [refreshToken, setRefreshToken] = useState(0);
  const [failure, setFailure] = useState<string | null>(null);
  const generation = useRef(0);
  const visibleItems = loadedWorkspace === workspaceId ? items : [];
  const sortedItems = useMemo(() => [...visibleItems].sort((a, b) => {
    const timeOrder = Date.parse(b.updatedAt) - Date.parse(a.updatedAt);
    return Number.isFinite(timeOrder) && timeOrder !== 0 ? timeOrder : a.taskId.localeCompare(b.taskId);
  }), [visibleItems]);

  useEffect(() => {
    const current = ++generation.current;
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
    if (!operatorReady || loadingMore || pages.every(page => page.nextCursor === null)) return;
    const current = generation.current;
    setLoadingMore(true);
    setFailure(null);
    try {
      const pending = pages.filter(page => page.nextCursor !== null);
      const results = await Promise.all(pending.map(async entry => ({
        status: entry.status,
        page: await invoke<TaskPage>("list_tasks", {
          workspaceId,
          status: entry.status,
          cursor: entry.nextCursor,
          limit: PAGE_SIZE,
        }),
      })));
      if (generation.current !== current) return;
      const seen = new Set(items.map(item => item.taskId));
      const nextItems = results.flatMap(result => result.page.items.map(task => {
        if (task.status !== result.status || !task.taskId || !task.objective) {
          throw new Error("Task status results did not match the requested Workspace query.");
        }
        return task;
      })).filter(task => {
        if (seen.has(task.taskId)) return false;
        seen.add(task.taskId);
        return true;
      });
      setItems(currentItems => [...currentItems, ...nextItems]);
      setPages(currentPages => currentPages.map(currentPage => {
        const updated = results.find(result => result.status === currentPage.status);
        return updated ? { status: currentPage.status, nextCursor: updated.page.nextCursor } : currentPage;
      }));
    } catch (error) {
      if (generation.current === current) setFailure(typeof error === "string" ? error : "More Tasks could not be loaded.");
    } finally {
      if (generation.current === current) setLoadingMore(false);
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
    {hasMore && <button type="button" className="quiet-button task-attention-more" onClick={() => void loadMore()} disabled={!operatorReady || loadingMore}>{loadingMore ? "Loading more…" : "Load more"}</button>}
  </div>;
}
