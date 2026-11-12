use std::sync::Arc;

use crate::anthropic_messages::MessagesProfile;
use crate::gemini::GeminiProfile;
use crate::openai_chat::ChatProfile;
use crate::openai_decisions::DecisionsProfile;
use crate::openai_images::ImagesProfile;
use crate::openai_responses::ResponsesProfile;
use crate::openai_speech::SpeechProfile;
use crate::openai_transcriptions::TranscriptionsProfile;
use crate::openai_videos::VideosProfile;
use crate::system_one::SystemOneProfile;
use indexmap::IndexMap;
use kaeso_kpp::{
    AudioFormat, DialectId, KppError, ModelInfo, ModelList, ModelsQuery, Origin, Reply, Request,
    RequestKind, Source, StreamEvent, StreamMode, TranscriptFormat, VideoJob,
};

pub trait Codec: sealed::Sealed {
    const DIALECT: DialectId;

    fn decode_request(
        kind: RequestKind,
        body: &[u8],
        headers: &HeaderList,
    ) -> Result<Request, KppError>;

    fn encode_response(reply: &Reply, ctx: &EncodeCtx) -> Result<Vec<u8>, KppError>;

    fn stream_encoder(ctx: &EncodeCtx) -> Box<dyn StreamEncoder>;

    /// The `content-type` of a reply [`encode_response`](Self::encode_response) wrote, such as
    /// an audio type for speech or `text/plain` for a subtitle transcript.
    fn response_media_type(_reply: &Reply, ctx: &EncodeCtx) -> String {
        if ctx.stream.is_off() {
            JSON_MEDIA_TYPE.to_owned()
        } else {
            EVENT_STREAM_MEDIA_TYPE.to_owned()
        }
    }

    /// The `content-type` of a stream the [`stream_encoder`](Self::stream_encoder) writes:
    /// server-sent events, or the audio itself for speech asked for without events.
    fn stream_media_type(_ctx: &EncodeCtx) -> String {
        EVENT_STREAM_MEDIA_TYPE.to_owned()
    }

    fn encode_error(err: &KppError) -> EncodedError;

    fn encode_error_for_request(err: &KppError, _request_id: &str) -> EncodedError {
        Self::encode_error(err)
    }

    fn check(req: &Request, ctx: &EgressCtx) -> Result<(), KppError>;

