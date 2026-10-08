import { invoke } from "@tauri-apps/api/core";
import { ArtifactApi } from "./artifact-api";
import type { ArtifactSaveAsRequest, ArtifactSaveAsResult } from "./artifact-api";

function decodeSaveAsResult(value: unknown): ArtifactSaveAsResult {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Artifact save response is invalid.");
  const result = value as Record<string, unknown>;
  if (result.status === "CANCELLED" && Object.keys(result).join(",") === "status") return { status: "CANCELLED" };
  if (result.status === "SAVED" && Object.keys(result).join(",") === "status") return { status: "SAVED" };
  throw new Error("Artifact save response is invalid.");
}

/** Uses the authenticated native Operator IPC bridge; there is no browser HTTP listener. */
export function desktopArtifactApi(workspaceId: string): ArtifactApi {
  return new ArtifactApi(async (path, init) => {
    if (init.signal?.aborted) throw new DOMException("Aborted", "AbortError");
    const url = new URL(path, "https://operator.invalid");
    const editHead = /^\/v1\/artifacts\/([^/]+)\/edit-head$/.exec(url.pathname);
    const textVersion = /^\/v1\/artifacts\/([^/]+)\/text-version$/.exec(url.pathname);
    const match = /^\/v1\/artifacts\/([^/]+)(?:\/versions\/(\d+)(\/content)?)?$/.exec(url.pathname);
    const task = /^\/v1\/tasks\/([^/]+)\/artifacts$/.exec(url.pathname);
    const method = (init.method ?? "GET").toUpperCase();
    let response: { status: number; contentType: string; bodyBase64: string };
    if (editHead && method === "GET") {
      response = await invoke<typeof response>("artifact_edit_head", {
        workspaceId, artifactId: decodeURIComponent(editHead[1]),
      });
    } else if (textVersion && method === "POST") {
      let input: Record<string, unknown>;
      try {
        const parsed: unknown = JSON.parse(typeof init.body === "string" ? init.body : "");
        if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) throw new Error();
        input = parsed as Record<string, unknown>;
      } catch { throw new Error("Artifact publication request is invalid."); }
      const keys = Object.keys(input).sort();
      if (keys.join(",") !== "content,expected_content_version,expected_parent_resource_revision_id,expected_resource_version"
        || typeof input.content !== "string"
        || typeof input.expected_content_version !== "number" || !Number.isSafeInteger(input.expected_content_version)
        || typeof input.expected_resource_version !== "number" || !Number.isSafeInteger(input.expected_resource_version)
        || typeof input.expected_parent_resource_revision_id !== "string") {
        throw new Error("Artifact publication request is invalid.");
      }
      const headers = new Headers(init.headers);
      const matchHeader = headers.get("If-Match");
      const matchVersion = matchHeader && /^\"([1-9][0-9]*)\"$/.exec(matchHeader);
      const requestId = headers.get("Idempotency-Key");
      if (!matchVersion || !requestId) throw new Error("Artifact publication preconditions are missing.");
      response = await invoke<typeof response>("artifact_append_text_version", {
        workspaceId,
        artifactId: decodeURIComponent(textVersion[1]),
        expectedArtifactVersion: Number(matchVersion[1]),
        expectedContentVersion: input.expected_content_version,
        expectedResourceVersion: input.expected_resource_version,
        expectedParentResourceRevisionId: input.expected_parent_resource_revision_id,
        idempotencyKey: requestId,
        content: input.content,
      });
    } else {
      if (method !== "GET") throw new Error("Artifact Library mutations are not available on this Runtime.");
      const operation = match ? match[2] ? match[3] ? "content" : "version" : "get"
        : task ? "task" : url.pathname === "/v1/artifacts" ? "list" : url.pathname === "/v1/library" ? "library" : null;
      if (!operation) throw new Error("Unsupported Artifact read path.");
      response = await invoke<typeof response>("artifact_read", {
        workspaceId, operation, artifactId: match ? decodeURIComponent(match[1]) : null,
        taskId: task ? decodeURIComponent(task[1]) : null, version: match?.[2] ? Number(match[2]) : null,
        libraryStatus: url.searchParams.get("library_status"), cursor: url.searchParams.get("cursor"),
        limit: url.searchParams.has("limit") ? Number(url.searchParams.get("limit")) : null,
      });
    }
    if (init.signal?.aborted) throw new DOMException("Aborted", "AbortError");
    const binary = atob(response.bodyBase64);
    const bytes = Uint8Array.from(binary, character => character.charCodeAt(0));
    return new Response(bytes, { status: response.status, headers: { "Content-Type": response.contentType } });
  }, async (request: ArtifactSaveAsRequest): Promise<ArtifactSaveAsResult> => {
    if (request.workspace_id !== workspaceId) throw new Error("Artifact selection belongs to another Workspace.");
    const result = await invoke<unknown>("artifact_save_as", {
      workspaceId,
      artifactId: request.artifact_id,
      version: request.version,
      expectedResourceRevisionId: request.resource_revision_id,
      expectedContentDigest: request.content_digest,
      expectedSizeBytes: request.size_bytes,
      expectedMediaType: request.media_type,
    });
    return decodeSaveAsResult(result);
  });
}
