import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import test from "node:test";
import { ArtifactApi } from "../src/artifacts/artifact-api.ts";

const input = {
  expected_content_version: 2, expected_resource_version: 4,
  expected_parent_resource_revision_id: "revision-2", content: "Draft café 🌱\n",
};

function receipt(content = input.content, replayed = false) {
  const size = Buffer.byteLength(content, "utf8");
  const digest = `sha256:${createHash("sha256").update(content, "utf8").digest("hex")}`;
  return {
    artifact: {
      artifact_id: "artifact-1", workspace_id: "workspace-1", resource_id: "resource-1",
      kind: "TEXT", display_name: "Report", current_version: 3, library_status: "SAVED",
      created_at: "2026-10-09T00:00:00Z", version: 6,
    },
    version: {
      artifact_id: "artifact-1", version: 3, resource_revision_id: "revision-3", input_refs: [],
      created_at: "2026-10-09T00:01:00Z",
      content: { kind: "MANAGED_BLOB", content_digest: digest, media_type: "text/plain", size_bytes: size,
        storage_ref: { digest, media_type: "text/plain", size_bytes: size } },
      provenance: { source_inputs: [], transformations: [], tool_reports: [] },
    }, replayed,
  };
}

for (const replayed of [false, true]) {
  test(`accepts exact UTF-8 content receipt (replayed=${replayed})`, async () => {
    const api = new ArtifactApi(async () => Response.json(receipt(input.content, replayed)));
    const committed = await api.appendTextVersion("artifact-1", input, 5, "original-request");
    assert.equal(committed.replayed, replayed);
    assert.equal(committed.version.content.kind, "MANAGED_BLOB");
  });

  test(`rejects different same-size published bytes (replayed=${replayed})`, async () => {
    const different = input.content.replace("Draft", "Other");
    assert.equal(Buffer.byteLength(different), Buffer.byteLength(input.content));
    const api = new ArtifactApi(async () => Response.json(receipt(different, replayed)));
    await assert.rejects(api.appendTextVersion("artifact-1", input, 5, "original-request"), /submitted content/);
  });
}

test("rejects UTF-16 character count used as the published byte size", async () => {
  const invalid = receipt();
  invalid.version.content.size_bytes = input.content.length;
  invalid.version.content.storage_ref.size_bytes = input.content.length;
  const api = new ArtifactApi(async () => Response.json(invalid));
  await assert.rejects(api.appendTextVersion("artifact-1", input, 5, "original-request"), /submitted version/);
});

test("an unconfirmed receipt can be retried with unchanged heads, bytes and request ID", async () => {
  const requests: { headers: Headers; body: string }[] = [];
  const api = new ArtifactApi(async (_path, init) => {
    requests.push({ headers: new Headers(init.headers), body: String(init.body) });
    return Response.json(requests.length === 1 ? receipt("different content") : receipt(input.content, true));
  });
  await assert.rejects(api.appendTextVersion("artifact-1", input, 5, "original-request"));
  assert.equal((await api.appendTextVersion("artifact-1", input, 5, "original-request")).replayed, true);
  assert.equal(requests.length, 2);
  assert.equal(requests[0].body, requests[1].body);
  assert.equal(requests[1].headers.get("If-Match"), '"5"');
  assert.equal(requests[1].headers.get("Idempotency-Key"), "original-request");
});
