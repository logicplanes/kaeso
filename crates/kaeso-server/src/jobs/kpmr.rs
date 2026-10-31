use crate::jobs::JobError;
use crate::services::kpmr::{self, Synced};
use crate::state::AppState;

pub async fn run(state: AppState) -> Result<String, JobError> {
    if !kpmr::due(&state).await? {
        return Ok("not due".into());
    }
    match kpmr::sync(&state).await? {
        Synced::Unchanged => Ok("unchanged".into()),
        Synced::Applied(a) => Ok(format!(
            "applied: {} price changes, {} delisted, {} listed again",
            a.price_changes, a.delisted, a.relisted
        )),
    }
}
