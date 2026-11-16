use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use bytes::Bytes;
use kaeso_core::id::{GuardrailId, OrgId, ProjectId, RequestId};
use kaeso_crypto::ExposeSecret;
use kaeso_store::pg::credentials::Sealed;
use kaeso_store::pg::guardrails::{self as store, Changes, GuardrailRow, NewGuardrail};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::platform::access::Ctx;
use crate::platform::audit::Action as Audit;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::platform::keys::{Party, creator, creator_of};
use crate::platform::pagination::List;
use crate::platform::params::{Path, id_param};
use crate::platform::precondition::{IfMatch, etag};
use crate::services::custom_providers::Reachability;
use crate::services::guardrails::contract::{self, Asked, Said, Verdict};
use crate::services::guardrails::{self as service, Action, Answer, Failure, Phase};
use crate::services::mcp_servers::{UrlProblem, clean_name, parse_url};
use crate::state::AppState;
use kaeso_core::masking::{Rules, RulesError};

type Tagged = [(axum::http::HeaderName, axum::http::HeaderValue); 1];

const MIN_TIMEOUT_MS: u32 = 100;
const MAX_TIMEOUT_MS: u32 = 10_000;
const DEFAULT_TIMEOUT_MS: u32 = 2_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(description = "What Kaeso does when a guardrail times out or gives no usable answer.")]
pub enum GuardrailOnError {
    #[doc = "Refuse the request with 503."]
    Block,
    #[doc = "Let it through, and record the failure."]
    Allow,
}

