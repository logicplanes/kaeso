use kaeso_kpp::{
    Block, DialectId, DocumentBlock, ErrorKind, Extras, GenerateRequest, ImageBlock, Message,
    Opaque, OpaqueKind, ReasoningBlock, Request, RequestKind, Role, Source, StreamEvent,
    StreamMode, ToolCallBlock, ToolResultBlock,
};
use serde_json::{Value, json};

use super::*;
use crate::test_support::{assert_json_eq, assert_sse_eq};

fn round_trip_request(action: GeminiAction, body: Value) -> Request {
    let bytes = serde_json::to_vec(&body).unwrap();
    let req =
        crate::dispatch::decode_gemini_request("gemini-x", action, &bytes, &HeaderList::new())
            .unwrap_or_else(|e| panic!("{e}"));
    let mut ctx = EgressCtx::new(&req);
    let enc = Gemini::encode_request(&req, &mut ctx).unwrap_or_else(|e| panic!("{e}"));
    assert_json_eq("edge request", &bytes, &enc.body);
    req
}

fn round_trip_reply(body: Value) -> kaeso_kpp::Reply {
    let bytes = serde_json::to_vec(&body).unwrap();
    let ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off);
    let reply = Gemini::decode_response(200, &HeaderList::new(), &bytes, &ctx).unwrap();
    let out = Gemini::encode_response(
        &reply,
        &EncodeCtx::new(RequestKind::Generate, StreamMode::Off),
    )
    .unwrap();
    assert_json_eq("edge reply", &bytes, &out);
    reply
}

fn stream(bytes: &[u8]) -> Vec<StreamEvent> {
    let mut dec = Gemini::stream_decoder(&EgressCtx::for_kind(
        RequestKind::Generate,
        StreamMode::on(true),
    ));
    let mut events = dec.feed(bytes);
    events.extend(dec.finish());
    events
}

fn replay(events: &[StreamEvent]) -> Vec<u8> {
    let mut enc =
        Gemini::stream_encoder(&EncodeCtx::new(RequestKind::Generate, StreamMode::on(true)));
    let mut out = Vec::new();
    for e in events {
        out.extend(enc.encode(e).unwrap());
    }
    out.extend(enc.finish().unwrap());
    out
}

fn foreign(mut r: GenerateRequest) -> Request {
    r.extras = Extras::new(DialectId::AnthropicMessages);
    Request::Generate(r)
}

fn encode(req: &Request) -> Result<Value, kaeso_kpp::KppError> {
    let mut ctx = EgressCtx::new(req);
    Gemini::encode_request(req, &mut ctx).map(|e| serde_json::from_slice(&e.body).unwrap())
}

#[test]
fn rare_request_shapes_round_trip() {
    round_trip_request(
        GeminiAction::GenerateContent,
        json!({
            "model": "models/gemini-x",
            "systemInstruction": {"role": "system", "parts": [{"text": "Be brief."}], "xNote": 1},
            "contents": [
                {"role": "user", "parts": []},
                {"parts": [{"text": "hi", "thought": false}]},
                {"role": "user", "parts": [{"text": "a"}, {"text": "b", "thoughtSignature": "c2ln"}]},
                {"role": "model", "parts": [{"text": "", "thought": true, "thoughtSignature": "dA=="}, {"text": "x"}]}
            ],
            "tools": [
                {"functionDeclarations": [
                    {"name": "f", "parameters": {"type": "OBJECT"}, "parametersJsonSchema": {"type": "object"}, "behavior": "BLOCKING"},
                    {"name": "g"}
                ]},
                {"functionDeclarations": [{"name": "h", "description": "d"}]}
            ],
            "toolConfig": {"functionCallingConfig": {}, "retrievalConfig": {"languageCode": "de"}},
            "generationConfig": {
                "stopSequences": [],
                "responseMimeType": "text/x.enum",
                "responseSchema": {"type": "STRING", "enum": ["a", "b"]},
                "responseJsonSchema": {"type": "string"},
                "temperature": 1,
                "frequencyPenalty": 0.5,
                "thinkingConfig": {"thinkingLevel": "low", "thinkingBudget": -5, "includeThoughts": false}
            },
            "store": false
        }),
    );
    round_trip_request(
        GeminiAction::GenerateContent,
        json!({
            "contents": [{"role": "user", "parts": [{"text": "q"}]}],
            "toolConfig": {"functionCallingConfig": {"mode": "VALIDATED"}},
            "generationConfig": {
                "responseMimeType": "application/json",
                "responseJsonSchema": {"type": "object"},
                "responseSchema": {"type": "OBJECT"},
                "thinkingConfig": {}
            }
        }),
    );
    round_trip_request(
        GeminiAction::GenerateContent,
        json!({
            "contents": [{"role": "user", "parts": [{"text": "q"}]}],
            "toolConfig": {"functionCallingConfig": {"mode": "ANY", "allowedFunctionNames": ["a", "b"]}},
            "generationConfig": {"responseMimeType": "application/json", "thinkingConfig": {"thinkingBudget": 0}}
        }),
    );
    for budget in [-1, 512] {
        round_trip_request(
            GeminiAction::GenerateContent,
            json!({
                "contents": [{"role": "user", "parts": [{"text": "q"}]}],
                "tools": [{"functionDeclarations": [{"name": "f"}]}],
                "toolConfig": {"functionCallingConfig": {"mode": "NONE"}},
                "generationConfig": {"thinkingConfig": {"thinkingBudget": budget, "thinkingLevel": "HIGH"}}
            }),
        );
    }
    round_trip_request(
        GeminiAction::CountTokens,
        json!({"contents": [{"role": "user", "parts": [{"text": "q"}]}], "xCount": true}),
    );
}

