#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use std::path::PathBuf;

use kaeso_kpp::{
    Block, BlockHeader, Delta, ErrorKind, Reply, Request, RequestKind, StopReason, StreamEvent,
    StreamMode,
};
use serde_json::{Value, json};

use super::eventstream;
use super::*;
use crate::anthropic_messages::AnthropicMessages;
use crate::openai_chat::OpenAiChat;

fn dir() -> PathBuf {
    crate::test_support::fixtures_dir().join("bedrock_converse")
}

fn chat(body: Value) -> Request {
    OpenAiChat::decode_request(
        RequestKind::Generate,
        body.to_string().as_bytes(),
        &HeaderList::new(),
    )
    .unwrap()
}

fn ctx(stream: bool) -> EgressCtx {
    let mode = if stream {
        StreamMode::on(true)
    } else {
        StreamMode::Off
    };
    EgressCtx::for_kind(RequestKind::Generate, mode).with_api_root("")
}

fn encode(req: &Request, stream: bool) -> (EncodedRequest, Value) {
    let mut c = ctx(stream);
    let e = BedrockConverse::encode_request(req, &mut c).unwrap();
    let body = serde_json::from_slice(&e.body).unwrap();
    (e, body)
}

fn refused(req: &Request) -> KppError {
    let mut c = ctx(false);
    let e = BedrockConverse::encode_request(req, &mut c).unwrap_err();
    assert_eq!(
        BedrockConverse::check(req, &c).unwrap_err().message,
        e.message
    );
    e
}

fn stream_bytes(name: &str) -> Vec<u8> {
    let json = std::fs::read(dir().join(format!("streams/{name}.events.json"))).unwrap();
    eventstream::from_fixture(&json).unwrap()
}

fn decode_stream(bytes: &[u8], chunk: usize) -> Vec<StreamEvent> {
    let mut d = BedrockConverse::stream_decoder(&ctx(true));
    let mut events = Vec::new();
    for c in bytes.chunks(chunk) {
        events.extend(d.feed(c));
    }
    events.extend(d.finish());
    events
}

