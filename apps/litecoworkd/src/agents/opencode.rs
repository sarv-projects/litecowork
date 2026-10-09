use super::{AgentInstallation, AuthenticationReadiness, SessionReadiness, discover_version};

pub(super) fn discover() -> AgentInstallation {
    // OpenCode's documented `--version` flag prints the version number itself,
    // unlike Codex's CLI label-prefixed output. The shared parser still accepts
    // only one bounded semver-shaped value and never returns arbitrary output.
    let (installation, version) = discover_version("opencode", "");
    AgentInstallation {
        agent_id: "opencode",
        display_name: "OpenCode",
        protocol_candidate: "ACP",
        installation,
        version,
        authentication: AuthenticationReadiness::Unknown,
        session_readiness: SessionReadiness::NotProbed,
    }
}

pub(super) fn unavailable() -> AgentInstallation {
    AgentInstallation {
        agent_id: "opencode",
        display_name: "OpenCode",
        protocol_candidate: "ACP",
        installation: super::InstallationState::VersionUnavailable,
        version: None,
        authentication: AuthenticationReadiness::Unknown,
        session_readiness: SessionReadiness::NotProbed,
    }
}
