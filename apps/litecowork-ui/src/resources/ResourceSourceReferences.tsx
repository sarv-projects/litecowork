import { resolveResourceSourceReferences, type ExactResourceSourcePin, type ResourceSourceReference } from "./resource-source-references";
import "./resource-source-references.css";

/** Shows retrieval provenance and opens only the exact Workspace/Resource revision pin. */
export function ResourceSourceReferences({
  workspaceId,
  references,
  onOpenExactSource,
}: {
  workspaceId: string;
  references: readonly ResourceSourceReference[];
  onOpenExactSource: (pin: ExactResourceSourcePin) => void;
}) {
  const sources = resolveResourceSourceReferences(workspaceId, references);
  if (sources.length === 0) return null;

  return <section className="resource-source-references" aria-label="Source references">
    <h3>Source references</h3>
    <ul>
      {sources.map((source) => <li key={source.id}>
        <div className="resource-source-references__copy">
          <strong>{source.displayName}</strong>
          {source.kind === "pinned-reference" && source.location && <span>{source.location}</span>}
          {source.kind === "pinned-reference" && source.excerpt && <blockquote>{source.excerpt}</blockquote>}
          {source.kind === "unavailable" && <span role="status">This source cannot be verified in the selected Workspace.</span>}
        </div>
        <button
          className="quiet-button"
          type="button"
          disabled={source.kind !== "pinned-reference"}
          onClick={() => { if (source.kind === "pinned-reference") onOpenExactSource(source.pin); }}
          aria-label={source.kind === "pinned-reference"
            ? `Open ${source.displayName}${source.location ? ` at ${source.location}` : ""} from its exact revision`
            : `${source.displayName} source unavailable`}
        >Open source</button>
      </li>)}
    </ul>
    <p className="resource-source-references__note">Open a reference to inspect its exact saved revision. A newer edit does not change this pin.</p>
  </section>;
}
