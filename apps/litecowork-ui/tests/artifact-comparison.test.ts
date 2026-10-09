import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import test from "node:test";
import { ArtifactApi } from "../src/artifacts/artifact-api.ts";

function digest(bytes: Uint8Array): string {
  return `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
}

function versionRecord(version: number, bytes: Uint8Array, mediaType = "text/plain") {
  const contentDigest = digest(bytes);
  return {
    artifact_id: "artifact-1", version, resource_revision_id: `revision-${version}`, input_refs: [],
    created_at: `2026-10-0${version}T00:00:00Z`,
    content: { kind: "MANAGED_BLOB", storage_ref: { digest: contentDigest, media_type: mediaType, size_bytes: bytes.byteLength },
      content_digest: contentDigest, media_type: mediaType, size_bytes: bytes.byteLength },
    provenance: { source_inputs: [], transformations: [], tool_reports: [] },
  };
}

test("loads a comparison from the exact immutable version routes and verifies its bytes", async () => {
  const text = new TextEncoder().encode("historical version\n");
  const requests: string[] = [];
  const api = new ArtifactApi(async path => {
    requests.push(path);
    if (path.endsWith("/versions/2")) return Response.json(versionRecord(2, text));
    if (path.endsWith("/versions/2/content")) return new Response(text, { headers: { "content-type": "text/plain" } });
    return new Response(null, { status: 404 });
  });

  const result = await api.loadComparableTextVersion("artifact-1", 2, 4);

  assert.equal(result.version.version, 2);
  assert.equal(result.version.resource_revision_id, "revision-2");
  assert.equal(result.text, "historical version\n");
  assert.deepEqual(requests, [
    "/v1/artifacts/artifact-1/versions/2",
    "/v1/artifacts/artifact-1/versions/2/content",
  ]);
});

test("rejects a comparison outside the Artifact snapshot without making a request", async () => {
  let requests = 0;
  const api = new ArtifactApi(async () => { requests += 1; return new Response(null, { status: 404 }); });

  await assert.rejects(api.loadComparableTextVersion("artifact-1", 5, 4), /committed version/);
  assert.equal(requests, 0);
});

test("does not compare unsupported media or bytes that fail the pinned digest", async () => {
  const pdf = new TextEncoder().encode("%PDF-1.7");
  const api = new ArtifactApi(async path => path.endsWith("/versions/1")
    ? Response.json(versionRecord(1, pdf, "application/pdf"))
    : new Response(pdf, { headers: { "content-type": "application/pdf" } }));
  await assert.rejects(api.loadComparableTextVersion("artifact-1", 1, 2), /text comparison/);

  const expected = new TextEncoder().encode("correct bytes");
  const corrupt = new TextEncoder().encode("altered bytes");
  const corruptApi = new ArtifactApi(async path => path.endsWith("/versions/1")
    ? Response.json(versionRecord(1, expected))
    : new Response(corrupt, { headers: { "content-type": "text/plain" } }));
  await assert.rejects(corruptApi.loadComparableTextVersion("artifact-1", 1, 2), /integrity/);
});

test("rejects invalid UTF-8 and metadata for a different Artifact", async () => {
  const invalidUtf8 = new Uint8Array([0xc3, 0x28]);
  const invalidApi = new ArtifactApi(async path => path.endsWith("/versions/1")
    ? Response.json(versionRecord(1, invalidUtf8))
    : new Response(invalidUtf8, { headers: { "content-type": "text/plain" } }));
  await assert.rejects(invalidApi.loadComparableTextVersion("artifact-1", 1, 2), /valid UTF-8/);

  const bytes = new TextEncoder().encode("other artifact");
  const foreign = versionRecord(1, bytes);
  foreign.artifact_id = "artifact-foreign";
  const foreignApi = new ArtifactApi(async () => Response.json(foreign));
  await assert.rejects(foreignApi.loadComparableTextVersion("artifact-1", 1, 2), /identity mismatch/);
});

test("rejects content served with a media type different from the immutable version", async () => {
  const bytes = new TextEncoder().encode("still the expected digest");
  const api = new ArtifactApi(async path => path.endsWith("/versions/1")
    ? Response.json(versionRecord(1, bytes, "text/plain"))
    : new Response(bytes, { headers: { "content-type": "application/pdf" } }));

  await assert.rejects(api.loadComparableTextVersion("artifact-1", 1, 2), /media type/);
});
