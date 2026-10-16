use std::error::Error;

use criterion::{BenchmarkId, Criterion, Throughput};
use kaeso_codecs::{EgressCtx, EncodeCtx, HeaderList, dispatch};
use kaeso_kpp::{
    Block, BlockDelta, BlockHeader, BlockStart, BlockStop, Delta, DialectId, FunctionTool,
    GenerateRequest, KppError, Message, RawJson, Request, RequestKind, ResponseDelta,
    ResponseStart, ResponseStop, Role, StopReason, StreamEvent, StreamMode, ToolDef, Usage,
};

const GENERATION: [DialectId; 3] = [
    DialectId::AnthropicMessages,
    DialectId::OpenAiChat,
    DialectId::OpenAiResponses,
];

const LIVE: StreamMode = StreamMode::On {
    include_usage: true,
};

fn conversation() -> Result<Request, Box<dyn Error>> {
    let schema = RawJson::from_string(
        r#"{"type":"object","properties":{"city":{"type":"string"}},"required":["city"]}"#.into(),
    )?;
    let mut tool = FunctionTool::new("get_weather", Some(schema));
    tool.description = Some("Current weather for a city".into());
    let mut req = GenerateRequest::new("bench-model")
        .with_system("You are a helpful assistant. Answer briefly and precisely.")
        .with_tool(ToolDef::Function(tool));
    req.sampling.max_output_tokens = Some(1024);
    let paragraph = "The quick brown fox jumps over the lazy dog. ".repeat(8);
    for i in 0..25 {
        req.messages
            .push(Message::user(format!("Question {i}: {paragraph}")));
        let args = RawJson::from_string(format!(r#"{{"city":"City {i}"}}"#))?;
        req.messages.push(Message::new(
            Role::Assistant,
            vec![
                Block::text(format!("Let me check {i}.")),
                Block::tool_call(format!("call_{i:04}"), "get_weather", args),
            ],
        ));
        req.messages.push(Message::new(
            Role::User,
            vec![Block::tool_result(
                format!("call_{i:04}"),
                vec![Block::text(format!("Sunny, {i} degrees"))],
                false,
            )],
        ));
        req.messages
            .push(Message::assistant(format!("Answer {i}: {paragraph}")));
    }
    Ok(Request::Generate(req))
}

fn stream_events() -> Vec<StreamEvent> {
    let mut start = ResponseStart::new("bench-1", "bench-model");
    start.usage = Some(Usage::new(100, 0));
    let mut events = vec![
        StreamEvent::ResponseStart(start),
        StreamEvent::BlockStart(BlockStart::new(0, BlockHeader::Text)),
    ];
    for i in 0..1994 {
        events.push(StreamEvent::BlockDelta(BlockDelta::new(
            0,
            Delta::Text(format!("token{i} ")),
        )));
    }
    events.push(StreamEvent::BlockStop(BlockStop::new(0)));
    events.push(StreamEvent::ResponseDelta(ResponseDelta::new(
        Some(StopReason::EndTurn),
        Some(Usage::new(100, 1994)),
    )));
    events.push(StreamEvent::ResponseStop(ResponseStop::new()));
    events
}

fn request_cycle(dialect: DialectId, body: &[u8]) -> Result<Vec<u8>, KppError> {
    let decoded =
        dispatch::decode_request(dialect, RequestKind::Generate, body, &HeaderList::new())?;
    let mut ctx = EgressCtx::new(&decoded);
    Ok(dispatch::encode_request(dialect, &decoded, &mut ctx)?.body)
}

fn stream_cycle(dialect: DialectId, sse: &[u8], enc_ctx: &EncodeCtx) -> Result<usize, KppError> {
    let ctx = EgressCtx::for_kind(RequestKind::Generate, LIVE);
    let mut dec = dispatch::stream_decoder(dialect, &ctx)?;
    let mut enc = dispatch::stream_encoder(dialect, enc_ctx)?;
    let mut out = 0usize;
    for chunk in sse.chunks(512) {
        for ev in dec.feed(chunk) {
            out += enc.encode(&ev)?.len();
        }
    }
    for ev in dec.finish() {
        out += enc.encode(&ev)?.len();
    }
    Ok(out + enc.finish()?.len())
}

fn encoded_stream(dialect: DialectId, enc_ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
    let mut enc = dispatch::stream_encoder(dialect, enc_ctx)?;
    let mut sse = Vec::new();
    for ev in &stream_events() {
        sse.extend(enc.encode(ev)?);
    }
    sse.extend(enc.finish()?);
    Ok(sse)
}

fn bench_requests(c: &mut Criterion) -> Result<(), Box<dyn Error>> {
    let req = conversation()?;
    let mut group = c.benchmark_group("request_100_messages");
    for dialect in GENERATION {
        let mut ctx = EgressCtx::new(&req);
        let body = dispatch::encode_request(dialect, &req, &mut ctx)?.body;
        group.throughput(Throughput::Bytes(body.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(dialect.as_str()),
            &body,
            |b, body| b.iter(|| request_cycle(dialect, body)),
        );
    }
    group.finish();
    Ok(())
}

fn bench_streams(c: &mut Criterion) -> Result<(), Box<dyn Error>> {
    let mut group = c.benchmark_group("stream_2000_events");
    for dialect in GENERATION {
        let enc_ctx = EncodeCtx::new(RequestKind::Generate, LIVE).with_created_at(1);
        let sse = encoded_stream(dialect, &enc_ctx)?;
        group.throughput(Throughput::Bytes(sse.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(dialect.as_str()),
            &sse,
            |b, sse| b.iter(|| stream_cycle(dialect, sse, &enc_ctx)),
        );
    }
    group.finish();
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut criterion = Criterion::default().configure_from_args();
    bench_requests(&mut criterion)?;
    bench_streams(&mut criterion)?;
    criterion.final_summary();
    Ok(())
}
