use crate::auth::sessions::IDLE_LIFETIME;
use crate::state::AppState;

pub async fn run(state: AppState) -> Result<String, String> {
    let now = state.clock.now();
    let sessions = kaeso_store::pg::sessions::delete_stale(&state.pg, now, now - IDLE_LIFETIME)
        .await
        .map_err(|e| e.to_string())?;
    let tokens = kaeso_store::pg::tokens::delete_stale(&state.pg, now)
        .await
        .map_err(|e| e.to_string())?;
    let invites = kaeso_store::pg::invites::delete_stale(&state.pg, now)
        .await
        .map_err(|e| e.to_string())?;
    Ok(format!(
        "deleted {sessions} sessions, {tokens} tokens, {invites} invites"
    ))
}
