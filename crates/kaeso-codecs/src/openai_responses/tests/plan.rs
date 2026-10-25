use serde_json::json;

use super::*;

fn for_plan(body: Value, from: DialectId) -> Value {
    let bytes = serde_json::to_vec(&body).unwrap();
    let mut req =
        crate::dispatch::decode_request(from, RequestKind::Generate, &bytes, &HeaderList::new())
            .unwrap();
    if let Request::Generate(g) = &mut req {
        for_chatgpt_plan(g);
    }
    let (encoded, _) = encode_req(&req);
    serde_json::from_slice(&encoded.body).unwrap()
}

#[test]
fn explicit_defaults_and_nulls_of_refused_fields_stay_out() {
    let sent = for_plan(
        json!({
            "model": "gpt-5",
            "input": "Hi",
            "background": false,
            "metadata": {},
            "truncation": "disabled",
            "temperature": null,
            "max_output_tokens": null,
            "top_p": null,
            "user": null,
            "safety_identifier": null,
            "previous_response_id": null,
            "max_tool_calls": null,
            "top_logprobs": 0,
            "prompt_cache_retention": null,
            "store": null,
            "stream": false,
        }),
        R,
    );
    let keys: Vec<&str> = sent
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    for refused in [
        "background",
        "metadata",
        "truncation",
        "temperature",
        "max_output_tokens",
        "top_p",
        "user",
        "safety_identifier",
        "previous_response_id",
        "max_tool_calls",
        "top_logprobs",
        "prompt_cache_retention",
    ] {
        assert!(!keys.contains(&refused), "{refused} in {sent}");
    }
    assert_eq!(sent["store"], false);
    assert_eq!(sent["stream"], true);
    assert_eq!(sent["input"], json!([{ "role": "user", "content": "Hi" }]));
}

#[test]
fn system_text_anywhere_goes_as_instructions_or_developer_messages() {
    let sent = for_plan(
        json!({
            "model": "gpt-5",
            "input": [
                { "role": "system", "content": "You are terse." },
                { "role": "user", "content": "Hi" },
                { "role": "system", "content": "be brief" },
                { "role": "user", "content": "Go" },
                { "role": "system", "content": [{ "type": "input_text", "text": "a" }, { "type": "input_image", "image_url": "https://kaeso.example.com/a.png" }] },
            ],
        }),
        R,
    );
    let roles: Vec<&str> = sent["input"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|i| i["role"].as_str())
        .collect();
    assert!(!roles.contains(&"system"), "{sent}");
    assert_eq!(roles, ["user", "developer", "user", "developer"]);
    assert_eq!(sent["instructions"], "You are terse.");
    assert_eq!(sent["input"][1]["content"], "be brief");

    let from_chat = for_plan(
        json!({
            "model": "gpt-5",
            "messages": [
                { "role": "user", "content": "Hi" },
                { "role": "system", "content": "be brief" },
                { "role": "user", "content": "Go" },
            ],
            "metadata": { "ticket": "42" },
            "store": true,
            "user": "end-user-7",
        }),
        DialectId::OpenAiChat,
    );
    assert!(!from_chat.to_string().contains("\"system\""), "{from_chat}");
    for refused in ["metadata", "user", "safety_identifier"] {
        assert!(from_chat.get(refused).is_none(), "{refused} in {from_chat}");
    }
    assert_eq!(from_chat["store"], false);
    assert_eq!(from_chat["stream"], true);
}

#[test]
fn leading_developer_items_reach_the_plan_route_as_the_app_wrote_them() {
    let input = json!([
        { "type": "message", "id": "msg_1", "role": "developer", "content": [{ "type": "input_text", "text": "You are Codex." }] },
        { "type": "message", "id": "msg_2", "role": "developer", "content": [{ "type": "input_text", "text": "<skills_instructions/>" }] },
        { "type": "message", "role": "user", "content": [{ "type": "input_text", "text": "Hi" }] },
    ]);
    let sent = for_plan(json!({ "model": "gpt-5", "input": input }), R);
    assert_eq!(sent["input"], input);
    assert!(sent.get("instructions").is_none(), "{sent}");
}
