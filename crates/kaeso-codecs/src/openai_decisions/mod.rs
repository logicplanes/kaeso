//! OpenAI's Decisions API (`POST /v1/decisions`): typed questions about an input of text,
//! images, JSON and conversation history, answered with probabilities.
//!
//! Answers come back in question order, so a reply is read by position against the questions
//! the request sent. The [`DecisionsProfile`] of the provider holds its path and limits.

mod parts;
mod profile;
mod request;
mod response;
mod state;
mod wire;

#[cfg(test)]
mod tests;

pub use profile::DecisionsProfile;

use kaeso_kpp::{
    DialectId, KppError, ModelInfo, ModelList, ModelsQuery, Origin, Reply, Request, RequestKind,
    StreamEvent,
};

use crate::codec::{
    Codec, EgressCtx, EncodeCtx, EncodedError, EncodedRequest, HeaderList, StreamDecoder,
    StreamEncoder,
};

pub(crate) const DIALECT: DialectId = DialectId::OpenAiDecisions;

/// The path apps send OpenAI Decisions requests to.
pub const DECISIONS_PATH: &str = "/v1/decisions";

// Model listings are OpenAI's.
const LISTING: DialectId = DialectId::OpenAiChat;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct OpenAiDecisions;

impl crate::codec::sealed::Sealed for OpenAiDecisions {}
impl crate::codec::sealed::Sealed for NoStreamEncoder {}
impl crate::codec::sealed::Sealed for NoStreamDecoder {}

fn no_streaming() -> KppError {
    KppError::unsupported_by(
        "streaming",
        DIALECT,
        "Decision requests return one complete response; send the request without streaming.",
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

impl OpenAiDecisions {
    pub(crate) fn decode_checked(
        kind: RequestKind,
        body: &[u8],
        headers: &HeaderList,
    ) -> Result<Request, KppError> {
        request::decode(kind, body, headers)
    }
}

impl Codec for OpenAiDecisions {
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
        response::decode(status, headers, body, ctx)
    }

    fn stream_decoder(ctx: &EgressCtx) -> Box<dyn StreamDecoder> {
        Box::new(NoStreamDecoder {
            origin: ctx.origin(DIALECT),
            done: false,
        })
    }

    fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
        crate::openai_responses::error::decode(DIALECT, status, headers, body)
    }

    fn decode_models_query(query: &str) -> Result<ModelsQuery, KppError> {
        crate::openai_models::decode_query(LISTING, query)
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
        crate::openai_models::decode_models(LISTING, body)
    }

    fn decode_model(status: u16, headers: &HeaderList, body: &[u8]) -> Result<ModelInfo, KppError> {
        if !(200..300).contains(&status) {
            return Err(Self::decode_error(status, headers, body));
        }
        crate::openai_models::decode_model(LISTING, body)
    }

    fn encode_model_request(model: &str) -> EncodedRequest {
        crate::openai_models::retrieve_request(model)
    }

    fn encode_models_request(query: &ModelsQuery) -> Result<EncodedRequest, KppError> {
        crate::openai_models::list_request(LISTING, query)
    }
}
