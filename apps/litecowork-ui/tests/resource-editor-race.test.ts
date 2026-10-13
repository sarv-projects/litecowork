import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const editorSource = await readFile(new URL("../src/resources/ResourceRevisionEditor.tsx", import.meta.url), "utf8");

function productionFunction(name: string) {
  const declaration = editorSource.match(new RegExp(`^export function ${name}\\(([\\s\\S]*?)\\)(?:\\s*:[^\\{\\n]+)?\\s*\\{([\\s\\S]*?)^\\}`, "m"));
  assert.ok(declaration, `ResourceRevisionEditor must export ${name} as a directly testable navigation guard.`);
  const parameters = declaration[1].split(",").map((parameter) => parameter.trim().split(":", 1)[0].trim()).join(",");
  return new Function(`return function ${name}(${parameters}) {${declaration[2]}}`)() as (...args: unknown[]) => unknown;
}

test("Resource editor requires a deliberate choice before switching away from a dirty Resource", () => {
  const decide = productionFunction("resourceNavigationDecision");
  const current = { workspaceId: "workspace-a", resourceId: "resource-a" };
  const sameResource = { workspaceId: "workspace-a", resourceId: "resource-a" };
  const otherResource = { workspaceId: "workspace-a", resourceId: "resource-b" };

  assert.equal(decide(current, sameResource, "workspace-a", true, false), "SAME");
  assert.equal(decide(current, otherResource, "workspace-a", true, false), "CONFIRM");
  assert.equal(decide(current, otherResource, "workspace-a", false, false), "SWITCH_RESOURCE");
});

test("Resource editor waits for ambiguous status and upload requests before changing targets", () => {
  const decide = productionFunction("resourceNavigationDecision");
  const current = { workspaceId: "workspace-a", resourceId: "resource-a" };
  const otherResource = { workspaceId: "workspace-a", resourceId: "resource-b" };

  assert.equal(decide(current, otherResource, "workspace-a", true, true), "WAIT");
  assert.equal(decide(current, otherResource, "workspace-a", false, true), "WAIT");
});

test("Workspace changes preserve dirty Resource drafts until Save, Discard, or Stay is chosen", () => {
  const decide = productionFunction("resourceNavigationDecision");
  const current = { workspaceId: "workspace-a", resourceId: "resource-a" };

  assert.equal(decide(current, current, "workspace-b", true, false), "CONFIRM");
  assert.equal(decide(current, current, "workspace-b", false, false), "CLOSE_WORKSPACE");
  assert.equal(decide(current, current, "workspace-b", true, true), "WAIT");
});

test("late Resource reads are accepted only for the current Workspace, Resource, and request generation", () => {
  const accepts = productionFunction("isCurrentResourceSelection");
  const selected = { workspaceId: "workspace-b", resourceId: "resource-b" };
  assert.equal(accepts({ workspaceId: "workspace-a", resourceId: "resource-a" }, selected, 3, 4), false);
  assert.equal(accepts(selected, selected, 3, 4), false);
  assert.equal(accepts(selected, selected, 4, 4), true);
});
