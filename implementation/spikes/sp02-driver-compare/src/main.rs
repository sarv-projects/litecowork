#[path = "../../../../crates/storage-sqlite/src/blob.rs"]
mod blob;

use blob::{FileBlobStore, WorkspaceBlobKey, WorkspaceBlobKeyProvider};
use domain_workspace::{ChangeReplicationPolicy, CreateWorkspace, EventContext, WorkspaceService};
use sqlx::{
    Connection, Row,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous},
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use storage_core::{
    AggregateStateRef, BlobPurpose, BlobStore, CommittedWorkspace, DomainEvent, EventDraft,
    EventStore, ReplicationPolicy, StateStore, StoreError, Workspace,
};
use tempfile::TempDir;
use tokio::runtime::Builder;
use zeroize::Zeroizing;

const DDL: &str = include_str!("../../../../docs/schemas/sqlite-v1.sql");
const PRODUCERS: usize = 8;
const STATE_MEDIA_TYPE: &str = "application/vnd.litecowork.aggregate-state+json";

#[derive(Clone)]
struct TestKeys;

impl WorkspaceBlobKeyProvider for TestKeys {
    fn current_key(
        &self,
        _workspace_id: &str,
        _purpose: BlobPurpose,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        Ok(WorkspaceBlobKey {
            version: 1,
            bytes: Zeroizing::new([19_u8; 32]),
        })
    }

    fn key_by_version(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        version: u32,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        if version != 1 {
            return Err(StoreError::Blob("unknown SP02 test key version".to_owned()));
        }
        self.current_key(workspace_id, purpose)
    }
}

#[derive(Clone)]
struct SqlxWorkspaceStore {
    inner: Arc<SqlxInner>,
}

struct SqlxInner {
    sender: mpsc::SyncSender<Command>,
    join: Mutex<Option<JoinHandle<()>>>,
    blobs: Arc<dyn BlobStore>,
}

enum Command {
    GetWorkspace {
        workspace_id: String,
        reply: mpsc::Sender<Result<Option<Workspace>, StoreError>>,
    },
    Commit {
        expected_version: Option<u64>,
        workspace: Box<Workspace>,
        draft: Box<EventDraft>,
        state_ref: AggregateStateRef,
        reply: mpsc::Sender<Result<CommittedWorkspace, StoreError>>,
    },
    ReadEvents {
        workspace_id: String,
        reply: mpsc::Sender<Result<Vec<DomainEvent>, StoreError>>,
    },
    SqliteVersion {
        reply: mpsc::Sender<Result<String, StoreError>>,
    },
    Shutdown,
}

#[derive(Debug)]
struct Summary {
    sqlite_version: String,
    elapsed: Duration,
    latency_p50: Duration,
    latency_p95: Duration,
    writes_per_second: f64,
    replayed_workspaces: usize,
    replay_matches: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let writes = parse_write_count()?;
    let summary = run(writes)?;
    println!(
        "SQLx prototype adapter (SQLite {}, encrypted aggregate-state blobs, bounded writer queue=32, WAL + FULL): updates={writes}, producers={PRODUCERS}, elapsed_ms={:.2}, throughput_per_s={:.1}, end_to_end_p50_us={}, end_to_end_p95_us={}, replayed_workspaces={}, replay_matches={}",
        summary.sqlite_version,
        summary.elapsed.as_secs_f64() * 1_000.0,
        summary.writes_per_second,
        summary.latency_p50.as_micros(),
        summary.latency_p95.as_micros(),
        summary.replayed_workspaces,
        summary.replay_matches,
    );
    Ok(())
}

