# Machine contract inventory (generated)

## sqlite-v1.sql (4358 lines)
- tables (107): schema_migrations, workspace_origin_sequences, workspaces, workspace_instruction_revisions, conversations, conversation_messages, conversation_turns, provider_circuit_states, tasks, goals, goal_revisions, goal_task_links, goal_routine_links, suggestions, suggestion_preferences, demonstration_sessions, task_spec_revisions, plan_revisions, steps, agent_profiles, agent_endpoints, agent_bindings, delegation_profiles, delegation_profile_revisions, coworkers, coworker_revisions, agent_host_instances, runtimes, runtime_incarnations, runtime_incarnation_local_observations, agent_endpoint_bindings, runtime_offers, pairing_tokens, environments, environment_provider_bindings, environment_provision_previews, environment_checkpoints, environment_checkpoint_provider_bindings, attempts, agent_sessions, agent_session_host_bindings, execution_leases, capability_grants, secret_leases, capability_host_instances, capability_activations, capability_activation_host_bindings, capability_locks, connections, channel_bindings, channel_host_assignments, channel_host_lease_records, channel_thread_mappings, channel_event_receipts, channel_ingress_cursor_bindings, artifacts, artifact_versions, effects, evidence, audit_records, verification_runs, approvals, approval_uses, routines, routine_revisions, automations, automation_revisions, automation_occurrences, automation_cursors, automation_trigger_bindings, domain_events, request_dedup, replication_cursors, replication_receipts, replication_aggregate_positions, pending_replication_events, resources, context_document_purge_plans, context_document_purge_receipts, resource_revisions, resource_revision_parents, resource_locations, resource_location_bindings, file_identity_bindings, workspace_roots, workspace_replication_roots, resource_edges, dependency_edges, invalidation_records, capability_invocations, capability_invocation_provider_bindings, user_requests, user_request_responses, provider_input_bindings, usage_observations, budget_reservations, notification_preferences, notification_deliveries, channel_reply_targets, skill_proposals, environment_control_leases, resource_upload_sessions, resource_upload_chunks, aggregate_snapshots, event_archive_segments, workspace_backup_manifests, handoffs
- alters: 0; indexes: 76; triggers: 140
## sqlite-v2.sql (152 lines)
- tables (3): resource_upload_chunk_requests, resource_upload_blob_reservations, resource_upload_blob_gc_fences
- alters: 3; indexes: 1; triggers: 5
## sqlite-v3.sql (115 lines)
- tables (1): runtime_workspace_bindings
- alters: 0; indexes: 2; triggers: 4
## sqlite-v4.sql (1159 lines)
- tables (9): runtimes_v4, environments_v4, channel_host_assignments_v4, channel_host_lease_records_v4, channel_host_drain_proofs, channel_host_lease_releases, channel_host_continuity_proofs, automation_occurrences_v4, automation_cursors_v4
- alters: 6; indexes: 7; triggers: 46
## sqlite-v5.sql (40 lines)
- tables (0): -
- alters: 0; indexes: 1; triggers: 4
## sqlite-v6.sql (39 lines)
- tables (1): resource_locations_v6
- alters: 1; indexes: 1; triggers: 0
## sqlite-v7.sql (85 lines)
- tables (2): resource_text_indexes, resource_text_index_terms
- alters: 0; indexes: 2; triggers: 3
## sqlite-v8.sql (57 lines)
- tables (0): -
- alters: 0; indexes: 0; triggers: 5
## sqlite-v9.sql (88 lines)
- tables (0): -
- alters: 0; indexes: 0; triggers: 6
## sqlite-v10.sql (46 lines)
- tables (1): goal_artifact_links
- alters: 0; indexes: 1; triggers: 3
## sqlite-v11.sql (28 lines)
- tables (0): -
- alters: 1; indexes: 0; triggers: 2
## sqlite-v12.sql (64 lines)
- tables (1): rich_presentations
- alters: 1; indexes: 1; triggers: 4
## sqlite-v13.sql (29 lines)
- tables (0): -
- alters: 0; indexes: 0; triggers: 1
## sqlite-v14.sql (17 lines)
- tables (0): -
- alters: 1; indexes: 0; triggers: 2
## sqlite-v15.sql (24 lines)
- tables (0): -
- alters: 0; indexes: 0; triggers: 2
## sqlite-v16.sql (219 lines)
- tables (5): agent_registry_cache, agent_installation_observations, agent_control_descriptors, agent_lifecycle_operations, agent_binding_configurations
- alters: 0; indexes: 5; triggers: 2
## sqlite-v17.sql (189 lines)
- tables (1): resource_content_blob_registry
- alters: 0; indexes: 1; triggers: 7
## sqlite-v18.sql (493 lines)
- tables (2): resource_upload_chunk_blob_registry, context_document_purge_receipts
- alters: 1; indexes: 2; triggers: 16
## sqlite-v19.sql (78 lines)
- tables (1): resource_index_blob_registry
- alters: 0; indexes: 1; triggers: 3
## sqlite-v20.sql (95 lines)
- tables (0): -
- alters: 2; indexes: 1; triggers: 4
## sqlite-v21.sql (121 lines)
- tables (4): workspace_root_observation_generations, workspace_root_observations, workspace_root_observation_bindings, workspace_root_observation_batch_receipts
- alters: 0; indexes: 3; triggers: 1
## sqlite-v22.sql (90 lines)
- tables (0): -
- alters: 1; indexes: 1; triggers: 3
## sqlite-v23.sql (32 lines)
- tables (2): workspace_root_file_locations, workspace_root_cleanup_outbox
- alters: 0; indexes: 1; triggers: 0
## sqlite-v24.sql (42 lines)
- tables (1): delegation_results
- alters: 0; indexes: 0; triggers: 2
## sqlite-v25.sql (13 lines)
- tables (0): -
- alters: 0; indexes: 0; triggers: 2
## sqlite-v26.sql (62 lines)
- tables (0): -
- alters: 0; indexes: 0; triggers: 2
## sqlite-v27.sql (92 lines)
- tables (1): automation_test_runs
- alters: 1; indexes: 0; triggers: 5
## sqlite-v28.sql (234 lines)
- tables (1): workspace_delegation_settings_revisions
- alters: 4; indexes: 1; triggers: 8

