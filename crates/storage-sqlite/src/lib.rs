mod blob;

pub use blob::{FileBlobStore, WorkspaceBlobKey, WorkspaceBlobKeyProvider};

use rusqlite::{Connection, ErrorCode, OptionalExtension, TransactionBehavior, params};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use storage_core::{
    AggregateStateRef, BlobPurpose, BlobStore, CommittedWorkspace, DomainEvent, EventDraft,
    EventStore, ReplicationPolicy, StateStore, StoreError, Workspace,
};

const SQLITE_V1_DDL: &str = include_str!("../../../docs/schemas/sqlite-v1.sql");
const SCHEMA_VERSION: i64 = 1;
const MIGRATION_NAME: &str = "baseline_v1";
const STATE_MEDIA_TYPE: &str = "application/vnd.litecowork.aggregate-state+json";

#[derive(Clone, Debug)]
pub struct SqliteConfig {
    pub writer_queue_capacity: usize,
    pub busy_timeout: Duration,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SqliteWriterMetricsSnapshot {
    /// Public storage commands submitted, including callers waiting for queue capacity.
    pub outstanding_commands: u64,
    /// Maximum simultaneous submitted commands observed since this adapter opened.
    pub outstanding_commands_peak: u64,
    /// Total elapsed time inside bounded-channel `send` calls, in nanoseconds.
    pub send_wait_nanos_total: u64,
    /// Maximum elapsed time inside one bounded-channel `send` call, in nanoseconds.
    pub send_wait_nanos_max: u64,
}

#[derive(Default)]
struct SqliteWriterMetrics {
    outstanding_commands: AtomicU64,
    outstanding_commands_peak: AtomicU64,
    send_wait_nanos_total: AtomicU64,
    send_wait_nanos_max: AtomicU64,
}

impl Default for SqliteConfig {
    fn default() -> Self {
        Self {
            writer_queue_capacity: 64,
            busy_timeout: Duration::from_secs(5),
        }
    }
}

#[derive(Clone)]
pub struct SqliteWorkspaceStore {
    inner: Arc<Inner>,
}

struct Inner {
    sender: SyncSender<Command>,
    join: Mutex<Option<JoinHandle<()>>>,
    blobs: Arc<dyn BlobStore>,
    writer_metrics: SqliteWriterMetrics,
}

enum Command {
    GetWorkspace {
        workspace_id: String,
        reply: mpsc::Sender<Result<Option<Workspace>, StoreError>>,
    },
    CommitWorkspace {
        commit: Box<WorkspaceCommit>,
        reply: mpsc::Sender<Result<CommittedWorkspace, StoreError>>,
    },
    ReadWorkspaceEvents {
        workspace_id: String,
        reply: mpsc::Sender<Result<Vec<DomainEvent>, StoreError>>,
    },
    SqliteVersion {
        reply: mpsc::Sender<Result<String, StoreError>>,
    },
    Shutdown,
}

struct WorkspaceCommit {
    expected_version: Option<u64>,
    workspace: Workspace,
    draft: EventDraft,
    state_ref: AggregateStateRef,
}

impl SqliteWorkspaceStore {
    pub fn open(
        database_path: impl AsRef<Path>,
        blobs: Arc<dyn BlobStore>,
        config: SqliteConfig,
    ) -> Result<Self, StoreError> {
        if config.writer_queue_capacity == 0 {
            return Err(StoreError::Invalid(
                "writer queue capacity must be greater than zero".to_owned(),
            ));
        }
        let database_path = database_path.as_ref().to_path_buf();
        let parent = database_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or_else(|| {
                StoreError::Invalid(
                    "SQLite database must be inside an explicit private state directory".to_owned(),
                )
            })?;
        ensure_private_directory(parent)?;

        let (sender, receiver) = mpsc::sync_channel(config.writer_queue_capacity);
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let worker_path = database_path.clone();
        let worker_config = config.clone();
        let join = thread::Builder::new()
            .name("litecowork-sqlite-writer".to_owned())
            .spawn(move || writer_loop(worker_path, worker_config, receiver, ready_sender))
            .map_err(|error| StoreError::Io(error.to_string()))?;

        match ready_receiver.recv() {
            Ok(Ok(())) => Ok(Self {
                inner: Arc::new(Inner {
                    sender,
                    join: Mutex::new(Some(join)),
                    blobs,
                    writer_metrics: SqliteWriterMetrics::default(),
                }),
            }),
            Ok(Err(error)) => {
                let _ = join.join();
                Err(error)
            }
            Err(_) => {
                let _ = join.join();
                Err(StoreError::ExecutorStopped)
            }
        }
    }

