use kaeso_kpp::{
    Block, DialectId, DocumentBlock, ErrorKind, GenerateRequest, ImageBlock, KppError, Message,
    Origin, Request, RequestKind, Role, Source, StreamEvent, StreamMode,
};
use serde_json::{Value, json};

use super::OpenAiChat;
use crate::codec::{Codec, EgressCtx, HeaderList};

fn decode_request(body: &Value) -> Result<Request, KppError> {
    let bytes = serde_json::to_vec(body).unwrap();
    OpenAiChat::decode_request(RequestKind::Generate, &bytes, &HeaderList::new())
}

fn decode_response(body: &Value) -> KppError {
    let bytes = serde_json::to_vec(body).unwrap();
    let ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off);
    OpenAiChat::decode_response(200, &HeaderList::new(), &bytes, &ctx).unwrap_err()
}

fn stream_errors(sse: &str) -> Vec<KppError> {
    let ctx = EgressCtx::for_kind(
        RequestKind::Generate,
        StreamMode::On {
            include_usage: true,
        },
    );
    let mut decoder = OpenAiChat::stream_decoder(&ctx);
    let mut events = decoder.feed(sse.as_bytes());
    events.extend(decoder.finish());
    events
        .into_iter()
        .filter_map(|e| match e {
            StreamEvent::Error(err) => Some(err),
            _ => None,
        })
        .collect()
}

fn chunk(choices: &Value) -> String {
    let body = json!({
        "id": "chatcmpl-1",
        "object": "chat.completion.chunk",
        "created": 1,
        "model": "gpt-5.6-sol",
        "choices": choices,
    });
    format!("data: {body}\n\n")
}

#[test]
fn malformed_requests_are_invalid_requests() {
    let cases = [
        json!({"model": "m", "messages": [{"role": "user", "content": [{"type": 42, "text": "x"}]}]}),
        json!({"model": "m", "messages": [{"role": "assistant", "content": []}]}),
    ];
    for body in cases {
        let err = decode_request(&body).unwrap_err();
        assert_eq!(err.kind, ErrorKind::InvalidRequest, "{body}: {err:?}");
    }
}

#[test]
fn responses_without_a_choice_or_a_finish_reason_are_provider_errors() {
    let base = json!({"id": "chatcmpl-1", "object": "chat.completion", "created": 1, "model": "m"});
    let mut no_choice = base.clone();
    no_choice["choices"] = json!([]);
    let mut no_finish = base.clone();
    no_finish["choices"] = json!([{"index": 0, "message": {"role": "assistant", "content": "Hi"}}]);
    let mut bad_usage = base;
    bad_usage["choices"] = json!([{"index": 0, "message": {"role": "assistant", "content": "Hi"}, "finish_reason": "stop"}]);
    bad_usage["usage"] = json!({"prompt_tokens": "many"});
    for body in [no_choice, no_finish, bad_usage] {
        let err = decode_response(&body);
        assert_eq!(err.kind, ErrorKind::ProviderError, "{body}: {err:?}");
    }
}

#[test]
fn streams_that_cannot_be_read_end_with_one_error() {
    let unreadable = "data: not json\n\n";
    let second_choice = chunk(&json!([{"index": 1, "delta": {"content": "Hi"}}]));
    let two_choices = chunk(&json!([
        {"index": 0, "delta": {"content": "Hi"}},
        {"index": 1, "delta": {"content": "Ho"}}
    ]));
    let bad_usage = format!(
        "{}data: {}\n\n",
        chunk(&json!([{"index": 0, "delta": {"role": "assistant", "content": ""}}])),
        json!({"id": "chatcmpl-1", "object": "chat.completion.chunk", "created": 1, "model": "m", "choices": [], "usage": {"prompt_tokens": "many"}})
    );
    let cases = [
        (unreadable.to_owned(), ErrorKind::ProviderError),
        (
            second_choice,
            ErrorKind::Unsupported {
                feature: "n".to_owned(),
            },
        ),
        (
            two_choices,
            ErrorKind::Unsupported {
                feature: "n".to_owned(),
            },
        ),
        (bad_usage, ErrorKind::ProviderError),
    ];
    for (sse, kind) in cases {
        let errors = stream_errors(&sse);
        assert_eq!(errors.len(), 1, "{sse}: {errors:?}");
        assert_eq!(errors[0].kind, kind, "{sse}");
    }
}

#[test]
fn sources_chat_cannot_carry_are_refused() {
    let anthropic = Origin::new(DialectId::AnthropicMessages);
    let image = Block::Image(ImageBlock::new(Source::text("text/plain", "not an image")));
    let document = Block::Document(DocumentBlock::new(Source::Url(
        "https://example.com/a.pdf".to_owned(),
    )));
    let file = Block::Document(DocumentBlock::new(Source::file_id(anthropic, "file_1")));
    for block in [image, document, file] {
        let req = Request::Generate(
            GenerateRequest::new("gpt-5.6-sol").with_message(Message::new(Role::User, vec![block])),
        );
        let mut ctx = EgressCtx::new(&req);
        let err = OpenAiChat::encode_request(&req, &mut ctx).unwrap_err();
        assert!(matches!(err.kind, ErrorKind::Unsupported { .. }), "{err:?}");
    }
}

