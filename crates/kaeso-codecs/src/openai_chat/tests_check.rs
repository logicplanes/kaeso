use kaeso_kpp::{
    Block, CacheHint, Citation, DialectId, DocumentBlock, EmbedInput, EmbedRequest, ErrorKind,
    Extras, GenerateRequest, ImageBlock, Message, Opaque, OpaqueKind, Origin, RawItem, RawJson,
    ReasoningBlock, ReasoningConfig, ReasoningEffort, ReasoningMode, ReasoningSummary, Request,
    Role, Source, TextBlock, ToolDef,
};

use super::OpenAiChat;
use crate::codec::{Codec, EgressCtx};

fn raw(s: &str) -> RawJson {
    RawJson::from_string(s.to_owned()).unwrap()
}

fn base() -> GenerateRequest {
    GenerateRequest::new("gpt-5.5").with_message(Message::user("hi"))
}

fn feature(req: GenerateRequest) -> String {
    let req = Request::Generate(req);
    let err = OpenAiChat::check(&req).unwrap_err();
    let mut ctx = EgressCtx::new(&req);
    assert_eq!(
        OpenAiChat::encode_request(&req, &mut ctx).unwrap_err(),
        err,
        "encode_request runs check first"
    );
    err.unsupported_feature()
        .unwrap_or_else(|| panic!("not Unsupported: {err:?}"))
        .to_owned()
}

fn passes(req: GenerateRequest) {
    let req = Request::Generate(req);
    OpenAiChat::check(&req).unwrap();
    let mut ctx = EgressCtx::new(&req);
    OpenAiChat::encode_request(&req, &mut ctx).unwrap();
}

fn encode_body(req: GenerateRequest) -> (serde_json::Value, EgressCtx) {
    let req = Request::Generate(req);
    let mut ctx = EgressCtx::new(&req);
    let out = OpenAiChat::encode_request(&req, &mut ctx).unwrap();
    (serde_json::from_slice(&out.body).unwrap(), ctx)
}

fn with_user_block(block: Block) -> GenerateRequest {
    GenerateRequest::new("m").with_message(Message::new(Role::User, vec![block]))
}

fn anthropic() -> Origin {
    Origin::new(DialectId::AnthropicMessages)
}

#[test]
fn other_endpoints_are_refused() {
    let err = OpenAiChat::check(&Request::CountTokens(base())).unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("token counting"));
    let embed = Request::Embed(EmbedRequest::new("e", vec![EmbedInput::Text("x".into())]));
    assert!(
        OpenAiChat::check(&embed)
            .unwrap_err()
            .unsupported_feature()
            .is_some()
    );
}

