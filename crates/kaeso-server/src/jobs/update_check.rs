use crate::jobs::{JobError, Ran};
use crate::services::updates::{self, Auto, Checked, Trigger};
use crate::state::AppState;

pub async fn run(state: AppState) -> Result<Ran, JobError> {
    let reconciled = updates::reconcile_running(&state).await?;
    let mut worked = reconciled.is_some();
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
                worked = true;
                "release list fetched"
            }
            Checked::Failed(_) => {
                worked = true;
                "check failed"
            }
        }
    } else {
        "check not due"
    };
    updates::report_available(&state).await;
    let auto = updates::auto_update(&state).await?;
    worked |= !matches!(auto, Auto::Off | Auto::NotDue | Auto::UpToDate);
    let summary = match auto {
        Auto::Off => format!("{job}{checked}"),
        auto => format!("{job}{checked}; {}", auto.summary()),
    };
    Ok(Ran::of(summary, worked))
}

async fn newest(state: &AppState) -> Result<Option<String>, JobError> {
    let settings = kaeso_store::pg::updates::settings(&state.pg).await?;
    let releases = updates::known(kaeso_store::pg::updates::releases(&state.pg).await?);
    Ok(updates::plan(&settings, &releases).map(|p| p.target.version.to_string()))
}
