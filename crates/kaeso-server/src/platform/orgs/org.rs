use axum::Json;
use axum::extract::State;
use kaeso_core::currency;
use kaeso_core::id::{LimitId, OrgId, ProjectId, UserId};
use kaeso_core::limits::Kind;
use kaeso_core::rights::{OrgGrant, OrgPreset};
use kaeso_store::pg::limits as limits_store;
use kaeso_store::pg::{avatars, members, orgs, users};
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use time::OffsetDateTime;
use utoipa::ToSchema;

use crate::platform::access::Ctx;
use crate::platform::audit::Action;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::grants::org_to_store;
use crate::platform::json::Body;
use crate::platform::params::Path;
use crate::services::exchange_rates as rates;
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "An organization.")]
pub struct OrgView {
    #[doc = "Id."]
    pub id: OrgId,
    #[doc = "Name."]
    #[schema(example = "Acme")]
    pub name: String,
    #[doc = "`active` or `scheduled_for_deletion`."]
    #[schema(example = "active")]
    pub state: String,
    #[doc = "When a scheduled deletion becomes final."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub deletion_due_at: Option<OffsetDateTime>,
    #[doc = "Whether request and reply bodies are logged."]
    pub content_logging: bool,
    #[doc = "How long log metadata is kept, in days."]
    pub metadata_retention_days: i32,
    #[doc = "How long log content is kept, in days."]
    pub content_retention_days: i32,
    #[doc = "Whether members need two-factor authentication."]
    pub require_2fa: bool,
    #[doc = "Whether the accounts the org manages sign in only through its SSO,\n\
        everywhere on the install; guests keep their own sign-in."]
    pub require_sso: bool,
    #[doc = "The longest lifetime of new API keys, in days; `null` for no limit."]
    pub max_key_lifetime_days: Option<i32>,
    #[doc = "The org preset of SSO users who arrive without an invite; `null`: invite only."]
    pub sso_default_preset: Option<String>,
    #[doc = "The picture's address, under `me/avatars`; `null` without one."]
    #[schema(
        example = "/platform/v1/me/avatars/9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"
    )]
    pub avatar_url: Option<String>,
    #[doc = "The currency the org reads and enters money in, an ISO 4217 code: spend,\n\
        prices and spend budgets. Kaeso stores and costs in USD and converts at the\n\
        ECB's daily rates."]
    #[schema(example = "EUR")]
    pub currency: String,

    #[doc = "Created."]
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

impl OrgView {
    pub fn from_row(o: &orgs::Org) -> OrgView {
        OrgView {
            id: OrgId::from_uuid(o.id),
            name: o.name.clone(),
            state: o.state.clone(),
            deletion_due_at: o.deletion_due_at,
            content_logging: o.content_logging,
            metadata_retention_days: o.metadata_retention_days,
            content_retention_days: o.content_retention_days,
            require_2fa: o.require_2fa,
            require_sso: o.require_sso,
            max_key_lifetime_days: o.max_key_lifetime_days,
            sso_default_preset: o.sso_default_preset.clone(),
            avatar_url: crate::platform::views::avatar_url(o.avatar.as_deref()),
            currency: o.currency.clone(),
            created_at: o.created_at,
        }
    }
}

#[utoipa::path(
    get,
    path = "/orgs/{org}",
    summary = "The org and its settings.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    responses((status = 200, body = OrgView))
)]
pub async fn get(ctx: Ctx, Path(_org): Path<String>) -> ApiResult<Json<OrgView>> {
    Ok(Json(OrgView::from_row(&ctx.org()?.org)))
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "General settings.")]
pub struct OrgUpdate {
    #[doc = "A new name, 1 to 100 characters."]
    #[schema(example = "Acme Inc.")]
    pub name: Option<String>,
    #[doc = "A new currency: `USD` or a code from `me/exchange-rates`. The spend\n\
        budgets of the org's projects change to it at the latest ECB rates."]
    #[schema(example = "EUR")]
    pub currency: Option<String>,
}

pub(crate) async fn lock_changeable(tx: &mut PgConnection, ctx: &Ctx) -> ApiResult<orgs::Org> {
    let row = orgs::get_for_update(&mut *tx, ctx.org()?.org.id)
        .await?
        .ok_or_else(|| ApiError::not_found("organization"))?;
    if row.state == "scheduled_for_deletion" {
        return Err(ApiError::OrgScheduledForDeletion);
    }
    Ok(row)
}

async fn save(
    tx: &mut PgConnection,
    ctx: &Ctx,
    before: &orgs::Org,
    after: &orgs::Org,
) -> ApiResult<()> {
    orgs::update(&mut *tx, after).await?;
    let id = OrgId::from_uuid(after.id);
    ctx.audit(Some(id), Action::OrgUpdate)
        .target("org", id)
        .before(OrgView::from_row(before))
        .after(OrgView::from_row(after))
        .write(tx)
        .await
}

