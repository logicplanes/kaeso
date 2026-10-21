use crate::jobs::JobError;
use crate::services::updates::{self, Auto, Checked, Trigger};
use crate::state::AppState;

pub async fn run(state: AppState) -> Result<String, JobError> {
    let reconciled = updates::reconcile_running(&state).await?;
    let job = match reconciled {
        Some(job) => format!("update {} is {}; ", job.to_version, job.state),
        None => String::new(),
    };
    let checked = if updates::due(&state).await? {
        match updates::check(&state, Trigger::Schedule).await? {
            Checked::Unchanged => "releases unchanged",
            Checked::Changed => "release list fetched",
            Checked::Failed(_) => "check failed",
        }
    } else {
        "check not due"
    };
    Ok(match updates::auto_update(&state).await? {
        Auto::Off => format!("{job}{checked}"),
        auto => format!("{job}{checked}; {}", auto.summary()),
    })
}