    pub fn rebuild_workspace_projection(
        &self,
        workspace_id: &str,
    ) -> Result<Option<Workspace>, StoreError> {
        let events = self.read_workspace_events(workspace_id)?;
        if events.is_empty() {
            return Ok(None);
        }

        let mut projection: Option<Workspace> = None;
        let mut previous_revision = 0_u64;
        for event in events {
            let state_ref = &event.aggregate_state_ref;
            if event.entity_revision != state_ref.entity_revision
                || event.entity_revision != previous_revision + 1
            {
                return Err(StoreError::Integrity(
                    "Workspace event revisions are not contiguous".to_owned(),
                ));
            }
            let bytes =
                self.inner
                    .blobs
                    .get(workspace_id, BlobPurpose::AggregateState, &state_ref.blob)?;
            let value: Workspace = serde_json::from_slice(&bytes)
                .map_err(|error| StoreError::Integrity(error.to_string()))?;
            let canonical = canonical_json(&value)?;
            if canonical != bytes
                || value.workspace_id != workspace_id
                || value.version != event.entity_revision
            {
                return Err(StoreError::Integrity(
                    "Workspace aggregate-state blob does not match its event".to_owned(),
                ));
            }
            let expected_payload_digest = digest(&canonical_json(&event.payload)?);
            if expected_payload_digest != event.payload_digest {
                return Err(StoreError::Integrity(
                    "event payload digest does not match canonical payload".to_owned(),
                ));
            }
            previous_revision = event.entity_revision;
            projection = Some(value);
        }
        Ok(projection)
    }

