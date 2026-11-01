use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Schemas owned by other domains remain explicit object values. The transaction
/// port must validate them against the current owning schema before admission.
pub type ContractObject = BTreeMap<String, serde_json::Value>;

macro_rules! string_enum {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
        pub enum $name { $($variant),+ }
    };
}
string_enum!(CoworkerStatus {
    Active,
    Paused,
    Archived
});
string_enum!(AutomationStatus {
    Enabled,
    Paused,
    Disabled
});
string_enum!(PrincipalKind {
    User,
    Service,
    Runtime,
    Agent,
    ChannelIdentity
});
string_enum!(DelegationStrategy {
    NativeDefault,
    Balanced,
    CostSaver,
    HostDelegationOnly
});
string_enum!(InteractionDefault {
    StandardTrustPolicy,
    RequireOwnerApproval,
    HandoffToOwner
});
string_enum!(ContextDocumentKind {
    PersonalProfile,
    CoworkerNotes,
    WorkspaceNotes,
    GoalNotes
});
string_enum!(BinaryNotification { Always, Silent });
string_enum!(CompletionNotification {
    Always,
    OnSuccess,
    Silent
});
string_enum!(AutomationNotification {
    Always,
    OnSuccess,
    OnFailure,
    OnCondition,
    Silent
});
string_enum!(TriggerPlacement {
    Hub,
    SpecificRuntime,
    Auto
});
string_enum!(OverlapPolicy {
    Skip,
    Queue,
    CancelOld,
    Allow
});
string_enum!(WakePolicy {
    Never,
    TryWake,
    RequireRuntimeAwake
});
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecurrenceFormat {
    #[serde(rename = "CRON_5")]
    Cron5,
    #[serde(rename = "RFC5545_RRULE")]
    Rfc5545Rrule,
}
string_enum!(AmbiguousLocalTime { Earlier, Later });
string_enum!(NonexistentLocalTime { Skip, NextValid });
string_enum!(WebhookAuthentication {
    Signature,
    BearerSecret,
    MutualTls
});

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrincipalRef {
    pub principal_id: String,
    pub kind: PrincipalKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoworkerRevisionRef {
    pub coworker_id: String,
    pub revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoworkerInteractionPolicy {
    pub read_only_work: InteractionDefault,
    pub draft_creation: InteractionDefault,
    pub external_mutation: InteractionDefault,
    pub destructive_action: InteractionDefault,
    pub financial_commitment: InteractionDefault,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoworkerContextPolicy {
    pub allowed_context_kinds: Vec<ContextDocumentKind>,
    pub max_retrieved_items: u32,
    pub retain_task_summaries: bool,
    pub require_user_confirmation_for_memory: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationPolicy {
    pub blockers: BinaryNotification,
    pub completion: CompletionNotification,
    pub failures: BinaryNotification,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CoworkerDefinition {
    pub name: String,
    pub avatar_ref: Option<ContractObject>,
    pub role_description: String,
    pub default_lead_agent_binding_id: Option<String>,
    pub delegation_strategy: DelegationStrategy,
    pub enabled_delegation_profile_ids: Vec<String>,
    pub delegation_budget_policy: Option<ContractObject>,
    pub lead_failover_policy: Option<ContractObject>,
    pub interaction_policy: CoworkerInteractionPolicy,
    pub context_policy: CoworkerContextPolicy,
    pub notification_policy: NotificationPolicy,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CoworkerRevision {
    pub coworker_id: String,
    pub revision: u64,
    #[serde(flatten)]
    pub definition: CoworkerDefinition,
    pub authored_by: PrincipalRef,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Coworker {
    pub coworker_id: String,
    pub workspace_id: String,
    pub current_revision: u64,
    pub status: CoworkerStatus,
    pub created_at: String,
    pub updated_at: String,
    pub version: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum MisfirePolicy {
    Skip,
    RunOnceWhenAvailable,
    CatchUpBounded { max_occurrences: u32 },
}

// Serde's internally tagged representation accepts unknown fields for unit
// variants. These closed policy objects are persisted and validated against the
// machine schema, so enforce the exact wire shape explicitly on deserialization.
impl<'de> Deserialize<'de> for MisfirePolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error as _;
        let value = serde_json::Value::deserialize(deserializer)?;
        let object = value
            .as_object()
            .ok_or_else(|| D::Error::custom("misfire policy must be an object"))?;
        let kind = object
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| D::Error::custom("misfire policy kind is required"))?;
        match kind {
            "SKIP" if object.len() == 1 => Ok(Self::Skip),
            "RUN_ONCE_WHEN_AVAILABLE" if object.len() == 1 => Ok(Self::RunOnceWhenAvailable),
            "CATCH_UP_BOUNDED" if object.len() == 2 => {
                let max_occurrences = object
                    .get("max_occurrences")
                    .and_then(serde_json::Value::as_u64)
                    .and_then(|value| u32::try_from(value).ok())
                    .ok_or_else(|| D::Error::custom("max_occurrences must be a u32"))?;
                Ok(Self::CatchUpBounded { max_occurrences })
            }
            _ => Err(D::Error::custom("misfire policy shape is invalid")),
        }
    }
}

/// The five V1 trigger definition variants. Remaining schema extension points
/// require qualified providers and are rejected as unsupported at the API adapter.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum TriggerDefinition {
    Schedule {
        recurrence_format: RecurrenceFormat,
        timezone: String,
        rrule_or_cron: String,
        recurrence_semantics_version: u32,
        start_at: Option<String>,
        end_at: Option<String>,
        misfire_policy: MisfirePolicy,
        ambiguous_local_time: AmbiguousLocalTime,
        nonexistent_local_time: NonexistentLocalTime,
    },
    OneShot {
        scheduled_at: String,
        misfire_policy: MisfirePolicy,
    },
    Webhook {
        source_identity: String,
        auth_profile_ref: String,
        authentication_mode: WebhookAuthentication,
        replay_window_ms: u64,
        max_payload_bytes: u64,
        delivery_id_field: Option<String>,
    },
    ConnectorEvent {
        connection_id: String,
        provider_event_type: String,
        filter_expression: Option<String>,
    },
    Manual,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
enum TriggerDefinitionWire {
    Schedule {
        recurrence_format: RecurrenceFormat,
        timezone: String,
        rrule_or_cron: String,
        recurrence_semantics_version: u32,
        start_at: Option<String>,
        end_at: Option<String>,
        misfire_policy: MisfirePolicy,
        ambiguous_local_time: AmbiguousLocalTime,
        nonexistent_local_time: NonexistentLocalTime,
    },
    OneShot {
        scheduled_at: String,
        misfire_policy: MisfirePolicy,
    },
    Webhook {
        source_identity: String,
        auth_profile_ref: String,
        authentication_mode: WebhookAuthentication,
        replay_window_ms: u64,
        max_payload_bytes: u64,
        delivery_id_field: Option<String>,
    },
    ConnectorEvent {
        connection_id: String,
        provider_event_type: String,
        filter_expression: Option<String>,
    },
    Manual,
}

impl<'de> Deserialize<'de> for TriggerDefinition {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error as _;
        let value = serde_json::Value::deserialize(deserializer)?;
        let object = value
            .as_object()
            .ok_or_else(|| D::Error::custom("trigger definition must be an object"))?;
        if object.get("kind").and_then(serde_json::Value::as_str) == Some("MANUAL") {
            return if object.len() == 1 {
                Ok(Self::Manual)
            } else {
                Err(D::Error::custom(
                    "manual trigger must not contain extra fields",
                ))
            };
        }
        let wire: TriggerDefinitionWire =
            serde_json::from_value(value).map_err(D::Error::custom)?;
        Ok(match wire {
            TriggerDefinitionWire::Schedule {
                recurrence_format,
                timezone,
                rrule_or_cron,
                recurrence_semantics_version,
                start_at,
                end_at,
                misfire_policy,
                ambiguous_local_time,
                nonexistent_local_time,
            } => Self::Schedule {
                recurrence_format,
                timezone,
                rrule_or_cron,
                recurrence_semantics_version,
                start_at,
                end_at,
                misfire_policy,
                ambiguous_local_time,
                nonexistent_local_time,
            },
            TriggerDefinitionWire::OneShot {
                scheduled_at,
                misfire_policy,
            } => Self::OneShot {
                scheduled_at,
                misfire_policy,
            },
            TriggerDefinitionWire::Webhook {
                source_identity,
                auth_profile_ref,
                authentication_mode,
                replay_window_ms,
                max_payload_bytes,
                delivery_id_field,
            } => Self::Webhook {
                source_identity,
                auth_profile_ref,
                authentication_mode,
                replay_window_ms,
                max_payload_bytes,
                delivery_id_field,
            },
            TriggerDefinitionWire::ConnectorEvent {
                connection_id,
                provider_event_type,
                filter_expression,
            } => Self::ConnectorEvent {
                connection_id,
                provider_event_type,
                filter_expression,
            },
            TriggerDefinitionWire::Manual => unreachable!("Manual is handled before wire decoding"),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TriggerSpec {
    pub trigger_id: String,
    pub placement: TriggerPlacement,
    pub runtime_id: Option<String>,
    pub trigger: TriggerDefinition,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutomationExecutionPolicy {
    pub placement_preference: PlacementPreference,
    pub max_concurrent_occurrences: u32,
    pub overlap_policy: OverlapPolicy,
    pub retry_policy: RetryPolicy,
    pub budget_ceiling: Option<ContractObject>,
    pub notification_policy: AutomationNotification,
    pub wake_policy: WakePolicy,
}
string_enum!(PlacementClass {
    Auto,
    LocalOnly,
    CloudPreferred,
    CloudOnly
});
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PlacementPreference {
    Class(PlacementClass),
    SpecificRuntime { runtime_id: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
    pub multiplier: f64,
    pub jitter: bool,
    pub retryable_error_codes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AutomationDefinition {
    pub routine_id: String,
    pub routine_revision: u64,
    pub triggers: Vec<TriggerSpec>,
    pub execution_policy: AutomationExecutionPolicy,
    pub coworker_ref: Option<CoworkerRevisionRef>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AutomationRevision {
    pub automation_id: String,
    pub revision: u64,
    #[serde(flatten)]
    pub definition: AutomationDefinition,
    pub authored_by: PrincipalRef,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Automation {
    pub automation_id: String,
    pub workspace_id: String,
    pub name: String,
    pub current_revision: u64,
    pub status: AutomationStatus,
    pub created_at: String,
    pub updated_at: String,
    pub version: u64,
}
