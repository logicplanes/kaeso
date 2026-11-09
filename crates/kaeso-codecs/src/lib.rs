#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod adapt;
mod codec;
pub mod continuity;
pub mod cross;
pub mod dispatch;
mod inline_tools;
mod keepalive;
pub mod limits;
mod media;
mod models;
pub mod multipart;
mod openai_models;
pub mod sse;
mod struct_fields;
pub mod testing;
mod tool_patterns;
mod tool_roots;
mod tool_search;
mod verbosity;
mod video_jobs;
mod web_search;
mod wire;

pub mod anthropic_messages;
pub mod dashscope;
pub mod elevenlabs;
pub mod gemini;
pub mod gemini_embeddings;
pub mod gemini_interactions;
pub mod minimax_images;
pub mod minimax_speech;
pub mod minimax_videos;
pub mod openai_chat;
pub mod openai_embeddings;
pub mod openai_images;
pub mod openai_responses;
pub mod openai_speech;
pub mod openai_transcriptions;
pub mod openai_videos;
pub mod openrouter_videos;
pub mod veo;
pub mod xai_videos;
pub mod zai_videos;

#[cfg(test)]
mod models_tests;
#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests_audio_dialects;

pub use codec::{
    Codec, EVENT_STREAM_MEDIA_TYPE, EgressCtx, EncodeCtx, EncodedError, EncodedRequest,
    FORWARDED_HEADERS, HeaderList, JSON_MEDIA_TYPE, StreamDecoder, StreamEncoder,
    is_forwarded_header, parse_retry_after,
};
pub use keepalive::HIDDEN_TEXT_PER_KEEPALIVE;
pub use tool_patterns::ToolPatterns;
pub use tool_roots::ToolSchemaRoot;
