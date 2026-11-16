use proptest::prelude::*;
use serde_json::json;

use super::*;
use crate::openai_responses::overrides::ONLY_DIFF;

fn both<T>(f: impl Fn() -> T) -> (T, T) {
    let quick = f();
    ONLY_DIFF.with(|d| d.set(true));
    let diffed = f();
    ONLY_DIFF.with(|d| d.set(false));
    (quick, diffed)
}

fn request(body: &[u8]) -> Option<Value> {
    OpenAiResponses::decode_request(RequestKind::Generate, body, &HeaderList::new())
        .ok()
        .map(|r| serde_json::to_value(r).unwrap())
}

fn same_request(name: &str, body: &[u8]) {
    let (quick, diffed) = both(|| request(body));
    assert_eq!(quick, diffed, "{name}");
    // Kaeso's own ids never go back to a provider, so their items don't round trip.
    if name.contains("_kaeso_") {
        return;
    }
    if let Ok(Request::Generate(g)) =
        OpenAiResponses::decode_request(RequestKind::Generate, body, &HeaderList::new())
    {
        assert_json_eq(name, body, &encode_req(&Request::Generate(g)).0.body);
    }
}

fn with_input(items: Vec<Value>) -> Vec<u8> {
    serde_json::to_vec(&json!({ "model": "gpt-5", "input": items })).unwrap()
}

#[test]
fn every_fixture_decodes_alike_with_or_without_the_canonical_diff() {
    for (name, body) in fixtures(DIR, "requests", "json") {
        let (quick, diffed) = both(|| request(&body));
        assert_eq!(quick, diffed, "{name}");
    }
    for (name, body) in fixtures(DIR, "responses", "json") {
        let (quick, diffed) = both(|| {
            OpenAiResponses::decode_response(200, &HeaderList::new(), &body, &gen_ctx())
                .ok()
                .map(|r| serde_json::to_value(r).unwrap())
        });
        assert_eq!(quick, diffed, "{name}");
    }
}

#[test]
fn items_an_sdk_never_writes_decode_alike_and_round_trip() {
    let cases = [
        json!({ "type": "message", "id": "msg_1", "status": "completed", "role": "system", "content": "Be brief." }),
        json!({ "role": "developer", "content": [{ "type": "input_text", "text": "a" }, { "type": "input_text", "text": "b" }] }),
        json!({ "type": "message", "role": "user", "content": [{ "type": "output_text", "text": "typed as output" }] }),
        json!({ "type": "message", "role": "assistant", "content": [{ "type": "input_text", "text": "typed as input" }] }),
        json!({ "type": "message", "role": "assistant", "content": [{ "type": "output_text", "text": "no annotations" }] }),
        json!({ "type": "message", "role": "assistant", "content": [{ "type": "output_text", "text": "null", "annotations": null }] }),
        json!({ "type": "message", "role": "assistant", "content": [{ "type": "output_text", "text": "cited", "annotations": [{ "type": "url_citation", "url": "https://example.com", "start_index": 0, "end_index": 5 }], "logprobs": [] }] }),
        json!({ "type": "message", "role": "user", "content": [{ "type": "input_text", "text": "x", "annotations": [] }] }),
        json!({ "type": "message", "role": "user", "content": [{ "type": "input_text", "text": "cache", "prompt_cache_breakpoint": { "mode": "explicit" } }] }),
        json!({ "type": "message", "role": "user", "content": [{ "type": "input_text", "text": "odd", "prompt_cache_breakpoint": true }] }),
        json!({ "type": "message", "role": "user", "content": [{ "type": "input_text", "text": "dotted", "a.b": 1 }] }),
        json!({ "type": "message", "role": "assistant", "content": [{ "type": "refusal", "refusal": "No.", "extra": [1] }] }),
        json!({ "type": "message", "role": "user", "content": [], "x_future": { "k": "v" } }),
        json!({ "type": "message", "role": "user", "content": [{ "type": "input_image", "image_url": "https://example.com/a.png", "detail": "auto" }] }),
        json!({ "type": "reasoning", "summary": [] }),
        json!({ "type": "reasoning", "id": null, "summary": [], "encrypted_content": null, "content": [] }),
        json!({ "type": "reasoning", "id": "rs_1", "summary": [{ "type": "summary_text", "text": "s", "x": 1 }] }),
        json!({ "type": "reasoning", "id": "rs_1", "summary": [{ "type": "other_text", "text": "s" }] }),
        json!({ "type": "reasoning", "id": "rs_1", "summary": [{ "text": "untyped" }] }),
        json!({ "type": "reasoning", "id": "rs_1", "summary": null }),
        json!({ "type": "reasoning", "id": "rs_1" }),
        json!({ "type": "reasoning", "id": "rs_1", "summary": [], "content": [{ "type": "reasoning_text", "text": "t" }] }),
        json!({ "type": "reasoning", "id": "rs_1", "summary": [], "content": [{ "type": "reasoning_text", "text": "t", "y": 2 }] }),
        json!({ "type": "reasoning", "id": "rs_kaeso_x_1", "summary": [], "status": "completed", "encrypted_content": "gAAAAAB" }),
        json!({ "type": "function_call", "id": "fc_1", "status": "completed", "name": "f", "arguments": "{\"a\":1}", "call_id": "c1" }),
        json!({ "type": "function_call", "name": "f", "arguments": "", "call_id": "c2", "namespace": "ns" }),
        json!({ "type": "function_call", "name": "f", "arguments": "not json", "call_id": "c3" }),
        json!({ "type": "function_call_output", "call_id": "c1", "output": "done", "status": "completed" }),
        json!({ "type": "function_call_output", "call_id": "c2", "output": [{ "type": "input_text", "text": "a" }] }),
        json!({ "type": "function_call_output", "call_id": "c3", "output": [] }),
        json!({ "type": "function_call_output", "call_id": "c4", "output": [{ "type": "input_image", "image_url": "data:image/png;base64,AAAA" }] }),
    ];
    for (i, item) in cases.iter().enumerate() {
        same_request(
            &format!("case {i}: {item}"),
            &with_input(vec![item.clone()]),
        );
    }
    let typed_twice = br#"{"model":"m","input":[{"type":"function_call","type":"other","name":"f","arguments":"{}","call_id":"c"},{"type":"message","role":"user","content":"hi","type":"message"},{"type":"message","role":"user","content":[{"type":"input_text","type":"output_text","text":"t"}]}]}"#;
    let (quick, diffed) = both(|| request(typed_twice));
    assert_eq!(quick, diffed);
    let escaped = br#"{"model":"m","input":[{"type":"message","role":"system","content":"x"},{"type":"function_call","name":"f","arguments":"{}","call_id":"c","id":"fc_1"}]}"#;
    same_request("escaped", escaped);
}

