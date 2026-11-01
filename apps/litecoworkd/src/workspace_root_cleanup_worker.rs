use std::{
    sync::mpsc::{self, Receiver, SyncSender, TryRecvError},
    thread::{self, JoinHandle},
    time::Duration,
};

use storage_sqlite::SqliteWorkspaceStore;

const EMPTY_OR_FAILED_POLL: Duration = Duration::from_secs(1);
const COMPLETED_BATCH_PAUSE: Duration = Duration::from_millis(25);

pub(crate) struct WorkspaceRootCleanupWorker {
    stop: SyncSender<()>,
    join: Option<JoinHandle<()>>,
}

impl WorkspaceRootCleanupWorker {
    pub(crate) fn start(store: SqliteWorkspaceStore, runtime_id: String) -> std::io::Result<Self> {
        let (stop, receiver) = mpsc::sync_channel(1);
        let join = thread::Builder::new()
            .name("litecowork-workspace-root-cleanup".to_owned())
            .spawn(move || {
                run_worker_loop(receiver, EMPTY_OR_FAILED_POLL, || {
                    let now = time::OffsetDateTime::now_utc()
                        .format(&time::format_description::well_known::Rfc3339)
                        .map_err(|_| {
                            storage_core::StoreError::Invalid(
                                "Runtime clock is unavailable".to_owned(),
                            )
                        })?;
                    store.process_next_workspace_root_cleanup_batch(&runtime_id, &now)
                });
            })?;
        Ok(Self {
            stop,
            join: Some(join),
        })
    }

    pub(crate) fn stop(mut self) -> Result<(), String> {
        self.request_stop();
        self.join
            .take()
            .ok_or_else(|| "WorkspaceRoot cleanup worker was already stopped".to_owned())?
            .join()
            .map_err(|_| "WorkspaceRoot cleanup worker terminated unexpectedly".to_owned())
    }

    fn request_stop(&self) {
        let _ = self.stop.try_send(());
    }
}

impl Drop for WorkspaceRootCleanupWorker {
    fn drop(&mut self) {
        self.request_stop();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn run_worker_loop(
    stop: Receiver<()>,
    idle_poll: Duration,
    mut process_next_batch: impl FnMut() -> Result<bool, storage_core::StoreError>,
) {
    loop {
        match stop.try_recv() {
            Ok(()) | Err(TryRecvError::Disconnected) => return,
            Err(TryRecvError::Empty) => {}
        }
        let pause = match process_next_batch() {
            Ok(true) => COMPLETED_BATCH_PAUSE,
            Ok(false) | Err(_) => idle_poll,
        };
        match stop.recv_timeout(pause) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => return,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supervised_loop_retries_and_stops_without_blocking_the_runtime() {
        let (stop_sender, stop_receiver) = mpsc::sync_channel(1);
        let mut calls = 0;
        run_worker_loop(stop_receiver, Duration::from_millis(1), || {
            calls += 1;
            if calls == 1 {
                Err(storage_core::StoreError::Io(
                    "temporary store error".to_owned(),
                ))
            } else {
                stop_sender.try_send(()).expect("request worker stop");
                Ok(false)
            }
        });
        assert_eq!(calls, 2);
    }
}
