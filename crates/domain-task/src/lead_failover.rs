//! Policy evaluation for a possible Task lead failover.
//!
//! This module returns a version-fenced decision only. It does not change a Task,
//! start a session, or transfer authority. The caller must revalidate every readiness
//! fact and commit the lead change and handoff through TaskService.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

const MAX_FALLBACKS: usize = 3;
const MAX_LEAD_CHANGES: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FailoverPolicyMode {
    Disabled,
    Ask,
    AllowListed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FailoverTrigger {
    AgentUnavailable,
    QuotaExhausted,
    RuntimeUnavailable,
}

impl FailoverTrigger {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AgentUnavailable => "AGENT_UNAVAILABLE",
            Self::QuotaExhausted => "QUOTA_EXHAUSTED",
            Self::RuntimeUnavailable => "RUNTIME_UNAVAILABLE",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FailoverPolicy {
    pub mode: FailoverPolicyMode,
    pub triggers: Vec<FailoverTrigger>,
    pub fallback_agent_binding_ids: Vec<String>,
    pub max_lead_changes: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidFailoverPolicy;

/// Validate the canonical JSON policy stored on a TaskSpecRevision. Keeping the
/// typed parser at the domain boundary prevents malformed or forward-unknown fields
/// from becoming durable continuation policy. This does not itself authorize a
/// failover; `decide_lead_failover` still requires fresh provider/runtime evidence.
pub fn validate_failover_policy_value(value: &Value) -> bool {
    serde_json::from_value::<FailoverPolicy>(value.clone())
        .is_ok_and(|policy| valid_policy(&policy))
}

/// Resolve the Task's immutable failover policy from an explicit owner request,
/// otherwise its pinned Coworker default, otherwise the disabled default. Every
/// resolved value passes the same strict domain validation before it is stored.
pub fn resolve_task_failover_policy(
    explicit: Option<&Value>,
    coworker_default: Option<Value>,
) -> Result<Value, InvalidFailoverPolicy> {
    let value = explicit.cloned().or(coworker_default).unwrap_or_else(|| {
        serde_json::json!({
            "mode":"DISABLED",
            "triggers":[],
            "fallback_agent_binding_ids":[],
            "max_lead_changes":0
        })
    });
    validate_failover_policy_value(&value)
        .then_some(value)
        .ok_or(InvalidFailoverPolicy)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailoverTriggerState {
    Confirmed,
    Unconfirmed,
}

/// Sanitized provider/runtime observation. `source_observation_id` must resolve to
/// the authenticated adapter/runtime observation; it is not provider credentials.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailoverTriggerObservation {
    pub trigger: FailoverTrigger,
    pub state: FailoverTriggerState,
    pub affected_agent_binding_id: String,
    pub source: String,
    pub source_observation_id: String,
    pub runtime_incarnation_id: Option<String>,
    pub observed_at: String,
    pub expires_at: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailoverCandidateReadiness {
    /// The owning admission layer freshly checked Workspace membership, binding
    /// enablement/lead eligibility, adapter options, endpoint/runtime, auth, Trust,
    /// inputs, deadline, and remaining budget for this exact Task version.
    Qualified,
    Ineligible,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailoverCandidate {
    pub agent_binding_id: String,
    pub readiness: FailoverCandidateReadiness,
    pub checked_task_version: u64,
    pub checked_task_spec_revision: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeadFailoverBlocker {
    InvalidPolicy,
    InvalidContext,
    TriggerNotConfirmed,
    TriggerNotAllowed,
    ObservationStale,
    ObservationDoesNotMatchCurrentLead,
    ChangeLimitReached,
    NoEligibleFallback,
    UnsafeBoundary,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FailoverDecision {
    Disabled,
    AskOwner {
        trigger: FailoverTrigger,
        observation_id: String,
        /// Ordered, currently eligible fallback choices to present to the owner. The
        /// order is the pinned policy order; the current lead is never offered.
        fallback_agent_binding_ids: Vec<String>,
    },
    Recommend {
        agent_binding_id: String,
        expected_task_version: u64,
        task_spec_revision: u64,
        trigger: FailoverTrigger,
        observation_id: String,
        next_change_number: u32,
    },
    Blocked(LeadFailoverBlocker),
}

/// Evaluate only the pinned policy and freshly observed facts. A recommendation is
/// not authority to change the lead; TaskService must compare the returned Task/spec
/// versions and repeat admission checks in its commit transaction.
#[allow(clippy::too_many_arguments)]
pub fn decide_lead_failover(
    policy: &FailoverPolicy,
    observation: &FailoverTriggerObservation,
    current_lead_binding_id: &str,
    current_lead_runtime_incarnation_id: Option<&str>,
    expected_task_version: u64,
    task_spec_revision: u64,
    prior_policy_failover_count: u32,
    now: &str,
    safe_boundary: bool,
    candidates: &[FailoverCandidate],
) -> FailoverDecision {
    if !valid_policy(policy) {
        return FailoverDecision::Blocked(LeadFailoverBlocker::InvalidPolicy);
    }
    if !valid_id(current_lead_binding_id)
        || expected_task_version == 0
        || task_spec_revision == 0
        || prior_policy_failover_count > MAX_LEAD_CHANGES
        || current_lead_runtime_incarnation_id.is_some_and(|value| !valid_id(value))
    {
        return FailoverDecision::Blocked(LeadFailoverBlocker::InvalidContext);
    }
    if policy.mode == FailoverPolicyMode::Disabled {
        return FailoverDecision::Disabled;
    }
    if observation.state != FailoverTriggerState::Confirmed {
        return FailoverDecision::Blocked(LeadFailoverBlocker::TriggerNotConfirmed);
    }
    if !policy.triggers.contains(&observation.trigger) {
        return FailoverDecision::Blocked(LeadFailoverBlocker::TriggerNotAllowed);
    }
    if observation.affected_agent_binding_id != current_lead_binding_id {
        return FailoverDecision::Blocked(LeadFailoverBlocker::ObservationDoesNotMatchCurrentLead);
    }
    if !valid_id(&observation.source)
        || !valid_id(&observation.source_observation_id)
        || observation
            .runtime_incarnation_id
            .as_ref()
            .is_some_and(|value| !valid_id(value))
        || (observation.trigger == FailoverTrigger::RuntimeUnavailable
            && observation.runtime_incarnation_id.is_none())
        || observation
            .runtime_incarnation_id
            .as_deref()
            .is_some_and(|observed| current_lead_runtime_incarnation_id != Some(observed))
    {
        return FailoverDecision::Blocked(LeadFailoverBlocker::InvalidContext);
    }
    let Some(now) = parse_time(now) else {
        return FailoverDecision::Blocked(LeadFailoverBlocker::InvalidContext);
    };
    let (Some(observed_at), Some(expires_at)) = (
        parse_time(&observation.observed_at),
        parse_time(&observation.expires_at),
    ) else {
        return FailoverDecision::Blocked(LeadFailoverBlocker::ObservationStale);
    };
    if observed_at > now || expires_at <= now || expires_at <= observed_at {
        return FailoverDecision::Blocked(LeadFailoverBlocker::ObservationStale);
    }
    let mut candidate_ids = std::collections::BTreeSet::new();
    if candidates.iter().any(|candidate| {
        !valid_id(&candidate.agent_binding_id) || !candidate_ids.insert(&candidate.agent_binding_id)
    }) {
        return FailoverDecision::Blocked(LeadFailoverBlocker::InvalidContext);
    }
    // The candidate list carries only the bindings that are eligible in this Task's
    // Workspace. `policy.fallback_agent_binding_ids` fixes the order, and the current
    // lead is never a candidate for itself. The first entry is therefore the same
    // binding the durable handoff transaction independently derives, so a placement
    // failure on it blocks instead of promoting a later fallback.
    let ordered_candidates: Vec<&FailoverCandidate> = policy
        .fallback_agent_binding_ids
        .iter()
        .filter(|candidate_id| candidate_id.as_str() != current_lead_binding_id)
        .filter_map(|candidate_id| {
            candidates
                .iter()
                .find(|candidate| candidate.agent_binding_id == *candidate_id)
        })
        .collect();
    let is_freshly_checked = |candidate: &FailoverCandidate| {
        candidate.readiness == FailoverCandidateReadiness::Qualified
            && candidate.checked_task_version == expected_task_version
            && candidate.checked_task_spec_revision == task_spec_revision
    };
    if policy.mode == FailoverPolicyMode::Ask {
        return FailoverDecision::AskOwner {
            trigger: observation.trigger,
            observation_id: observation.source_observation_id.clone(),
            fallback_agent_binding_ids: ordered_candidates
                .into_iter()
                .filter(|candidate| is_freshly_checked(candidate))
                .map(|candidate| candidate.agent_binding_id.clone())
                .collect(),
        };
    }
    if prior_policy_failover_count >= policy.max_lead_changes {
        return FailoverDecision::Blocked(LeadFailoverBlocker::ChangeLimitReached);
    }
    if !safe_boundary {
        return FailoverDecision::Blocked(LeadFailoverBlocker::UnsafeBoundary);
    }

    let Some(candidate) = ordered_candidates.first().copied() else {
        return FailoverDecision::Blocked(LeadFailoverBlocker::NoEligibleFallback);
    };
    if !is_freshly_checked(candidate) {
        return FailoverDecision::Blocked(LeadFailoverBlocker::NoEligibleFallback);
    }
    FailoverDecision::Recommend {
        agent_binding_id: candidate.agent_binding_id.clone(),
        expected_task_version,
        task_spec_revision,
        trigger: observation.trigger,
        observation_id: observation.source_observation_id.clone(),
        next_change_number: prior_policy_failover_count + 1,
    }
}

fn valid_policy(policy: &FailoverPolicy) -> bool {
    let unique_triggers = policy
        .triggers
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let unique_fallbacks = policy
        .fallback_agent_binding_ids
        .iter()
        .collect::<std::collections::BTreeSet<_>>();
    if unique_triggers.len() != policy.triggers.len()
        || policy.fallback_agent_binding_ids.len() > MAX_FALLBACKS
        || unique_fallbacks.len() != policy.fallback_agent_binding_ids.len()
        || policy
            .fallback_agent_binding_ids
            .iter()
            .any(|id| !valid_id(id))
    {
        return false;
    }
    match policy.mode {
        FailoverPolicyMode::Disabled => {
            policy.triggers.is_empty()
                && policy.fallback_agent_binding_ids.is_empty()
                && policy.max_lead_changes == 0
        }
        FailoverPolicyMode::Ask => {
            !policy.triggers.is_empty()
                && policy.max_lead_changes == 0
                && policy.fallback_agent_binding_ids.len() <= MAX_FALLBACKS
        }
        FailoverPolicyMode::AllowListed => {
            !policy.triggers.is_empty()
                && !policy.fallback_agent_binding_ids.is_empty()
                && (1..=MAX_LEAD_CHANGES).contains(&policy.max_lead_changes)
                && policy.max_lead_changes as usize <= policy.fallback_agent_binding_ids.len()
        }
    }
}

fn valid_id(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 512 && !value.chars().any(char::is_control)
}

fn parse_time(value: &str) -> Option<OffsetDateTime> {
    if value.is_empty() || value.len() > 64 || value.chars().any(char::is_control) {
        return None;
    }
    OffsetDateTime::parse(value, &Rfc3339).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_spec_failover_policy_json_is_strict_and_structurally_validated() {
        let disabled = serde_json::json!({
            "mode":"DISABLED",
            "triggers":[],
            "fallback_agent_binding_ids":[],
            "max_lead_changes":0
        });
        assert!(validate_failover_policy_value(&disabled));

        let ask = serde_json::json!({
            "mode":"ASK",
            "triggers":["AGENT_UNAVAILABLE"],
            "fallback_agent_binding_ids":["binding-a"],
            "max_lead_changes":0
        });
        assert!(validate_failover_policy_value(&ask));

        let allow_listed = serde_json::json!({
            "mode":"ALLOW_LISTED",
            "triggers":["QUOTA_EXHAUSTED", "RUNTIME_UNAVAILABLE"],
            "fallback_agent_binding_ids":["binding-a", "binding-b"],
            "max_lead_changes":2
        });
        assert!(validate_failover_policy_value(&allow_listed));

        for invalid in [
            serde_json::json!({
                "mode":"DISABLED", "triggers":["AGENT_UNAVAILABLE"],
                "fallback_agent_binding_ids":[], "max_lead_changes":0
            }),
            serde_json::json!({
                "mode":"ASK", "triggers":[],
                "fallback_agent_binding_ids":[], "max_lead_changes":0
            }),
            serde_json::json!({
                "mode":"ALLOW_LISTED", "triggers":["AGENT_UNAVAILABLE"],
                "fallback_agent_binding_ids":["binding-a"], "max_lead_changes":2
            }),
            serde_json::json!({
                "mode":"ALLOW_LISTED", "triggers":["AGENT_UNAVAILABLE", "AGENT_UNAVAILABLE"],
                "fallback_agent_binding_ids":["binding-a"], "max_lead_changes":1
            }),
            serde_json::json!({
                "mode":"ALLOW_LISTED", "triggers":["AGENT_UNAVAILABLE"],
                "fallback_agent_binding_ids":["binding-a", "binding-a"], "max_lead_changes":1
            }),
            serde_json::json!({
                "mode":"ASK", "triggers":["AGENT_UNAVAILABLE"],
                "fallback_agent_binding_ids":[], "max_lead_changes":0,
                "automatically_switch":true
            }),
            serde_json::json!({"mode":"SURPRISE"}),
            serde_json::json!(null),
        ] {
            assert!(
                !validate_failover_policy_value(&invalid),
                "unexpectedly accepted {invalid}"
            );
        }
    }

    #[test]
    fn task_policy_resolution_pins_explicit_over_coworker_and_defaults_disabled() {
        let coworker = serde_json::json!({
            "mode":"ASK",
            "triggers":["AGENT_UNAVAILABLE"],
            "fallback_agent_binding_ids":[],
            "max_lead_changes":0
        });
        let explicit = serde_json::json!({
            "mode":"DISABLED",
            "triggers":[],
            "fallback_agent_binding_ids":[],
            "max_lead_changes":0
        });
        assert_eq!(
            resolve_task_failover_policy(Some(&explicit), Some(coworker.clone())).unwrap(),
            explicit
        );
        assert_eq!(
            resolve_task_failover_policy(None, Some(coworker.clone())).unwrap(),
            coworker
        );
        let disabled = resolve_task_failover_policy(None, None).unwrap();
        assert_eq!(disabled["mode"], "DISABLED");
        assert!(
            resolve_task_failover_policy(
                Some(&Value::Null),
                Some(serde_json::json!({
                    "mode":"ASK",
                    "triggers":["AGENT_UNAVAILABLE"],
                    "fallback_agent_binding_ids":[],
                    "max_lead_changes":0
                }))
            )
            .is_err()
        );
        assert!(
            resolve_task_failover_policy(
                None,
                Some(serde_json::json!({
                    "mode":"ALLOW_LISTED",
                    "triggers":["AGENT_UNAVAILABLE"],
                    "fallback_agent_binding_ids":["binding-a"],
                    "max_lead_changes":2
                }))
            )
            .is_err()
        );
    }

    fn policy(mode: FailoverPolicyMode) -> FailoverPolicy {
        match mode {
            FailoverPolicyMode::Disabled => FailoverPolicy {
                mode,
                triggers: vec![],
                fallback_agent_binding_ids: vec![],
                max_lead_changes: 0,
            },
            FailoverPolicyMode::Ask => FailoverPolicy {
                mode,
                triggers: vec![FailoverTrigger::QuotaExhausted],
                fallback_agent_binding_ids: vec!["fallback-a".into()],
                max_lead_changes: 0,
            },
            FailoverPolicyMode::AllowListed => FailoverPolicy {
                mode,
                triggers: vec![FailoverTrigger::QuotaExhausted],
                fallback_agent_binding_ids: vec!["fallback-a".into(), "fallback-b".into()],
                max_lead_changes: 2,
            },
        }
    }

    fn observation() -> FailoverTriggerObservation {
        FailoverTriggerObservation {
            trigger: FailoverTrigger::QuotaExhausted,
            state: FailoverTriggerState::Confirmed,
            affected_agent_binding_id: "lead".into(),
            source: "codex-adapter".into(),
            source_observation_id: "quota-observation-1".into(),
            runtime_incarnation_id: Some("runtime-incarnation-1".into()),
            observed_at: "2026-10-17T12:00:00Z".into(),
            expires_at: "2026-10-17T12:01:00Z".into(),
        }
    }

    fn candidate(id: &str, readiness: FailoverCandidateReadiness) -> FailoverCandidate {
        FailoverCandidate {
            agent_binding_id: id.into(),
            readiness,
            checked_task_version: 7,
            checked_task_spec_revision: 3,
        }
    }

    fn decide(
        policy: &FailoverPolicy,
        observation: &FailoverTriggerObservation,
        count: u32,
        safe: bool,
        candidates: &[FailoverCandidate],
    ) -> FailoverDecision {
        decide_lead_failover(
            policy,
            observation,
            "lead",
            Some("runtime-incarnation-1"),
            7,
            3,
            count,
            "2026-10-17T12:00:30Z",
            safe,
            candidates,
        )
    }

    #[test]
    fn disabled_policy_never_changes_the_lead() {
        assert_eq!(
            decide(
                &policy(FailoverPolicyMode::Disabled),
                &observation(),
                0,
                true,
                &[]
            ),
            FailoverDecision::Disabled
        );
    }

    #[test]
    fn ask_policy_requests_owner_decision_without_recommending_a_fallback() {
        assert_eq!(
            decide(
                &policy(FailoverPolicyMode::Ask),
                &observation(),
                0,
                true,
                &[]
            ),
            FailoverDecision::AskOwner {
                trigger: FailoverTrigger::QuotaExhausted,
                observation_id: "quota-observation-1".into(),
                fallback_agent_binding_ids: vec![],
            }
        );
    }

    #[test]
    fn ask_policy_presents_the_ordered_eligible_fallback_choices() {
        let mut with_choices = policy(FailoverPolicyMode::Ask);
        with_choices.fallback_agent_binding_ids = vec!["fallback-a".into(), "fallback-b".into()];
        // `ASK` presents the ordered fallback list as the owner's choices, so a policy
        // with an empty list cannot present a choice at all.
        assert!(validate_failover_policy_value(
            &serde_json::to_value(&with_choices).unwrap()
        ));
        let actual = decide(
            &with_choices,
            &observation(),
            0,
            true,
            &[
                candidate("fallback-b", FailoverCandidateReadiness::Qualified),
                candidate("fallback-a", FailoverCandidateReadiness::Qualified),
                candidate("fallback-c", FailoverCandidateReadiness::Ineligible),
            ],
        );
        assert_eq!(
            actual,
            FailoverDecision::AskOwner {
                trigger: FailoverTrigger::QuotaExhausted,
                observation_id: "quota-observation-1".into(),
                fallback_agent_binding_ids: vec!["fallback-a".into(), "fallback-b".into()],
            }
        );
    }

    #[test]
    fn allow_listed_uses_policy_order_and_returns_version_fenced_recommendation() {
        let actual = decide(
            &policy(FailoverPolicyMode::AllowListed),
            &observation(),
            0,
            true,
            &[
                candidate("fallback-b", FailoverCandidateReadiness::Qualified),
                candidate("fallback-a", FailoverCandidateReadiness::Qualified),
            ],
        );
        assert_eq!(
            actual,
            FailoverDecision::Recommend {
                agent_binding_id: "fallback-a".into(),
                expected_task_version: 7,
                task_spec_revision: 3,
                trigger: FailoverTrigger::QuotaExhausted,
                observation_id: "quota-observation-1".into(),
                next_change_number: 1,
            }
        );
    }

    #[test]
    fn unconfirmed_or_unconfigured_trigger_never_fails_over() {
        let mut report = observation();
        report.state = FailoverTriggerState::Unconfirmed;
        assert_eq!(
            decide(
                &policy(FailoverPolicyMode::AllowListed),
                &report,
                0,
                true,
                &[]
            ),
            FailoverDecision::Blocked(LeadFailoverBlocker::TriggerNotConfirmed)
        );
        report.state = FailoverTriggerState::Confirmed;
        report.trigger = FailoverTrigger::AgentUnavailable;
        assert_eq!(
            decide(
                &policy(FailoverPolicyMode::AllowListed),
                &report,
                0,
                true,
                &[]
            ),
            FailoverDecision::Blocked(LeadFailoverBlocker::TriggerNotAllowed)
        );
    }

    #[test]
    fn stale_wrong_lead_or_expired_observations_are_rejected() {
        let mut report = observation();
        report.expires_at = "2026-10-17T12:00:20Z".into();
        assert_eq!(
            decide(
                &policy(FailoverPolicyMode::AllowListed),
                &report,
                0,
                true,
                &[]
            ),
            FailoverDecision::Blocked(LeadFailoverBlocker::ObservationStale)
        );
        report = observation();
        report.affected_agent_binding_id = "old-lead".into();
        assert_eq!(
            decide(
                &policy(FailoverPolicyMode::AllowListed),
                &report,
                0,
                true,
                &[]
            ),
            FailoverDecision::Blocked(LeadFailoverBlocker::ObservationDoesNotMatchCurrentLead)
        );
    }

    #[test]
    fn exhausted_change_budget_unsafe_boundary_and_stale_candidate_block() {
        let policy = policy(FailoverPolicyMode::AllowListed);
        let qualified = [candidate(
            "fallback-a",
            FailoverCandidateReadiness::Qualified,
        )];
        assert_eq!(
            decide(&policy, &observation(), 2, true, &qualified),
            FailoverDecision::Blocked(LeadFailoverBlocker::ChangeLimitReached)
        );
        assert_eq!(
            decide(&policy, &observation(), 0, false, &qualified),
            FailoverDecision::Blocked(LeadFailoverBlocker::UnsafeBoundary)
        );
        let mut stale = candidate("fallback-a", FailoverCandidateReadiness::Qualified);
        stale.checked_task_version = 6;
        assert_eq!(
            decide(&policy, &observation(), 0, true, &[stale]),
            FailoverDecision::Blocked(LeadFailoverBlocker::NoEligibleFallback)
        );
    }

    #[test]
    fn a_failed_first_fallback_never_promotes_a_later_one() {
        // The durable handoff transaction independently derives the next fallback as the
        // first same-Workspace enabled and lead-eligible binding in policy order. The
        // domain must not recommend a later binding that the transaction cannot
        // qualify, so a failed first candidate blocks the whole change.
        let policy = policy(FailoverPolicyMode::AllowListed);
        assert_eq!(
            decide(
                &policy,
                &observation(),
                0,
                true,
                &[
                    candidate("fallback-a", FailoverCandidateReadiness::Ineligible),
                    candidate("fallback-b", FailoverCandidateReadiness::Qualified),
                ],
            ),
            FailoverDecision::Blocked(LeadFailoverBlocker::NoEligibleFallback)
        );
    }

    #[test]
    fn the_current_lead_is_never_offered_as_its_own_fallback() {
        let mut with_lead = policy(FailoverPolicyMode::Ask);
        with_lead.fallback_agent_binding_ids = vec!["lead".into(), "fallback-a".into()];
        assert_eq!(
            decide(
                &with_lead,
                &observation(),
                0,
                true,
                &[
                    candidate("lead", FailoverCandidateReadiness::Qualified),
                    candidate("fallback-a", FailoverCandidateReadiness::Qualified),
                ],
            ),
            FailoverDecision::AskOwner {
                trigger: FailoverTrigger::QuotaExhausted,
                observation_id: "quota-observation-1".into(),
                fallback_agent_binding_ids: vec!["fallback-a".into()],
            }
        );
    }

    #[test]
    fn invalid_policy_and_duplicate_fallbacks_fail_closed() {
        let mut malformed = policy(FailoverPolicyMode::AllowListed);
        malformed.fallback_agent_binding_ids = vec!["same".into(), "same".into()];
        assert_eq!(
            decide(&malformed, &observation(), 0, true, &[]),
            FailoverDecision::Blocked(LeadFailoverBlocker::InvalidPolicy)
        );

        malformed = policy(FailoverPolicyMode::AllowListed);
        malformed.fallback_agent_binding_ids = vec![
            "fallback-a".into(),
            "fallback-b".into(),
            "fallback-c".into(),
            "fallback-d".into(),
        ];
        malformed.max_lead_changes = 3;
        assert_eq!(
            decide(&malformed, &observation(), 0, true, &[]),
            FailoverDecision::Blocked(LeadFailoverBlocker::InvalidPolicy)
        );

        malformed = policy(FailoverPolicyMode::AllowListed);
        malformed.max_lead_changes = 3;
        assert_eq!(
            decide(&malformed, &observation(), 0, true, &[]),
            FailoverDecision::Blocked(LeadFailoverBlocker::InvalidPolicy)
        );
    }

    #[test]
    fn runtime_failover_requires_incarnation_and_candidate_observations_are_unambiguous() {
        let mut report = observation();
        report.trigger = FailoverTrigger::RuntimeUnavailable;
        report.runtime_incarnation_id = None;
        let mut configured = policy(FailoverPolicyMode::AllowListed);
        configured.triggers = vec![FailoverTrigger::RuntimeUnavailable];
        assert_eq!(
            decide(&configured, &report, 0, true, &[]),
            FailoverDecision::Blocked(LeadFailoverBlocker::InvalidContext)
        );
        report.runtime_incarnation_id = Some("old-incarnation".into());
        assert_eq!(
            decide(&configured, &report, 0, true, &[]),
            FailoverDecision::Blocked(LeadFailoverBlocker::InvalidContext)
        );

        let duplicate = candidate("fallback-a", FailoverCandidateReadiness::Qualified);
        assert_eq!(
            decide(
                &policy(FailoverPolicyMode::AllowListed),
                &observation(),
                0,
                true,
                &[duplicate.clone(), duplicate]
            ),
            FailoverDecision::Blocked(LeadFailoverBlocker::InvalidContext)
        );
    }
}
