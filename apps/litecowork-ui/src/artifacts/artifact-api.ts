/** Wire types follow docs/schemas/operator-api.openapi.yaml, not Resource intake views. */
export type PinnedResourceRef = { workspace_id: string; resource_id: string; revision_id: string };
export type ResourceInput = { resource_ref: PinnedResourceRef; observed_digest?: string };
export type ProvenanceRecord = {
  source_inputs: ResourceInput[];
  transformations: { operation: string; inputs: ResourceInput[]; output_digest?: string; capability_ref?: unknown }[];
  tool_reports: PinnedResourceRef[];
  provider_ref?: string;
  created_by_attempt?: string;
  capability_ref?: unknown;
  folder_import?: { relative_path: string } | null;
};
export type Artifact = {
  artifact_id: string; workspace_id: string; resource_id: string; task_id?: string | null;
  kind: string; display_name: string; current_version: number;
  library_status: "TRANSIENT" | "SAVED" | "ARCHIVED"; created_at: string; version: number;
};
export type ArtifactContent = {
  kind: "MANAGED_BLOB";
  storage_ref: { digest: string; size_bytes: number; media_type: string };
  content_digest: string; media_type: string; size_bytes: number;
} | {
  kind: "EXTERNAL_RESOURCE"; resource_ref: PinnedResourceRef;
  provider_revision?: string | null; observed_digest?: string | null; observed_at: string;
};
export type ArtifactVersion = {
  artifact_id: string; version: number; resource_revision_id: string; input_refs: PinnedResourceRef[];
  content: ArtifactContent; provenance: ProvenanceRecord; created_at: string;
  created_by_attempt?: string | null; verification_refs?: string[];
};
export type ArtifactTextEditHead = {
  artifact_id: string;
  aggregate_version: number;
  content_version: number;
  resource_version: number;
  parent_resource_revision_id: string;
};
export type ArtifactTextVersionReceipt = {
  artifact: Artifact;
  version: ArtifactVersion;
  replayed: boolean;
};
export type ArtifactTextVersionInput = {
  expected_content_version: number;
  expected_resource_version: number;
  expected_parent_resource_revision_id: string;
  content: string;
};
export type ArtifactSaveAsRequest = {
  workspace_id: string;
  artifact_id: string;
  version: number;
  resource_revision_id: string;
  content_digest: string;
  size_bytes: number;
  media_type: string;
};
export type ArtifactSaveAsResult = { status: "SAVED" } | { status: "CANCELLED" };
export type ArtifactSaveAsTransport = (request: ArtifactSaveAsRequest) => Promise<ArtifactSaveAsResult>;

/** Inject an authenticated Operator transport. Desktop IPC must adapt its native bridge here.
 * No localhost HTTP assumption, provider URL fetch, or fixture fallback is made. */
export type ArtifactTransport = (path: string, init: RequestInit) => Promise<Response>;

export class ArtifactApiError extends Error {
  constructor(public readonly code: string, public readonly status: number) {
    super(code === "STALE_VERSION" ? "This artifact changed. Refresh before trying again."
      : status === 401 || status === 403 ? "Access to this artifact is unavailable."
      : status === 404 ? "This artifact or version is unavailable."
      : status === 501 ? "Artifact service is not available on this Runtime."
      : "The artifact request could not be completed.");
  }
}

