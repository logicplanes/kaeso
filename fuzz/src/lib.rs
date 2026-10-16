use kaeso_codecs::sse::{SseError, SseItem, SseParser, write_item};
use kaeso_codecs::{EgressCtx, EncodeCtx, HeaderList, dispatch};
use kaeso_kpp::{
    CountTokensResponse, DialectId, EmbedResponse, ModelList, Reply, Request, RequestKind,
    Response, StreamEvent, StreamMode,
};

const KINDS: [RequestKind; 3] = [
    RequestKind::Generate,
    RequestKind::CountTokens,
    RequestKind::Embed,
];

const LIVE: StreamMode = StreamMode::on(true);

const SSE_LIMIT: usize = 1 << 16;

const NESTING_STEP: usize = 32;

const TOOL_RESULT_OPEN: &str = r#"{"type":"tool_result","tool_use_id":"toolu_fuzz","content":["#;

fn salt(data: &[u8]) -> u8 {
    data.iter().fold(0, |acc, byte| acc.wrapping_add(*byte))
}

fn chunk_size(data: &[u8]) -> usize {
    usize::from(salt(data)) + 1
}

pub fn decoders(dialect: DialectId, data: &[u8]) {
    let salt = salt(data);
    let headers = HeaderList::new().with("retry-after", "3");
    for kind in KINDS {
        if let Ok(req) = dispatch::decode_request(dialect, kind, data, &headers) {
            request(dialect, &req);
        }
        let ctx = EgressCtx::for_kind(kind, StreamMode::Off);
        if let Ok(reply) = dispatch::decode_response(dialect, 200, &headers, data, &ctx) {
            response(&reply);
        }
    }
    let err = dispatch::decode_error(dialect, 400 + u16::from(salt), &headers, data);
    for target in DialectId::ALL {
        let _ = dispatch::encode_error(target, &err);
    }
    models(dialect, data, salt, &headers);
    stream(dialect, data);
    if dialect == DialectId::AnthropicMessages {
        nested(data, salt, &headers);
    }
}

fn nested(data: &[u8], salt: u8, headers: &HeaderList) {
    let Ok(inner) = std::str::from_utf8(data) else {
        return;
    };
    if !salt.is_multiple_of(8)
        || kaeso_kpp::serde_json::from_str::<&kaeso_kpp::serde_json::value::RawValue>(inner)
            .is_err()
    {
        return;
    }
    let levels = usize::from(salt) * NESTING_STEP;
    let mut blocks = TOOL_RESULT_OPEN.repeat(levels);
    blocks.push_str(inner);
    blocks.push_str(&"]}".repeat(levels));
    let request = format!(
        r#"{{"model":"m","max_tokens":1,"messages":[{{"role":"user","content":[{blocks}]}}]}}"#
    );
    if let Ok(req) = dispatch::decode_request(
        DialectId::AnthropicMessages,
        RequestKind::Generate,
        request.as_bytes(),
        headers,
    ) {
        self::request(DialectId::AnthropicMessages, &req);
    }
    let reply = format!(
        r#"{{"id":"msg_fuzz","type":"message","role":"assistant","model":"m","content":[{blocks}],"stop_reason":"end_turn","stop_sequence":null,"usage":{{"input_tokens":1,"output_tokens":1}}}}"#
    );
    let ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off);
    if let Ok(reply) = dispatch::decode_response(
        DialectId::AnthropicMessages,
        200,
        headers,
        reply.as_bytes(),
        &ctx,
    ) {
        response(&reply);
    }
}

fn request(dialect: DialectId, req: &Request) {
    if req.validate().is_err() {
        return;
    }
    if let Request::Generate(generate) | Request::CountTokens(generate) = req {
        let _ = generate.required_capabilities();
    }
    for target in DialectId::ALL {
        let mut ctx = EgressCtx::new(req);
        if dispatch::check(target, req, &ctx).is_err() {
            continue;
        }
        let _ = dispatch::encode_request(target, req, &mut ctx);
    }
    let _ = dispatch::encode_response(
        dialect,
        &empty_reply(req),
        &EncodeCtx::for_request(req.clone()),
    );
}

