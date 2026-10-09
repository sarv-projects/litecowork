use super::*;

#[test]
fn canonical_occurrence_vectors() {
    assert_eq!(
        OccurrenceIdentity::Schedule {
            trigger_id: "trigger-weekly".into(),
            scheduled_epoch_ms: 1791000000000
        }
        .key()
        .unwrap(),
        "3761b46716c5a9a3c4088c0dcb18cfc42dd647fea86ccf2d3149140c1d502460"
    );
    assert_eq!(
        OccurrenceIdentity::Manual {
            trigger_id: "trigger-manual".into(),
            principal_id: "principal-1".into(),
            request_id: "request-1".into(),
            automation_id: "automation-1".into()
        }
        .key()
        .unwrap(),
        "1a29dba53c8865650745074940a438f97e44eca49c011dcfb6883cd212d7f294"
    );
    assert_eq!(
        OccurrenceIdentity::ConnectorEvent {
            trigger_id: "trigger-github".into(),
            connection_id: "connection-1".into(),
            provider_event_id: "delivery-1".into()
        }
        .key()
        .unwrap(),
        "b0b42fa53ec4eb84d1d5487f6b72f3156550746414df4c239e562d47e8e63b45"
    );
}

#[test]
fn occurrence_encoding_uses_utf8_lengths_and_field_boundaries() {
    assert_eq!(
        &encode_occurrence_fields(&["é"]).unwrap()[b"LiteCowork/AutomationOccurrence/v2\0".len()..],
        &[0, 0, 0, 2, 0xc3, 0xa9]
    );
    assert_ne!(
        encode_occurrence_fields(&["ab", "c"]).unwrap(),
        encode_occurrence_fields(&["a", "bc"]).unwrap()
    );
}

#[test]
fn paused_identity_accepts_owner_work_and_blocks_proactive_work() {
    assert_eq!(
        check_origin_status(CoworkerStatus::Paused, WorkOrigin::Owner),
        Ok(())
    );
    assert_eq!(
        check_origin_status(CoworkerStatus::Paused, WorkOrigin::Proactive),
        Err(DomainError::CoworkerInactive)
    );
    assert_eq!(
        check_origin_status(CoworkerStatus::Archived, WorkOrigin::Owner),
        Err(DomainError::CoworkerArchived)
    );
}

#[test]
fn archive_requires_primary_clear_and_settled_responsibilities() {
    let clear = CoworkerLifecycleFacts {
        is_primary: false,
        has_active_automation: false,
        has_nonterminal_tasks: false,
        resume_reconciled: false,
    };
    assert_eq!(
        check_coworker_transition(CoworkerStatus::Active, CoworkerStatus::Archived, &clear),
        Ok(())
    );
    for facts in [
        CoworkerLifecycleFacts {
            is_primary: true,
            ..clear
        },
        CoworkerLifecycleFacts {
            has_active_automation: true,
            ..clear
        },
        CoworkerLifecycleFacts {
            has_nonterminal_tasks: true,
            ..clear
        },
    ] {
        assert_eq!(
            check_coworker_transition(CoworkerStatus::Paused, CoworkerStatus::Archived, &facts),
            Err(DomainError::ArchiveBlocked)
        );
    }
    assert_eq!(
        check_coworker_transition(CoworkerStatus::Archived, CoworkerStatus::Active, &clear),
        Err(DomainError::CoworkerArchived)
    );
}

#[test]
fn resume_requires_reconciliation_and_disabled_is_terminal() {
    let facts = AutomationLifecycleFacts {
        routine_active: true,
        coworker_active: true,
        trigger_dependencies_reconciled: false,
    };
    assert_eq!(
        check_automation_transition(AutomationStatus::Paused, AutomationStatus::Enabled, &facts),
        Err(DomainError::ReconciliationRequired)
    );
    assert_eq!(
        check_automation_transition(AutomationStatus::Enabled, AutomationStatus::Paused, &facts),
        Ok(())
    );
    let ready = AutomationLifecycleFacts {
        trigger_dependencies_reconciled: true,
        ..facts
    };
    assert_eq!(
        check_automation_transition(AutomationStatus::Paused, AutomationStatus::Enabled, &ready),
        Ok(())
    );
    assert_eq!(
        check_automation_transition(
            AutomationStatus::Paused,
            AutomationStatus::Enabled,
            &AutomationLifecycleFacts {
                routine_active: false,
                ..ready
            }
        ),
        Err(DomainError::RoutineArchived)
    );
    assert_eq!(
        check_automation_transition(
            AutomationStatus::Disabled,
            AutomationStatus::Enabled,
            &ready
        ),
        Err(DomainError::AutomationDisabled)
    );
}

