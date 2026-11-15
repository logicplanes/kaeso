//! Voyage AI's rerank API (<https://www.mongodb.com/docs/api/doc/atlas-embedding-and-reranking-api>,
//! `rerankDocuments`): `POST /v1/rerank` with a `model`, a `query`, `documents` as strings,
//! `top_k`, `return_documents` and `truncation`, answered with `data` sorted by
//! `relevance_score`, best first, and `usage.total_tokens`. Voyage refuses every field it doesn't
//! know with a 400, so the encoder writes only these.

mod error;
mod request;
mod response;
mod wire;

#[cfg(test)]
mod tests;

use kaeso_kpp::{
    DialectId, KppError, ModelInfo, ModelList, ModelsQuery, Origin, Reply, Request, RequestKind,
    StreamEvent,
};

use crate::codec::{
    Codec, EgressCtx, EncodeCtx, EncodedError, EncodedRequest, HeaderList, StreamDecoder,
    StreamEncoder,
};

const DIALECT: DialectId = DialectId::VoyageRerank;

/// Where Voyage AI serves its rerank API.
pub const RERANK_PATH: &str = "/v1/rerank";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct VoyageRerank;

impl crate::codec::sealed::Sealed for VoyageRerank {}
impl crate::codec::sealed::Sealed for NoStreamEncoder {}
impl crate::codec::sealed::Sealed for NoStreamDecoder {}

fn no_streaming() -> KppError {
    KppError::unsupported_by(
        "streaming",
        DIALECT,
        "The rerank endpoint returns one complete response; send the request without streaming.",
    )
}

/// The refusal every other kind of request gets; a decision request gets the one every codec
/// that isn't a decision codec gives.
pub(crate) fn not_rerank(kind: RequestKind, what: &str) -> KppError {
    if let Err(decision) = crate::decisions::refuse_kind(kind, DIALECT)
        && kind == RequestKind::Decide
    {
        return decision;
    }
    KppError::unsupported_by(
        what,
        DIALECT,
        "Send other requests to the endpoint of their kind, such as `/v1/chat/completions`.",
    )
}

struct NoStreamEncoder;

impl StreamEncoder for NoStreamEncoder {
    fn encode(&mut self, _event: &StreamEvent) -> Result<Vec<u8>, KppError> {
        Err(no_streaming())
    }

    fn finish(&mut self) -> Result<Vec<u8>, KppError> {
        Ok(Vec::new())
    }
}

struct NoStreamDecoder {
    origin: Origin,
    done: bool,
}

impl NoStreamDecoder {
    fn error(&mut self) -> Vec<StreamEvent> {
        if self.done {
            return Vec::new();
        }
        self.done = true;
        vec![StreamEvent::Error(
            no_streaming().with_origin(self.origin.clone()),
        )]
    }
}

impl StreamDecoder for NoStreamDecoder {
    fn feed(&mut self, _bytes: &[u8]) -> Vec<StreamEvent> {
        self.error()
    }

    fn finish(&mut self) -> Vec<StreamEvent> {
        self.error()
    }
}

impl VoyageRerank {
    pub(crate) fn decode_checked(
        kind: RequestKind,
        body: &[u8],
        headers: &HeaderList,
    ) -> Result<Request, KppError> {
        request::decode(kind, body, headers)
    }
}

impl Codec for VoyageRerank {
    const DIALECT: DialectId = DIALECT;

    fn decode_request(
        kind: RequestKind,
        body: &[u8],
        headers: &HeaderList,
    ) -> Result<Request, KppError> {
        crate::limits::request_shape(Self::DIALECT, body)?;
        Self::decode_checked(kind, body, headers)
    }

    fn encode_response(reply: &Reply, ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
        response::encode(reply, ctx)
    }

    fn stream_encoder(_ctx: &EncodeCtx) -> Box<dyn StreamEncoder> {
        Box::new(NoStreamEncoder)
    }

    fn encode_error(err: &KppError) -> EncodedError {
        error::encode(err)
    }

    fn check(req: &Request, ctx: &EgressCtx) -> Result<(), KppError> {
        request::check(req, &ctx.origin(DIALECT))
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
        response::decode(status, headers, body, ctx)
    }

    fn stream_decoder(ctx: &EgressCtx) -> Box<dyn StreamDecoder> {
        Box::new(NoStreamDecoder {
            origin: ctx.origin(DIALECT),
            done: false,
        })
    }

    fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
        error::decode(status, headers, body)
    }

    // Voyage AI lists no models (`GET /v1/models` answers 404, recorded 2026-11-14); the shared
    // OpenAI listing stands in for the trait.
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
