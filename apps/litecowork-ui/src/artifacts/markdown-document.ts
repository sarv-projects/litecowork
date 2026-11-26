export type MarkdownToken = { kind: "text" | "strong" | "em" | "code" | "link"; text: string; href?: string };
export type MarkdownList = { ordered: boolean; start: number; items: { content: MarkdownToken[]; children: MarkdownList[] }[] };
export type MarkdownBlock =
  | { kind: "heading"; level: number; content: MarkdownToken[]; source: string }
  | { kind: "paragraph" | "quote"; lines: MarkdownToken[][]; source: string }
  | ({ kind: "list"; source: string } & MarkdownList)
  | { kind: "table"; headers: MarkdownToken[][]; alignments: ("left" | "center" | "right" | null)[]; rows: MarkdownToken[][][]; source: string }
  | { kind: "code"; language: string | null; code: string; source: string }
  | { kind: "rule"; source: string }
  | { kind: "literal"; text: string; source: string };
export type MarkdownDocument = { blocks: MarkdownBlock[]; limited: boolean };

export const MARKDOWN_LIMITS = Object.freeze({
  documentBytes: 1_048_576,
  lines: 20_000,
  lineCharacters: 16_384,
  blocks: 2_000,
  listDepth: 8,
  listItems: 2_000,
  orderedListStart: 2_147_483_647,
  tableRows: 500,
  tableColumns: 32,
  tableCellCharacters: 8_192,
});

const INLINE_PATTERN = /(!\[[^\]\n]*\]\([^\n)]*\)|\*\*[^*\n]+\*\*|\*[^*\n]+\*|`[^`\n]+`|\[[^\]\n]+\]\([^\s)]+\))/g;
const LIST_PATTERN = /^(\s*)([-+*]|(\d+)[.)])\s+(.+)$/;
const encoder = new TextEncoder();

function literal(text: string, source = text): MarkdownBlock {
  return { kind: "literal", text, source };
}

function safeHttpsHref(raw: string): string | null {
  try {
    const url = new URL(raw);
    if (url.protocol !== "https:" || url.username || url.password) return null;
    return url.href;
  } catch {
    return null;
  }
}

export function parseMarkdownInline(text: string): MarkdownToken[] {
  const tokens: MarkdownToken[] = [];
  let offset = 0;
  for (const match of text.matchAll(INLINE_PATTERN)) {
    const value = match[0];
    const start = match.index ?? 0;
    if (start > offset) tokens.push({ kind: "text", text: text.slice(offset, start) });
    if (value.startsWith("![")) {
      tokens.push({ kind: "text", text: value });
    } else if (value.startsWith("**")) {
      tokens.push({ kind: "strong", text: value.slice(2, -2) });
    } else if (value.startsWith("*")) {
      tokens.push({ kind: "em", text: value.slice(1, -1) });
    } else if (value.startsWith("`")) {
      tokens.push({ kind: "code", text: value.slice(1, -1) });
    } else {
      const link = /^\[([^\]]+)\]\(([^\s)]+)\)$/.exec(value);
      const href = link ? safeHttpsHref(link[2]) : null;
      if (!link || !href) tokens.push({ kind: "text", text: value });
      else tokens.push({ kind: "link", text: link[1], href });
    }
    offset = start + value.length;
  }
  if (offset < text.length) tokens.push({ kind: "text", text: text.slice(offset) });
  if (tokens.length === 0 && text.length > 0) tokens.push({ kind: "text", text });
  return tokens;
}

type LineState = { lines: string[]; rawLines: string[] };
type ParseResult = { blocks: MarkdownBlock[]; next: number; limited: boolean };

function sourceOf(state: LineState, start: number, end: number): string {
  return state.rawLines.slice(start, end).join("\n");
}

function withinLineBounds(text: string): boolean {
  let lineCount = 1;
  let lineStart = 0;
  for (let index = 0; index < text.length; index += 1) {
    if (text.charCodeAt(index) !== 10) continue;
    const lineEnd = index > lineStart && text.charCodeAt(index - 1) === 13 ? index - 1 : index;
    if (lineEnd - lineStart > MARKDOWN_LIMITS.lineCharacters) return false;
    lineCount += 1;
    if (lineCount > MARKDOWN_LIMITS.lines) return false;
    lineStart = index + 1;
  }
  const finalEnd = text.length > lineStart && text.charCodeAt(text.length - 1) === 13 ? text.length - 1 : text.length;
  return finalEnd - lineStart <= MARKDOWN_LIMITS.lineCharacters;
}

function marker(line: string) {
  const match = LIST_PATTERN.exec(line);
  if (!match) return null;
  const start = match[3] ? Number(match[3]) : 1;
  return {
    indent: match[1].length,
    ordered: match[3] !== undefined,
    start: Number.isSafeInteger(start) && start <= MARKDOWN_LIMITS.orderedListStart ? start : null,
    content: match[4],
  };
}