#[test]
fn version_check_rejects_stale_command_and_overflow() {
    assert_eq!(next_version(4, 3), Err(DomainError::VersionConflict));
    assert_eq!(next_version(4, 4), Ok(5));
    assert_eq!(
        next_version(u64::MAX, u64::MAX),
        Err(DomainError::VersionOverflow)
    );
}

#[test]
fn manual_run_and_test_run_have_separate_idempotency_domains() {
    let manual = OccurrenceIdentity::Manual {
        trigger_id: "manual".into(),
        principal_id: "p".into(),
        request_id: "r".into(),
        automation_id: "a".into(),
    }
    .key()
    .unwrap();
    let test = TestRunIdentity {
        automation_id: "a".into(),
        automation_revision: 3,
        principal_id: "p".into(),
        request_id: "r".into(),
    };
    assert_ne!(manual, test.key().unwrap());
    assert_eq!(test.key().unwrap(), test.clone().key().unwrap());
    assert_ne!(
        test.key().unwrap(),
        TestRunIdentity {
            automation_revision: 4,
            ..test
        }
        .key()
        .unwrap()
    );
}

#[test]
fn trigger_placement_and_unique_ids_are_enforced() {
    let manual = TriggerSpec {
        trigger_id: "manual".into(),
        placement: TriggerPlacement::Auto,
        runtime_id: None,
        trigger: TriggerDefinition::Manual,
    };
    assert_eq!(validate_triggers(&[manual.clone()]), Ok(()));
    assert_eq!(validate_triggers(&[]), Err(DomainError::InvalidDefinition));
    assert_eq!(
        validate_triggers(&[manual.clone(), manual.clone()]),
        Err(DomainError::InvalidDefinition)
    );
    assert_eq!(
        validate_triggers(&[TriggerSpec {
            placement: TriggerPlacement::SpecificRuntime,
            ..manual.clone()
        }]),
        Err(DomainError::InvalidDefinition)
    );
    assert_eq!(
        validate_triggers(&[TriggerSpec {
            runtime_id: Some("r".into()),
            ..manual
        }]),
        Err(DomainError::InvalidDefinition)
    );
}

#[test]
fn source_replacement_requires_new_trigger_id_but_instruction_edit_does_not() {
    let old = TriggerSpec {
        trigger_id: "connector".into(),
        placement: TriggerPlacement::Auto,
        runtime_id: None,
        trigger: TriggerDefinition::ConnectorEvent {
            connection_id: "one".into(),
            provider_event_type: "change".into(),
            filter_expression: None,
        },
    };
    assert_eq!(
        check_trigger_identity(&[old.clone()], &[old.clone()]),
        Ok(())
    );
    let changed = TriggerSpec {
        trigger: TriggerDefinition::ConnectorEvent {
            connection_id: "two".into(),
            provider_event_type: "change".into(),
            filter_expression: None,
        },
        ..old
    };
    assert_eq!(
        check_trigger_identity(
            &[changed.clone()],
            &[TriggerSpec {
                trigger: TriggerDefinition::Manual,
                ..changed.clone()
            }]
        ),
        Err(DomainError::TriggerSourceChanged)
    );
    assert_eq!(
        check_trigger_identity(
            &[changed.clone()],
            &[TriggerSpec {
                trigger_id: "new".into(),
                trigger: TriggerDefinition::Manual,
                ..changed
            }]
        ),
        Ok(())
    );
}

