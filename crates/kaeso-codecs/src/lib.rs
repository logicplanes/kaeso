#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod adapt;
mod codec;
pub mod continuity;
pub mod cross;
pub mod dispatch;
mod keepalive;
pub mod limits;
mod models;
mod openai_models;
pub mod sse;
mod struct_fields;
pub mod testing;
mod tool_search;
mod web_search;
mod wire;

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
pub use keepalive::HIDDEN_TEXT_PER_KEEPALIVE;