impl GuardrailOnError {
    fn as_str(self) -> &'static str {
        match self {
            GuardrailOnError::Block => "block",
            GuardrailOnError::Allow => "allow",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(description = "What a guardrail that checks replies does with streamed ones.")]
pub enum GuardrailStreams {
    #[doc = "Streams go to the app unchecked."]
    Skip,
    #[doc = "Kaeso gathers the whole stream, checks it, then streams it to the app; the app\n\
        waits longer for the first token."]
    Hold,
}

impl GuardrailStreams {
    fn as_str(self) -> &'static str {
        match self {
            GuardrailStreams::Skip => "skip",
            GuardrailStreams::Hold => "hold",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(description = "How a guardrail checks.")]
pub enum GuardrailKind {
    #[doc = "An HTTP service of the organization that Kaeso asks."]
    Http,
    #[doc = "Built-in rules that find secrets and personal data inside Kaeso."]
    Rules,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(
    description = "A check of the organization that Kaeso runs before a request goes to a model\n\
    and, optionally, before a reply goes back: an HTTP service or built-in rules."
)]
pub struct GuardrailView {
    #[doc = "Id."]
    pub id: GuardrailId,
    #[doc = "How it checks."]
    pub kind: GuardrailKind,
    #[doc = "The name people and apps see, such as in an error."]
    #[schema(example = "PII check")]
    pub name: String,
    #[doc = "For `http`: the address Kaeso posts to."]
    #[schema(example = "https://guardrail.example.com/check")]
    pub url: Option<String>,
    #[doc = "For `rules`: what they find and what they do."]
    pub rules: Option<Rules>,
    #[doc = "How long Kaeso waits for an answer, in milliseconds."]
    #[schema(example = 2000)]
    pub timeout_ms: u32,
    #[doc = "What Kaeso does when the guardrail times out or fails."]
    pub on_error: GuardrailOnError,
    #[doc = "When it checks."]
    pub phases: Vec<Phase>,
    #[doc = "What it does with streamed replies."]
    pub streams: GuardrailStreams,
    #[doc = "Projects that use it."]
    pub projects: Vec<ProjectId>,
    #[doc = "Who added it."]
    pub created_by: Party,
    #[doc = "Added."]
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[doc = "Last changed."]
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    #[doc = "Goes up with every change. Send it as `If-Match: \"<revision>\"` to change or delete\n\
        the guardrail only if nothing changed it since."]
    #[schema(example = 1)]
    pub revision: i64,
}

fn phases(row: &GuardrailRow) -> Vec<Phase> {
    let mut out = Vec::new();
    if row.request_phase {
        out.push(Phase::Request);
    }
    if row.reply_phase {
        out.push(Phase::Reply);
    }
    out
}

fn view(row: &GuardrailRow, projects: &[Uuid]) -> GuardrailView {
    GuardrailView {
        id: GuardrailId::from_uuid(row.id),
        kind: if row.kind == "rules" {
            GuardrailKind::Rules
        } else {
            GuardrailKind::Http
        },
        name: row.name.clone(),
        url: row.url.clone(),
        rules: row
            .rules
            .as_ref()
            .and_then(|r| serde_json::from_value(r.clone()).ok()),
        timeout_ms: u32::try_from(row.timeout_ms).unwrap_or(DEFAULT_TIMEOUT_MS),
        on_error: if row.on_error == "allow" {
            GuardrailOnError::Allow
        } else {
            GuardrailOnError::Block
        },
        phases: phases(row),
        streams: if row.streams == "hold" {
            GuardrailStreams::Hold
        } else {
            GuardrailStreams::Skip
        },
        projects: projects.iter().copied().map(ProjectId::from_uuid).collect(),
        created_by: creator(&row.created_by_kind, row.created_by_id),
        created_at: row.created_at,
        updated_at: row.updated_at,
        revision: row.revision,
    }
}

async fn links(state: &AppState, org: Uuid) -> ApiResult<BTreeMap<Uuid, Vec<Uuid>>> {
    let mut out: BTreeMap<Uuid, Vec<Uuid>> = BTreeMap::new();
    for (guardrail, project) in store::project_links(&state.pg, org).await? {
        out.entry(guardrail).or_default().push(project);
    }
    Ok(out)
}

async fn one(state: &AppState, row: &GuardrailRow) -> ApiResult<GuardrailView> {
    let links = links(state, row.org_id).await?;
    Ok(view(
        row,
        links.get(&row.id).map(Vec::as_slice).unwrap_or_default(),
    ))
}

fn guardrail_param(raw: &str) -> ApiResult<Uuid> {
    id_param::<GuardrailId>(raw, "guardrail").map(|id| id.uuid())
}

async fn guardrail_row(state: &AppState, org: Uuid, raw: &str) -> ApiResult<GuardrailRow> {
    store::get(&state.pg, org, guardrail_param(raw)?)
        .await?
        .ok_or_else(|| ApiError::not_found("guardrail"))
}

fn checked_name(raw: &str) -> ApiResult<String> {
    clean_name(raw).ok_or_else(|| {
        ApiError::invalid(
            "name",
            "The name must be 1 to 100 characters, without control characters.",
        )
    })
}

async fn checked_url(state: &AppState, raw: &str) -> ApiResult<url::Url> {
    let url = parse_url(raw).map_err(|p| {
        ApiError::invalid(
            "url",
            match p {
                UrlProblem::Length => "`url` must be a URL of at most 2048 characters.",
                UrlProblem::Scheme => {
                    "`url` must start with `https://`, or `http://` for an allowed network."
                }
                UrlProblem::Host => {
                    "`url` must be a full URL with a host, such as `https://guardrail.example.com`."
                }
                UrlProblem::Credentials => "`url` can't hold a user name or password.",
                UrlProblem::Fragment => "`url` can't have a fragment.",
            },
        )
    })?;
    match service::reachability(state, &url).await {
        Reachability::Allowed | Reachability::Unknown => Ok(url),
        Reachability::Refused => Err(ApiError::invalid(
            "url",
            "`url` points to an address this install does not allow.",
        )),
    }
}

fn checked_timeout(ms: u32) -> ApiResult<i32> {
    if (MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(&ms) {
        Ok(i32::try_from(ms).unwrap_or(i32::MAX))
    } else {
        Err(ApiError::invalid(
            "timeout_ms",
            "`timeout_ms` must be from 100 to 10000 milliseconds.",
        ))
    }
}

fn checked_phases(p: &[Phase]) -> ApiResult<(bool, bool)> {
    let request = p.contains(&Phase::Request);
    let reply = p.contains(&Phase::Reply);
    if request || reply {
        Ok((request, reply))
    } else {
        Err(ApiError::invalid(
            "phases",
            "`phases` must name `request`, `reply` or both.",
        ))
    }
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/guardrails",
    summary = "The organization's guardrails, by name.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    responses((status = 200, body = List<GuardrailView>))
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
) -> ApiResult<Json<List<GuardrailView>>> {
    let org = ctx.org()?.org.id;
    let links = links(&state, org).await?;
    let rows = store::list(&state.pg, org).await?;
    Ok(Json(List::complete(
        rows.iter()
            .map(|r| view(r, links.get(&r.id).map(Vec::as_slice).unwrap_or_default()))
            .collect(),
    )))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/guardrails/{guardrail}",
    summary = "One guardrail.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("guardrail" = GuardrailId, Path)),
    responses((status = 200, body = GuardrailView, headers(("ETag" = String, description = "The guardrail's revision as an entity tag."))))
)]
pub async fn get(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, guardrail)): Path<(String, String)>,
) -> ApiResult<(Tagged, Json<GuardrailView>)> {
    let row = guardrail_row(&state, ctx.org()?.org.id, &guardrail).await?;
    Ok((etag(row.revision), Json(one(&state, &row).await?)))
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "A new guardrail.")]
pub struct GuardrailCreate {
    #[doc = "How it checks; `http` when absent."]
    pub kind: Option<GuardrailKind>,
    #[doc = "The name people and apps see, 1 to 100 characters."]
    #[schema(example = "PII check")]
    pub name: String,
    #[doc = "For `http`: the address Kaeso posts to."]
    #[schema(example = "https://guardrail.example.com/check")]
    pub url: Option<String>,
    #[doc = "For `rules`: what they find and what they do."]
    pub rules: Option<Rules>,
    #[doc = "How long Kaeso waits for an answer, from 100 to 10000 milliseconds; 2000 when\n\
        absent."]
    pub timeout_ms: Option<u32>,
    #[doc = "What Kaeso does when the guardrail times out or fails; `block` when absent."]
    pub on_error: Option<GuardrailOnError>,
    #[doc = "When it checks; `[\"request\"]` when absent."]
    pub phases: Option<Vec<Phase>>,
    #[doc = "What it does with streamed replies; `skip` when absent."]
    pub streams: Option<GuardrailStreams>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "A guardrail with its signing secret, shown only in this answer.")]
