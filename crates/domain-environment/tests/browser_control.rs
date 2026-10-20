use domain_environment::{
    BrowserControlActor, BrowserControlError, BrowserControlLease, BrowserFrameGeometry,
    BrowserObservation, BrowserPointerError, BrowserViewport, PaneBounds, PointerClientPosition,
};

fn lease() -> BrowserControlLease {
    BrowserControlLease::new(
        "env-1",
        "task-1",
        "attempt-1",
        "runtime-1",
        "incarnation-1",
        BrowserControlActor::Agent {
            actor_id: "agent-1".into(),
        },
        7,
        100,
        1_000,
    )
    .unwrap()
}

fn pointer_geometry(control_epoch: u64, captured_at_ms: u64) -> BrowserFrameGeometry {
    BrowserFrameGeometry::new(
        "session-1",
        "tab-1",
        1,
        control_epoch,
        1,
        1,
        captured_at_ms,
        0.0,
        0.0,
        BrowserViewport::new(800, 600, 1.0).unwrap(),
        PaneBounds::new(0.0, 0.0, 800.0, 600.0).unwrap(),
    )
    .unwrap()
}

#[test]
fn pointer_mapping_requires_the_current_lease_owner_epoch_and_fresh_frame() {
    let lease = lease();
    let agent = BrowserControlActor::Agent {
        actor_id: "agent-1".into(),
    };
    let point = lease
        .map_pointer_for_current_owner(
            &agent,
            7,
            210,
            &pointer_geometry(7, 200),
            PointerClientPosition { x: 400.0, y: 300.0 },
            "session-1",
            "tab-1",
            1,
            1,
            1,
            100,
        )
        .unwrap();
    assert_eq!(point.x_css, 400.0);
    assert_eq!(point.y_css, 300.0);
    assert_eq!(
        lease.map_pointer_for_current_owner(
            &agent,
            7,
            211,
            &pointer_geometry(7, 200),
            PointerClientPosition { x: 400.0, y: 300.0 },
            "session-1",
            "tab-1",
            1,
            1,
            1,
            10,
        ),
        Err(BrowserControlError::Pointer(
            BrowserPointerError::ExpiredFrame
        ))
    );

    let user = lease
        .take_control(
            BrowserControlActor::User {
                actor_id: "principal-1".into(),
            },
            7,
            210,
            0,
        )
        .unwrap();
    assert_eq!(
        user.map_pointer_for_current_owner(
            &agent,
            7,
            211,
            &pointer_geometry(7, 200),
            PointerClientPosition { x: 400.0, y: 300.0 },
            "session-1",
            "tab-1",
            1,
            1,
            1,
            100,
        ),
        Err(BrowserControlError::StaleEpoch {
            expected: 8,
            actual: 7,
        })
    );
    assert_eq!(
        user.map_pointer_for_current_owner(
            &BrowserControlActor::User {
                actor_id: "principal-1".into(),
            },
            8,
            211,
            &pointer_geometry(7, 200),
            PointerClientPosition { x: 400.0, y: 300.0 },
            "session-1",
            "tab-1",
            1,
            1,
            1,
            100,
        ),
        Err(BrowserControlError::Pointer(
            BrowserPointerError::StaleControlEpoch
        ))
    );
}

#[test]
fn rejects_an_agent_action_queued_before_control_transfers_to_user() {
    let lease = lease()
        .take_control(
            BrowserControlActor::User {
                actor_id: "principal-1".into(),
            },
            7,
            200,
            0,
        )
        .unwrap();

    assert_eq!(
        lease.authorize_action(
            &BrowserControlActor::Agent {
                actor_id: "agent-1".into()
            },
            7,
            201
        ),
        Err(BrowserControlError::StaleEpoch {
            expected: 8,
            actual: 7
        })
    );
}

#[test]
fn takeover_requires_the_old_owner_to_be_quiescent() {
    assert_eq!(
        lease().take_control(
            BrowserControlActor::User {
                actor_id: "principal-1".into()
            },
            7,
            200,
            1
        ),
        Err(BrowserControlError::ActionsInFlight)
    );
}