function record(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Invalid Artifact API response.");
  return value as Record<string, unknown>;
}
function str(value: unknown): string {
  if (typeof value !== "string" || value.length === 0) throw new Error("Invalid Artifact API response.");
  return value;
}
function integer(value: unknown, minimum = 1): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < minimum) throw new Error("Invalid Artifact API response.");
  return value;
}
function refs(value: unknown): void {
  if (!Array.isArray(value)) throw new Error("Invalid Artifact API response.");
  for (const ref of value) {
    const item = record(ref);
    str(item.workspace_id); str(item.resource_id); str(item.revision_id);
  }
}
function digest(value: unknown): string {
  const result = str(value);
  if (!/^sha256:[a-f0-9]{64}$/.test(result)) throw new Error("Invalid content digest.");
  return result;
}
export function decodeArtifact(value: unknown): Artifact {
  const item = record(value);
  for (const name of ["artifact_id", "workspace_id", "resource_id", "kind", "display_name", "created_at"]) str(item[name]);
  integer(item.current_version); integer(item.version);
  if (!["TRANSIENT", "SAVED", "ARCHIVED"].includes(str(item.library_status))) throw new Error("Invalid Artifact status.");
  if (item.task_id != null) str(item.task_id);
  return item as Artifact;
}
export function decodeArtifactVersion(value: unknown): ArtifactVersion {
  const item = record(value);
  str(item.artifact_id); integer(item.version); str(item.resource_revision_id); str(item.created_at); refs(item.input_refs);
  const provenance = record(item.provenance);
  const inputs = (value: unknown) => {
    if (!Array.isArray(value)) throw new Error("Invalid provenance inputs.");
    for (const input of value) {
      const row = record(input); refs([row.resource_ref]);
      if (row.observed_digest !== undefined) digest(row.observed_digest);
    }
  };
  inputs(provenance.source_inputs); refs(provenance.tool_reports);
  if (!Array.isArray(provenance.transformations)) throw new Error("Invalid provenance transformations.");
  for (const transformation of provenance.transformations) {
    const row = record(transformation); str(row.operation); inputs(row.inputs);
    if (row.output_digest !== undefined) digest(row.output_digest);
  }
  if (provenance.provider_ref !== undefined) str(provenance.provider_ref);
  if (item.verification_refs !== undefined && (!Array.isArray(item.verification_refs) || item.verification_refs.some(ref => typeof ref !== "string"))) throw new Error("Invalid Evidence references.");
  const content = record(item.content);
  if (content.kind === "MANAGED_BLOB") {
    digest(content.content_digest); str(content.media_type); integer(content.size_bytes, 0);
    const blob = record(content.storage_ref);
    digest(blob.digest); integer(blob.size_bytes, 0); str(blob.media_type);
    if (blob.digest !== content.content_digest || blob.size_bytes !== content.size_bytes || blob.media_type !== content.media_type) throw new Error("Artifact blob metadata does not match.");
  } else if (content.kind === "EXTERNAL_RESOURCE") {
    refs([content.resource_ref]); str(content.observed_at);
    if (content.provider_revision != null) str(content.provider_revision);
    if (content.observed_digest != null) digest(content.observed_digest);
  } else throw new Error("Unsupported Artifact content contract.");
  return item as ArtifactVersion;
}

