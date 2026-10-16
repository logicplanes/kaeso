use kaeso_kpp::{
    DocumentBlock, ImageBlock, RawItem, ReasoningConfig, ReasoningEffort, ReasoningMode, Role,
    Source, ToolDef,
};

use super::*;

fn base() -> GenerateRequest {
    GenerateRequest::new("gpt-5.5").with_message(Message::user("hi"))
}

fn with_reasoning(f: impl FnOnce(&mut ReasoningConfig)) -> GenerateRequest {
    let mut r = base();
    let mut c = ReasoningConfig::default();
    f(&mut c);
    r.reasoning = Some(c);
    r
}

fn with_block(b: Block) -> GenerateRequest {
    base().with_message(Message::new(Role::User, vec![b]))
}

#[test]
fn a_plain_request_passes() {
    OpenAiResponses::check(&Request::Generate(base())).unwrap();
    OpenAiResponses::check(&Request::CountTokens(base())).unwrap();
}

#[test]
fn sampling_parameters_responses_cannot_express_are_refused() {
    let mut r = base();
    r.sampling.top_k = Some(5);
    assert_eq!(feature(r), "top_k");
    let mut r = base();
    r.sampling.seed = Some(1);
    assert_eq!(feature(r), "seed");
    let mut r = base();
    r.sampling.frequency_penalty = Some(0.5);
    assert_eq!(feature(r), "frequency_penalty");
    let mut r = base();
    r.sampling.presence_penalty = Some(0.5);
    assert_eq!(feature(r), "presence_penalty");
    let mut r = base();
    r.sampling.stop = vec!["END".into()];
    assert_eq!(feature(r), "stop sequences");
}

#[test]
fn reasoning_budgets_and_modes_follow_d4() {
    let effort_for = |mode: ReasoningMode, budget: Option<u32>| {
        let req = Request::Generate(with_reasoning(|c| {
            c.mode = Some(mode);
            c.budget_tokens = budget;
        }));
        OpenAiResponses::check(&req).unwrap();
        let out = OpenAiResponses::encode_request(&req, &mut EgressCtx::new(&req)).unwrap();
        let body: serde_json::Value = serde_json::from_slice(&out.body).unwrap();
        body["reasoning"]["effort"].as_str().map(str::to_owned)
    };
    assert_eq!(
        effort_for(ReasoningMode::Enabled, Some(1_024)).as_deref(),
        Some("low")
    );
    assert_eq!(
        effort_for(ReasoningMode::Enabled, Some(10_000)).as_deref(),
        Some("medium")
    );
    assert_eq!(
        effort_for(ReasoningMode::Enabled, Some(20_000)).as_deref(),
        Some("high")
    );
    assert_eq!(
        effort_for(ReasoningMode::Disabled, None).as_deref(),
        Some("none")
    );
    assert!(
        feature(with_reasoning(
            |c| c.mode = Some(ReasoningMode::Other("x".into()))
        ))
        .starts_with("reasoning mode")
    );
    OpenAiResponses::check(&Request::Generate(with_reasoning(|c| {
        c.mode = Some(ReasoningMode::Adaptive);
        c.effort = Some(ReasoningEffort::High);
    })))
    .unwrap();
}

#[test]
fn documents_and_tool_errors_responses_cannot_express_are_refused() {
    let mut doc = DocumentBlock::new(Source::text("text/plain", "hello"));
    assert_eq!(
        feature(with_block(Block::Document(doc.clone()))),
        "text document"
    );
    doc.source = Source::base64("application/pdf", "JVBE");
    doc.title = Some("t".into());
    assert_eq!(
        feature(with_block(Block::Document(doc.clone()))),
        "document title"
    );
    doc.title = None;
    doc.context = Some("c".into());
    assert_eq!(
        feature(with_block(Block::Document(doc.clone()))),
        "document context"
    );
    doc.context = None;
    doc.citations = true;
    assert_eq!(
        feature(with_block(Block::Document(doc))),
        "document citations"
    );
    assert_eq!(
        feature(with_block(Block::Image(ImageBlock::new(Source::text(
            "image/png",
            "x"
        ))))),
        "text image source"
    );
    let r = base()
        .with_message(Message::new(
            Role::Assistant,
            vec![Block::tool_call("c", "f", raw("{}"))],
        ))
        .with_message(Message::new(
            Role::User,
            vec![Block::tool_result("c", vec![Block::text("boom")], true)],
        ));
    OpenAiResponses::check(&Request::Generate(r.clone())).unwrap();
    assert_eq!(body_of(r)["input"][2]["output"], "boom");
    let doc = DocumentBlock::new(Source::text("text/plain", "x"));
    let r = base()
        .with_message(Message::new(
            Role::Assistant,
            vec![Block::tool_call("c", "f", raw("{}"))],
        ))
        .with_message(Message::new(
            Role::User,
            vec![Block::tool_result("c", vec![Block::Document(doc)], false)],
        ));
    let err = OpenAiResponses::check(&Request::Generate(r)).unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("text document"));
    assert_eq!(
        err.param.as_deref(),
        Some("messages[2].blocks[0].content[0]")
    );
}

#[test]
fn foreign_data_is_refused_via_check_foreign() {
    let a = DialectId::AnthropicMessages;
    let mut r = base();
    r.extras = Extras::new(a).with_field("top_k_like", raw("1"));
    assert_eq!(feature(r), "top_k_like");
    let r = base().with_tool(ToolDef::Unknown(RawItem::new(
        a,
        raw(r#"{"type":"web_search_20250305"}"#),
    )));
    assert_eq!(feature(r), "web_search_20250305 tool");
    let r = with_block(Block::Image(ImageBlock::new(Source::file_id(
        a, "file_011",
    ))));
    assert_eq!(feature(r), "file id");
    let mut thinking = kaeso_kpp::ReasoningBlock::new(a);
    thinking.opaque.push(kaeso_kpp::Opaque::new(
        a,
        kaeso_kpp::OpaqueKind::AnthropicThinkingSignature,
        "s",
    ));
    let r = base().with_message(Message::new(
        Role::Assistant,
        vec![Block::Reasoning(thinking), Block::text("x")],
    ));
    OpenAiResponses::check(&Request::Generate(r)).unwrap();
}

#[test]
fn embeddings_requests_are_refused() {
    let err = OpenAiResponses::check(&Request::Embed(kaeso_kpp::EmbedRequest::new("m", vec![])))
        .unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("embeddings"));
}

#[test]
fn encode_request_runs_check_first() {
    let mut r = base();
    r.sampling.top_k = Some(3);
    let req = Request::Generate(r);
    let mut ctx = EgressCtx::new(&req);
    let err = OpenAiResponses::encode_request(&req, &mut ctx).unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("top_k"));
}
