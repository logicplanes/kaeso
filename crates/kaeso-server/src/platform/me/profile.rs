use axum::Json;
use axum::extract::State;
use kaeso_core::id::UserId;
use kaeso_core::secret;
use kaeso_crypto::random_token;
use kaeso_store::pg::tokens::{self, NewToken, TokenKind};
use kaeso_store::pg::{avatars, totp, users};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use time::Duration;
use utoipa::ToSchema;

use crate::auth::throttle::{self, EMAILS_PER_ADDRESS, EMAILS_PER_IP, TOKENS_PER_IP};
use crate::platform::access::Ctx;
use crate::platform::auth::setup::{validate_email, validate_name};
use crate::platform::auth::{EmailSent, refused, throttle_ip};
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::platform::me::security::{SecondFactor, require_second_factor};
use crate::platform::views::UserView;
use crate::state::AppState;

const EMAIL_CHANGE_LIFETIME: Duration = Duration::hours(24);

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The signed-in user's account.")]
pub struct Me {
    #[doc = "The user."]
    #[serde(flatten)]
    pub user: UserView,
    #[doc = "Whether the user has confirmed TOTP."]
    pub totp_enabled: bool,
    #[doc = "Whether a password is set."]
    pub has_password: bool,
    #[doc = "Unused recovery codes."]
    pub recovery_codes_left: i64,
    #[doc = "The language the console is shown in: `auto` follows the browser."]
    pub locale: UserLocale,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[schema(description = "The language a person reads the console in.")]
pub enum UserLocale {
    #[default]
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "en")]
    En,
    #[serde(rename = "de")]
    De,
    #[serde(rename = "zh-CN")]
    ZhCn,
}

impl UserLocale {
    fn stored(self) -> Option<&'static str> {
        match self {
            Self::Auto => None,
            Self::En => Some("en"),
            Self::De => Some("de"),
            Self::ZhCn => Some("zh-CN"),
        }
    }

    pub(crate) fn from_stored(s: Option<&str>) -> Self {
        match s {
            Some("en") => Self::En,
            Some("de") => Self::De,
            Some("zh-CN") => Self::ZhCn,
            _ => Self::Auto,
        }
    }
}

pub(crate) async fn load(state: &AppState, user: UserId) -> ApiResult<(users::User, Me)> {
    let row = users::get(&state.pg, user.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("user"))?;
    let me = Me {
        user: UserView::from_row(&row),
        totp_enabled: totp::is_enabled(&state.pg, user.uuid()).await?,
        has_password: row.password_hash.is_some(),
        recovery_codes_left: totp::remaining_recovery_codes(&state.pg, user.uuid()).await?,
        locale: UserLocale::from_stored(
            users::get_locale(&state.pg, user.uuid()).await?.as_deref(),
        ),
    };
    Ok((row, me))
}

#[utoipa::path(
    get,
    path = "/me",
    summary = "The signed-in user's profile and security state.",
    tag = "me",
    responses((status = 200, body = Me))
)]
pub async fn get(State(state): State<AppState>, ctx: Ctx) -> ApiResult<Json<Me>> {
    Ok(Json(load(&state, ctx.user_id()?).await?.1))
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "Profile changes.")]
pub struct ProfileUpdate {
    #[doc = "A new display name, 1 to 100 characters."]
    #[schema(example = "Ada Lovelace")]
    pub name: Option<String>,
    #[doc = "The language the console is shown in; `auto` follows the browser."]
    pub locale: Option<UserLocale>,
}

#[utoipa::path(
    patch,
    path = "/me",
    summary = "Changes the user's display name or language.",
    tag = "me",
    request_body = ProfileUpdate,
    responses((status = 200, body = Me))
)]
pub async fn update(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<ProfileUpdate>,
) -> ApiResult<Json<Me>> {
    let user = ctx.user_id()?;
    if let Some(name) = req.name {
        let name = validate_name("name", &name)?;
        let before = users::get(&state.pg, user.uuid())
            .await?
            .and_then(|u| u.name);
        let mut tx = state.pg.begin().await?;
        users::set_name(&mut *tx, user.uuid(), &name).await?;
        ctx.audit(None, "user.update")
            .target("user", user)
            .before(serde_json::json!({ "name": before }))
            .after(serde_json::json!({ "name": name }))
            .write(&mut tx)
            .await?;
        tx.commit().await?;
    }
    if let Some(locale) = req.locale {
        users::set_locale(&state.pg, user.uuid(), locale.stored()).await?;
    }
    Ok(Json(load(&state, user).await?.1))
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "A new email address.")]
pub struct EmailChange {
    #[doc = "The new address. A confirmation link is sent to it."]
    #[schema(example = "ada@example.com")]
    pub email: String,
    #[doc = "A code or recovery code, needed while TOTP is on."]
    #[serde(flatten)]
    pub second_factor: SecondFactor,
}

