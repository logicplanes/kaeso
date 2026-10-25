use std::collections::BTreeMap;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::id::OrgId;
use kaeso_core::provider::{self, ProviderId};
use kaeso_core::providers::{self as custom, CustomAuth, CustomProvider, Draft, Problem};
use kaeso_kpp::DialectId;
use kaeso_store::pg::custom_providers::{self as store, CustomProviderRow, Fields, InUse};
use serde::{Deserialize, Deserializer, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::platform::access::Ctx;
use crate::platform::audit::Action;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::platform::keys::{Party, creator, creator_of};
use crate::platform::pagination::List;
use crate::platform::params::Path;
use crate::platform::precondition::{IfMatch, etag};
use crate::services::custom_providers::{self as service, Reachability};
use crate::state::AppState;

type Tagged = [(axum::http::HeaderName, axum::http::HeaderValue); 1];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(description = "How Kaeso sends a custom provider's key.")]
pub enum CustomAuthView {
    #[doc = "`Authorization: Bearer <key>`, as OpenAI-compatible servers expect."]
    Bearer,
    #[doc = "`x-api-key: <key>`, as Anthropic-compatible servers expect."]
    XApiKey,
    #[doc = "No key: requests go out without one, for a server on your own network."]
    None,
}

impl From<CustomAuth> for CustomAuthView {
    fn from(a: CustomAuth) -> Self {
        match a {
            CustomAuth::Bearer => CustomAuthView::Bearer,
            CustomAuth::XApiKey => CustomAuthView::XApiKey,
            CustomAuth::None => CustomAuthView::None,
        }
    }
}

