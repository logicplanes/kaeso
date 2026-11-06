use std::collections::BTreeMap;

use axum::Json;
use axum::extract::State;
use kaeso_core::id::{OrgId, UserId};
use kaeso_core::notifications::{Area, Delivery, Kind};
use kaeso_store::pg::{members, notifications, users};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::auth::throttle::{self, Outage, Throttle};
use crate::config::MailTransport;
use crate::platform::access::{Ctx, agent_scope};
use crate::platform::audit::Action;
use crate::platform::auth::refused;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::instance::settings::TestEmailResult;
use crate::platform::json::Body;
use crate::platform::params::{Path, Query};
use crate::services::notifications::{self as service, Reason};
use crate::state::AppState;

const TEST_EMAILS_PER_USER: Throttle = Throttle {
    name: "test_email",
    limit: 1,
    window: std::time::Duration::from_secs(60),
    on_outage: Outage::Allow,
};
const INVALID_LINK: &str =
    "This unsubscribe link is invalid; choose your emails under Account › Notifications.";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(description = "When an email goes out: `now`, in the `daily` digest, or `off`.")]
pub enum NotificationDelivery {
    Now,
    Daily,
    Off,
}

impl From<Delivery> for NotificationDelivery {
    fn from(d: Delivery) -> NotificationDelivery {
        match d {
            Delivery::Now => NotificationDelivery::Now,
            Delivery::Daily => NotificationDelivery::Daily,
            Delivery::Off => NotificationDelivery::Off,
        }
    }
}

impl From<NotificationDelivery> for Delivery {
    fn from(d: NotificationDelivery) -> Delivery {
        match d {
            NotificationDelivery::Now => Delivery::Now,
            NotificationDelivery::Daily => Delivery::Daily,
            NotificationDelivery::Off => Delivery::Off,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "An email the user can receive.")]
pub struct NotificationKindView {
    #[doc = "The kind, such as `budget_alerts`."]
    pub kind: &'static str,
    #[doc = "Its name in English."]
    pub name: &'static str,
    #[doc = "What it is about, in English."]
    pub description: &'static str,
    #[doc = "A security email: always sent at once, even for a muted org, with no setting."]
    pub always: bool,
    #[doc = "The delivery without a choice of the user's."]
    pub default: NotificationDelivery,
    #[doc = "The delivery that applies."]
    pub delivery: NotificationDelivery,
    #[doc = "The right that makes the user responsible, such as `members.invite`; `null` for\n\
        an email about the user's own account, access or keys."]
    pub reason: Option<&'static str>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The emails of one area.")]
pub struct NotificationArea {
    #[doc = "`security`, `personal`, `project`, `tools`, `organization` or `instance`."]
    pub area: &'static str,
    #[doc = "Its name in English."]
    pub name: &'static str,
    #[doc = "Its emails the user can receive."]
    pub kinds: Vec<NotificationKindView>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The emails the user's rights let them receive, and when each goes out.")]
pub struct NotificationSettings {
    #[doc = "`false` when the install only writes emails to its log (`mail.transport` is\n\
        `log`), so none reach anyone."]
    pub sends_email: bool,
    #[doc = "Areas in page order; an area without an email the user can receive is left out."]
    pub areas: Vec<NotificationArea>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "Deliveries to change; kinds left out keep theirs.")]
pub struct NotificationsUpdate {
    #[doc = "Kind to delivery, such as `{\"org_members\": \"off\"}`."]
    pub kinds: BTreeMap<String, NotificationDelivery>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "Whether the user gets emails about an org.")]
