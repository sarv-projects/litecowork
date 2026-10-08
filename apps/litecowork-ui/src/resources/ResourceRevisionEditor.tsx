import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./resource-revision-editor.css";

type Resource = {
  resourceId: string;
  workspaceId: string;
  resourceRevisionId: string;
  displayName: string;
  mediaType: string;
  contentDigest: string;
  sizeBytes: number;
};
type ContextDocument = { kind: string; status: string; owner_ref?: { kind?: string; workspace_id?: string; principal_id?: string; coworker_id?: string; goal_id?: string } };
type Detail = { resourceId: string; workspaceId: string; kind: string; displayName: string; currentRevisionId: string | null; version: number; contextDocument: ContextDocument | null };
type Revision = { resourceRevisionId: string; resourceId: string; parentRevisionIds: string[]; contentDigest: string | null; sizeBytes: number | null; mediaType: string | null; observedAt: string; isHead: boolean };
type RevisionPage = { items: Revision[]; nextCursor: string | null };
type UploadSession = { uploadId: string; workspaceId: string; displayName: string; mediaType: string; expectedSizeBytes: number; expectedDigest: string | null; chunkSizeBytes: number; nextMissingOffset: number; state: string; resourceId: string | null; expectedResourceVersion: number | null; parentRevisionIds: string[] };

const MAX_REVISION_BYTES = 100 * 1024 * 1024;
const MAX_EDIT_TEXT_BYTES = 1024 * 1024;

