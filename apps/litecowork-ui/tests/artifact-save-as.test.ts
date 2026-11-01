import assert from "node:assert/strict";
import test from "node:test";
import { artifactSaveAsStatusMessage } from "../src/artifacts/artifact-api.ts";

test("reports only a confirmed exact-version save as Saved", () => {
  assert.equal(artifactSaveAsStatusMessage({ status: "SAVED" }, 4), "Saved Artifact version 4.");
});

test("reports native picker cancellation as Cancelled", () => {
  assert.equal(artifactSaveAsStatusMessage({ status: "CANCELLED" }, 4), "Save cancelled for Artifact version 4.");
});
