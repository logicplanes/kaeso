use crate::services::models_dev::{self, Synced};
use crate::state::AppState;

pub async fn run(state: AppState) -> Result<String, String> {
    if !models_dev::due(&state).await.map_err(|e| e.to_string())? {
        return Ok("not due".into());
    }
    match models_dev::sync(&state).await.map_err(|e| e.to_string())? {
        Synced::Unchanged => Ok("unchanged".into()),
        Synced::Applied(a) => Ok(format!(
            "applied: {} price changes, {} delisted, {} listed again",
            a.price_changes, a.delisted, a.relisted
        )),
    }
}
