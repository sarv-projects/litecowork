//! Coworker persistence through the existing bounded SQLite writer.
use super::*;
use domain_responsibility::*;
use serde::{Deserialize, Serialize};

pub(super) type WriterOperation = Box<dyn FnOnce(&mut Connection) + Send>;
type Decision = Box<
    dyn Fn(&mut dyn ResponsibilityTransaction) -> Result<CommittedResponsibility, DomainError>
        + Send,
>;

/// Runtime-generated event context, separate from owner request data. Each service
/// instance is constructed for one command; retries may use fresh context because
/// context is deliberately outside the request fingerprint.
#[derive(Clone, Debug)]
pub struct CoworkerEventContext {
    pub event_id: String,
    pub origin_runtime_id: String,
    pub hlc_timestamp: String,
    pub correlation_id: String,
    pub causation_id: Option<String>,
    pub recorded_at: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CoworkerPage {
    pub items: Vec<(Coworker, CoworkerRevision)>,
    /// Cursor is the final returned (updated_at, coworker_id), ordered descending.
    pub next: Option<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AutomationPage {
    pub items: Vec<(Automation, AutomationRevision)>,
    pub next: Option<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AutomationRevisionPage {
    /// Immutable revisions in descending order, so the first page contains the current head.
    pub items: Vec<AutomationRevision>,
    pub next: Option<u64>,
}

#[derive(Clone)]
pub struct SqliteCoworkerStore {
    pub(super) store: SqliteWorkspaceStore,
    pub(super) context: CoworkerEventContext,
}
impl SqliteCoworkerStore {
    pub fn new(
        store: SqliteWorkspaceStore,
        mut context: CoworkerEventContext,
    ) -> Result<Self, DomainError> {
        if [
            &context.event_id,
            &context.origin_runtime_id,
            &context.hlc_timestamp,
            &context.correlation_id,
        ]
        .iter()
        .any(|s| s.trim().is_empty())
        {
            return Err(DomainError::InvalidDefinition);
        }
        context.recorded_at =
            canonicalize_utc_timestamp(&context.recorded_at).map_err(domain_error)?;
        Ok(Self { store, context })
    }

    pub(super) fn run<T, F>(&self, operation: F) -> Result<T, DomainError>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> Result<T, DomainError> + Send + 'static,
    {
        let (reply, receive) = mpsc::channel();
        self.store
            .execute_command(
                Command::CoworkerOperation {
                    operation: Box::new(move |connection| {
                        let _ = reply.send(Ok(operation(connection)));
                    }),
                },
                receive,
            )
            .map_err(domain_error)?
    }

    pub fn get(
        &self,
        principal_id: &str,
        workspace_id: &str,
        coworker_id: &str,
    ) -> Result<Option<(Coworker, CoworkerRevision)>, DomainError> {
        let (principal, workspace, id) = (
            principal_id.to_owned(),
            workspace_id.to_owned(),
            coworker_id.to_owned(),
        );
        self.run(move |connection| {
            // One read snapshot prevents ownership revocation between check/read.
            let tx = connection.transaction().map_err(sql_error)?;
            authorize(&tx, &principal, &workspace)?;
            load_coworker(&tx, &workspace, &id)
        })
    }

    /// Exact immutable revision read for reconstructing an idempotent command
    /// response. Authorization and revision lookup share one SQLite snapshot.
    pub fn get_revision(
        &self,
        principal_id: &str,
        workspace_id: &str,
        coworker_id: &str,
        revision: u64,
    ) -> Result<Option<CoworkerRevision>, DomainError> {
        if revision == 0 {
            return Err(DomainError::InvalidDefinition);
        }
        let (principal, workspace, id) = (
            principal_id.to_owned(),
            workspace_id.to_owned(),
            coworker_id.to_owned(),
        );
        self.run(move |connection| {
            let tx = connection.transaction().map_err(sql_error)?;
            authorize(&tx, &principal, &workspace)?;
            load_coworker_revision(&tx, &workspace, &id, revision)
        })
    }

    pub fn list(
        &self,
        principal_id: &str,
        workspace_id: &str,
        status: Option<CoworkerStatus>,
        after: Option<(String, String)>,
        limit: usize,
    ) -> Result<CoworkerPage, DomainError> {
        if !(1..=200).contains(&limit)
            || after
                .as_ref()
                .is_some_and(|(time, id)| time.is_empty() || id.is_empty())
        {
            return Err(DomainError::InvalidDefinition);
        }
        let (principal, workspace) = (principal_id.to_owned(), workspace_id.to_owned());
        let after = after
            .map(|(time, id)| canonicalize_utc_timestamp(&time).map(|time| (time, id)))
            .transpose()
            .map_err(domain_error)?;
        self.run(move |connection| {
            let tx = connection.transaction().map_err(sql_error)?;
            authorize(&tx, &principal, &workspace)?;
            let mut stmt = tx.prepare("SELECT coworker_id FROM coworkers WHERE workspace_id = ?1 AND (?2 IS NULL OR status = ?2) AND (?3 IS NULL OR updated_at < ?3 OR (updated_at = ?3 AND coworker_id < ?4)) ORDER BY updated_at DESC, coworker_id DESC LIMIT ?5").map_err(sql_error)?;
            let ids = stmt.query_map(params![workspace, status.map(coworker_status), after.as_ref().map(|c| c.0.as_str()), after.as_ref().map(|c| c.1.as_str()), (limit + 1) as i64], |row| row.get::<_, String>(0)).map_err(sql_error)?
                .collect::<rusqlite::Result<Vec<_>>>().map_err(sql_error)?;
            let more = ids.len() > limit;
            let mut items = Vec::new();
            for id in ids.into_iter().take(limit) { items.push(load_coworker(&tx, &workspace, &id)?.ok_or(DomainError::Storage)?); }
            let next = if more { items.last().map(|(head, _)| (head.updated_at.clone(), head.coworker_id.clone())) } else { None };
            Ok(CoworkerPage { items, next })
        })
    }

    pub fn get_automation(
        &self,
        principal_id: &str,
        workspace_id: &str,
        automation_id: &str,
    ) -> Result<Option<(Automation, AutomationRevision)>, DomainError> {
        let (principal, workspace, id) = (
            principal_id.to_owned(),
            workspace_id.to_owned(),
            automation_id.to_owned(),
        );
        self.run(move |connection| {
            let tx = connection.transaction().map_err(sql_error)?;
            authorize(&tx, &principal, &workspace)?;
            load_automation(&tx, &workspace, &id)
        })
    }

    pub fn get_automation_revision(
        &self,
        principal_id: &str,
        workspace_id: &str,
        automation_id: &str,
        revision: u64,
    ) -> Result<Option<AutomationRevision>, DomainError> {
        if revision == 0 {
            return Err(DomainError::InvalidDefinition);
        }
        let (principal, workspace, id) = (
            principal_id.to_owned(),
            workspace_id.to_owned(),
            automation_id.to_owned(),
        );
        self.run(move |connection| {
            let tx = connection.transaction().map_err(sql_error)?;
            authorize(&tx, &principal, &workspace)?;
            load_automation_revision(&tx, &workspace, &id, revision)
        })
    }

    /// Lists immutable revisions in descending order. The initial page begins at the
    /// current revision, allowing the owner to safely reconstruct an edit form without
    /// scanning unbounded history.
    pub fn list_automation_revisions(
        &self,
        principal_id: &str,
        workspace_id: &str,
        automation_id: &str,
        before_revision: Option<u64>,
        limit: usize,
    ) -> Result<AutomationRevisionPage, DomainError> {
        if !(1..=200).contains(&limit) || before_revision == Some(0) {
            return Err(DomainError::InvalidDefinition);
        }
        let (principal, workspace, id) = (
            principal_id.to_owned(),
            workspace_id.to_owned(),
            automation_id.to_owned(),
        );
        let before_revision = before_revision
            .map(|value| sql_u64(value, "Automation revision"))
            .transpose()?;
        self.run(move |connection| {
            let tx = connection.transaction().map_err(sql_error)?;
            authorize(&tx, &principal, &workspace)?;
            if load_automation(&tx, &workspace, &id)?.is_none() { return Err(DomainError::NotFound); }
            let mut statement = tx.prepare(
                "SELECT revision FROM automation_revisions WHERE workspace_id = ?1 AND automation_id = ?2 AND (?3 IS NULL OR revision < ?3) ORDER BY revision DESC LIMIT ?4"
            ).map_err(sql_error)?;
            let revisions = statement.query_map(
                params![workspace, id, before_revision, (limit + 1) as i64],
                |row| from_row_u64(row, 0),
            ).map_err(sql_error)?.collect::<rusqlite::Result<Vec<_>>>().map_err(sql_error)?;
            let more = revisions.len() > limit;
            let mut items = Vec::with_capacity(revisions.len().min(limit));
            for revision in revisions.into_iter().take(limit) {
                items.push(load_automation_revision(&tx, &workspace, &id, revision)?.ok_or(DomainError::Storage)?);
            }
            let next = if more { items.last().map(|revision| revision.revision) } else { None };
            Ok(AutomationRevisionPage { items, next })
        })
    }

    pub fn list_automations(
        &self,
        principal_id: &str,
        workspace_id: &str,
        status: Option<AutomationStatus>,
        after: Option<(String, String)>,
        limit: usize,
    ) -> Result<AutomationPage, DomainError> {
        if !(1..=200).contains(&limit)
            || after
                .as_ref()
                .is_some_and(|(time, id)| time.is_empty() || id.is_empty())
        {
            return Err(DomainError::InvalidDefinition);
        }
        let (principal, workspace) = (principal_id.to_owned(), workspace_id.to_owned());
        let after = after
            .map(|(time, id)| canonicalize_utc_timestamp(&time).map(|time| (time, id)))
            .transpose()
            .map_err(domain_error)?;
        self.run(move |connection| {
            let tx = connection.transaction().map_err(sql_error)?;
            authorize(&tx, &principal, &workspace)?;
            let mut stmt = tx.prepare("SELECT automation_id FROM automations WHERE workspace_id = ?1 AND (?2 IS NULL OR status = ?2) AND (?3 IS NULL OR updated_at < ?3 OR (updated_at = ?3 AND automation_id < ?4)) ORDER BY updated_at DESC, automation_id DESC LIMIT ?5").map_err(sql_error)?;
            let ids = stmt.query_map(params![workspace, status.map(automation_status), after.as_ref().map(|c| c.0.as_str()), after.as_ref().map(|c| c.1.as_str()), (limit + 1) as i64], |row| row.get::<_, String>(0)).map_err(sql_error)?
                .collect::<rusqlite::Result<Vec<_>>>().map_err(sql_error)?;
            let more = ids.len() > limit;
            let mut items = Vec::new();
            for id in ids.into_iter().take(limit) { items.push(load_automation(&tx, &workspace, &id)?.ok_or(DomainError::Storage)?); }
            let next = if more { items.last().map(|(head, _)| (head.updated_at.clone(), head.automation_id.clone())) } else { None };
            Ok(AutomationPage { items, next })
        })
    }
}

impl ResponsibilityStore for SqliteCoworkerStore {
    fn transaction<F>(
        &mut self,
        scope: &OwnerCommandScope,
        fingerprint: &str,
        operation: F,
    ) -> Result<CommittedResponsibility, DomainError>
    where
        F: Fn(&mut dyn ResponsibilityTransaction) -> Result<CommittedResponsibility, DomainError>
            + Send
            + 'static,
    {
        let scope = scope.clone();
        let fingerprint = digest(
            &canonical_json(
                &json!({"workspace_id": scope.workspace_id, "command_digest": fingerprint}),
            )
            .map_err(domain_error)?,
        );
        let context = self.context.clone();
        let blobs = Arc::clone(&self.store.inner.blobs);
        self.run(move |connection| {
            execute_transaction(
                connection,
                &scope,
                &fingerprint,
                &context,
                blobs.as_ref(),
                Box::new(operation),
            )
        })
    }
}

fn authorize(connection: &Connection, principal: &str, workspace: &str) -> Result<(), DomainError> {
    if principal.trim().is_empty() || workspace.trim().is_empty() {
        return Err(DomainError::Unauthorized);
    }
    let owned: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM workspaces WHERE workspace_id = ?1 AND owner_principal_id = ?2)", params![workspace, principal], |row| row.get(0)).map_err(sql_error)?;
    if owned {
        Ok(())
    } else {
        Err(DomainError::Unauthorized)
    }
}
fn authorize_active(
    connection: &Connection,
    principal: &str,
    workspace: &str,
) -> Result<(), DomainError> {
    authorize(connection, principal, workspace)?;
    let status: Option<String> = connection
        .query_row(
            "SELECT status FROM workspaces WHERE workspace_id = ?1 AND owner_principal_id = ?2",
            params![workspace, principal],
            |row| row.get(0),
        )
        .optional()
        .map_err(sql_error)?;
    match status.as_deref() {
        Some("ACTIVE") => Ok(()),
        Some("ARCHIVED") => Err(DomainError::WorkspaceArchived),
        _ => Err(DomainError::Unauthorized),
    }
}
fn domain_error(error: StoreError) -> DomainError {
    match error {
        StoreError::NotFound => DomainError::NotFound,
        StoreError::Conflict { .. } => DomainError::VersionConflict,
        StoreError::Invalid(_) => DomainError::InvalidDefinition,
        _ => DomainError::Storage,
    }
}
fn sql_error(error: rusqlite::Error) -> DomainError {
    domain_error(map_database_error(error))
}
fn sql_u64(value: u64, field: &str) -> Result<i64, DomainError> {
    to_sql_i64(value, field).map_err(domain_error)
}
fn encode<T: Serialize>(value: &T) -> Result<String, DomainError> {
    String::from_utf8(canonical_json(value).map_err(domain_error)?)
        .map_err(|_| DomainError::InvalidDefinition)
}
fn decode<T: serde::de::DeserializeOwned>(value: String) -> Result<T, DomainError> {
    serde_json::from_str(&value).map_err(|_| DomainError::Storage)
}
fn coworker_status(status: CoworkerStatus) -> &'static str {
    match status {
        CoworkerStatus::Active => "ACTIVE",
        CoworkerStatus::Paused => "PAUSED",
        CoworkerStatus::Archived => "ARCHIVED",
    }
}
fn automation_status(status: AutomationStatus) -> &'static str {
    match status {
        AutomationStatus::Enabled => "ENABLED",
        AutomationStatus::Paused => "PAUSED",
        AutomationStatus::Disabled => "DISABLED",
    }
}