#[test]
fn the_encoder_refuses_what_check_refuses() {
    use kaeso_kpp::{RawItem, RawJson, RefusalBlock, ToolChoice, ToolDef};
    let anthropic = Origin::new(DialectId::AnthropicMessages);
    let unknown = || {
        RawItem::new(
            anthropic.clone(),
            RawJson::from_string("{\"type\":\"x\"}".to_owned()).unwrap(),
        )
    };
    let image = || Block::Image(ImageBlock::new(Source::base64("image/png", "AA==")));
    let user = |blocks| Message::new(Role::User, blocks);
    let mut cases = Vec::new();
    let mut r = GenerateRequest::new("gpt-5.6-sol").with_message(user(vec![Block::text("Hi")]));
    r.tools.push(ToolDef::Unknown(unknown()));
    cases.push(r);
    let mut r = GenerateRequest::new("gpt-5.6-sol").with_message(user(vec![Block::text("Hi")]));
    r.tool_choice = Some(ToolChoice::Unknown(unknown()));
    cases.push(r);
    let mut r = GenerateRequest::new("gpt-5.6-sol").with_message(user(vec![Block::text("Hi")]));
    r.system = vec![image()];
    cases.push(r);
    cases.push(
        GenerateRequest::new("gpt-5.6-sol")
            .with_message(user(vec![Block::Refusal(RefusalBlock::new("No."))])),
    );
    cases.push(
        GenerateRequest::new("gpt-5.6-sol")
            .with_message(user(vec![Block::text("Hi")]))
            .with_message(Message::new(Role::Assistant, vec![image()])),
    );
    for generate in cases {
        let req = Request::Generate(generate.clone());
        assert!(super::check::check(&req).is_err(), "{generate:?}");
        let mut ctx = EgressCtx::new(&req);
        let err = super::request_encode::encode_generate(&generate, &mut ctx).unwrap_err();
        assert!(matches!(err.kind, ErrorKind::Unsupported { .. }), "{err:?}");
    }
}

fn assert_stream_round_trips(name: &str, sse: &str) {
    let ctx = EgressCtx::for_kind(
        RequestKind::Generate,
        StreamMode::On {
            include_usage: false,
        },
    );
    for size in [1, 7, usize::MAX] {
        let mut decoder = OpenAiChat::stream_decoder(&ctx);
        let mut events = Vec::new();
        for piece in sse.as_bytes().chunks(size.min(sse.len())) {
            events.extend(decoder.feed(piece));
        }
        events.extend(decoder.finish());
        let encode_ctx = crate::codec::EncodeCtx::new(
            RequestKind::Generate,
            StreamMode::On {
                include_usage: false,
            },
        );
        let mut encoder = OpenAiChat::stream_encoder(&encode_ctx);
        let mut out = Vec::new();
        for event in &events {
            out.extend(encoder.encode(event).unwrap());
        }
        out.extend(encoder.finish().unwrap());
        crate::test_support::assert_sse_eq(name, sse.as_bytes(), &out);
    }
}

#[test]
fn unusual_sse_framing_round_trips() {
    assert_stream_round_trips(
        "framing",
        r#"id: evt-1
retry: 3000
data: {"id":"chatcmpl-x9future","object":"chat.completion.chunk","created":1791650000,"model":"gpt-5.5-2026-04-23","system_fingerprint":"fp_future","choices":[{"index":0,"delta":{"role":"assistant","content":""},"logprobs":null,"finish_reason":null}]}

event: x_future_event
data: {"note":"made-up event"}

event: x_future_event
data: not json

x_future_line: value
data: {"id":"chatcmpl-x9future","object":"chat.completion.chunk","created":1791650000,"model":"gpt-5.5-2026-04-23","system_fingerprint":"fp_future","choices":[{"index":0,"delta":{"content":"Hi"},"logprobs":null,"finish_reason":null}]}

data: {"id":"chatcmpl-x9future","object":"chat.completion.chunk","created":1791650000,"model":"gpt-5.5-2026-04-23","system_fingerprint":"fp_future","choices":[{"index":0,"delta":{"role":"assistant","content":""},"logprobs":null,"finish_reason":null}]}

data: {"id":"chatcmpl-x9future","object":"chat.completion.chunk","created":1791650000,"model":"gpt-5.5-2026-04-23","system_fingerprint":"fp_future","choices":[{"index":0,"delta":{"content":" there"},"logprobs":null,"finish_reason":null}]}

data: {"id":"chatcmpl-x9future","object":"chat.completion.chunk","created":1791650000,"model":"gpt-5.5-2026-04-23","system_fingerprint":"fp_future","choices":[{"index":0,"delta":{},"logprobs":null,"finish_reason":"stop"}]}

data: [DONE]

data: {"x_future_after_done":true}

"#,
    );
}

