import type { ReactNode } from "react";
import { StructuredTextPreview } from "../artifacts/StructuredTextPreview";
import { resolveRichResponseContent, type RichBlock, type ValidatedRichPresentation } from "./rich-presentation";
import "./rich-response.css";

function renderBlock(block: RichBlock, key: string): ReactNode {
  switch (block.kind) {
    case "TEXT_SLICE":
      return <p className={`rich-response-text width-${(block.width ?? "READABLE").toLowerCase()}`} key={key}>{block.text}</p>;
    case "LAYOUT":
      return <div className={`rich-response-layout layout-${block.layout.toLowerCase()} width-${(block.width ?? "READABLE").toLowerCase()}`} key={key}>{block.children.map((child, index) => renderBlock(child, `${key}.${index}`))}</div>;
    case "CARD":
      return <section className="rich-response-card" key={key}><h3>{block.title}</h3><div className="rich-response-card-content">{block.children.map((child, index) => renderBlock(child, `${key}.${index}`))}</div></section>;
    case "CALLOUT":
      return <aside className="rich-response-callout" data-tone={block.tone} aria-label={block.accessibleSummary} key={key}>{renderPlainText(block.text)}</aside>;
    case "TABLE":
      return <figure className="rich-response-table-frame" key={key}>
        {block.accessibleSummary && <figcaption className="rich-response-table-summary">{block.accessibleSummary}</figcaption>}
        <div className="rich-response-table-scroll" role="region" aria-label={block.accessibleSummary || "Response table"} tabIndex={0}>
          <table><thead><tr>{block.headers.map((header, index) => <th scope="col" key={`${index}:${header}`}>{header}</th>)}</tr></thead>
            <tbody>{block.rows.map((row, rowIndex) => <tr key={rowIndex}>{row.map((cell, cellIndex) => <td key={cellIndex}>{cell === null ? "—" : String(cell)}</td>)}</tr>)}</tbody>
          </table>
        </div>
      </figure>;
  }
}

function renderPlainText(text: string): ReactNode {
  return <span className="rich-response-text">{text}</span>;
}

/**
 * A parser-branded immutable presentation may upgrade the semantic answer in place.
 * The complete committed semantic answer remains available in an accessible disclosure;
 * missing, unsupported, or rejected presentation data falls back to it directly.
 * Rich content is display-only and intentionally has no action or host-state bindings.
 */
export function RichResponseView({ semanticText, presentation }: {
  semanticText: string;
  presentation?: ValidatedRichPresentation | null;
}) {
  const response = resolveRichResponseContent(semanticText, presentation);
  const safePresentation = response.primary === "RICH" ? response.presentation : null;
  return <section className="rich-response" aria-label="Assistant response">
    {safePresentation && safePresentation.rootBlocks.length > 0 ? <>
      {safePresentation.accessibilitySummary && <p className="rich-response-accessibility-summary">{safePresentation.accessibilitySummary}</p>}
      <div className="rich-response-blocks" aria-label="Formatted response" data-testid="rich-answer">
        {safePresentation.rootBlocks.map((block, index) => renderBlock(block, `root.${index}`))}
      </div>
      <details className="rich-response-semantic-fallback">
        <summary>Read the original response</summary>
        <div className="rich-response-semantic" data-testid="semantic-answer"><StructuredTextPreview text={response.semanticText} mediaType="text/markdown" /></div>
      </details>
    </> : <div className="rich-response-semantic" data-testid="semantic-answer"><StructuredTextPreview text={response.semanticText} mediaType="text/markdown" /></div>}
  </section>;
}
