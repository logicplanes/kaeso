//! The Playground's media and decision requests: images, speech, sound effects, music,
//! transcripts, videos and decisions, sent as the signed-in member through the model pipeline in
//! the formats apps use (OpenAI's Images, Audio and Videos APIs, ElevenLabs' sound effects and
//! music, TypeSafe's System One API), charged to the project and logged with the source
//! `playground`.

use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Response};
use kaeso_codecs::elevenlabs::ElevenLabsCall;
use kaeso_codecs::openai_images::{EditsForm, SizeForm};
use kaeso_core::catalogue::Effective;
use kaeso_core::id::{OrgId, ProjectId};
use kaeso_kpp::{
    DialectId, ErrorKind, ImageOperation, KppError, RequestKind, TranscriptionOperation,
};
use serde::Serialize;
use utoipa::ToSchema;

use super::playground::{Ask, answer_with};
use crate::config::BodyLimitsConfig;
use crate::model_api::hooks::{Caller as ModelCaller, Source, VideoCall};
use crate::model_api::pipeline::{self, Incoming};
use crate::model_api::videos::{self, VideoIncoming, Viewer};
use crate::platform::access::{Caller, Ctx};
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::params::Path;
use crate::state::AppState;

/// What a model makes in the Playground, and so the form its column takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlaygroundMode {
    /// A conversation: text in, text out, streamed.
    Text,
    /// Images from a prompt, or edits of uploaded images.
    Image,
    /// Speech from text, in a voice.
    Speech,
    /// A sound effect from a description.
    SoundEffects,
    /// Music from a description.
    Music,
    /// Text from uploaded or recorded audio.
    Transcription,
    /// A video from a prompt, made as a job to wait for.
    Video,
    /// Answers to typed questions about a state, each with its probabilities.
    Decision,
}

/// How the Playground sizes images for this model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlaygroundImageSize {
    /// Pixel sizes, such as `1024x1024`.
    Pixels,
    /// No size: the model chooses.
    None,
}

/// What an image model takes in the Playground.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, ToSchema)]
pub struct PlaygroundImageOptions {
    #[doc = "Whether it edits uploaded images (`images/edits`)."]
    pub edits: bool,
    #[doc = "The most images one request may ask for; 1 when it makes one at a time."]
    pub max_count: u32,
    #[doc = "The `quality` values it takes; empty when it takes none."]
    pub qualities: Vec<String>,
    #[doc = "Whether it takes a pixel size."]
    pub size: PlaygroundImageSize,
}

/// What a model that makes decisions takes in the Playground, through its provider's decisions
/// API.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, ToSchema)]
pub struct PlaygroundDecisionOptions {
    #[doc = "The question types it answers: `predicate`, `choice`, `score`, and `box` where the\n\
        provider takes box questions."]
    pub question_types: Vec<String>,
    #[doc = "The most questions one request may ask; `null` when no limit is known."]
    pub max_questions: Option<u32>,
    #[doc = "The most options one choice question may have; `null` when no limit is known."]
    pub max_options: Option<u32>,
    #[doc = "The most levels one score question may have; `null` when no limit is known."]
    pub max_levels: Option<u32>,
    #[doc = "Whether the state may hold images."]
    pub images: bool,
    #[doc = "The most images one request may show; `null` when no limit is known."]
    pub max_images: Option<u32>,
    #[doc = "The largest decision request Kaeso takes, in bytes, images included as sent."]
    #[schema(example = 67108864)]
    pub max_request_bytes: u64,
    #[doc = "Whether the project's provider key is an account that can't call the provider's\n\
        decisions API, such as a ChatGPT account; decisions fail until the project uses an API key."]
    pub needs_api_key: bool,
}

const QUESTION_TYPES: [&str; 4] = ["predicate", "choice", "score", "box"];