#[test]
fn rare_function_responses_and_media_round_trip() {
    let req = round_trip_request(
        GeminiAction::GenerateContent,
        json!({
            "contents": [
                {"role": "model", "parts": [{"functionCall": {"name": "f", "args": {"a": 1}, "xKey": 1}}, {"functionCall": {"id": "c2", "name": "g"}}]},
                {"role": "user", "parts": [
                    {"functionResponse": {"name": "f", "response": {"output": 3}}},
                    {"functionResponse": {"name": "unmatched", "response": {"error": "no"}}},
                    {"functionResponse": {"id": "c2", "name": "other", "willContinue": true}},
                    {"functionResponse": {"id": "c3", "name": "h", "response": {"output": "x"}, "parts": [
                        {"fileData": {"mimeType": "application/pdf", "fileUri": "files/abc"}},
                        {"fileData": {"mimeType": "video/mp4", "fileUri": "files/vid"}},
                        {"inlineData": {"mimeType": "audio/wav", "data": "AAAA"}},
                        {"futurePart": 1},
                        "not an object"
                    ]}}
                ]},
                {"role": "user", "parts": [
                    {"inlineData": {"mimeType": "application/pdf", "data": "JVBERg==", "displayName": "a.pdf"}},
                    {"fileData": {"fileUri": "https://example.com/doc"}},
                    {"fileData": {"mimeType": "image/png", "fileUri": "https://example.com/i.png"}},
                    {"text": "a", "inlineData": {"mimeType": "image/png", "data": "AAAA"}},
                    {"thought": true, "functionCall": {"name": "f"}},
                    "not an object"
                ]}
            ]
        }),
    );
    let Request::Generate(r) = req else { panic!() };
    let Block::ToolResult(unmatched) = &r.messages[1].blocks[1] else {
        panic!()
    };
    assert!(unmatched.is_error);
    assert!(unmatched.call_id.starts_with(parts::GENERATED_ID_PREFIX));
}

