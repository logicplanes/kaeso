use kaeso_codecs::sse::{SseError, SseItem, SseParser};
use kaeso_codecs::{EgressCtx, EncodeCtx, HeaderList, dispatch};
use kaeso_kpp::{DialectId, ModelList, RequestKind, StreamMode};

const KINDS: [RequestKind; 3] = [
    RequestKind::Generate,
    RequestKind::CountTokens,
    RequestKind::Embed,
];

const LIVE: StreamMode = StreamMode::On {
    include_usage: true,
};

const SSE_LIMIT: usize = 1 << 16;

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
        let _ = dispatch::decode_request(dialect, kind, data, &headers);
        let ctx = EgressCtx::for_kind(kind, StreamMode::Off);
        let _ = dispatch::decode_response(dialect, 200, &headers, data, &ctx);
    }
    let _ = dispatch::decode_error(dialect, 400 + u16::from(salt), &headers, data);
    models(dialect, data, salt, &headers);
    stream(dialect, data);
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
    if let Ok(mut encoder) =
        dispatch::stream_encoder(dialect, &EncodeCtx::new(RequestKind::Generate, LIVE))
    {
        for event in &events {
            let _ = encoder.encode(event);
        }
        let _ = encoder.finish();
    }
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
    if let (Ok(whole), Ok(chunked)) = (&whole, &chunked) {
        assert_eq!(whole, chunked);
    }
}
