//! An organization's webhooks: where its operational notices go, signed.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::id::{OrgId, ProjectId, WebhookId, WebhookMessageId};
use kaeso_core::notifications::Kind;
use kaeso_store::pg::webhooks as store;
use secrecy::ExposeSecret;
use serde::{Deserialize, Deserializer, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::platform::access::Ctx;
use crate::platform::audit::Action;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::platform::keys::creator_of;
use crate::platform::pagination::List;
use crate::platform::params::{Path, id_param};
use crate::services::webhooks as service;
use crate::state::AppState;

/// The deliveries a webhook's list shows, newest first.
const SHOWN_DELIVERIES: i64 = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(
    description = "What a webhook is sent: `json`, Kaeso's event, or `slack`, `{\"text\": …}` \
    as Slack's incoming webhooks and compatible receivers take it."
)]
pub enum WebhookFormat {
    Json,
    Slack,
}

impl WebhookFormat {
    fn as_str(self) -> &'static str {
        match self {
            WebhookFormat::Json => "json",
            WebhookFormat::Slack => "slack",
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A webhook, without its secret.")]
pub struct WebhookView {
    #[doc = "Its id."]
    pub id: WebhookId,
    #[doc = "A name to recognise it by."]
    #[schema(example = "On-call")]
    pub name: String,
    #[doc = "Where events are sent."]
    #[schema(example = "https://hooks.example.com/kaeso")]
    pub url: String,
    #[doc = "What it is sent: `json` or `slack`."]
    pub format: WebhookFormat,
    #[doc = "The notices it receives, by the ids of the notification catalogue."]
    #[schema(example = json!(["budget_alerts", "provider_keys"]))]
    pub kinds: Vec<String>,
    #[doc = "The one project whose events it receives; `null` for the whole organization."]
    pub project: Option<ProjectId>,
    #[doc = "Whether events are sent to it."]
    pub enabled: bool,
    #[doc = "`failures` when Kaeso turned it off after 20 failed tries in a row; `null` otherwise."]
    pub disabled_reason: Option<String>,
    #[doc = "Failed tries since the last delivery."]
    pub consecutive_failures: i32,
    #[doc = "The receiver's status at the last try; `null` before any or when it couldn't be reached."]
    pub last_status: Option<i32>,
    #[doc = "Why the last try failed; `null` when it worked."]
    pub last_error: Option<String>,
    #[doc = "When it was last sent an event; `null` before the first."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_attempt_at: Option<OffsetDateTime>,
    #[doc = "When it was added."]
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[doc = "When it last changed."]
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

fn format_of(row: &store::WebhookRow) -> WebhookFormat {
    if row.format == "slack" {
        WebhookFormat::Slack
    } else {
        WebhookFormat::Json
    }
}

fn view(row: &store::WebhookRow) -> WebhookView {
    WebhookView {
        id: WebhookId::from_uuid(row.id),
        name: row.name.clone(),
        url: row.url.clone(),
        format: format_of(row),
        kinds: row.kinds.clone(),
        project: row.project_id.map(ProjectId::from_uuid),
        enabled: row.enabled,
        disabled_reason: row.disabled_reason.clone(),
        consecutive_failures: row.consecutive_failures,
        last_status: row.last_status,
        last_error: row.last_error.clone(),
        last_attempt_at: row.last_attempt_at,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

#[derive(Serialize, ToSchema)]
#[schema(description = "A webhook with its signing secret, shown once.")]
pub struct CreatedWebhook {
    #[serde(flatten)]
    pub webhook: WebhookView,
    #[doc = "The secret that signs every delivery, as Standard Webhooks writes it. Shown only now."]
    #[schema(example = "whsec_MfKQ9r8GKYqrTwjUPD8ILPZIo2LaLaSw")]
    pub secret: String,
}

impl std::fmt::Debug for CreatedWebhook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreatedWebhook")
            .field("webhook", &self.webhook)
            .field("secret", &"[redacted]")
            .finish()
    }
}

#[derive(Serialize, ToSchema)]
#[schema(description = "A webhook's new signing secret, shown once.")]
pub struct WebhookSecret {
    #[doc = "The new secret, as Standard Webhooks writes it. Shown only now."]
    #[schema(example = "whsec_MfKQ9r8GKYqrTwjUPD8ILPZIo2LaLaSw")]
    pub secret: String,
}

impl std::fmt::Debug for WebhookSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("WebhookSecret([redacted])")
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "One event sent, or to be sent, to a webhook.")]
pub struct WebhookDeliveryView {
    #[doc = "The event's id, the `webhook-id` header the receiver gets in every try."]
    pub id: WebhookMessageId,
    #[doc = "The event's type, such as `budget.alert`."]
    #[serde(rename = "type")]
    #[schema(example = "budget.alert")]
    pub event_type: String,
    #[doc = "`pending` while tries are left, `delivered`, or `failed` after the last try."]
    #[schema(example = "delivered")]
    pub status: String,
    #[doc = "How many times it was sent."]
    pub attempts: i32,
    #[doc = "The receiver's status at the last try."]
    pub last_status: Option<i32>,
    #[doc = "Why the last try failed."]
    pub last_error: Option<String>,
    #[doc = "When it is sent again; `null` unless it is pending."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub next_attempt_at: Option<OffsetDateTime>,
    #[doc = "When the event happened."]
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[doc = "When it was delivered or failed for good; `null` while pending."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub finished_at: Option<OffsetDateTime>,
}

fn delivery(d: &store::DeliveryRow) -> WebhookDeliveryView {
    WebhookDeliveryView {
        id: WebhookMessageId::from_uuid(d.message_id),
        event_type: d.event_type.clone(),
        status: d.status.clone(),
        attempts: d.attempts,
        last_status: d.last_status,
        last_error: d.last_error.clone(),
        next_attempt_at: (d.status == "pending").then_some(d.next_attempt_at),
        created_at: d.created_at,
        finished_at: d.finished_at,
    }
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "A new webhook.")]
pub struct WebhookCreate {
    #[doc = "A name to recognise it by, 1 to 100 characters."]
    #[schema(example = "On-call")]
    pub name: String,
    #[doc = "Where to send events: `https`, or `http` for a network the instance allows."]
    #[schema(example = "https://hooks.example.com/kaeso")]
    pub url: String,
    #[doc = "`json` (the default) or `slack`."]
    #[serde(default)]
    pub format: Option<WebhookFormat>,
    #[doc = "The notices to receive: `budget_alerts`, `limit_reached`, `key_expiry`,\n\
        `model_deprecated`, `model_shutdown_soon`, `model_lines`, `content_logging`,\n\
        `mcp_servers`, `org_security`, `org_members`, `provider_keys`, `model_changes`."]
    #[schema(example = json!(["budget_alerts", "provider_keys"]))]
    pub kinds: Vec<String>,
    #[doc = "Only this project's events; absent or `null` for the whole organization."]
    #[serde(default)]
    pub project: Option<ProjectId>,
}

fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(d).map(Some)
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "Changes to a webhook; absent fields stay as they are.")]
pub struct WebhookUpdate {
    #[doc = "A new name."]
    pub name: Option<String>,
    #[doc = "A new address; checked as when it was added."]
    pub url: Option<String>,
    #[doc = "A new format."]
    pub format: Option<WebhookFormat>,
    #[doc = "The notices to receive, replacing the list."]
    pub kinds: Option<Vec<String>>,
    #[doc = "Only this project's events; `null` for the whole organization."]
    #[serde(default, deserialize_with = "present")]
    #[schema(value_type = Option<ProjectId>, nullable)]
    pub project: Option<Option<ProjectId>>,
    #[doc = "Turns it on or off; turning it on clears its failures."]
    pub enabled: Option<bool>,
}