#[test]
fn takeover_rejects_a_clock_value_before_the_current_owner_epoch_started() {
    assert_eq!(
        lease().take_control(
            BrowserControlActor::User {
                actor_id: "principal-1".into()
            },
            7,
            99,
            0
        ),
        Err(BrowserControlError::TimeBeforeOwnerSince)
    );
}

#[test]
fn takeover_cannot_be_requested_by_an_agent_actor() {
    assert_eq!(
        lease().take_control(
            BrowserControlActor::Agent {
                actor_id: "agent-2".into()
            },
            7,
            200,
            0
        ),
        Err(BrowserControlError::WrongOwner)
    );
}

#[test]
fn agent_cannot_resume_while_user_input_is_still_in_flight() {
    let user = lease()
        .take_control(
            BrowserControlActor::User {
                actor_id: "principal-1".into(),
            },
            7,
            200,
            0,
        )
        .unwrap();
    let observation = BrowserObservation::new(
        "env-1",
        "task-1",
        "attempt-1",
        "runtime-1",
        "incarnation-1",
        8,
        4,
        210,
    )
    .unwrap();

    assert_eq!(
        user.return_to_agent(
            BrowserControlActor::User {
                actor_id: "principal-1".into(),
            },
            BrowserControlActor::Agent {
                actor_id: "agent-1".into()
            },
            8,
            220,
            1,
            0,
            1_000,
            &observation
        ),
        Err(BrowserControlError::ActionsInFlight)
    );
}

#[test]
fn returning_to_agent_requires_an_observation_from_the_current_user_epoch() {
    let user = lease()
        .take_control(
            BrowserControlActor::User {
                actor_id: "principal-1".into(),
            },
            7,
            200,
            0,
        )
        .unwrap();
    let stale = BrowserObservation::new(
        "env-1",
        "task-1",
        "attempt-1",
        "runtime-1",
        "incarnation-1",
        7,
        1,
        210,
    )
    .unwrap();

    assert_eq!(
        user.return_to_agent(
            BrowserControlActor::User {
                actor_id: "principal-1".into(),
            },
            BrowserControlActor::Agent {
                actor_id: "agent-1".into()
            },
            8,
            220,
            0,
            0,
            1_000,
            &stale
        ),
        Err(BrowserControlError::StaleObservation)
    );
}

#[test]
fn returning_to_agent_rejects_an_observation_with_the_same_millisecond_as_takeover() {
    let user = lease()
        .take_control(
            BrowserControlActor::User {
                actor_id: "principal-1".into(),
            },
            7,
            200,
            0,
        )
        .unwrap();
    let observation = BrowserObservation::new(
        "env-1",
        "task-1",
        "attempt-1",
        "runtime-1",
        "incarnation-1",
        8,
        2,
        200,
    )
    .unwrap();

    assert_eq!(
        user.return_to_agent(
            BrowserControlActor::User {
                actor_id: "principal-1".into(),
            },
            BrowserControlActor::Agent {
                actor_id: "agent-1".into(),
            },
            8,
            220,
            0,
            1,
            1_000,
            &observation,
        ),
        Err(BrowserControlError::StaleObservation)
    );
}

#[test]
fn returning_to_agent_requires_an_observation_newer_than_the_last_known_revision() {
    let user = lease()
        .take_control(
            BrowserControlActor::User {
                actor_id: "principal-1".into(),
            },
            7,
            200,
            0,
        )
        .unwrap();
    let observation = BrowserObservation::new(
        "env-1",
        "task-1",
        "attempt-1",
        "runtime-1",
        "incarnation-1",
        8,
        4,
        210,
    )
    .unwrap();

    assert_eq!(
        user.return_to_agent(
            BrowserControlActor::User {
                actor_id: "principal-1".into(),
            },
            BrowserControlActor::Agent {
                actor_id: "agent-1".into(),
            },
            8,
            220,
            0,
            4,
            1_000,
            &observation,
        ),
        Err(BrowserControlError::StaleObservation)
    );
}

