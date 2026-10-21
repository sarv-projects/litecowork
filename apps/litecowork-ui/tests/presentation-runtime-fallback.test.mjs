import assert from "node:assert/strict";
import { after, test } from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import react from "@vitejs/plugin-react";
import { createServer } from "vite";

// The production component and its validator are loaded through Vite so the test exercises
// the real module graph (CSS imports included) rather than a re-implemented copy.
const vite = await createServer({
  configFile: false,
  root: new URL("..", import.meta.url).pathname,
  plugins: [react()],
  server: { middlewareMode: true, hmr: false },
  optimizeDeps: { noDiscovery: true, include: [] },
  appType: "custom",
});
const { PresentationRuntime } = await vite.ssrLoadModule("/src/presentation/PresentationRuntime.tsx");
const { parsePresentationItem, projectPresentationEntries } = await vite.ssrLoadModule(
  "/src/presentation/presentation-types.ts",
);
after(async () => vite.close());

function item(overrides = {}) {
  return {
    item_key: "item-1",
    kind: "TEXT",
    payload_version: 1,
    source_refs: [],
    order_key: "0001",
    freshness: "CURRENT",
    payload: { text: "Saved answer text." },
    ...overrides,
  };
}

function render(items) {
  return renderToStaticMarkup(createElement(PresentationRuntime, { items }));
}

function itemCount(html) {
  return (html.match(/class="presentation-item"/g) ?? []).length;
}

test("renders a positive semantic projection from typed fields in source order", () => {
  const html = render([
    item({
      item_key: "later",
      order_key: "0002",
      kind: "ARTIFACT",
      payload: { artifact_id: "artifact-1", version: 3, display_name: "Quarterly report", artifact_kind: "REPORT" },
    }),
    item({ item_key: "first", order_key: "0001", payload: { text: "The answer is 42." } }),
    item({ item_key: "stale", order_key: "0003", freshness: "STALE", payload: { text: "Older saved note." } }),
  ]);

  assert.equal(itemCount(html), 3);
  assert.match(html, /The answer is 42\./);
  assert.match(html, /Quarterly report/);
  assert.match(html, /version 3/);
  assert.ok(html.indexOf("The answer is 42.") < html.indexOf("Quarterly report"), "items render in source order");
  assert.match(html, /Source freshness stale\./);
});

test("a hostile payload stays inert text and can never become markup or an attribute", () => {
  const hostile = `<img src=x onerror="alert(1)"><script>alert(2)</script>[click](javascript:alert(3))`;
  const html = render([item({ payload: { text: hostile } })]);

  assert.match(html, /&lt;img src=x onerror=/);
  assert.doesNotMatch(html, /<img\b/i);
  assert.doesNotMatch(html, /<script\b/i);
  assert.doesNotMatch(html, /<a\b[^>]*href=/i);
  assert.doesNotMatch(html, /<[a-z]+[^>]*\sonerror=/i);

  // A hostile, unparseable item must not echo its markup-bearing identity as markup either.
  const hostileKind = render([item({ kind: `<svg onload="alert(1)">`, payload: {} })]);
  assert.doesNotMatch(hostileKind, /<svg\b/i);
  assert.doesNotMatch(hostileKind, /<[a-z]+[^>]*\sonload=/i);
  assert.match(hostileKind, /Unsupported content: &lt;svg/);
  assert.equal(itemCount(hostileKind), 1);
  assert.match(hostileKind, /cannot be displayed by this version of LiteCowork|could not be displayed safely/);
});

test("an unsupported payload version is shown as an honest fallback, not dropped", () => {
  const html = render([
    item({ item_key: "future", payload_version: 2 }),
    item({ item_key: "plain", order_key: "0002", payload: { text: "Still readable." } }),
  ]);

  assert.equal(itemCount(html), 2);
  assert.match(html, /version 2/);
  assert.match(html, /Still readable\./);
  assert.doesNotMatch(html, /Saved answer text\./);
});

test("a partial or malformed payload degrades to a visible, non-fabricating fallback", () => {
  const html = render([
    item({ item_key: "broken", payload: { text: 42, forged_status: "APPROVED" } }),
    item({ item_key: "plain", order_key: "0002", payload: { text: "The rest of the answer." } }),
  ]);

  assert.equal(itemCount(html), 2, "an invalid item must not disappear from the transcript");
  assert.match(html, /could not be displayed safely/);
  assert.match(html, /The rest of the answer\./);
  assert.doesNotMatch(html, /forged_status|"text":42|payload_version/i, "no raw payload dump");
  assert.doesNotMatch(html, /APPROVED|Approved/, "an unvalidated payload cannot invent host state");
});

test("a rejected item keeps its safe source references available for review", () => {
  const html = render([
    item({
      source_refs: [{ kind: "TASK", id: "task-1" }, { kind: "NOT_A_SOURCE", id: "forged" }],
      payload: {},
    }),
  ]);

  assert.equal(itemCount(html), 1);
  assert.match(html, /Sources \(1\)/);
  assert.match(html, /task-1/);
  assert.doesNotMatch(html, /forged/);
});

test("projectPresentationEntries reports rejected and omitted items instead of hiding them", () => {
  const valid = item({ item_key: "ok" });
  const invalid = item({ item_key: "bad", payload: { text: null } });
  const projection = projectPresentationEntries([valid, invalid]);

  assert.equal(projection.entries.length, 2);
  assert.equal(projection.rejectedCount, 1);
  assert.equal(projection.omittedCount, 0);
  assert.equal(projection.entries[0].entry, "ITEM");
  assert.equal(projection.entries[1].entry, "REJECTED");

  const bounded = projectPresentationEntries([valid, invalid, item({ item_key: "third" })], 2);
  assert.equal(bounded.entries.length, 2);
  assert.equal(bounded.omittedCount, 1);
});

test("the IPC-boundary validator stays strict while the renderer projection stays honest", () => {
  assert.equal(parsePresentationItem(item({ payload: { text: 42 } })), null);
  assert.equal(parsePresentationItem({ ...item(), payload_version: 0 }), null);
  assert.ok(parsePresentationItem(item()));

  const html = render([item({ payload: { text: 42 } })]);
  assert.equal(itemCount(html), 1);
  assert.match(html, /could not be displayed safely/);
});
