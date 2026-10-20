mod check;
mod convert;
mod error;
mod profile;
mod request;
mod request_encode;
mod response;
mod stream;
mod stream_encode;
mod wire;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_check;
#[cfg(test)]
mod tests_cross;
#[cfg(test)]
mod tests_deepseek;
#[cfg(test)]
mod tests_malformed;
#[cfg(test)]
mod tests_profile;
#[cfg(test)]
mod tests_reasoning_fields;
#[cfg(test)]
mod tests_regressions;
#[cfg(test)]
mod tests_stream;
#[cfg(test)]
mod tests_thinking;

use kaeso_kpp::{
    DialectId, GenerateRequest, KppError, ModelInfo, ModelList, ModelsQuery, Reply, Request,
    RequestKind,
};

use crate::codec::{
    Codec, EgressCtx, EncodeCtx, EncodedError, EncodedRequest, HeaderList, StreamDecoder,
    StreamEncoder, is_forwarded_header,
};
use crate::wire::to_body;

pub use profile::{
    ChatProfile, OutputLimit, ReplyProfile, ReplyReasoning, RequestProfile, SeedField,
};

pub(crate) const DIALECT: DialectId = DialectId::OpenAiChat;

pub(crate) fn kind_feature(kind: RequestKind) -> &'static str {
    match kind {
        RequestKind::CountTokens => "token counting",
        RequestKind::Embed => "embeddings",
        _ => "this endpoint",
    }
}

pub(crate) fn output_limit_field(r: &GenerateRequest) -> &'static str {
    let sent_null = |key: &str| {
        r.extras
            .form_value(DIALECT, crate::wire::VERBATIM_FORM_KEY)
            .and_then(|fields| fields.get(key))
            .is_some()
    };
    match r.extras.form_str(DIALECT, request::FORM_MAX_TOKENS) {
        Some("max_tokens") => "max_tokens",
        Some(_) => "max_completion_tokens",
        None if r.sampling.max_output_tokens.is_none()
            && sent_null("max_tokens")
            && !sent_null("max_completion_tokens") =>
        {
            "max_tokens"
        }
        None => "max_completion_tokens",
    }
}

pub const CHAT_COMPLETIONS_PATH: &str = "/v1/chat/completions";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct OpenAiChat;

impl crate::codec::sealed::Sealed for OpenAiChat {}
impl crate::codec::sealed::Sealed for stream::ChatStreamDecoder {}
impl crate::codec::sealed::Sealed for stream_encode::ChatStreamEncoder {}

impl Codec for OpenAiChat {
    const DIALECT: DialectId = DIALECT;

    fn decode_request(
        kind: RequestKind,
        body: &[u8],
        headers: &HeaderList,
    ) -> Result<Request, KppError> {
        request::decode_request(kind, body, headers)
    }

    fn encode_response(reply: &Reply, ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
        response::encode_response(reply, ctx)
    }

    fn stream_encoder(ctx: &EncodeCtx) -> Box<dyn StreamEncoder> {
        Box::new(stream_encode::ChatStreamEncoder::new(ctx))
    }

    fn encode_error(err: &KppError) -> EncodedError {
        error::encode_error(err)
    }

    fn check(req: &Request, ctx: &EgressCtx) -> Result<(), KppError> {
        check::check(req, &ctx.origin(DIALECT))
    }

    fn encode_request(req: &Request, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
        check::check(req, &ctx.origin(DIALECT))?;
        let Request::Generate(generate) = req else {
            return Err(KppError::unsupported_by(
                kind_feature(req.kind()),
                DIALECT,
                "Chat Completions only generates responses; send token counting and embeddings requests to an API that offers them (Anthropic Messages, OpenAI Responses or OpenAI Embeddings).",
            ));
        };
        let wire = request_encode::encode_generate(generate, ctx)?;
        ctx.chat_profile.refuse(generate, &wire)?;
        let body = to_body(&wire)?;
        let mut headers = HeaderList::new().with("content-type", "application/json");
        if generate.extras.is_from(DIALECT) {
            for (name, value) in generate.extras.headers() {
                if is_forwarded_header(DIALECT, name) {
                    headers.push(name, value);
                }
            }
        }
        Ok(EncodedRequest::post_json(
            CHAT_COMPLETIONS_PATH,
            headers,
            body,
        ))
    }

    fn decode_response(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
        ctx: &EgressCtx,
    ) -> Result<Reply, KppError> {
        if !(200..300).contains(&status) {
            return Err(error::decode_error(status, headers, body));
        }
        let probe = std::str::from_utf8(body).map(error::probe);
        if probe.is_ok_and(|p| p.error && !p.choices) {
            return Err(error::decode_body(None, body));
        }
        response::decode_response(body, ctx)
    }

    fn stream_decoder(ctx: &EgressCtx) -> Box<dyn StreamDecoder> {
        Box::new(stream::ChatStreamDecoder::new(ctx))
    }

    fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
        error::decode_error(status, headers, body)
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
