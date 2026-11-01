import { useEffect, useRef, useState } from "react";
import type {
  DiscoverApi,
  DiscoverItem,
  DiscoverItemDetail,
} from "./discover-api.ts";
import { connectActionOutcome, discoverCatalogView, discoverRowView } from "./discover-state.ts";
import "./discover-page.css";

type Props = {
  api: DiscoverApi;
};

type LoadState =
  | { kind: "loading" }
  | { kind: "ready"; rows: DiscoverItem[]; observations: DiscoverItem[]; providerMessage: string; packageServiceQualified: boolean; nextCursor: string | null }
  | { kind: "error"; message: string };

type DiscoverSection = "explore" | "installed" | "managed" | "connections";

type ItemDetailState =
  | { kind: "closed" }
  | { kind: "loading" }
  | { kind: "ready"; detail: DiscoverItemDetail }
  | { kind: "error" };

const DISCOVER_PROVIDER_STATUS_ID = "discover-package-service-status";

type DiscoverCatalogItemRowProps = {
  api: DiscoverApi;
  item: DiscoverItem;
  providerStatusId: string;
};

export function DiscoverCatalogItemRow({ api, item, providerStatusId }: DiscoverCatalogItemRowProps) {
  const row = discoverRowView(item);
  const action = connectActionOutcome(row);
  const actionReasonId = "discover-action-reason-" + encodeURIComponent(row.itemRef);
  const detailId = "discover-capabilities-" + encodeURIComponent(row.itemRef);
  const [detailState, setDetailState] = useState<ItemDetailState>({ kind: "closed" });
  const detailOpen = detailState.kind !== "closed";

  useEffect(() => {
    if (detailState.kind !== "loading") return undefined;

    const controller = new AbortController();
    void api.item(row.itemRef, controller.signal)
      .then(detail => {
        if (!controller.signal.aborted) setDetailState({ kind: "ready", detail });
      })
      .catch(() => {
        if (!controller.signal.aborted) setDetailState({ kind: "error" });
      });

    return () => controller.abort();
  }, [api, row.itemRef, detailState.kind]);

  function toggleDetails() {
    setDetailState(current => current.kind === "closed" ? { kind: "loading" } : { kind: "closed" });
  }

  return (
    <li className={"discover-row discover-row--" + row.tone}>
      <div className="discover-row__body">
        <span className="discover-row__kind">{row.kind}</span>
        <span className="discover-row__title">{row.title}</span>
        <span className="discover-row__summary">{row.summary}</span>
      </div>
      <div className="discover-row__state">
        <span className="discover-row__availability">{row.availabilityLabel}</span>
        <span className="discover-row__reason">{row.availabilityReason}</span>
      </div>
      <div className="discover-row__actions">
        <button
          type="button"
          className="discover-row__action"
          disabled={!action.allowed}
          aria-describedby={providerStatusId + " " + actionReasonId}
        >{row.requiresUserAction ? "Connect" : row.source === "LITECOWORK_LOCAL" ? "Open" : "Use"}</button>
        <small id={actionReasonId} className="discover-row__action-reason">{action.message}</small>
        <button
          type="button"
          className="discover-row__details-toggle"
          aria-expanded={detailOpen}
          aria-controls={detailId}
          onClick={toggleDetails}
        >{detailOpen ? "Close details" : "View capabilities"}</button>
      </div>
      <div
        id={detailId}
        className="discover-row__details"
        hidden={!detailOpen}
        aria-live="polite"
      >
        {detailState.kind === "loading" && <p>Loading capability details…</p>}
        {detailState.kind === "error" && (
          <p className="discover-row__details-error">Details are unavailable. Close and reopen to retry.</p>
        )}
        {detailState.kind === "ready" && (
          <>
            {detailState.detail.item.description && <p>{detailState.detail.item.description}</p>}
            {detailState.detail.capabilities.length > 0 ? (
              <ul className="discover-row__capability-list">
                {detailState.detail.capabilities.map(capability => (
                  <li key={capability.capability_id}>
                    <strong>{capability.display_name}</strong>
                    <span>Operation: {capability.operation_class}</span>
                    <span>Required scopes: {capability.required_scopes.length > 0 ? capability.required_scopes.join(", ") : "None listed"}</span>
                  </li>
                ))}
              </ul>
            ) : (
              <p>No capability summary is listed for this item.</p>
            )}
            {detailState.detail.provider_note && <p>{detailState.detail.provider_note}</p>}
            <p className="discover-row__review-note">Review only. No capability is granted or installed here.</p>
          </>
        )}
      </div>
    </li>
  );
}