impl From<CustomAuthView> for CustomAuth {
    fn from(a: CustomAuthView) -> Self {
        match a {
            CustomAuthView::Bearer => CustomAuth::Bearer,
            CustomAuthView::XApiKey => CustomAuth::XApiKey,
            CustomAuthView::None => CustomAuth::None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(description = "An API format a custom provider generates with.")]
pub enum GenerationDialect {
    #[doc = "OpenAI Chat Completions, `POST /v1/chat/completions`: vLLM, Ollama, LM Studio,\n\
        TGI, SGLang and most OpenAI-compatible servers."]
    OpenaiChat,
    #[doc = "OpenAI Responses, `POST /v1/responses`."]
    OpenaiResponses,
    #[doc = "Anthropic Messages, `POST /v1/messages`."]
    AnthropicMessages,
}

impl From<GenerationDialect> for DialectId {
    fn from(d: GenerationDialect) -> Self {
        match d {
            GenerationDialect::OpenaiChat => DialectId::OpenAiChat,
            GenerationDialect::OpenaiResponses => DialectId::OpenAiResponses,
            GenerationDialect::AnthropicMessages => DialectId::AnthropicMessages,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(description = "An API format a custom provider creates embeddings with.")]
pub enum EmbeddingsDialect {
    #[doc = "OpenAI Embeddings, `POST /v1/embeddings`."]
    OpenaiEmbeddings,
}

impl From<EmbeddingsDialect> for DialectId {
    fn from(_: EmbeddingsDialect) -> Self {
        DialectId::OpenAiEmbeddings
    }
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "What uses a custom provider in the organization.")]
pub struct CustomProviderUse {
    #[doc = "Its provider keys."]
    pub keys: i64,
    #[doc = "Catalogue models that call it, across all projects."]
    pub models: i64,
    #[doc = "Projects with at least one of those models."]
    pub projects: i64,
}

impl From<InUse> for CustomProviderUse {
    fn from(u: InUse) -> Self {
        CustomProviderUse {
            keys: u.keys,
            models: u.models,
            projects: u.projects,
        }
    }
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(
    description = "A provider the organization added itself: a base URL Kaeso calls in the API\n\
    formats it names. Kaeso knows nothing else about it; prices, limits and\n\
    capabilities come from its catalogue models."
)]
pub struct CustomProviderView {
    #[doc = "The id, which names the provider in model names such as\n\
        `my-vllm/llama-3.3-70b`, in provider keys and in catalogue models. It\n\
        never changes."]
    pub id: ProviderId,
    #[doc = "The name people see."]
    #[schema(example = "Local vLLM")]
    pub name: String,
    #[doc = "Where Kaeso sends requests; the format's path, such as\n\
        `/v1/chat/completions`, is appended to it."]
    #[schema(example = "https://vllm.example.com")]
    pub base_url: String,
    #[doc = "How Kaeso sends the provider key."]
    pub auth: CustomAuthView,
    #[doc = "The format of its generation models."]
    pub generation: GenerationDialect,
    #[doc = "The format of its embeddings models; `null` when it has none."]
    pub embeddings: Option<EmbeddingsDialect>,
    #[doc = "The format in which it counts tokens, its `generation` format;\n\
        `null` when it counts none."]
    pub count_tokens: Option<GenerationDialect>,
    #[doc = "The name of the built-in provider with the same id, which a newer Kaeso\n\
        release added; inside this organization the custom provider takes its\n\
        place. `null` without one."]
    #[schema(example = json!(null))]
    pub hides_built_in: Option<String>,
    #[doc = "What deleting it would remove."]
    pub in_use: CustomProviderUse,
    #[doc = "Who added it."]
    pub created_by: Party,
    #[doc = "Added."]
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[doc = "Last changed."]
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    #[doc = "Goes up with every change. Send it as `If-Match: \"<revision>\"` to change\n\
        or delete the provider only if nothing changed it since."]
    #[schema(example = 1)]
    pub revision: i64,
}

fn generation_view(d: DialectId) -> GenerationDialect {
    match d {
        DialectId::OpenAiResponses => GenerationDialect::OpenaiResponses,
        DialectId::AnthropicMessages => GenerationDialect::AnthropicMessages,
        _ => GenerationDialect::OpenaiChat,
    }
}

fn view(row: &CustomProviderRow, in_use: InUse) -> ApiResult<CustomProviderView> {
    let p = row.custom_provider()?;
    Ok(CustomProviderView {
        id: p.id().clone(),
        name: p.name().to_owned(),
        base_url: p.base_url().as_str().to_owned(),
        auth: p.auth().into(),
        generation: generation_view(p.generation()),
        embeddings: p.embeddings().map(|_| EmbeddingsDialect::OpenaiEmbeddings),
        count_tokens: p.count_tokens().map(generation_view),
        hides_built_in: provider::provider(p.id().as_str()).map(|b| b.name.to_owned()),
        in_use: in_use.into(),
        created_by: creator(&row.created_by_kind, row.created_by_id),
        created_at: row.created_at,
        updated_at: row.updated_at,
        revision: row.revision,
    })
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "A new custom provider.")]
pub struct CustomProviderCreate {
    #[doc = "The id: lowercase letters and digits with single hyphens between them, at\n\
        most 40 characters. It can't be the id of a built-in provider or of a\n\
        provider listed on models.dev."]
    #[schema(value_type = ProviderId, example = "my-vllm")]
    pub id: String,
    #[doc = "The name people see, 1 to 100 characters."]
    #[schema(example = "Local vLLM")]
    pub name: String,
    #[doc = "`https`, or `http` for a network the instance allows; no user name,\n\
        query or fragment. A path is kept, such as `https://proxy.example.com/llm`."]
    #[schema(example = "https://vllm.example.com")]
    pub base_url: String,
    #[doc = "How Kaeso sends the provider key."]
    pub auth: CustomAuthView,
    #[doc = "The format of its generation models."]
    pub generation: GenerationDialect,
    #[doc = "The format of its embeddings models; absent or `null` for none."]
    #[serde(default)]
    pub embeddings: Option<EmbeddingsDialect>,
    #[doc = "Whether it counts tokens in its `generation` format\n\
        (`openai_responses` or `anthropic_messages` only); absent or `null` for none."]
    #[serde(default)]
    pub count_tokens: Option<GenerationDialect>,
}

fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(d).map(Some)
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "Changes to a custom provider; absent fields stay as they are.")]
pub struct CustomProviderUpdate {
    #[doc = "A new name."]
    pub name: Option<String>,
    #[doc = "A new base URL; the provider's keys go to it from now on."]
    pub base_url: Option<String>,
    #[doc = "A new way to send the key; `none` only once the provider has no keys."]
    pub auth: Option<CustomAuthView>,
    #[doc = "A new generation format."]
    pub generation: Option<GenerationDialect>,
    #[doc = "A new embeddings format; `null` for none."]
    #[serde(default, deserialize_with = "present")]
    #[schema(value_type = Option<EmbeddingsDialect>, nullable)]
    pub embeddings: Option<Option<EmbeddingsDialect>>,
    #[doc = "Whether it counts tokens in its `generation` format; `null` for none."]
    #[serde(default, deserialize_with = "present")]
    #[schema(value_type = Option<GenerationDialect>, nullable)]
    pub count_tokens: Option<Option<GenerationDialect>>,
}

