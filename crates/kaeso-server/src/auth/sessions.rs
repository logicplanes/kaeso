use std::net::IpAddr;
use std::sync::Arc;

use kaeso_core::id::{SessionId, UserId};
use kaeso_core::secret;
use kaeso_crypto::random_token;
use kaeso_store::StoreError;
use kaeso_store::pg::sessions::{self, NewSession, SessionRow};
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
    pub ip: Option<IpAddr>,
    pub user_agent: Option<String>,
}

#[derive(Debug)]
pub struct Created {
    pub token: secrecy::SecretString,
    pub id: SessionId,
}

pub async fn create(
    db: &mut sqlx::PgConnection,
    now: OffsetDateTime,
    s: &SignIn,
) -> Result<Created, StoreError> {
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
            now,
            expires_at: now + ABSOLUTE_LIFETIME,
            ip: s.ip,
            user_agent: s.user_agent.as_deref(),
        },
    )
    .await?;
    Ok(Created {
        token: secrecy::SecretString::from(token),
        id,
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
        sessions::touch(&state.pg, row.id, now).await?;
        let mut touched = (*row).clone();
        touched.last_used_at = now;
        state.caches.sessions.put(&key, touched).await;
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

pub async fn revoke_all(
    state: &AppState,
    user: UserId,
    keep: Option<SessionId>,
) -> Result<usize, StoreError> {
    let hashes = sessions::revoke_all(
        &state.pg,
        user.uuid(),
        keep.map(|k| k.uuid()),
        state.clock.now(),
    )
    .await?;
    forget(state, &hashes).await;
    Ok(hashes.len())
}

pub async fn refresh_user(state: &AppState, user: UserId) -> Result<(), StoreError> {
    let hashes = sessions::hashes_for_user(&state.pg, user.uuid()).await?;
    forget(state, &hashes).await;
    Ok(())
}
