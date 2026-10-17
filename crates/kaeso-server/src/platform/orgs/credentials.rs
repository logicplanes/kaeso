use std::collections::BTreeSet;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::id::{CredentialId, OrgId, ProjectId};
use kaeso_core::provider::ProviderId;
use kaeso_store::pg::catalogue;
use kaeso_store::pg::credentials::{self, AssignmentRow, CredentialRow, NewCredential};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::platform::access::Ctx;
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
    #[doc = "The last four characters of the secret, for recognising it."]
    #[schema(example = "wxyz")]
    pub last4: String,
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

impl CredentialView {
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
    #[doc = "The provider: `openai` or `anthropic`."]
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

fn validate_provider(provider: &str) -> ApiResult<&'static kaeso_core::provider::Provider> {
    kaeso_core::provider::lookup(provider).map_err(|_| {
        ApiError::invalid(
            "provider",
            format!(
                "The provider must be one of {}.",
                kaeso_core::provider::ids()
            ),
        )
    })
}

async fn view(db: &mut sqlx::PgConnection, org: Uuid, id: Uuid) -> ApiResult<CredentialView> {
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

async fn forget(state: &AppState, org: Uuid, projects: Vec<Uuid>) {
    service::forget_org(state, org).await;
    for p in projects {
        crate::services::catalogue::forget(state, p).await;
    }
}

fn parse_id(id: &str) -> ApiResult<Uuid> {
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
    let provider = validate_provider(&req.provider)?;
    let name = validate_name(&req.name)?;
    let (secret, last4) = validate_secret(&req.secret)?;
    let id = CredentialId::new();
    let sealed =
        service::seal(&state, id, secret).map_err(|e| ApiError::internal(e.to_string()))?;
    let (kind, by) = creator_of(&ctx);
    let mut tx = state.pg.begin().await?;
    credentials::lock_provider(&mut tx, org, provider.id).await?;
    credentials::insert(
        &mut *tx,
        &NewCredential {
            id: id.uuid(),
            org_id: org,
            provider: provider.id,
            name: &name,
            ciphertext: &sealed.ciphertext,
            nonce: &sealed.nonce,
            wrapped_key: &sealed.wrapped_key,
            key_id: &sealed.key_id,
            last4: &last4,
            created_by_kind: kind,
            created_by_id: by,
        },
    )
    .await?;
    if req.make_default || !credentials::has_default(&mut *tx, org, provider.id).await? {
        credentials::set_default(&mut tx, org, provider.id, id.uuid()).await?;
    }
    let view = view(&mut tx, org, id.uuid()).await?;
    ctx.audit(Some(ctx.org()?.id()), "credential.create")
        .target("credential", id)
        .after(&view)
        .write(&mut tx)
        .await?;
    let projects = reset_availability(&mut tx, org, provider.id).await?;
    tx.commit().await?;
    forget(&state, org, projects).await;
    Ok((StatusCode::CREATED, Json(view)))
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
    let name = match &req.name {
        Some(n) => validate_name(n)?,
        None => before.name.clone(),
    };
    let (secret, last4) = validate_secret(&req.secret)?;
    let sealed =
        service::seal(&state, before.id, secret).map_err(|e| ApiError::internal(e.to_string()))?;
    let (kind, by) = creator_of(&ctx);
    credentials::lock_provider(&mut tx, org, &before.provider).await?;
    let done = credentials::replace(
        &mut *tx,
        &NewCredential {
            id,
            org_id: org,
            provider: &before.provider,
            name: &name,
            ciphertext: &sealed.ciphertext,
            nonce: &sealed.nonce,
            wrapped_key: &sealed.wrapped_key,
            key_id: &sealed.key_id,
            last4: &last4,
            created_by_kind: kind,
            created_by_id: by,
        },
    )
    .await?;
    if !done {
        return Err(ApiError::not_found("credential"));
    }
    let after = view(&mut tx, org, id).await?;
    ctx.audit(Some(ctx.org()?.id()), "credential.replace")
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
    ctx.audit(Some(ctx.org()?.id()), "credential.rename")
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
    ctx.audit(Some(ctx.org()?.id()), "credential.default")
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
    ctx.audit(Some(ctx.org()?.id()), "credential.assign")
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
    if !credentials::delete(&mut *tx, org, id).await? {
        return Err(ApiError::not_found("credential"));
    }
    ctx.audit(Some(ctx.org()?.id()), "credential.delete")
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
        ctx.audit(Some(ctx.org()?.id()), "credential.default")
            .target("credential", after.id)
            .before(&was)
            .after(&after)
            .write(&mut tx)
            .await?;
    }
    let projects = reset_availability(&mut tx, org, &before.provider).await?;
    tx.commit().await?;
    forget(&state, org, projects).await;
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