#[test]
fn foreign_extras_are_refused_via_cross() {
    let mut r = base();
    r.extras = Extras::new(anthropic()).with_field("service_tier", raw("\"auto\""));
    assert_eq!(feature(r), "service_tier");
    let mut r = base();
    r.tools.push(ToolDef::Unknown(RawItem::new(
        anthropic(),
        raw(r#"{"type":"web_search_20250305","name":"web_search"}"#),
    )));
    assert_eq!(feature(r), "web_search_20250305 tool");
}

#[test]
fn top_k_is_refused() {
    let mut r = base();
    r.sampling.top_k = Some(40);
    assert_eq!(feature(r), "top_k");
}

#[test]
fn more_than_four_stop_sequences_are_refused() {
    let mut r = base();
    r.sampling.stop = (0..5).map(|i| i.to_string()).collect();
    assert_eq!(feature(r.clone()), "stop");
    r.sampling.stop.truncate(4);
    passes(r);
}

#[test]
fn reasoning_config_chat_cannot_express_is_refused() {
    let effort_for = |mode: Option<ReasoningMode>, budget: Option<u32>| {
        let mut config = ReasoningConfig::default();
        config.mode = mode;
        config.budget_tokens = budget;
        let mut r = base();
        r.reasoning = Some(config);
        passes(r.clone());
        encode_body(r).0.get("reasoning_effort").cloned()
    };
    let enabled = Some(ReasoningMode::Enabled);
    assert_eq!(effort_for(enabled.clone(), Some(2_048)), Some("low".into()));
    assert_eq!(
        effort_for(enabled.clone(), Some(4_096)),
        Some("medium".into())
    );
    assert_eq!(
        effort_for(enabled.clone(), Some(16_383)),
        Some("medium".into())
    );
    assert_eq!(
        effort_for(enabled.clone(), Some(16_384)),
        Some("high".into())
    );
    assert_eq!(effort_for(enabled, None), None);
    assert_eq!(
        effort_for(Some(ReasoningMode::Disabled), None),
        Some("none".into())
    );

    let mut config = ReasoningConfig::default();
    config.mode = Some(ReasoningMode::Other("between_tools".into()));
    let mut r = base();
    r.reasoning = Some(config.clone());
    assert_eq!(feature(r), "reasoning.mode");

    config.mode = None;
    config.summary = Some(ReasoningSummary::Auto);
    config.include_encrypted = true;
    let mut r = base();
    r.reasoning = Some(config.clone());
    passes(r.clone());
    let (body, _) = encode_body(r);
    assert!(
        body.get("reasoning").is_none() && body.get("include").is_none(),
        "{body}"
    );
    let mut own = config.clone();
    own.extras = Extras::new(DialectId::OpenAiChat);
    let mut r = base();
    r.reasoning = Some(own.clone());
    assert_eq!(feature(r), "reasoning.summary");
    own.summary = None;
    let mut r = base();
    r.reasoning = Some(own);
    assert_eq!(feature(r), "reasoning.include_encrypted");

    config.summary = None;
    config.include_encrypted = false;
    config.mode = Some(ReasoningMode::Adaptive);
    config.effort = Some(ReasoningEffort::Max);
    let mut r = base();
    r.reasoning = Some(config);
    passes(r);
}

#[test]
fn citations_outside_assistant_messages_are_refused() {
    let mut text = TextBlock::new("cited");
    text.citations.push(Citation::new(
        Origin::new(DialectId::OpenAiChat),
        raw(r#"{"type":"url_citation"}"#),
    ));
    let r = base().with_message(Message::new(Role::User, vec![Block::Text(text.clone())]));
    assert_eq!(feature(r), "citations");
    passes(base().with_message(Message::new(Role::Assistant, vec![Block::Text(text)])));
    let mut foreign = TextBlock::new("cited");
    foreign.citations.push(Citation::new(
        anthropic(),
        raw(r#"{"type":"char_location"}"#),
    ));
    let r = base().with_message(Message::new(Role::Assistant, vec![Block::Text(foreign)]));
    assert_eq!(feature(r), "citations");
}

#[test]
fn documents_chat_cannot_carry_are_refused() {
    let doc = |source| DocumentBlock::new(source);
    assert_eq!(
        feature(with_user_block(Block::Document(doc(Source::text(
            "text/plain",
            "x"
        ))))),
        "text document"
    );
    assert_eq!(
        feature(with_user_block(Block::Document(doc(Source::Url(
            "https://example.com/a.pdf".into()
        ))))),
        "document URL"
    );
    let mut d = doc(Source::base64("application/pdf", "JVBE"));
    d.citations = true;
    assert_eq!(
        feature(with_user_block(Block::Document(d))),
        "document citations"
    );
    let mut d = doc(Source::base64("application/pdf", "JVBE"));
    d.title = Some("Report".into());
    assert_eq!(
        feature(with_user_block(Block::Document(d))),
        "document title"
    );
    let mut d = doc(Source::base64("application/pdf", "JVBE"));
    d.context = Some("Q3".into());
    assert_eq!(
        feature(with_user_block(Block::Document(d))),
        "document context"
    );
    passes(with_user_block(Block::Document(doc(Source::base64(
        "application/pdf",
        "JVBE",
    )))));
}

#[test]
fn image_file_ids_are_refused() {
    let img = ImageBlock::new(Source::file_id(DialectId::OpenAiResponses, "file-1"));
    assert_eq!(feature(with_user_block(Block::Image(img))), "image file id");
}

#[test]
fn foreign_file_ids_are_refused_via_cross() {
    let doc = DocumentBlock::new(Source::file_id(anthropic(), "file_011"));
    assert_eq!(feature(with_user_block(Block::Document(doc))), "file id");
    let doc = DocumentBlock::new(Source::file_id(DialectId::OpenAiResponses, "file-1"));
    passes(with_user_block(Block::Document(doc)));
}

#[test]
fn content_in_roles_that_cannot_carry_it_is_refused() {
    let img = || Block::image(Source::Url("https://example.com/a.png".into()));
    let r = base().with_message(Message::new(Role::Assistant, vec![img()]));
    assert_eq!(feature(r), "image in assistant messages");
    let r = base().with_message(Message::new(Role::System, vec![img()]));
    assert_eq!(feature(r), "image in system, developer and tool messages");
    let r = with_user_block(Block::Refusal(kaeso_kpp::RefusalBlock::new("no")));
    assert_eq!(feature(r), "refusal in user messages");
    let doc = Block::Document(DocumentBlock::new(Source::base64(
        "application/pdf",
        "JVBE",
    )));
    let r = base()
        .with_message(Message::new(
            Role::Assistant,
            vec![Block::tool_call("c1", "f", raw("{}"))],
        ))
        .with_message(Message::new(
            Role::User,
            vec![Block::tool_result("c1", vec![doc], false)],
        ));
    assert_eq!(
        feature(r),
        "document in system, developer and tool messages"
    );
}

#[test]
fn hints_are_dropped_not_refused() {
    let mut text = TextBlock::new("cached");
    text.cache = Some(CacheHint::default());
    let mut reasoning = ReasoningBlock::new(anthropic());
    reasoning.opaque.push(Opaque::new(
        anthropic(),
        OpaqueKind::AnthropicThinkingSignature,
        "s",
    ));
    let r = GenerateRequest::new("m")
        .with_message(Message::new(Role::User, vec![Block::Text(text)]))
        .with_message(Message::new(
            Role::Assistant,
            vec![
                Block::Reasoning(reasoning),
                Block::tool_call("c1", "f", raw("{}")),
            ],
        ))
        .with_message(Message::new(
            Role::User,
            vec![Block::tool_result("c1", vec![Block::text("boom")], true)],
        ));
    passes(r);
}

#[test]
fn invalid_requests_fail_validation_after_the_feature_rules() {
    let r = GenerateRequest::new("m").with_message(Message::new(
        Role::User,
        vec![Block::tool_result("missing", vec![], false)],
    ));
    let err = OpenAiChat::check(&Request::Generate(r)).unwrap_err();
    assert_eq!(err.kind, ErrorKind::InvalidRequest);
}

#[test]
fn inline_text_images_are_refused() {
    let img = ImageBlock::new(Source::text("image/svg+xml", "<svg/>"));
    assert_eq!(feature(with_user_block(Block::Image(img))), "image source");
}