fn webhook_param(raw: &str) -> ApiResult<Uuid> {
    id_param::<WebhookId>(raw, "webhook").map(|id| id.uuid())
}

async fn row(state: &AppState, ctx: &Ctx, raw: &str) -> ApiResult<store::WebhookRow> {
    store::get(&state.pg, ctx.org()?.org.id, webhook_param(raw)?)
        .await?
        .ok_or_else(|| ApiError::not_found("webhook"))
}

fn checked_name(raw: &str) -> ApiResult<String> {
    crate::services::mcp_servers::clean_name(raw).ok_or_else(|| {
        ApiError::invalid(
            "name",
            "The name must be 1 to 100 characters, without control characters.",
        )
    })
}

async fn checked_url(state: &AppState, raw: &str) -> ApiResult<String> {
    use crate::services::mcp_servers::UrlProblem;
    let url = crate::services::mcp_servers::parse_url(raw).map_err(|p| {
        ApiError::invalid(
            "url",
            match p {
                UrlProblem::Length => "`url` must be a URL of at most 2048 characters.",
                UrlProblem::Scheme => {
                    "`url` must start with `https://`, or `http://` for an allowed network."
                }
                UrlProblem::Host | UrlProblem::Credentials | UrlProblem::Fragment => {
                    "`url` must be a full URL with a host, without a user or a fragment."
                }
            },
        )
    })?;
    if let Some(why) = service::refusal(state, &url).await {
        return Err(ApiError::invalid("url", why));
    }
    Ok(url.to_string())
}

