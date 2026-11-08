mod accounts;
pub mod auth;
pub mod errors;
pub mod gemini;
pub mod hooks;
pub mod in_flight;
mod input_count;
pub mod listing;
mod media;
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
use kaeso_kpp::{DialectId, ErrorKind, ImageOperation, RequestKind};

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
    serve_call(state, ingress, kind, request_id, client_ip, req, None).await
}

async fn serve_call(
    state: AppState,
    ingress: DialectId,
    kind: RequestKind,
    request_id: RequestId,
    client_ip: ClientIp,
    req: Request,
    image: Option<ImageOperation>,
) -> Response {
    let (parts, body) = req.into_parts();
    let max_body = if image.is_some() {
        state.config.body_limits.images.as_usize()
    } else {
        state.config.body_limits.model_api.as_usize()
    };
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
            alias: None,
            gemini: None,
            image,
        },
    )
    .await
}

macro_rules! image_endpoint {
    ($name:ident, $operation:expr) => {
        async fn $name(
            State(state): State<AppState>,
            Extension(id): Extension<RequestId>,
            ip: ClientIp,
            req: Request,
        ) -> Response {
            let kind = RequestKind::Image;
            serve_call(
                state,
                DialectId::OpenAiImages,
                kind,
                id,
                ip,
                req,
                Some($operation),
            )
            .await
        }
    };
}

image_endpoint!(image_generations, ImageOperation::Generate);
image_endpoint!(image_edits, ImageOperation::Edit);
image_endpoint!(image_variations, ImageOperation::Variation);

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

async fn models_post(
    OriginalUri(uri): OriginalUri,
    method: Method,
    headers: HeaderMap,
) -> Response {
    let raw = uri.path().strip_prefix("/v1/models/").unwrap_or("");
    gemini::on_v1(raw).unwrap_or_else(|| not_allowed(&method, &headers, uri.path()))
}

async fn method_not_allowed(
    method: Method,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    _body: Body,
) -> Response {
    if gemini::is_gemini_path(uri.path()) {
        return gemini::method_not_allowed(uri.path(), &method);
    }
    not_allowed(&method, &headers, uri.path())
}

fn not_allowed(method: &Method, headers: &HeaderMap, path: &str) -> Response {
    let mut resp = respond(
        header_dialect(headers),
        &error(
            ErrorKind::InvalidRequest,
            405,
            format!("`{path}` does not accept `{method}` requests."),
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
        .route("/v1/images/generations", post(image_generations))
        .route("/v1/images/edits", post(image_edits))
        .route("/v1/images/variations", post(image_variations))
        .route("/v1/messages", post(messages))
        .route("/v1/messages/count_tokens", post(messages_count_tokens))
        .route("/v1/models", get(models_list))
        .route("/v1/models/{*id}", get(models_get).post(models_post))
        .route("/v1", axum::routing::any(not_found))
        .route("/v1/{*rest}", axum::routing::any(not_found))
        .merge(gemini::router())
        .method_not_allowed_fallback(method_not_allowed)
}