#[test]
fn returning_to_agent_rejects_an_observation_older_than_the_callers_freshness_limit() {
    let user = lease()
        .take_control(
            BrowserControlActor::User {
                actor_id: "principal-1".into(),
            },
            7,
            200,
            0,
        )
        .unwrap();
    let observation = BrowserObservation::new(
        "env-1",
        "task-1",
        "attempt-1",
        "runtime-1",
        "incarnation-1",
        8,
        2,
        210,
    )
    .unwrap();

    assert_eq!(
        user.return_to_agent(
            BrowserControlActor::User {
                actor_id: "principal-1".into(),
            },
            BrowserControlActor::Agent {
                actor_id: "agent-1".into(),
            },
            8,
            900,
            0,
            1,
            100,
            &observation,
        ),
        Err(BrowserControlError::StaleObservation)
    );
}

#[test]
fn a_different_user_cannot_return_control_from_the_current_user() {
    let user = lease()
        .take_control(
            BrowserControlActor::User {
                actor_id: "principal-1".into(),
            },
            7,
            200,
            0,
        )
        .unwrap();
    let observation = BrowserObservation::new(
        "env-1",
        "task-1",
        "attempt-1",
        "runtime-1",
        "incarnation-1",
        8,
        1,
        210,
    )
    .unwrap();

    assert_eq!(
        user.return_to_agent(
            BrowserControlActor::User {
                actor_id: "principal-2".into(),
            },
            BrowserControlActor::Agent {
                actor_id: "agent-1".into(),
            },
            8,
            220,
            0,
            0,
            1_000,
            &observation,
        ),
        Err(BrowserControlError::WrongOwner)
    );
}

#[test]
fn current_epoch_does_not_authorize_a_different_agent() {
    assert_eq!(
        lease().authorize_action(
            &BrowserControlActor::Agent {
                actor_id: "agent-2".into()
            },
            7,
            201
        ),
        Err(BrowserControlError::WrongOwner)
    );
}

#[test]
fn returning_to_agent_rejects_observation_from_another_attempt() {
    let user = lease()
        .take_control(
            BrowserControlActor::User {
                actor_id: "principal-1".into(),
            },
            7,
            200,
            0,
        )
        .unwrap();
    let observation = BrowserObservation::new(
        "env-1",
        "task-1",
        "attempt-2",
        "runtime-1",
        "incarnation-1",
        8,
        1,
        210,
    )
    .unwrap();

    assert_eq!(
        user.return_to_agent(
            BrowserControlActor::User {
                actor_id: "principal-1".into(),
            },
            BrowserControlActor::Agent {
                actor_id: "agent-1".into()
            },
            8,
            220,
            0,
            0,
            1_000,
            &observation
        ),
        Err(BrowserControlError::ObservationIdentityMismatch)
    );
}

#[test]
fn hydrated_user_owned_lease_can_return_to_its_original_agent_session() {
    let lease = BrowserControlLease::new_with_agent_owner(
        "env-1",
        "task-1",
        "attempt-1",
        "runtime-1",
        "incarnation-1",
        BrowserControlActor::User {
            actor_id: "principal-1".into(),
        },
        Some(BrowserControlActor::Agent {
            actor_id: "agent-1".into(),
        }),
        8,
        200,
        1_000,
    )
    .unwrap();
    let observation = BrowserObservation::new(
        "env-1",
        "task-1",
        "attempt-1",
        "runtime-1",
        "incarnation-1",
        8,
        4,
        210,
    )
    .unwrap();

    let returned = lease
        .return_to_agent(
            BrowserControlActor::User {
                actor_id: "principal-1".into(),
            },
            BrowserControlActor::Agent {
                actor_id: "agent-1".into(),
            },
            8,
            220,
            0,
            3,
            1_000,
            &observation,
        )
        .unwrap();

    assert_eq!(returned.epoch(), 9);
    assert_eq!(
        returned.owner(),
        &BrowserControlActor::Agent {
            actor_id: "agent-1".into()
        }
    );
}

