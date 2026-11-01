import assert from "node:assert/strict";
import { test } from "node:test";
import { ArtifactRequestEpoch } from "../src/artifacts/artifact-request-epoch.ts";

test("rejects edit responses after switching Workspace or Artifact", () => {
  const requests = new ArtifactRequestEpoch();
  const first = requests.activate("workspace-a", "artifact-a");
  const second = requests.activate("workspace-b", "artifact-b");

  assert.equal(requests.isCurrent(first), false);
  assert.equal(requests.isCurrent(second), true);
});

test("fences an old response when the user leaves and returns to the same Artifact", () => {
  const requests = new ArtifactRequestEpoch();
  const firstVisit = requests.activate("workspace-a", "artifact-a");
  requests.activate("workspace-a", "artifact-b");
  const secondVisit = requests.activate("workspace-a", "artifact-a");

  assert.equal(requests.isCurrent(firstVisit), false);
  assert.equal(requests.isCurrent(secondVisit), true);
});
