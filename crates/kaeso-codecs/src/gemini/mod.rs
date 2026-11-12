mod audio;
pub(crate) mod error;
mod images;
pub(crate) mod models;
pub(crate) mod parts;
mod path;
mod request;
mod response;
mod sdk;
mod speech;
pub(crate) mod stream;
mod wire;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_audio;
#[cfg(test)]
mod tests_edges;
#[cfg(test)]
mod tests_images;

use kaeso_kpp::{
    DialectId, KppError, ModelInfo, ModelList, ModelsQuery, Reply, Request, RequestKind,
};

use crate::codec::{
    Codec, EgressCtx, EncodeCtx, EncodedError, EncodedRequest, HeaderList, StreamDecoder,
    StreamEncoder,
};

pub use path::{GeminiAction, GeminiProfile, split_model_action};

pub(crate) use error::{decode_error_value, encode_error as encode_google_error, is_google_error};

const D: DialectId = DialectId::Gemini;

/// The Gemini API: `generateContent`, `streamGenerateContent`, `countTokens` and the model
/// listing. A Gemini request names its model and method in the path, so apps' requests are
/// decoded with [`crate::dispatch::decode_gemini_request`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Gemini;

impl crate::codec::sealed::Sealed for Gemini {}
impl crate::codec::sealed::Sealed for stream::Decoder {}
impl crate::codec::sealed::Sealed for stream::Encoder {}

impl Gemini {
    pub(crate) fn decode_call(
        model: &str,
        action: GeminiAction,
        body: &[u8],
        headers: &HeaderList,
    ) -> Result<Request, KppError> {
        request::decode(model, action, body, headers)
    }
}

impl Codec for Gemini {
    const DIALECT: DialectId = D;

    /// Decodes a body without its path: the model is empty and a generation request is
    /// `generateContent`. Apps' requests go through
    /// [`crate::dispatch::decode_gemini_request`], which takes both.
    fn decode_request(
        kind: RequestKind,
        body: &[u8],
        headers: &HeaderList,
    ) -> Result<Request, KppError> {
        crate::decisions::refuse_kind(kind, Self::DIALECT)?;
        crate::limits::request_shape(D, body)?;
        let action = match kind {
            RequestKind::CountTokens => GeminiAction::CountTokens,
            RequestKind::Embed => GeminiAction::EmbedContent,
            _ => GeminiAction::GenerateContent,
        };
        request::decode("", action, body, headers)
    }

    fn encode_response(reply: &Reply, ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
        crate::decisions::refuse_reply(reply, Self::DIALECT)?;
        response::encode_response(reply, ctx)
    }

    fn stream_encoder(ctx: &EncodeCtx) -> Box<dyn StreamEncoder> {
        Box::new(stream::Encoder::new(ctx))
    }

    fn encode_error(err: &KppError) -> EncodedError {
        error::encode_error(err, D)
    }

    fn check(req: &Request, ctx: &EgressCtx) -> Result<(), KppError> {
        crate::decisions::refuse_request(req, Self::DIALECT)?;
        match req {
            Request::Image(r) => return images::check(r, ctx.gemini_profile.image_output_options),
            Request::Speech(r) => return audio::check_speech(r),
            Request::Transcription(r) => return audio::check_transcription(r),
            _ => {}
        }
        let req = crate::adapt::for_target(req, D, ctx)?;
        request::check(&req, ctx)
    }

    fn encode_request(req: &Request, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
        crate::decisions::refuse_request(req, Self::DIALECT)?;
        match req {
            Request::Image(r) => return images::encode(r, ctx),
            Request::Speech(r) => return audio::encode_speech(r, ctx),
            Request::Transcription(r) => return audio::encode_transcription(r, ctx),
            _ => {}
        }
        let crate::adapt::Adapted {
            req,
            left_out,
            adapted,
            ..
        } = crate::adapt::for_target(req, D, ctx)?;
        ctx.left_out = left_out;
        ctx.adapted = adapted;
        request::check(&req, ctx)?;
        request::encode(&req, ctx)
    }

    fn decode_response(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
        ctx: &EgressCtx,
    ) -> Result<Reply, KppError> {
        if (200..300).contains(&status) {
            match ctx.kind {
                RequestKind::Image => return images::decode(body, &ctx.origin(D)),
                RequestKind::Speech => return audio::decode_speech(body, &ctx.origin(D)),
                RequestKind::Transcription => {
                    return audio::decode_transcription(body, &ctx.origin(D));
                }
                _ => {}
            }
        }
        response::decode_response(status, headers, body, ctx)
    }

    fn stream_decoder(ctx: &EgressCtx) -> Box<dyn StreamDecoder> {
        if ctx.kind == RequestKind::Speech {
            return Box::new(audio::SpeechStream::new(ctx));
        }
        Box::new(stream::Decoder::new(ctx))
    }

    fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
        error::decode_error(status, headers, body)
    }

    fn decode_models_query(query: &str) -> Result<ModelsQuery, KppError> {
        models::decode_query(query)
    }

    fn encode_models(list: &ModelList) -> Result<Vec<u8>, KppError> {
        models::encode_models(list)
    }

    fn encode_model(model: &ModelInfo) -> Result<Vec<u8>, KppError> {
        models::encode_model(model)
    }

    fn decode_models(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
    ) -> Result<ModelList, KppError> {
        if !(200..300).contains(&status) {
            return Err(Self::decode_error(status, headers, body));
        }
        models::decode_models(body)
    }

    fn decode_model(status: u16, headers: &HeaderList, body: &[u8]) -> Result<ModelInfo, KppError> {
        if !(200..300).contains(&status) {
            return Err(Self::decode_error(status, headers, body));
        }
        models::decode_model(body)
    }

    fn encode_model_request(model: &str) -> EncodedRequest {
        Self::encode_model_request_in(model, &GeminiProfile::GOOGLE_AI_STUDIO).unwrap_or_else(
            |_| {
                EncodedRequest::get(
                    GeminiProfile::GOOGLE_AI_STUDIO.list_path().to_owned(),
                    HeaderList::new(),
                )
            },
        )
    }

    fn encode_models_request(query: &ModelsQuery) -> Result<EncodedRequest, KppError> {
        models::list_request(query, &GeminiProfile::GOOGLE_AI_STUDIO)
    }

    fn encode_model_request_with(model: &str, ctx: &EgressCtx) -> Result<EncodedRequest, KppError> {
        models::retrieve_request(model, &ctx.gemini_profile)
    }

    fn encode_models_request_with(
        query: &ModelsQuery,
        ctx: &EgressCtx,
    ) -> Result<EncodedRequest, KppError> {
        models::list_request(query, &ctx.gemini_profile)
    }
}

impl Gemini {
    fn encode_model_request_in(
        model: &str,
        profile: &GeminiProfile,
    ) -> Result<EncodedRequest, KppError> {
        models::retrieve_request(model, profile)
    }
}