fn turned_off(state: &AppState) -> ApiResult<()> {
    if service::enabled(state) {
        Ok(())
    } else {
        Err(ApiError::Conflict(
            "Custom providers are turned off on this install; ask its operator.".into(),
        ))
    }
}

fn parse_id(id: &str) -> ApiResult<ProviderId> {
    let id = ProviderId::parse(id).map_err(|_| {
        ApiError::invalid(
            "id",
            "`id` must be lowercase letters and digits with single hyphens between them.",
        )
    })?;
    if id.as_str().len() > custom::MAX_ID_LEN {
        return Err(ApiError::invalid(
            "id",
            "`id` can have at most 40 characters.",
        ));
    }
    if provider::provider(id.as_str()).is_some() {
        return Err(ApiError::invalid(
            "id",
            format!("`{id}` is a built-in provider; choose another id."),
        ));
    }
    if custom::is_reserved(id.as_str()) {
        return Err(ApiError::invalid(
            "id",
            format!(
                "`{id}` is a provider listed on models.dev that Kaeso may add; choose another id."
            ),
        ));
    }
    Ok(id)
}

fn problem(p: Problem) -> ApiError {
    let (field, message) = match p {
        Problem::IdTooLong => ("id", "`id` can have at most 40 characters."),
        Problem::Name => (
            "name",
            "The name must be 1 to 100 characters, without control characters.",
        ),
        Problem::BaseUrlLength => (
            "base_url",
            "`base_url` must be a URL of at most 2048 characters.",
        ),
        Problem::BaseUrlScheme => (
            "base_url",
            "`base_url` must start with `https://`, or `http://` for an allowed network.",
        ),
        Problem::BaseUrlHost => (
            "base_url",
            "`base_url` must be a full URL with a host, such as `https://llm.example.com`.",
        ),
        Problem::BaseUrlCredentials => (
            "base_url",
            "`base_url` can't hold a user name or password; add a provider key instead.",
        ),
        Problem::BaseUrlQuery => ("base_url", "`base_url` can't have a query or a fragment."),
        Problem::Generation => (
            "generation",
            "`generation` must be `openai_chat`, `openai_responses` or `anthropic_messages`.",
        ),
        Problem::Embeddings => (
            "embeddings",
            "`embeddings` must be `openai_embeddings` or `null`.",
        ),
        Problem::CountTokens => (
            "count_tokens",
            "`count_tokens` must be `null` or the `generation` format, which must count tokens.",
        ),
    };
    ApiError::invalid(field, message)
}

async fn check_reach(state: &AppState, p: &CustomProvider) -> ApiResult<()> {
    let url = p.base_url();
    match service::reachability(state, url).await {
        Reachability::Allowed | Reachability::Unknown => Ok(()),
        Reachability::Refused if url.scheme() == "http" => {
            let mut https = url.clone();
            let public = https.set_scheme("https").is_ok()
                && service::reachability(state, &https).await == Reachability::Allowed;
            Err(ApiError::invalid(
                "base_url",
                if public {
                    "`base_url` uses `http` for a public address; use `https`."
                } else {
                    "`base_url` points to an address this install does not allow."
                },
            ))
        }
        Reachability::Refused => Err(ApiError::invalid(
            "base_url",
            "`base_url` points to an address this install does not allow.",
        )),
    }
}

async fn in_use(state: &AppState, org: Uuid) -> ApiResult<BTreeMap<String, InUse>> {
    let mut conn = state.pg.acquire().await?;
    Ok(store::in_use(&mut conn, org).await?)
}

async fn forget(state: &AppState, org: Uuid, more: Vec<Uuid>) -> ApiResult<()> {
    let mut projects = store::org_projects(&state.pg, org).await?;
    projects.extend(more);
    service::forget(state, projects).await;
    crate::services::credentials::forget_org(state, org).await;
    Ok(())
}