#[test]
fn trigger_codec_matches_current_canonical_contract() {
    assert_eq!(
        serde_json::to_value(RecurrenceFormat::Cron5).unwrap(),
        serde_json::json!("CRON_5")
    );
    assert_eq!(
        serde_json::to_value(RecurrenceFormat::Rfc5545Rrule).unwrap(),
        serde_json::json!("RFC5545_RRULE")
    );
    assert_eq!(
        serde_json::to_value(MisfirePolicy::Skip).unwrap(),
        serde_json::json!({"kind": "SKIP"})
    );
    assert_eq!(
        serde_json::to_value(MisfirePolicy::CatchUpBounded { max_occurrences: 2 }).unwrap(),
        serde_json::json!({"kind": "CATCH_UP_BOUNDED", "max_occurrences": 2})
    );
    assert!(
        serde_json::from_value::<MisfirePolicy>(
            serde_json::json!({"kind": "SKIP", "max_occurrences": 2})
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<TriggerDefinition>(
            serde_json::json!({"kind": "MANUAL", "command": "untrusted"})
        )
        .is_err()
    );
}

fn neutral_coworker() -> CoworkerDefinition {
    CoworkerDefinition {
        name: "Coworker".into(),
        avatar_ref: None,
        role_description: String::new(),
        default_lead_agent_binding_id: None,
        delegation_strategy: DelegationStrategy::NativeDefault,
        enabled_delegation_profile_ids: vec![],
        delegation_budget_policy: None,
        lead_failover_policy: None,
        interaction_policy: CoworkerInteractionPolicy {
            read_only_work: InteractionDefault::StandardTrustPolicy,
            draft_creation: InteractionDefault::StandardTrustPolicy,
            external_mutation: InteractionDefault::RequireOwnerApproval,
            destructive_action: InteractionDefault::HandoffToOwner,
            financial_commitment: InteractionDefault::HandoffToOwner,
        },
        context_policy: CoworkerContextPolicy {
            allowed_context_kinds: vec![],
            max_retrieved_items: 0,
            retain_task_summaries: false,
            require_user_confirmation_for_memory: true,
        },
        notification_policy: NotificationPolicy {
            blockers: BinaryNotification::Always,
            completion: CompletionNotification::OnSuccess,
            failures: BinaryNotification::Always,
        },
    }
}

#[test]
fn neutral_coworker_needs_no_personalization_and_memory_confirmation_cannot_weaken() {
    let neutral = neutral_coworker();
    assert_eq!(validate_coworker_definition(&neutral), Ok(()));
    let mut unsafe_memory = neutral.clone();
    unsafe_memory
        .context_policy
        .require_user_confirmation_for_memory = false;
    assert_eq!(
        validate_coworker_definition(&unsafe_memory),
        Err(DomainError::InvalidDefinition)
    );
    let mut duplicate = neutral;
    duplicate.enabled_delegation_profile_ids = vec!["worker".into(), "worker".into()];
    assert_eq!(
        validate_coworker_definition(&duplicate),
        Err(DomainError::InvalidDefinition)
    );
}

#[test]
fn coworker_revision_codec_is_flat_and_roundtrips_pinned_provenance() {
    let revision = CoworkerRevision {
        coworker_id: "coworker".into(),
        revision: 2,
        definition: neutral_coworker(),
        authored_by: PrincipalRef {
            principal_id: "owner".into(),
            kind: PrincipalKind::User,
        },
        created_at: "2026-10-08T00:00:00Z".into(),
    };
    let value = serde_json::to_value(&revision).unwrap();
    assert_eq!(value["name"], "Coworker");
    assert!(value.get("definition").is_none());
    assert_eq!(value["revision"], 2);
    assert_eq!(
        serde_json::from_value::<CoworkerRevision>(value).unwrap(),
        revision
    );
}

#[test]
fn revision_numbers_and_status_versions_are_independent() {
    // Status commands call next_version, while current_revision remains pinned.
    let mut head = Coworker {
        coworker_id: "c".into(),
        workspace_id: "w".into(),
        current_revision: 3,
        status: CoworkerStatus::Active,
        created_at: "t".into(),
        updated_at: "t".into(),
        version: 8,
    };
    head.version = next_version(head.version, 8).unwrap();
    head.status = CoworkerStatus::Paused;
    assert_eq!(head.current_revision, 3);
    assert_eq!(head.version, 9);
}
