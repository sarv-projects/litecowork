use domain_workspace::{ChangeReplicationPolicy, CreateWorkspace, EventContext, WorkspaceService};
use std::{
    sync::Arc,
    sync::Barrier,
    sync::atomic::{AtomicBool, Ordering},
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
const READERS: usize = 4;
const MIXED_WRITERS: usize = 40;
const MAX_MIXED_UPDATES_PER_WORKSPACE: usize = 10_000;
const MAX_RECORDED_READ_SAMPLES_PER_READER: usize = 100_000;

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
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() == Some("mixed") {
        let updates_per_workspace = args
            .next()
            .map(|arg| arg.parse::<usize>())
            .transpose()?
            .unwrap_or(200);
        if updates_per_workspace == 0 {
            return Err("updates per workspace must be greater than zero".into());
        }
        if updates_per_workspace > MAX_MIXED_UPDATES_PER_WORKSPACE {
            return Err(format!(
                "updates per workspace must not exceed {MAX_MIXED_UPDATES_PER_WORKSPACE}"
            )
            .into());
        }
        let summary = run_mixed_load(updates_per_workspace)?;
        println!(
            "rusqlite product adapter mixed load (SQLite {}, queue=32, WAL + FULL): updates={}, reads={}, elapsed_ms={:.2}, write_p50_us={}, write_p95_us={}, read_p50_us={}, read_p95_us={}, send_wait_total_us={}, send_wait_max_us={}, outstanding_peak={}, snapshots_coherent={}, replayed_workspaces={}, replay_matches={}",
            summary.sqlite_version,
            summary.completed_updates,
            summary.observed_reads,
            summary.elapsed.as_secs_f64() * 1_000.0,
            summary.write_p50.as_micros(),
            summary.write_p95.as_micros(),
            summary.read_p50.as_micros(),
            summary.read_p95.as_micros(),
            summary.send_wait_total.as_micros(),
            summary.send_wait_max.as_micros(),
            summary.outstanding_commands_peak,
            summary.coherent_snapshots,
            summary.replayed_workspaces,
            summary.replay_matches,
        );
        return Ok(());
    }

    let writes = std::env::args()
        .nth(1)
        .map(|arg| arg.parse::<usize>())
        .transpose()?
        .unwrap_or(1_000);
    if writes == 0 {
        return Err("write count must be greater than zero".into());
    }
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

#[derive(Debug)]
struct MixedLoadSummary {
    sqlite_version: String,
    elapsed: Duration,
    completed_updates: usize,
    observed_reads: usize,
    write_p50: Duration,
    write_p95: Duration,
    read_p50: Duration,
    read_p95: Duration,
    send_wait_total: Duration,
    send_wait_max: Duration,
    outstanding_commands_peak: u64,
    coherent_snapshots: bool,
    replayed_workspaces: usize,
    replay_matches: bool,
}

fn run_mixed_load(
    updates_per_workspace: usize,
) -> Result<MixedLoadSummary, Box<dyn std::error::Error>> {
    if updates_per_workspace == 0 || updates_per_workspace > MAX_MIXED_UPDATES_PER_WORKSPACE {
        return Err(format!(
            "updates per workspace must be between 1 and {MAX_MIXED_UPDATES_PER_WORKSPACE}"
        )
        .into());
    }
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
    let setup = WorkspaceService::new(store.clone());
    for producer in 0..MIXED_WRITERS {
        setup.create(CreateWorkspace {
            workspace_id: workspace_id(producer),
            name: format!("SP02 mixed workspace {producer}"),
            owner_principal_id: "sp02-owner".to_owned(),
            event: event_context(&format!("mixed-create-{producer}")),
        })?;
    }

    let barrier = Arc::new(Barrier::new(MIXED_WRITERS + READERS + 1));
    let finished = Arc::new(AtomicBool::new(false));
    let started = Instant::now();
    let mut writer_threads = Vec::with_capacity(MIXED_WRITERS);
    for producer in 0..MIXED_WRITERS {
        let store = store.clone();
        let barrier = Arc::clone(&barrier);
        writer_threads.push(thread::spawn(move || {
            let service = WorkspaceService::new(store);
            let mut latencies = Vec::with_capacity(updates_per_workspace);
            barrier.wait();
            for update in 0..updates_per_workspace {
                let operation_started = Instant::now();
                service
                    .change_replication_policy(ChangeReplicationPolicy {
                        workspace_id: workspace_id(producer),
                        expected_version: update as u64 + 1,
                        policy: policy_for_update(update),
                        replication_scope_root_ids: Vec::new(),
                        event: event_context(&format!("mixed-update-{producer}-{update}")),
                    })
                    .map_err(|error| error.to_string())?;
                latencies.push(operation_started.elapsed());
            }
            Ok::<_, String>(latencies)
        }));
    }

    let mut reader_threads = Vec::with_capacity(READERS);
    for _ in 0..READERS {
        let store = store.clone();
        let barrier = Arc::clone(&barrier);
        let finished = Arc::clone(&finished);
        reader_threads.push(thread::spawn(move || {
            let mut last_versions = [0_u64; MIXED_WRITERS];
            let mut latencies = Vec::new();
            let mut coherent = true;
            barrier.wait();
            while !finished.load(Ordering::Acquire) {
                for (workspace_index, last_version) in last_versions.iter_mut().enumerate() {
                    let operation_started = Instant::now();
                    let workspace = store
                        .get_workspace(&workspace_id(workspace_index))
                        .map_err(|error| error.to_string())?
                        .ok_or_else(|| "mixed-load Workspace disappeared".to_owned())?;
                    if latencies.len() < MAX_RECORDED_READ_SAMPLES_PER_READER {
                        latencies.push(operation_started.elapsed());
                    }
                    coherent &= workspace.version >= *last_version
                        && workspace.replication_policy == policy_for_version(workspace.version);
                    *last_version = workspace.version;
                }
            }
            Ok::<_, String>((latencies, coherent))
        }));
    }

    barrier.wait();
    let mut write_latencies = Vec::with_capacity(MIXED_WRITERS * updates_per_workspace);
    let mut writer_error = None;
    for writer in writer_threads {
        match writer.join() {
            Ok(Ok(latencies)) => write_latencies.extend(latencies),
            Ok(Err(error)) => {
                writer_error.get_or_insert(error);
            }
            Err(_) => {
                writer_error.get_or_insert("mixed-load writer panicked".to_owned());
            }
        };
    }
    finished.store(true, Ordering::Release);

    let mut read_latencies = Vec::new();
    let mut coherent_snapshots = true;
    let mut reader_error = None;
    for reader in reader_threads {
        match reader.join() {
            Ok(Ok((latencies, coherent))) => {
                read_latencies.extend(latencies);
                coherent_snapshots &= coherent;
            }
            Ok(Err(error)) => {
                reader_error.get_or_insert(error);
            }
            Err(_) => {
                reader_error.get_or_insert("mixed-load reader panicked".to_owned());
            }
        };
    }
    if let Some(error) = writer_error.or(reader_error) {
        return Err(error.into());
    }
    let elapsed = started.elapsed();
    if write_latencies.len() != MIXED_WRITERS * updates_per_workspace {
        return Err("mixed-load writer count did not match requested updates".into());
    }
    if read_latencies.is_empty() {
        return Err("mixed-load readers observed no Workspace snapshots".into());
    }

    read_latencies.sort_unstable();
    write_latencies.sort_unstable();
    let writer_metrics = store.writer_metrics_snapshot();
    drop(setup);
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
    let mut replay_matches = coherent_snapshots;
    for producer in 0..MIXED_WRITERS {
        let projection = reopened_store
            .get_workspace(&workspace_id(producer))?
            .ok_or("missing Workspace projection after mixed-load reopen")?;
        let replayed = reopened_store
            .rebuild_workspace_projection(&workspace_id(producer))?
            .ok_or("missing replayed Workspace after mixed-load reopen")?;
        replay_matches &= projection == replayed
            && replayed.version == updates_per_workspace as u64 + 1
            && replayed.replication_policy == policy_for_version(replayed.version);
    }
    if !replay_matches {
        return Err("mixed-load snapshots or reopened replay were inconsistent".into());
    }

    Ok(MixedLoadSummary {
        sqlite_version,
        elapsed,
        completed_updates: write_latencies.len(),
        observed_reads: read_latencies.len(),
        write_p50: percentile(&write_latencies, 0.50),
        write_p95: percentile(&write_latencies, 0.95),
        read_p50: percentile(&read_latencies, 0.50),
        read_p95: percentile(&read_latencies, 0.95),
        send_wait_total: Duration::from_nanos(writer_metrics.send_wait_nanos_total),
        send_wait_max: Duration::from_nanos(writer_metrics.send_wait_nanos_max),
        outstanding_commands_peak: writer_metrics.outstanding_commands_peak,
        coherent_snapshots,
        replayed_workspaces: MIXED_WRITERS,
        replay_matches,
    })
}

fn policy_for_update(update: usize) -> ReplicationPolicy {
    if update.is_multiple_of(2) {
        ReplicationPolicy::MetadataOnly
    } else {
        ReplicationPolicy::LocalOnly
    }
}

fn policy_for_version(version: u64) -> ReplicationPolicy {
    if version <= 1 || (version - 2) % 2 == 1 {
        ReplicationPolicy::LocalOnly
    } else {
        ReplicationPolicy::MetadataOnly
    }
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

    #[test]
    fn mixed_reads_and_writes_preserve_monotonic_coherent_snapshots_and_replay() {
        let summary = run_mixed_load(2).expect("rusqlite mixed-load qualification run");

        assert!(summary.sqlite_version.starts_with("3."));
        assert_eq!(summary.completed_updates, MIXED_WRITERS * 2);
        assert!(summary.observed_reads > 0);
        assert!(summary.coherent_snapshots);
        assert_eq!(summary.replayed_workspaces, MIXED_WRITERS);
        assert!(summary.replay_matches);
    }
}
