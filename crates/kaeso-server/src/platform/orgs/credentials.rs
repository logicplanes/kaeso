use std::collections::BTreeSet;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::id::{CredentialId, OrgId, ProjectId};
use kaeso_core::provider::{AuthMethod, ProviderId};
use kaeso_core::providers::ProviderRef;
use kaeso_store::pg::catalogue;
use kaeso_store::pg::credentials::{self, AssignmentRow, CredentialRow, NewCredential};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::platform::access::Ctx;
use crate::platform::audit::Action;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::platform::keys::{Party, creator, creator_of, validate_name};
use crate::platform::pagination::List;
use crate::platform::params::{Path, id_param};
use crate::services::credentials as service;
use crate::state::AppState;

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "A project an org key is assigned to.")]
pub struct CredentialProject {
    #[doc = "The project."]
    pub id: ProjectId,
    #[doc = "Its name."]
    #[schema(example = "Support bot")]
    pub name: String,
    #[doc = "Its picture; `null` without one."]
    pub avatar_url: Option<String>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "A provider key of the org, without its secret.")]
pub struct CredentialView {
    #[doc = "Id."]
    pub id: CredentialId,
    #[doc = "The provider it authenticates to."]
    #[schema(example = "openai")]
    pub provider: String,
    #[doc = "A name to recognise it by."]
    #[schema(example = "Production OpenAI key")]
    pub name: String,
    #[doc = "Whether projects without an assigned key use it for its provider.\n\
        At most one key per provider is the default."]
    pub is_default: bool,
    #[doc = "The projects that use this key for its provider instead of the default."]
    pub projects: Vec<CredentialProject>,
    #[doc = "The last four characters of the secret, for recognising it; empty for\n\
        an account."]
    #[schema(example = "wxyz")]
    pub last4: String,
    #[doc = "How it authenticates: `api_key`, `chatgpt` for a ChatGPT account or\n\
        `grok` for a Grok account, both signed in with OAuth, whose tokens Kaeso\n\
        renews by itself."]
    pub auth_method: AuthMethodView,
    #[doc = "The signed-in account's email address; `null` for an API key."]
    #[schema(example = "owner@example.com")]
    pub account: Option<String>,
    #[doc = "Why the account can't make requests until someone signs in again:\n\
        `signed_out` when the provider ended its session, `no_api_access` when its\n\
        plan doesn't include API access; `null` while it works."]
    pub auth_error: Option<AuthErrorView>,
    #[doc = "The id of the master key that wraps it (never the key)."]
    #[schema(example = "k1")]
    pub key_id: String,
    #[doc = "Who added it."]
    pub created_by: Party,
    #[doc = "Added."]
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[doc = "Last replaced."]
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    #[doc = "Last used for a request."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_used_at: Option<OffsetDateTime>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(description = "How a provider key authenticates.")]
pub enum AuthMethodView {
    #[doc = "An API key."]
    ApiKey,
    #[doc = "A ChatGPT account, signed in with OAuth."]
    #[serde(rename = "chatgpt")]
    ChatGpt,
    #[doc = "A Grok account, signed in with OAuth."]
    Grok,
}

impl From<AuthMethod> for AuthMethodView {
    fn from(m: AuthMethod) -> Self {
        match m {
            AuthMethod::ApiKey => AuthMethodView::ApiKey,
            AuthMethod::ChatGpt => AuthMethodView::ChatGpt,
            AuthMethod::Grok => AuthMethodView::Grok,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(description = "Why an account can't make requests.")]
pub enum AuthErrorView {
    #[doc = "The provider ended the account's session; sign in again."]
    SignedOut,
    #[doc = "The account's plan doesn't include API access; sign in with another\n\
        account or use an API key."]
    NoApiAccess,
}

impl CredentialView {
    pub(super) fn method(&self) -> AuthMethod {
        match self.auth_method {
            AuthMethodView::ApiKey => AuthMethod::ApiKey,
            AuthMethodView::ChatGpt => AuthMethod::ChatGpt,
            AuthMethodView::Grok => AuthMethod::Grok,
        }
    }

    fn new(r: &CredentialRow, assigned: &[AssignmentRow]) -> CredentialView {
        CredentialView {
            id: CredentialId::from_uuid(r.id),
            provider: r.provider.clone(),
            name: r.name.clone(),
            is_default: r.is_default,
            projects: assigned
                .iter()
                .filter(|a| a.credential_id == r.id)
                .map(|a| CredentialProject {
                    id: ProjectId::from_uuid(a.project_id),
                    name: a.project_name.clone(),
                    avatar_url: crate::platform::views::avatar_url(a.project_avatar.as_deref()),
                })
                .collect(),
            last4: r.last4.clone(),
            auth_method: AuthMethod::parse(&r.auth_method)
                .unwrap_or(AuthMethod::ApiKey)
                .into(),
            account: r.account_label.clone(),
            auth_error: r.auth_error.as_deref().map(|e| match e {
                service::NO_API_ACCESS => AuthErrorView::NoApiAccess,
                _ => AuthErrorView::SignedOut,
            }),
            key_id: r.key_id.clone(),
            created_by: creator(&r.created_by_kind, r.created_by_id),
            created_at: r.created_at,
            updated_at: r.updated_at,
            last_used_at: r.last_used_at,
        }
    }
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "A new provider key.")]
pub struct CredentialCreate {
    #[doc = "The id of a provider Kaeso calls: `openai`, `anthropic`, `deepseek`, `xai`,\n\
        `togetherai`, `groq`, `openrouter`, `mistral`, `vercel`, `cohere`,\n\
        `ollama-cloud`, `moonshotai`, `alibaba`, or one of the organization's custom\n\
        providers that takes a key."]
    #[schema(value_type = ProviderId, example = "openai")]
    pub provider: String,
    #[doc = "A name to recognise it by."]
    #[schema(example = "Production OpenAI key")]
    pub name: String,
    #[doc = "The provider's API key. Stored encrypted and never shown again."]
    #[schema(example = "sk-proj-…")]
    pub secret: String,
    #[doc = "Make it the default for its provider. The first key of a provider\n\
        becomes the default either way."]
    #[serde(default)]
    pub make_default: bool,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "A replacement secret.")]
pub struct CredentialReplace {
    #[doc = "The new provider API key. Stored encrypted and never shown again."]
    #[schema(example = "sk-proj-…")]
    pub secret: String,
    #[doc = "A new name; unchanged when absent."]
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "A new name.")]
pub struct CredentialRename {
    #[doc = "A name to recognise the key by."]
    #[schema(example = "Production OpenAI key")]
    pub name: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "The projects a key is assigned to.")]
