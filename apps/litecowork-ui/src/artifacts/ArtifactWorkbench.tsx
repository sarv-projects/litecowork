import { useEffect, useId, useRef, useState } from "react";
import { ArtifactApi, ArtifactApiError, artifactSaveAsStatusMessage, supportsTextPreview } from "./artifact-api";
import { artifactDownloadFileName } from "./artifact-download-name";
import type { Artifact, ArtifactTextEditHead, ArtifactTextVersionInput, ArtifactTextVersionReceipt, ArtifactVersion, PinnedResourceRef } from "./artifact-api";
import { StructuredTextPreview } from "./StructuredTextPreview";
import { TextVersionDiff } from "./TextVersionDiff";
import "./artifact-workbench.css";

const PREVIEW_LIMIT = 1024 * 1024;
const DOWNLOAD_LIMIT = 10 * 1024 * 1024;
const RECENT_VERSION_LIMIT = 10;
type Props = {
  api: ArtifactApi;
  workspaceId: string;
  artifactId: string;
  onClose?: () => void;
  /** The host must resolve this exact revision through current source authorization. */
  onOpenSource?: (ref: PinnedResourceRef) => void;
  onLibraryChanged?: (artifact: Artifact) => void;
};
type Preview = { text: string; comparison: { text: string; version: ArtifactVersion } | null };
type LoadedEditHead = { head: ArtifactTextEditHead; text: string };
type PendingTextSave = { requestId: string; input: ArtifactTextVersionInput; expectedArtifactVersion: number };
type PendingLibraryCommand = { snapshot: Artifact; action: "promote" | "archive"; requestId: string };
function message(error: unknown): string {
  return error instanceof Error ? error.message : "Artifact service is unavailable.";
}

function isAuthorizationFailure(error: unknown): boolean {
  return error instanceof Error && "status" in error && (error.status === 401 || error.status === 403);
}