fn response(name: &str) -> kaeso_kpp::Response {
    let body = std::fs::read(dir().join(format!("responses/{name}.json"))).unwrap();
    let mut headers = HeaderList::new();
    headers.push("x-amzn-requestid", "c6a1e8f0-1b2c-4d3e-8f90-0a1b2c3d4e5f");
    match BedrockConverse::decode_response(200, &headers, &body, &ctx(false)).unwrap() {
        Reply::Generate(r) => r,
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_openai_chat_request_becomes_a_converse_request() {
    let req = chat(json!({
        "model": "us.amazon.nova-pro-v1:0",
        "messages": [
            {"role": "system", "content": "You answer briefly."},
            {"role": "user", "content": "What is the most popular song on WZPZ?"}
        ],
        "max_tokens": 1000,
        "temperature": 0,
        "top_p": 0.9,
        "stop": ["END"],
        "tools": [{"type": "function", "function": {
            "name": "top_song",
            "description": "Get the most popular song played on a radio station.",
            "parameters": {"type": "object", "properties": {"sign": {"type": "string"}}, "required": ["sign"]}
        }}],
        "tool_choice": "required"
    }));
    let (e, body) = encode(&req, false);
    assert_eq!(e.path, "/model/us.amazon.nova-pro-v1%3A0/converse");
    assert_eq!(e.headers.get("content-type"), Some("application/json"));
    assert_eq!(
        body,
        json!({
            "system": [{"text": "You answer briefly."}],
            "messages": [{"role": "user", "content": [{"text": "What is the most popular song on WZPZ?"}]}],
            "inferenceConfig": {"maxTokens": 1000, "temperature": 0.0, "topP": 0.9, "stopSequences": ["END"]},
            "toolConfig": {
                "tools": [{"toolSpec": {
                    "name": "top_song",
                    "description": "Get the most popular song played on a radio station.",
                    "inputSchema": {"json": {"type": "object", "properties": {"sign": {"type": "string"}}, "required": ["sign"]}}
                }}],
                "toolChoice": {"any": {}}
            }
        })
    );
    let (e, _) = encode(&req, true);
    assert_eq!(e.path, "/model/us.amazon.nova-pro-v1%3A0/converse-stream");
    assert_eq!(
        e.headers.get("accept"),
        Some("application/vnd.amazon.eventstream")
    );
}

#[test]
fn history_with_tool_calls_media_and_cache_points_goes_in_converse_blocks() {
    let png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";
    let req = AnthropicMessages::decode_request(
        RequestKind::Generate,
        json!({
            "model": "amazon.nova-lite-v1:0",
            "max_tokens": 300,
            "system": [{"type": "text", "text": "Be helpful.", "cache_control": {"type": "ephemeral", "ttl": "1h"}}],
            "tools": [{"name": "top.song", "input_schema": {"type": "object"}}],
            "tool_choice": {"type": "tool", "name": "top.song"},
            "messages": [
                {"role": "user", "content": [
                    {"type": "text", "text": "Look:"},
                    {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": png}},
                    {"type": "document", "title": "Q3 report: final!", "source": {"type": "text", "media_type": "text/plain", "data": "Revenue rose."}}
                ]},
                {"role": "assistant", "content": [
                    {"type": "tool_use", "id": "toolu_01A", "name": "top.song", "input": {"sign": "WZPZ"}}
                ]},
                {"role": "user", "content": [
                    {"type": "tool_result", "tool_use_id": "toolu_01A", "is_error": true, "content": "Station unknown."}
                ]},
                {"role": "user", "content": "Try again."}
            ]
        })
        .to_string()
        .as_bytes(),
        &HeaderList::new(),
    )
    .unwrap();
    let mut c = ctx(false);
    let e = BedrockConverse::encode_request(&req, &mut c).unwrap();
    let body: Value = serde_json::from_slice(&e.body).unwrap();
    let name = body["toolConfig"]["tools"][0]["toolSpec"]["name"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(name.starts_with("top_song_") && name.len() <= 64, "{name}");
    assert_eq!(c.original_tool_name(&name), "top.song");
    assert_eq!(
        body,
        json!({
            "system": [{"text": "Be helpful."}, {"cachePoint": {"type": "default", "ttl": "1h"}}],
            "messages": [
                {"role": "user", "content": [
                    {"text": "Look:"},
                    {"image": {"format": "png", "source": {"bytes": png}}},
                    {"document": {"format": "txt", "name": "Q3 report- final-", "source": {"text": "Revenue rose."}}}
                ]},
                {"role": "assistant", "content": [
                    {"toolUse": {"toolUseId": "toolu_01A", "name": name, "input": {"sign": "WZPZ"}}}
                ]},
                {"role": "user", "content": [
                    {"toolResult": {"toolUseId": "toolu_01A", "content": [{"text": "Station unknown."}], "status": "error"}},
                    {"text": "Try again."}
                ]}
            ],
            "inferenceConfig": {"maxTokens": 300},
            "toolConfig": {
                "tools": [{"toolSpec": {"name": name, "inputSchema": {"json": {"type": "object"}}}}],
                "toolChoice": {"tool": {"name": name}}
            }
        })
    );
}

#[test]
fn what_converse_cant_carry_is_refused_before_sending() {
    let base = || {
        json!({
            "model": "amazon.nova-pro-v1:0",
            "messages": [{"role": "user", "content": "Hi"}]
        })
    };
    let with = |k: &str, v: Value| {
        let mut b = base();
        b[k] = v;
        chat(b)
    };
    for (req, feature) in [
        (with("seed", json!(1)), "seed"),
        (with("presence_penalty", json!(0.5)), "presence_penalty"),
        (
            with("response_format", json!({"type": "json_object"})),
            "structured output",
        ),
        (
            with("reasoning_effort", json!("high")),
            "reasoning settings",
        ),
        (
            with("parallel_tool_calls", json!(false)),
            "`parallel_tool_calls: false`",
        ),
        (
            chat(json!({
                "model": "m", "tool_choice": "none",
                "messages": [{"role": "user", "content": "Hi"}],
                "tools": [{"type": "function", "function": {"name": "f"}}]
            })),
            "`tool_choice: none`",
        ),
        (
            chat(
                json!({"model": "m", "messages": [{"role": "user", "content": [
                    {"type": "image_url", "image_url": {"url": "https://example.com/a.png"}}
                ]}]}),
            ),
            "media by link or file id",
        ),
        (
            chat(
                json!({"model": "m", "messages": [{"role": "user", "content": [
                    {"type": "image_url", "image_url": {"url": "data:image/bmp;base64,Qk0="}}
                ]}]}),
            ),
            "image type `image/bmp`",
        ),
        (
            chat(
                json!({"model": "m", "messages": [{"role": "user", "content": [
                    {"type": "input_audio", "input_audio": {"data": "UklGRg==", "format": "wav"}}
                ]}]}),
            ),
            "audio block",
        ),
    ] {
        let e = refused(&req);
        assert!(
            matches!(&e.kind, ErrorKind::Unsupported { feature: f, .. } if f == feature),
            "{feature}: {e:?}"
        );
    }
    let e = AnthropicMessages::decode_request(
        RequestKind::Generate,
        json!({"model": "m", "max_tokens": 5, "top_k": 3, "messages": [{"role": "user", "content": "Hi"}]})
            .to_string()
            .as_bytes(),
        &HeaderList::new(),
    )
    .map(|r| refused(&r))
    .unwrap();
    assert_eq!(
        e.message,
        "`top_k` is not supported by Amazon Bedrock Converse. Converse takes `maxTokens`, `temperature`, `topP` and stop sequences only; remove `top_k`."
    );
}

#[test]
fn a_reply_reads_its_text_tool_call_stop_and_usage() {
    let r = response("built-nova-tool-use");
    assert_eq!(r.id, "c6a1e8f0-1b2c-4d3e-8f90-0a1b2c3d4e5f");
    assert_eq!(r.stop, StopReason::ToolUse);
    assert_eq!((r.usage.input_tokens, r.usage.output_tokens), (372, 41));
    let Block::Text(t) = &r.blocks[0] else {
        panic!()
    };
    assert!(t.text.starts_with("<thinking>"));
    let Block::ToolCall(c) = &r.blocks[1] else {
        panic!()
    };
    assert_eq!(
        (c.id.as_str(), c.name.as_str(), c.arguments.get()),
        (
            "tooluse_20Z9zl0BQWSXjFuLKdTJcA",
            "top_song",
            r#"{"sign":"WZPZ"}"#
        )
    );

    let body = json!({
        "output": {"message": {"role": "assistant", "content": [
            {"reasoningContent": {"reasoningText": {"text": "Think.", "signature": "c2ln"}}},
            {"reasoningContent": {"redactedContent": "cmVk"}},
            {"citationsContent": {"content": [{"text": "x"}]}},
            {"text": "Done."}
        ]}},
        "stopReason": "max_tokens",
        "usage": {"inputTokens": 3, "outputTokens": 5, "totalTokens": 18, "cacheReadInputTokens": 6, "cacheWriteInputTokens": 4}
    });
    let Reply::Generate(r) = BedrockConverse::decode_response(
        200,
        &HeaderList::new(),
        body.to_string().as_bytes(),
        &ctx(false),
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(r.stop, StopReason::MaxTokens);
    assert_eq!(
        (r.usage.cache_read_tokens, r.usage.cache_write_tokens),
        (6, 4)
    );
    let Block::Reasoning(think) = &r.blocks[0] else {
        panic!()
    };
    assert_eq!(think.text, ["Think."]);
    assert_eq!(
        think
            .opaque_of(kaeso_kpp::OpaqueKind::BedrockReasoningSignature)
            .unwrap()
            .data,
        "c2ln"
    );
    let Block::Reasoning(redacted) = &r.blocks[1] else {
        panic!()
    };
    assert_eq!(
        redacted
            .opaque_of(kaeso_kpp::OpaqueKind::BedrockRedactedReasoning)
            .unwrap()
            .data,
        "cmVk"
    );
    assert!(matches!(r.blocks[2], Block::Unknown(_)));

    // Reasoning goes back to Converse as it came.
    let mut req = chat(json!({"model": "m", "messages": [{"role": "user", "content": "Hi"}]}));
    if let Request::Generate(g) = &mut req {
        let mut m = g.messages[0].clone();
        m.role = kaeso_kpp::Role::Assistant;
        m.blocks = r.blocks[..2].to_vec();
        g.messages.push(m);
    }
    let (_, sent) = encode(&req, false);
    assert_eq!(
        sent["messages"][1]["content"],
        json!([
            {"reasoningContent": {"reasoningText": {"text": "Think.", "signature": "c2ln"}}},
            {"reasoningContent": {"redactedContent": "cmVk"}}
        ])
    );

    for (stop, want) in [
        ("stop_sequence", StopReason::StopSequence(String::new())),
        ("guardrail_intervened", StopReason::ContentFilter),
        ("content_filtered", StopReason::ContentFilter),
        ("model_context_window_exceeded", StopReason::MaxTokens),
        (
            "malformed_tool_use",
            StopReason::Other("malformed_tool_use".into()),
        ),
    ] {
        assert_eq!(response::stop_reason(stop), want);
    }
    for bad in [
        &b"nope"[..],
        br#"{"output":{}}"#,
        br#"{"output":{"message":{"content":[{"toolUse":{"name":"x"}}]}}}"#,
    ] {
        let e = BedrockConverse::decode_response(200, &HeaderList::new(), bad, &ctx(false))
            .unwrap_err();
        assert_eq!(e.kind, ErrorKind::ProviderError, "{e:?}");
    }
}

#[test]
fn a_binary_stream_adds_up_to_the_reply_in_any_chunks() {
    let bytes = stream_bytes("built-nova-tool-use");
    let reply = response("built-nova-tool-use");
    for chunk in [1, 13, bytes.len()] {
        let events = decode_stream(&bytes, chunk);
        let mut texts: Vec<String> = Vec::new();
        let mut tool = (String::new(), String::new(), String::new());
        let mut stop = None;
        let mut usage = None;
        for e in &events {
            match e {
                StreamEvent::BlockStart(s) => match &s.block {
                    BlockHeader::Text => texts.push(String::new()),
                    BlockHeader::ToolCall(h) => {
                        tool = (h.id.clone(), h.name.clone(), String::new())
                    }
                    other => panic!("{other:?}"),
                },
                StreamEvent::BlockDelta(d) => match &d.delta {
                    Delta::Text(t) => texts.last_mut().unwrap().push_str(t),
                    Delta::ToolArguments(a) => tool.2.push_str(a),
                    other => panic!("{other:?}"),
                },
                StreamEvent::ResponseDelta(d) => {
                    stop = d.stop.clone();
                    usage = d.usage.clone();
                }
                _ => {}
            }
        }
        assert!(matches!(
            events.first(),
            Some(StreamEvent::ResponseStart(_))
        ));
        assert!(
            matches!(events.last(), Some(StreamEvent::ResponseStop(_))),
            "{events:?}"
        );
        let Block::Text(t) = &reply.blocks[0] else {
            panic!()
        };
        assert_eq!(texts, std::slice::from_ref(&t.text));
        let Block::ToolCall(c) = &reply.blocks[1] else {
            panic!()
        };
        assert_eq!(
            tool,
            (c.id.clone(), c.name.clone(), c.arguments.get().to_owned())
        );
        assert_eq!(stop, Some(reply.stop.clone()));
        assert_eq!(
            usage.map(|u| (u.input_tokens, u.output_tokens)),
            Some((372, 41))
        );
    }
}

#[test]
fn an_exception_frame_ends_the_stream_with_an_error() {
    let events = decode_stream(&stream_bytes("built-throttling-exception"), 7);
    let Some(StreamEvent::Error(e)) = events.last() else {
        panic!("{events:?}")
    };
    assert_eq!(
        (e.kind.clone(), e.status),
        (ErrorKind::RateLimited, Some(429))
    );
    assert_eq!(
        e.message,
        "Too many tokens, please wait before trying again."
    );

    let unmodeled = eventstream::encode(
        &[
            (":message-type", "error"),
            (":error-code", "InternalError"),
            (":error-message", "Broke."),
        ],
        b"",
    );
    let events = decode_stream(&unmodeled, 100);
    let [StreamEvent::Error(e)] = events.as_slice() else {
        panic!("{events:?}")
    };
    assert_eq!(
        (e.kind.clone(), e.message.as_str()),
        (ErrorKind::ProviderError, "Broke.")
    );

    let mut cut = stream_bytes("built-nova-text");
    cut.truncate(cut.len() - 3);
    let events = decode_stream(&cut, 100);
    let Some(StreamEvent::Error(e)) = events.last() else {
        panic!()
    };
    assert!(
        e.message.contains("in the middle of a message"),
        "{}",
        e.message
    );

    let early = eventstream::event("messageStart", br#"{"role":"assistant"}"#);
    let events = decode_stream(&early, 100);
    let Some(StreamEvent::Error(e)) = events.last() else {
        panic!()
    };
    assert_eq!(
        e.message,
        "Amazon Bedrock's Converse stream ended before the response was complete."
    );

    let mut corrupt = stream_bytes("built-nova-text");
    corrupt[20] ^= 0xff;
    let events = decode_stream(&corrupt, 100);
    assert!(
        matches!(events.as_slice(), [StreamEvent::Error(_)]),
        "{events:?}"
    );

    let not_json = eventstream::event("messageStart", b"{");
    assert!(matches!(
        decode_stream(&not_json, 100).as_slice(),
        [StreamEvent::Error(_)]
    ));
}

#[test]
fn a_stream_without_metadata_still_ends_and_reasoning_deltas_carry_their_signature() {
    let mut bytes = Vec::new();
    for (ty, data) in [
        ("messageStart", json!({"role": "assistant"})),
        (
            "contentBlockDelta",
            json!({"contentBlockIndex": 0, "delta": {"reasoningContent": {"text": "Hm."}}}),
        ),
        (
            "contentBlockDelta",
            json!({"contentBlockIndex": 0, "delta": {"reasoningContent": {"signature": "c2ln"}}}),
        ),
        (
            "contentBlockDelta",
            json!({"contentBlockIndex": 1, "delta": {"text": "Hi"}}),
        ),
        ("contentBlockDelta", json!({"contentBlockIndex": 2})),
        ("ping", json!({})),
        ("messageStop", json!({"stopReason": "end_turn"})),
    ] {
        bytes.extend(eventstream::event(ty, data.to_string().as_bytes()));
    }
    let events = decode_stream(&bytes, 9);
    let kinds: Vec<String> = events
        .iter()
        .map(|e| match e {
            StreamEvent::ResponseStart(_) => "start".into(),
            StreamEvent::BlockStart(s) => format!("open {}", s.index),
            StreamEvent::BlockDelta(d) => match &d.delta {
                Delta::ReasoningText(t) => format!("think {t}"),
                Delta::Opaque(o) => format!("sig {}", o.data),
                Delta::Text(t) => format!("text {t}"),
                other => format!("{other:?}"),
            },
            StreamEvent::BlockStop(s) => format!("close {}", s.index),
            StreamEvent::ResponseDelta(d) => format!("stop {:?}", d.stop),
            StreamEvent::ResponseStop(_) => "end".into(),
            other => format!("{other:?}"),
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "start",
            "open 0",
            "think Hm.",
            "sig c2ln",
            "open 1",
            "text Hi",
            "close 0",
            "close 1",
            "stop Some(EndTurn)",
            "end"
        ]
    );
    let mut d = BedrockConverse::stream_decoder(&ctx(true));
    assert!(d.flush().is_empty());
    assert!(d.feed(&bytes).is_empty() && d.finish().is_empty());
}

#[test]
fn errors_name_their_kind_from_the_fixtures() {
    for (name, kind) in [
        ("built-validation-too-long", ErrorKind::ContextTooLong),
        ("built-throttling", ErrorKind::RateLimited),
        ("built-access-denied", ErrorKind::PermissionDenied),
    ] {
        let body = std::fs::read(dir().join(format!("errors/{name}.json"))).unwrap();
        let meta: Value = serde_json::from_slice(
            &std::fs::read(dir().join(format!("errors/{name}.meta.json"))).unwrap(),
        )
        .unwrap();
        let mut headers = HeaderList::new();
        for (k, v) in meta["headers"].as_object().unwrap() {
            headers.push(k.clone(), v.as_str().unwrap());
        }
        let status = u16::try_from(meta["status"].as_u64().unwrap()).unwrap();
        let e = BedrockConverse::decode_response(status, &headers, &body, &ctx(false)).unwrap_err();
        assert_eq!((e.kind, e.status), (kind, Some(status)), "{name}");
    }
}

#[test]
fn no_app_sends_converse_and_only_generation_goes_out() {
    let e = BedrockConverse::decode_request(RequestKind::Generate, b"{}", &HeaderList::new())
        .unwrap_err();
    assert!(
        e.message.starts_with("Kaeso sends Amazon Bedrock Converse"),
        "{}",
        e.message
    );
    let reply = Reply::Generate(response("built-nova-text"));
    assert!(
        BedrockConverse::encode_response(
            &reply,
            &EncodeCtx::new(RequestKind::Generate, StreamMode::Off)
        )
        .is_err()
    );
    let _ =
        BedrockConverse::stream_encoder(&EncodeCtx::new(RequestKind::Generate, StreamMode::Off));
    let encoded = BedrockConverse::encode_error(&KppError::invalid_request("bad"));
    assert_eq!(encoded.status, 400);
    let embed = crate::openai_embeddings::OpenAiEmbeddings::decode_request(
        RequestKind::Embed,
        br#"{"model":"m","input":"x"}"#,
        &HeaderList::new(),
    )
    .unwrap();
    assert!(
        refused(&embed)
            .message
            .contains("serves message generation only")
    );
    assert_eq!(
        request::model_path(
            "arn:aws:bedrock:us-east-1:123456789012:inference-profile/us.amazon.nova-pro-v1:0",
            false
        ),
        "/model/arn%3Aaws%3Abedrock%3Aus-east-1%3A123456789012%3Ainference-profile%2Fus.amazon.nova-pro-v1%3A0/converse"
    );
}

fn anthropic(body: Value) -> Request {
    AnthropicMessages::decode_request(
        RequestKind::Generate,
        body.to_string().as_bytes(),
        &HeaderList::new(),
    )
    .unwrap()
}

#[test]
fn documents_tool_result_media_and_cache_lifetimes_take_converse_shapes() {
    let pdf = "JVBERi0xLjQK";
    let png = "iVBORw0KGgo=";
    let req = anthropic(json!({
        "model": "amazon.nova-pro-v1:0",
        "max_tokens": 10,
        "tools": [
            {"name": "look", "description": "", "input_schema": {"type": "object"}, "strict": true, "cache_control": {"type": "ephemeral"}}
        ],
        "tool_choice": {"type": "auto"},
        "messages": [
            {"role": "user", "content": [
                {"type": "document", "source": {"type": "base64", "media_type": "application/pdf", "data": pdf}, "cache_control": {"type": "ephemeral", "ttl": "5m"}},
                {"type": "image", "source": {"type": "base64", "media_type": "image/jpeg", "data": png}}
            ]},
            {"role": "assistant", "content": [{"type": "tool_use", "id": "call.1:x", "name": "look", "input": {}}]},
            {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "call.1:x", "content": [
                {"type": "text", "text": "seen"},
                {"type": "image", "source": {"type": "base64", "media_type": "image/webp", "data": png}},
                {"type": "document", "title": "  ", "source": {"type": "base64", "media_type": "text/csv", "data": "YSxi"}}
            ]}]}
        ]
    }));
    let (_, body) = encode(&req, false);
    assert_eq!(
        body["messages"][0]["content"],
        json!([
            {"document": {"format": "pdf", "name": "Document 1", "source": {"bytes": pdf}}},
            {"cachePoint": {"type": "default", "ttl": "5m"}},
            {"image": {"format": "jpeg", "source": {"bytes": png}}}
        ])
    );
    assert_eq!(
        body["messages"][1]["content"][0]["toolUse"]["toolUseId"],
        "call.1:x"
    );
    assert_eq!(
        body["messages"][2]["content"][0]["toolResult"]["content"],
        json!([
            {"text": "seen"},
            {"image": {"format": "webp", "source": {"bytes": png}}},
            {"document": {"format": "csv", "name": "Document 2", "source": {"bytes": "YSxi"}}}
        ])
    );
    assert_eq!(
        body["toolConfig"],
        json!({
            "tools": [
                {"toolSpec": {"name": "look", "inputSchema": {"json": {"type": "object"}}, "strict": true}},
                {"cachePoint": {"type": "default"}}
            ],
            "toolChoice": {"auto": {}}
        })
    );

    let chat_doc = chat(
        json!({"model": "m", "messages": [{"role": "user", "content": [
            {"type": "file", "file": {"file_data": "data:application/pdf;base64,JVBERi0xLjQK", "filename": "Q3 [draft] (v2).pdf"}},
            {"type": "image_url", "image_url": {"url": "data:image/gif;base64,R0lGODlh"}}
        ]}]}),
    );
    let (_, body) = encode(&chat_doc, false);
    let content = &body["messages"][0]["content"];
    assert_eq!(content[0]["document"]["format"], "pdf");
    assert_eq!(content[1]["image"]["format"], "gif");
    assert_eq!(
        super::request::model_path("a b", true),
        "/model/a%20b/converse-stream"
    );
}

#[test]
fn more_of_what_converse_cant_carry_is_refused() {
    let cases = [
        (
            anthropic(
                json!({"model": "m", "max_tokens": 5, "messages": [{"role": "user", "content": [
                    {"type": "document", "citations": {"enabled": true}, "source": {"type": "text", "media_type": "text/plain", "data": "x"}}
                ]}]}),
            ),
            "document citations",
        ),
        (
            anthropic(
                json!({"model": "m", "max_tokens": 5, "messages": [{"role": "user", "content": [
                    {"type": "document", "source": {"type": "base64", "media_type": "application/zip", "data": "UEs="}}
                ]}]}),
            ),
            "document type `application/zip`",
        ),
        (
            anthropic(
                json!({"model": "m", "max_tokens": 5, "messages": [{"role": "user", "content": [
                    {"type": "text", "text": "x", "cache_control": {"type": "ephemeral", "ttl": "2h"}}
                ]}]}),
            ),
            "cache time `2h`",
        ),
        (
            anthropic(json!({"model": "m", "max_tokens": 5,
                "system": [{"type": "text", "text": "s", "cache_control": {"type": "ephemeral", "ttl": "2h"}}],
                "messages": [{"role": "user", "content": "x"}]})),
            "cache time `2h`",
        ),
        (
            anthropic(json!({"model": "m", "max_tokens": 5,
                "tools": [{"name": "t", "input_schema": {"type": "object"}, "cache_control": {"type": "ephemeral", "ttl": "2h"}}],
                "messages": [{"role": "user", "content": "x"}]})),
            "cache time `2h`",
        ),
        (
            anthropic(json!({"model": "m", "max_tokens": 5,
                "tools": [{"type": "web_search_20250305", "name": "web_search"}],
                "messages": [{"role": "user", "content": "x"}]})),
            "web_search_20250305 tool",
        ),
        (
            chat(
                json!({"model": "m", "messages": [{"role": "user", "content": "x"}],
                "tools": [{"type": "function", "function": {"name": "f", "parameters": {"type": "array"}}}]}),
            ),
            "tool parameters that are not a JSON object schema",
        ),
        (
            chat(
                json!({"model": "m", "messages": [{"role": "user", "content": [
                    {"type": "file", "file": {"file_id": "file-abc"}}
                ]}]}),
            ),
            "media by link or file id",
        ),
    ];
    for (req, feature) in cases {
        let e = refused(&req);
        assert!(
            matches!(&e.kind, ErrorKind::Unsupported { feature: f, .. } if f == feature),
            "{feature}: {e:?}"
        );
    }
}