export class ArtifactApi {
  constructor(private readonly transport: ArtifactTransport, private readonly saveAsTransport?: ArtifactSaveAsTransport) {}
  get supportsNativeSaveAs(): boolean { return this.saveAsTransport !== undefined; }
  private path(id: string): string { return `/v1/artifacts/${encodeURIComponent(id)}`; }
  private async request(path: string, init: RequestInit): Promise<Response> {
    const response = await this.transport(path, { ...init, cache: "no-store" });
    if (!response.ok) {
      let code = "ARTIFACT_REQUEST_FAILED";
      try { const body = record(await response.json()); if (typeof body.code === "string") code = body.code; } catch { /* Error bodies are not shown as markup or raw server text. */ }
      throw new ArtifactApiError(code, response.status);
    }
    return response;
  }
  async get(id: string, signal?: AbortSignal): Promise<Artifact> {
    const item = decodeArtifact(await (await this.request(this.path(id), { signal })).json());
    if (item.artifact_id !== id) throw new Error("Artifact response identity mismatch.");
    return item;
  }
  async list(options: { libraryStatus?: Artifact["library_status"]; savedOnly?: boolean; cursor?: string; limit?: number } = {}, signal?: AbortSignal): Promise<{ items: Artifact[]; next_cursor: string | null }> {
    const query = new URLSearchParams({ limit: String(options.limit ?? 50) });
    if (options.libraryStatus) query.set("library_status", options.libraryStatus);
    if (options.cursor) query.set("cursor", options.cursor);
    const body = record(await (await this.request(`/v1/${options.savedOnly ? "library" : "artifacts"}?${query}`, { signal })).json());
    if (!Array.isArray(body.items) || (body.next_cursor != null && typeof body.next_cursor !== "string")) throw new Error("Invalid Artifact page.");
    return { items: body.items.map(decodeArtifact), next_cursor: body.next_cursor as string | null ?? null };
  }
  async listForTask(taskId: string, workspaceId: string, signal?: AbortSignal): Promise<Artifact[]> {
    str(taskId); str(workspaceId);
    const value: unknown = await (await this.request(`/v1/tasks/${encodeURIComponent(taskId)}/artifacts`, { signal })).json();
    if (!Array.isArray(value) || value.length > 200) throw new Error("Invalid Task Artifact list.");
    const items = value.map(decodeArtifact);
    if (items.some(item => item.task_id !== taskId || item.workspace_id !== workspaceId)) {
      throw new Error("Task Artifact response belongs to a different Task or Workspace.");
    }
    return items;
  }
  async getVersion(id: string, version: number, signal?: AbortSignal): Promise<ArtifactVersion> {
    integer(version);
    const item = decodeArtifactVersion(await (await this.request(`${this.path(id)}/versions/${version}`, { signal })).json());
    if (item.artifact_id !== id || item.version !== version) throw new Error("Artifact version identity mismatch.");
    return item;
  }
  async getTextEditHead(id: string, signal?: AbortSignal): Promise<ArtifactTextEditHead> {
    const item = record(await (await this.request(`${this.path(id)}/edit-head`, { signal })).json());
    const head: ArtifactTextEditHead = {
      artifact_id: str(item.artifact_id),
      aggregate_version: integer(item.aggregate_version),
      content_version: integer(item.content_version),
      resource_version: integer(item.resource_version),
      parent_resource_revision_id: str(item.parent_resource_revision_id),
    };
    if (head.artifact_id !== id) throw new Error("Artifact edit head identity mismatch.");
    return head;
  }
  async appendTextVersion(
    id: string,
    input: ArtifactTextVersionInput,
    expectedArtifactVersion: number,
    requestId: string,
    signal?: AbortSignal,
  ): Promise<ArtifactTextVersionReceipt> {
    // Capture the submitted bytes/heads before awaiting the transport. A receipt
    // resolves this command only when it identifies the exact published text.
    input = { ...input };
    integer(input.expected_content_version);
    integer(input.expected_resource_version);
    str(input.expected_parent_resource_revision_id);
    integer(expectedArtifactVersion);
    str(requestId);
    const submittedBytes = new TextEncoder().encode(input.content);
    if (submittedBytes.byteLength > 1024 * 1024) {
      throw new Error("Text Artifact versions must not exceed 1 MiB.");
    }
    const body = record(await (await this.request(`${this.path(id)}/text-version`, {
      method: "POST",
      headers: { "Content-Type": "application/json", "If-Match": `\"${expectedArtifactVersion}\"`, "Idempotency-Key": requestId },
      body: JSON.stringify(input),
      signal,
    })).json());
    const artifact = decodeArtifact(body.artifact);
    const version = decodeArtifactVersion(body.version);
    if (body.replayed !== true && body.replayed !== false) throw new Error("Invalid Artifact publication receipt.");
    if (artifact.artifact_id !== id || version.artifact_id !== id || version.version !== input.expected_content_version + 1
      || artifact.current_version !== version.version || artifact.version !== expectedArtifactVersion + 1
      || version.content.kind !== "MANAGED_BLOB" || version.content.media_type !== "text/plain"
      || version.content.size_bytes !== submittedBytes.byteLength) {
      throw new Error("Artifact publication receipt does not match the submitted version.");
    }
    const submittedHash = new Uint8Array(await crypto.subtle.digest("SHA-256", submittedBytes));
    const submittedDigest = `sha256:${Array.from(submittedHash, byte => byte.toString(16).padStart(2, "0")).join("")}`;
    if (version.content.content_digest !== submittedDigest) {
      throw new Error("Artifact publication receipt does not match the submitted content.");
    }
    return { artifact, version, replayed: body.replayed };
  }
  async content(version: ArtifactVersion, maxBytes: number, signal?: AbortSignal): Promise<Uint8Array> {
    integer(maxBytes);
    if (version.content.kind === "MANAGED_BLOB" && version.content.size_bytes > maxBytes) throw new Error("Content exceeds the desktop transfer limit.");
    const response = await this.request(`${this.path(version.artifact_id)}/versions/${version.version}/content`, { signal });
    if (!response.body) throw new Error("Artifact content is unavailable.");
    const reader = response.body.getReader();
    const chunks: Uint8Array[] = []; let size = 0;
    try {
      while (true) {
        const chunk = await reader.read();
        if (chunk.done) break;
        size += chunk.value.byteLength;
        if (size > maxBytes) throw new Error("Content exceeds the desktop transfer limit.");
        chunks.push(chunk.value);
      }
    } catch (error) { await reader.cancel().catch(() => undefined); throw error; }
    finally { reader.releaseLock(); }
    const bytes = new Uint8Array(size); let offset = 0;
    for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.byteLength; }
    const expectedDigest = version.content.kind === "MANAGED_BLOB" ? version.content.content_digest : version.content.observed_digest;
    if (version.content.kind === "MANAGED_BLOB" && bytes.byteLength !== version.content.size_bytes) throw new Error("Artifact integrity check failed.");
    if (expectedDigest) {
      const hash = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
      const actual = `sha256:${Array.from(hash, byte => byte.toString(16).padStart(2, "0")).join("")}`;
      if (actual !== expectedDigest) throw new Error("Artifact integrity check failed.");
    }
    return bytes;
  }
  async saveVersionAs(workspaceId: string, artifact: Artifact, version: ArtifactVersion): Promise<ArtifactSaveAsResult> {
    if (!this.saveAsTransport) throw new Error("Native Save As is unavailable on this Operator.");
    if (artifact.workspace_id !== workspaceId || version.artifact_id !== artifact.artifact_id
      || version.version < 1 || version.version > artifact.current_version) {
      throw new Error("Artifact selection changed. Refresh and select the exact version again.");
    }
    if (version.content.kind !== "MANAGED_BLOB" || version.content.size_bytes > 10 * 1024 * 1024) {
      throw new Error("Only managed Artifact versions up to 10 MiB can be saved from this Workbench.");
    }
    return this.saveAsTransport({
      workspace_id: workspaceId,
      artifact_id: artifact.artifact_id,
      version: version.version,
      resource_revision_id: version.resource_revision_id,
      content_digest: version.content.content_digest,
      size_bytes: version.content.size_bytes,
      media_type: version.content.media_type,
    });
  }
  async libraryCommand(snapshot: Artifact, command: "promote" | "archive", idempotencyKey: string): Promise<Artifact> {
    integer(snapshot.version); str(idempotencyKey);
    if (command === "promote" && snapshot.library_status !== "TRANSIENT"
      || command === "archive" && snapshot.library_status === "TRANSIENT") throw new Error("Invalid Artifact Library transition.");
    const response = decodeArtifact(await (await this.request(`${this.path(snapshot.artifact_id)}/${command}`, {
      method: "POST", headers: { "If-Match": `"${snapshot.version}"`, "Idempotency-Key": idempotencyKey },
    })).json());
    const target = command === "promote" ? "SAVED" : "ARCHIVED";
    const expectedVersion = command === "archive" && snapshot.library_status === "ARCHIVED" ? snapshot.version : snapshot.version + 1;
    for (const field of ["artifact_id", "workspace_id", "resource_id", "kind", "display_name", "current_version", "created_at"] as const) {
      if (response[field] !== snapshot[field]) throw new Error("Artifact Library response changed immutable identity or content.");
    }
    if ((response.task_id ?? null) !== (snapshot.task_id ?? null) || response.library_status !== target || response.version !== expectedVersion) {
      throw new Error("Artifact Library response does not match the requested transition.");
    }
    return response;
  }
}

/** Active formats are excluded even when a provider labels them text. */
export function supportsTextPreview(mediaType: string): boolean {
  return /^(text\/(plain|markdown|csv|tab-separated-values|x-[a-z0-9.+-]+)|application\/(json|xml|yaml|x-yaml))$/.test(mediaType.split(";", 1)[0].trim().toLowerCase());
}
