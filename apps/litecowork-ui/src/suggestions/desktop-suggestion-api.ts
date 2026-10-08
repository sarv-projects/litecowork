import { invoke } from "@tauri-apps/api/core";
import { createSuggestionApi, type SuggestionAcceptTaskTransport, type SuggestionActionTransport, type SuggestionPreferenceTransport, type SuggestionPreferenceUpdateTransport, type SuggestionTransport } from "./suggestion-api";

function desktopTransport(workspaceId: string): SuggestionTransport {
  return async (_requestedWorkspace, visibility, cursor, signal) => {
    if (signal?.aborted) throw new DOMException("Aborted", "AbortError");
    const page = await invoke<unknown>("list_suggestions", { workspaceId, visibility, cursor: cursor ?? null });
    if (signal?.aborted) throw new DOMException("Aborted", "AbortError");
    return page;
  };
}

const desktopActionTransport: SuggestionActionTransport = async (workspaceId, suggestionId, operation, expectedVersion, requestId, snoozedUntil) => {
  return invoke<unknown>("suggestion_owner_action", {
    workspaceId, suggestionId, operation, expectedVersion, requestId,
    snoozedUntil: snoozedUntil ?? null,
  });
};

const desktopAcceptTaskTransport: SuggestionAcceptTaskTransport = async (workspaceId, suggestionId, expectedVersion, requestId) => {
  return invoke<string>("accept_suggestion_task", { workspaceId, suggestionId, expectedVersion, requestId });
};

const desktopPreferenceTransport: SuggestionPreferenceTransport = async workspaceId =>
  invoke<unknown>("list_suggestion_preferences", { workspaceId });

const desktopPreferenceUpdateTransport: SuggestionPreferenceUpdateTransport = async (workspaceId, kind, muted, expectedVersion, requestId) =>
  invoke<unknown>("set_suggestion_preference", { workspaceId, kind, muted, expectedVersion, requestId });

export function desktopSuggestionApi(workspaceId: string) {
  return createSuggestionApi(workspaceId, desktopTransport(workspaceId), desktopActionTransport, desktopAcceptTaskTransport, desktopPreferenceTransport, desktopPreferenceUpdateTransport);
}
