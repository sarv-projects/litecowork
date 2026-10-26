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
const { DiscoverCatalogItemRow } = await vite.ssrLoadModule("/src/discover/DiscoverPage.tsx");
after(async () => vite.close());

const item = {
  item_ref: "skill:skill-1",
  kind: "SKILL",
  display_name: "Invoice cleanup",
  summary: "Clean up an invoice",
  description: null,
  compatibility: "COMPATIBLE",
  compatibility_reason: null,
  requires_user_action: false,
  availability: "AVAILABLE",
  availability_reason: "READY",
  install_state: "INSTALLED",
  auth_state: "NOT_REQUIRED",
  source: "LITECOWORK_LOCAL",
  publisher: "LiteCowork",
  version: "1",
  digest: null,
  grants_required: [],
  authoritative: false,
};

test("Discover explains that installation is unavailable and exposes only an on-demand read-only detail control", () => {
  let detailRequests = 0;
  const api = {
    item: async () => {
      detailRequests += 1;
      return { item, capabilities: [], provider_note: null };
    },
    search: async () => {
      throw new Error("not used");
    },
  };
  const html = renderToStaticMarkup(createElement(DiscoverCatalogItemRow, {
    api,
    item,
    providerStatusId: "discover-package-status",
  }));

  assert.match(html, /<button[^>]*disabled=""[^>]*>Open<\/button>/);
  assert.match(html, /View capabilities/, "row offers a read-only detail view");
  assert.match(html, /aria-expanded="false"/, "details are closed by default");
  assert.match(html, /aria-describedby="discover-package-status discover-action-reason-[^"]+"/);
  assert.match(html, /Opening or running a Workspace skill is not available from Discover\./);
  assert.equal(detailRequests, 0, "catalog details are not fetched until the user opens them");
});
