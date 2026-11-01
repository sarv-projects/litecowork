import { useEffect, useState } from "react";
import type {
  WorkerPerformanceProjection,
  WorkerTaskCategory,
} from "../coworkers/delegation-profile-api";
import "./delegation-profile-performance.css";

const CATEGORIES: Array<{ value: WorkerTaskCategory; label: string }> = [
  { value: "OTHER", label: "Other work" },
  { value: "SOFTWARE_ENGINEERING", label: "Software engineering" },
  { value: "RESEARCH", label: "Research" },
  { value: "WRITING", label: "Writing" },
  { value: "DATA_ANALYSIS", label: "Data analysis" },
  { value: "OFFICE", label: "Office work" },
  { value: "BROWSER", label: "Browser work" },
  { value: "PERSONAL_ADMIN", label: "Personal admin" },
];

export type WorkerPerformanceApi = {
  getPerformance(profileId: string, category?: WorkerTaskCategory, signal?: AbortSignal): Promise<WorkerPerformanceProjection>;
};

function rate(value: number | null): string {
  return value === null ? "Not enough verified data" : `${Math.round(value * 100)}%`;
}

export function WorkerPerformanceSummary({ projection }: { projection: WorkerPerformanceProjection }) {
  const category = CATEGORIES.find(item => item.value === projection.task_category)?.label ?? "Other work";
  const cost = projection.median_observed_cost;
  return <div className="worker-performance__content">
    <div className="worker-performance__summary">
      <span><strong>{projection.sample_count}</strong> completed or failed attempts</span>
      <span className={`worker-performance__confidence is-${projection.confidence.toLowerCase()}`}>{projection.confidence === "LOW" ? "Early signal" : projection.confidence === "MEDIUM" ? "Building evidence" : "Established sample"}</span>
    </div>
    {projection.confidence === "LOW" && <p className="worker-performance__note">Too little verified history to compare reliably. Missing observations are shown as unknown.</p>}
    <dl className="worker-performance__metrics">
      <div><dt>Verified success</dt><dd>{rate(projection.verifier_pass_rate)}</dd></div>
      <div><dt>Typical time</dt><dd>{projection.median_latency_ms === null ? "Not enough data" : `${Math.round(projection.median_latency_ms / 1000)} sec`}</dd></div>
      <div><dt>Retries</dt><dd>{rate(projection.retry_rate)}</dd></div>
      <div><dt>Needed your help</dt><dd>{rate(projection.human_intervention_rate)}</dd></div>
      <div><dt>Observed cost</dt><dd>{cost === null ? "Unknown" : `${cost.quantity} ${cost.unit}${cost.currency ? ` · ${cost.currency}` : ""}${cost.confidence === "ESTIMATED" ? " · estimated" : ""}`}</dd></div>
    </dl>
    {projection.failure_categories.length > 0 && <p className="worker-performance__failures">Common recorded outcomes: {projection.failure_categories.join(", ")}</p>}
    <p className="worker-performance__note">{category} only. This summary never exposes prompts or agent transcripts. Worker execution is not available in this Runtime.</p>
  </div>;
}

export function DelegationProfilePerformance({ api, profileId }: { api: WorkerPerformanceApi; profileId: string }) {
  const [open, setOpen] = useState(false);
  const [category, setCategory] = useState<WorkerTaskCategory>("OTHER");
  const [projection, setProjection] = useState<WorkerPerformanceProjection | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [retry, setRetry] = useState(0);

  useEffect(() => {
    if (!open) return;
    const controller = new AbortController();
    let current = true;
    setLoading(true);
    setError(null);
    setProjection(null);
    void api.getPerformance(profileId, category, controller.signal).then(value => {
      if (current) setProjection(value);
    }).catch(() => {
      if (current && !controller.signal.aborted) setError("Performance history could not be loaded. Retry when the local Runtime is available.");
    }).finally(() => {
      if (current) setLoading(false);
    });
    return () => {
      current = false;
      controller.abort();
    };
  }, [api, category, open, profileId, retry]);

  return <section className="worker-performance" aria-label={`Worker performance for ${profileId}`}>
    <button type="button" className="worker-performance__toggle" aria-expanded={open} onClick={() => setOpen(value => !value)}>
      <span>Past work</span><span>{open ? "Hide summary" : "Show summary"}</span>
    </button>
    {open && <>
      <div className="worker-performance__heading">
        <h4>Performance history</h4>
      <label>Work type<select value={category} onChange={event => setCategory(event.currentTarget.value as WorkerTaskCategory)}>
        {CATEGORIES.map(item => <option key={item.value} value={item.value}>{item.label}</option>)}
      </select></label>
      </div>
      {loading ? <p className="worker-performance__state" role="status">Loading performance history…</p>
        : error ? <div className="worker-performance__state" role="status"><p>{error}</p><button type="button" onClick={() => setRetry(value => value + 1)}>Retry</button></div>
          : projection ? <WorkerPerformanceSummary projection={projection} />
            : <p className="worker-performance__state">No performance history is available for this profile.</p>}
    </>}
  </section>;
}
