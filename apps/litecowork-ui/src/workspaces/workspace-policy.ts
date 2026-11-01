export type WorkspacePolicyUpdateIntent = {
  workspaceId: string;
  expectedVersion: number;
  replicationPolicy: string;
};

export type PendingWorkspacePolicyUpdate = WorkspacePolicyUpdateIntent & {
  requestId: string;
};

export type WorkspacePolicyProjection = {
  workspaceId: string;
  version: number;
  replicationPolicy: string;
};

export type WorkspacePolicyRefreshOutcome = "CONFIRMED" | "SUPERSEDED" | "UNCHANGED" | "MISSING";

export function workspacePolicyRefreshOutcome(
  workspace: WorkspacePolicyProjection | undefined,
  request: WorkspacePolicyUpdateIntent,
): WorkspacePolicyRefreshOutcome {
  if (!workspace) return "MISSING";
  if (workspace.version <= request.expectedVersion) return "UNCHANGED";
  return workspace.replicationPolicy === request.replicationPolicy ? "CONFIRMED" : "SUPERSEDED";
}

export function workspacePolicyUpdateRequest(
  pending: Map<string, PendingWorkspacePolicyUpdate>,
  intent: WorkspacePolicyUpdateIntent,
  createRequestId: () => string,
): PendingWorkspacePolicyUpdate {
  const key = workspacePolicyIntentKey(intent);
  const existing = pending.get(key);
  if (existing) return existing;

  // A newer version or target is a new intent. Keep requests for other Workspaces
  // isolated, but do not leave an obsolete same-Workspace request available to UI.
  for (const [pendingKey, request] of pending) {
    if (request.workspaceId === intent.workspaceId) pending.delete(pendingKey);
  }

  const request = { ...intent, requestId: createRequestId() };
  pending.set(key, request);
  return request;
}

export function clearWorkspacePolicyUpdateRequest(
  pending: Map<string, PendingWorkspacePolicyUpdate>,
  request: PendingWorkspacePolicyUpdate,
): void {
  const key = workspacePolicyIntentKey(request);
  if (pending.get(key)?.requestId === request.requestId) pending.delete(key);
}

export function reconcileWorkspacePolicyUpdateRequests(
  pending: Map<string, PendingWorkspacePolicyUpdate>,
  workspaces: readonly WorkspacePolicyProjection[],
): void {
  const byId = new Map(workspaces.map(workspace => [workspace.workspaceId, workspace]));
  for (const [key, request] of pending) {
    const workspace = byId.get(request.workspaceId);
    if (workspace && workspace.version > request.expectedVersion) pending.delete(key);
  }
}

export function isWorkspacePolicyUpdateResponse(
  response: WorkspacePolicyProjection,
  request: PendingWorkspacePolicyUpdate,
): boolean {
  return !!response
    && typeof response === "object"
    && response.workspaceId === request.workspaceId
    && response.replicationPolicy === request.replicationPolicy
    && Number.isSafeInteger(response.version)
    && response.version === request.expectedVersion + 1;
}

export function mergeWorkspacePolicyProjection<T extends WorkspacePolicyProjection>(
  workspaces: readonly T[],
  updated: T,
): T[] {
  return workspaces.map(workspace => workspace.workspaceId === updated.workspaceId
    && workspace.version < updated.version
    ? updated
    : workspace);
}

export function mergeWorkspaceListProjection<T extends WorkspacePolicyProjection>(
  current: readonly T[],
  refreshed: readonly T[],
): T[] {
  const currentById = new Map(current.map(workspace => [workspace.workspaceId, workspace]));
  return refreshed.map(workspace => {
    const existing = currentById.get(workspace.workspaceId);
    return existing && existing.version > workspace.version ? existing : workspace;
  });
}

function workspacePolicyIntentKey(intent: WorkspacePolicyUpdateIntent): string {
  return JSON.stringify([intent.workspaceId, intent.expectedVersion, intent.replicationPolicy]);
}
