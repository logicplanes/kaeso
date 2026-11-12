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
            Block::Image(ImageBlock::new(
                Source::base64("image/png", "AA==").unwrap(),
            )),
            "image",
        ),
        (
            Block::Document(DocumentBlock::new(Source::text("text/plain", "notes"))),
            "document",
        ),
        (
            Block::audio(Source::base64("audio/wav", "UklGRg==").unwrap()),
            "audio",
        ),
        (
            Block::video(Source::Url("https://example.com/clip.mp4".to_owned())),
            "video",
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
        Source::base64("image/png", "AA==").unwrap().media_type(),
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

#[test]
fn media_bytes_are_held_once_and_keep_their_json_form() {
    let source = Source::base64("image/png", "iVBORw0KGgo=").unwrap();
    let Source::Base64(b) = &source else {
        panic!("{source:?}")
    };
    assert_eq!(&b.data[..], b"\x89PNG\r\n\x1a\n");
    assert_eq!(b.to_base64(), "iVBORw0KGgo=");
    assert_eq!(b.base64().to_string(), "iVBORw0KGgo=");
    assert_eq!(b.data_url(), "data:image/png;base64,iVBORw0KGgo=");
    assert_eq!(
        b.data_url_with("data:image/png;name=a.png;base64"),
        "data:image/png;name=a.png;base64,iVBORw0KGgo="
    );
    assert_eq!(
        format!("{source:?}"),
        r#"Base64(Base64Source { media_type: "image/png", data: 8 bytes })"#
    );

    let json = kaeso_kpp::serde_json::to_value(&source).unwrap();
    assert_eq!(
        json,
        kaeso_kpp::serde_json::json!({"base64": {"media_type": "image/png", "data": "iVBORw0KGgo="}})
    );
    let back: Source = kaeso_kpp::serde_json::from_value(json).unwrap();
    assert_eq!(back, source);
    let unpadded: Source = kaeso_kpp::serde_json::from_str(
        r#"{"base64": {"media_type": "image/png", "data": "iVBORw0KGgo"}}"#,
    )
    .unwrap();
    assert_eq!(unpadded, source);
    assert!(
        kaeso_kpp::serde_json::from_str::<Source>(
            r#"{"base64": {"media_type": "image/png", "data": "not base64!"}}"#
        )
        .is_err()
    );

    let shared = kaeso_kpp::bytes::Bytes::from_static(b"\x89PNG\r\n\x1a\n");
    let from_bytes = Source::bytes("image/png", shared.clone());
    assert_eq!(from_bytes, source);
    let Source::Base64(b) = from_bytes.clone() else {
        panic!()
    };
    assert_eq!(b.data.as_ptr(), shared.as_ptr(), "cloning shares the bytes");

    let refused = Source::base64("image/png", "not base64!").unwrap_err();
    assert_eq!(refused.kind, ErrorKind::InvalidRequest);
}

#[cfg(test)]
mod decisions {
    use kaeso_kpp::{
        Answer, AnswerValue, Asset, Block, BoxAnswer, BoxQuestion, Choice, ChoiceAnswer,
        ChoiceOption, ChoiceValue, DecideRequest, DecideResponse, DecisionCache, DecisionInput,
        Description, DialectId, ImageBlock, Instructions, JsonBlock, Level, Message, Predicate,
        PredicateAnswer, Question, QuestionKind, RawJson, Refusal, Reply, Request, RequestKind,
        Role, Score, ScoreAnswer, Source, StreamMode,
    };
    use serde_json::json;

    fn raw(s: &str) -> RawJson {
        RawJson::from_string(s.to_owned()).unwrap()
    }

    fn noul(name: &str) -> Question {
        Question::new(
            Some(name.to_owned()),
            QuestionKind::Predicate(Predicate::default()),
        )
    }

    fn choice(options: &[&str]) -> QuestionKind {
        QuestionKind::Choice(Choice::new(
            options
                .iter()
                .map(|o| ChoiceOption::new(ChoiceValue::Text((*o).to_owned()), None))
                .collect(),
        ))
    }

    #[test]
    fn system_one_is_a_dialect() {
        assert_eq!(DialectId::SystemOne.as_str(), "system_one");
        assert_eq!(DialectId::SystemOne.display_name(), "System One");
        assert_eq!(
            "system_one".parse::<DialectId>().unwrap(),
            DialectId::SystemOne
        );
        assert!(DialectId::ALL.contains(&DialectId::SystemOne));
    }

    #[test]
    fn a_decision_request_is_its_own_kind_and_never_streams() {
        let req = Request::Decide(
            DecideRequest::new("jev-latest", DecisionInput::Text("Hi".into()))
                .with_question(noul("spam")),
        );
        assert_eq!(req.kind(), RequestKind::Decide);
        assert_eq!(req.model(), "jev-latest");
        assert_eq!(req.stream_mode(), StreamMode::Off);
        req.validate().unwrap();
        let reply = Reply::Decide(DecideResponse::new("jev-1.14.0", vec![]));
        assert_eq!(reply.kind(), RequestKind::Decide);
    }

    #[test]
    fn validation_needs_a_model_questions_options_and_used_assets() {
        let base = || DecideRequest::new("m", DecisionInput::Text("photo-1 and more".into()));
        let param = |r: DecideRequest| r.validate().unwrap_err().param;
        assert_eq!(
            param(DecideRequest::new("", DecisionInput::Text("x".into())).with_question(noul("a"))),
            Some("model".into())
        );
        assert_eq!(param(base()), Some("questions".into()));
        assert_eq!(
            param(base().with_question(noul("a")).with_question(noul("a"))),
            Some("questions[1]".into())
        );
        assert_eq!(
            param(base().with_question(Question::new(None, choice(&[])))),
            Some("questions[0]".into())
        );
        assert_eq!(
            param(base().with_question(Question::new(None, QuestionKind::Score(Score::default())))),
            Some("questions[0]".into())
        );
        base()
            .with_question(Question::new(None, choice(&["a"])))
            .with_question(Question::new(None, choice(&["b"])))
            .validate()
            .unwrap();
        let image = || Asset::new(Source::bytes("image/png", vec![0u8; 4]));
        base()
            .with_question(noul("a"))
            .with_asset("photo-1", image())
            .validate()
            .unwrap();
        assert_eq!(
            param(
                base()
                    .with_question(noul("a"))
                    .with_asset("photo-2", image())
            ),
            Some("assets.photo-2".into())
        );
        assert_eq!(
            param(base().with_question(noul("a")).with_asset(" ", image())),
            Some("assets. ".into())
        );
        let in_key = DecideRequest::new("m", DecisionInput::Json(raw(r#"{"photo-1":"x"}"#)))
            .with_question(noul("a"))
            .with_asset("photo-1", image());
        assert_eq!(param(in_key), Some("assets.photo-1".into()));
        let mut described = Predicate::default();
        described.when_true = Some(Description::Json(raw(r#"{"examples":["photo-3"]}"#)));
        DecideRequest::new("m", DecisionInput::Text("x".into()))
            .with_question(Question::new(None, QuestionKind::Predicate(described)))
            .with_asset("photo-3", image())
            .validate()
            .unwrap();
        let mut level = Level::new(
            Some("low".into()),
            Some(Description::Text("photo-4".into())),
        );
        level.label = Some("low".into());
        DecideRequest::new("m", DecisionInput::Messages(vec![Message::user("x")]))
            .with_question(
                Question::new(None, QuestionKind::Score(Score::new(vec![level])))
                    .with_instructions(Instructions::Blocks(vec![Block::Json(JsonBlock::new(
                        raw(r#"["photo-5"]"#),
                    ))])),
            )
            .with_asset("photo-4", image())
            .with_asset("photo-5", image())
            .validate()
            .unwrap();
    }

    #[test]
    fn capabilities_name_media_boxes_and_cache() {
        let text =
            DecideRequest::new("m", DecisionInput::Text("x".into())).with_question(noul("a"));
        assert_eq!(
            text.required_capabilities(),
            kaeso_kpp::Capabilities::default()
        );
        let mut boxed = text
            .clone()
            .with_question(Question::new(None, QuestionKind::Box(BoxQuestion::new())));
        boxed.cache = Some(DecisionCache::new(Some("k".into())));
        let caps = boxed.required_capabilities();
        assert!(caps.box_questions && caps.cache_hints && !caps.images);
        let media = DecideRequest::new(
            "m",
            DecisionInput::Messages(vec![Message::new(
                Role::User,
                vec![
                    Block::audio(Source::Url("https://example.com/a.wav".into())),
                    Block::video(Source::Url("https://example.com/v.mp4".into())),
                    Block::Document(kaeso_kpp::DocumentBlock::new(Source::Url(
                        "https://example.com/d.pdf".into(),
                    ))),
                ],
            )]),
        )
        .with_question(noul("a").with_instructions(Instructions::Blocks(vec![
            Block::Image(ImageBlock::new(Source::Url(
                "https://example.com/i.png".into(),
            ))),
        ])));
        let caps = media.required_capabilities();
        assert!(caps.audio_input && caps.video_input && caps.pdf && caps.images);
        let with_asset = text.with_asset(
            "x",
            Asset::new(Source::Url("https://example.com/i.png".into())),
        );
        assert!(with_asset.required_capabilities().images);
    }

    #[test]
    fn decisions_have_a_small_stable_json_form() {
        let req = DecideRequest::new("jev-latest", DecisionInput::Text("Hi".into())).with_question(
            Question::new(Some("tone".into()), choice(&["calm", "angry"])),
        );
        assert_eq!(
            serde_json::to_value(Request::Decide(req.clone())).unwrap(),
            json!({"decide": {"model": "jev-latest", "input": {"text": "Hi"}, "questions": [
                {"name": "tone", "kind": {"choice": {"options": [
                    {"value": {"text": "calm"}}, {"value": {"text": "angry"}}
                ]}}}
            ]}})
        );
        let back: DecideRequest =
            serde_json::from_value(serde_json::to_value(&req).unwrap()).unwrap();
        assert_eq!(back, req);
        let reply = DecideResponse::new(
            "jev-1.14.0",
            vec![
                Answer::new(AnswerValue::Predicate(PredicateAnswer::new(0.9))),
                Answer::new(AnswerValue::Choice(ChoiceAnswer::new(
                    1,
                    vec![Some(0.2), None],
                )))
                .with_confidence(Some(0.6)),
                Answer::new(AnswerValue::Score(ScoreAnswer::new(
                    1.5,
                    vec![Some(0.5), Some(0.5)],
                ))),
                Answer::new(AnswerValue::Box(BoxAnswer::new(0.1, 0.9, 0.2, 0.8))),
                Answer::new(AnswerValue::Refusal(Refusal::new())),
            ],
        );
        let v = serde_json::to_value(Reply::Decide(reply.clone())).unwrap();
        assert_eq!(
            v["decide"]["answers"],
            json!([
                {"value": {"predicate": {"probability": 0.9}}},
                {"value": {"choice": {"option": 1, "probabilities": [0.2, null]}}, "confidence": 0.6},
                {"value": {"score": {"score": 1.5, "probabilities": [0.5, 0.5]}}},
                {"value": {"box": {"top": 0.1, "bottom": 0.9, "left": 0.2, "right": 0.8}}},
                {"value": {"refusal": {}}}
            ])
        );
        let names: Vec<&str> = reply.answers.iter().map(|a| a.value.as_str()).collect();
        assert_eq!(names, ["predicate", "choice", "score", "box", "refusal"]);
        assert_eq!(
            serde_json::from_value::<Reply>(v).unwrap(),
            Reply::Decide(reply)
        );
        let q = Question::new(None, QuestionKind::Box(BoxQuestion::new()));
        assert_eq!(q.type_name(), "box");
        assert_eq!(Block::json(raw("{}")).kind(), "json");
        assert!(Block::json(raw("{}")).extras().is_some());
    }
}
