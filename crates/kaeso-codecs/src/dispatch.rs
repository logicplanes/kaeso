use kaeso_kpp::{
    DialectId, ErrorKind, KppError, ModelInfo, ModelList, ModelsQuery, RawItem, Reply, Request,
    RequestKind,
};

use crate::anthropic_messages::AnthropicMessages;
use crate::elevenlabs::{ElevenLabs, ElevenLabsCall};
use crate::gemini::{Gemini, GeminiAction};
use crate::gemini_embeddings::GeminiEmbeddings;
use crate::gemini_interactions::GeminiInteractions;
use crate::limits::Checked;
use crate::minimax_images::MinimaxImages;
use crate::minimax_speech::MinimaxSpeech;
use crate::minimax_videos::MinimaxVideos;
use crate::openai_chat::OpenAiChat;
use crate::openai_embeddings::OpenAiEmbeddings;
use crate::openai_images::OpenAiImages;
use crate::openai_responses::OpenAiResponses;
use crate::openai_speech::OpenAiSpeech;
use crate::openai_transcriptions::OpenAiTranscriptions;
use crate::openai_videos::OpenAiVideos;
use crate::openrouter_videos::OpenRouterVideos;
use crate::veo::Veo;
use crate::xai_videos::XaiVideos;
use crate::zai_videos::ZaiVideos;
use crate::{
    Codec, EgressCtx, EncodeCtx, EncodedError, EncodedRequest, HeaderList, StreamDecoder,
    StreamEncoder,
};

macro_rules! dispatch {
    ($dialect:expr, $f:ident ( $($arg:expr),* )) => {
        match $dialect {
            DialectId::AnthropicMessages => Ok(AnthropicMessages::$f($($arg),*)),
            DialectId::OpenAiChat => Ok(OpenAiChat::$f($($arg),*)),
            DialectId::OpenAiResponses => Ok(OpenAiResponses::$f($($arg),*)),
            DialectId::OpenAiEmbeddings => Ok(OpenAiEmbeddings::$f($($arg),*)),
            DialectId::Gemini => Ok(Gemini::$f($($arg),*)),
            DialectId::GeminiEmbeddings => Ok(GeminiEmbeddings::$f($($arg),*)),
            DialectId::OpenAiImages => Ok(OpenAiImages::$f($($arg),*)),
            DialectId::MinimaxImages => Ok(MinimaxImages::$f($($arg),*)),
            DialectId::OpenAiSpeech => Ok(OpenAiSpeech::$f($($arg),*)),
            DialectId::OpenAiTranscriptions => Ok(OpenAiTranscriptions::$f($($arg),*)),
            DialectId::MinimaxSpeech => Ok(MinimaxSpeech::$f($($arg),*)),
            DialectId::ElevenLabs => Ok(ElevenLabs::$f($($arg),*)),
            DialectId::OpenAiVideos => Ok(OpenAiVideos::$f($($arg),*)),
            DialectId::XaiVideos => Ok(XaiVideos::$f($($arg),*)),
            DialectId::GeminiInteractions => Ok(GeminiInteractions::$f($($arg),*)),
            DialectId::Veo => Ok(Veo::$f($($arg),*)),
            DialectId::OpenRouterVideos => Ok(OpenRouterVideos::$f($($arg),*)),
            DialectId::MinimaxVideos => Ok(MinimaxVideos::$f($($arg),*)),
            DialectId::ZaiVideos => Ok(ZaiVideos::$f($($arg),*)),
            other => Err(no_codec(other)),
        }
    };
}

fn no_codec(dialect: DialectId) -> KppError {
    KppError::unsupported(
        dialect.as_str(),
        format!(
            "This version of Kaeso has no codec for {}.",
            dialect.display_name()
        ),
    )
}

pub fn decode_request(
    dialect: DialectId,
    kind: RequestKind,
    body: &[u8],
    headers: &HeaderList,
) -> Result<Request, KppError> {
    dispatch!(dialect, decode_request(kind, body, headers))?
}

