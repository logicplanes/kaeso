mod accounts;
pub mod auth;
pub mod errors;
pub mod hooks;
pub mod in_flight;
mod input_count;
pub mod listing;
pub mod memory;
pub mod pipeline;
mod stream;

use axum::Router;
use axum::body::Body;
use axum::extract::{Extension, OriginalUri, Request, State};
use axum::http::{HeaderMap, Method, StatusCode};
use axum::response::Response;
use axum::routing::{get, post};
use kaeso_codecs::dispatch;
use kaeso_core::id::RequestId;
use kaeso_kpp::{DialectId, ErrorKind, RequestKind};

use self::errors::{Served, error, respond};
use self::pipeline::Incoming;
use crate::http::client_ip::ClientIp;
use crate::state::AppState;

async fn serve(
    state: AppState,
    ingress: DialectId,
    kind: RequestKind,
    request_id: RequestId,
    client_ip: ClientIp,
    req: Request,
) -> Response {
    let (parts, body) = req.into_parts();
    let max_body = state.config.body_limits.model_api.as_usize();
    pipeline::serve(
        state,
        Incoming {
            ingress,
            kind,
            headers: parts.headers,
            body,
            request_id,
            client_ip: client_ip.0,
            caller: None,
            source: hooks::Source::Api,
            stream_only: false,
            max_body,
        },
    )
    .await
}

macro_rules! endpoint {
    ($name:ident, $dialect:expr, $kind:expr) => {
        async fn $name(
            State(state): State<AppState>,
            Extension(id): Extension<RequestId>,
            ip: ClientIp,
            req: Request,
        ) -> Response {
            serve(state, $dialect, $kind, id, ip, req).await
        }
    };
}

endpoint!(
    chat_completions,
    DialectId::OpenAiChat,
    RequestKind::Generate
);
endpoint!(responses, DialectId::OpenAiResponses, RequestKind::Generate);
endpoint!(
    responses_input_tokens,
    DialectId::OpenAiResponses,
    RequestKind::CountTokens
);
endpoint!(embeddings, DialectId::OpenAiEmbeddings, RequestKind::Embed);
endpoint!(
    messages,
    DialectId::AnthropicMessages,
    RequestKind::Generate
);
endpoint!(
    messages_count_tokens,
    DialectId::AnthropicMessages,
    RequestKind::CountTokens
);

async fn models_list(
    State(state): State<AppState>,
    ip: ClientIp,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
) -> Response {
    listing::list(&state, &headers, ip.0, uri.query().unwrap_or("")).await
}

async fn models_get(
    State(state): State<AppState>,
    ip: ClientIp,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let raw = uri.path().strip_prefix("/v1/models/").unwrap_or("");
    let id = percent_encoding::percent_decode_str(raw)
        .decode_utf8_lossy()
        .into_owned();
    listing::get(&state, &headers, ip.0, &id).await
}

fn header_dialect(headers: &HeaderMap) -> DialectId {
    dispatch::models_dialect(&pipeline::header_list(headers))
}

async fn not_found(headers: HeaderMap, OriginalUri(uri): OriginalUri, _body: Body) -> Response {
    respond(
        header_dialect(&headers),
        &error(
            ErrorKind::NotFound,
            404,
            format!("Kaeso does not serve `{}`.", uri.path()),
        ),
        &Served::default(),
    )
}

async fn method_not_allowed(
    method: Method,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    _body: Body,
) -> Response {
    let mut resp = respond(
        header_dialect(&headers),
        &error(
            ErrorKind::InvalidRequest,
            405,
            format!("`{}` does not accept `{method}` requests.", uri.path()),
        ),
        &Served::default(),
    );
    *resp.status_mut() = StatusCode::METHOD_NOT_ALLOWED;
    resp
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/responses", post(responses))
        .route("/v1/responses/input_tokens", post(responses_input_tokens))
        .route("/v1/embeddings", post(embeddings))
        .route("/v1/messages", post(messages))
        .route("/v1/messages/count_tokens", post(messages_count_tokens))
        .route("/v1/models", get(models_list))
        .route("/v1/models/{*id}", get(models_get))
        .route("/v1", axum::routing::any(not_found))
        .route("/v1/{*rest}", axum::routing::any(not_found))
        .method_not_allowed_fallback(method_not_allowed)
}
