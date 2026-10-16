#![forbid(unsafe_code)]

mod codec;
pub mod continuity;
pub mod cross;
pub mod dispatch;
pub mod keepalive;
mod models;
mod openai_models;
pub mod sse;
pub mod testing;
#[doc(hidden)]
pub mod wire;

pub mod anthropic_messages;
pub mod openai_chat;
pub mod openai_embeddings;
pub mod openai_responses;

#[cfg(test)]
mod models_tests;
#[cfg(test)]
pub(crate) mod test_support;

pub use codec::{
    Codec, EgressCtx, EncodeCtx, EncodedError, EncodedRequest, FORWARDED_HEADERS, HeaderList,
    StreamDecoder, StreamEncoder, is_forwarded_header, parse_retry_after,
};
