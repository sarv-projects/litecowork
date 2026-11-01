//! Unit tests for the read-only health and drift projections.

use super::*;
use crate::{PlacementPreference, RoutineRevisionInput};

fn revision_input() -> RoutineRevisionInput {
    RoutineRevisionInput {
        objective_template: "Summarize the weekly research.".to_owned(),
        instructions: "Read the pinned resources and report.".to_owned(),
        input_schema: Default::default(),
        constraints: Vec::new(),
        non_goals: Vec::new(),
        required_outputs: Vec::new(),
        acceptance_criteria: Vec::new(),
        approvals_required: Vec::new(),
        input_bindings: Vec::new(),
        required_capabilities: Vec::new(),
        preferred_agent_binding_id: None,
        placement_preference: PlacementPreference::Class(crate::PlacementClass::Auto),
        budget_ceiling: None,
        verification_policy: Default::default(),
    }
}

fn healthy_dependency() -> DependencyHealth {
    DependencyHealth {
        subject_ref: serde_json::json!({"resource_id": "resource-1", "revision_id": "revision-3"}),
        state: DependencyHealthState::Healthy,
        observed_at: Some("2026-03-01T09:00:00Z".to_owned()),
        reason_code: None,
    }
}

#[test]
fn explicit_incompatibility_evidence_is_drifted() {
    let inputs = DriftInputs {
        explicit_drift_evidence: vec!["SKILL_DRIFT_DETECTED".to_owned()],
        transient_failures: 0,
        dependencies: vec![healthy_dependency()],
    };
    assert_eq!(evaluate_drift_state(&inputs), DriftState::Drifted);
}

#[test]
fn a_failed_task_without_evidence_is_only_a_warning() {
    let inputs = DriftInputs {
        explicit_drift_evidence: Vec::new(),
        transient_failures: 2,
        dependencies: vec![healthy_dependency()],
    };
    assert_eq!(evaluate_drift_state(&inputs), DriftState::Warning);
}

#[test]
fn a_degraded_dependency_is_a_warning() {
    let inputs = DriftInputs {
        explicit_drift_evidence: Vec::new(),
        transient_failures: 0,
        dependencies: vec![DependencyHealth {
            subject_ref: serde_json::json!({"resource_id": "resource-2", "revision_id": "revision-1"}),
            state: DependencyHealthState::Degraded,
            observed_at: Some("2026-03-01T09:00:00Z".to_owned()),
            reason_code: Some("SOURCE_STALE".to_owned()),
        }],
    };
    assert_eq!(evaluate_drift_state(&inputs), DriftState::Warning);
}

#[test]
fn the_absence_of_an_observation_is_unknown_not_healthy() {
    let inputs = DriftInputs {
        explicit_drift_evidence: Vec::new(),
        transient_failures: 0,
        dependencies: Vec::new(),
    };
    assert_eq!(evaluate_drift_state(&inputs), DriftState::Unknown);
}

#[test]
fn an_unobserved_dependency_is_unknown() {
    let inputs = DriftInputs {
        explicit_drift_evidence: Vec::new(),
        transient_failures: 0,
        dependencies: vec![DependencyHealth {
            subject_ref: serde_json::json!({"resource_id": "resource-3", "revision_id": "revision-1"}),
            state: DependencyHealthState::Unknown,
            observed_at: None,
            reason_code: None,
        }],
    };
    assert_eq!(evaluate_drift_state(&inputs), DriftState::Unknown);
}

#[test]
fn a_fresh_healthy_observation_is_healthy() {
    let inputs = DriftInputs {
        explicit_drift_evidence: Vec::new(),
        transient_failures: 0,
        dependencies: vec![healthy_dependency()],
    };
    assert_eq!(evaluate_drift_state(&inputs), DriftState::Healthy);
}

#[test]
fn a_repair_proposal_is_only_offered_for_a_drifted_revision() {
    let healthy = DriftInputs {
        explicit_drift_evidence: Vec::new(),
        transient_failures: 1,
        dependencies: vec![healthy_dependency()],
    };
    assert!(propose_drift_repair("routine-1", 2, &revision_input(), &healthy).is_err());
    let drifted = DriftInputs {
        explicit_drift_evidence: vec!["SKILL_DRIFT_DETECTED".to_owned()],
        transient_failures: 0,
        dependencies: vec![healthy_dependency()],
    };
    let proposal =
        propose_drift_repair("routine-1", 2, &revision_input(), &drifted).expect("a proposal exists");
    assert_eq!(proposal.current_revision, 2);
    assert_eq!(proposal.drift_state, DriftState::Drifted);
    assert!(proposal.requires_owner_review);
    // The proposal carries the current content as the draft base. It never
    // rewrites the published revision.
    assert_eq!(proposal.proposed_revision, revision_input());
}

#[test]
fn a_repair_proposal_rejects_a_missing_revision() {
    let drifted = DriftInputs {
        explicit_drift_evidence: vec!["SKILL_DRIFT_DETECTED".to_owned()],
        transient_failures: 0,
        dependencies: Vec::new(),
    };
    assert!(propose_drift_repair("", 0, &revision_input(), &drifted).is_err());
}

#[test]
fn the_drift_reasons_explain_the_state() {
    let inputs = DriftInputs {
        explicit_drift_evidence: vec!["SKILL_DRIFT_DETECTED".to_owned()],
        transient_failures: 1,
        dependencies: vec![DependencyHealth {
            subject_ref: serde_json::json!({"resource_id": "resource-4", "revision_id": "revision-1"}),
            state: DependencyHealthState::Unhealthy,
            observed_at: Some("2026-03-01T09:00:00Z".to_owned()),
            reason_code: Some("SOURCE_MISSING".to_owned()),
        }],
    };
    let reasons = drift_reasons(&inputs);
    assert!(reasons.contains(&DriftReason::ExplicitEvidence(
        "SKILL_DRIFT_DETECTED".to_owned()
    )));
    assert!(reasons.contains(&DriftReason::TransientFailure));
    assert!(reasons.contains(&DriftReason::Dependency(
        serde_json::json!({"resource_id": "resource-4", "revision_id": "revision-1"})
    )));
}
