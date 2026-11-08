use super::*;

const IGNORING: MessagesProfile = MessagesProfile::compatible("Example Cloud")
    .refusing(&[
        "top_k",
        "stop_sequences",
        "mcp_servers",
        "container",
        "context_management",
    ])
    .sampling_fixed_at(&[
        ("temperature", FixedValue::thousandths(1000)),
        ("top_p", FixedValue::thousandths(950)),
    ])
    .auto_tool_choice_only();

fn sent_with(
    dialect: DialectId,
    body: Value,
    profile: MessagesProfile,
) -> Result<(Value, Vec<String>), (String, String)> {
    let req = crate::dispatch::decode_request(
        dialect,
        RequestKind::Generate,
        &serde_json::to_vec(&body).unwrap(),
        &HeaderList::new(),
    )
    .unwrap();
    let mut ctx = EgressCtx::new(&req)
        .with_provider("example")
        .with_messages_profile(profile);
    crate::dispatch::check(A, &req, &ctx).map_err(|e| (e.param.clone().unwrap(), e.message))?;
    let out = AnthropicMessages::encode_request(&req, &mut ctx)
        .map_err(|e| (e.param.unwrap(), e.message))?;
    Ok((serde_json::from_slice(&out.body).unwrap(), ctx.left_out))
}

fn sent(body: Value) -> Result<(Value, Vec<String>), (String, String)> {
    sent_with(A, body, IGNORING)
}

fn request(extra: Value) -> Value {
    let mut body = json!({
        "model": "m",
        "max_tokens": 64,
        "tools": [{ "name": "f", "input_schema": { "type": "object" } }],
        "messages": [{ "role": "user", "content": "Hi" }]
    });
    for (k, v) in extra.as_object().unwrap() {
        body[k] = v.clone();
    }
    body
}

#[test]
fn ignored_fields_are_refused_with_the_field_the_app_wrote() {
    for (extra, param) in [
        (json!({ "top_k": 5 }), "top_k"),
        (json!({ "stop_sequences": ["END"] }), "stop_sequences"),
        (json!({ "container": "container_011" }), "container"),
        (
            json!({ "mcp_servers": [{ "type": "url", "url": "https://mcp.example.com", "name": "x" }] }),
            "mcp_servers",
        ),
    ] {
        let (got, message) = sent(request(extra)).unwrap_err();
        assert_eq!(got, param);
        assert_eq!(
            message,
            format!("`{param}` is not supported by Example Cloud; remove it from the request.")
        );
    }
    let chat = json!({
        "model": "m",
        "max_completion_tokens": 64,
        "stop": ["END"],
        "messages": [{ "role": "user", "content": "Hi" }]
    });
    let (param, _) = sent_with(DialectId::OpenAiChat, chat, IGNORING).unwrap_err();
    assert_eq!(param, "stop");
}

#[test]
fn sampling_goes_only_at_the_values_the_provider_always_uses() {
    let (body, _) = sent(request(json!({ "temperature": 1, "top_p": 0.95 }))).unwrap();
    assert_eq!(
        (body["temperature"].clone(), body["top_p"].clone()),
        (json!(1), json!(0.95))
    );
    let (param, message) = sent(request(json!({ "temperature": 0.2 }))).unwrap_err();
    assert_eq!(param, "temperature");
    assert_eq!(
        message,
        "Example Cloud always samples at `temperature` 1; remove `temperature` or set it to 1."
    );
    let (param, message) = sent(request(json!({ "top_p": 0.9 }))).unwrap_err();
    assert_eq!(param, "top_p");
    assert!(message.ends_with("set it to 0.95."), "{message}");
}

#[test]
fn only_auto_tool_choice_reaches_a_provider_that_chooses_tools_itself() {
    let (body, _) = sent(request(json!({ "tool_choice": { "type": "auto" } }))).unwrap();
    assert_eq!(body["tool_choice"], json!({ "type": "auto" }));
    for choice in [
        json!({ "type": "any" }),
        json!({ "type": "tool", "name": "f" }),
        json!({ "type": "none" }),
    ] {
        let (param, message) = sent(request(json!({ "tool_choice": choice }))).unwrap_err();
        assert_eq!(param, "tool_choice");
        assert_eq!(
            message,
            "Example Cloud chooses tools itself; remove `tool_choice` or set it to `auto`."
        );
    }
}

#[test]
fn context_management_that_only_clears_thinking_is_left_out_and_named() {
    let clears = json!({ "edits": [{ "type": "clear_thinking_20251015", "keep": "all" }] });
    let (body, left_out) = sent(request(json!({ "context_management": clears }))).unwrap();
    assert!(body.get("context_management").is_none(), "{body}");
    assert_eq!(left_out, ["context_management"]);
    let tool_uses = json!({ "edits": [{ "type": "clear_tool_uses_20250919" }] });
    let (param, message) = sent(request(json!({ "context_management": tool_uses }))).unwrap_err();
    assert_eq!(param, "context_management");
    assert!(
        message.contains("CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS=1"),
        "{message}"
    );
}

#[test]
fn anthropics_own_profile_sends_every_field_as_written() {
    let body = request(json!({
        "top_k": 5,
        "temperature": 0.2,
        "stop_sequences": ["END"],
        "tool_choice": { "type": "any" },
        "container": "container_011"
    }));
    let (sent, left_out) = sent_with(A, body.clone(), MessagesProfile::ANTHROPIC).unwrap();
    for field in [
        "top_k",
        "temperature",
        "stop_sequences",
        "tool_choice",
        "container",
    ] {
        assert_eq!(sent[field], body[field], "{field}");
    }
    assert!(left_out.is_empty());
    assert_eq!(MessagesProfile::default(), MessagesProfile::ANTHROPIC);
    assert_eq!(
        MessagesProfile::compatible("Example Cloud").provider,
        "Example Cloud"
    );
}
