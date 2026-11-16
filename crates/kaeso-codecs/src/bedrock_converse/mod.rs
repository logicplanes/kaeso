//! Amazon Bedrock's Converse API (`POST /model/{modelId}/converse` and `/converse-stream`), the
//! one format Bedrock serves every chat model in, such as Amazon Nova and Meta Llama
//! (<https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_Converse.html>).
//!
//! Egress only: Kaeso sends Converse to Bedrock, and no app sends it to Kaeso. A stream answers
//! in AWS's binary event stream encoding, read by [`eventstream`].

mod error;
pub mod eventstream;
mod request;
mod response;
mod stream;

#[cfg(test)]
mod tests;

use kaeso_kpp::{DialectId, KppError, Origin, Reply, Request, RequestKind};

use crate::video_jobs::openai_listing;

use crate::codec::{
    Codec, EgressCtx, EncodeCtx, EncodedError, EncodedRequest, HeaderList, StreamDecoder,
    StreamEncoder,
};

pub(crate) const D: DialectId = DialectId::BedrockConverse;

pub struct BedrockConverse;

impl crate::codec::sealed::Sealed for BedrockConverse {}
impl crate::codec::sealed::Sealed for stream::Decoder {}

fn egress_only() -> KppError {
    KppError::unsupported(
        "Converse requests",
        "Kaeso sends Amazon Bedrock Converse to Bedrock but takes no Converse requests; send the request in an OpenAI, Anthropic or Gemini format.",
    )
}

impl Codec for BedrockConverse {
    const DIALECT: DialectId = D;

    fn decode_request(
        kind: RequestKind,
        _body: &[u8],
        _headers: &HeaderList,
    ) -> Result<Request, KppError> {
        crate::decisions::refuse_kind(kind, D)?;
        Err(egress_only())
    }

    fn encode_response(reply: &Reply, _ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
        crate::decisions::refuse_reply(reply, D)?;
        Err(egress_only())
    }

    fn stream_encoder(_ctx: &EncodeCtx) -> Box<dyn StreamEncoder> {
        crate::video_jobs::no_stream(Origin::new(D))
    }

    fn encode_error(err: &KppError) -> EncodedError {
        crate::openai_responses::error::encode(err)
    }

    fn check(req: &Request, ctx: &EgressCtx) -> Result<(), KppError> {
        crate::decisions::refuse_request(req, D)?;
        let req = crate::adapt::for_target(req, D, ctx)?;
        request::check(&req, ctx)
    }

    fn encode_request(req: &Request, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
        crate::decisions::refuse_request(req, D)?;
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
        if !(200..300).contains(&status) {
            return Err(error::decode(status, headers, body));
        }
        response::decode(headers, body, ctx).map(Reply::Generate)
    }

    fn stream_decoder(ctx: &EgressCtx) -> Box<dyn StreamDecoder> {
        Box::new(stream::Decoder::new(ctx))
    }

    fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
        error::decode(status, headers, body)
    }

    openai_listing!(D);
}
