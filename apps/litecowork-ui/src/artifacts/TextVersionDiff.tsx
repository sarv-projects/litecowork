import { useMemo } from "react";
import "./text-version-diff.css";

const MAX_DIFF_LINES = 400;
const MAX_DIFF_CELLS = 160_000;
const MAX_DIFF_LINE_CHARS = 16_384;

type DiffRow = {
  kind: "same" | "removed" | "added";
  comparedLine: number | null;
  selectedLine: number | null;
  comparedText: string;
  selectedText: string;
};
type Diff = { rows: DiffRow[]; additions: number; removals: number };

/**
 * Bounded LCS line diff. Large/highly fragmented files use the Workbench's exact
 * side-by-side view instead, keeping untrusted Artifact text from causing unbounded
 * CPU or memory use in the UI process.
 */
function lineDiff(comparedText: string, selectedText: string): Diff | null {
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

export function TextVersionDiff({ comparedText, selectedText, comparedVersion, selectedVersion, comparedRevisionId, selectedRevisionId }: {
  comparedText: string;
  selectedText: string;
  comparedVersion: number;
  selectedVersion: number;
  comparedRevisionId: string;
  selectedRevisionId: string;
}) {
  const diff = useMemo(() => lineDiff(comparedText, selectedText), [comparedText, selectedText]);
  if (!diff) return <div className="artifact-line-diff-fallback" role="status">
    <p>Line comparison is limited to at most 400 lines per version, 160,000 line-pairs, and 16,384 characters per line. Use side-by-side view for this content.</p>
  </div>;
  return <section className="artifact-line-diff" aria-label={`Line comparison of version ${comparedVersion} and version ${selectedVersion}`}>
    <p className="artifact-line-diff-summary" role="status">{diff.additions} line{diff.additions === 1 ? "" : "s"} added · {diff.removals} removed</p>
    <div className="artifact-line-diff-scroll" role="region" aria-label="Scrollable line comparison" tabIndex={0}>
      <table>
        <caption>Compared version {comparedVersion} and selected version {selectedVersion}; text is displayed literally.</caption>
        <thead><tr>
          <th scope="col" colSpan={2}><span>Compared · v{comparedVersion}</span><small>Output revision {comparedRevisionId}</small></th>
          <th scope="col" colSpan={2}><span>Selected · v{selectedVersion}</span><small>Output revision {selectedRevisionId}</small></th>
        </tr></thead>
        <tbody>{diff.rows.map((row, index) => (
          <tr className={`artifact-line-diff-row is-${row.kind}`} key={`${row.kind}-${row.comparedLine ?? "x"}-${row.selectedLine ?? "x"}-${index}`}>
            <td className="artifact-line-diff-number">{row.comparedLine ?? ""}</td>
            <td className="artifact-line-diff-text">{row.comparedText}</td>
            <td className="artifact-line-diff-number">{row.selectedLine ?? ""}</td>
            <td className="artifact-line-diff-text">{row.selectedText}</td>
          </tr>
        ))}</tbody>
      </table>
    </div>
  </section>;
}
