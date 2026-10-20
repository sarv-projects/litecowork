//! Conservative classification for reusing one LiteSPM-managed capability through
//! different native agents. This is a policy projection only; it does not install,
//! activate, grant, or dispatch a capability.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AgentCapabilityRoute {
    DirectNativeAttachment,
    LitecoworkGateway,
    HostContextOnly,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CapabilityBridgeDecision {
    QualifiedDirectRelay,
    GatewayRequiredForCoreMediation,
    GuidanceOnlyContext,
    PackageIdentityUnqualified,
    AgentRouteUnsupported,
}

/// Evidence already observed by a trusted AgentModule and LiteSPM adapter. Fields
/// are explicit so a native package with a matching display name cannot masquerade
/// as the LiteSPM capability, and unknown mediation guarantees stay false.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentCapabilityBridgeInput {
    pub capability_identity_digest: String,
    pub lite_spm_managed: bool,
    pub package_digest_verified: bool,
    pub is_guidance_only: bool,
    pub agent_supports_native_attachment: bool,
    pub agent_supports_host_context: bool,
    pub gateway_available: bool,
    pub direct_path_authenticates_each_call: bool,
    pub direct_path_records_invocation_before_dispatch: bool,
    pub direct_path_enforces_grant_scope: bool,
    pub direct_path_enforces_fencing: bool,
    pub direct_path_records_effect_before_mutation: bool,
    pub direct_path_supports_idempotency_or_reconciliation: bool,
}

