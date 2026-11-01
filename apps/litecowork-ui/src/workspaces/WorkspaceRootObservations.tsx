import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import {
  isWorkspaceRootObservationPage,
  mergeWorkspaceRootObservationPage,
  normalizeObservationQuery,
  OBSERVATION_PAGE_LIMIT,
  type WorkspaceRootObservation,
  type WorkspaceRootObservationRequest,
} from "./workspace-root-observations";
import "./workspace-root-observations.css";

export type {
  WorkspaceRootObservation,
  WorkspaceRootObservationPage,
  WorkspaceRootObservationRequest,
} from "./workspace-root-observations";
export { isWorkspaceRootObservationPage } from "./workspace-root-observations";

type Root = {
  workspaceId: string;
  workspaceRootId: string;
  displayName: string;
  version: number;
  status: string;
};

export function WorkspaceRootObservations({
  root,
  loadPage,
  openResource,
  scanUnsupported = false,
}: {
  root: Root;
  loadPage: (request: WorkspaceRootObservationRequest) => Promise<unknown>;
  openResource?: (resourceId: string, displayName: string, sizeBytes: number) => void;
  scanUnsupported?: boolean;
}) {
  const [expanded, setExpanded] = useState(false);
  const [query, setQuery] = useState("");
  const [freshness, setFreshness] = useState<WorkspaceRootObservationRequest["freshness"]>("");
  const [submittedQuery, setSubmittedQuery] = useState("");
  const [submittedFreshness, setSubmittedFreshness] = useState<WorkspaceRootObservationRequest["freshness"]>("");
  const [items, setItems] = useState<WorkspaceRootObservation[]>([]);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);
  const epoch = useRef(0);
  const inFlight = useRef(false);

  useLayoutEffect(() => {
    epoch.current += 1;
    inFlight.current = false;
    setItems([]);
    setNextCursor(null);
    setError(null);
    setLoaded(false);
    setBusy(false);
  }, [root.workspaceId, root.workspaceRootId, root.version]);

  const requestPage = useCallback(async (cursor: string | null, append: boolean) => {
    if (inFlight.current || root.status !== "ACTIVE") return;
    const query = normalizeObservationQuery(submittedQuery);
    if (query === null) {
      setError("Search text is not available as a file-name query. Shorten it and try again.");
      setLoaded(true);
      return;
    }
    const requestEpoch = epoch.current;
    inFlight.current = true;
    setBusy(true);
    setError(null);
    try {
      const response = await loadPage({
        workspaceId: root.workspaceId,
        workspaceRootId: root.workspaceRootId,
        rootVersion: root.version,
        query,
        freshness: submittedFreshness,
        cursor,
        limit: OBSERVATION_PAGE_LIMIT,
      });
      if (requestEpoch !== epoch.current) return;
      if (!isWorkspaceRootObservationPage(response, root.workspaceId, root.workspaceRootId, cursor)) {
        throw new Error("The folder returned an unexpected observation list.");
      }
      setItems((current) => mergeWorkspaceRootObservationPage(current, response, !append).items);
      setNextCursor(response.next_cursor);
      setLoaded(true);
    } catch (cause) {
      if (requestEpoch !== epoch.current) return;
      setError(cause instanceof Error ? cause.message : "Observed files could not be loaded.");
      setLoaded(true);
    } finally {
      if (requestEpoch === epoch.current) {
        inFlight.current = false;
        setBusy(false);
      }
    }
  }, [loadPage, root, submittedFreshness, submittedQuery]);

  useEffect(() => {
    if (expanded && !loaded && !busy && root.status === "ACTIVE") void requestPage(null, false);
  }, [expanded, loaded, busy, root.status, requestPage]);

  const toggle = () => {
    const opening = !expanded;
    setExpanded(opening);
  };

  return <section className="workspace-root-observations" aria-label={`Observed files in ${root.displayName}`}>
    <button className="workspace-root-observations-toggle" type="button" aria-expanded={expanded} onClick={toggle}>
      <span aria-hidden="true">{expanded ? "▾" : "▸"}</span> Browse observed files
    </button>
    {expanded && <div className="workspace-root-observations-content">
      <p className="workspace-root-observations-note">
        Names and file metadata only. A linked Resource may have no content revision yet; this scan does not read file contents.
      </p>
      {scanUnsupported && <p className="workspace-root-observations-paused" role="status">Metadata scanning is not available on this platform, so this observed-file list cannot be refreshed here.</p>}
      {root.status !== "ACTIVE" && <p className="workspace-root-observations-paused" role="status">This folder is paused. Resume it to refresh observed files.</p>}
      <form className="workspace-root-observations-filters" onSubmit={(event) => {
        event.preventDefault();
        epoch.current += 1;
        inFlight.current = false;
        setItems([]);
        setNextCursor(null);
        setLoaded(false);
        setError(null);
        setBusy(false);
        setSubmittedQuery(query);
        setSubmittedFreshness(freshness);
      }}>
        <label>
          <span>Find by file name</span>
          <input value={query} maxLength={256} onChange={(event) => setQuery(event.currentTarget.value)} />
        </label>
        <label>
          <span>Freshness</span>
          <select value={freshness} onChange={(event) => setFreshness(event.currentTarget.value as typeof freshness)}>
            <option value="">Any observed status</option>
            <option value="CURRENT">Current</option>
            <option value="STALE">Possibly changed</option>
            <option value="UNKNOWN">Unknown</option>
          </select>
        </label>
        <button className="quiet-button" type="submit" disabled={busy || root.status !== "ACTIVE"}>Search</button>
      </form>
      {busy && items.length === 0 && <p role="status" aria-live="polite">Loading observed files…</p>}
      {busy && items.length > 0 && <p role="status" aria-live="polite">Refreshing observed files…</p>}
      {error && <p className="inline-error" role="alert">{error}</p>}
      {loaded && !busy && !error && items.length === 0 && <p role="status">
        {scanUnsupported
          ? "No matching metadata observations are currently available. Metadata scanning is unsupported on this platform."
          : "No matching metadata observations are currently available. Run Scan folder to refresh this list."}
      </p>}
      {items.length > 0 && <ul className="workspace-root-observation-list">
        {items.map((item) => <li key={item.observation_id}>
          <span className="workspace-root-observation-file-icon" aria-hidden="true">▤</span>
          <span className="workspace-root-observation-name">{item.display_name}</span>
          <span className={`workspace-root-observation-freshness freshness-${item.freshness.toLowerCase()}`}>
            {item.freshness === "CURRENT" ? "Present in the last complete scan" : item.freshness === "STALE" ? "Not seen on the last complete scan" : "Freshness unknown"}
          </span>
          <span className="workspace-root-observation-size">{formatBytes(item.size_bytes)}</span>
          {item.resource_id && openResource && <button className="quiet-button" type="button" onClick={() => openResource(item.resource_id!, item.display_name, item.size_bytes)}>
            Open Resource details
          </button>}
        </li>)}
      </ul>}
      {nextCursor && <button className="quiet-button" type="button" disabled={busy} onClick={() => void requestPage(nextCursor, true)}>
        {busy ? "Loading…" : "Load more observed files"}
      </button>}
    </div>}
  </section>;
}

function formatBytes(value: number): string {
  if (value < 1024) return `${value} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let scaled = value / 1024;
  for (let index = 0; index < units.length; index += 1) {
    if (scaled < 1024 || index === units.length - 1) {
      return `${scaled.toLocaleString(undefined, { maximumFractionDigits: 1 })} ${units[index]}`;
    }
    scaled /= 1024;
  }
  return `${value} B`;
}
