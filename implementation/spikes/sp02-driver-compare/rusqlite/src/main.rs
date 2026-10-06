use domain_workspace::{ChangeReplicationPolicy, CreateWorkspace, EventContext, WorkspaceService};
use std::{
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
use storage_core::{BlobPurpose, ReplicationPolicy, StateStore, StoreError};
use storage_sqlite::{
    FileBlobStore, SqliteConfig, SqliteWorkspaceStore, WorkspaceBlobKey, WorkspaceBlobKeyProvider,
};
use tempfile::TempDir;
use zeroize::Zeroizing;

const PRODUCERS: usize = 8;

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
        "rusqlite product adapter (SQLite {}, encrypted aggregate-state blobs, bounded writer queue=32, WAL + FULL): updates={writes}, producers={PRODUCERS}, elapsed_ms={:.2}, throughput_per_s={:.1}, end_to_end_p50_us={}, end_to_end_p95_us={}, replayed_workspaces={}, replay_matches={}",
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
    let blobs = Arc::new(FileBlobStore::new(directory.path().join("blobs"), TestKeys));
    let store = SqliteWorkspaceStore::open(
        directory.path().join("state").join("state.sqlite3"),
        blobs,
        SqliteConfig {
            writer_queue_capacity: 32,
            busy_timeout: Duration::from_secs(5),
        },
    )?;
    let service = WorkspaceService::new(store.clone());
    let base_count = writes / PRODUCERS;
    let remainder = writes % PRODUCERS;
    let counts: Vec<_> = (0..PRODUCERS)
        .map(|producer| base_count + usize::from(producer < remainder))
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
    for (producer, count) in counts.into_iter().enumerate() {
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
    let reopened_blobs = Arc::new(FileBlobStore::new(directory.path().join("blobs"), TestKeys));
    let reopened_store = SqliteWorkspaceStore::open(
        directory.path().join("state").join("state.sqlite3"),
        reopened_blobs,
        SqliteConfig {
            writer_queue_capacity: 32,
            busy_timeout: Duration::from_secs(5),
        },
    )?;
    let sqlite_version = reopened_store.sqlite_version()?;
    let mut replayed_workspaces = 0;
    let mut replay_matches = true;
    for producer in 0..PRODUCERS {
        let expected_version = counts_for(writes, producer) as u64 + 1;
        let projection = reopened_store
            .get_workspace(&workspace_id(producer))?
            .ok_or("missing Workspace projection after reopen check")?;
        let replayed = reopened_store
            .rebuild_workspace_projection(&workspace_id(producer))?
            .ok_or("missing replayed Workspace")?;
        replay_matches &= projection == replayed && replayed.version == expected_version;
        replayed_workspaces += 1;
    }
    if !replay_matches {
        return Err("reopened Workspace projection does not match encrypted event replay".into());
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

fn counts_for(total: usize, producer: usize) -> usize {
    total / PRODUCERS + usize::from(producer < total % PRODUCERS)
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
    let index = ((values.len() - 1) as f64 * fraction).ceil() as usize;
    values[index]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypted_aggregate_state_replays_after_concurrent_workspace_updates() {
        let summary = run(12).expect("rusqlite product-adapter qualification run");
        assert!(summary.sqlite_version.starts_with("3."));
        assert_eq!(summary.replayed_workspaces, PRODUCERS);
        assert!(summary.replay_matches);
    }
}
