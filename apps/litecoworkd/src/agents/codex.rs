use super::{
    AgentInstallation, AuthenticationReadiness, SessionReadiness, discover_version,
};

pub(super) fn discover() -> AgentInstallation {
    let (installation, version) = discover_version("codex", "codex-cli ");
    AgentInstallation {
        agent_id: "codex-cli",
        display_name: "Codex",
        protocol_candidate: "CODEX_APP_SERVER",
        installation,
        version,
        authentication: AuthenticationReadiness::Unknown,
        session_readiness: SessionReadiness::NotProbed,
    }
}

pub(super) fn unavailable() -> AgentInstallation {
    AgentInstallation {
        agent_id: "codex-cli",
        display_name: "Codex",
        protocol_candidate: "CODEX_APP_SERVER",
        installation: super::InstallationState::VersionUnavailable,
        version: None,
        authentication: AuthenticationReadiness::Unknown,
        session_readiness: SessionReadiness::NotProbed,
    }
}