#[utoipa::path(
    patch,
    path = "/orgs/{org}",
    summary = "Renames the org or changes its currency.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    request_body = OrgUpdate,
    responses((status = 200, body = OrgView))
)]
pub async fn update(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
    Body(req): Body<OrgUpdate>,
) -> ApiResult<Json<OrgView>> {
    let name = req
        .name
        .map(|n| crate::platform::validate::validate_name("name", &n))
        .transpose()?;
    if let Some(code) = req.currency.as_deref()
        && !(currency::is_code(code) && rates::known(&state, code))
    {
        return Err(crate::platform::exchange_rates::unknown("currency", code));
    }
    let mut tx = state.pg.begin().await?;
    let before = lock_changeable(&mut tx, &ctx).await?;
    let mut org = before.clone();
    if let Some(name) = name {
        org.name = name;
    }
    if let Some(code) = req.currency {
        org.currency = code;
    }
    save(&mut tx, &ctx, &before, &org).await?;
    let changed = if org.currency == before.currency {
        Vec::new()
    } else {
        convert_budgets(&state, &ctx, &mut tx, org.id, &org.currency).await?
    };
    tx.commit().await?;
    for project in changed {
        crate::services::limits::forget(&state, project).await;
    }
    Ok(Json(OrgView::from_row(&org)))
}

// Spend budgets keep the org's currency, so each reads as it was entered; cents are enough for a
// budget, and a budget never rounds down to nothing.
async fn convert_budgets(
    state: &AppState,
    ctx: &Ctx,
    tx: &mut PgConnection,
    org: uuid::Uuid,
    code: &str,
) -> ApiResult<Vec<uuid::Uuid>> {
    let book = rates::book(state);
    let mut projects = Vec::new();
    for row in limits_store::list_org(&mut *tx, org).await? {
        let before = row.limit()?;
        if before.kind != Kind::SpendBudget || before.currency() == code {
            continue;
        }
        let Some(amount) = book
            .to_usd(before.amount, before.currency())
            .and_then(|usd| book.from_usd(usd, code))
        else {
            continue;
        };
        let mut after = before.clone();
        after.amount = amount
            .round_dp_with_strategy(2, RoundingStrategy::MidpointNearestEven)
            .max(Decimal::new(1, 2))
            .normalize();
        after.currency = Some(code.to_owned());
        limits_store::update(&mut *tx, row.project_id, &after).await?;
        ctx.audit(Some(OrgId::from_uuid(org)), Action::LimitUpdate)
            .project(ProjectId::from_uuid(row.project_id))
            .target("limit", LimitId::from_uuid(after.id))
            .before(serde_json::json!({ "description": before.describe() }))
            .after(serde_json::json!({ "description": after.describe() }))
            .write(&mut *tx)
            .await?;
        if !projects.contains(&row.project_id) {
            projects.push(row.project_id);
        }
    }
    Ok(projects)
}

#[utoipa::path(
    put,
    path = "/orgs/{org}/avatar",
    summary = "Sets the org's picture: a square PNG, JPEG or WebP image of at most 512 KiB.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    request_body = crate::platform::avatars::AvatarUpload,
    responses((status = 200, body = OrgView))
)]
pub async fn set_avatar(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
    Body(req): Body<crate::platform::avatars::AvatarUpload>,
) -> ApiResult<Json<OrgView>> {
    let picture = crate::platform::avatars::check(&req)?;
    let mut tx = state.pg.begin().await?;
    let before = lock_changeable(&mut tx, &ctx).await?;
    let mut org = before.clone();
    org.avatar = Some(picture.hash.clone());
    avatars::put(&mut *tx, &picture.hash, picture.content_type, &picture.data).await?;
    save(&mut tx, &ctx, &before, &org).await?;
    if let Some(old) = before.avatar.as_deref().filter(|old| *old != picture.hash) {
        avatars::delete_if_unused(&mut *tx, old).await?;
    }
    tx.commit().await?;
    Ok(Json(OrgView::from_row(&org)))
}

#[utoipa::path(
    delete,
    path = "/orgs/{org}/avatar",
    summary = "Removes the org's picture; its initials show again.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    responses((status = 200, body = OrgView))
)]
pub async fn remove_avatar(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
) -> ApiResult<Json<OrgView>> {
    let mut tx = state.pg.begin().await?;
    let before = lock_changeable(&mut tx, &ctx).await?;
    let Some(old) = before.avatar.clone() else {
        return Ok(Json(OrgView::from_row(&before)));
    };
    let mut org = before.clone();
    org.avatar = None;
    save(&mut tx, &ctx, &before, &org).await?;
    avatars::delete_if_unused(&mut *tx, &old).await?;
    tx.commit().await?;
    Ok(Json(OrgView::from_row(&org)))
}

