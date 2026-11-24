import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const app = await readFile(new URL("../src/App.tsx", import.meta.url), "utf8");
const conversationPage = await readFile(new URL("../src/conversations/ConversationsPage.tsx", import.meta.url), "utf8");
const conversationStyles = await readFile(new URL("../src/conversations/conversations-page.css", import.meta.url), "utf8");
const globalStyles = await readFile(new URL("../src/styles.css", import.meta.url), "utf8");

function pageUnion() {
  const match = app.match(/type Page = ([^;]+);/);
  assert.ok(match, "Page union present");
  return [...match[1].matchAll(/"([^"]+)"/g)].map(item => item[1]);
}

function navigationLabels() {
  const block = app.match(/export const navigation[\s\S]*?\n\];/);
  assert.ok(block, "navigation array present");
  return [...block[0].matchAll(/label: "([^"]+)"/g)].map(item => item[1]);
}

test("every mounted Page has a navigation path (AC-021)", () => {
  const labels = new Set(navigationLabels());
  for (const page of pageUnion()) {
    assert.ok(labels.has(page), `${page} is in the Page union but has no navigation entry`);
  }
});

test("navigation has no label outside the Page union", () => {
  const pages = new Set(pageUnion());
  for (const label of navigationLabels()) assert.ok(pages.has(label), `${label} is not a Page`);
});

test("Back and Forward are removed because navigation never pushes history (AC-021)", () => {
  assert.doesNotMatch(app, /window\.history\.(back|forward)\(/);
  assert.doesNotMatch(app, /aria-label="(Back|Forward)"/);
  assert.doesNotMatch(app, /history\.pushState|history\.replaceState/);
});

function sidebarSource() {
  const start = app.indexOf('id="main-navigation"');
  const end = app.indexOf("</aside>", start);
  assert.ok(start > 0 && end > start, "sidebar block present");
  return app.slice(start, end);
}

test("every Page is reachable from the rendered sidebar or a documented in-page link (AC-021)", () => {
  const sidebar = sidebarSource();
  const direct = new Set([...sidebar.matchAll(/requestPageNavigation\("([^"]+)"(?:,\s*navigationGuardFocusTarget\(\))?\)/g)].map(item => item[1]));
  // Conversations is the landing page and the New Chat target.
  direct.add("Conversations");
  const more = app.match(/const SIDEBAR_MORE_PAGES: Page\[\] = \[([^\]]*)\]/);
  assert.ok(more, "SIDEBAR_MORE_PAGES present");
  for (const match of more[1].matchAll(/"([^"]+)"/g)) direct.add(match[1]);
  assert.match(sidebar, /navigation\.filter\(\(item\) => SIDEBAR_MORE_PAGES\.includes\(item\.label\)\)\.map/);
  // Routines is reached from the Automations page (requestPageNavigation("Routines") outside the sidebar).
  const inPage = new Set(["Routines"]);
  assert.match(app, /requestPageNavigation\("Routines"\)/);
  for (const page of pageUnion()) {
    assert.ok(direct.has(page) || inPage.has(page), `${page} is not reachable from the sidebar`);
  }
});

test("primary shell keeps workspace entry points compact and secondary domains in More (A-123)", () => {
  const sidebar = sidebarSource();
  assert.match(sidebar, /New Chat/);
  assert.match(sidebar, /Recent Chats/);
  assert.match(sidebar, /requestPageNavigation\("Projects", navigationGuardFocusTarget\(\)\)/);
  assert.match(sidebar, /requestPageNavigation\("Needs You", navigationGuardFocusTarget\(\)\)/);
  assert.match(sidebar, /requestPageNavigation\("Settings", navigationGuardFocusTarget\(\)\)/);
  const more = app.match(/const SIDEBAR_MORE_PAGES: Page\[\] = \[([^\]]*)\]/);
  assert.ok(more, "More page set present");
  for (const page of ["Work", "Coworkers", "Library", "Automations", "Discover", "Dashboard", "Goals", "Notifications", "Ideas", "Home"]) {
    assert.match(more[1], new RegExp(`"${page}"`), `${page} remains reachable under More`);
  }
  assert.doesNotMatch(sidebar, /sidebar-section-count/);
});

test("Projects is a Workspace-backed destination and does not invent a Project API (A-123)", () => {
  assert.match(app, /page === "Projects"/);
  assert.match(app, /<WorkspaceProjectsPage\b/);
  assert.match(app, /list_workspaces/);
  assert.doesNotMatch(app, /create_project|list_projects|ProjectStore/);
});

test("New Chat creates an explicit blank-chat intent and preserves the Library navigation guard", () => {
  const sidebar = sidebarSource();
  assert.match(sidebar, /requestConversationNavigation\(\{ kind: "NEW_CHAT" \}/);
  assert.match(app, /pendingConversationNavigationIntent/);
  assert.match(app, /pendingLibraryNavigation === null/);
  assert.match(conversationPage, /shouldAutoSelectFirstConversation\(externalSelectedId, suppressAutoSelectFirstConversation\.current\)/);
});

test("narrow Conversation has a menu trigger wired to the shared drawer and focus-return ref", () => {
  assert.match(conversationPage, /className="icon-button mobile-navigation-toggle conversation-navigation-toggle"/);
  assert.match(conversationPage, /aria-controls="main-navigation"/);
  assert.match(conversationPage, /aria-haspopup="dialog"/);
  assert.match(conversationPage, /aria-expanded=\{navigationOpen\}/);
  assert.match(conversationPage, /ref=\{navigationTriggerRef\}/);
  assert.match(conversationPage, /onClick=\{onOpenNavigation\}/);
  assert.match(app, /onOpenNavigation=\{\(\) => \{ setWorkbenchOpen\(false\); setNavigationOpen\(true\); \}\}/);
  assert.match(app, /navigationTriggerRef=\{navigationTriggerRef\}/);
  assert.match(globalStyles, /mobile-navigation-toggle:not\(\.conversation-navigation-toggle\)/);
  assert.match(conversationStyles, /\.conversation-top-breadcrumbs \.conversation-navigation-toggle\s*\{\s*display: none;\s*\}/);
  assert.match(conversationStyles, /@media \(max-width: 900px\)[\s\S]*?\.conversation-top-breadcrumbs \.conversation-navigation-toggle\s*\{[\s\S]*?display: inline-flex/);
  assert.match(app, /if \(window\.innerWidth <= 900\) navigationTriggerRef\.current\?\.focus\(\)/);
});

test("More is a keyboard disclosure that keeps the active secondary destination labeled", () => {
  const sidebar = sidebarSource();
  assert.match(sidebar, /<details\s+className="sidebar-more"/);
  assert.match(sidebar, /onToggle=\{event => setSidebarMoreOpen\(event\.currentTarget\.open\)\}/);
  assert.match(sidebar, /More\{SIDEBAR_MORE_PAGES\.includes\(page\) \? ` · \$\{navigation\.find/);
  assert.match(sidebar, /aria-current=\{page === item\.label \? "page" : undefined\}/);
});

test("App.tsx has no duplicated Operator-unavailable page bodies; one shared panel is used (011-T20)", () => {
  assert.doesNotMatch(app, /<h2>Local Operator unavailable<\/h2>/);
  assert.ok((app.match(/<OperatorUnavailablePanel\b/g) ?? []).length >= 6);
});

test("LibraryPage lives in its own module and App imports it", () => {
  assert.doesNotMatch(app, /\nfunction LibraryPage\(/);
  assert.match(app, /import \{ LibraryPage \} from "\.\/library\/LibraryPage"/);
});