fn least(a: Option<u32>, b: Option<usize>) -> Option<u32> {
    let b = b.map(|b| u32::try_from(b).unwrap_or(u32::MAX));
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

// The catalogue's limits for the model, narrowed by what its provider's decisions API takes.
fn decision_options(
    limits: &BodyLimitsConfig,
    eff: &Effective,
) -> Option<PlaygroundDecisionOptions> {
    let fields = eff.fields.decisions.as_ref()?;
    let egress = eff.provider.decisions(&eff.model_id)?;
    let system_one = (egress.dialect == DialectId::SystemOne).then_some(egress.system_one);
    Some(PlaygroundDecisionOptions {
        question_types: QUESTION_TYPES
            .iter()
            .filter(|t| fields.answers(t) && (**t != "box" || egress.box_questions()))
            .map(|t| (*t).to_owned())
            .collect(),
        max_questions: least(fields.max_questions, egress.max_questions()),
        max_options: least(fields.max_options, system_one.and_then(|p| p.max_options)),
        max_levels: least(fields.max_levels, system_one.and_then(|p| p.max_levels)),
        images: eff.accepts("image"),
        max_images: least(fields.max_images, system_one.and_then(|p| p.max_images)),
        max_request_bytes: limits.decisions.0,
        needs_api_key: false,
    })
}

const DEFAULT_MAX_IMAGES: u32 = 10;

// A profile that takes every `quality` is OpenAI's own, whose GPT Image models take these
// (`CreateImageRequest.quality` at openai-openapi `926a7b8`).
const GPT_IMAGE_QUALITIES: &[&str] = &["auto", "low", "medium", "high"];

fn image_options(eff: &Effective, accepts_images: bool) -> Option<PlaygroundImageOptions> {
    let egress = eff.provider.images(&eff.model_id)?;
    let openai = egress.dialect == DialectId::OpenAiImages;
    let p = egress.profile;
    let edits = accepts_images && !(openai && p.edits == EditsForm::None);
    Some(PlaygroundImageOptions {
        edits,
        max_count: if openai && p.count {
            p.max_count.unwrap_or(DEFAULT_MAX_IMAGES)
        } else {
            1
        },
        qualities: if openai {
            p.qualities
                .unwrap_or(GPT_IMAGE_QUALITIES)
                .iter()
                .map(|s| (*s).to_owned())
                .collect()
        } else {
            Vec::new()
        },
        size: if openai && matches!(p.sizes, SizeForm::Size | SizeForm::WidthHeight) {
            PlaygroundImageSize::Pixels
        } else {
            PlaygroundImageSize::None
        },
    })
}

/// The voices a speech model's provider documents, when it names them.
fn voices(eff: &Effective) -> Vec<String> {
    eff.provider
        .speech(&eff.model_id)
        .filter(|s| s.dialect == DialectId::OpenAiSpeech)
        .and_then(|s| s.profile.voices)
        .map(|v| v.iter().map(|s| (*s).to_owned()).collect())
        .unwrap_or_default()
}

fn translates(eff: &Effective) -> bool {
    eff.provider.transcriptions(&eff.model_id).is_some_and(|t| {
        t.dialect == DialectId::OpenAiTranscriptions && t.profile.translations_path.is_some()
    })
}

// ElevenLabs takes `duration_seconds` and `music_length_ms`; QwenCloud's sound effects model,
// which the same request reaches, has no field for a length.
fn takes_lengths(eff: &Effective) -> bool {
    eff.provider
        .speech(&eff.model_id)
        .is_some_and(|s| s.dialect == DialectId::ElevenLabs)
}

fn has(list: Option<&Vec<String>>, modality: &str) -> bool {
    list.is_some_and(|l| l.iter().any(|m| m == modality))
}

/// The model's mode: its kind in the catalogue where it has one, otherwise what it takes and
/// makes and which of its provider's APIs serve it. `None` for a model the Playground can't try,
/// such as embeddings, rerankers and realtime models. A model that both generates text and makes
/// decisions is `Text`, with [`ModeOptions::decisions`] for the Playground's decision mode.
pub(super) fn mode_of(eff: &Effective, kind: Option<&str>) -> Option<PlaygroundMode> {
    if eff.decides_only() || kind == Some("decision") {
        let decides = eff.decides() && eff.provider.decisions(&eff.model_id).is_some();
        return decides.then_some(PlaygroundMode::Decision);
    }
    let f = &eff.fields;
    let (input, output) = (f.input_modalities.as_ref(), f.output_modalities.as_ref());
    let id = eff.model_id.as_str();
    let text_in = input.is_none_or(|l| l.iter().any(|m| m == "text"));
    let speaks = eff.provider.speech(id).is_some();
    let media = match kind {
        Some("image") if eff.provider.images(id).is_some() => Some(PlaygroundMode::Image),
        Some("video") if eff.provider.videos(id).is_some() => Some(PlaygroundMode::Video),
        Some("speech") if speaks && text_in => Some(PlaygroundMode::Speech),
        Some("sound_effects") if speaks && text_in => Some(PlaygroundMode::SoundEffects),
        Some("music") if speaks && text_in => Some(PlaygroundMode::Music),
        Some("transcription") if eff.provider.transcriptions(id).is_some() => {
            Some(PlaygroundMode::Transcription)
        }
        Some(
            "embedding" | "reranking" | "realtime" | "image" | "video" | "speech" | "sound_effects"
            | "music" | "transcription",
        ) => return None,
        Some(_) => None,
        None if has(output, "image") && eff.provider.images(id).is_some() => {
            Some(PlaygroundMode::Image)
        }
        None if has(output, "video") && eff.provider.videos(id).is_some() => {
            Some(PlaygroundMode::Video)
        }
        None if has(output, "audio") && speaks && text_in => Some(PlaygroundMode::Speech),
        None if has(input, "audio")
            && !has(input, "text")
            && eff.provider.transcriptions(id).is_some() =>
        {
            Some(PlaygroundMode::Transcription)
        }
        None => None,
    };
    if media.is_some() {
        return media;
    }
    let chats = |l: Option<&Vec<String>>| l.is_none_or(|l| l.iter().any(|m| m == "text"));
    (!eff.is_embeddings() && chats(input) && chats(output)).then_some(PlaygroundMode::Text)
}

/// What the Playground offers a model of each mode.
pub(super) struct ModeOptions {
    pub image: Option<PlaygroundImageOptions>,
    pub voices: Vec<String>,
    pub translations: bool,
    pub durations: bool,
    pub decisions: Option<PlaygroundDecisionOptions>,
    pub max_request_bytes: u64,
}

pub(super) fn options(
    limits: &BodyLimitsConfig,
    eff: &Effective,
    mode: PlaygroundMode,
) -> ModeOptions {
    let accepts_images = has(eff.fields.input_modalities.as_ref(), "image");
    ModeOptions {
        image: (mode == PlaygroundMode::Image)
            .then(|| image_options(eff, accepts_images))
            .flatten(),
        voices: if mode == PlaygroundMode::Speech {
            voices(eff)
        } else {
            Vec::new()
        },
        translations: mode == PlaygroundMode::Transcription && translates(eff),
        durations: matches!(mode, PlaygroundMode::SoundEffects | PlaygroundMode::Music)
            && takes_lengths(eff),
        decisions: matches!(mode, PlaygroundMode::Text | PlaygroundMode::Decision)
            .then(|| decision_options(limits, eff))
            .flatten(),
        max_request_bytes: match mode {
            PlaygroundMode::Text => limits.model_api.0,
            PlaygroundMode::Image => limits.images.0,
            PlaygroundMode::Transcription => limits.audio.0,
            PlaygroundMode::Decision => limits.decisions.0,
            PlaygroundMode::Speech
            | PlaygroundMode::SoundEffects
            | PlaygroundMode::Music
            | PlaygroundMode::Video => limits.platform_api.0,
        },
    }
}

const MEDIA: Ask = Ask {
    too_large: "The upload is larger than Kaeso accepts for this model.",
    unreadable: "Kaeso can't read this request for the model; check its settings.",
};

const DECISIONS: Ask = Ask {
    too_large: "The decision request is larger than Kaeso accepts for this model.",
    unreadable: "The request is not a System One request Kaeso can read.",
};

/// The pipeline's request for one Playground call.
struct Call {
    ingress: DialectId,
    kind: RequestKind,
    max_body: usize,
    image: Option<ImageOperation>,
    transcription: Option<TranscriptionOperation>,
    elevenlabs: Option<ElevenLabsCall>,
    video: Option<VideoIncoming>,
}

fn member(ctx: &Ctx) -> ApiResult<ModelCaller> {
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
    Ok(ModelCaller {
        org_id: scope.project.org_id,
        project_id: scope.project.id,
        key_id: None,
        user_id: Some(session.user_id),
        service_account_id: None,
    })
}

// The member's cookie and the console's own headers stay behind; the provider hears only what
// the body needs read.
fn passed(ctx: &Ctx, sent: &HeaderMap) -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Some(t) = sent.get(header::CONTENT_TYPE) {
        headers.insert(header::CONTENT_TYPE, t.clone());
    }
    if let Some(ua) = ctx
        .user_agent
        .as_deref()
        .and_then(|ua| HeaderValue::from_str(ua).ok())
    {
        headers.insert(header::USER_AGENT, ua);
    }
    headers
}

