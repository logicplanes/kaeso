use kaeso_core::id::OrgId;

use crate::jobs::JobError;
use crate::state::AppState;

pub async fn run(state: AppState) -> Result<String, JobError> {
    let now = state.clock.now();
    let due = kaeso_store::pg::orgs::due_for_deletion(&state.pg, now).await?;
    let mut deleted = 0;
    for org in due {
        match crate::services::orgs::purge(&state, org, now).await {
            Ok(true) => deleted += 1,
            Ok(false) => {}
            Err(e) => {
                tracing::warn!(org = %OrgId::from_uuid(org), error = %e, "cannot delete the org yet; retrying later");
            }
        }
    }
    Ok(format!("deleted {deleted} organizations"))
}
