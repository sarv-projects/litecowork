import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

/**
 * A-26 empty-state truthfulness guard (docs/V1-UI-SCREEN-CONTRACT.md "Conversation screen").
 *
 * The no-selection / no-message Conversation canvas must render a truthful, LiteCowork-branded
 * empty state. It must NOT carry third-party "Antigravity" branding and must NOT advertise any
 * capability the current build cannot perform. This is the chat-side analogue of
 * workbench-truthfulness.test.mjs; it reads the component source so the removed placeholder
 * cannot silently return.
 */

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const conversationsPagePath = join(packageRoot, "src", "conversations", "ConversationsPage.tsx");

const source = await readFile(conversationsPagePath, "utf8");

/**
 * Isolate the empty-state JSX block so the guard checks what actually renders, not the
 * explanatory comment that documents why the placeholder was removed (which legitimately
 * names "Antigravity" to say it must not appear).
 */
function emptyStateMarkup(src) {
  const anchor = src.indexOf("conversation-empty-state");
  assert.notEqual(anchor, -1, "the empty-state container className conversation-empty-state must exist");
  // Capture a generous window around the rendered block (the <div> body only), excluding comments.
  const windowText = src.slice(anchor, anchor + 1200);
  return windowText.replace(/\{\/\*[\s\S]*?\*\/\}/g, "");
}

const BANNED_BRANDING = ["Antigravity"];

// Capabilities this build cannot perform and must not advertise in an empty chat canvas.
const FABRICATED_CAPABILITY_CLAIMS = [
  "verifiable command",
  "run test suite",
  "inspect terminal output",
  "zero hallucination",
  "Execute verifiable",
];

test("the empty Conversation canvas carries no third-party Antigravity branding", () => {
  const markup = emptyStateMarkup(source);
  for (const banned of BANNED_BRANDING) {
    assert.equal(
      markup.includes(banned),
      false,
      `empty-state markup must not contain third-party branding: ${banned}`,
    );
  }
});

test("the empty Conversation canvas advertises no capability this build cannot perform", () => {
  const markup = emptyStateMarkup(source);
  for (const claim of FABRICATED_CAPABILITY_CLAIMS) {
    assert.equal(
      markup.toLowerCase().includes(claim.toLowerCase()),
      false,
      `empty-state markup must not advertise unsupported capability: ${claim}`,
    );
  }
});

test("the Send control is gated on a dispatch-eligible binding, not merely a non-empty draft", () => {
  // The current UI's dispatch-eligibility gate includes the binding predicate; this does not prove
  // provider authentication, native-session readiness, or the accepted disabled-until-qualified gate.
  assert.match(
    source,
    /disabled=\{!sendEligible\}/,
    "Send button must be disabled unless sendEligible (includes a dispatch-eligible binding) is true",
  );
  assert.match(
    source,
    /bindingEligibleForSend/,
    "send eligibility must incorporate a dispatch-eligibility predicate",
  );
});

test("no fabricated model, title or composer hint copy remains in the Conversation page", () => {
  for (const fabricated of ["Gemini 3.8 Flash Low", "AI Capabilities Overview", "@ to mention, / for actions", "Turbo"]) {
    assert.equal(source.includes(fabricated), false, `fabricated copy must not return: ${fabricated}`);
  }
});

test("composer controls without an owner route are disabled and named unavailable (AC-025)", () => {
  for (const name of ["Voice input (unavailable)", "More options (unavailable)"]) {
    const at = source.indexOf(`aria-label="${name}"`);
    assert.notEqual(at, -1, `${name} must exist as an unavailable control`);
    assert.match(source.slice(at, at + 60), /disabled/, `${name} must be disabled`);
  }
});

test("the attachment control has an owner route and stays fenced to a selected Conversation (001-T14)", () => {
  const at = source.indexOf('aria-label="Add context or file"');
  assert.notEqual(at, -1, "the attach control exists");
  assert.match(source.slice(at, at + 200), /disabled=\{!operatorReady \|\| !selectedId \|\| isTurnRunning\}/);
  assert.match(source, /invoke<\{ items: AttachmentCandidate\[\]; nextCursor: string \| null \}>\("list_resources"/);
  assert.match(source, /resourceRefs: toResourceRefs\(chips, workspaceId\)/);
});

test("Stop and Retry send the projected turn version, never a hard-coded one (AC-024, AC-025)", () => {
  assert.equal(/expectedVersion:\s*1\b/.test(source), false, "expectedVersion must not be hard-coded");
  assert.match(source, /expectedVersion: stopping\.expectedVersion/);
  assert.match(source, /expectedVersion: retrying\.version/);
  assert.match(source, /invoke<unknown>\("retry_conversation_turn"/);
});
