export function shouldAutoSelectFirstConversation(
  externallySelectedConversationId: string | null,
  newChatIntentActive: boolean,
): boolean {
  return externallySelectedConversationId === null && !newChatIntentActive;
}
