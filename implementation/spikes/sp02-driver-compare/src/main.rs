use sqlx::{
    Connection, Row,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous},
};
use std::{
    path::Path,
    time::{Duration, Instant},
};
use tempfile::TempDir;
use tokio::sync::{mpsc, oneshot};

const DDL: &str = include_str!("../../../../docs/schemas/sqlite-v1.sql");
const QUEUE_CAPACITY: usize = 32;

struct WriteRequest {
    enqueued_at: Instant,
    reply: oneshot::Sender<Result<Measurement, String>>,
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let writes = std::env::args()
        .nth(1)
        .map(|arg| arg.parse::<usize>())
        .transpose()?
        .unwrap_or(1_000);
    if writes == 0 {
        return Err("write count must be greater than zero".into());
    }
    let summary = run(writes).await?;
    println!(
        "SQLx prototype (SQLite {}, contract v1 DDL, one bounded writer, WAL + FULL): writes={writes}, elapsed_ms={:.2}, throughput_per_s={:.1}, queue_p50_us={}, queue_p95_us={}, commit_p50_us={}, commit_p95_us={}, final_version={}, origin_sequence={}, events={}, restart_state_matches={}",
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
    println!(
        "This isolated SQLx process is not directly linkable with the current rusqlite 0.40.2 adapter: their libsqlite3-sys links versions conflict."
    );
    Ok(())
}

async fn run(writes: usize) -> Result<Summary, Box<dyn std::error::Error>> {
    let directory = TempDir::new()?;
    let database = directory.path().join("sqlx.db");
    let connect_options = options(&database);
    let mut connection = sqlx::SqliteConnection::connect_with(&connect_options).await?;
    sqlx::raw_sql(DDL).execute(&mut connection).await?;
    seed(&mut connection).await?;
    connection.close().await?;

    let (sender, receiver) = mpsc::channel::<WriteRequest>(QUEUE_CAPACITY);
    let writer_path = database.clone();
    let writer = tokio::spawn(async move { writer_loop(writer_path, receiver).await });
    let started = Instant::now();
    let producer_count = writes.min(8);
    let base_count = writes / producer_count;
    let remainder = writes % producer_count;
    let mut producers = Vec::with_capacity(producer_count);
    for producer in 0..producer_count {
        let producer_sender = sender.clone();
        let count = base_count + usize::from(producer < remainder);
        producers.push(tokio::spawn(async move {
            let mut measurements = Vec::with_capacity(count);
            for _ in 0..count {
                let (reply, response) = oneshot::channel();
                producer_sender
                    .send(WriteRequest {
                        enqueued_at: Instant::now(),
                        reply,
                    })
                    .await
                    .map_err(|error| error.to_string())?;
                measurements.push(response.await.map_err(|error| error.to_string())??);
            }
            Ok::<_, String>(measurements)
        }));
    }
    drop(sender);
    let mut measurements = Vec::with_capacity(writes);
    for producer in producers {
        measurements.extend(producer.await??);
    }
    writer.await??;
    let elapsed = started.elapsed();

    // Reopen a fresh connection to establish that the committed projection and journal
    // survive process-local connection teardown.
    let mut reopened = sqlx::SqliteConnection::connect_with(&options(&database)).await?;
    let row = sqlx::query("SELECT sqlite_version() AS sqlite_version, (SELECT version FROM workspaces WHERE workspace_id='sp02') AS version, (SELECT last_sequence FROM workspace_origin_sequences WHERE workspace_id='sp02' AND origin_runtime_id='runtime-sp02') AS origin_sequence, (SELECT count(*) FROM domain_events WHERE workspace_id='sp02') AS event_count")
        .fetch_one(&mut reopened).await?;
    let final_version: i64 = row.try_get("version")?;
    let final_sequence: i64 = row.try_get("origin_sequence")?;
    let event_count: i64 = row.try_get("event_count")?;
    let sqlite_version: String = row.try_get("sqlite_version")?;
    reopened.close().await?;
    let mut queue_times: Vec<_> = measurements.iter().map(|item| item.queue_wait).collect();
    let mut commit_times: Vec<_> = measurements.iter().map(|item| item.commit).collect();
    let queue_p50 = percentile(&mut queue_times, 0.50);
    let queue_p95 = percentile(&mut queue_times, 0.95);
    let commit_p50 = percentile(&mut commit_times, 0.50);
    let commit_p95 = percentile(&mut commit_times, 0.95);
    Ok(Summary {
        sqlite_version,
        elapsed,
        queue_p50,
        queue_p95,
        commit_p50,
        commit_p95,
        writes_per_second: writes as f64 / elapsed.as_secs_f64(),
        final_version,
        final_sequence,
        event_count,
        restart_state_matches: final_version == writes as i64 + 1
            && final_sequence == writes as i64
            && event_count == writes as i64,
    })
}

fn options(database: &Path) -> SqliteConnectOptions {
    SqliteConnectOptions::new()
        .filename(database)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Full)
        .busy_timeout(Duration::from_secs(5))
}

