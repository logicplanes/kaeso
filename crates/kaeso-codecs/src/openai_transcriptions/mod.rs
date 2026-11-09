mod profile;
mod reply;
mod request;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_edges;

use kaeso_kpp::{
    DialectId, KppError, ModelInfo, ModelList, ModelsQuery, Reply, Request, RequestKind,
    TranscriptionOperation,
};

use crate::codec::{
    Codec, EgressCtx, EncodeCtx, EncodedError, EncodedRequest, HeaderList, StreamDecoder,
    StreamEncoder,
};

pub use profile::{LanguageField, TimestampsField, TranscriptionBody, TranscriptionsProfile};
pub(crate) use request::default_name;
pub use request::{MAX_AUDIO_BYTES, MAX_FORM_PARTS};

const DIALECT: DialectId = DialectId::OpenAiTranscriptions;

/// OpenAI's transcription API: `POST /v1/audio/transcriptions` and `/v1/audio/translations`,
/// uploads in, transcripts out in every `response_format`, or `transcript.text.*` events,
/// and the providers that speak a form of it, each described by a [`TranscriptionsProfile`].
/// Apps' requests name their operation in the path and are decoded with
/// [`crate::dispatch::decode_transcription_request`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct OpenAiTranscriptions;

impl crate::codec::sealed::Sealed for OpenAiTranscriptions {}
impl crate::codec::sealed::Sealed for reply::Decoder {}
impl crate::codec::sealed::Sealed for reply::Encoder {}

pub(crate) fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
    crate::openai_responses::error::decode(DIALECT, status, headers, body)
}

impl OpenAiTranscriptions {
    pub(crate) fn decode_call(
        operation: TranscriptionOperation,
        body: &[u8],
        headers: &HeaderList,
    ) -> Result<Request, KppError> {
        request::decode(operation, crate::multipart::Upload::Borrowed(body), headers)
    }

    pub(crate) fn decode_upload(
        operation: TranscriptionOperation,
        body: crate::multipart::Upload<'_>,
        headers: &HeaderList,
    ) -> Result<Request, KppError> {
        request::decode(operation, body, headers)
    }
}

impl Codec for OpenAiTranscriptions {
    const DIALECT: DialectId = DIALECT;

    /// Decodes an upload without its path as a transcription.
    fn decode_request(
        _kind: RequestKind,
        body: &[u8],
        headers: &HeaderList,
    ) -> Result<Request, KppError> {
        crate::limits::request_shape(DIALECT, body)?;
        request::decode(
            TranscriptionOperation::Transcribe,
            crate::multipart::Upload::Borrowed(body),
            headers,
        )
    }

    fn encode_response(reply: &Reply, ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
        reply::encode(reply, ctx)
    }

    fn stream_encoder(_ctx: &EncodeCtx) -> Box<dyn StreamEncoder> {
        Box::new(reply::Encoder)
    }

    fn response_media_type(_reply: &Reply, ctx: &EncodeCtx) -> String {
        reply::app_media_type(ctx)
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

    fn decode_response(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
        ctx: &EgressCtx,
    ) -> Result<Reply, KppError> {
        reply::decode(status, headers, body, ctx)
    }

    fn stream_decoder(ctx: &EgressCtx) -> Box<dyn StreamDecoder> {
        Box::new(reply::Decoder::new(ctx))
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
