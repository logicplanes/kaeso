#![cfg(test)]

use std::path::Path;

use kaeso_codecs::{EgressCtx, HeaderList, dispatch};
use kaeso_kpp::{DialectId, GenerateRequest, Message, Reply, Request, StreamEvent, Usage};

const A: DialectId = DialectId::AnthropicMessages;
const R: DialectId = DialectId::OpenAiResponses;

fn fixture(dialect: DialectId, path: &str) -> Vec<u8> {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(dialect.as_str())
        .join(path);
    std::fs::read(file).unwrap()
}

fn request() -> Request {
    Request::Generate(GenerateRequest::new("m").with_message(Message::user("hi")))
}

fn reply_usage(dialect: DialectId, path: &str) -> Usage {
    let req = request();
    let ctx = EgressCtx::new(&req);
    match dispatch::decode_response(
        dialect,
        200,
        &HeaderList::new(),
        &fixture(dialect, path),
        &ctx,
    ) {
        Ok(Reply::Generate(r)) => r.usage,
        other => panic!("{other:?}"),
    }
}

fn stream_usage(dialect: DialectId, path: &str) -> Usage {
    let req = request();
    let ctx = EgressCtx::new(&req);
    let mut decoder = dispatch::stream_decoder(dialect, &ctx).unwrap();
    let mut events = decoder.feed(&fixture(dialect, path));
    events.extend(decoder.finish());
    events
        .into_iter()
        .filter_map(|e| match e {
            StreamEvent::ResponseDelta(d) => d.usage,
            _ => None,
        })
        .last()
        .unwrap()
}

#[test]
fn the_searches_a_provider_bills_per_call_reach_the_usage() {
    assert_eq!(
        reply_usage(A, "responses/web-search-server-tool.json").web_searches,
        Some(1)
    );
    assert_eq!(
        stream_usage(A, "streams/web-search-server-tool.sse").web_searches,
        Some(1)
    );
    assert_eq!(
        reply_usage(R, "responses/recorded-web-search.json").web_searches,
        Some(1)
    );
    assert_eq!(
        stream_usage(R, "streams/recorded-web-search-stream.sse").web_searches,
        Some(1)
    );
    assert_eq!(
        reply_usage(A, "responses/cache-prewarm.json").web_searches,
        None
    );
}
