export type WorkspaceRootStatusAction = "pause" | "resume";

export type WorkspaceRootStatusRequest = {
  workspaceId: string;
  workspaceRootId: string;
  action: WorkspaceRootStatusAction;
  expectedVersion: number;
};

export type WorkspaceRootStatusResponse = {
  workspaceId: string;
  workspaceRootId: string;
  resourceId: string;
  displayName: string;
  watchPolicy: string;
  replicationPolicy: string;
  status: string;
  version: number;
  locationAvailability: string;
};

export type WorkspaceRootStatusPage<T extends WorkspaceRootStatusResponse> = {
  items: T[];
  nextCursor: string | null;
};

export function isWorkspaceRootProjection(
  response: WorkspaceRootStatusResponse,
  workspaceId: string,
): boolean {
  if (!response || typeof response !== "object") return false;
  return response.workspaceId === workspaceId
    && typeof response.workspaceRootId === "string"
    && response.workspaceRootId.length > 0
    && typeof response.resourceId === "string"
    && response.resourceId.length > 0
    && typeof response.displayName === "string"
    && response.displayName.trim().length > 0
    && ["METADATA", "CONTENT_DIGESTS", "SELECTED_TEXT_EXTRACTION"].includes(response.watchPolicy)
    && ["NONE", "ACTIVE_TASKS", "SELECTED_WORKSPACE_POLICY"].includes(response.replicationPolicy)
    && ["ACTIVE", "PAUSED", "UNAVAILABLE", "REVOKED"].includes(response.status)
    && ["AVAILABLE", "OFFLINE", "PLACEHOLDER", "REVOKED", "UNKNOWN", "UNAVAILABLE"].includes(response.locationAvailability)
    && Number.isSafeInteger(response.version)
    && response.version >= 1;
}

export type WorkspaceRootStatusActionOutcome<T extends WorkspaceRootStatusResponse> =
  | { kind: "confirmed"; root: T }
  | {
    kind: "uncertain";
    failureMessage: string;
    refreshedRoots: T[];
    targetFound: boolean;
    refreshError: string | null;
  }
  | { kind: "stale" };

export function workspaceRootStatusRequest(
  requests: Map<string, string>,
  request: WorkspaceRootStatusRequest,
  createRequestId: () => string,
): { key: string; requestId: string } {
  const key = JSON.stringify([
    request.workspaceId,
    request.workspaceRootId,
    request.action,
    request.expectedVersion,
  ]);
  const requestId = requests.get(key) ?? createRequestId();
  requests.set(key, requestId);
  return { key, requestId };
}

export function clearWorkspaceRootStatusRequest(requests: Map<string, string>, key: string): void {
  requests.delete(key);
}

export function isWorkspaceRootStatusResponse(
  response: WorkspaceRootStatusResponse,
  request: WorkspaceRootStatusRequest,
): boolean {
  const expectedStatus = request.action === "pause" ? "PAUSED" : "ACTIVE";
  return isWorkspaceRootProjection(response, request.workspaceId)
    && response.workspaceRootId === request.workspaceRootId
    && response.status === expectedStatus
    && response.version === request.expectedVersion + 1
    && (request.action !== "resume" || response.locationAvailability === "AVAILABLE");
}

export function isCurrentWorkspaceRootRequest(
  requestWorkspaceId: string,
  requestGeneration: number,
  selectedWorkspaceId: string,
  currentGeneration: number,
): boolean {
  return requestWorkspaceId === selectedWorkspaceId && requestGeneration === currentGeneration;
}

export function resourceLocationAvailabilityLabel(availability: string): string {
  switch (availability) {
    case "AVAILABLE": return "Last observed: available";
    case "OFFLINE": return "Last observed: offline";
    case "PLACEHOLDER": return "Last observed: placeholder";
    case "REVOKED": return "Last observed: revoked";
    case "UNKNOWN": return "Last observed: unknown";
    case "UNAVAILABLE": return "Last observed: unavailable";
    default: return "Last observed: unsupported";
  }
}

function errorMessage(error: unknown, fallback: string): string {
  return typeof error === "string" ? error : error instanceof Error ? error.message : fallback;
}

export async function performWorkspaceRootStatusAction<T extends WorkspaceRootStatusResponse>(input: {
  requests: Map<string, string>;
  request: WorkspaceRootStatusRequest;
  createRequestId: () => string;
  execute: (requestId: string) => Promise<T>;
  list: (cursor: string | null) => Promise<WorkspaceRootStatusPage<T>>;
  isCurrent: () => boolean;
  onRefreshStart?: () => void;
}): Promise<WorkspaceRootStatusActionOutcome<T>> {
  const { key, requestId } = workspaceRootStatusRequest(input.requests, input.request, input.createRequestId);
  try {
    const root = await input.execute(requestId);
    if (!isWorkspaceRootStatusResponse(root, input.request)) {
      throw new Error("The Runtime did not confirm this folder status change.");
    }
    clearWorkspaceRootStatusRequest(input.requests, key);
    return input.isCurrent() ? { kind: "confirmed", root } : { kind: "stale" };
  } catch (error) {
    if (!input.isCurrent()) return { kind: "stale" };
    const failureMessage = errorMessage(error, "Persistent folder status could not be updated.");
    const refreshedRoots: T[] = [];
    const seenCursors = new Set<string>();
    let cursor: string | null = null;
    let targetFound = false;
    input.onRefreshStart?.();
    try {
      do {
        if (!input.isCurrent()) return { kind: "stale" };
        const page = await input.list(cursor);
        if (!page || !Array.isArray(page.items) || (page.nextCursor !== null && typeof page.nextCursor !== "string")) {
          throw new Error("The Runtime returned an invalid folder list.");
        }
        if (page.items.some((item) => !isWorkspaceRootProjection(item, input.request.workspaceId))) {
          throw new Error("The Runtime returned an invalid folder list.");
        }
        refreshedRoots.push(...page.items);
        if (page.items.some((item) => item.workspaceRootId === input.request.workspaceRootId)) {
          targetFound = true;
          break;
        }
        cursor = page.nextCursor;
        if (cursor !== null) {
          if (seenCursors.has(cursor)) throw new Error("The Runtime repeated a folder-list cursor.");
          seenCursors.add(cursor);
        }
      } while (cursor !== null);
      if (!input.isCurrent()) return { kind: "stale" };
      return { kind: "uncertain", failureMessage, refreshedRoots, targetFound, refreshError: null };
    } catch (refreshError) {
      if (!input.isCurrent()) return { kind: "stale" };
      return {
        kind: "uncertain",
        failureMessage,
        refreshedRoots,
        targetFound,
        refreshError: errorMessage(refreshError, "The folder list could not be reloaded."),
      };
    }
  }
}
