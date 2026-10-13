export type WorkspaceRootAddRequest = {
  action: "add";
  workspaceId: string;
  expectedVersion: number;
  watchPolicy: string;
  replicationPolicy: string;
};

export type WorkspaceRootRevokeRequest = {
  action: "revoke";
  workspaceId: string;
  workspaceRootId: string;
  expectedVersion: number;
};

export type WorkspaceRootMutationRequest = WorkspaceRootAddRequest | WorkspaceRootRevokeRequest;

export type WorkspaceRootMutationResponse = {
  workspaceId: string;
  workspaceRootId: string;
  resourceId: string;
  displayName: string;
  watchPolicy: string;
  replicationPolicy: string;
  status: string;
  locationAvailability: string;
  version: number;
};

export function workspaceRootMutationRequest(
  requests: Map<string, string>,
  request: WorkspaceRootMutationRequest,
  createRequestId: () => string,
): { key: string; requestId: string } {
  const key = JSON.stringify(request);
  const requestId = requests.get(key) ?? createRequestId();
  requests.set(key, requestId);
  return { key, requestId };
}

export function clearWorkspaceRootMutationRequest(requests: Map<string, string>, key: string): void {
  requests.delete(key);
}

export function isWorkspaceRootAddResponse(
  response: WorkspaceRootMutationResponse,
  request: WorkspaceRootAddRequest,
): boolean {
  return isValidRootProjection(response)
    && response.workspaceId === request.workspaceId
    && response.status === "ACTIVE"
    && response.version === 1
    && response.watchPolicy === request.watchPolicy
    && response.replicationPolicy === request.replicationPolicy
    && response.locationAvailability === "AVAILABLE";
}

export function isWorkspaceRootRevokeResponse(
  response: WorkspaceRootMutationResponse,
  request: WorkspaceRootRevokeRequest,
): boolean {
  return isValidRootProjection(response)
    && response.workspaceId === request.workspaceId
    && response.workspaceRootId === request.workspaceRootId
    && response.status === "REVOKED"
    && Number.isSafeInteger(request.expectedVersion)
    && response.version === request.expectedVersion + 1
    && response.locationAvailability === "REVOKED";
}

function isValidRootProjection(response: WorkspaceRootMutationResponse): boolean {
  return !!response && typeof response === "object"
    && typeof response.workspaceId === "string"
    && response.workspaceId.length > 0
    && typeof response.workspaceRootId === "string"
    && response.workspaceRootId.length > 0
    && typeof response.resourceId === "string"
    && response.resourceId.length > 0
    && typeof response.displayName === "string"
    && response.displayName.trim().length > 0
    && ["METADATA", "CONTENT_DIGESTS", "SELECTED_TEXT_EXTRACTION"].includes(response.watchPolicy)
    && ["NONE", "ACTIVE_TASKS", "SELECTED_WORKSPACE_POLICY"].includes(response.replicationPolicy)
    && ["AVAILABLE", "OFFLINE", "PLACEHOLDER", "REVOKED", "UNKNOWN", "UNAVAILABLE"].includes(response.locationAvailability)
    && Number.isSafeInteger(response.version)
    && response.version >= 1;
}