async fn send(
    state: AppState,
    ctx: &Ctx,
    headers: &HeaderMap,
    body: axum::body::Body,
    call: Call,
) -> ApiResult<Response> {
    send_as(state, ctx, headers, body, call, &MEDIA).await
}

async fn send_as(
    state: AppState,
    ctx: &Ctx,
    headers: &HeaderMap,
    body: axum::body::Body,
    call: Call,
    ask: &'static Ask,
) -> ApiResult<Response> {
    let caller = member(ctx)?;
    let incoming = Incoming {
        ingress: call.ingress,
        kind: call.kind,
        headers: passed(ctx, headers),
        body,
        request_id: ctx.request_id,
        client_ip: ctx.ip,
        caller: Some(caller),
        source: Source::Playground,
        stream_only: false,
        max_body: call.max_body,
        alias: None,
        gemini: None,
        image: call.image,
        transcription: call.transcription,
        elevenlabs: call.elevenlabs,
        video: call.video,
        generation: None,
    };
    Ok(pipeline::serve_with(state, incoming, answer_with(ask)).await)
}

fn image(state: &AppState, op: ImageOperation) -> Call {
    Call {
        ingress: DialectId::OpenAiImages,
        kind: RequestKind::Image,
        max_body: state.config.body_limits.images.as_usize(),
        image: Some(op),
        transcription: None,
        elevenlabs: None,
        video: None,
    }
}

fn transcription(state: &AppState, op: TranscriptionOperation) -> Call {
    Call {
        ingress: DialectId::OpenAiTranscriptions,
        kind: RequestKind::Transcription,
        max_body: state.config.body_limits.audio.as_usize(),
        image: None,
        transcription: Some(op),
        elevenlabs: None,
        video: None,
    }
}

fn elevenlabs(state: &AppState, path: &str) -> ApiResult<Call> {
    let call = ElevenLabsCall::from_path(path, "")
        .ok_or_else(|| ApiError::internal(format!("no ElevenLabs call for {path}")))?;
    Ok(Call {
        ingress: DialectId::ElevenLabs,
        kind: call.kind(),
        max_body: state.config.body_limits.platform_api.as_usize(),
        image: None,
        transcription: None,
        elevenlabs: Some(call),
        video: None,
    })
}

fn speech_call(state: &AppState) -> Call {
    Call {
        ingress: DialectId::OpenAiSpeech,
        kind: RequestKind::Speech,
        max_body: state.config.body_limits.platform_api.as_usize(),
        image: None,
        transcription: None,
        elevenlabs: None,
        video: None,
    }
}

