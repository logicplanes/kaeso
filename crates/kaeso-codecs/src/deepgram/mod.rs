//! Deepgram's own API (<https://developers.deepgram.com/reference/deepgram-api-overview>):
//! pre-recorded speech to text, `POST /v1/listen`, and Aura text to speech, `POST /v1/speak`.
//! Every option travels in the query string; `/v1/listen` takes the audio itself as the body
//! (or JSON with a `url`) and answers JSON, `/v1/speak` takes `{"text"}` and answers the audio.
//! The wire is pinned to the OpenAPI document of `deepgram/deepgram-api-specs` at commit
//! `0ccc6f4680b3fe5b8f9002be6a78cd50e5b32116` (read 2026-11-14).

mod error;
mod listen;
mod listing;
mod speak;

#[cfg(test)]
mod tests;

use kaeso_kpp::{
    DialectId, KppError, ModelInfo, ModelList, ModelsQuery, Reply, Request, RequestKind,
};

use crate::codec::{
    Codec, EgressCtx, EncodeCtx, EncodedError, EncodedRequest, HeaderList, StreamDecoder,
    StreamEncoder,
};

const D: DialectId = DialectId::Deepgram;

/// The path of Deepgram's pre-recorded speech-to-text API.
pub const LISTEN_PATH: &str = "/v1/listen";

/// The path of Deepgram's Aura text-to-speech API.
pub const SPEAK_PATH: &str = "/v1/speak";

/// The extras key that holds the query parameters Kaeso doesn't type, as `[name, value]`
/// pairs in their order, a repeated parameter (`keyterm`) once per value.
pub const QUERY: &str = "query";

pub use speak::{MAX_SPEECH_CHARACTERS, SPEECH_FAMILIES};

/// The characters Deepgram billed a speech request, from its `dg-char-count` response header
/// (<https://developers.deepgram.com/docs/text-to-speech>).
pub fn billed_characters(headers: &HeaderList) -> Option<u64> {
    headers.get("dg-char-count")?.trim().parse().ok()
}

/// Deepgram's id of a request, from its `dg-request-id` response header.
pub fn request_id(headers: &HeaderList) -> Option<&str> {
    headers.get("dg-request-id")
}

/// Deepgram's speech-to-text and text-to-speech APIs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Deepgram;

impl crate::codec::sealed::Sealed for Deepgram {}
impl crate::codec::sealed::Sealed for speak::Decoder {}
impl crate::codec::sealed::Sealed for speak::Encoder {}

fn not_audio(kind: RequestKind) -> KppError {
    KppError::unsupported_by(
        format!("{kind:?} request"),
        D,
        "Deepgram transcribes audio and makes speech from text.",
    )
}

impl Deepgram {
    /// Decodes a Deepgram request: `kind` says whether it is for `/v1/listen` or `/v1/speak`,
    /// `query` is its query string, which carries the model and every option, and `headers`
    /// its `content-type`, which tells uploaded audio from JSON.
    pub fn decode_call(
        kind: RequestKind,
        query: &str,
        body: &[u8],
        headers: &HeaderList,
    ) -> Result<Request, KppError> {
        crate::decisions::refuse_kind(kind, D)?;
        let pairs = crate::models::query_pairs(query)?;
        match kind {
            RequestKind::Transcription => listen::decode_request(pairs, body, headers),
            RequestKind::Speech => {
                crate::limits::request_shape(D, body)?;
                speak::decode_request(pairs, body)
            }
            other => Err(not_audio(other)),
        }
    }
}

impl Codec for Deepgram {
    const DIALECT: DialectId = D;

    /// Reads a body without its query string, so without a model; requests that carry their
    /// options go through [`Deepgram::decode_call`].
    fn decode_request(
        kind: RequestKind,
        body: &[u8],
        headers: &HeaderList,
    ) -> Result<Request, KppError> {
        Self::decode_call(kind, "", body, headers)
    }

    fn encode_response(reply: &Reply, _ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
        crate::decisions::refuse_reply(reply, D)?;
        match reply {
            Reply::Transcription(r) => listen::encode_response(r),
            Reply::Speech(r) => speak::encode_response(r),
            other => Err(not_audio(other.kind())),
        }
    }

    fn stream_encoder(ctx: &EncodeCtx) -> Box<dyn StreamEncoder> {
        Box::new(speak::Encoder::new(ctx.kind))
    }

    fn response_media_type(reply: &Reply, _ctx: &EncodeCtx) -> String {
        match reply {
            Reply::Speech(r) => speak::media_type_of_reply(r),
            _ => crate::codec::JSON_MEDIA_TYPE.to_owned(),
        }
    }

    fn stream_media_type(ctx: &EncodeCtx) -> String {
        match ctx.request.as_deref() {
            Some(Request::Speech(r)) => speak::media_type_of_request(r),
            _ => crate::codec::JSON_MEDIA_TYPE.to_owned(),
        }
    }

    fn encode_error(err: &KppError) -> EncodedError {
        error::encode(err)
    }

    fn check(req: &Request, _ctx: &EgressCtx) -> Result<(), KppError> {
        crate::decisions::refuse_request(req, D)?;
        match req {
            Request::Transcription(r) => listen::check(r),
            Request::Speech(r) => speak::check(r).map(|_| ()),
            other => Err(not_audio(other.kind())),
        }
    }

    fn encode_request(req: &Request, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
        crate::decisions::refuse_request(req, D)?;
        match req {
            Request::Transcription(r) => listen::encode_request(r, ctx),
            Request::Speech(r) => speak::encode_request(r, ctx),
            other => Err(not_audio(other.kind())),
        }
    }

    fn reads_reply_as_stream(ctx: &EgressCtx) -> bool {
        ctx.kind == RequestKind::Speech
    }

    fn decode_response(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
        ctx: &EgressCtx,
    ) -> Result<Reply, KppError> {
        if !(200..300).contains(&status) {
            return Err(error::decode(status, headers, body));
        }
        match ctx.kind {
            RequestKind::Transcription => listen::decode_response(body, ctx),
            RequestKind::Speech => Ok(speak::decode_response(headers, body, ctx)),
            other => Err(not_audio(other)),
        }
    }

    fn stream_decoder(ctx: &EgressCtx) -> Box<dyn StreamDecoder> {
        Box::new(speak::Decoder::new(ctx))
    }

    fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
        error::decode(status, headers, body)
    }

    fn decode_models_query(query: &str) -> Result<ModelsQuery, KppError> {
        crate::openai_models::decode_query(D, query)
    }

    fn encode_models(list: &ModelList) -> Result<Vec<u8>, KppError> {
        listing::encode(list)
    }

    fn encode_model(model: &ModelInfo) -> Result<Vec<u8>, KppError> {
        listing::encode_one(model)
    }

    fn decode_models(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
    ) -> Result<ModelList, KppError> {
        if !(200..300).contains(&status) {
            return Err(error::decode(status, headers, body));
        }
        listing::decode(body)
    }

    fn decode_model(status: u16, headers: &HeaderList, body: &[u8]) -> Result<ModelInfo, KppError> {
        if !(200..300).contains(&status) {
            return Err(error::decode(status, headers, body));
        }
        listing::decode_one(body)
    }

    fn encode_model_request(model: &str) -> EncodedRequest {
        crate::openai_models::retrieve_request(model)
    }

    fn encode_models_request(query: &ModelsQuery) -> Result<EncodedRequest, KppError> {
        crate::openai_models::list_request(D, query)
    }
}
