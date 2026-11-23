use kaeso_core::id::OrgId;

use crate::jobs::{JobError, Ran};
use crate::state::AppState;

pub async fn run(state: AppState) -> Result<Ran, JobError> {
    let now = state.clock.now();
    let due = kaeso_store::pg::orgs::due_for_deletion(&state.pg, now).await?;
    let mut deleted = 0;
    let mut failed = 0;
    for org in due {
        match crate::services::orgs::purge(&state, org, now).await {
            Ok(true) => deleted += 1,
            Ok(false) => {}
            Err(e) => {
                failed += 1;
                tracing::warn!(org = %OrgId::from_uuid(org), error = %e, "cannot delete the org yet; retrying later");
            }
        }
    }
    Ok(Ran::of(
        format!("deleted {deleted} organizations"),
        deleted + failed > 0,
    ))
}
