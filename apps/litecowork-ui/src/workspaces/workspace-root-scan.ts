export type WorkspaceRootScanInput = {
  workspaceId: string;
  workspaceRootId: string;
  expectedVersion: number;
};

export type WorkspaceRootScanRequest = WorkspaceRootScanInput & {
  requestId: string;
};

export type WorkspaceRootScanPartialReason =
  | "TIME_LIMIT_REACHED"
  | "SCAN_BUDGET_REACHED"
  | "ENTRY_CHANGED_OR_UNREADABLE"
  | "MOUNT_BOUNDARY_REACHED";

export type WorkspaceRootScanSummary = {
  workspaceId: string;
  workspaceRootId: string;
  requestId: string;
  rootVersion: number;
  status: "COMPLETE" | "PARTIAL";
  entriesExamined: number;
  filesObserved: number;
  partialReason: WorkspaceRootScanPartialReason | null;
  completedAt: string;
};

export type WorkspaceRootScanFailure = {
  kind: "stale" | "changed" | "ambiguous" | "unsupported" | "rejected";
  message: string;
  retrySameRequest: boolean;
};

export type ScannableWorkspaceRoot = {
  workspaceId: string;
  workspaceRootId: string;
  version: number;
  status: string;
  locationAvailability: string;
};

function requestKey(request: WorkspaceRootScanInput): string {
  return JSON.stringify([request.workspaceId, request.workspaceRootId, request.expectedVersion]);
}

export function isWorkspaceRootScannable(root: unknown, selectedWorkspaceId: string): root is ScannableWorkspaceRoot {
  if (root === null || typeof root !== "object" || Array.isArray(root)) return false;
  const candidate = root as Partial<ScannableWorkspaceRoot>;
  return typeof selectedWorkspaceId === "string"
    && selectedWorkspaceId.length > 0
    && candidate.workspaceId === selectedWorkspaceId
    && typeof candidate.workspaceRootId === "string"
    && candidate.workspaceRootId.length > 0
    && candidate.status === "ACTIVE"
    && candidate.locationAvailability === "AVAILABLE"
    && Number.isSafeInteger(candidate.version)
    && (candidate.version as number) >= 1
    && (candidate.version as number) < Number.MAX_SAFE_INTEGER;
}

export function workspaceRootScanRequest(
  requests: Map<string, string>,
  request: WorkspaceRootScanInput,
  createRequestId: () => string,
): WorkspaceRootScanRequest {
  const key = requestKey(request);
  const requestId = requests.get(key) ?? createRequestId();
  requests.set(key, requestId);
  return { ...request, requestId };
}

export function isWorkspaceRootScanResponse(
  response: unknown,
  request: WorkspaceRootScanRequest,
): response is WorkspaceRootScanSummary {
  if (response === null || typeof response !== "object" || Array.isArray(response)) return false;
  const summary = response as Partial<WorkspaceRootScanSummary>;
  const validPartialReason = typeof summary.partialReason === "string"
    && [
      "TIME_LIMIT_REACHED",
      "SCAN_BUDGET_REACHED",
      "ENTRY_CHANGED_OR_UNREADABLE",
      "MOUNT_BOUNDARY_REACHED",
    ].includes(summary.partialReason);
  const reasonMatchesStatus = summary.status === "COMPLETE"
    ? summary.partialReason === null
    : summary.status === "PARTIAL" && validPartialReason;
  return summary.workspaceId === request.workspaceId
    && summary.workspaceRootId === request.workspaceRootId
    && summary.requestId === request.requestId
    && summary.rootVersion === request.expectedVersion
    && (summary.status === "COMPLETE" || summary.status === "PARTIAL")
    && Number.isSafeInteger(summary.entriesExamined)
    && (summary.entriesExamined as number) <= 1_000
    && Number.isSafeInteger(summary.filesObserved)
    && (summary.entriesExamined as number) >= 0
    && (summary.filesObserved as number) >= 0
    && (summary.filesObserved as number) <= (summary.entriesExamined as number)
    && reasonMatchesStatus
    && typeof summary.completedAt === "string"
    && Number.isFinite(Date.parse(summary.completedAt));
}

/** A valid terminal receipt ends this exact retry identity. Invalid or failed responses retain it. */
export function acceptWorkspaceRootScanResponse(
  requests: Map<string, string>,
  request: WorkspaceRootScanRequest,
  response: unknown,
): response is WorkspaceRootScanSummary {
  const key = requestKey(request);
  if (requests.get(key) !== request.requestId || !isWorkspaceRootScanResponse(response, request)) return false;
  requests.delete(key);
  return true;
}

