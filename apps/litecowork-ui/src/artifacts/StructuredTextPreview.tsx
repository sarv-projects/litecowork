import { useMemo, useState } from "react";
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

type InlinePart = { kind: "text" | "strong" | "em" | "code" | "link"; text: string; href?: string };
const INLINE_PATTERN = /(\*\*[^*\n]+\*\*|\*[^*\n]+\*|`[^`\n]+`|\[[^\]\n]+\]\(https:\/\/[^\s)]+\))/g;

function parseInline(text: string): InlinePart[] | null {
  const parts: InlinePart[] = [];
  let offset = 0;
  for (const match of text.matchAll(INLINE_PATTERN)) {
    const token = match[0];
    const start = match.index ?? 0;
    if (start > offset) parts.push({ kind: "text", text: text.slice(offset, start) });
    if (token.startsWith("**")) parts.push({ kind: "strong", text: token.slice(2, -2) });
    else if (token.startsWith("*")) parts.push({ kind: "em", text: token.slice(1, -1) });
    else if (token.startsWith("`")) parts.push({ kind: "code", text: token.slice(1, -1) });
    else {
      const link = /^\[([^\]]+)\]\((https:\/\/[^\s)]+)\)$/.exec(token);
      if (!link) return null;
      try {
        const url = new URL(link[2]);
        if (url.protocol !== "https:" || url.username || url.password) return null;
        parts.push({ kind: "link", text: link[1], href: url.href });
      } catch { return null; }
    }
    offset = start + token.length;
  }
  if (offset < text.length) parts.push({ kind: "text", text: text.slice(offset) });
  // Markdown punctuation we do not implement must never be presented as partially parsed.
  const residue = parts.filter(part => part.kind === "text").map(part => part.text).join("");
  if (/[*_`~\[\]{}]/.test(residue) || /!\[/.test(text)) return null;
  return parts;
}

type Block = { kind: "heading" | "paragraph" | "quote" | "ul" | "ol" | "code"; level?: number; lines: string[] };
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
function parseMarkdown(text: string): Block[] | null {
  if (text.length > 1_048_576) return null;
  const lines = text.replace(/\r\n?/g, "\n").split("\n");
  if (lines.length > 20_000 || lines.some(line => line.length > 16_384)) return null;
  const blocks: Block[] = [];
  let index = 0;
  const pushInline = (kind: Block["kind"], line: string, level?: number) => {
    if (!parseInline(line)) return false;
    blocks.push({ kind, level, lines: [line] });
    return true;
  };
  while (index < lines.length) {
    const line = lines[index];
    if (!line.trim()) { index++; continue; }
    if (/^\s*</.test(line) || /^\s{4,}\S/.test(line) || /^\s*(?:---+|\*\*\*+)\s*$/.test(line)) return null;
    if (/^\s*\|/.test(line)) return null;
    if (/^```/.test(line)) {
      if (!/^```(?:[A-Za-z0-9_+-]+)?$/.test(line)) return null;
      const body: string[] = [];
      index++;
      while (index < lines.length && lines[index] !== "```") body.push(lines[index++]);
      if (index >= lines.length) return null;
      blocks.push({ kind: "code", lines: body }); index++; continue;
    }
    const heading = /^(#{1,6})[ \t]+(.+?)(?:[ \t]+#+[ \t]*)?$/.exec(line);
    if (heading) { if (!pushInline("heading", heading[2], heading[1].length)) return null; index++; continue; }
    if (/^#{1,6}(?:\s|$)/.test(line)) return null;
    const list = /^(\s*)([-+*]|\d+[.)])\s+(.+)$/.exec(line);
    if (list) {
      if (list[1].length > 3) return null;
      const kind = /^\d/.test(list[2]) ? "ol" : "ul";
      const indent = list[1].length;
      const items: string[] = [];
      while (index < lines.length) {
        const item = /^(\s*)([-+*]|\d+[.)])\s+(.+)$/.exec(lines[index]);
        if (!item) break;
        // Nested or differently indented lists are outside this renderer's subset.
        // Fall back to the exact source instead of flattening their structure.
        if (item[1].length !== indent) return null;
        if ((/^\d/.test(item[2]) ? "ol" : "ul") !== kind) break;
        if (!parseInline(item[3])) return null;
        items.push(item[3]); index++;
      }
      blocks.push({ kind, lines: items }); continue;
    }
    if (/^\s*(?:[-+*]|\d+[.)])(?:\s|$)/.test(line)) return null;
    if (/^>\s?/.test(line)) {
      const quotes: string[] = [];
      while (index < lines.length && /^>\s?/.test(lines[index])) {
        const quote = lines[index++].replace(/^>\s?/, "");
        if (!parseInline(quote)) return null;
        quotes.push(quote);
      }
      blocks.push({ kind: "quote", lines: quotes }); continue;
    }
    const paragraph = [line]; index++;
    while (index < lines.length && lines[index].trim() && !/^(?:#{1,6}\s|```|>\s?|\s*(?:[-+*]|\d+[.)])\s+)/.test(lines[index])) paragraph.push(lines[index++]);
    if (!paragraph.every(part => parseInline(part))) return null;
    blocks.push({ kind: "paragraph", lines: paragraph });
  }
  return blocks;
}

function renderInline(text: string, keyPrefix: string) {
  const parts = parseInline(text);
  if (!parts) return text;
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

function MarkdownPreview({ text }: { text: string }) {
  const blocks = useMemo(() => parseMarkdown(text), [text]);
  if (!blocks) return <div className="artifact-markdown-fallback" role="status"><p>Some Markdown syntax is unsupported. Showing the original text.</p><pre tabIndex={0}>{text}</pre></div>;
  return <article className="artifact-markdown-preview" aria-label="Markdown preview">
    {blocks.map((block, index) => {
      const content = block.lines.map((line, lineIndex) => <span key={lineIndex}>{renderInline(line, `${index}-${lineIndex}`)}{lineIndex < block.lines.length - 1 && <br />}</span>);
      if (block.kind === "heading") { const Tag = headingTag(block.level); return <Tag key={index}>{content}</Tag>; }
      if (block.kind === "paragraph") return <p key={index}>{content}</p>;
      if (block.kind === "quote") return <blockquote key={index}>{content}</blockquote>;
      if (block.kind === "code") return <pre key={index}><code>{block.lines.join("\n")}</code></pre>;
      const Tag = block.kind === "ol" ? "ol" : "ul";
      return <Tag key={index}>{block.lines.map((item, itemIndex) => <li key={itemIndex}>{renderInline(item, `${index}-${itemIndex}`)}</li>)}</Tag>;
    })}
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
