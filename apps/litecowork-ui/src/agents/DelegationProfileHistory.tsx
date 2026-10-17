import { useState } from "react";
import type { DelegationProfileRevisionView } from "../coworkers/delegation-profile-api";
import type { desktopDelegationProfileCatalogApi } from "../coworkers/desktop-delegation-profile-api";
import "./delegation-profile-history.css";

type Api = ReturnType<typeof desktopDelegationProfileCatalogApi>;

export function DelegationProfileHistory({
  api,
  profileId,
  currentRevision,
}: {
  api: Api;
  profileId: string;
  currentRevision: number;
}) {
  const [open, setOpen] = useState(false);
  const [loading, setLoading] = useState(false);
  const [revisions, setRevisions] = useState<DelegationProfileRevisionView[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function toggle() {
    if (open) {
      setOpen(false);
      return;
    }
    setOpen(true);
    if (revisions || loading) return;
    setLoading(true);
    setError(null);
    try {
      setRevisions(await api.listRevisions(profileId));
    } catch {
      setError("Revision history could not be loaded. Check the local Runtime and try again.");
    } finally {
      setLoading(false);
    }
  }

  return <div className="delegation-history">
    <button
      type="button"
      className="agent-catalog__secondary"
      aria-expanded={open}
      onClick={() => void toggle()}
    >{open ? "Hide revision history" : "Revision history"}</button>
    {open && <section className="delegation-history__panel" aria-label="Worker profile revision history">
      {loading && <p role="status">Loading saved revisions…</p>}
      {error && <p role="alert">{error}</p>}
      {revisions && revisions.length === 0 && <p>No revisions are available.</p>}
      {revisions && revisions.length > 0 && <ol>
        {revisions.map(revision => <li key={revision.revision}>
          <div className="delegation-history__heading">
            <strong>Revision {revision.revision}{revision.revision === currentRevision ? " · Current" : ""}</strong>
            <time dateTime={revision.created_at}>{revision.created_at}</time>
          </div>
          <strong>{revision.name}</strong>
          <p>{revision.routing_description}</p>
          {revision.instructions && <details>
            <summary>Worker guidance</summary>
            <p>{revision.instructions}</p>
          </details>}
        </li>)}
      </ol>}
    </section>}
  </div>;
}