#[utoipa::path(
    delete,
    path = "/orgs/{org}",
    summary = "Schedules the org for deletion: every key stops\n\
    working at once, members and instance admins are told, and the org stays\n\
    read-only for the instance's grace period, during which an owner or an\n\
    instance admin can restore it. Refused when the instance doesn't allow\n\
    deleting orgs.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    responses((status = 200, body = OrgView))
)]
pub async fn delete(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
) -> ApiResult<Json<OrgView>> {
    let scope = ctx.org()?;
    let mut tx = state.pg.begin().await?;
    let (allowed, grace) = crate::services::orgs::deletion_policy(&mut *tx).await?;
    crate::services::orgs::ensure_deletion_allowed(allowed)?;
    let mut org = lock_changeable(&mut tx, &ctx).await?;
    org.state = "scheduled_for_deletion".into();
    org.deletion_due_at = Some(ctx.now + grace);
    orgs::update(&mut *tx, &org).await?;
    ctx.audit(Some(scope.id()), Action::OrgScheduleDeletion)
        .target("org", scope.id())
        .after(serde_json::json!({ "deletion_due_at": org.deletion_due_at }))
        .write(&mut tx)
        .await?;
    ctx.audit(None, Action::OrgScheduleDeletion)
        .target("org", scope.id())
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    crate::services::keys::forget_org(&state, org.id).await;
    crate::services::orgs::notify_deletion(&state, &ctx, &org).await;
    Ok(Json(OrgView::from_row(&org)))
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/restore",
    summary = "Restores an org scheduled for deletion. Its keys work again.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    responses((status = 200, body = OrgView))
)]
pub async fn restore(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
) -> ApiResult<Json<OrgView>> {
    let scope = ctx.org()?;
    let org = crate::services::orgs::restore(&state, &ctx, scope.id()).await?;
    Ok(Json(OrgView::from_row(&org)))
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "Who becomes an owner.")]
pub struct OwnershipTransfer {
    #[doc = "A member of the org who becomes an owner. The caller becomes an admin."]
    pub user_id: UserId,
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/ownership-transfer",
    summary = "Makes another member an owner; the caller, an owner, becomes an admin.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    request_body = OwnershipTransfer,
    responses((status = 200, body = crate::platform::orgs::members::MemberView))
)]
pub async fn transfer(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
    Body(req): Body<OwnershipTransfer>,
) -> ApiResult<Json<crate::platform::orgs::members::MemberView>> {
    let scope = ctx.org()?;
    let me = ctx.user_id()?;
    if !scope.is_owner() {
        return Err(ApiError::PermissionDenied(
            "Only owners can make someone an owner or change an owner's role.".into(),
        ));
    }
    if req.user_id == me {
        return Err(ApiError::invalid("user_id", "Choose another member."));
    }
    let mut tx = state.pg.begin().await?;
    members::lock_owners(&mut tx, scope.org.id).await?;
    let caller = members::get_org(&mut *tx, scope.org.id, me.uuid())
        .await?
        .filter(|m| m.preset == "owner");
    if caller.is_none() {
        return Err(ApiError::PermissionDenied(
            "Only owners can make someone an owner or change an owner's role.".into(),
        ));
    }
    let target = members::get_org(&mut *tx, scope.org.id, req.user_id.uuid())
        .await?
        .filter(|m| m.user_state == "active")
        .ok_or_else(|| {
            ApiError::invalid(
                "user_id",
                "The new owner must be an active member of the organization.",
            )
        })?;
    if !members::update_org(
        &mut *tx,
        scope.org.id,
        target.user_id,
        &org_to_store(&OrgGrant::preset(OrgPreset::Owner)),
    )
    .await?
    {
        return Err(ApiError::invalid(
            "user_id",
            "The new owner must be an active member of the organization.",
        ));
    }
    members::update_org(
        &mut *tx,
        scope.org.id,
        me.uuid(),
        &org_to_store(&OrgGrant::preset(OrgPreset::Admin)),
    )
    .await?;
    ctx.audit(Some(scope.id()), Action::OrgOwnershipTransfer)
        .target("user", req.user_id)
        .before(serde_json::json!({ "preset": target.preset }))
        .after(serde_json::json!({ "preset": "owner", "previous_owner": me.to_string() }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    crate::services::keys::forget_user_in_org(&state, scope.org.id, target.user_id).await;
    crate::services::keys::forget_user_in_org(&state, scope.org.id, me.uuid()).await;
    if let Some(email) = &target.email {
        let name = users::get(&state.pg, me.uuid()).await?.and_then(|u| u.name);
        let by = name
            .as_ref()
            .map_or(kaeso_mail::Actor::Owner, kaeso_mail::Actor::from);
        let lang = crate::services::mail_lang::of_user(
            &state.pg,
            target.user_id,
            ctx.accept_language.as_deref(),
        )
        .await;
        let url = format!("{}orgs/{}", state.config.public_url, scope.id());
        state
            .mailer
            .send(kaeso_mail::templates::ownership_transferred(
                lang,
                email,
                &scope.org.name,
                by,
                &url,
            ));
    }
    let row = members::get_org(&state.pg, scope.org.id, req.user_id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("member"))?;
    let domains = super::members::domains(&state, scope.org.id).await?;
    Ok(Json(super::members::view(&row, &domains)?))
}
