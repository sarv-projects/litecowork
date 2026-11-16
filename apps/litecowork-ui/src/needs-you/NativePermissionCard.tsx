import type { NativePermissionDecision, NativePermissionPrompt } from "./native-permission";
import "./native-permission.css";

export function NativePermissionCard({
  prompt,
  conversationTitle,
  disabled = false,
  refreshDisabled = false,
  sendingDecision,
  onRefresh,
  onDecision,
}: {
  prompt: NativePermissionPrompt;
  conversationTitle?: string | null;
  disabled?: boolean;
  refreshDisabled?: boolean;
  sendingDecision?: NativePermissionDecision | null;
  onRefresh?: () => void;
  onDecision: (decision: NativePermissionDecision) => void;
}) {
  const pending = prompt.state === "PENDING" && !disabled && !sendingDecision;
  const displayedState = sendingDecision ? "REPLYING" : prompt.state;
  const displayedDecision = sendingDecision ?? prompt.decision;
  const stateLabel = displayedState === "PENDING" ? "Waiting for your response"
    : displayedState === "REPLYING" ? "Sending decision…"
      : displayedState === "ANSWERED" ? "Answered through the agent’s permission flow"
        : displayedState === "OUTCOME_UNKNOWN" ? "Outcome could not be confirmed"
          : "Agent session ended";

  return <article className={`native-permission-card native-permission-${prompt.state.toLowerCase()}`} aria-labelledby={`native-permission-${prompt.promptToken}-title`}>
    <div className="native-permission-heading">
      <span className="native-permission-icon" aria-hidden="true">!</span>
      <div>
        <h2 id={`native-permission-${prompt.promptToken}-title`}>The agent is asking</h2>
        {conversationTitle && <p className="native-permission-conversation">{conversationTitle}</p>}
      </div>
      <span className="native-permission-state" role="status">{stateLabel}</span>
    </div>
    <p className="native-permission-action">{prompt.action}</p>
    {prompt.explanation && <p className="native-permission-explanation">{prompt.explanation}</p>}
    {prompt.resourceCount > 0 && <p className="native-permission-resources">
      {prompt.resourceCount} requested resource{prompt.resourceCount === 1 ? "" : "s"}{prompt.resources.length > 0 ? ` · ${prompt.resources.join(", ")}` : ""}
    </p>}
    <p className="native-permission-disclosure">
      This is a request from the connected agent. LiteCowork has not granted it. “Reject” also rejects every other request currently waiting in this agent session.
    </p>
    {prompt.state === "OUTCOME_UNKNOWN" && <p className="native-permission-notice" role="alert">The answer may have reached the agent, but LiteCowork could not confirm the result. This request is disabled until the live state can be checked.</p>}
    {prompt.state === "SESSION_ENDED" && <p className="native-permission-notice" role="note">The agent session ended. LiteCowork will not send a decision for this request.</p>}
    {prompt.state === "ANSWERED" && <p className="native-permission-notice" role="note">The agent’s permission system acknowledged the decision shown in the conversation.</p>}
    {displayedDecision && displayedState !== "PENDING" && <p className="native-permission-decision">{displayedState === "ANSWERED"
      ? displayedDecision === "ALLOW_ONCE" ? "You allowed this request once." : "You rejected this request and the other requests waiting then."
      : `You chose ${displayedDecision === "ALLOW_ONCE" ? "Allow once" : "Reject waiting requests"}; the agent’s result is not confirmed.`}</p>}
    <div className="native-permission-actions">
      {displayedState === "OUTCOME_UNKNOWN" && onRefresh && <button type="button" disabled={refreshDisabled} onClick={onRefresh}>Check live status</button>}
      <button type="button" className="native-permission-allow" disabled={!pending} onClick={() => onDecision("ALLOW_ONCE")}>
        {displayedState === "REPLYING" && displayedDecision === "ALLOW_ONCE" ? "Sending…" : "Allow once"}
      </button>
      <button type="button" className="native-permission-reject" disabled={!pending} onClick={() => onDecision("REJECT_SESSION_PENDING")}>
        {displayedState === "REPLYING" && displayedDecision === "REJECT_SESSION_PENDING" ? "Sending…" : "Reject this and waiting requests"}
      </button>
    </div>
  </article>;
}