#[test]
fn unusual_tool_call_chunks_round_trip() {
    assert_stream_round_trips(
        "tool calls",
        r#"data: {"id":"chatcmpl-x9future","object":"chat.completion.chunk","created":1791650000,"model":"gpt-5.5-2026-04-23","system_fingerprint":"fp_future","choices":[{"index":0,"delta":{"role":"assistant","content":null,"tool_calls":[{"index":2,"id":"call_a","type":"function","function":{"name":"get_weather","arguments":""}}]},"logprobs":null,"finish_reason":null}]}

data: {"id":"chatcmpl-x9future","object":"chat.completion.chunk","created":1791650000,"model":"gpt-5.5-2026-04-23","system_fingerprint":"fp_future","choices":[{"index":0,"delta":{"tool_calls":[{"index":2,"function":{"arguments":"{\"city\""}}]},"logprobs":null,"finish_reason":null}]}

data: {"id":"chatcmpl-x9future","object":"chat.completion.chunk","created":1791650000,"model":"gpt-5.5-2026-04-23","system_fingerprint":"fp_future","choices":[{"index":0,"delta":{"tool_calls":[{"index":2,"function":{"arguments":":\"Paris\"}","x_future_fn":1},"x_future_tc":1}]},"logprobs":null,"finish_reason":null}]}

data: {"id":"chatcmpl-x9future","object":"chat.completion.chunk","created":1791650000,"model":"gpt-5.5-2026-04-23","system_fingerprint":"fp_future","choices":[{"index":0,"delta":{"tool_calls":[{"index":5,"type":"x_future_tool","x_future_payload":{"a":1}}]},"logprobs":null,"finish_reason":null}]}

data: {"id":"chatcmpl-x9future","object":"chat.completion.chunk","created":1791650000,"model":"gpt-5.5-2026-04-23","system_fingerprint":"fp_future","choices":[{"index":0,"delta":{"tool_calls":[{"index":5,"x_future_payload":{"b":2}}]},"logprobs":null,"finish_reason":null}]}

data: {"id":"chatcmpl-x9future","object":"chat.completion.chunk","created":1791650000,"model":"gpt-5.5-2026-04-23","system_fingerprint":"fp_future","choices":[{"index":0,"delta":{"tool_calls":[{"index":7,"function":{"arguments":"{}"}}]},"logprobs":null,"finish_reason":null}]}

data: {"id":"chatcmpl-x9future","object":"chat.completion.chunk","created":1791650000,"model":"gpt-5.5-2026-04-23","system_fingerprint":"fp_future","choices":[{"index":0,"delta":{"tool_calls":[{"index":7,"id":"call_late","type":"function","function":{"name":"late_name"}}]},"logprobs":null,"finish_reason":null}]}

data: {"id":"chatcmpl-x9future","object":"chat.completion.chunk","created":1791650000,"model":"gpt-5.5-2026-04-23","system_fingerprint":"fp_future","choices":[{"index":0,"delta":{"tool_calls":[{"index":8}]},"logprobs":null,"finish_reason":null}]}

data: {"id":"chatcmpl-x9future","object":"chat.completion.chunk","created":1791650000,"model":"gpt-5.5-2026-04-23","system_fingerprint":"fp_future","choices":[{"index":0,"delta":{"tool_calls":[{"index":8,"id":"call_nofn"}]},"logprobs":null,"finish_reason":null}]}

data: {"id":"chatcmpl-x9future","object":"chat.completion.chunk","created":1791650000,"model":"gpt-5.5-2026-04-23","system_fingerprint":"fp_future","choices":[{"index":0,"delta":{},"logprobs":null,"finish_reason":"tool_calls"}]}

data: [DONE]

"#,
    );
}

#[test]
fn a_response_without_object_round_trips() {
    let body = json!({
        "id": "chatcmpl-2",
        "created": 1,
        "model": "gpt-5.6-sol",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "Hi"}, "logprobs": null, "finish_reason": "stop"}],
        "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
    });
    let bytes = serde_json::to_vec(&body).unwrap();
    let ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off);
    let reply = OpenAiChat::decode_response(200, &HeaderList::new(), &bytes, &ctx).unwrap();
    let encode_ctx = crate::codec::EncodeCtx::new(RequestKind::Generate, StreamMode::Off);
    let out = OpenAiChat::encode_response(&reply, &encode_ctx).unwrap();
    crate::test_support::assert_json_eq("no object", &bytes, &out);
}
