use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use kaeso_core::id::{CatalogueModelId, OrgId, ProjectId};
use kaeso_core::provider;
use kaeso_kpp::{DialectId, ErrorKind, KppError, RequestKind};
use kaeso_store::pg::credentials;
use serde::Serialize;
use utoipa::ToSchema;

use crate::model_api::errors::stamp;
use crate::model_api::hooks::{Caller as ModelCaller, Source};
use crate::model_api::pipeline::{self, Failure, Incoming, Stage};
use crate::platform::access::{Caller, Ctx};
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::pagination::List;
use crate::platform::params::Path;
use crate::state::AppState;

#[derive(Debug, ToSchema)]
#[schema(
    description = "A Playground request: an OpenAI Responses request, streamed. The console\n\
    sends this subset; the body is decoded by kpp like a `/v1/responses`\n\
    body, so other Responses fields are taken as there."
)]
pub struct PlaygroundRequest {
    #[doc = "A model of this project's catalogue: its id, `provider/model`, or an\n\
        alias of the project."]
    #[schema(example = "claude-sonnet-4-5")]
    pub model: String,
    #[doc = "The conversation: a string, or Responses input items (messages, and\n\
        the reasoning items of earlier replies, sent back as received)."]
    #[schema(value_type = Object)]
    pub input: serde_json::Value,
    #[doc = "System instructions."]
    pub instructions: Option<String>,
    #[doc = "Sampling temperature."]
    pub temperature: Option<f64>,
    #[doc = "Nucleus sampling."]
    pub top_p: Option<f64>,
    #[doc = "The most output tokens."]
    pub max_output_tokens: Option<u64>,
    #[doc = "Reasoning: `effort` (`none`, `minimal`, `low`, `medium`, `high`,\n\
        `xhigh`, `max`) and `summary` (`auto`, `concise`, `detailed`)."]
    #[schema(value_type = Option<Object>)]
    pub reasoning: Option<serde_json::Value>,
    #[doc = "Must be `true`: the Playground serves streams only."]
    #[schema(example = true)]
    pub stream: bool,
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/playground/responses",
    summary = "Sends a Playground request as the signed-in member.",
    description = "Sends a request to one of the project's models as the signed-in member\n\
    and streams the reply as OpenAI Responses events (`response.created`,\n\
    `response.output_item.added`, `response.reasoning_summary_text.delta`,\n\
    `response.output_text.delta`, `response.completed` with usage, `error`),\n\
    with keepalive comments while the model is quiet. Closing the stream\n\
    cancels the provider request. The request counts against the member's\n\
    and the project's limits and budgets, is charged to the project, and is\n\
    logged with the source `playground`. Needs a console session; failures\n\
    before the stream starts are Platform API errors (`provider_error` when\n\
    the provider fails, refuses the provider key, is overloaded or times out).",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    request_body = PlaygroundRequest,
    responses(
        (status = 200, description = "OpenAI Responses stream events.", content_type = "text/event-stream", body = String),
        (status = 502, description = "The provider failed or refused the provider key (`provider_error`).", body = crate::platform::error::ErrorEnvelope),
        (status = 504, description = "The provider did not answer in time (`provider_error`).", body = crate::platform::error::ErrorEnvelope),
    )
)]
pub async fn responses(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    body: axum::body::Body,
) -> ApiResult<Response> {
    let Caller::Session(session) = &ctx.caller else {
        return Err(ApiError::PermissionDenied(
            "The Playground needs a console session; apps call models with a key on `/v1`.".into(),
        ));
    };
    let scope = ctx.project()?;
    if scope.project.archived_at.is_some() {
        return Err(ApiError::Conflict(
            "The project is archived; its models cannot be called.".into(),
        ));
    }
    let mut headers = HeaderMap::new();
    if let Some(ua) = ctx
        .user_agent
        .as_deref()
        .and_then(|ua| HeaderValue::from_str(ua).ok())
    {
        headers.insert(header::USER_AGENT, ua);
    }
    let incoming = Incoming {
        ingress: DialectId::OpenAiResponses,
        kind: RequestKind::Generate,
        headers,
        body,
        request_id: ctx.request_id,
        client_ip: ctx.ip,
        caller: Some(ModelCaller {
            org_id: scope.project.org_id,
            project_id: scope.project.id,
            key_id: None,
            user_id: Some(session.user_id),
            service_account_id: None,
        }),
        source: Source::Playground,
        stream_only: true,
    };
    Ok(pipeline::serve_with(state, incoming, answer).await)
}

fn answer(f: &Failure) -> Response {
    let mut resp = refusal(f).into_response();
    stamp(&mut resp, &f.served);
    resp
}