function splitTableRow(line: string): string[] | null {
  const trimmed = line.trim();
  if (!trimmed.includes("|")) return null;
  let body = trimmed;
  if (body.startsWith("|")) body = body.slice(1);
  if (body.endsWith("|") && !body.endsWith("\\|")) body = body.slice(0, -1);
  const cells: string[] = [];
  let cell = "";
  let escaped = false;
  for (const character of body) {
    if (escaped) {
      cell += character === "|" ? "|" : `\\${character}`;
      escaped = false;
    } else if (character === "\\") escaped = true;
    else if (character === "|") { cells.push(cell.trim()); cell = ""; }
    else cell += character;
    if (cell.length > MARKDOWN_LIMITS.tableCellCharacters) return null;
  }
  if (escaped) cell += "\\";
  cells.push(cell.trim());
  return cells.length > 0 ? cells : null;
}

function findTableFallbackEnd(state: LineState, start: number): number {
  let index = start + 2;
  while (index < state.lines.length && state.lines[index]!.trim() && state.lines[index]!.includes("|")) index += 1;
  return index;
}

function parseTable(state: LineState, start: number): { block: MarkdownBlock; next: number } | null {
  if (start + 1 >= state.lines.length) return null;
  const separators = splitTableRow(state.lines[start + 1]);
  if (!separators || !separators.every(cell => /^:?-{3,}:?$/.test(cell))) return null;
  const headers = splitTableRow(state.lines[start]);
  const fallbackEnd = () => findTableFallbackEnd(state, start);
  if (!headers || headers.length !== separators.length || headers.length > MARKDOWN_LIMITS.tableColumns) {
    const end = fallbackEnd();
    return { block: literal(sourceOf(state, start, end)), next: end };
  }
  const alignments = separators.map(cell => cell.startsWith(":") ? (cell.endsWith(":") ? "center" : "left") : (cell.endsWith(":") ? "right" : null)) as ("left" | "center" | "right" | null)[];
  const rows: MarkdownToken[][][] = [];
  let index = start + 2;
  while (index < state.lines.length && state.lines[index].trim()) {
    const values = splitTableRow(state.lines[index]);
    if (!values) {
      if (state.lines[index]!.includes("|")) {
        const end = fallbackEnd();
        return { block: literal(sourceOf(state, start, end)), next: end };
      }
      break;
    }
    if (values.length !== headers.length || rows.length >= MARKDOWN_LIMITS.tableRows) {
      const end = fallbackEnd();
      return { block: literal(sourceOf(state, start, end)), next: end };
    }
    rows.push(values.map(parseMarkdownInline));
    index += 1;
  }
  const block: MarkdownBlock = {
    kind: "table",
    headers: headers.map(parseMarkdownInline),
    alignments,
    rows,
    source: sourceOf(state, start, index),
  };
  return { block, next: index };
}

function parseListAt(state: LineState, start: number, depth: number, budget: { items: number }): { list: MarkdownList; next: number } | null {
  const first = marker(state.lines[start]);
  if (!first || first.start === null || (depth === 0 && first.indent >= 4) || depth >= MARKDOWN_LIMITS.listDepth) return null;
  const list: MarkdownList = { ordered: first.ordered, start: first.start, items: [] };
  let index = start;
  while (index < state.lines.length) {
    const current = marker(state.lines[index]);
    if (!current || current.indent !== first.indent || current.ordered !== first.ordered) break;
    budget.items += 1;
    if (budget.items > MARKDOWN_LIMITS.listItems) return null;
    list.items.push({ content: parseMarkdownInline(current.content), children: [] });
    const item = list.items[list.items.length - 1]!;
    index += 1;
    while (index < state.lines.length) {
      if (!state.lines[index].trim()) {
        let nextNonBlank = index + 1;
        while (nextNonBlank < state.lines.length && !state.lines[nextNonBlank].trim()) nextNonBlank += 1;
        const lookahead = nextNonBlank < state.lines.length ? marker(state.lines[nextNonBlank]) : null;
        if (lookahead && lookahead.indent >= first.indent) { index = nextNonBlank; continue; }
        break;
      }
      const nested = marker(state.lines[index]);
      if (!nested || nested.indent <= first.indent) break;
      const parsed = parseListAt(state, index, depth + 1, budget);
      if (!parsed) return null;
      item.children.push(parsed.list);
      index = parsed.next;
    }
  }
  return list.items.length ? { list, next: index } : null;
}

function findListFallbackEnd(state: LineState, start: number): number {
  let index = start + 1;
  while (index < state.lines.length) {
    const line = state.lines[index];
    if (!line.trim()) {
      if (index + 1 < state.lines.length && (marker(state.lines[index + 1]) || /^\s{2,}\S/.test(state.lines[index + 1]))) index += 1;
      else break;
    } else if (marker(line) || /^\s{2,}\S/.test(line)) index += 1;
    else break;
  }
  return index;
}

