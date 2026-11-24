#![allow(clippy::unwrap_used)]

use std::time::{Duration, Instant};

use kaeso_codecs::gemini::GeminiAction;
use kaeso_codecs::{HeaderList, dispatch};
use kaeso_kpp::{Block, Request};
use serde_json::{Value, json};

const PARTS_PER_TURN: usize = 5_000;

fn turns(role: &str, parts: Vec<Value>) -> Vec<Value> {
    parts
        .chunks(PARTS_PER_TURN)
        .map(|parts| json!({ "role": role, "parts": parts }))
        .collect()
}

fn answered_newest_first(calls: usize) -> Vec<u8> {
    let asked = (0..calls)
        .map(|i| json!({ "functionCall": { "name": "lookup", "id": i.to_string() } }))
        .collect();
    let answered = (0..calls)
        .rev()
        .map(|i| {
            json!({ "functionResponse": {
                "name": "lookup", "id": i.to_string(), "response": { "output": "ok" }
            } })
        })
        .collect();
    let mut contents = turns("model", asked);
    contents.extend(turns("user", answered));
    serde_json::to_vec(&json!({ "contents": contents })).unwrap()
}

fn decode(body: &[u8]) -> Request {
    dispatch::decode_gemini_request(
        "models/gemini-2.5-flash",
        GeminiAction::GenerateContent,
        body,
        &HeaderList::new(),
    )
    .unwrap()
}

fn fastest_decode(body: &[u8]) -> Duration {
    (0..3)
        .map(|_| {
            let started = Instant::now();
            decode(body);
            started.elapsed()
        })
        .min()
        .unwrap()
}

#[test]
#[ignore = "timing, run with --release --ignored"]
fn decoding_grows_linearly_with_the_function_calls_of_a_conversation() {
    let small = fastest_decode(&answered_newest_first(5_000));
    let large = fastest_decode(&answered_newest_first(40_000));
    assert!(
        large < small * 24,
        "8 times the function calls took {large:?}, more than 24 times {small:?}"
    );
}

fn result_ids(req: &Request) -> Vec<&str> {
    let Request::Generate(req) = req else {
        return Vec::new();
    };
    req.messages
        .iter()
        .flat_map(|m| &m.blocks)
        .filter_map(|block| match block {
            Block::ToolResult(result) => Some(result.call_id.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_response_without_an_id_answers_the_oldest_open_call_of_its_name() {
    let call = |name: &str, id: &str| json!({ "functionCall": { "name": name, "id": id } });
    let response = |name: &str, id: Option<&str>| {
        let mut response = json!({ "name": name, "response": { "output": "ok" } });
        if let Some(id) = id {
            response["id"] = id.into();
        }
        json!({ "functionResponse": response })
    };
    let body = json!({ "contents": [
        { "role": "model", "parts": [
            call("lookup", "a"), call("search", "b"), call("lookup", "c"),
            call("lookup", "a"), call("lookup", "d"),
        ] },
        { "role": "user", "parts": [
            response("lookup", Some("c")),
            response("lookup", None),
            response("search", None),
            response("lookup", None),
            response("lookup", None),
        ] },
    ] });
    let req = decode(&serde_json::to_vec(&body).unwrap());
    assert_eq!(result_ids(&req), ["c", "a", "b", "a", "d"]);
}