    fn encode_request(req: &Request, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError>;

    fn decode_response(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
        ctx: &EgressCtx,
    ) -> Result<Reply, KppError>;

    fn stream_decoder(ctx: &EgressCtx) -> Box<dyn StreamDecoder>;

    /// Whether the provider's reply is read as it arrives, with the
    /// [`stream_decoder`](Self::stream_decoder): for a stream, and for audio a provider sends
    /// as it makes it.
    fn reads_reply_as_stream(ctx: &EgressCtx) -> bool {
        !ctx.stream.is_off()
    }

    fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError;

    fn decode_provider_error(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
        _ctx: &EgressCtx,
    ) -> KppError {
        Self::decode_error(status, headers, body)
    }

    /// The request that asks the provider how its video job `job`, made by `model`, stands.
    fn encode_video_poll(
        _job: &str,
        _model: &str,
        _ctx: &EgressCtx,
    ) -> Result<EncodedRequest, KppError> {
        Err(no_video_jobs(Self::DIALECT))
    }

    /// Reads the provider's answer to [`encode_video_poll`](Self::encode_video_poll).
    fn decode_video_poll(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
        _ctx: &EgressCtx,
    ) -> Result<VideoJob, KppError> {
        if !(200..300).contains(&status) {
            return Err(Self::decode_error(status, headers, body));
        }
        Err(no_video_jobs(Self::DIALECT))
    }

    /// The request that deletes the provider's video job `job`, or `None` when the provider
    /// documents no way to delete one.
    fn encode_video_delete(
        _job: &str,
        _model: &str,
        _ctx: &EgressCtx,
    ) -> Result<Option<EncodedRequest>, KppError> {
        Ok(None)
    }

    /// The request that turns the file id a finished video job named into a link to the file.
    fn encode_video_file(_file: &str, _ctx: &EgressCtx) -> Result<EncodedRequest, KppError> {
        Err(no_video_jobs(Self::DIALECT))
    }

    /// Reads the provider's answer to [`encode_video_file`](Self::encode_video_file): the
    /// file's link.
    fn decode_video_file(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
    ) -> Result<Source, KppError> {
        if !(200..300).contains(&status) {
            return Err(Self::decode_error(status, headers, body));
        }
        Err(no_video_jobs(Self::DIALECT))
    }

    fn decode_models_query(query: &str) -> Result<ModelsQuery, KppError>;

    fn encode_models(list: &ModelList) -> Result<Vec<u8>, KppError>;

    fn encode_model(model: &ModelInfo) -> Result<Vec<u8>, KppError>;

    fn decode_models(status: u16, headers: &HeaderList, body: &[u8])
    -> Result<ModelList, KppError>;

    fn decode_model(status: u16, headers: &HeaderList, body: &[u8]) -> Result<ModelInfo, KppError>;

    fn encode_model_request(model: &str) -> EncodedRequest;

    fn encode_models_request(query: &ModelsQuery) -> Result<EncodedRequest, KppError>;

    /// The request for one model at the provider `ctx` describes, such as a Gemini provider
    /// with its own models path.
    fn encode_model_request_with(
        model: &str,
        _ctx: &EgressCtx,
    ) -> Result<EncodedRequest, KppError> {
        Ok(Self::encode_model_request(model))
    }

    /// The listing request for the provider `ctx` describes.
    fn encode_models_request_with(
        query: &ModelsQuery,
        _ctx: &EgressCtx,
    ) -> Result<EncodedRequest, KppError> {
        Self::encode_models_request(query)
    }
}

fn no_video_jobs(dialect: DialectId) -> KppError {
    KppError::unsupported(
        "video jobs",
        format!("{} runs no video jobs.", dialect.display_name()),
    )
}

/// The `content-type` of a JSON reply.
pub const JSON_MEDIA_TYPE: &str = "application/json";
/// The `content-type` of a stream of server-sent events.
pub const EVENT_STREAM_MEDIA_TYPE: &str = "text/event-stream; charset=utf-8";

pub(crate) mod sealed {
    pub trait Sealed {}
}

pub trait StreamDecoder: Send + sealed::Sealed {
    fn feed(&mut self, bytes: &[u8]) -> Vec<StreamEvent>;
    fn finish(&mut self) -> Vec<StreamEvent>;
    /// Releases the events the decoder still holds back, such as a Chat Completions tool call
    /// waiting for its `id` and `name`, for a stream that ends without [`finish`](Self::finish):
    /// the provider's connection broke, the app left, or the gateway ended the stream. Unlike
    /// `finish`, it adds no error for the unfinished reply. The decoder takes no more bytes after.
    fn flush(&mut self) -> Vec<StreamEvent> {
        Vec::new()
    }
}

pub trait StreamEncoder: Send + sealed::Sealed {
    fn encode(&mut self, event: &StreamEvent) -> Result<Vec<u8>, KppError>;
    fn finish(&mut self) -> Result<Vec<u8>, KppError>;
    fn keepalive(&mut self) -> Vec<u8> {
        Vec::new()
    }
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct EncodeCtx {
    pub kind: RequestKind,
    pub stream: StreamMode,
    pub request: Option<Arc<Request>>,
    pub created_at: Option<u64>,
}

impl EncodeCtx {
    pub fn new(kind: RequestKind, stream: StreamMode) -> Self {
        EncodeCtx {
            kind,
            stream,
            request: None,
            created_at: None,
        }
    }

    pub fn for_request(req: impl Into<Arc<Request>>) -> Self {
        let req = req.into();
        EncodeCtx {
            kind: req.kind(),
            stream: req.stream_mode(),
            request: Some(req),
            created_at: None,
        }
    }

