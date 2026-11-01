export type ArtifactEditorFocusIntent = "EDITOR" | "EDIT" | "RESTORE" | "PUBLISHED_STATUS";
export type ArtifactEditorFocusTarget = { isConnected: boolean; focus: () => void };

export function focusArtifactEditorTarget(
  intent: ArtifactEditorFocusIntent,
  targets: {
    editor: ArtifactEditorFocusTarget | null;
    edit: ArtifactEditorFocusTarget | null;
    restore: ArtifactEditorFocusTarget | null;
    publishedStatus: ArtifactEditorFocusTarget | null;
    fallback: ArtifactEditorFocusTarget | null;
  },
): boolean {
  const target = intent === "EDITOR" ? targets.editor
    : intent === "EDIT" ? targets.edit
      : intent === "RESTORE" ? targets.restore
        : targets.publishedStatus;
  const availableTarget = target?.isConnected ? target : targets.fallback?.isConnected ? targets.fallback : null;
  if (!availableTarget) return false;
  availableTarget.focus();
  return true;
}
