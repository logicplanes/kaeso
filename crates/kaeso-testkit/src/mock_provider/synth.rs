use std::time::Duration;

use axum::body::Bytes;
use axum::http::HeaderMap;
use kaeso_codecs::{EncodeCtx, HeaderList, dispatch};
use kaeso_kpp::{DialectId, Reply, Request, RequestKind, StreamEvent};
use serde_json::json;

use super::marker;

const TOKENS_PER_DELTA: u64 = 4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Shape {
    pub(super) out: u64,
    pub(super) input: u64,
    pub(super) cached: u64,
    pub(super) reasoning: u64,
    pub(super) tps: f64,
}

impl Shape {
    pub(super) fn of(text: &str, body_len: usize) -> Option<Shape> {
        let num = |name| marker(text, name).and_then(|v| v.parse::<u64>().ok());
        let out = num("gen")?;
        let input = num("in").unwrap_or((body_len as u64 / 4).max(1));
        Some(Shape {
            out: out.max(1),
            input,
            cached: num("cached").unwrap_or(0).min(input),
            reasoning: num("reasoning").unwrap_or(0),
            tps: marker(text, "tps")
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|t| *t > 0.0)
                .unwrap_or(80.0),
        })
    }

    fn usage(&self, output: u64) -> serde_json::Value {
        json!({
            "input_tokens": self.input - self.cached,
            "cache_read_tokens": self.cached,
            "cache_write_tokens": 0,
            "output_tokens": output,
            "reasoning_tokens": self.reasoning,
        })
    }
}

const WORDS: &[&str] = &[
    "the",
    "order",
    "ships",
    "tomorrow",
    "and",
    "the",
    "refund",
    "is",
    "on",
    "its",
    "way",
    "to",
    "your",
    "account",
    "within",
    "three",
    "business",
    "days",
    "we",
    "checked",
    "the",
    "warehouse",
    "records",
    "for",
    "parcel",
    "4471",
    "which",
    "left",
    "the",
    "dock",
    "this",
    "morning",
    "so",
    "tracking",
    "updates",
    "should",
    "appear",
    "soon",
];

fn pieces(tokens: u64, seed: u64) -> Vec<String> {
    let mut state = seed | 1;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let n = tokens.div_ceil(TOKENS_PER_DELTA);
    (0..n)
        .map(|_| {
            let mut piece = String::new();
            while piece.len() < (TOKENS_PER_DELTA * 4) as usize {
                let i = usize::try_from(next() % WORDS.len() as u64).unwrap_or(0);
                piece.push_str(WORDS.get(i).copied().unwrap_or("the"));
                piece.push(' ');
            }
            piece
        })
        .collect()
}

fn header_list(headers: &HeaderMap) -> HeaderList {
    headers
        .iter()
        .filter_map(|(n, v)| {
            v.to_str()
                .ok()
                .map(|v| (n.as_str().to_owned(), v.to_owned()))
        })
        .collect()
}

fn decode(dialect: DialectId, body: &[u8], headers: &HeaderMap) -> Option<Request> {
    let req = dispatch::decode_request(dialect, RequestKind::Generate, body, &header_list(headers))
        .ok()?;
    matches!(req, Request::Generate(_)).then_some(req)
}

fn id_for(dialect: DialectId, seed: u64) -> String {
    match dialect {
        DialectId::AnthropicMessages => format!("msg_mock{seed:016x}"),
        DialectId::OpenAiResponses => format!("resp_mock{seed:016x}"),
        _ => format!("chatcmpl-mock{seed:016x}"),
    }
}

pub(super) fn complete(
    dialect: DialectId,
    body: &[u8],
    headers: &HeaderMap,
    shape: Shape,
    seed: u64,
) -> Option<Vec<u8>> {
    let req = decode(dialect, body, headers)?;
    let text: String = pieces(shape.out, seed).concat();
    let reply: Reply = serde_json::from_value(json!({
        "generate": {
            "id": id_for(dialect, seed),
            "model": req.model(),
            "blocks": [{ "text": { "text": text.trim_end() } }],
            "stop": "end_turn",
            "usage": shape.usage(shape.out + shape.reasoning),
        }
    }))
    .ok()?;
    dispatch::encode_response(dialect, &reply, &EncodeCtx::for_request(req)).ok()
}

pub(super) fn stream(
    dialect: DialectId,
    body: &[u8],
    headers: &HeaderMap,
    shape: Shape,
    seed: u64,
) -> Option<Vec<(Duration, Bytes)>> {
    let req = decode(dialect, body, headers)?;
    let mut encoder =
        dispatch::stream_encoder(dialect, &EncodeCtx::for_request(req.clone())).ok()?;
    let event = |v: serde_json::Value| serde_json::from_value::<StreamEvent>(v).ok();
    let pieces = pieces(shape.out, seed);
    let per_delta = Duration::from_secs_f64(TOKENS_PER_DELTA as f64 / shape.tps);
    let thinking = Duration::from_secs_f64(shape.reasoning as f64 / shape.tps);
    let mut events: Vec<(Duration, StreamEvent)> = vec![
        (
            Duration::ZERO,
            event(json!({ "response_start": {
                "id": id_for(dialect, seed),
                "model": req.model(),
                "usage": shape.usage(1),
            }}))?,
        ),
        (
            Duration::ZERO,
            event(json!({ "block_start": { "index": 0, "block": "text" } }))?,
        ),
    ];
    for (i, piece) in pieces.iter().enumerate() {
        let pause = if i == 0 { thinking } else { per_delta };
        events.push((
            pause,
            event(json!({ "block_delta": { "index": 0, "delta": { "text": piece } } }))?,
        ));
    }
    events.push((
        Duration::ZERO,
        event(json!({ "block_stop": { "index": 0 } }))?,
    ));
    events.push((
        Duration::ZERO,
        event(json!({ "response_delta": {
            "stop": "end_turn",
            "usage": shape.usage(shape.out + shape.reasoning),
        }}))?,
    ));
    events.push((Duration::ZERO, event(json!({ "response_stop": {} }))?));
    let mut chunks = Vec::with_capacity(events.len() + 1);
    let mut pause = Duration::ZERO;
    for (wait, e) in events {
        pause += wait;
        let bytes = encoder.encode(&e).ok()?;
        if !bytes.is_empty() {
            chunks.push((std::mem::take(&mut pause), Bytes::from(bytes)));
        }
    }
    let tail = encoder.finish().ok()?;
    if !tail.is_empty() {
        chunks.push((Duration::ZERO, Bytes::from(tail)));
    }
    Some(chunks)
}