function isBlockStart(lines: string[], index: number): boolean {
  const line = lines[index] ?? "";
  return /^ {0,3}#{1,6}[ \t]+/.test(line)
    || /^ {0,3}(?:```|~~~)/.test(line)
    || /^ {0,3}>/.test(line)
    || /^ {0,3}(?:(?:\*\s*){3,}|(?:-\s*){3,}|(?:_\s*){3,})$/.test(line)
    || marker(line) !== null
    || (index + 1 < lines.length && splitTableRow(line) !== null && splitTableRow(lines[index + 1])?.every(cell => /^:?-{3,}:?$/.test(cell)) === true);
}

function parseBlocks(state: LineState, start: number, depth = 0): ParseResult {
  const blocks: MarkdownBlock[] = [];
  let index = start;
  let limited = false;
  while (index < state.lines.length) {
    const line = state.lines[index]!;
    if (!line.trim()) { index += 1; continue; }
    if (blocks.length >= MARKDOWN_LIMITS.blocks) {
      const previous = blocks.pop();
      const remaining = sourceOf(state, index, state.lines.length);
      blocks.push(literal(previous ? `${previous.source}\n${remaining}` : remaining));
      return { blocks, next: state.lines.length, limited: true };
    }
    if (/^ {4,}\S/.test(line)) {
      const begin = index;
      while (index < state.lines.length && (/^ {4,}\S/.test(state.lines[index]!) || !state.lines[index]!.trim())) index += 1;
      while (index > begin && !state.lines[index - 1]!.trim()) index -= 1;
      blocks.push(literal(sourceOf(state, begin, index)));
      continue;
    }
    const fence = /^ {0,3}(`{3,}|~{3,})(.*)$/.exec(line);
    if (fence) {
      const begin = index;
      const markerValue = fence[1]!;
      const info = fence[2]!.trim();
      const language = /^[A-Za-z0-9_+.#-]{1,32}$/.test(info) ? info.toLowerCase() : null;
      index += 1;
      const codeLines: string[] = [];
      let closed = false;
      while (index < state.lines.length) {
        const closing = new RegExp(`^ {0,3}${markerValue[0]}{${markerValue.length},}\\s*$`);
        if (closing.test(state.lines[index]!)) { closed = true; index += 1; break; }
        codeLines.push(state.lines[index]!);
        index += 1;
      }
      if (!closed || (info && language === null)) blocks.push(literal(sourceOf(state, begin, index)));
      else blocks.push({ kind: "code", language, code: codeLines.join("\n"), source: sourceOf(state, begin, index) });
      continue;
    }
    const heading = /^ {0,3}(#{1,6})[ \t]+(.+?)(?:[ \t]+#+[ \t]*)?$/.exec(line);
    if (heading) {
      blocks.push({ kind: "heading", level: heading[1]!.length, content: parseMarkdownInline(heading[2]!), source: sourceOf(state, index, index + 1) });
      index += 1;
      continue;
    }
    if (/^ {0,3}(?:(?:\*\s*){3,}|(?:-\s*){3,}|(?:_\s*){3,})$/.test(line)) {
      blocks.push({ kind: "rule", source: sourceOf(state, index, index + 1) });
      index += 1;
      continue;
    }
    if (/^ {0,3}>/.test(line)) {
      const begin = index;
      const quoteLines: MarkdownToken[][] = [];
      while (index < state.lines.length && /^ {0,3}>/.test(state.lines[index]!)) {
        quoteLines.push(parseMarkdownInline(state.lines[index]!.replace(/^ {0,3}> ?/, "")));
        index += 1;
      }
      blocks.push({ kind: "quote", lines: quoteLines, source: sourceOf(state, begin, index) });
      continue;
    }
    const table = parseTable(state, index);
    if (table) {
      blocks.push(table.block);
      index = table.next;
      continue;
    }
    const list = parseListAt(state, index, depth, { items: 0 });
    if (list) {
      blocks.push({ kind: "list", ...list.list, source: sourceOf(state, index, list.next) });
      index = list.next;
      continue;
    }
    if (marker(line)) {
      const end = findListFallbackEnd(state, index);
      blocks.push(literal(sourceOf(state, index, end)));
      index = end;
      continue;
    }
    const begin = index;
    const paragraphLines = [line];
    index += 1;
    while (index < state.lines.length && state.lines[index]!.trim() && !isBlockStart(state.lines, index)) {
      if (/^ {4,}\S/.test(state.lines[index]!)) break;
      paragraphLines.push(state.lines[index]!);
      index += 1;
    }
    blocks.push({ kind: "paragraph", lines: paragraphLines.map(parseMarkdownInline), source: sourceOf(state, begin, index) });
  }
  return { blocks, next: index, limited };
}

export function parseMarkdownDocument(text: string): MarkdownDocument {
  const byteLength = text.length <= MARKDOWN_LIMITS.documentBytes ? encoder.encode(text).byteLength : text.length + 1;
  if (byteLength > MARKDOWN_LIMITS.documentBytes) return { blocks: [literal(text)], limited: true };
  if (!withinLineBounds(text)) return { blocks: [literal(text)], limited: true };
  const rawLines = text.split("\n");
  const lines = rawLines.map(line => line.replace(/\r$/, ""));
  const parsed = parseBlocks({ lines, rawLines }, 0);
  return { blocks: parsed.blocks, limited: parsed.limited };
}
