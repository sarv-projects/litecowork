import assert from "node:assert/strict";
import { test } from "node:test";
import { invalidatesVisibleArtifactContent } from "../src/artifacts/artifact-access-state.ts";

test("authorization and non-disclosing missing responses invalidate cached Artifact bytes", () => {
  for (const status of [401, 403, 404]) {
    assert.equal(invalidatesVisibleArtifactContent(Object.assign(new Error("denied"), { status })), true);
  }
});

test("ordinary transport and validation failures preserve an already authorized preview", () => {
  for (const status of [400, 409, 412, 422, 500, undefined]) {
    const error = status === undefined ? new Error("offline") : Object.assign(new Error("request failed"), { status });
    assert.equal(invalidatesVisibleArtifactContent(error), false);
  }
});
