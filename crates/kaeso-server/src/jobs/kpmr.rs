use crate::jobs::{JobError, Ran};
use crate::services::kpmr::{self, Synced};
use crate::state::AppState;

pub async fn run(state: AppState) -> Result<Ran, JobError> {
    if !kpmr::due(&state).await? {
        return Ok(Ran::idle("not due"));
    }
    match kpmr::sync(&state).await? {
        Synced::Unchanged => Ok(Ran::idle("unchanged")),
        Synced::Applied(a) => Ok(Ran::worked(format!(
            "applied: {} price changes, {} delisted, {} listed again",
            a.price_changes, a.delisted, a.relisted
        ))),
    }
}