Total CREATE TABLE across migrations: 143

## operator-api.openapi.yaml
- openapi 3.1.0; paths 203; operations 229
- methods: {'GET': 108, 'POST': 107, 'PATCH': 8, 'PUT': 6}
- tags: {'agents': 22, 'runtime-lifecycle': 5, 'workspaces': 15, 'delegation': 10, 'recovery': 1, 'runtimes': 10, 'conversations': 18, 'usage': 1, 'presentation': 3, 'tasks': 27, 'artifacts': 10, 'approvals': 3, 'trust': 2, 'routines': 7, 'automations': 14, 'coworkers': 8, 'goals': 5, 'suggestions': 6, 'demonstrations': 6, 'connections': 3, 'channels': 5, 'discover': 4, 'capabilities': 13, 'resources': 29, 'environments': 8, 'needs-you': 1, 'notifications': 3}
- securitySchemes: {"workspaceContext": {"type": "apiKey", "in": "header", "name": "X-Workspace-ID", "description": "Required Workspace scope for all operations except Workspace list/create, the local agent-installation inventory, Task planning-readiness, and the recovery bootstrap; for path/body Workspace IDs, values must match."}, "operatorAuth": {"type": "http", "scheme": "bearer", "bearerFormat": "opaque"}}
- components.schemas: 405

### operation list