fn load_coworker(
    connection: &Connection,
    workspace: &str,
    id: &str,
) -> Result<Option<(Coworker, CoworkerRevision)>, DomainError> {
    let raw: Option<(String, u64, String, String, String, u64)> = connection.query_row(
        "SELECT workspace_id, current_revision, status, created_at, updated_at, version FROM coworkers WHERE workspace_id = ?1 AND coworker_id = ?2",
        params![workspace, id], |row| Ok((row.get(0)?, from_row_u64(row, 1)?, row.get(2)?, row.get(3)?, row.get(4)?, from_row_u64(row, 5)?)),
    ).optional().map_err(sql_error)?;
    let Some((workspace_id, current_revision, status, created_at, updated_at, version)) = raw
    else {
        return Ok(None);
    };
    let head = Coworker {
        coworker_id: id.into(),
        workspace_id,
        current_revision,
        status: decode(serde_json::to_string(&status).map_err(|_| DomainError::Storage)?)?,
        created_at,
        updated_at,
        version,
    };
    let revision = load_coworker_revision(connection, workspace, id, current_revision)?
        .ok_or(DomainError::Storage)?;
    Ok(Some((head, revision)))
}

fn load_coworker_revision(
    connection: &Connection,
    workspace: &str,
    id: &str,
    revision: u64,
) -> Result<Option<CoworkerRevision>, DomainError> {
    let revision_sql = sql_u64(revision, "Coworker revision")?;
    let value = connection.query_row(
        "SELECT name, avatar_ref_json, role_description, default_lead_agent_binding_id, delegation_strategy, enabled_delegation_profile_ids_json, delegation_budget_policy_json, lead_failover_policy_json, interaction_policy_json, context_policy_json, notification_policy_json, authored_by_json, created_at FROM coworker_revisions WHERE workspace_id = ?1 AND coworker_id = ?2 AND revision = ?3",
        params![workspace, id, revision_sql], |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?, row.get::<_, String>(2)?, row.get::<_, Option<String>>(3)?, row.get::<_, String>(4)?, row.get::<_, String>(5)?, row.get::<_, Option<String>>(6)?, row.get::<_, Option<String>>(7)?, row.get::<_, String>(8)?, row.get::<_, String>(9)?, row.get::<_, String>(10)?, row.get::<_, String>(11)?, row.get::<_, String>(12)?)),
    ).optional().map_err(sql_error)?;
    let Some(value) = value else {
        return Ok(None);
    };
    let definition = CoworkerDefinition {
        name: value.0,
        avatar_ref: value.1.map(decode).transpose()?,
        role_description: value.2,
        default_lead_agent_binding_id: value.3,
        delegation_strategy: decode(
            serde_json::to_string(&value.4).map_err(|_| DomainError::Storage)?,
        )?,
        enabled_delegation_profile_ids: decode(value.5)?,
        delegation_budget_policy: value.6.map(decode).transpose()?,
        lead_failover_policy: value.7.map(decode).transpose()?,
        interaction_policy: decode(value.8)?,
        context_policy: decode(value.9)?,
        notification_policy: decode(value.10)?,
    };
    let revision = CoworkerRevision {
        coworker_id: id.into(),
        revision,
        definition,
        authored_by: decode(value.11)?,
        created_at: value.12,
    };
    Ok(Some(revision))
}

