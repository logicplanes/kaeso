use std::net::IpAddr;
use std::sync::Arc;

use kaeso_core::id::{SessionId, UserId};
use kaeso_core::secret;
use kaeso_crypto::random_token;
use kaeso_store::StoreError;
use kaeso_store::pg::sessions::{self, NewSession, SessionRow, Touch};
use kaeso_store::pg::tokens::{self, TokenKind};
use time::{Duration, OffsetDateTime};

use crate::cache::hex;
use crate::state::AppState;

pub const IDLE_LIFETIME: Duration = Duration::days(7);
pub const ABSOLUTE_LIFETIME: Duration = Duration::days(30);
pub const RECENT_SIGN_IN: Duration = Duration::minutes(15);
const TOUCH_EVERY: Duration = Duration::seconds(60);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Password,
    MagicLink,
    Oidc,
    Setup,
    Signup,
}

impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Method::Password => "password",
            Method::MagicLink => "magic_link",
            Method::Oidc => "oidc",
            Method::Setup => "setup",
            Method::Signup => "signup",
        }
    }
}

#[derive(Clone, Debug)]
pub struct SignIn {
    pub user_id: UserId,
    pub method: Method,
    pub second_factor: bool,
    pub oidc_org: Option<uuid::Uuid>,
    pub signed_in_at: OffsetDateTime,
    pub ip: Option<IpAddr>,
    pub user_agent: Option<String>,
}

#[derive(Debug)]
pub struct Created {
    pub token: secrecy::SecretString,
    pub id: SessionId,
    pub expires_at: OffsetDateTime,
}

pub async fn create(
    db: &mut sqlx::PgConnection,
    now: OffsetDateTime,
    s: &SignIn,
) -> Result<Created, StoreError> {
    create_until(db, now, s, now + ABSOLUTE_LIFETIME).await
}

pub async fn create_until(
    db: &mut sqlx::PgConnection,
    now: OffsetDateTime,
    s: &SignIn,
    expires_at: OffsetDateTime,
) -> Result<Created, StoreError> {
    let expires_at = expires_at.min(now + ABSOLUTE_LIFETIME);
    let token = random_token();
    let hash = secret::hash(&token);
    let id = SessionId::new();
    sessions::insert(
        db,
        &NewSession {
            id: id.uuid(),
            token_hash: &hash,
            user_id: s.user_id.uuid(),
            method: s.method.as_str(),
            second_factor: s.second_factor,
            oidc_org_id: s.oidc_org,
            signed_in_at: s.signed_in_at.min(now),
            now,
            expires_at,
            ip: s.ip,
            user_agent: s.user_agent.as_deref(),
        },
    )
    .await?;
    Ok(Created {
        token: secrecy::SecretString::from(token),
        id,
        expires_at,
    })
}

pub async fn authenticate(
    state: &AppState,
    token: &str,
) -> Result<Option<Arc<SessionRow>>, StoreError> {
    let hash = secret::hash(token);
    let key = hex(&hash);
    let pg = state.pg.clone();
    let Some(row) = state
        .caches
        .sessions
        .get_or_load(
            &key,
            || async move { sessions::get_by_hash(&pg, &hash).await },
        )
        .await?
    else {
        return Ok(None);
    };
    let now = state.clock.now();
    if row.user_state != "active"
        || now >= row.expires_at
        || now - row.last_used_at >= IDLE_LIFETIME
    {
        return Ok(None);
    }
    if now - row.last_used_at >= TOUCH_EVERY {
        // Writing the touched row back could land after a revocation's invalidation and cache
        // the revoked session again, so the next request reads the touched row from PostgreSQL.
        match sessions::touch(&state.pg, row.id, now).await? {
            Touch::Touched => state.caches.sessions.invalidate(&key).await,
            Touch::Ended => {
                state.caches.sessions.invalidate(&key).await;
                return Ok(None);
            }
            Touch::Recent => {}
        }
    }
    Ok(Some(row))
}

pub async fn forget(state: &AppState, hashes: &[Vec<u8>]) {
    for h in hashes {
        state.caches.sessions.invalidate(&hex(h)).await;
    }
}

pub async fn revoke(state: &AppState, user: UserId, id: SessionId) -> Result<bool, StoreError> {
    let hash = sessions::revoke(&state.pg, user.uuid(), id.uuid(), state.clock.now()).await?;
    if let Some(h) = &hash {
        forget(state, std::slice::from_ref(h)).await;
    }
    Ok(hash.is_some())
}

pub async fn replace(state: &AppState, user: UserId, id: SessionId) -> Result<bool, StoreError> {
    let hash = sessions::replace(&state.pg, user.uuid(), id.uuid(), state.clock.now()).await?;
    if let Some(h) = &hash {
        forget(state, std::slice::from_ref(h)).await;
    }
    Ok(hash.is_some())
}

pub async fn was_replaced(state: &AppState, token: &str) -> Result<bool, StoreError> {
    sessions::is_replaced(&state.pg, &secret::hash(token)).await
}

pub async fn revoke_all(
    state: &AppState,
    user: UserId,
    keep: Option<SessionId>,
) -> Result<usize, StoreError> {
    let now = state.clock.now();
    let hashes = sessions::revoke_all(&state.pg, user.uuid(), keep.map(|k| k.uuid()), now).await?;
    for kind in [TokenKind::EmailChange, TokenKind::EmailChangeCancel] {
        tokens::retire_all(&state.pg, kind, Some(user.uuid()), now).await?;
    }
    forget(state, &hashes).await;
    Ok(hashes.len())
}

pub async fn refresh_user(state: &AppState, user: UserId) -> Result<(), StoreError> {
    let hashes = sessions::hashes_for_user(&state.pg, user.uuid()).await?;
    forget(state, &hashes).await;
    Ok(())
}