pub struct CredentialAssign {
    #[doc = "Every project that uses this key for its provider. Projects that used\n\
        another key of the org for the provider switch to this one; projects\n\
        left out use the default again."]
    pub projects: Vec<ProjectId>,
}

impl std::fmt::Debug for CredentialCreate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CredentialCreate")
            .field("provider", &self.provider)
            .field("name", &self.name)
            .field("make_default", &self.make_default)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for CredentialReplace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CredentialReplace")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

const MAX_SECRET: usize = 4096;
const MIN_SECRET: usize = 12;

fn validate_secret(secret: &str) -> ApiResult<(&str, String)> {
    let secret = secret.trim();
    let len = secret.chars().count();
    if !(MIN_SECRET..=MAX_SECRET).contains(&len) {
        return Err(ApiError::invalid(
            "secret",
            format!("The secret must be {MIN_SECRET} to {MAX_SECRET} characters."),
        ));
    }
    if secret.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(ApiError::invalid(
            "secret",
            "The secret must not contain spaces or control characters.",
        ));
    }
    let last4: String = secret.chars().skip(len - 4).collect();
    Ok((secret, last4))
}

pub(super) fn validate_provider(
    provider: &str,
) -> ApiResult<&'static kaeso_core::provider::Provider> {
    kaeso_core::provider::lookup(provider).map_err(|_| {
        ApiError::invalid(
            "provider",
            "This organization has no such provider; choose one under Providers.",
        )
    })
}

