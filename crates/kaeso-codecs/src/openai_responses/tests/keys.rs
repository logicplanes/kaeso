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

fn reencoded(body: &[u8]) -> Option<Value> {
    let req =
        OpenAiResponses::decode_request(RequestKind::Generate, body, &HeaderList::new()).ok()?;
    let mut ctx = EgressCtx::new(&req).with_provider("openai");
    let sent = OpenAiResponses::encode_request(&req, &mut ctx).ok()?;
    serde_json::from_slice(&sent.body).ok()
}

const ODD_KEYS: [&str; 8] = [
    "a.b",
    "",
    "%",
    "a%2Eb",
    "@scope",
    "~event",
    "header:x-id",
    "%68eader:x",
];

fn items_of<'a>(body: &'a mut Value, list: &str) -> impl Iterator<Item = &'a mut Value> {
    body[list]
        .as_array_mut()
        .into_iter()
        .flatten()
        .filter(|item| item.is_object())
}

fn without(mut body: Value, list: Option<&str>, key: &str, name: &str) -> Value {
    let held = json!({ "kept": 1.50 });
    match list {
        None => assert_eq!(
            body.as_object_mut().unwrap().remove(key),
            Some(held),
            "{name} {key:?}"
        ),
        Some(list) => {
            for item in items_of(&mut body, list) {
                let fields = item.as_object_mut().unwrap();
                assert_eq!(fields.remove(key), Some(held.clone()), "{name} {key:?}");
            }
        }
    }
    body
}

fn with(body: &[u8], list: Option<&str>, key: &str) -> Vec<u8> {
    let mut body: Value = serde_json::from_slice(body).unwrap();
    let held = json!({ "kept": 1.50 });
    match list {
        None => body[key] = held,
        Some(list) => items_of(&mut body, list).for_each(|item| item[key] = held.clone()),
    }
    serde_json::to_vec(&body).unwrap()
}

#[test]
fn a_key_a_path_cannot_spell_stays_in_a_request_with_every_other_field() {
    for key in ODD_KEYS {
        for (name, body) in fixtures(DIR, "requests", "json") {
            let Some(before) = reencoded(&body) else {
                continue;
            };
            for list in [None, Some("input")] {
                let after = reencoded(&with(&body, list, key)).unwrap();
                assert_eq!(without(after, list, key, &name), before, "{name} {key:?}");
            }
        }
    }
}

#[test]
fn a_key_a_path_cannot_spell_stays_in_a_part_a_call_and_a_result() {
    for key in ODD_KEYS {
        let body = with_input(vec![
            json!({ "type": "message", "role": "user", "content": [
                { "type": "input_text", "text": "Read this.", key: 2 },
                { "type": "input_file", "file_id": "file-1", key: 3 },
            ] }),
            json!({ "type": "function_call", "call_id": "c", "name": "f", "arguments": "{}", key: 4 }),
            json!({ "type": "function_call_output", "call_id": "c", "output": "o", key: 5 }),
        ]);
        let sent = encode_req(&decode_req(RequestKind::Generate, &body)).0.body;
        assert_json_eq(key, &body, &sent);
    }
}

#[test]
fn a_key_a_path_cannot_spell_is_never_sent_as_a_header_and_other_dialects_refuse_it() {
    let body =
        br#"{"model":"gpt-5","input":[{"role":"user","content":"hi"}],"header:x-id":"1","a.b":2}"#;
    let req = decode_req(RequestKind::Generate, body);
    let (sent, _) = encode_req(&req);
    assert!(
        !format!("{:?}", sent.headers).contains("x-id"),
        "{:?}",
        sent.headers
    );
    let Request::Generate(generate) = &req else {
        panic!("a generate request");
    };
    assert_eq!(generate.extras.headers().count(), 0);
    let refused = crate::dispatch::check(DialectId::AnthropicMessages, &req, &EgressCtx::new(&req))
        .unwrap_err();
    assert!(refused.unsupported_feature().is_some(), "{refused:?}");
}

#[test]
fn a_key_a_path_cannot_spell_stays_in_a_reply() {
    let ctx = EncodeCtx::new(RequestKind::Generate, kaeso_kpp::StreamMode::Off);
    let reencoded = |body: &[u8]| -> Option<Value> {
        let reply =
            OpenAiResponses::decode_response(200, &HeaderList::new(), body, &gen_ctx()).ok()?;
        let sent = OpenAiResponses::encode_response(&reply, &ctx).ok()?;
        serde_json::from_slice(&sent).ok()
    };
    for key in ODD_KEYS {
        for (name, body) in fixtures(DIR, "responses", "json") {
            let Some(before) = reencoded(&body) else {
                continue;
            };
            for list in [None, Some("output")] {
                let after = reencoded(&with(&body, list, key)).unwrap();
                assert_eq!(without(after, list, key, &name), before, "{name} {key:?}");
            }
        }
    }
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