    pub fn sqlite_version(&self) -> Result<String, StoreError> {
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::SqliteVersion {
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    /// Reports process-local writer dispatch pressure; this is operational telemetry,
    /// not durable Workspace or Task state.
    pub fn writer_metrics_snapshot(&self) -> SqliteWriterMetricsSnapshot {
        SqliteWriterMetricsSnapshot {
            outstanding_commands: self
                .inner
                .writer_metrics
                .outstanding_commands
                .load(Ordering::Relaxed),
            outstanding_commands_peak: self
                .inner
                .writer_metrics
                .outstanding_commands_peak
                .load(Ordering::Relaxed),
            send_wait_nanos_total: self
                .inner
                .writer_metrics
                .send_wait_nanos_total
                .load(Ordering::Relaxed),
            send_wait_nanos_max: self
                .inner
                .writer_metrics
                .send_wait_nanos_max
                .load(Ordering::Relaxed),
        }
    }

    fn execute_command<T>(
        &self,
        command: Command,
        reply_receiver: Receiver<Result<T, StoreError>>,
    ) -> Result<T, StoreError> {
        let metrics = &self.inner.writer_metrics;
        let outstanding = metrics
            .outstanding_commands
            .fetch_add(1, Ordering::Relaxed)
            .saturating_add(1);
        metrics
            .outstanding_commands_peak
            .fetch_max(outstanding, Ordering::Relaxed);

        let send_started = Instant::now();
        let sent = self.inner.sender.send(command);
        let send_wait = duration_as_nanos(send_started.elapsed());
        atomic_saturating_add(&metrics.send_wait_nanos_total, send_wait);
        metrics
            .send_wait_nanos_max
            .fetch_max(send_wait, Ordering::Relaxed);
        if sent.is_err() {
            metrics.outstanding_commands.fetch_sub(1, Ordering::Relaxed);
            return Err(StoreError::ExecutorStopped);
        }

        let response = reply_receiver.recv();
        metrics.outstanding_commands.fetch_sub(1, Ordering::Relaxed);
        response.map_err(|_| StoreError::ExecutorStopped)?
    }
}

impl StateStore for SqliteWorkspaceStore {
    fn get_workspace(&self, workspace_id: &str) -> Result<Option<Workspace>, StoreError> {
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::GetWorkspace {
                workspace_id: workspace_id.to_owned(),
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn commit_workspace(
        &self,
        expected_version: Option<u64>,
        workspace: Workspace,
        event: EventDraft,
    ) -> Result<CommittedWorkspace, StoreError> {
        validate_commit(&workspace, &event, expected_version)?;
        let state_bytes = canonical_json(&workspace)?;
        let state_blob = self.inner.blobs.put(
            &workspace.workspace_id,
            BlobPurpose::AggregateState,
            &state_bytes,
            STATE_MEDIA_TYPE,
        )?;
        if state_blob.size_bytes != state_bytes.len() as u64
            || self.inner.blobs.get(
                &workspace.workspace_id,
                BlobPurpose::AggregateState,
                &state_blob,
            )? != state_bytes
        {
            return Err(StoreError::Integrity(
                "BlobStore did not durably verify aggregate state".to_owned(),
            ));
        }
        let state_ref = AggregateStateRef {
            blob: state_blob,
            entity_revision: workspace.version,
            record_schema_version: 1,
        };
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::CommitWorkspace {
                commit: Box::new(WorkspaceCommit {
                    expected_version,
                    workspace,
                    draft: event,
                    state_ref,
                }),
                reply: reply_sender,
            },
            reply_receiver,
        )
    }
}

impl EventStore for SqliteWorkspaceStore {
    fn read_workspace_events(&self, workspace_id: &str) -> Result<Vec<DomainEvent>, StoreError> {
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::ReadWorkspaceEvents {
                workspace_id: workspace_id.to_owned(),
                reply: reply_sender,
            },
            reply_receiver,
        )
    }
}

fn duration_as_nanos(duration: Duration) -> u64 {
    duration.as_nanos().min(u64::MAX as u128) as u64
}

fn atomic_saturating_add(counter: &AtomicU64, value: u64) {
    let mut current = counter.load(Ordering::Relaxed);
    loop {
        let next = current.saturating_add(value);
        match counter.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return,
            Err(observed) => current = observed,
        }
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        let _ = self.sender.send(Command::Shutdown);
        if let Ok(mut join) = self.join.lock() {
            if let Some(handle) = join.take() {
                let _ = handle.join();
            }
        }
    }
}

fn writer_loop(
    path: PathBuf,
    config: SqliteConfig,
    receiver: Receiver<Command>,
    ready: SyncSender<Result<(), StoreError>>,
) {
    let connection = open_and_migrate(&path, &config);
    match connection {
        Ok(mut connection) => {
            if ready.send(Ok(())).is_err() {
                return;
            }
            while let Ok(command) = receiver.recv() {
                match command {
                    Command::GetWorkspace {
                        workspace_id,
                        reply,
                    } => {
                        let _ = reply.send(load_workspace(&connection, &workspace_id));
                    }
                    Command::CommitWorkspace { commit, reply } => {
                        let _ = reply.send(commit_workspace_transaction(
                            &mut connection,
                            commit.expected_version,
                            commit.workspace,
                            commit.draft,
                            commit.state_ref,
                            None,
                        ));
                    }
                    Command::ReadWorkspaceEvents {
                        workspace_id,
                        reply,
                    } => {
                        let _ = reply.send(read_workspace_events(&connection, &workspace_id));
                    }
                    Command::SqliteVersion { reply } => {
                        let version = connection
                            .query_row("SELECT sqlite_version()", [], |row| row.get(0))
                            .map_err(map_database_error);
                        let _ = reply.send(version);
                    }
                    Command::Shutdown => break,
                }
            }
        }
        Err(error) => {
            let _ = ready.send(Err(error));
        }
    }
}

fn open_and_migrate(path: &Path, config: &SqliteConfig) -> Result<Connection, StoreError> {
    validate_database_file_path(path)?;
    let mut connection = Connection::open(path).map_err(map_database_error)?;
    restrict_database_file(path)?;
    connection
        .busy_timeout(config.busy_timeout)
        .map_err(map_database_error)?;
    connection
        .pragma_update(None, "foreign_keys", true)
        .map_err(map_database_error)?;
    let foreign_keys: i64 = connection
        .pragma_query_value(None, "foreign_keys", |row| row.get(0))
        .map_err(map_database_error)?;
    if foreign_keys != 1 {
        return Err(StoreError::CorruptSchema(
            "foreign-key enforcement is unavailable".to_owned(),
        ));
    }
    let journal_mode: String = connection
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .map_err(map_database_error)?;
    if !journal_mode.eq_ignore_ascii_case("wal") {
        let selected: String = connection
            .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
            .map_err(map_database_error)?;
        if !selected.eq_ignore_ascii_case("wal") {
            return Err(StoreError::CorruptSchema(
                "SQLite did not enable WAL mode".to_owned(),
            ));
        }
    }
    connection
        .pragma_update(None, "synchronous", "FULL")
        .map_err(map_database_error)?;
    validate_sqlite_runtime(&connection)?;
    migrate(&mut connection)?;
    validate_schema(&connection)?;
    Ok(connection)
}

fn ensure_private_directory(path: &Path) -> Result<(), StoreError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(StoreError::Io(
                    "SQLite state path must be a real directory".to_owned(),
                ));
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if metadata.permissions().mode() & 0o077 != 0 {
                    return Err(StoreError::Io(
                        "SQLite state directory grants group or other access".to_owned(),
                    ));
                }
            }
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                let mut builder = fs::DirBuilder::new();
                builder.recursive(true).mode(0o700);
                builder.create(path).map_err(map_io_error)?;
            }
            #[cfg(not(unix))]
            fs::create_dir_all(path).map_err(map_io_error)?;
            ensure_private_directory(path)
        }
        Err(error) => Err(map_io_error(error)),
    }
}