fn load_automation(
    connection: &Connection,
    workspace: &str,
    id: &str,
) -> Result<Option<(Automation, AutomationRevision)>, DomainError> {
    let raw: Option<(String, String, u64, String, String, String, u64)> = connection.query_row(
        "SELECT workspace_id, name, current_revision, status, created_at, updated_at, version FROM automations WHERE workspace_id = ?1 AND automation_id = ?2",
        params![workspace, id], |row| Ok((row.get(0)?, row.get(1)?, from_row_u64(row, 2)?, row.get(3)?, row.get(4)?, row.get(5)?, from_row_u64(row, 6)?)),
    ).optional().map_err(sql_error)?;
    let Some((workspace_id, name, current_revision, status, created_at, updated_at, version)) = raw
    else {
        return Ok(None);
    };
    let status = match status.as_str() {
        "ENABLED" => AutomationStatus::Enabled,
        "PAUSED" => AutomationStatus::Paused,
        "DISABLED" => AutomationStatus::Disabled,
        _ => return Err(DomainError::Storage),
    };
    let head = Automation {
        automation_id: id.to_owned(),
        workspace_id,
        name,
        current_revision,
        status,
        created_at,
        updated_at,
        version,
    };
    let revision = load_automation_revision(connection, workspace, id, current_revision)?
        .ok_or(DomainError::Storage)?;
    Ok(Some((head, revision)))
}

fn load_automation_revision(
    connection: &Connection,
    workspace: &str,
    id: &str,
    revision: u64,
) -> Result<Option<AutomationRevision>, DomainError> {
    let revision_sql = sql_u64(revision, "Automation revision")?;
    let raw: Option<(String, u64, String, Option<String>, Option<u64>, String, String, String, String)> = connection.query_row(
        "SELECT routine_id, routine_revision, triggers_json, coworker_id, coworker_revision, execution_policy_json, authored_by_json, created_at, automation_id FROM automation_revisions WHERE workspace_id = ?1 AND automation_id = ?2 AND revision = ?3",
        params![workspace, id, revision_sql], |row| Ok((row.get(0)?, from_row_u64(row, 1)?, row.get(2)?, row.get(3)?, row_u64_opt(row, 4)?, row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?)),
    ).optional().map_err(sql_error)?;
    let Some((
        routine_id,
        routine_revision,
        triggers_json,
        coworker_id,
        coworker_revision,
        execution_policy_json,
        authored_by_json,
        created_at,
        automation_id,
    )) = raw
    else {
        return Ok(None);
    };
    let coworker_ref = match (coworker_id, coworker_revision) {
        (Some(coworker_id), Some(revision)) => Some(CoworkerRevisionRef {
            coworker_id,
            revision,
        }),
        (None, None) => None,
        _ => return Err(DomainError::Storage),
    };
    Ok(Some(AutomationRevision {
        automation_id,
        revision,
        definition: AutomationDefinition {
            routine_id,
            routine_revision,
            triggers: decode(triggers_json)?,
            execution_policy: decode(execution_policy_json)?,
            coworker_ref,
        },
        authored_by: decode(authored_by_json)?,
        created_at,
    }))
}