/// Decodes a request body [`Checked::new`] already checked, without counting it again.
pub fn decode_checked_request<B: AsRef<[u8]>>(
    kind: RequestKind,
    checked: &Checked<B>,
    headers: &HeaderList,
) -> Result<Request, KppError> {
    let body = checked.body().as_ref();
    match checked.dialect() {
        DialectId::AnthropicMessages => AnthropicMessages::decode_checked(kind, body, headers),
        DialectId::OpenAiChat => OpenAiChat::decode_checked(kind, body, headers),
        DialectId::OpenAiResponses => OpenAiResponses::decode_checked(kind, body, headers),
        DialectId::OpenAiEmbeddings => OpenAiEmbeddings::decode_checked(kind, body, headers),
        DialectId::OpenAiImages => OpenAiImages::decode_checked(kind, body, headers),
        DialectId::MinimaxImages => MinimaxImages::decode_request(kind, body, headers),
        DialectId::OpenAiSpeech => OpenAiSpeech::decode_checked(kind, body, headers),
        DialectId::MinimaxSpeech => MinimaxSpeech::decode_request(kind, body, headers),
        DialectId::ElevenLabs => ElevenLabs::decode_request(kind, body, headers),
        DialectId::OpenAiVideos => OpenAiVideos::decode_checked(kind, body, headers),
        DialectId::XaiVideos => XaiVideos::decode_request(kind, body, headers),
        DialectId::GeminiInteractions => GeminiInteractions::decode_request(kind, body, headers),
        DialectId::Veo => Veo::decode_request(kind, body, headers),
        DialectId::OpenRouterVideos => OpenRouterVideos::decode_request(kind, body, headers),
        DialectId::MinimaxVideos => MinimaxVideos::decode_request(kind, body, headers),
        DialectId::ZaiVideos => ZaiVideos::decode_request(kind, body, headers),
        DialectId::OpenAiTranscriptions => OpenAiTranscriptions::decode_call(
            kaeso_kpp::TranscriptionOperation::Transcribe,
            body,
            headers,
        ),
        other @ (DialectId::Gemini | DialectId::GeminiEmbeddings) => Err(gemini_needs_path(other)),
        other => Err(no_codec(other)),
    }
}

fn gemini_needs_path(dialect: DialectId) -> KppError {
    KppError::invalid_request(format!(
        "A {} request names its model and method in the path; decode it with `decode_gemini_request`.",
        dialect.display_name()
    ))
}

/// Decodes a Gemini request for `action` on `model`, as the path
/// `/v1beta/models/{model}:{action}` names them ([`crate::gemini::split_model_action`]).
/// The action decides the dialect: embeddings methods go to the Gemini embeddings codec.
pub fn decode_gemini_request(
    model: &str,
    action: GeminiAction,
    body: &[u8],
    headers: &HeaderList,
) -> Result<Request, KppError> {
    crate::limits::request_shape(action.dialect(), body)?;
    decode_gemini_unchecked(model, action, body, headers)
}

/// Decodes a Gemini request body [`Checked::new`] already checked, without counting it again.
pub fn decode_checked_gemini_request<B: AsRef<[u8]>>(
    model: &str,
    action: GeminiAction,
    checked: &Checked<B>,
    headers: &HeaderList,
) -> Result<Request, KppError> {
    if checked.dialect() != action.dialect() {
        return Err(KppError::invalid_request(format!(
            "The body was checked as {}, but `{}` is a {} method.",
            checked.dialect().display_name(),
            action.as_str(),
            action.dialect().display_name()
        )));
    }
    decode_gemini_unchecked(model, action, checked.body().as_ref(), headers)
}

fn decode_gemini_unchecked(
    model: &str,
    action: GeminiAction,
    body: &[u8],
    headers: &HeaderList,
) -> Result<Request, KppError> {
    match action.dialect() {
        DialectId::GeminiEmbeddings => GeminiEmbeddings::decode_call(model, action, body, headers),
        _ => Gemini::decode_call(model, action, body, headers),
    }
}

