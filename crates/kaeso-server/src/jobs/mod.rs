pub mod cleanup;
pub mod key_expiry;
pub mod models_dev;
pub mod org_deletion;
pub mod update_check;

use std::time::Duration;

use futures::future::BoxFuture;
use kaeso_store::StoreError;
use kaeso_store::pg::{locks, try_with_advisory_lock};
use tokio_util::sync::CancellationToken;

use crate::services::models_dev::SyncError;
use crate::state::AppState;

#[derive(Debug, thiserror::Error)]
pub enum JobError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    ModelsDev(#[from] SyncError),
}

#[derive(Clone, Copy)]
pub struct Job {
    pub name: &'static str,
    pub every: Duration,
    pub lock: i64,
    pub run: fn(AppState) -> BoxFuture<'static, Result<String, JobError>>,
}

pub fn all() -> Vec<Job> {
    vec![
        Job {
            name: "cleanup",
            every: Duration::from_secs(3600),
            lock: 1,
            run: |s| Box::pin(cleanup::run(s)),
        },
        Job {
            name: "org_deletion",
            every: Duration::from_secs(600),
            lock: 2,
            run: |s| Box::pin(org_deletion::run(s)),
        },
        Job {
            name: "key_expiry_warnings",
            every: Duration::from_secs(3600),
            lock: 3,
            run: |s| Box::pin(key_expiry::run(s)),
        },
        Job {
            name: "log_jobs",
            every: Duration::from_secs(30),
            lock: 5,
            run: |s| Box::pin(async move { Ok(crate::services::logs::run_jobs(&s).await?) }),
        },
        Job {
            name: "models_dev_sync",
            every: Duration::from_secs(300),
            lock: 4,
            run: |s| Box::pin(models_dev::run(s)),
        },
        Job {
            name: "update_check",
            every: Duration::from_secs(300),
            lock: 6,
            run: |s| Box::pin(update_check::run(s)),
        },
    ]
}

pub async fn run_once(state: &AppState, job: Job) -> Result<Option<String>, JobError> {
    let st = state.clone();
    let outcome = try_with_advisory_lock(&state.pg, locks::JOBS_BASE + job.lock, || async move {
        Ok((job.run)(st).await)
    })
    .await?;
    match outcome {
        None => Ok(None),
        Some(Ok(summary)) => Ok(Some(summary)),
        Some(Err(e)) => Err(e),
    }
}

pub fn spawn_all(state: &AppState, shutdown: &CancellationToken) {
    for job in all() {
        let state = state.clone();
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            let jitter = Duration::from_millis(
                u64::from(u32::from_le_bytes(kaeso_crypto::random_bytes::<4>())) % 30_000,
            );
            tokio::select! {
                () = shutdown.cancelled() => return,
                () = tokio::time::sleep(jitter) => {}
            }
            loop {
                match run_once(&state, job).await {
                    Ok(Some(summary)) => {
                        metrics::counter!(crate::metrics::JOB_RUNS, "job" => job.name, "outcome" => "ok").increment(1);
                        tracing::info!(job = job.name, %summary, "job ran");
                    }
                    Ok(None) => {
                        metrics::counter!(crate::metrics::JOB_RUNS, "job" => job.name, "outcome" => "skipped").increment(1);
                    }
                    Err(e) => {
                        metrics::counter!(crate::metrics::JOB_RUNS, "job" => job.name, "outcome" => "failed").increment(1);
                        tracing::error!(job = job.name, error = %e, "job failed");
                    }
                }
                tokio::select! {
                    () = shutdown.cancelled() => return,
                    () = tokio::time::sleep(job.every) => {}
                }
            }
        });
    }
}
