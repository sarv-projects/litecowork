import assert from "node:assert/strict";
import test from "node:test";
import {
  isWorkspaceRootObservationPage,
  mergeWorkspaceRootObservationPage,
  normalizeObservationQuery,
  OBSERVATION_PAGE_LIMIT,
  utf8ByteLength,
  type WorkspaceRootObservation,
  type WorkspaceRootObservationPage,
} from "../src/workspaces/workspace-root-observations.ts";

const WORKSPACE = "workspace-1";
const ROOT = "root-1";
const digest = (character: string) => `sha256:${character.repeat(64)}`;
const resourceId = (character: string) => `res_file_${character.repeat(64)}`;

function observation(character: string, overrides: Partial<WorkspaceRootObservation> = {}): WorkspaceRootObservation {
  return {
    observation_id: digest(character),
    display_name: `file-${character}.txt`,
    size_bytes: 12,
    modified_at: null,
    observed_at: "2026-10-17T10:00:00.000Z",
    freshness: "CURRENT",
    resource_id: resourceId(character),
    ...overrides,
  };
}

function page(
  items: WorkspaceRootObservation[],
  nextCursor: string | null = null,
): WorkspaceRootObservationPage {
  return { workspace_id: WORKSPACE, workspace_root_id: ROOT, items, next_cursor: nextCursor };
}

test("accepts a complete page for the selected root and a strictly advancing cursor", () => {
  const first = page([observation("a"), observation("b")], "cursor-2");
  assert.equal(isWorkspaceRootObservationPage(first, WORKSPACE, ROOT), true);

  const second = page([observation("c")], null);
  assert.equal(isWorkspaceRootObservationPage(second, WORKSPACE, ROOT, "cursor-2"), true);

  assert.equal(OBSERVATION_PAGE_LIMIT, 50);
});

test("legacy observations may omit the canonical Resource link", () => {
  const legacy = page([observation("a", { resource_id: null, modified_at: "2026-10-17T09:00:00Z" })]);
  assert.equal(isWorkspaceRootObservationPage(legacy, WORKSPACE, ROOT), true);
});

test("rejects another Workspace or root, extra fields, and non-object pages", () => {
  assert.equal(isWorkspaceRootObservationPage({ ...page([]), workspace_id: "workspace-2" }, WORKSPACE, ROOT), false);
  assert.equal(isWorkspaceRootObservationPage({ ...page([]), workspace_root_id: "root-2" }, WORKSPACE, ROOT), false);
  assert.equal(isWorkspaceRootObservationPage({ ...page([]), total: 0 }, WORKSPACE, ROOT), false);
  assert.equal(isWorkspaceRootObservationPage(null, WORKSPACE, ROOT), false);
  assert.equal(isWorkspaceRootObservationPage([], WORKSPACE, ROOT), false);
});

test("rejects an empty or repeated continuation cursor instead of looping Load more", () => {
  assert.equal(isWorkspaceRootObservationPage(page([observation("a")], "cursor-2"), WORKSPACE, ROOT, "cursor-2"), false);
  assert.equal(isWorkspaceRootObservationPage(page([observation("a")], ""), WORKSPACE, ROOT), false);
  assert.equal(isWorkspaceRootObservationPage(page([observation("a")], "x".repeat(4097)), WORKSPACE, ROOT), false);
});

test("rejects an empty continuation page that still advertises a cursor", () => {
  assert.equal(isWorkspaceRootObservationPage(page([], "cursor-3"), WORKSPACE, ROOT, "cursor-2"), false);
});

test("accepts an empty continuation page as the end of the list", () => {
  // The last observed file can be renamed or removed between pages. The Operator then
  // reports no further rows and no cursor, which is a truthful end of list.
  assert.equal(isWorkspaceRootObservationPage(page([], null), WORKSPACE, ROOT, "cursor-2"), true);
});

test("rejects duplicate observation identity within one page", () => {
  const duplicated = page([observation("a"), observation("a", { display_name: "other.txt" })]);
  assert.equal(isWorkspaceRootObservationPage(duplicated, WORKSPACE, ROOT), false);
});

test("rejects malformed observation identity, Resource links, metadata, and freshness", () => {
  const invalid: unknown[] = [
    observation("a", { observation_id: "sha256:not-a-digest" }),
    observation("a", { observation_id: `sha256:${"A".repeat(64)}` }),
    observation("a", { resource_id: "res_other_1" }),
    observation("a", { resource_id: `res_file_${"A".repeat(64)}` }),
    observation("a", { display_name: "" }),
    observation("a", { display_name: "nul\0name" }),
    observation("a", { display_name: "n".repeat(1025) }),
    observation("a", { size_bytes: -1 }),
    observation("a", { size_bytes: 1.5 }),
    observation("a", { observed_at: "not-a-timestamp" }),
    observation("a", { modified_at: "not-a-timestamp" }),
    observation("a", { freshness: "FRESH" as WorkspaceRootObservation["freshness"] }),
    { ...observation("a"), private_relative_locator: "etc/passwd" },
  ];
  for (const item of invalid) {
    assert.equal(
      isWorkspaceRootObservationPage(page([item as WorkspaceRootObservation]), WORKSPACE, ROOT),
      false,
      `accepted malformed observation ${JSON.stringify(item)}`,
    );
  }
});

test("merging a continuation page keeps existing rows and never duplicates identity", () => {
  const first = observation("a");
  const second = observation("b");
  const merged = mergeWorkspaceRootObservationPage([first], page([
    { ...first, display_name: "rewritten.txt" },
    second,
  ], "cursor-3"));
  assert.deepEqual(merged.items.map((item) => item.display_name), [first.display_name, second.display_name]);
  assert.equal(merged.nextCursor, "cursor-3");
});

test("replacing the first page drops rows that are no longer returned", () => {
  const merged = mergeWorkspaceRootObservationPage([observation("a")], page([observation("b")]), true);
  assert.deepEqual(merged.items.map((item) => item.observation_id), [digest("b")]);
  assert.equal(merged.nextCursor, null);
});

test("search text is bounded in UTF-8 bytes and rejects control input", () => {
  assert.equal(normalizeObservationQuery("  quarterly report  "), "quarterly report");
  assert.equal(normalizeObservationQuery(""), "");
  assert.equal(normalizeObservationQuery("a\0b"), null);
  const exactly = "é".repeat(128);
  assert.equal(utf8ByteLength(exactly), 256);
  assert.equal(normalizeObservationQuery(exactly), exactly);
  assert.equal(normalizeObservationQuery(`${exactly}é`), null);
});