/// Classify the only route LiteCowork may project for one exact AgentModule and
/// capability identity. The returned route grants no authority itself.
pub fn classify_agent_capability_route(
    input: &AgentCapabilityBridgeInput,
) -> (AgentCapabilityRoute, CapabilityBridgeDecision) {
    if !input.lite_spm_managed
        || !input.package_digest_verified
        || !valid_digest(&input.capability_identity_digest)
    {
        return (
            AgentCapabilityRoute::Unsupported,
            CapabilityBridgeDecision::PackageIdentityUnqualified,
        );
    }

    if input.is_guidance_only {
        return if input.agent_supports_host_context {
            (
                AgentCapabilityRoute::HostContextOnly,
                CapabilityBridgeDecision::GuidanceOnlyContext,
            )
        } else {
            (
                AgentCapabilityRoute::Unsupported,
                CapabilityBridgeDecision::AgentRouteUnsupported,
            )
        };
    }

    let direct_path_is_core_enforced = input.agent_supports_native_attachment
        && input.direct_path_authenticates_each_call
        && input.direct_path_records_invocation_before_dispatch
        && input.direct_path_enforces_grant_scope
        && input.direct_path_enforces_fencing
        && input.direct_path_records_effect_before_mutation
        && input.direct_path_supports_idempotency_or_reconciliation;
    if direct_path_is_core_enforced {
        return (
            AgentCapabilityRoute::DirectNativeAttachment,
            CapabilityBridgeDecision::QualifiedDirectRelay,
        );
    }

    if input.gateway_available {
        return (
            AgentCapabilityRoute::LitecoworkGateway,
            CapabilityBridgeDecision::GatewayRequiredForCoreMediation,
        );
    }

    (
        AgentCapabilityRoute::Unsupported,
        CapabilityBridgeDecision::AgentRouteUnsupported,
    )
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn qualified() -> AgentCapabilityBridgeInput {
        AgentCapabilityBridgeInput {
            capability_identity_digest: format!("sha256:{}", "a".repeat(64)),
            lite_spm_managed: true,
            package_digest_verified: true,
            is_guidance_only: false,
            agent_supports_native_attachment: true,
            agent_supports_host_context: true,
            gateway_available: true,
            direct_path_authenticates_each_call: true,
            direct_path_records_invocation_before_dispatch: true,
            direct_path_enforces_grant_scope: true,
            direct_path_enforces_fencing: true,
            direct_path_records_effect_before_mutation: true,
            direct_path_supports_idempotency_or_reconciliation: true,
        }
    }

    #[test]
    fn direct_native_attachment_requires_every_mediation_guarantee() {
        let input = qualified();
        assert_eq!(
            classify_agent_capability_route(&input),
            (
                AgentCapabilityRoute::DirectNativeAttachment,
                CapabilityBridgeDecision::QualifiedDirectRelay
            )
        );

        let weaken: [fn(&mut AgentCapabilityBridgeInput); 6] = [
            |i: &mut AgentCapabilityBridgeInput| i.direct_path_authenticates_each_call = false,
            |i: &mut AgentCapabilityBridgeInput| {
                i.direct_path_records_invocation_before_dispatch = false
            },
            |i: &mut AgentCapabilityBridgeInput| i.direct_path_enforces_grant_scope = false,
            |i: &mut AgentCapabilityBridgeInput| i.direct_path_enforces_fencing = false,
            |i: &mut AgentCapabilityBridgeInput| {
                i.direct_path_records_effect_before_mutation = false
            },
            |i: &mut AgentCapabilityBridgeInput| {
                i.direct_path_supports_idempotency_or_reconciliation = false
            },
        ];
        for weaken in weaken {
            let mut input = qualified();
            weaken(&mut input);
            assert_eq!(
                classify_agent_capability_route(&input),
                (
                    AgentCapabilityRoute::LitecoworkGateway,
                    CapabilityBridgeDecision::GatewayRequiredForCoreMediation
                )
            );
        }
    }

    #[test]
    fn unverified_or_native_only_same_named_package_is_not_reused_as_litespm() {
        let mut input = qualified();
        input.lite_spm_managed = false;
        assert_eq!(
            classify_agent_capability_route(&input),
            (
                AgentCapabilityRoute::Unsupported,
                CapabilityBridgeDecision::PackageIdentityUnqualified
            )
        );
        let mut input = qualified();
        input.package_digest_verified = false;
        assert_eq!(
            classify_agent_capability_route(&input).0,
            AgentCapabilityRoute::Unsupported
        );
    }

    #[test]
    fn guidance_only_can_use_context_but_never_gateway_tools() {
        let mut input = qualified();
        input.is_guidance_only = true;
        assert_eq!(
            classify_agent_capability_route(&input),
            (
                AgentCapabilityRoute::HostContextOnly,
                CapabilityBridgeDecision::GuidanceOnlyContext
            )
        );
        input.agent_supports_host_context = false;
        assert_eq!(
            classify_agent_capability_route(&input).0,
            AgentCapabilityRoute::Unsupported
        );
    }

    #[test]
    fn missing_gateway_and_incomplete_direct_path_is_explicitly_unsupported() {
        let mut input = qualified();
        input.gateway_available = false;
        input.direct_path_enforces_fencing = false;
        assert_eq!(
            classify_agent_capability_route(&input),
            (
                AgentCapabilityRoute::Unsupported,
                CapabilityBridgeDecision::AgentRouteUnsupported
            )
        );
    }

    #[test]
    fn malformed_identity_digest_fails_closed() {
        let mut input = qualified();
        input.capability_identity_digest = format!("sha256:{}", "A".repeat(64));
        assert_eq!(
            classify_agent_capability_route(&input).0,
            AgentCapabilityRoute::Unsupported
        );
    }

    #[test]
    fn route_values_match_the_stable_shared_contract() {
        for (route, expected) in [
            (
                AgentCapabilityRoute::DirectNativeAttachment,
                "DIRECT_NATIVE_ATTACHMENT",
            ),
            (
                AgentCapabilityRoute::LitecoworkGateway,
                "LITECOWORK_GATEWAY",
            ),
            (AgentCapabilityRoute::HostContextOnly, "HOST_CONTEXT_ONLY"),
            (AgentCapabilityRoute::Unsupported, "UNSUPPORTED"),
        ] {
            assert_eq!(
                serde_json::to_string(&route).unwrap(),
                format!("\"{expected}\"")
            );
        }
    }
}
