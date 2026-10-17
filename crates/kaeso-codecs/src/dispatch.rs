use kaeso_kpp::{
    DialectId, ErrorKind, KppError, ModelInfo, ModelList, ModelsQuery, Reply, Request, RequestKind,
};

use crate::anthropic_messages::AnthropicMessages;
use crate::openai_chat::OpenAiChat;
use crate::openai_embeddings::OpenAiEmbeddings;
use crate::openai_responses::OpenAiResponses;
use crate::{
    Codec, EgressCtx, EncodeCtx, EncodedError, EncodedRequest, HeaderList, StreamDecoder,
    StreamEncoder,
};

macro_rules! dispatch {
    ($dialect:expr, $f:ident ( $($arg:expr),* )) => {
        match $dialect {
            DialectId::AnthropicMessages => Ok(AnthropicMessages::$f($($arg),*)),
            DialectId::OpenAiChat => Ok(OpenAiChat::$f($($arg),*)),
            DialectId::OpenAiResponses => Ok(OpenAiResponses::$f($($arg),*)),
            DialectId::OpenAiEmbeddings => Ok(OpenAiEmbeddings::$f($($arg),*)),
            other => Err(no_codec(other)),
        }
    };
}

fn no_codec(dialect: DialectId) -> KppError {
    KppError::unsupported(
        dialect.as_str(),
        format!(
            "This version of Kaeso has no codec for {}.",
            dialect.display_name()
        ),
    )
}

pub fn decode_request(
    dialect: DialectId,
    kind: RequestKind,
    body: &[u8],
    headers: &HeaderList,
) -> Result<Request, KppError> {
    dispatch!(dialect, decode_request(kind, body, headers))?
}

pub fn encode_response(
    dialect: DialectId,
    reply: &Reply,
    ctx: &EncodeCtx,
) -> Result<Vec<u8>, KppError> {
    dispatch!(dialect, encode_response(reply, ctx))?
}

pub fn stream_encoder(
    dialect: DialectId,
    ctx: &EncodeCtx,
) -> Result<Box<dyn StreamEncoder>, KppError> {
    dispatch!(dialect, stream_encoder(ctx))
}

pub fn encode_error(dialect: DialectId, err: &KppError) -> EncodedError {
    dispatch!(dialect, encode_error(err)).unwrap_or_else(|_| no_codec_error(err))
}

pub fn encode_error_for_request(
    dialect: DialectId,
    err: &KppError,
    request_id: &str,
) -> EncodedError {
    dispatch!(dialect, encode_error_for_request(err, request_id))
        .unwrap_or_else(|_| no_codec_error(err))
}

fn no_codec_error(err: &KppError) -> EncodedError {
    let body = serde_json::json!({ "error": { "message": err.message } });
    EncodedError::new(
        err.status.unwrap_or(500),
        HeaderList::new().with("content-type", "application/json"),
        serde_json::to_vec(&body).unwrap_or_default(),
    )
}

pub fn check(dialect: DialectId, req: &Request, ctx: &EgressCtx) -> Result<(), KppError> {
    dispatch!(dialect, check(req, ctx))?
}

pub fn encode_request(
    dialect: DialectId,
    req: &Request,
    ctx: &mut EgressCtx,
) -> Result<EncodedRequest, KppError> {
    dispatch!(dialect, encode_request(req, ctx))?
}

pub fn decode_response(
    dialect: DialectId,
    status: u16,
    headers: &HeaderList,
    body: &[u8],
    ctx: &EgressCtx,
) -> Result<Reply, KppError> {
    dispatch!(dialect, decode_response(status, headers, body, ctx))?
}

pub fn stream_decoder(
    dialect: DialectId,
    ctx: &EgressCtx,
) -> Result<Box<dyn StreamDecoder>, KppError> {
    dispatch!(dialect, stream_decoder(ctx))
}

pub fn decode_error(
    dialect: DialectId,
    status: u16,
    headers: &HeaderList,
    body: &[u8],
) -> KppError {
    dispatch!(dialect, decode_error(status, headers, body))
        .unwrap_or_else(|e| KppError::new(ErrorKind::ProviderError, e.message).with_status(status))
}

pub fn models_dialect(headers: &HeaderList) -> DialectId {
    if headers.get("anthropic-version").is_some() {
        DialectId::AnthropicMessages
    } else {
        DialectId::OpenAiChat
    }
}

pub fn decode_models_query(dialect: DialectId, query: &str) -> Result<ModelsQuery, KppError> {
    dispatch!(dialect, decode_models_query(query))?
}

pub fn encode_models(dialect: DialectId, list: &ModelList) -> Result<Vec<u8>, KppError> {
    dispatch!(dialect, encode_models(list))?
}

pub fn encode_model(dialect: DialectId, model: &ModelInfo) -> Result<Vec<u8>, KppError> {
    dispatch!(dialect, encode_model(model))?
}

pub fn decode_models(
    dialect: DialectId,
    status: u16,
    headers: &HeaderList,
    body: &[u8],
) -> Result<ModelList, KppError> {
    dispatch!(dialect, decode_models(status, headers, body))?
}

pub fn decode_model(
    dialect: DialectId,
    status: u16,
    headers: &HeaderList,
    body: &[u8],
) -> Result<ModelInfo, KppError> {
    dispatch!(dialect, decode_model(status, headers, body))?
}

pub fn encode_model_request(dialect: DialectId, model: &str) -> Result<EncodedRequest, KppError> {
    dispatch!(dialect, encode_model_request(model))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_known_dialect_has_a_codec() {
        for d in DialectId::ALL {
            let err = decode_request(d, RequestKind::Generate, b"not json", &HeaderList::new())
                .unwrap_err();
            assert!(
                err.unsupported_feature() != Some(d.as_str()),
                "{d} has no codec"
            );
            for err in [
                decode_models_query(d, "%").unwrap_err(),
                encode_models(d, &ModelList::default())
                    .map_or_else(|e| e, |_| KppError::invalid_request("ok")),
                encode_model(d, &ModelInfo::new("m"))
                    .map_or_else(|e| e, |_| KppError::invalid_request("ok")),
                decode_models(d, 200, &HeaderList::new(), b"x").unwrap_err(),
                decode_model(d, 200, &HeaderList::new(), b"x").unwrap_err(),
            ] {
                assert!(
                    err.unsupported_feature() != Some(d.as_str()),
                    "{d} has no models codec"
                );
            }
        }
    }

    #[test]
    fn models_dialect_follows_the_anthropic_version_header() {
        let anthropic = HeaderList::new().with("Anthropic-Version", "2023-06-01");
        assert_eq!(models_dialect(&anthropic), DialectId::AnthropicMessages);
        let openai = HeaderList::new().with("authorization", "Bearer x");
        assert_eq!(models_dialect(&openai), DialectId::OpenAiChat);
        assert_eq!(models_dialect(&HeaderList::new()), DialectId::OpenAiChat);
        let beta_only = HeaderList::new().with("anthropic-beta", "x");
        assert_eq!(models_dialect(&beta_only), DialectId::OpenAiChat);
    }
}