#[utoipa::path(
    post,
    path = "/me/email",
    summary = "Starts an email change: sends a confirmation link to the new address\n\
    and a link to cancel it to the current one. Needs a recent sign-in and,\n\
    while TOTP is on, a code. Ending the session that asked, or every\n\
    session, cancels the change.",
    tag = "me",
    request_body = EmailChange,
    responses((status = 200, body = EmailSent))
)]
pub async fn change_email(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<EmailChange>,
) -> ApiResult<Json<EmailSent>> {
    ctx.require_recent_sign_in()?;
    let user = ctx.user_id()?;
    let email = validate_email("email", &req.email)?;
    throttle_ip(&state, &ctx, EMAILS_PER_IP).await?;
    throttle::hit(&state, EMAILS_PER_ADDRESS, &email)
        .await
        .map_err(|r| {
            refused(r, |_| {
                "Too many emails to this address; try again later.".to_owned()
            })
        })?;
    if users::email_taken(&state.pg, &email, Some(user.uuid())).await? {
        return Err(ApiError::Conflict(
            "Another account uses this address.".into(),
        ));
    }
    require_second_factor(&state, &ctx, user, &req.second_factor).await?;
    let current = users::get(&state.pg, user.uuid())
        .await?
        .and_then(|u| u.email);
    let token = random_token();
    let cancel = random_token();
    let mut tx = state.pg.begin().await?;
    retire_email_changes(&mut tx, user, ctx.now).await?;
    tokens::insert(
        &mut *tx,
        NewToken {
            kind: TokenKind::EmailChange,
            hash: &secret::hash(&token),
            user_id: Some(user.uuid()),
            email: Some(&email),
            data: serde_json::json!({ "session": ctx.session()?.id }),
            expires_at: ctx.now + EMAIL_CHANGE_LIFETIME,
        },
    )
    .await?;
    tokens::insert(
        &mut *tx,
        NewToken {
            kind: TokenKind::EmailChangeCancel,
            hash: &secret::hash(&cancel),
            user_id: Some(user.uuid()),
            email: Some(&email),
            data: serde_json::json!({}),
            expires_at: ctx.now + EMAIL_CHANGE_LIFETIME,
        },
    )
    .await?;
    ctx.audit(None, "user.email_change_request")
        .target("user", user)
        .after(serde_json::json!({ "email": email }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    let url = format!("{}me/email/confirm#token={token}", state.config.public_url);
    let lang =
        crate::services::mail_lang::of_user(&state.pg, user.uuid(), ctx.accept_language.as_deref())
            .await;
    state
        .mailer
        .send(kaeso_mail::templates::email_change_confirmation(
            lang, &email, &url,
        ));
    if let Some(current) = current {
        let url = format!("{}me/email/cancel#token={cancel}", state.config.public_url);
        state
            .mailer
            .send(kaeso_mail::templates::email_change_requested(
                lang, &current, &email, &url,
            ));
    }
    Ok(Json(EmailSent {
        status: "email_sent",
        message: format!("Open the link we sent to {email} to confirm the change."),
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "The confirmation token.")]
pub struct EmailConfirm {
    #[doc = "The token from `…/me/email/confirm#token=…`."]
    #[schema(value_type = String, write_only)]
    pub token: SecretString,
}

#[utoipa::path(
    post,
    path = "/me/email/confirm",
    summary = "Confirms a new address with the link sent to it, and tells the old\n\
    address. Works without a session, so the link can be opened anywhere.",
    tag = "me",
    request_body = EmailConfirm,
    responses((status = 200, body = UserView))
)]
pub async fn confirm_email(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<EmailConfirm>,
) -> ApiResult<Json<UserView>> {
    throttle_ip(&state, &ctx, TOKENS_PER_IP).await?;
    let mut tx = state.pg.begin().await?;
    let token = tokens::consume(
        &mut *tx,
        TokenKind::EmailChange,
        &secret::hash(req.token.expose_secret()),
        ctx.now,
    )
    .await?
    .ok_or_else(|| {
        ApiError::Authentication("This confirmation link is invalid, used or expired.".into())
    })?;
    let (Some(user_id), Some(new_email)) = (token.user_id, token.email) else {
        return Err(ApiError::internal(
            "email change token without user or address",
        ));
    };
    let asked_by = token
        .data
        .get("session")
        .and_then(|s| s.as_str())
        .and_then(|s| s.parse::<uuid::Uuid>().ok());
    let live = match asked_by {
        Some(session) => kaeso_store::pg::sessions::is_live(&mut *tx, session, ctx.now).await?,
        None => false,
    };
    if !live {
        return Err(ApiError::Authentication(
            "This confirmation link is invalid, used or expired.".into(),
        ));
    }
    let user = users::get(&mut *tx, user_id)
        .await?
        .filter(|u| u.state == "active")
        .ok_or_else(|| ApiError::PermissionDenied("This account is deactivated.".into()))?;
    if !users::set_email(&mut *tx, user_id, &new_email, ctx.now).await? {
        return Err(ApiError::Conflict(
            "Another account uses this address now.".into(),
        ));
    }
    for kind in [
        TokenKind::MagicLink,
        TokenKind::PasswordReset,
        TokenKind::EmailChangeCancel,
    ] {
        tokens::retire_all(&mut *tx, kind, Some(user_id), ctx.now).await?;
    }
    let user_id = UserId::from_uuid(user_id);
    ctx.audit(None, "user.email_change")
        .by(user_id)
        .target("user", user_id)
        .before(serde_json::json!({ "email": user.email }))
        .after(serde_json::json!({ "email": new_email }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    if let Some(old) = &user.email {
        let lang =
            crate::services::mail_lang::of_user(&state.pg, user.id, ctx.accept_language.as_deref())
                .await;
        state
            .mailer
            .send(kaeso_mail::templates::email_changed(lang, old, &new_email));
    }
    let row = users::get(&state.pg, user_id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("user"))?;
    Ok(Json(UserView::from_row(&row)))
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "The cancel token.")]
pub struct EmailCancel {
    #[doc = "The token from `…/me/email/cancel#token=…`, sent to the current address."]
    #[schema(value_type = String, write_only)]
    pub token: SecretString,
}

pub(crate) async fn retire_email_changes(
    db: &mut sqlx::PgConnection,
    user: UserId,
    now: time::OffsetDateTime,
) -> ApiResult<()> {
    for kind in [TokenKind::EmailChange, TokenKind::EmailChangeCancel] {
        tokens::retire_all(&mut *db, kind, Some(user.uuid()), now).await?;
    }
    Ok(())
}

#[utoipa::path(
    post,
    path = "/me/email/cancel",
    summary = "Cancels a pending email change with the link sent to the current\n\
    address. Works without a session.",
    tag = "me",
    request_body = EmailCancel,
    responses((status = 204, description = "Cancelled; the address stays as it is."))
)]
pub async fn cancel_email(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<EmailCancel>,
) -> ApiResult<axum::http::StatusCode> {
    throttle_ip(&state, &ctx, TOKENS_PER_IP).await?;
    let mut tx = state.pg.begin().await?;
    let token = tokens::consume(
        &mut *tx,
        TokenKind::EmailChangeCancel,
        &secret::hash(req.token.expose_secret()),
        ctx.now,
    )
    .await?
    .ok_or_else(|| {
        ApiError::Authentication(
            "This link is invalid, used or expired; the change may already be done.".into(),
        )
    })?;
    let user = token
        .user_id
        .map(UserId::from_uuid)
        .ok_or_else(|| ApiError::internal("email change cancel token without user"))?;
    retire_email_changes(&mut tx, user, ctx.now).await?;
    ctx.audit(None, "user.email_change_cancel")
        .by(user)
        .target("user", user)
        .before(serde_json::json!({ "email": token.email }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[utoipa::path(
    put,
    path = "/me/avatar",
    summary = "Sets the user's picture: a square PNG, JPEG or WebP image of at most 512 KiB.",
    tag = "me",
    request_body = crate::platform::avatars::AvatarUpload,
    responses((status = 200, body = Me))
)]
pub async fn set_avatar(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<crate::platform::avatars::AvatarUpload>,
) -> ApiResult<Json<Me>> {
    let user = ctx.user_id()?;
    let picture = crate::platform::avatars::check(&req)?;
    let (row, _) = load(&state, user).await?;
    let mut tx = state.pg.begin().await?;
    avatars::put(&mut *tx, &picture.hash, picture.content_type, &picture.data).await?;
    users::set_avatar(&mut *tx, user.uuid(), Some(&picture.hash)).await?;
    if let Some(old) = row.avatar.as_deref().filter(|old| *old != picture.hash) {
        avatars::delete_if_unused(&mut *tx, old).await?;
    }
    ctx.audit(None, "user.update")
        .target("user", user)
        .before(serde_json::json!({ "avatar": row.avatar }))
        .after(serde_json::json!({ "avatar": picture.hash }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(Json(load(&state, user).await?.1))
}

#[utoipa::path(
    delete,
    path = "/me/avatar",
    summary = "Removes the user's picture; their initials show again.",
    tag = "me",
    responses((status = 200, body = Me))
)]
pub async fn remove_avatar(State(state): State<AppState>, ctx: Ctx) -> ApiResult<Json<Me>> {
    let user = ctx.user_id()?;
    let (row, _) = load(&state, user).await?;
    if let Some(old) = row.avatar.as_deref() {
        let mut tx = state.pg.begin().await?;
        users::set_avatar(&mut *tx, user.uuid(), None).await?;
        avatars::delete_if_unused(&mut *tx, old).await?;
        ctx.audit(None, "user.update")
            .target("user", user)
            .before(serde_json::json!({ "avatar": old }))
            .after(serde_json::json!({ "avatar": null }))
            .write(&mut tx)
            .await?;
        tx.commit().await?;
    }
    Ok(Json(load(&state, user).await?.1))
}