#[test]
fn content_from_other_formats_takes_gemini_parts() {
    let call = ToolCallBlock::new(
        "call_1",
        "f",
        kaeso_kpp::RawJson::from_string("{}".into()).unwrap(),
    );
    let mut doc = DocumentBlock::new(Source::text("text/plain", "hello"));
    doc.extras = Extras::new(DialectId::AnthropicMessages);
    let gemini_file = ImageBlock::new(Source::file_id(DialectId::Gemini, "files/img"));
    let mut result = ToolResultBlock::new(
        "call_1",
        vec![
            Block::text("see"),
            Block::Document(DocumentBlock::new(Source::Url(
                "https://example.com/r.pdf".into(),
            ))),
            Block::image(Source::Url("data:image/png;base64,AAAA".into())),
        ],
    );
    result.extras = Extras::new(DialectId::AnthropicMessages);
    let r = GenerateRequest::new("gemini-x")
        .with_message(Message::new(
            Role::User,
            vec![
                Block::Document(doc),
                Block::Image(gemini_file),
                Block::Document(DocumentBlock::new(Source::Url(
                    "https://example.com/notes".into(),
                ))),
            ],
        ))
        .with_message(Message::new(Role::Assistant, vec![Block::ToolCall(call)]))
        .with_message(Message::new(Role::User, vec![Block::ToolResult(result)]));
    let body = encode(&foreign(r.clone())).unwrap();
    let parts = &body["contents"][0]["parts"];
    assert_eq!(
        parts[0]["inlineData"],
        json!({"mimeType": "text/plain", "data": "aGVsbG8="})
    );
    assert_eq!(parts[1]["fileData"], json!({"fileUri": "files/img"}));
    assert_eq!(
        parts[2]["fileData"],
        json!({"fileUri": "https://example.com/notes"})
    );
    let response = &body["contents"][2]["parts"][0]["functionResponse"];
    assert_eq!(
        response["parts"],
        json!([
            {"fileData": {"mimeType": "application/pdf", "fileUri": "https://example.com/r.pdf"}},
            {"inlineData": {"mimeType": "image/png", "data": "AAAA"}}
        ])
    );
    let mut bad = r.clone();
    bad.messages[2].blocks = vec![Block::tool_result(
        "call_1",
        vec![Block::image(Source::Url("data:image/png,raw".into()))],
        false,
    )];
    assert_eq!(
        encode(&foreign(bad)).unwrap_err().unsupported_feature(),
        Some("image data URL that is not base64")
    );
    let mut nested = r.clone();
    nested.messages[2].blocks = vec![Block::tool_result(
        "call_1",
        vec![Block::tool_result("x", vec![], false)],
        false,
    )];
    assert_eq!(
        encode(&foreign(nested)).unwrap_err().unsupported_feature(),
        Some("tool_result block in a tool result")
    );
    let mut openai_file = r;
    openai_file.messages[0].blocks = vec![Block::image(Source::file_id(
        DialectId::OpenAiChat,
        "file-1",
    ))];
    let req = foreign(openai_file);
    assert!(Gemini::check(&req, &EgressCtx::new(&req)).is_err());
}

#[test]
fn signatures_without_a_following_part_still_reach_gemini() {
    let sig = |s: &str| {
        let mut b = ReasoningBlock::new(DialectId::Gemini);
        b.opaque.push(Opaque::new(
            DialectId::Gemini,
            OpaqueKind::GeminiThoughtSignature,
            s,
        ));
        Block::Reasoning(b)
    };
    let mut thought = ReasoningBlock::new(DialectId::Gemini);
    thought.text.push("why".into());
    let mut anthropic = ReasoningBlock::new(DialectId::AnthropicMessages);
    anthropic.text.push("hidden".into());
    let r = GenerateRequest::new("gemini-x")
        .with_message(Message::user("q"))
        .with_message(Message::new(
            Role::Assistant,
            vec![
                Block::Reasoning(anthropic),
                sig("a"),
                sig("b"),
                Block::Reasoning(thought),
                sig("c"),
                Block::Unknown(kaeso_kpp::RawItem::new(
                    DialectId::Gemini,
                    kaeso_kpp::RawJson::from_string(r#"{"executableCode":{"code":"1"}}"#.into())
                        .unwrap(),
                )),
                sig("d"),
            ],
        ));
    let mut r = r;
    r.extras = Extras::new(DialectId::Gemini);
    let body = encode(&Request::Generate(r)).unwrap();
    assert_eq!(
        body["contents"][1]["parts"],
        json!([
            {"text": "", "thoughtSignature": "a"},
            {"text": "", "thoughtSignature": "b"},
            {"text": "why", "thought": true},
            {"text": "", "thoughtSignature": "c"},
            {"executableCode": {"code": "1"}},
            {"text": "", "thoughtSignature": "d"}
        ])
    );
}

#[test]
fn rare_reply_shapes_round_trip() {
    round_trip_reply(json!({"candidates": []}));
    round_trip_reply(json!({"candidates": [{"finishReason": "STOP"}], "responseId": "r"}));
    round_trip_reply(json!({"candidates": [{"content": {"parts": [{"text": "x"}]}}]}));
    round_trip_reply(
        json!({"candidates": [{"content": {"role": "assistant", "parts": [{"text": "x"}]}, "finishReason": "SAFETY"}]}),
    );
    let reply = round_trip_reply(json!({
        "candidates": [{"content": {"role": "model", "parts": [{"functionCall": {"name": "f", "args": {}}}]}, "finishReason": "STOP"}],
        "responseId": "Ab-c_1/2"
    }));
    let kaeso_kpp::Reply::Generate(r) = reply else {
        panic!()
    };
    let Block::ToolCall(c) = &r.blocks[0] else {
        panic!()
    };
    assert_eq!(c.id, format!("{}Ab-c_12_0", parts::GENERATED_ID_PREFIX));
    let ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off);
    for bad in [
        r#"{"usageMetadata": {"promptTokenCount": "x"}}"#,
        r#"{"candidates": [{"content": {"parts": [1]}}], "usageMetadata": "x"}"#,
    ] {
        let e = Gemini::decode_response(200, &HeaderList::new(), bad.as_bytes(), &ctx).unwrap_err();
        assert_eq!(e.kind, ErrorKind::ProviderError, "{bad}");
    }
    let e = Gemini::decode_response(
        400,
        &HeaderList::new(),
        br#"{"error": {"code": 400, "message": "Bad.", "status": "INVALID_ARGUMENT"}}"#,
        &ctx,
    )
    .unwrap_err();
    assert_eq!(e.message, "Bad.");
    let count = EgressCtx::for_kind(RequestKind::CountTokens, StreamMode::Off);
    assert!(Gemini::decode_response(200, &HeaderList::new(), b"[]", &count).is_err());
    let embed = EgressCtx::for_kind(RequestKind::Embed, StreamMode::Off);
    assert!(Gemini::decode_response(200, &HeaderList::new(), b"{}", &embed).is_err());
    let reply = kaeso_kpp::Reply::Embed(kaeso_kpp::EmbedResponse::new("m", vec![]));
    assert!(
        Gemini::encode_response(&reply, &EncodeCtx::new(RequestKind::Embed, StreamMode::Off))
            .is_err()
    );
}