pub struct GuardrailWithSecret {
    #[doc = "The guardrail."]
    pub guardrail: GuardrailView,
    #[doc = "For `http`: the secret Kaeso signs its requests with (Standard Webhooks), shown\n\
        once."]
    #[schema(example = "whsec_MfKQ9r8GKYqrTwjUPD8ILPZIo2LaLaSw")]
    pub secret: Option<String>,
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/guardrails",
    summary = "Adds a guardrail to the organization, with a new signing secret.",
    description = "Kaeso checks the address now and on every connection: private, loopback,\n\
    link-local and cloud metadata addresses are refused unless the instance allows the network\n\
    (`guardrails.allowed_networks`). The answer holds the signing secret; it is never shown\n\
    again. Projects use it once it is set for them (`PUT …/guardrails/projects`).",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    request_body = GuardrailCreate,
    responses((status = 201, body = GuardrailWithSecret, headers(("ETag" = String, description = "The guardrail's revision as an entity tag."))))
)]
pub async fn create(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
    Body(req): Body<GuardrailCreate>,
) -> ApiResult<(StatusCode, Tagged, Json<GuardrailWithSecret>)> {
    let org = ctx.org()?.org.id;
    let name = checked_name(&req.name)?;
    let rules_kind = req.kind == Some(GuardrailKind::Rules);
    let timeout_ms = checked_timeout(req.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS))?;
    let (request_phase, reply_phase) =
        checked_phases(req.phases.as_deref().unwrap_or(&[Phase::Request]))?;
    let (url, rules) = if rules_kind {
        if req.url.is_some() {
            return Err(ApiError::invalid(
                "url",
                "`url` is only for guardrails of kind `http`.",
            ));
        }
        if reply_phase {
            return Err(ApiError::invalid(
                "phases",
                "Built-in rules check requests only; leave out `phases`.",
            ));
        }
        let rules = req.rules.as_ref().ok_or_else(|| {
            ApiError::invalid("rules", "Guardrails of kind `rules` need `rules`.")
        })?;
        (None, Some(checked_rules(rules)?))
    } else {
        if req.rules.is_some() {
            return Err(ApiError::invalid(
                "rules",
                "`rules` is only for guardrails of kind `rules`.",
            ));
        }
        let raw = req
            .url
            .as_deref()
            .ok_or_else(|| ApiError::invalid("url", "Guardrails of kind `http` need a `url`."))?;
        (Some(checked_url(&state, raw).await?), None)
    };
    let id = Uuid::now_v7();
    let secret = (!rules_kind).then(service::new_secret);
    let sealed = secret
        .as_ref()
        .map(|s| service::seal(&state, id, s))
        .transpose()
        .map_err(ApiError::internal)?;
    let (kind, by) = creator_of(&ctx);
    let mut tx = state.pg.begin().await?;
    store::insert(
        &mut *tx,
        &NewGuardrail {
            id,
            org_id: org,
            kind: if rules_kind { "rules" } else { "http" },
            name: &name,
            url: url.as_ref().map(url::Url::as_str),
            rules: rules.as_ref(),
            secret: sealed.as_ref().map(|sealed| Sealed {
                ciphertext: &sealed.ciphertext,
                nonce: &sealed.nonce,
                wrapped_key: &sealed.wrapped_key,
                key_id: &sealed.key_id,
            }),
            timeout_ms,
            on_error: req.on_error.unwrap_or(GuardrailOnError::Block).as_str(),
            request_phase,
            reply_phase,
            streams: req.streams.unwrap_or(GuardrailStreams::Skip).as_str(),
            created_by_kind: kind,
            created_by_id: by,
        },
    )
    .await?;
    let row = store::get(&mut *tx, org, id)
        .await?
        .ok_or_else(|| ApiError::internal("new guardrail vanished"))?;
    let shown = view(&row, &[]);
    ctx.audit(Some(ctx.org()?.id()), Audit::GuardrailCreate)
        .target("guardrail", GuardrailId::from_uuid(id))
        .after(&shown)
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        etag(row.revision),
        Json(GuardrailWithSecret {
            guardrail: shown,
            secret: secret.map(|s| s.expose_secret().to_owned()),
        }),
    ))
}