pub(super) async fn provider_in_org(
    state: &AppState,
    org: Uuid,
    provider: &str,
) -> ApiResult<ProviderRef> {
    let providers = crate::services::custom_providers::of_org(state, &state.pg, org).await?;
    providers.get(provider).ok_or_else(|| {
        ApiError::invalid(
            "provider",
            "This organization has no such provider; choose one under Providers.",
        )
    })
}

pub(super) async fn view(
    db: &mut sqlx::PgConnection,
    org: Uuid,
    id: Uuid,
) -> ApiResult<CredentialView> {
    let row = credentials::get(&mut *db, org, id)
        .await?
        .ok_or_else(|| ApiError::not_found("credential"))?;
    let assigned = credentials::assignments(&mut *db, org, Some(id)).await?;
    Ok(CredentialView::new(&row, &assigned))
}

async fn reset_availability(
    tx: &mut sqlx::PgConnection,
    org: Uuid,
    provider: &str,
) -> ApiResult<Vec<Uuid>> {
    Ok(catalogue::clear_availability(&mut *tx, org, provider).await?)
}

pub(super) async fn forget(state: &AppState, org: Uuid, projects: Vec<Uuid>) {
    service::forget_org(state, org).await;
    for p in projects {
        crate::services::catalogue::forget(state, p).await;
    }
}

pub(super) fn parse_id(id: &str) -> ApiResult<Uuid> {
    Ok(id_param::<CredentialId>(id, "credential")?.uuid())
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/credentials",
    summary = "Adds a provider key to the org. The first key of a provider becomes its\n\
    default; later ones only with `make_default`.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    request_body = CredentialCreate,
    responses((status = 201, body = CredentialView))
)]
pub async fn create(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
    Body(req): Body<CredentialCreate>,
) -> ApiResult<(StatusCode, Json<CredentialView>)> {
    let org = ctx.org()?.org.id;
    let provider = provider_in_org(&state, org, &req.provider).await?;
    if provider.auth().is_none() {
        return Err(ApiError::invalid(
            "provider",
            format!(
                "The custom provider `{}` takes no key; Kaeso calls it without one.",
                provider.id()
            ),
        ));
    }
    let name = validate_name(&req.name)?;
    let (secret, last4) = validate_secret(&req.secret)?;
    let view = add(
        &state,
        &ctx,
        org,
        Added {
            id: CredentialId::new(),
            provider: provider.id(),
            name: &name,
            make_default: req.make_default,
            secret,
            auth: Auth::key(&last4),
        },
    )
    .await?;
    Ok((StatusCode::CREATED, Json(view)))
}

pub(super) struct Auth<'a> {
    pub method: AuthMethod,
    pub last4: &'a str,
    pub expires_at: Option<OffsetDateTime>,
    pub account: Option<&'a str>,
}

impl<'a> Auth<'a> {
    fn key(last4: &'a str) -> Auth<'a> {
        Auth {
            method: AuthMethod::ApiKey,
            last4,
            expires_at: None,
            account: None,
        }
    }
}

pub(super) struct Added<'a> {
    pub id: CredentialId,
    pub provider: &'a str,
    pub name: &'a str,
    pub make_default: bool,
    pub secret: &'a str,
    pub auth: Auth<'a>,
}

