import { useEffect, useState, type SyntheticEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./task-resource-text-preview.css";

export type PinnedTaskResourceRef = {
  workspaceId: string;
  resourceId: string;
  revisionId: string;
  displayName?: string;
};

type PreviewState =
  | { state: "idle" }
  | { state: "loading"; key: string }
  | { state: "ready"; key: string; text: string }
  | { state: "error"; key: string; message: string };

const MAX_PREVIEW_BYTES = 1024 * 1024;

/** Read-only, on-demand preview of one exact Task input revision. */
export function TaskResourceTextPreview({
  workspaceId,
  input,
}: {
  workspaceId: string;
  input: PinnedTaskResourceRef;
}) {
  const [open, setOpen] = useState(false);
  const [retry, setRetry] = useState(0);
  const [preview, setPreview] = useState<PreviewState>({ state: "idle" });
  const pinKey = JSON.stringify([workspaceId, input.workspaceId, input.resourceId, input.revisionId]);

  useEffect(() => {
    if (!open) return;
    let current = true;

    if (!workspaceId || input.workspaceId !== workspaceId || !input.resourceId || !input.revisionId) {
      setPreview({ state: "error", key: pinKey, message: "This Task input does not have a valid pin in the selected Workspace." });
      return () => { current = false; };
    }

    setPreview({ state: "loading", key: pinKey });
    void invoke<unknown>("preview_resource_text", {
      workspaceId,
      resourceId: input.resourceId,
      revisionId: input.revisionId,
    }).then((value) => {
      if (!current) return;
      if (typeof value !== "string") throw new Error("Local Runtime returned an unsupported text preview.");
      if (new TextEncoder().encode(value).byteLength > MAX_PREVIEW_BYTES) {
        throw new Error("Text preview is limited to 1 MiB.");
      }
      setPreview({ state: "ready", key: pinKey, text: value });
    }).catch((cause: unknown) => {
      if (!current) return;
      setPreview({ state: "error", key: pinKey, message: previewErrorMessage(cause) });
    });

    return () => { current = false; };
  }, [input.resourceId, input.revisionId, input.workspaceId, open, pinKey, retry, workspaceId]);

  const onDisclosureToggle = (event: SyntheticEvent<HTMLDetailsElement>) => {
    setOpen(event.currentTarget.open);
  };

  const title = input.displayName?.trim() || "Resource";
  const pinIsValid = Boolean(workspaceId && input.workspaceId === workspaceId && input.resourceId && input.revisionId);
  const visiblePreview = preview.state !== "idle" && preview.key === pinKey
    ? preview
    : open ? { state: "loading" as const, key: pinKey } : { state: "idle" as const };

  return <details className="task-resource-text-preview" onToggle={onDisclosureToggle}>
    <summary>Preview pinned text · {title}</summary>
    <p className="task-resource-text-preview__pin">
      Exact Task input: <code>resource://{input.workspaceId || "unavailable"}/{input.resourceId || "unavailable"}@{input.revisionId || "unavailable"}</code>
    </p>
    <p className="task-resource-text-preview__notice">
      Read-only preview of this Task input. It does not attach content to an agent or change the saved Task.
    </p>
    {!pinIsValid && <p className="task-resource-text-preview__error" role="alert">
      This Task input does not have a valid pin in the selected Workspace.
    </p>}
    {visiblePreview.state === "loading" && <p className="task-resource-text-preview__status" role="status">Loading the exact pinned revision…</p>}
    {visiblePreview.state === "error" && <div className="task-resource-text-preview__error" role="alert">
      <p>{visiblePreview.message}</p>
      <button type="button" className="quiet-button" disabled={!pinIsValid} onClick={() => setRetry((value) => value + 1)}>
        Retry this pinned revision
      </button>
    </div>}
    {visiblePreview.state === "ready" && <pre className="task-resource-text-preview__content" tabIndex={0} aria-label={`Read-only text preview of ${title}, revision ${input.revisionId}`}>
      {visiblePreview.text}
    </pre>}
  </details>;
}

function previewErrorMessage(cause: unknown): string {
  const raw = cause instanceof Error ? cause.message : typeof cause === "string" ? cause : "";
  const message = raw.toLowerCase();
  if (message.includes("resource_content_external")) {
    return "This exact Resource revision is stored by an external provider and is not available to the local preview. The Task pin is unchanged.";
  }
  if (message.includes("text files only") || message.includes("not text") || message.includes("allowlisted")) {
    return "This Resource revision is not in the local text-preview allowlist. Binary and active content are not rendered here.";
  }
  if (message.includes("1 mib") || message.includes("1024")) {
    return "This text revision exceeds the 1 MiB preview limit.";
  }
  if (message.includes("utf-8") || message.includes("utf8")) {
    return "This text revision is not valid UTF-8 and cannot be previewed safely.";
  }
  if (message.includes("unavailable") || message.includes("could not read") || message.includes("not responding")) {
    return "The local Runtime could not read this pinned Resource revision. Check Runtime availability and retry.";
  }
  return "The exact pinned revision could not be previewed. The Task input remains unchanged.";
}