/// Decodes an OpenAI Images API request for `operation`, as the path names it:
/// `/v1/images/generations`, `/v1/images/edits` or `/v1/images/variations`. `headers` must
/// carry the request's `content-type`, which tells an upload from a JSON body.
pub fn decode_image_request(
    operation: kaeso_kpp::ImageOperation,
    body: &[u8],
    headers: &HeaderList,
) -> Result<Request, KppError> {
    crate::limits::request_shape(DialectId::OpenAiImages, body)?;
    OpenAiImages::decode_call(operation, body, headers)
}

/// Decodes an Images API body [`Checked::new`] already checked, without counting it again.
pub fn decode_checked_image_request<B: AsRef<[u8]>>(
    operation: kaeso_kpp::ImageOperation,
    checked: &Checked<B>,
    headers: &HeaderList,
) -> Result<Request, KppError> {
    OpenAiImages::decode_call(operation, checked.body().as_ref(), headers)
}

/// Decodes an OpenAI transcription API upload for `operation`, as the path names it:
/// `/v1/audio/transcriptions` or `/v1/audio/translations`. `headers` must carry the request's
/// `content-type` with its boundary.
pub fn decode_transcription_request(
    operation: kaeso_kpp::TranscriptionOperation,
    body: &[u8],
    headers: &HeaderList,
) -> Result<Request, KppError> {
    crate::limits::request_shape(DialectId::OpenAiTranscriptions, body)?;
    OpenAiTranscriptions::decode_call(operation, body, headers)
}

/// Decodes a transcription upload [`Checked::new`] already checked, without counting it again.
pub fn decode_checked_transcription_request<B: AsRef<[u8]>>(
    operation: kaeso_kpp::TranscriptionOperation,
    checked: &Checked<B>,
    headers: &HeaderList,
) -> Result<Request, KppError> {
    OpenAiTranscriptions::decode_call(operation, checked.body().as_ref(), headers)
}

/// Decodes an ElevenLabs-format request for the call its path names
/// ([`ElevenLabsCall::from_path`]). `headers` must carry an upload's `content-type`.
pub fn decode_elevenlabs_request(
    call: &ElevenLabsCall,
    body: &[u8],
    headers: &HeaderList,
) -> Result<Request, KppError> {
    crate::limits::request_shape(DialectId::ElevenLabs, body)?;
    ElevenLabs::decode_call(call, body, headers)
}

/// Decodes an ElevenLabs body [`Checked::new`] already checked, without counting it again.
pub fn decode_checked_elevenlabs_request<B: AsRef<[u8]>>(
    call: &ElevenLabsCall,
    checked: &Checked<B>,
    headers: &HeaderList,
) -> Result<Request, KppError> {
    ElevenLabs::decode_call(call, checked.body().as_ref(), headers)
}

/// The `content-type` of a reply [`encode_response`] wrote for `dialect`.
pub fn response_media_type(dialect: DialectId, reply: &Reply, ctx: &EncodeCtx) -> String {
    dispatch!(dialect, response_media_type(reply, ctx))
        .unwrap_or_else(|_| crate::codec::JSON_MEDIA_TYPE.to_owned())
}

/// The `content-type` of a stream the [`stream_encoder`] of `dialect` writes.
pub fn stream_media_type(dialect: DialectId, ctx: &EncodeCtx) -> String {
    dispatch!(dialect, stream_media_type(ctx))
        .unwrap_or_else(|_| crate::codec::EVENT_STREAM_MEDIA_TYPE.to_owned())
}

/// Whether the provider's reply to a request encoded with `ctx` is read as it arrives, with
/// [`stream_decoder`], rather than whole with [`decode_response`].
pub fn reads_reply_as_stream(dialect: DialectId, ctx: &EgressCtx) -> bool {
    dispatch!(dialect, reads_reply_as_stream(ctx)).unwrap_or(false)
}

