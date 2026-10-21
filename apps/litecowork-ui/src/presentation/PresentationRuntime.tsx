import type { ReactNode } from "react";
import {
  projectPresentationEntries,
  type PresentationItem,
  type PresentationKind,
  type PresentationSourceRef,
  type RejectedPresentationItem,
} from "./presentation-types";
import "./presentation-runtime.css";

export type RendererContext = {
  openTask?: (taskId: string) => void;
  openArtifact?: (artifactId: string, version: number) => void;
  openSource?: (source: PresentationSourceRef) => void;
};

export type PresentationRenderer = (item: PresentationItem, context: RendererContext) => ReactNode;
export type RendererRegistry = ReadonlyMap<PresentationKind, PresentationRenderer>;

function renderText(item: PresentationItem): ReactNode {
  if (item.kind === "TEXT" || item.kind === "CITATION") return <p className="presentation-prose">{item.payload.text}</p>;
  return null;
}

/** Domain enum values are useful to inspectors, but the ordinary presentation surface
 * uses the same plain-language vocabulary as Work. Unknown future states fail closed. */
function taskStatusLabel(status: string): string {
  const labels: Record<string, string> = {
    READY: "Ready",
    RUNNING: "Working",
    WAITING_USER: "Waiting for you",
    BLOCKED: "Blocked",
    VERIFYING: "Checking the result",
    NEEDS_USER: "Needs your decision",
    INCOMPLETE: "Not finished",
    PAUSE_REQUESTED: "Pausing safely",
    PAUSED: "Paused",
    COMPLETED: "Completed",
    FAILED: "Failed",
    CANCEL_REQUESTED: "Stopping",
    CANCELLED: "Cancelled",
  };
  return labels[status] ?? "Status unavailable";
}

export const builtinRendererRegistry: RendererRegistry = new Map<PresentationKind, PresentationRenderer>([
  ["TEXT", renderText],
  ["CITATION", (item) => item.kind === "CITATION" ? <blockquote className="presentation-citation"><p>{item.payload.text}</p><cite>{item.payload.source_label}</cite></blockquote> : null],
  ["ACTIVITY", (item) => item.kind === "ACTIVITY" ? <div className="presentation-activity"><span className="presentation-status-dot" aria-hidden="true" /><div><strong>{item.label ?? item.payload.summary}</strong>{item.payload.detail && <p>{item.payload.detail}</p>}</div></div> : null],
  ["TASK_CARD", (item, ctx) => item.kind === "TASK_CARD" ? <article className="presentation-card"><header><strong>{item.payload.objective}</strong><span>{taskStatusLabel(item.payload.status)}</span></header>{item.payload.step_summary && <p>{item.payload.step_summary}</p>}{ctx.openTask && <button type="button" onClick={() => ctx.openTask?.(item.payload.task_id)}>Open work</button>}</article> : null],
  ["USER_REQUEST", (item) => item.kind === "USER_REQUEST" ? <article className="presentation-card"><h3>{item.payload.title}</h3><p>{item.payload.message}</p><small>Open Needs You to respond.</small></article> : null],
  ["APPROVAL", (item) => item.kind === "APPROVAL" ? <article className="presentation-card"><h3>{item.payload.title}</h3><p>{item.payload.summary}</p><small>Review this in Needs You.</small></article> : null],
  ["ARTIFACT", (item, ctx) => item.kind === "ARTIFACT" ? <article className="presentation-card"><div><strong>{item.payload.display_name}</strong><p>{item.payload.artifact_kind} · version {item.payload.version}{item.payload.verification_status ? ` · ${item.payload.verification_status}` : ""}</p></div>{ctx.openArtifact && <button type="button" onClick={() => ctx.openArtifact?.(item.payload.artifact_id, item.payload.version)}>Open artifact</button>}</article> : null],
  ["CODE", (item) => item.kind === "CODE" ? <pre className="presentation-code"><code>{item.payload.text}</code></pre> : null],
  ["DIFF", (item) => item.kind === "DIFF" ? <details className="presentation-details"><summary>Changes in {item.payload.file_count} files</summary><pre className="presentation-code"><code>{item.payload.text}</code></pre></details> : null],
  ["TABLE", (item) => item.kind === "TABLE" ? <div className="presentation-table-wrap"><table><thead><tr>{item.payload.columns.map((column, i) => <th key={`${i}:${column}`}>{column}</th>)}</tr></thead><tbody>{item.payload.rows.map((row, ri) => <tr key={ri}>{row.map((cell, ci) => <td key={ci}>{cell}</td>)}</tr>)}</tbody></table></div> : null],
  ["CHART", (item) => item.kind === "CHART" ? <figure className="presentation-card"><figcaption><strong>{item.payload.title}</strong><p>{item.payload.alt_text}</p></figcaption><ol>{item.payload.values.map((value, i) => <li key={`${item.payload.labels[i]}:${i}`}>{item.payload.labels[i]}: {value}</li>)}</ol></figure> : null],
  ["IMAGE", (item) => item.kind === "IMAGE" ? <figure className="presentation-card"><div className="presentation-image-fallback" role="img" aria-label={item.payload.alt_text}>Image preview is available in the owning artifact or resource.</div><figcaption>{item.payload.alt_text}</figcaption></figure> : null],
  ["BROWSER", (item) => item.kind === "BROWSER" ? <article className="presentation-card"><strong>{item.payload.title}</strong><p>{item.payload.state}</p><small>Browser controls are available from the owning Task.</small></article> : null],
  ["TERMINAL", (item) => item.kind === "TERMINAL" ? <details className="presentation-details"><summary>{item.payload.title}</summary><pre className="presentation-code"><code>{item.payload.text}</code></pre></details> : null],
  ["MCP_APP", (item) => item.kind === "MCP_APP" ? <article className="presentation-card"><strong>{item.payload.display_name}</strong><p>Interactive app content is unavailable in this renderer.</p></article> : null],
  ["ERROR", (item) => item.kind === "ERROR" ? <article className="presentation-error"><strong>{item.payload.code}</strong><p>{item.payload.message}</p>{item.payload.recovery_hint && <small>{item.payload.recovery_hint}</small>}</article> : null],
  ["UNSUPPORTED", (item) => item.kind === "UNSUPPORTED" ? <div className="presentation-fallback" role="note"><strong>{item.label ?? "Unsupported content"}</strong><p>This saved item cannot be displayed by this version of LiteCowork. The saved response text is unchanged.</p></div> : null],
]);