fn video_call(state: &AppState) -> Call {
    Call {
        ingress: DialectId::OpenAiVideos,
        kind: RequestKind::Video,
        max_body: state.config.body_limits.platform_api.as_usize(),
        image: None,
        transcription: None,
        elevenlabs: None,
        video: Some(VideoIncoming {
            call: VideoCall::Create,
            remix: None,
        }),
    }
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/playground/images/generations",
    summary = "Makes images in the Playground as the signed-in member.",
    description = "An OpenAI `images/generations` request to one of the project's image models, as\n\
    the signed-in member: the reply is OpenAI's, with the images as `b64_json` or `url`.\n\
    Charged to the project and logged with the source `playground`; needs a console session.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    request_body(content = Object, content_type = "application/json"),
    responses(
        (status = 200, description = "The model API's reply, as an app gets it, with the `kaeso-cost` header."),
        (status = 413, description = "The body is larger than Kaeso takes for this model (`request_too_large`).", body = crate::platform::error::ErrorEnvelope),
        (status = 502, description = "The provider failed or refused the provider key (`provider_error`).", body = crate::platform::error::ErrorEnvelope),
    )
)]
pub async fn image_generations(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    headers: HeaderMap,
    body: axum::body::Body,
) -> ApiResult<Response> {
    let call = image(&state, ImageOperation::Generate);
    send(state, &ctx, &headers, body, call).await
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/playground/images/edits",
    summary = "Edits images in the Playground as the signed-in member.",
    description = "An OpenAI `images/edits` upload (`multipart/form-data` with `image[]`) to one of the\n\
    project's image models, as the signed-in member, up to `KAESO_BODY_LIMITS_IMAGES`.\n\
    Charged to the project and logged with the source `playground`; needs a console session.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    request_body(content = Object, content_type = "multipart/form-data"),
    responses(
        (status = 200, description = "The model API's reply, as an app gets it, with the `kaeso-cost` header."),
        (status = 413, description = "The body is larger than Kaeso takes for this model (`request_too_large`).", body = crate::platform::error::ErrorEnvelope),
        (status = 502, description = "The provider failed or refused the provider key (`provider_error`).", body = crate::platform::error::ErrorEnvelope),
    )
)]
pub async fn image_edits(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    headers: HeaderMap,
    body: axum::body::Body,
) -> ApiResult<Response> {
    let call = image(&state, ImageOperation::Edit);
    send(state, &ctx, &headers, body, call).await
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/playground/audio/speech",
    summary = "Speaks text in the Playground as the signed-in member.",
    description = "An OpenAI `audio/speech` request to one of the project's speech models, as the\n\
    signed-in member: the reply is the audio. Charged to the project and logged with the\n\
    source `playground`; needs a console session.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    request_body(content = Object, content_type = "application/json"),
    responses(
        (status = 200, description = "The model API's reply, as an app gets it, with the `kaeso-cost` header."),
        (status = 413, description = "The body is larger than Kaeso takes for this model (`request_too_large`).", body = crate::platform::error::ErrorEnvelope),
        (status = 502, description = "The provider failed or refused the provider key (`provider_error`).", body = crate::platform::error::ErrorEnvelope),
    )
)]
pub async fn speech(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    headers: HeaderMap,
    body: axum::body::Body,
) -> ApiResult<Response> {
    let call = speech_call(&state);
    send(state, &ctx, &headers, body, call).await
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/playground/audio/transcriptions",
    summary = "Transcribes audio in the Playground as the signed-in member.",
    description = "An OpenAI `audio/transcriptions` upload to one of the project's transcription models,\n\
    as the signed-in member, up to `KAESO_BODY_LIMITS_AUDIO`. Charged to the project and\n\
    logged with the source `playground`; needs a console session.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    request_body(content = Object, content_type = "multipart/form-data"),
    responses(
        (status = 200, description = "The model API's reply, as an app gets it, with the `kaeso-cost` header."),
        (status = 413, description = "The body is larger than Kaeso takes for this model (`request_too_large`).", body = crate::platform::error::ErrorEnvelope),
        (status = 502, description = "The provider failed or refused the provider key (`provider_error`).", body = crate::platform::error::ErrorEnvelope),
    )
)]
pub async fn transcriptions(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    headers: HeaderMap,
    body: axum::body::Body,
) -> ApiResult<Response> {
    let call = transcription(&state, TranscriptionOperation::Transcribe);
    send(state, &ctx, &headers, body, call).await
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/playground/audio/translations",
    summary = "Translates audio into English text in the Playground as the signed-in member.",
    description = "An OpenAI `audio/translations` upload to one of the project's transcription models\n\
    that translate, as the signed-in member, up to `KAESO_BODY_LIMITS_AUDIO`. Charged to the\n\
    project and logged with the source `playground`; needs a console session.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    request_body(content = Object, content_type = "multipart/form-data"),
    responses(
        (status = 200, description = "The model API's reply, as an app gets it, with the `kaeso-cost` header."),
        (status = 413, description = "The body is larger than Kaeso takes for this model (`request_too_large`).", body = crate::platform::error::ErrorEnvelope),
        (status = 502, description = "The provider failed or refused the provider key (`provider_error`).", body = crate::platform::error::ErrorEnvelope),
    )
)]
pub async fn translations(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    headers: HeaderMap,
    body: axum::body::Body,
) -> ApiResult<Response> {
    let call = transcription(&state, TranscriptionOperation::Translate);
    send(state, &ctx, &headers, body, call).await
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/playground/sound-effects",
    summary = "Makes a sound effect in the Playground as the signed-in member.",
    description = "An ElevenLabs `sound-generation` request (`text`, `duration_seconds`, `model_id`) to\n\
    one of the project's sound effect models, as the signed-in member: the reply is the audio.\n\
    Charged to the project and logged with the source `playground`; needs a console session.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    request_body(content = Object, content_type = "application/json"),
    responses(
        (status = 200, description = "The model API's reply, as an app gets it, with the `kaeso-cost` header."),
        (status = 413, description = "The body is larger than Kaeso takes for this model (`request_too_large`).", body = crate::platform::error::ErrorEnvelope),
        (status = 502, description = "The provider failed or refused the provider key (`provider_error`).", body = crate::platform::error::ErrorEnvelope),
    )
)]
pub async fn sound_effects(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    headers: HeaderMap,
    body: axum::body::Body,
) -> ApiResult<Response> {
    let call = elevenlabs(&state, "/v1/sound-generation")?;
    send(state, &ctx, &headers, body, call).await
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/playground/music",
    summary = "Makes music in the Playground as the signed-in member.",
    description = "An ElevenLabs `music` request (`prompt`, `music_length_ms`, `model_id`) to one of the\n\
    project's music models, as the signed-in member: the reply is the audio. Charged to the\n\
    project and logged with the source `playground`; needs a console session.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    request_body(content = Object, content_type = "application/json"),
    responses(
        (status = 200, description = "The model API's reply, as an app gets it, with the `kaeso-cost` header."),
        (status = 413, description = "The body is larger than Kaeso takes for this model (`request_too_large`).", body = crate::platform::error::ErrorEnvelope),
        (status = 502, description = "The provider failed or refused the provider key (`provider_error`).", body = crate::platform::error::ErrorEnvelope),
    )
)]
pub async fn music(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    headers: HeaderMap,
    body: axum::body::Body,
) -> ApiResult<Response> {
    let call = elevenlabs(&state, "/v1/music")?;
    send(state, &ctx, &headers, body, call).await
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/playground/videos",
    summary = "Starts a video in the Playground as the signed-in member.",
    description = "An OpenAI `videos` request to one of the project's video models, as the signed-in\n\
    member: the reply is the job, to follow with `GET …/playground/videos/{video}`. The job is\n\
    charged to the project when it ends and logged with the source `playground`; needs a\n\
    console session.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    request_body(content = Object, content_type = "application/json"),
    responses(
        (status = 200, description = "The model API's reply, as an app gets it, with the `kaeso-cost` header."),
        (status = 413, description = "The body is larger than Kaeso takes for this model (`request_too_large`).", body = crate::platform::error::ErrorEnvelope),
        (status = 502, description = "The provider failed or refused the provider key (`provider_error`).", body = crate::platform::error::ErrorEnvelope),
    )
)]
pub async fn video_create(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    headers: HeaderMap,
    body: axum::body::Body,
) -> ApiResult<Response> {
    let call = video_call(&state);
    send(state, &ctx, &headers, body, call).await
}

