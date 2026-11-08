mod profile;
mod reply;
mod request;

#[cfg(test)]
mod tests;

use kaeso_kpp::{
    DialectId, KppError, ModelInfo, ModelList, ModelsQuery, Reply, Request, RequestKind,
};

use crate::codec::{
    Codec, EgressCtx, EncodeCtx, EncodedError, EncodedRequest, HeaderList, StreamDecoder,
    StreamEncoder,
};

pub use profile::{SpeechBody, SpeechEvents, SpeechProfile, SpeechReply};

const DIALECT: DialectId = DialectId::OpenAiSpeech;

/// OpenAI's speech API, `POST /v1/audio/speech`: JSON in, the audio out, or `speech.audio.*`
/// events with `stream_format: "sse"`, and the providers that speak a form of it, each
/// described by a [`SpeechProfile`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct OpenAiSpeech;

impl crate::codec::sealed::Sealed for OpenAiSpeech {}
impl crate::codec::sealed::Sealed for reply::Decoder {}
impl crate::codec::sealed::Sealed for reply::Encoder {}

pub(crate) fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
    crate::openai_responses::error::decode(DIALECT, status, headers, body)
}

impl OpenAiSpeech {
    pub(crate) fn decode_checked(
        _kind: RequestKind,
        body: &[u8],
        headers: &HeaderList,
    ) -> Result<Request, KppError> {
        request::decode(body, headers)
    }
}

impl Codec for OpenAiSpeech {
    const DIALECT: DialectId = DIALECT;

    fn decode_request(
        kind: RequestKind,
        body: &[u8],
        headers: &HeaderList,
    ) -> Result<Request, KppError> {
        crate::limits::request_shape(DIALECT, body)?;
        Self::decode_checked(kind, body, headers)
    }

    fn encode_response(reply: &Reply, ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
        reply::encode(reply, ctx)
    }

    fn stream_encoder(ctx: &EncodeCtx) -> Box<dyn StreamEncoder> {
        Box::new(reply::Encoder::new(!ctx.stream.is_off()))
    }

    fn response_media_type(reply: &Reply, ctx: &EncodeCtx) -> String {
        let audio = match reply {
            Reply::Speech(r) => Some(&r.audio),
            _ => None,
        };
        reply::app_media_type(ctx, audio)
    }

    fn stream_media_type(ctx: &EncodeCtx) -> String {
        reply::app_media_type(ctx, None)
    }

    fn encode_error(err: &KppError) -> EncodedError {
        crate::openai_responses::error::encode(err)
    }

    fn check(req: &Request, ctx: &EgressCtx) -> Result<(), KppError> {
        request::check(req, ctx)
    }

    fn encode_request(req: &Request, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
        request::encode(req, ctx)
    }

    fn reads_reply_as_stream(ctx: &EgressCtx) -> bool {
        reply::streams(ctx)
    }

    fn decode_response(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
        ctx: &EgressCtx,
    ) -> Result<Reply, KppError> {
        reply::decode(status, headers, body, ctx)
    }

    fn stream_decoder(ctx: &EgressCtx) -> Box<dyn StreamDecoder> {
        Box::new(reply::Decoder::new(ctx, !ctx.stream.is_off()))
    }

    fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
        decode_error(status, headers, body)
    }

    fn decode_models_query(query: &str) -> Result<ModelsQuery, KppError> {
        crate::openai_models::decode_query(DIALECT, query)
    }

    fn encode_models(list: &ModelList) -> Result<Vec<u8>, KppError> {
        crate::openai_models::encode_models(list)
    }

    fn encode_model(model: &ModelInfo) -> Result<Vec<u8>, KppError> {
        crate::openai_models::encode_model(model)
    }

    fn decode_models(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
    ) -> Result<ModelList, KppError> {
        if !(200..300).contains(&status) {
            return Err(Self::decode_error(status, headers, body));
        }
        crate::openai_models::decode_models(DIALECT, body)
    }

    fn decode_model(status: u16, headers: &HeaderList, body: &[u8]) -> Result<ModelInfo, KppError> {
        if !(200..300).contains(&status) {
            return Err(Self::decode_error(status, headers, body));
        }
        crate::openai_models::decode_model(DIALECT, body)
    }

    fn encode_model_request(model: &str) -> EncodedRequest {
        crate::openai_models::retrieve_request(model)
    }

    fn encode_models_request(query: &ModelsQuery) -> Result<EncodedRequest, KppError> {
        crate::openai_models::list_request(DIALECT, query)
    }
}