fn restrict_database_file(path: &Path) -> Result<(), StoreError> {
    let metadata = fs::symlink_metadata(path).map_err(map_io_error)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(StoreError::Io(
            "SQLite database path must be a regular file".to_owned(),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(map_io_error)?;
    }
    Ok(())
}

fn validate_database_file_path(path: &Path) -> Result<(), StoreError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => Err(
            StoreError::Io("SQLite database path must be a regular file".to_owned()),
        ),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(map_io_error(error)),
    }
}

fn map_io_error(error: std::io::Error) -> StoreError {
    StoreError::Io(error.to_string())
}

fn validate_sqlite_runtime(connection: &Connection) -> Result<(), StoreError> {
    let version: String = connection
        .query_row("SELECT sqlite_version()", [], |row| row.get(0))
        .map_err(map_database_error)?;
    let parts: Vec<u32> = version
        .split('.')
        .map(|part| part.parse::<u32>().unwrap_or(0))
        .collect();
    let actual = (
        parts.first().copied().unwrap_or(0),
        parts.get(1).copied().unwrap_or(0),
        parts.get(2).copied().unwrap_or(0),
    );
    if actual < (3, 38, 0) {
        return Err(StoreError::CorruptSchema(format!(
            "SQLite {version} is older than the required 3.38"
        )));
    }
    let json_functions: i64 = connection
        .query_row("SELECT json_valid('{}')", [], |row| row.get(0))
        .map_err(map_database_error)?;
    if json_functions != 1 {
        return Err(StoreError::CorruptSchema(
            "SQLite JSON functions are unavailable".to_owned(),
        ));
    }
    Ok(())
}

fn migrate(connection: &mut Connection) -> Result<(), StoreError> {
    migrate_with_failpoint(connection, None)
}

fn migrate_with_failpoint(
    connection: &mut Connection,
    failpoint: Option<Failpoint>,
) -> Result<(), StoreError> {
    let schema_version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(map_database_error)?;
    if schema_version > SCHEMA_VERSION {
        return Err(StoreError::UnsupportedSchema(schema_version));
    }

    if schema_version == 0 {
        let object_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'",
                [],
                |row| row.get(0),
            )
            .map_err(map_database_error)?;
        if object_count != 0 {
            return Err(StoreError::CorruptSchema(
                "unversioned database is not empty".to_owned(),
            ));
        }

        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_database_error)?;
        transaction
            .execute_batch(SQLITE_V1_DDL)
            .map_err(map_database_error)?;
        fail_if(failpoint, Failpoint::DuringMigration)?;
        let fingerprint = schema_fingerprint(&transaction)?;
        let source_checksum = digest(SQLITE_V1_DDL.as_bytes());
        transaction
            .execute(
                "INSERT INTO schema_migrations(version, name, source_checksum, schema_fingerprint, applied_at) VALUES (?1, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
                params![SCHEMA_VERSION, MIGRATION_NAME, source_checksum, fingerprint],
            )
            .map_err(map_database_error)?;
        transaction
            .pragma_update(None, "user_version", SCHEMA_VERSION)
            .map_err(map_database_error)?;
        transaction.commit().map_err(map_database_error)?;
    } else {
        let (name, source_checksum, stored_fingerprint): (String, String, String) = connection
            .query_row(
                "SELECT name, source_checksum, schema_fingerprint FROM schema_migrations WHERE version = ?1",
                [SCHEMA_VERSION],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(map_database_error)?
            .ok_or_else(|| StoreError::CorruptSchema("migration record is missing".to_owned()))?;
        if name != MIGRATION_NAME || source_checksum != digest(SQLITE_V1_DDL.as_bytes()) {
            return Err(StoreError::CorruptSchema(
                "applied migration source checksum changed".to_owned(),
            ));
        }
        let actual_fingerprint = schema_fingerprint(connection)?;
        if stored_fingerprint != actual_fingerprint {
            return Err(StoreError::CorruptSchema(
                "database schema objects differ from the applied migration".to_owned(),
            ));
        }
    }
    Ok(())
}