#[test]
fn rare_stream_shapes_replay_exactly() {
    let sse = concat!(
        "id: 7\n",
        "data: {\"candidates\": [{\"content\": {\"parts\": [{\"text\": \"plan\", \"thought\": true, \"thoughtSignature\": \"c2ln\"}], \"role\": \"model\"}}], \"responseId\": \"r\"}\n\n",
        "data: {\"candidates\": [{\"content\": {\"parts\": [{\"executableCode\": {\"code\": \"1\"}}, {\"inlineData\": {\"mimeType\": \"image/png\", \"data\": \"AAAA\"}, \"thoughtSignature\": \"aW1n\"}, {\"text\": \"x\", \"xKey\": 1}]}}]}\n\n",
        "data: {\"promptFeedback\": {}}\n\n",
        "data: {\"candidates\": [{\"content\": {\"role\": \"model\"}, \"finishReason\": \"MAX_TOKENS\"}], \"usageMetadata\": {\"promptTokenCount\": 1}}\n\n",
    );
    let events = stream(sse.as_bytes());
    assert!(
        !events.iter().any(|e| matches!(e, StreamEvent::Error(_))),
        "{events:?}"
    );
    assert_sse_eq("rare stream", sse.as_bytes(), &replay(&events));
    let stripped: Vec<StreamEvent> = events
        .iter()
        .cloned()
        .map(|mut e| {
            if let StreamEvent::BlockDelta(d) = &mut e {
                d.extras.form.clear();
            }
            e
        })
        .collect();
    let out = String::from_utf8(replay(&stripped)).unwrap();
    assert!(out.contains("c2ln"), "{out}");
}