- GET /agent-installations -> listLocalAgentInstallations [agents]
- GET /agent-registry -> listAgentRegistry [agents]
- POST /agent-registry/refresh -> refreshAgentRegistry [agents]
- POST /agent-registry/{registryAgentId}/install -> installRegistryAgent [agents]
- POST /agent-registry/{registryAgentId}/update -> updateRegistryAgent [agents]
- GET /operator/readiness -> getLocalOperatorReadiness [runtime-lifecycle]
- GET /workspaces -> listWorkspaces [workspaces]
- POST /workspaces -> createWorkspace [workspaces]
- GET /workspaces/{workspaceId} -> getWorkspace [workspaces]
- PATCH /workspaces/{workspaceId} -> updateWorkspacePolicy [workspaces]
- GET /workspaces/{workspaceId}/delegation-settings -> getWorkspaceDelegationSettings [delegation]
- PUT /workspaces/{workspaceId}/delegation-settings -> setWorkspaceDelegationSettings [delegation]
- GET /workspaces/{workspaceId}/backups -> listWorkspaceBackups [workspaces]
- POST /workspaces/{workspaceId}/backups -> createWorkspaceBackup [workspaces]
- GET /workspaces/{workspaceId}/backups/{backupId} -> getWorkspaceBackup [workspaces]
- POST /recovery/backups/{backupId}/restore -> restoreWorkspaceBackup [recovery]
- PATCH /workspaces/{workspaceId}/default-agent-binding -> setWorkspaceDefaultAgentBinding [workspaces]
- POST /workspaces/{workspaceId}/archive -> archiveWorkspace [workspaces]
- GET /workspaces/{workspaceId}/runtime-bindings -> listWorkspaceRuntimeBindings [workspaces,runtimes]
- GET /workspaces/{workspaceId}/runtime-bindings/current-local -> getCurrentLocalRuntimeWorkspaceBinding [workspaces,runtimes]
- POST /workspaces/{workspaceId}/runtime-bindings/local-enrollment -> enrollLocalRuntimeInWorkspace [workspaces,runtimes]
- POST /workspaces/{workspaceId}/runtime-bindings/{bindingId}/revoke -> revokeWorkspaceRuntimeBinding [workspaces,runtimes]
- GET /conversations -> listConversations [conversations]
- POST /conversations -> createConversation [conversations]
- GET /conversations/{conversationId} -> getConversation [conversations]
- PATCH /conversations/{conversationId}/agent-binding -> setConversationAgentBinding [conversations,agents]
- GET /conversations/{conversationId}/usage -> getConversationUsage [conversations,usage]
- GET /conversations/{conversationId}/presentation -> getConversationPresentation [conversations,presentation]
- GET /rich-presentations/{presentationId} -> getRichPresentation [conversations,presentation]
- GET /conversations/{conversationId}/messages -> listConversationMessages [conversations]
- POST /conversations/{conversationId}/messages -> appendConversationMessage [conversations]
- GET /conversations/{conversationId}/tasks -> listConversationTasks [conversations]
- POST /conversations/{conversationId}/turns -> submitConversationTurn [conversations]
- POST /conversations/{conversationId}/turns/{turnId}/retry -> retryConversationTurn [conversations]
- POST /conversations/{conversationId}/turns/{turnId}/cancel -> cancelConversationTurn [conversations]
- GET /tasks -> listTasks [tasks]
- POST /tasks -> createTask [tasks]
- GET /tasks/{taskId} -> getTask [tasks]
- GET /tasks/{taskId}/planning-readiness -> getTaskPlanningReadiness [tasks]
- GET /tasks/{taskId}/presentation -> getTaskPresentation [tasks,presentation]
- GET /tasks/{taskId}/progress -> getTaskProgress [tasks]
- GET /tasks/{taskId}/steps/{stepId}/execution-dependencies -> getStepExecutionDependencies [tasks]
- POST /tasks/{taskId}/steer -> steerTask [tasks]
- POST /tasks/{taskId}/lead-agent -> changeTaskLeadAgent [tasks]
- POST /tasks/{taskId}/cancel -> cancelTask [tasks]
- POST /tasks/{taskId}/pause -> pauseTask [tasks]
- POST /tasks/{taskId}/resume -> resumeTask [tasks]
- POST /tasks/{taskId}/steps/{stepId}/recover -> recoverTaskStep [tasks]
- GET /tasks/{taskId}/spec-revisions -> listTaskSpecRevisions [tasks]
- POST /tasks/{taskId}/spec-revisions -> reviseTaskSpec [tasks]
- GET /tasks/{taskId}/plan-revisions -> listPlanRevisions [tasks]
- POST /tasks/{taskId}/plan-revisions -> submitTaskPlan [tasks]
- GET /tasks/{taskId}/steps -> listTaskSteps [tasks]
- GET /tasks/{taskId}/attempts -> listTaskAttempts [tasks]
- GET /tasks/{taskId}/timeline -> getTaskTimeline [tasks]
- GET /tasks/{taskId}/artifacts -> listTaskArtifacts [artifacts]
- GET /tasks/{taskId}/effects -> listTaskEffects [tasks]
- GET /approvals -> listApprovals [approvals]
- GET /approvals/{approvalId} -> getApproval [approvals]
- POST /approvals/{approvalId}/resolve -> resolveApproval [approvals]
- POST /capability-grants -> createCapabilityGrant [trust]
- POST /capability-grants/{grantId}/revoke -> revokeCapabilityGrant [trust]
- GET /artifacts -> listArtifacts [artifacts]
- GET /artifacts/{artifactId} -> getArtifact [artifacts]
- GET /artifacts/{artifactId}/edit-head -> getArtifactTextEditHead [artifacts]
- POST /artifacts/{artifactId}/text-version -> appendArtifactTextVersion [artifacts]
- GET /artifacts/{artifactId}/versions/{version} -> getArtifactVersion [artifacts]
- GET /artifacts/{artifactId}/versions/{version}/content -> downloadArtifactVersion [artifacts]
- POST /artifacts/{artifactId}/promote -> promoteArtifact [artifacts]
- POST /artifacts/{artifactId}/archive -> archiveArtifact [artifacts]
- GET /library -> listLibrary [artifacts]
- GET /routines -> listRoutines [routines]
- POST /routines -> createRoutine [routines]
- GET /routines/{routineId} -> getRoutine [routines]
- GET /routines/{routineId}/revisions -> listRoutineRevisions [routines]
- POST /routines/{routineId}/revisions -> reviseRoutine [routines]
- GET /routines/{routineId}/health -> getRoutineHealth [automations]
- POST /routines/{routineId}/run -> runRoutine [routines]
- POST /routines/{routineId}/archive -> archiveRoutine [routines]
- GET /automations -> listAutomations [automations]
- POST /automations -> createAutomation [automations]
- GET /automations/{automationId} -> getAutomation [automations]
- PATCH /automations/{automationId} -> updateAutomation [automations]
- POST /automations/{automationId}/run -> runAutomationNow [automations]
- POST /automations/{automationId}/test-runs -> runAutomationTest [automations]
- GET /automations/{automationId}/revisions -> listAutomationRevisions [automations]
- GET /automations/{automationId}/occurrences -> listAutomationOccurrences [automations]
- GET /automations/{automationId}/health -> getAutomationHealth [automations]
- POST /automations/host/settle -> settleAutomationTriggers [automations]
- POST /automations/{automationId}/pause -> pauseAutomation [automations]
- POST /automations/{automationId}/resume -> resumeAutomation [automations]
- POST /automations/{automationId}/disable -> disableAutomation [automations]
- GET /runtime-lifecycle -> getLocalRuntimeLifecycle [runtime-lifecycle]
- PATCH /runtime-lifecycle/startup-policy -> setLocalRuntimeStartupPolicy [runtime-lifecycle]
- GET /runtime-lifecycle/stop-preview -> previewLocalRuntimeStop [runtime-lifecycle]
- POST /runtime-lifecycle/stop -> requestLocalRuntimeStop [runtime-lifecycle]
- GET /runtimes -> listRuntimes [runtimes]
- POST /runtimes/pair-token -> createRuntimePairToken [runtimes]
- POST /runtimes/{runtimeId}/revoke -> revokeRuntime [runtimes]
- GET /runtimes/{runtimeId}/incarnations -> listRuntimeIncarnations [runtimes]
- GET /runtimes/{runtimeId}/offers -> getRuntimeOffers [runtimes]
- GET /runtimes/{runtimeId}/capability-hosts -> listRuntimeCapabilityHosts [runtimes]
- GET /agent-profiles -> listAgentProfiles [agents]
- POST /agent-profiles/probe -> probeLocalAgentProfile [agents]
- POST /agent-profiles/{agentProfileId}/refresh -> refreshAgentProfileControl [agents]
- GET /agent-profiles/{agentProfileId}/control-descriptor -> getAgentControlDescriptor [agents]
- POST /agent-profiles/{agentProfileId}/auth/{authMethodId}/begin -> beginAgentAuthentication [agents]
- POST /agent-profiles/{agentProfileId}/credentials/{credentialSlotId} -> submitAgentCredential [agents]
- POST /agent-profiles/{agentProfileId}/native-config/{targetId}/open -> openAgentNativeConfiguration [agents]
- GET /agent-bindings -> listAgentBindings [agents]
- POST /agent-bindings -> createAgentBinding [agents]
- GET /agent-bindings/{agentBindingId} -> getAgentBinding [agents]
- PUT /agent-bindings/{agentBindingId}/configuration -> configureAgentBinding [agents]
- POST /agent-bindings/{agentBindingId}/enable -> enableAgentBinding [agents]
- POST /agent-bindings/{agentBindingId}/disable -> disableAgentBinding [agents]
- GET /agent-bindings/{agentBindingId}/session-options -> getAgentSessionOptions [agents]
- GET /agent-bindings/{agentBindingId}/harness-capabilities -> getAgentHarnessCapabilities [agents]
- GET /agent-bindings/{agentBindingId}/quota-observation -> getAgentQuotaObservation [agents]
- GET /delegation-profiles -> listDelegationProfiles [delegation]
- POST /delegation-profiles -> createDelegationProfile [delegation]
- GET /delegation-profiles/{delegationProfileId} -> getDelegationProfile [delegation]
- POST /delegation-profiles/{delegationProfileId}/duplicate -> duplicateDelegationProfile [delegation]
- GET /delegation-profiles/{delegationProfileId}/performance -> getDelegationProfilePerformance [delegation]
- GET /delegation-profiles/{delegationProfileId}/revisions -> listDelegationProfileRevisions [delegation]
- POST /delegation-profiles/{delegationProfileId}/revisions -> reviseDelegationProfile [delegation]
- POST /delegation-profiles/{delegationProfileId}/status -> changeDelegationProfileStatus [delegation]
- GET /coworkers -> listCoworkers [coworkers]
- POST /coworkers -> createCoworker [coworkers]
- GET /coworkers/{coworkerId} -> getCoworker [coworkers]
- GET /coworkers/{coworkerId}/revisions/{revision} -> getCoworkerRevision [coworkers]
- GET /coworkers/{coworkerId}/presence -> getCoworkerPresence [coworkers]
- POST /coworkers/{coworkerId}/revisions -> reviseCoworker [coworkers]
- POST /coworkers/{coworkerId}/status -> changeCoworkerStatus [coworkers]
- POST /workspaces/{workspaceId}/primary-coworker -> setPrimaryCoworker [coworkers]
- GET /goals -> listGoals [goals]
- POST /goals -> createGoal [goals]
- GET /goals/{goalId} -> getGoal [goals]
- POST /goals/{goalId}/revisions -> reviseGoal [goals]
- POST /goals/{goalId}/status -> changeGoalStatus [goals]
- GET /suggestions -> listSuggestions [suggestions]
- POST /suggestions/{suggestionId}/resolve -> resolveSuggestion [suggestions]
- POST /suggestions/{suggestionId}/accept-task -> acceptSuggestionTask [suggestions,tasks]
- POST /suggestions/{suggestionId}/snooze -> snoozeSuggestion [suggestions]
- GET /workspaces/{workspaceId}/suggestion-preferences -> listSuggestionPreferences [suggestions]
- PUT /workspaces/{workspaceId}/suggestion-preferences/{kind} -> setSuggestionPreference [suggestions]
- POST /demonstrations -> startDemonstration [demonstrations]
- POST /demonstrations/{demonstrationId}/actions -> appendDemonstrationAction [demonstrations]
- POST /demonstrations/{demonstrationId}/complete -> completeDemonstration [demonstrations]
- POST /demonstrations/{demonstrationId}/pause -> pauseDemonstration [demonstrations]
- POST /demonstrations/{demonstrationId}/resume -> resumeDemonstration [demonstrations]
- POST /demonstrations/{demonstrationId}/abort -> abortDemonstration [demonstrations]
- GET /connections -> listConnections [connections]
- GET /connections/{connectionId} -> getConnection [connections]
- POST /connections/{connectionId}/disconnect -> disconnectConnection [connections]
- GET /channel-bindings -> listChannelBindings [channels]
- GET /channel-bindings/{channelBindingId} -> getChannelBinding [channels]
- PATCH /channel-bindings/{channelBindingId} -> updateChannelBindingActions [channels]
- POST /channel-bindings/{channelBindingId}/revoke -> revokeChannelBinding [channels]
- POST /channel-bindings/{channelBindingId}/host-assignment -> reassignChannelHost [channels]
- GET /discover/search -> searchDiscover [discover]
- GET /discover/items/{itemId} -> describeDiscoverItem [discover]
- POST /capabilities/{capabilityId}/connect-or-install -> connectOrInstallCapability [discover]
- POST /capabilities/{capabilityId}/test -> testCapabilityConnection [discover]
- GET /capabilities/zip-intake -> getZipIntakeReadiness [capabilities]
- GET /resources -> listLocalResources [resources]
- POST /resources/quick-import -> quickImportResource [resources]
- POST /resources/{resourceId}/archive/preview -> previewResourceArchive [resources]
- GET /resources/{resourceId}/content -> readLocalResourceContent [resources]
- POST /resources/uploads -> createResourceUpload [resources]
- GET /resources/uploads/{uploadId} -> getResourceUpload [resources]
- PUT /resources/uploads/{uploadId}/chunks/{chunkIndex} -> uploadResourceChunk [resources]
- POST /resources/uploads/{uploadId}/commit -> commitResourceUpload [resources]
- POST /resources/{resourceId}/revision-uploads -> createResourceRevisionUpload [resources]
- PATCH /resources/{resourceId}/context-document/status -> setContextDocumentStatus [resources]
- POST /resources/{resourceId}/context-document/deletion -> requestContextDocumentDeletion [resources]
- GET /resources/{resourceId}/context-document/deletion -> getContextDocumentDeletionStatus [resources]
- GET /resources/search -> searchResources [resources]
- GET /capabilities/local-models -> getLocalModelAvailability [capabilities]
- POST /resources/retrieval -> groundedRetrieval [resources]
- GET /resources/{resourceId}/index-state -> getResourceIndexState [resources]
- POST /resources/{resourceId}/text-index/rebuild -> rebuildResourceTextIndex [resources]
- GET /resources/{resourceId} -> getResource [resources]
- GET /resources/{resourceId}/locations -> listResourceLocations [resources]
- GET /resources/{resourceId}/revisions -> listResourceRevisions [resources]
- GET /workspace-roots -> listWorkspaceRoots [resources]
- POST /workspace-roots -> addWorkspaceRoot [resources]
- PATCH /workspace-roots/{rootId} -> updateWorkspaceRoot [resources]
- POST /workspace-roots/{rootId}/pause -> pauseWorkspaceRoot [resources]
- POST /workspace-roots/{rootId}/resume -> resumeWorkspaceRoot [resources]
- POST /workspace-roots/{rootId}/revoke -> revokeWorkspaceRoot [resources]
- POST /workspace-roots/{rootId}/scan -> scanWorkspaceRoot [resources]
- GET /workspace-roots/{rootId}/observations -> searchWorkspaceRootObservations [resources]
- GET /user-requests -> listUserRequests [conversations]
- POST /user-requests/{requestId}/respond -> respondToUserRequest [conversations]
- POST /user-requests/{requestId}/external-handoff -> openUserRequestExternalHandoff [conversations]
- POST /tasks/{taskId}/attempts/{attemptId}/take-control -> takeEnvironmentControl [tasks]
- POST /tasks/{taskId}/attempts/{attemptId}/return-control -> returnEnvironmentControl [tasks]
- GET /workspaces/{workspaceId}/instructions/revisions -> listWorkspaceInstructionRevisions [workspaces]
- POST /workspaces/{workspaceId}/instructions/revisions -> createWorkspaceInstructionRevision [workspaces]
- GET /workspaces/{workspaceId}/environments -> listWorkspaceEnvironments [environments]
- POST /workspaces/{workspaceId}/environments -> createPersistentWorkspaceEnvironment [environments]
- POST /workspaces/{workspaceId}/environments/provision-preview -> previewPersistentEnvironmentProvision [environments]
- GET /workspaces/{workspaceId}/environments/{environmentId} -> getWorkspaceEnvironment [environments]
- POST /workspaces/{workspaceId}/environments/{environmentId}/suspend -> suspendWorkspaceEnvironment [environments]
- POST /workspaces/{workspaceId}/environments/{environmentId}/resume -> resumeWorkspaceEnvironment [environments]
- POST /workspaces/{workspaceId}/environments/{environmentId}/sharing-scope -> changeWorkspaceEnvironmentSharingScope [environments]
- POST /workspaces/{workspaceId}/environments/{environmentId}/destroy -> destroyWorkspaceEnvironment [environments]
- GET /needs-you -> listNeedsYou [needs-you]
- GET /user-requests/{requestId} -> getUserRequest [conversations]
- POST /user-requests/{requestId}/dismiss -> dismissUserRequest [conversations]
- GET /notifications -> listNotifications [notifications]
- GET /notification-preferences -> listNotificationPreferences [notifications]
- PUT /notification-preferences/{eventClass} -> setNotificationPreference [notifications]
- GET /skills -> listSkills [capabilities]
- POST /skills -> createSkill [capabilities]
- GET /skills/{skillId} -> getSkill [capabilities]
- PUT /skills/{skillId} -> reviseSkill [capabilities]
- POST /mcp-apps/frames -> issueMcpAppFrame [capabilities]
- POST /mcp-apps/messages -> validateMcpAppMessage [capabilities]
- GET /skill-proposals -> listSkillProposals [capabilities]
- POST /skill-proposals -> createSkillProposal [capabilities]
- POST /skill-proposals/{proposalId}/approve -> approveSkillProposal [capabilities]
- POST /skill-proposals/{proposalId}/reject -> rejectSkillProposal [capabilities]
- GET /tasks/{taskId}/invocations -> listTaskCapabilityInvocations [tasks]
- GET /capability-invocations/{invocationId} -> getCapabilityInvocation [capabilities]
- GET /tasks/{taskId}/usage -> listTaskUsage [tasks]
- GET /resources/{resourceId}/dependents -> listResourceDependents [resources]
- GET /dependency-edges/{dependencyEdgeId}/invalidations -> listDependencyInvalidations [resources]
- GET /tasks/{taskId}/verification-runs -> listTaskVerificationRuns [tasks]