use crate::auth::sessions::IDLE_LIFETIME;
use crate::jobs::JobError;
use crate::state::AppState;

pub async fn run(state: AppState) -> Result<String, JobError> {
    let now = state.clock.now();
    let sessions =
        kaeso_store::pg::sessions::delete_stale(&state.pg, now, now - IDLE_LIFETIME).await?;
    let tokens = kaeso_store::pg::tokens::delete_stale(&state.pg, now).await?;
    let invites = kaeso_store::pg::invites::delete_stale(&state.pg, now).await?;
    Ok(format!(
        "deleted {sessions} sessions, {tokens} tokens, {invites} invites"
    ))
}