async fn seed(connection: &mut sqlx::SqliteConnection) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO workspaces (workspace_id,name,owner_principal_id,replication_policy,status,created_at,updated_at,version) VALUES ('sp02','SP02','owner','LOCAL_ONLY','ACTIVE','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z',1)")
        .execute(&mut *connection).await?;
    sqlx::query("INSERT INTO workspace_origin_sequences (workspace_id,origin_runtime_id,last_sequence) VALUES ('sp02','runtime-sp02',0)")
        .execute(&mut *connection).await?;
    Ok(())
}

async fn writer_loop(
    database: std::path::PathBuf,
    mut receiver: mpsc::Receiver<WriteRequest>,
) -> Result<(), sqlx::Error> {
    let mut connection = sqlx::SqliteConnection::connect_with(&options(&database)).await?;
    while let Some(request) = receiver.recv().await {
        let queue_wait = request.enqueued_at.elapsed();
        let result = commit(&mut connection, false)
            .await
            .map(|commit| Measurement { queue_wait, commit });
        let _ = request
            .reply
            .send(result.map_err(|error| error.to_string()));
    }
    connection.close().await
}

async fn commit(
    connection: &mut sqlx::SqliteConnection,
    inject_failure_after_projection: bool,
) -> Result<Duration, sqlx::Error> {
    let started = Instant::now();
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *connection)
        .await?;
    let result = async {
        let row = sqlx::query("SELECT version FROM workspaces WHERE workspace_id='sp02'")
            .fetch_one(&mut *connection).await?;
        let version: i64 = row.try_get("version")?;
        let next_version = version + 1;
        sqlx::query("UPDATE workspaces SET version=?1, updated_at=?2 WHERE workspace_id='sp02' AND version=?3")
            .bind(next_version).bind(format!("v{next_version}")).bind(version)
            .execute(&mut *connection).await?;
        let row = sqlx::query("UPDATE workspace_origin_sequences SET last_sequence=last_sequence+1 WHERE workspace_id='sp02' AND origin_runtime_id='runtime-sp02' RETURNING last_sequence")
            .fetch_one(&mut *connection).await?;
        let sequence: i64 = row.try_get("last_sequence")?;
        if inject_failure_after_projection {
            return Err(sqlx::Error::Protocol("injected failure after projection and sequence writes".to_owned()));
        }
        sqlx::query("INSERT INTO domain_events (event_id,workspace_id,entity_type,entity_id,origin_runtime_id,origin_sequence,entity_revision,hlc_timestamp,correlation_id,schema_version,type,payload_json,aggregate_state_ref_json,recorded_at,payload_digest) VALUES (?1,'sp02','Workspace','sp02','runtime-sp02',?2,?3,?4,?5,1,'workspace.replication_policy.changed.v1','{}','{}',?4,?6)")
            .bind(format!("event-{sequence}"))
            .bind(sequence)
            .bind(next_version)
            .bind(format!("2026-01-01T00:00:{:02}Z", sequence % 60))
            .bind(format!("correlation-{sequence}"))
            .bind(format!("sha256:{}", "0".repeat(64)))
            .execute(&mut *connection).await?;
        sqlx::query("COMMIT").execute(&mut *connection).await?;
        Ok::<(), sqlx::Error>(())
    }.await;
    if let Err(error) = result {
        let _ = sqlx::query("ROLLBACK").execute(&mut *connection).await;
        return Err(error);
    }
    Ok(started.elapsed())
}

fn percentile(values: &mut [Duration], fraction: f64) -> Duration {
    values.sort_unstable();
    let index = ((values.len() - 1) as f64 * fraction).ceil() as usize;
    values[index]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn contract_schema_and_transaction_shape_survive_reopen() {
        let summary = run(12).await.expect("SQLx qualification run");
        assert_eq!(summary.final_version, 13);
        assert_eq!(summary.final_sequence, 12);
        assert_eq!(summary.event_count, 12);
        assert!(summary.restart_state_matches);
    }

    #[tokio::test]
    async fn failed_transaction_rolls_back_projection_sequence_and_event() {
        let directory = TempDir::new().expect("temporary directory");
        let database = directory.path().join("rollback.db");
        let mut connection = sqlx::SqliteConnection::connect_with(&options(&database))
            .await
            .expect("open SQLite");
        sqlx::raw_sql(DDL)
            .execute(&mut connection)
            .await
            .expect("apply DDL");
        seed(&mut connection).await.expect("seed workspace");

        assert!(commit(&mut connection, true).await.is_err());
        let row = sqlx::query("SELECT (SELECT version FROM workspaces WHERE workspace_id='sp02') AS version, (SELECT last_sequence FROM workspace_origin_sequences WHERE workspace_id='sp02' AND origin_runtime_id='runtime-sp02') AS sequence, (SELECT count(*) FROM domain_events WHERE workspace_id='sp02') AS event_count")
            .fetch_one(&mut connection).await.expect("read rolled-back state");
        assert_eq!(row.try_get::<i64, _>("version").unwrap(), 1);
        assert_eq!(row.try_get::<i64, _>("sequence").unwrap(), 0);
        assert_eq!(row.try_get::<i64, _>("event_count").unwrap(), 0);
        connection.close().await.expect("close SQLite");
    }
}
