use time::Duration;

use crate::auth::agents::UNUSED_LIFETIME;
use crate::auth::sessions::IDLE_LIFETIME;
use crate::jobs::{JobError, Ran};
use crate::state::AppState;

const AGENT_ACTIVITY_KEPT: Duration = Duration::days(90);
const UNUSED_CLIENTS_KEPT: Duration = Duration::days(1);

pub async fn run(state: AppState) -> Result<Ran, JobError> {
    let now = state.clock.now();
    let sessions =
        kaeso_store::pg::sessions::delete_stale(&state.pg, now, now - IDLE_LIFETIME).await?;
    let tokens = kaeso_store::pg::tokens::delete_stale(&state.pg, now).await?;
    let invites = kaeso_store::pg::invites::delete_stale(&state.pg, now).await?;
    let agents =
        kaeso_store::pg::agents::revoke_unused(&state.pg, now - UNUSED_LIFETIME, now).await?;
    let mut conn = state
        .pg
        .acquire()
        .await
        .map_err(kaeso_store::StoreError::from)?;
    let swept = kaeso_store::pg::agents::sweep(
        &mut conn,
        now,
        now - AGENT_ACTIVITY_KEPT,
        now - UNUSED_CLIENTS_KEPT,
    )
    .await?;
    let deliveries =
        kaeso_store::pg::webhooks::prune(&state.pg, now - crate::services::webhooks::KEEP).await?;
    let any = sessions
        + tokens
        + invites
        + agents
        + swept.requests
        + swept.tokens
        + swept.clients
        + swept.activity
        + deliveries
        > 0;
    Ok(Ran::of(
        format!(
            "deleted {sessions} sessions, {tokens} tokens, {invites} invites; revoked {agents} agents; \
             deleted {} agent requests, {} agent tokens, {} agent clients, {} agent tool calls, \
             {deliveries} webhook deliveries",
            swept.requests, swept.tokens, swept.clients, swept.activity
        ),
        any,
    ))
}
