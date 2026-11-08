#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod block;
mod dialect;
mod error;
mod extras;
mod image;
mod models;
mod raw;
mod request;
mod response;
mod stream;
mod validate;

pub use block::{
    AudioBlock, Base64Source, Block, CacheHint, CacheTtl, Citation, DocumentBlock, FileIdSource,
    ImageBlock, Opaque, OpaqueKind, RawItem, ReasoningBlock, RefusalBlock, Source, TextBlock,
    TextSource, ToolCallBlock, ToolResultBlock, VideoBlock,
};
pub use dialect::{DialectId, Origin, UnknownDialect};
pub use error::{ErrorKind, KppError};
pub use extras::{Extras, HEADER_PREFIX};
pub use image::{
    GeneratedImage, ImageBackground, ImageDelivery, ImageEvent, ImageFormat, ImageInput,
    ImageOperation, ImageQuality, ImageRequest, ImageResponse, ImageSettings, ImageSize,
};
pub use models::{ModelInfo, ModelLifecycle, ModelList, ModelsQuery};
pub use raw::RawJson;
pub use request::{
    AudioFormat, AudioOutput, EmbedEncoding, EmbedInput, EmbedRequest, FunctionTool,
    GenerateRequest, JsonSchemaSpec, Message, Modality, OutputSpec, PCM16_MEDIA_TYPE,
    ReasoningConfig, ReasoningEffort, ReasoningMode, ReasoningSummary, Request, RequestKind, Role,
    Sampling, StreamMode, ToolChoice, ToolDef,
};
pub use response::{
    CacheWriteBreakdown, CountTokensResponse, EmbedResponse, Embedding, EmbeddingVector, RawUsage,
    Reply, Response, StopReason, Usage,
};
pub use stream::{
    AudioDelta, AudioHeader, BlockDelta, BlockHeader, BlockStart, BlockStop, Delta,
    ReasoningHeader, ResponseDelta, ResponseStart, ResponseStop, StreamEvent, SummaryDelta,
    ToolCallHeader,
};
pub use validate::Capabilities;

pub use indexmap;
pub use serde_json;
