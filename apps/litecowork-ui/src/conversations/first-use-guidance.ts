export type FirstUseConversationGuidanceState =
  | "WORKSPACE_REQUIRED"
  | "RUNTIME_REQUIRED"
  | "SENDING_NOT_AVAILABLE";

export interface FirstUseConversationGuidance {
  state: FirstUseConversationGuidanceState;
  heading: string;
  message: string;
  actionLabel: string | null;
}

export interface FirstUseConversationGuidanceInput {
  workspaceAvailable: boolean;
  operatorReady: boolean;
}

export function firstUseConversationGuidance({
  workspaceAvailable,
  operatorReady,
}: FirstUseConversationGuidanceInput): FirstUseConversationGuidance {
  if (!workspaceAvailable) {
    return {
      state: "WORKSPACE_REQUIRED",
      heading: "Choose a Workspace to load conversations",
      message:
        "LiteCowork can’t check whether any conversations are saved until you choose a Workspace. Any draft stays only in this open window.",
      actionLabel: "Choose a Workspace in Settings",
    };
  }

  if (!operatorReady) {
    return {
      state: "RUNTIME_REQUIRED",
      heading: "Reconnect to saved conversations",
      message:
        "The local Runtime is unavailable, so LiteCowork can’t load this Workspace’s history or send messages. Any draft stays only in this open window.",
      actionLabel: "Open Runtime settings",
    };
  }

  return {
    state: "SENDING_NOT_AVAILABLE",
    heading: "Conversation sending isn't available yet",
    message:
      "Your Workspace and Runtime are ready, but this build cannot send Conversation messages. Your draft stays in the current window; it is not saved or sent, and opening Settings will not send it automatically.",
    actionLabel: null,
  };
}
