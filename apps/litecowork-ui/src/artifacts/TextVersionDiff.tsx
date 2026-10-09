import { useMemo } from "react";
import { lineDiff } from "./text-version-diff-model";
import "./text-version-diff.css";

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
