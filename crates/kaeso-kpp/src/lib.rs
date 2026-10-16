#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod block;
mod dialect;
mod error;
mod extras;
mod models;
mod raw;
mod request;
mod response;
mod stream;
mod validate;

pub use block::{
    Base64Source, Block, CacheHint, CacheTtl, Citation, DocumentBlock, FileIdSource, ImageBlock,
    Opaque, OpaqueKind, RawItem, ReasoningBlock, RefusalBlock, Source, TextBlock, TextSource,
    ToolCallBlock, ToolResultBlock,
};
pub use dialect::{DialectId, Origin, UnknownDialect};
pub use error::{ErrorKind, KppError};
pub use extras::{Extras, HEADER_PREFIX};
pub use models::{ModelInfo, ModelLifecycle, ModelList, ModelsQuery};
pub use raw::RawJson;
pub use request::{
    EmbedEncoding, EmbedInput, EmbedRequest, FunctionTool, GenerateRequest, JsonSchemaSpec,
    Message, OutputSpec, ReasoningConfig, ReasoningEffort, ReasoningMode, ReasoningSummary,
    Request, RequestKind, Role, Sampling, StreamMode, ToolChoice, ToolDef,
};
pub use response::{
    CacheWriteBreakdown, CountTokensResponse, EmbedResponse, Embedding, EmbeddingVector, RawUsage,
    Reply, Response, StopReason, Usage,
};
pub use stream::{
    BlockDelta, BlockHeader, BlockStart, BlockStop, Delta, ReasoningHeader, ResponseDelta,
    ResponseStart, ResponseStop, StreamEvent, SummaryDelta, ToolCallHeader,
};
pub use validate::Capabilities;

pub use indexmap;
pub use serde_json;
