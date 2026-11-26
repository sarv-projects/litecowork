export type CodeToken = { kind: "text" | "keyword" | "string" | "number" | "comment" | "punctuation"; text: string };

export const MAX_HIGHLIGHT_CHARACTERS = 100_000;
export const MAX_HIGHLIGHT_TOKENS = 20_000;
const SUPPORTED_LANGUAGES = new Set(["js", "javascript", "jsx", "ts", "typescript", "tsx", "py", "python", "rs", "rust", "sh", "bash", "shell", "json", "css", "html"]);
const COMMON_KEYWORDS = new Set("as async await break case catch class const continue default do else export extends finally for from function if import in instanceof let new return static super switch this throw try typeof var void while yield interface type enum pub use fn mod struct impl trait match mut self crate where move ref dyn const async await static unsafe true false null undefined".split(" "));
const PYTHON_KEYWORDS = new Set("and as assert async await break class continue def del elif else except False finally for from global if import in is lambda None nonlocal not or pass raise return True try while with yield".split(" "));

function wordStart(character: string): boolean { return /[A-Za-z_$]/.test(character); }
function wordPart(character: string): boolean { return /[A-Za-z0-9_$]/.test(character); }

function keywordsFor(language: string): Set<string> {
  return language === "py" || language === "python" ? PYTHON_KEYWORDS : COMMON_KEYWORDS;
}

function lineCommentStart(language: string, source: string, index: number): string | null {
  if ((language === "js" || language === "javascript" || language === "jsx" || language === "ts" || language === "typescript" || language === "tsx" || language === "rust" || language === "rs") && source.startsWith("//", index)) return "//";
  if ((language === "py" || language === "python" || language === "sh" || language === "bash" || language === "shell") && source[index] === "#") return "#";
  return null;
}

function commentEnd(source: string, index: number, start: string): number {
  if (start === "/*") {
    const close = source.indexOf("*/", index + start.length);
    return close < 0 ? source.length : close + 2;
  }
  if (start === "<!--") {
    const close = source.indexOf("-->", index + start.length);
    return close < 0 ? source.length : close + 3;
  }
  let end = index + start.length;
  while (end < source.length && source[end] !== "\n") end += 1;
  return end;
}

function stringEnd(source: string, index: number, quote: string): number {
  let end = index + 1;
  while (end < source.length) {
    if (source[end] === "\\") { end += 2; continue; }
    if (source[end] === quote) return end + 1;
    end += 1;
  }
  return source.length;
}

/** Returns safe text tokens; it never generates markup or performs language evaluation. */
export function highlightCode(source: string, language: string | null): CodeToken[] | null {
  const normalizedLanguage = language?.toLowerCase() ?? "";
  if (!SUPPORTED_LANGUAGES.has(normalizedLanguage) || source.length > MAX_HIGHLIGHT_CHARACTERS) return null;
  const tokens: CodeToken[] = [];
  const push = (kind: CodeToken["kind"], start: number, end: number) => {
    if (end > start) tokens.push({ kind, text: source.slice(start, end) });
    return tokens.length <= MAX_HIGHLIGHT_TOKENS;
  };
  let index = 0;
  while (index < source.length) {
    const character = source[index]!;
    const lineComment = lineCommentStart(normalizedLanguage, source, index);
    const blockComment = (normalizedLanguage === "css" || normalizedLanguage === "js" || normalizedLanguage === "javascript" || normalizedLanguage === "jsx" || normalizedLanguage === "ts" || normalizedLanguage === "typescript" || normalizedLanguage === "tsx" || normalizedLanguage === "rust" || normalizedLanguage === "rs") && source.startsWith("/*", index) ? "/*" : null;
    const htmlComment = normalizedLanguage === "html" && source.startsWith("<!--", index) ? "<!--" : null;
    const comment = lineComment ?? blockComment ?? htmlComment;
    if (comment) {
      const end = commentEnd(source, index, comment);
      if (!push("comment", index, end)) return null;
      index = end;
      continue;
    }
    if (character === "'" || character === '"' || ((normalizedLanguage === "js" || normalizedLanguage === "javascript" || normalizedLanguage === "jsx" || normalizedLanguage === "ts" || normalizedLanguage === "typescript" || normalizedLanguage === "tsx") && character === "`")) {
      const end = stringEnd(source, index, character);
      if (!push("string", index, end)) return null;
      index = end;
      continue;
    }
    if (/\s/.test(character)) {
      let end = index + 1;
      while (end < source.length && /\s/.test(source[end]!)) end += 1;
      if (!push("text", index, end)) return null;
      index = end;
      continue;
    }
    if (/\d/.test(character)) {
      let end = index + 1;
      while (end < source.length && /[A-Za-z0-9_.]/.test(source[end]!)) end += 1;
      if (!push("number", index, end)) return null;
      index = end;
      continue;
    }
    if (wordStart(character)) {
      let end = index + 1;
      while (end < source.length && wordPart(source[end]!)) end += 1;
      const word = source.slice(index, end);
      if (!push(keywordsFor(normalizedLanguage).has(word) ? "keyword" : "text", index, end)) return null;
      index = end;
      continue;
    }
    if (!push("punctuation", index, index + 1)) return null;
    index += 1;
  }
  return tokens;
}