#[cfg(test)]
mod tests {

    use super::*;
    use kaeso_codecs::EgressCtx;

    fn request(dialect: DialectId, stream: bool) -> Vec<u8> {
        let s = if stream { r#","stream":true"# } else { "" };
        match dialect {
            DialectId::AnthropicMessages => format!(
                r#"{{"model":"claude-haiku-4-5","max_tokens":1000,"messages":[{{"role":"user","content":"hi [mock:gen=120] [mock:cached=40] [mock:in=300] [mock:reasoning=30]"}}]{s}}}"#
            ),
            DialectId::OpenAiResponses => format!(
                r#"{{"model":"gpt-5.4-mini","input":"hi [mock:gen=120] [mock:cached=40] [mock:in=300] [mock:reasoning=30]"{s}}}"#
            ),
            _ => format!(
                r#"{{"model":"gpt-5.4-mini","messages":[{{"role":"user","content":"hi [mock:gen=120] [mock:cached=40] [mock:in=300] [mock:reasoning=30]"}}]{}}}"#,
                if stream {
                    r#","stream":true,"stream_options":{"include_usage":true}"#
                } else {
                    ""
                }
            ),
        }
        .into_bytes()
    }

    const DIALECTS: [DialectId; 3] = [
        DialectId::AnthropicMessages,
        DialectId::OpenAiChat,
        DialectId::OpenAiResponses,
    ];

    #[test]
    fn markers_shape_the_reply() {
        let s = Shape::of("x [mock:gen=50] [mock:tps=20]", 400).unwrap();
        assert_eq!((s.out, s.input, s.cached, s.reasoning), (50, 100, 0, 0));
        assert!((s.tps - 20.0).abs() < f64::EPSILON);
        assert_eq!(Shape::of("no markers", 400), None);
        let capped = Shape::of("[mock:gen=5] [mock:in=10] [mock:cached=99]", 0).unwrap();
        assert_eq!(capped.cached, 10);
    }

    #[test]
    fn complete_replies_decode_with_the_asked_usage() {
        for dialect in DIALECTS {
            let body = request(dialect, false);
            let shape = Shape::of(std::str::from_utf8(&body).unwrap(), body.len()).unwrap();
            let bytes = complete(dialect, &body, &HeaderMap::new(), shape, 7).expect("encoded");
            let req = decode(dialect, &body, &HeaderMap::new()).unwrap();
            let reply = dispatch::decode_response(
                dialect,
                200,
                &HeaderList::default(),
                &bytes,
                &EgressCtx::new(&req),
            )
            .unwrap_or_else(|e| panic!("{dialect:?}: {e}"));
            let Reply::Generate(r) = reply else {
                panic!("{dialect:?}: not a generation")
            };
            assert_eq!(r.usage.input_tokens, 260, "{dialect:?}");
            assert_eq!(r.usage.cache_read_tokens, 40, "{dialect:?}");
            assert_eq!(r.usage.output_tokens, 150, "{dialect:?}");
            assert_eq!(r.usage.reasoning_tokens, Some(30), "{dialect:?}");
        }
    }

    #[test]
    fn streams_decode_with_the_asked_usage_and_a_delta_per_four_tokens() {
        for dialect in DIALECTS {
            let body = request(dialect, true);
            let shape = Shape::of(std::str::from_utf8(&body).unwrap(), body.len()).unwrap();
            let chunks = stream(dialect, &body, &HeaderMap::new(), shape, 7).expect("encoded");
            let req = decode(dialect, &body, &HeaderMap::new()).unwrap();
            let mut decoder = dispatch::stream_decoder(dialect, &EgressCtx::new(&req)).unwrap();
            let mut events = Vec::new();
            for (_, c) in &chunks {
                events.extend(decoder.feed(c));
            }
            events.extend(decoder.finish());
            let deltas = events
                .iter()
                .filter(|e| matches!(e, StreamEvent::BlockDelta(_)))
                .count();
            let expected = if dialect == DialectId::OpenAiChat {
                31
            } else {
                30
            };
            assert_eq!(deltas, expected, "{dialect:?}");
            let usage = final_usage(&events);
            assert_eq!(usage.input_tokens, 260, "{dialect:?}");
            assert_eq!(usage.cache_read_tokens, 40, "{dialect:?}");
            assert_eq!(usage.output_tokens, 150, "{dialect:?}");
            let paced: Duration = chunks.iter().map(|(d, _)| *d).sum();
            assert!(
                paced >= Duration::from_millis(1800) && paced <= Duration::from_millis(1900),
                "{dialect:?}: {paced:?}"
            );
        }
    }

    fn final_usage(events: &[StreamEvent]) -> kaeso_kpp::Usage {
        events
            .iter()
            .rev()
            .find_map(|e| match e {
                StreamEvent::ResponseDelta(d) => d.usage.clone(),
                _ => None,
            })
            .expect("final usage")
    }
}