fn validate_schema(connection: &Connection) -> Result<(), StoreError> {
    let violations: Vec<String> = {
        let mut statement = connection
            .prepare("PRAGMA foreign_key_check")
            .map_err(map_database_error)?;
        let rows = statement
            .query_map([], |row| {
                let table: String = row.get(0)?;
                let rowid: Option<i64> = row.get(1)?;
                Ok(format!("{table}:{rowid:?}"))
            })
            .map_err(map_database_error)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(map_database_error)?
    };
    if !violations.is_empty() {
        return Err(StoreError::CorruptSchema(format!(
            "foreign-key violations: {}",
            violations.join(", ")
        )));
    }
    Ok(())
}

fn schema_fingerprint(connection: &Connection) -> Result<String, StoreError> {
    let mut statement = connection
        .prepare(
            "SELECT type, name, tbl_name, COALESCE(sql, '') FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name",
        )
        .map_err(map_database_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok(json!({
                "type": row.get::<_, String>(0)?,
                "name": row.get::<_, String>(1)?,
                "table": row.get::<_, String>(2)?,
                "sql": row.get::<_, String>(3)?,
            }))
        })
        .map_err(map_database_error)?;
    let objects = rows
        .collect::<Result<Vec<Value>, _>>()
        .map_err(map_database_error)?;
    Ok(digest(&canonical_json(&objects)?))
}