#[test]
fn rejects_rehydrated_user_lease_without_its_original_agent_session() {
    assert!(matches!(
        BrowserControlLease::new_with_agent_owner(
            "env-1",
            "task-1",
            "attempt-1",
            "runtime-1",
            "incarnation-1",
            BrowserControlActor::User {
                actor_id: "principal-1".into(),
            },
            None,
            8,
            200,
            1_000,
        ),
        Err(BrowserControlError::WrongOwner)
    ));
}

#[test]
fn returning_to_agent_advances_epoch_only_after_fresh_observation_and_quiescence() {
    let user = lease()
        .take_control(
            BrowserControlActor::User {
                actor_id: "principal-1".into(),
            },
            7,
            200,
            0,
        )
        .unwrap();
    let observation = BrowserObservation::new(
        "env-1",
        "task-1",
        "attempt-1",
        "runtime-1",
        "incarnation-1",
        8,
        4,
        210,
    )
    .unwrap();

    let agent = user
        .return_to_agent(
            BrowserControlActor::User {
                actor_id: "principal-1".into(),
            },
            BrowserControlActor::Agent {
                actor_id: "agent-1".into(),
            },
            8,
            220,
            0,
            0,
            10,
            &observation,
        )
        .unwrap();

    assert_eq!(agent.epoch(), 9);
    assert_eq!(
        agent.owner(),
        &BrowserControlActor::Agent {
            actor_id: "agent-1".into()
        }
    );
    assert_eq!(
        agent.authorize_action(
            &BrowserControlActor::Agent {
                actor_id: "agent-1".into()
            },
            9,
            221
        ),
        Ok(())
    );
}

#[test]
fn a_lease_from_an_old_runtime_incarnation_cannot_authorize_actions() {
    let old = lease()
        .take_control(
            BrowserControlActor::User {
                actor_id: "principal-1".into(),
            },
            7,
            200,
            0,
        )
        .unwrap();
    let observation = BrowserObservation::new(
        "env-1",
        "task-1",
        "attempt-1",
        "runtime-1",
        "new-incarnation",
        8,
        1,
        210,
    )
    .unwrap();

    assert_eq!(
        old.return_to_agent(
            BrowserControlActor::User {
                actor_id: "principal-1".into(),
            },
            BrowserControlActor::Agent {
                actor_id: "agent-1".into()
            },
            8,
            220,
            0,
            0,
            1_000,
            &observation
        ),
        Err(BrowserControlError::ObservationIdentityMismatch)
    );
}

#[test]
fn expired_lease_cannot_admit_even_the_current_owner() {
    assert_eq!(
        lease().authorize_action(
            &BrowserControlActor::Agent {
                actor_id: "agent-1".into()
            },
            7,
            1_000
        ),
        Err(BrowserControlError::Expired)
    );
}

#[test]
fn control_epoch_exhaustion_fails_closed_for_both_transfer_directions() {
    let agent_owned = BrowserControlLease::new(
        "env-1",
        "task-1",
        "attempt-1",
        "runtime-1",
        "incarnation-1",
        BrowserControlActor::Agent {
            actor_id: "agent-1".into(),
        },
        u64::MAX,
        100,
        1_000,
    )
    .unwrap();
    assert_eq!(
        agent_owned.take_control(
            BrowserControlActor::User {
                actor_id: "principal-1".into(),
            },
            u64::MAX,
            200,
            0,
        ),
        Err(BrowserControlError::EpochExhausted)
    );

    let user_owned = BrowserControlLease::new_with_agent_owner(
        "env-1",
        "task-1",
        "attempt-1",
        "runtime-1",
        "incarnation-1",
        BrowserControlActor::User {
            actor_id: "principal-1".into(),
        },
        Some(BrowserControlActor::Agent {
            actor_id: "agent-1".into(),
        }),
        u64::MAX,
        200,
        1_000,
    )
    .unwrap();
    let fresh_observation = BrowserObservation::new(
        "env-1",
        "task-1",
        "attempt-1",
        "runtime-1",
        "incarnation-1",
        u64::MAX,
        2,
        210,
    )
    .unwrap();
    assert_eq!(
        user_owned.return_to_agent(
            BrowserControlActor::User {
                actor_id: "principal-1".into(),
            },
            BrowserControlActor::Agent {
                actor_id: "agent-1".into(),
            },
            u64::MAX,
            220,
            0,
            1,
            20,
            &fresh_observation,
        ),
        Err(BrowserControlError::EpochExhausted)
    );
}

