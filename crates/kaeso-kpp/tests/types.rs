use kaeso_kpp::{
    Block, CacheHint, DialectId, DocumentBlock, EmbedInput, EmbedRequest, ErrorKind, FunctionTool,
    GenerateRequest, ImageBlock, KppError, Message, Origin, RawItem, RawJson, ReasoningBlock,
    RefusalBlock, Request, Role, Source, ToolDef,
};

#[test]
fn every_error_kind_has_a_stable_name() {
    let kinds = [
        (ErrorKind::InvalidRequest, "invalid_request"),
        (ErrorKind::Authentication, "authentication"),
        (ErrorKind::PermissionDenied, "permission_denied"),
        (ErrorKind::NotFound, "not_found"),
        (ErrorKind::RequestTooLarge, "request_too_large"),
        (ErrorKind::RateLimited, "rate_limited"),
        (ErrorKind::Overloaded, "overloaded"),
        (ErrorKind::ContextTooLong, "context_too_long"),
        (ErrorKind::ContentFiltered, "content_filtered"),
        (ErrorKind::unsupported("n"), "unsupported"),
        (ErrorKind::Timeout, "timeout"),
        (ErrorKind::Billing, "billing"),
        (ErrorKind::ProviderError, "provider_error"),
    ];
    for (kind, name) in kinds {
        assert_eq!(kind.as_str(), name);
        assert_eq!(kind.to_string(), name);
    }
}

#[test]
fn every_block_names_its_kind() {
    let origin = Origin::new(DialectId::AnthropicMessages);
    let blocks = [
        (Block::text("Hi"), "text"),
        (
            Block::Image(ImageBlock::new(Source::base64("image/png", "AA=="))),
            "image",
        ),
        (
            Block::Document(DocumentBlock::new(Source::text("text/plain", "notes"))),
            "document",
        ),
        (Block::tool_result("call_1", vec![], false), "tool_result"),
        (
            Block::Reasoning(ReasoningBlock::new(origin.clone())),
            "reasoning",
        ),
        (Block::Refusal(RefusalBlock::new("No.")), "refusal"),
        (
            Block::Unknown(RawItem::new(
                origin,
                RawJson::from_string("{}".to_owned()).unwrap(),
            )),
            "unknown",
        ),
    ];
    for (block, kind) in blocks {
        assert_eq!(block.kind(), kind);
    }
}

#[test]
fn only_inline_sources_have_a_media_type() {
    let origin = Origin::new(DialectId::OpenAiResponses);
    assert_eq!(
        Source::base64("image/png", "AA==").media_type(),
        Some("image/png")
    );
    assert_eq!(
        Source::text("text/plain", "x").media_type(),
        Some("text/plain")
    );
    assert_eq!(
        Source::Url("https://example.com".to_owned()).media_type(),
        None
    );
    assert_eq!(Source::file_id(origin, "file_1").media_type(), None);
}

#[test]
fn raw_json_shows_its_text() {
    let value = RawJson::from_string("{\"a\": 1}".to_owned()).unwrap();
    assert_eq!(value.to_string(), "{\"a\": 1}");
    assert_eq!(format!("{value:?}"), "RawJson({\"a\": 1})");
    let converted = RawJson::try_from(&serde_json::json!({"b": [true]})).unwrap();
    assert_eq!(converted.get(), "{\"b\":[true]}");
}

fn generate(tools: Vec<ToolDef>, blocks: Vec<Block>) -> Request {
    let mut req =
        GenerateRequest::new("gpt-5.6-sol").with_message(Message::new(Role::User, blocks));
    req.tools = tools;
    Request::Generate(req)
}

#[test]
fn tools_need_a_name() {
    let req = generate(
        vec![ToolDef::Function(FunctionTool::new("", None))],
        vec![Block::text("Hi")],
    );
    let err: KppError = req.validate().unwrap_err();
    assert_eq!(err.kind, ErrorKind::InvalidRequest);
    assert_eq!(err.param.as_deref(), Some("tools[0]"));
}

#[test]
fn capabilities_follow_documents_and_cached_tools() {
    let mut tool = FunctionTool::new("lookup", None);
    tool.cache = Some(CacheHint::new(None));
    let Request::Generate(req) = generate(
        vec![ToolDef::Function(tool)],
        vec![Block::Document(DocumentBlock::new(Source::text(
            "text/plain",
            "notes",
        )))],
    ) else {
        panic!()
    };
    let caps = req.required_capabilities();
    assert!(caps.text_documents);
    assert!(!caps.pdf);
    assert!(caps.cache_hints);
    assert!(caps.tools);
}

#[test]
fn embedding_requests_need_a_model_and_input() {
    let ok = Request::Embed(EmbedRequest::new(
        "text-embedding-3-small",
        vec![EmbedInput::Text("hi".to_owned())],
    ));
    assert!(ok.validate().is_ok());
    let no_model = Request::Embed(EmbedRequest::new("", vec![EmbedInput::Tokens(vec![1])]));
    assert_eq!(
        no_model.validate().unwrap_err().param.as_deref(),
        Some("model")
    );
    let no_input = Request::Embed(EmbedRequest::new("text-embedding-3-small", vec![]));
    assert_eq!(
        no_input.validate().unwrap_err().param.as_deref(),
        Some("input")
    );
}

#[test]
fn unknown_blocks_that_are_not_objects_carry_no_call_ids() {
    let origin = Origin::new(DialectId::OpenAiResponses);
    let req = generate(
        vec![],
        vec![
            Block::text("Hi"),
            Block::Unknown(RawItem::new(
                origin,
                RawJson::from_string("[1, 2]".to_owned()).unwrap(),
            )),
        ],
    );
    assert!(req.validate().is_ok());
}
