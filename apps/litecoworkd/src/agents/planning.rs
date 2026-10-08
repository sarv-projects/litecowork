//! Runtime-local planning preflight. The existing native transports do not provide
//! Task isolation or lifecycle settlement, so preflight never starts a provider.
//! This gate applies only to this preflight path. Native thread/turn constructors
//! remain separately callable internal transport operations; this is not a global
//! enforcement boundary or proof that those constructors are admitted planners.

use domain_task::{PreparePlanningAssignment, TaskPlanningPacket, TaskService};
use serde::Serialize;
use storage_core::{AgentCatalogStore, StoreError, TaskStore};

/// Qualification/admission gaps, not persisted Task blockers or public ErrorCodes.
/// These observations cannot change Task/Step/Attempt state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum PlanningDispatchBlocker {
    TaskStateNotEligible,
    PlanAlreadyAccepted,
    CurrentLeadOrEndpointUnavailable,
    TaskIsolationUnavailable,
    NativeCapabilitiesUnmediated,
    ProtocolUnqualified,
    ProcessContainmentUnqualified,
    PlanningContextResourceUnavailable,
    SessionSettlementUnavailable,
    ProviderUnsupported,
}

/// Safe projection of a preflight, without model input or native configuration.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct LocalPlanningPreflightView {
    pub task_id: String,
    pub task_version: u64,
    pub task_spec_revision: u64,
    pub lead_agent_binding_id: String,
    pub endpoint_id: String,
    pub dispatch_available: bool,
    pub dispatch_blockers: Vec<PlanningDispatchBlocker>,
}

/// Bounded input prepared from storage. It carries no producer authority and does
/// not reserve an AgentSession, activate a Task, or create an execution Attempt.
/// No Debug/Serialize implementation: a model prompt is not diagnostic output.
pub(crate) struct LocalPlanningPreflight {
    view: LocalPlanningPreflightView,
    packet: TaskPlanningPacket,
}

impl LocalPlanningPreflight {
    pub(crate) fn view(&self) -> &LocalPlanningPreflightView { &self.view }
    pub(crate) fn packet(&self) -> &TaskPlanningPacket { &self.packet }

    /// A mandatory check before session reservation, not after process startup.
    /// Readiness probes and installed executables cannot satisfy these boundaries.
    pub(crate) fn require_dispatch(&self) -> Result<(), StoreError> {
        Err(StoreError::Invalid("native Task planning dispatch is unavailable until isolation and lifecycle qualification are implemented".to_owned()))
    }
}

/// Authenticate the owner before this internal call. TaskService resolves the
/// persisted lead and fresh endpoint; storage remains the later admission authority.
/// The Runtime identity must come from litecoworkd, never the request JSON.
pub(crate) fn prepare_local_task_planning<S: TaskStore + AgentCatalogStore>(
    store: S,
    request: PreparePlanningAssignment,
) -> Result<LocalPlanningPreflight, StoreError> {
    let assignment = TaskService::new(store).prepare_planning_assignment(request)?;
    let packet = TaskPlanningPacket::from_assignment(&assignment)?;
    let dispatch_blockers = dispatch_blockers(&assignment.endpoint.protocol);
    Ok(LocalPlanningPreflight {
        view: LocalPlanningPreflightView {
            task_id: packet.task_id().to_owned(),
            task_version: packet.task_version(),
            task_spec_revision: packet.task_spec_revision(),
            lead_agent_binding_id: packet.lead_agent_binding_id().to_owned(),
            endpoint_id: assignment.endpoint.endpoint_id,
            dispatch_available: false,
            dispatch_blockers,
        },
        packet,
    })
}

fn dispatch_blockers(protocol: &str) -> Vec<PlanningDispatchBlocker> {
    let mut blockers = vec![
        PlanningDispatchBlocker::TaskIsolationUnavailable,
        PlanningDispatchBlocker::NativeCapabilitiesUnmediated,
        PlanningDispatchBlocker::ProtocolUnqualified,
        PlanningDispatchBlocker::ProcessContainmentUnqualified,
        PlanningDispatchBlocker::PlanningContextResourceUnavailable,
        PlanningDispatchBlocker::SessionSettlementUnavailable,
    ];
    if !matches!(protocol, "CODEX_APP_SERVER" | "OPENCODE_SERVER") {
        blockers.push(PlanningDispatchBlocker::ProviderUnsupported);
    }
    blockers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn existing_native_transports_always_retain_dispatch_gates() {
        for protocol in ["CODEX_APP_SERVER", "OPENCODE_SERVER"] {
            let blockers = dispatch_blockers(protocol);
            assert!(blockers.contains(&PlanningDispatchBlocker::TaskIsolationUnavailable));
            assert!(blockers.contains(&PlanningDispatchBlocker::NativeCapabilitiesUnmediated));
            assert!(blockers.contains(&PlanningDispatchBlocker::SessionSettlementUnavailable));
            assert!(!blockers.contains(&PlanningDispatchBlocker::ProviderUnsupported));
        }
        assert!(dispatch_blockers("UNKNOWN").contains(&PlanningDispatchBlocker::ProviderUnsupported));
    }
}
