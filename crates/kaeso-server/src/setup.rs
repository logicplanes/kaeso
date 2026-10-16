use kaeso_core::secret;
use kaeso_crypto::random_token;
use kaeso_store::StoreError;
use kaeso_store::pg::tokens::{self, NewToken, TokenKind};
use kaeso_store::pg::{locks, users, with_advisory_lock};

use crate::state::AppState;

pub const SETUP_LINK_LIFETIME: time::Duration = time::Duration::hours(24);

pub async fn issue_link(state: &AppState) -> Result<Option<String>, StoreError> {
    with_advisory_lock(&state.pg, locks::SETUP, || async {
        if users::any_exist(&state.pg).await? {
            return Ok(None);
        }
        let now = state.clock.now();
        let token = random_token();
        let hash = secret::hash(&token);
        let mut tx = state.pg.begin().await?;
        tokens::retire_all(&mut *tx, TokenKind::Setup, None, now).await?;
        tokens::insert(
            &mut *tx,
            NewToken {
                kind: TokenKind::Setup,
                hash: &hash,
                user_id: None,
                email: None,
                data: serde_json::json!({}),
                expires_at: now + SETUP_LINK_LIFETIME,
            },
        )
        .await?;
        tx.commit().await?;
        Ok(Some(setup_url(state, &token)))
    })
    .await
}

fn setup_url(state: &AppState, token: &str) -> String {
    format!("{}setup#token={token}", state.config.public_url)
}