fn decision_call(state: &AppState) -> Call {
    Call {
        ingress: DialectId::SystemOne,
        kind: RequestKind::Decide,
        max_body: state.config.body_limits.decisions.as_usize(),
        image: None,
        transcription: None,
        elevenlabs: None,
        video: None,
    }
}

#[derive(Debug, ToSchema)]
#[schema(
    description = "A Playground decision request: a TypeSafe System One request, whatever format\n\
    the model's provider takes. The console sends this subset; the body is decoded like a\n\
    `/v1/systemone` body, so other System One fields are taken as there."
)]
pub struct PlaygroundDecisionRequest {
    #[doc = "A decision model of this project's catalogue, or a model that also makes decisions:\n\
        its id, `provider/model`, or an alias of the project."]
    #[schema(example = "jev-1.14.0")]
    pub model: String,
    #[doc = "What the questions are about: text, or a JSON object or array. An image is named in\n\
        it by its key in `assets`."]
    #[schema(value_type = Value)]
    pub state: serde_json::Value,
    #[doc = "The questions, under keys the reply's `answers` use again, in the order to ask them.\n\
        Each has a `type`: `noul` (yes or no, with optional `criteria.true` and `criteria.false`),\n\
        `choice` (`criteria`: each option's value as a key, its description as the value),\n\
        `score` (`criteria`: the levels' descriptions, lowest first) or `box` (where something\n\
        is in the image), and optional `instructions`."]
    #[schema(value_type = Object)]
    pub questions: serde_json::Value,
    #[doc = "Images the state names, under the key it names them by: `type` `image`, `format`\n\
        (`png`, `jpg`, `jpeg`) and `data_base64`."]
    #[schema(value_type = Option<Object>)]
    pub assets: Option<serde_json::Value>,
}

#[derive(Debug, ToSchema)]
#[schema(
    description = "A Playground decision reply: TypeSafe System One's, whatever format the\n\
    model's provider answered in."
)]
pub struct PlaygroundDecisionReply {
    #[doc = "The model that answered."]
    pub model: String,
    #[doc = "One answer under each question's key: `noul` (the probability of yes), `choice`\n\
        (the chosen option's value, `probabilities` per value, `confidence`), `score` (the\n\
        expected level, `probabilities` and `legend` per level, `confidence`), `box` (`top`,\n\
        `bottom`, `left`, `right` as fractions of the image, `confidence`), or `type` `refusal`\n\
        when the provider refused that question."]
    #[schema(value_type = Object)]
    pub answers: serde_json::Value,
    #[doc = "`input_tokens` and `output_tokens`."]
    #[schema(value_type = Object)]
    pub usage: serde_json::Value,
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/playground/decisions",
    summary = "Asks a decision model typed questions in the Playground as the signed-in member.",
    description = "A TypeSafe System One request (`model`, `state`, `questions`, `assets`), as JSON or\n\
    as `multipart/form-data` whose first part is `request`, to one of the project's models that\n\
    make decisions, as the signed-in member, up to `KAESO_BODY_LIMITS_DECISIONS`. Kaeso sends it\n\
    in the format the model's provider takes and answers in System One's. Charged to the project,\n\
    counted against its limits and logged with the source `playground`; needs a console session.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    request_body(content = PlaygroundDecisionRequest, content_type = "application/json"),
    responses(
        (status = 200, description = "The answers, with the `kaeso-cost` header.", body = PlaygroundDecisionReply),
        (status = 400, description = "The model makes no decisions, or can't take a question, an image or as many questions (`invalid_request`).", body = crate::platform::error::ErrorEnvelope),
        (status = 413, description = "The body is larger than Kaeso takes for this model (`request_too_large`).", body = crate::platform::error::ErrorEnvelope),
        (status = 502, description = "The provider failed or refused the provider key (`provider_error`).", body = crate::platform::error::ErrorEnvelope),
    )
)]
pub async fn decisions(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    headers: HeaderMap,
    body: axum::body::Body,
) -> ApiResult<Response> {
    let call = decision_call(&state);
    send_as(state, &ctx, &headers, body, call, &DECISIONS).await
}

