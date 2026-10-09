export const MAX_RESOURCE_SELECTION_FILES = 100;
export const MAX_RESOURCE_SELECTION_BYTES = 100 * 1024 * 1024;
export const MAX_RESOURCE_FILE_BYTES = 100 * 1024 * 1024;

export type ResourceIntakeFile = Pick<File, "name" | "size"> & {
  webkitRelativePath?: string;
};

export type ResourceIndexAction =
  | { kind: "REBUILD"; label: "Rebuild local text index" }
  | {
    kind: "UNAVAILABLE";
    label: "Not indexed (ZIP)";
    reason: "ZIP files are stored intact; their contents are not extracted or indexed.";
  };

export function isOpaqueZipResource(resource: { displayName: string; mediaType: string }): boolean {
  const mediaType = resource.mediaType.split(";", 1)[0]?.trim().toLowerCase();
  return resource.displayName.toLowerCase().endsWith(".zip")
    || mediaType === "application/zip"
    || mediaType === "application/x-zip-compressed"
    || mediaType?.endsWith("+zip") === true;
}

export function resourceIndexAction(resource: { displayName: string; mediaType: string }): ResourceIndexAction {
  if (isOpaqueZipResource(resource)) {
    return {
      kind: "UNAVAILABLE",
      label: "Not indexed (ZIP)",
      reason: "ZIP files are stored intact; their contents are not extracted or indexed.",
    };
  }
  return { kind: "REBUILD", label: "Rebuild local text index" };
}

export function isSensitiveOrGenerated(file: ResourceIntakeFile): boolean {
  const path = file.webkitRelativePath || file.name;
  const parts = path.split(/[\\/]/).filter(Boolean).map((part) => part.toLowerCase());
  const filename = parts[parts.length - 1] ?? "";
  const excludedDirectories = new Set([".git", ".svn", ".hg", "node_modules", "target", "dist", "coverage", ".venv"]);
  if (parts.slice(0, -1).some((part) => excludedDirectories.has(part))) return true;
  if (filename === ".env" || filename.startsWith(".env.") || filename.endsWith(".pem") || filename.endsWith(".p12") || filename.endsWith(".pfx")) return true;
  if (filename === "id_rsa" || filename === "id_ed25519" || filename.endsWith(".key") || filename.includes("credentials")) return true;
  return parts.includes(".ssh") || parts.includes(".aws");
}

export function resourceDisplayName(file: ResourceIntakeFile): string {
  return resourceFolderRelativePath(file) ?? file.name;
}

export function resourceFolderRelativePath(file: ResourceIntakeFile): string | null {
  const relativePath = file.webkitRelativePath;
  if (!relativePath) return null;
  const parts = relativePath.split("/");
  if (relativePath.startsWith("/")
    || relativePath.endsWith("/")
    || relativePath.includes("\\")
    || relativePath.length > 4096
    || parts.length > 128
    || parts.some((part) => !part || part === "." || part === ".." || new TextEncoder().encode(part).byteLength > 255)
    || /[\u0000-\u001f\u007f]/u.test(relativePath)
    || /^[A-Za-z]:/.test(relativePath)) {
    throw new Error("A selected folder contains an invalid relative path.");
  }
  return relativePath;
}

export function validateResourceFileSelection(files: readonly ResourceIntakeFile[]): string | null {
  if (files.length > MAX_RESOURCE_SELECTION_FILES) return "Choose up to 100 files per selection.";
  if (files.some((file) => !Number.isSafeInteger(file.size) || file.size < 0 || file.size > MAX_RESOURCE_FILE_BYTES)) {
    return "Each file is limited to 100 MiB.";
  }
  if (files.reduce((sum, file) => sum + file.size, 0) > MAX_RESOURCE_SELECTION_BYTES) {
    return "Choose up to 100 MiB total per selection.";
  }
  return null;
}
