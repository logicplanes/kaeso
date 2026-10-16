use std::sync::Arc;

use indexmap::IndexMap;
use kaeso_kpp::{
    DialectId, KppError, ModelInfo, ModelList, ModelsQuery, Origin, Reply, Request, RequestKind,
    StreamEvent, StreamMode,
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

    fn encode_error(err: &KppError) -> EncodedError;

    fn check(req: &Request) -> Result<(), KppError>;

    fn encode_request(req: &Request, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError>;

    fn decode_response(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
        ctx: &EgressCtx,
    ) -> Result<Reply, KppError>;

    fn stream_decoder(ctx: &EgressCtx) -> Box<dyn StreamDecoder>;

    fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError;

    fn decode_models_query(query: &str) -> Result<ModelsQuery, KppError>;

    fn encode_models(list: &ModelList) -> Result<Vec<u8>, KppError>;

    fn encode_model(model: &ModelInfo) -> Result<Vec<u8>, KppError>;

    fn decode_models(status: u16, headers: &HeaderList, body: &[u8])
    -> Result<ModelList, KppError>;

    fn decode_model(status: u16, headers: &HeaderList, body: &[u8]) -> Result<ModelInfo, KppError>;
}

pub(crate) mod sealed {
    pub trait Sealed {}
}

pub trait StreamDecoder: Send {
    fn feed(&mut self, bytes: &[u8]) -> Vec<StreamEvent>;
    fn finish(&mut self) -> Vec<StreamEvent>;
}

pub trait StreamEncoder: Send {
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

    pub fn for_request(req: &Request) -> Self {
        EncodeCtx {
            kind: req.kind(),
            stream: req.stream_mode(),
            request: Some(Arc::new(req.clone())),
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
    pub tool_ids: IndexMap<String, String>,
    pub tool_names: IndexMap<String, String>,
    pub adaptive_thinking: Option<bool>,
    pub reasoning: Option<bool>,
}

impl EgressCtx {
    pub fn new(req: &Request) -> Self {
        EgressCtx::for_kind(req.kind(), req.stream_mode())
    }

    pub fn for_kind(kind: RequestKind, stream: StreamMode) -> Self {
        EgressCtx {
            kind,
            stream,
            provider: None,
            tool_ids: IndexMap::new(),
            tool_names: IndexMap::new(),
            adaptive_thinking: None,
            reasoning: None,
        }
    }

    #[must_use]
    pub fn with_adaptive_thinking(mut self, supported: bool) -> Self {
        self.adaptive_thinking = Some(supported);
        self
    }

    #[must_use]
    pub fn with_reasoning(mut self, reasons: bool) -> Self {
        self.reasoning = Some(reasons);
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

#[cfg(test)]
mod tests {
    use super::*;
    use kaeso_kpp::{GenerateRequest, Message};

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
                .with_stream(StreamMode::On {
                    include_usage: true,
                }),
        );
        let enc = EncodeCtx::for_request(&req);
        assert_eq!(enc.kind, RequestKind::Generate);
        assert!(enc.request.is_some());
        assert_eq!(
            enc.stream,
            StreamMode::On {
                include_usage: true
            }
        );
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
