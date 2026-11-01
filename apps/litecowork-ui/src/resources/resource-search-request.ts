export function isCurrentWorkspaceSearchRequest(
  request: number,
  currentRequest: number,
  requestWorkspaceId: string,
  selectedWorkspaceId: string,
): boolean {
  return request === currentRequest && requestWorkspaceId === selectedWorkspaceId;
}