fn rules_problem(e: RulesError) -> ApiError {
    let n = |i: usize| i.saturating_add(1);
    match e {
        RulesError::Empty => {
            ApiError::invalid("rules", "`rules` needs a detector or a custom pattern.")
        }
        RulesError::TooMany => {
            ApiError::invalid("rules.custom", "`rules.custom` takes at most 20 patterns.")
        }
        RulesError::Name(i) => ApiError::invalid(
            "rules.custom",
            format!(
                "Pattern {} needs a name of lowercase letters, digits and `_` that no detector uses.",
                n(i)
            ),
        ),
        RulesError::Repeated(i) => ApiError::invalid(
            "rules.custom",
            format!("Pattern {} repeats the name of an earlier one.", n(i)),
        ),
        RulesError::Length(i) => ApiError::invalid(
            "rules.custom",
            format!("Pattern {} must be 1 to 500 characters.", n(i)),
        ),
        RulesError::Pattern(i) => ApiError::invalid(
            "rules.custom",
            format!(
                "Pattern {} isn't a regular expression Kaeso can compile.",
                n(i)
            ),
        ),
    }
}

fn checked_rules(rules: &Rules) -> ApiResult<serde_json::Value> {
    rules.compile().map_err(rules_problem)?;
    serde_json::to_value(rules).map_err(ApiError::internal)
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "Changes to a guardrail; absent fields stay as they are.")]
pub struct GuardrailUpdate {
    #[doc = "A new name."]
    pub name: Option<String>,
    #[doc = "For `http`: a new address."]
    pub url: Option<String>,
    #[doc = "For `rules`: new rules, replacing the old ones."]
    pub rules: Option<Rules>,
    #[doc = "A new timeout, from 100 to 10000 milliseconds."]
    pub timeout_ms: Option<u32>,
    #[doc = "What Kaeso does when the guardrail times out or fails."]
    pub on_error: Option<GuardrailOnError>,
    #[doc = "When it checks."]
    pub phases: Option<Vec<Phase>>,
    #[doc = "What it does with streamed replies."]
    pub streams: Option<GuardrailStreams>,
}

