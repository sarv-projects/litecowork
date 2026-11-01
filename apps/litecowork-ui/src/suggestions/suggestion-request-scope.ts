/** UI side effects from a command belong only to the Workspace that admitted it. */
export function isSuggestionRequestCurrent(requestWorkspaceId: string, activeWorkspaceId: string): boolean {
  return requestWorkspaceId.trim().length > 0 && requestWorkspaceId === activeWorkspaceId;
}

/** Reject pages whose rows escape the request Workspace, including an empty-page-safe scope check. */
export function isSuggestionWorkspaceResponseCurrent(
  requestWorkspaceId: string,
  activeWorkspaceId: string,
  responseWorkspaceIds: readonly string[],
): boolean {
  return isSuggestionRequestCurrent(requestWorkspaceId, activeWorkspaceId)
    && responseWorkspaceIds.every((workspaceId) => workspaceId === requestWorkspaceId);
}