export function ResourceRevisionEditor({ resource, onClose, onCommitted }: { resource: Resource; onClose: () => void; onCommitted: (resource: Resource) => void }) {
  const [detail, setDetail] = useState<Detail | null>(null);
  const [history, setHistory] = useState<Revision[]>([]);
  const [cursor, setCursor] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(true);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const [draftText, setDraftText] = useState<string | null>(null);
  const [uploadBytes, setUploadBytes] = useState(0);

  const reload = useCallback(async (append = false) => {
    setLoading(true);
    setError(null);
    try {
      const [nextDetail, page] = await Promise.all([
        invoke<Detail>("get_resource_detail", { workspaceId: resource.workspaceId, resourceId: resource.resourceId }),
        invoke<RevisionPage>("list_resource_revisions", { workspaceId: resource.workspaceId, resourceId: resource.resourceId, cursor: append ? cursor : null }),
      ]);
      if (nextDetail.workspaceId !== resource.workspaceId || nextDetail.resourceId !== resource.resourceId) throw new Error("Resource response did not match this Workspace.");
      setDetail(nextDetail);
      setHistory((current) => append ? [...current, ...page.items] : page.items);
      setCursor(page.nextCursor);
    } catch (cause) {
      setError(messageOf(cause, "Resource details and revision history could not be loaded."));
    } finally {
      setLoading(false);
    }
  }, [cursor, resource.resourceId, resource.workspaceId]);

  useEffect(() => { void reload(false); }, [resource.resourceId, resource.workspaceId]);

  const context = detail?.contextDocument ?? null;
  const isCurrentTextEditable = isSmallText(resource.mediaType, resource.sizeBytes);

  const openTextEditor = async () => {
    if (!detail?.currentRevisionId || !isCurrentTextEditable) return;
    setBusy(true); setError(null); setMessage(null);
    try {
      const text = await invoke<string>("preview_resource_text", {
        workspaceId: resource.workspaceId,
        resourceId: resource.resourceId,
        revisionId: detail.currentRevisionId,
      });
      setDraftText(text);
      setSelectedFile(null);
    } catch (cause) {
      setError(messageOf(cause, "Current text could not be opened for editing."));
    } finally { setBusy(false); }
  };

  const saveRevision = async (file: File) => {
    if (!detail || !context || context.status !== "ACTIVE" || !detail.currentRevisionId) return;
    if (file.size > MAX_REVISION_BYTES) { setError("A revision file cannot exceed 100 MiB."); return; }
    setBusy(true); setError(null); setMessage(null); setUploadBytes(0);
    try {
      const bytes = await file.arrayBuffer();
      const expectedDigest = `sha256:${await digestHex(bytes)}`;
      const parentRevisionIds = [detail.currentRevisionId];
      const session = await invoke<UploadSession>("create_resource_revision_upload", {
        workspaceId: resource.workspaceId,
        resourceId: resource.resourceId,
        expectedResourceVersion: detail.version,
        parentRevisionIds,
        mediaType: file.type || resource.mediaType || "application/octet-stream",
        sizeBytes: file.size,
        expectedDigest,
        requestId: crypto.randomUUID(),
      });
      if (session.resourceId !== resource.resourceId || session.expectedResourceVersion !== detail.version
        || session.parentRevisionIds.length !== 1 || session.parentRevisionIds[0] !== detail.currentRevisionId
        || session.expectedDigest !== expectedDigest || session.expectedSizeBytes !== file.size) {
        throw new Error("Upload session did not preserve the selected Resource version and parent.");
      }
      if (session.chunkSizeBytes <= 0 || session.chunkSizeBytes > 4 * 1024 * 1024) throw new Error("Local Runtime returned an unsupported chunk size.");
      const bytesView = new Uint8Array(bytes);
      if (session.nextMissingOffset !== 0) throw new Error("The new revision upload session was not empty. No chunks were sent.");
      for (let offset = 0, index = 0; offset < file.size; offset += session.chunkSizeBytes, index += 1) {
        const end = Math.min(offset + session.chunkSizeBytes, file.size);
        const chunk = bytesView.slice(offset, end);
        await invoke("upload_resource_chunk", {
          workspaceId: resource.workspaceId,
          uploadId: session.uploadId,
          chunkIndex: index,
          contentRange: `bytes ${offset}-${end - 1}/${file.size}`,
          chunkSha256: await digestHex(chunk.buffer),
          contentBase64: bytesToBase64(chunk),
          requestId: crypto.randomUUID(),
        });
        setUploadBytes(end);
      }
      setMessage("All selected bytes uploaded. Committing the new revision…");
      const committed = await invoke<Resource>("commit_resource_revision_upload", {
        workspaceId: resource.workspaceId,
        uploadId: session.uploadId,
        requestId: crypto.randomUUID(),
      });
      setSelectedFile(null); setDraftText(null);
      onCommitted(committed);
      await reload(false);
      setMessage("New Resource revision saved.");
    } catch (cause) {
      const text = messageOf(cause, "Resource revision could not be saved.");
      setError(text);
      if (text.includes("RESOURCE_CONFLICT")) {
        setSelectedFile(null);
        setDraftText(null);
        setMessage("No revision was committed. Current metadata and history have been refreshed. Choose the content again after reviewing the current head; LiteCowork will not rebase your edit automatically.");
        await reload(false);
      }
    } finally { setBusy(false); }
  };

  const saveText = async () => {
    if (draftText === null) return;
    const encoded = new TextEncoder().encode(draftText);
    if (encoded.byteLength > MAX_EDIT_TEXT_BYTES) { setError("Text edits are limited to 1 MiB."); return; }
    const blob = new Blob([encoded], { type: resource.mediaType || "text/plain" });
    await saveRevision(new File([blob], resource.displayName, { type: resource.mediaType || "text/plain" }));
  };

  return <section className="resource-revision-panel" aria-labelledby="resource-revision-title">
    <header className="resource-revision-header">
      <div><p className="eyebrow">RESOURCE HISTORY</p><h2 id="resource-revision-title">{resource.displayName}</h2><p>Each save creates a new immutable revision.</p></div>
      <button className="quiet-button" type="button" onClick={onClose} disabled={busy}>Close</button>
    </header>
    {loading && !detail ? <p className="inline-status" role="status">Loading Resource metadata…</p> : null}
    {error && <p className="inline-error" role="alert">{error}</p>}
    {message && <p className="inline-status" role="status">{message}</p>}
    {detail && <>
      <div className="resource-revision-meta">
        <span><strong>Kind</strong>{detail.kind}</span>
        <span><strong>Resource version</strong>{detail.version}</span>
        <span><strong>Current revision</strong>{detail.currentRevisionId ?? "Conflicted or unavailable"}</span>
        <span><strong>Context status</strong>{context?.status ?? "Not a ContextDocument"}</span>
      </div>
      {context ? <p className="resource-context-owner">{context.kind} · owner-authored ContextDocument</p> : <p className="inline-status">Revision editing is available only for owner-authored ContextDocuments.</p>}
      {context && context.status !== "ACTIVE" && <p className="inline-status">This ContextDocument is {context.status.toLowerCase()}. Editing is unavailable. Revocation and deletion controls are not available in this desktop slice.</p>}
      {context?.status === "ACTIVE" && !detail.currentRevisionId && <p className="inline-status">This Resource has conflicting or unavailable revision heads. Resolve the conflict before editing; this screen will not select a branch for you.</p>}
      {context?.status === "ACTIVE" && detail.currentRevisionId && <div className="resource-revision-edit">
        <h3>Create a revision</h3>
        <p>New uploads are pinned to Resource version {detail.version} and current head {detail.currentRevisionId}. A concurrent change blocks commit.</p>
        <label className="resource-revision-file">Choose a revised local file
          <input type="file" disabled={busy} onChange={(event) => { setSelectedFile(event.currentTarget.files?.[0] ?? null); setDraftText(null); setError(null); }} />
        </label>
        {selectedFile && <div className="resource-revision-staged"><span>{selectedFile.name} · {formatBytes(selectedFile.size)}</span><button className="primary-button" type="button" disabled={busy} onClick={() => void saveRevision(selectedFile)}>{busy ? `Uploading ${formatBytes(uploadBytes)} of ${formatBytes(selectedFile.size)}…` : "Upload as new revision"}</button></div>}
        {!selectedFile && isCurrentTextEditable && <button className="quiet-button" type="button" disabled={busy} onClick={() => void openTextEditor()}>{draftText === null ? "Edit small text file" : "Reload current text"}</button>}
        {draftText !== null && <div className="resource-revision-text-editor"><label htmlFor="resource-revision-text">Edit content (up to 1 MiB)</label><textarea id="resource-revision-text" value={draftText} disabled={busy} onChange={(event) => setDraftText(event.currentTarget.value)} spellCheck={false} /><div className="resource-revision-actions"><span>{formatBytes(new TextEncoder().encode(draftText).byteLength)} · unsaved draft</span><button className="primary-button" type="button" disabled={busy} onClick={() => void saveText()}>{busy ? `Uploading ${formatBytes(uploadBytes)}…` : "Save as new revision"}</button></div></div>}
        {busy && <p className="inline-status" role="status">Uploading {formatBytes(uploadBytes)} of {formatBytes(selectedFile?.size ?? new TextEncoder().encode(draftText ?? "").byteLength)}. This transfer uses the authenticated local Runtime.</p>}
      </div>}
    </>}
    <div className="resource-revision-history">
      <div className="section-heading"><div><h3>Revision history</h3><p>Parent IDs show the exact ancestry pinned by each revision.</p></div><button className="quiet-button" type="button" disabled={loading || busy} onClick={() => void reload(false)}>Refresh</button></div>
      {history.length === 0 && !loading ? <p className="empty-inline">No revision history is available.</p> : <ol>{history.map((revision) => <li key={revision.resourceRevisionId}>
        <div><strong>{revision.isHead ? "Current head" : "Revision"} · {revision.resourceRevisionId}</strong><small>{revision.observedAt} · {formatBytes(revision.sizeBytes ?? 0)} · {revision.mediaType ?? "Unknown media type"}</small><small>Parents: {revision.parentRevisionIds.length ? revision.parentRevisionIds.join(", ") : "None (initial revision)"}</small></div>
      </li>)}</ol>}
      {cursor && <button className="quiet-button" type="button" disabled={loading || busy} onClick={() => void reload(true)}>Load more history</button>}
    </div>
    <div className="resource-revision-unavailable-actions" aria-label="Unavailable ContextDocument actions">
      <span>Revocation</span><span>Unavailable</span><span>Deletion</span><span>Unavailable</span>
    </div>
  </section>;
}

function isSmallText(mediaType: string, sizeBytes: number): boolean {
  const type = mediaType.toLowerCase().split(";", 1)[0].trim();
  return sizeBytes <= MAX_EDIT_TEXT_BYTES && (type.startsWith("text/") || ["application/json", "application/xml", "application/yaml", "application/x-yaml", "application/javascript"].includes(type));
}

async function digestHex(buffer: ArrayBuffer): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", buffer);
  return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("");
}

function bytesToBase64(bytes: Uint8Array): string {
  let binary = "";
  for (let offset = 0; offset < bytes.length; offset += 0x8000) binary += String.fromCharCode(...bytes.subarray(offset, Math.min(offset + 0x8000, bytes.length)));
  return btoa(binary);
}

function formatBytes(size: number): string {
  if (!size) return "0 B";
  return size < 1024 ? `${size} B` : size < 1024 * 1024 ? `${Math.round(size / 1024)} KB` : `${(size / (1024 * 1024)).toFixed(1)} MB`;
}

function messageOf(error: unknown, fallback: string): string {
  return typeof error === "string" ? error : error instanceof Error ? error.message : fallback;
}
