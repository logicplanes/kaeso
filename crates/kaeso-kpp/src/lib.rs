#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

macro_rules! wire_enum {
    ($(#[$doc:meta])* $name:ident { $($variant:ident = $wire:literal),* $(,)? }) => {
        $(#[$doc])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        #[non_exhaustive]
        pub enum $name {
            $($variant,)*
            /// A value KPP doesn't name, as written.
            Other(String),
        }

        impl $name {
            pub fn from_wire(s: &str) -> Self {
                match s {
                    $($wire => $name::$variant,)*
                    other => $name::Other(other.to_owned()),
                }
            }

            pub fn as_wire(&self) -> &str {
                match self {
                    $($name::$variant => $wire,)*
                    $name::Other(s) => s,
                }
            }
        }

        impl Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(self.as_wire())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let s = String::deserialize(d)?;
                Ok($name::from_wire(&s))
            }
        }
    };
}

mod audio;
mod b64;
mod block;
mod decision;
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
mod video;

pub use audio::{
    AudioInput, DialogueLine, SpeechAlignment, SpeechEvent, SpeechOperation, SpeechRequest,
    SpeechResponse, TimestampGranularity, TranscriptEvent, TranscriptFormat, TranscriptSegment,
    TranscriptWord, TranscriptionOperation, TranscriptionRequest, TranscriptionResponse, Voice,
};
pub use b64::Base64Text;
pub use block::{
    AudioBlock, Base64Source, Block, CacheHint, CacheTtl, Citation, DocumentBlock, FileIdSource,
    ImageBlock, JsonBlock, Opaque, OpaqueKind, RawItem, ReasoningBlock, RefusalBlock, Source,
    TextBlock, TextSource, ToolCallBlock, ToolResultBlock, VideoBlock,
};
pub use decision::{
    Answer, AnswerValue, Asset, BoxAnswer, BoxQuestion, Choice, ChoiceAnswer, ChoiceOption,
    ChoiceValue, DecideRequest, DecideResponse, DecisionCache, DecisionInput, Description,
    Instructions, Level, Predicate, PredicateAnswer, Question, QuestionKind, Refusal, Score,
    ScoreAnswer,
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
pub use video::{
    ListOrder, VideoError, VideoImage, VideoImageRole, VideoJob, VideoList, VideoListQuery,
    VideoOperation, VideoOutput, VideoRequest, VideoStatus, VideoVariant,
};

pub use bytes;
pub use indexmap;
pub use serde_json;