pub(super) async fn add(
    state: &AppState,
    ctx: &Ctx,
    org: Uuid,
    a: Added<'_>,
) -> ApiResult<CredentialView> {
    let sealed =
        service::seal(state, a.id, a.secret).map_err(|e| ApiError::internal(e.to_string()))?;
    let (kind, by) = creator_of(ctx);
    let mut tx = state.pg.begin().await?;
    credentials::lock_provider(&mut tx, org, a.provider).await?;
    credentials::insert(
        &mut *tx,
        &NewCredential {
            id: a.id.uuid(),
            org_id: org,
            provider: a.provider,
            name: a.name,
            ciphertext: &sealed.ciphertext,
            nonce: &sealed.nonce,
            wrapped_key: &sealed.wrapped_key,
            key_id: &sealed.key_id,
            last4: a.auth.last4,
            created_by_kind: kind,
            created_by_id: by,
            auth_method: a.auth.method.as_str(),
            expires_at: a.auth.expires_at,
            account_label: a.auth.account,
        },
    )
    .await?;
    if a.make_default || !credentials::has_default(&mut *tx, org, a.provider).await? {
        credentials::set_default(&mut tx, org, a.provider, a.id.uuid()).await?;
    }
    let view = view(&mut tx, org, a.id.uuid()).await?;
    ctx.audit(Some(ctx.org()?.id()), Action::CredentialCreate)
        .target("credential", a.id)
        .after(&view)
        .write(&mut tx)
        .await?;
    let projects = reset_availability(&mut tx, org, a.provider).await?;
    tx.commit().await?;
    forget(state, org, projects).await;
    Ok(view)
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/credentials",
    summary = "The org's provider keys (metadata only), by provider, each provider's\n\
    default first.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    responses((status = 200, body = List<CredentialView>))
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
) -> ApiResult<Json<List<CredentialView>>> {
    let org = ctx.org()?.org.id;
    let rows = credentials::list(&state.pg, org).await?;
    let assigned = credentials::assignments(&state.pg, org, None).await?;
    Ok(Json(List::complete(
        rows.iter()
            .map(|r| CredentialView::new(r, &assigned))
            .collect(),
    )))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/credentials/{credential}",
    summary = "One provider key of the org (metadata only).",
    tag = "orgs",
    params(("org" = OrgId, Path), ("credential" = CredentialId, Path)),
    responses((status = 200, body = CredentialView))
)]
pub async fn get(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, id)): Path<(String, String)>,
) -> ApiResult<Json<CredentialView>> {
    let org = ctx.org()?.org.id;
    let mut db = state.pg.acquire().await?;
    Ok(Json(view(&mut db, org, parse_id(&id)?).await?))
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/credentials/{credential}/replace",
    summary = "Replaces a key's secret (and optionally its name). Requests use the new\n\
    one within seconds.",
    description = "An account isn't replaced with a secret: sign in again with\n\
    `POST /orgs/{org}/credentials/{credential}/oauth`.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("credential" = CredentialId, Path)),
    request_body = CredentialReplace,
    responses((status = 200, body = CredentialView))
)]
pub async fn replace(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, id)): Path<(String, String)>,
    Body(req): Body<CredentialReplace>,
) -> ApiResult<Json<CredentialView>> {
    let org = ctx.org()?.org.id;
    let id = parse_id(&id)?;
    let mut tx = state.pg.begin().await?;
    let before = view(&mut tx, org, id).await?;
    if before.method().is_account() {
        return Err(ApiError::bad_request(REPLACE_ACCOUNT));
    }
    let name = match &req.name {
        Some(n) => validate_name(n)?,
        None => before.name.clone(),
    };
    let (secret, last4) = validate_secret(&req.secret)?;
    let after = rewrite(&state, &ctx, tx, &before, &name, secret, Auth::key(&last4)).await?;
    Ok(Json(after))
}

const REPLACE_ACCOUNT: &str =
    "This key signs in with an account; sign in again instead of replacing it.";

