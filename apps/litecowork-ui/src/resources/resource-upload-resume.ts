export type ResourceUploadContextDocument = {
  kind: "WORKSPACE_NOTES";
  owner_ref: { kind: "WORKSPACE"; workspace_id: string };
};

export type ResourceUploadIdentity = {
  workspaceId: string;
  displayName: string;
  mediaType: string;
  folderRelativePath: string | null;
  sizeBytes: number;
  contextDocument: ResourceUploadContextDocument | null;
};

export type ResourceUploadSessionIdentity = {
  workspaceId: string;
  displayName: string;
  mediaType: string;
  folderRelativePath: string | null;
  expectedSizeBytes: number;
  contextDocument?: unknown;
  expectedDigest: string | null;
  chunkSizeBytes: number;
  state: string;
  expiresAt: string;
};

export type ResourceUploadReceivedRange = {
  startOffset: number;
  endOffsetInclusive: number;
  sha256: string;
};

export type ResourceUploadRanges = {
  chunkSizeBytes: number;
  receivedRanges: ResourceUploadReceivedRange[];
};

export function prepareResourceUploadCommitRequestId<T extends { commitRequestId?: string }>(
  resume: T,
  persist: (resume: T) => boolean | void,
  createRequestId: () => string,
): string {
  const hasSavedRequestId = typeof resume.commitRequestId === "string" && resume.commitRequestId.length > 0;
  const requestId = hasSavedRequestId ? resume.commitRequestId! : createRequestId();
  resume.commitRequestId = requestId;
  if (!hasSavedRequestId && persist(resume) === false) {
    throw new Error("Could not persist resource upload recovery metadata; commit was not sent.");
  }
  return requestId;
}

const MAX_UPLOAD_CHUNK_BYTES = 4 * 1024 * 1024;
const SHA256_DIGEST = /^sha256:[a-f0-9]{64}$/;

function contextDocumentMatches(
  actual: unknown,
  expected: ResourceUploadContextDocument | null,
): boolean {
  if (!expected) return actual == null;
  if (
    typeof actual !== "object"
    || actual === null
    || Object.keys(actual).length !== 2
    || !Object.prototype.hasOwnProperty.call(actual, "kind")
    || !Object.prototype.hasOwnProperty.call(actual, "owner_ref")
  ) {
    return false;
  }
  const value = actual as ResourceUploadContextDocument;
  const ownerRef: unknown = value.owner_ref;
  if (
    typeof ownerRef !== "object"
    || ownerRef === null
    || Array.isArray(ownerRef)
    || Object.keys(ownerRef).length !== 2
    || !Object.prototype.hasOwnProperty.call(ownerRef, "kind")
    || !Object.prototype.hasOwnProperty.call(ownerRef, "workspace_id")
  ) {
    return false;
  }
  const owner = ownerRef as ResourceUploadContextDocument["owner_ref"];
  return value.kind === expected.kind
    && owner.kind === expected.owner_ref.kind
    && owner.workspace_id === expected.owner_ref.workspace_id;
}

export function matchesResourceUploadIdentity(
  session: ResourceUploadSessionIdentity,
  expected: ResourceUploadIdentity,
): boolean {
  return session.workspaceId === expected.workspaceId
    && session.displayName === expected.displayName
    && session.mediaType === expected.mediaType
    && session.folderRelativePath === expected.folderRelativePath
    && session.expectedSizeBytes === expected.sizeBytes
    && contextDocumentMatches(session.contextDocument, expected.contextDocument);
}

export function resumableResourceUploadSession(
  session: ResourceUploadSessionIdentity,
  expected: ResourceUploadIdentity,
  expectedDigest: string,
  nowEpochMs: number,
): boolean {
  return matchesResourceUploadIdentity(session, expected)
    && session.expectedDigest === expectedDigest
    && session.chunkSizeBytes > 0
    && session.chunkSizeBytes <= MAX_UPLOAD_CHUNK_BYTES
    && (session.state === "OPEN" || session.state === "CONTENT_RECEIVED")
    && Date.parse(session.expiresAt) > nowEpochMs;
}

export async function receivedUploadRangesMatchFile(
  file: Pick<Blob, "size" | "slice">,
  session: ResourceUploadRanges,
): Promise<boolean> {
  const { chunkSizeBytes, receivedRanges } = session;
  if (
    !Number.isSafeInteger(file.size)
    || file.size < 0
    || !Number.isSafeInteger(chunkSizeBytes)
    || chunkSizeBytes <= 0
    || chunkSizeBytes > MAX_UPLOAD_CHUNK_BYTES
    || !Array.isArray(receivedRanges)
  ) {
    return false;
  }

  const seenChunks = new Set<number>();
  for (const candidate of receivedRanges) {
    if (
      typeof candidate !== "object"
      || candidate === null
      || Array.isArray(candidate)
      || Object.keys(candidate).length !== 3
      || !Object.prototype.hasOwnProperty.call(candidate, "startOffset")
      || !Object.prototype.hasOwnProperty.call(candidate, "endOffsetInclusive")
      || !Object.prototype.hasOwnProperty.call(candidate, "sha256")
    ) {
      return false;
    }
    const range = candidate as ResourceUploadReceivedRange;
    if (
      !Number.isSafeInteger(range.startOffset)
      || !Number.isSafeInteger(range.endOffsetInclusive)
      || range.startOffset < 0
      || range.startOffset >= file.size
      || range.endOffsetInclusive < range.startOffset
      || range.endOffsetInclusive >= file.size
      || !SHA256_DIGEST.test(range.sha256)
    ) {
      return false;
    }

    const chunkIndex = Math.floor(range.startOffset / chunkSizeBytes);
    const expectedStart = chunkIndex * chunkSizeBytes;
    const expectedEnd = Math.min(expectedStart + chunkSizeBytes, file.size) - 1;
    if (
      range.startOffset !== expectedStart
      || range.endOffsetInclusive !== expectedEnd
      || seenChunks.has(chunkIndex)
    ) {
      return false;
    }
    seenChunks.add(chunkIndex);

    const bytes = new Uint8Array(
      await file.slice(range.startOffset, range.endOffsetInclusive + 1).arrayBuffer(),
    );
    const digest = await crypto.subtle.digest("SHA-256", bytes);
    const observedDigest = `sha256:${Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
    if (observedDigest !== range.sha256) return false;
  }
  return true;
}
