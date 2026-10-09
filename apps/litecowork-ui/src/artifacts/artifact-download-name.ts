/** Builds a filesystem-safe name for one exact immutable Artifact version. */
export function artifactDownloadFileName(displayName: string, version: number): string {
  if (!Number.isSafeInteger(version) || version < 1) {
    throw new RangeError("Artifact version must be a positive safe integer.");
  }

  const sanitized = displayName
    .replace(/[\u0000-\u001f\u007f<>:"/\\|?*]/g, "_")
    .trim()
    .replace(/[. ]+$/g, "")
    .slice(0, 180);
  const name = sanitized.replace(/[_. -]/g, "") ? sanitized : "artifact";
  const extensionStart = name.lastIndexOf(".");
  const hasExtension = extensionStart > 0 && extensionStart < name.length - 1;
  const base = hasExtension ? name.slice(0, extensionStart) : name;
  const extension = hasExtension ? name.slice(extensionStart) : "";

  return `${base}-v${version}${extension}`;
}
