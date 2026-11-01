import { useEffect, useRef, useState } from "react";
import {
  parseAgentControlReadError,
  type AgentControlDescriptor,
  type AgentControlDescriptorApi,
} from "./agent-control-api";
import { desktopAgentControlDescriptorApi } from "./desktop-agent-control-api";
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
};

export function AgentControlDescriptorPanel({
  workspaceId,
  profileId,
  displayName,
  api = desktopAgentControlDescriptorApi,
}: AgentControlDescriptorPanelProps) {
  const key = `${workspaceId}\u0000${profileId}`;
  const generation = useRef(0);
  const [state, setState] = useState<PanelState>({ key, status: "LOADING" });

  useEffect(() => {
    const request = ++generation.current;
    let active = true;
    setState({ key, status: "LOADING" });
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
    };
  }, [api, key, profileId, workspaceId]);

  const refresh = () => {
    const request = ++generation.current;
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

  const visible = state.key === key ? state : { key, status: "LOADING" as const };
  return (
    <section className="agent-control-panel" aria-labelledby={`agent-controls-${safeId(profileId)}`}>
      <div className="agent-control-panel__heading">
        <div>
          <h4 id={`agent-controls-${safeId(profileId)}`}>Native controls</h4>
          <p>Verified controls for {displayName}. Reading these does not start or authorize a conversation.</p>
        </div>
        <button type="button" onClick={refresh} disabled={visible.status === "LOADING"}>
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
      {visible.status === "READY" && <DescriptorSummary descriptor={visible.descriptor} />}
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

function DescriptorSummary({ descriptor }: { descriptor: AgentControlDescriptor }) {
  return (
    <div className="agent-control-panel__summary">
      <dl>
        <div><dt>Adapter</dt><dd>{descriptor.module_id} · {descriptor.module_version}</dd></div>
        <div><dt>Installation</dt><dd>{descriptor.installation.state}</dd></div>
        <div><dt>Authentication</dt><dd>{descriptor.auth.state}</dd></div>
        <div><dt>Observed</dt><dd><time dateTime={descriptor.observed_at}>{descriptor.observed_at}</time></dd></div>
        <div><dt>Expires</dt><dd><time dateTime={descriptor.expires_at}>{descriptor.expires_at}</time></dd></div>
      </dl>
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
