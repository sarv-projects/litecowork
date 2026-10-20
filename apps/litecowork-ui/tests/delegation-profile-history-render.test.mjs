import assert from "node:assert/strict";
import { after, test } from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import react from "@vitejs/plugin-react";
import { createServer } from "vite";

const vite = await createServer({
  configFile: false,
  root: new URL("..", import.meta.url).pathname,
  plugins: [react()],
  server: { middlewareMode: true, hmr: false },
  optimizeDeps: { noDiscovery: true, include: [] },
  appType: "custom",
});
const { DelegationProfileHistory } = await vite.ssrLoadModule("/src/agents/DelegationProfileHistory.tsx");
const { DelegationProfilePerformance, WorkerPerformanceSummary } = await vite.ssrLoadModule("/src/agents/DelegationProfilePerformance.tsx");
after(async () => vite.close());

test("worker profile history starts collapsed and makes no Runtime request", () => {
  let requests = 0;
  const html = renderToStaticMarkup(createElement(DelegationProfileHistory, {
    api: { listRevisions: async () => { requests += 1; return []; } },
    profileId: "profile-1",
    currentRevision: 2,
  }));

  assert.match(html, /aria-expanded="false"/);
  assert.match(html, />Revision history<\/button>/);
  assert.equal(requests, 0);
});

test("worker performance starts collapsed and makes no Runtime request", () => {
  let requests = 0;
  const html = renderToStaticMarkup(createElement(DelegationProfilePerformance, {
    api: { getPerformance: async () => { requests += 1; throw new Error("not requested"); } },
    profileId: "profile-1",
  }));

  assert.match(html, /aria-expanded="false"/);
  assert.match(html, /Show summary/);
  assert.equal(requests, 0);
});

test("low-confidence worker summary keeps missing verification and cost visibly unknown", () => {
  const html = renderToStaticMarkup(createElement(WorkerPerformanceSummary, { projection: {
    delegation_profile_id: "profile-1",
    task_category: "RESEARCH",
    sample_count: 2,
    verifier_pass_rate: null,
    median_latency_ms: 1200,
    median_observed_cost: null,
    retry_rate: null,
    human_intervention_rate: null,
    failure_categories: [],
    confidence: "LOW",
  } }));

  assert.match(html, /Early signal/);
  assert.match(html, /Not enough verified data/);
  assert.match(html, />Unknown</);
  assert.match(html, /never exposes prompts or agent transcripts/i);
});
