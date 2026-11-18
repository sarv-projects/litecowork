import { useEffect, useRef, useState } from "react";
import {
  parseAgentControlReadError,
  type AgentControlActionsApi,
  type AgentControlDescriptor,
  type AgentControlDescriptorApi,
} from "./agent-control-api";
import { desktopAgentControlActionsApi, desktopAgentControlDescriptorApi } from "./desktop-agent-control-api";
import "./agent-control-descriptor-panel.css";

type PanelState =
  | { key: string; status: "LOADING" }
  | { key: string; status: "READY"; descriptor: AgentControlDescriptor }
  | { key: string; status: "UNAVAILABLE"; message: string; reference?: string }
  | { key: string; status: "FAILED"; message: string; reference?: string };

export type AgentControlDescriptorPanelProps = {
  workspaceId: string;
  profileId: string;
  displayName: string;
  api?: AgentControlDescriptorApi;
  actionApi?: AgentControlActionsApi;
};

export function AgentControlDescriptorPanel({
  workspaceId,
  profileId,
  displayName,
  api = desktopAgentControlDescriptorApi,
  actionApi = desktopAgentControlActionsApi,
}: AgentControlDescriptorPanelProps) {
  const key = `${workspaceId}\u0000${profileId}`;
  const generation = useRef(0);
  const actionGeneration = useRef(0);
  const [state, setState] = useState<PanelState>({ key, status: "LOADING" });
  const [descriptorExpired, setDescriptorExpired] = useState(false);
  const [actionState, setActionState] = useState<{ key: string; status: "BUSY" | "INFO" | "ERROR"; message: string; reference?: string } | null>(null);

  useEffect(() => {
    const request = ++generation.current;
    let active = true;
    setState({ key, status: "LOADING" });
    actionGeneration.current += 1;
    setActionState(null);
    void api.get(workspaceId, profileId).then(
      (descriptor) => {
        if (active && request === generation.current) setState({ key, status: "READY", descriptor });
      },
      (error: unknown) => {
        if (active && request === generation.current) setState(toErrorState(key, error));
      },
    );
    return () => {
      active = false;
      generation.current += 1;
      actionGeneration.current += 1;
    };
  }, [api, key, profileId, workspaceId]);

  const visible = state.key === key ? state : { key, status: "LOADING" as const };
  const descriptor = visible.status === "READY" ? visible.descriptor : undefined;

  useEffect(() => {
    if (!descriptor) {
      setDescriptorExpired(false);
      return;
    }
    const expiresAt = Date.parse(descriptor.expires_at);
    if (isAgentControlDescriptorExpired(descriptor.expires_at)) {
      setDescriptorExpired(true);
      return;
    }
    setDescriptorExpired(false);
    let timer: number | undefined;
    const markExpiredWhenDue = () => {
      const remaining = expiresAt - Date.now();
      if (remaining <= 0) {
        setDescriptorExpired(true);
      } else {
        timer = window.setTimeout(markExpiredWhenDue, Math.min(remaining, 2_147_000_000));
      }
    };
    markExpiredWhenDue();
    return () => {
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, [descriptor?.expires_at, key]);

  const runAction = (action: "AUTH" | "CONFIG", id: string) => {
    if (visible.status !== "READY" || descriptorExpired || !workspaceId) return;
    const request = ++actionGeneration.current;
    setActionState({ key, status: "BUSY", message: action === "AUTH" ? "Starting the declared sign-in flow…" : "Opening the declared native configuration…" });
    const task = action === "AUTH"
      ? actionApi.beginAuth(workspaceId, profileId, id).then((challenge) => {
        if (challenge.kind === "NATIVE_FLOW_STARTED") {
          return "The agent reports that its sign-in flow started. Complete sign-in in the agent’s own window, then refresh controls to recheck the reported state. LiteCowork does not receive credential values.";
        }
        if (challenge.kind === "SECRET_REQUIRED") {
          return "This agent requested a credential, but secure credential submission is unavailable. No secret was sent.";
        }
        return "The agent reports that native configuration is required. No credential was sent.";
      })
      : actionApi.openNativeConfig(workspaceId, profileId, id).then(() =>
        "The declared native configuration target was opened.");
    void task.then(
      (message) => {
        if (request === actionGeneration.current) setActionState({ key, status: "INFO", message });
      },
      (error: unknown) => {
        if (request !== actionGeneration.current) return;
        const parsed = parseAgentControlReadError(error);
        const unavailable = parsed.code === "DEPENDENCY_UNAVAILABLE";
        setActionState({
          key,
          status: "ERROR",
          message: action === "AUTH"
            ? unavailable ? "No qualified sign-in launcher is available for this agent. Nothing was launched and no credentials were sent." : "Sign-in could not start. Refresh the descriptor and try again. No credentials were sent."
            : unavailable ? "No qualified native configuration opener is available. Nothing was opened." : "The native configuration request could not be completed. Refresh the descriptor and try again.",
          reference: parsed.reference,
        });
      },
    );
  };

  const refresh = () => {
    const request = ++generation.current;
    actionGeneration.current += 1;
    setActionState(null);
    setState({ key, status: "LOADING" });
    void api.refresh(workspaceId, profileId).then(
      (descriptor) => {
        if (request === generation.current) setState({ key, status: "READY", descriptor });
      },
      (error: unknown) => {
        if (request === generation.current) setState(toErrorState(key, error));
      },
    );
  };

  return (
    <section className="agent-control-panel" aria-labelledby={`agent-controls-${safeId(profileId)}`}>
      <div className="agent-control-panel__heading">
        <div>
          <h4 id={`agent-controls-${safeId(profileId)}`}>Native controls</h4>
          <p>Module-declared controls for {displayName}. Reading these does not start or authorize a conversation.</p>
        </div>
        <button type="button" onClick={refresh} disabled={visible.status === "LOADING" || actionState?.status === "BUSY"}>
          {visible.status === "LOADING" ? "Checking…" : "Refresh controls"}
        </button>
      </div>
      {visible.status === "LOADING" && <p role="status">Checking the local agent adapter…</p>}
      {visible.status === "UNAVAILABLE" && (
        <UnavailableControlsNotice message={visible.message} reference={visible.reference} />
      )}
      {visible.status === "FAILED" && (
        <div className="agent-control-panel__notice" role="alert">
          <strong>Could not read agent controls</strong>
          <p>{visible.message}</p>
          {visible.reference && <small>Reference: {visible.reference}</small>}
        </div>
      )}
      {visible.status === "READY" && <DescriptorSummary
        descriptor={visible.descriptor}
        stale={descriptorExpired}
        actions={{
          onBeginAuth: (methodId) => runAction("AUTH", methodId),
          onOpenNativeConfig: (targetId) => runAction("CONFIG", targetId),
        }}
        actionBusy={actionState?.status === "BUSY"}
      />}
      {actionState?.key === key && actionState.status !== "BUSY" && (
        <div className="agent-control-panel__notice" role={actionState.status === "ERROR" ? "alert" : "status"} aria-live="polite">
          <strong>{actionState.status === "ERROR" ? "Agent action unavailable" : "Agent action status"}</strong>
          <p>{actionState.message}</p>
          {actionState.reference && <small>Reference: {actionState.reference}</small>}
        </div>
      )}
    </section>
  );
}

export function UnavailableControlsNotice({ message, reference }: { message: string; reference?: string }) {
  return (
    <div className="agent-control-panel__notice" role="status">
      <strong>Agent controls are not available yet</strong>
      <p>{message}</p>
      {reference && <small>Reference: {reference}</small>}
    </div>
  );
}

export type DescriptorActions = {
  /** Present only when a served begin-authentication command exists for this desktop build. */
  onBeginAuth?: (authMethodId: string) => void;
  /** Present only when a served open-native-configuration command exists. */
  onOpenNativeConfig?: (targetId: string) => void;
};

const AUTH_TEXT: Record<AgentControlDescriptor["auth"]["state"], string> = {
  UNKNOWN: "Unknown. Sign-in has not been observed.",
  NEEDS_AUTH: "Needs sign-in.",
  AUTHENTICATING: "Authenticating. Complete the sign-in in the agent's own window.",
  AUTHENTICATED: "Signed in (reported by the agent module).",
  EXPIRED: "Sign-in expired.",
  DEGRADED: "Degraded. Sign-in may be partially working.",
  ERROR: "Sign-in failed or was denied.",
};

export function DescriptorSummary({ descriptor, actions = {}, actionBusy = false, stale = false }: { descriptor: AgentControlDescriptor; actions?: DescriptorActions; actionBusy?: boolean; stale?: boolean }) {
  const nativeFlows = descriptor.auth.methods.filter((method) => method.kind === "NATIVE_FLOW");
  const targets = descriptor.configuration.native_config_targets;
  return (
    <div className="agent-control-panel__summary">
      <dl>
        <div><dt>Adapter</dt><dd>{descriptor.module_id} · {descriptor.module_version}</dd></div>
        <div><dt>Installation</dt><dd>{descriptor.installation.state}</dd></div>
        <div><dt>Authentication</dt><dd>{descriptor.auth.state}</dd></div>
        <div><dt>Observed</dt><dd><time dateTime={descriptor.observed_at}>{descriptor.observed_at}</time></dd></div>
        <div><dt>Expires</dt><dd><time dateTime={descriptor.expires_at}>{descriptor.expires_at}</time></dd></div>
      </dl>
      {stale && <p className="agent-control-panel__notice" role="status" aria-live="polite">
        These controls are stale. Refresh controls to check them again; sign-in and configuration actions are paused until then.
      </p>}
      <p>
        Native input: {descriptor.input_surface.text ? "text" : "no text"}
        {descriptor.input_surface.images ? ", images" : ""}
        {descriptor.input_surface.files ? ", files" : ""}
        {descriptor.input_surface.resources ? ", Workspace resources" : ""}.
      </p>
      <p className="agent-control-panel__commands">
        {descriptor.input_surface.slash_commands.length
          ? `Commands: ${descriptor.input_surface.slash_commands.map((command) => command.token).join(", ")}`
          : "No native commands declared."}
      </p>
      <ul className="agent-control-panel__axes" aria-label="Independent readiness axes">
        <li><b>Detection</b>: {descriptor.installation.state === "MISSING" ? "not detected" : "detected"}</li>
        <li><b>Installation</b>: {descriptor.installation.state.toLowerCase().replace(/_/g, " ")}</li>
        <li><b>Configuration</b>: {descriptor.configuration.fields.length ? `${descriptor.configuration.fields.length} declared fields` : "no LiteCowork-managed fields"}</li>
        <li><b>Authentication</b>: {AUTH_TEXT[descriptor.auth.state]}</li>
        <li><b>Readiness</b>: checked separately at send; this descriptor does not prove it.</li>
      </ul>
      {nativeFlows.length === 0
        ? <p className="agent-control-panel__signin">No sign-in method is declared by this agent module.</p>
        : nativeFlows.map((method) => (
          <button
            key={method.auth_method_id}
            type="button"
            disabled={stale || !actions.onBeginAuth || actionBusy || descriptor.auth.state === "AUTHENTICATING"}
            onClick={() => actions.onBeginAuth?.(method.auth_method_id)}
          >
            {descriptor.auth.state === "AUTHENTICATING" ? "Authenticating…" : `Sign in: ${method.display_name}`}
          </button>
        ))}
      {targets.length === 0
        ? <p className="agent-control-panel__native-config">No native configuration target is declared, so Open native config is unavailable.</p>
        : targets.map((target) => (
          <button
            key={target.target_id}
            type="button"
            disabled={stale || !actions.onOpenNativeConfig || actionBusy}
            onClick={() => actions.onOpenNativeConfig?.(target.target_id)}
          >
            Open native config: {target.display_name}
          </button>
        ))}
      <p className="agent-control-panel__native-state">
        Native MCP servers, hooks and extensions are native harness state. LiteCowork does not mediate or record them as Core Effects.
      </p>
      <p className="agent-control-panel__qualification">
        Descriptor observed. Session readiness and send authorization are checked separately.
      </p>
    </div>
  );
}

function toErrorState(key: string, error: unknown): PanelState {
  const parsed = parseAgentControlReadError(error);
  if (parsed.code === "DEPENDENCY_UNAVAILABLE") {
    return {
      key,
      status: "UNAVAILABLE",
      message: "LiteCowork has not verified this agent’s commands and input options yet. Try again later.",
      reference: parsed.reference,
    };
  }
  return {
    key,
    status: "FAILED",
    message: "The local Runtime could not provide current agent controls. Try again when it is available.",
    reference: parsed.reference,
  };
}

function safeId(value: string): string {
  return value.replace(/[^a-zA-Z0-9_-]/g, "-").slice(0, 96) || "agent";
}

export function isAgentControlDescriptorExpired(expiresAt: string, now = Date.now()): boolean {
  const timestamp = Date.parse(expiresAt);
  return !Number.isFinite(timestamp) || timestamp <= now;
}
