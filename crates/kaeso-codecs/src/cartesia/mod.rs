//! Cartesia's API (<https://docs.cartesia.ai/use-the-api/api-conventions>, version
//! [`VERSION`], read 2026-11-14): Sonic text to speech, `POST /tts/bytes`, which takes JSON and
//! answers the audio as it is made, and Ink batch speech to text, `POST /stt`, which takes a
//! `multipart/form-data` upload and answers JSON. Every request carries the `Cartesia-Version`
//! header; the wire is pinned to the reference pages of that version.

mod error;
mod stt;
mod tts;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_edges;

use kaeso_kpp::{
    DialectId, KppError, ModelInfo, ModelList, ModelsQuery, Reply, Request, RequestKind,
};

use crate::codec::{
    Codec, EgressCtx, EncodeCtx, EncodedError, EncodedRequest, HeaderList, StreamDecoder,
    StreamEncoder,
};

const D: DialectId = DialectId::Cartesia;
const PROVIDER: &str = "Cartesia";

/// The API version Kaeso speaks, sent as `Cartesia-Version` on every request: the version of
/// the reference pages the dialect follows, which Cartesia names as its latest in the error it
/// answers a request without the header with.
pub const VERSION: &str = "2026-08-14";

const VERSION_HEADER: &str = "cartesia-version";

/// The path of Sonic text to speech.
pub const TTS_PATH: &str = "/tts/bytes";

/// The path of Ink batch speech to text.
pub const STT_PATH: &str = "/stt";

/// The path the model check asks: one voice of the voice library, which needs a valid key.
pub const VOICES_PATH: &str = "/voices?limit=1";

/// Cartesia's text-to-speech and speech-to-text APIs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Cartesia;

impl crate::codec::sealed::Sealed for Cartesia {}
impl crate::codec::sealed::Sealed for tts::Decoder {}
impl crate::codec::sealed::Sealed for tts::Encoder {}

fn not_audio(kind: RequestKind) -> KppError {
    KppError::unsupported_by(
        format!("{kind:?} request"),
        D,
        "Cartesia makes speech from text and transcribes audio.",
    )
}

fn versioned(headers: HeaderList) -> HeaderList {
    headers.with(VERSION_HEADER, VERSION)
}

impl Codec for Cartesia {
    const DIALECT: DialectId = D;

    /// Reads a `/tts/bytes` body. Kaeso writes `/stt` uploads but doesn't read them: no app
    /// sends Cartesia's own transcription form to Kaeso.
    fn decode_request(
        kind: RequestKind,
        body: &[u8],
        _headers: &HeaderList,
    ) -> Result<Request, KppError> {
        crate::decisions::refuse_kind(kind, D)?;
        match kind {
            RequestKind::Speech => {
                crate::limits::request_shape(D, body)?;
                tts::decode_request(body)
            }
            other => Err(not_audio(other)),
        }
    }

    fn encode_response(reply: &Reply, _ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
        crate::decisions::refuse_reply(reply, D)?;
        match reply {
            Reply::Transcription(r) => stt::encode_response(r),
            Reply::Speech(r) => tts::encode_response(r),
            other => Err(not_audio(other.kind())),
        }
    }

    fn stream_encoder(ctx: &EncodeCtx) -> Box<dyn StreamEncoder> {
        Box::new(tts::Encoder::new(ctx.kind))
    }

    fn response_media_type(reply: &Reply, _ctx: &EncodeCtx) -> String {
        match reply {
            Reply::Speech(r) => tts::media_type_of_reply(r),
            _ => crate::codec::JSON_MEDIA_TYPE.to_owned(),
        }
    }

    fn stream_media_type(ctx: &EncodeCtx) -> String {
        match ctx.request.as_deref() {
            Some(Request::Speech(r)) => tts::media_type_of(r.format.as_ref()),
            _ => crate::codec::JSON_MEDIA_TYPE.to_owned(),
        }
    }

    fn encode_error(err: &KppError) -> EncodedError {
        error::encode(err)
    }

    fn check(req: &Request, _ctx: &EgressCtx) -> Result<(), KppError> {
        crate::decisions::refuse_request(req, D)?;
        match req {
            Request::Transcription(r) => stt::check(r),
            Request::Speech(r) => tts::check(r),
            other => Err(not_audio(other.kind())),
        }
    }

    fn encode_request(req: &Request, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
        crate::decisions::refuse_request(req, D)?;
        match req {
            Request::Transcription(r) => stt::encode_request(r, ctx),
            Request::Speech(r) => tts::encode_request(r, ctx),
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
            RequestKind::Transcription => stt::decode_response(body, ctx).map(Reply::Transcription),
            RequestKind::Speech => Ok(tts::decode_response(headers, body, ctx)),
            other => Err(not_audio(other)),
        }
    }

    fn stream_decoder(ctx: &EgressCtx) -> Box<dyn StreamDecoder> {
        Box::new(tts::Decoder::new(ctx))
    }

    fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
        error::decode(status, headers, body)
    }

    fn decode_models_query(query: &str) -> Result<ModelsQuery, KppError> {
        crate::openai_models::decode_query(D, query)
    }

    fn encode_models(list: &ModelList) -> Result<Vec<u8>, KppError> {
        crate::openai_models::encode_models(list)
    }

    fn encode_model(model: &ModelInfo) -> Result<Vec<u8>, KppError> {
        crate::openai_models::encode_model(model)
    }

    /// Reads the voice library's first page, which only shows that the key works: Cartesia
    /// lists no models (`GET /voices`, <https://docs.cartesia.ai/api-reference/voices/list>).
    fn decode_models(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
    ) -> Result<ModelList, KppError> {
        if !(200..300).contains(&status) {
            return Err(error::decode(status, headers, body));
        }
        let v: serde_json::Value = serde_json::from_slice(body).map_err(|_| {
            KppError::provider(format!("{PROVIDER} sent a voice list that is not JSON."))
        })?;
        if !v.get("data").is_some_and(serde_json::Value::is_array) {
            return Err(KppError::provider(format!(
                "{PROVIDER} sent a voice list without `data`."
            )));
        }
        Ok(ModelList::new(Vec::new()))
    }

    fn decode_model(status: u16, headers: &HeaderList, body: &[u8]) -> Result<ModelInfo, KppError> {
        Self::decode_models(status, headers, body)?;
        Err(KppError::unsupported_by(
            "a model lookup",
            D,
            "Cartesia lists no models.",
        ))
    }

    fn encode_model_request(_model: &str) -> EncodedRequest {
        EncodedRequest::get(VOICES_PATH, versioned(HeaderList::new()))
    }

    fn encode_models_request(_query: &ModelsQuery) -> Result<EncodedRequest, KppError> {
        Ok(EncodedRequest::get(
            VOICES_PATH,
            versioned(HeaderList::new()),
        ))
    }
}