struct CoworkerTransaction<'a> {
    connection: &'a Connection,
    scope: &'a OwnerCommandScope,
    context: &'a CoworkerEventContext,
    pending: Option<ResponsibilityMutation>,
}
impl ResponsibilityTransaction for CoworkerTransaction<'_> {
    fn now(&self) -> String {
        self.context.recorded_at.clone()
    }
    fn coworker(&mut self, id: &str) -> Result<Option<(Coworker, CoworkerRevision)>, DomainError> {
        load_coworker(self.connection, &self.scope.workspace_id, id)
    }
    fn automation(
        &mut self,
        id: &str,
    ) -> Result<Option<(Automation, AutomationRevision)>, DomainError> {
        load_automation(self.connection, &self.scope.workspace_id, id)
    }
    fn validate_coworker_references(
        &mut self,
        definition: &CoworkerDefinition,
    ) -> Result<(), DomainError> {
        validate_references(self.connection, &self.scope.workspace_id, definition)
    }
    fn validate_automation_references(
        &mut self,
        _: Option<&AutomationRevision>,
        definition: &AutomationDefinition,
        enabling: bool,
    ) -> Result<(), DomainError> {
        validate_automation_definition(definition)?;
        let routine_revision = sql_u64(definition.routine_revision, "Routine revision")?;
        let routine_exists: bool = self.connection.query_row("SELECT EXISTS(SELECT 1 FROM routines r JOIN routine_revisions rr ON rr.routine_id = r.routine_id AND rr.workspace_id = r.workspace_id WHERE r.workspace_id = ?1 AND r.routine_id = ?2 AND rr.revision = ?3)", params![self.scope.workspace_id, definition.routine_id, routine_revision], |row| row.get(0)).map_err(sql_error)?;
        if !routine_exists {
            return Err(DomainError::NotFound);
        }
        if enabling {
            let routine_active: bool = self.connection.query_row("SELECT EXISTS(SELECT 1 FROM routines WHERE workspace_id = ?1 AND routine_id = ?2 AND status = 'ACTIVE' AND current_revision = ?3)", params![self.scope.workspace_id, definition.routine_id, routine_revision], |row| row.get(0)).map_err(sql_error)?;
            if !routine_active {
                return Err(DomainError::RoutineArchived);
            }
        }
        if let Some(reference) = &definition.coworker_ref {
            let coworker_revision = sql_u64(reference.revision, "Coworker revision")?;
            let coworker_exists: bool = self.connection.query_row("SELECT EXISTS(SELECT 1 FROM coworker_revisions WHERE workspace_id = ?1 AND coworker_id = ?2 AND revision = ?3)", params![self.scope.workspace_id, reference.coworker_id, coworker_revision], |row| row.get(0)).map_err(sql_error)?;
            if !coworker_exists {
                return Err(DomainError::NotFound);
            }
            if enabling {
                let coworker_active: bool = self.connection.query_row("SELECT EXISTS(SELECT 1 FROM coworkers WHERE workspace_id = ?1 AND coworker_id = ?2 AND status = 'ACTIVE')", params![self.scope.workspace_id, reference.coworker_id], |row| row.get(0)).map_err(sql_error)?;
                if !coworker_active {
                    return Err(DomainError::CoworkerInactive);
                }
            }
        }
        if enabling {
            if definition
                .triggers
                .iter()
                .any(|trigger| !matches!(trigger.trigger, TriggerDefinition::Manual))
            {
                return Err(DomainError::TriggerUnsupported);
            }
            let local_trigger_host: bool = self.connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM runtime_workspace_bindings b WHERE b.workspace_id = ?1 AND b.runtime_id = ?2 AND b.status = 'ACTIVE' AND EXISTS(SELECT 1 FROM json_each(b.roles_json) role WHERE role.value = 'TRIGGER_HOST'))",
                params![self.scope.workspace_id, self.context.origin_runtime_id],
                |row| row.get(0),
            ).map_err(sql_error)?;
            if !local_trigger_host {
                return Err(DomainError::ReconciliationRequired);
            }
        }
        Ok(())
    }
    fn automation_lifecycle_facts(
        &mut self,
        id: &str,
    ) -> Result<AutomationLifecycleFacts, DomainError> {
        let Some((_, revision)) = load_automation(self.connection, &self.scope.workspace_id, id)?
        else {
            return Err(DomainError::NotFound);
        };
        let routine_revision = sql_u64(revision.definition.routine_revision, "Routine revision")?;
        let routine_active: bool = self.connection.query_row("SELECT EXISTS(SELECT 1 FROM routines WHERE workspace_id = ?1 AND routine_id = ?2 AND status = 'ACTIVE' AND current_revision = ?3)", params![self.scope.workspace_id, revision.definition.routine_id, routine_revision], |row| row.get(0)).map_err(sql_error)?;
        let coworker_active = match &revision.definition.coworker_ref {
            None => true,
            Some(reference) => self.connection.query_row("SELECT EXISTS(SELECT 1 FROM coworkers WHERE workspace_id = ?1 AND coworker_id = ?2 AND status = 'ACTIVE')", params![self.scope.workspace_id, reference.coworker_id], |row| row.get(0)).map_err(sql_error)?,
        };
        let local_trigger_host: bool = self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM runtime_workspace_bindings b WHERE b.workspace_id = ?1 AND b.runtime_id = ?2 AND b.status = 'ACTIVE' AND EXISTS(SELECT 1 FROM json_each(b.roles_json) role WHERE role.value = 'TRIGGER_HOST'))",
            params![self.scope.workspace_id, self.context.origin_runtime_id],
            |row| row.get(0),
        ).map_err(sql_error)?;
        let trigger_dependencies_reconciled = local_trigger_host
            && revision
                .definition
                .triggers
                .iter()
                .all(|trigger| matches!(trigger.trigger, TriggerDefinition::Manual));
        Ok(AutomationLifecycleFacts {
            routine_active,
            coworker_active,
            trigger_dependencies_reconciled,
        })
    }
    fn coworker_lifecycle_facts(
        &mut self,
        id: &str,
    ) -> Result<CoworkerLifecycleFacts, DomainError> {
        lifecycle_facts(self.connection, &self.scope.workspace_id, id)
    }
    fn commit(
        &mut self,
        mutation: ResponsibilityMutation,
    ) -> Result<CommittedResponsibility, DomainError> {
        if self.pending.is_some() {
            return Err(DomainError::InvalidDefinition);
        }
        let result = mutation.aggregate.clone();
        self.pending = Some(mutation);
        Ok(result)
    }
}

fn lifecycle_facts(
    connection: &Connection,
    workspace: &str,
    id: &str,
) -> Result<CoworkerLifecycleFacts, DomainError> {
    let is_primary = connection
        .query_row(
            "SELECT primary_coworker_id = ?2 FROM workspaces WHERE workspace_id = ?1",
            params![workspace, id],
            |row| Ok(row.get::<_, Option<bool>>(0)?.unwrap_or(false)),
        )
        .map_err(sql_error)?;
    let has_active_automation = connection.query_row("SELECT EXISTS(SELECT 1 FROM automations a JOIN automation_revisions ar ON ar.automation_id = a.automation_id AND ar.revision = a.current_revision WHERE a.workspace_id = ?1 AND ar.coworker_id = ?2 AND a.status = 'ENABLED') OR EXISTS(SELECT 1 FROM automation_occurrences o JOIN automation_revisions ar ON ar.automation_id = o.automation_id AND ar.revision = o.automation_revision AND ar.workspace_id = o.workspace_id WHERE o.workspace_id = ?1 AND ar.coworker_id = ?2 AND o.status NOT IN ('COMPLETED','SKIPPED','FAILED'))", params![workspace, id], |row| row.get(0)).map_err(sql_error)?;
    let has_nonterminal_tasks = connection.query_row("SELECT EXISTS(SELECT 1 FROM tasks WHERE workspace_id = ?1 AND origin_coworker_id = ?2 AND status NOT IN ('COMPLETED','FAILED','CANCELLED'))", params![workspace, id], |row| row.get(0)).map_err(sql_error)?;
    // Trigger reconciliation is not implemented here. Resume is safe only when no
    // retained non-disabled Automation or pending occurrence needs reconciliation.
    Ok(CoworkerLifecycleFacts {
        is_primary,
        has_active_automation,
        has_nonterminal_tasks,
        resume_reconciled: !has_active_automation,
    })
}