pub fn encode_response(
    dialect: DialectId,
    reply: &Reply,
    ctx: &EncodeCtx,
) -> Result<Vec<u8>, KppError> {
    dispatch!(dialect, encode_response(reply, ctx))?
}

pub fn stream_encoder(
    dialect: DialectId,
    ctx: &EncodeCtx,
) -> Result<Box<dyn StreamEncoder>, KppError> {
    dispatch!(dialect, stream_encoder(ctx))
}

pub fn encode_error(dialect: DialectId, err: &KppError) -> EncodedError {
    dispatch!(dialect, encode_error(err)).unwrap_or_else(|_| no_codec_error(err))
}

pub fn encode_error_for_request(
    dialect: DialectId,
    err: &KppError,
    request_id: &str,
) -> EncodedError {
    dispatch!(dialect, encode_error_for_request(err, request_id))
        .unwrap_or_else(|_| no_codec_error(err))
}

fn no_codec_error(err: &KppError) -> EncodedError {
    let body = serde_json::json!({ "error": { "message": err.message } });
    EncodedError::new(
        err.status.unwrap_or(500),
        HeaderList::new().with("content-type", "application/json"),
        serde_json::to_vec(&body).unwrap_or_default(),
    )
}

pub fn check(dialect: DialectId, req: &Request, ctx: &EgressCtx) -> Result<(), KppError> {
    dispatch!(dialect, check(req, ctx))?
}

pub fn encode_request(
    dialect: DialectId,
    req: &Request,
    ctx: &mut EgressCtx,
) -> Result<EncodedRequest, KppError> {
    dispatch!(dialect, encode_request(req, ctx))?
}

pub fn decode_response(
    dialect: DialectId,
    status: u16,
    headers: &HeaderList,
    body: &[u8],
    ctx: &EgressCtx,
) -> Result<Reply, KppError> {
    dispatch!(dialect, decode_response(status, headers, body, ctx))?
}

pub fn stream_decoder(
    dialect: DialectId,
    ctx: &EgressCtx,
) -> Result<Box<dyn StreamDecoder>, KppError> {
    dispatch!(dialect, stream_decoder(ctx))
}

pub fn decode_error(
    dialect: DialectId,
    status: u16,
    headers: &HeaderList,
    body: &[u8],
) -> KppError {
    dispatch!(dialect, decode_error(status, headers, body))
        .unwrap_or_else(|e| KppError::new(ErrorKind::ProviderError, e.message).with_status(status))
}

pub fn decode_provider_error(
    dialect: DialectId,
    status: u16,
    headers: &HeaderList,
    body: &[u8],
    ctx: &EgressCtx,
) -> KppError {
    dispatch!(dialect, decode_provider_error(status, headers, body, ctx))
        .unwrap_or_else(|e| KppError::new(ErrorKind::ProviderError, e.message).with_status(status))
}

pub fn finished_item(dialect: DialectId, start: &RawItem, deltas: &[RawItem]) -> Option<RawItem> {
    match dialect {
        DialectId::OpenAiResponses => Some(crate::openai_responses::assembly::finished_item(
            start, deltas,
        )),
        _ => None,
    }
}

pub fn reports_item_progress(dialect: DialectId, frame: &RawItem) -> bool {
    match dialect {
        DialectId::OpenAiResponses => {
            crate::openai_responses::assembly::reports_item_progress(frame)
        }
        _ => false,
    }
}

pub fn models_dialect(headers: &HeaderList) -> DialectId {
    if headers.get("anthropic-version").is_some() {
        DialectId::AnthropicMessages
    } else if headers.get("xi-api-key").is_some() {
        DialectId::ElevenLabs
    } else {
        DialectId::OpenAiChat
    }
}

pub fn decode_models_query(dialect: DialectId, query: &str) -> Result<ModelsQuery, KppError> {
    dispatch!(dialect, decode_models_query(query))?
}

pub fn encode_models(dialect: DialectId, list: &ModelList) -> Result<Vec<u8>, KppError> {
    dispatch!(dialect, encode_models(list))?
}