#[utoipa::path(
    patch,
    path = "/orgs/{org}/guardrails/{guardrail}",
    summary = "Changes a guardrail; every project that uses it gets the change at once.",
    description = "With `If-Match`, only while the guardrail is still at that revision; a newer\n\
    one answers 412.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("guardrail" = GuardrailId, Path), ("If-Match" = Option<String>, Header, description = "The `ETag` the change is meant for, such as `\"3\"`.")),
    request_body = GuardrailUpdate,
    responses((status = 200, body = GuardrailView, headers(("ETag" = String, description = "The guardrail's revision as an entity tag."))))
)]
pub async fn update(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, guardrail)): Path<(String, String)>,
    if_match: IfMatch,
    Body(req): Body<GuardrailUpdate>,
) -> ApiResult<(Tagged, Json<GuardrailView>)> {
    let org = ctx.org()?.org.id;
    let before_row = guardrail_row(&state, org, &guardrail).await?;
    if_match.check(before_row.revision, "guardrail")?;
    let name = req.name.as_deref().map(checked_name).transpose()?;
    let rules_kind = before_row.kind == "rules";
    if rules_kind && req.url.is_some() {
        return Err(ApiError::invalid(
            "url",
            "`url` is only for guardrails of kind `http`.",
        ));
    }
    if !rules_kind && req.rules.is_some() {
        return Err(ApiError::invalid(
            "rules",
            "`rules` is only for guardrails of kind `rules`.",
        ));
    }
    let rules = req.rules.as_ref().map(checked_rules).transpose()?;
    if rules_kind
        && req
            .phases
            .as_ref()
            .is_some_and(|p| p.contains(&Phase::Reply))
    {
        return Err(ApiError::invalid(
            "phases",
            "Built-in rules check requests only; leave out `phases`.",
        ));
    }
    let url = match &req.url {
        Some(u) => Some(checked_url(&state, u).await?),
        None => None,
    };
    let timeout_ms = req.timeout_ms.map(checked_timeout).transpose()?;
    let phases = req.phases.as_deref().map(checked_phases).transpose()?;
    let before = one(&state, &before_row).await?;
    let mut tx = state.pg.begin().await?;
    let row = store::update(
        &mut *tx,
        org,
        before_row.id,
        &Changes {
            name: name.as_deref(),
            url: url.as_ref().map(url::Url::as_str),
            rules: rules.as_ref(),
            timeout_ms,
            on_error: req.on_error.map(GuardrailOnError::as_str),
            request_phase: phases.map(|p| p.0),
            reply_phase: phases.map(|p| p.1),
            streams: req.streams.map(GuardrailStreams::as_str),
        },
    )
    .await?
    .ok_or_else(|| ApiError::not_found("guardrail"))?;
    let projects: Vec<Uuid> = before.projects.iter().map(|p| p.uuid()).collect();
    let after = view(&row, &projects);
    ctx.audit(Some(ctx.org()?.id()), Audit::GuardrailUpdate)
        .target("guardrail", GuardrailId::from_uuid(row.id))
        .before(&before)
        .after(&after)
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    service::forget(&state, projects).await;
    Ok((etag(row.revision), Json(after)))
}

