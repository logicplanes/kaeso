#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};

use kaeso_codecs::{EgressCtx, HeaderList, dispatch};
use kaeso_core::assembler::Assembler;
use kaeso_kpp::{Block, DialectId, RequestKind, Response, StreamEvent, StreamMode};
use serde_json::{Value, json};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../kaeso-codecs/fixtures")
}

fn provider(d: DialectId) -> &'static str {
    match d {
        DialectId::AnthropicMessages => "anthropic",
        _ => "openai",
    }
}

fn ctx(d: DialectId, stream: bool) -> EgressCtx {
    let mode = if stream {
        StreamMode::On {
            include_usage: true,
        }
    } else {
        StreamMode::Off
    };
    EgressCtx::for_kind(RequestKind::Generate, mode).with_provider(provider(d))
}

fn assemble(d: DialectId, sse: &[u8]) -> kaeso_core::assembler::Assembled {
    let mut decoder = dispatch::stream_decoder(d, &ctx(d, true)).unwrap();
    let mut asm = Assembler::new();
    let mut events: Vec<StreamEvent> = Vec::new();
    for chunk in sse.chunks(97) {
        events.extend(decoder.feed(chunk));
    }
    events.extend(decoder.finish());
    for e in &events {
        asm.feed(e);
    }
    asm.finish()
}

fn project(r: &Response) -> Value {
    let blocks: Vec<Value> = r
        .blocks
        .iter()
        .map(|b| match b {
            Block::Text(t) => json!({ "text": t.text, "citations": t.citations.len() }),
            Block::ToolCall(c) => json!({
                "tool": c.id, "name": c.name,
                "arguments": c.arguments.to_value().unwrap_or(Value::Null),
            }),
            Block::Reasoning(x) => json!({
                "reasoning": x.text.concat(),
                "summary": x.summary,
                "opaque": x.opaque.iter().map(|o| o.data.clone()).collect::<Vec<_>>(),
            }),
            Block::Refusal(x) => json!({ "refusal": x.text }),
            _ => json!("other"),
        })
        .collect();
    json!({
        "id": r.id,
        "model": r.model,
        "stop": serde_json::to_value(&r.stop).unwrap(),
        "usage": [r.usage.input_tokens, r.usage.output_tokens, r.usage.cache_read_tokens,
                  r.usage.cache_write_tokens, r.usage.reasoning_tokens],
        "blocks": blocks,
    })
}

#[test]
fn streams_assemble_to_their_paired_responses() {
    let root = fixtures();
    let mut compared = 0;
    let mut streams = 0;
    for d in [
        DialectId::AnthropicMessages,
        DialectId::OpenAiChat,
        DialectId::OpenAiResponses,
    ] {
        let dir = root.join(d.as_str());
        let Ok(entries) = std::fs::read_dir(dir.join("streams")) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("sse") {
                continue;
            }
            streams += 1;
            let name = path.file_stem().unwrap().to_string_lossy().into_owned();
            let sse = std::fs::read(&path).unwrap();
            let assembled = assemble(d, &sse);
            assert!(
                assembled.complete || assembled.error.is_some(),
                "{d}/{name}: neither complete nor failed"
            );
            let response = dir.join("responses").join(format!("{name}.json"));
            let Ok(body) = std::fs::read(&response) else {
                continue;
            };
            let Ok(kaeso_kpp::Reply::Generate(expected)) =
                dispatch::decode_response(d, 200, &HeaderList::new(), &body, &ctx(d, false))
            else {
                continue;
            };
            let got = assembled
                .response
                .unwrap_or_else(|| panic!("{d}/{name}: no response"));
            let (mut a, mut b) = (project(&got), project(&expected));
            if got.id != expected.id {
                mask_ids(&mut a);
                mask_ids(&mut b);
            }
            assert_eq!(a, b, "{d}/{name}");
            compared += 1;
        }
    }
    eprintln!("{streams} streams, {compared} compared with their responses");
    assert!(streams >= 30, "only {streams} stream fixtures found");
    assert!(compared >= 10, "only {compared} paired fixtures compared");
}

#[test]
fn a_stream_cut_off_keeps_what_arrived() {
    let root = fixtures();
    let sse = std::fs::read(root.join("anthropic_messages/streams/text-simple.sse")).unwrap();
    let half = &sse[..sse.len() / 2];
    let assembled = assemble(DialectId::AnthropicMessages, half);
    assert!(!assembled.complete);
    assert!(assembled.error.is_some());
    let r = assembled.response.unwrap();
    assert!(!r.id.is_empty());
    assert!(assembled_usage_is_known(&r));
}

fn mask_ids(v: &mut Value) {
    v["id"] = json!("*");
    for b in v["blocks"].as_array_mut().into_iter().flatten() {
        if b.get("tool").is_some() {
            b["tool"] = json!("*");
        }
    }
}

fn assembled_usage_is_known(r: &Response) -> bool {
    r.usage.input_tokens > 0
}