fn video_error(e: &KppError) -> ApiError {
    match (&e.kind, e.param.as_deref()) {
        (_, _) if videos::is_unavailable(e) => ApiError::Unavailable(
            "Kaeso cannot read its videos right now; try again shortly.".into(),
        ),
        (ErrorKind::InvalidRequest, Some("variant")) => ApiError::invalid(
            "variant",
            "`variant` must be `video`, `thumbnail` or `spritesheet`.",
        ),
        (ErrorKind::NotFound, Some("variant")) => {
            ApiError::NotFound("The provider keeps no file of this kind for the video.".into())
        }
        (ErrorKind::NotFound, _) if videos::is_expired(e) => {
            ApiError::NotFound("The video's files expired; make the video again.".into())
        }
        (ErrorKind::NotFound, _) => {
            ApiError::NotFound("No video of yours in this project has this id.".into())
        }
        (ErrorKind::RateLimited, _) => ApiError::RateLimited {
            message: "A limit of this project refused the request; try again later.".into(),
            retry_after: e.retry_after.map(|d| d.as_secs().max(1)),
        },
        (ErrorKind::InvalidRequest, _) if e.status == Some(409) => ApiError::Conflict(
            "The provider key that started this video was replaced; start it again.".into(),
        ),
        (ErrorKind::InvalidRequest, _) => ApiError::InvalidRequest {
            message: "The video isn't ready for that yet; wait until it is completed.".into(),
            param: Some("video".into()),
        },
        _ => ApiError::Provider {
            status: axum::http::StatusCode::BAD_GATEWAY,
            message: "The provider did not say how the video stands; try again shortly.".into(),
            retry_after: None,
        },
    }
}

fn json(bytes: Vec<u8>) -> Response {
    let mut resp = bytes.into_response();
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    resp
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/playground/videos/{video}",
    summary = "A video the signed-in member started in the Playground.",
    description = "The job, as OpenAI's Videos API shows it, after asking its provider how it\n\
    stands, with the `kaeso-cost` header once it ended: what it was charged, `0` for a job that\n\
    failed, or `unknown`. Only videos the member started in the Playground; needs a console session.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("video" = String, Path, description = "The video's id, as `video_create` answered it.")),
    responses(
        (status = 200, description = "The job.", body = Object),
        (status = 404, description = "No video of the member's in this project has this id (`not_found`).", body = crate::platform::error::ErrorEnvelope),
    )
)]
pub async fn video_get(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_, _, video)): Path<(String, String, String)>,
) -> ApiResult<Response> {
    let viewer = Viewer::member(member(&ctx)?);
    let (body, charged) = videos::retrieve_as(&state, &viewer, &video)
        .await
        .map_err(|e| video_error(&e))?;
    let mut resp = json(body);
    if let Some(v) = charged.and_then(|c| HeaderValue::from_str(&c).ok()) {
        resp.headers_mut().insert(pipeline::COST_HEADER, v);
    }
    Ok(resp)
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/playground/videos/{video}/content",
    summary = "The file of a video the signed-in member made in the Playground.",
    description = "The completed video's file, streamed from its provider with `Range` passed\n\
    through; `variant` (`video`, `thumbnail`, `spritesheet`) as in OpenAI's Videos API.\n\
    Needs a console session.",
    tag = "projects",
    params(
        ("org" = OrgId, Path),
        ("project" = ProjectId, Path),
        ("video" = String, Path, description = "The video's id."),
        ("variant" = Option<String>, Query, description = "`video` (the default), `thumbnail` or `spritesheet`."),
    ),
    responses(
        (status = 200, description = "The file.", content_type = "video/mp4", body = Vec<u8>),
        (status = 206, description = "The range asked for.", content_type = "video/mp4", body = Vec<u8>),
        (status = 404, description = "No video of the member's in this project has this id (`not_found`).", body = crate::platform::error::ErrorEnvelope),
    )
)]
pub async fn video_content(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_, _, video)): Path<(String, String, String)>,
    axum::extract::RawQuery(query): axum::extract::RawQuery,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let viewer = Viewer::member(member(&ctx)?);
    videos::content_as(
        &state,
        &viewer,
        &headers,
        &video,
        query.as_deref().unwrap_or(""),
    )
    .await
    .map_err(|e| video_error(&e))
}

#[utoipa::path(
    delete,
    path = "/orgs/{org}/projects/{project}/playground/videos/{video}",
    summary = "Cancels or deletes a video the signed-in member started in the Playground.",
    description = "Deletes the job at its provider where the provider can, which cancels one\n\
    still running there, and forgets it. Needs a console session.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("video" = String, Path, description = "The video's id.")),
    responses(
        (status = 200, description = "The deleted job's id.", body = Object),
        (status = 404, description = "No video of the member's in this project has this id (`not_found`).", body = crate::platform::error::ErrorEnvelope),
    )
)]
pub async fn video_delete(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_, _, video)): Path<(String, String, String)>,
) -> ApiResult<Response> {
    let viewer = Viewer::member(member(&ctx)?);
    videos::delete_as(&state, &viewer, &video)
        .await
        .map(json)
        .map_err(|e| video_error(&e))
}

#[cfg(test)]
mod tests {
    use crate::config::ByteSize;
    use kaeso_core::catalogue::{DecisionFields, Fields};
    use kaeso_core::providers::ProviderRef;

    use super::*;

