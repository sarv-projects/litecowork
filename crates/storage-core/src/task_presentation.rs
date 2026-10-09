//! Bounded, same-read-snapshot source records for the Operator Task presentation.

use crate::{ArtifactRecord, ArtifactVersionRecord, StepRecord, StoreError, TaskView};

pub const MAX_TASK_PRESENTATION_STEPS: usize = 100;
pub const MAX_TASK_PRESENTATION_ARTIFACTS: usize = 200;

#[derive(Clone, Debug, PartialEq)]
pub struct TaskPresentationArtifactVersion {
    pub artifact: ArtifactRecord,
    pub version: ArtifactVersionRecord,
}

/// Latest committed domain event for one Task, Step, or Attempt in the bounded
/// presentation read. Payloads are deliberately excluded; projections use event kind
/// and timestamp only, then derive labels from the corresponding persisted records.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskPresentationActivityEvent {
    pub entity_type: String,
    pub entity_id: String,
    pub event_type: String,
    pub recorded_at: String,
}

/// The persisted current Attempt attached to a current-plan Step, terminal or otherwise.
/// Its latest event may contribute factual historical activity; progress includes it as
/// a workstream only while its stored state is nonterminal. The label is resolved from
/// the same-Workspace AgentProfile and does not prove process/provider liveness.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskPresentationCurrentAttempt {
    pub step_id: String,
    pub attempt_id: String,
    pub status: String,
    pub worker_label: Option<String>,
    pub last_event: Option<TaskPresentationActivityEvent>,
}

/// Authorized Task, current-plan Step, current Attempt, latest domain-event, Evidence,
/// and Artifact source records observed from one consistent storage read snapshot.
/// Activity sources are bounded to the Task and each returned current-plan Step plus
/// at most one current Attempt per Step, including terminal Attempts for latest-activity
/// provenance. Only nonterminal Attempts are rendered as active workstreams. Artifact rows whose current
/// immutable version cannot be resolved are omitted.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskPresentationReadModel {
    pub task: TaskView,
    pub steps: Vec<StepRecord>,
    pub steps_overflow: bool,
    pub artifacts: Vec<TaskPresentationArtifactVersion>,
    pub artifacts_overflow: bool,
    pub activity_events: Vec<TaskPresentationActivityEvent>,
    pub current_attempts: Vec<TaskPresentationCurrentAttempt>,
    pub last_evidence_at: Option<String>,
}

/// Bounded Task presentation source read. Implementations must observe the Task,
/// current-plan Steps, Task Artifacts, resolvable current ArtifactVersions, relevant
/// committed events, current Attempts, and latest Evidence time from one consistent
/// storage snapshot. Returned Step and Artifact counts never exceed the published caps;
/// overflow flags mean the corresponding source count exceeds its cap. It contains no
/// process-liveness, provider-progress, or Environment observations.
pub trait TaskPresentationReadStore: Send + Sync {
    fn get_task_presentation(
        &self,
        workspace_id: &str,
        task_id: &str,
    ) -> Result<Option<TaskPresentationReadModel>, StoreError>;
}
