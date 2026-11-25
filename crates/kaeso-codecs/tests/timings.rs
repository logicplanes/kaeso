#![allow(clippy::unwrap_used, clippy::print_stderr)]

use std::sync::Arc;
use std::time::Instant;

use kaeso_codecs::{EncodeCtx, HeaderList, dispatch};
use kaeso_kpp::{DialectId, Reply, RequestKind};
use serde_json::json;

fn long_conversation(turns: usize) -> Vec<u8> {
    let messages: Vec<_> = (0..turns)
        .map(|i| {
            let role = if i % 2 == 0 { "user" } else { "assistant" };
            json!({ "role": role, "content": format!("turn {i}: {}", "lorem ipsum ".repeat(80)) })
        })
        .collect();
    serde_json::to_vec(&json!({ "model": "gpt-4o", "messages": messages })).unwrap()
}

fn reply() -> Reply {
    let body = json!({
        "id": "chatcmpl-1", "object": "chat.completion", "created": 1, "model": "gpt-4o",
        "choices": [{ "index": 0, "finish_reason": "stop",
            "message": { "role": "assistant", "content": "Done." } }],
        "usage": { "prompt_tokens": 10, "completion_tokens": 2, "total_tokens": 12 }
    });
    let ctx = kaeso_codecs::EgressCtx::for_kind(RequestKind::Generate, kaeso_kpp::StreamMode::Off);
    dispatch::decode_response(
        DialectId::OpenAiChat,
        200,
        &HeaderList::new(),
        &serde_json::to_vec(&body).unwrap(),
        &ctx,
    )
    .unwrap()
}

#[test]
#[ignore = "timing, run with --ignored --nocapture"]
fn responses_reply_to_a_long_conversation() {
    for turns in [2, 2_000] {
        let body = long_conversation(turns);
        let req = dispatch::decode_request(
            DialectId::OpenAiChat,
            RequestKind::Generate,
            &body,
            &HeaderList::new(),
        )
        .unwrap();
        let ctx = EncodeCtx::for_request(Arc::new(req));
        let reply = reply();
        let rounds = 20;
        let started = Instant::now();
        for _ in 0..rounds {
            dispatch::encode_response(DialectId::OpenAiResponses, &reply, &ctx).unwrap();
        }
        let each = started.elapsed() / rounds;
        eprintln!(
            "a Responses reply to a {} KB request: {each:?} each",
            body.len() / 1024
        );
    }
}

fn fixture(path: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join(path),
    )
    .unwrap()
}

#[test]
#[ignore = "timing, run with --ignored --nocapture"]
fn streamed_events_through_each_encoder() {
    let sources = [
        (
            DialectId::AnthropicMessages,
            "anthropic_messages/streams/text-simple.sse",
        ),
        (
            DialectId::OpenAiChat,
            "openai_chat/streams/stream-include-usage.sse",
        ),
        (
            DialectId::OpenAiResponses,
            "openai_responses/streams/text-basic-stream.sse",
        ),
    ];
    for (from, path) in sources {
        let ectx = kaeso_codecs::EgressCtx::for_kind(
            RequestKind::Generate,
            kaeso_kpp::StreamMode::on(true),
        )
        .with_provider(if from == DialectId::AnthropicMessages {
            "anthropic"
        } else {
            "openai"
        });
        let mut decoder = dispatch::stream_decoder(from, &ectx).unwrap();
        let mut events = decoder.feed(&fixture(path));
        events.extend(decoder.finish());
        let deltas: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, kaeso_kpp::StreamEvent::BlockDelta(_)))
            .cloned()
            .collect();
        let (head, tail): (Vec<_>, Vec<_>) = events.iter().cloned().partition(|e| {
            matches!(
                e,
                kaeso_kpp::StreamEvent::ResponseStart(_) | kaeso_kpp::StreamEvent::BlockStart(_)
            )
        });
        let tail: Vec<_> = tail
            .into_iter()
            .filter(|e| !matches!(e, kaeso_kpp::StreamEvent::BlockDelta(_)))
            .collect();
        for to in [
            DialectId::AnthropicMessages,
            DialectId::OpenAiChat,
            DialectId::OpenAiResponses,
        ] {
            let ctx = EncodeCtx::new(RequestKind::Generate, kaeso_kpp::StreamMode::on(true));
            let mut encoder = dispatch::stream_encoder(to, &ctx).unwrap();
            for e in &head {
                encoder.encode(e).unwrap();
            }
            let rounds = 20_000 / deltas.len().max(1);
            let started = Instant::now();
            let mut bytes = 0;
            for _ in 0..rounds {
                for e in &deltas {
                    bytes += encoder.encode(e).unwrap().len();
                }
            }
            let each = started.elapsed() / u32::try_from(rounds * deltas.len()).unwrap();
            for e in &tail {
                encoder.encode(e).unwrap();
            }
            eprintln!("{from} -> {to}: {each:?} per text delta ({bytes} bytes)");
        }
    }
}

