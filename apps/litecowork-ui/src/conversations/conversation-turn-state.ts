/**
 * Pure Conversation turn, composer and scroll state (001-T16, T17, T18, T26).
 *
 * Everything here is derived from the projected snapshot, never from a transient
 * stream frame, so Stop, Retry and the composer fence stay correct when no
 * `turn.delta` frame is flowing (for example on the OpenCode path before text arrives).
 */

export type ActiveTurnView = {
  turn_id: string;
  version: number;
  status: string;
  retry_ordinal: number;
  presentation_preference?: string;
};

export type RetryableTurnView = {
  turn_id: string;
  version: number;
  retry_ordinal: number;
  reason_code: string | null;
};

/** Every status in which the single-flight lane is occupied (B-01). */
export const NONTERMINAL_TURN_STATUSES = ["OPEN", "RUNNING", "WAITING_USER", "WAITING_DEPENDENCY", "CANCEL_REQUESTED"] as const;

export function isNonterminalTurnStatus(status: string | null | undefined): boolean {
  return status !== null && status !== undefined && (NONTERMINAL_TURN_STATUSES as readonly string[]).includes(status);
}

export type TurnFenceInput = {
  activeTurn: { status: string } | null;
  /** A transient streamed turn is still being drawn for this Conversation. */
  transientStreaming: boolean;
  /** The local submit is in flight, including the gap after the 202 and before OPEN projects. */
  submitting: boolean;
};

/** True while a turn occupies the composer: any nonterminal status, a streamed turn or an in-flight submit. */
export function isTurnOccupyingComposer(input: TurnFenceInput): boolean {
  return input.submitting || input.transientStreaming || isNonterminalTurnStatus(input.activeTurn?.status);
}

export type SendEligibilityInput = TurnFenceInput & {
  draft: string;
  hasWorkspace: boolean;
  operatorReady: boolean;
  executionReady: boolean;
  bindingEligible: boolean;
};

export function isSendEligible(input: SendEligibilityInput): boolean {
  return input.draft.trim().length > 0
    && input.hasWorkspace
    && input.operatorReady
    && input.executionReady
    && input.bindingEligible
    && !isTurnOccupyingComposer(input);
}

export function turnStatusLabel(status: string | null | undefined): string | null {
  switch (status) {
    case "OPEN": return "Preparing";
    case "RUNNING": return "A conversation turn is running";
    case "WAITING_USER": return "Waiting for your response";
    case "WAITING_DEPENDENCY": return "Waiting for a service";
    case "CANCEL_REQUESTED": return "Stopping";
    default: return null;
  }
}

export type StopTarget = { turnId: string; expectedVersion: number };

/**
 * Stop is offered only for RUNNING, the state currently supported by the daemon's
 * provider-aware cancellation path. Waiting states occupy the composer but their
 * cancellation/continuation path is not served yet; do not render a control that will
 * always fail. OPEN has no confirmed native work to interrupt.
 */
export function stopTarget(activeTurn: ActiveTurnView | null | undefined): StopTarget | null {
  if (!activeTurn || activeTurn.status !== "RUNNING") return null;
  return { turnId: activeTurn.turn_id, expectedVersion: activeTurn.version };
}

/** Retry is offered only for the latest FAILED turn while no other turn is in flight. */
export function retryTarget(
  activeTurn: { status: string } | null | undefined,
  retryable: RetryableTurnView | null | undefined,
  submitting: boolean,
): RetryableTurnView | null {
  if (!retryable || submitting || isNonterminalTurnStatus(activeTurn?.status)) return null;
  return retryable;
}

export function canRetryTurn(
  target: RetryableTurnView | null | undefined,
  pending: boolean,
  executionReady: boolean,
): boolean {
  return target !== null && target !== undefined && !pending && executionReady;
}

/** A safe, human reading of a typed settlement reason code. Unknown codes never leak raw text. */
export function retryReasonLabel(reasonCode: string | null | undefined): string {
  switch (reasonCode) {
    case "AGENT_TURN_TIMEOUT": return "The agent stopped responding.";
    case "AGENT_EMPTY_RESPONSE": return "The agent finished without an answer.";
    case "AGENT_PROVIDER_ERROR": return "The agent reported an error.";
    case "AGENT_TRANSPORT_ERROR": return "The connection to the agent was interrupted.";
    case "ADAPTER_AUTH_FAILED": return "The agent needs you to sign in again.";
    case "ADAPTER_EXECUTABLE_MISSING": return "The agent program was not found.";
    case "ADAPTER_VERSION_UNSUPPORTED": return "This agent version is not supported.";
    case "ADAPTER_STARTUP_FAILED": return "The agent could not start.";
    case "AGENT_UNAVAILABLE":
    case "CONVERSATION_SESSION_ADMISSION_UNAVAILABLE": return "The selected agent was not available.";
    case "RUNTIME_RESTARTED": return "LiteCowork restarted while the agent was working.";
    default: return "The agent could not finish this turn.";
  }
}

export function conversationTitle(title: string | null | undefined): string {
  return title && title.trim() ? title : "Untitled conversation";
}

/** The Workspace default's real label, or an honest "No agent selected". Never an invented model name. */
export function defaultAgentOptionLabel(workspaceDefaultLabel: string | null | undefined): string {
  return workspaceDefaultLabel && workspaceDefaultLabel.trim() ? workspaceDefaultLabel : "No agent selected";
}

export const COMPOSER_PLACEHOLDER = "Message the selected agent";

export type ScrollMetrics = { scrollTop: number; clientHeight: number; scrollHeight: number };

/** Within this many pixels of the bottom counts as "at the bottom". */
export const AT_BOTTOM_THRESHOLD_PX = 48;

export function isAtBottom(metrics: ScrollMetrics, threshold = AT_BOTTOM_THRESHOLD_PX): boolean {
  return metrics.scrollHeight - metrics.scrollTop - metrics.clientHeight <= threshold;
}

export type ScrollFollowDecision = "FOLLOW" | "INDICATE" | "NONE";

/**
 * New content follows only a reader who is already at the bottom. A reader who scrolled
 * up keeps their position and gets the "new messages" indicator instead; selecting text
 * is treated like reading history so autoscroll never steals the selection.
 */
export function scrollFollowDecision(input: { wasAtBottom: boolean; hasNewContent: boolean; selectionActive: boolean }): ScrollFollowDecision {
  if (!input.hasNewContent) return "NONE";
  return input.wasAtBottom && !input.selectionActive ? "FOLLOW" : "INDICATE";
}

/** Content identity used to notice new messages or new streamed text without comparing bodies. */
export function contentSignature(messageIds: readonly string[], transientTextLength: number): string {
  return `${messageIds.length}:${messageIds[messageIds.length - 1] ?? ""}:${transientTextLength}`;
}