#[utoipa::path(
    delete,
    path = "/orgs/{org}/guardrails/{guardrail}",
    summary = "Deletes a guardrail; the projects that used it stop asking it at once.",
    description = "With `If-Match`, only while the guardrail is still at that revision; a newer\n\
    one answers 412. The audit log keeps what was removed.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("guardrail" = GuardrailId, Path), ("If-Match" = Option<String>, Header, description = "The `ETag` the change is meant for, such as `\"3\"`.")),
    responses((status = 204))
)]
pub async fn delete(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, guardrail)): Path<(String, String)>,
    if_match: IfMatch,
) -> ApiResult<StatusCode> {
    let org = ctx.org()?.org.id;
    let row = guardrail_row(&state, org, &guardrail).await?;
    if_match.check(row.revision, "guardrail")?;
    let before = one(&state, &row).await?;
    let mut tx = state.pg.begin().await?;
    if !store::delete(&mut *tx, org, row.id).await? {
        return Err(ApiError::not_found("guardrail"));
    }
    ctx.audit(Some(ctx.org()?.id()), Audit::GuardrailDelete)
        .target("guardrail", GuardrailId::from_uuid(row.id))
        .before(&before)
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    let gone: Vec<Uuid> = before.projects.iter().map(|p| p.uuid()).collect();
    service::forget(&state, gone).await;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/guardrails/{guardrail}/secret",
    summary = "Gives a guardrail a new signing secret; the old one stops at once.",
    description = "The answer holds the new secret; it is never shown again.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("guardrail" = GuardrailId, Path)),
    responses((status = 200, body = GuardrailWithSecret, headers(("ETag" = String, description = "The guardrail's revision as an entity tag."))))
)]
pub async fn rotate_secret(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, guardrail)): Path<(String, String)>,
) -> ApiResult<(Tagged, Json<GuardrailWithSecret>)> {
    let org = ctx.org()?.org.id;
    let before = guardrail_row(&state, org, &guardrail).await?;
    if before.kind == "rules" {
        return Err(ApiError::Conflict(
            "Built-in rules have no signing secret.".into(),
        ));
    }
    let secret = service::new_secret();
    let sealed = service::seal(&state, before.id, &secret).map_err(ApiError::internal)?;
    let mut tx = state.pg.begin().await?;
    let row = store::set_secret(
        &mut *tx,
        org,
        before.id,
        &Sealed {
            ciphertext: &sealed.ciphertext,
            nonce: &sealed.nonce,
            wrapped_key: &sealed.wrapped_key,
            key_id: &sealed.key_id,
        },
    )
    .await?
    .ok_or_else(|| ApiError::not_found("guardrail"))?;
    let shown = one(&state, &row).await?;
    ctx.audit(Some(ctx.org()?.id()), Audit::GuardrailSecretRotate)
        .target("guardrail", GuardrailId::from_uuid(row.id))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    let using: Vec<Uuid> = shown.projects.iter().map(|p| p.uuid()).collect();
    service::forget(&state, using).await;
    Ok((
        etag(row.revision),
        Json(GuardrailWithSecret {
            guardrail: shown,
            secret: Some(secret.expose_secret().to_owned()),
        }),
    ))
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "A sample for a guardrail to check; Kaeso stores nothing of it.")]
pub struct GuardrailTest {
    #[doc = "The phase to ask about; `request` when absent."]
    pub phase: Option<Phase>,
    #[doc = "The conversation, as Kaeso sends it."]
    pub messages: Vec<Said>,
    #[doc = "For the `reply` phase: the reply to check."]
    pub reply: Option<Said>,
    #[doc = "The model to name; empty when absent."]
    pub model: Option<String>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "What a guardrail answered to a sample.")]
pub struct GuardrailTestResult {
    #[doc = "Its verdict; `null` when it gave no usable answer."]
    pub action: Option<Action>,
    #[doc = "The category it named."]
    pub category: Option<String>,
    #[doc = "The reason it gave for a block."]
    pub message: Option<String>,
    #[doc = "For `replace`: the messages it sent back."]
    pub messages: Option<Vec<Said>>,
    #[doc = "Why it gave no usable answer."]
    pub failure: Option<Failure>,
    #[doc = "The HTTP status it answered with, when it answered."]
    pub status: Option<u16>,
    #[doc = "How long it took, in milliseconds."]
    pub duration_ms: u64,
    #[doc = "For `rules`: how many values each kind found, such as `api_key`."]
    #[schema(example = json!({ "api_key": 1 }))]
    pub found: BTreeMap<String, u32>,
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/guardrails/{guardrail}/test",
    summary = "Asks a guardrail about a sample conversation, as a request would.",
    description = "The question is signed like a real one; its `source` is `test` and it names\n\
    no project. Nothing of the sample or the answer is stored.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("guardrail" = GuardrailId, Path)),
    request_body = GuardrailTest,
    responses((status = 200, body = GuardrailTestResult))
)]
pub async fn test(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, guardrail)): Path<(String, String)>,
    Body(req): Body<GuardrailTest>,
) -> ApiResult<Json<GuardrailTestResult>> {
    let org = ctx.org()?.org.id;
    let row = guardrail_row(&state, org, &guardrail).await?;
    let phase = req.phase.unwrap_or(Phase::Request);
    if phase == Phase::Reply && req.reply.is_none() {
        return Err(ApiError::invalid(
            "reply",
            "A test of the `reply` phase needs a `reply`.",
        ));
    }
    let sealed = store::sealed(&state.pg, org, row.id).await?;
    let armed = service::opened(&state, &row, sealed).map_err(ApiError::internal)?;
    if let service::Checker::Rules(scanner) = &armed.checker {
        return Ok(Json(rules_test(scanner, req.messages)));
    }
    let request_id = RequestId::new().to_string();
    let id = format!("{request_id}.{}", phase.as_str());
    let organization = OrgId::from_uuid(org).to_string();
    let model = req.model.unwrap_or_default();
    let asked = Asked {
        id: &id,
        phase,
        request_id: &request_id,
        organization: &organization,
        project: None,
        key: None,
        member: None,
        service_account: None,
        model: &model,
        format: kaeso_kpp::DialectId::OpenAiChat.as_str(),
        source: "test",
        messages: req.messages,
        reply: req.reply.filter(|_| phase == Phase::Reply),
    };
    let body = serde_json::to_vec(&asked).map_err(ApiError::internal)?;
    let started = Instant::now();
    let answer = service::ask(&state, &armed, &id, Bytes::from(body)).await;
    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let mut out = GuardrailTestResult {
        action: None,
        category: None,
        message: None,
        messages: None,
        failure: None,
        status: None,
        duration_ms,
        found: BTreeMap::new(),
    };
    match answer {
        Answer::Verdict(Verdict::Allow { category }) => {
            out.action = Some(Action::Allow);
            out.category = contract::category(category.as_deref());
        }
        Answer::Verdict(Verdict::Block { message, category }) => {
            out.action = Some(Action::Block);
            out.category = contract::category(category.as_deref());
            out.message = contract::message(message.as_deref());
        }
        Answer::Verdict(Verdict::Replace { messages, category }) => {
            out.action = Some(Action::Replace);
            out.category = contract::category(category.as_deref());
            out.messages = Some(messages);
        }
        Answer::Failed { failure, status } => {
            out.failure = Some(failure);
            out.status = status;
        }
    }
    Ok(Json(out))
}

