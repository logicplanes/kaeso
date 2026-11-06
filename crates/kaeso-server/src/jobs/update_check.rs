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
        let before = newest(&state).await?;
        match updates::check(&state, Trigger::Schedule).await? {
            Checked::Unchanged => "releases unchanged",
            Checked::Changed => {
                if let Some(version) = newest(&state).await?
                    && Some(&version) != before.as_ref()
                {
                    crate::services::notifications::system::update_available(&state, &version)
                        .await;
                }
                "release list fetched"
            }
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

async fn newest(state: &AppState) -> Result<Option<String>, JobError> {
    let settings = kaeso_store::pg::updates::settings(&state.pg).await?;
    let releases = updates::known(kaeso_store::pg::updates::releases(&state.pg).await?);
    Ok(updates::plan(&settings, &releases).map(|p| p.target.version.to_string()))
}