fn validate_references(
    connection: &Connection,
    workspace: &str,
    definition: &CoworkerDefinition,
) -> Result<(), DomainError> {
    validate_coworker_definition(definition)?;
    let mut bindings = definition
        .default_lead_agent_binding_id
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    if let Some(policy) = &definition.lead_failover_policy {
        let allowed = [
            "mode",
            "triggers",
            "fallback_agent_binding_ids",
            "max_lead_changes",
        ];
        if policy.keys().any(|key| !allowed.contains(&key.as_str())) {
            return Err(DomainError::InvalidDefinition);
        }
        let mode = policy
            .get("mode")
            .and_then(Value::as_str)
            .ok_or(DomainError::InvalidDefinition)?;
        let triggers = policy
            .get("triggers")
            .and_then(Value::as_array)
            .ok_or(DomainError::InvalidDefinition)?;
        let fallback = policy
            .get("fallback_agent_binding_ids")
            .and_then(Value::as_array)
            .ok_or(DomainError::InvalidDefinition)?;
        let changes = policy
            .get("max_lead_changes")
            .and_then(Value::as_u64)
            .ok_or(DomainError::InvalidDefinition)?;
        let mut seen = std::collections::HashSet::new();
        for trigger in triggers {
            let trigger = trigger.as_str().ok_or(DomainError::InvalidDefinition)?;
            if ![
                "AGENT_UNAVAILABLE",
                "QUOTA_EXHAUSTED",
                "RUNTIME_UNAVAILABLE",
            ]
            .contains(&trigger)
                || !seen.insert(trigger)
            {
                return Err(DomainError::InvalidDefinition);
            }
        }
        let mut seen = std::collections::HashSet::new();
        for id in fallback {
            let id = id
                .as_str()
                .filter(|s| !s.trim().is_empty())
                .ok_or(DomainError::InvalidDefinition)?;
            if !seen.insert(id) {
                return Err(DomainError::InvalidDefinition);
            }
            bindings.push(id.to_owned());
        }
        if fallback.len() > 3
            || changes > 3
            || match mode {
                "DISABLED" => !triggers.is_empty() || !fallback.is_empty() || changes != 0,
                "ASK" => triggers.is_empty() || changes != 0,
                "ALLOW_LISTED" => {
                    triggers.is_empty()
                        || fallback.is_empty()
                        || changes == 0
                        || changes as usize > fallback.len()
                }
                _ => true,
            }
        {
            return Err(DomainError::InvalidDefinition);
        }
    }
    for binding in bindings {
        let valid: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM agent_bindings WHERE workspace_id = ?1 AND agent_binding_id = ?2 AND enabled = 1 AND lead_eligible = 1)", params![workspace, binding], |row| row.get(0)).map_err(sql_error)?;
        if !valid {
            return Err(DomainError::InvalidDefinition);
        }
    }
    for profile in &definition.enabled_delegation_profile_ids {
        let valid: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM delegation_profiles WHERE workspace_id = ?1 AND delegation_profile_id = ?2 AND status = 'ENABLED')", params![workspace, profile], |row| row.get(0)).map_err(sql_error)?;
        if !valid {
            return Err(DomainError::InvalidDefinition);
        }
    }
    if let Some(avatar) = &definition.avatar_ref {
        if avatar
            .keys()
            .any(|key| !["workspace_id", "resource_id", "revision_id"].contains(&key.as_str()))
            || avatar.get("workspace_id").and_then(Value::as_str) != Some(workspace)
        {
            return Err(DomainError::InvalidDefinition);
        }
        let resource = avatar
            .get("resource_id")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or(DomainError::InvalidDefinition)?;
        let revision = avatar
            .get("revision_id")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or(DomainError::InvalidDefinition)?;
        let valid: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM resources r JOIN resource_revisions rr ON rr.resource_id = r.resource_id WHERE r.workspace_id = ?1 AND r.resource_id = ?2 AND rr.resource_revision_id = ?3 AND (r.context_document_json IS NULL OR json_extract(r.context_document_json, '$.status') = 'ACTIVE'))", params![workspace, resource, revision], |row| row.get(0)).map_err(sql_error)?;
        if !valid {
            return Err(DomainError::InvalidDefinition);
        }
    }
    if let Some(policy) = &definition.delegation_budget_policy {
        if policy.keys().any(|key| {
            ![
                "max_concurrent_children",
                "max_host_delegation_depth",
                "max_per_attempt",
                "max_per_task",
                "max_per_profile",
                "on_threshold",
            ]
            .contains(&key.as_str())
        }) {
            return Err(DomainError::InvalidDefinition);
        }
        optional_u64(policy, "max_concurrent_children", 1, 8)?;
        optional_u64(policy, "max_host_delegation_depth", 0, 2)?;
        let action = policy
            .get("on_threshold")
            .and_then(Value::as_str)
            .ok_or(DomainError::InvalidDefinition)?;
        if ![
            "WARN",
            "REDUCE_CONCURRENCY",
            "PREFER_CHEAPER",
            "REQUIRE_APPROVAL",
            "STOP_NEW_DELEGATION",
        ]
        .contains(&action)
        {
            return Err(DomainError::InvalidDefinition);
        }
        for name in ["max_per_attempt", "max_per_task", "max_per_profile"] {
            if let Some(value) = policy.get(name).filter(|v| !v.is_null()) {
                let budget = value.as_object().ok_or(DomainError::InvalidDefinition)?;
                if budget.keys().any(|key| {
                    ![
                        "max_wall_time_ms",
                        "max_cost_minor_units",
                        "currency",
                        "max_tokens",
                        "max_child_attempts",
                        "max_concurrency",
                    ]
                    .contains(&key.as_str())
                }) {
                    return Err(DomainError::InvalidDefinition);
                }
                for (name, value) in budget {
                    if value.is_null() {
                        continue;
                    }
                    if name == "currency" {
                        if value.as_str().is_none_or(|s| s.chars().count() != 3) {
                            return Err(DomainError::InvalidDefinition);
                        }
                    } else if value
                        .as_u64()
                        .is_none_or(|n| name == "max_concurrency" && n == 0)
                    {
                        return Err(DomainError::InvalidDefinition);
                    }
                }
            }
        }
    }
    // Reject non-I-JSON numeric values before any digest/blob/database write.
    canonical_json(definition).map_err(domain_error)?;
    Ok(())
}
fn optional_u64(object: &ContractObject, key: &str, min: u64, max: u64) -> Result<(), DomainError> {
    if object
        .get(key)
        .filter(|v| !v.is_null())
        .is_some_and(|value| value.as_u64().is_none_or(|n| !(min..=max).contains(&n)))
    {
        return Err(DomainError::InvalidDefinition);
    }
    Ok(())
}