fn mask_parts(m: &mut kaeso_core::masking::Masker<'_>, parts: &mut [contract::Part]) -> bool {
    let mut changed = false;
    for part in parts {
        match part {
            contract::Part::Text { text } => {
                if let Some(out) = m.text(text) {
                    *text = out;
                    changed = true;
                }
            }
            contract::Part::ToolCall { arguments, .. } => {
                if let Some(out) = m.text(arguments) {
                    *arguments = out;
                    changed = true;
                }
            }
            contract::Part::ToolResult { content, .. } => changed |= mask_parts(m, content),
            _ => {}
        }
    }
    changed
}

fn rules_test(
    scanner: &kaeso_core::masking::Scanner,
    mut messages: Vec<Said>,
) -> GuardrailTestResult {
    let started = Instant::now();
    let mut m = scanner.masker();
    let mut changed = false;
    for said in &mut messages {
        changed |= mask_parts(&mut m, &mut said.content);
    }
    let blocked = m.blocked.clone();
    GuardrailTestResult {
        action: Some(if blocked.is_some() {
            Action::Block
        } else if changed {
            Action::Replace
        } else {
            Action::Allow
        }),
        message: blocked
            .as_deref()
            .and_then(|k| contract::message(Some(&service::rules::what(k)))),
        category: blocked,
        messages: changed.then_some(messages),
        failure: None,
        status: None,
        duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        found: m.found.iter().map(|(k, f)| (k.clone(), f.count)).collect(),
    }
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "Rules and a sample text to try them on; Kaeso stores neither.")]
pub struct GuardrailRulesTry {
    #[doc = "The rules."]
    pub rules: Rules,
    #[doc = "The sample, at most 100000 characters."]
    #[schema(example = "Mail ada@example.com")]
    pub text: String,
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/guardrails/try",
    summary = "Tries built-in rules on a sample text before they are saved.",
    description = "The answer is what a request with that text would get: the text as masked, what\n\
    each kind found and whether a rule blocks it. Nothing is stored.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    request_body = GuardrailRulesTry,
    responses((status = 200, body = GuardrailTestResult))
)]
pub async fn try_rules(
    ctx: Ctx,
    Path(_org): Path<String>,
    Body(req): Body<GuardrailRulesTry>,
) -> ApiResult<Json<GuardrailTestResult>> {
    ctx.org()?;
    if req.text.chars().count() > 100_000 {
        return Err(ApiError::invalid(
            "text",
            "`text` takes at most 100000 characters.",
        ));
    }
    let scanner = req.rules.compile().map_err(rules_problem)?;
    let sample = vec![Said {
        role: "user".to_owned(),
        content: vec![contract::Part::Text { text: req.text }],
    }];
    Ok(Json(rules_test(&scanner, sample)))
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "A guardrail a project uses, as the project sees it.")]
pub struct ProjectGuardrailView {
    #[doc = "Id."]
    pub id: GuardrailId,
    #[doc = "Its name."]
    #[schema(example = "PII check")]
    pub name: String,
    #[doc = "When it checks."]
    pub phases: Vec<Phase>,
    #[doc = "What Kaeso does when it times out or fails."]
    pub on_error: GuardrailOnError,
    #[doc = "What it does with streamed replies."]
    pub streams: GuardrailStreams,
    #[doc = "How long Kaeso waits for it, in milliseconds."]
    pub timeout_ms: u32,
}