#[test]
fn broken_streams_end_with_an_error() {
    let last_error = |bytes: &[u8]| match stream(bytes).last() {
        Some(StreamEvent::Error(e)) => e.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(
        last_error(b"data: {\"candidates\": 3}\n\n").kind,
        ErrorKind::ProviderError
    );
    assert_eq!(
        last_error(b"data: {\"candidates\": [{\"finishReason\": \"STOP\"}], \"usageMetadata\": {\"promptTokenCount\": \"x\"}}\n\n").kind,
        ErrorKind::ProviderError
    );
    assert_eq!(last_error(b"data: \xff\n\n").kind, ErrorKind::ProviderError);
    assert_eq!(last_error(b"data: {\"cand").kind, ErrorKind::ProviderError);
    let mut dec = Gemini::stream_decoder(&EgressCtx::for_kind(
        RequestKind::Generate,
        StreamMode::on(true),
    ));
    dec.feed(b"data: \xff\n\n");
    assert!(dec.feed(b"data: {}\n\n").is_empty());
    assert!(dec.finish().is_empty());
    let events = stream(b"data: {\"candidates\": [{\"content\": {\"parts\": [{\"functionCall\": {\"name\": \"f\", \"args\": {}}}]}, \"finishReason\": \"STOP\"}]}\n\ndata: {}\n");
    assert!(matches!(events.last(), Some(StreamEvent::ResponseStop(_))));
}

#[test]
fn tool_arguments_that_are_not_json_reach_gemini_as_an_object() {
    use kaeso_kpp::{BlockDelta, BlockHeader, BlockStart, BlockStop, Delta, ToolCallHeader};
    let events = vec![
        StreamEvent::BlockStart(BlockStart::new(
            0,
            BlockHeader::ToolCall(ToolCallHeader::new("c", "f")),
        )),
        StreamEvent::BlockDelta(BlockDelta::new(0, Delta::ToolArguments("{\"cut".into()))),
        StreamEvent::BlockStop(BlockStop::new(0)),
        StreamEvent::BlockStart(BlockStart::new(
            1,
            BlockHeader::ToolCall(ToolCallHeader::new("d", "g")),
        )),
        StreamEvent::BlockStop(BlockStop::new(1)),
    ];
    let out = String::from_utf8(replay(&events)).unwrap();
    assert!(out.contains(r#""args":{"input":"{\"cut"}"#), "{out}");
    assert!(out.contains(r#""args":{}"#), "{out}");
}

#[test]
fn codec_entry_points_without_a_path() {
    let count = Gemini::decode_request(
        RequestKind::CountTokens,
        br#"{"contents": []}"#,
        &HeaderList::new(),
    )
    .unwrap();
    assert!(matches!(count, Request::CountTokens(_)));
    let e = Gemini::decode_request(
        RequestKind::Embed,
        br#"{"content": {}}"#,
        &HeaderList::new(),
    )
    .unwrap_err();
    assert!(e.unsupported_feature().is_some());
    assert_eq!(Gemini::encode_model_request("").path, "/v1beta/models");
    let models = Gemini::decode_models(
        200,
        &HeaderList::new(),
        br#"{"models": [{"name": "tunedModels/x"}]}"#,
    )
    .unwrap();
    assert_eq!(models.models[0].id, "tunedModels/x");
    let back: Value = serde_json::from_slice(&Gemini::encode_models(&models).unwrap()).unwrap();
    assert_eq!(back, json!({"models": [{"name": "tunedModels/x"}]}));
    assert!(Gemini::decode_models(404, &HeaderList::new(), b"{}").is_err());
    assert!(Gemini::decode_models(200, &HeaderList::new(), b"[]").is_err());
    assert!(Gemini::decode_model(200, &HeaderList::new(), b"[]").is_err());
    let generic = crate::dispatch::encode_model_request_with(
        DialectId::OpenAiChat,
        "gpt-x",
        &EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off),
    )
    .unwrap();
    assert_eq!(generic.path, "/v1/models/gpt-x");
    let generic = crate::dispatch::encode_models_request_with(
        DialectId::OpenAiChat,
        &kaeso_kpp::ModelsQuery::default(),
        &EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off),
    )
    .unwrap();
    assert_eq!(generic.path, "/v1/models");
    assert!(
        crate::dispatch::encode_model_request_with(
            DialectId::Gemini,
            "..",
            &EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off)
        )
        .is_err()
    );
}

#[test]
fn shapes_over_the_limits_name_gemini_fields() {
    let parts: Vec<Value> = (0..=crate::limits::MAX_BLOCKS_PER_MESSAGE)
        .map(|_| json!({"text": "a"}))
        .collect();
    let body = serde_json::to_vec(&json!({"contents": [{"parts": parts}]})).unwrap();
    let e = crate::limits::request_shape(DialectId::Gemini, &body).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("contents"));
    let tools = serde_json::to_vec(&json!({"tools": [{"googleSearch": {}, "codeExecution": {}}, {"functionDeclarations": [{"name": "f"}]}], "generateContentRequest": {"contents": [{"parts": [{"text": "a"}]}]}})).unwrap();
    let shape = crate::limits::request_shape(DialectId::Gemini, &tools).unwrap();
    assert_eq!((shape.tools, shape.items, shape.blocks), (3, 1, 1));
    let requests: Vec<Value> = (0..=crate::limits::MAX_EMBEDDING_INPUTS)
        .map(|_| json!({"content": {"parts": [{"text": "a"}]}}))
        .collect();
    let body = serde_json::to_vec(&json!({"requests": requests})).unwrap();
    let e = crate::limits::request_shape(DialectId::GeminiEmbeddings, &body).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("requests"));
    let one = crate::limits::request_shape(
        DialectId::GeminiEmbeddings,
        br#"{"content": {"parts": [{"text": "a"}, {"text": "b"}]}}"#,
    )
    .unwrap();
    assert_eq!((one.items, one.blocks), (1, 2));
}