export function DiscoverPage({ api }: Props) {
  const [state, setState] = useState<LoadState>({ kind: "loading" });
  const [query, setQuery] = useState("");
  const [settledQuery, setSettledQuery] = useState("");
  const [section, setSection] = useState<DiscoverSection>("explore");
  const [pageBusy, setPageBusy] = useState(false);
  const [pageError, setPageError] = useState<string | null>(null);
  const [retryNonce, setRetryNonce] = useState(0);
  const generation = useRef(0);
  const pageController = useRef<AbortController | null>(null);

  useEffect(() => {
    const timer = window.setTimeout(() => setSettledQuery(query.trim()), 250);
    return () => window.clearTimeout(timer);
  }, [query]);

  useEffect(() => {
    const requestGeneration = ++generation.current;
    pageController.current?.abort();
    pageController.current = null;
    setPageBusy(false);
    setPageError(null);
    setState({ kind: "loading" });

    const controller = new AbortController();
    api.search(settledQuery || null, null, undefined, controller.signal)
      .then(page => {
        if (controller.signal.aborted || requestGeneration !== generation.current) return;
        const view = discoverCatalogView(page);
        setState({
          kind: "ready",
          rows: page.items,
          observations: page.runtime_capabilities,
          providerMessage: view.providerMessage,
          packageServiceQualified: view.packageServiceQualified,
          nextCursor: page.next_cursor,
        });
      })
      .catch(() => {
        if (controller.signal.aborted || requestGeneration !== generation.current) return;
        setState({ kind: "error", message: "The Workspace catalog could not be read. Try again." });
      });

    return () => {
      controller.abort();
      pageController.current?.abort();
      if (generation.current === requestGeneration) generation.current += 1;
    };
  }, [api, settledQuery, retryNonce]);

  async function loadMore() {
    if (state.kind !== "ready" || state.nextCursor === null || pageController.current !== null) return;
    const requestGeneration = generation.current;
    const cursor = state.nextCursor;
    const controller = new AbortController();
    pageController.current = controller;
    setPageBusy(true);
    setPageError(null);

    try {
      const page = await api.search(settledQuery || null, null, cursor, controller.signal);
      if (controller.signal.aborted || requestGeneration !== generation.current) return;
      if (page.next_cursor === cursor) {
        setState(current => current.kind === "ready" ? { ...current, nextCursor: null } : current);
        setPageError("The catalog did not advance to another page. Change the search to try again.");
        return;
      }
      setState(current => {
        if (current.kind !== "ready") return current;
        const seen = new Set(current.rows.map(item => item.item_ref));
        const rows = [...current.rows];
        for (const item of page.items) {
          if (seen.has(item.item_ref)) continue;
          seen.add(item.item_ref);
          rows.push(item);
        }
        return { ...current, rows, nextCursor: page.next_cursor };
      });
    } catch {
      if (!controller.signal.aborted && requestGeneration === generation.current) {
        setPageError("More results could not be loaded. You can retry.");
      }
    } finally {
      if (pageController.current === controller) {
        pageController.current = null;
        setPageBusy(false);
      }
    }
  }

  const tabs: { id: DiscoverSection; label: string }[] = [
    { id: "explore", label: "Explore" },
    { id: "installed", label: "Installed / native" },
    { id: "managed", label: "Managed" },
    { id: "connections", label: "Connections" },
  ];
  const observedRows = state.kind === "ready"
    ? state.observations.filter(item => {
        const needle = settledQuery.toLowerCase();
        return needle === "" || [item.display_name, item.summary, item.item_ref].some(value => value.toLowerCase().includes(needle));
      })
    : [];

  return (
    <section className="discover-page" aria-label="Discover">
      <header className="discover-page__header">
        <h2>Discover</h2>
        <p className="discover-page__hint">
          Catalog, host, and connection state stay separate. A listing never grants access or proves an installation.
        </p>
        <nav className="discover-page__tabs" aria-label="Discover sections">
          {tabs.map(tab => (
            <button
              key={tab.id}
              type="button"
              className="discover-page__tab"
              aria-pressed={section === tab.id}
              onClick={() => setSection(tab.id)}
            >{tab.label}</button>
          ))}
        </nav>
        {(section === "explore" || section === "installed") && (
          <label className="discover-page__search">
            <span>Search {section === "installed" ? "Runtime observations" : "Workspace skills"}</span>
            <input
              type="search"
              value={query}
              maxLength={512}
              placeholder={section === "installed" ? "Search observed hosts" : "Search Workspace skills"}
              onChange={event => setQuery(event.target.value)}
            />
          </label>
        )}
      </header>

      {state.kind === "loading" && <p className="discover-page__status" role="status">Loading Workspace and Runtime inventory…</p>}
      {state.kind === "error" && (
        <div className="discover-page__status-card" role="alert">
          <p>{state.message}</p>
          <button type="button" onClick={() => setRetryNonce(current => current + 1)}>Retry</button>
        </div>
      )}

      {state.kind === "ready" && (
        <section className="discover-page__panel" aria-label={tabs.find(tab => tab.id === section)?.label}>
          {section === "explore" && (
            <>
              <div className="discover-page__section-heading">
                <div>
                  <h3>Workspace-local skills</h3>
                  <p>These records come from this Workspace. Public package discovery is not available in this build.</p>
                </div>
                <span className="discover-page__state-pill">{state.packageServiceQualified ? "Package service ready" : "Package catalog unavailable"}</span>
              </div>
              <p className="discover-page__provider" id={DISCOVER_PROVIDER_STATUS_ID} role="note">{state.providerMessage}</p>
              {state.rows.length === 0 ? (
                <p className="discover-page__empty">No Workspace-local skills match this search.</p>
              ) : (
                <ul className="discover-page__list">
                  {state.rows.map(item => (
                    <DiscoverCatalogItemRow
                      key={item.item_ref}
                      api={api}
                      item={item}
                      providerStatusId={DISCOVER_PROVIDER_STATUS_ID}
                    />
                  ))}
                </ul>
              )}
              {pageError && <p className="discover-page__status discover-page__status--error" role="status">{pageError}</p>}
              {state.nextCursor && (
                <button type="button" className="discover-page__more" disabled={pageBusy} onClick={() => void loadMore()}>
                  {pageBusy ? "Loading more…" : pageError ? "Retry loading more" : "Load more skills"}
                </button>
              )}
            </>
          )}

          {section === "installed" && (
            <>
              <div className="discover-page__section-heading">
                <div>
                  <h3>Observed on this Runtime</h3>
                  <p>Up to 50 capability hosts reported by this Runtime appear here. This is not a complete native-app inventory.</p>
                </div>
                <span className="discover-page__state-pill">Runtime observation</span>
              </div>
              {observedRows.length === 0 ? (
                <p className="discover-page__empty">
                  {settledQuery
                    ? "No reported Runtime host matched this search."
                    : "This response reports no Runtime capability hosts. It does not establish that no native installations exist."}
                </p>
              ) : (
                <ul className="discover-page__list">
                  {observedRows.map(item => {
                    const row = discoverRowView(item);
                    return (
                      <li key={row.itemRef} className={"discover-row discover-row--" + row.tone}>
                        <div className="discover-row__body">
                          <span className="discover-row__kind">{row.kind} · runtime host</span>
                          <span className="discover-row__title">{row.title}</span>
                          <span className="discover-row__summary">{row.summary}</span>
                        </div>
                        <div className="discover-row__state">
                          <span className="discover-row__availability">{row.availabilityLabel}</span>
                          <span className="discover-row__reason">{row.availabilityReason}</span>
                        </div>
                      </li>
                    );
                  })}
                </ul>
              )}
            </>
          )}

          {section === "managed" && (
            <div className="discover-page__unavailable">
              <span className="discover-page__state-pill">Not available</span>
              <h3>Package management is not connected</h3>
              <p>The LiteSPM ownership ledger and install, adopt, repair, update, and unmanage flows are not qualified. Workspace-local skills and Runtime observations are not treated as LiteSPM-managed packages.</p>
              {state.providerMessage && <p className="discover-page__provider">{state.providerMessage}</p>}
            </div>
          )}

          {section === "connections" && (
            <div className="discover-page__unavailable">
              <span className="discover-page__state-pill">Not available</span>
              <h3>Connections cannot be read here</h3>
              <p>This read-only catalog route does not expose account connections or sign-in state. No connected account is inferred from a package listing or Runtime observation.</p>
            </div>
          )}
        </section>
      )}
    </section>
  );
}