    fn model(provider: &str, id: &str, input: &[&str], output: &[&str]) -> Effective {
        let provider = ProviderRef::from(kaeso_core::provider::provider(provider).unwrap());
        let list = |l: &[&str]| Some(l.iter().map(|s| (*s).to_owned()).collect());
        let fields = Fields {
            input_modalities: list(input),
            output_modalities: list(output),
            ..Fields::default()
        };
        Effective {
            id: uuid::Uuid::nil(),
            dialect: provider.dialect_for(id),
            provider,
            model_id: id.to_owned(),
            fields,
        }
    }

    #[test]
    fn a_model_without_a_kind_takes_the_mode_of_what_it_makes_and_its_providers_apis() {
        let cases = [
            (
                model("openai", "gpt-image-2", &["text", "image"], &["image"]),
                Some(PlaygroundMode::Image),
            ),
            (
                model("openai", "tts-1", &["text"], &["audio"]),
                Some(PlaygroundMode::Speech),
            ),
            (
                model("openai", "whisper-1", &["audio"], &["text"]),
                Some(PlaygroundMode::Transcription),
            ),
            (
                model("xai", "grok-imagine-video-1.5", &["text"], &["video"]),
                Some(PlaygroundMode::Video),
            ),
            (
                model(
                    "anthropic",
                    "claude-haiku-4-5",
                    &["text", "image"],
                    &["text"],
                ),
                Some(PlaygroundMode::Text),
            ),
            (
                model("anthropic", "claude-haiku-4-5", &["audio"], &["audio"]),
                None,
            ),
        ];
        for (eff, mode) in cases {
            assert_eq!(mode_of(&eff, None), mode, "{}", eff.model_id);
        }
    }

    fn decider(provider: &str, id: &str, fields: DecisionFields) -> Effective {
        let mut eff = model(provider, id, &["text", "image"], &[]);
        eff.fields.decisions = Some(fields);
        eff
    }

    #[test]
    fn decision_models_take_the_decision_mode_and_dual_models_keep_text() {
        let only = DecisionFields {
            only: true,
            ..DecisionFields::default()
        };
        let jev = decider("openrouter", "typesafe/jev-1.14", only.clone());
        assert_eq!(
            mode_of(&jev, Some("decision")),
            Some(PlaygroundMode::Decision)
        );
        assert_eq!(mode_of(&jev, None), Some(PlaygroundMode::Decision));
        let undecided = model("openrouter", "typesafe/jev-1.14", &["text"], &[]);
        assert_eq!(
            mode_of(&undecided, Some("decision")),
            None,
            "a decision kind without decisions makes nothing the Playground can ask"
        );
        let unserved = decider("anthropic", "claude-decider", only);
        assert_eq!(mode_of(&unserved, None), None);
        let mut luna = model("openai", "gpt-6-luna", &["text"], &["text"]);
        luna.fields.decisions = Some(DecisionFields::default());
        assert_eq!(
            mode_of(&luna, Some("generation")),
            Some(PlaygroundMode::Text)
        );
    }

    #[test]
    fn a_custom_providers_decision_model_is_asked_like_a_built_in_one() {
        use kaeso_core::providers::{CustomAuth, CustomProvider, DecisionsApi, Draft, MediaApi};
        let custom = |media: Vec<MediaApi>, decisions: Option<DecisionsApi>| {
            ProviderRef::Custom(std::sync::Arc::new(
                CustomProvider::new(Draft {
                    uuid: uuid::Uuid::now_v7(),
                    id: kaeso_core::provider::ProviderId::parse("liquid").unwrap(),
                    name: "Liquid".into(),
                    base_url: "https://api.liquid.example".into(),
                    auth: CustomAuth::Bearer,
                    generation: DialectId::OpenAiChat,
                    embeddings: None,
                    count_tokens: None,
                    server: None,
                    media,
                    decisions,
                })
                .unwrap(),
            ))
        };
        let only = DecisionFields {
            only: true,
            ..DecisionFields::default()
        };
        let mut d1 = decider("openai", "d1", only);
        d1.provider = custom(vec![MediaApi::Decisions], None);
        assert_eq!(mode_of(&d1, None), Some(PlaygroundMode::Decision));
        let d = decision_options(&limits(), &d1).unwrap();
        assert_eq!(d.question_types, ["predicate", "choice", "score"]);
        d1.provider = custom(Vec::new(), None);
        assert_eq!(
            mode_of(&d1, None),
            None,
            "a server with its decisions API off can't be asked"
        );
    }

    fn limits() -> BodyLimitsConfig {
        BodyLimitsConfig {
            model_api: ByteSize(32 << 20),
            images: ByteSize(64 << 20),
            audio: ByteSize(26 << 20),
            decisions: ByteSize(64 << 20),
            in_flight: ByteSize(1 << 30),
            platform_api: ByteSize(1 << 20),
            provider_response: ByteSize(256 << 20),
        }
    }