impl SqlxWorkspaceStore {
    fn open(database: &Path, blobs: Arc<dyn BlobStore>) -> Result<Self, StoreError> {
        let parent = database
            .parent()
            .ok_or_else(|| StoreError::Invalid("database needs a parent directory".to_owned()))?;
        std::fs::create_dir_all(parent).map_err(|error| StoreError::Io(error.to_string()))?;
        let (sender, receiver) = mpsc::sync_channel(32);
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let worker_database = database.to_path_buf();
        let join = thread::Builder::new()
            .name("sp02-sqlx-writer".to_owned())
            .spawn(move || writer_thread(worker_database, receiver, ready_sender))
            .map_err(|error| StoreError::Io(error.to_string()))?;
        match ready_receiver.recv() {
            Ok(Ok(())) => Ok(Self {
                inner: Arc::new(SqlxInner {
                    sender,
                    join: Mutex::new(Some(join)),
                    blobs,
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

    fn request<T>(
        &self,
        build: impl FnOnce(mpsc::Sender<Result<T, StoreError>>) -> Command,
    ) -> Result<T, StoreError> {
        let (reply, response) = mpsc::channel();
        self.inner
            .sender
            .send(build(reply))
            .map_err(|_| StoreError::ExecutorStopped)?;
        response.recv().map_err(|_| StoreError::ExecutorStopped)?
    }

    fn sqlite_version(&self) -> Result<String, StoreError> {
        self.request(|reply| Command::SqliteVersion { reply })
    }

    fn replay_workspace(&self, workspace_id: &str) -> Result<Option<Workspace>, StoreError> {
        let events = self.read_workspace_events(workspace_id)?;
        let mut projection = None;
        let mut previous_revision = 0;
        for event in events {
            let state_ref = &event.aggregate_state_ref;
            if event.entity_revision != previous_revision + 1
                || event.entity_revision != state_ref.entity_revision
            {
                return Err(StoreError::Integrity(
                    "Workspace replay revision chain is not contiguous".to_owned(),
                ));
            }
            let bytes =
                self.inner
                    .blobs
                    .get(workspace_id, BlobPurpose::AggregateState, &state_ref.blob)?;
            let workspace: Workspace = serde_json::from_slice(&bytes)
                .map_err(|error| StoreError::Integrity(error.to_string()))?;
            let canonical = canonical_json(&workspace)?;
            if canonical != bytes
                || workspace.workspace_id != workspace_id
                || workspace.version != event.entity_revision
                || digest(&canonical_json(&event.payload)?) != event.payload_digest
            {
                return Err(StoreError::Integrity(
                    "aggregate-state or event payload failed replay verification".to_owned(),
                ));
            }
            previous_revision = event.entity_revision;
            projection = Some(workspace);
        }
        Ok(projection)
    }
}

impl Drop for SqlxInner {
    fn drop(&mut self) {
        let _ = self.sender.send(Command::Shutdown);
        if let Some(join) = self.join.lock().expect("writer join mutex").take() {
            let _ = join.join();
        }
    }
}

impl StateStore for SqlxWorkspaceStore {
    fn get_workspace(&self, workspace_id: &str) -> Result<Option<Workspace>, StoreError> {
        self.request(|reply| Command::GetWorkspace {
            workspace_id: workspace_id.to_owned(),
            reply,
        })
    }

    fn commit_workspace(
        &self,
        expected_version: Option<u64>,
        workspace: Workspace,
        draft: EventDraft,
    ) -> Result<CommittedWorkspace, StoreError> {
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
        self.request(|reply| Command::Commit {
            expected_version,
            workspace: Box::new(workspace),
            draft: Box::new(draft),
            state_ref,
            reply,
        })
    }
}

impl EventStore for SqlxWorkspaceStore {
    fn read_workspace_events(&self, workspace_id: &str) -> Result<Vec<DomainEvent>, StoreError> {
        self.request(|reply| Command::ReadEvents {
            workspace_id: workspace_id.to_owned(),
            reply,
        })
    }
}

fn writer_thread(
    database: PathBuf,
    receiver: mpsc::Receiver<Command>,
    ready: mpsc::SyncSender<Result<(), StoreError>>,
) {
    let runtime = match Builder::new_current_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            let _ = ready.send(Err(StoreError::Database(error.to_string())));
            return;
        }
    };
    let mut connection = match runtime.block_on(open_connection(&database)) {
        Ok(connection) => connection,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    if ready.send(Ok(())).is_err() {
        return;
    }
    while let Ok(command) = receiver.recv() {
        match command {
            Command::GetWorkspace {
                workspace_id,
                reply,
            } => {
                let result = runtime.block_on(load_workspace(&mut connection, &workspace_id));
                let _ = reply.send(result);
            }
            Command::Commit {
                expected_version,
                workspace,
                draft,
                state_ref,
                reply,
            } => {
                let result = runtime.block_on(commit_workspace(
                    &mut connection,
                    expected_version,
                    *workspace,
                    *draft,
                    state_ref,
                ));
                let _ = reply.send(result);
            }
            Command::ReadEvents {
                workspace_id,
                reply,
            } => {
                let result = runtime.block_on(read_events(&mut connection, &workspace_id));
                let _ = reply.send(result);
            }
            Command::SqliteVersion { reply } => {
                let result = runtime.block_on(async {
                    sqlx::query("SELECT sqlite_version() AS version")
                        .fetch_one(&mut connection)
                        .await
                        .map_err(map_sqlx_error)?
                        .try_get("version")
                        .map_err(map_sqlx_error)
                });
                let _ = reply.send(result);
            }
            Command::Shutdown => break,
        }
    }
    let _ = runtime.block_on(connection.close());
}

async fn open_connection(database: &Path) -> Result<sqlx::SqliteConnection, StoreError> {
    let options = SqliteConnectOptions::new()
        .filename(database)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Full)
        .busy_timeout(Duration::from_secs(5));
    let mut connection = sqlx::SqliteConnection::connect_with(&options)
        .await
        .map_err(map_sqlx_error)?;
    let has_schema = sqlx::query("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='schema_migrations') AS present")
        .fetch_one(&mut connection)
        .await
        .map_err(map_sqlx_error)?
        .try_get::<i64, _>("present")
        .map_err(map_sqlx_error)?;
    if has_schema == 0 {
        sqlx::raw_sql(DDL)
            .execute(&mut connection)
            .await
            .map_err(map_sqlx_error)?;
    }
    Ok(connection)
}

async fn load_workspace(
    connection: &mut sqlx::SqliteConnection,
    workspace_id: &str,
) -> Result<Option<Workspace>, StoreError> {
    let row = sqlx::query("SELECT workspace_id,name,owner_principal_id,replication_policy,current_instruction_revision,default_agent_binding_id,primary_coworker_id,hub_runtime_id,status,created_at,updated_at,version FROM workspaces WHERE workspace_id=?1")
        .bind(workspace_id)
        .fetch_optional(&mut *connection)
        .await
        .map_err(map_sqlx_error)?;
    row.map(|row| {
        let policy: String = row.try_get("replication_policy").map_err(map_sqlx_error)?;
        let replication_policy = serde_json::from_value(serde_json::Value::String(policy))
            .map_err(|error| StoreError::Integrity(error.to_string()))?;
        Ok(Workspace {
            workspace_id: row.try_get("workspace_id").map_err(map_sqlx_error)?,
            name: row.try_get("name").map_err(map_sqlx_error)?,
            owner_principal_id: row.try_get("owner_principal_id").map_err(map_sqlx_error)?,
            replication_policy,
            replication_scope_root_ids: Vec::new(),
            current_instruction_revision: row
                .try_get::<Option<i64>, _>("current_instruction_revision")
                .map_err(map_sqlx_error)?
                .map(|value| {
                    u64::try_from(value).map_err(|_| {
                        StoreError::Integrity("negative instruction revision".to_owned())
                    })
                })
                .transpose()?,
            default_agent_binding_id: row
                .try_get("default_agent_binding_id")
                .map_err(map_sqlx_error)?,
            primary_coworker_id: row.try_get("primary_coworker_id").map_err(map_sqlx_error)?,
            hub_runtime_id: row.try_get("hub_runtime_id").map_err(map_sqlx_error)?,
            status: row.try_get("status").map_err(map_sqlx_error)?,
            created_at: row.try_get("created_at").map_err(map_sqlx_error)?,
            updated_at: row.try_get("updated_at").map_err(map_sqlx_error)?,
            version: u64::try_from(row.try_get::<i64, _>("version").map_err(map_sqlx_error)?)
                .map_err(|_| StoreError::Integrity("negative Workspace version".to_owned()))?,
        })
    })
    .transpose()
}

async fn commit_workspace(
    connection: &mut sqlx::SqliteConnection,
    expected_version: Option<u64>,
    workspace: Workspace,
    draft: EventDraft,
    state_ref: AggregateStateRef,
) -> Result<CommittedWorkspace, StoreError> {
    if workspace.workspace_id != draft.workspace_id
        || workspace.workspace_id != draft.entity_id
        || draft.entity_type != "Workspace"
        || workspace.version != expected_version.unwrap_or(0) + 1
        || workspace.version != draft.entity_revision
        || state_ref.entity_revision != workspace.version
        || state_ref.record_schema_version != 1
    {
        return Err(StoreError::Invalid(
            "aggregate/event identity or revision mismatch".to_owned(),
        ));
    }
    let payload_json = String::from_utf8(canonical_json(&draft.payload)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let payload_digest = digest(payload_json.as_bytes());
    let state_ref_json = String::from_utf8(canonical_json(&state_ref)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *connection)
        .await
        .map_err(map_sqlx_error)?;
    let result = async {
        let actual = sqlx::query("SELECT version FROM workspaces WHERE workspace_id=?1")
            .bind(&workspace.workspace_id)
            .fetch_optional(&mut *connection)
            .await?
            .map(|row| row.try_get::<i64, _>("version"))
            .transpose()?;
        let actual = actual.map(|value| u64::try_from(value).map_err(|_| sqlx::Error::Protocol("negative Workspace version".to_owned()))).transpose()?;
        if actual != expected_version {
            return Err(sqlx::Error::Protocol("Workspace version conflict".to_owned()));
        }
        match expected_version {
            None => {
                sqlx::query("INSERT INTO workspaces(workspace_id,name,owner_principal_id,replication_policy,current_instruction_revision,default_agent_binding_id,primary_coworker_id,hub_runtime_id,status,created_at,updated_at,version) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)")
                    .bind(&workspace.workspace_id).bind(&workspace.name).bind(&workspace.owner_principal_id).bind(workspace.replication_policy.as_str())
                    .bind(workspace.current_instruction_revision.map(|value| value as i64)).bind(&workspace.default_agent_binding_id).bind(&workspace.primary_coworker_id).bind(&workspace.hub_runtime_id)
                    .bind(&workspace.status).bind(&workspace.created_at).bind(&workspace.updated_at).bind(workspace.version as i64)
                    .execute(&mut *connection).await?;
            }
            Some(expected) => {
                let updated = sqlx::query("UPDATE workspaces SET name=?1,replication_policy=?2,current_instruction_revision=?3,default_agent_binding_id=?4,primary_coworker_id=?5,hub_runtime_id=?6,status=?7,updated_at=?8,version=?9 WHERE workspace_id=?10 AND version=?11")
                    .bind(&workspace.name).bind(workspace.replication_policy.as_str()).bind(workspace.current_instruction_revision.map(|value| value as i64))
                    .bind(&workspace.default_agent_binding_id).bind(&workspace.primary_coworker_id).bind(&workspace.hub_runtime_id).bind(&workspace.status)
                    .bind(&workspace.updated_at).bind(workspace.version as i64).bind(&workspace.workspace_id).bind(expected as i64)
                    .execute(&mut *connection).await?;
                if updated.rows_affected() != 1 {
                    return Err(sqlx::Error::Protocol("Workspace version conflict".to_owned()));
                }
                sqlx::query("DELETE FROM workspace_replication_roots WHERE workspace_id=?1")
                    .bind(&workspace.workspace_id).execute(&mut *connection).await?;
            }
        }
        sqlx::query("INSERT INTO workspace_origin_sequences(workspace_id,origin_runtime_id,last_sequence) VALUES (?1,?2,1) ON CONFLICT(workspace_id,origin_runtime_id) DO UPDATE SET last_sequence=last_sequence+1")
            .bind(&draft.workspace_id).bind(&draft.origin_runtime_id).execute(&mut *connection).await?;
        let sequence: i64 = sqlx::query("SELECT last_sequence FROM workspace_origin_sequences WHERE workspace_id=?1 AND origin_runtime_id=?2")
            .bind(&draft.workspace_id).bind(&draft.origin_runtime_id).fetch_one(&mut *connection).await?.try_get("last_sequence")?;
        let event = DomainEvent {
            event_id: draft.event_id,
            workspace_id: draft.workspace_id,
            entity_type: draft.entity_type,
            entity_id: draft.entity_id,
            origin_runtime_id: draft.origin_runtime_id,
            origin_sequence: u64::try_from(sequence).map_err(|_| sqlx::Error::Protocol("negative origin sequence".to_owned()))?,
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
        sqlx::query("INSERT INTO domain_events(event_id,workspace_id,entity_type,entity_id,origin_runtime_id,origin_sequence,entity_revision,hlc_timestamp,correlation_id,causation_id,schema_version,type,payload_json,aggregate_state_ref_json,recorded_at,payload_digest) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)")
            .bind(&event.event_id).bind(&event.workspace_id).bind(&event.entity_type).bind(&event.entity_id).bind(&event.origin_runtime_id)
            .bind(event.origin_sequence as i64).bind(event.entity_revision as i64).bind(&event.hlc_timestamp).bind(&event.correlation_id).bind(&event.causation_id)
            .bind(i64::from(event.schema_version)).bind(&event.event_type).bind(&payload_json).bind(&state_ref_json).bind(&event.recorded_at).bind(&event.payload_digest)
            .execute(&mut *connection).await?;
        Ok::<_, sqlx::Error>(CommittedWorkspace { workspace, event })
    }.await;
    match result {
        Ok(committed) => {
            sqlx::query("COMMIT")
                .execute(&mut *connection)
                .await
                .map_err(map_sqlx_error)?;
            Ok(committed)
        }
        Err(error) => {
            let _ = sqlx::query("ROLLBACK").execute(&mut *connection).await;
            Err(map_sqlx_error(error))
        }
    }
}

async fn read_events(
    connection: &mut sqlx::SqliteConnection,
    workspace_id: &str,
) -> Result<Vec<DomainEvent>, StoreError> {
    let rows = sqlx::query("SELECT event_id,workspace_id,entity_type,entity_id,origin_runtime_id,origin_sequence,entity_revision,hlc_timestamp,correlation_id,causation_id,schema_version,type,payload_json,aggregate_state_ref_json,recorded_at,payload_digest FROM domain_events WHERE workspace_id=?1 ORDER BY origin_sequence")
        .bind(workspace_id).fetch_all(&mut *connection).await.map_err(map_sqlx_error)?;
    rows.into_iter()
        .map(|row| {
            let payload: String = row.try_get("payload_json").map_err(map_sqlx_error)?;
            let state_ref: String = row
                .try_get("aggregate_state_ref_json")
                .map_err(map_sqlx_error)?;
            Ok(DomainEvent {
                event_id: row.try_get("event_id").map_err(map_sqlx_error)?,
                workspace_id: row.try_get("workspace_id").map_err(map_sqlx_error)?,
                entity_type: row.try_get("entity_type").map_err(map_sqlx_error)?,
                entity_id: row.try_get("entity_id").map_err(map_sqlx_error)?,
                origin_runtime_id: row.try_get("origin_runtime_id").map_err(map_sqlx_error)?,
                origin_sequence: u64::try_from(
                    row.try_get::<i64, _>("origin_sequence")
                        .map_err(map_sqlx_error)?,
                )
                .map_err(|_| StoreError::Integrity("negative origin sequence".to_owned()))?,
                entity_revision: u64::try_from(
                    row.try_get::<i64, _>("entity_revision")
                        .map_err(map_sqlx_error)?,
                )
                .map_err(|_| StoreError::Integrity("negative entity revision".to_owned()))?,
                hlc_timestamp: row.try_get("hlc_timestamp").map_err(map_sqlx_error)?,
                correlation_id: row.try_get("correlation_id").map_err(map_sqlx_error)?,
                causation_id: row.try_get("causation_id").map_err(map_sqlx_error)?,
                schema_version: u32::try_from(
                    row.try_get::<i64, _>("schema_version")
                        .map_err(map_sqlx_error)?,
                )
                .map_err(|_| StoreError::Integrity("invalid event schema version".to_owned()))?,
                event_type: row.try_get("type").map_err(map_sqlx_error)?,
                payload: serde_json::from_str(&payload)
                    .map_err(|error| StoreError::Integrity(error.to_string()))?,
                aggregate_state_ref: serde_json::from_str(&state_ref)
                    .map_err(|error| StoreError::Integrity(error.to_string()))?,
                recorded_at: row.try_get("recorded_at").map_err(map_sqlx_error)?,
                payload_digest: row.try_get("payload_digest").map_err(map_sqlx_error)?,
            })
        })
        .collect()
}

fn canonical_json<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, StoreError> {
    let value =
        serde_json::to_value(value).map_err(|error| StoreError::Invalid(error.to_string()))?;
    serde_json_canonicalizer::to_vec(&value).map_err(|error| StoreError::Invalid(error.to_string()))
}

fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn map_sqlx_error(error: sqlx::Error) -> StoreError {
    if let sqlx::Error::Database(database) = &error
        && (database.code().as_deref() == Some("5") || database.code().as_deref() == Some("6"))
    {
        return StoreError::Busy;
    }
    if error.to_string().contains("Workspace version conflict") {
        return StoreError::Conflict {
            expected: None,
            actual: None,
        };
    }
    StoreError::Database(error.to_string())
}

fn parse_write_count() -> Result<usize, Box<dyn std::error::Error>> {
    let writes = std::env::args()
        .nth(1)
        .map(|arg| arg.parse::<usize>())
        .transpose()?
        .unwrap_or(1_000);
    if writes == 0 {
        return Err("write count must be greater than zero".into());
    }
    Ok(writes)
}

fn run(writes: usize) -> Result<Summary, Box<dyn std::error::Error>> {
    let directory = TempDir::new()?;
    let blob_root = directory.path().join("blobs");
    let database = directory.path().join("state").join("state.sqlite3");
    let blobs = Arc::new(FileBlobStore::new(&blob_root, TestKeys));
    let store = SqlxWorkspaceStore::open(&database, blobs)?;
    let service = WorkspaceService::new(store.clone());
    let counts: Vec<_> = (0..PRODUCERS)
        .map(|producer| writes / PRODUCERS + usize::from(producer < writes % PRODUCERS))
        .collect();
    for producer in 0..PRODUCERS {
        service.create(CreateWorkspace {
            workspace_id: workspace_id(producer),
            name: format!("SP02 workspace {producer}"),
            owner_principal_id: "sp02-owner".to_owned(),
            event: event_context(&format!("create-{producer}")),
        })?;
    }
    let started = Instant::now();
    let mut producers = Vec::with_capacity(PRODUCERS);
    for (producer, count) in counts.iter().copied().enumerate() {
        let store = store.clone();
        producers.push(thread::spawn(move || {
            let service = WorkspaceService::new(store);
            let mut latencies = Vec::with_capacity(count);
            for update in 0..count {
                let started = Instant::now();
                service
                    .change_replication_policy(ChangeReplicationPolicy {
                        workspace_id: workspace_id(producer),
                        expected_version: update as u64 + 1,
                        policy: if update % 2 == 0 {
                            ReplicationPolicy::MetadataOnly
                        } else {
                            ReplicationPolicy::LocalOnly
                        },
                        replication_scope_root_ids: Vec::new(),
                        event: event_context(&format!("update-{producer}-{update}")),
                    })
                    .map_err(|error| error.to_string())?;
                latencies.push(started.elapsed());
            }
            Ok::<_, String>(latencies)
        }));
    }
    let mut latencies = Vec::with_capacity(writes);
    for producer in producers {
        latencies.extend(
            producer
                .join()
                .map_err(|_| std::io::Error::other("producer panicked"))?
                .map_err(std::io::Error::other)?,
        );
    }
    let elapsed = started.elapsed();
    drop(service);
    drop(store);
    let reopened_blobs = Arc::new(FileBlobStore::new(&blob_root, TestKeys));
    let reopened_store = SqlxWorkspaceStore::open(&database, reopened_blobs)?;
    let sqlite_version = reopened_store.sqlite_version()?;
    let mut replay_matches = true;
    let mut replayed_workspaces = 0;
    for (producer, count) in counts.iter().copied().enumerate() {
        let expected_version = count as u64 + 1;
        let projection = reopened_store
            .get_workspace(&workspace_id(producer))?
            .ok_or("missing SQLx Workspace projection")?;
        let replayed = reopened_store
            .replay_workspace(&workspace_id(producer))?
            .ok_or("missing SQLx replay result")?;
        replay_matches &= projection == replayed && replayed.version == expected_version;
        replayed_workspaces += 1;
    }
    if !replay_matches {
        return Err(
            "reopened SQLx Workspace projection does not match encrypted event replay".into(),
        );
    }
    latencies.sort_unstable();
    Ok(Summary {
        sqlite_version,
        elapsed,
        latency_p50: percentile(&latencies, 0.50),
        latency_p95: percentile(&latencies, 0.95),
        writes_per_second: writes as f64 / elapsed.as_secs_f64(),
        replayed_workspaces,
        replay_matches,
    })
}

fn workspace_id(producer: usize) -> String {
    format!("sp02-{producer}")
}

fn event_context(event_id: &str) -> EventContext {
    let timestamp = "2026-10-06T10:00:00Z".to_owned();
    EventContext {
        event_id: event_id.to_owned(),
        origin_runtime_id: "runtime-sp02".to_owned(),
        hlc_timestamp: timestamp.clone(),
        correlation_id: format!("correlation-{event_id}"),
        causation_id: None,
        recorded_at: timestamp,
    }
}

fn percentile(values: &[Duration], fraction: f64) -> Duration {
    values[((values.len() - 1) as f64 * fraction).ceil() as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypted_aggregate_state_replays_after_concurrent_workspace_updates() {
        let summary = run(12).expect("SQLx full-path qualification run");
        assert!(summary.sqlite_version.starts_with("3."));
        assert_eq!(summary.replayed_workspaces, PRODUCERS);
        assert!(summary.replay_matches);
    }
}
