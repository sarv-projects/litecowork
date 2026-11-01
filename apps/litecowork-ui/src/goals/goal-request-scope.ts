/** A Goal request may affect the UI only while its Workspace is still selected. */
export function isGoalRequestCurrent(requestWorkspaceId: string, activeWorkspaceId: string): boolean {
  return requestWorkspaceId.trim().length > 0 && requestWorkspaceId === activeWorkspaceId;
}

/** Gate both the synchronously selected Workspace and any owner carried by the response. */
export function isGoalWorkspaceResponseCurrent(
  requestWorkspaceId: string,
  activeWorkspaceId: string,
  responseWorkspaceIds: readonly string[],
): boolean {
  return isGoalRequestCurrent(requestWorkspaceId, activeWorkspaceId)
    && responseWorkspaceIds.every((workspaceId) => workspaceId === requestWorkspaceId);
}