fn own(message: &str, fallback: &str) -> String {
    let fits = message.chars().count() <= 120
        && message.ends_with('.')
        && !message.contains('\n')
        && !several_sentences(message);
    if fits { message } else { fallback }.to_owned()
}

fn several_sentences(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let mut in_code = false;
    for (i, c) in chars.iter().enumerate() {
        match c {
            '`' => in_code = !in_code,
            '.' | '!' | '?' if !in_code => {
                let rest = chars.get(i + 1..).unwrap_or_default();
                let gap = rest.iter().take_while(|c| c.is_whitespace()).count();
                if gap > 0
                    && rest
                        .get(gap)
                        .is_some_and(|n| n.is_uppercase() || matches!(n, '`' | '{' | '('))
                {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

fn retry_after(e: &KppError) -> Option<u64> {
    e.retry_after.map(|d| d.as_secs().max(1))
}

fn provider_error(status: StatusCode, message: &str, e: &KppError) -> ApiError {
    ApiError::Provider {
        status,
        message: message.to_owned(),
        retry_after: retry_after(e),
    }
}

fn refusal(f: &Failure) -> ApiError {
    let e = &f.err;
    match &f.stage {
        Stage::Request if e.kind == ErrorKind::RequestTooLarge => ApiError::TooLarge(own(
            &e.message,
            "The conversation is larger than Kaeso accepts.",
        )),
        Stage::Request => ApiError::InvalidRequest {
            message: own(
                &e.message,
                "The request is not a Responses request Kaeso can read.",
            ),
            param: e.param.clone(),
        },
        Stage::Catalogue => ApiError::Unavailable(
            "Kaeso cannot load this project's models right now; try again shortly.".into(),
        ),
        Stage::UnknownModel(m) => ApiError::invalid(
            "model",
            format!("The model `{m}` is not in this project's catalogue."),
        ),
        Stage::AmbiguousModel(m) => ApiError::invalid(
            "model",
            format!("Several providers serve `{m}` here; name it as `provider/{m}`."),
        ),
        Stage::ModelAccess(m) => ApiError::PermissionDenied(format!(
            "The model `{m}` is not among the models you may use in this project."
        )),
        Stage::Capability => ApiError::InvalidRequest {
            message: own(
                &e.message,
                "The model cannot take this request as it is; check its settings.",
            ),
            param: e.param.clone(),
        },
        Stage::Limits if e.status == Some(503) => ApiError::Unavailable(
            "Kaeso cannot check this project's budgets right now; try again shortly.".into(),
        ),
        Stage::Limits => ApiError::RateLimited {
            message: own(
                &e.message,
                "A limit of this project refused the request; try again later.",
            ),
            retry_after: retry_after(e),
        },
        Stage::MissingCredential(p) => ApiError::Conflict(format!(
            "No provider key for {} is set up for this project; an org admin can add one.",
            provider::provider(p).map_or(*p, |p| p.name)
        )),
        Stage::Credential if e.status == Some(503) => ApiError::Unavailable(
            "Kaeso cannot load the project's provider key right now; try again shortly.".into(),
        ),
        Stage::Upstream if e.kind == ErrorKind::Timeout => provider_error(
            StatusCode::GATEWAY_TIMEOUT,
            "The provider did not answer in time; send the request again.",
            e,
        ),
        Stage::Upstream => provider_error(
            StatusCode::BAD_GATEWAY,
            "Kaeso cannot reach the provider right now; try again shortly.",
            e,
        ),
        Stage::Provider => from_provider(e),
        Stage::Reply => provider_error(
            StatusCode::BAD_GATEWAY,
            "The provider's answer could not be read; try again.",
            e,
        ),
        Stage::Shutdown => ApiError::Unavailable(
            "Kaeso is restarting and ended this request; send it again.".into(),
        ),
        Stage::Caller | Stage::Setup | Stage::Credential => {
            ApiError::internal(format!("playground request failed: {}", e.message))
        }
    }
}

fn from_provider(e: &KppError) -> ApiError {
    match &e.kind {
        ErrorKind::ContextTooLong => ApiError::invalid(
            "input",
            "The conversation is longer than the model's context window.",
        ),
        ErrorKind::ContentFiltered => {
            ApiError::bad_request("The provider's content filter refused the request.")
        }
        ErrorKind::RequestTooLarge => {
            ApiError::TooLarge("The provider refused the request as too large.".into())
        }
        ErrorKind::InvalidRequest | ErrorKind::NotFound | ErrorKind::Unsupported { .. } => {
            ApiError::InvalidRequest {
                message: "The provider refused the request as invalid; check the model's settings."
                    .into(),
                param: e.param.clone(),
            }
        }
        ErrorKind::Authentication | ErrorKind::PermissionDenied => provider_error(
            StatusCode::BAD_GATEWAY,
            "The provider refused the project's provider key; an org admin can replace it.",
            e,
        ),
        ErrorKind::Billing => provider_error(
            StatusCode::BAD_GATEWAY,
            "The provider refused the request for billing reasons; check the provider account.",
            e,
        ),
        ErrorKind::RateLimited => provider_error(
            StatusCode::TOO_MANY_REQUESTS,
            "The provider is rate limiting the provider key; try again shortly.",
            e,
        ),
        ErrorKind::Overloaded => provider_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "The provider is overloaded right now; try again shortly.",
            e,
        ),
        ErrorKind::Timeout => provider_error(
            StatusCode::GATEWAY_TIMEOUT,
            "The provider did not answer in time; send the request again.",
            e,
        ),
        _ => provider_error(
            StatusCode::BAD_GATEWAY,
            "The provider failed to answer; try again shortly.",
            e,
        ),
    }
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "A model the member can call in the Playground.")]
pub struct PlaygroundModel {
    #[doc = "The catalogue entry."]
    pub id: CatalogueModelId,
    #[doc = "The name to send as `model`: the provider's model id, or\n\
        `provider/model` when several providers in the project serve it."]
    #[schema(example = "claude-sonnet-4-5")]
    pub model: String,
    #[doc = "The provider."]
    #[schema(example = "anthropic")]
    pub provider: String,
    #[doc = "The provider's model id."]
    pub model_id: String,
    #[doc = "The name to show."]
    #[schema(example = "Claude Sonnet 4.5")]
    pub display_name: String,
    #[doc = "The project's aliases pointing at it."]
    pub aliases: Vec<String>,
    #[doc = "Whether the model reasons: the Playground offers reasoning effort\n\
        and summaries only then."]
    pub reasoning: bool,
    #[doc = "The reasoning efforts it supports; empty when not listed."]
    pub efforts: Vec<String>,
    #[doc = "Whether it accepts `temperature`."]
    pub temperature: bool,
    #[doc = "Accepted inputs, e.g. `text`, `image`."]
    pub input_modalities: Vec<String>,
    #[doc = "The context window in tokens."]
    pub context_limit: Option<u64>,
    #[doc = "The output limit in tokens."]
    pub output_limit: Option<u64>,
    #[doc = "USD per million uncached input tokens."]
    #[schema(value_type = Option<String>, example = "3")]
    pub input_price: Option<String>,
    #[doc = "USD per million output tokens (reasoning included)."]
    #[schema(value_type = Option<String>, example = "15")]
    pub output_price: Option<String>,
    #[doc = "USD per million cache-read tokens."]
    #[schema(value_type = Option<String>)]
    pub cache_read_price: Option<String>,
    #[doc = "USD per million cache-write tokens."]
    #[schema(value_type = Option<String>)]
    pub cache_write_price: Option<String>,
    #[doc = "Whether the project has a provider key for its provider (assigned to\n\
        the project, or the org's default); without one, requests fail."]
    pub provider_key: bool,
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/playground/models",
    summary = "The models the signed-in member can call in the Playground.",
    description = "The project's catalogue without embeddings models, narrowed by the member's\n\
    model access, with what the Playground shows and offers (reasoning,\n\
    efforts, temperature, prices). A reply's cost is `(input − cached) ×\n\
    input_price + cached × cache_read_price + output × output_price`, per\n\
    million tokens, as the request log charges it (Anthropic cache writes,\n\
    which Responses usage does not show, are in the log's cost only).",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    responses((status = 200, body = List<PlaygroundModel>))
)]
pub async fn models(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
) -> ApiResult<axum::Json<List<PlaygroundModel>>> {
    let Caller::Session(session) = &ctx.caller else {
        return Err(ApiError::PermissionDenied(
            "The Playground needs a console session; apps list models with a key on `/v1`.".into(),
        ));
    };
    let project = &ctx.project()?.project;
    let models = crate::services::catalogue::project_models(&state, project.id).await?;
    let access = models.access(None, Some(session.user_id), None);
    let mut keys: Vec<(&str, bool)> = Vec::new();
    let mut out = Vec::new();
    for entry in models.entries.iter().filter(|e| access.allows(e.id)) {
        let Some(eff) = crate::services::catalogue::effective(&state, entry) else {
            continue;
        };
        if eff.is_embeddings() {
            continue;
        }
        let provider_key = match keys.iter().find(|(p, _)| *p == eff.provider.id) {
            Some((_, has)) => *has,
            None => {
                let has =
                    credentials::resolve(&state.pg, project.org_id, project.id, eff.provider.id)
                        .await?
                        .is_some();
                keys.push((eff.provider.id, has));
                has
            }
        };
        let shared = models
            .entries
            .iter()
            .filter(|e| e.model_id == entry.model_id)
            .count()
            > 1;
        let f = &eff.fields;
        let price = |p: Option<rust_decimal::Decimal>| p.map(|d| d.normalize().to_string());
        out.push(PlaygroundModel {
            id: CatalogueModelId::from_uuid(entry.id),
            model: if shared {
                format!("{}/{}", entry.provider, entry.model_id)
            } else {
                entry.model_id.clone()
            },
            provider: entry.provider.clone(),
            model_id: entry.model_id.clone(),
            display_name: eff.display_name().to_owned(),
            aliases: models
                .aliases
                .iter()
                .filter(|a| a.target == entry.id)
                .map(|a| a.name.clone())
                .collect(),
            reasoning: f.reasoning.unwrap_or(false),
            efforts: f.efforts.clone().unwrap_or_default(),
            temperature: f.temperature.unwrap_or(true),
            input_modalities: f.input_modalities.clone().unwrap_or_default(),
            context_limit: f.context_limit,
            output_limit: f.output_limit,
            input_price: price(f.input_price),
            output_price: price(f.output_price),
            cache_read_price: price(f.cache_read_price),
            cache_write_price: price(f.cache_write_price),
            provider_key,
        });
    }
    Ok(axum::Json(List::complete(out)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_one_short_sentence_is_shown() {
        let fallback = "Fallback.";
        for kept in [
            "Rate limit reached: the member's 2 requests per minute; retry in 5 seconds.",
            "`temperature` must be between 0 and 2.",
            "Budget used up: the project's $1.50 per month; it resets at 2026-11-01T00:00:00Z.",
        ] {
            assert_eq!(own(kept, fallback), kept);
        }
        for replaced in [
            "Two sentences. Here.",
            "A sentence. `code` starts another.",
            "No period at the end",
            "A line\nbreak.",
            "Kaeso cannot reach the provider (connect). Try again.",
            &"Long. ".repeat(30),
        ] {
            assert_eq!(own(replaced, fallback), fallback, "{replaced}");
        }
    }

    fn failure(stage: Stage, err: KppError) -> Failure {
        Failure {
            err,
            served: crate::model_api::errors::Served::default(),
            stage,
        }
    }

    #[test]
    fn provider_failures_are_provider_errors_with_their_status() {
        let cases = [
            (ErrorKind::Authentication, 502),
            (ErrorKind::Billing, 502),
            (ErrorKind::RateLimited, 429),
            (ErrorKind::Overloaded, 503),
            (ErrorKind::Timeout, 504),
            (ErrorKind::ProviderError, 502),
        ];
        for (kind, status) in cases {
            let e = refusal(&failure(
                Stage::Provider,
                KppError::new(kind.clone(), "Provider text. With details."),
            ));
            assert_eq!(e.kind(), "provider_error", "{kind:?}");
            assert_eq!(e.status().as_u16(), status, "{kind:?}");
            assert!(!e.to_string().contains("Provider text"), "{kind:?}");
        }
        let e = refusal(&failure(
            Stage::Provider,
            KppError::new(ErrorKind::ContextTooLong, "Too long. Really."),
        ));
        assert_eq!((e.kind(), e.status().as_u16()), ("invalid_request", 400));
        let timeout = refusal(&failure(
            Stage::Upstream,
            KppError::new(ErrorKind::Timeout, "x"),
        ));
        assert_eq!(timeout.status().as_u16(), 504);
    }

    #[test]
    fn limits_keep_their_retry_and_budgets_have_none() {
        let rate = refusal(&failure(
            Stage::Limits,
            KppError::new(
                ErrorKind::RateLimited,
                "Rate limit reached: the project's 1 request per minute; retry in 30 seconds.",
            )
            .with_status(429)
            .with_retry_after(std::time::Duration::from_secs(30)),
        ));
        assert!(matches!(
            rate,
            ApiError::RateLimited {
                retry_after: Some(30),
                ..
            }
        ));
        let unchecked = refusal(&failure(
            Stage::Limits,
            KppError::new(ErrorKind::ProviderError, "x").with_status(503),
        ));
        assert_eq!(unchecked.kind(), "unavailable");
    }
}
