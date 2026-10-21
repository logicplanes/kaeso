mod blocks;
mod check;
mod common;
mod error;
mod models;
mod request;
mod response;
mod stream;
mod stream_encoder;
mod wire;

#[cfg(test)]
mod tests;

use kaeso_kpp::{
    DialectId, KppError, ModelInfo, ModelList, ModelsQuery, Reply, Request, RequestKind,
};

use crate::codec::{
    Codec, EgressCtx, EncodeCtx, EncodedError, EncodedRequest, HeaderList, StreamDecoder,
    StreamEncoder,
};

pub const ANTHROPIC_VERSION: &str = "2023-06-01";

pub const MESSAGES_PATH: &str = "/v1/messages";

pub const COUNT_TOKENS_PATH: &str = "/v1/messages/count_tokens";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct AnthropicMessages;

impl crate::codec::sealed::Sealed for AnthropicMessages {}
impl crate::codec::sealed::Sealed for stream::Decoder {}
impl crate::codec::sealed::Sealed for stream_encoder::Encoder {}

impl Codec for AnthropicMessages {
    const DIALECT: DialectId = DialectId::AnthropicMessages;

    fn decode_request(
        kind: RequestKind,
        body: &[u8],
        headers: &HeaderList,
    ) -> Result<Request, KppError> {
        crate::limits::request_shape(Self::DIALECT, body)?;
        request::decode_request(kind, body, headers)
    }

    fn encode_response(reply: &Reply, ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
        response::encode_response(reply, ctx)
    }

    fn stream_encoder(ctx: &EncodeCtx) -> Box<dyn StreamEncoder> {
        Box::new(stream_encoder::Encoder::new(ctx))
    }

    fn encode_error(err: &KppError) -> EncodedError {
        error::encode_error(err, None)
    }

    fn encode_error_for_request(err: &KppError, request_id: &str) -> EncodedError {
        error::encode_error(err, Some(request_id))
    }

    fn check(req: &Request, ctx: &EgressCtx) -> Result<(), KppError> {
        check::check(req, &ctx.origin(Self::DIALECT))
    }

    fn encode_request(req: &Request, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
        check::check(req, &ctx.origin(Self::DIALECT))?;
        request::encode_request(req, ctx)
    }

    fn decode_response(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
        ctx: &EgressCtx,
    ) -> Result<Reply, KppError> {
        response::decode_response(status, headers, body, ctx)
    }

    fn stream_decoder(ctx: &EgressCtx) -> Box<dyn StreamDecoder> {
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
        models::retrieve_request(model)
    }

    fn encode_models_request(query: &ModelsQuery) -> Result<EncodedRequest, KppError> {
        models::list_request(query)
    }
}