    #[must_use]
    pub fn with_created_at(mut self, unix_seconds: u64) -> Self {
        self.created_at = Some(unix_seconds);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct EgressCtx {
    pub kind: RequestKind,
    pub stream: StreamMode,
    pub provider: Option<String>,
    pub(crate) tool_ids: IndexMap<String, String>,
    pub(crate) tool_names: IndexMap<String, String>,
    pub adaptive_thinking: Option<bool>,
    pub thinking_binding: Option<bool>,
    pub reasoning: Option<bool>,
    pub tool_namespaces: Option<bool>,
    pub chat_profile: ChatProfile,
    pub responses_profile: ResponsesProfile,
    pub messages_profile: MessagesProfile,
    /// Where a Gemini-format provider serves its models; Google AI Studio by default.
    pub gemini_profile: GeminiProfile,
    /// How a provider that speaks a form of OpenAI's Images API takes image requests.
    pub images_profile: ImagesProfile,
    /// How a provider that speaks a form of OpenAI's speech API takes speech requests.
    pub speech_profile: SpeechProfile,
    /// How a provider that speaks a form of OpenAI's transcription API takes transcriptions.
    pub transcriptions_profile: TranscriptionsProfile,
    /// How a provider that speaks a form of OpenAI's Videos API takes video jobs.
    pub videos_profile: VideosProfile,
    /// How a provider that speaks a form of TypeSafe's System One API takes decision requests.
    pub system_one_profile: SystemOneProfile,
    /// How a provider that speaks OpenAI's Decisions API takes decision requests.
    pub decisions_profile: DecisionsProfile,
    /// The questions a decision request was sent with, in question order, under the keys the
    /// encoder chose, so that keyed answers are read back by position. The encoder sets it.
    pub(crate) decision_questions: Vec<crate::decisions::SentQuestion>,
    pub left_out: Vec<String>,
    pub adapted: Vec<String>,
    /// The prompt-cache breakpoints (`cache_control`) an encoded generation request
    /// carries, set by encoders whose providers cache only what a request marks
    /// (Anthropic Messages); `None` for the other dialects.
    pub cache_breakpoints: Option<usize>,
    /// Where a provider that names the project and location in its paths serves the
    /// dialect, instead of its usual `/v1/…` path: the publisher's models for Anthropic
    /// Messages with [`MessagesProfile::raw_predict`], such as
    /// `/v1/projects/{project}/locations/{location}/publishers/anthropic/models`, or the
    /// directory of `chat/completions` for OpenAI Chat Completions, such as
    /// `/v1/projects/{project}/locations/{location}/endpoints/openapi`, or the directory of
    /// `interactions` for Gemini Interactions, `/v1beta1/projects/{project}/locations/global`.
    pub base_path: Option<String>,
    /// What takes the place of `/v1` for a provider whose API path names the account, such as
    /// Cloudflare's `/client/v4/accounts/{account_id}/ai/v1`: Chat Completions, Responses and
    /// Messages send their usual path under it. A [`EgressCtx::base_path`] goes first.
    pub api_root: Option<String>,
    /// The audio format the request asked for, which tells a reply decoder the media type of
    /// audio the provider doesn't label, as Chat Completions' `message.audio`. The encoder sets
    /// the format it chose when the app named none.
    pub audio_format: Option<AudioFormat>,
    /// The transcript form the provider was asked for, which tells the reply decoder how to
    /// read a plain-text body. The encoder sets it.
    pub transcript_format: Option<TranscriptFormat>,
    /// Whether the speech reply is JSON with the audio inside, read whole, as ElevenLabs'
    /// `with-timestamps` replies are. The encoder sets it.
    pub speech_reply_json: bool,
}

impl EgressCtx {
    pub fn new(req: &Request) -> Self {
        let mut ctx = EgressCtx::for_kind(req.kind(), req.stream_mode());
        match req {
            Request::Generate(g) => {
                ctx.audio_format = g.audio_output.as_ref().and_then(|a| a.format.clone());
            }
            Request::Speech(s) => ctx.audio_format.clone_from(&s.format),
            _ => {}
        }
        ctx
    }

    pub fn for_kind(kind: RequestKind, stream: StreamMode) -> Self {
        EgressCtx {
            kind,
            stream,
            provider: None,
            tool_ids: IndexMap::new(),
            tool_names: IndexMap::new(),
            adaptive_thinking: None,
            thinking_binding: None,
            reasoning: None,
            tool_namespaces: None,
            chat_profile: ChatProfile::OPENAI,
            responses_profile: ResponsesProfile::OPENAI,
            messages_profile: MessagesProfile::ANTHROPIC,
            gemini_profile: GeminiProfile::GOOGLE_AI_STUDIO,
            images_profile: ImagesProfile::OPENAI,
            speech_profile: SpeechProfile::OPENAI,
            transcriptions_profile: TranscriptionsProfile::OPENAI,
            videos_profile: VideosProfile::OPENAI,
            system_one_profile: SystemOneProfile::TYPESAFE,
            decisions_profile: DecisionsProfile::OPENAI,
            decision_questions: Vec::new(),
            left_out: Vec::new(),
            adapted: Vec::new(),
            cache_breakpoints: None,
            base_path: None,
            api_root: None,
            audio_format: None,
            transcript_format: None,
            speech_reply_json: false,
        }
    }

    #[must_use]
    pub fn with_adaptive_thinking(mut self, supported: bool) -> Self {
        self.adaptive_thinking = Some(supported);
        self
    }

    #[must_use]
    pub fn with_thinking_binding(mut self, checks: bool) -> Self {
        self.thinking_binding = Some(checks);
        self
    }

    #[must_use]
    pub fn with_reasoning(mut self, reasons: bool) -> Self {
        self.reasoning = Some(reasons);
        self
    }

    #[must_use]
    pub fn with_tool_namespaces(mut self, supported: bool) -> Self {
        self.tool_namespaces = Some(supported);
        self
    }

    pub fn takes_tool_namespaces(&self, dialect: DialectId) -> bool {
        dialect == DialectId::OpenAiResponses
            && self
                .tool_namespaces
                .unwrap_or(self.responses_profile.tool_namespaces)
    }

    #[must_use]
    pub fn with_chat_profile(mut self, profile: ChatProfile) -> Self {
        self.chat_profile = profile;
        self
    }

    #[must_use]
    pub fn with_responses_profile(mut self, profile: ResponsesProfile) -> Self {
        self.responses_profile = profile;
        self
    }

    /// Sends Gemini requests to the models path and with the wire switches of `profile`.
    #[must_use]
    pub fn with_gemini_profile(mut self, profile: GeminiProfile) -> Self {
        self.gemini_profile = profile;
        self
    }

    /// Sends image requests in the form `profile` describes.
    #[must_use]
    pub fn with_images_profile(mut self, profile: ImagesProfile) -> Self {
        self.images_profile = profile;
        self
    }

    /// Sends speech requests in the form `profile` describes.
    #[must_use]
    pub fn with_speech_profile(mut self, profile: SpeechProfile) -> Self {
        self.speech_profile = profile;
        self
    }

    /// Sends transcription requests in the form `profile` describes.
    #[must_use]
    pub fn with_transcriptions_profile(mut self, profile: TranscriptionsProfile) -> Self {
        self.transcriptions_profile = profile;
        self
    }

    /// Sends video jobs in the form `profile` describes.
    #[must_use]
    pub fn with_videos_profile(mut self, profile: VideosProfile) -> Self {
        self.videos_profile = profile;
        self
    }

    /// Sends decision requests in the System One form `profile` describes.
    #[must_use]
    pub fn with_system_one_profile(mut self, profile: SystemOneProfile) -> Self {
        self.system_one_profile = profile;
        self
    }

    /// Sends decision requests in the OpenAI Decisions form `profile` describes.
    #[must_use]
    pub fn with_decisions_profile(mut self, profile: DecisionsProfile) -> Self {
        self.decisions_profile = profile;
        self
    }

    /// The keys a decision request's questions were sent under, in question order: their names,
    /// or `q0`…`qN-1` where a provider couldn't take them. Empty until a decision request is
    /// encoded.
    pub fn decision_keys(&self) -> impl Iterator<Item = &str> {
        self.decision_questions.iter().map(|q| q.key.as_str())
    }

    #[must_use]
    pub fn with_messages_profile(mut self, profile: MessagesProfile) -> Self {
        self.messages_profile = profile;
        self
    }

    /// Sends Chat Completions, Responses and Messages requests under `root` in place of `/v1`
    /// ([`EgressCtx::api_root`]); a trailing slash is ignored.
    #[must_use]
    pub fn with_api_root(mut self, root: impl Into<String>) -> Self {
        let root: String = root.into();
        self.api_root = Some(root.trim_end_matches('/').to_owned());
        self
    }

    /// Sends the dialect's requests under `path` instead of its usual `/v1/…` path
    /// ([`EgressCtx::base_path`]); a trailing slash is ignored.
    #[must_use]
    pub fn with_base_path(mut self, path: impl Into<String>) -> Self {
        let path: String = path.into();
        self.base_path = Some(path.trim_end_matches('/').to_owned());
        self
    }

    #[must_use]
    pub fn with_provider(mut self, provider: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self
    }

    pub fn origin(&self, dialect: DialectId) -> Origin {
        match &self.provider {
            Some(p) => Origin::new(dialect).with_provider(p.clone()),
            None => Origin::new(dialect),
        }
    }

    pub fn original_tool_id<'a>(&'a self, sent: &'a str) -> &'a str {
        self.tool_ids.get(sent).map_or(sent, String::as_str)
    }

    pub fn original_tool_name<'a>(&'a self, sent: &'a str) -> &'a str {
        self.tool_names.get(sent).map_or(sent, String::as_str)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct EncodedRequest {
    pub method: &'static str,
    pub path: String,
    pub headers: HeaderList,
    pub body: Vec<u8>,
}

impl EncodedRequest {
    pub fn post_json(path: impl Into<String>, headers: HeaderList, body: Vec<u8>) -> Self {
        EncodedRequest {
            method: "POST",
            path: path.into(),
            headers,
            body,
        }
    }

    pub fn delete(path: impl Into<String>, headers: HeaderList) -> Self {
        EncodedRequest {
            method: "DELETE",
            path: path.into(),
            headers,
            body: Vec::new(),
        }
    }

    pub fn get(path: impl Into<String>, headers: HeaderList) -> Self {
        EncodedRequest {
            method: "GET",
            path: path.into(),
            headers,
            body: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct EncodedError {
    pub status: u16,
    pub headers: HeaderList,
    pub body: Vec<u8>,
}

impl EncodedError {
    pub fn new(status: u16, headers: HeaderList, body: Vec<u8>) -> Self {
        EncodedError {
            status,
            headers,
            body,
        }
    }
}

/// A dialect's `/v1/…` path under the context's API root, which takes the place of `/v1`
/// ([`EgressCtx::api_root`]); the path itself without one.
pub(crate) fn under_api_root(ctx: &EgressCtx, path: &str) -> String {
    match (&ctx.api_root, path.strip_prefix("/v1")) {
        (Some(base), Some(rest)) => format!("{base}{rest}"),
        _ => path.to_owned(),
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HeaderList(Vec<(String, String)>);

impl HeaderList {
    pub fn new() -> Self {
        HeaderList(Vec::new())
    }

    pub fn push(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.0.push((name.into(), value.into()));
    }

    #[must_use]
    pub fn with(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.push(name, value);
        self
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    pub fn get_all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a str> {
        self.0
            .iter()
            .filter(move |(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().map(|(n, v)| (n.as_str(), v.as_str()))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<N: Into<String>, V: Into<String>> FromIterator<(N, V)> for HeaderList {
    fn from_iter<T: IntoIterator<Item = (N, V)>>(iter: T) -> Self {
        HeaderList(
            iter.into_iter()
                .map(|(n, v)| (n.into(), v.into()))
                .collect(),
        )
    }
}

pub const FORWARDED_HEADERS: &[(DialectId, &str)] = &[
    (DialectId::AnthropicMessages, "anthropic-beta"),
    (DialectId::AnthropicMessages, "anthropic-version"),
    (DialectId::OpenAiChat, "openai-beta"),
    (DialectId::OpenAiResponses, "openai-beta"),
    (DialectId::OpenAiEmbeddings, "openai-beta"),
    (DialectId::OpenAiImages, "openai-beta"),
    (DialectId::OpenAiSpeech, "openai-beta"),
    (DialectId::OpenAiTranscriptions, "openai-beta"),
    (DialectId::OpenAiVideos, "openai-beta"),
    (DialectId::OpenAiDecisions, "openai-beta"),
    (DialectId::SystemOne, "typesafe-danger-zone-images"),
    (DialectId::SystemOne, "typesafe-danger-zone-long-context"),
];

pub fn is_forwarded_header(dialect: DialectId, name: &str) -> bool {
    FORWARDED_HEADERS
        .iter()
        .any(|(d, n)| *d == dialect && n.eq_ignore_ascii_case(name))
}

pub fn parse_retry_after(headers: &HeaderList) -> Option<std::time::Duration> {
    let parse = |name: &str, scale: f64| {
        let value: f64 = headers.get(name)?.trim().parse().ok()?;
        std::time::Duration::try_from_secs_f64(value / scale).ok()
    };
    parse("retry-after-ms", 1000.0).or_else(|| parse("retry-after", 1.0))
}

pub(crate) fn push_retry_headers(
    headers: &mut HeaderList,
    retry: std::time::Duration,
    with_millis: bool,
) {
    let whole = retry.subsec_nanos() == 0;
    let secs = retry.as_secs().saturating_add(u64::from(!whole));
    headers.push("retry-after", secs.to_string());
    if with_millis && !whole {
        let millis = retry.as_nanos().div_ceil(1_000_000);
        headers.push("retry-after-ms", millis.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaeso_kpp::{GenerateRequest, Message};

    #[test]
    fn retry_headers_round_up_and_add_milliseconds_only_when_needed() {
        let write = |retry: std::time::Duration, with_millis: bool| {
            let mut headers = HeaderList::new();
            push_retry_headers(&mut headers, retry, with_millis);
            headers
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
        };
        let fractional = std::time::Duration::from_micros(1_000_500);
        assert_eq!(
            write(fractional, true),
            ["retry-after=2", "retry-after-ms=1001"]
        );
        assert_eq!(write(fractional, false), ["retry-after=2"]);
        assert_eq!(
            write(std::time::Duration::from_micros(500), true),
            ["retry-after=1", "retry-after-ms=1"]
        );
        assert_eq!(
            write(std::time::Duration::from_secs(3), true),
            ["retry-after=3"]
        );
        let back = HeaderList::new()
            .with("retry-after", "2")
            .with("retry-after-ms", "1001");
        assert_eq!(
            parse_retry_after(&back),
            Some(std::time::Duration::from_millis(1001))
        );
    }

    #[test]
    fn headers_are_case_insensitive_and_ordered() {
        let h = HeaderList::new()
            .with("Anthropic-Beta", "a")
            .with("anthropic-beta", "b")
            .with("Retry-After", "1.5");
        assert_eq!(h.get("ANTHROPIC-BETA"), Some("a"));
        assert_eq!(h.get_all("anthropic-beta").collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!(
            parse_retry_after(&h),
            Some(std::time::Duration::from_millis(1500))
        );
        assert_eq!(
            parse_retry_after(&HeaderList::new().with("retry-after", "soon")),
            None
        );
        assert_eq!(
            parse_retry_after(&HeaderList::new().with("retry-after", "-1")),
            None
        );
    }

    #[test]
    fn only_allowlisted_headers_are_forwarded() {
        assert!(is_forwarded_header(
            DialectId::AnthropicMessages,
            "Anthropic-Beta"
        ));
        assert!(!is_forwarded_header(
            DialectId::OpenAiChat,
            "anthropic-beta"
        ));
        assert!(!is_forwarded_header(DialectId::OpenAiChat, "authorization"));
    }

    #[test]
    fn contexts_follow_the_request() {
        let req = Request::Generate(
            GenerateRequest::new("m")
                .with_message(Message::user("x"))
                .with_stream(StreamMode::on(true)),
        );
        let shared = Arc::new(req.clone());
        let enc = EncodeCtx::for_request(Arc::clone(&shared));
        assert_eq!(enc.kind, RequestKind::Generate);
        assert!(
            enc.request
                .as_ref()
                .is_some_and(|r| Arc::ptr_eq(r, &shared))
        );
        assert_eq!(enc.stream, StreamMode::on(true));
        assert_eq!(EncodeCtx::for_request(req.clone()), enc);
        let mut eg = EgressCtx::new(&req).with_provider("anthropic");
        eg.tool_ids.insert("sent".into(), "orig".into());
        assert_eq!(eg.original_tool_id("sent"), "orig");
        assert_eq!(eg.original_tool_id("other"), "other");
        assert_eq!(
            eg.origin(DialectId::AnthropicMessages).provider.as_deref(),
            Some("anthropic")
        );
    }
}