pub(super) async fn rewrite(
    state: &AppState,
    ctx: &Ctx,
    mut tx: sqlx::Transaction<'_, sqlx::Postgres>,
    before: &CredentialView,
    name: &str,
    secret: &str,
    auth: Auth<'_>,
) -> ApiResult<CredentialView> {
    let org = ctx.org()?.org.id;
    let id = before.id.uuid();
    let sealed =
        service::seal(state, before.id, secret).map_err(|e| ApiError::internal(e.to_string()))?;
    let (kind, by) = creator_of(ctx);
    credentials::lock_provider(&mut tx, org, &before.provider).await?;
    let done = credentials::replace(
        &mut *tx,
        &NewCredential {
            id,
            org_id: org,
            provider: &before.provider,
            name,
            ciphertext: &sealed.ciphertext,
            nonce: &sealed.nonce,
            wrapped_key: &sealed.wrapped_key,
            key_id: &sealed.key_id,
            last4: auth.last4,
            created_by_kind: kind,
            created_by_id: by,
            auth_method: auth.method.as_str(),
            expires_at: auth.expires_at,
            account_label: auth.account,
        },
    )
    .await?;
    if !done {
        return Err(ApiError::not_found("credential"));
    }
    let after = view(&mut tx, org, id).await?;
    ctx.audit(Some(ctx.org()?.id()), Action::CredentialReplace)
        .target("credential", before.id)
        .before(before)
        .after(&after)
        .write(&mut tx)
        .await?;
    let projects = reset_availability(&mut tx, org, &before.provider).await?;
    tx.commit().await?;
    forget(state, org, projects).await;
    Ok(after)
}

#[utoipa::path(
    patch,
    path = "/orgs/{org}/credentials/{credential}",
    summary = "Renames a provider key.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("credential" = CredentialId, Path)),
    request_body = CredentialRename,
    responses((status = 200, body = CredentialView))
)]
pub async fn rename(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, id)): Path<(String, String)>,
    Body(req): Body<CredentialRename>,
) -> ApiResult<Json<CredentialView>> {
    let org = ctx.org()?.org.id;
    let id = parse_id(&id)?;
    let name = validate_name(&req.name)?;
    let mut tx = state.pg.begin().await?;
    let before = view(&mut tx, org, id).await?;
    if !credentials::rename(&mut *tx, org, id, &name).await? {
        return Err(ApiError::not_found("credential"));
    }
    let after = view(&mut tx, org, id).await?;
    ctx.audit(Some(ctx.org()?.id()), Action::CredentialRename)
        .target("credential", before.id)
        .before(&before)
        .after(&after)
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(Json(after))
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/credentials/{credential}/default",
    summary = "Makes a key the default for its provider: every project without an\n\
    assigned key for the provider uses it from now on.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("credential" = CredentialId, Path)),
    responses((status = 200, body = CredentialView))
)]
pub async fn make_default(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, id)): Path<(String, String)>,
) -> ApiResult<Json<CredentialView>> {
    let org = ctx.org()?.org.id;
    let id = parse_id(&id)?;
    let mut tx = state.pg.begin().await?;
    let before = view(&mut tx, org, id).await?;
    if before.is_default {
        return Ok(Json(before));
    }
    credentials::lock_provider(&mut tx, org, &before.provider).await?;
    if !credentials::set_default(&mut tx, org, &before.provider, id).await? {
        return Err(ApiError::not_found("credential"));
    }
    let after = view(&mut tx, org, id).await?;
    ctx.audit(Some(ctx.org()?.id()), Action::CredentialDefault)
        .target("credential", before.id)
        .before(&before)
        .after(&after)
        .write(&mut tx)
        .await?;
    let projects = reset_availability(&mut tx, org, &before.provider).await?;
    tx.commit().await?;
    forget(&state, org, projects).await;
    Ok(Json(after))
}

