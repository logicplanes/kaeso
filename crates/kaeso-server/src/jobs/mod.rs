pub mod cleanup;
pub mod key_expiry;
pub mod kpmr;
pub mod mcp_sync;
pub mod notifications;
pub mod org_deletion;
pub mod update_check;

use std::panic::AssertUnwindSafe;
use std::time::Duration;

use futures::FutureExt;
use futures::future::BoxFuture;
use kaeso_store::StoreError;
use kaeso_store::pg::{locks, try_with_advisory_lock};
use tokio_util::sync::CancellationToken;

use crate::services::kpmr::SyncError;
use crate::state::AppState;

#[derive(Debug, thiserror::Error)]
pub enum JobError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Kpmr(#[from] SyncError),
    #[error(transparent)]
    Notify(#[from] crate::platform::error::ApiError),
    #[error("the job panicked: {0}")]
    Panicked(String),
    #[error("{0}")]
    Incomplete(String),
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "no message".to_owned())
}

/// What one run of a job did, in a few words for its log line. A run that found nothing to do
/// is idle: jobs run every few seconds or minutes, and their idle runs would bury the runs that
/// changed something, so those are logged at debug.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ran {
    pub summary: String,
    pub idle: bool,
}

impl Ran {
    pub fn worked(summary: impl Into<String>) -> Ran {
        Ran {
            summary: summary.into(),
            idle: false,
        }
    }

    pub fn idle(summary: impl Into<String>) -> Ran {
        Ran {
            summary: summary.into(),
            idle: true,
        }
    }

    /// Idle when `worked` is false.
    pub fn of(summary: impl Into<String>, worked: bool) -> Ran {
        Ran {
            summary: summary.into(),
            idle: !worked,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Job {
    pub name: &'static str,
    pub every: Duration,
    pub lock: i64,
    pub run: fn(AppState) -> BoxFuture<'static, Result<Ran, JobError>>,
}

pub fn all() -> Vec<Job> {
    vec![
        Job {
            name: "batches",
            every: Duration::from_secs(300),
            lock: 10,
            run: |s| Box::pin(async move { Ok(crate::model_api::batches::settle_open(&s).await?) }),
        },
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
            name: "kpmr_sync",
            every: Duration::from_secs(300),
            lock: 4,
            run: |s| Box::pin(kpmr::run(s)),
        },
        // Often enough to retry a failed fetch within 15 minutes; `due` waits six hours after one
        // that worked.
        Job {
            name: "exchange_rates",
            every: Duration::from_secs(300),
            lock: 12,
            run: |s| {
                Box::pin(async move {
                    if !crate::services::exchange_rates::due(&s).await? {
                        return Ok(Ran::idle("not due"));
                    }
                    Ok(crate::services::exchange_rates::sync(&s).await?)
                })
            },
        },
        Job {
            name: "update_check",
            every: Duration::from_secs(60),
            lock: 6,
            run: |s| Box::pin(update_check::run(s)),
        },
        Job {
            name: "notifications",
            every: Duration::from_secs(60),
            lock: 8,
            run: |s| Box::pin(notifications::run(s)),
        },
        Job {
            name: "mcp_sync",
            every: Duration::from_secs(60),
            lock: 7,
            run: |s| Box::pin(mcp_sync::run(s)),
        },
        // Often, so a webhook's event goes out within seconds of its notice.
        Job {
            name: "webhooks",
            every: Duration::from_secs(5),
            lock: 10,
            run: |s| Box::pin(crate::services::webhooks::run(s)),
        },
        // Hourly, so a warning goes out on its day and a move soon after a shutdown.
        Job {
            name: "model_lifecycle",
            every: Duration::from_secs(3600),
            lock: 11,
            run: |s| {
                Box::pin(async move {
                    let r = crate::services::lifecycle::run(&s).await?;
                    Ok(Ran::of(
                        format!(
                            "{} announced, {} warned, {} moved, {} replacements added",
                            r.announced, r.warned, r.moved, r.added
                        ),
                        r.announced + r.warned + r.moved + r.added > 0,
                    ))
                })
            },
        },
        // Often enough that a line follows KPMR soon after a sync.
        Job {
            name: "model_lines",
            every: Duration::from_secs(600),
            lock: 13,
            run: |s| {
                Box::pin(async move {
                    let r = crate::services::lines::run(&s).await?;
                    Ok(Ran::of(
                        format!("{} moved, {} waiting, {} held", r.moved, r.waiting, r.held),
                        r.moved + r.waiting + r.held > 0,
                    ))
                })
            },
        },
        // Far more often than `limits::JOB_HOLD`, so a running job never loses what it holds.
        Job {
            name: "media_jobs",
            every: Duration::from_secs(60),
            lock: 14,
            run: |s| {
                Box::pin(async move {
                    let videos = crate::model_api::videos::look_after(&s).await;
                    let images = crate::model_api::generations::look_after(&s).await;
                    let ((videos, running_videos), (images, running_images)) = (videos?, images?);
                    let (billed, running) = (videos + images, running_videos + running_images);
                    Ok(Ran::of(
                        format!("{billed} billed, {running} running"),
                        billed + running > 0,
                    ))
                })
            },
        },
        // Shorter than `RENEW_AHEAD`, so an account in use never waits for its renewal.
        Job {
            name: "account_renewal",
            every: Duration::from_secs(60),
            lock: 9,
            run: |s| {
                Box::pin(async move { Ok(crate::services::credentials::renew_ahead(&s).await?) })
            },
        },
    ]
}

pub async fn run_once(state: &AppState, job: Job) -> Result<Option<Ran>, JobError> {
    let st = state.clone();
    // A panic is caught inside the lock so the lock is released and the job's loop keeps running.
    let (outcome, events) = crate::services::notifications::dispatch::collecting(
        try_with_advisory_lock(&state.pg, locks::JOBS_BASE + job.lock, || async move {
            Ok(AssertUnwindSafe(async move { (job.run)(st).await })
                .catch_unwind()
                .await
                .unwrap_or_else(|payload| Err(JobError::Panicked(panic_message(&*payload)))))
        }),
    )
    .await;
    // Events of a job that failed may come from a transaction that rolled back; only a finished
    // job's changes are known to be committed.
    if matches!(outcome, Ok(Some(Ok(_)))) {
        crate::services::notifications::dispatch::dispatch(state, &events).await;
    }
    match outcome? {
        None => Ok(None),
        Some(Ok(ran)) => Ok(Some(ran)),
        Some(Err(e)) => Err(e),
    }
}

/// Runs the job once, as its loop does: counts the run and logs it.
pub async fn tick(state: &AppState, job: Job) {
    match run_once(state, job).await {
        Ok(Some(ran)) => {
            metrics::counter!(crate::metrics::JOB_RUNS, "job" => job.name, "outcome" => "ok")
                .increment(1);
            let summary = ran.summary;
            if ran.idle {
                tracing::debug!(job = job.name, %summary, "job ran");
            } else {
                tracing::info!(job = job.name, %summary, "job ran");
            }
        }
        Ok(None) => {
            metrics::counter!(crate::metrics::JOB_RUNS, "job" => job.name, "outcome" => "skipped")
                .increment(1);
        }
        Err(e) => {
            metrics::counter!(crate::metrics::JOB_RUNS, "job" => job.name, "outcome" => "failed")
                .increment(1);
            tracing::error!(job = job.name, error = %e, "job failed");
        }
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
                tick(&state, job).await;
                tokio::select! {
                    () = shutdown.cancelled() => return,
                    () = tokio::time::sleep(job.every) => {}
                }
            }
        });
    }
}
