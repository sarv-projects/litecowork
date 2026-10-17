/** A denied or non-disclosing missing read makes previously cached bytes stale. */
export function invalidatesVisibleArtifactContent(error: unknown): boolean {
  return Boolean(error && typeof error === "object" && "status" in error
    && [401, 403, 404].includes((error as { status?: unknown }).status as number));
}