fn checked_kinds(raw: &[String]) -> ApiResult<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for k in raw {
        let kind: Option<Kind> = k.parse().ok();
        if !kind.is_some_and(service::takes) {
            return Err(ApiError::invalid(
                "kinds",
                format!("`{k}` is no notice a webhook can receive."),
            ));
        }
        if !out.contains(k) {
            out.push(k.clone());
        }
    }
    if out.is_empty() {
        return Err(ApiError::invalid(
            "kinds",
            "Choose at least one notice for the webhook to receive.",
        ));
    }
    Ok(out)
}

async fn checked_project(state: &AppState, org: Uuid, p: ProjectId) -> ApiResult<Uuid> {
    let known = kaeso_store::pg::projects::ids_of_org(&state.pg, org).await?;
    if known.contains(&p.uuid()) {
        Ok(p.uuid())
    } else {
        Err(ApiError::invalid(
            "project",
            format!("`{p}` is not a project of this organization."),
        ))
    }
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/webhooks",
    summary = "The organization's webhooks, oldest first.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    responses((status = 200, body = List<WebhookView>))
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
) -> ApiResult<Json<List<WebhookView>>> {
    let rows = store::list(&state.pg, ctx.org()?.org.id).await?;
    Ok(Json(List::complete(rows.iter().map(view).collect())))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/webhooks/{webhook}",
    summary = "One webhook.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("webhook" = WebhookId, Path)),
    responses((status = 200, body = WebhookView))
)]
pub async fn get(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, id)): Path<(String, String)>,
) -> ApiResult<Json<WebhookView>> {
    Ok(Json(view(&row(&state, &ctx, &id).await?)))
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/webhooks",
    summary = "Adds a webhook. Its signing secret is in the answer, and only there.",
    description = "An organization has at most 10 webhooks. The URL must reach a public\n\
    `https` address, or a network the instance allows (`KAESO_WEBHOOKS_ALLOWED_NETWORKS`).",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    request_body = WebhookCreate,
    responses((status = 201, body = CreatedWebhook))
)]
pub async fn create(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
    Body(req): Body<WebhookCreate>,
) -> ApiResult<(StatusCode, Json<CreatedWebhook>)> {
    let scope = ctx.org()?;
    let org = scope.org.id;
    let name = checked_name(&req.name)?;
    let url = checked_url(&state, &req.url).await?;
    let kinds = checked_kinds(&req.kinds)?;
    let project = match req.project {
        Some(p) => Some(checked_project(&state, org, p).await?),
        None => None,
    };
    let format = req.format.unwrap_or(WebhookFormat::Json);
    let id = Uuid::now_v7();
    let secret = service::new_secret();
    let sealed = service::seal(&state, id, &secret)?;
    let (kind, by) = creator_of(&ctx);
    let mut tx = state.pg.begin().await?;
    store::lock(&mut *tx, org).await?;
    if store::count(&mut *tx, org).await? >= store::MAX_PER_ORG {
        return Err(ApiError::Conflict(
            "An organization has at most 10 webhooks; delete one first.".into(),
        ));
    }
    store::insert(
        &mut *tx,
        &store::NewWebhook {
            id,
            org_id: org,
            project_id: project,
            name: &name,
            url: &url,
            format: format.as_str(),
            kinds: &kinds,
            secret: store::Secret {
                ciphertext: &sealed.ciphertext,
                nonce: &sealed.nonce,
                wrapped_key: &sealed.wrapped_key,
                key_id: &sealed.key_id,
            },
            created_by_kind: kind,
            created_by_id: by,
            at: ctx.now,
        },
    )
    .await?;
    let row = store::get(&mut *tx, org, id)
        .await?
        .ok_or_else(|| ApiError::internal("new webhook vanished"))?;
    let shown = view(&row);
    ctx.audit(Some(scope.id()), Action::WebhookCreate)
        .target("webhook", WebhookId::from_uuid(id))
        .after(&shown)
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(CreatedWebhook {
            webhook: shown,
            secret: secret.expose_secret().to_owned(),
        }),
    ))
}