export function isCurrentWorkspaceRootScan(
  request: WorkspaceRootScanRequest,
  requestGeneration: number,
  selectedWorkspaceId: string,
  currentGeneration: number,
  currentRoot: ScannableWorkspaceRoot | null,
): boolean {
  return request.workspaceId === selectedWorkspaceId
    && requestGeneration === currentGeneration
    && currentRoot !== null
    && currentRoot.workspaceRootId === request.workspaceRootId
    && currentRoot.version === request.expectedVersion
    && isWorkspaceRootScannable(currentRoot, selectedWorkspaceId);
}

export function workspaceRootScanFailure(
  error: unknown,
  request: WorkspaceRootScanRequest,
  currentRoot: ScannableWorkspaceRoot | null,
): WorkspaceRootScanFailure {
  const message = typeof error === "string"
    ? error
    : error instanceof Error
      ? error.message
      : "";
  const sameCachedRoot = currentRoot !== null
    && currentRoot.workspaceId === request.workspaceId
    && currentRoot.workspaceRootId === request.workspaceRootId
    && currentRoot.version === request.expectedVersion
    && isWorkspaceRootScannable(currentRoot, request.workspaceId);

  if (message === "SCAN_UNSUPPORTED_PLATFORM") {
    return {
      kind: "unsupported",
      message: "Metadata scanning is not available on this platform. This scan cannot be retried here.",
      retrySameRequest: false,
    };
  }
  if (message === "STALE_WORKSPACE_VERSION" && sameCachedRoot) {
    return {
      kind: "stale",
      message: "The Runtime reports this folder changed. Leave and reopen the Library to refresh its status before scanning again.",
      retrySameRequest: false,
    };
  }
  if (message === "STALE_WORKSPACE_VERSION" || !sameCachedRoot) {
    return {
      kind: "changed",
      message: "The folder status changed while its scan was running. Review the current folder status before scanning again.",
      retrySameRequest: false,
    };
  }
  const rejectedMessages: Record<string, string> = {
    INVALID_ARGUMENT: "The Runtime rejected this scan request. Leave and reopen the Library to refresh the folder status before scanning again.",
    RESOURCE_IDENTITY_UNAVAILABLE: "LiteCowork could not verify this saved folder. Check it or remove and add it again before scanning.",
    NOT_FOUND: "This folder is no longer available in the selected Workspace. Leave and reopen the Library to refresh its status.",
    CONFLICT: "This scan request conflicts with an earlier request. Leave and reopen the Library to refresh the folder status before scanning again.",
  };
  if (Object.hasOwn(rejectedMessages, message)) {
    return {
      kind: "rejected",
      message: rejectedMessages[message],
      retrySameRequest: false,
    };
  }
  if (message === "SCAN_UNAVAILABLE") {
    return {
      kind: "ambiguous",
      message: "Folder metadata could not be verified safely. Retry to check the same scan request.",
      retrySameRequest: true,
    };
  }
  return {
    kind: "ambiguous",
    message: "The folder scan could not be confirmed. Retry to check the same scan request.",
    retrySameRequest: true,
  };
}

export function workspaceRootScanAnnouncement(summary: WorkspaceRootScanSummary): string {
  if (summary.status === "PARTIAL") {
    const reason = summary.partialReason === "TIME_LIMIT_REACHED"
      ? "Time limit reached"
      : summary.partialReason === "SCAN_BUDGET_REACHED"
        ? "Scan size limit reached"
        : summary.partialReason === "ENTRY_CHANGED_OR_UNREADABLE"
          ? "A folder entry changed or could not be read"
          : summary.partialReason === "MOUNT_BOUNDARY_REACHED"
            ? "A nested mounted location was reached"
            : "The scan could not be completed";
    return `Partial scan: counted metadata for ${summary.filesObserved} files in ${summary.entriesExamined} folder entries. Reason: ${reason}.`;
  }
  if (summary.filesObserved === 0) {
    return `No files were observed in the scanned entries. Examined ${summary.entriesExamined} folder entries.`;
  }
  return `Scan complete: counted metadata for ${summary.filesObserved} files in ${summary.entriesExamined} folder entries.`;
}