fn replay(
    connection: &Connection,
    scope: &OwnerCommandScope,
    fingerprint: &str,
) -> Result<Option<CommittedResponsibility>, DomainError> {
    let receipt: Option<(String, Option<String>, Option<String>)> = connection.query_row("SELECT request_digest, response_json, response_digest FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2", params![scope.principal_id, scope.request_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).optional().map_err(sql_error)?;
    let Some((prior, response, response_digest)) = receipt else {
        return Ok(None);
    };
    if prior != fingerprint {
        return Err(DomainError::IdempotencyConflict);
    }
    let response = response.ok_or(DomainError::Storage)?;
    if response_digest.as_deref() != Some(digest(response.as_bytes()).as_str()) {
        return Err(DomainError::Storage);
    }
    let result: CommittedResponsibility = decode(response)?;
    match &result {
        CommittedResponsibility::Coworker(head) if head.workspace_id == scope.workspace_id => {
            Ok(Some(result))
        }
        CommittedResponsibility::Automation(head) if head.workspace_id == scope.workspace_id => {
            Ok(Some(result))
        }
        _ => Err(DomainError::Storage),
    }
}

fn prepare(
    connection: &Connection,
    scope: &OwnerCommandScope,
    context: &CoworkerEventContext,
    decision: &Decision,
) -> Result<ResponsibilityMutation, DomainError> {
    let mut boundary = CoworkerTransaction {
        connection,
        scope,
        context,
        pending: None,
    };
    let result = decision(&mut boundary)?;
    let mutation = boundary.pending.ok_or(DomainError::InvalidDefinition)?;
    if result != mutation.aggregate {
        return Err(DomainError::InvalidDefinition);
    }
    Ok(mutation)
}

fn state_record(
    connection: &Connection,
    scope: &OwnerCommandScope,
    mutation: &ResponsibilityMutation,
) -> Result<Value, DomainError> {
    match &mutation.aggregate {
        CommittedResponsibility::Coworker(head) => {
            let revision = match &mutation.append {
                Some(RevisionAppend::Coworker(revision)) => revision.clone(),
                None => {
                    load_coworker(connection, &scope.workspace_id, &head.coworker_id)?
                        .ok_or(DomainError::NotFound)?
                        .1
                }
                _ => return Err(DomainError::InvalidDefinition),
            };
            if head.workspace_id != scope.workspace_id
                || revision.coworker_id != head.coworker_id
                || revision.revision != head.current_revision
                || head.version == 0
            {
                return Err(DomainError::InvalidDefinition);
            }
            Ok(json!({"coworker": head, "revision": revision}))
        }
        CommittedResponsibility::Automation(head) => {
            let revision = match &mutation.append {
                Some(RevisionAppend::Automation(revision)) => revision.clone(),
                None => {
                    load_automation(connection, &scope.workspace_id, &head.automation_id)?
                        .ok_or(DomainError::NotFound)?
                        .1
                }
                _ => return Err(DomainError::InvalidDefinition),
            };
            if head.workspace_id != scope.workspace_id
                || revision.automation_id != head.automation_id
                || revision.revision != head.current_revision
                || head.version == 0
            {
                return Err(DomainError::InvalidDefinition);
            }
            Ok(json!({"automation": head, "revision": revision}))
        }
    }
}

fn execute_transaction(
    connection: &mut Connection,
    scope: &OwnerCommandScope,
    fingerprint: &str,
    context: &CoworkerEventContext,
    blobs: &dyn BlobStore,
    decision: Decision,
) -> Result<CommittedResponsibility, DomainError> {
    let (prepared, prepared_state) = {
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sql_error)?;
        authorize_active(&tx, &scope.principal_id, &scope.workspace_id)?;
        if let Some(prior) = replay(&tx, scope, fingerprint)? {
            return Ok(prior);
        }
        let prepared = prepare(&tx, scope, context, &decision)?;
        let state = canonical_json(&state_record(&tx, scope, &prepared)?).map_err(domain_error)?;
        // Preparation transaction writes nothing and releases before blob I/O.
        (prepared, state)
    };
    let media_type = match &prepared.aggregate {
        CommittedResponsibility::Coworker(_) => "application/vnd.litecowork.coworker+json",
        CommittedResponsibility::Automation(_) => "application/vnd.litecowork.automation+json",
    };
    let blob = blobs
        .put(
            &scope.workspace_id,
            BlobPurpose::AggregateState,
            &prepared_state,
            media_type,
        )
        .map_err(domain_error)?;
    if blob.digest != digest(&prepared_state)
        || blob.size_bytes != prepared_state.len() as u64
        || blobs
            .get(&scope.workspace_id, BlobPurpose::AggregateState, &blob)
            .map_err(domain_error)?
            != prepared_state
    {
        return Err(DomainError::Storage);
    }
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql_error)?;
    authorize_active(&tx, &scope.principal_id, &scope.workspace_id)?;
    if let Some(prior) = replay(&tx, scope, fingerprint)? {
        return Ok(prior);
    }
    let final_mutation = prepare(&tx, scope, context, &decision)?;
    let final_state =
        canonical_json(&state_record(&tx, scope, &final_mutation)?).map_err(domain_error)?;
    if final_mutation != prepared
        || final_state != prepared_state
        || digest(&final_state) != blob.digest
    {
        return Err(DomainError::VersionConflict);
    }
    let result = persist(&tx, scope, context, &final_mutation, blob)?;
    let response = encode(&result)?;
    tx.execute("INSERT INTO request_dedup(principal_id, request_id, request_digest, response_json, response_digest, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)", params![scope.principal_id, scope.request_id, fingerprint, response, digest(response.as_bytes()), context.recorded_at]).map_err(sql_error)?;
    tx.commit().map_err(sql_error)?;
    Ok(result)
}

fn persist(
    tx: &Transaction<'_>,
    scope: &OwnerCommandScope,
    context: &CoworkerEventContext,
    mutation: &ResponsibilityMutation,
    blob: BlobRef,
) -> Result<CommittedResponsibility, DomainError> {
    match &mutation.aggregate {
        CommittedResponsibility::Coworker(_) => {
            persist_coworker(tx, scope, context, mutation, blob)
        }
        CommittedResponsibility::Automation(_) => {
            persist_automation(tx, scope, context, mutation, blob)
        }
    }
}

fn persist_coworker(
    tx: &Transaction<'_>,
    scope: &OwnerCommandScope,
    context: &CoworkerEventContext,
    mutation: &ResponsibilityMutation,
    blob: BlobRef,
) -> Result<CommittedResponsibility, DomainError> {
    let CommittedResponsibility::Coworker(head) = &mutation.aggregate else {
        return Err(DomainError::TriggerUnsupported);
    };
    let actual = load_coworker(tx, &scope.workspace_id, &head.coworker_id)?;
    if head.workspace_id != scope.workspace_id || head.updated_at != context.recorded_at {
        return Err(DomainError::InvalidDefinition);
    }
    let expected_event = match (&actual, &mutation.append) {
        (None, Some(RevisionAppend::Coworker(revision)))
            if head.created_at == context.recorded_at && revision.revision == 1 =>
        {
            (
                "coworker.created.v1",
                json!({"coworker_id": head.coworker_id, "workspace_id": head.workspace_id, "current_revision": 1, "status": "ACTIVE", "aggregate_version": 1}),
            )
        }
        (Some((current, _)), Some(RevisionAppend::Coworker(revision)))
            if head.status == current.status
                && head.current_revision
                    == current
                        .current_revision
                        .checked_add(1)
                        .ok_or(DomainError::VersionOverflow)? =>
        {
            (
                "coworker.revised.v1",
                json!({"coworker_id": head.coworker_id, "revision": revision.revision, "revision_digest": digest(&canonical_json(revision).map_err(domain_error)?), "authored_by": revision.authored_by, "aggregate_version": head.version}),
            )
        }
        (Some((current, _)), None)
            if head.current_revision == current.current_revision
                && head.status != current.status =>
        {
            (
                "coworker.status.changed.v1",
                json!({"coworker_id": head.coworker_id, "from": current.status, "to": head.status, "aggregate_version": head.version}),
            )
        }
        _ => return Err(DomainError::InvalidDefinition),
    };
    if mutation.event.kind != expected_event.0 || mutation.event.payload != expected_event.1 {
        return Err(DomainError::InvalidDefinition);
    }
    match (mutation.expected_version, &actual) {
        (None, None)
            if head.version == 1
                && head.current_revision == 1
                && head.status == CoworkerStatus::Active =>
        {
            tx.execute("INSERT INTO coworkers(coworker_id, workspace_id, current_revision, status, created_at, updated_at, version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)", params![head.coworker_id, head.workspace_id, sql_u64(head.current_revision, "Coworker revision")?, coworker_status(head.status), head.created_at, head.updated_at, sql_u64(head.version, "Coworker version")?]).map_err(sql_error)?;
        }
        (Some(expected), Some((current, _)))
            if current.version == expected
                && head.version == next_version(current.version, expected)? =>
        {
            if head.created_at != current.created_at || current.status == CoworkerStatus::Archived {
                return Err(DomainError::CoworkerArchived);
            }
            if current.status != head.status {
                check_coworker_transition(
                    current.status,
                    head.status,
                    &lifecycle_facts(tx, &scope.workspace_id, &head.coworker_id)?,
                )?;
            }
            if head.status == CoworkerStatus::Active && current.status == CoworkerStatus::Paused {
                validate_references(
                    tx,
                    &scope.workspace_id,
                    &actual.as_ref().ok_or(DomainError::Storage)?.1.definition,
                )?;
            }
            let changed = tx.execute("UPDATE coworkers SET current_revision = ?1, status = ?2, updated_at = ?3, version = ?4 WHERE workspace_id = ?5 AND coworker_id = ?6 AND version = ?7", params![sql_u64(head.current_revision, "Coworker revision")?, coworker_status(head.status), head.updated_at, sql_u64(head.version, "Coworker version")?, head.workspace_id, head.coworker_id, sql_u64(expected, "expected Coworker version")?]).map_err(sql_error)?;
            if changed != 1 {
                return Err(DomainError::VersionConflict);
            }
        }
        _ => return Err(DomainError::VersionConflict),
    }
    if let Some(RevisionAppend::Coworker(revision)) = &mutation.append {
        if revision.authored_by
            != (PrincipalRef {
                principal_id: scope.principal_id.clone(),
                kind: PrincipalKind::User,
            })
            || revision.created_at != context.recorded_at
        {
            return Err(DomainError::InvalidDefinition);
        }
        validate_references(tx, &scope.workspace_id, &revision.definition)?;
        let d = &revision.definition;
        let strategy_value = serde_json::to_value(d.delegation_strategy)
            .map_err(|_| DomainError::InvalidDefinition)?;
        let strategy = strategy_value
            .as_str()
            .ok_or(DomainError::InvalidDefinition)?;
        tx.execute("INSERT INTO coworker_revisions(coworker_id, revision, workspace_id, name, avatar_ref_json, role_description, default_lead_agent_binding_id, delegation_strategy, enabled_delegation_profile_ids_json, delegation_budget_policy_json, lead_failover_policy_json, interaction_policy_json, context_policy_json, notification_policy_json, authored_by_json, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)", params![revision.coworker_id, sql_u64(revision.revision, "Coworker revision")?, scope.workspace_id, d.name, d.avatar_ref.as_ref().map(encode).transpose()?, d.role_description, d.default_lead_agent_binding_id, strategy, encode(&d.enabled_delegation_profile_ids)?, d.delegation_budget_policy.as_ref().map(encode).transpose()?, d.lead_failover_policy.as_ref().map(encode).transpose()?, encode(&d.interaction_policy)?, encode(&d.context_policy)?, encode(&d.notification_policy)?, encode(&revision.authored_by)?, revision.created_at]).map_err(sql_error)?;
    } else if mutation.append.is_some() {
        return Err(DomainError::TriggerUnsupported);
    }
    // Scoped readback verifies the exact head/revision referenced by the state blob.
    let persisted =
        load_coworker(tx, &scope.workspace_id, &head.coworker_id)?.ok_or(DomainError::Storage)?;
    if persisted.0 != *head {
        return Err(DomainError::Storage);
    }
    if mutation.append.as_ref().is_some_and(|append| match append {
        RevisionAppend::Coworker(revision) => *revision != persisted.1,
        _ => true,
    }) {
        return Err(DomainError::Storage);
    }
    write_event(
        tx,
        scope,
        context,
        "Coworker",
        &head.coworker_id,
        head.version,
        &mutation.event,
        blob,
    )?;
    Ok(mutation.aggregate.clone())
}