pub struct MembershipNotifications {
    #[doc = "`true` stops every email about the org except security emails."]
    pub muted: bool,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct UnsubscribeQuery {
    #[doc = "The signed token from the email's unsubscribe link."]
    pub token: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The email an unsubscribe link stops.")]
pub struct Unsubscribe {
    #[doc = "The kind, such as `weekly_summary`."]
    pub kind: &'static str,
    #[doc = "Its name in English."]
    pub name: &'static str,
    #[doc = "The account's email address."]
    pub email: Option<String>,
    #[doc = "Its delivery now; `off` after `POST`."]
    pub delivery: NotificationDelivery,
}

async fn settings_of(state: &AppState, user: &users::User) -> ApiResult<NotificationSettings> {
    let held = service::held(state, user.id, user.is_instance_admin).await?;
    let chosen = service::deliveries(&notifications::settings(&state.pg, user.id).await?);
    let areas = Area::ALL
        .iter()
        .map(|area| NotificationArea {
            area: area.id(),
            name: area.name(),
            kinds: Kind::ALL
                .iter()
                .filter(|k| k.area() == *area)
                .filter_map(|k| {
                    let reason = held.reason(*k)?;
                    Some(NotificationKindView {
                        kind: k.as_str(),
                        name: k.name(),
                        description: k.description(),
                        always: k.always(),
                        default: k.default_delivery().into(),
                        delivery: chosen
                            .get(k)
                            .copied()
                            .unwrap_or(k.default_delivery())
                            .into(),
                        reason: match reason {
                            Reason::AboutYou => None,
                            Reason::Holds(r) => Some(r.as_str()),
                        },
                    })
                })
                .collect(),
        })
        .filter(|a: &NotificationArea| !a.kinds.is_empty())
        .collect();
    Ok(NotificationSettings {
        sends_email: state.config.mail.transport != MailTransport::Log,
        areas,
    })
}

async fn me(state: &AppState, ctx: &Ctx) -> ApiResult<users::User> {
    users::get(&state.pg, ctx.user_id()?.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("user"))
}

#[utoipa::path(
    get,
    path = "/me/notifications",
    summary = "The emails the user's rights let them receive, grouped by area, each with\n\
    the right that makes them responsible and when it goes out.",
    tag = "me",
    responses((status = 200, body = NotificationSettings))
)]
pub async fn get(State(state): State<AppState>, ctx: Ctx) -> ApiResult<Json<NotificationSettings>> {
    let user = me(&state, &ctx).await?;
    Ok(Json(settings_of(&state, &user).await?))
}

#[utoipa::path(
    put,
    path = "/me/notifications",
    summary = "Chooses when emails go out: now, in the daily digest or not at all.\n\
    Security emails have no setting.",
    tag = "me",
    request_body = NotificationsUpdate,
    responses((status = 200, body = NotificationSettings))
)]
pub async fn update(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<NotificationsUpdate>,
) -> ApiResult<Json<NotificationSettings>> {
    let user = me(&state, &ctx).await?;
    let mut changes = Vec::with_capacity(req.kinds.len());
    for (key, delivery) in &req.kinds {
        let param = format!("kinds.{key}");
        let kind: Kind = key
            .parse()
            .map_err(|_| ApiError::invalid(&param, format!("`{key}` is not an email kind.")))?;
        if kind.always() {
            return Err(ApiError::invalid(
                &param,
                "Security emails are always sent; they have no setting.",
            ));
        }
        changes.push((kind, Delivery::from(*delivery)));
    }
    let mut tx = state.pg.begin().await?;
    for (kind, delivery) in &changes {
        if *delivery == kind.default_delivery() {
            notifications::reset_delivery(&mut *tx, user.id, kind.as_str()).await?;
        } else {
            notifications::set_delivery(&mut *tx, user.id, kind.as_str(), delivery.as_str())
                .await?;
        }
    }
    let after: BTreeMap<&str, &str> = changes
        .iter()
        .map(|(k, d)| (k.as_str(), d.as_str()))
        .collect();
    ctx.audit(None, Action::UserNotifications)
        .target("user", UserId::from_uuid(user.id))
        .after(&after)
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(Json(settings_of(&state, &user).await?))
}

