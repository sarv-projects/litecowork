const MAX_DIFF_LINES = 400;
const MAX_DIFF_CELLS = 160_000;
const MAX_DIFF_LINE_CHARS = 16_384;

export type DiffRow = {
  kind: "same" | "removed" | "added";
  comparedLine: number | null;
  selectedLine: number | null;
  comparedText: string;
  selectedText: string;
};

export type LineDiff = { rows: DiffRow[]; additions: number; removals: number };

/**
 * Bounded LCS line diff. Large/highly fragmented files use the Workbench's exact
 * side-by-side view instead, keeping untrusted Artifact text from causing unbounded
 * CPU or memory use in the UI process. Returns source strings as data; callers render
 * them as text nodes and must not interpret them as markup.
 */
export function lineDiff(comparedText: string, selectedText: string): LineDiff | null {
  const compared = comparedText.replace(/\r\n?/g, "\n").split("\n");
  const selected = selectedText.replace(/\r\n?/g, "\n").split("\n");
  if (compared.length > MAX_DIFF_LINES || selected.length > MAX_DIFF_LINES
    || compared.length * selected.length > MAX_DIFF_CELLS
    || compared.some(line => line.length > MAX_DIFF_LINE_CHARS)
    || selected.some(line => line.length > MAX_DIFF_LINE_CHARS)) return null;

  const rows = Array.from({ length: compared.length + 1 }, () => new Uint16Array(selected.length + 1));
  for (let left = compared.length - 1; left >= 0; left -= 1) {
    for (let right = selected.length - 1; right >= 0; right -= 1) {
      rows[left][right] = compared[left] === selected[right]
        ? rows[left + 1][right + 1] + 1
        : Math.max(rows[left + 1][right], rows[left][right + 1]);
    }
  }

  const result: DiffRow[] = [];
  let left = 0;
  let right = 0;
  let additions = 0;
  let removals = 0;
  while (left < compared.length || right < selected.length) {
    if (left < compared.length && right < selected.length && compared[left] === selected[right]) {
      result.push({ kind: "same", comparedLine: left + 1, selectedLine: right + 1, comparedText: compared[left], selectedText: selected[right] });
      left += 1;
      right += 1;
    } else if (left < compared.length && (right === selected.length || rows[left + 1][right] >= rows[left][right + 1])) {
      result.push({ kind: "removed", comparedLine: left + 1, selectedLine: null, comparedText: compared[left], selectedText: "" });
      left += 1;
      removals += 1;
    } else {
      result.push({ kind: "added", comparedLine: null, selectedLine: right + 1, comparedText: "", selectedText: selected[right] });
      right += 1;
      additions += 1;
    }
  }
  return { rows: result, additions, removals };
}