fn persist_automation(
    tx: &Transaction<'_>,
    scope: &OwnerCommandScope,
    context: &CoworkerEventContext,
    mutation: &ResponsibilityMutation,
    blob: BlobRef,
) -> Result<CommittedResponsibility, DomainError> {
    let CommittedResponsibility::Automation(head) = &mutation.aggregate else {
        return Err(DomainError::InvalidDefinition);
    };
    let actual = load_automation(tx, &scope.workspace_id, &head.automation_id)?;
    if head.workspace_id != scope.workspace_id || head.updated_at != context.recorded_at {
        return Err(DomainError::InvalidDefinition);
    }
    let expected_event = match (&actual, &mutation.append) {
        (None, Some(RevisionAppend::Automation(revision)))
            if head.created_at == context.recorded_at && revision.revision == 1 =>
        {
            (
                "automation.created.v1",
                json!({"automation_id": head.automation_id, "current_revision": 1, "status": "PAUSED", "aggregate_version": 1}),
            )
        }
        (Some((current, _)), Some(RevisionAppend::Automation(revision)))
            if head.status == current.status
                && head.current_revision
                    == current
                        .current_revision
                        .checked_add(1)
                        .ok_or(DomainError::VersionOverflow)? =>
        {
            (
                "automation.revision.created.v1",
                json!({"automation_id": head.automation_id, "revision": revision.revision, "definition_digest": digest(&canonical_json(revision).map_err(domain_error)?), "authored_by": revision.authored_by, "coworker_ref": revision.definition.coworker_ref}),
            )
        }
        (Some((current, _)), None)
            if head.current_revision == current.current_revision
                && head.status != current.status =>
        {
            (
                "automation.status.changed.v1",
                json!({"automation_id": head.automation_id, "from": current.status, "to": head.status, "aggregate_version": head.version}),
            )
        }
        _ => return Err(DomainError::InvalidDefinition),
    };
    if mutation.event.kind != expected_event.0 || mutation.event.payload != expected_event.1 {
        return Err(DomainError::InvalidDefinition);
    }
    match (mutation.expected_version, &actual) {
        (None, None)
            if head.version == 1
                && head.current_revision == 1
                && head.status == AutomationStatus::Paused =>
        {
            tx.execute("INSERT INTO automations(automation_id, workspace_id, name, current_revision, status, created_at, updated_at, version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)", params![head.automation_id, head.workspace_id, head.name, sql_u64(head.current_revision, "Automation revision")?, automation_status(head.status), head.created_at, head.updated_at, sql_u64(head.version, "Automation version")?]).map_err(sql_error)?;
        }
        (Some(expected), Some((current, _)))
            if current.version == expected
                && head.version == next_version(current.version, expected)? =>
        {
            if head.created_at != current.created_at || current.status == AutomationStatus::Disabled
            {
                return Err(DomainError::AutomationDisabled);
            }
            let changed = tx.execute("UPDATE automations SET name = ?1, current_revision = ?2, status = ?3, updated_at = ?4, version = ?5 WHERE workspace_id = ?6 AND automation_id = ?7 AND version = ?8", params![head.name, sql_u64(head.current_revision, "Automation revision")?, automation_status(head.status), head.updated_at, sql_u64(head.version, "Automation version")?, scope.workspace_id, head.automation_id, sql_u64(expected, "expected Automation version")?]).map_err(sql_error)?;
            if changed != 1 {
                return Err(DomainError::VersionConflict);
            }
        }
        _ => return Err(DomainError::VersionConflict),
    }
    if let Some(RevisionAppend::Automation(revision)) = &mutation.append {
        if revision.authored_by
            != (PrincipalRef {
                principal_id: scope.principal_id.clone(),
                kind: PrincipalKind::User,
            })
            || revision.created_at != context.recorded_at
            || revision.automation_id != head.automation_id
            || revision.revision != head.current_revision
        {
            return Err(DomainError::InvalidDefinition);
        }
        validate_automation_definition(&revision.definition)?;
        if head.status == AutomationStatus::Enabled {
            return Err(DomainError::ReconciliationRequired);
        }
        let coworker = revision.definition.coworker_ref.as_ref();
        tx.execute("INSERT INTO automation_revisions(workspace_id, automation_id, revision, routine_id, routine_revision, coworker_id, coworker_revision, triggers_json, execution_policy_json, authored_by_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)", params![scope.workspace_id, revision.automation_id, sql_u64(revision.revision, "Automation revision")?, revision.definition.routine_id, sql_u64(revision.definition.routine_revision, "Routine revision")?, coworker.map(|value| value.coworker_id.as_str()), coworker.map(|value| value.revision).map(|value| sql_u64(value, "Coworker revision")).transpose()?, encode(&revision.definition.triggers)?, encode(&revision.definition.execution_policy)?, encode(&revision.authored_by)?, revision.created_at]).map_err(sql_error)?;
    } else if mutation.append.is_some() {
        return Err(DomainError::InvalidDefinition);
    }
    let persisted = load_automation(tx, &scope.workspace_id, &head.automation_id)?
        .ok_or(DomainError::Storage)?;
    if persisted.0 != *head
        || mutation.append.as_ref().is_some_and(|append| match append {
            RevisionAppend::Automation(revision) => *revision != persisted.1,
            _ => true,
        })
    {
        return Err(DomainError::Storage);
    }
    write_event(
        tx,
        scope,
        context,
        "Automation",
        &head.automation_id,
        head.version,
        &mutation.event,
        blob.clone(),
    )?;
    if head.status == AutomationStatus::Enabled {
        let (_, revision) = load_automation(tx, &scope.workspace_id, &head.automation_id)?
            .ok_or(DomainError::Storage)?;
        ensure_manual_trigger_cursors(tx, scope, context, head, &revision, blob)?;
    }
    Ok(mutation.aggregate.clone())
}

