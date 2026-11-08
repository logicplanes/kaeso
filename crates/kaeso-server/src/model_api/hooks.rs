use std::net::IpAddr;
use std::time::Duration;

use bytes::Bytes;
use futures::future::BoxFuture;
use kaeso_core::catalogue::Fields;
use kaeso_core::cost::Unpriced;
use kaeso_core::id::RequestId;
use kaeso_core::limits::WorstCase;
use kaeso_kpp::{DialectId, RequestKind, Response, Usage};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::state::AppState;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Caller {
    pub org_id: Uuid,
    pub project_id: Uuid,
    pub key_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub service_account_id: Option<Uuid>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Source {
    #[default]
    Api,
    Playground,
}

impl Source {
    pub const fn as_str(self) -> &'static str {
        match self {
            Source::Api => "api",
            Source::Playground => "playground",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Admission {
    pub request_id: RequestId,
    pub caller: Caller,
    pub catalogue_model_id: Uuid,
    pub kind: RequestKind,
    pub at: OffsetDateTime,
    pub timeout: Duration,
    pub worst: WorstCase,
    pub unpriced: Option<Unpriced>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reservation {
    pub(crate) counters: Vec<String>,
    pub(crate) ttl: i64,
}

impl Reservation {
    pub fn is_held(&self) -> bool {
        !self.counters.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub status: u16,
    pub message: String,
    pub retry_after: Option<Duration>,
    pub param: Option<&'static str>,
}

pub trait Limits: Send + Sync + std::fmt::Debug {
    fn admit<'a>(
        &'a self,
        state: &'a AppState,
        a: &'a Admission,
    ) -> BoxFuture<'a, Result<Reservation, Refusal>>;

    fn release<'a>(
        &'a self,
        _state: &'a AppState,
        _held: &'a Reservation,
        _request_id: &'a str,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async {})
    }
}

#[derive(Debug, Default)]
pub struct NoLimits;

impl Limits for NoLimits {
    fn admit<'a>(
        &'a self,
        _state: &'a AppState,
        _a: &'a Admission,
    ) -> BoxFuture<'a, Result<Reservation, Refusal>> {
        Box::pin(async { Ok(Reservation::default()) })
    }
}

pub const STREAM_INTERRUPTED: &str = "stream_interrupted";

pub const STREAM_LIMIT: &str = "stream_limit";

pub const CLIENT_DISCONNECTED: &str = "client_disconnected";

pub const CLIENT_STALLED: &str = "client_stalled";

pub const SHUT_DOWN: &str = "shutdown";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StreamProgress {
    pub events: u32,
    pub bytes: u64,
    pub first_byte: Option<Duration>,
    pub keepalives: u32,
}

#[derive(Clone, Debug)]
pub struct Outcome {
    pub request_id: RequestId,
    pub caller: Option<Caller>,
    pub source: Source,
    pub ingress: DialectId,
    pub kind: RequestKind,
    pub egress: Option<DialectId>,
    pub provider: Option<&'static str>,
    pub custom_provider: Option<String>,
    pub requested_model: Option<String>,
    pub catalogue_model_id: Option<Uuid>,
    pub model_id: Option<String>,
    pub response_model: Option<String>,
    pub prices: Option<Fields>,
    pub stream: bool,
    pub status: u16,
    pub error_kind: Option<String>,
    pub error_message: Option<String>,
    pub usage: Option<Usage>,
    pub started_at: OffsetDateTime,
    pub duration: Duration,
    pub ttfb: Option<Duration>,
    pub upload: Option<Duration>,
    pub overhead: Option<Duration>,
    pub stream_processing: Option<Duration>,
    pub user_agent: Option<String>,
    pub client_ip: Option<IpAddr>,
    pub upstream_request_id: Option<String>,
    pub request_body: Option<Bytes>,
    pub reply: Option<Response>,
    pub cancelled: bool,
    pub stream_progress: Option<StreamProgress>,
    pub usage_estimated: bool,
    pub content_kept: bool,
    pub cache_breakpoints: Option<usize>,
    pub response_cache: Option<crate::services::response_cache::Verdict>,
    pub alias: Option<String>,
    pub fallbacks: Vec<FellBack>,
    pub gemini: Option<kaeso_codecs::gemini::GeminiAction>,
    pub image: Option<kaeso_kpp::ImageOperation>,
    pub transcription: Option<kaeso_kpp::TranscriptionOperation>,
    pub elevenlabs: Option<kaeso_codecs::elevenlabs::ElevenLabsOperation>,
    pub media: Option<kaeso_core::cost::MediaUnits>,
    /// The media type of a whole reply that isn't JSON, such as speech's audio.
    pub content_type: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FellBack {
    pub catalogue_model_id: Uuid,
    #[serde(default)]
    pub path: String,
    pub model: String,
    pub status: u16,
    pub error_kind: String,
    pub error_message: String,
    pub duration_ms: u64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub skipped: bool,
}

impl Outcome {
    pub fn is_error(&self) -> bool {
        self.status >= 400
    }
}

pub trait Sink: Send + Sync + std::fmt::Debug {
    fn finish<'a>(&'a self, state: &'a AppState, outcome: Outcome) -> BoxFuture<'a, ()>;

    // A reply under a limit ends only once it is charged, so the app's next request sees the
    // charge; a sink that charges first can let the reply end before the rest of its work.
    fn finish_charged<'a>(
        &'a self,
        state: &'a AppState,
        outcome: Outcome,
        charged: tokio::sync::oneshot::Sender<()>,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            self.finish(state, outcome).await;
            let _ = charged.send(());
        })
    }
}

#[derive(Debug, Default)]
pub struct Discard;

impl Sink for Discard {
    fn finish<'a>(&'a self, _state: &'a AppState, _outcome: Outcome) -> BoxFuture<'a, ()> {
        Box::pin(async {})
    }
}