fn provider_param(id: &str) -> ApiResult<String> {
    ProviderId::parse(id)
        .map(|id| id.as_str().to_owned())
        .map_err(|_| ApiError::not_found("custom provider"))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/custom-providers",
    summary = "The organization's custom providers, by id.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    responses((status = 200, body = List<CustomProviderView>))
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
) -> ApiResult<Json<List<CustomProviderView>>> {
    let org = ctx.org()?.org.id;
    let rows = store::list(&state.pg, org).await?;
    let mut used = in_use(&state, org).await?;
    Ok(Json(List::complete(
        rows.iter()
            .map(|r| view(r, used.remove(&r.provider).unwrap_or_default()))
            .collect::<ApiResult<Vec<_>>>()?,
    )))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/custom-providers/{provider}",
    summary = "One custom provider.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("provider" = ProviderId, Path)),
    responses((status = 200, body = CustomProviderView, headers(("ETag" = String, description = "The provider's revision as an entity tag."))))
)]
pub async fn get(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, id)): Path<(String, String)>,
) -> ApiResult<(Tagged, Json<CustomProviderView>)> {
    let org = ctx.org()?.org.id;
    let id = provider_param(&id)?;
    let row = store::get(&state.pg, org, &id)
        .await?
        .ok_or_else(|| ApiError::not_found("custom provider"))?;
    let used = in_use(&state, org).await?.remove(&id).unwrap_or_default();
    Ok((etag(row.revision), Json(view(&row, used)?)))
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/custom-providers",
    summary = "Adds a custom provider: a server that speaks an API format Kaeso\n\
    translates, such as a self-hosted vLLM or an internal proxy.",
    description = "Kaeso checks the base URL's address now and on every connection:\n\
    private, loopback, link-local and cloud metadata addresses are refused\n\
    unless the instance allows the network\n\
    (`custom_providers.allowed_networks`). Add its models to a project's\n\
    catalogue with `custom: true`, their prices and limits set by hand.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    request_body = CustomProviderCreate,
    responses((status = 201, body = CustomProviderView, headers(("ETag" = String, description = "The provider's revision as an entity tag."))))
)]
pub async fn create(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
    Body(req): Body<CustomProviderCreate>,
) -> ApiResult<(StatusCode, Tagged, Json<CustomProviderView>)> {
    turned_off(&state)?;
    let org = ctx.org()?.org.id;
    let id = parse_id(&req.id)?;
    let p = CustomProvider::new(Draft {
        uuid: Uuid::now_v7(),
        id: id.clone(),
        name: req.name,
        base_url: req.base_url,
        auth: req.auth.into(),
        generation: req.generation.into(),
        embeddings: req.embeddings.map(Into::into),
        count_tokens: req.count_tokens.map(Into::into),
    })
    .map_err(problem)?;
    check_reach(&state, &p).await?;
    let (kind, by) = creator_of(&ctx);
    let mut tx = state.pg.begin().await?;
    let added = store::insert(
        &mut *tx,
        &store::NewCustomProvider {
            id: p.uuid(),
            org_id: org,
            provider: id.as_str(),
            fields: Fields::of(&p),
            created_by_kind: kind,
            created_by_id: by,
        },
    )
    .await?;
    if !added {
        return Err(ApiError::Conflict(format!(
            "This organization already has a custom provider `{id}`."
        )));
    }
    let row = store::get(&mut *tx, org, id.as_str())
        .await?
        .ok_or_else(|| ApiError::internal("new custom provider vanished"))?;
    let shown = view(&row, InUse::default())?;
    ctx.audit(Some(ctx.org()?.id()), Action::CustomProviderCreate)
        .target("custom_provider", &id)
        .after(&shown)
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    forget(&state, org, Vec::new()).await?;
    Ok((StatusCode::CREATED, etag(row.revision), Json(shown)))
}