#[utoipa::path(
    put,
    path = "/me/memberships/{org}/notifications",
    summary = "Mutes or unmutes emails about an org. Security emails still arrive.",
    tag = "me",
    params(("org" = OrgId, Path)),
    request_body = MembershipNotifications,
    responses((status = 200, body = MembershipNotifications))
)]
pub async fn mute(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(org): Path<OrgId>,
    Body(req): Body<MembershipNotifications>,
) -> ApiResult<Json<MembershipNotifications>> {
    let me = ctx.user_id()?;
    if let Some(agent) = ctx.agent() {
        agent_scope(&state, agent, org, None, true)
            .await
            .map_err(|e| match e {
                ApiError::NotFound(_) => ApiError::not_found("membership"),
                e => e,
            })?;
    }
    members::get_org(&state.pg, org.uuid(), me.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("membership"))?;
    let mut tx = state.pg.begin().await?;
    notifications::set_muted(&mut *tx, me.uuid(), org.uuid(), req.muted).await?;
    ctx.audit(Some(org), Action::UserNotifications)
        .target("user", me)
        .after(serde_json::json!({ "muted": req.muted }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(Json(req))
}

#[utoipa::path(
    post,
    path = "/me/notifications/test",
    summary = "Sends a test email to the user's address, at most once a minute.",
    tag = "me",
    responses((status = 200, body = TestEmailResult))
)]
pub async fn test(State(state): State<AppState>, ctx: Ctx) -> ApiResult<Json<TestEmailResult>> {
    let user = me(&state, &ctx).await?;
    let to = user
        .email
        .clone()
        .ok_or_else(|| ApiError::Conflict("Your account has no email address.".into()))?;
    throttle::hit(&state, TEST_EMAILS_PER_USER, &user.id.to_string())
        .await
        .map_err(|r| {
            refused(r, |retry_after| {
                format!(
                    "You sent a test email a moment ago; try again in {}.",
                    throttle::wait(retry_after)
                )
            })
        })?;
    let lang =
        crate::services::mail_lang::of_user(&state.pg, user.id, ctx.accept_language.as_deref())
            .await;
    state
        .mailer
        .send_now(kaeso_mail::templates::test(lang, &to))
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "test email not sent");
            ApiError::Unavailable(
                "The email was not sent; ask an instance admin to check the mail settings.".into(),
            )
        })?;
    Ok(Json(TestEmailResult { status: "sent", to }))
}

async fn unsubscribing(state: &AppState, token: &str) -> ApiResult<(users::User, Kind, Delivery)> {
    let (user, kind) = service::verify_unsubscribe(state, token)
        .ok_or_else(|| ApiError::NotFound(INVALID_LINK.into()))?;
    let user = users::get(&state.pg, user.uuid())
        .await?
        .filter(|u| u.state != "deleted")
        .ok_or_else(|| ApiError::NotFound(INVALID_LINK.into()))?;
    let delivery = service::deliveries(&notifications::settings(&state.pg, user.id).await?)
        .get(&kind)
        .copied()
        .unwrap_or(kind.default_delivery());
    Ok((user, kind, delivery))
}

#[utoipa::path(
    get,
    path = "/notifications/unsubscribe",
    summary = "The email an unsubscribe link stops, without a session.",
    tag = "me",
    params(UnsubscribeQuery),
    responses((status = 200, body = Unsubscribe))
)]
pub async fn unsubscribe_link(
    State(state): State<AppState>,
    _ctx: Ctx,
    Query(q): Query<UnsubscribeQuery>,
) -> ApiResult<Json<Unsubscribe>> {
    let (user, kind, delivery) = unsubscribing(&state, &q.token).await?;
    Ok(Json(Unsubscribe {
        kind: kind.as_str(),
        name: kind.name(),
        email: user.email,
        delivery: delivery.into(),
    }))
}

#[utoipa::path(
    post,
    path = "/notifications/unsubscribe",
    summary = "Turns one kind of email off with the link from such an email, without a\n\
    session.",
    description = "Mail apps send `List-Unsubscribe=One-Click` here as RFC 8058 describes, \
    without `Origin`; the signed token is the authority, so the request needs neither a session \
    nor `Origin`. Any body is ignored.",
    tag = "me",
    params(UnsubscribeQuery),
    responses((status = 200, body = Unsubscribe))
)]
pub async fn unsubscribe(
    State(state): State<AppState>,
    ctx: Ctx,
    Query(q): Query<UnsubscribeQuery>,
) -> ApiResult<Json<Unsubscribe>> {
    let (user, kind, _) = unsubscribing(&state, &q.token).await?;
    let id = UserId::from_uuid(user.id);
    let mut tx = state.pg.begin().await?;
    if kind.default_delivery() == Delivery::Off {
        notifications::reset_delivery(&mut *tx, user.id, kind.as_str()).await?;
    } else {
        notifications::set_delivery(&mut *tx, user.id, kind.as_str(), Delivery::Off.as_str())
            .await?;
    }
    ctx.audit(None, Action::UserNotifications)
        .by(id)
        .target("user", id)
        .after(serde_json::json!({ kind.as_str(): "off", "via": "unsubscribe_link" }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(Json(Unsubscribe {
        kind: kind.as_str(),
        name: kind.name(),
        email: user.email,
        delivery: NotificationDelivery::Off,
    }))
}