#[utoipa::path(
    put,
    path = "/orgs/{org}/credentials/{credential}/projects",
    summary = "Sets the projects that use a key for its provider instead of the default.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("credential" = CredentialId, Path)),
    request_body = CredentialAssign,
    responses((status = 200, body = CredentialView))
)]
pub async fn assign(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, id)): Path<(String, String)>,
    Body(req): Body<CredentialAssign>,
) -> ApiResult<Json<CredentialView>> {
    let org = ctx.org()?.org.id;
    let id = parse_id(&id)?;
    let wanted: BTreeSet<Uuid> = req.projects.iter().map(|p| p.uuid()).collect();
    let mut tx = state.pg.begin().await?;
    let before = view(&mut tx, org, id).await?;
    let current: BTreeSet<Uuid> = before.projects.iter().map(|p| p.id.uuid()).collect();
    let added: Vec<Uuid> = wanted.difference(&current).copied().collect();
    let active: BTreeSet<Uuid> = credentials::active_projects(&mut *tx, org, &added)
        .await?
        .into_iter()
        .collect();
    if let Some(missing) = added.iter().find(|p| !active.contains(p)) {
        return Err(ApiError::invalid(
            "projects",
            format!(
                "`{}` is not an active project of this organization.",
                ProjectId::from_uuid(*missing)
            ),
        ));
    }
    let projects: Vec<Uuid> = wanted.into_iter().collect();
    credentials::lock_provider(&mut tx, org, &before.provider).await?;
    credentials::set_assignments(&mut tx, org, &before.provider, id, &projects).await?;
    let after = view(&mut tx, org, id).await?;
    ctx.audit(Some(ctx.org()?.id()), Action::CredentialAssign)
        .target("credential", before.id)
        .before(&before)
        .after(&after)
        .write(&mut tx)
        .await?;
    let projects = reset_availability(&mut tx, org, &before.provider).await?;
    tx.commit().await?;
    forget(&state, org, projects).await;
    Ok(Json(after))
}

#[utoipa::path(
    delete,
    path = "/orgs/{org}/credentials/{credential}",
    summary = "Deletes a provider key. Its projects use the default again; when it was\n\
    the default, the most recently added remaining key of its provider\n\
    becomes the default, and without one, projects can no longer call that\n\
    provider's models.",
    description = "For an account, Kaeso also asks the provider to end the account's\n\
    session once the key is deleted. The key is deleted even when the\n\
    provider can't be reached.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("credential" = CredentialId, Path)),
    responses((status = 204))
)]
pub async fn delete(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, id)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    let org = ctx.org()?.org.id;
    let id = parse_id(&id)?;
    let mut tx = state.pg.begin().await?;
    let before = view(&mut tx, org, id).await?;
    credentials::lock_provider(&mut tx, org, &before.provider).await?;
    credentials::lock_refresh(&mut tx, id).await?;
    let revocable = super::credential_accounts::revocable(&state, &mut tx, org, &before).await;
    if !credentials::delete(&mut *tx, org, id).await? {
        return Err(ApiError::not_found("credential"));
    }
    ctx.audit(Some(ctx.org()?.id()), Action::CredentialDelete)
        .target("credential", before.id)
        .before(&before)
        .write(&mut tx)
        .await?;
    if before.is_default
        && let Some(promoted) = credentials::promote_newest(&mut *tx, org, &before.provider).await?
    {
        let after = view(&mut tx, org, promoted).await?;
        let was = CredentialView {
            is_default: false,
            ..after.clone()
        };
        ctx.audit(Some(ctx.org()?.id()), Action::CredentialDefault)
            .target("credential", after.id)
            .before(&was)
            .after(&after)
            .write(&mut tx)
            .await?;
    }
    let projects = reset_availability(&mut tx, org, &before.provider).await?;
    tx.commit().await?;
    forget(&state, org, projects).await;
    if let Some(revocable) = revocable {
        revocable.revoke().await;
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_are_checked_and_trimmed() {
        let (s, last4) =
            validate_secret("  sk-test-0123456789abcd \n").unwrap_or_else(|_| ("", String::new()));
        assert_eq!(s, "sk-test-0123456789abcd");
        assert_eq!(last4, "abcd");
        assert!(validate_secret("short").is_err());
        assert!(validate_secret("sk-test 0123456789").is_err());
        assert!(validate_secret(&"x".repeat(4097)).is_err());
    }
}
