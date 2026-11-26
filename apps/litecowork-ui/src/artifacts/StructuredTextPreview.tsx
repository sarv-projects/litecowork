import { useEffect, useMemo, useState, type ReactNode } from "react";
import { parseMarkdownDocument, type MarkdownBlock, type MarkdownList, type MarkdownToken } from "./markdown-document";
import type { CodeToken } from "./markdown-highlighter";
import "./structured-text-preview.css";

const MAX_ROWS = 500;
const MAX_COLUMNS = 32;
const MAX_CELL_CHARS = 8192;

type ParsedTable = { rows: string[][]; truncated: boolean };

/** Parse a bounded delimited-text preview without HTML interpretation or dependencies. */
function parseDelimited(text: string, delimiter: "," | "\t"): ParsedTable {
  if (text.length === 0) return { rows: [], truncated: false };
  const rows: string[][] = [];
  let row: string[] = [];
  let cell = "";
  let quoted = false;
  let closedQuote = false;
  let truncated = false;
  const finishCell = () => {
    if (cell.length > MAX_CELL_CHARS) throw new Error("A cell exceeds the safe preview limit.");
    row.push(cell);
    if (row.length > MAX_COLUMNS) throw new Error("The table has too many columns for the preview.");
    cell = "";
    closedQuote = false;
  };
  const finishRow = () => {
    finishCell();
    if (rows.length < MAX_ROWS) rows.push(row);
    else truncated = true;
    row = [];
  };

  for (let index = 0; index < text.length; index += 1) {
    const character = text[index];
    if (quoted) {
      if (character === '"') {
        if (text[index + 1] === '"') { cell += '"'; index += 1; }
        else { quoted = false; closedQuote = true; }
      } else cell += character;
      if (cell.length > MAX_CELL_CHARS) throw new Error("A cell exceeds the safe preview limit.");
      continue;
    }
    if (closedQuote && character !== delimiter && character !== "\r" && character !== "\n") {
      throw new Error("Unexpected content after a quoted field.");
    }
    if (character === '"') {
      if (cell.length !== 0 || closedQuote) throw new Error("A quoted field is malformed.");
      quoted = true;
    } else if (character === delimiter) finishCell();
    else if (character === "\r" || character === "\n") {
      finishRow();
      if (character === "\r" && text[index + 1] === "\n") index += 1;
      if (truncated) break;
    } else cell += character;
    if (cell.length > MAX_CELL_CHARS) throw new Error("A cell exceeds the safe preview limit.");
  }
  if (quoted) throw new Error("The file ends inside a quoted field.");
  if (!truncated && (cell.length > 0 || row.length > 0 || !/[\r\n]$/.test(text))) finishRow();
  return { rows, truncated };
}

function normalizedType(mediaType: string): string {
  return mediaType.split(";", 1)[0].trim().toLowerCase();
}

type HeadingTag = "h1" | "h2" | "h3" | "h4" | "h5" | "h6";
function headingTag(level: number | undefined): HeadingTag {
  switch (level) {
    case 1: return "h1";
    case 2: return "h2";
    case 3: return "h3";
    case 4: return "h4";
    case 5: return "h5";
    case 6: return "h6";
    default: return "h1";
  }
}

function renderInline(parts: MarkdownToken[], keyPrefix: string): ReactNode {
  return parts.map((part, index) => {
    const key = `${keyPrefix}-${index}`;
    if (part.kind === "strong") return <strong key={key}>{part.text}</strong>;
    if (part.kind === "em") return <em key={key}>{part.text}</em>;
    if (part.kind === "code") return <code key={key}>{part.text}</code>;
    if (part.kind === "link") return <a key={key} href={part.href} onClick={event => {
      event.preventDefault();
      if (part.href && window.confirm(`Open this external link?\n\n${part.href}`)) window.open(part.href, "_blank", "noopener,noreferrer");
    }}>{part.text} <span className="artifact-markdown-link-note">(external link)</span></a>;
    return <span key={key}>{part.text}</span>;
  });
}

function CodeBlock({ code, language }: { code: string; language: string | null }) {
  const [tokens, setTokens] = useState<CodeToken[] | null>(null);
  const [copyStatus, setCopyStatus] = useState("");
  useEffect(() => {
    let active = true;
    if (!language || code.length > 100_000) return () => { active = false; };
    void import("./markdown-highlighter").then(module => {
      if (active) setTokens(module.highlightCode(code, language));
    }).catch(() => {
      if (active) setTokens(null);
    });
    return () => { active = false; };
  }, [code, language]);
  const copy = () => {
    try {
      const writeText = navigator.clipboard?.writeText;
      if (!writeText) { setCopyStatus("Copy unavailable"); return; }
      setCopyStatus("Copying");
      void writeText.call(navigator.clipboard, code).then(
        () => setCopyStatus("Copied"),
        () => setCopyStatus("Copy unavailable"),
      );
    } catch {
      setCopyStatus("Copy unavailable");
    }
  };
  return <section className="artifact-markdown-code-block" aria-label={language ? `${language} code` : "Code block"}>
    <div className="artifact-markdown-code-toolbar">
      <span>{language ?? "Plain text"}</span>
      <button type="button" onClick={copy} aria-label="Copy code">Copy</button>
      {copyStatus && <span role="status">{copyStatus}</span>}
    </div>
    <pre tabIndex={0}><code>{tokens ? tokens.map((token, index) => token.kind === "text"
      ? token.text
      : <span className={`artifact-code-token-${token.kind}`} key={index}>{token.text}</span>) : code}</code></pre>
  </section>;
}