fn ensure_manual_trigger_cursors(
    tx: &Transaction<'_>,
    scope: &OwnerCommandScope,
    context: &CoworkerEventContext,
    automation: &Automation,
    revision: &AutomationRevision,
    state_blob: BlobRef,
) -> Result<(), DomainError> {
    if revision
        .definition
        .triggers
        .iter()
        .any(|trigger| !matches!(trigger.trigger, TriggerDefinition::Manual))
    {
        return Err(DomainError::TriggerUnsupported);
    }
    let host_ready: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM runtime_workspace_bindings b WHERE b.workspace_id = ?1 AND b.runtime_id = ?2 AND b.status = 'ACTIVE' AND EXISTS(SELECT 1 FROM json_each(b.roles_json) role WHERE role.value = 'TRIGGER_HOST'))",
        params![scope.workspace_id, context.origin_runtime_id],
        |row| row.get(0),
    ).map_err(sql_error)?;
    if !host_ready {
        return Err(DomainError::ReconciliationRequired);
    }
    for trigger in &revision.definition.triggers {
        let cursor_digest = digest(&canonical_json(trigger).map_err(domain_error)?);
        let current = load_automation_cursor(
            tx,
            &scope.workspace_id,
            &automation.automation_id,
            &trigger.trigger_id,
        )?;
        let (host_epoch, cursor_version) = if let Some((host, epoch, version)) = current {
            if host != context.origin_runtime_id {
                return Err(DomainError::ReconciliationRequired);
            }
            update_automation_cursor_revision(
                tx,
                &scope.workspace_id,
                &automation.automation_id,
                &trigger.trigger_id,
                version,
                revision.revision,
                &cursor_digest,
                &context.recorded_at,
            )?;
            (
                epoch,
                version.checked_add(1).ok_or(DomainError::VersionOverflow)?,
            )
        } else {
            tx.execute(
                "INSERT INTO automation_cursors(workspace_id, automation_id, active_automation_revision, trigger_id, trigger_host_runtime_id, host_epoch, cursor_digest, last_seen_digest, last_observation_ref_json, next_scheduled_at, last_checked_at, observation_gap_since, version) VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, NULL, NULL, NULL, ?7, NULL, 1)",
                params![scope.workspace_id, automation.automation_id, sql_u64(revision.revision, "Automation revision")?, trigger.trigger_id, context.origin_runtime_id, cursor_digest, context.recorded_at],
            ).map_err(sql_error)?;
            (1, 1)
        };
        let event = ResponsibilityEvent {
            kind: "automation.cursor.changed.v1",
            payload: json!({
                "automation_id": automation.automation_id,
                "trigger_id": trigger.trigger_id,
                "active_automation_revision": revision.revision,
                "trigger_host_runtime_id": context.origin_runtime_id,
                "host_epoch": host_epoch,
                "cursor_digest": cursor_digest,
                "last_checked_at": context.recorded_at,
                "aggregate_version": automation.version,
            }),
        };
        let mut cursor_context = context.clone();
        cursor_context.event_id = format!(
            "{}:cursor:{}:{}",
            context.event_id, trigger.trigger_id, cursor_version
        );
        write_event(
            tx,
            scope,
            &cursor_context,
            "Automation",
            &automation.automation_id,
            automation.version,
            &event,
            state_blob.clone(),
        )?;
    }
    Ok(())
}

/// Cursor identity is Workspace-scoped even though current local schemas also use
/// globally unique Automation IDs. Keep the tenant predicate at every read/write
/// boundary so upgrades to Workspace-local IDs cannot cross-link a host cursor.
pub(super) fn load_automation_cursor(
    connection: &Connection,
    workspace_id: &str,
    automation_id: &str,
    trigger_id: &str,
) -> Result<Option<(String, u64, u64)>, DomainError> {
    connection.query_row(
        "SELECT trigger_host_runtime_id, host_epoch, version FROM automation_cursors WHERE workspace_id = ?1 AND automation_id = ?2 AND trigger_id = ?3",
        params![workspace_id, automation_id, trigger_id],
        |row| Ok((row.get(0)?, from_row_u64(row, 1)?, from_row_u64(row, 2)?)),
    ).optional().map_err(sql_error)
}

pub(super) fn update_automation_cursor_revision(
    connection: &Connection,
    workspace_id: &str,
    automation_id: &str,
    trigger_id: &str,
    expected_version: u64,
    revision: u64,
    cursor_digest: &str,
    last_checked_at: &str,
) -> Result<(), DomainError> {
    let changed = connection.execute(
        "UPDATE automation_cursors SET active_automation_revision = ?1, cursor_digest = ?2, last_checked_at = ?3, version = version + 1 WHERE workspace_id = ?4 AND automation_id = ?5 AND trigger_id = ?6 AND version = ?7",
        params![sql_u64(revision, "Automation revision")?, cursor_digest, last_checked_at, workspace_id, automation_id, trigger_id, sql_u64(expected_version, "Automation cursor version")?],
    ).map_err(sql_error)?;
    if changed == 1 {
        Ok(())
    } else {
        Err(DomainError::VersionConflict)
    }
}

fn write_event(
    tx: &Transaction<'_>,
    scope: &OwnerCommandScope,
    context: &CoworkerEventContext,
    entity_type: &str,
    entity_id: &str,
    version: u64,
    event: &ResponsibilityEvent,
    blob: BlobRef,
) -> Result<(), DomainError> {
    let allowed = match entity_type {
        "Coworker" => [
            "coworker.created.v1",
            "coworker.revised.v1",
            "coworker.status.changed.v1",
        ]
        .contains(&event.kind),
        "Automation" => [
            "automation.created.v1",
            "automation.revision.created.v1",
            "automation.status.changed.v1",
            "automation.cursor.changed.v1",
        ]
        .contains(&event.kind),
        _ => false,
    };
    if !allowed {
        return Err(DomainError::InvalidDefinition);
    }
    tx.execute("INSERT INTO workspace_origin_sequences(workspace_id, origin_runtime_id, last_sequence) VALUES (?1, ?2, 1) ON CONFLICT(workspace_id, origin_runtime_id) DO UPDATE SET last_sequence = last_sequence + 1", params![scope.workspace_id, context.origin_runtime_id]).map_err(sql_error)?;
    let sequence: i64 = tx.query_row("SELECT last_sequence FROM workspace_origin_sequences WHERE workspace_id = ?1 AND origin_runtime_id = ?2", params![scope.workspace_id, context.origin_runtime_id], |row| row.get(0)).map_err(sql_error)?;
    let payload = encode(&event.payload)?;
    let state_ref = AggregateStateRef {
        blob,
        entity_revision: version,
        record_schema_version: 1,
    };
    tx.execute("INSERT INTO domain_events(event_id, workspace_id, entity_type, entity_id, origin_runtime_id, origin_sequence, entity_revision, hlc_timestamp, correlation_id, causation_id, schema_version, type, payload_json, aggregate_state_ref_json, recorded_at, payload_digest) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, ?11, ?12, ?13, ?14, ?15)", params![context.event_id, scope.workspace_id, entity_type, entity_id, context.origin_runtime_id, sequence, sql_u64(version, "aggregate version")?, context.hlc_timestamp, context.correlation_id, context.causation_id, event.kind, payload, encode(&state_ref)?, context.recorded_at, digest(payload.as_bytes())]).map_err(sql_error)?;
    Ok(())
}