fn project_view(row: &GuardrailRow) -> ProjectGuardrailView {
    let v = view(row, &[]);
    ProjectGuardrailView {
        id: v.id,
        name: v.name,
        phases: v.phases,
        on_error: v.on_error,
        streams: v.streams,
        timeout_ms: v.timeout_ms,
    }
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/guardrails",
    summary = "The guardrails the project's requests pass, in the order they run.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    responses((status = 200, body = List<ProjectGuardrailView>))
)]
pub async fn list_project(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p)): Path<(String, String)>,
) -> ApiResult<Json<List<ProjectGuardrailView>>> {
    let p = &ctx.project()?.project;
    let rows = store::of_project(&state.pg, p.id).await?;
    Ok(Json(List::complete(
        rows.iter().map(project_view).collect(),
    )))
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "The guardrails a project uses.")]
pub struct ProjectGuardrailsSet {
    #[doc = "The project."]
    pub project: ProjectId,
    #[doc = "Every guardrail the project's requests pass from now on, in the order they run;\n\
        guardrails left out stop checking its requests."]
    pub guardrails: Vec<GuardrailId>,
}

#[utoipa::path(
    put,
    path = "/orgs/{org}/guardrails/projects",
    summary = "Sets the guardrails a project uses and their order.",
    description = "Only organization admins with Manage guardrails choose them: a guardrail is\n\
    the organization's policy, so a project's own admins can't take one off. The project's\n\
    list is `GET …/projects/{project}/guardrails`.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    request_body = ProjectGuardrailsSet,
    responses((status = 200, body = List<ProjectGuardrailView>))
)]
pub async fn set_project(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
    Body(req): Body<ProjectGuardrailsSet>,
) -> ApiResult<Json<List<ProjectGuardrailView>>> {
    let org = ctx.org()?.org.id;
    let project = req.project;
    let in_org: BTreeSet<Uuid> = kaeso_store::pg::projects::ids_of_org(&state.pg, org)
        .await?
        .into_iter()
        .collect();
    if !in_org.contains(&project.uuid()) {
        return Err(ApiError::invalid(
            "project",
            format!("`{project}` is not a project of this organization."),
        ));
    }
    let known: BTreeSet<Uuid> = store::list(&state.pg, org)
        .await?
        .into_iter()
        .map(|g| g.id)
        .collect();
    let mut wanted: Vec<Uuid> = Vec::new();
    for id in &req.guardrails {
        if !known.contains(&id.uuid()) {
            return Err(ApiError::invalid(
                "guardrails",
                format!("`{id}` is not a guardrail of this organization."),
            ));
        }
        if wanted.contains(&id.uuid()) {
            return Err(ApiError::invalid(
                "guardrails",
                format!("`{id}` is in the list twice."),
            ));
        }
        wanted.push(id.uuid());
    }
    let before: Vec<GuardrailId> = store::of_project(&state.pg, project.uuid())
        .await?
        .into_iter()
        .map(|g| GuardrailId::from_uuid(g.id))
        .collect();
    let mut tx = state.pg.begin().await?;
    store::set_project(&mut tx, org, project.uuid(), &wanted).await?;
    ctx.audit(Some(ctx.org()?.id()), Audit::ProjectGuardrailsUpdate)
        .target("project", project)
        .before(serde_json::json!({ "guardrails": before }))
        .after(serde_json::json!({
            "guardrails": wanted.iter().copied().map(GuardrailId::from_uuid).collect::<Vec<_>>()
        }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    service::forget(&state, [project.uuid()]).await;
    let rows = store::of_project(&state.pg, project.uuid()).await?;
    Ok(Json(List::complete(
        rows.iter().map(project_view).collect(),
    )))
}
