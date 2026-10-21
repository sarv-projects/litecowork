/** A Notification request may affect the UI only while its Workspace is still selected. */
export function isNotificationRequestCurrent(requestWorkspaceId: string, activeWorkspaceId: string): boolean {
  return requestWorkspaceId.trim().length > 0 && requestWorkspaceId === activeWorkspaceId;
}