#[utoipa::path(
    patch,
    path = "/orgs/{org}/custom-providers/{provider}",
    summary = "Changes a custom provider's name, base URL, key style or formats.",
    description = "With `If-Match`, only while the provider is still at that revision; a\n\
    newer one answers 412. A new base URL, key style or format makes its\n\
    models `unchecked` again.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("provider" = ProviderId, Path), ("If-Match" = Option<String>, Header, description = "The `ETag` the change is meant for, such as `\"3\"`.")),
    request_body = CustomProviderUpdate,
    responses((status = 200, body = CustomProviderView, headers(("ETag" = String, description = "The provider's revision as an entity tag."))))
)]
pub async fn update(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, id)): Path<(String, String)>,
    if_match: IfMatch,
    Body(req): Body<CustomProviderUpdate>,
) -> ApiResult<(Tagged, Json<CustomProviderView>)> {
    turned_off(&state)?;
    let org = ctx.org()?.org.id;
    let id = provider_param(&id)?;
    let mut tx = state.pg.begin().await?;
    kaeso_store::pg::credentials::lock_provider(&mut tx, org, &id).await?;
    let row = store::get_for_update(&mut *tx, org, &id)
        .await?
        .ok_or_else(|| ApiError::not_found("custom provider"))?;
    if_match.check(row.revision, "custom provider")?;
    let before_provider = row.custom_provider()?;
    let mut draft = before_provider.draft();
    if let Some(name) = req.name {
        draft.name = name;
    }
    if let Some(url) = req.base_url {
        draft.base_url = url;
    }
    if let Some(auth) = req.auth {
        draft.auth = auth.into();
    }
    if let Some(g) = req.generation {
        draft.generation = g.into();
    }
    if let Some(e) = req.embeddings {
        draft.embeddings = e.map(Into::into);
    }
    if let Some(c) = req.count_tokens {
        draft.count_tokens = c.map(Into::into);
    }
    let after_provider = CustomProvider::new(draft).map_err(problem)?;
    let used = store::in_use(&mut tx, org)
        .await?
        .remove(&id)
        .unwrap_or_default();
    if after_provider.auth() == CustomAuth::None && used.keys > 0 {
        return Err(ApiError::invalid(
            "auth",
            "Delete the provider's keys before switching it to no key.",
        ));
    }
    if after_provider.base_url() != before_provider.base_url() {
        check_reach(&state, &after_provider).await?;
    }
    let before = view(&row, used)?;
    if !store::update(&mut *tx, org, &id, &Fields::of(&after_provider)).await? {
        return Err(ApiError::not_found("custom provider"));
    }
    let row = store::get(&mut *tx, org, &id)
        .await?
        .ok_or_else(|| ApiError::internal("changed custom provider vanished"))?;
    let after = view(&row, used)?;
    ctx.audit(Some(ctx.org()?.id()), Action::CustomProviderUpdate)
        .target("custom_provider", &id)
        .before(&before)
        .after(&after)
        .write(&mut tx)
        .await?;
    let reset = if after_provider.base_url() != before_provider.base_url()
        || after_provider.auth() != before_provider.auth()
        || after_provider.dialects() != before_provider.dialects()
    {
        kaeso_store::pg::catalogue::clear_availability(&mut *tx, org, &id).await?
    } else {
        Vec::new()
    };
    tx.commit().await?;
    forget(&state, org, reset).await?;
    Ok((etag(row.revision), Json(after)))
}

#[utoipa::path(
    delete,
    path = "/orgs/{org}/custom-providers/{provider}",
    summary = "Deletes a custom provider with its provider keys and its models in every\n\
    project's catalogue; requests for them fail from now on.",
    description = "With `If-Match`, only while the provider is still at that revision; a\n\
    newer one answers 412. The audit log keeps what was removed.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("provider" = ProviderId, Path), ("If-Match" = Option<String>, Header, description = "The `ETag` the change is meant for, such as `\"3\"`.")),
    responses((status = 204))
)]
pub async fn delete(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, id)): Path<(String, String)>,
    if_match: IfMatch,
) -> ApiResult<StatusCode> {
    let org = ctx.org()?.org.id;
    let id = provider_param(&id)?;
    let mut tx = state.pg.begin().await?;
    kaeso_store::pg::credentials::lock_provider(&mut tx, org, &id).await?;
    let row = store::get_for_update(&mut *tx, org, &id)
        .await?
        .ok_or_else(|| ApiError::not_found("custom provider"))?;
    if_match.check(row.revision, "custom provider")?;
    let used = store::in_use(&mut tx, org)
        .await?
        .remove(&id)
        .unwrap_or_default();
    let before = view(&row, used)?;
    let removed = store::delete(&mut tx, org, &id)
        .await?
        .ok_or_else(|| ApiError::not_found("custom provider"))?;
    ctx.audit(Some(ctx.org()?.id()), Action::CustomProviderDelete)
        .target("custom_provider", &id)
        .before(&before)
        .after(serde_json::json!({
            "removed_keys": removed.credentials,
            "removed_models": removed.models,
            "projects": removed.projects,
        }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    forget(&state, org, removed.projects).await?;
    Ok(StatusCode::NO_CONTENT)
}