    #[test]
    fn decision_options_narrow_the_catalogue_by_the_providers_decisions_api() {
        let limits = limits();
        let typesafe = DecisionFields {
            only: true,
            question_types: Some(QUESTION_TYPES.map(str::to_owned).to_vec()),
            max_options: Some(300),
            ..DecisionFields::default()
        };
        let jev = decider("typesafe", "jev-1.14.0", typesafe.clone());
        let o = options(&limits, &jev, PlaygroundMode::Decision);
        assert_eq!(o.max_request_bytes, limits.decisions.0);
        let d = o.decisions.unwrap();
        assert_eq!(d.question_types, ["predicate", "choice", "score", "box"]);
        assert_eq!((d.max_options, d.max_levels), (Some(255), Some(10)));
        assert!(d.images && !d.needs_api_key);
        assert_eq!(d.max_request_bytes, o.max_request_bytes);

        let perplexity = decider("perplexity", "pplx-decider-v1.2", typesafe);
        let d = options(&limits, &perplexity, PlaygroundMode::Decision)
            .decisions
            .unwrap();
        assert!(
            !d.question_types.iter().any(|t| t == "box"),
            "box questions go only where the provider takes them"
        );

        let mut luna = model("openai", "gpt-6-luna", &["text"], &["text"]);
        luna.fields.decisions = Some(DecisionFields {
            max_questions: Some(1_000),
            max_levels: Some(10),
            ..DecisionFields::default()
        });
        let o = options(&limits, &luna, PlaygroundMode::Text);
        assert_eq!(o.max_request_bytes, limits.model_api.0);
        let d = o.decisions.unwrap();
        assert_eq!(d.question_types, ["predicate", "choice", "score"]);
        assert_eq!((d.max_questions, d.max_levels), (Some(400), Some(10)));
        assert!(!d.images);
        assert_eq!(d.max_request_bytes, limits.decisions.0);

        let claude = model("anthropic", "claude-haiku-4-5", &["text"], &["text"]);
        assert_eq!(
            options(&limits, &claude, PlaygroundMode::Text).decisions,
            None
        );
    }

    #[test]
    fn a_kind_the_providers_apis_cant_serve_is_left_out() {
        let chat = model("anthropic", "claude-haiku-4-5", &["text"], &["text"]);
        assert_eq!(mode_of(&chat, Some("image")), None);
        assert_eq!(mode_of(&chat, Some("realtime")), None);
        assert_eq!(
            mode_of(&chat, Some("generation")),
            Some(PlaygroundMode::Text)
        );
        let sts = model("openai", "tts-1", &["audio"], &["audio"]);
        assert_eq!(mode_of(&sts, Some("speech")), None);
        let effects = model(
            "elevenlabs",
            "eleven_text_to_sound_v2",
            &["text"],
            &["audio"],
        );
        assert_eq!(
            mode_of(&effects, Some("sound_effects")),
            Some(PlaygroundMode::SoundEffects)
        );
        assert_eq!(
            mode_of(&effects, Some("music")),
            Some(PlaygroundMode::Music)
        );
    }

    #[test]
    fn image_options_follow_the_providers_images_api() {
        let gpt = model("openai", "gpt-image-2", &["text", "image"], &["image"]);
        let o = image_options(&gpt, true).unwrap();
        assert!(o.edits && o.max_count == 10 && o.size == PlaygroundImageSize::Pixels);
        assert!(!image_options(&gpt, false).unwrap().edits);
        let grok = model("xai", "grok-imagine-image", &["text"], &["image"]);
        assert!(!image_options(&grok, false).unwrap().edits);
        let claude = model("anthropic", "claude-haiku-4-5", &["text"], &["text"]);
        assert!(voices(&claude).is_empty() && !translates(&claude));
        assert!(
            voices(&model("openai", "tts-1", &["text"], &["audio"])).contains(&"alloy".to_owned())
        );
    }

    #[test]
    fn sound_effects_and_music_need_a_speech_api_and_only_elevenlabs_takes_a_length() {
        let qwen = model(
            "alibaba",
            "qwen-audio-3.1-tts-next",
            &["text", "audio"],
            &["audio"],
        );
        assert_eq!(
            mode_of(&qwen, Some("sound_effects")),
            Some(PlaygroundMode::SoundEffects)
        );
        assert!(!takes_lengths(&qwen));
        let eleven = model(
            "elevenlabs",
            "eleven_text_to_sound_v2",
            &["text"],
            &["audio"],
        );
        assert!(takes_lengths(&eleven));
        let no_speech = model("anthropic", "claude-haiku-4-5", &["text"], &["audio"]);
        assert_eq!(mode_of(&no_speech, Some("sound_effects")), None);
        assert_eq!(mode_of(&no_speech, Some("music")), None);
    }

    #[test]
    fn every_video_failure_reads_as_what_went_wrong() {
        let said = |e: KppError| {
            let a = video_error(&e);
            (a.status().as_u16(), a.to_string())
        };
        let kpp = |kind: ErrorKind, status: u16, message: &str| {
            KppError::new(kind, message).with_status(status)
        };
        assert_eq!(
            said(kpp(
                ErrorKind::ProviderError,
                503,
                "Kaeso cannot read its videos right now; try again shortly."
            )),
            (
                503,
                "Kaeso cannot read its videos right now; try again shortly.".to_owned()
            )
        );
        assert_eq!(
            said(kpp(ErrorKind::InvalidRequest, 400, "`variant` must be …").with_param("variant"))
                .0,
            400
        );
        assert_eq!(
            said(kpp(ErrorKind::NotFound, 404, "x").with_param("variant")).1,
            "The provider keeps no file of this kind for the video."
        );
        assert_eq!(
            said(
                kpp(
                    ErrorKind::NotFound,
                    404,
                    "The files of `video_1` expired; create the video again."
                )
                .with_param("video_id")
            )
            .1,
            "The video's files expired; make the video again."
        );
        assert_eq!(
            said(kpp(
                ErrorKind::NotFound,
                404,
                "No video `x` belongs to this project."
            ))
            .1,
            "No video of yours in this project has this id."
        );
        assert_eq!(said(kpp(ErrorKind::InvalidRequest, 409, "replaced")).0, 409);
        assert_eq!(
            said(kpp(ErrorKind::InvalidRequest, 400, "running").with_param("video_id")).0,
            400
        );
        assert_eq!(said(kpp(ErrorKind::RateLimited, 429, "slow down")).0, 429);
        assert_eq!(said(kpp(ErrorKind::ProviderError, 502, "down")).0, 502);
    }
}