#[test]
fn return_observation_must_match_each_environment_task_and_runtime_binding() {
    let user_owned = lease()
        .take_control(
            BrowserControlActor::User {
                actor_id: "principal-1".into(),
            },
            7,
            200,
            0,
        )
        .unwrap();
    let current_user = BrowserControlActor::User {
        actor_id: "principal-1".into(),
    };
    let original_agent = BrowserControlActor::Agent {
        actor_id: "agent-1".into(),
    };
    let observations = [
        BrowserObservation::new(
            "env-other",
            "task-1",
            "attempt-1",
            "runtime-1",
            "incarnation-1",
            8,
            2,
            210,
        )
        .unwrap(),
        BrowserObservation::new(
            "env-1",
            "task-other",
            "attempt-1",
            "runtime-1",
            "incarnation-1",
            8,
            2,
            210,
        )
        .unwrap(),
        BrowserObservation::new(
            "env-1",
            "task-1",
            "attempt-1",
            "runtime-other",
            "incarnation-1",
            8,
            2,
            210,
        )
        .unwrap(),
    ];

    for observation in &observations {
        assert_eq!(
            user_owned.return_to_agent(
                current_user.clone(),
                original_agent.clone(),
                8,
                220,
                0,
                1,
                20,
                observation,
            ),
            Err(BrowserControlError::ObservationIdentityMismatch)
        );
    }
}

#[test]
fn return_observation_at_exact_maximum_age_is_accepted_but_older_is_rejected() {
    let user_owned = lease()
        .take_control(
            BrowserControlActor::User {
                actor_id: "principal-1".into(),
            },
            7,
            200,
            0,
        )
        .unwrap();
    let user = BrowserControlActor::User {
        actor_id: "principal-1".into(),
    };
    let agent = BrowserControlActor::Agent {
        actor_id: "agent-1".into(),
    };
    let observation = BrowserObservation::new(
        "env-1",
        "task-1",
        "attempt-1",
        "runtime-1",
        "incarnation-1",
        8,
        2,
        210,
    )
    .unwrap();
    assert!(
        user_owned
            .return_to_agent(user.clone(), agent.clone(), 8, 230, 0, 1, 20, &observation,)
            .is_ok()
    );
    assert_eq!(
        user_owned.return_to_agent(user, agent, 8, 231, 0, 1, 20, &observation),
        Err(BrowserControlError::StaleObservation)
    );
}

#[test]
fn expired_control_lease_cannot_transfer_to_user_or_return_to_agent() {
    let agent_owned = lease();
    assert_eq!(
        agent_owned.take_control(
            BrowserControlActor::User {
                actor_id: "principal-1".into(),
            },
            7,
            1_000,
            0,
        ),
        Err(BrowserControlError::Expired)
    );

    let user_owned = agent_owned
        .take_control(
            BrowserControlActor::User {
                actor_id: "principal-1".into(),
            },
            7,
            200,
            0,
        )
        .unwrap();
    let observation = BrowserObservation::new(
        "env-1",
        "task-1",
        "attempt-1",
        "runtime-1",
        "incarnation-1",
        8,
        2,
        210,
    )
    .unwrap();
    assert_eq!(
        user_owned.return_to_agent(
            BrowserControlActor::User {
                actor_id: "principal-1".into(),
            },
            BrowserControlActor::Agent {
                actor_id: "agent-1".into(),
            },
            8,
            1_000,
            0,
            1,
            1_000,
            &observation,
        ),
        Err(BrowserControlError::Expired)
    );
}

#[test]
fn stale_expected_epoch_cannot_take_control() {
    assert_eq!(
        lease().take_control(
            BrowserControlActor::User {
                actor_id: "principal-1".into(),
            },
            6,
            200,
            0,
        ),
        Err(BrowserControlError::StaleEpoch {
            expected: 7,
            actual: 6,
        })
    );
}
