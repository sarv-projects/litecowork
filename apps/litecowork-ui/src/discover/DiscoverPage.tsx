import { useEffect, useState } from "react";
import type { DiscoverApi, DiscoverEntryKind, DiscoverItem } from "./discover-api.ts";
import { discoverCatalogView, discoverRowView } from "./discover-state.ts";
import "./discover-page.css";

type Props = {
  api: DiscoverApi;
};

type LoadState =
  | { kind: "loading" }
  | { kind: "ready"; rows: DiscoverItem[]; observations: DiscoverItem[]; providerMessage: string; packageServiceQualified: boolean }
  | { kind: "empty"; providerMessage: string; packageServiceQualified: boolean }
  | { kind: "error"; message: string };

const KIND_FILTERS: { value: DiscoverEntryKind | null; label: string }[] = [
  { value: null, label: "All" },
  { value: "APP", label: "Apps" },
  { value: "SKILL", label: "Skills" },
  { value: "CAPABILITY", label: "Capabilities" },
  { value: "AGENT", label: "Agents" },
];

export function DiscoverPage({ api }: Props) {
  const [state, setState] = useState<LoadState>({ kind: "loading" });
  const [query, setQuery] = useState("");
  const [kind, setKind] = useState<DiscoverEntryKind | null>(null);

  useEffect(() => {
    const controller = new AbortController();
    setState({ kind: "loading" });
    api
      .search(query.trim() === "" ? null : query.trim(), kind, undefined, controller.signal)
      .then(page => {
        if (controller.signal.aborted) return;
        const view = discoverCatalogView(page);
        setState(
          view.empty
            ? { kind: "empty", providerMessage: view.providerMessage, packageServiceQualified: view.packageServiceQualified }
            : {
                kind: "ready",
                rows: page.items,
                observations: page.runtime_capabilities,
                providerMessage: view.providerMessage,
                packageServiceQualified: view.packageServiceQualified,
              },
        );
      })
      .catch(error => {
        if (controller.signal.aborted) return;
        setState({ kind: "error", message: error instanceof Error ? error.message : "The capability catalog could not be read." });
      });
    return () => controller.abort();
  }, [api, query, kind]);

  return (
    <section className="discover-page" aria-label="Discover">
      <header className="discover-page__header">
        <h2>Discover</h2>
        <p className="discover-page__hint">
          A catalog row is a projection. It never means a package is installed, connected, or authorized.
        </p>
        <div className="discover-page__filters">
          <label>
            <span>Search</span>
            <input
              type="search"
              value={query}
              maxLength={512}
              placeholder="Search capabilities"
              onChange={event => setQuery(event.target.value)}
            />
          </label>
          <label>
            <span>Kind</span>
            <select value={kind ?? ""} onChange={event => setKind(event.target.value === "" ? null : (event.target.value as DiscoverEntryKind))}>
              {KIND_FILTERS.map(filter => (
                <option key={filter.value ?? "all"} value={filter.value ?? ""}>
                  {filter.label}
                </option>
              ))}
            </select>
          </label>
        </div>
      </header>

      {state.kind === "loading" && <p className="discover-page__status">Loading the capability catalog…</p>}
      {state.kind === "error" && <p className="discover-page__status discover-page__status--error">{state.message}</p>}
      {state.kind === "empty" && (
        <div className="discover-page__empty">
          <p>No capability is available in this Workspace yet.</p>
          <p className="discover-page__hint">{state.providerMessage}</p>
        </div>
      )}

      {state.kind === "ready" && (
        <>
          <p className="discover-page__provider">{state.providerMessage}</p>
          <ul className="discover-page__list">
            {state.rows.map(item => {
              const row = discoverRowView(item);
              return (
                <li key={row.itemRef} className={`discover-row discover-row--${row.tone}`}>
                  <div className="discover-row__body">
                    <span className="discover-row__kind">{row.kind}</span>
                    <span className="discover-row__title">{row.title}</span>
                    <span className="discover-row__summary">{row.summary}</span>
                  </div>
                  <div className="discover-row__state">
                    <span className="discover-row__availability">{row.availabilityLabel}</span>
                    <span className="discover-row__reason">{row.availabilityReason}</span>
                  </div>
                  <button type="button" className="discover-row__action" disabled={row.disabled}>
                    {row.requiresUserAction ? "Connect" : "Use"}
                  </button>
                </li>
              );
            })}
          </ul>

          {state.observations.length > 0 && (
            <>
              <h3 className="discover-page__observations-title">Observed by this Runtime</h3>
              <p className="discover-page__hint">These are observations of running providers, not installable catalog rows.</p>
              <ul className="discover-page__list">
                {state.observations.map(item => {
                  const row = discoverRowView(item);
                  return (
                    <li key={row.itemRef} className={`discover-row discover-row--${row.tone}`}>
                      <div className="discover-row__body">
                        <span className="discover-row__kind">{row.kind}</span>
                        <span className="discover-row__title">{row.title}</span>
                      </div>
                      <div className="discover-row__state">
                        <span className="discover-row__availability">{row.availabilityLabel}</span>
                      </div>
                    </li>
                  );
                })}
              </ul>
            </>
          )}
        </>
      )}
    </section>
  );
}
