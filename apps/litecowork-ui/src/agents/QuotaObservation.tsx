import { useEffect, useState } from "react";
import {
  isQuotaObservationExpired,
  type AgentQuotaObservationApi,
  type QuotaObservation,
} from "./agent-quota-api";
import "./quota-observation.css";

const STATE_LABEL: Record<QuotaObservation["state"], string> = {
  NORMAL: "Normal",
  LOW: "Low",
  EXHAUSTED: "Exhausted",
  UNKNOWN: "Unknown",
};

function observedAge(timestamp: string, now: number): string {
  const elapsed = Math.max(0, now - Date.parse(timestamp));
  if (elapsed < 60_000) return "less than a minute ago";
  const minutes = Math.floor(elapsed / 60_000);
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} hr ago`;
  return `${Math.floor(hours / 24)} days ago`;
}

export function QuotaObservationSummary({ observation, now = Date.now() }: {
  observation: QuotaObservation | null;
  now?: number;
}) {
  if (observation === null) {
    return <p className="quota-observation__empty">No qualified usage observation.</p>;
  }
  const expired = isQuotaObservationExpired(observation, now);
  const timestampInFuture = Date.parse(observation.observed_at) > now;
  const unavailable = expired || timestampInFuture;
  return <div className="quota-observation__summary">
    <dl>
      <div><dt>Provider state</dt><dd>{unavailable ? `Unknown · ${expired ? "expired observation" : "future timestamp"}` : STATE_LABEL[observation.state]}</dd></div>
      <div><dt>Source</dt><dd>{observation.source}</dd></div>
      <div><dt>Observed</dt><dd>{timestampInFuture ? "future timestamp" : observedAge(observation.observed_at, now)}</dd></div>
      {observation.expires_at !== null && <div><dt>Expires</dt><dd>{new Date(observation.expires_at).toLocaleString()}</dd></div>}
      {!unavailable && observation.remaining_hint !== null && <div>
        <dt>{observation.remaining_hint.confidence === "ESTIMATED" ? "Estimated remaining usage" : "Provider reported remaining"}</dt>
        <dd>{observation.remaining_hint.quantity} {observation.remaining_hint.unit}
          {observation.remaining_hint.currency ? ` · ${observation.remaining_hint.currency}` : ""}
          {observation.remaining_hint.confidence === "ESTIMATED" ? " · estimate" : " · exact"}
        </dd>
      </div>}
      {!unavailable && observation.reset_at !== null && <div><dt>Resets</dt><dd>{new Date(observation.reset_at).toLocaleString()}</dd></div>}
    </dl>
    <p>Provider data only. Unknown or missing usage is never treated as zero, and this observation cannot switch the lead agent.</p>
  </div>;
}

export function AgentQuotaObservation({ api, agentBindingId }: {
  api: AgentQuotaObservationApi;
  agentBindingId: string;
}) {
  const [open, setOpen] = useState(false);
  const [observation, setObservation] = useState<QuotaObservation | null>(null);
  const [hasObservation, setHasObservation] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(false);
  const [retry, setRetry] = useState(0);

  useEffect(() => {
    if (!open) return;
    const controller = new AbortController();
    let current = true;
    setLoading(true);
    setError(false);
    setHasObservation(false);
    setObservation(null);
    void api.getQuotaObservation(agentBindingId, controller.signal).then(result => {
      if (!current) return;
      setObservation(result);
      setHasObservation(true);
    }).catch(() => {
      if (current && !controller.signal.aborted) setError(true);
    }).finally(() => {
      if (current) setLoading(false);
    });
    return () => {
      current = false;
      controller.abort();
    };
  }, [api, agentBindingId, open, retry]);

  return <section className="quota-observation">
    <button type="button" className="agent-catalog__secondary" aria-expanded={open} onClick={() => setOpen(value => !value)}>
      {open ? "Hide usage observation" : "View usage observation"}
    </button>
    {open && <div className="quota-observation__panel" aria-live="polite">
      {loading ? <p>Checking for a provider usage observation…</p>
        : error ? <div role="alert"><p>Usage observation could not be loaded.</p><button type="button" className="agent-catalog__secondary" onClick={() => setRetry(value => value + 1)}>Retry</button></div>
          : hasObservation ? <QuotaObservationSummary observation={observation} /> : null}
    </div>}
  </section>;
}