function sourceLabel(source: PresentationSourceRef): string {
  return source.kind.replaceAll("_", " ");
}

/** Known source identities stay reviewable when the item around them cannot be rendered. */
function SourceDisclosure({ sources, context }: { sources: readonly PresentationSourceRef[]; context: RendererContext }) {
  if (sources.length === 0) return null;
  return <details className="presentation-sources">
    <summary>Sources ({sources.length})</summary>
    <ul>{sources.map((source, i) => <li key={`${source.kind}:${source.id}:${i}`}>
      {context.openSource
        ? <button type="button" onClick={() => context.openSource?.(source)}>{sourceLabel(source)} · {source.id}</button>
        : <span>{sourceLabel(source)} · {source.id}</span>}
    </li>)}</ul>
  </details>;
}

/** A saved item that failed validation is named and explained, never rendered or dropped. */
function RejectedItem({ rejected }: { rejected: RejectedPresentationItem }) {
  const identity = rejected.reportedKind
    ? `${rejected.reportedKind}${rejected.reportedVersion === null ? "" : ` (version ${rejected.reportedVersion})`} content`
    : "This saved item";
  return <div className="presentation-fallback" role="note">
    <strong>Content not shown</strong>
    <p>{identity} could not be displayed safely, so nothing from it was rendered. The rest of the response is unchanged.</p>
  </div>;
}

/** Resolve a validated item to its registered renderer, or an explicit safe fallback. */
function renderEntry(item: PresentationItem, renderers: RendererRegistry, context: RendererContext): ReactNode {
  const renderer = item.payload_version === 1 ? renderers.get(item.kind) : undefined;
  return renderer
    ? renderer(item, context)
    : <div className="presentation-fallback">
      <strong>{item.label ?? item.kind.replaceAll("_", " ")}</strong>
      <p>No compatible renderer is enabled. Its source remains available for review.</p>
    </div>;
}

export function PresentationRuntime({ items, renderers = builtinRendererRegistry, context = {}, ariaLabel = "Conversation content" }: {
  items: readonly unknown[];
  renderers?: RendererRegistry;
  context?: RendererContext;
  ariaLabel?: string;
}) {
  const projection = projectPresentationEntries(items);
  return <section className="presentation-runtime" aria-label={ariaLabel}>
    {projection.entries.map((entry) => entry.entry === "REJECTED"
      ? <article className="presentation-item" key={entry.key} data-freshness="UNKNOWN">
        <RejectedItem rejected={entry.rejected} />
        <SourceDisclosure sources={entry.rejected.sourceRefs} context={context} />
      </article>
      : <article className="presentation-item" key={entry.key} data-freshness={entry.item.freshness}>
        {entry.item.freshness !== "CURRENT" && <p className="presentation-freshness" role="note">Source freshness {entry.item.freshness.toLowerCase()}.</p>}
        {renderEntry(entry.item, renderers, context)}
        <SourceDisclosure sources={entry.item.source_refs} context={context} />
      </article>)}
    {projection.omittedCount > 0 && <p className="presentation-empty" role="note">{projection.omittedCount} more saved {projection.omittedCount === 1 ? "item is" : "items are"} not shown because this projection exceeded the display limit.</p>}
    {projection.entries.length === 0 && projection.omittedCount === 0 && <p className="presentation-empty">No presentation items are available.</p>}
  </section>;
}
