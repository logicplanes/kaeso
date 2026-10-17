use crate::jobs::JobError;
use crate::services::updates::{self, Checked};
use crate::state::AppState;

pub async fn run(state: AppState) -> Result<String, JobError> {
    let reconciled = updates::reconcile_running(&state).await?;
    let job = match reconciled {
        Some(job) => format!("update {} is {}; ", job.to_version, job.state),
        None => String::new(),
    };
    if !updates::due(&state).await? {
        return Ok(format!("{job}check not due"));
    }
    Ok(match updates::check(&state).await? {
        Checked::Unchanged => format!("{job}releases unchanged"),
        Checked::Changed => format!("{job}release list fetched"),
        Checked::Failed(_) => format!("{job}check failed"),
    })
}
