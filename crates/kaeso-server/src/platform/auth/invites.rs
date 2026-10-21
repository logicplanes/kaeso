use axum::Json;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use kaeso_core::id::{OrgId, UserId};
use kaeso_core::secret;
use kaeso_store::pg::invites::{self, Invite};
use kaeso_store::pg::{orgs, users};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;

use super::setup::{hash_password, validate_name};
use super::{sign_in, throttle_ip};
use crate::auth::sessions::Method;
use crate::auth::throttle::TOKENS_PER_IP;
use crate::platform::access::{Caller, Ctx};
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::platform::views::OrgRef;
use crate::services::invites::{Accepted, accept};
use crate::state::AppState;

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "The token from an invitation link.")]
pub struct InviteToken {
    #[doc = "The token from `…/invite#token=…`."]
    #[schema(value_type = String, write_only)]
    pub token: SecretString,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "What an invitation offers.")]
pub struct InviteInfo {
    #[doc = "The org; `null` for an invitation to the install."]
    pub org: Option<OrgRef>,
    #[doc = "The invited address."]
    pub email: String,
    #[doc = "The org role it gives; `null` for an invitation to the install."]
    pub role: Option<crate::platform::grants::RoleRef>,
    #[doc = "Deprecated: the org role's built-in name, `custom` for a custom role."]
    #[schema(deprecated)]
    pub preset: Option<String>,
    #[doc = "Whether an account with this address exists (then sign in to accept)."]
    pub account_exists: bool,
    #[doc = "When the invitation expires."]
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
}

async fn pending(state: &AppState, ctx: &Ctx, token: &SecretString) -> ApiResult<Invite> {
    throttle_ip(state, ctx, TOKENS_PER_IP).await?;
    invites::get_pending_by_hash(&state.pg, &secret::hash(token.expose_secret()), ctx.now)
        .await?
        .ok_or_else(|| {
            ApiError::NotFound("This invitation is no longer valid; ask for a new one.".into())
        })
}

