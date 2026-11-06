use std::time::Duration;

use kaeso_store::pg::mcp_servers;
use uuid::Uuid;

use super::JobError;
use crate::services::mcp_tools;
use crate::state::AppState;

const BATCH: i64 = 50;
const SHORTEST: Duration = Duration::from_secs(60);

// Each server's check comes up to a tenth of the interval early, by its id, so servers added at
// the same time don't call out in the same minute every time.
fn jitter(id: Uuid, interval: Duration) -> Duration {
    let spread = u128::from(interval.as_secs() / 10);
    if spread == 0 {
        return Duration::ZERO;
    }
    Duration::from_secs(u64::try_from(id.as_u128() % spread).unwrap_or(0))
}

fn time(d: Duration) -> time::Duration {
    time::Duration::try_from(d).unwrap_or(time::Duration::MAX)
}

// One server at a time: a sync calls the server, and an instance with many servers should not
// open as many connections at once.
pub async fn run(state: AppState) -> Result<String, JobError> {
    let interval = state.config.mcp_servers.sync_interval.max(SHORTEST);
    let now = state.clock.now();
    let earliest = now - time(interval) + time(interval / 10);
    let mut synced = 0usize;
    let mut failed = 0usize;
    for id in mcp_servers::due(&state.pg, earliest, BATCH).await? {
        let Some(server) = mcp_servers::get_by_id(&state.pg, id).await? else {
            continue;
        };
        let wait = time(interval) - time(jitter(id, interval));
        if server.last_checked_at.is_some_and(|at| now - at < wait) {
            continue;
        }
        match mcp_tools::sync(&state, &server).await {
            Ok(_) => synced += 1,
            Err(e) => {
                failed += 1;
                tracing::info!(server = %id, error = %e, "an MCP server couldn't be synced");
            }
        }
    }
    Ok(format!("{synced} MCP servers synced, {failed} not reached"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jitter_stays_within_a_tenth_of_the_interval() {
        let interval = Duration::from_secs(900);
        for n in [0u128, 1, 89, 90, 12_345, u128::MAX] {
            assert!(jitter(Uuid::from_u128(n), interval) < Duration::from_secs(90));
        }
        assert_eq!(
            jitter(Uuid::from_u128(7), Duration::from_secs(5)),
            Duration::ZERO
        );
        assert_ne!(
            jitter(Uuid::from_u128(1), interval),
            jitter(Uuid::from_u128(2), interval)
        );
    }
}