fn empty_reply(req: &Request) -> Reply {
    match req.kind() {
        RequestKind::Embed => Reply::Embed(EmbedResponse::new(req.model(), Vec::new())),
        RequestKind::CountTokens => Reply::CountTokens(CountTokensResponse::new(0)),
        _ => Reply::Generate(Response::new("resp_fuzz", req.model(), Vec::new())),
    }
}

fn response(reply: &Reply) {
    let ctx = EncodeCtx::new(reply.kind(), StreamMode::Off);
    for target in DialectId::ALL {
        let _ = dispatch::encode_response(target, reply, &ctx);
    }
}

fn other_family(dialect: DialectId) -> DialectId {
    if dialect == DialectId::AnthropicMessages {
        DialectId::OpenAiChat
    } else {
        DialectId::AnthropicMessages
    }
}

fn models(dialect: DialectId, data: &[u8], salt: u8, headers: &HeaderList) {
    let other = other_family(dialect);
    let status = if salt.is_multiple_of(2) {
        200
    } else {
        400 + u16::from(salt)
    };
    if let Ok(list) = dispatch::decode_models(dialect, status, headers, data) {
        let _ = dispatch::encode_models(dialect, &list);
        if let Ok(bytes) = dispatch::encode_models(other, &list)
            && let Ok(back) = dispatch::decode_models(other, 200, headers, &bytes)
        {
            let _ = dispatch::encode_models(dialect, &back);
        }
        let query = std::str::from_utf8(data)
            .ok()
            .and_then(|q| dispatch::decode_models_query(dialect, q).ok())
            .unwrap_or_default();
        let _ = ModelList::page(&list.models, &query, Some(u32::from(salt)));
    }
    if let Ok(model) = dispatch::decode_model(dialect, status, headers, data) {
        let _ = dispatch::encode_model(dialect, &model);
        if let Ok(bytes) = dispatch::encode_model(other, &model)
            && let Ok(back) = dispatch::decode_model(other, 200, headers, &bytes)
        {
            let _ = dispatch::encode_model(dialect, &back);
        }
    }
}

fn stream(dialect: DialectId, data: &[u8]) {
    let Ok(mut decoder) =
        dispatch::stream_decoder(dialect, &EgressCtx::for_kind(RequestKind::Generate, LIVE))
    else {
        return;
    };
    let mut events = Vec::new();
    for chunk in data.chunks(chunk_size(data)) {
        events.extend(decoder.feed(chunk));
    }
    events.extend(decoder.finish());
    for target in DialectId::ALL {
        reencode(target, &events);
    }
}

fn reencode(target: DialectId, events: &[StreamEvent]) {
    let Ok(mut encoder) =
        dispatch::stream_encoder(target, &EncodeCtx::new(RequestKind::Generate, LIVE))
    else {
        return;
    };
    for event in events {
        let _ = encoder.encode(event);
        let _ = encoder.keepalive();
    }
    let _ = encoder.finish();
}

fn parse<'a>(chunks: impl IntoIterator<Item = &'a [u8]>) -> Result<Vec<SseItem>, SseError> {
    let mut parser = SseParser::with_limit(SSE_LIMIT);
    let mut items = Vec::new();
    for chunk in chunks {
        parser.feed_into(chunk, &mut items)?;
    }
    items.extend(parser.finish()?);
    Ok(items)
}

pub fn sse(data: &[u8]) {
    let whole = parse([data]);
    let chunked = parse(data.chunks(chunk_size(data)));
    assert_eq!(whole, chunked);
    let bytewise = parse(data.chunks(1));
    assert_eq!(whole, bytewise);
    if let Ok(items) = whole {
        let mut written = Vec::new();
        for item in &items {
            write_item(&mut written, item);
        }
        assert_eq!(parse([written.as_slice()]), Ok(items));
    }
}
