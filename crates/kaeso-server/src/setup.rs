use kaeso_core::secret;
use kaeso_crypto::random_token;
use kaeso_store::StoreError;
use kaeso_store::pg::tokens::{self, NewToken, TokenKind};
use kaeso_store::pg::{locks, users, with_advisory_lock};

use crate::state::AppState;

pub const SETUP_LINK_LIFETIME: time::Duration = time::Duration::hours(24);

pub async fn announce(state: &AppState) {
    match users::any_exist(&state.pg).await {
        Ok(true) => {}
        Ok(false) if state.config.setup.email().is_some() => email_link(state).await,
        Ok(false) => tracing::info!(
            "Kaeso is not set up yet; run `kaeso setup-link` where the gateway runs to print the one-time setup link."
        ),
        Err(e) => tracing::error!(error = %e, "cannot tell whether Kaeso is set up"),
    }
}

// The address is the operator's own setting, so it may be logged; the link never is.
async fn email_link(state: &AppState) {
    let Some(to) = state.config.setup.email() else {
        return;
    };
    let url = match issue_link(state).await {
        Ok(Some(url)) => url,
        Ok(None) => return,
        Err(e) => {
            tracing::error!(error = %e, "cannot create the setup link to email");
            return;
        }
    };
    let email = kaeso_mail::templates::setup_link(kaeso_mail::Locale::En, to, &url);
    match state.mailer.send_now(email).await {
        Ok(()) => tracing::info!(
            to,
            "Kaeso is not set up yet; the one-time setup link was emailed to the address in setup.email"
        ),
        Err(e) => {
            tracing::error!(error = %e, to, "cannot email the setup link; run `kaeso setup-link` where the gateway runs instead")
        }
    }
}

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