function renderList(list: MarkdownList, key: string): ReactNode {
  const Tag = list.ordered ? "ol" : "ul";
  return <Tag key={key} start={list.ordered ? list.start : undefined}>
    {list.items.map((item, index) => <li key={`${key}-${index}`}>
      {renderInline(item.content, `${key}-${index}`)}
      {item.children.map((child, childIndex) => renderList(child, `${key}-${index}-${childIndex}`))}
    </li>)}
  </Tag>;
}

function renderBlock(block: MarkdownBlock, index: number): ReactNode {
  if (block.kind === "heading") { const Tag = headingTag(block.level); return <Tag key={index}>{renderInline(block.content, `${index}`)}</Tag>; }
  if (block.kind === "paragraph" || block.kind === "quote") {
    const content = block.lines.map((line, lineIndex) => <span key={lineIndex}>{renderInline(line, `${index}-${lineIndex}`)}{lineIndex < block.lines.length - 1 && <br />}</span>);
    return block.kind === "quote" ? <blockquote key={index}>{content}</blockquote> : <p key={index}>{content}</p>;
  }
  if (block.kind === "list") return renderList(block, `${index}`);
  if (block.kind === "table") return <div className="artifact-markdown-table-scroll" role="region" aria-label="Markdown table" tabIndex={0} key={index}>
    <table className="artifact-markdown-table"><thead><tr>{block.headers.map((cell, column) => <th key={column} scope="col" className={block.alignments[column] ? `align-${block.alignments[column]}` : undefined}>{renderInline(cell, `${index}-head-${column}`)}</th>)}</tr></thead>
      <tbody>{block.rows.map((row, rowIndex) => <tr key={rowIndex}>{row.map((cell, column) => <td key={column} className={block.alignments[column] ? `align-${block.alignments[column]}` : undefined}>{renderInline(cell, `${index}-${rowIndex}-${column}`)}</td>)}</tr>)}</tbody>
    </table>
  </div>;
  if (block.kind === "code") return <CodeBlock key={index} code={block.code} language={block.language} />;
  if (block.kind === "rule") return <hr key={index} />;
  if (block.kind === "literal") return <pre className="artifact-markdown-literal" tabIndex={0} key={index}>{block.text}</pre>;
  return null;
}

function MarkdownPreview({ text }: { text: string }) {
  const document = useMemo(() => parseMarkdownDocument(text), [text]);
  return <article className="artifact-markdown-preview" aria-label="Markdown preview">
    {document.limited && <p className="artifact-markdown-limit" role="status">This answer exceeds the formatting limit. The original text is shown below.</p>}
    {document.blocks.map(renderBlock)}
  </article>;
}

/** Structured renderer for small text tables; raw source remains available verbatim. */
export function StructuredTextPreview({ text, mediaType }: { text: string; mediaType: string }) {
  const type = normalizedType(mediaType);
  const [showRaw, setShowRaw] = useState(false);
  const parsed = useMemo(() => {
    if (type !== "text/csv" && type !== "text/tab-separated-values") return null;
    try { return { table: parseDelimited(text, type === "text/csv" ? "," : "\t"), error: null }; }
    catch (error) { return { table: null, error: error instanceof Error ? error.message : "This table cannot be previewed safely." }; }
  }, [text, type]);

  if (type === "text/markdown") return <MarkdownPreview text={text} />;
  if (!parsed) return <pre tabIndex={0}>{text}</pre>;
  if (!parsed.table) return <div className="artifact-structured-fallback" role="status">
    <p>{parsed.error} Showing the original text instead.</p><pre tabIndex={0}>{text}</pre>
  </div>;

  const columnCount = Math.max(0, ...parsed.table.rows.map(row => row.length));
  return <section className="artifact-table-preview" aria-label="Delimited text preview">
    <div className="artifact-table-preview-heading"><p>{parsed.table.rows.length.toLocaleString()} preview rows · {columnCount} columns</p>
      <button type="button" aria-expanded={showRaw} onClick={() => setShowRaw(value => !value)}>{showRaw ? "Hide raw text" : "Show raw text"}</button>
    </div>
    {parsed.table.truncated && <p role="status">Showing the first {MAX_ROWS} rows. Download the immutable version to inspect the complete file.</p>}
    <div className="artifact-table-scroll" role="region" aria-label="Scrollable table preview" tabIndex={0}>
      <table><caption>{mediaType} Artifact content, rendered as untrusted text</caption><thead><tr>
        {Array.from({ length: columnCount }, (_, index) => <th key={index} scope="col">Column {index + 1}</th>)}
      </tr></thead><tbody>{parsed.table.rows.map((values, rowIndex) => <tr key={rowIndex}>
        {Array.from({ length: columnCount }, (_, columnIndex) => <td key={columnIndex}>{values[columnIndex] ?? ""}</td>)}
      </tr>)}</tbody></table>
    </div>
    {showRaw && <details open><summary>Original text</summary><pre tabIndex={0}>{text}</pre></details>}
  </section>;
}