pub fn encode_model(dialect: DialectId, model: &ModelInfo) -> Result<Vec<u8>, KppError> {
    dispatch!(dialect, encode_model(model))?
}

pub fn decode_models(
    dialect: DialectId,
    status: u16,
    headers: &HeaderList,
    body: &[u8],
) -> Result<ModelList, KppError> {
    dispatch!(dialect, decode_models(status, headers, body))?
}

pub fn decode_model(
    dialect: DialectId,
    status: u16,
    headers: &HeaderList,
    body: &[u8],
) -> Result<ModelInfo, KppError> {
    dispatch!(dialect, decode_model(status, headers, body))?
}

/// The query for the page after `page` of a listing asked with `query`, or `None` when `page`
/// was the last or names no page after it. A Gemini listing names its next page with an opaque
/// `nextPageToken`; the other dialects continue after the last id of the page.
pub fn next_models_query(
    dialect: DialectId,
    query: &ModelsQuery,
    page: &ModelList,
) -> Option<ModelsQuery> {
    if !page.has_more {
        return None;
    }
    let cursor = match dialect {
        DialectId::Gemini | DialectId::GeminiEmbeddings => {
            crate::gemini::models::next_page_token(page)
        }
        _ => page.models.last().map(|m| m.id.clone()),
    }?;
    if query.after_id.as_deref() == Some(cursor.as_str()) {
        return None;
    }
    let mut next = query.clone();
    next.after_id = Some(cursor);
    Some(next)
}

fn path_segment(model: &str) -> Result<(), KppError> {
    // A URL parser removes a `.` or `..` path segment even when it is percent-encoded
    // (WHATWG URL, "single-dot" and "double-dot" segments), so no encoding keeps them.
    if matches!(model, "." | "..") {
        return Err(KppError::invalid_request(format!(
            "The model id `{model}` can't be part of a URL; use the model's real id."
        ))
        .with_param("model")
        .with_status(400));
    }
    Ok(())
}

pub fn encode_model_request(dialect: DialectId, model: &str) -> Result<EncodedRequest, KppError> {
    path_segment(model)?;
    dispatch!(dialect, encode_model_request(model))
}

pub fn encode_models_request(
    dialect: DialectId,
    query: &ModelsQuery,
) -> Result<EncodedRequest, KppError> {
    dispatch!(dialect, encode_models_request(query))?
}

/// [`encode_model_request`] for the provider `ctx` describes, such as a Gemini provider with
/// its own models path ([`EgressCtx::with_gemini_profile`]).
pub fn encode_model_request_with(
    dialect: DialectId,
    model: &str,
    ctx: &EgressCtx,
) -> Result<EncodedRequest, KppError> {
    path_segment(model)?;
    dispatch!(dialect, encode_model_request_with(model, ctx))?
}

/// [`encode_models_request`] for the provider `ctx` describes.
pub fn encode_models_request_with(
    dialect: DialectId,
    query: &ModelsQuery,
    ctx: &EgressCtx,
) -> Result<EncodedRequest, KppError> {
    dispatch!(dialect, encode_models_request_with(query, ctx))?
}

/// The request that asks the provider how its video job `job`, made by `model`, stands.
pub fn encode_video_poll(
    dialect: DialectId,
    job: &str,
    model: &str,
    ctx: &EgressCtx,
) -> Result<EncodedRequest, KppError> {
    dispatch!(dialect, encode_video_poll(job, model, ctx))?
}

/// Reads the provider's answer to [`encode_video_poll`].
pub fn decode_video_poll(
    dialect: DialectId,
    status: u16,
    headers: &HeaderList,
    body: &[u8],
    ctx: &EgressCtx,
) -> Result<kaeso_kpp::VideoJob, KppError> {
    dispatch!(dialect, decode_video_poll(status, headers, body, ctx))?
}

/// The request that deletes the provider's video job, or `None` when it documents no delete.
pub fn encode_video_delete(
    dialect: DialectId,
    job: &str,
    model: &str,
    ctx: &EgressCtx,
) -> Result<Option<EncodedRequest>, KppError> {
    dispatch!(dialect, encode_video_delete(job, model, ctx))?
}

