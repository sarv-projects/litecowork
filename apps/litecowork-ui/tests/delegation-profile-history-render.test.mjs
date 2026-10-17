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
