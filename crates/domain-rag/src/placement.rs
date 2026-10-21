//! Resource placement rules.
//!
//! Placement is explicit: a Resource is either usable only on this device or it is
//! eligible for a cloud consumer. LiteCowork Core owns the decision. A UI or an Agent
//! may not widen it, and a local-only Resource blocks a cloud consumer instead of
//! being silently uploaded.

use serde::{Deserialize, Serialize};

/// The Workspace replication policy from `DATA-MODEL.md`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkspaceReplicationPolicy {
    LocalOnly,
    MetadataOnly,
    ActiveTaskInputs,
    SelectedFolders,
}

impl WorkspaceReplicationPolicy {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "LOCAL_ONLY" => Some(Self::LocalOnly),
            "METADATA_ONLY" => Some(Self::MetadataOnly),
            "ACTIVE_TASK_INPUTS" => Some(Self::ActiveTaskInputs),
            "SELECTED_FOLDERS" => Some(Self::SelectedFolders),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::LocalOnly => "LOCAL_ONLY",
            Self::MetadataOnly => "METADATA_ONLY",
            Self::ActiveTaskInputs => "ACTIVE_TASK_INPUTS",
            Self::SelectedFolders => "SELECTED_FOLDERS",
        }
    }
}

/// Where a Resource may be consumed.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResourcePlacement {
    /// The Resource and its content stay on this device.
    LocalOnly,
    /// The Resource has a replicated or provider-backed location and may be consumed by
    /// a cloud consumer when policy allows it.
    CloudEligible,
}

/// Where a consumer runs.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaskPlacement {
    Local,
    Cloud,
}

/// The Core-owned inputs of one placement decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlacementInputs {
    pub policy: WorkspaceReplicationPolicy,
    /// True when the Resource has a location that is not the local managed blob.
    pub has_remote_location: bool,
    /// True when the Resource was explicitly created as local-only.
    pub declared_local_only: bool,
}

/// Why a consumer may not use a Resource.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlacementBlocker {
    pub code: &'static str,
    pub reason: String,
}

/// Resolves the placement of one Resource.
///
/// An explicit local-only declaration always wins. Otherwise the Workspace policy
/// decides: `LOCAL_ONLY` and `METADATA_ONLY` keep content on this device, and the
/// remaining policies need a replicated or provider-backed location before content may
/// leave the device.
pub fn resolve_resource_placement(inputs: PlacementInputs) -> ResourcePlacement {
    if inputs.declared_local_only {
        return ResourcePlacement::LocalOnly;
    }
    match inputs.policy {
        WorkspaceReplicationPolicy::LocalOnly | WorkspaceReplicationPolicy::MetadataOnly => {
            ResourcePlacement::LocalOnly
        }
        WorkspaceReplicationPolicy::ActiveTaskInputs
        | WorkspaceReplicationPolicy::SelectedFolders => {
            if inputs.has_remote_location {
                ResourcePlacement::CloudEligible
            } else {
                ResourcePlacement::LocalOnly
            }
        }
    }
}

/// Returns the blocker when a consumer may not use a Resource.
pub fn evaluate_task_placement(
    resource_placement: ResourcePlacement,
    task_placement: TaskPlacement,
) -> Result<(), PlacementBlocker> {
    match (resource_placement, task_placement) {
        (ResourcePlacement::LocalOnly, TaskPlacement::Cloud) => Err(PlacementBlocker {
            code: "RESOURCE_LOCAL_ONLY",
            reason: "This Resource is local-only and cannot be resolved for a cloud Task."
                .to_owned(),
        }),
        _ => Ok(()),
    }
}

/// The observable placement state a UI renders.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PlacementDecision {
    pub resource_id: String,
    pub placement: ResourcePlacement,
    pub policy: WorkspaceReplicationPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<String>,
}

impl PlacementDecision {
    pub fn resolve(
        resource_id: &str,
        inputs: PlacementInputs,
        task_placement: TaskPlacement,
    ) -> Self {
        let placement = resolve_resource_placement(inputs);
        let blocker = evaluate_task_placement(placement, task_placement).err();
        Self {
            resource_id: resource_id.to_owned(),
            placement,
            policy: inputs.policy,
            blocked_code: blocker
                .as_ref()
                .map(|blocker| blocker.code.to_owned()),
            blocked_reason: blocker.map(|blocker| blocker.reason),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PlacementInputs, ResourcePlacement, TaskPlacement, WorkspaceReplicationPolicy,
        evaluate_task_placement, resolve_resource_placement,
    };

    #[test]
    fn an_explicit_local_only_declaration_wins_over_policy() {
        let placement = resolve_resource_placement(PlacementInputs {
            policy: WorkspaceReplicationPolicy::ActiveTaskInputs,
            has_remote_location: true,
            declared_local_only: true,
        });
        assert_eq!(placement, ResourcePlacement::LocalOnly);
    }

    #[test]
    fn a_local_only_workspace_keeps_every_resource_local() {
        let placement = resolve_resource_placement(PlacementInputs {
            policy: WorkspaceReplicationPolicy::LocalOnly,
            has_remote_location: true,
            declared_local_only: false,
        });
        assert_eq!(placement, ResourcePlacement::LocalOnly);
    }

    #[test]
    fn a_metadata_only_workspace_keeps_content_local() {
        let placement = resolve_resource_placement(PlacementInputs {
            policy: WorkspaceReplicationPolicy::MetadataOnly,
            has_remote_location: true,
            declared_local_only: false,
        });
        assert_eq!(placement, ResourcePlacement::LocalOnly);
    }

    #[test]
    fn a_replicated_resource_is_cloud_eligible_only_with_a_remote_location() {
        let replicated = resolve_resource_placement(PlacementInputs {
            policy: WorkspaceReplicationPolicy::ActiveTaskInputs,
            has_remote_location: true,
            declared_local_only: false,
        });
        assert_eq!(replicated, ResourcePlacement::CloudEligible);

        let local = resolve_resource_placement(PlacementInputs {
            policy: WorkspaceReplicationPolicy::ActiveTaskInputs,
            has_remote_location: false,
            declared_local_only: false,
        });
        assert_eq!(local, ResourcePlacement::LocalOnly);
    }

    #[test]
    fn a_local_only_resource_blocks_a_cloud_consumer() {
        let blocker =
            evaluate_task_placement(ResourcePlacement::LocalOnly, TaskPlacement::Cloud).unwrap_err();
        assert_eq!(blocker.code, "RESOURCE_LOCAL_ONLY");
        assert!(evaluate_task_placement(ResourcePlacement::LocalOnly, TaskPlacement::Local).is_ok());
        assert!(
            evaluate_task_placement(ResourcePlacement::CloudEligible, TaskPlacement::Cloud).is_ok()
        );
    }

    #[test]
    fn an_unknown_policy_value_is_not_parsed() {
        assert!(WorkspaceReplicationPolicy::parse("SOMETHING_ELSE").is_none());
        assert_eq!(
            WorkspaceReplicationPolicy::parse("LOCAL_ONLY"),
            Some(WorkspaceReplicationPolicy::LocalOnly)
        );
    }
}