/// The request that turns the file id a finished video job named into a link.
pub fn encode_video_file(
    dialect: DialectId,
    file: &str,
    ctx: &EgressCtx,
) -> Result<EncodedRequest, KppError> {
    dispatch!(dialect, encode_video_file(file, ctx))?
}

/// Reads the provider's answer to [`encode_video_file`]: the file's link.
pub fn decode_video_file(
    dialect: DialectId,
    status: u16,
    headers: &HeaderList,
    body: &[u8],
) -> Result<kaeso_kpp::Source, KppError> {
    dispatch!(dialect, decode_video_file(status, headers, body))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_known_dialect_has_a_codec() {
        for d in DialectId::ALL {
            let err = decode_request(d, RequestKind::Generate, b"not json", &HeaderList::new())
                .unwrap_err();
            assert!(
                err.unsupported_feature() != Some(d.as_str()),
                "{d} has no codec"
            );
            for err in [
                decode_models_query(d, "%").unwrap_err(),
                encode_models(d, &ModelList::default())
                    .map_or_else(|e| e, |_| KppError::invalid_request("ok")),
                encode_model(d, &ModelInfo::new("m"))
                    .map_or_else(|e| e, |_| KppError::invalid_request("ok")),
                decode_models(d, 200, &HeaderList::new(), b"x").unwrap_err(),
                decode_model(d, 200, &HeaderList::new(), b"x").unwrap_err(),
            ] {
                assert!(
                    err.unsupported_feature() != Some(d.as_str()),
                    "{d} has no models codec"
                );
            }
        }
    }

    #[test]
    fn models_dialect_follows_the_anthropic_version_header() {
        let anthropic = HeaderList::new().with("Anthropic-Version", "2023-06-01");
        assert_eq!(models_dialect(&anthropic), DialectId::AnthropicMessages);
        let openai = HeaderList::new().with("authorization", "Bearer x");
        assert_eq!(models_dialect(&openai), DialectId::OpenAiChat);
        assert_eq!(models_dialect(&HeaderList::new()), DialectId::OpenAiChat);
        let beta_only = HeaderList::new().with("anthropic-beta", "x");
        assert_eq!(models_dialect(&beta_only), DialectId::OpenAiChat);
    }

    #[test]
    fn the_next_page_follows_the_gemini_token_or_else_the_last_id() {
        let fixture = |name: &str| {
            std::fs::read(format!(
                "{}/fixtures/gemini/models/{name}.json",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap()
        };
        let mut query = ModelsQuery::default();
        query.limit = Some(5);
        for d in [DialectId::Gemini, DialectId::GeminiEmbeddings] {
            let page = decode_models(
                d,
                200,
                &HeaderList::new(),
                &fixture("recorded-list-page-size-5"),
            )
            .unwrap();
            let next = next_models_query(d, &query, &page).unwrap();
            assert_eq!(
                next.after_id.as_deref(),
                Some("Chltb2RlbHMvZ2VtbWEtNC0yNmItYTRiLWl0")
            );
            assert_eq!(next.limit, Some(5));
            assert_eq!(next_models_query(d, &next, &page), None);
            let path = encode_models_request(d, &next).unwrap().path;
            assert_eq!(
                path,
                "/v1beta/models?pageSize=5&pageToken=Chltb2RlbHMvZ2VtbWEtNC0yNmItYTRiLWl0"
            );
        }
        let mut page = ModelList::new(vec![ModelInfo::new("a"), ModelInfo::new("b")]);
        assert_eq!(
            next_models_query(DialectId::OpenAiChat, &query, &page),
            None
        );
        page.has_more = true;
        let next = next_models_query(DialectId::AnthropicMessages, &query, &page).unwrap();
        assert_eq!(next.after_id.as_deref(), Some("b"));
        assert_eq!(next_models_query(DialectId::Gemini, &query, &page), None);
    }
}
