use kaeso_store::pg::keys;
use uuid::Uuid;

use crate::cache::hex;
use crate::state::AppState;

pub async fn forget(state: &AppState, ids: &[Uuid]) {
    if ids.is_empty() {
        return;
    }
    match keys::hashes_of(&state.pg, ids).await {
        Ok(hashes) => {
            for h in hashes {
                state.caches.api_keys.invalidate(&hex(&h)).await;
            }
        }
        Err(e) => tracing::error!(error = %e, "cannot drop changed keys from the cache"),
    }
}

pub async fn forget_org(state: &AppState, org: Uuid) {
    match keys::hashes_of_org(&state.pg, org).await {
        Ok(hashes) => {
            for h in hashes {
                state.caches.api_keys.invalidate(&hex(&h)).await;
                state.caches.org_keys.invalidate(&hex(&h)).await;
            }
        }
        Err(e) => tracing::error!(error = %e, "cannot drop the org's keys from the cache"),
    }
}

pub async fn forget_org_key(state: &AppState, id: Uuid) {
    match keys::org_hash_of(&state.pg, id).await {
        Ok(Some(h)) => state.caches.org_keys.invalidate(&hex(&h)).await,
        Ok(None) => {}
        Err(e) => tracing::error!(error = %e, "cannot drop the platform key from the cache"),
    }
}

pub async fn forget_user_in_org(state: &AppState, org: Uuid, user: Uuid) {
    match keys::ids_of_user_in_org(&state.pg, org, user).await {
        Ok(ids) => forget(state, &ids).await,
        Err(e) => tracing::error!(error = %e, "cannot drop the user's keys from the cache"),
    }
}
