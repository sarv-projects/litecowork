//! Bounded, same-read-snapshot source records for the Operator Task presentation.

use crate::{ArtifactRecord, ArtifactVersionRecord, StoreError, StepRecord, TaskView};

pub const MAX_TASK_PRESENTATION_STEPS: usize = 100;
pub const MAX_TASK_PRESENTATION_ARTIFACTS: usize = 200;

#[derive(Clone, Debug, PartialEq)]
pub struct TaskPresentationArtifactVersion {
    pub artifact: ArtifactRecord,
    pub version: ArtifactVersionRecord,
}

/// Authorized source records observed from one consistent storage read snapshot.
/// Artifact rows whose current immutable version cannot be resolved are omitted.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskPresentationReadModel {
    pub task: TaskView,
    pub steps: Vec<StepRecord>,
    pub steps_overflow: bool,
    pub artifacts: Vec<TaskPresentationArtifactVersion>,
    pub artifacts_overflow: bool,
}

/// Bounded Task presentation source read. Implementations must observe the Task,
/// current-plan Steps, Task Artifacts, and resolvable current ArtifactVersions from
/// one consistent storage snapshot. Returned record counts never exceed the published
/// caps; overflow flags mean the corresponding source count exceeds its cap.
pub trait TaskPresentationReadStore: Send + Sync {
    fn get_task_presentation(
        &self,
        workspace_id: &str,
        task_id: &str,
    ) -> Result<Option<TaskPresentationReadModel>, StoreError>;
}
