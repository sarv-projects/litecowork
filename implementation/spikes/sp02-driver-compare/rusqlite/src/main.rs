use rusqlite::{Connection, TransactionBehavior, params};
use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};
use tempfile::TempDir;

const DDL: &str = include_str!("../../../../../docs/schemas/sqlite-v1.sql");
const QUEUE_CAPACITY: usize = 32;

struct Request {
    enqueued_at: Instant,
    reply: mpsc::Sender<Result<Measurement, String>>,
}

#[derive(Clone, Copy, Debug)]
struct Measurement {
    queue_wait: Duration,
    commit: Duration,
}

#[derive(Debug)]
struct Summary {
    sqlite_version: String,
    elapsed: Duration,
    queue_p50: Duration,
    queue_p95: Duration,
    commit_p50: Duration,
    commit_p95: Duration,
    writes_per_second: f64,
    final_version: i64,
    final_sequence: i64,
    event_count: i64,
    restart_state_matches: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
        "rusqlite prototype (SQLite {}, contract v1 DDL, one bounded writer, WAL + FULL): writes={writes}, elapsed_ms={:.2}, throughput_per_s={:.1}, queue_p50_us={}, queue_p95_us={}, commit_p50_us={}, commit_p95_us={}, final_version={}, origin_sequence={}, events={}, restart_state_matches={}",
        summary.sqlite_version,
        summary.elapsed.as_secs_f64() * 1_000.0,
        summary.writes_per_second,
        summary.queue_p50.as_micros(),
        summary.queue_p95.as_micros(),
        summary.commit_p50.as_micros(),
        summary.commit_p95.as_micros(),
        summary.final_version,
        summary.final_sequence,
        summary.event_count,
        summary.restart_state_matches,
    );
    Ok(())
}