fn validate_commit(
    workspace: &Workspace,
    event: &EventDraft,
    expected_version: Option<u64>,
) -> Result<(), StoreError> {
    let next_version = expected_version
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| StoreError::Invalid("aggregate version overflow".to_owned()))?;
    if workspace.workspace_id != event.workspace_id
        || workspace.workspace_id != event.entity_id
        || event.entity_type != "Workspace"
        || workspace.version != next_version
        || workspace.version != event.entity_revision
    {
        return Err(StoreError::Invalid(
            "aggregate and event identity/revision do not match".to_owned(),
        ));
    }
    if event.schema_version != 1 || !event.event_type.ends_with(".v1") {
        return Err(StoreError::Invalid(
            "unsupported domain event version".to_owned(),
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Failpoint {
    DuringMigration,
    AfterAggregate,
    AfterSequence,
    AfterEvent,
    BeforeCommit,
}

fn commit_workspace_transaction(
    connection: &mut Connection,
    expected_version: Option<u64>,
    workspace: Workspace,
    draft: EventDraft,
    state_ref: AggregateStateRef,
    failpoint: Option<Failpoint>,
) -> Result<CommittedWorkspace, StoreError> {
    validate_commit(&workspace, &draft, expected_version)?;
    if state_ref.entity_revision != workspace.version || state_ref.record_schema_version != 1 {
        return Err(StoreError::Invalid(
            "aggregate state reference revision/schema mismatch".to_owned(),
        ));
    }
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    let actual_version = transaction
        .query_row(
            "SELECT version FROM workspaces WHERE workspace_id = ?1",
            [&workspace.workspace_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(map_database_error)?
        .map(|version| from_sql_i64(version, "Workspace version"))
        .transpose()?;
    if actual_version != expected_version {
        return Err(StoreError::Conflict {
            expected: expected_version,
            actual: actual_version,
        });
    }

    match expected_version {
        None => {
            let version = to_sql_i64(workspace.version, "Workspace version")?;
            transaction
                .execute(
                    "INSERT INTO workspaces(workspace_id, name, owner_principal_id, replication_policy, current_instruction_revision, default_agent_binding_id, primary_coworker_id, hub_runtime_id, status, created_at, updated_at, version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                    params![
                        workspace.workspace_id,
                        workspace.name,
                        workspace.owner_principal_id,
                        workspace.replication_policy.as_str(),
                        workspace.current_instruction_revision.map(|value| to_sql_i64(value, "instruction revision")).transpose()?,
                        workspace.default_agent_binding_id,
                        workspace.primary_coworker_id,
                        workspace.hub_runtime_id,
                        workspace.status,
                        workspace.created_at,
                        workspace.updated_at,
                        version,
                    ],
                )
                .map_err(map_database_error)?;
        }
        Some(expected) => {
            let version = to_sql_i64(workspace.version, "Workspace version")?;
            let expected_sql = to_sql_i64(expected, "expected Workspace version")?;
            let affected = transaction
                .execute(
                    "UPDATE workspaces SET name = ?1, replication_policy = ?2, current_instruction_revision = ?3, default_agent_binding_id = ?4, primary_coworker_id = ?5, hub_runtime_id = ?6, status = ?7, updated_at = ?8, version = ?9 WHERE workspace_id = ?10 AND version = ?11",
                    params![
                        workspace.name,
                        workspace.replication_policy.as_str(),
                        workspace.current_instruction_revision.map(|value| to_sql_i64(value, "instruction revision")).transpose()?,
                        workspace.default_agent_binding_id,
                        workspace.primary_coworker_id,
                        workspace.hub_runtime_id,
                        workspace.status,
                        workspace.updated_at,
                        version,
                        workspace.workspace_id,
                        expected_sql,
                    ],
                )
                .map_err(map_database_error)?;
            if affected != 1 {
                return Err(StoreError::Conflict {
                    expected: Some(expected),
                    actual: None,
                });
            }
            transaction
                .execute(
                    "DELETE FROM workspace_replication_roots WHERE workspace_id = ?1",
                    [&workspace.workspace_id],
                )
                .map_err(map_database_error)?;
        }
    }
    fail_if(failpoint, Failpoint::AfterAggregate)?;

    for root_id in &workspace.replication_scope_root_ids {
        let active: Option<i64> = transaction
            .query_row(
                "SELECT 1 FROM workspace_roots WHERE workspace_id = ?1 AND workspace_root_id = ?2 AND status = 'ACTIVE'",
                params![workspace.workspace_id, root_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_database_error)?;
        if active.is_none() || workspace.replication_policy != ReplicationPolicy::SelectedFolders {
            return Err(StoreError::Invalid(
                "replication scope must reference active roots in SELECTED_FOLDERS mode".to_owned(),
            ));
        }
        transaction
            .execute(
                "INSERT INTO workspace_replication_roots(workspace_id, workspace_root_id) VALUES (?1, ?2)",
                params![workspace.workspace_id, root_id],
            )
            .map_err(map_database_error)?;
    }

    transaction
        .execute(
            "INSERT INTO workspace_origin_sequences(workspace_id, origin_runtime_id, last_sequence) VALUES (?1, ?2, 1) ON CONFLICT(workspace_id, origin_runtime_id) DO UPDATE SET last_sequence = last_sequence + 1",
            params![draft.workspace_id, draft.origin_runtime_id],
        )
        .map_err(map_database_error)?;
    let origin_sequence_sql: i64 = transaction
        .query_row(
            "SELECT last_sequence FROM workspace_origin_sequences WHERE workspace_id = ?1 AND origin_runtime_id = ?2",
            params![draft.workspace_id, draft.origin_runtime_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    let origin_sequence = from_sql_i64(origin_sequence_sql, "origin sequence")?;
    fail_if(failpoint, Failpoint::AfterSequence)?;

    let payload_json = String::from_utf8(canonical_json(&draft.payload)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let payload_digest = digest(payload_json.as_bytes());
    let state_ref_json = String::from_utf8(canonical_json(&state_ref)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let event = DomainEvent {
        event_id: draft.event_id,
        workspace_id: draft.workspace_id,
        entity_type: draft.entity_type,
        entity_id: draft.entity_id,
        origin_runtime_id: draft.origin_runtime_id,
        origin_sequence,
        entity_revision: draft.entity_revision,
        hlc_timestamp: draft.hlc_timestamp,
        correlation_id: draft.correlation_id,
        causation_id: draft.causation_id,
        schema_version: draft.schema_version,
        event_type: draft.event_type,
        payload: draft.payload,
        aggregate_state_ref: state_ref,
        recorded_at: draft.recorded_at,
        payload_digest,
    };
    transaction
        .execute(
            "INSERT INTO domain_events(event_id, workspace_id, entity_type, entity_id, origin_runtime_id, origin_sequence, entity_revision, hlc_timestamp, correlation_id, causation_id, schema_version, type, payload_json, aggregate_state_ref_json, recorded_at, payload_digest) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                event.event_id,
                event.workspace_id,
                event.entity_type,
                event.entity_id,
                event.origin_runtime_id,
                to_sql_i64(event.origin_sequence, "origin sequence")?,
                to_sql_i64(event.entity_revision, "event revision")?,
                event.hlc_timestamp,
                event.correlation_id,
                event.causation_id,
                i64::from(event.schema_version),
                event.event_type,
                payload_json,
                state_ref_json,
                event.recorded_at,
                event.payload_digest,
            ],
        )
        .map_err(map_database_error)?;
    fail_if(failpoint, Failpoint::AfterEvent)?;
    fail_if(failpoint, Failpoint::BeforeCommit)?;
    transaction.commit().map_err(map_database_error)?;

    Ok(CommittedWorkspace { workspace, event })
}

fn fail_if(actual: Option<Failpoint>, at: Failpoint) -> Result<(), StoreError> {
    if actual == Some(at) {
        return Err(StoreError::Database(format!("injected {at:?}")));
    }
    Ok(())
}

type WorkspaceRow = (
    String,
    String,
    String,
    String,
    Option<i64>,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
    String,
    String,
    i64,
);

fn load_workspace(
    connection: &Connection,
    workspace_id: &str,
) -> Result<Option<Workspace>, StoreError> {
    let base: Option<WorkspaceRow> = connection
        .query_row(
            "SELECT workspace_id, name, owner_principal_id, replication_policy, current_instruction_revision, default_agent_binding_id, primary_coworker_id, hub_runtime_id, status, created_at, updated_at, version FROM workspaces WHERE workspace_id = ?1",
            [workspace_id],
            |row| Ok((
                row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?,
                row.get(6)?, row.get(7)?, row.get(8)?, row.get(9)?, row.get(10)?, row.get(11)?,
            )),
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((
        id,
        name,
        owner,
        policy,
        instruction_revision,
        default_agent,
        primary_coworker,
        hub_runtime,
        status,
        created_at,
        updated_at,
        version_sql,
    )) = base
    else {
        return Ok(None);
    };

    let roots = {
        let mut statement = connection
            .prepare("SELECT workspace_root_id FROM workspace_replication_roots WHERE workspace_id = ?1 ORDER BY workspace_root_id")
            .map_err(map_database_error)?;
        statement
            .query_map([workspace_id], |row| row.get::<_, String>(0))
            .map_err(map_database_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_database_error)?
    };

    Ok(Some(Workspace {
        workspace_id: id,
        name,
        owner_principal_id: owner,
        replication_policy: parse_policy(&policy)?,
        replication_scope_root_ids: roots,
        current_instruction_revision: instruction_revision
            .map(|value| from_sql_i64(value, "instruction revision"))
            .transpose()?,
        default_agent_binding_id: default_agent,
        primary_coworker_id: primary_coworker,
        hub_runtime_id: hub_runtime,
        status,
        created_at,
        updated_at,
        version: from_sql_i64(version_sql, "Workspace version")?,
    }))
}

fn parse_policy(value: &str) -> Result<ReplicationPolicy, StoreError> {
    match value {
        "LOCAL_ONLY" => Ok(ReplicationPolicy::LocalOnly),
        "METADATA_ONLY" => Ok(ReplicationPolicy::MetadataOnly),
        "ACTIVE_TASK_INPUTS" => Ok(ReplicationPolicy::ActiveTaskInputs),
        "SELECTED_FOLDERS" => Ok(ReplicationPolicy::SelectedFolders),
        "FULL_WORKSPACE" => Ok(ReplicationPolicy::FullWorkspace),
        _ => Err(StoreError::Integrity(
            "unknown stored replication policy".to_owned(),
        )),
    }
}

fn read_workspace_events(
    connection: &Connection,
    workspace_id: &str,
) -> Result<Vec<DomainEvent>, StoreError> {
    let mut statement = connection
        .prepare(
            "SELECT event_id, workspace_id, entity_type, entity_id, origin_runtime_id, origin_sequence, entity_revision, hlc_timestamp, correlation_id, causation_id, schema_version, type, payload_json, aggregate_state_ref_json, recorded_at, payload_digest FROM domain_events WHERE workspace_id = ?1 AND entity_type = 'Workspace' AND entity_id = ?1 ORDER BY entity_revision",
        )
        .map_err(map_database_error)?;
    let rows = statement
        .query_map([workspace_id], |row| {
            let payload_json: String = row.get(12)?;
            let state_ref_json: String = row.get(13)?;
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, i64>(10)?,
                row.get::<_, String>(11)?,
                payload_json,
                state_ref_json,
                row.get::<_, String>(14)?,
                row.get::<_, String>(15)?,
            ))
        })
        .map_err(map_database_error)?;
    let mut events = Vec::new();
    for row in rows {
        let row = row.map_err(map_database_error)?;
        events.push(DomainEvent {
            event_id: row.0,
            workspace_id: row.1,
            entity_type: row.2,
            entity_id: row.3,
            origin_runtime_id: row.4,
            origin_sequence: from_sql_i64(row.5, "origin sequence")?,
            entity_revision: from_sql_i64(row.6, "event revision")?,
            hlc_timestamp: row.7,
            correlation_id: row.8,
            causation_id: row.9,
            schema_version: u32::try_from(row.10)
                .map_err(|_| StoreError::Integrity("event schema version is invalid".to_owned()))?,
            event_type: row.11,
            payload: serde_json::from_str(&row.12)
                .map_err(|error| StoreError::Integrity(error.to_string()))?,
            aggregate_state_ref: serde_json::from_str(&row.13)
                .map_err(|error| StoreError::Integrity(error.to_string()))?,
            recorded_at: row.14,
            payload_digest: row.15,
        });
    }
    Ok(events)
}

fn canonical_json<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, StoreError> {
    let value = serde_json::to_value(value)
        .map_err(|error| StoreError::Invalid(format!("value is not valid JSON: {error}")))?;
    validate_jcs_numbers(&value)?;
    serde_json_canonicalizer::to_vec(&value)
        .map_err(|error| StoreError::Invalid(format!("value is not canonical JSON: {error}")))
}

fn validate_jcs_numbers(value: &Value) -> Result<(), StoreError> {
    const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
    match value {
        Value::Number(number) if number.is_u64() => {
            if number
                .as_u64()
                .is_some_and(|integer| integer > MAX_SAFE_INTEGER)
            {
                return Err(StoreError::Invalid(
                    "integer exceeds the exact RFC 8785/I-JSON range; encode it as a string"
                        .to_owned(),
                ));
            }
        }
        Value::Number(number) if number.is_i64() => {
            if number
                .as_i64()
                .is_some_and(|integer| integer.unsigned_abs() > MAX_SAFE_INTEGER)
            {
                return Err(StoreError::Invalid(
                    "integer exceeds the exact RFC 8785/I-JSON range; encode it as a string"
                        .to_owned(),
                ));
            }
        }
        Value::Array(items) => {
            for item in items {
                validate_jcs_numbers(item)?;
            }
        }
        Value::Object(fields) => {
            for item in fields.values() {
                validate_jcs_numbers(item)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn to_sql_i64(value: u64, field: &str) -> Result<i64, StoreError> {
    i64::try_from(value)
        .map_err(|_| StoreError::Invalid(format!("{field} exceeds SQLite INTEGER range")))
}

fn from_sql_i64(value: i64, field: &str) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| StoreError::Integrity(format!("stored {field} is negative")))
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn map_database_error(error: rusqlite::Error) -> StoreError {
    if let rusqlite::Error::SqliteFailure(code, _) = &error {
        match code.code {
            ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked => return StoreError::Busy,
            ErrorCode::DiskFull => return StoreError::Io("database disk is full".to_owned()),
            ErrorCode::ConstraintViolation => {
                return StoreError::Database("database constraint rejected the write".to_owned());
            }
            _ => {}
        }
    }
    StoreError::Database(error.to_string())
}

#[cfg(test)]
mod tests;
