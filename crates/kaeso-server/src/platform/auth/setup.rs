use axum::extract::State;
use axum::response::Response;
use kaeso_core::id::UserId;
use kaeso_core::secret;
use kaeso_store::pg::tokens::{self, TokenKind};
use kaeso_store::pg::{instance, users};
use secrecy::SecretString;
use serde::Deserialize;
use utoipa::ToSchema;

use super::{SignedIn, sign_in, throttle_ip};
use crate::auth::sessions::Method;
use crate::auth::throttle::TOKENS_PER_IP;
use crate::platform::access::Ctx;
use crate::platform::audit::Action;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
pub(crate) use crate::platform::validate::{validate_email, validate_name};
use crate::platform::views::OrgRef;
use crate::services::orgs;
use crate::state::AppState;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "The first user and organization.")]
pub struct SetupRequest {
    #[doc = "The token from the setup link (`…/setup#token=…`)."]
    #[schema(value_type = String, write_only)]
    pub token: SecretString,
    #[doc = "The first user's email address."]
    #[schema(example = "owner@example.com")]
    pub email: String,
    #[doc = "The first user's name."]
    #[schema(example = "Ada Lovelace")]
    pub name: String,
    #[doc = "The first user's password, 12 to 128 characters. Required while password\n\
        sign-in is on (`password` in `auth/methods`); refused while it is off,\n\
        where the first user signs in with a sign-in link or SSO."]
    #[schema(value_type = Option<String>, format = Password, write_only)]
    pub password: Option<SecretString>,
    #[doc = "The first organization's name."]
    #[schema(example = "Acme")]
    pub org_name: String,
    #[doc = "The install's IANA time zone, usually the first user's; the console\n\
        shows every time in it. Default: `instance.timezone` from the configuration."]
    #[schema(example = "Europe/Berlin")]
    pub timezone: Option<String>,
}

pub(crate) async fn hash_password(param: &str, password: SecretString) -> ApiResult<String> {
    kaeso_crypto::password::check_policy(&password)
        .map_err(|e| ApiError::invalid(param, e.to_string()))?;
    tokio::task::spawn_blocking(move || kaeso_crypto::password::hash(&password))
        .await
        .map_err(ApiError::internal)?
        .map_err(ApiError::internal)
}

#[utoipa::path(
    post,
    path = "/auth/setup",
    summary = "Creates the first user (an instance admin) and the first organization,\n\
    whose owner they become, and signs them in. The setup link works once.",
    description = "With password sign-in off, the first user has no password: they connect \
    the organization's identity provider in this session, and `kaeso user sign-in-link` \
    prints a one-time sign-in link for them until it works.",
    tag = "auth",
    request_body = SetupRequest,
    responses(
        (status = 200, description = "Set up; the session cookie is set.", body = SignedIn),
    )
)]
pub async fn setup(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<SetupRequest>,
) -> ApiResult<Response> {
    throttle_ip(&state, &ctx, TOKENS_PER_IP).await?;
    let email = validate_email("email", &req.email)?;
    let name = validate_name("name", &req.name)?;
    let org_name = validate_name("org_name", &req.org_name)?;
    let timezone = req
        .timezone
        .as_deref()
        .map(|z| crate::platform::validate::validate_time_zone("timezone", z))
        .transpose()?;
    let password_hash = match (state.config.auth.password, req.password) {
        (true, Some(password)) => Some(hash_password("password", password).await?),
        (true, None) => {
            return Err(ApiError::invalid("password", "`password` is required."));
        }
        (false, Some(_)) => {
            return Err(ApiError::invalid(
                "password",
                "Password sign-in is turned off on this install; leave `password` out.",
            ));
        }
        (false, None) => None,
    };

    let mut tx = state.pg.begin().await?;
    kaeso_store::pg::lock_setup(&mut tx).await?;
    if users::any_exist(&mut *tx).await? {
        return Err(ApiError::Conflict(
            "Kaeso is already set up; sign in instead.".into(),
        ));
    }
    let token = secret::hash(secrecy::ExposeSecret::expose_secret(&req.token));
    if tokens::consume(&mut *tx, TokenKind::Setup, &token, ctx.now)
        .await?
        .is_none()
    {
        return Err(ApiError::invalid(
            "token",
            "The setup link is invalid or expired; run `kaeso setup-link` for a new one.",
        ));
    }
    let user = UserId::new();
    users::insert(
        &mut *tx,
        &users::NewUser {
            id: user.uuid(),
            email: &email,
            name: &name,
            password_hash: password_hash.as_deref(),
            email_verified_at: Some(ctx.now),
            is_instance_admin: true,
        },
    )
    .await?;
    let (org, project) = orgs::create(&mut tx, &org_name, user).await?;
    instance::complete_setup(&mut *tx, ctx.now).await?;
    if let Some(zone) = &timezone {
        instance::set_time_zone(&mut *tx, zone).await?;
    }
    ctx.audit(None, Action::InstanceSetup)
        .by(user)
        .target("user", user)
        .after(serde_json::json!({ "email": email, "name": name, "is_instance_admin": true, "has_password": password_hash.is_some() }))
        .write(&mut tx)
        .await?;
    ctx.audit(Some(org), Action::OrgCreate)
        .by(user)
        .target("org", org)
        .after(serde_json::json!({
            "org": OrgRef { id: org, name: org_name.clone() },
            "default_project": project.to_string(),
            "owner": user.to_string(),
        }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    if timezone.is_some() {
        state.caches.time_zone.invalidate_all();
    }

    let row = users::get(&state.pg, user.uuid())
        .await?
        .ok_or_else(|| ApiError::internal("the new user vanished"))?;
    sign_in(&state, &ctx, &row, Method::Setup, false, None).await
}