fn leaf() -> impl Strategy<Value = Value> {
    prop_oneof![
        Just(Value::Null),
        Just(json!([])),
        Just(json!({})),
        Just(json!("completed")),
        Just(json!("x.y")),
        Just(json!(1.5)),
        Just(json!([{ "type": "summary_text", "text": "s" }])),
        Just(json!([{ "type": "reasoning_text", "text": "r" }])),
    ]
}

fn key() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("id".to_owned()),
        Just("status".to_owned()),
        Just("type".to_owned()),
        Just("annotations".to_owned()),
        Just("summary".to_owned()),
        Just("content".to_owned()),
        Just("encrypted_content".to_owned()),
        Just("role".to_owned()),
        Just("logprobs".to_owned()),
        Just("x_future".to_owned()),
        Just("a.b".to_owned()),
    ]
}

fn item() -> impl Strategy<Value = Value> {
    let base = prop_oneof![
        Just(json!({ "type": "message", "role": "user", "content": "hi" })),
        Just(
            json!({ "type": "message", "role": "assistant", "content": [{ "type": "output_text", "text": "a", "annotations": [] }] })
        ),
        Just(json!({ "role": "system", "content": [{ "type": "input_text", "text": "s" }] })),
        Just(json!({ "type": "function_call", "name": "f", "arguments": "{}", "call_id": "c" })),
        Just(json!({ "type": "function_call_output", "call_id": "c", "output": "o" })),
        Just(
            json!({ "type": "reasoning", "id": "rs_1", "summary": [{ "type": "summary_text", "text": "s" }], "encrypted_content": "gAAAAAB" })
        ),
    ];
    (
        base,
        proptest::collection::vec((key(), leaf()), 0..4),
        proptest::collection::vec(key(), 0..2),
    )
        .prop_map(|(mut item, set, drop)| {
            let o = item.as_object_mut().unwrap();
            for k in drop {
                o.remove(&k);
            }
            for (k, v) in set {
                o.insert(k, v);
            }
            item
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn any_item_decodes_alike_with_or_without_the_canonical_diff(items in proptest::collection::vec(item(), 1..5)) {
        let body = with_input(items);
        let (quick, diffed) = both(|| request(&body));
        prop_assert_eq!(quick, diffed);
    }
}
