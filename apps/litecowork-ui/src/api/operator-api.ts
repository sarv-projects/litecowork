type JsonValue = null | boolean | number | string | JsonValue[] | {
    [key: string]: JsonValue;
};
export type paths = {
    "/agent-installations": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listLocalAgentInstallations"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/agent-registry": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listAgentRegistry"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/agent-registry/refresh": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["refreshAgentRegistry"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/agent-registry/{registryAgentId}/install": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["installRegistryAgent"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/agent-registry/{registryAgentId}/update": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["updateRegistryAgent"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/operator/readiness": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getLocalOperatorReadiness"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listWorkspaces"];
        put?: never;
        post: operations["createWorkspace"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{workspaceId}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getWorkspace"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch: operations["updateWorkspacePolicy"];
        trace?: never;
    };
    "/workspaces/{workspaceId}/backups": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listWorkspaceBackups"];
        put?: never;
        post: operations["createWorkspaceBackup"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{workspaceId}/backups/{backupId}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getWorkspaceBackup"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/recovery/backups/{backupId}/restore": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["restoreWorkspaceBackup"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{workspaceId}/default-agent-binding": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch: operations["setWorkspaceDefaultAgentBinding"];
        trace?: never;
    };
    "/workspaces/{workspaceId}/archive": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** @description Owner-authenticated terminal transition. Requires the selected Workspace path/header to match, current If-Match, and an idempotency key. Returns WORKSPACE_NOT_QUIESCENT with safe blocker kind/count entries while durable work or authority remains. A persisted AutomationCursor is active trigger-host ownership even when its Automation is disabled; channel hosts also block until a durable drain acknowledgement is supported. This operation does not stop providers or destroy Environments, roots, Resources, or files. */
        post: operations["archiveWorkspace"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{workspaceId}/runtime-bindings": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** @description Returns the selected Workspace owner's retained PENDING, ACTIVE, and REVOKED RuntimeWorkspaceBinding records in ascending binary runtime_workspace_binding_id order. The opaque keyset cursor is bound to the Workspace and page size; page size defaults to 50 and is capped at 200. This control-plane inventory does not imply current Runtime presence or readiness. */
        get: operations["listWorkspaceRuntimeBindings"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{workspaceId}/runtime-bindings/current-local": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getCurrentLocalRuntimeWorkspaceBinding"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{workspaceId}/runtime-bindings/local-enrollment": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["enrollLocalRuntimeInWorkspace"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{workspaceId}/runtime-bindings/{bindingId}/revoke": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["revokeWorkspaceRuntimeBinding"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/conversations": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listConversations"];
        put?: never;
        post: operations["createConversation"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/conversations/{conversationId}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getConversation"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/conversations/{conversationId}/presentation": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getConversationPresentation"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/rich-presentations/{presentationId}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getRichPresentation"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/conversations/{conversationId}/messages": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listConversationMessages"];
        put?: never;
        post: operations["appendConversationMessage"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/conversations/{conversationId}/tasks": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listConversationTasks"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/conversations/{conversationId}/turns": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["submitConversationTurn"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/conversations/{conversationId}/turns/{turnId}/retry": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["retryConversationTurn"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/conversations/{conversationId}/turns/{turnId}/cancel": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["cancelConversationTurn"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/tasks": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listTasks"];
        put?: never;
        post: operations["createTask"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/tasks/{taskId}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getTask"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/tasks/{taskId}/planning-readiness": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** @description Read-only local preflight projection. It does not start planning, create a session/plan/Step/Attempt/lease, invoke a provider, or mutate Task state. The response never contains a planning packet, objective text, provider handles, or endpoint identifiers. */
        get: operations["getTaskPlanningReadiness"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/tasks/{taskId}/presentation": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getTaskPresentation"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/tasks/{taskId}/progress": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getTaskProgress"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/tasks/{taskId}/steps/{stepId}/execution-dependencies": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getStepExecutionDependencies"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/tasks/{taskId}/steer": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["steerTask"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/tasks/{taskId}/lead-agent": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["changeTaskLeadAgent"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/tasks/{taskId}/cancel": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["cancelTask"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/tasks/{taskId}/pause": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["pauseTask"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/tasks/{taskId}/resume": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["resumeTask"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/tasks/{taskId}/steps/{stepId}/recover": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["recoverTaskStep"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/tasks/{taskId}/spec-revisions": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listTaskSpecRevisions"];
        put?: never;
        post: operations["reviseTaskSpec"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/tasks/{taskId}/plan-revisions": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listPlanRevisions"];
        put?: never;
        /** @description Authenticated plan submission. In addition to the authenticated Workspace owner context, the bearer identity must carry the currently authorized producer scope: either the current lead's active TASK_PLANNING AgentSession or the current lead execution Attempt's active AgentSession and valid lease. Producer IDs are derived from trusted authentication context; they are not request-body authority. If-Match pins the Task aggregate version and task_spec_revision pins the current TaskSpec. An identical idempotent retry replays the original committed PlanAcceptance after authentication/Workspace authorization and receipt-subject matching; it does not re-run mutable producer admission after the producer session has closed. */
        post: operations["submitTaskPlan"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/tasks/{taskId}/steps": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listTaskSteps"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/tasks/{taskId}/attempts": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listTaskAttempts"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/tasks/{taskId}/timeline": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getTaskTimeline"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/tasks/{taskId}/artifacts": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listTaskArtifacts"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/tasks/{taskId}/effects": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listTaskEffects"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/approvals": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listApprovals"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/approvals/{approvalId}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getApproval"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/approvals/{approvalId}/resolve": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["resolveApproval"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/artifacts": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listArtifacts"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/artifacts/{artifactId}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getArtifact"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/artifacts/{artifactId}/edit-head": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getArtifactTextEditHead"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/artifacts/{artifactId}/text-version": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["appendArtifactTextVersion"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/artifacts/{artifactId}/versions/{version}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getArtifactVersion"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/artifacts/{artifactId}/versions/{version}/content": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["downloadArtifactVersion"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/artifacts/{artifactId}/promote": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** @description Owner-authenticated TRANSIENT to SAVED metadata transition. Empty body; If-Match names the Artifact aggregate version. An identical Idempotency-Key replay returns the original committed Artifact. Linked provider content is not fetched or mutated. */
        post: operations["promoteArtifact"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/artifacts/{artifactId}/archive": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** @description Owner-authenticated SAVED to ARCHIVED metadata transition. Empty body; stale If-Match conflicts even if already archived. A fresh current-version already-archived command returns its representation without an event/version increment. Identical replay returns its original receipt. Content history and linked provider objects remain unchanged. */
        post: operations["archiveArtifact"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/library": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listLibrary"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/routines": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listRoutines"];
        put?: never;
        post: operations["createRoutine"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/routines/{routineId}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getRoutine"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/routines/{routineId}/revisions": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listRoutineRevisions"];
        put?: never;
        post: operations["reviseRoutine"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/routines/{routineId}/health": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getRoutineHealth"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/routines/{routineId}/run": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["runRoutine"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/routines/{routineId}/archive": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["archiveRoutine"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/automations": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listAutomations"];
        put?: never;
        post: operations["createAutomation"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/automations/{automationId}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getAutomation"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch: operations["updateAutomation"];
        trace?: never;
    };
    "/automations/{automationId}/run": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["runAutomationNow"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/automations/{automationId}/revisions": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listAutomationRevisions"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/automations/{automationId}/occurrences": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listAutomationOccurrences"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/automations/{automationId}/pause": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["pauseAutomation"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/automations/{automationId}/resume": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["resumeAutomation"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/automations/{automationId}/disable": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["disableAutomation"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/runtime-lifecycle": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getLocalRuntimeLifecycle"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/runtime-lifecycle/startup-policy": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch: operations["setLocalRuntimeStartupPolicy"];
        trace?: never;
    };
    "/runtime-lifecycle/stop-preview": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["previewLocalRuntimeStop"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/runtime-lifecycle/stop": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["requestLocalRuntimeStop"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/runtimes": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listRuntimes"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/runtimes/pair-token": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["createRuntimePairToken"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/runtimes/{runtimeId}/revoke": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["revokeRuntime"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/runtimes/{runtimeId}/incarnations": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listRuntimeIncarnations"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/runtimes/{runtimeId}/offers": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getRuntimeOffers"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/runtimes/{runtimeId}/capability-hosts": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** @description Workspace-owner and active EXECUTOR binding scoped page for the Runtime's exact current incarnation. Ordered by host_instance_id; expired or future-dated observations retain state but return health UNKNOWN. Activation counts include only the selected Workspace. Provider-private references are omitted. */
        get: operations["listRuntimeCapabilityHosts"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/agent-profiles": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listAgentProfiles"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/agent-profiles/probe": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * @deprecated
         * @description Transitional source-only Codex/OpenCode probe. Target clients use registry/lifecycle/profile refresh operations below.
         */
        post: operations["probeLocalAgentProfile"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/agent-profiles/{agentProfileId}/refresh": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["refreshAgentProfileControl"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/agent-profiles/{agentProfileId}/control-descriptor": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getAgentControlDescriptor"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/agent-profiles/{agentProfileId}/auth/{authMethodId}/begin": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["beginAgentAuthentication"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/agent-profiles/{agentProfileId}/credentials/{credentialSlotId}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["submitAgentCredential"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/agent-profiles/{agentProfileId}/native-config/{targetId}/open": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["openAgentNativeConfiguration"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/agent-bindings": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listAgentBindings"];
        put?: never;
        post: operations["createAgentBinding"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/agent-bindings/{agentBindingId}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getAgentBinding"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/agent-bindings/{agentBindingId}/configuration": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put: operations["configureAgentBinding"];
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/agent-bindings/{agentBindingId}/enable": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["enableAgentBinding"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/agent-bindings/{agentBindingId}/disable": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["disableAgentBinding"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/agent-bindings/{agentBindingId}/session-options": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getAgentSessionOptions"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/agent-bindings/{agentBindingId}/harness-capabilities": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getAgentHarnessCapabilities"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/agent-bindings/{agentBindingId}/quota-observation": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getAgentQuotaObservation"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/delegation-profiles": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listDelegationProfiles"];
        put?: never;
        post: operations["createDelegationProfile"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/delegation-profiles/{delegationProfileId}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getDelegationProfile"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/delegation-profiles/{delegationProfileId}/duplicate": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["duplicateDelegationProfile"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/delegation-profiles/{delegationProfileId}/performance": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getDelegationProfilePerformance"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/delegation-profiles/{delegationProfileId}/revisions": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listDelegationProfileRevisions"];
        put?: never;
        post: operations["reviseDelegationProfile"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/delegation-profiles/{delegationProfileId}/status": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["changeDelegationProfileStatus"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/coworkers": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listCoworkers"];
        put?: never;
        post: operations["createCoworker"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/coworkers/{coworkerId}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getCoworker"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/coworkers/{coworkerId}/revisions/{revision}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getCoworkerRevision"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/coworkers/{coworkerId}/presence": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getCoworkerPresence"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/coworkers/{coworkerId}/revisions": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["reviseCoworker"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/coworkers/{coworkerId}/status": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["changeCoworkerStatus"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{workspaceId}/primary-coworker": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["setPrimaryCoworker"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/goals": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listGoals"];
        put?: never;
        post: operations["createGoal"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/goals/{goalId}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getGoal"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/goals/{goalId}/revisions": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["reviseGoal"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/goals/{goalId}/status": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["changeGoalStatus"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/suggestions": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listSuggestions"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/suggestions/{suggestionId}/resolve": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["resolveSuggestion"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/suggestions/{suggestionId}/accept-task": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** @description Atomically commits a normal READY Task from the exact pinned proposal and resolves the Suggestion with result_task_id. It does not start planning, an AgentSession, or execution. The bounded response receipt proves the Suggestion-to-Task link. Repeating the same idempotency request returns the original committed link. */
        post: operations["acceptSuggestionTask"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/suggestions/{suggestionId}/snooze": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["snoozeSuggestion"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{workspaceId}/suggestion-preferences": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listSuggestionPreferences"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{workspaceId}/suggestion-preferences/{kind}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put: operations["setSuggestionPreference"];
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/demonstrations": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["startDemonstration"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/demonstrations/{demonstrationId}/complete": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["completeDemonstration"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/demonstrations/{demonstrationId}/pause": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["pauseDemonstration"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/demonstrations/{demonstrationId}/resume": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["resumeDemonstration"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/demonstrations/{demonstrationId}/abort": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["abortDemonstration"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/connections": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listConnections"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/connections/{connectionId}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getConnection"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/connections/{connectionId}/disconnect": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["disconnectConnection"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/channel-bindings": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listChannelBindings"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/channel-bindings/{channelBindingId}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getChannelBinding"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch: operations["updateChannelBindingActions"];
        trace?: never;
    };
    "/channel-bindings/{channelBindingId}/revoke": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["revokeChannelBinding"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/channel-bindings/{channelBindingId}/host-assignment": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["reassignChannelHost"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/discover/search": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["searchDiscover"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/discover/items/{itemId}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["describeDiscoverItem"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/capabilities/{capabilityId}/connect-or-install": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["connectOrInstallCapability"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/capabilities/{capabilityId}/test": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["testCapabilityConnection"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/capabilities/zip-intake": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** @description Read-only local readiness observation. It never reads ZIP bytes or invokes the parser. Current status is UNAVAILABLE until isolated worker qualification. */
        get: operations["getZipIntakeReadiness"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/resources": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** @description Keyset-paginated metadata-only catalog for the selected Workspace. Cursor ordering is descending by created_at then resource_id; content bytes are never returned. */
        get: operations["listLocalResources"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/resources/quick-import": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** @description Local desktop first-slice import, limited to 10 MiB. Uses encrypted local blob storage and commits the Resource event/revision atomically. Folder roots, ZIP extraction, resumable sessions, indexing and download use separate contracts. */
        post: operations["quickImportResource"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/resources/{resourceId}/content": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** @description Returns verified bytes for the exact selected immutable ResourceRevision. Optional revision_id selects a historical revision in this exact Workspace/Resource and never falls back to current head; omission selects current head. Optional max_bytes must be positive and no greater than 10 MiB and is checked against revision metadata before BlobStore access. Historical bytes require a local managed encrypted BlobStore location; unsupported external content returns RESOURCE_CONTENT_EXTERNAL. Storage rejects reads of REVOKED, DELETION_PENDING, or DELETED ContextDocuments before BlobStore access; a read admitted while ACTIVE may finish after a concurrent status change. Derived-state commits must recheck status. The local desktop requests max_bytes=1 MiB for inert text previews; Resource revision comparison further limits both selections to UTF-8 text/plain or Markdown. Active content is never interpreted or rendered. This is not a general large-file download path. */
        get: operations["readLocalResourceContent"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/resources/uploads": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["createResourceUpload"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/resources/uploads/{uploadId}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getResourceUpload"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/resources/uploads/{uploadId}/chunks/{chunkIndex}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        /** @description Content-Range uses inclusive HTTP byte offsets. For fixed chunk_size_bytes, chunkIndex n covers [n*chunk_size_bytes, min((n+1)*chunk_size_bytes, size)-1]. Overlap or a different replay for an accepted chunk index is rejected. */
        put: operations["uploadResourceChunk"];
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/resources/uploads/{uploadId}/commit": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["commitResourceUpload"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/resources/{resourceId}/revision-uploads": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["createResourceRevisionUpload"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/resources/{resourceId}/context-document/status": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        /** @description Authenticated local Workspace owner may revoke or restore retained ContextDocument content. Requires current Resource If-Match and an Idempotency-Key. This operation does not purge bytes or invalidate an AgentSession that already received them. */
        patch: operations["setContextDocumentStatus"];
        trace?: never;
    };
    "/resources/{resourceId}/context-document/deletion": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getContextDocumentDeletionStatus"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/resources/search": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Search managed Workspace Resources
         * @description METADATA (default) searches display-name/media-type fields. ON_DEMAND_CONTENT scans bounded current managed Resource bytes in memory. INDEXED_CONTENT searches the revision-scoped encrypted local text index using deterministic AND token matching. Neither mode searches ZIP members, rich documents, or persistent WorkspaceRoots; results are not automatically attached to a Task.
         */
        get: operations["searchResources"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/resources/{resourceId}/text-index/rebuild": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Rebuild the current Resource's encrypted local lexical index
         * @description Owner-triggered, local-only projection maintenance. The request pins the exact Resource revision and content digest observed by the Library. The operation is bounded to 1 MiB source bytes and 20,000 normalized terms, uses ordinary Idempotency-Key replay, and returns only the outcome/reason (never source content or terms). It does not append a domain event or alter Resource/Task history. Unsupported and unsafe text inputs return NOT_INDEXABLE with a typed reason; stale pins and mismatched idempotency-key reuse return CONFLICT. A non-ACTIVE ContextDocument is rejected before BlobStore access with CONTEXT_DOCUMENT_NOT_ACTIVE. If a read was admitted while ACTIVE before a concurrent status change, the final transaction rejects the prepared index unless status remains ACTIVE.
         */
        post: operations["rebuildResourceTextIndex"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/resources/{resourceId}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getResource"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/resources/{resourceId}/locations": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** @description Returns all committed public location observations in stable location_id order. Availability and freshness describe the last stored observation and never trigger a live provider probe. Provider and locator references are excluded. */
        get: operations["listResourceLocations"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/resources/{resourceId}/revisions": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listResourceRevisions"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspace-roots": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listWorkspaceRoots"];
        put?: never;
        post: operations["addWorkspaceRoot"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspace-roots/{rootId}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch: operations["updateWorkspaceRoot"];
        trace?: never;
    };
    "/workspace-roots/{rootId}/pause": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Pause an active persistent folder scope
         * @description Requires the current WorkspaceRoot version and an idempotency key. Only ACTIVE roots can be paused. The transition preserves saved local identity bindings and the selected replication-scope preference; consumers must gate use on the committed PAUSED status. This endpoint does not start or stop a watcher or indexer.
         */
        post: operations["pauseWorkspaceRoot"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspace-roots/{rootId}/resume": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Resume a paused persistent folder scope
         * @description Requires the current WorkspaceRoot version and an idempotency key. Only PAUSED roots can be resumed while the last committed ResourceLocation is AVAILABLE. A fresh request reopens the saved private folder locator without following symlinks, compares its live directory identity with the prior authorized bindings and keyed Resource identity, and atomically refreshes the locator and identity bindings for the current READY or DEGRADED Runtime incarnation with the resume transition. The prior binding pair may belong to an earlier Runtime incarnation. Exact replay returns the committed receipt without reopening the folder. A location recorded UNAVAILABLE must first be restored by Runtime identity revalidation; a WorkspaceRoot whose status is UNAVAILABLE cannot use this action.
         */
        post: operations["resumeWorkspaceRoot"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspace-roots/{rootId}/revoke": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["revokeWorkspaceRoot"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/user-requests": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listUserRequests"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/user-requests/{requestId}/respond": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["respondToUserRequest"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/user-requests/{requestId}/external-handoff": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** @description Explicitly reveals a pending EXTERNAL_URL handoff to the authenticated Workspace owner. The source Runtime returns the confidential provider URL only for this user-initiated action; the response must carry Cache-Control: no-store and must not be logged, cached, replicated, or written to analytics. */
        post: operations["openUserRequestExternalHandoff"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/tasks/{taskId}/attempts/{attemptId}/take-control": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["takeEnvironmentControl"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/tasks/{taskId}/attempts/{attemptId}/return-control": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["returnEnvironmentControl"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{workspaceId}/instructions/revisions": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listWorkspaceInstructionRevisions"];
        put?: never;
        post: operations["createWorkspaceInstructionRevision"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{workspaceId}/environments": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listWorkspaceEnvironments"];
        put?: never;
        post: operations["createPersistentWorkspaceEnvironment"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{workspaceId}/environments/provision-preview": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["previewPersistentEnvironmentProvision"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{workspaceId}/environments/{environmentId}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getWorkspaceEnvironment"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{workspaceId}/environments/{environmentId}/suspend": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["suspendWorkspaceEnvironment"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{workspaceId}/environments/{environmentId}/resume": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["resumeWorkspaceEnvironment"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{workspaceId}/environments/{environmentId}/sharing-scope": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["changeWorkspaceEnvironmentSharingScope"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{workspaceId}/environments/{environmentId}/destroy": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["destroyWorkspaceEnvironment"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/needs-you": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listNeedsYou"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/user-requests/{requestId}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getUserRequest"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/user-requests/{requestId}/dismiss": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["dismissUserRequest"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/notifications": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listNotifications"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/notification-preferences": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listNotificationPreferences"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/notification-preferences/{eventClass}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put: operations["setNotificationPreference"];
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/skill-proposals": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listSkillProposals"];
        put?: never;
        post: operations["createSkillProposal"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/skill-proposals/{proposalId}/approve": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["approveSkillProposal"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/skill-proposals/{proposalId}/reject": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["rejectSkillProposal"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/tasks/{taskId}/invocations": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listTaskCapabilityInvocations"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/capability-invocations/{invocationId}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getCapabilityInvocation"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/tasks/{taskId}/usage": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listTaskUsage"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/resources/{resourceId}/dependents": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listResourceDependents"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/dependency-edges/{dependencyEdgeId}/invalidations": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listDependencyInvalidations"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/tasks/{taskId}/verification-runs": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listTaskVerificationRuns"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
};
export type webhooks = Record<string, never>;
export type components = {
    schemas: {
        ErrorCode: components["schemas"]["error-codes.schema"];
        ResourceLimits: {
            cpu_millis: number;
            memory_bytes: number;
            storage_bytes: number;
            max_processes?: number;
            max_lifetime_seconds?: number;
        };
        NetworkPolicy: {
            /** @enum {string} */
            mode: "NONE" | "RESTRICTED";
            allowed_domains: string[];
            max_response_bytes: number;
            max_download_bytes: number;
            /** @constant */
            deny_private_networks: true;
        };
        EnvironmentProvisionPreviewRequest: {
            name: string;
            /** @enum {string} */
            environment_class: "CONTAINER" | "VM" | "CLOUD_SANDBOX" | "REMOTE_MACHINE";
            runtime_preference: components["schemas"]["PlacementPreference"];
            source_resources: components["schemas"]["PinnedResourceRef"][];
            resource_limits: components["schemas"]["ResourceLimits"];
            network_policy: components["schemas"]["NetworkPolicy"];
            retention_policy: {
                /** Format: date-time */
                expires_at: string | null;
            };
            /**
             * @default EXCLUDED
             * @enum {string}
             */
            backup_policy: "EXCLUDED" | "INCLUDE_CHECKPOINTS";
            budget_ceiling: components["schemas"]["BudgetSpec"] & ({
                max_wall_time_ms: number;
            } | {
                max_cost_minor_units: number;
                currency: string;
            });
            /** @enum {string} */
            budget_enforcement_policy: "REQUIRE_PROVIDER_ENFORCED" | "ALLOW_HOST_MONITORED";
            /** @enum {string} */
            sharing_scope: "COWORKER_PRIVATE" | "WORKSPACE_SHARED";
            /** @description Required for COWORKER_PRIVATE; null for WORKSPACE_SHARED. */
            owner_coworker_id: string | null;
        } & unknown;
        CreatePersistentEnvironmentRequest: {
            name: string;
            /** @enum {string} */
            environment_class: "CONTAINER" | "VM" | "CLOUD_SANDBOX" | "REMOTE_MACHINE";
            runtime_preference: components["schemas"]["PlacementPreference"];
            source_resources: components["schemas"]["PinnedResourceRef"][];
            resource_limits: components["schemas"]["ResourceLimits"];
            network_policy: components["schemas"]["NetworkPolicy"];
            retention_policy: {
                /** Format: date-time */
                expires_at: string | null;
            };
            /**
             * @default EXCLUDED
             * @enum {string}
             */
            backup_policy: "EXCLUDED" | "INCLUDE_CHECKPOINTS";
            budget_ceiling: components["schemas"]["BudgetSpec"] & ({
                max_wall_time_ms: number;
            } | {
                max_cost_minor_units: number;
                currency: string;
            });
            /** @enum {string} */
            budget_enforcement_policy: "REQUIRE_PROVIDER_ENFORCED" | "ALLOW_HOST_MONITORED";
            /** @enum {string} */
            sharing_scope: "COWORKER_PRIVATE" | "WORKSPACE_SHARED";
            /** @description Required for COWORKER_PRIVATE; null for WORKSPACE_SHARED. */
            owner_coworker_id: string | null;
            preview_digest: string;
        } & unknown;
        DestroyEnvironmentRequest: {
            /** @constant */
            confirm_destroy: true;
        };
        ChangeEnvironmentSharingScopeRequest: {
            /** @enum {string} */
            sharing_scope: "COWORKER_PRIVATE" | "WORKSPACE_SHARED";
            owner_coworker_id: string | null;
            /** @constant */
            confirm_no_active_use: true;
        } & unknown;
        EnvironmentView: {
            environment_id: string;
            workspace_id: string;
            name: string;
            runtime_id: string;
            /** @enum {string} */
            environment_class: "LOCAL_WORKSPACE" | "GIT_WORKTREE" | "CONTAINER" | "VM" | "CLOUD_SANDBOX" | "REMOTE_MACHINE" | "BROWSER" | "DESKTOP";
            /** @constant */
            lifetime: "WORKSPACE_PERSISTENT";
            /**
             * @description USER_SHARED is reserved and cannot be provisioned or admitted in v1.
             * @enum {string}
             */
            sharing_scope: "ATTEMPT_PRIVATE" | "TASK_SHARED" | "COWORKER_PRIVATE" | "WORKSPACE_SHARED" | "USER_SHARED";
            owner_task_id: string | null;
            owner_attempt_id: string | null;
            owner_coworker_id: string | null;
            owner_principal_id: string | null;
            /** @enum {string} */
            status: "NEW" | "PROVISIONING" | "READY" | "BUSY" | "CHECKPOINTING" | "SUSPENDED" | "FAILED" | "DESTROYING" | "DESTROYED";
            /** @enum {string} */
            health: "HEALTHY" | "DEGRADED" | "UNHEALTHY" | "UNKNOWN";
            source_resources: components["schemas"]["PinnedResourceRef"][];
            resource_limits: components["schemas"]["ResourceLimits"];
            network_policy: components["schemas"]["NetworkPolicy"];
            active_attempt_count: number;
            retention_policy: {
                /** Format: date-time */
                expires_at: string | null;
            };
            /** @enum {string} */
            backup_policy: "EXCLUDED" | "INCLUDE_CHECKPOINTS";
            budget_ceiling: components["schemas"]["BudgetSpec"];
            /** @enum {string} */
            budget_enforcement_policy: "REQUIRE_PROVIDER_ENFORCED" | "ALLOW_HOST_MONITORED";
            /** @enum {string} */
            budget_enforcement: "PROVIDER_ENFORCED" | "HOST_MONITORED" | "UNAVAILABLE";
            budget_usage: components["schemas"]["EnvironmentBudgetUsage"];
            provision_preview_digest: string;
            version: number;
            /** Format: date-time */
            created_at: string;
            /** Format: date-time */
            updated_at: string;
            blockers: components["schemas"]["Blocker"][];
        };
        EnvironmentProvisionPreview: {
            workspace_id: string;
            /** @enum {string} */
            environment_class: "CONTAINER" | "VM" | "CLOUD_SANDBOX" | "REMOTE_MACHINE";
            eligible_runtime_ids: string[];
            selected_runtime_id: string | null;
            dependencies: components["schemas"]["Blocker"][];
            estimated_cost: {
                amount_minor_units: number | null;
                currency: string | null;
                /** @enum {string} */
                confidence: "QUOTED" | "ESTIMATED" | "UNAVAILABLE";
                period_seconds: number | null;
            };
            /** @enum {string} */
            budget_enforcement: "PROVIDER_ENFORCED" | "HOST_MONITORED" | "UNAVAILABLE";
            /** @description Opaque */
            preview_digest: string;
            /** Format: date-time */
            expires_at: string;
            blockers: components["schemas"]["Blocker"][];
        };
        EnvironmentPage: {
            items: components["schemas"]["EnvironmentView"][];
            next_cursor?: string | null;
        };
        NeedsYouItem: {
            item_id: string;
            /** @enum {string} */
            kind: "APPROVAL" | "USER_REQUEST" | "TASK_BLOCKER";
            /** @enum {string} */
            status: "OPEN" | "RESOLVED";
            approval_id: string | null;
            user_request_id: string | null;
            task_id: string | null;
            blocker_id: string | null;
            title: string;
            safe_summary: string;
            /** Format: date-time */
            created_at: string;
            /** Format: date-time */
            resolved_at: string | null;
        } & (unknown & unknown & unknown);
        NeedsYouPage: {
            items: components["schemas"]["NeedsYouItem"][];
            next_cursor?: string | null;
        };
        CreateWorkspaceRequest: {
            name: string;
            /**
             * @description Defaults to LOCAL_ONLY. SELECTED_FOLDERS is set after WorkspaceRoots have been created.
             * @default LOCAL_ONLY
             * @enum {string}
             */
            replication_policy: "LOCAL_ONLY" | "METADATA_ONLY" | "ACTIVE_TASK_INPUTS" | "FULL_WORKSPACE";
        };
        JsonValue: JsonValue;
        /** @description Bounded inline JSON Schema 2020-12 schema; remote references are forbidden. */
        JsonSchema: {
            [key: string]: unknown;
        };
        Choice: {
            choice_id: string;
            label: string;
            description?: string;
            value: components["schemas"]["JsonValue"];
        };
        QuietHours: {
            /** @description IANA time zone identifier */
            timezone: string;
            weekdays: ("MONDAY" | "TUESDAY" | "WEDNESDAY" | "THURSDAY" | "FRIDAY" | "SATURDAY" | "SUNDAY")[];
            start_local_time: string;
            end_local_time: string;
        };
        UpdateWorkspacePolicyRequest: {
            /** @enum {string} */
            replication_policy: "LOCAL_ONLY" | "METADATA_ONLY" | "ACTIVE_TASK_INPUTS" | "SELECTED_FOLDERS" | "FULL_WORKSPACE";
            /** @description Required and non-empty when replication_policy is SELECTED_FOLDERS; every ID must be an active root in this Workspace. */
            replication_scope_root_ids?: string[];
        };
        SetWorkspaceDefaultAgentBindingRequest: {
            /** @description Null clears the default binding. */
            agent_binding_id: string | null;
        };
        Workspace: {
            workspace_id: string;
            name: string;
            owner_principal_id: string;
            /** @enum {string} */
            replication_policy: "LOCAL_ONLY" | "METADATA_ONLY" | "ACTIVE_TASK_INPUTS" | "SELECTED_FOLDERS" | "FULL_WORKSPACE";
            replication_scope_root_ids: string[];
            current_instruction_revision: number | null;
            default_agent_binding_id: string | null;
            primary_coworker_id: string | null;
            hub_runtime_id: string | null;
            /** @enum {string} */
            status: "ACTIVE" | "ARCHIVED";
            version: number;
            /** Format: date-time */
            created_at: string;
            /** Format: date-time */
            updated_at: string;
        };
        WorkspacePage: {
            items: components["schemas"]["Workspace"][];
            next_cursor?: string | null;
        };
        RuntimeEventCursor: {
            origin_runtime_id: string;
            last_sequence: number;
        };
        WorkspaceBackupManifest: {
            backup_id: string;
            workspace_id: string;
            schema_version: number;
            event_cursors: components["schemas"]["RuntimeEventCursor"][];
            database_snapshot_ref: components["schemas"]["BlobRef"];
            blob_manifest_ref: components["schemas"]["BlobRef"];
            blob_manifest_digest: string;
            readonly encryption_key_ref: string;
            integrity_digest: string;
            /** @description Provider-generated authentication value; contains no key material */
            readonly manifest_authentication: string;
            /** Format: date-time */
            created_at: string;
            /** Format: date-time */
            verified_at: string;
        };
        WorkspaceBackupPage: {
            items: components["schemas"]["WorkspaceBackupManifest"][];
            next_cursor?: string | null;
        };
        RestoreWorkspaceBackupRequest: {
            /** @constant */
            confirm_empty_installation: true;
        };
        RestoreReceipt: {
            backup_id: string;
            workspace_id: string;
            new_runtime_id: string;
            restored_event_cursors: components["schemas"]["RuntimeEventCursor"][];
            reauthentication_required_connection_ids: string[];
            /** Format: date-time */
            restored_at: string;
        };
        Connection: {
            connection_id: string;
            workspace_id: string;
            external_provider_ref: string;
            account_ref?: string | null;
            /** @enum {string} */
            status: "CONNECTING" | "CONNECTED" | "DEGRADED" | "REAUTH_REQUIRED" | "DISCONNECTED";
            /** Format: date-time */
            created_at: string;
            /** Format: date-time */
            updated_at: string;
            version: number;
        };
        ConnectionPage: {
            items: components["schemas"]["Connection"][];
            next_cursor?: string | null;
        };
        /** @description ACTIVE bindings always have a current host assignment. A DEGRADED binding may be safely unassigned (host_assignment null), including after RuntimeWorkspaceBinding revocation found no eligible replacement; this is not deletion of assignment history. REVOKED bindings may also have no current assignment. */
        ChannelBinding: {
            channel_binding_id: string;
            workspace_id: string;
            connection_id: string | null;
            provider_ref: string;
            external_account_ref: string;
            identity_ref: components["schemas"]["PrincipalRef"];
            /** @enum {string} */
            assurance_level: "VIEW_ONLY" | "STEER_SAFE" | "APPROVE_SAFE" | "APPROVE_SENSITIVE" | "LOCAL_STRONG";
            allowed_actions: ("VIEW" | "STEER" | "RESPOND" | "APPROVE_SAFE" | "APPROVE_SENSITIVE")[];
            /** @enum {string} */
            status: "ACTIVE" | "DEGRADED" | "REVOKED";
            /** @description Null only when the binding currently has no assignment; historical assignment changes remain available from the event history. */
            host_assignment: components["schemas"]["ChannelHostAssignment"] | null;
            /** Format: date-time */
            created_at: string;
            /** Format: date-time */
            updated_at: string;
            version: number;
        } & unknown;
        /** @description Current assignment projection. Optional provenance fields are additive for older clients; an absent value on legacy records means provenance is unavailable, not that a decision or proof is inferred. For current epoch N, source-release projection joins the immutable lease-release row for epoch N-1 and, for QUIESCENT, its drain_proof_id to the scoped drain-proof row. The initial assignment has no source-release provenance. */
        ChannelHostAssignment: {
            channel_binding_id: string;
            runtime_id: string;
            host_epoch: number;
            /** Format: date-time */
            lease_expires_at: string;
            /** @enum {string} */
            status: "ACTIVE" | "DRAINING";
            /** @enum {string} */
            ingress_continuity: "CONTINUOUS" | "GAP_ACCEPTED";
            /** Format: date-time */
            ingress_gap_since?: string | null;
            /**
             * @description Set only by RuntimeMesh service validation for a cross-Runtime ACTIVE reassignment. SQL release_kind QUIESCENT maps to SAFE_SETTLEMENT_PROOF; EXPIRY_PLUS_SKEW maps to LEASE_EXPIRY_PLUS_SKEW after Hub-authoritative safe_reassign_after. The assignment schema cannot compare source and target Runtime IDs, so the service must compare them against the current assignment.
             * @enum {string|null}
             */
            source_release_basis?: "SAFE_SETTLEMENT_PROOF" | "LEASE_EXPIRY_PLUS_SKEW" | null;
            /** @description Opaque scoped reference to the immutable SQL ChannelHostDrainProof row, present only with SAFE_SETTLEMENT_PROOF. */
            source_drain_proof_ref?: components["schemas"]["ChannelHostDrainProofRef"] | null;
            /** @description SQL channel_host_drain_proofs.proof_digest for source_drain_proof_ref; null together with the reference when release relied on lease expiry plus skew. */
            source_drain_proof_digest?: string | null;
            /** @description Opaque scoped reference to the immutable SQL ChannelHostContinuityProof row, present for a verified cross-Runtime ACTIVE assignment with continuous ingress. */
            continuity_proof_ref?: components["schemas"]["ChannelHostContinuityProofRef"] | null;
            /** @description SQL channel_host_continuity_proofs.proof_digest for continuity_proof_ref; null together with the reference when the assignment has no continuity proof. */
            continuity_proof_digest?: string | null;
            /** @description Authenticated owner decision and audit provenance for a newly accepted ingress gap. Null or absent on legacy assignments means provenance is unavailable. */
            ingress_gap_decision?: components["schemas"]["ChannelIngressGapDecision"] | null;
            /** Format: date-time */
            assigned_at: string;
            version: number;
        } & (unknown & unknown & unknown & unknown & unknown & unknown & unknown & unknown);
        /** @description Assigns or reassigns this binding to the requested eligible Runtime; it does not clear an assignment. If-Match fences the ChannelBinding aggregate version, including when unassigned, while expected_assignment_version fences the assignment snapshot (null only when no current assignment exists). Safe no-target clearing is an internal RuntimeMesh operation of RuntimeWorkspaceBinding revocation, not a public assignment-delete command. */
        ReassignChannelHostRequest: {
            runtime_id: string;
            /** @description Must equal the current ChannelHostAssignment.version */
            expected_assignment_version: number | null;
            /**
             * @description Must be explicitly true to confirm a host move when provider cursor transfer/replay cannot prove continuity. The authenticated owner principal and the generated audit record are retained in ingress_gap_decision; callers cannot supply or impersonate that provenance.
             * @default false
             */
            accept_ingress_gap: boolean;
        };
        /** @description Projection of the immutable same-Workspace AuditRecord referenced by ingress_gap_decision_audit_id. RuntimeMesh service validation must require decided_by, decided_at, and reason_code to exactly match that record; SQL assignment triggers currently enforce the record ID, Workspace, ALLOW decision, action, and ChannelBinding reference. */
        ChannelIngressGapDecision: {
            decided_by: components["schemas"]["PrincipalRef"];
            /** Format: date-time */
            decided_at: string;
            audit_record_id: string;
            reason_code: string;
        };
        /** @description Opaque, non-authorizing reference to a ChannelHostDrainProof row. The service resolves proof_id and verifies every scope field against the SQL row and source assignment; this is not a LiteCowork ResourceRef or filesystem/provider locator. */
        ChannelHostDrainProofRef: {
            proof_id: string;
            channel_binding_id: string;
            workspace_id: string;
            runtime_id: string;
            host_epoch: number;
        };
        /** @description Opaque, non-authorizing reference to a ChannelHostContinuityProof row. The service resolves proof_id, verifies every scope field and proof_digest against the SQL row and prior/current assignments; this is not a ResourceRef or filesystem/provider locator. */
        ChannelHostContinuityProofRef: {
            proof_id: string;
            channel_binding_id: string;
            workspace_id: string;
            source_runtime_id: string;
            source_host_epoch: number;
            target_runtime_id: string;
            target_host_epoch: number;
        };
        ChannelBindingPage: {
            items: components["schemas"]["ChannelBinding"][];
            next_cursor?: string | null;
        };
        UpdateChannelBindingActionsRequest: {
            allowed_actions: ("VIEW" | "STEER" | "RESPOND" | "APPROVE_SAFE" | "APPROVE_SENSITIVE")[];
        };
        JsonObject: {
            [key: string]: unknown;
        };
        PlacementPreference: ("AUTO" | "LOCAL_ONLY" | "CLOUD_PREFERRED" | "CLOUD_ONLY") | {
            runtime_id: string;
        };
        SecretRef: {
            secret_ref_id: string;
            /** @description Stable non-secret SecretStore key; grants no authority without a Trust-issued SecretLease */
            provider_ref: string;
            /** @enum {string} */
            placement: "LOCAL_ONLY" | "CLOUD_AVAILABLE" | "RUNTIME_BOUND" | "EXTERNAL_AGENT_OWNED";
        };
        RetryPolicy: {
            max_attempts: number;
            initial_backoff_ms: number;
            max_backoff_ms: number;
            multiplier: number;
            jitter: boolean;
            retryable_error_codes: string[];
        };
        TaskInputBinding: {
            source_pointer: string;
            template_variable: string;
            /** @enum {string} */
            value_kind: "TEXT" | "RESOURCE_REF";
            required: boolean;
            max_bytes?: number;
        };
        AgentRegistrySnapshot: {
            source_registry: string;
            source_revision: string | null;
            /** Format: date-time */
            observed_at: string;
            /** Format: date-time */
            expires_at?: string | null;
            /** @default false */
            stale: boolean;
            items: components["schemas"]["AgentRegistryEntry"][];
        };
        AgentRegistryPage: {
            items: components["schemas"]["AgentRegistryEntry"][];
            next_cursor: string | null;
            /** Format: date-time */
            observed_at: string;
            stale: boolean;
        };
        AgentRegistryEntry: {
            registry_agent_id: string;
            display_name: string;
            description?: string | null;
            /** Format: uri */
            homepage?: string | null;
            source_registry: string;
            source_revision?: string | null;
            distributions: components["schemas"]["AgentDistribution"][];
            /** @enum {string} */
            adapter_support?: "SUPPORTED" | "NO_ADAPTER" | "UNSUPPORTED_OS";
            /** Format: date-time */
            metadata_observed_at: string;
            /** Format: date-time */
            cache_expires_at?: string | null;
        };
        AgentDistribution: {
            distribution_id: string;
            /** @enum {string} */
            kind: "NPM" | "PYPI" | "BINARY" | "HOMEBREW" | "WINGET" | "SCOOP" | "CARGO" | "MANUAL" | "OTHER";
            package_ref: string;
            version_constraint?: string | null;
            os: ("WINDOWS" | "LINUX" | "MACOS")[];
            architecture?: ("X86_64" | "AARCH64" | "OTHER")[];
        };
        AgentInstallationObservation: {
            registry_agent_id: string;
            module_id: string;
            runtime_id: string;
            runtime_incarnation_id: string;
            /** @enum {string} */
            state: "MISSING" | "INSTALLING" | "INSTALLED" | "UPDATE_AVAILABLE" | "UPDATING" | "VERSION_UNAVAILABLE" | "BROKEN";
            installed_version?: string | null;
            available_version?: string | null;
            installation_source?: string | null;
            /** Format: date-time */
            observed_at: string;
            /** Format: date-time */
            expires_at: string;
        };
        InstallAgentRequest: {
            distribution_id: string;
            expected_registry_revision: string;
        };
        UpdateAgentRequest: {
            expected_installed_version: string;
            target_version?: string | null;
        };
        AgentLifecycleOperation: {
            lifecycle_operation_id: string;
            registry_agent_id: string;
            /** @enum {string} */
            operation: "INSTALL" | "UPDATE" | "REPAIR";
            /** @enum {string} */
            state: "QUEUED" | "RUNNING" | "SUCCEEDED" | "FAILED" | "UNKNOWN";
            observed_version?: string | null;
            message?: string | null;
            /** Format: date-time */
            started_at: string;
            /** Format: date-time */
            settled_at?: string | null;
        };
        AgentAuthDescriptor: {
            /** @enum {string} */
            state: "UNKNOWN" | "NEEDS_AUTH" | "AUTHENTICATING" | "AUTHENTICATED" | "EXPIRED" | "DEGRADED" | "ERROR";
            account_label?: string | null;
            methods: components["schemas"]["AgentAuthMethodDescriptor"][];
            credential_slots: components["schemas"]["AgentCredentialSlotDescriptor"][];
        };
        AgentAuthMethodDescriptor: {
            auth_method_id: string;
            display_name: string;
            /** @enum {string} */
            kind: "NATIVE_FLOW" | "EXTERNAL_HANDOFF" | "DEVICE_CODE" | "SECRET_SLOT" | "NATIVE_CONFIG_ONLY";
            credential_slot_id?: string | null;
            requires_restart: boolean;
        };
        AgentCredentialSlotDescriptor: {
            credential_slot_id: string;
            display_name: string;
            /** @enum {string} */
            secret_kind: "API_KEY" | "TOKEN" | "PASSWORD" | "OTHER";
            /** @enum {string} */
            storage_owner: "NATIVE_AGENT" | "LITECOWORK_SECRET_STORE";
            required: boolean;
        };
        AgentCredentialBinding: {
            credential_slot_id: string;
            secret_ref: components["schemas"]["SecretRef"];
        };
        AgentCredentialSecretSubmission: {
            descriptor_digest: string;
            /** @description Confidential local-Operator-only secret payload. Must never be logged, echoed, persisted outside SecretStore, or copied into domain events. */
            secret_base64: string;
        };
        AgentAuthChallenge: {
            /** @enum {string} */
            kind: "NATIVE_FLOW_STARTED" | "EXTERNAL_HANDOFF" | "DEVICE_CODE" | "SECRET_REQUIRED" | "NATIVE_CONFIG_REQUIRED";
            auth_method_id: string;
            external_handoff?: components["schemas"]["ExternalAuthHandoff"] | null;
            device_code_display?: string | null;
            /** Format: date-time */
            expires_at?: string | null;
        };
        AgentConfigurationDescriptor: {
            schema_digest: string;
            fields: components["schemas"]["AgentConfigurationField"][];
            native_config_targets: components["schemas"]["NativeConfigTarget"][];
        };
        AgentConfigurationField: {
            field_id: string;
            display_name: string;
            /** @enum {string} */
            type: "BOOLEAN" | "ENUM" | "STRING" | "INTEGER" | "PATH" | "STRING_LIST";
            required: boolean;
            default_value?: unknown;
            constraints: components["schemas"]["JsonObject"];
            /** @constant */
            ownership: "LITECOWORK_NON_SECRET";
        };
        NativeConfigTarget: {
            target_id: string;
            display_name: string;
            /** @enum {string} */
            kind: "FILE" | "DIRECTORY" | "NATIVE_SETTINGS" | "EXTERNAL_UI";
        };
        AgentSessionOptionDescriptor: {
            descriptor_digest: string;
            options: components["schemas"]["AgentSessionOption"][];
        };
        AgentSessionOption: {
            option_id: string;
            display_name: string;
            /** @enum {string|null} */
            semantic_hint?: "MODEL" | "REASONING" | "EFFORT" | "MODE" | "OTHER" | null;
            value_schema: components["schemas"]["JsonSchema"];
            /** @enum {string} */
            mutable_scope: "LIVE_SESSION" | "NEW_SESSION" | "UNSUPPORTED";
        };
        NativeCommandDescriptor: {
            command_id: string;
            token: string;
            display_name: string;
            description?: string | null;
            argument_schema?: components["schemas"]["JsonSchema"] | null;
        };
        NativeReferenceKindDescriptor: {
            reference_kind_id: string;
            trigger: string;
            display_name: string;
            description?: string | null;
            /** @enum {string} */
            selection_mode: "NATIVE_PICKER" | "HOST_RESOURCE_PICKER" | "TEXT_COMPLETION";
        };
        AgentInputSurfaceDescriptor: {
            text: boolean;
            images: boolean;
            files: boolean;
            resources: boolean;
            slash_commands: components["schemas"]["NativeCommandDescriptor"][];
            reference_kinds: components["schemas"]["NativeReferenceKindDescriptor"][];
            supports_command_discovery: boolean;
            supports_reference_discovery: boolean;
        };
        AgentNativeSurfaceDescriptor: {
            native_config_preserved: boolean;
            native_extensions_preserved: boolean;
            native_subagents_preserved: boolean;
            summary?: string | null;
        };
        AgentControlDescriptor: {
            descriptor_digest: string;
            descriptor_api_version: number;
            registry_agent_id: string;
            module_id: string;
            module_version: string;
            harness_version?: string | null;
            protocol_version?: string | null;
            installation: components["schemas"]["AgentInstallationObservation"];
            auth: components["schemas"]["AgentAuthDescriptor"];
            configuration: components["schemas"]["AgentConfigurationDescriptor"];
            session_options?: components["schemas"]["AgentSessionOptionDescriptor"] | null;
            input_surface: components["schemas"]["AgentInputSurfaceDescriptor"];
            native_surface: components["schemas"]["AgentNativeSurfaceDescriptor"];
            capabilities: components["schemas"]["AgentCapabilities"];
            /** Format: date-time */
            observed_at: string;
            /** Format: date-time */
            expires_at: string;
        };
        NativeConfigOpenReceipt: {
            agent_profile_id: string;
            target_id: string;
            opened: boolean;
        };
        AgentCapabilities: {
            protocol: string;
            protocol_version?: string | null;
            session: {
                resume: boolean;
                steer: boolean;
                interrupt: boolean;
                cancel: boolean;
                fork: boolean;
                /** @enum {string} */
                model_switch: "LIVE" | "NEW_SESSION" | "UNSUPPORTED";
            };
            input: {
                text: boolean;
                image: boolean;
                file: boolean;
                resources: boolean;
            };
            extension: {
                mcp_stdio: boolean;
                mcp_http: boolean;
                skills: boolean;
                plugins: boolean;
                dynamic_attach: boolean;
            };
            reporting: {
                tool_calls: boolean;
                plan: boolean;
                usage: boolean;
                native_subagents: boolean;
                approvals: boolean;
            };
            environment: {
                cwd: boolean;
                extra_directories: boolean;
            };
            limits: {
                max_context_hint?: number | null;
                max_concurrent_sessions?: number | null;
            };
        };
        AgentProfile: {
            agent_profile_id: string;
            provider_key: string;
            display_name: string;
            endpoints: components["schemas"]["AgentEndpoint"][];
            /** Format: date-time */
            discovered_at: string;
        };
        ProbeLocalAgentProfileRequest: {
            /** @enum {string} */
            provider_key: "CODEX" | "OPENCODE";
        };
        AgentInstallationPage: {
            items: components["schemas"]["AgentInstallation"][];
        };
        AgentInstallation: {
            agent_id: string;
            display_name: string;
            /** @enum {string} */
            protocol_candidate: "CODEX_APP_SERVER" | "ACP";
            /** @enum {string} */
            installation: "MISSING" | "INSTALLED" | "VERSION_UNAVAILABLE";
            version: string | null;
            /** @enum {string} */
            authentication: "UNKNOWN";
            /** @enum {string} */
            session_readiness: "NOT_PROBED";
        };
        AgentEndpoint: {
            endpoint_id: string;
            agent_profile_id: string;
            /** @enum {string} */
            protocol: "ACP" | "A2A" | "SDK" | "API" | "CLI" | "TERMINAL";
            /** @enum {string} */
            topology: "LOCAL_INTERACTIVE" | "REMOTE_AGENT_SERVICE" | "VENDOR_SERVICE" | "PROCESS_ADAPTER";
            protocol_version?: string | null;
            capabilities: components["schemas"]["AgentCapabilities"];
        };
        AgentProfilePage: {
            items: components["schemas"]["AgentProfileView"][];
            next_cursor?: string | null;
        };
        AgentProfileView: components["schemas"]["AgentProfile"] & {
            observations: components["schemas"]["AgentProfileObservation"][];
        };
        AgentProfileObservation: {
            endpoint_id: string;
            runtime_id: string;
            runtime_incarnation_id: string;
            compatible: boolean;
            /** @enum {string} */
            readiness: "AVAILABLE" | "STARTABLE" | "STARTING" | "READY" | "BUSY" | "DEGRADED" | "OFFLINE" | "NEEDS_AUTH" | "UNAVAILABLE";
            /** Format: date-time */
            observed_at: string;
            /** Format: date-time */
            offer_expires_at: string;
            constraints: components["schemas"]["JsonObject"] | components["schemas"]["OpenCodeProfileProbeConstraints"];
        };
        /** @description Sanitized OpenCode profile-probe observation. At most 32 providers, 64 models per provider, and 256 models total are returned. Catalog metadata is display-only; authentication, inference entitlement, model selection, and Task admission are not established. */
        OpenCodeProfileProbeConstraints: {
            /** @constant */
            agent_id: "opencode";
            /** @constant */
            protocol: "OPENCODE_SERVER";
            protocol_version: string | null;
            /** @enum {string} */
            probe_readiness: "COMPLETE" | "PARTIAL" | "FAILED";
            /** @enum {string|null} */
            failure: "INVALID_ADMISSION" | "HOST_UNAVAILABLE" | "PROVIDER_STATUS_UNAVAILABLE" | "CATALOG_UNAVAILABLE" | "HOST_STOP_UNOBSERVED" | "UNSUPPORTED_VERSION" | null;
            /** @enum {string} */
            reported_provider_status: "OBSERVED" | "UNKNOWN" | "NOT_PROBED";
            reported_connected_provider_ids: string[];
            reported_connected_provider_ids_truncated: boolean;
            reported_provider_ids: string[];
            reported_provider_ids_truncated: boolean;
            /** @enum {string} */
            reported_model_catalog: "OBSERVED" | "UNKNOWN" | "NOT_PROBED";
            configured_providers: components["schemas"]["OpenCodeProviderOption"][];
            catalog_truncated: boolean;
            /** @constant */
            authentication_observation: "UNKNOWN";
            /** @constant */
            session_model_selection: "NOT_QUALIFIED";
            /** @constant */
            inference_access_verified: false;
            host_process_stopped: boolean;
            /** @constant */
            writer_quiescence_proven: false;
            /** @constant */
            profile_admission_supported: false;
        };
        OpenCodeProviderOption: {
            id: string;
            display_name: string | null;
            models: components["schemas"]["OpenCodeModelOption"][];
        };
        OpenCodeModelOption: {
            id: string;
            display_name: string | null;
        };
        AgentBinding: {
            agent_binding_id: string;
            workspace_id: string;
            agent_profile_id: string;
            runtime_id: string | null;
            endpoint_selection_policy: components["schemas"]["EndpointSelectionPolicy"];
            /**
             * @deprecated
             * @description Transitional single-secret field. Target credential slots use credential_bindings.
             */
            auth_ref?: components["schemas"]["SecretRef"] | null;
            configuration_descriptor_digest: string | null;
            /** @description Closed adapter-validated non-secret configuration only; raw credentials are forbidden. */
            configuration: Record<string, never>;
            credential_bindings: components["schemas"]["AgentCredentialBinding"][];
            default_session_options_descriptor_digest: string | null;
            /** @description Opaque agent-owned default session options validated against the named descriptor. */
            default_session_options: Record<string, never>;
            enabled: boolean;
            lead_eligible: boolean;
            /** Format: date-time */
            created_at: string;
            version: number;
        };
        AgentBindingPage: {
            items: components["schemas"]["AgentBinding"][];
            next_cursor?: string | null;
        };
        CreateAgentBindingRequest: {
            workspace_id: string;
            agent_profile_id: string;
            lead_eligible: boolean;
            runtime_id?: string | null;
            /**
             * @default {
             *       "mode": "AUTO_COMPATIBLE",
             *       "required_features": [],
             *       "preferred_topologies": []
             *     }
             */
            endpoint_selection_policy: components["schemas"]["EndpointSelectionPolicy"];
            configuration_descriptor_digest?: string | null;
            configuration?: Record<string, never>;
            credential_bindings?: components["schemas"]["AgentCredentialBinding"][];
            default_session_options_descriptor_digest?: string | null;
            default_session_options?: Record<string, never>;
        };
        ConfigureAgentBindingRequest: {
            descriptor_digest: string;
            /** @description Non-secret values only; service validates against current AgentConfigurationDescriptor. */
            configuration: Record<string, never>;
            credential_bindings: components["schemas"]["AgentCredentialBinding"][];
            /** @description Agent-owned option values validated against current AgentSessionOptionDescriptor. */
            default_session_options: Record<string, never>;
        };
        EndpointSelectionPolicy: {
            /** @constant */
            mode: "AUTO_COMPATIBLE";
            required_features: components["schemas"]["AgentFeature"][];
            preferred_topologies: components["schemas"]["AgentEndpointTopology"][];
        } | {
            /** @constant */
            mode: "PINNED_ENDPOINT";
            endpoint_id: string;
            required_features: components["schemas"]["AgentFeature"][];
            preferred_topologies: components["schemas"]["AgentEndpointTopology"][];
        };
        /** @enum {string} */
        AgentFeature: "session.resume" | "session.steer" | "session.interrupt" | "session.cancel" | "session.fork" | "input.text" | "input.image" | "input.file" | "input.resources" | "extension.mcp_stdio" | "extension.mcp_http" | "extension.skills" | "extension.plugins" | "extension.dynamic_attach" | "reporting.tool_calls" | "reporting.plan" | "reporting.usage" | "reporting.native_subagents" | "reporting.approvals" | "environment.cwd" | "environment.extra_directories";
        /** @enum {string} */
        AgentEndpointTopology: "LOCAL_INTERACTIVE" | "REMOTE_AGENT_SERVICE" | "VENDOR_SERVICE" | "PROCESS_ADAPTER";
        PrincipalRef: {
            principal_id: string;
            /** @enum {string} */
            kind: "USER" | "SERVICE" | "RUNTIME" | "AGENT" | "CHANNEL_IDENTITY";
        };
        ServiceRef: {
            service_id: string;
        };
        MessageContentBlock: {
            /** @constant */
            kind: "TEXT";
            text: string;
        } | {
            /** @constant */
            kind: "RESOURCE";
            resource_ref: components["schemas"]["ResourceRef"];
            display_name?: string | null;
        };
        ApprovalRequirement: {
            requirement_id: string;
            action_class: string;
            /** @enum {string} */
            required_assurance: "VIEW_ONLY" | "STEER_SAFE" | "APPROVE_SAFE" | "APPROVE_SENSITIVE" | "LOCAL_STRONG";
            mandatory: boolean;
        };
        AcceptanceCriterion: {
            criterion_id: string;
            description: string;
            subject_refs?: components["schemas"]["ResourceRef"][];
            /** @enum {string} */
            required_evidence: "REPORTED" | "OBSERVED" | "VERIFIED";
            verifier_hint?: string | null;
            mandatory: boolean;
        };
        OutputRequirement: {
            output_id: string;
            description: string;
            kind?: string | null;
            media_type?: string | null;
            destination?: components["schemas"]["ResourceRef"] | null;
            mandatory: boolean;
        };
        BudgetSpec: {
            max_wall_time_ms?: number | null;
            max_cost_minor_units?: number | null;
            currency?: string | null;
            max_tokens?: number | null;
            max_child_attempts?: number | null;
            max_concurrency?: number | null;
        };
        EnvironmentBudgetUsage: {
            /** @enum {string} */
            status: "WITHIN_LIMIT" | "LIMIT_REACHED" | "UNKNOWN";
            consumed_cost_minor_units: number | null;
            currency: string | null;
            consumed_wall_time_ms: number | null;
            /** @enum {string} */
            confidence: "EXACT" | "ESTIMATED" | "UNKNOWN";
            /** Format: date-time */
            observed_at: string | null;
        };
        CapabilityRequirement: {
            semantic_requirement: string;
            operation_ids: string[];
            resource_scope?: components["schemas"]["ResourceScope"];
        };
        ResourceScope: {
            resource_refs?: components["schemas"]["ResourceRef"][];
            operation_ids?: string[];
            constraints?: {
                [key: string]: unknown;
            };
        };
        FailureRecord: {
            code: string;
            /** @enum {string} */
            category: "TRANSIENT" | "PERMANENT" | "POLICY" | "USER_REQUIRED" | "AMBIGUOUS";
            safe_message: string;
            retryable: boolean;
            signature: string;
            correlation_id: string;
            details?: {
                [key: string]: unknown;
            } | null;
        };
        ProvenanceRecord: {
            created_by_attempt?: string;
            provider_ref?: string;
            capability_ref?: components["schemas"]["CapabilityRef"];
            source_inputs: components["schemas"]["ResourceInput"][];
            transformations: components["schemas"]["ProvenanceTransformation"][];
            tool_reports: components["schemas"]["PinnedResourceRef"][];
            folder_import?: components["schemas"]["FolderImportMetadata"] | null;
        };
        FolderImportMetadata: {
            /** @description Slash-separated selected path; no absolute/drive/UNC form */
            relative_path: string;
        };
        ProvenanceTransformation: {
            operation: string;
            inputs: components["schemas"]["ResourceInput"][];
            output_digest?: string;
            capability_ref?: components["schemas"]["CapabilityRef"];
        };
        PlannedStep: {
            logical_key: string;
            title: string;
            objective: string;
            depends_on_logical_keys: string[];
            required_capabilities: components["schemas"]["CapabilityRequirement"][];
            acceptance_criteria: components["schemas"]["AcceptanceCriterion"][];
        };
        PlanRevision: {
            task_id: string;
            revision: number;
            task_spec_revision: number;
            produced_by_agent_session_id: string;
            produced_by_attempt_id?: string | null;
            steps: components["schemas"]["PlannedStep"][];
            reason_for_revision?: string | null;
            /** Format: date-time */
            created_at: string;
        };
        SubmitPlanRequest: {
            /** @description Must equal the current TaskSpec revision and the producer's authorized planning scope. */
            task_spec_revision: number;
            steps: components["schemas"]["PlannedStep"][];
            reason_for_revision?: string | null;
        };
        PlanAcceptance: {
            plan_revision: components["schemas"]["PlanRevision"];
            materialized_steps: components["schemas"]["Step"][];
            /** @description Task aggregate version after the acceptance transaction commits. */
            task_version: number;
        };
        Step: {
            step_id: string;
            task_id: string;
            plan_revision: number;
            logical_key?: string | null;
            title: string;
            objective: string;
            dependencies: string[];
            required_capabilities: components["schemas"]["CapabilityRequirement"][];
            acceptance_criteria: components["schemas"]["AcceptanceCriterion"][];
            /** @enum {string} */
            status: "PENDING" | "READY" | "RUNNING" | "WAITING_USER" | "BLOCKED" | "VERIFYING" | "COMPLETED" | "FAILED" | "CANCEL_REQUESTED" | "CANCELLED" | "SUPERSEDED";
            current_attempt_id?: string | null;
            /** Format: date-time */
            created_at: string;
            /** Format: date-time */
            updated_at: string;
            version: number;
        };
        Attempt: {
            attempt_id: string;
            task_id: string;
            step_id: string;
            parent_attempt_id: string | null;
            agent_binding_id: string;
            delegation_profile_id: string | null;
            delegation_profile_revision: number | null;
            agent_session_id?: string | null;
            runtime_id: string;
            runtime_incarnation_id: string;
            environment_id: string;
            capability_grant_ids: string[];
            execution_lease_id?: string | null;
            /** @enum {string} */
            failover_class: "SAFE_PORTABLE" | "REPLAYABLE" | "HANDOFF_REQUIRED" | "LOCAL_BOUND";
            checkpoint_ref?: components["schemas"]["ResourceRef"] | null;
            /** @enum {string} */
            status: "CREATED" | "PREPARING" | "RUNNING" | "WAITING_APPROVAL" | "WAITING_RESOURCE" | "CHECKPOINTING" | "COMPLETED" | "FAILED" | "ABANDONED" | "CANCEL_REQUESTED" | "CANCELLED";
            failure?: components["schemas"]["FailureRecord"] | null;
            /** Format: date-time */
            started_at?: string | null;
            /** Format: date-time */
            settled_at?: string | null;
            /** Format: date-time */
            created_at: string;
            version: number;
        } & (unknown & unknown & unknown & unknown);
        ExecutionDependencyPlan: {
            plan_id: string;
            task_id: string;
            step_id: string;
            task_version: number;
            step_version: number;
            task_spec_revision: number;
            plan_revision: number;
            selected_runtime_id: string | null;
            plan_digest: string;
            environment_candidates: components["schemas"]["EnvironmentPlacementCandidate"][];
            nodes: components["schemas"]["ExecutionDependencyNode"][];
            /** @enum {string} */
            status: "READY" | "WAITING" | "INFEASIBLE" | "STALE";
            /** Format: date-time */
            computed_at: string;
            /** Format: date-time */
            expires_at: string;
        };
        EnvironmentPlacementCandidate: {
            candidate_id: string;
            runtime_id: string;
            runtime_incarnation_id: string;
            environment_id: string;
            environment_version: number;
            /** @enum {string} */
            readiness: "AVAILABLE" | "STARTABLE" | "STARTING" | "READY" | "BUSY" | "DEGRADED" | "OFFLINE" | "NEEDS_AUTH" | "UNAVAILABLE";
            eligible: boolean;
            dependency_node_ids: string[];
            blockers: components["schemas"]["Blocker"][];
        };
        ExecutionDependencyNode: {
            node_id: string;
            /** @enum {string} */
            kind: "RESOURCE" | "AGENT_HOST" | "ENVIRONMENT" | "CAPABILITY" | "SECRET" | "APPLICATION" | "RUNTIME_ROLE";
            ref: string;
            depends_on_node_ids: string[];
            runtime_id?: string | null;
            /** @enum {string} */
            readiness: "AVAILABLE" | "STARTABLE" | "STARTING" | "READY" | "BUSY" | "DEGRADED" | "OFFLINE" | "NEEDS_AUTH" | "UNAVAILABLE";
            blocker_code?: string | null;
        };
        Effect: {
            effect_id: string;
            task_id: string;
            attempt_id: string;
            capability_ref?: components["schemas"]["CapabilityRef"] | null;
            operation: string;
            target: components["schemas"]["ResourceRef"] | string;
            idempotency_key?: string | null;
            /** @enum {string} */
            state: "PROPOSED" | "STARTED" | "ACKNOWLEDGED" | "RECONCILING" | "OBSERVED" | "VERIFIED" | "FAILED" | "AMBIGUOUS";
            request_digest: string;
            capability_invocation_id: string;
            /** @enum {string} */
            execution_method: "STRUCTURED_API" | "STRUCTURED_BROWSER" | "ACCESSIBILITY_BROWSER" | "SCREEN_COMPUTER_USE" | "DETERMINISTIC_LOCAL" | "NATIVE_AGENT_TOOL" | "UNKNOWN";
            dispatch_ordinal: number;
            result_ref?: components["schemas"]["ResourceRef"] | null;
            observed_state?: {
                [key: string]: unknown;
            } | null;
            verification_ref?: string | null;
            /** Format: date-time */
            created_at: string;
            /** Format: date-time */
            updated_at: string;
            version: number;
        };
        TimelineEntry: {
            entry_id: string;
            event_type: string;
            summary: string;
            entity_ref?: string | null;
            actor_ref?: components["schemas"]["PrincipalRef"];
            /** @enum {string|null} */
            evidence_level?: "REPORTED" | "OBSERVED" | "VERIFIED" | null;
            /** Format: date-time */
            occurred_at: string;
            correlation_id?: string | null;
        };
        CommandReceipt: {
            command_id: string;
            entity_ref: string;
            /** Format: date-time */
            accepted_at: string;
            correlation_id: string;
        };
        RuntimeIncarnation: {
            runtime_incarnation_id: string;
            runtime_id: string;
            /** Format: date-time */
            process_started_at: string;
            litecowork_version: string;
            version: number;
            recovered_from_unclean_shutdown: boolean;
            /** @enum {string} */
            recovery_state: "STARTING" | "RECOVERING" | "READY" | "DEGRADED" | "DRAINING" | "STOPPING" | "STOPPED";
            /** Format: date-time */
            ready_at?: string | null;
            /** Format: date-time */
            stopped_at?: string | null;
        };
        RuntimeIncarnationPage: {
            items: components["schemas"]["RuntimeIncarnation"][];
            next_cursor?: string | null;
        };
        RuntimeLifecycleView: {
            runtime_id: string;
            /** @enum {string} */
            startup_policy: "MANUAL" | "LOGIN_BACKGROUND" | "ALWAYS_ON_SERVICE";
            incarnation: components["schemas"]["RuntimeIncarnation"] | null;
            /** @enum {string} */
            availability: "PAIRING" | "STARTING" | "RECOVERING" | "ONLINE" | "DEGRADED" | "DRAINING" | "OFFLINE" | "REVOKED";
            blockers: components["schemas"]["Blocker"][];
            dependent_tasks?: components["schemas"]["TaskSummary"][];
            dependent_automations?: components["schemas"]["AutomationSummary"][];
        };
        LocalOperatorReadiness: {
            /** @enum {string} */
            operator_state: "SERVING";
            runtime_id: string;
            local_incarnation_id: string;
            /** @constant */
            api_contract_version: 1;
        };
        UpdateRuntimeStartupPolicyRequest: {
            /** @enum {string} */
            startup_policy: "MANUAL" | "LOGIN_BACKGROUND" | "ALWAYS_ON_SERVICE";
        };
        RuntimeStopPreview: {
            runtime_id: string;
            runtime_incarnation_id: string;
            /** Format: date-time */
            observed_at: string;
            dependent_task_ids: string[];
            dependent_automation_ids: string[];
            workspace_root_ids: string[];
            shared_service_refs: string[];
            eligible_handoff_task_ids: string[];
            blockers: components["schemas"]["Blocker"][];
        };
        StopRuntimeRequest: {
            /** @enum {string} */
            drain_choice: "CANCEL" | "HANDOFF_ELIGIBLE" | "STOP_ANYWAY";
            expected_incarnation_id: string;
        };
        RuntimeDrainReceipt: {
            runtime_id: string;
            runtime_incarnation_id: string;
            accepted: boolean;
            affected_task_ids: string[];
            affected_automation_ids: string[];
            blockers: components["schemas"]["Blocker"][];
        };
        RuntimeDescriptor: {
            runtime_id: string;
            device_identity: {
                device_id: string;
                public_key: string;
                key_version: number;
                /** Format: date-time */
                issued_at: string;
                display_name?: string | null;
            };
            runtime_version: string;
            platform: string;
            architecture: string;
            roles: ("WORKSPACE_HUB" | "EXECUTOR" | "RESOURCE_NODE" | "CHANNEL_HOST" | "TRIGGER_HOST" | "OPERATOR_ENDPOINT")[];
            trust_zone: string;
            /** @enum {string} */
            startup_policy?: "MANUAL" | "LOGIN_BACKGROUND" | "ALWAYS_ON_SERVICE";
            current_incarnation?: components["schemas"]["RuntimeIncarnation"] | null;
            /** @enum {string} */
            availability: "PAIRING" | "STARTING" | "RECOVERING" | "ONLINE" | "DEGRADED" | "DRAINING" | "OFFLINE" | "REVOKED";
            offers: components["schemas"]["RuntimeOffer"][];
            resource_capacity: {
                [key: string]: unknown;
            };
            /** Format: date-time */
            last_seen: string;
            version: number;
        };
        RuntimeWorkspaceBinding: {
            runtime_workspace_binding_id: string;
            runtime_id: string;
            workspace_id: string;
            /** @enum {string} */
            enrollment_mode: "LOCAL_ENROLLMENT" | "MESH_PAIRING";
            /** @enum {string} */
            status: "PENDING" | "ACTIVE" | "REVOKED";
            roles: ("WORKSPACE_HUB" | "EXECUTOR" | "RESOURCE_NODE" | "CHANNEL_HOST" | "TRIGGER_HOST" | "OPERATOR_ENDPOINT")[];
            /** Format: date-time */
            created_at: string;
            /** Format: date-time */
            activated_at: string | null;
            /** Format: date-time */
            revoked_at: string | null;
            version: number;
        };
        RuntimeWorkspaceBindingPage: {
            items: components["schemas"]["RuntimeWorkspaceBinding"][];
            next_cursor: string | null;
        };
        RuntimePage: {
            items: components["schemas"]["RuntimeDescriptor"][];
            next_cursor?: string | null;
        };
        RuntimeOffer: {
            runtime_id: string;
            runtime_incarnation_id: string;
            /** @enum {string} */
            offer_kind: "AGENT_ENDPOINT" | "CAPABILITY_PROVIDER" | "ENVIRONMENT_PROVIDER" | "CHANNEL_ADAPTER" | "TRIGGER_PROVIDER" | "APPLICATION";
            offer_ref: string;
            /** @enum {string} */
            readiness: "AVAILABLE" | "STARTABLE" | "STARTING" | "READY" | "BUSY" | "DEGRADED" | "OFFLINE" | "NEEDS_AUTH" | "UNAVAILABLE";
            compatible: boolean;
            constraints: {
                [key: string]: unknown;
            };
            /** Format: date-time */
            observed_at: string;
            /** Format: date-time */
            expires_at: string;
        };
        RuntimeOfferPage: {
            items: components["schemas"]["RuntimeOffer"][];
            next_cursor?: string | null;
        };
        CapabilityHostInstanceView: {
            host_instance_id: string;
            runtime_id: string;
            runtime_incarnation_id: string;
            capability_ref: components["schemas"]["CapabilityRef"];
            /** @enum {string} */
            hosting_mode: "LOCAL_MANAGED" | "REMOTE_PROVIDER";
            /** @enum {string} */
            sharing_policy: "EXCLUSIVE" | "TASK_ISOLATED" | "TRUST_PARTITION_SHARED";
            /** @enum {string} */
            state: "STARTING" | "READY" | "BUSY" | "DEGRADED" | "STOPPING" | "STOPPED" | "FAILED";
            /** @enum {string} */
            health: "HEALTHY" | "DEGRADED" | "UNHEALTHY" | "UNKNOWN";
            active_activation_count: number;
            /** Format: date-time */
            observed_at: string;
            /** Format: date-time */
            expires_at: string;
        };
        CapabilityHostInstancePage: {
            items: components["schemas"]["CapabilityHostInstanceView"][];
            next_cursor?: string | null;
        };
        CreatePairingTokenRequest: {
            workspace_id: string;
            allowed_roles: ("WORKSPACE_HUB" | "EXECUTOR" | "RESOURCE_NODE" | "CHANNEL_HOST" | "TRIGGER_HOST" | "OPERATOR_ENDPOINT")[];
            requested_ttl_ms?: number | null;
        };
        PairingToken: {
            pairing_token: string;
            workspace_id: string;
            allowed_roles: ("WORKSPACE_HUB" | "EXECUTOR" | "RESOURCE_NODE" | "CHANNEL_HOST" | "TRIGGER_HOST" | "OPERATOR_ENDPOINT")[];
            /** Format: date-time */
            expires_at: string;
        };
        CapabilityRef: {
            capability_id: string;
            /** @constant */
            identity_kind: "PACKAGE_COMPONENT";
            source: string;
            /** @description Exact opaque version label normalized from LiteSPM */
            package_version: string;
            digest: string;
            component?: string;
        } | {
            capability_id: string;
            /** @constant */
            identity_kind: "MCP_SKILL";
            /** @description Host-authenticated MCP server identity */
            source: string;
            /** @description Digest of the complete advertised Skill manifest */
            digest: string;
            /**
             * Format: uri
             * @description Exact advertised SKILL.md URI
             */
            component: string;
        };
        DiscoverItem: {
            item_ref: string;
            /** @enum {string} */
            kind: "APP" | "CAPABILITY" | "SKILL" | "AGENT";
            display_name: string;
            summary: string;
            description?: string | null;
            /** @enum {string} */
            compatibility: "COMPATIBLE" | "INCOMPATIBLE" | "UNKNOWN";
            compatibility_reason?: string | null;
            /** @default false */
            requires_user_action: boolean;
        };
        DiscoverItemPage: {
            items: components["schemas"]["DiscoverItem"][];
            next_cursor?: string | null;
        };
        CapabilitySetupReceipt: {
            operation_id: string;
            capability_id: string;
            /** @enum {string} */
            state: "ACCEPTED" | "AWAITING_APPROVAL" | "ACTIVATING" | "READY" | "FAILED";
            approval_id?: string | null;
            activation_id?: string | null;
            correlation_id: string;
        };
        CapabilityHealthResult: {
            /** @enum {string} */
            health: "HEALTHY" | "DEGRADED" | "UNHEALTHY" | "UNKNOWN";
            /** Format: date-time */
            checked_at: string;
            safe_summary?: string | null;
        };
        QuickImportResourceRequest: {
            display_name: string;
            media_type: string;
            /** @description Maximum decoded content is 10 MiB. */
            content_base64: string;
        };
        QuickImportResourceResponse: {
            resourceId: string;
            workspaceId: string;
            resourceRevisionId: string;
            displayName: string;
            mediaType: string;
            contentDigest: string;
            sizeBytes: number;
        };
        ZipIntakeReadiness: {
            /** @constant */
            capability_id: "litecowork.zip-intake";
            /** @constant */
            status: "UNAVAILABLE";
            /** @constant */
            resource_behavior: "OPAQUE_RESOURCE_ONLY";
            provider_integrated: boolean;
            extraction_enabled: boolean;
            /** @constant */
            reason_code: "ISOLATED_WORKER_NOT_QUALIFIED";
            reason: string;
        };
        ResourceSummary: {
            resourceId: string;
            workspaceId: string;
            resourceRevisionId: string;
            displayName: string;
            mediaType: string;
            contentDigest: string;
            sizeBytes: number;
            /** Format: date-time */
            createdAt: string;
        };
        ResourceSummaryPage: {
            items: components["schemas"]["ResourceSummary"][];
            nextCursor: string | null;
        };
        CreateResourceUploadRequest: {
            workspace_id: string;
            display_name: string;
            media_type: string;
            /** @description Maximum initial upload size is 100 MiB. */
            size_bytes: number;
            /** @description Required whole-content SHA-256; binds every resumable chunk to the selected file. */
            expected_digest: string;
            context_document?: components["schemas"]["ContextDocumentCreateMetadata"];
            folder_import?: components["schemas"]["FolderImportMetadata"] | null;
        };
        CreateResourceRevisionUploadRequest: {
            media_type: string;
            /** @description Maximum revision upload size is 100 MiB. */
            size_bytes: number;
            expected_digest: string;
            /** @description Must equal the Resource's complete current head set at admission and commit. */
            parent_revision_ids: string[];
        };
        SetContextDocumentStatusRequest: {
            /**
             * @description Owner may revoke or restore retained content. Purge transitions are unavailable in the local desktop V1 Operator.
             * @enum {string}
             */
            status: "ACTIVE" | "REVOKED";
        };
        ContextDocumentDeletionStatus: {
            resource_id: string;
            /** @enum {string} */
            status: "ACTIVE" | "REVOKED" | "DELETION_PENDING" | "DELETED";
            manifest_digest: string | null;
            required_replica_count: number;
            acknowledged_replica_count: number;
            pending_provider_sources: string[];
            /** Format: date-time */
            updated_at: string;
        };
        ResourceUploadSession: {
            upload_id: string;
            workspace_id: string;
            display_name: string;
            media_type: string;
            /** @description Newly created sessions are capped at 100 MiB; a migrated legacy session may report a larger original value and is not resumable. */
            expected_size_bytes: number;
            /** @description Null only for an unmigrated legacy session that cannot be resumed or committed. */
            expected_digest: string | null;
            /** @description Pinned create-time owner/kind only; status and purge fields exist on the committed Resource */
            context_document: components["schemas"]["ContextDocumentCreateMetadata"] | null;
            /** @description Pinned one-time folder attachment provenance; not a filesystem locator or grant. */
            folder_import: components["schemas"]["FolderImportMetadata"] | null;
            resource_id: string | null;
            /** @description Set only when commit atomically creates the Resource. */
            committed_resource_id: string | null;
            /** @description Pinned from If-Match for a revision upload; null for an initial Resource upload. */
            expected_resource_version: number | null;
            parent_revision_ids: string[];
            /** @description Newly created sessions use 4 MiB chunks; a migrated legacy session may report a larger original size and is not resumable. */
            chunk_size_bytes: number;
            received_ranges: {
                start_offset: number;
                end_offset_inclusive: number;
                sha256: string;
            }[];
            next_missing_offset: number;
            /** @enum {string} */
            state: "OPEN" | "CONTENT_RECEIVED" | "COMMITTED" | "FAILED" | "EXPIRED";
            /** Format: date-time */
            expires_at: string;
            /** Format: date-time */
            created_at: string;
            version: number;
            /** @description Advances for accepted chunk progress; lifecycle events use version. */
            progress_version: number;
        };
        CreateConversationRequest: {
            workspace_id: string;
            title?: string | null;
        };
        Conversation: {
            conversation_id: string;
            workspace_id: string;
            title?: string | null;
            active_agent_binding_id?: string | null;
            version: number;
            /** Format: date-time */
            created_at: string;
        };
        ConversationListPage: {
            items: components["schemas"]["Conversation"][];
            next_cursor: string | null;
        };
        AppendMessageRequest: {
            content: components["schemas"]["MessageContentBlock"][];
            resource_refs?: components["schemas"]["ResourceRef"][];
        };
        ConversationMessage: {
            message_id: string;
            conversation_id: string;
            author: components["schemas"]["PrincipalRef"];
            /** @enum {string} */
            role: "USER" | "AGENT" | "SYSTEM_NOTICE" | "CHANNEL";
            agent_session_id?: string | null;
            agent_binding_id?: string | null;
            turn_id?: string | null;
            content: components["schemas"]["MessageContentBlock"][];
            resource_refs?: components["schemas"]["ResourceRef"][];
            /** Format: date-time */
            created_at: string;
        };
        RichPresentationRef: {
            presentation_id: string;
            message_id: string;
            /** @constant */
            schema_version: 1;
            renderer_contract_version: number;
            semantic_content_digest: string;
            document_digest: string;
            document_size_bytes: number;
            /** @enum {string} */
            availability: "AVAILABLE" | "FETCHING" | "UNAVAILABLE" | "POLICY_OMITTED" | "UNSUPPORTED_VERSION" | "INTEGRITY_FAILED";
        };
        ConversationPresentationMessage: {
            message: components["schemas"]["ConversationMessage"];
            rich_presentation: components["schemas"]["RichPresentationRef"] | null;
            linked_items: components["schemas"]["PresentationItem"][];
        };
        PresentationItem: {
            item_key: string;
            /** @enum {string} */
            kind: "TEXT" | "ACTIVITY" | "TASK_CARD" | "USER_REQUEST" | "APPROVAL" | "ARTIFACT" | "CITATION" | "CODE" | "DIFF" | "TABLE" | "CHART" | "IMAGE" | "BROWSER" | "TERMINAL" | "MCP_APP" | "ERROR" | "UNSUPPORTED";
            payload_version: number;
            source_refs: {
                /** @enum {string} */
                kind: "MESSAGE" | "TASK" | "STEP" | "RESOURCE_REVISION" | "ARTIFACT_VERSION" | "USER_REQUEST" | "APPROVAL" | "INVOCATION";
                id: string;
                revision?: string;
            }[];
            /** Format: date-time */
            occurred_at?: string | null;
            order_key: string;
            /** @enum {string|null} */
            status?: "IN_PROGRESS" | "WAITING" | "NEEDS_USER" | "COMPLETE" | "INCOMPLETE" | "FAILED" | "UNAVAILABLE" | "UNKNOWN" | null;
            label?: string | null;
            /** @enum {string} */
            freshness: "CURRENT" | "STALE" | "UNKNOWN";
            payload: {
                [key: string]: components["schemas"]["JsonValue"];
            };
        };
        ConversationPresentationSnapshot: {
            workspace_id: string;
            conversation_id: string;
            projection_revision: number;
            /** @description Opaque cursor identifying this Conversation projection snapshot; not the message pagination cursor or durable event frontier. */
            stream_cursor: string;
            /** @description Durable SQLite-compatible origin event frontier as a decimal string, captured in the same read snapshot as this projection. */
            origin_sequence: string;
            items: components["schemas"]["ConversationPresentationMessage"][];
            /** @description Conversation message pagination cursor. */
            next_cursor?: string | null;
            active_turn: components["schemas"]["ConversationTurnPresentation"] | null;
        };
        ConversationTurnPresentation: {
            turn_id: string;
            /** @enum {string} */
            status: "OPEN" | "RUNNING" | "WAITING_USER" | "WAITING_DEPENDENCY" | "CANCEL_REQUESTED";
            retry_ordinal: number;
            /** @enum {string} */
            presentation_preference: "AUTO" | "SIMPLE" | "RICH";
        };
        RichPresentationDocumentResponse: {
            workspace_id: string;
            conversation_id: string;
            message_id: string;
            presentation_id: string;
            /** @constant */
            schema_version: 1;
            renderer_contract_version: number;
            semantic_content_digest: string;
            document_digest: string;
            document_size_bytes: number;
            /** @description Must validate against docs/schemas/rich-presentation.schema.json before rendering. */
            document: Record<string, never>;
        };
        MessagePage: {
            items: components["schemas"]["ConversationMessage"][];
            next_cursor?: string | null;
        };
        CreateTaskRequest: {
            workspace_id: string;
            conversation_id?: string | null;
            source_message_refs?: string[];
            objective: string;
            /** @enum {string|null} */
            task_category?: "SOFTWARE_ENGINEERING" | "RESEARCH" | "WRITING" | "DATA_ANALYSIS" | "OFFICE" | "BROWSER" | "PERSONAL_ADMIN" | "OTHER" | null;
            constraints?: string[];
            non_goals?: string[];
            input_refs?: components["schemas"]["PinnedResourceRef"][];
            required_outputs?: components["schemas"]["OutputRequirement"][];
            acceptance_criteria?: components["schemas"]["AcceptanceCriterion"][];
            approvals_required?: components["schemas"]["ApprovalRequirement"][];
            budget?: components["schemas"]["BudgetSpec"] | null;
            delegation_budget_policy?: components["schemas"]["DelegationBudgetPolicy"] | null;
            /** @description Optional on create; omission resolves Coworker default or explicit DISABLED. Null is not accepted. */
            lead_failover_policy?: components["schemas"]["LeadFailoverPolicy"];
            /** Format: date-time */
            deadline?: string | null;
            placement_preference?: components["schemas"]["PlacementPreference"];
            preferred_lead_agent_binding_id?: string | null;
            /** @description Explicit Task origin selection; if omitted */
            coworker_id?: string | null;
            /** @description Optional optimistic check for the selected Coworker head; the service pins its current revision. */
            expected_coworker_version?: number | null;
        } & unknown;
        ReviseTaskSpecRequest: {
            parent_revisions: number[];
            objective: string;
            constraints?: string[];
            non_goals?: string[];
            input_refs?: components["schemas"]["PinnedResourceRef"][];
            required_outputs?: components["schemas"]["OutputRequirement"][];
            acceptance_criteria?: components["schemas"]["AcceptanceCriterion"][];
            approvals_required?: components["schemas"]["ApprovalRequirement"][];
            placement_preference?: components["schemas"]["PlacementPreference"];
            preferred_lead_agent_binding_id?: string | null;
            /** @description Optional; omission inherits the current TaskSpec policy. */
            lead_failover_policy?: components["schemas"]["LeadFailoverPolicy"];
        };
        TaskSpecRevision: {
            task_id: string;
            revision: number;
            parent_revisions: number[];
            objective: string;
            /** @enum {string|null} */
            task_category?: "SOFTWARE_ENGINEERING" | "RESEARCH" | "WRITING" | "DATA_ANALYSIS" | "OFFICE" | "BROWSER" | "PERSONAL_ADMIN" | "OTHER" | null;
            constraints: string[];
            non_goals: string[];
            required_outputs: components["schemas"]["OutputRequirement"][];
            acceptance_criteria: components["schemas"]["AcceptanceCriterion"][];
            approvals_required: components["schemas"]["ApprovalRequirement"][];
            input_refs: components["schemas"]["PinnedResourceRef"][];
            workspace_instruction_revision?: number | null;
            budget?: components["schemas"]["BudgetSpec"] | null;
            delegation_budget_policy?: components["schemas"]["DelegationBudgetPolicy"] | null;
            /** Format: date-time */
            deadline?: string | null;
            source_message_refs?: string[];
            placement_preference: components["schemas"]["PlacementPreference"];
            preferred_lead_agent_binding_id?: string | null;
            lead_failover_policy: components["schemas"]["LeadFailoverPolicy"];
            authored_by: components["schemas"]["PrincipalRef"];
            /** Format: date-time */
            created_at: string;
        };
        Task: {
            task_id: string;
            workspace_id: string;
            conversation_id?: string | null;
            routine_id?: string | null;
            routine_revision?: number | null;
            automation_id?: string | null;
            automation_occurrence_id?: string | null;
            origin_coworker_id: string | null;
            origin_coworker_revision: number | null;
            lead_agent_binding_id: string;
            blocking_conditions: components["schemas"]["Blocker"][];
            /** @enum {string} */
            priority: "LOW" | "NORMAL" | "HIGH";
            created_by: components["schemas"]["PrincipalRef"];
            current_spec_revision: number;
            current_plan_revision?: number | null;
            /** @enum {string|null} */
            resume_status?: "READY" | "RUNNING" | "WAITING_USER" | "BLOCKED" | "VERIFYING" | null;
            /** @enum {string} */
            status: "READY" | "RUNNING" | "WAITING_USER" | "BLOCKED" | "VERIFYING" | "NEEDS_USER" | "INCOMPLETE" | "PAUSE_REQUESTED" | "PAUSED" | "COMPLETED" | "FAILED" | "CANCEL_REQUESTED" | "CANCELLED";
            version: number;
            /** Format: date-time */
            created_at: string;
            /** Format: date-time */
            updated_at: string;
            /** Format: date-time */
            completed_at?: string | null;
        } & (unknown & unknown & unknown & unknown);
        TaskView: {
            task: components["schemas"]["Task"];
            current_spec_revision: components["schemas"]["TaskSpecRevision"];
        };
        TaskPlanningReadiness: {
            task_id: string;
            task_version: number;
            task_spec_revision: number;
            /** @enum {string} */
            task_status: "READY" | "RUNNING" | "WAITING_USER" | "BLOCKED" | "VERIFYING" | "NEEDS_USER" | "INCOMPLETE" | "PAUSE_REQUESTED" | "PAUSED" | "COMPLETED" | "FAILED" | "CANCEL_REQUESTED" | "CANCELLED";
            /** Format: date-time */
            observed_at: string;
            /** @constant */
            dispatch_available: false;
            /** @constant */
            planning_started: false;
            /** @constant */
            agent_session_started: false;
            /** @constant */
            plan_created: false;
            blockers: ("TASK_STATE_NOT_ELIGIBLE" | "PLAN_ALREADY_ACCEPTED" | "CURRENT_LEAD_OR_ENDPOINT_UNAVAILABLE" | "TASK_ISOLATION_UNAVAILABLE" | "NATIVE_CAPABILITIES_UNMEDIATED" | "PROTOCOL_UNQUALIFIED" | "PROCESS_CONTAINMENT_UNQUALIFIED" | "PLANNING_CONTEXT_RESOURCE_UNAVAILABLE" | "SESSION_SETTLEMENT_UNAVAILABLE" | "PROVIDER_UNSUPPORTED")[];
        };
        TaskPresentationSnapshot: {
            workspace_id: string;
            task_id: string;
            /** Format: date-time */
            computed_at: string;
            /** @enum {string} */
            freshness: "CURRENT" | "STALE" | "UNKNOWN";
            items: components["schemas"]["TaskPresentationItem"][];
        };
        TaskPresentationItem: {
            item_key: string;
            /** @enum {string} */
            kind: "TASK_CARD" | "ACTIVITY" | "ARTIFACT";
            /** @constant */
            payload_version: 1;
            source_refs: {
                /** @enum {string} */
                kind: "TASK" | "STEP" | "ARTIFACT_VERSION" | "RESOURCE_REVISION";
                id: string;
                revision?: string;
            }[];
            /** Format: date-time */
            occurred_at?: string | null;
            order_key: string;
            /** @enum {string} */
            status: "IN_PROGRESS" | "WAITING" | "NEEDS_USER" | "COMPLETE" | "INCOMPLETE" | "FAILED" | "UNAVAILABLE" | "UNKNOWN";
            label?: string | null;
            payload: {
                task_id: string;
                objective: string;
                status: string;
                step_summary?: string;
            } | {
                summary: string;
                detail?: string;
            } | {
                artifact_id: string;
                version: number;
                display_name: string;
                artifact_kind: string;
                verification_status?: string;
            };
            /** @enum {string} */
            freshness: "CURRENT" | "STALE" | "UNKNOWN";
        };
        TaskSummary: {
            task_id: string;
            /** @enum {string} */
            status: "READY" | "RUNNING" | "WAITING_USER" | "BLOCKED" | "VERIFYING" | "NEEDS_USER" | "INCOMPLETE" | "PAUSE_REQUESTED" | "PAUSED" | "COMPLETED" | "FAILED" | "CANCEL_REQUESTED" | "CANCELLED";
            objective: string;
            /** Format: date-time */
            created_at: string;
            /** Format: date-time */
            updated_at: string;
        };
        TaskWorkstreamProjection: {
            step_id: string;
            title: string;
            /** @enum {string} */
            step_status: "PENDING" | "READY" | "RUNNING" | "WAITING_USER" | "BLOCKED" | "VERIFYING" | "COMPLETED" | "FAILED" | "CANCEL_REQUESTED" | "CANCELLED" | "SUPERSEDED";
            active_attempt_ids: string[];
            worker_labels: string[];
            /** Format: date-time */
            last_activity_at: string | null;
        };
        TaskProgressProjection: {
            task_id: string;
            /** Format: date-time */
            computed_at: string;
            /** Format: date-time */
            last_activity_at: string | null;
            /** @enum {string|null} */
            last_activity_source: "TASK_EVENT" | "STEP_EVENT" | "ATTEMPT_EVENT" | "CAPABILITY_INVOCATION" | "PROVIDER_PROGRESS" | "ENVIRONMENT_OBSERVATION" | null;
            /** Format: date-time */
            last_evidence_at: string | null;
            activity_summary: string | null;
            active_workstreams: components["schemas"]["TaskWorkstreamProjection"][];
            blockers: components["schemas"]["Blocker"][];
            newest_artifact: components["schemas"]["ArtifactVersionRef"] | null;
        };
        TaskPage: {
            items: components["schemas"]["TaskSummary"][];
            next_cursor?: string | null;
        };
        Blocker: {
            blocker_id: string;
            code: components["schemas"]["error-codes.schema"];
            safe_message: string;
            subject_ref?: components["schemas"]["ResourceRef"] | components["schemas"]["ServiceRef"] | null;
            resolution_hint: string;
            user_request_id?: string | null;
            approval_id?: string | null;
            /** Format: date-time */
            created_at: string;
        };
        SteerTaskRequest: {
            message: string;
            /** @enum {string} */
            classification?: "CHAT_ONLY" | "TASK_SPEC_CHANGE" | "PLAN_HINT" | "URGENT_INTERRUPT" | "CANCEL";
        };
        ChangeLeadAgentRequest: {
            agent_binding_id: string;
        };
        PauseTaskRequest: {
            reason?: string | null;
        };
        RecoverStepRequest: {
            expected_step_version: number;
            recovery_reason: string;
            placement_override?: {
                candidate_id: string;
                plan_digest: string;
            } | null;
        };
        SubmitConversationTurnRequest: {
            content: components["schemas"]["MessageContentBlock"][];
            resource_refs?: components["schemas"]["ResourceRef"][];
            agent_binding_id?: string | null;
            /**
             * @default AUTO
             * @enum {string}
             */
            presentation_preference: "AUTO" | "SIMPLE" | "RICH";
        };
        ConversationTurnReceipt: {
            turn_id: string;
            conversation_id: string;
            user_message_id: string;
            /** @enum {string} */
            status: "OPEN" | "RUNNING" | "WAITING_USER" | "WAITING_DEPENDENCY" | "COMPLETED" | "FAILED" | "CANCEL_REQUESTED" | "CANCELLED";
            /** Format: date-time */
            accepted_at: string;
            correlation_id: string;
            /** @enum {string} */
            presentation_preference: "AUTO" | "SIMPLE" | "RICH";
        };
        TimelinePage: {
            items: components["schemas"]["TimelineEntry"][];
            next_cursor?: string | null;
        };
        ResolveApprovalRequest: {
            /** @enum {string} */
            decision: "APPROVE" | "DENY";
            confirmation_digest?: string | null;
        };
        Approval: {
            approval_id: string;
            task_id: string;
            requested_by_attempt?: string | null;
            kind: string;
            action_summary: string;
            target_ref: components["schemas"]["ResourceRef"];
            scope_digest: string;
            action_digest: string;
            /** @enum {string} */
            risk: "SAFE" | "SENSITIVE" | "HIGH_IMPACT";
            required_assurance: string;
            /** @enum {string} */
            status: "PENDING" | "APPROVED" | "DENIED" | "EXPIRED" | "CANCELLED";
            /** Format: date-time */
            requested_at: string;
            /** Format: date-time */
            expires_at?: string | null;
            resolved_by?: components["schemas"]["PrincipalRef"] | null;
            /** Format: date-time */
            resolved_at?: string | null;
            version: number;
        };
        Artifact: {
            artifact_id: string;
            workspace_id: string;
            resource_id: string;
            task_id?: string | null;
            kind: string;
            display_name: string;
            current_version: number;
            /** @enum {string} */
            library_status: "TRANSIENT" | "SAVED" | "ARCHIVED";
            /** Format: date-time */
            created_at: string;
            version: number;
        };
        ArtifactVersion: {
            artifact_id: string;
            version: number;
            resource_revision_id: string;
            created_by_attempt?: string | null;
            input_refs: components["schemas"]["PinnedResourceRef"][];
            content: components["schemas"]["ArtifactContent"];
            provenance: components["schemas"]["ProvenanceRecord"];
            verification_refs?: string[];
            /** Format: date-time */
            created_at: string;
        };
        ArtifactVersionRef: {
            workspace_id: string;
            artifact_id: string;
            version: number;
        };
        ArtifactContent: components["schemas"]["ManagedBlobContent"] | components["schemas"]["ExternalResourceContent"];
        ManagedBlobContent: {
            /** @constant */
            kind: "MANAGED_BLOB";
            storage_ref: components["schemas"]["BlobRef"];
            content_digest: string;
            media_type: string;
            size_bytes: number;
        };
        ExternalResourceContent: {
            /** @constant */
            kind: "EXTERNAL_RESOURCE";
            resource_ref: components["schemas"]["PinnedResourceRef"];
            provider_revision?: string | null;
            observed_digest?: string | null;
            /** Format: date-time */
            observed_at: string;
        };
        ConnectCapabilityRequest: {
            workspace_id: string;
            runtime_id?: string | null;
            requested_scope?: components["schemas"]["ResourceScope"];
        };
        TestCapabilityRequest: {
            workspace_id: string;
            runtime_id?: string | null;
        };
        ApprovalPage: {
            items: components["schemas"]["Approval"][];
            next_cursor?: string | null;
        };
        AutomationPage: {
            items: components["schemas"]["Automation"][];
            next_cursor?: string | null;
        };
        AutomationRevisionPage: {
            items: components["schemas"]["AutomationRevision"][];
            next_cursor?: string | null;
        };
        AutomationSummary: {
            automation_id: string;
            name: string;
            /** @enum {string} */
            status: "ENABLED" | "PAUSED" | "DISABLED";
        };
        AutomationOccurrencePage: {
            items: components["schemas"]["AutomationOccurrence"][];
            next_cursor?: string | null;
        };
        ArtifactPage: {
            items: components["schemas"]["Artifact"][];
            next_cursor?: string | null;
        };
        ArtifactTextEditHead: {
            artifact_id: string;
            aggregate_version: number;
            content_version: number;
            resource_version: number;
            parent_resource_revision_id: string;
        };
        AppendArtifactTextVersionRequest: {
            expected_content_version: number;
            expected_resource_version: number;
            expected_parent_resource_revision_id: string;
            /** @description UTF-8 text; server enforces a 1 MiB encoded byte limit and rejects control characters except line breaks and tabs */
            content: string;
        };
        ArtifactTextVersionAppendResponse: {
            artifact: components["schemas"]["Artifact"];
            version: components["schemas"]["ArtifactVersion"];
            replayed: boolean;
        };
        Routine: {
            routine_id: string;
            workspace_id: string;
            name: string;
            current_revision: number;
            /** @enum {string} */
            status: "ACTIVE" | "ARCHIVED";
            version: number;
            /** Format: date-time */
            created_at: string;
            /** Format: date-time */
            updated_at: string;
        };
        RoutinePage: {
            items: components["schemas"]["Routine"][];
            next_cursor?: string | null;
        };
        DependencyHealth: {
            subject_ref: components["schemas"]["ResourceRef"] | components["schemas"]["CapabilityRef"];
            /** @enum {string} */
            state: "HEALTHY" | "DEGRADED" | "UNHEALTHY" | "UNKNOWN";
            /** Format: date-time */
            observed_at: string | null;
            reason_code: string | null;
        };
        RoutineHealth: {
            routine_id: string;
            current_revision: number;
            /** Format: date-time */
            last_run_at: string | null;
            /** Format: date-time */
            last_success_at: string | null;
            recent_success_rate: number | null;
            sample_size: number;
            average_duration_ms: number | null;
            observed_costs: components["schemas"]["UsageQuantity"][];
            required_dependency_health: components["schemas"]["DependencyHealth"][];
            /** @enum {string} */
            drift_state: "HEALTHY" | "WARNING" | "DRIFTED" | "UNKNOWN";
            drift_evidence_refs: string[];
        };
        RoutineRevision: {
            routine_id: string;
            revision: number;
            objective_template: string;
            instructions: string;
            input_schema: {
                [key: string]: unknown;
            };
            constraints: string[];
            non_goals: string[];
            required_outputs: components["schemas"]["OutputRequirement"][];
            acceptance_criteria: components["schemas"]["AcceptanceCriterion"][];
            approvals_required: components["schemas"]["ApprovalRequirement"][];
            input_bindings: components["schemas"]["TaskInputBinding"][];
            required_capabilities: components["schemas"]["CapabilityRequirement"][];
            preferred_agent_binding_id?: string | null;
            placement_preference: components["schemas"]["PlacementPreference"];
            budget_ceiling?: components["schemas"]["BudgetSpec"] | null;
            verification_policy: {
                [key: string]: unknown;
            };
            authored_by: components["schemas"]["PrincipalRef"];
            /** Format: date-time */
            created_at: string;
        };
        RoutineRevisionPage: {
            items: components["schemas"]["RoutineRevision"][];
            next_cursor?: string | null;
        };
        CreateRoutineRequest: {
            workspace_id: string;
            name: string;
            revision: components["schemas"]["RoutineRevisionInput"];
        };
        CreateRoutineRevisionRequest: {
            revision: components["schemas"]["RoutineRevisionInput"];
        };
        RoutineRevisionInput: {
            objective_template: string;
            instructions: string;
            input_schema: {
                [key: string]: unknown;
            };
            constraints: string[];
            non_goals: string[];
            required_outputs: components["schemas"]["OutputRequirement"][];
            acceptance_criteria: components["schemas"]["AcceptanceCriterion"][];
            approvals_required: components["schemas"]["ApprovalRequirement"][];
            input_bindings: components["schemas"]["TaskInputBinding"][];
            required_capabilities: components["schemas"]["CapabilityRequirement"][];
            preferred_agent_binding_id?: string | null;
            placement_preference: components["schemas"]["PlacementPreference"];
            budget_ceiling?: components["schemas"]["BudgetSpec"] | null;
            verification_policy: {
                [key: string]: unknown;
            };
        };
        RunRoutineRequest: {
            routine_revision: number;
            inputs: {
                [key: string]: unknown;
            };
            conversation_id?: string | null;
        };
        TriggerSpec: {
            trigger_id: string;
            /** @enum {string} */
            placement: "HUB" | "SPECIFIC_RUNTIME" | "AUTO";
            runtime_id?: string | null;
            trigger: components["schemas"]["TriggerDefinition"];
        } & unknown;
        TriggerDefinition: components["schemas"]["ScheduleTrigger"] | components["schemas"]["OneShotTrigger"] | components["schemas"]["WebhookTrigger"] | components["schemas"]["ConnectorEventTrigger"] | components["schemas"]["ChannelEventTrigger"] | components["schemas"]["ResourceEventTrigger"] | components["schemas"]["TaskEventTrigger"] | components["schemas"]["RuntimeEventTrigger"] | components["schemas"]["ProcessExitTrigger"] | components["schemas"]["CommandExitTrigger"] | components["schemas"]["ConditionWatchTrigger"] | components["schemas"]["ManualTrigger"];
        MisfirePolicy: {
            /** @constant */
            kind: "SKIP";
        } | {
            /** @constant */
            kind: "RUN_ONCE_WHEN_AVAILABLE";
        } | {
            /** @constant */
            kind: "CATCH_UP_BOUNDED";
            max_occurrences: number;
        };
        ScheduleTrigger: {
            /**
             * @description discriminator enum property added by openapi-typescript
             * @enum {string}
             */
            kind: "ScheduleTrigger";
            /** @description IANA time-zone identifier */
            timezone: string;
            /** @enum {string} */
            recurrence_format: "CRON_5" | "RFC5545_RRULE";
            recurrence_semantics_version: number;
            rrule_or_cron: string;
            /** Format: date-time */
            start_at?: string | null;
            /** Format: date-time */
            end_at?: string | null;
            misfire_policy: components["schemas"]["MisfirePolicy"];
            /** @enum {string} */
            ambiguous_local_time: "EARLIER" | "LATER";
            /** @enum {string} */
            nonexistent_local_time: "SKIP" | "NEXT_VALID";
        };
        OneShotTrigger: {
            /**
             * @description discriminator enum property added by openapi-typescript
             * @enum {string}
             */
            kind: "OneShotTrigger";
            /** Format: date-time */
            scheduled_at: string;
            misfire_policy: components["schemas"]["MisfirePolicy"];
        };
        WebhookTrigger: {
            /**
             * @description discriminator enum property added by openapi-typescript
             * @enum {string}
             */
            kind: "WebhookTrigger";
            source_identity: string;
            auth_profile_ref: string;
            /** @enum {string} */
            authentication_mode: "SIGNATURE" | "BEARER_SECRET" | "MUTUAL_TLS";
            replay_window_ms: number;
            max_payload_bytes: number;
            delivery_id_field?: string | null;
        };
        ConnectorEventTrigger: {
            /**
             * @description discriminator enum property added by openapi-typescript
             * @enum {string}
             */
            kind: "ConnectorEventTrigger";
            connection_id: string;
            provider_event_type: string;
            filter_expression?: {
                [key: string]: unknown;
            } | null;
        };
        ChannelEventTrigger: {
            /**
             * @description discriminator enum property added by openapi-typescript
             * @enum {string}
             */
            kind: "ChannelEventTrigger";
            channel_binding_id: string;
            provider_event_type: string;
            filter_expression?: {
                [key: string]: unknown;
            } | null;
        };
        ResourceEventTrigger: {
            /**
             * @description discriminator enum property added by openapi-typescript
             * @enum {string}
             */
            kind: "ResourceEventTrigger";
            workspace_root_id?: string | null;
            resource_id?: string | null;
            event_kinds: string[];
            debounce_ms: number;
        } & ({
            workspace_root_id: string;
            resource_id?: null;
        } | {
            resource_id: string;
            workspace_root_id?: null;
        });
        TaskEventTrigger: {
            /**
             * @description discriminator enum property added by openapi-typescript
             * @enum {string}
             */
            kind: "TaskEventTrigger";
            event_kinds: string[];
            task_filter?: {
                [key: string]: unknown;
            } | null;
            ignore_origin_automation_id?: string | null;
        };
        RuntimeEventTrigger: {
            /**
             * @description discriminator enum property added by openapi-typescript
             * @enum {string}
             */
            kind: "RuntimeEventTrigger";
            runtime_id?: string | null;
            event_kinds: string[];
            debounce_ms: number;
        };
        ProcessExitTrigger: {
            /**
             * @description discriminator enum property added by openapi-typescript
             * @enum {string}
             */
            kind: "ProcessExitTrigger";
            runtime_id: string;
            managed_process_ref: string;
            exit_filter?: {
                [key: string]: unknown;
            } | null;
        };
        CommandExitTrigger: {
            /**
             * @description discriminator enum property added by openapi-typescript
             * @enum {string}
             */
            kind: "CommandExitTrigger";
            runtime_id: string;
            managed_execution_ref: string;
            exit_filter?: {
                [key: string]: unknown;
            } | null;
        };
        ConditionWatchTrigger: {
            /**
             * @description discriminator enum property added by openapi-typescript
             * @enum {string}
             */
            kind: "ConditionWatchTrigger";
            provider_ref: string;
            source_ref: components["schemas"]["ResourceRef"] | components["schemas"]["CapabilityRef"];
            poll_interval_ms: number;
            condition_digest: string;
            end_condition?: {
                [key: string]: unknown;
            } | null;
        };
        ManualTrigger: {
            /**
             * @description discriminator enum property added by openapi-typescript
             * @enum {string}
             */
            kind: "ManualTrigger";
        };
        AutomationExecutionPolicy: {
            placement_preference: components["schemas"]["PlacementPreference"];
            /** @default 1 */
            max_concurrent_occurrences: number;
            /** @enum {string} */
            overlap_policy: "SKIP" | "QUEUE" | "CANCEL_OLD" | "ALLOW";
            retry_policy: components["schemas"]["RetryPolicy"];
            budget_ceiling?: components["schemas"]["BudgetSpec"] | null;
            /** @enum {string} */
            notification_policy: "ALWAYS" | "ON_SUCCESS" | "ON_FAILURE" | "ON_CONDITION" | "SILENT";
            /** @enum {string} */
            wake_policy: "NEVER" | "TRY_WAKE" | "REQUIRE_RUNTIME_AWAKE";
        };
        ManualAutomationRunRequest: {
            automation_revision: number;
            /** @description Bounded values validated against the exact pinned Routine input schema and bindings. */
            inputs: {
                [key: string]: unknown;
            };
        };
        ManualAutomationRunResponse: {
            automation_id: string;
            automation_revision: number;
            trigger_id: string;
            occurrence_id: string;
            /** @constant */
            occurrence_version: 3;
            /** @constant */
            occurrence_status: "STARTED";
            task: components["schemas"]["TaskView"];
        };
        CreateAutomationRequest: {
            workspace_id: string;
            name: string;
            routine_id: string;
            routine_revision: number;
            triggers: components["schemas"]["TriggerSpec"][];
            execution_policy: components["schemas"]["AutomationExecutionPolicy"];
            coworker_ref: components["schemas"]["CoworkerRevisionRef"] | null;
        };
        UpdateAutomationRequest: {
            name: string;
            routine_id: string;
            routine_revision: number;
            triggers: components["schemas"]["TriggerSpec"][];
            execution_policy: components["schemas"]["AutomationExecutionPolicy"];
            coworker_ref: components["schemas"]["CoworkerRevisionRef"] | null;
        };
        Automation: {
            automation_id: string;
            workspace_id: string;
            name: string;
            current_revision: number;
            /** @enum {string} */
            status: "ENABLED" | "PAUSED" | "DISABLED";
            version: number;
            /** Format: date-time */
            created_at: string;
            /** Format: date-time */
            updated_at: string;
        };
        AutomationRevision: {
            automation_id: string;
            revision: number;
            routine_id: string;
            routine_revision: number;
            triggers: components["schemas"]["TriggerSpec"][];
            execution_policy: components["schemas"]["AutomationExecutionPolicy"];
            coworker_ref: components["schemas"]["CoworkerRevisionRef"] | null;
            authored_by: components["schemas"]["PrincipalRef"];
            /** Format: date-time */
            created_at: string;
        };
        AutomationOccurrence: {
            occurrence_id: string;
            automation_id: string;
            automation_revision: number;
            routine_id: string;
            routine_revision: number;
            trigger_id: string;
            trigger_host_runtime_id: string;
            occurrence_key: string;
            /** @description Monotonic occurrence aggregate revision; event and snapshot entity_revision. */
            version: number;
            claim_epoch: number;
            /** Format: date-time */
            claim_expires_at?: string | null;
            /** Format: date-time */
            scheduled_for?: string | null;
            covered_misfire_range?: null | {
                /** Format: date-time */
                from: string;
                /** Format: date-time */
                to: string;
                count: number;
            };
            trigger_input_ref?: components["schemas"]["ResourceRef"] | null;
            trigger_payload_digest?: string | null;
            task_id?: string | null;
            blockers: components["schemas"]["Blocker"][];
            /** @enum {string} */
            status: "PENDING" | "CLAIMED" | "WAITING_DEPENDENCY" | "STARTED" | "COMPLETED" | "SKIPPED" | "FAILED";
            /** Format: date-time */
            created_at: string;
            /** Format: date-time */
            updated_at: string;
        };
        BlobRef: {
            digest: string;
            size_bytes: number;
            media_type: string;
        };
        ContextOwnerRef: {
            /** @constant */
            kind: "USER";
            principal_id: string;
        } | {
            /** @constant */
            kind: "WORKSPACE";
            workspace_id: string;
        } | {
            /** @constant */
            kind: "COWORKER";
            workspace_id: string;
            coworker_id: string;
        } | {
            /** @constant */
            kind: "GOAL";
            workspace_id: string;
            goal_id: string;
        };
        ContextDocumentMetadata: {
            /** @enum {string} */
            kind: "PERSONAL_PROFILE" | "COWORKER_NOTES" | "WORKSPACE_NOTES" | "GOAL_NOTES";
            owner_ref: components["schemas"]["ContextOwnerRef"];
            /**
             * @description Only Core may set DELETED after all owned content/index replicas acknowledge purge.
             * @enum {string}
             */
            status: "ACTIVE" | "REVOKED" | "DELETION_PENDING" | "DELETED";
            purge_manifest_digest?: string;
            purge_target_count?: number;
        } & (unknown & unknown);
        ContextDocumentCreateMetadata: {
            /** @enum {string} */
            kind: "PERSONAL_PROFILE" | "COWORKER_NOTES" | "WORKSPACE_NOTES" | "GOAL_NOTES";
            owner_ref: components["schemas"]["ContextOwnerRef"];
        };
        CoworkerRevisionRef: {
            coworker_id: string;
            revision: number;
        };
        ResourceRef: {
            workspace_id: string;
            resource_id: string;
            revision_id?: string | null;
        };
        PinnedResourceRef: components["schemas"]["ResourceRef"] & {
            revision_id: string;
        };
        ResourceInput: {
            resource_ref: components["schemas"]["PinnedResourceRef"];
            observed_digest?: string;
        };
        CommittedResource: {
            resource: components["schemas"]["Resource"];
            revision: components["schemas"]["ResourceRevision"];
            resource_ref: components["schemas"]["ResourceRef"];
        };
        Resource: {
            resource_id: string;
            workspace_id: string;
            /** @enum {string} */
            kind: "FILE" | "FOLDER" | "ARTIFACT" | "CONNECTOR_OBJECT" | "WEB_RESOURCE" | "OTHER";
            provider_identity: components["schemas"]["ProviderIdentity"];
            identity_digest?: string | null;
            display_name: string;
            current_revision_id?: string | null;
            /** @enum {string} */
            sensitivity: "PUBLIC" | "PERSONAL" | "CONFIDENTIAL" | "RESTRICTED";
            context_document: components["schemas"]["ContextDocumentMetadata"] | null;
            provenance: components["schemas"]["ProvenanceRecord"];
            /** Format: date-time */
            created_at: string;
            /** Format: date-time */
            updated_at: string;
            version: number;
        } & (unknown & unknown);
        /** @enum {string} */
        ResourceFreshness: "CURRENT" | "STALE" | "CONFLICTED" | "UNKNOWN" | "UNAVAILABLE";
        /** @enum {string} */
        ResourceLocationFreshness: "CURRENT" | "STALE" | "UNKNOWN" | "UNAVAILABLE";
        /** @enum {string} */
        DependencyFreshness: "CURRENT" | "STALE" | "CONFLICTED" | "UNKNOWN";
        ResourceRevision: {
            resource_revision_id: string;
            resource_id: string;
            parent_revision_ids: string[];
            provider_revision?: string | null;
            content_digest?: string | null;
            size_bytes?: number | null;
            media_type?: string | null;
            /** Format: date-time */
            observed_at: string;
            created_by: components["schemas"]["PrincipalRef"] | components["schemas"]["ServiceRef"];
        };
        ProviderIdentity: {
            provider_instance_id: string;
            stable_object_id: string;
            /** @enum {string} */
            identity_confidence: "STRONG" | "PROVIDER_SCOPED" | "WEAK";
            file_identity?: components["schemas"]["FileIdentity"] | null;
        };
        FileIdentity: {
            filesystem_instance_id: string;
            volume_id?: string | null;
            file_id: string;
            generation?: string | null;
            platform_kind: string;
        };
        ResourceLocationView: {
            location_id: string;
            resource_id: string;
            runtime_id?: string | null;
            environment_id?: string | null;
            connection_id?: string | null;
            /** @enum {string} */
            availability: "AVAILABLE" | "OFFLINE" | "PLACEHOLDER" | "REVOKED" | "UNKNOWN" | "UNAVAILABLE";
            writable: boolean;
            observed_revision_id?: string | null;
            observed_digest?: string | null;
            /** Format: date-time */
            observed_at: string;
            /** Format: date-time */
            last_checked_at?: string | null;
            freshness: components["schemas"]["ResourceLocationFreshness"];
        };
        WorkspaceRoot: {
            workspace_root_id: string;
            workspace_id: string;
            resource_id: string;
            location_id: string;
            display_name: string;
            /** @enum {string} */
            watch_policy: "METADATA" | "CONTENT_DIGESTS" | "SELECTED_TEXT_EXTRACTION";
            /** @enum {string} */
            replication_policy: "NONE" | "ACTIVE_TASKS" | "SELECTED_WORKSPACE_POLICY";
            /** @enum {string} */
            status: "ACTIVE" | "PAUSED" | "REVOKED" | "UNAVAILABLE";
            added_by: components["schemas"]["PrincipalRef"];
            /** Format: date-time */
            created_at: string;
            /** Format: date-time */
            updated_at: string;
            version: number;
        };
        AddWorkspaceRootRequest: {
            resource_ref: components["schemas"]["ResourceRef"];
            location_id: string;
            /** @enum {string} */
            watch_policy: "METADATA" | "CONTENT_DIGESTS" | "SELECTED_TEXT_EXTRACTION";
            /** @enum {string} */
            replication_policy: "NONE" | "ACTIVE_TASKS" | "SELECTED_WORKSPACE_POLICY";
        };
        UpdateWorkspaceRootRequest: {
            /** @enum {string} */
            watch_policy?: "METADATA" | "CONTENT_DIGESTS" | "SELECTED_TEXT_EXTRACTION";
            /** @enum {string} */
            replication_policy?: "NONE" | "ACTIVE_TASKS" | "SELECTED_WORKSPACE_POLICY";
        };
        WorkspaceRootPage: {
            items: components["schemas"]["WorkspaceRootListItem"][];
            next_cursor?: string | null;
        };
        WorkspaceRootListItem: components["schemas"]["WorkspaceRoot"] & {
            /**
             * @description Last committed ResourceLocation observation; not a live Runtime probe.
             * @enum {string}
             */
            location_availability: "AVAILABLE" | "OFFLINE" | "PLACEHOLDER" | "REVOKED" | "UNKNOWN" | "UNAVAILABLE";
        };
        ResourceSearchResult: {
            resource_ref: components["schemas"]["PinnedResourceRef"];
            /** @description Digest of the exact immutable ResourceRevision named by resource_ref.revision_id; source_matches offsets are valid only against these bytes. */
            source_content_digest: string;
            /** @description Empty unless an INDEXED_CONTENT result has source-grounded lexical matches. In INDEXED_CONTENT, contains one first-occurrence span per distinct normalized query term, all into the exact pinned ResourceRevision bytes named by resource_ref and source_content_digest. */
            source_matches: components["schemas"]["ResourceTextMatchSpan"][];
            display_name: string;
            locations: components["schemas"]["ResourceLocationView"][];
            freshness: components["schemas"]["ResourceFreshness"];
            match_reasons: ("NAME" | "MEDIA_TYPE" | "CONTENT_ON_DEMAND" | "CONTENT_INDEXED")[];
            /** @description Bounded plain-text excerpt from a memory scan or digest-verified encrypted local index. It is never rendered as HTML. */
            snippet?: string | null;
        };
        /** @description A lexical token match in the original UTF-8 bytes of the containing ResourceSearchResult's pinned ResourceRevision. Offsets are zero-based and half-open; consumers must verify the containing source_content_digest and never apply offsets to a newer Resource head. */
        ResourceTextMatchSpan: {
            /** @description Normalized Unicode alphanumeric query term. */
            term: string;
            start_utf8_byte: number;
            end_utf8_byte_exclusive: number;
        };
        ResourceSearchPage: {
            items: components["schemas"]["ResourceSearchResult"][];
            next_cursor?: string | null;
            /** @enum {string} */
            mode: "METADATA" | "ON_DEMAND_CONTENT" | "INDEXED_CONTENT";
            content_scan: components["schemas"]["ResourceContentScan"];
        };
        RebuildResourceTextIndexRequest: {
            resource_revision_id: string;
            content_digest: string;
        };
        ResourceTextIndexRebuildResult: {
            requestId: string;
            correlationId: string;
            workspaceId: string;
            resourceId: string;
            resourceRevisionId: string;
            contentDigest: string;
            /** @enum {string} */
            outcome: "INDEXED" | "NOT_INDEXABLE";
            /**
             * @description Required for NOT_INDEXABLE and null for INDEXED.
             * @enum {string|null}
             */
            reason: "UNSUPPORTED_TYPE" | "OVER_SIZE_LIMIT" | "INVALID_UTF8" | "CONTROL_CHARACTERS" | "TERM_LIMIT_EXCEEDED" | null;
        };
        ResourceContentScan: {
            candidates_scanned: number;
            text_resources_checked: number;
            skipped_unsupported_type: number;
            skipped_over_file_limit: number;
            skipped_revision_changed: number;
            byte_budget_exhausted: boolean;
            candidate_budget_exhausted: boolean;
            /** @constant */
            max_candidates: 20;
            /** @constant */
            max_file_bytes: 1048576;
            /** @constant */
            max_total_bytes: 8388608;
        } | null;
        ResourceRevisionView: components["schemas"]["ResourceRevision"] & {
            is_head: boolean;
        };
        ResourceRevisionPage: {
            items: components["schemas"]["ResourceRevisionView"][];
            next_cursor?: string | null;
        };
        UserRequest: ({
            request_id: string;
            workspace_id: string;
            conversation_id?: string | null;
            conversation_turn_id?: string | null;
            task_id?: string | null;
            attempt_id?: string | null;
            agent_session_id: string;
            invocation_id?: string | null;
            /** @enum {string} */
            kind: "QUESTION" | "DECISION" | "RESOURCE_SELECTION" | "EXTERNAL_AUTHORIZATION";
            /** @enum {string} */
            interaction_mode: "FORM" | "EXTERNAL_URL";
            prompt: string;
            response_schema?: components["schemas"]["JsonSchema"] | null;
            choices: components["schemas"]["Choice"][];
            /** @enum {string} */
            status: "PENDING" | "ANSWERED" | "DISMISSED" | "EXPIRED" | "CANCELLED";
            /** Format: date-time */
            expires_at?: string | null;
            /** Format: date-time */
            created_at: string;
            /** Format: date-time */
            resolved_at?: string | null;
            resolved_by?: components["schemas"]["PrincipalRef"] | null;
            response_digest?: string | null;
            version: number;
        } & (unknown & unknown)) & ({
            conversation_id: string;
            conversation_turn_id: string;
            task_id?: null;
            attempt_id?: null;
        } | {
            conversation_id?: null;
            conversation_turn_id?: null;
            task_id: string;
            attempt_id?: null;
        } | {
            conversation_id?: null;
            conversation_turn_id?: null;
            task_id: string;
            attempt_id: string;
        });
        UserRequestPage: {
            items: components["schemas"]["UserRequest"][];
            next_cursor?: string | null;
        };
        UserRequestResponseInput: {
            /** @description Non-secret form data for FORM requests, or exactly {action: accept|decline|cancel} for EXTERNAL_URL requests. The server enforces the request's interaction mode. */
            response: components["schemas"]["JsonValue"];
        };
        UserRequestResponse: {
            request_id: string;
            response_digest: string;
            responded_by: components["schemas"]["PrincipalRef"];
            response_channel_ref?: null | {
                channel_binding_id: string;
                provider_event_id: string;
            };
            /** Format: date-time */
            responded_at: string;
        };
        ExternalAuthHandoff: {
            /**
             * Format: uri
             * @description Confidential Runtime-private URL; clients must open externally and not persist or log it.
             */
            url: string;
            /**
             * Format: uri
             * @description Parsed destination origin; TLS identity is confirmed by the system browser.
             */
            display_origin: string;
        };
        EnvironmentControlLeaseView: {
            control_lease_id: string;
            environment_id: string;
            task_id: string;
            attempt_id: string;
            runtime_id: string;
            runtime_incarnation_id: string;
            /** @enum {string} */
            owner_kind: "AGENT" | "HUMAN";
            owner_ref: string | components["schemas"]["PrincipalRef"];
            epoch: number;
            /** @enum {string} */
            state: "ACTIVE" | "RELEASING" | "RELEASED" | "EXPIRED" | "REVOKED";
            /** Format: date-time */
            acquired_at: string;
            /** Format: date-time */
            expires_at: string;
            version: number;
        };
        TakeEnvironmentControlRequest: {
            expected_control_epoch: number;
            reason?: string | null;
        };
        ReturnEnvironmentControlRequest: {
            expected_control_epoch: number;
            reconcile_observation_ref?: components["schemas"]["ResourceRef"] | null;
        };
        /** @description Immutable user-authored Workspace instructions; the pinned text Resource is UTF-8 and at most 64 KiB. */
        WorkspaceInstructionRevision: {
            workspace_id: string;
            revision: number;
            parent_revisions: number[];
            content_ref: components["schemas"]["ResourceRef"];
            content_digest: string;
            authored_by: components["schemas"]["PrincipalRef"];
            /** Format: date-time */
            created_at: string;
        };
        WorkspaceInstructionRevisionPage: {
            items: components["schemas"]["WorkspaceInstructionRevision"][];
            next_cursor: string | null;
        };
        /** @description Creates instructions from the current head of a pinned, same-Workspace UTF-8 text Resource no larger than 64 KiB. Historical or conflicted/non-current heads are rejected. The immutable instruction pin remains valid if the Resource later advances. */
        CreateWorkspaceInstructionRevisionRequest: {
            parent_revisions: number[];
            content_ref: components["schemas"]["ResourceRef"];
            content_digest: string;
        };
        CapabilityInvocation: {
            invocation_id: string;
            workspace_id: string;
            scope: components["schemas"]["ConversationInvocationScope"] | components["schemas"]["PlanningInvocationScope"] | components["schemas"]["AttemptInvocationScope"];
            agent_session_id: string;
            capability_grant_id: string;
            activation_id: string;
            capability_ref: components["schemas"]["CapabilityRef"];
            operation: string;
            request_digest: string;
            /**
             * @description Selected by the adapter before admission and immutable; UNKNOWN only when the exact route cannot be proven. Never accepted from the agent as evidence.
             * @enum {string}
             */
            execution_method: "STRUCTURED_API" | "STRUCTURED_BROWSER" | "ACCESSIBILITY_BROWSER" | "SCREEN_COMPUTER_USE" | "DETERMINISTIC_LOCAL" | "NATIVE_AGENT_TOOL" | "UNKNOWN";
            action_batch: components["schemas"]["ActionBatchMemberRef"] | null;
            idempotency_key?: string | null;
            /** @enum {string} */
            status: "CREATED" | "DISPATCHED" | "WAITING" | "INPUT_REQUIRED" | "CANCEL_REQUESTED" | "SUCCEEDED" | "FAILED" | "CANCELLED" | "AMBIGUOUS";
            /** @enum {string|null} */
            provider_task_status?: "WORKING" | "INPUT_REQUIRED" | "COMPLETED" | "FAILED" | "CANCELLED" | "UNKNOWN" | null;
            /** Format: date-time */
            provider_task_created_at?: string | null;
            /** Format: date-time */
            provider_task_expires_at?: string | null;
            provider_task_ttl_ms?: number | null;
            provider_poll_after_ms?: number | null;
            /** Format: date-time */
            provider_updated_at?: string | null;
            partial_result_refs: components["schemas"]["ResourceRef"][];
            result_refs: components["schemas"]["ResourceRef"][];
            effect_id?: string | null;
            failure?: components["schemas"]["FailureRecord"] | null;
            /** Format: date-time */
            created_at: string;
            /** Format: date-time */
            updated_at: string;
            /** Format: date-time */
            completed_at?: string | null;
            version: number;
        };
        ActionBatchMemberRef: {
            action_batch_id: string;
            /** @description Zero-based; must be less than operation_count. */
            ordinal: number;
            operation_count: number;
            batch_digest: string;
        };
        ConversationInvocationScope: {
            /** @constant */
            kind: "CONVERSATION";
            conversation_id: string;
        };
        PlanningInvocationScope: {
            /** @constant */
            kind: "TASK_PLANNING";
            task_id: string;
        };
        AttemptInvocationScope: {
            /** @constant */
            kind: "ATTEMPT_EXECUTION";
            task_id: string;
            attempt_id: string;
        };
        CapabilityInvocationPage: {
            items: components["schemas"]["CapabilityInvocation"][];
            next_cursor?: string | null;
        };
        UsageObservation: {
            usage_observation_id: string;
            workspace_id: string;
            task_id?: string | null;
            environment_id?: string | null;
            attempt_id?: string | null;
            invocation_id?: string | null;
            agent_session_id?: string | null;
            /** @enum {string} */
            source: "AGENT_REPORTED" | "PROVIDER_REPORTED" | "HOST_MEASURED";
            /** @enum {string} */
            metric: "TOKEN_INPUT" | "TOKEN_OUTPUT" | "WALL_TIME" | "COST" | "BYTES_READ" | "BYTES_WRITTEN" | "CALL_COUNT";
            quantity: number | null;
            unit: string;
            currency?: string | null;
            /** @enum {string} */
            confidence: "EXACT" | "ESTIMATED" | "UNKNOWN";
            /** Format: date-time */
            observed_at: string;
            source_ref?: string | null;
        } & (unknown & unknown & unknown);
        NotificationPreference: {
            workspace_id: string;
            event_class: string;
            /** @enum {string} */
            policy: "ALWAYS" | "ON_SUCCESS" | "ON_FAILURE" | "ON_CONDITION" | "SILENT";
            preferred_channels: string[];
            quiet_hours?: components["schemas"]["QuietHours"] | null;
            version: number;
        };
        UpdateNotificationPreferenceRequest: {
            /** @enum {string} */
            policy: "ALWAYS" | "ON_SUCCESS" | "ON_FAILURE" | "ON_CONDITION" | "SILENT";
            preferred_channels: string[];
            quiet_hours?: components["schemas"]["QuietHours"] | null;
        };
        Notification: {
            delivery_id: string;
            workspace_id: string;
            dedupe_key: string;
            source_event_id: string;
            channel_binding_id?: string | null;
            attempt_runtime_id?: string | null;
            attempt_host_epoch?: number | null;
            /** @enum {string} */
            status: "PENDING" | "SENDING" | "SENT" | "FAILED" | "AMBIGUOUS" | "SUPPRESSED";
            attempt_count: number;
            /** Format: date-time */
            next_attempt_at?: string | null;
            last_error_code?: components["schemas"]["error-codes.schema"] | null;
            /** Format: date-time */
            created_at: string;
            /** Format: date-time */
            settled_at?: string | null;
        };
        NotificationPage: {
            items: components["schemas"]["Notification"][];
            next_cursor?: string | null;
        };
        SkillProposal: {
            skill_proposal_id: string;
            workspace_id: string;
            source_task_id: string;
            source_artifact_id: string;
            draft_resource_ref: components["schemas"]["ResourceRef"];
            draft_digest: string;
            /** @enum {string} */
            redaction_status: "PENDING" | "PASSED" | "FAILED";
            /** @enum {string} */
            status: "DRAFT" | "REVIEW" | "APPROVED" | "REJECTED" | "PUBLISHED";
            approved_by?: components["schemas"]["PrincipalRef"] | null;
            published_capability_ref?: components["schemas"]["CapabilityRef"] | null;
            /** Format: date-time */
            created_at: string;
            /** Format: date-time */
            updated_at: string;
            version: number;
        };
        SkillProposalPage: {
            items: components["schemas"]["SkillProposal"][];
            next_cursor?: string | null;
        };
        CreateSkillProposalRequest: {
            source_task_id: string;
            source_artifact_id: string;
            draft_resource_ref: components["schemas"]["ResourceRef"];
            draft_digest: string;
        };
        DependencyEdge: {
            dependency_edge_id: string;
            workspace_id: string;
            source_resource_id: string;
            /** @description Exact source ResourceRevision consumed by this dependent. */
            source_revision_id: string;
            /** @enum {string} */
            dependent_kind: "ARTIFACT_VERSION" | "VERIFICATION_RUN";
            /** @description artifact://<workspace-id>/<artifact-id>@v<version> or verification://<verification-run-id> */
            dependent_ref: string;
            /** Format: date-time */
            created_at: string;
        };
        InvalidationRecord: {
            invalidation_record_id: string;
            dependency_edge_id: string;
            /** @description Newly observed source ResourceRevision that made the pinned dependency stale. */
            observed_revision_id: string;
            reason_code: string;
            /** Format: date-time */
            created_at: string;
        };
        ResourceDependencyView: {
            dependency_edge: components["schemas"]["DependencyEdge"];
            dependent_freshness: components["schemas"]["DependencyFreshness"];
            latest_invalidation: components["schemas"]["InvalidationRecord"] | null;
        };
        ResourceDependencyPage: {
            items: components["schemas"]["ResourceDependencyView"][];
            next_cursor?: string | null;
        };
        InvalidationPage: {
            items: components["schemas"]["InvalidationRecord"][];
            next_cursor?: string | null;
        };
        VerificationRun: {
            verification_run_id: string;
            task_id: string;
            criterion_id: string;
            task_spec_revision: number;
            criterion_digest: string;
            verifier_kind: string;
            verifier_version: string;
            subject_refs: components["schemas"]["PinnedResourceRef"][];
            /** @description Exact logical resources/revisions consumed and their observed byte digests when available. */
            inputs: components["schemas"]["ResourceInput"][];
            /** @enum {string} */
            status: "PENDING" | "RUNNING" | "PASSED" | "FAILED" | "INCONCLUSIVE";
            evidence_refs: string[];
            /** Format: date-time */
            started_at?: string | null;
            /** Format: date-time */
            completed_at?: string | null;
        };
        VerificationRunPage: {
            items: components["schemas"]["VerificationRun"][];
            next_cursor?: string | null;
        };
        AgentSessionOptions: {
            agent_binding_id: string;
            descriptor_digest: string;
            /** Format: date-time */
            observed_at: string;
            /** Format: date-time */
            expires_at: string;
            options: components["schemas"]["AgentSessionOption"][];
        };
        AgentHarnessCapabilities: {
            agent_binding_id: string;
            endpoint_id: string;
            harness_version: string | null;
            protocol_version?: string | null;
            descriptor_digest: string;
            effective_config_digest?: string | null;
            features: {
                [key: string]: unknown;
            };
            /** Format: date-time */
            observed_at: string;
            /** Format: date-time */
            expires_at: string | null;
        };
        DelegatedWorkerPolicy: {
            capability_allowlist: components["schemas"]["CapabilityRef"][];
            /** @enum {string} */
            maximum_effect_risk: "SAFE" | "SENSITIVE" | "HIGH_IMPACT";
            /** @enum {string} */
            filesystem_write_scope: "WORKTREE_ONLY" | "ATTEMPT_PRIVATE" | "EXPLICIT_SHARED";
            /** @enum {string} */
            external_effects: "DENY" | "REQUIRE_EXISTING_POLICY";
            /** @enum {string} */
            secret_access: "NONE" | "TASK_SCOPED_GRANTS_ONLY";
        };
        DelegatedEnvironmentPolicy: {
            placement_preference: components["schemas"]["PlacementPreference"];
            /** @enum {string} */
            isolation: "REQUIRED" | "PREFERRED";
            /** @enum {string} */
            sharing_scope: "ATTEMPT_PRIVATE" | "TASK_SHARED" | "COWORKER_PRIVATE" | "WORKSPACE_SHARED";
        };
        WarmPolicy: {
            /** @enum {string} */
            host: "COLD" | "TTL" | "PIN_WHILE_ACTIVE";
            /** @enum {string} */
            native_session: "CLOSE_ON_SETTLE" | "REUSE_IF_SAFE";
            /** @enum {string} */
            capability_hosts: "COLD" | "TTL";
            /** @enum {string} */
            browser_environment: "COLD" | "TASK" | "WORKSPACE";
            /** @enum {string} */
            local_model: "PROVIDER_DEFAULT" | "KEEP_RECENT_HINT";
            ttl_ms?: number | null;
            max_memory_bytes?: number | null;
            max_idle_cost?: components["schemas"]["CostLimit"] | null;
            triggers: ("ACTIVE_TASK" | "RECENT_USE" | "USER_SELECTED" | "QUOTA_LOW" | "PREDICTED_FAILOVER" | "DEADLINE_APPROACHING")[];
        };
        DelegationProfileRevisionInput: {
            /** @description Trimmed and NFC-normalized; compared by Unicode case-folding within the AgentBinding. */
            name: string;
            routing_description: string;
            instructions?: string | null;
            /** @description Adapter-negotiated non-secret values only; canonical JSON is at most 64 KiB. */
            session_options: {
                [key: string]: unknown;
            };
            /** @description Required when session_options is nonempty; pins its adapter-negotiated schema. */
            session_options_descriptor_digest: string | null;
            required_features: components["schemas"]["AgentFeature"][];
            preferred_features: components["schemas"]["AgentFeature"][];
            enforced_policy: components["schemas"]["DelegatedWorkerPolicy"];
            /** @enum {string} */
            optimization_preference: "QUALITY_FIRST" | "BALANCED" | "COST_FIRST" | "LATENCY_FIRST";
            quality_floor?: components["schemas"]["AcceptanceCriterion"][] | null;
            max_concurrency: number;
            max_host_delegation_depth: number;
            budget_ceiling?: components["schemas"]["BudgetSpec"] | null;
            /** @enum {string} */
            latency_class: "STANDARD" | "INTERACTIVE" | "DEADLINE_SENSITIVE";
            environment_policy: components["schemas"]["DelegatedEnvironmentPolicy"];
            /** @enum {string} */
            native_delegation_policy: "INHERIT" | "ALLOW" | "DENY_IF_SUPPORTED";
            warm_policy: components["schemas"]["WarmPolicy"];
        } & (unknown & unknown);
        DelegationProfileRevisionView: {
            delegation_profile_id: string;
            revision: number;
            workspace_id: string;
            name: string;
            routing_description: string;
            instructions?: string | null;
            session_options: {
                [key: string]: unknown;
            };
            session_options_descriptor_digest: string | null;
            required_features: components["schemas"]["AgentFeature"][];
            preferred_features: components["schemas"]["AgentFeature"][];
            enforced_policy: components["schemas"]["DelegatedWorkerPolicy"];
            /** @enum {string} */
            optimization_preference: "QUALITY_FIRST" | "BALANCED" | "COST_FIRST" | "LATENCY_FIRST";
            quality_floor?: components["schemas"]["AcceptanceCriterion"][] | null;
            max_concurrency: number;
            max_host_delegation_depth: number;
            budget_ceiling?: components["schemas"]["BudgetSpec"] | null;
            /** @enum {string} */
            latency_class: "STANDARD" | "INTERACTIVE" | "DEADLINE_SENSITIVE";
            environment_policy: components["schemas"]["DelegatedEnvironmentPolicy"];
            /** @enum {string} */
            native_delegation_policy: "INHERIT" | "ALLOW" | "DENY_IF_SUPPORTED";
            warm_policy: components["schemas"]["WarmPolicy"];
            authored_by: components["schemas"]["PrincipalRef"];
            /** Format: date-time */
            created_at: string;
        } & (unknown & unknown);
        CreateDelegationProfileRequest: {
            workspace_id: string;
            agent_binding_id: string;
            revision: components["schemas"]["DelegationProfileRevisionInput"];
        };
        DuplicateDelegationProfileRequest: {
            /** @description New profile name; normalized and uniqueness-checked by DelegationProfileService. */
            name: string;
        };
        DelegationProfileStatusRequest: {
            /** @enum {string} */
            status: "ENABLED" | "DISABLED" | "ARCHIVED";
        };
        DelegationProfile: {
            delegation_profile_id: string;
            workspace_id: string;
            agent_binding_id: string;
            name: string;
            current_revision: number;
            revision: components["schemas"]["DelegationProfileRevisionView"];
            /** @enum {string} */
            status: "ENABLED" | "DISABLED" | "ARCHIVED";
            /** Format: date-time */
            created_at: string;
            /** Format: date-time */
            updated_at: string;
            version: number;
        };
        WorkerPerformanceProjection: {
            delegation_profile_id: string;
            /** @enum {string} */
            task_category: "SOFTWARE_ENGINEERING" | "RESEARCH" | "WRITING" | "DATA_ANALYSIS" | "OFFICE" | "BROWSER" | "PERSONAL_ADMIN" | "OTHER";
            sample_count: number;
            verifier_pass_rate: number | null;
            median_latency_ms: number | null;
            median_observed_cost: components["schemas"]["UsageQuantity"] | null;
            retry_rate: number | null;
            human_intervention_rate: number | null;
            failure_categories: string[];
            /** @enum {string} */
            confidence: "LOW" | "MEDIUM" | "HIGH";
        };
        DelegationProfilePage: {
            items: components["schemas"]["DelegationProfile"][];
            next_cursor?: string | null;
        };
        DelegationProfileRevisionPage: {
            items: components["schemas"]["DelegationProfileRevisionView"][];
            next_cursor?: string | null;
        };
        NotificationPolicy: {
            /** @enum {string} */
            blockers: "ALWAYS" | "SILENT";
            /** @enum {string} */
            completion: "ALWAYS" | "ON_SUCCESS" | "SILENT";
            /** @enum {string} */
            failures: "ALWAYS" | "SILENT";
        };
        CoworkerContextPolicy: {
            allowed_context_kinds: ("PERSONAL_PROFILE" | "COWORKER_NOTES" | "WORKSPACE_NOTES" | "GOAL_NOTES")[];
            max_retrieved_items: number;
            retain_task_summaries: boolean;
            /** @constant */
            require_user_confirmation_for_memory: true;
        };
        CoworkerInteractionPolicy: {
            /** @enum {string} */
            read_only_work: "STANDARD_TRUST_POLICY" | "REQUIRE_OWNER_APPROVAL" | "HANDOFF_TO_OWNER";
            /** @enum {string} */
            draft_creation: "STANDARD_TRUST_POLICY" | "REQUIRE_OWNER_APPROVAL" | "HANDOFF_TO_OWNER";
            /** @enum {string} */
            external_mutation: "STANDARD_TRUST_POLICY" | "REQUIRE_OWNER_APPROVAL" | "HANDOFF_TO_OWNER";
            /** @enum {string} */
            destructive_action: "STANDARD_TRUST_POLICY" | "REQUIRE_OWNER_APPROVAL" | "HANDOFF_TO_OWNER";
            /** @enum {string} */
            financial_commitment: "STANDARD_TRUST_POLICY" | "REQUIRE_OWNER_APPROVAL" | "HANDOFF_TO_OWNER";
        };
        CoworkerRevisionInput: {
            name: string;
            avatar_ref?: components["schemas"]["PinnedResourceRef"] | null;
            role_description: string;
            default_lead_agent_binding_id?: string | null;
            /** @enum {string} */
            delegation_strategy: "NATIVE_DEFAULT" | "BALANCED" | "COST_SAVER" | "HOST_DELEGATION_ONLY";
            enabled_delegation_profile_ids: string[];
            delegation_budget_policy?: components["schemas"]["DelegationBudgetPolicy"] | null;
            lead_failover_policy?: components["schemas"]["LeadFailoverPolicy"] | null;
            interaction_policy: components["schemas"]["CoworkerInteractionPolicy"];
            context_policy: components["schemas"]["CoworkerContextPolicy"];
            notification_policy: components["schemas"]["NotificationPolicy"];
        };
        CreateCoworkerRequest: {
            workspace_id: string;
            revision: components["schemas"]["CoworkerRevisionInput"];
        };
        CoworkerStatusRequest: {
            /** @enum {string} */
            status: "ACTIVE" | "PAUSED" | "ARCHIVED";
        };
        SetPrimaryCoworkerRequest: {
            coworker_id: string | null;
        };
        Coworker: {
            coworker_id: string;
            workspace_id: string;
            current_revision: number;
            revision: components["schemas"]["CoworkerRevisionInput"];
            /** @enum {string} */
            status: "ACTIVE" | "PAUSED" | "ARCHIVED";
            is_primary: boolean;
            /** Format: date-time */
            created_at: string;
            /** Format: date-time */
            updated_at: string;
            version: number;
        };
        CoworkerRevisionRecord: {
            coworker_id: string;
            revision: number;
            definition: components["schemas"]["CoworkerRevisionInput"];
            authored_by: components["schemas"]["PrincipalRef"];
            /** Format: date-time */
            created_at: string;
        };
        CoworkerPresenceProjection: {
            coworker_id: string;
            /** Format: date-time */
            computed_at: string;
            /** @enum {string} */
            proactive_status: "ACTIVE" | "PAUSED" | "ARCHIVED";
            /** @enum {string} */
            activity_status: "AVAILABLE" | "PLANNING" | "WORKING" | "WAITING" | "NEEDS_YOU";
            /**
             * @description Readiness of this Coworker's selected default lead path only; it does not summarize every worker
             * @enum {string}
             */
            runtime_status: "AVAILABLE" | "DEGRADED" | "OFFLINE" | "UNKNOWN";
            active_task_count: number;
            waiting_task_count: number;
            needs_you_count: number;
            /** Format: date-time */
            last_task_activity_at: string | null;
        };
        CoworkerPage: {
            items: components["schemas"]["Coworker"][];
            next_cursor?: string | null;
        };
        GoalRevisionInput: {
            objective: string;
            success_criteria: string[];
            constraints: string[];
            /** Format: date-time */
            horizon?: string | null;
            related_task_ids: string[];
            /** @description Exact current Artifact versions are captured as owner-authored Goal revision provenance; each reference must belong to the selected Workspace. */
            related_artifact_refs?: components["schemas"]["ArtifactVersionRef"][];
            related_routine_refs: {
                routine_id: string;
                revision: number;
            }[];
        };
        CreateGoalRequest: {
            workspace_id: string;
            coworker_id?: string | null;
            revision: components["schemas"]["GoalRevisionInput"];
        };
        GoalStatusRequest: {
            /** @enum {string} */
            status: "ACTIVE" | "PAUSED" | "COMPLETED" | "ARCHIVED";
        };
        Goal: {
            goal_id: string;
            workspace_id: string;
            coworker_id: string | null;
            current_revision: number;
            revision: components["schemas"]["GoalRevisionInput"];
            /** @enum {string} */
            status: "ACTIVE" | "PAUSED" | "COMPLETED" | "ARCHIVED";
            /** @description A read-only Task/Evidence projection. Null only when the projection cannot be read; PARTIAL explicitly reports unsupported verification/freshness dimensions and uses null counts rather than inferred zeros. */
            progress: components["schemas"]["GoalProgressProjection"] | null;
            /** Format: date-time */
            created_at: string;
            /** Format: date-time */
            updated_at: string;
            version: number;
        };
        GoalPage: {
            items: components["schemas"]["Goal"][];
            next_cursor?: string | null;
        };
        GoalTaskContribution: {
            task_id: string;
            /** @enum {string} */
            task_status: "READY" | "RUNNING" | "WAITING_USER" | "BLOCKED" | "VERIFYING" | "NEEDS_USER" | "INCOMPLETE" | "PAUSE_REQUESTED" | "PAUSED" | "COMPLETED" | "FAILED" | "CANCEL_REQUESTED" | "CANCELLED";
            /** @enum {string} */
            outcome_state: "VERIFIED" | "INCOMPLETE" | "UNVERIFIED" | "STALE" | "CONFLICTED";
            evidence_refs: string[];
        };
        GoalProgressProjection: {
            /** Format: date-time */
            computed_at: string;
            /** @enum {string} */
            availability: "COMPLETE" | "PARTIAL";
            limitations: ("VERIFICATION_RUN_READ_MODEL_UNAVAILABLE" | "TASK_DEPENDENCY_FRESHNESS_UNAVAILABLE" | "ARTIFACT_DEPENDENCY_FRESHNESS_UNAVAILABLE" | "ARTIFACT_EVIDENCE_REFERENCE_UNRESOLVED" | "EVIDENCE_LIST_TRUNCATED")[];
            /** @description Null until passing VerificationRuns can be matched to current mandatory Task criteria and inputs. */
            verified_task_count: number | null;
            linked_task_count: number;
            /** @description Null while Task and Artifact dependency freshness readers are unavailable. */
            stale_source_count: number | null;
            /** @description Null while Task and Artifact dependency freshness readers are unavailable. */
            conflicted_source_count: number | null;
            contributions: components["schemas"]["GoalTaskContribution"][];
            artifact_evidence_refs: components["schemas"]["GoalArtifactEvidenceRefs"][];
            summary: string;
        };
        GoalArtifactEvidenceRefs: {
            artifact_id: string;
            version: number;
            evidence_refs: string[];
        };
        UsageQuantity: {
            quantity: number;
            unit: string;
            currency?: string | null;
            /** @enum {string} */
            confidence: "EXACT" | "ESTIMATED";
        };
        CostLimit: {
            amount_minor_units: number;
            currency: string;
        };
        QuotaObservation: {
            agent_binding_id: string;
            source: string;
            /** Format: date-time */
            observed_at: string;
            /** Format: date-time */
            expires_at: string | null;
            /** @enum {string} */
            state: "NORMAL" | "LOW" | "EXHAUSTED" | "UNKNOWN";
            remaining_hint: components["schemas"]["UsageQuantity"] | null;
            /** Format: date-time */
            reset_at: string | null;
        };
        DelegationBudgetPolicy: {
            max_concurrent_children?: number | null;
            max_host_delegation_depth?: number | null;
            max_per_attempt?: components["schemas"]["BudgetSpec"] | null;
            max_per_task?: components["schemas"]["BudgetSpec"] | null;
            max_per_profile?: components["schemas"]["BudgetSpec"] | null;
            /** @enum {string} */
            on_threshold: "WARN" | "REDUCE_CONCURRENCY" | "PREFER_CHEAPER" | "REQUIRE_APPROVAL" | "STOP_NEW_DELEGATION";
        };
        LeadFailoverPolicy: {
            /** @enum {string} */
            mode: "DISABLED" | "ASK" | "ALLOW_LISTED";
            triggers: ("AGENT_UNAVAILABLE" | "QUOTA_EXHAUSTED" | "RUNTIME_UNAVAILABLE")[];
            /** @description Ordered same-Workspace lead-eligible bindings; revalidated on every use. */
            fallback_agent_binding_ids: string[];
            max_lead_changes: number;
        } & (unknown & unknown & unknown);
        TaskSpecProposal: {
            objective: string;
            /** @enum {string|null} */
            task_category?: "SOFTWARE_ENGINEERING" | "RESEARCH" | "WRITING" | "DATA_ANALYSIS" | "OFFICE" | "BROWSER" | "PERSONAL_ADMIN" | "OTHER" | null;
            constraints: string[];
            input_refs: components["schemas"]["PinnedResourceRef"][];
            required_outputs: components["schemas"]["OutputRequirement"][];
            acceptance_criteria: components["schemas"]["AcceptanceCriterion"][];
            budget?: components["schemas"]["BudgetSpec"] | null;
            delegation_budget_policy?: components["schemas"]["DelegationBudgetPolicy"] | null;
            /** Format: date-time */
            deadline?: string | null;
        };
        Suggestion: {
            suggestion_id: string;
            workspace_id: string;
            coworker_id: string | null;
            dedupe_key: string;
            /** @enum {string} */
            kind: "TASK_OPPORTUNITY" | "ROUTINE_OPPORTUNITY" | "AUTOMATION_OPPORTUNITY";
            reason: string;
            source_refs: components["schemas"]["PinnedResourceRef"][];
            goal_refs: {
                goal_id: string;
                revision: number;
            }[];
            /** @enum {string} */
            proposed_action: "TASK" | "OPEN_ROUTINE_EDITOR" | "OPEN_AUTOMATION_EDITOR";
            proposed_by: components["schemas"]["ServiceRef"];
            proposed_task_spec: components["schemas"]["TaskSpecProposal"] | null;
            estimated_cost: components["schemas"]["UsageQuantity"] | null;
            /**
             * @description Execution urgency/latency class
             * @enum {string|null}
             */
            latency_class_hint: "STANDARD" | "INTERACTIVE" | "DEADLINE_SENSITIVE" | null;
            /** @enum {string} */
            status: "PROPOSED" | "ACCEPTED" | "DISMISSED" | "EXPIRED";
            /** Format: date-time */
            created_at: string;
            /** Format: date-time */
            expires_at: string;
            /** Format: date-time */
            snoozed_until: string | null;
            /** Format: date-time */
            resolved_at: string | null;
            resolved_by: components["schemas"]["PrincipalRef"] | null;
            /** @enum {string|null} */
            resolution_reason: "ACCEPTED_BY_OWNER" | "DISMISSED_BY_OWNER" | "MUTED_KIND" | "SYSTEM_EXPIRY" | null;
            result_task_id: string | null;
            version: number;
        } & (unknown & unknown & unknown);
        /** @description Bounded proof of the atomic Suggestion-to-READY-Task acceptance. CREATED is returned from the write transaction; REPLAYED is returned from the original Task request receipt and accepted Suggestion validated together. Task status describes the originally committed READY Task and does not advance with later Task state. */
        SuggestionTaskAcceptanceReceipt: {
            /** @description Must equal the selected Workspace and task.workspace_id. */
            workspace_id: string;
            /** @enum {string} */
            disposition: "CREATED" | "REPLAYED";
            suggestion: {
                suggestion_id: string;
                /** @constant */
                status: "ACCEPTED";
                /** @description Must equal task.task_id. */
                result_task_id: string;
                version: number;
            };
            task: {
                task_id: string;
                workspace_id: string;
                /** @constant */
                status: "READY";
                version: number;
            };
        };
        ResolveSuggestionRequest: {
            /** @constant */
            resolution: "DISMISSED";
        };
        AcceptSuggestionTaskRequest: Record<string, never>;
        SnoozeSuggestionRequest: {
            /** Format: date-time */
            snoozed_until: string | null;
        };
        SuggestionPreference: {
            workspace_id: string;
            /** @enum {string} */
            kind: "TASK_OPPORTUNITY" | "ROUTINE_OPPORTUNITY" | "AUTOMATION_OPPORTUNITY";
            muted: boolean;
            /**
             * Format: date-time
             * @description Null only for a virtual default preference that has never been persisted
             */
            updated_at: string | null;
            version: number;
        };
        SuggestionPreferencePage: {
            items: components["schemas"]["SuggestionPreference"][];
        };
        SetSuggestionPreferenceRequest: {
            muted: boolean;
        };
        SuggestionPage: {
            items: components["schemas"]["Suggestion"][];
            next_cursor?: string | null;
        };
        StartDemonstrationRequest: {
            workspace_id: string;
            environment_id: string;
            /** @constant */
            consent_semantic_capture: true;
            capture_policy: components["schemas"]["DemonstrationCapturePolicy"];
        };
        DemonstrationCapturePolicy: {
            max_duration_ms: number;
            max_actions: number;
            max_trace_bytes: number;
            /** @enum {string} */
            allowed_environment_class: "BROWSER" | "DESKTOP";
            /** @enum {string} */
            sensitive_region_policy: "PAUSE_ON_DETECTION" | "OMIT_SENSITIVE_FIELDS";
        };
        DemonstrationSession: {
            demonstration_session_id: string;
            workspace_id: string;
            environment_id: string;
            /** @enum {string} */
            status: "CREATED" | "CAPTURING" | "PAUSED" | "REVIEW" | "CONVERTED" | "ABORTED";
            capture_policy: components["schemas"]["DemonstrationCapturePolicy"];
            captured_action_count: number;
            captured_trace_bytes: number;
            /** Format: date-time */
            started_at: string;
            /** Format: date-time */
            completed_at: string | null;
            trace_resource_id: string | null;
            skill_proposal_id: string | null;
            version: number;
        };
        ApiError: {
            code: components["schemas"]["error-codes.schema"];
            message: string;
            retryable: boolean;
            correlation_id: string;
            details?: Record<string, never> | null;
        };
        /**
         * LiteCowork ErrorCode
         * @enum {string}
         */
        "error-codes.schema": "NOT_FOUND" | "UNAUTHORIZED" | "FORBIDDEN" | "POLICY_DENIED" | "APPROVAL_REQUIRED" | "WORKSPACE_ARCHIVED" | "WORKSPACE_NOT_QUIESCENT" | "STALE_WORKSPACE_VERSION" | "INVALID_ARGUMENT" | "INVALID_TRANSITION" | "STALE_VERSION" | "AGENT_UNAVAILABLE" | "AGENT_NOT_LEAD_ELIGIBLE" | "AGENT_SESSION_OVERRIDE_UNSUPPORTED" | "AGENT_NATIVE_CONFIG_CHANGED" | "AGENT_QUOTA_EXHAUSTED" | "STALE_TASK_VERSION" | "STALE_SPEC_REVISION" | "CONFLICT" | "ARTIFACT_ARCHIVED" | "INVALID_ARTIFACT_TRANSITION" | "TASK_NOT_FOUND" | "TASK_TERMINAL" | "INVALID_PLAN" | "PLAN_CYCLE" | "STEP_NOT_READY" | "DELEGATION_PROFILE_DISABLED" | "DELEGATION_PROFILE_ARCHIVED" | "DELEGATION_PROFILE_INCOMPATIBLE" | "DELEGATION_PROFILE_OPTIONS_INVALID" | "DELEGATION_DEPTH_EXCEEDED" | "DELEGATION_CONCURRENCY_EXCEEDED" | "LEASE_CONFLICT" | "STALE_FENCE" | "INVALID_RESOURCE_REF" | "TASK_PAUSE_UNSAFE" | "TASK_ALREADY_PAUSED" | "RECOVERY_EXHAUSTED" | "SKILL_DRIFT_DETECTED" | "PLACEMENT_UNAVAILABLE" | "RUNTIME_UNAVAILABLE" | "ENVIRONMENT_UNAVAILABLE" | "CAPABILITY_UNAVAILABLE" | "CAPABILITY_UNHEALTHY" | "SECRET_UNAVAILABLE" | "RESOURCE_UNAVAILABLE" | "AUTOMATION_NOT_FOUND" | "AUTOMATION_REVISION_NOT_FOUND" | "AUTOMATION_DISABLED" | "AUTOMATION_NOT_PAUSED" | "OCCURRENCE_NOT_FOUND" | "INVALID_TRIGGER" | "TRIGGER_UNSUPPORTED" | "TRIGGER_HOST_UNAVAILABLE" | "CHANNEL_INGRESS_GAP_CONFIRMATION_REQUIRED" | "ROUTINE_NOT_FOUND" | "ROUTINE_REVISION_NOT_FOUND" | "ROUTINE_ARCHIVED" | "ROUTINE_INPUT_INVALID" | "RUNTIME_STOP_BLOCKED" | "RUNTIME_STARTUP_UNAVAILABLE" | "STALE_CLAIM_EPOCH" | "MISFIRE_LIMIT_EXCEEDED" | "OCCURRENCE_CONFLICT" | "EFFECT_AMBIGUOUS" | "EFFECT_RECONCILIATION_REQUIRED" | "VERIFICATION_FAILED" | "RATE_LIMITED" | "INVOCATION_NOT_FOUND" | "INVOCATION_AMBIGUOUS" | "PROVIDER_INPUT_UNSUPPORTED" | "SENSITIVE_INPUT_UNSUPPORTED" | "USER_REQUEST_NOT_FOUND" | "USER_REQUEST_EXPIRED" | "APPROVAL_ALREADY_CONSUMED" | "RESOURCE_STALE" | "RESOURCE_CONFLICT" | "RESOURCE_LOCATION_UNAVAILABLE" | "RESOURCE_CONTENT_EXTERNAL" | "UPLOAD_OFFSET_CONFLICT" | "UPLOAD_EXPIRED" | "BUDGET_EXCEEDED" | "WORKER_QUALITY_FLOOR_UNMET" | "WORKER_BUDGET_EXCEEDED" | "DEADLINE_EXECUTION_PRECONDITION_FAILED" | "COWORKER_PAUSED" | "COWORKER_ARCHIVED" | "GOAL_ARCHIVED" | "SUGGESTION_EXPIRED" | "TIMEOUT" | "DEPENDENCY_UNAVAILABLE" | "UNSUPPORTED_VERSION" | "INTEGRITY_FAILURE" | "INTERNAL" | "LEAD_FAILOVER_POLICY_INVALID" | "RESOURCE_REVISION_PARENT_MISMATCH" | "CONTEXT_DOCUMENT_NOT_ACTIVE" | "CONTEXT_DOCUMENT_OWNER_SCOPE_MISMATCH" | "DEMONSTRATION_CAPTURE_LIMIT_REACHED" | "COWORKER_HAS_ACTIVE_WORK";
    };
    responses: {
        /** @description Safe typed error */
        Error: {
            headers: {
                /** @description Same identifier as the correlation_id field in the ApiError body. */
                "X-Correlation-ID"?: string;
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["ApiError"];
            };
        };
    };
    parameters: {
        WorkspaceId: string;
        IdempotencyKey: string;
        /** @description Expected aggregate version for optimistic concurrency. */
        IfMatch: string;
        Cursor: string;
        Limit: number;
        ConversationId: string;
        TaskId: string;
    };
    requestBodies: never;
    headers: never;
    pathItems: never;
};
export type $defs = Record<string, never>;
export interface operations {
    listLocalAgentInstallations: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Authenticated runtime-local executable inventory; not negotiated readiness */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AgentInstallationPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listAgentRegistry: {
        parameters: {
            query?: {
                q?: string;
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Bounded cached ACP/distribution registry metadata. Listing is not installation, authentication, capability, readiness, or Workspace authority. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AgentRegistryPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    refreshAgentRegistry: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Refresh the bounded registry cache. Untrusted registry metadata never defines executable install/auth commands. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AgentRegistrySnapshot"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    installRegistryAgent: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path: {
                registryAgentId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["InstallAgentRequest"];
            };
        };
        responses: {
            /** @description Explicit local owner install operation accepted; creates no AgentBinding or work session. */
            202: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AgentLifecycleOperation"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    updateRegistryAgent: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path: {
                registryAgentId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["UpdateAgentRequest"];
            };
        };
        responses: {
            /** @description Explicit local owner update operation accepted; active sessions remain pinned to their admitted descriptors until they settle. */
            202: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AgentLifecycleOperation"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getLocalOperatorReadiness: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Authenticated local API-serving handshake; does not indicate Runtime execution readiness */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["LocalOperatorReadiness"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listWorkspaces: {
        parameters: {
            query?: {
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Workspaces owned by the authenticated principal */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WorkspacePage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    createWorkspace: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CreateWorkspaceRequest"];
            };
        };
        responses: {
            /** @description Workspace created */
            201: {
                headers: {
                    "X-Correlation-ID": string;
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Workspace"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getWorkspace: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                workspaceId: components["parameters"]["WorkspaceId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Workspace owned by the authenticated principal */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Workspace"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    updateWorkspacePolicy: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                workspaceId: components["parameters"]["WorkspaceId"];
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["UpdateWorkspacePolicyRequest"];
            };
        };
        responses: {
            /** @description Policy update applies to future replication transfers */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Workspace"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listWorkspaceBackups: {
        parameters: {
            query?: {
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path: {
                workspaceId: components["parameters"]["WorkspaceId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Integrity-verified immutable Workspace backup manifests */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WorkspaceBackupPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    createWorkspaceBackup: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path: {
                workspaceId: components["parameters"]["WorkspaceId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Complete backup verified and immutable manifest published */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WorkspaceBackupManifest"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getWorkspaceBackup: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                workspaceId: components["parameters"]["WorkspaceId"];
                backupId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Integrity-verified immutable backup manifest */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WorkspaceBackupManifest"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    restoreWorkspaceBackup: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path: {
                backupId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["RestoreWorkspaceBackupRequest"];
            };
        };
        responses: {
            /** @description Backup restored into an empty installation and a new Runtime identity registered */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RestoreReceipt"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    setWorkspaceDefaultAgentBinding: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                workspaceId: components["parameters"]["WorkspaceId"];
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["SetWorkspaceDefaultAgentBindingRequest"];
            };
        };
        responses: {
            /** @description Default used for newly admitted work */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Workspace"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    archiveWorkspace: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                workspaceId: components["parameters"]["WorkspaceId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Quiescent Workspace archived and made read-only */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Workspace"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listWorkspaceRuntimeBindings: {
        parameters: {
            query?: {
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path: {
                workspaceId: components["parameters"]["WorkspaceId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Owner-authorized retained Runtime binding inventory for the selected Workspace */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RuntimeWorkspaceBindingPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getCurrentLocalRuntimeWorkspaceBinding: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                workspaceId: components["parameters"]["WorkspaceId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Zero or one active enrollment for the exact current local Runtime incarnation. This endpoint is desktop-local and is not the complete Workspace Runtime-binding inventory. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RuntimeWorkspaceBindingPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    enrollLocalRuntimeInWorkspace: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                workspaceId: components["parameters"]["WorkspaceId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Local-only RuntimeWorkspaceBinding created; no Mesh pairing implied */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RuntimeWorkspaceBinding"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    revokeWorkspaceRuntimeBinding: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                workspaceId: components["parameters"]["WorkspaceId"];
                bindingId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description This Runtime's access to this Workspace was revoked. RuntimeMesh first drains/fences its channel hosts; if no eligible replacement exists, it atomically clears the released assignment, and the affected ChannelBinding remains DEGRADED with host_assignment null. Re-fetch affected bindings to observe their new projections; assignment history remains in versioned events. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RuntimeWorkspaceBinding"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listConversations: {
        parameters: {
            query?: {
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Newest-first owner-scoped Conversation page for the selected Workspace */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ConversationListPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    createConversation: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CreateConversationRequest"];
            };
        };
        responses: {
            /** @description Conversation created; an exact owner-scoped receipt replays before the active-Workspace check, while a fresh create is rejected after archival */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Conversation"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getConversation: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                conversationId: components["parameters"]["ConversationId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Conversation projection */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Conversation"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getConversationPresentation: {
        parameters: {
            query?: {
                cursor?: components["parameters"]["Cursor"];
                limit?: number;
            };
            header?: never;
            path: {
                conversationId: components["parameters"]["ConversationId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Bounded authenticated semantic-message snapshot with optional rich-presentation refs and stream cursor */
            200: {
                headers: {
                    "Cache-Control"?: "no-store";
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ConversationPresentationSnapshot"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getRichPresentation: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                presentationId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Exact authorized immutable RichPresentation document; verify digest and message binding before render */
            200: {
                headers: {
                    "Cache-Control"?: "no-store";
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RichPresentationDocumentResponse"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listConversationMessages: {
        parameters: {
            query?: {
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path: {
                conversationId: components["parameters"]["ConversationId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Ordered message page */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["MessagePage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    appendConversationMessage: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path: {
                conversationId: components["parameters"]["ConversationId"];
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["AppendMessageRequest"];
            };
        };
        responses: {
            /** @description Message appended */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ConversationMessage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listConversationTasks: {
        parameters: {
            query?: {
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path: {
                conversationId: components["parameters"]["ConversationId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Tasks associated with the Conversation */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TaskPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    submitConversationTurn: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path: {
                conversationId: components["parameters"]["ConversationId"];
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["SubmitConversationTurnRequest"];
            };
        };
        responses: {
            /** @description Durable user turn accepted; agent reply is streamed separately */
            202: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ConversationTurnReceipt"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    retryConversationTurn: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                conversationId: components["parameters"]["ConversationId"];
                turnId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Failed turn retried in a new Conversation AgentSession */
            202: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ConversationTurnReceipt"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    cancelConversationTurn: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                conversationId: components["parameters"]["ConversationId"];
                turnId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Turn cancellation requested; response confirms after stop or a racing terminal outcome */
            202: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ConversationTurnReceipt"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listTasks: {
        parameters: {
            query?: {
                status?: "READY" | "RUNNING" | "WAITING_USER" | "BLOCKED" | "VERIFYING" | "NEEDS_USER" | "INCOMPLETE" | "PAUSE_REQUESTED" | "PAUSED" | "COMPLETED" | "FAILED" | "CANCEL_REQUESTED" | "CANCELLED";
                conversation_id?: string;
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Task summary page */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TaskPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    createTask: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CreateTaskRequest"];
            };
        };
        responses: {
            /** @description Task and initial TaskSpecRevision committed */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TaskView"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getTask: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                taskId: components["parameters"]["TaskId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Current Task projection and pinned current TaskSpecRevision */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TaskView"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getTaskPlanningReadiness: {
        parameters: {
            query?: never;
            header: {
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                taskId: components["parameters"]["TaskId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Sanitized readiness blockers observed for this Task version; dispatch remains unavailable in this desktop slice */
            200: {
                headers: {
                    "Cache-Control"?: "no-store";
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TaskPlanningReadiness"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getTaskPresentation: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                taskId: components["parameters"]["TaskId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Bounded read-only snapshot of committed Task/current spec, current-plan Steps, Task Artifacts, and resolvable current ArtifactVersions read in one SQLite transaction. Caps are 100 Steps and 200 Artifacts; over-cap sources return an error without a partial snapshot. CURRENT freshness means only one consistent SQLite snapshot, not worker liveness, progress, verification, or external-provider freshness. Unresolvable ArtifactVersions are omitted. */
            200: {
                headers: {
                    "Cache-Control"?: "no-store";
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TaskPresentationSnapshot"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getTaskProgress: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                taskId: components["parameters"]["TaskId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Rebuildable factual progress projection; no synthetic percentage or ETA */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TaskProgressProjection"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getStepExecutionDependencies: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                taskId: components["parameters"]["TaskId"];
                stepId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Short-lived placement and prerequisite preview; starts no workers or providers */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ExecutionDependencyPlan"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    steerTask: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                taskId: components["parameters"]["TaskId"];
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["SteerTaskRequest"];
            };
        };
        responses: {
            /** @description Steering accepted for Task Runtime processing */
            202: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CommandReceipt"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    changeTaskLeadAgent: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                taskId: components["parameters"]["TaskId"];
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ChangeLeadAgentRequest"];
            };
        };
        responses: {
            /** @description Lead replacement accepted; active Attempts drain under lease and Effect rules */
            202: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CommandReceipt"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    cancelTask: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                taskId: components["parameters"]["TaskId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Cancellation requested; Task may remain stopping while Attempts, provider Invocations, VerificationRuns, pending Task-scoped UserRequests/Approvals, and Effects settle. Cancellation is valid from PAUSED and supersedes PAUSE_REQUESTED. */
            202: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CommandReceipt"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    pauseTask: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                taskId: components["parameters"]["TaskId"];
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["PauseTaskRequest"];
            };
        };
        responses: {
            /** @description Pause coordination accepted; Task remains PAUSE_REQUESTED until checkpoint, provider Invocation and VerificationRun settlement, Effect reconciliation, and lease settlement finish */
            202: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CommandReceipt"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    resumeTask: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                taskId: components["parameters"]["TaskId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Resume accepted after input, policy, and Effect revalidation */
            202: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CommandReceipt"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    recoverTaskStep: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                taskId: components["parameters"]["TaskId"];
                stepId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["RecoverStepRequest"];
            };
        };
        responses: {
            /** @description Step recovery accepted after budget and Effect checks */
            202: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CommandReceipt"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listTaskSpecRevisions: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                taskId: components["parameters"]["TaskId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Immutable TaskSpecRevision history */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TaskSpecRevision"][];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    reviseTaskSpec: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                taskId: components["parameters"]["TaskId"];
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ReviseTaskSpecRequest"];
            };
        };
        responses: {
            /** @description New immutable TaskSpecRevision */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TaskSpecRevision"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listPlanRevisions: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                taskId: components["parameters"]["TaskId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Immutable PlanRevision history */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PlanRevision"][];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    submitTaskPlan: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                taskId: components["parameters"]["TaskId"];
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["SubmitPlanRequest"];
            };
        };
        responses: {
            /** @description PlanRevision and its materialized Steps committed atomically */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PlanAcceptance"];
                };
            };
            /** @description Stale Task/spec, producer no longer authorized, or idempotency-key reuse with different request digest */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiError"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listTaskSteps: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                taskId: components["parameters"]["TaskId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Task Steps */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Step"][];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listTaskAttempts: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                taskId: components["parameters"]["TaskId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Task Attempts */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Attempt"][];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getTaskTimeline: {
        parameters: {
            query?: {
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path: {
                taskId: components["parameters"]["TaskId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Projected Task timeline */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TimelinePage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listTaskArtifacts: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                taskId: components["parameters"]["TaskId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Artifact summaries */
            200: {
                headers: {
                    "Cache-Control": "no-store";
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Artifact"][];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listTaskEffects: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                taskId: components["parameters"]["TaskId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Effect summaries with evidence level */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Effect"][];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listApprovals: {
        parameters: {
            query?: {
                status?: "PENDING" | "APPROVED" | "DENIED" | "EXPIRED" | "CANCELLED";
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Approval page */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApprovalPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getApproval: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                approvalId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Approval bound to an exact target and action digest */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Approval"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    resolveApproval: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                approvalId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ResolveApprovalRequest"];
            };
        };
        responses: {
            /** @description Approval decision recorded */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Approval"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listArtifacts: {
        parameters: {
            query?: {
                library_status?: "TRANSIENT" | "SAVED" | "ARCHIVED";
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Authorized Artifacts filtered by Library status; response is no-store */
            200: {
                headers: {
                    "Cache-Control": "no-store";
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ArtifactPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getArtifact: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                artifactId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Artifact metadata; response is no-store */
            200: {
                headers: {
                    "Cache-Control": "no-store";
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Artifact"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getArtifactTextEditHead: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                artifactId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Exact immutable heads required for a bounded text edit; response is no-store */
            200: {
                headers: {
                    "Cache-Control": "no-store";
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ArtifactTextEditHead"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    appendArtifactTextVersion: {
        parameters: {
            query?: never;
            header: {
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path: {
                artifactId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["AppendArtifactTextVersionRequest"];
            };
        };
        responses: {
            /** @description Idempotent replay of the original committed result */
            200: {
                headers: {
                    "Cache-Control": "no-store";
                    "X-Correlation-ID": string;
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ArtifactTextVersionAppendResponse"];
                };
            };
            /** @description New immutable Artifact and Resource versions committed */
            201: {
                headers: {
                    "Cache-Control": "no-store";
                    "X-Correlation-ID": string;
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ArtifactTextVersionAppendResponse"];
                };
            };
            /** @description Artifact or Resource head changed, or Artifact is archived */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiError"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getArtifactVersion: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                artifactId: string;
                version: number;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Immutable artifact version metadata; response is no-store */
            200: {
                headers: {
                    "Cache-Control": "no-store";
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ArtifactVersion"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    downloadArtifactVersion: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                artifactId: string;
                version: number;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Immutable artifact bytes; Content-Type is the stored media type and response is no-store */
            200: {
                headers: {
                    "Cache-Control": "no-store";
                    [name: string]: unknown;
                };
                content: {
                    "application/octet-stream": string;
                };
            };
            default: components["responses"]["Error"];
        };
    };
    promoteArtifact: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                artifactId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Artifact promoted into the user Library */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Artifact"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    archiveArtifact: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                artifactId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Saved Artifact archived and removed from the default Library projection */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Artifact"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listLibrary: {
        parameters: {
            query?: {
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Library page; response is no-store */
            200: {
                headers: {
                    "Cache-Control": "no-store";
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ArtifactPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listRoutines: {
        parameters: {
            query?: {
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Workspace Routines */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RoutinePage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    createRoutine: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CreateRoutineRequest"];
            };
        };
        responses: {
            /** @description Routine and initial immutable revision created */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Routine"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getRoutine: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                routineId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Routine metadata and current revision number; retrieve content from revision history */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Routine"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listRoutineRevisions: {
        parameters: {
            query?: {
                cursor?: components["parameters"]["Cursor"];
            };
            header?: never;
            path: {
                routineId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Immutable RoutineRevision history */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RoutineRevisionPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    reviseRoutine: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                routineId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CreateRoutineRevisionRequest"];
            };
        };
        responses: {
            /** @description New immutable RoutineRevision created */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RoutineRevision"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getRoutineHealth: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                routineId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Rebuildable health projection for the current immutable RoutineRevision */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RoutineHealth"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    runRoutine: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path: {
                routineId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["RunRoutineRequest"];
            };
        };
        responses: {
            /** @description Ordinary READY Task saved from the exact active RoutineRevision; no planning or execution is started */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TaskView"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    archiveRoutine: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                routineId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Routine archived after active Automations are handled */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Routine"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listAutomations: {
        parameters: {
            query?: {
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Automation page */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AutomationPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    createAutomation: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CreateAutomationRequest"];
            };
        };
        responses: {
            /** @description Automation and immutable revision 1 created */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Automation"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getAutomation: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                automationId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Automation definition */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Automation"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    updateAutomation: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                automationId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["UpdateAutomationRequest"];
            };
        };
        responses: {
            /** @description Automation updated */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Automation"];
                };
            };
            /** @description Automation is enabled, disabled, stale, or has a conflicting idempotency key */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiError"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    runAutomationNow: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                automationId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ManualAutomationRunRequest"];
            };
        };
        responses: {
            /** @description Exact idempotency receipt replay; the original Manual occurrence and READY Task */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ManualAutomationRunResponse"];
                };
            };
            /** @description Manual occurrence and READY Task atomically admitted; planning has not started */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ManualAutomationRunResponse"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listAutomationRevisions: {
        parameters: {
            query?: {
                cursor?: components["parameters"]["Cursor"];
            };
            header?: never;
            path: {
                automationId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Immutable AutomationRevision history in descending revision order; the first page contains the current revision for safe edit reconstruction */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AutomationRevisionPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listAutomationOccurrences: {
        parameters: {
            query?: {
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path: {
                automationId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Logical occurrences pinned to immutable AutomationRevisions */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AutomationOccurrencePage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    pauseAutomation: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                automationId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Automation paused */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Automation"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    resumeAutomation: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                automationId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Automation resumed */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Automation"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    disableAutomation: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                automationId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Automation disabled permanently */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Automation"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getLocalRuntimeLifecycle: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Local daemon startup policy, incarnation, and recovery state */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RuntimeLifecycleView"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    setLocalRuntimeStartupPolicy: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["UpdateRuntimeStartupPolicyRequest"];
            };
        };
        responses: {
            /** @description Startup policy updated for the local daemon service manager */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RuntimeLifecycleView"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    previewLocalRuntimeStop: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Read-only dependency and handoff preview; does not begin drain */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RuntimeStopPreview"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    requestLocalRuntimeStop: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["StopRuntimeRequest"];
            };
        };
        responses: {
            /** @description Safe Runtime drain requested; blockers and dependent work are returned */
            202: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RuntimeDrainReceipt"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listRuntimes: {
        parameters: {
            query?: {
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Runtime descriptors; secrets and private keys are omitted */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RuntimePage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    createRuntimePairToken: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CreatePairingTokenRequest"];
            };
        };
        responses: {
            /** @description Single-use short-lived pairing token; returned once */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PairingToken"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    revokeRuntime: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                runtimeId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Runtime authority revoked */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RuntimeDescriptor"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listRuntimeIncarnations: {
        parameters: {
            query?: {
                cursor?: components["parameters"]["Cursor"];
            };
            header?: never;
            path: {
                runtimeId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Daemon process incarnations for a stable Runtime identity */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RuntimeIncarnationPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getRuntimeOffers: {
        parameters: {
            query?: {
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path: {
                runtimeId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Current, unexpired offer observations visible through the selected Workspace's active Runtime binding; cursor is bound to that Workspace, Runtime, incarnation, and page size */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RuntimeOfferPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listRuntimeCapabilityHosts: {
        parameters: {
            query?: {
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path: {
                runtimeId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Normalized current-incarnation provider host observations and selected-Workspace LiteCowork activation-use counts; cursor binds Workspace, Runtime, incarnation, and page size */
            200: {
                headers: {
                    "Cache-Control": "no-store";
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CapabilityHostInstancePage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listAgentProfiles: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Workspace-visible stable profiles and their current unexpired Runtime offers. This initial local implementation returns the complete bounded catalog and no cursor. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AgentProfilePage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    probeLocalAgentProfile: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ProbeLocalAgentProfileRequest"];
            };
        };
        responses: {
            /** @description Explicit owner-triggered, bounded local adapter observation. Each request performs a new probe and records a refreshed expiring Runtime offer; it is not an idempotent domain mutation. Requires the selected Workspace owner and an active local RuntimeWorkspaceBinding. Codex and OpenCode are accepted. OpenCode currently accepts only qualified CLI 2.0.26 and reads /api/provider plus /api/model; it returns bounded sanitized catalog IDs and display names, never native provider/model objects, options, headers, keys, URLs, raw responses, or location paths. Catalog provider IDs are not connected-provider or authentication evidence. Unsupported versions fail closed without route fallback. OpenCode session-model selection is NOT_QUALIFIED and its offer is always incompatible, so the profile cannot be bound or enabled. Raw native messages, endpoint locators, account identifiers, and private configuration are omitted. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AgentProfileView"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    refreshAgentProfileControl: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                agentProfileId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Refresh the selected module's bounded AgentControlDescriptor and normalized Runtime offer. Requires Workspace owner and active local Runtime enrollment. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AgentControlDescriptor"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getAgentControlDescriptor: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                agentProfileId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Current sanitized time-bounded install/auth/config/session/native-input descriptor; contains no secret bytes or private native config. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AgentControlDescriptor"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    beginAgentAuthentication: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                agentProfileId: string;
                authMethodId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Begin one adapter-declared native/external/device authentication flow. Does not accept secret bytes in this operation. */
            200: {
                headers: {
                    "Cache-Control": "no-store";
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AgentAuthChallenge"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    submitAgentCredential: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                agentProfileId: string;
                credentialSlotId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["AgentCredentialSecretSubmission"];
            };
        };
        responses: {
            /** @description Secret written directly to local SecretStore for an adapter-declared secure slot; response contains only the SecretRef binding. */
            201: {
                headers: {
                    "Cache-Control": "no-store";
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AgentCredentialBinding"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    openAgentNativeConfiguration: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                agentProfileId: string;
                targetId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Open an adapter-known native configuration target. Caller cannot provide an arbitrary path or command. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["NativeConfigOpenReceipt"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listAgentBindings: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Complete current AgentBinding list for the selected Workspace; this initial local implementation returns no cursor. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AgentBindingPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    createAgentBinding: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CreateAgentBindingRequest"];
            };
        };
        responses: {
            /** @description Disabled AgentBinding created from a currently discovered profile */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AgentBinding"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getAgentBinding: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                agentBindingId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description AgentBinding view; auth is represented by SecretRef only */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AgentBinding"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    configureAgentBinding: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                agentBindingId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ConfigureAgentBindingRequest"];
            };
        };
        responses: {
            /** @description Versioned non-secret adapter configuration, credential SecretRef bindings, and default session options saved after descriptor validation. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AgentBinding"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    enableAgentBinding: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                agentBindingId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Binding enabled for new planning/Attempt admission */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AgentBinding"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    disableAgentBinding: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                agentBindingId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Binding disabled for new admission; already admitted sessions remain pinned and settle safely */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AgentBinding"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getAgentSessionOptions: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                agentBindingId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Fresh adapter-negotiated option descriptors; no native configuration or locators */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AgentSessionOptions"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getAgentHarnessCapabilities: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                agentBindingId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Current negotiated, time-bounded harness feature descriptor */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AgentHarnessCapabilities"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getAgentQuotaObservation: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                agentBindingId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Latest time-bounded adapter/provider observation; null means no observation was made */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["QuotaObservation"] | null;
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listDelegationProfiles: {
        parameters: {
            query?: {
                agent_binding_id?: string;
                status?: "ENABLED" | "DISABLED" | "ARCHIVED";
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Workspace worker profiles; only enabled profiles are eligible for selection */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DelegationProfilePage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    createDelegationProfile: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CreateDelegationProfileRequest"];
            };
        };
        responses: {
            /** @description Disabled profile and immutable revision 1 created; no agent host started. The current desktop implementation accepts an empty session_options object until current adapter descriptor validation is integrated. */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DelegationProfile"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getDelegationProfile: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                delegationProfileId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Profile head and pinned current revision */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DelegationProfile"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    duplicateDelegationProfile: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                delegationProfileId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["DuplicateDelegationProfileRequest"];
            };
        };
        responses: {
            /** @description Disabled profile created from the source's current revision; no execution or authority state is copied */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DelegationProfile"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getDelegationProfilePerformance: {
        parameters: {
            query?: {
                task_category?: "SOFTWARE_ENGINEERING" | "RESEARCH" | "WRITING" | "DATA_ANALYSIS" | "OFFICE" | "BROWSER" | "PERSONAL_ADMIN" | "OTHER";
            };
            header?: never;
            path: {
                delegationProfileId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Rebuildable verified outcome projection; small samples are explicitly low confidence */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WorkerPerformanceProjection"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listDelegationProfileRevisions: {
        parameters: {
            query?: {
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path: {
                delegationProfileId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Immutable revisions in descending revision order, including author and creation time */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DelegationProfileRevisionPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    reviseDelegationProfile: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                delegationProfileId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["DelegationProfileRevisionInput"];
            };
        };
        responses: {
            /** @description Immutable profile revision created and made current */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DelegationProfile"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    changeDelegationProfileStatus: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                delegationProfileId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["DelegationProfileStatusRequest"];
            };
        };
        responses: {
            /** @description New admissions use the committed status; admitted Attempts remain pinned. The current desktop implementation rejects ENABLED with DEPENDENCY_UNAVAILABLE until adapter descriptor, Trust, and Environment admission validation are integrated. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DelegationProfile"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listCoworkers: {
        parameters: {
            query?: {
                status?: "ACTIVE" | "PAUSED" | "ARCHIVED";
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Coworkers with current Workspace primary marker */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CoworkerPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    createCoworker: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CreateCoworkerRequest"];
            };
        };
        responses: {
            /** @description Coworker and immutable revision 1 created */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Coworker"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getCoworker: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                coworkerId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Coworker current view */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Coworker"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getCoworkerRevision: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                coworkerId: string;
                revision: number;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Exact immutable Coworker revision in the selected Workspace; never resolves the current revision implicitly */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CoworkerRevisionRecord"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getCoworkerPresence: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                coworkerId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Rebuildable activity, proactive-status, and Runtime-availability projection */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CoworkerPresenceProjection"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    reviseCoworker: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                coworkerId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CoworkerRevisionInput"];
            };
        };
        responses: {
            /** @description Immutable revision created and made current */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Coworker"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    changeCoworkerStatus: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                coworkerId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CoworkerStatusRequest"];
            };
        };
        responses: {
            /** @description Status transition committed */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Coworker"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    setPrimaryCoworker: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                workspaceId: components["parameters"]["WorkspaceId"];
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["SetPrimaryCoworkerRequest"];
            };
        };
        responses: {
            /** @description Workspace primary Coworker changed or explicitly cleared */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Workspace"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listGoals: {
        parameters: {
            query?: {
                coworker_id?: string;
                status?: "ACTIVE" | "PAUSED" | "COMPLETED" | "ARCHIVED";
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Goal heads and immutable current revisions with read-only Task/Evidence projections; unavailable verification and freshness dimensions are explicit. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["GoalPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    createGoal: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CreateGoalRequest"];
            };
        };
        responses: {
            /** @description Goal and immutable revision 1 created; no Task started */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Goal"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getGoal: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                goalId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Goal and immutable current revision with a read-only Task/Evidence projection; unavailable verification and freshness dimensions are explicit. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Goal"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    reviseGoal: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                goalId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["GoalRevisionInput"];
            };
        };
        responses: {
            /** @description Immutable Goal revision created and made current */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Goal"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    changeGoalStatus: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                goalId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["GoalStatusRequest"];
            };
        };
        responses: {
            /** @description Owner-requested Goal state transition */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Goal"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listSuggestions: {
        parameters: {
            query?: {
                status?: "PROPOSED" | "ACCEPTED" | "DISMISSED" | "EXPIRED";
                /** @description Applies to PROPOSED items; other statuses are history */
                visibility?: "VISIBLE" | "SNOOZED" | "ALL";
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Snoozed proposals remain addressable through visibility=SNOOZED or ALL; no action is run by listing */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SuggestionPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    resolveSuggestion: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                suggestionId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ResolveSuggestionRequest"];
            };
        };
        responses: {
            /** @description Owner dismissal commits the terminal Suggestion state; this endpoint does not create a Task */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Suggestion"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    acceptSuggestionTask: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                suggestionId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["AcceptSuggestionTaskRequest"];
            };
        };
        responses: {
            /** @description Idempotent retry returns the original Suggestion and READY Task link */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SuggestionTaskAcceptanceReceipt"];
                };
            };
            /** @description Task and Suggestion resolution committed atomically; Task remains READY */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SuggestionTaskAcceptanceReceipt"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    snoozeSuggestion: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                suggestionId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["SnoozeSuggestionRequest"];
            };
        };
        responses: {
            /** @description Suggestion remains proposed; a timestamp snoozes it until then or expiry and null clears snooze */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Suggestion"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listSuggestionPreferences: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                workspaceId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Workspace-scoped mute preferences for every kind; never-mutated defaults return version 0 and updated_at null */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SuggestionPreferencePage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    setSuggestionPreference: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": string;
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                workspaceId: string;
                kind: "TASK_OPPORTUNITY" | "ROUTINE_OPPORTUNITY" | "AUTOMATION_OPPORTUNITY";
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["SetSuggestionPreferenceRequest"];
            };
        };
        responses: {
            /** @description Preference and dismissal of all currently proposed Suggestions of this kind commit atomically; If-Match 0 selects an absent virtual default */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SuggestionPreference"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    startDemonstration: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["StartDemonstrationRequest"];
            };
        };
        responses: {
            /** @description User-consented semantic capture started in the selected Environment */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DemonstrationSession"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    completeDemonstration: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                demonstrationId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Capture finalized and converted to a reviewable SkillProposal */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DemonstrationSession"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    pauseDemonstration: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                demonstrationId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Capture is paused; no further observations are appended until explicit resume. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DemonstrationSession"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    resumeDemonstration: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                demonstrationId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Capture resumes after consent, Environment, and remaining limits are revalidated. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DemonstrationSession"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    abortDemonstration: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                demonstrationId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Capture stopped; partial trace follows Resource retention policy */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DemonstrationSession"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listConnections: {
        parameters: {
            query?: {
                status?: "CONNECTING" | "CONNECTED" | "DEGRADED" | "REAUTH_REQUIRED" | "DISCONNECTED";
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Normalized connection metadata, without credentials */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ConnectionPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getConnection: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                connectionId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Normalized connection metadata */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Connection"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    disconnectConnection: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                connectionId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Connection use stopped; provider account and historical records are preserved */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Connection"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listChannelBindings: {
        parameters: {
            query?: {
                status?: "ACTIVE" | "DEGRADED" | "REVOKED";
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Channel identity bindings and their current authority */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ChannelBindingPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getChannelBinding: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                channelBindingId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Channel binding projection */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ChannelBinding"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    updateChannelBindingActions: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                channelBindingId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["UpdateChannelBindingActionsRequest"];
            };
        };
        responses: {
            /** @description Allowed channel actions updated after TrustService authorization */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ChannelBinding"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    revokeChannelBinding: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                channelBindingId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Binding revoked; inbound commands are rejected and history is preserved */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ChannelBinding"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    reassignChannelHost: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                channelBindingId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ReassignChannelHostRequest"];
            };
        };
        responses: {
            /** @description Version-fenced explicit assignment to the requested eligible Runtime. If-Match must contain the current ChannelBinding aggregate version, and expected_assignment_version must match the current assignment version or be null when the binding is unassigned. The response is the current ChannelBinding projection. This route does not expose arbitrary clear/delete; RuntimeMesh performs a safe clear internally during RuntimeWorkspaceBinding revocation when no target is eligible, leaving a DEGRADED binding with host_assignment null. Assignment-change events retain history. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ChannelBinding"];
                };
            };
            /** @description The ChannelBinding If-Match version or expected_assignment_version is stale; reload the binding and retry against the current nullable assignment state. */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiError"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    searchDiscover: {
        parameters: {
            query: {
                q: string;
                kind?: "APP" | "CAPABILITY" | "SKILL" | "AGENT";
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Bounded user-facing candidates; package details remain LiteSPM-owned */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DiscoverItemPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    describeDiscoverItem: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                itemId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Normalized user-facing capability/agent description */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DiscoverItem"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    connectOrInstallCapability: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path: {
                capabilityId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ConnectCapabilityRequest"];
            };
        };
        responses: {
            /** @description Capability resolution/connection accepted; approval or runtime activation may still be required */
            202: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CapabilitySetupReceipt"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    testCapabilityConnection: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path: {
                capabilityId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["TestCapabilityRequest"];
            };
        };
        responses: {
            /** @description Bounded connection health result; no package-manager internals are returned */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CapabilityHealthResult"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getZipIntakeReadiness: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description ZIP intake readiness and current safe behavior */
            200: {
                headers: {
                    /** @example no-store */
                    "Cache-Control"?: string;
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ZipIntakeReadiness"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listLocalResources: {
        parameters: {
            query?: {
                cursor?: components["parameters"]["Cursor"];
                limit?: number;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description A stable Resource catalog page */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ResourceSummaryPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    quickImportResource: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["QuickImportResourceRequest"];
            };
        };
        responses: {
            /** @description Stored Resource and its immutable first revision */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["QuickImportResourceResponse"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    readLocalResourceContent: {
        parameters: {
            query?: {
                /** @description When supplied, selects this exact immutable ResourceRevision. It must belong to this Workspace and Resource; it need not be the current head. */
                revision_id?: string;
                /** @description Positive maximum content size for this read; storage rejects larger revisions before BlobStore access. Defaults to the 10 MiB Operator ceiling. */
                max_bytes?: number;
            };
            header?: never;
            path: {
                resourceId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Verified bytes for the exact immutable ResourceRevision; content is marked for download and is not browser-rendered */
            200: {
                headers: {
                    "X-Resource-Revision-Id": string;
                    "X-Resource-Media-Type": string;
                    "X-Content-Type-Options": "nosniff";
                    "Cache-Control": "no-store";
                    [name: string]: unknown;
                };
                content: {
                    "application/octet-stream": string;
                };
            };
            /** @description Resource or exact revision is not present in the selected Workspace/Resource; no current-head fallback occurs */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiError"];
                };
            };
            /** @description The selected ContextDocument is not ACTIVE; returns CONTEXT_DOCUMENT_NOT_ACTIVE */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiError"];
                };
            };
            /** @description The selected revision exceeds the requested or 10 MiB default read limit; no BlobStore bytes were fetched */
            413: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiError"];
                };
            };
            /** @description Selected revision has no supported local managed content provider; returns RESOURCE_CONTENT_EXTERNAL */
            422: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiError"];
                };
            };
            /** @description BlobStore content is unavailable; after a failed read, the daemon returns this only when a ContextDocument status transition cannot be proven */
            503: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiError"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    createResourceUpload: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CreateResourceUploadRequest"];
            };
        };
        responses: {
            /** @description Bounded temporary upload session */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ResourceUploadSession"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getResourceUpload: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                uploadId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Current resumable upload progress */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ResourceUploadSession"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    uploadResourceChunk: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                "Content-Range": string;
                "X-Chunk-SHA256": string;
            };
            path: {
                uploadId: string;
                chunkIndex: number;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/octet-stream": string;
            };
        };
        responses: {
            /** @description Idempotent content chunk accepted; conflicting replay returns UPLOAD_OFFSET_CONFLICT */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            default: components["responses"]["Error"];
        };
    };
    commitResourceUpload: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path: {
                uploadId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Digest-verified Resource and pinned revision created */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CommittedResource"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    createResourceRevisionUpload: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                resourceId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CreateResourceRevisionUploadRequest"];
            };
        };
        responses: {
            /** @description Bounded upload session for a new immutable revision; parent heads are checked before accepting chunks. */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ResourceUploadSession"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    setContextDocumentStatus: {
        parameters: {
            query?: never;
            header: {
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path: {
                resourceId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["SetContextDocumentStatusRequest"];
            };
        };
        responses: {
            /** @description Owner revocation or restoration committed with the Resource status event, snapshot, and idempotency receipt. Only ACTIVE and REVOKED are currently accepted; this does not purge content or invalidate sessions that already received it. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Resource"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getContextDocumentDeletionStatus: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                resourceId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Current deletion/tombstone projection; content bytes and provider results are never included. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ContextDocumentDeletionStatus"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    searchResources: {
        parameters: {
            query?: {
                /** @description Optional and bounded to 256 UTF-8 bytes. METADATA may omit or use an empty q to browse using kind/freshness filters only; METADATA and ON_DEMAND_CONTENT otherwise use a literal case-insensitive substring. INDEXED_CONTENT requires 1 to 32 distinct Unicode alphanumeric terms (128 normalized Unicode characters maximum per term) and AND-matches exact normalized tokens. Content modes reject an empty query. */
                q?: string;
                /** @description Optional exact Resource kind. */
                kind?: "FILE" | "FOLDER" | "ARTIFACT" | "CONNECTOR_OBJECT" | "WEB_RESOURCE" | "OTHER";
                /** @description METADATA searches metadata. ON_DEMAND_CONTENT scans at most 20 current managed candidates, 1 MiB per Resource and 8 MiB total per request. INDEXED_CONTENT requires a non-empty query of at most 32 distinct terms and matches all terms in the local encrypted index; source parsing is capped at 20,000 normalized terms of at most 128 characters each. */
                mode?: "METADATA" | "ON_DEMAND_CONTENT" | "INDEXED_CONTENT";
                /** @description The local managed-resource search supports CURRENT, STALE, UNKNOWN, and UNAVAILABLE. Conflicted revisions are not searchable in this implementation slice. */
                freshness?: "CURRENT" | "STALE" | "UNKNOWN" | "UNAVAILABLE";
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Deterministic Workspace-authorized results with pinned ResourceRefs. ON_DEMAND_CONTENT snippets are generated in request memory. INDEXED_CONTENT snippets are recovered from an encrypted, digest-verified local projection for the exact current ResourceRevision; only keyed term tokens and encrypted text snapshots are persisted. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ResourceSearchPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    rebuildResourceTextIndex: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path: {
                resourceId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["RebuildResourceTextIndexRequest"];
            };
        };
        responses: {
            /** @description Exact idempotent outcome for the pinned Resource revision */
            200: {
                headers: {
                    "X-Correlation-ID": string;
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ResourceTextIndexRebuildResult"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getResource: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                resourceId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Stable Resource metadata */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Resource"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listResourceLocations: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                resourceId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Authorized location and freshness projections */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ResourceLocationView"][];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listResourceRevisions: {
        parameters: {
            query?: {
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path: {
                resourceId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Immutable revision ancestry with derived head markers, in bounded append order (parents are inserted before children); cursor pagination is Workspace/Resource scoped and keyset based. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ResourceRevisionPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listWorkspaceRoots: {
        parameters: {
            query?: {
                status?: string;
                cursor?: components["parameters"]["Cursor"];
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Owner-scoped persistent roots with last committed ResourceLocation availability (not a live Runtime probe); response is no-store and includes no private locator or identity binding. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WorkspaceRootPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    addWorkspaceRoot: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["AddWorkspaceRootRequest"];
            };
        };
        responses: {
            /** @description Explicit persistent root grant created */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WorkspaceRoot"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    updateWorkspaceRoot: {
        parameters: {
            query?: never;
            header: {
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path: {
                rootId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["UpdateWorkspaceRootRequest"];
            };
        };
        responses: {
            /** @description WorkspaceRoot state updated */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WorkspaceRoot"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    pauseWorkspaceRoot: {
        parameters: {
            query?: never;
            header: {
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path: {
                rootId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description WorkspaceRoot paused */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WorkspaceRoot"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    resumeWorkspaceRoot: {
        parameters: {
            query?: never;
            header: {
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path: {
                rootId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description WorkspaceRoot resumed after fresh directory identity validation and current-incarnation binding refresh */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WorkspaceRoot"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    revokeWorkspaceRoot: {
        parameters: {
            query?: never;
            header: {
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path: {
                rootId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description WorkspaceRoot permanently revoked */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WorkspaceRoot"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listUserRequests: {
        parameters: {
            query?: {
                status?: string;
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Structured pending user decisions and questions */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["UserRequestPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    respondToUserRequest: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                requestId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["UserRequestResponseInput"];
            };
        };
        responses: {
            /** @description Response stored as immutable user input; Task-scoped responses received while PAUSED are queued until explicit resume, while PAUSE_REQUESTED returns CONFLICT */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["UserRequestResponse"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    openUserRequestExternalHandoff: {
        parameters: {
            query?: never;
            header: {
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                requestId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Confidential transient browser handoff data returned only for this explicit action */
            200: {
                headers: {
                    "Cache-Control": "no-store";
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ExternalAuthHandoff"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    takeEnvironmentControl: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path: {
                taskId: components["parameters"]["TaskId"];
                attemptId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["TakeEnvironmentControlRequest"];
            };
        };
        responses: {
            /** @description Human input-control epoch acquired after queued agent commands are fenced */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EnvironmentControlLeaseView"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    returnEnvironmentControl: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path: {
                taskId: components["parameters"]["TaskId"];
                attemptId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ReturnEnvironmentControlRequest"];
            };
        };
        responses: {
            /** @description Fresh observation/reconciliation required before a new agent control epoch */
            202: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CommandReceipt"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listWorkspaceInstructionRevisions: {
        parameters: {
            query?: {
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path: {
                workspaceId: components["parameters"]["WorkspaceId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Immutable instruction history */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WorkspaceInstructionRevisionPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    createWorkspaceInstructionRevision: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                workspaceId: components["parameters"]["WorkspaceId"];
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CreateWorkspaceInstructionRevisionRequest"];
            };
        };
        responses: {
            /** @description New immutable instructions revision */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WorkspaceInstructionRevision"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listWorkspaceEnvironments: {
        parameters: {
            query?: {
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path: {
                workspaceId: components["parameters"]["WorkspaceId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Workspace-owned Environments without provider-private locators */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EnvironmentPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    createPersistentWorkspaceEnvironment: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path: {
                workspaceId: components["parameters"]["WorkspaceId"];
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CreatePersistentEnvironmentRequest"];
            };
        };
        responses: {
            /** @description Authorized persistent Environment provisioning accepted */
            202: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EnvironmentView"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    previewPersistentEnvironmentProvision: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                workspaceId: components["parameters"]["WorkspaceId"];
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["EnvironmentProvisionPreviewRequest"];
            };
        };
        responses: {
            /** @description Read-only eligible placement, dependencies, cost estimate, and budget enforcement preview */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EnvironmentProvisionPreview"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getWorkspaceEnvironment: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                workspaceId: components["parameters"]["WorkspaceId"];
                environmentId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Environment state and safe provider status */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EnvironmentView"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    suspendWorkspaceEnvironment: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                workspaceId: components["parameters"]["WorkspaceId"];
                environmentId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Suspension accepted after active use and Effects are reconciled */
            202: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EnvironmentView"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    resumeWorkspaceEnvironment: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                workspaceId: components["parameters"]["WorkspaceId"];
                environmentId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Provider resume and health revalidation accepted; no prior authority is restored */
            202: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EnvironmentView"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    changeWorkspaceEnvironmentSharingScope: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                workspaceId: components["parameters"]["WorkspaceId"];
                environmentId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ChangeEnvironmentSharingScopeRequest"];
            };
        };
        responses: {
            /** @description Suspended persistent Environment scope changed after use/Effect reconciliation */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EnvironmentView"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    destroyWorkspaceEnvironment: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                workspaceId: components["parameters"]["WorkspaceId"];
                environmentId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["DestroyEnvironmentRequest"];
            };
        };
        responses: {
            /** @description Destruction accepted after use, checkpoint, and Effect holds are cleared */
            202: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EnvironmentView"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listNeedsYou: {
        parameters: {
            query?: {
                status?: "OPEN" | "RESOLVED";
                kind?: "APPROVAL" | "USER_REQUEST" | "TASK_BLOCKER";
                task_id?: string;
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Deduplicated authorized approvals, user requests, and actionable blockers */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["NeedsYouPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getUserRequest: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                requestId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Structured request and current status */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["UserRequest"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    dismissUserRequest: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                requestId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Request dismissed */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["UserRequest"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listNotifications: {
        parameters: {
            query?: {
                status?: "PENDING" | "SENDING" | "SENT" | "FAILED" | "AMBIGUOUS" | "SUPPRESSED";
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Workspace notification delivery history */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["NotificationPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listNotificationPreferences: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Workspace notification policies */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["NotificationPreference"][];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    setNotificationPreference: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                eventClass: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["UpdateNotificationPreferenceRequest"];
            };
        };
        responses: {
            /** @description Versioned preference saved */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["NotificationPreference"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listSkillProposals: {
        parameters: {
            query?: {
                status?: string;
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Reviewed Skill drafts */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SkillProposalPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    createSkillProposal: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CreateSkillProposalRequest"];
            };
        };
        responses: {
            /** @description Draft created and queued for redaction/review */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SkillProposal"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    approveSkillProposal: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                proposalId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description User approval recorded; publication remains LiteSPM-owned */
            202: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SkillProposal"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    rejectSkillProposal: {
        parameters: {
            query?: never;
            header: {
                "Idempotency-Key": components["parameters"]["IdempotencyKey"];
                /** @description Expected aggregate version for optimistic concurrency. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                proposalId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Proposal rejected */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SkillProposal"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listTaskCapabilityInvocations: {
        parameters: {
            query?: {
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path: {
                taskId: components["parameters"]["TaskId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Durable capability call history */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CapabilityInvocationPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getCapabilityInvocation: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                invocationId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Durable Invocation state and result references */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CapabilityInvocation"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listTaskUsage: {
        parameters: {
            query?: {
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path: {
                taskId: components["parameters"]["TaskId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Normalized usage with source and confidence */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["UsageObservation"][];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listResourceDependents: {
        parameters: {
            query?: {
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path: {
                resourceId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Exact input dependencies, dependent freshness, and invalidation history */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ResourceDependencyPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listDependencyInvalidations: {
        parameters: {
            query?: {
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path: {
                dependencyEdgeId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Paginated immutable invalidation history for this exact dependency edge */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["InvalidationPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listTaskVerificationRuns: {
        parameters: {
            query?: {
                cursor?: components["parameters"]["Cursor"];
                limit?: components["parameters"]["Limit"];
            };
            header?: never;
            path: {
                taskId: components["parameters"]["TaskId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Verification runs pinned to criteria and input digests */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["VerificationRunPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
}