fn run(writes: usize) -> Result<Summary, Box<dyn std::error::Error>> {
    let directory = TempDir::new()?;
    let database = directory.path().join("rusqlite.db");
    initialize(&database)?;
    let (sender, receiver) = mpsc::sync_channel::<Request>(QUEUE_CAPACITY);
    let writer_database = database.clone();
    let writer = thread::Builder::new()
        .name("sp02-rusqlite-writer".to_owned())
        .spawn(move || writer_loop(&writer_database, receiver))?;

    let started = Instant::now();
    let producer_count = writes.min(8);
    let base_count = writes / producer_count;
    let remainder = writes % producer_count;
    let mut producers = Vec::with_capacity(producer_count);
    for producer in 0..producer_count {
        let producer_sender = sender.clone();
        let count = base_count + usize::from(producer < remainder);
        producers.push(thread::spawn(
            move || -> Result<Vec<Measurement>, String> {
                let mut measurements = Vec::with_capacity(count);
                for _ in 0..count {
                    let (reply, result) = mpsc::channel();
                    producer_sender
                        .send(Request {
                            enqueued_at: Instant::now(),
                            reply,
                        })
                        .map_err(|error| error.to_string())?;
                    measurements.push(result.recv().map_err(|error| error.to_string())??);
                }
                Ok(measurements)
            },
        ));
    }
    drop(sender);
    let mut measurements = Vec::with_capacity(writes);
    for producer in producers {
        let producer_measurements = producer
            .join()
            .map_err(|_| std::io::Error::other("producer panicked"))?
            .map_err(std::io::Error::other)?;
        measurements.extend(producer_measurements);
    }
    writer
        .join()
        .map_err(|_| std::io::Error::other("writer panicked"))??;
    let elapsed = started.elapsed();
    let connection = Connection::open(&database)?;
    let sqlite_version: String =
        connection.query_row("SELECT sqlite_version()", [], |row| row.get(0))?;
    let (final_version, final_sequence, event_count): (i64, i64, i64) = connection.query_row(
        "SELECT (SELECT version FROM workspaces WHERE workspace_id='sp02'), (SELECT last_sequence FROM workspace_origin_sequences WHERE workspace_id='sp02' AND origin_runtime_id='runtime-sp02'), (SELECT count(*) FROM domain_events WHERE workspace_id='sp02')",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    let mut queue_times: Vec<_> = measurements.iter().map(|item| item.queue_wait).collect();
    let mut commit_times: Vec<_> = measurements.iter().map(|item| item.commit).collect();
    Ok(Summary {
        sqlite_version,
        elapsed,
        queue_p50: percentile(&mut queue_times, 0.50),
        queue_p95: percentile(&mut queue_times, 0.95),
        commit_p50: percentile(&mut commit_times, 0.50),
        commit_p95: percentile(&mut commit_times, 0.95),
        writes_per_second: writes as f64 / elapsed.as_secs_f64(),
        final_version,
        final_sequence,
        event_count,
        restart_state_matches: final_version == writes as i64 + 1
            && final_sequence == writes as i64
            && event_count == writes as i64,
    })
}

fn initialize(database: &Path) -> rusqlite::Result<()> {
    let connection = Connection::open(database)?;
    connection.execute_batch(
        "PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;",
    )?;
    connection.execute_batch(DDL)?;
    connection.execute(
        "INSERT INTO workspaces (workspace_id,name,owner_principal_id,replication_policy,status,created_at,updated_at,version) VALUES ('sp02','SP02','owner','LOCAL_ONLY','ACTIVE','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z',1)",
        [],
    )?;
    connection.execute(
        "INSERT INTO workspace_origin_sequences (workspace_id,origin_runtime_id,last_sequence) VALUES ('sp02','runtime-sp02',0)",
        [],
    )?;
    Ok(())
}

fn writer_loop(database: &PathBuf, receiver: Receiver<Request>) -> rusqlite::Result<()> {
    let mut connection = Connection::open(database)?;
    connection.busy_timeout(Duration::from_secs(5))?;
    for request in receiver {
        let queue_wait = request.enqueued_at.elapsed();
        let started = Instant::now();
        let result = commit(&mut connection).map(|()| Measurement {
            queue_wait,
            commit: started.elapsed(),
        });
        let _ = request
            .reply
            .send(result.map_err(|error| error.to_string()));
    }
    Ok(())
}

fn commit(connection: &mut Connection) -> rusqlite::Result<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: i64 = transaction.query_row(
        "SELECT version FROM workspaces WHERE workspace_id='sp02'",
        [],
        |row| row.get(0),
    )?;
    let next_version = version + 1;
    transaction.execute(
        "UPDATE workspaces SET version=?1, updated_at=?2 WHERE workspace_id='sp02' AND version=?3",
        params![next_version, format!("v{next_version}"), version],
    )?;
    let sequence: i64 = transaction.query_row(
        "UPDATE workspace_origin_sequences SET last_sequence=last_sequence+1 WHERE workspace_id='sp02' AND origin_runtime_id='runtime-sp02' RETURNING last_sequence",
        [],
        |row| row.get(0),
    )?;
    transaction.execute(
        "INSERT INTO domain_events (event_id,workspace_id,entity_type,entity_id,origin_runtime_id,origin_sequence,entity_revision,hlc_timestamp,correlation_id,schema_version,type,payload_json,aggregate_state_ref_json,recorded_at,payload_digest) VALUES (?1,'sp02','Workspace','sp02','runtime-sp02',?2,?3,?4,?5,1,'workspace.replication_policy.changed.v1','{}','{}',?4,?6)",
        params![format!("event-{sequence}"), sequence, next_version, format!("2026-01-01T00:00:{:02}Z", sequence % 60), format!("correlation-{sequence}"), format!("sha256:{}", "0".repeat(64))],
    )?;
    transaction.commit()
}

fn percentile(values: &mut [Duration], fraction: f64) -> Duration {
    values.sort_unstable();
    let index = ((values.len() - 1) as f64 * fraction).ceil() as usize;
    values[index]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_schema_and_transaction_shape_survive_reopen() {
        let summary = run(12).expect("rusqlite qualification run");
        assert_eq!(summary.final_version, 13);
        assert_eq!(summary.final_sequence, 12);
        assert_eq!(summary.event_count, 12);
        assert!(summary.restart_state_matches);
    }
}