#[utoipa::path(
    post,
    path = "/auth/invites/lookup",
    summary = "Shows what an invitation offers, so the console can ask the right questions.",
    tag = "auth",
    request_body = InviteToken,
    responses((status = 200, body = InviteInfo))
)]
pub async fn lookup(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<InviteToken>,
) -> ApiResult<Json<InviteInfo>> {
    let invite = pending(&state, &ctx, &req.token).await?;
    let org = match invite.org_id {
        Some(id) => orgs::get(&state.pg, id).await?.map(|o| OrgRef {
            id: OrgId::from_uuid(o.id),
            name: o.name,
        }),
        None => None,
    };
    let role = match (invite.org_id, invite.org_preset.as_deref()) {
        (Some(org), Some(preset)) => {
            let book = crate::platform::grants::RoleBook::load(&state.pg, org).await?;
            Some(match preset.parse::<kaeso_core::rights::OrgPreset>() {
                Ok(p) => crate::platform::grants::RoleRef::org_builtin(p),
                Err(_) => crate::platform::grants::RoleRef::custom(
                    invite.org_role_id,
                    invite
                        .org_role_id
                        .and_then(|id| book.get(id))
                        .map(|r| r.name.as_str()),
                ),
            })
        }
        _ => None,
    };
    Ok(Json(InviteInfo {
        org,
        account_exists: users::get_by_email(&state.pg, &invite.email)
            .await?
            .is_some(),
        email: invite.email,
        role,
        preset: invite.org_preset,
        expires_at: invite.expires_at,
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "Accepting an invitation.")]
pub struct AcceptRequest {
    #[doc = "The token from the invitation link."]
    #[schema(value_type = String, write_only)]
    pub token: SecretString,
    #[doc = "For a new account: the display name."]
    #[schema(example = "Ada Lovelace")]
    pub name: Option<String>,
    #[doc = "For a new account: a password (12 to 128 characters); leave out to sign in by sign-in link or SSO."]
    #[schema(value_type = Option<String>, format = Password, write_only)]
    pub password: Option<SecretString>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The invitation was accepted.")]
pub struct AcceptedView {
    #[doc = "Always `accepted`."]
    #[schema(example = "accepted")]
    pub status: &'static str,
    #[doc = "The org joined; `null` for an invitation to the install."]
    pub org_id: Option<OrgId>,
}

#[utoipa::path(
    post,
    path = "/auth/invites/accept",
    summary = "Accepts an invitation. Signed in, the session's user must have the\n\
    invited address. Not signed in, the account is created (with `name`) and\n\
    signed in; an existing account must sign in first. Refused for a member of\n\
    the org; accepting revokes the address's other pending invitations to it.",
    tag = "auth",
    request_body = AcceptRequest,
    responses(
        (status = 200, description = "Accepted. A new account is also signed in (the session cookie is set).", body = AcceptedView),
    )
)]
pub async fn accept_invite(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<AcceptRequest>,
) -> ApiResult<Response> {
    let invite = pending(&state, &ctx, &req.token).await?;
    let existing = users::get_by_email(&state.pg, &invite.email).await?;
    if let Caller::Session(session) = &ctx.caller {
        let me = users::get(&state.pg, session.user_id)
            .await?
            .ok_or_else(|| ApiError::not_found("user"))?;
        let mine = me
            .email
            .as_deref()
            .is_some_and(|e| e.eq_ignore_ascii_case(&invite.email));
        if !mine {
            return Err(ApiError::PermissionDenied(format!(
                "This invitation is for {}; sign in with that address to accept it.",
                invite.email
            )));
        }
        let user = UserId::from_uuid(me.id);
        let mut tx = state.pg.begin().await?;
        let org = finish(&state, &ctx, &mut tx, &invite, user).await?;
        tx.commit().await?;
        return Ok(Json(AcceptedView {
            status: "accepted",
            org_id: org,
        })
        .into_response());
    }
    if existing.is_some() {
        return Err(ApiError::Conflict(format!(
            "An account with {} exists; sign in to accept the invitation.",
            invite.email
        )));
    }
    let name = validate_name("name", req.name.as_deref().unwrap_or_default())?;
    let password_hash = match req.password {
        Some(p) => Some(hash_password("password", p).await?),
        None => None,
    };
    let user = UserId::new();
    let mut tx = state.pg.begin().await?;
    let created = users::insert(
        &mut *tx,
        &users::NewUser {
            id: user.uuid(),
            email: &invite.email,
            name: &name,
            password_hash: password_hash.as_deref(),
            email_verified_at: Some(ctx.now),
            is_instance_admin: false,
        },
    )
    .await?;
    if !created {
        return Err(ApiError::Conflict(format!(
            "An account with {} exists; sign in to accept the invitation.",
            invite.email
        )));
    }
    ctx.audit(None, "user.create_by_invite")
        .by(user)
        .target("user", user)
        .after(serde_json::json!({ "email": invite.email }))
        .write(&mut tx)
        .await?;
    finish(&state, &ctx, &mut tx, &invite, user).await?;
    tx.commit().await?;
    let row = users::get(&state.pg, user.uuid())
        .await?
        .ok_or_else(|| ApiError::internal("new user vanished"))?;
    sign_in(&state, &ctx, &row, Method::Signup, false, None).await
}

async fn finish(
    state: &AppState,
    ctx: &Ctx,
    tx: &mut sqlx::PgConnection,
    invite: &Invite,
    user: UserId,
) -> ApiResult<Option<OrgId>> {
    if let Some(org) = invite.org_id {
        crate::services::orgs::ensure_changeable(&state.pg, org).await?;
    }
    let outcome = accept(&mut *tx, invite, user, ctx.now).await?;
    let (org, revoked_others) = match outcome {
        Accepted::Gone => {
            return Err(ApiError::NotFound(
                "This invitation is no longer valid.".into(),
            ));
        }
        Accepted::AlreadyMember => {
            return Err(ApiError::Conflict(format!(
                "{} is already a member of this organization.",
                invite.email
            )));
        }
        Accepted::Instance => (None, 0),
        Accepted::Joined {
            org,
            revoked_others,
        } => (Some(org), revoked_others),
    };
    ctx.audit(org, "invite.accept")
        .by(user)
        .target("invite", kaeso_core::id::InviteId::from_uuid(invite.id))
        .after(serde_json::json!({ "revoked_invites": revoked_others }))
        .write(tx)
        .await?;
    Ok(org)
}