/** Artifact versions stay immutable; supported text edits publish a new version. */
export function ArtifactWorkbench({ api, workspaceId, artifactId, onClose, onOpenSource, onLibraryChanged }: Props) {
  const headingId = useId();
  const [refresh, setRefresh] = useState(0);
  const [artifact, setArtifact] = useState<Artifact | null>(null);
  const [selected, setSelected] = useState<number | null>(null);
  const [versionInput, setVersionInput] = useState("");
  const [versionSelectionError, setVersionSelectionError] = useState<string | null>(null);
  const [version, setVersion] = useState<ArtifactVersion | null>(null);
  const [showHistory, setShowHistory] = useState(false);
  const [history, setHistory] = useState<ArtifactVersion[]>([]);
  const [historyLoading, setHistoryLoading] = useState(false);
  const [historyFailure, setHistoryFailure] = useState<string | null>(null);
  const [comparisonVersion, setComparisonVersion] = useState<number | null>(null);
  const [comparisonInput, setComparisonInput] = useState("");
  const [comparisonMode, setComparisonMode] = useState<"SIDE_BY_SIDE" | "LINE_DIFF">("SIDE_BY_SIDE");
  const [comparisonError, setComparisonError] = useState<string | null>(null);
  const [comparisonRetry, setComparisonRetry] = useState(0);
  const [comparisonLoading, setComparisonLoading] = useState(false);
  const [preview, setPreview] = useState<Preview | null>(null);
  const [loading, setLoading] = useState(true);
  const [failure, setFailure] = useState<string | null>(null);
  const [previewFailure, setPreviewFailure] = useState<string | null>(null);
  const [downloadFailure, setDownloadFailure] = useState<string | null>(null);
  const [saveNotice, setSaveNotice] = useState<string | null>(null);
  const [downloading, setDownloading] = useState(false);
  const [copying, setCopying] = useState(false);
  const [copyFailure, setCopyFailure] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);
  const [editHead, setEditHead] = useState<ArtifactTextEditHead | null>(null);
  const [editBase, setEditBase] = useState("");
  const [editDraft, setEditDraft] = useState("");
  const [editLoading, setEditLoading] = useState(false);
  const [editSaving, setEditSaving] = useState(false);
  const [editFailure, setEditFailure] = useState<string | null>(null);
  const [editConflict, setEditConflict] = useState(false);
  const [latestEdit, setLatestEdit] = useState<LoadedEditHead | null>(null);
  const [editRebased, setEditRebased] = useState(false);
  const [restoreSourceVersion, setRestoreSourceVersion] = useState<number | null>(null);
  const [pendingSave, setPendingSave] = useState<PendingTextSave | null>(null);
  const [publishedReceipt, setPublishedReceipt] = useState<ArtifactTextVersionReceipt | null>(null);
  const [publishedRestoreFromVersion, setPublishedRestoreFromVersion] = useState<number | null>(null);
  const downloadController = useRef<AbortController | null>(null);
  const libraryGeneration = useRef(0);
  const libraryInFlight = useRef(false);
  const [libraryBusy, setLibraryBusy] = useState(false);
  const [pendingLibrary, setPendingLibrary] = useState<PendingLibraryCommand | null>(null);
  const [libraryFailure, setLibraryFailure] = useState<string | null>(null);
  const [libraryNotice, setLibraryNotice] = useState<string | null>(null);
  const [libraryNeedsRefresh, setLibraryNeedsRefresh] = useState(false);

  useEffect(() => {
    libraryGeneration.current += 1;
    libraryInFlight.current = false; setLibraryBusy(false); setPendingLibrary(null); setLibraryFailure(null); setLibraryNotice(null); setLibraryNeedsRefresh(false);
    return () => { libraryGeneration.current += 1; };
  }, [api, workspaceId, artifactId]);

  async function changeLibrary(action: "promote" | "archive") {
    if (!artifact || editing || editLoading || editSaving || libraryInFlight.current || libraryNeedsRefresh) return;
    const pending = pendingLibrary ?? { snapshot: artifact, action, requestId: crypto.randomUUID() };
    if (pending.action !== action) return;
    if (!pendingLibrary && !window.confirm(action === "promote"
      ? `Save “${artifact.display_name}” to the Library? This changes its Library status and preserves every content version. Linked content stays with its provider.`
      : `Archive “${artifact.display_name}”? It will leave the Saved Library and cannot receive new versions. History remains readable while authorized. Linked provider content is not deleted.`)) return;
    const generation = libraryGeneration.current;
    libraryInFlight.current = true; setLibraryBusy(true); setLibraryFailure(null); setLibraryNotice(null); setPendingLibrary(pending);
    try {
      const committed = await api.libraryCommand(pending.snapshot, pending.action, pending.requestId);
      if (generation !== libraryGeneration.current) return;
      setPendingLibrary(null); setArtifact(committed);
      setLibraryNotice(action === "promote" ? "Saved to the Library. Content history is unchanged." : "Archived. Content history remains readable while authorized.");
      onLibraryChanged?.(committed);
      // A retry may return an older committed receipt after a different client has
      // advanced the Artifact. Re-read its current head rather than calling that
      // historical response the current Library status.
      try {
        const current = await api.get(artifactId);
        if (generation !== libraryGeneration.current) return;
        if (current.workspace_id !== workspaceId || current.version < committed.version) throw new Error("Artifact current head does not match its confirmed receipt.");
        setArtifact(current); setLibraryNeedsRefresh(false); onLibraryChanged?.(current);
      } catch (refreshError) {
        if (generation !== libraryGeneration.current) return;
        setLibraryFailure(`The Library command is confirmed, but its latest status could not be refreshed. ${message(refreshError)} Refresh before another action.`);
        setLibraryNeedsRefresh(true);
        if (isAuthorizationFailure(refreshError)) { setArtifact(null); setVersion(null); setPreview(null); }
      }
    } catch (error) {
      if (generation !== libraryGeneration.current) return;
      const rejected = error instanceof ArtifactApiError && [400, 401, 403, 404, 409, 412, 422].includes(error.status);
      if (rejected) setPendingLibrary(null);
      if (rejected) setLibraryNeedsRefresh(true);
      setLibraryFailure(`${message(error)} ${rejected ? "Refresh and review its current status before trying again." : "The result is unconfirmed. Retry the same command to resolve it using its original request ID."}`);
      if (isAuthorizationFailure(error)) { setArtifact(null); setVersion(null); setPreview(null); }
    } finally {
      if (generation === libraryGeneration.current) { libraryInFlight.current = false; setLibraryBusy(false); }
    }
  }

  useEffect(() => {
    const controller = new AbortController();
    downloadController.current?.abort();
    setArtifact(null); setSelected(null); setVersionInput(""); setVersionSelectionError(null); setVersion(null); setPreview(null);
    setShowHistory(false); setHistory([]); setHistoryLoading(false); setHistoryFailure(null);
    setFailure(null); setPreviewFailure(null); setDownloadFailure(null); setComparisonVersion(null); setComparisonInput(""); setComparisonMode("SIDE_BY_SIDE"); setComparisonError(null); setLoading(true); setDownloading(false);
    setSaveNotice(null);
    setEditing(false); setEditHead(null); setEditBase(""); setEditDraft(""); setEditFailure(null); setEditConflict(false);
    setLatestEdit(null); setEditRebased(false); setRestoreSourceVersion(null); setPendingSave(null); setEditLoading(false); setEditSaving(false);
    void api.get(artifactId, controller.signal).then(item => {
      if (controller.signal.aborted) return;
      if (item.workspace_id !== workspaceId) throw new Error("Artifact belongs to another Workspace.");
      setArtifact(item); setSelected(item.current_version); setVersionInput(String(item.current_version));
      setLibraryNeedsRefresh(false); setLibraryFailure(null);
      setComparisonInput(String(item.current_version > 1 ? item.current_version - 1 : 1));
    }).catch(error => {
      if (!controller.signal.aborted) { setFailure(message(error)); setLoading(false); }
    });
    return () => { controller.abort(); downloadController.current?.abort(); };
  }, [api, workspaceId, artifactId, refresh]);

  useEffect(() => {
    const controller = new AbortController();
    if (!showHistory || !artifact) return () => controller.abort();
    const first = Math.max(1, artifact.current_version - RECENT_VERSION_LIMIT + 1);
    const versions = Array.from(
      { length: artifact.current_version - first + 1 },
      (_, index) => first + index,
    );
    setHistory([]); setHistoryFailure(null); setHistoryLoading(true);
    void Promise.all(versions.map(number => api.getVersion(artifact.artifact_id, number, controller.signal)))
      .then(items => {
        if (!controller.signal.aborted) setHistory(items.sort((left, right) => right.version - left.version));
      })
      .catch(error => {
        if (!controller.signal.aborted) setHistoryFailure(message(error));
      })
      .finally(() => { if (!controller.signal.aborted) setHistoryLoading(false); });
    return () => controller.abort();
  }, [api, artifact, showHistory]);

  useEffect(() => { setPublishedReceipt(null); setPublishedRestoreFromVersion(null); }, [workspaceId, artifactId]);

  useEffect(() => {
    const controller = new AbortController();
    downloadController.current?.abort(); setDownloading(false); setDownloadFailure(null);
    setComparisonLoading(false);
    setVersion(null); setPreview(null); setPreviewFailure(null); setCopyFailure(null);
    if (selected === null || !artifact) return () => controller.abort();
    setLoading(true); setFailure(null);
    void (async () => {
      try {
        const item = await api.getVersion(artifactId, selected, controller.signal);
        if (controller.signal.aborted) return;
        if (item.artifact_id !== artifactId || item.version !== selected) throw new Error("Artifact version identity mismatch.");
        setVersion(item); setLoading(false);
        if (item.content.kind !== "MANAGED_BLOB" || !supportsTextPreview(item.content.media_type) || item.content.size_bytes > PREVIEW_LIMIT) return;
        const bytes = await api.content(item, PREVIEW_LIMIT, controller.signal);
        const text = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
        if (controller.signal.aborted) return;
        // Keep the selected authorized version readable even if the comparison
        // target is missing or no longer authorized.
        setPreview({ text, comparison: null });
        setPreviewFailure(null);
        if (comparisonVersion === null) return;
        setComparisonLoading(true);
        try {
          const target = await api.loadComparableTextVersion(artifactId, comparisonVersion, artifact.current_version, controller.signal);
          if (!controller.signal.aborted) setPreview({ text, comparison: { text: target.text, version: target.version } });
        } catch (error) {
          if (!controller.signal.aborted) {
            setComparisonError(`Could not compare version ${comparisonVersion}: ${message(error)}`);
          }
        } finally {
          if (!controller.signal.aborted) setComparisonLoading(false);
        }
      } catch (error) {
        if (!controller.signal.aborted) {
          setFailure(message(error)); setLoading(false);
          // The selected version is the Workbench's primary authorized object.
          // A denied read invalidates its metadata; comparison denial does not.
          if (isAuthorizationFailure(error)) { setArtifact(null); setVersion(null); setPreview(null); }
        }
      }
    })();
    return () => controller.abort();
  }, [api, artifactId, artifact, selected, comparisonVersion, comparisonRetry]);

  async function download() {
    if (!version || !artifact || downloading) return;
    const controller = new AbortController(); downloadController.current = controller;
    setDownloading(true); setDownloadFailure(null);
    try {
      const bytes = await api.content(version, DOWNLOAD_LIMIT, controller.signal);
      if (controller.signal.aborted) return;
      const url = URL.createObjectURL(new Blob([new Uint8Array(bytes).buffer], { type: "application/octet-stream" }));
      const link = document.createElement("a");
      link.href = url; link.download = artifactDownloadFileName(artifact.display_name, version.version);
      document.body.append(link); link.click(); link.remove();
      window.setTimeout(() => URL.revokeObjectURL(url), 1000);
    } catch (error) {
      if (!controller.signal.aborted) {
        setDownloadFailure(message(error));
        if (error instanceof Error && "status" in error && (error.status === 401 || error.status === 403)) { setArtifact(null); setVersion(null); setPreview(null); }
      }
    } finally { if (!controller.signal.aborted) setDownloading(false); }
  }

  async function saveVersionAs() {
    if (!version || !artifact || downloading || version.content.kind !== "MANAGED_BLOB"
      || version.content.size_bytes > DOWNLOAD_LIMIT) return;
    setDownloading(true); setDownloadFailure(null); setSaveNotice(null);
    try {
      const result = await api.saveVersionAs(workspaceId, artifact, version);
      setSaveNotice(artifactSaveAsStatusMessage(result, version.version));
    } catch (error) {
      setDownloadFailure(message(error));
    } finally { setDownloading(false); }
  }

  async function copyText() {
    if (!preview || copying) return;
    setCopying(true); setCopyFailure(null);
    try {
      await navigator.clipboard.writeText(preview.text);
    } catch {
      setCopyFailure("Could not copy this text. Check desktop clipboard access and try again.");
    } finally { setCopying(false); }
  }

  async function loadEditableHead(signal?: AbortSignal): Promise<LoadedEditHead> {
    const head = await api.getTextEditHead(artifactId, signal);
    const current = await api.getVersion(artifactId, head.content_version, signal);
    if (current.content.kind !== "MANAGED_BLOB" || current.content.media_type.trim().toLowerCase() !== "text/plain"
      || current.content.size_bytes > PREVIEW_LIMIT) {
      throw new Error("Only managed text/plain Artifacts up to 1 MiB can be edited.");
    }
    const text = new TextDecoder("utf-8", { fatal: true }).decode(await api.content(current, PREVIEW_LIMIT, signal));
    return { head, text };
  }

  async function beginTextEdit() {
    if (editLoading || editing || libraryBusy || pendingLibrary || libraryNeedsRefresh) return;
    setEditLoading(true); setEditFailure(null); setEditConflict(false); setLatestEdit(null); setEditRebased(false); setRestoreSourceVersion(null);
    try {
      const loaded = await loadEditableHead();
      setEditHead(loaded.head); setEditBase(loaded.text); setEditDraft(loaded.text); setPendingSave(null); setEditing(true);
    } catch (error) { setEditFailure(message(error)); }
    finally { setEditLoading(false); }
  }

  async function beginRestoreTextVersion() {
    const source = version;
    const sourceText = preview?.text;
    if (editLoading || editing || libraryBusy || pendingLibrary || libraryNeedsRefresh || !artifact || !source || sourceText === undefined
      || source.version !== selected || source.version >= artifact.current_version || source.artifact_id !== artifactId
      || source.content.kind !== "MANAGED_BLOB"
      || source.content.media_type.trim().toLowerCase() !== "text/plain"
      || source.content.size_bytes > PREVIEW_LIMIT) return;
    const confirmed = window.confirm(
      `Copy the text from version ${source.version} into a draft based on the latest version? This will not overwrite history or publish anything until you choose Publish new version. The current Artifact contract records the result as a user text edit; it does not store a separate restored-from link.`,
    );
    if (!confirmed) return;
    setEditLoading(true); setEditFailure(null); setEditConflict(false); setLatestEdit(null); setEditRebased(false);
    try {
      const loaded = await loadEditableHead();
      if (source.version >= loaded.head.content_version) {
        setVersionSelectionError(`Version ${source.version} is now current. Refresh the Artifact and choose an older version.`);
        return;
      }
      setEditHead(loaded.head);
      setEditBase(loaded.text);
      setEditDraft(sourceText);
      setRestoreSourceVersion(source.version);
      setPendingSave(null);
      setEditing(true);
    } catch (error) { setEditFailure(message(error)); }
    finally { setEditLoading(false); }
  }

  async function checkLatestEditHead() {
    if (editLoading) return;
    setEditLoading(true); setEditFailure(null); setLatestEdit(null);
    try { setLatestEdit(await loadEditableHead()); }
    catch (error) { setEditFailure(message(error)); }
    finally { setEditLoading(false); }
  }

  function rebaseDraft() {
    if (!latestEdit) return;
    setEditHead(latestEdit.head); setEditBase(latestEdit.text); setLatestEdit(null); setPendingSave(null);
    setEditConflict(false); setEditRebased(true); setEditFailure(null);
  }

  async function saveTextVersion() {
    if (!editHead || editSaving || editConflict) return;
    const draftBytes = new TextEncoder().encode(editDraft).byteLength;
    if (draftBytes > PREVIEW_LIMIT) { setEditFailure("Text Artifact versions must not exceed 1 MiB."); return; }
    if (editDraft === editBase && restoreSourceVersion === null && !pendingSave) return;
    const pending = pendingSave && pendingSave.input.content === editDraft ? pendingSave : {
      requestId: crypto.randomUUID(),
      expectedArtifactVersion: editHead.aggregate_version,
      input: {
        expected_content_version: editHead.content_version,
        expected_resource_version: editHead.resource_version,
        expected_parent_resource_revision_id: editHead.parent_resource_revision_id,
        content: editDraft,
      },
    };
    setPendingSave(pending); setEditSaving(true); setEditFailure(null);
    try {
      const receipt = await api.appendTextVersion(artifactId, pending.input, pending.expectedArtifactVersion, pending.requestId);
      setPublishedRestoreFromVersion(restoreSourceVersion);
      setPublishedReceipt(receipt); setEditing(false); setPendingSave(null); setEditConflict(false);
      setRefresh(value => value + 1);
    } catch (error) {
      if (error instanceof Error && "status" in error && error.status === 409) {
        setEditConflict(true);
        setEditFailure("The Artifact changed before this draft could be published. Your draft is preserved. Check the latest version, then explicitly rebase before saving again.");
      } else {
        setEditFailure(`${message(error)} Your draft is preserved. Retrying unchanged will reuse the same request ID.`);
      }
    } finally { setEditSaving(false); }
  }

  function discardTextDraft() {
    if (editDraft !== editBase && !window.confirm("Discard this unpublished text draft?")) return;
    setEditing(false); setEditHead(null); setEditBase(""); setEditDraft(""); setPendingSave(null);
    setEditConflict(false); setLatestEdit(null); setEditFailure(null); setEditRebased(false); setRestoreSourceVersion(null);
  }

  function closeWorkbench() {
    if (!onClose) return;
    if (editing && (editDraft !== editBase || pendingSave !== null)
      && !window.confirm("Discard this unpublished text draft and close the Artifact Workbench?")) return;
    onClose();
  }

  function openVersion(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!artifact) return;
    const value = Number(versionInput);
    if (!Number.isSafeInteger(value) || value < 1 || value > artifact.current_version) {
      setVersionSelectionError(`Choose a version from 1 to ${artifact.current_version}.`);
      return;
    }
    selectVersion(value);
  }

  function compareVersions(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!artifact || selected === null || editing || editLoading || loading) return;
    const value = Number(comparisonInput);
    if (!Number.isSafeInteger(value) || value < 1 || value > artifact.current_version) {
      setComparisonError(`Choose a committed version from 1 to ${artifact.current_version}.`);
      return;
    }
    if (value === selected) {
      setComparisonError("Choose a different version from the one you are viewing.");
      return;
    }
    setComparisonError(null);
    setComparisonVersion(value);
    setComparisonMode("SIDE_BY_SIDE");
    setComparisonRetry(current => current + 1);
  }

  function selectVersion(value: number) {
    setVersionSelectionError(null);
    setVersionInput(String(value));
    setComparisonVersion(null);
    setComparisonMode("SIDE_BY_SIDE");
    setComparisonInput(String(value > 1 ? value - 1 : Math.min(2, artifact?.current_version ?? 1)));
    setComparisonError(null);
    setVersion(null);
    setPreview(null);
    setPreviewFailure(null);
    setSaveNotice(null);
    setLoading(true);
    setSelected(value);
  }

  const textSupported = version?.content.kind === "MANAGED_BLOB" && supportsTextPreview(version.content.media_type) && version.content.size_bytes <= PREVIEW_LIMIT;
  const canEditCurrentText = artifact?.library_status !== "ARCHIVED" && selected === artifact?.current_version
    && version?.content.kind === "MANAGED_BLOB"
    && version.content.media_type.trim().toLowerCase() === "text/plain"
    && version.content.size_bytes <= PREVIEW_LIMIT;
  const canRestoreTextVersion = artifact?.library_status !== "ARCHIVED" && selected !== null
    && selected < (artifact?.current_version ?? 0) && version?.version === selected && preview !== null
    && version?.content.kind === "MANAGED_BLOB"
    && version.content.media_type.trim().toLowerCase() === "text/plain"
    && version.content.size_bytes <= PREVIEW_LIMIT;
  const draftBytes = new TextEncoder().encode(editDraft).byteLength;
  return <section className="artifact-workbench" aria-labelledby={headingId}>
    <header className="artifact-workbench-header">
      <div><p className="artifact-workbench-eyebrow">Artifact Workbench</p><h2 id={headingId}>{artifact?.display_name ?? "Artifact"}</h2>
        {artifact && <p>Viewing version {selected} · Current version {artifact.current_version} · {artifact.library_status.toLowerCase()}</p>}
      </div>
      <div className="artifact-workbench-actions"><button type="button" disabled={editing || editLoading || editSaving || libraryBusy || pendingLibrary !== null} onClick={() => setRefresh(value => value + 1)}>Refresh</button>{onClose && <button type="button" disabled={editLoading || editSaving || pendingSave !== null || libraryBusy || pendingLibrary !== null} onClick={closeWorkbench}>Close</button>}</div>
    </header>
    <p role="status" aria-live="polite">{loading ? "Loading committed version…" : editing ? `Editing a draft based on version ${editHead?.content_version}.` : version ? `Version ${version.version} loaded. Read only.` : "Artifact unavailable."}</p>
    {failure && <p role="alert">{failure}</p>}
    {artifact && <section className="artifact-workbench-library" aria-label="Library status">
      <p>{libraryNeedsRefresh ? "Last confirmed Library status" : "Library status"}: {artifact.library_status.toLowerCase()}. Actions apply to the named Artifact and preserve all content versions.</p>
      <div className="artifact-workbench-actions">
        {artifact.library_status === "TRANSIENT" && <button type="button" disabled={loading || editing || editLoading || editSaving || libraryBusy || pendingLibrary !== null || libraryNeedsRefresh} onClick={() => void changeLibrary("promote")}>Save to Library…</button>}
        {artifact.library_status === "SAVED" && <button type="button" disabled={loading || editing || editLoading || editSaving || libraryBusy || pendingLibrary !== null || libraryNeedsRefresh} onClick={() => void changeLibrary("archive")}>Archive Artifact…</button>}
        {pendingLibrary && <button type="button" disabled={libraryBusy} onClick={() => void changeLibrary(pendingLibrary.action)}>Retry unchanged {pendingLibrary.action === "promote" ? "Library save" : "archive"}</button>}
      </div>
      {artifact.library_status === "ARCHIVED" && <p>Archive is terminal. This Artifact cannot receive new versions.</p>}
      {libraryBusy && <p role="status">Confirming {pendingLibrary?.action === "promote" ? "Library save" : "archive"} with the Runtime…</p>}
      {libraryNotice && <p role="status">{libraryNotice}</p>}
    </section>}
    {libraryFailure && <p role="alert">{libraryFailure}</p>}
    {saveNotice && <p className="artifact-workbench-published" role="status">{saveNotice}</p>}
    {publishedReceipt && <p className="artifact-workbench-published" role="status">{publishedRestoreFromVersion === null ? "Published" : `Restored the text from version ${publishedRestoreFromVersion} as`} version {publishedReceipt.version.version}{publishedReceipt.replayed ? " (confirmed from the original save request)" : ""}. This is a new immutable version; the current Artifact contract records it as a user text edit without a separate restored-from link.</p>}
    {artifact && <nav className="artifact-workbench-actions" aria-label="Version history">
      <form className="artifact-workbench-version-picker" onSubmit={openVersion}>
        <label htmlFor={`${headingId}-version`}>Version</label>
        <input id={`${headingId}-version`} inputMode="numeric" type="number" min="1" max={artifact.current_version} step="1" value={versionInput} disabled={loading || editLoading || editing} aria-invalid={versionSelectionError !== null} aria-describedby={versionSelectionError ? `${headingId}-version-error` : undefined} onChange={event => { setVersionInput(event.target.value); setVersionSelectionError(null); }} />
        <button type="submit" disabled={loading || editLoading || editing || versionInput === String(selected)}>Open</button>
      </form>
      <button type="button" disabled={loading || editLoading || editing || !selected || selected <= 1} onClick={() => { if (selected !== null) selectVersion(selected - 1); }}>Older version</button>
      <button type="button" disabled={loading || editLoading || editing || selected === null || selected >= artifact.current_version} onClick={() => { if (selected !== null) selectVersion(selected + 1); }}>Newer version</button>
      <button type="button" disabled={editLoading || editing} aria-pressed={showHistory} onClick={() => setShowHistory(value => !value)}>{showHistory ? "Hide version history" : "Show version history"}</button>
      {api.supportsNativeSaveAs
        ? <button type="button" disabled={!version || version.content.kind !== "MANAGED_BLOB" || version.content.size_bytes > DOWNLOAD_LIMIT || downloading || editLoading} onClick={() => void saveVersionAs()}>{downloading ? "Saving…" : "Save this version…"}</button>
        : <button type="button" disabled={!version || downloading || editLoading} onClick={() => void download()}>{downloading ? "Downloading…" : "Download this version"}</button>}
    </nav>}
    {artifact && textSupported && artifact.current_version > 1 && <form className="artifact-workbench-version-picker artifact-workbench-comparison-picker" onSubmit={compareVersions}>
      <label htmlFor={`${headingId}-comparison`}>Compare with version</label>
      <input id={`${headingId}-comparison`} inputMode="numeric" type="number" min="1" max={artifact.current_version} step="1"
        value={comparisonInput} disabled={loading || editLoading || editing} aria-invalid={comparisonError !== null}
        aria-describedby={`${headingId}-comparison-help${comparisonError ? ` ${headingId}-comparison-error` : ""}`}
        onChange={event => { setComparisonInput(event.target.value); setComparisonError(null); }} />
      <button type="submit" disabled={loading || comparisonLoading || editLoading || editing || (comparisonVersion === Number(comparisonInput) && preview?.comparison != null)}>{comparisonLoading ? "Loading comparison…" : "Compare"}</button>
      {comparisonVersion !== null && <button type="button" disabled={editLoading || editing} onClick={() => { setComparisonVersion(null); setComparisonError(null); }}>Stop comparison</button>}
      <p id={`${headingId}-comparison-help`}>Choose another committed version. Both text previews are limited to 1 MiB.</p>
    </form>}
    {comparisonLoading && <p role="status">Loading exact comparison version {comparisonVersion}…</p>}
    {comparisonError && <p id={`${headingId}-comparison-error`} role="alert">{comparisonError}</p>}
    {preview?.comparison && <div className="artifact-workbench-comparison-mode" role="group" aria-label="Comparison display">
      <span>Compare view</span>
      <button type="button" disabled={editing || editLoading} aria-pressed={comparisonMode === "SIDE_BY_SIDE"} onClick={() => setComparisonMode("SIDE_BY_SIDE")}>Side by side</button>
      <button type="button" disabled={editing || editLoading} aria-pressed={comparisonMode === "LINE_DIFF"} onClick={() => setComparisonMode("LINE_DIFF")}>Line comparison</button>
    </div>}
    {showHistory && artifact && <section className="artifact-version-history" aria-label="Recent Artifact versions">
      <h3>Recent versions</h3>
      <p>Records are loaded from the authorized immutable version route. Select one to open its exact saved content.</p>
      {artifact.current_version > RECENT_VERSION_LIMIT && <p>Showing the {RECENT_VERSION_LIMIT} most recent versions, through version {artifact.current_version}.</p>}
      {historyLoading && <p role="status">Loading recent version records…</p>}
      {historyFailure && <p role="alert">Could not load version history: {historyFailure}</p>}
      {!historyLoading && !historyFailure && <ol>
        {history.map(item => <li key={item.version}>
          <button type="button" aria-current={item.version === selected ? "true" : undefined} disabled={editLoading || editing || item.version === selected}
            onClick={() => selectVersion(item.version)}>
            <strong>Version {item.version}{item.version === artifact.current_version ? " · Current" : ""}</strong>
            <span>{item.created_at}</span>
          <span>{item.content.kind === "MANAGED_BLOB" ? item.content.media_type : "Linked external content"}</span>
          <span>Output revision {item.resource_revision_id}</span>
          <span>{item.input_refs.length} pinned source{item.input_refs.length === 1 ? "" : "s"} · {item.provenance.provider_ref ?? "No provider recorded"}</span>
            {item.created_by_attempt && <span>Task Attempt {item.created_by_attempt}</span>}
          </button>
        </li>)}
      </ol>}
    </section>}
    {versionSelectionError && <p id={`${headingId}-version-error`} role="alert">{versionSelectionError}</p>}
    {downloadFailure && <p role="alert">{downloadFailure}</p>}
    {version && <>
      {previewFailure && <p role="alert">Preview unavailable: {previewFailure} Download and provenance remain available when authorized.</p>}
      {api.supportsNativeSaveAs && version.content.kind === "EXTERNAL_RESOURCE" && <p>Save As is unavailable for linked external content. Open it through its provider while access remains available.</p>}
      {api.supportsNativeSaveAs && version.content.kind === "MANAGED_BLOB" && version.content.size_bytes > DOWNLOAD_LIMIT && <p>This version exceeds the desktop Save As limit of 10 MiB.</p>}
      {copyFailure && <p role="alert">{copyFailure}</p>}
      {preview && !editing && <div className="artifact-workbench-actions"><button type="button" disabled={copying || editLoading} onClick={() => void copyText()}>{copying ? "Copying…" : "Copy selected text"}</button>
        {canEditCurrentText && <button type="button" disabled={editLoading || libraryBusy || pendingLibrary !== null || libraryNeedsRefresh} onClick={() => void beginTextEdit()}>{editLoading ? "Opening editor…" : "Edit text"}</button>}
        {canRestoreTextVersion && <button type="button" disabled={editLoading || libraryBusy || pendingLibrary !== null || libraryNeedsRefresh} onClick={() => void beginRestoreTextVersion()}>{editLoading ? "Preparing restore…" : "Restore as new version"}</button>}
      </div>}
      {editFailure && !editing && <p role="alert">Could not prepare the text draft: {editFailure}</p>}
      {editing && editHead && <section className="artifact-text-editor" aria-labelledby={`${headingId}-editor-title`}>
        <header><div><h3 id={`${headingId}-editor-title`}>{restoreSourceVersion === null ? "Edit text" : "Restore text as a new version"}</h3><p>{restoreSourceVersion === null ? `Draft from version ${editHead.content_version}. Saving publishes a new immutable version.` : `Draft copied from version ${restoreSourceVersion}, based on current version ${editHead.content_version}. Review it before publishing; the existing versions remain unchanged.`}</p></div>
          <p className={draftBytes > PREVIEW_LIMIT ? "artifact-text-editor-size is-over-limit" : "artifact-text-editor-size"}>{draftBytes.toLocaleString()} / 1,048,576 bytes</p>
        </header>
        {editRebased && <p role="status">Draft rebased on version {editHead.content_version}. Review the updated base and draft before saving.</p>}
        <label htmlFor={`${headingId}-text-draft`}>Text content</label>
        <textarea id={`${headingId}-text-draft`} spellCheck={false} value={editDraft} disabled={editSaving || pendingSave !== null}
          aria-describedby={`${headingId}-editor-help`} onChange={event => { setEditDraft(event.target.value); setPendingSave(null); setEditFailure(null); }} />
        <p id={`${headingId}-editor-help`}>{pendingSave
          ? "The last publish result is unconfirmed. The draft and its original request are locked; retry unchanged to learn whether that immutable version was committed."
          : "Only managed UTF-8 text/plain Artifacts up to 1 MiB can be edited here. Other formats stay read-only."}</p>
        <details><summary>Show the base version used for this draft</summary><pre className="artifact-text-editor-base" tabIndex={0}>{editBase}</pre></details>
        {editFailure && <p role="alert">{editFailure}</p>}
        {editConflict && <div className="artifact-text-editor-conflict">
          <p>This draft was not published. Your text remains in the editor.</p>
          <button type="button" disabled={editLoading} onClick={() => void checkLatestEditHead()}>{editLoading ? "Checking…" : "Check latest version"}</button>
          {latestEdit && <div><p>Latest published version: {latestEdit.head.content_version}. Its text is shown below for review.</p>
            <pre tabIndex={0}>{latestEdit.text}</pre>
            <button type="button" onClick={rebaseDraft}>Rebase my draft on version {latestEdit.head.content_version}</button>
          </div>}
        </div>}
        {!editConflict && pendingSave && <p role="status">A save request may have reached the Runtime. Retry unchanged to confirm it using the same request ID.</p>}
        <div className="artifact-workbench-actions">
          <button type="button" disabled={editSaving || editLoading || editConflict || draftBytes > PREVIEW_LIMIT || (editDraft === editBase && restoreSourceVersion === null && !pendingSave)} onClick={() => void saveTextVersion()}>
            {editSaving ? "Publishing…" : pendingSave ? "Retry unchanged save" : "Publish new version"}
          </button>
          <button type="button" disabled={editSaving || pendingSave !== null} onClick={discardTextDraft}>Discard draft</button>
        </div>
      </section>}
      {preview ? comparisonMode === "LINE_DIFF" && preview.comparison !== null
        ? <TextVersionDiff comparedText={preview.comparison.text} selectedText={preview.text} comparedVersion={preview.comparison.version.version} selectedVersion={version.version} comparedRevisionId={preview.comparison.version.resource_revision_id} selectedRevisionId={version.resource_revision_id} />
        : <div className={`artifact-workbench-preview ${preview.comparison !== null ? "artifact-workbench-compare" : ""}`}>
        {preview.comparison !== null && <div className="artifact-workbench-version-pane">
          <h3>Comparison · Version {preview.comparison.version.version}</h3>
          <p>Output revision <code>{preview.comparison.version.resource_revision_id}</code></p>
          <StructuredTextPreview text={preview.comparison.text} mediaType={preview.comparison.version.content.kind === "MANAGED_BLOB" ? preview.comparison.version.content.media_type : "text/plain"} />
        </div>}
        <div className="artifact-workbench-version-pane">
          <h3>{preview.comparison !== null ? "Selected · " : ""}Version {version.version}</h3>
          <p>Output revision <code>{version.resource_revision_id}</code></p>
          <StructuredTextPreview text={preview.text} mediaType={version.content.kind === "MANAGED_BLOB" ? version.content.media_type : "text/plain"} />
        </div>
        {preview.comparison !== null && <p className="artifact-workbench-compare-note" role="status">Comparing immutable version {preview.comparison.version.version} with selected version {version.version}. This view publishes nothing.</p>}
      </div> : !previewFailure && <p>{textSupported ? "Loading text preview…" : version.content.kind === "EXTERNAL_RESOURCE" ? "Linked source. Content availability depends on its provider." : "Preview unavailable for this format or size. Download the immutable version to open it."}</p>}
      <details open><summary>Provenance and sources</summary>
        <dl><dt>Published</dt><dd>{version.created_at}</dd><dt>Source Task</dt><dd>{artifact?.task_id ?? "No source Task recorded"}</dd>
          {version.content.kind === "MANAGED_BLOB" && <>
            <dt>Content type</dt><dd>{version.content.media_type}</dd>
            <dt>Size</dt><dd>{version.content.size_bytes.toLocaleString()} bytes</dd>
            <dt>Content digest</dt><dd><code>{version.content.content_digest}</code></dd>
          </>}
          {version.content.kind === "EXTERNAL_RESOURCE" && version.content.observed_digest && <><dt>Observed digest</dt><dd><code>{version.content.observed_digest}</code></dd></>}
          <dt>Provider</dt><dd>{version.provenance.provider_ref ?? "No provider recorded"}</dd>
          <dt>Verification</dt><dd>{version.verification_refs?.length ? `${version.verification_refs.length} Evidence references; assurance has not been evaluated by this view.` : "No verification Evidence referenced."}</dd>
          <dt>Output revision</dt><dd>{version.resource_revision_id}</dd>
          {version.content.kind === "EXTERNAL_RESOURCE" && <><dt>External revision</dt><dd>{version.content.provider_revision ?? version.content.resource_ref.revision_id}</dd></>}
        </dl>
        <p>These are historical input references. Opening a source checks current access.</p>
        {version.input_refs.length === 0 ? <p>No source inputs recorded.</p> : <ul>{version.input_refs.map((ref, index) => <li key={`${ref.workspace_id}:${ref.resource_id}:${ref.revision_id}:${index}`}>
          {onOpenSource ? <button type="button" onClick={() => onOpenSource(ref)}>{ref.resource_id} · revision {ref.revision_id}</button> : <span>{ref.resource_id} · revision {ref.revision_id}</span>}
        </li>)}</ul>}
        {version.provenance.transformations.length > 0 && <ul>{version.provenance.transformations.map((transformation, index) => <li key={index}>{transformation.operation}</li>)}</ul>}
      </details>
    </>}
  </section>;
}