#[utoipa::path(
    patch,
    path = "/orgs/{org}/webhooks/{webhook}",
    summary = "Changes a webhook, or turns it on or off.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("webhook" = WebhookId, Path)),
    request_body = WebhookUpdate,
    responses((status = 200, body = WebhookView))
)]
pub async fn update(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, id)): Path<(String, String)>,
    Body(req): Body<WebhookUpdate>,
) -> ApiResult<Json<WebhookView>> {
    let scope = ctx.org()?;
    let org = scope.org.id;
    let before = row(&state, &ctx, &id).await?;
    let name = req.name.as_deref().map(checked_name).transpose()?;
    let url = match &req.url {
        Some(u) => Some(checked_url(&state, u).await?),
        None => None,
    };
    let kinds = req.kinds.as_deref().map(checked_kinds).transpose()?;
    let project = match req.project {
        Some(Some(p)) => Some(Some(checked_project(&state, org, p).await?)),
        Some(None) => Some(None),
        None => None,
    };
    let mut tx = state.pg.begin().await?;
    let changed = store::update(
        &mut *tx,
        org,
        before.id,
        &store::Change {
            name: name.as_deref(),
            url: url.as_deref(),
            format: req.format.map(WebhookFormat::as_str),
            kinds: kinds.as_deref(),
            project_id: project,
            enabled: req.enabled,
        },
        ctx.now,
    )
    .await?;
    if !changed {
        return Err(ApiError::not_found("webhook"));
    }
    let after = store::get(&mut *tx, org, before.id)
        .await?
        .ok_or_else(|| ApiError::not_found("webhook"))?;
    let shown = view(&after);
    ctx.audit(Some(scope.id()), Action::WebhookUpdate)
        .target("webhook", WebhookId::from_uuid(before.id))
        .before(view(&before))
        .after(&shown)
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(Json(shown))
}

#[utoipa::path(
    delete,
    path = "/orgs/{org}/webhooks/{webhook}",
    summary = "Deletes a webhook with its deliveries; events no longer go to it.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("webhook" = WebhookId, Path)),
    responses((status = 204))
)]
pub async fn delete(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, id)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    let scope = ctx.org()?;
    let before = row(&state, &ctx, &id).await?;
    let mut tx = state.pg.begin().await?;
    if !store::delete(&mut *tx, scope.org.id, before.id).await? {
        return Err(ApiError::not_found("webhook"));
    }
    ctx.audit(Some(scope.id()), Action::WebhookDelete)
        .target("webhook", WebhookId::from_uuid(before.id))
        .before(view(&before))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/webhooks/{webhook}/secret",
    summary = "Replaces a webhook's signing secret; the answer holds the new one, shown once.",
    description = "Deliveries are signed with the new secret from now on, tries of earlier\n\
    events included.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("webhook" = WebhookId, Path)),
    responses((status = 200, body = WebhookSecret))
)]
pub async fn rotate(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, id)): Path<(String, String)>,
) -> ApiResult<Json<WebhookSecret>> {
    let scope = ctx.org()?;
    let row = row(&state, &ctx, &id).await?;
    let secret = service::new_secret();
    let sealed = service::seal(&state, row.id, &secret)?;
    let mut tx = state.pg.begin().await?;
    let set = store::set_secret(
        &mut *tx,
        scope.org.id,
        row.id,
        store::Secret {
            ciphertext: &sealed.ciphertext,
            nonce: &sealed.nonce,
            wrapped_key: &sealed.wrapped_key,
            key_id: &sealed.key_id,
        },
        ctx.now,
    )
    .await?;
    if !set {
        return Err(ApiError::not_found("webhook"));
    }
    ctx.audit(Some(scope.id()), Action::WebhookSecretRotate)
        .target("webhook", WebhookId::from_uuid(row.id))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(Json(WebhookSecret {
        secret: secret.expose_secret().to_owned(),
    }))
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/webhooks/{webhook}/test",
    summary = "Sends the webhook a `webhook.test` event now, once, and says how it went.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("webhook" = WebhookId, Path)),
    responses((status = 200, body = WebhookDeliveryView))
)]
pub async fn test(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, id)): Path<(String, String)>,
) -> ApiResult<Json<WebhookDeliveryView>> {
    let row = row(&state, &ctx, &id).await?;
    if !row.enabled {
        return Err(ApiError::Conflict(
            "The webhook is off; turn it on to send it a test event.".into(),
        ));
    }
    let queued = service::queue_test(&state, &row).await?;
    service::send_now(&state, queued)
        .await
        .map_err(|e| ApiError::internal(format!("the test event couldn't be sent: {e}")))?;
    let sent = store::deliveries(&state.pg, row.id, SHOWN_DELIVERIES)
        .await?
        .into_iter()
        .find(|d| d.id == queued)
        .ok_or_else(|| ApiError::internal("the test delivery vanished"))?;
    Ok(Json(delivery(&sent)))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/webhooks/{webhook}/deliveries",
    summary = "A webhook's last 50 deliveries, newest first; deliveries are kept 7 days.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("webhook" = WebhookId, Path)),
    responses((status = 200, body = List<WebhookDeliveryView>))
)]
pub async fn deliveries(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, id)): Path<(String, String)>,
) -> ApiResult<Json<List<WebhookDeliveryView>>> {
    let row = row(&state, &ctx, &id).await?;
    let rows = store::deliveries(&state.pg, row.id, SHOWN_DELIVERIES).await?;
    Ok(Json(List::complete(rows.iter().map(delivery).collect())))
}
