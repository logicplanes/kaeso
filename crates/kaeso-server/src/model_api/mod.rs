pub(crate) mod accounts;
pub mod auth;
mod elevenlabs;
pub mod errors;
pub mod gemini;
mod generations;
pub mod hooks;
pub mod in_flight;
mod input_count;
mod links;
pub mod listing;
mod media;
pub mod memory;
pub mod pipeline;
mod realtime;
mod stream;
pub mod videos;

use axum::Router;
use axum::body::Body;
use axum::extract::{Extension, OriginalUri, Request, State};
use axum::http::{HeaderMap, Method, StatusCode};
use axum::response::Response;
use axum::routing::{get, post};
use kaeso_codecs::dispatch;
use kaeso_core::id::RequestId;
use kaeso_kpp::{DialectId, ErrorKind, ImageOperation, RequestKind, TranscriptionOperation};

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
    serve_call(
        state,
        ingress,
        kind,
        request_id,
        client_ip,
        req,
        Media::None,
    )
    .await
}

/// The operation a media endpoint's path names.
#[derive(Clone, Copy)]
enum Media {
    None,
    Image(ImageOperation),
    Transcription(TranscriptionOperation),
}

async fn serve_call(
    state: AppState,
    ingress: DialectId,
    kind: RequestKind,
    request_id: RequestId,
    client_ip: ClientIp,
    req: Request,
    media: Media,
) -> Response {
    let (parts, body) = req.into_parts();
    let limits = &state.config.body_limits;
    let (max_body, image, transcription) = match media {
        Media::Image(op) => (limits.images.as_usize(), Some(op), None),
        Media::Transcription(op) => (limits.audio.as_usize(), None, Some(op)),
        Media::None => (limits.model_api.as_usize(), None, None),
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
            transcription,
            elevenlabs: None,
            video: None,
            generation: None,
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
                Media::Image($operation),
            )
            .await
        }
    };
}

macro_rules! transcription_endpoint {
    ($name:ident, $operation:expr) => {
        async fn $name(
            State(state): State<AppState>,
            Extension(id): Extension<RequestId>,
            ip: ClientIp,
            req: Request,
        ) -> Response {
            serve_call(
                state,
                DialectId::OpenAiTranscriptions,
                RequestKind::Transcription,
                id,
                ip,
                req,
                Media::Transcription($operation),
            )
            .await
        }
    };
}

transcription_endpoint!(audio_transcriptions, TranscriptionOperation::Transcribe);
transcription_endpoint!(audio_translations, TranscriptionOperation::Translate);

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
endpoint!(audio_speech, DialectId::OpenAiSpeech, RequestKind::Speech);
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
    if header_dialect(&headers) == DialectId::ElevenLabs {
        return elevenlabs::management(OriginalUri(uri)).await;
    }
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
        .route("/v1/audio/speech", post(audio_speech))
        .route("/v1/audio/transcriptions", post(audio_transcriptions))
        .route("/v1/audio/translations", post(audio_translations))
        .route(
            "/v1/text-to-speech/{*rest}",
            post(elevenlabs::generate).get(realtime::socket),
        )
        .route("/v1/text-to-dialogue", post(elevenlabs::generate))
        .route(
            "/v1/text-to-dialogue/{*rest}",
            post(elevenlabs::generate).get(realtime::socket),
        )
        .route("/v1/sound-generation", post(elevenlabs::generate))
        .route("/v1/music", post(elevenlabs::generate))
        .route("/v1/music/{*rest}", post(elevenlabs::generate))
        .route("/v1/voice-changer/{*rest}", post(elevenlabs::generate))
        .route("/v1/speech-to-text", post(elevenlabs::generate))
        .route("/v1/speech-to-text/realtime", get(realtime::socket))
        .route("/v1/voices", get(elevenlabs::voices))
        .route("/v1/voices/{*rest}", get(elevenlabs::voices))
        .route("/v2/voices", get(elevenlabs::voices))
        .route("/v2/{*rest}", axum::routing::any(elevenlabs::management))
        .route(
            "/v1/flows/image",
            post(generations::create).get(generations::list),
        )
        .route(
            "/v1/flows/image/{generation_id}",
            get(generations::retrieve),
        )
        .route("/v1/videos", post(videos::create).get(videos::list))
        .route("/v1/videos/edits", post(videos::not_served))
        .route("/v1/videos/extensions", post(videos::not_served))
        .route(
            "/v1/videos/characters",
            axum::routing::any(videos::not_served),
        )
        .route(
            "/v1/videos/characters/{*rest}",
            axum::routing::any(videos::not_served),
        )
        .route(
            "/v1/videos/{video_id}",
            get(videos::retrieve).delete(videos::delete),
        )
        .route("/v1/videos/{video_id}/content", get(videos::content))
        .route("/v1/videos/{video_id}/remix", post(videos::remix))
        .route("/v1/messages", post(messages))
        .route("/v1/messages/count_tokens", post(messages_count_tokens))
        .route("/v1/models", get(models_list))
        .route("/v1/models/{*id}", get(models_get).post(models_post))
        .route("/v1", axum::routing::any(not_found))
        .route("/v1/{*rest}", axum::routing::any(not_found))
        .merge(gemini::router())
        .method_not_allowed_fallback(method_not_allowed)
}