// A coding agent's conversation as Codex sends it on its 600th turn: instructions, tools, and
// for every turn a reasoning item, a function call and its output.
fn codex_conversation(turns: usize) -> Vec<u8> {
    let tools: Vec<_> = (0..20)
        .map(|i| {
            json!({
                "type": "function",
                "name": format!("tool_{i:02}"),
                "description": format!("Tool {i} of the workspace. {}", "It reads or changes files and says what happened. ".repeat(6)),
                "strict": false,
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "The absolute path." },
                        "limit": { "type": "integer", "minimum": 1, "maximum": 2000 }
                    },
                    "required": ["path"],
                    "additionalProperties": false
                }
            })
        })
        .collect();
    let mut input = vec![
        json!({ "type": "message", "role": "developer", "content": [{ "type": "input_text", "text": "Follow the repository's rules." }] }),
        json!({ "type": "message", "role": "user", "content": [{ "type": "input_text", "text": "Find out why the export is slow and fix it." }] }),
    ];
    for i in 0..turns {
        input.push(json!({
            "type": "reasoning",
            "id": format!("rs_{i:06}"),
            "summary": [{ "type": "summary_text", "text": format!("Step {i}: read the next stage before changing it.") }],
            "content": null,
            "encrypted_content": format!("gAAAAAB{}", "x".repeat(2400)),
        }));
        if i % 5 == 0 {
            input.push(json!({
                "type": "message",
                "status": "completed",
                "id": format!("msg_{i:06}"),
                "role": "assistant",
                "content": [{ "type": "output_text", "text": format!("Stage {i} reads its rows one batch at a time; I'll look at the next one."), "annotations": [] }],
            }));
        }
        input.push(json!({
            "type": "function_call",
            "id": format!("fc_{i:06}"),
            "name": format!("tool_{:02}", i % 20),
            "arguments": format!(r#"{{"path":"/workspace/export/stage_{i}.rs","limit":200}}"#),
            "call_id": format!("call_{i:06}"),
        }));
        input.push(json!({
            "type": "function_call_output",
            "call_id": format!("call_{i:06}"),
            "output": (0..36).map(|l| format!("{l:>4}  let rows = export.stage({i}).read_batch(cursor, {l})?;\n")).collect::<String>(),
        }));
    }
    serde_json::to_vec(&json!({
        "model": "gpt-5.1-codex",
        "max_output_tokens": 32000,
        "instructions": "You are Codex, a coding agent. ".repeat(500),
        "input": input,
        "tools": tools,
        "tool_choice": "auto",
        "parallel_tool_calls": false,
        "reasoning": { "effort": "medium", "summary": "auto" },
        "store": false,
        "stream": true,
        "include": ["reasoning.encrypted_content"],
        "prompt_cache_key": "019a0e5b-7c01-7000-8000-000000000001",
    }))
    .unwrap()
}

fn time_decode(dialect: DialectId, body: &[u8], rounds: u32) -> std::time::Duration {
    let mut best = std::time::Duration::MAX;
    for _ in 0..rounds {
        let started = Instant::now();
        dispatch::decode_request(dialect, RequestKind::Generate, body, &HeaderList::new()).unwrap();
        best = best.min(started.elapsed());
    }
    best
}

#[test]
#[ignore = "timing, run with --release --ignored --nocapture"]
fn a_long_responses_conversation_decodes_about_as_fast_as_the_same_messages() {
    let responses = codex_conversation(600);
    let req = dispatch::decode_request(
        DialectId::OpenAiResponses,
        RequestKind::Generate,
        &responses,
        &HeaderList::new(),
    )
    .unwrap();
    let mut ctx = kaeso_codecs::EgressCtx::new(&req).with_provider("anthropic");
    let messages = dispatch::encode_request(DialectId::AnthropicMessages, &req, &mut ctx)
        .unwrap()
        .body;
    let checked = {
        let started = Instant::now();
        kaeso_codecs::limits::Checked::new(DialectId::OpenAiResponses, responses.as_slice())
            .unwrap();
        started.elapsed()
    };
    let r = time_decode(DialectId::OpenAiResponses, &responses, 60);
    let m = time_decode(DialectId::AnthropicMessages, &messages, 60);
    let mb = |b: &[u8]| b.len() as f64 / 1_000_000.0;
    eprintln!(
        "Responses {:.2} MB: decode {r:?} ({:.1} ms/MB), checked {checked:?}; Messages {:.2} MB: decode {m:?} ({:.1} ms/MB); ratio {:.2}",
        mb(&responses),
        r.as_secs_f64() * 1000.0 / mb(&responses),
        mb(&messages),
        m.as_secs_f64() * 1000.0 / mb(&messages),
        r.as_secs_f64() / m.as_secs_f64(),
    );
}

fn timed_speech(chunks: &[&[u8]]) -> std::time::Duration {
    let ctx =
        kaeso_codecs::EgressCtx::for_kind(RequestKind::Speech, kaeso_kpp::StreamMode::on(true));
    let mut decoder = dispatch::stream_decoder(DialectId::ElevenLabs, &ctx).unwrap();
    let started = Instant::now();
    for chunk in chunks {
        decoder.feed(chunk);
    }
    decoder.finish();
    started.elapsed()
}

#[test]
#[ignore = "timing, run with --release --ignored --nocapture"]
fn elevenlabs_speech_lines_read_in_time_linear_in_their_bytes() {
    let newlines = |lines: usize| timed_speech(&[&vec![b'\n'; lines]]);
    let (few, many) = (newlines(50_000), newlines(400_000));
    eprintln!("empty lines in one chunk: 50,000 in {few:?}, 400,000 in {many:?}");
    assert!(
        many < few * 40,
        "eight times the lines took {many:?}, not {few:?} times eight"
    );

    let unbroken = |megabytes: usize| {
        let line = vec![b'a'; megabytes << 20];
        timed_speech(&line.chunks(16 * 1024).collect::<Vec<_>>())
    };
    let (short, long) = (unbroken(4), unbroken(32));
    eprintln!("a line in 16 KiB chunks: 4 MiB in {short:?}, 32 MiB in {long:?}");
    assert!(
        long < short * 40,
        "eight times the bytes took {long:?}, not {short:?} times eight"
    );
}
