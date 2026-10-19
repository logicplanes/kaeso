use kaeso_kpp::{
    DialectId, ErrorKind, GenerateRequest, KppError, Message, Origin, Request, StreamMode,
};
use serde_json::{Value, json};

use super::tests::decode;
use super::{ChatProfile, OpenAiChat, OutputLimit, SeedField};
use crate::codec::{Codec, EgressCtx};
use crate::test_support::fixtures;

const STRICT: ChatProfile = ChatProfile::compatible("Example Cloud")
    .max_tokens()
    .system_role()
    .random_seed()
    .without_end_user()
    .refusing(&["logit_bias", "store"])
    .refusing_in_messages(&["name", "refusal"]);

fn encode_with(req: &Request, profile: ChatProfile) -> Result<Value, KppError> {
    let mut ctx = EgressCtx::new(req).with_chat_profile(profile);
    let out = OpenAiChat::encode_request(req, &mut ctx)?;
    Ok(serde_json::from_slice(&out.body).unwrap())
}

fn chat(body: Value) -> Request {
    decode(body.to_string().as_bytes()).unwrap()
}

fn foreign(dialect: DialectId) -> GenerateRequest {
    let mut req = GenerateRequest::new("example-model");
    req.extras.origin = Some(Origin::new(dialect));
    req.messages = vec![Message::user("Hi")];
    req
}

fn refusal(err: &KppError) -> (Option<&str>, Option<&str>, String) {
    assert!(matches!(err.kind, ErrorKind::Unsupported { .. }), "{err:?}");
    (
        err.unsupported_feature(),
        err.param.as_deref(),
        err.message.clone(),
    )
}

#[test]
fn the_openai_profile_is_the_default_and_a_bare_compatible_profile_changes_nothing() {
    assert_eq!(ChatProfile::default(), ChatProfile::OPENAI);
    assert_eq!(
        EgressCtx::new(&chat(json!({"model": "m", "messages": []}))).chat_profile,
        ChatProfile::OPENAI
    );
    let bare = ChatProfile::compatible("Example Cloud");
    assert_eq!(
        (
            bare.output_limit,
            bare.developer_role,
            bare.seed,
            bare.end_user
        ),
        (OutputLimit::AsWritten, true, SeedField::Seed, true)
    );
    for (name, body) in fixtures("openai_chat", "requests", "json") {
        let Ok(req) = decode(&body) else {
            continue;
        };
        let mut ctx = EgressCtx::new(&req);
        let Ok(openai) = OpenAiChat::encode_request(&req, &mut ctx) else {
            continue;
        };
        let mut ctx = EgressCtx::new(&req).with_chat_profile(bare);
        let compatible = OpenAiChat::encode_request(&req, &mut ctx).unwrap();
        assert_eq!(openai.body, compatible.body, "{name}");
    }
}

#[test]
fn a_max_tokens_profile_sends_max_tokens_whatever_the_app_wrote() {
    let profile = ChatProfile::compatible("Example Cloud").max_tokens();
    for dialect in [DialectId::AnthropicMessages, DialectId::OpenAiResponses] {
        let mut req = foreign(dialect);
        req.sampling.max_output_tokens = Some(300);
        let out = encode_with(&Request::Generate(req), profile).unwrap();
        assert_eq!(out["max_tokens"], 300, "{dialect:?}");
        assert!(out.get("max_completion_tokens").is_none(), "{dialect:?}");
    }
    for body in [
        json!({"model": "m", "messages": [], "max_completion_tokens": 20}),
        json!({"model": "m", "messages": [], "max_tokens": 20}),
        json!({"model": "m", "messages": [], "max_tokens": 20, "max_completion_tokens": 20}),
    ] {
        let out = encode_with(&chat(body.clone()), profile).unwrap();
        assert_eq!(
            out,
            json!({"model": "m", "messages": [], "max_tokens": 20}),
            "{body}"
        );
    }
    let out = encode_with(
        &chat(json!({"model": "m", "messages": [], "max_completion_tokens": null})),
        profile,
    )
    .unwrap();
    assert_eq!(
        out,
        json!({"model": "m", "messages": [], "max_tokens": null})
    );
}

#[test]
fn a_system_role_profile_sends_developer_messages_as_system_messages() {
    let profile = ChatProfile::compatible("Example Cloud").system_role();
    let body = json!({"model": "m", "messages": [
        {"role": "developer", "content": "Be brief."},
        {"role": "user", "content": "Hi"},
        {"role": "assistant", "content": "Hello"},
        {"role": "developer", "content": [{"type": "text", "text": "Now in French."}]},
        {"role": "system", "content": "Stay polite."},
        {"role": "user", "content": "Again"}
    ]});
    let out = encode_with(&chat(body.clone()), profile).unwrap();
    let roles: Vec<&str> = out["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["role"].as_str().unwrap())
        .collect();
    assert_eq!(
        roles,
        ["system", "user", "assistant", "system", "system", "user"]
    );
    assert_eq!(
        out["messages"][3]["content"],
        body["messages"][3]["content"]
    );
    let openai = encode_with(&chat(body.clone()), ChatProfile::OPENAI).unwrap();
    assert_eq!(openai, body);
}

#[test]
fn a_random_seed_profile_renames_the_seed_and_refuses_what_it_cannot_take() {
    let profile = ChatProfile::compatible("Example Cloud").random_seed();
    let out = encode_with(
        &chat(json!({"model": "m", "messages": [], "seed": 42})),
        profile,
    )
    .unwrap();
    assert_eq!(
        out,
        json!({"model": "m", "messages": [], "random_seed": 42})
    );

    let err = encode_with(
        &chat(json!({"model": "m", "messages": [], "seed": -1})),
        profile,
    )
    .unwrap_err();
    assert_eq!(
        refusal(&err),
        (
            Some("seed"),
            Some("seed"),
            "Example Cloud takes a `seed` of 0 or more; send a seed that is not negative."
                .to_owned()
        )
    );

    let err = encode_with(
        &chat(json!({"model": "m", "messages": [], "seed": 1, "random_seed": 2})),
        profile,
    )
    .unwrap_err();
    assert_eq!(
        refusal(&err).2,
        "Example Cloud takes one seed; send `seed` or `random_seed`, not both."
    );

    for both in [
        json!({"model": "m", "messages": [], "seed": 5, "random_seed": null}),
        json!({"model": "m", "messages": [], "seed": null, "random_seed": 7}),
        json!({"model": "m", "messages": [], "seed": null, "random_seed": null}),
    ] {
        let err = encode_with(&chat(both.clone()), profile).unwrap_err();
        assert_eq!(refusal(&err).1, Some("seed"), "{both}");
    }

    let own = encode_with(
        &chat(json!({"model": "m", "messages": [], "random_seed": 7})),
        profile,
    )
    .unwrap();
    assert_eq!(own, json!({"model": "m", "messages": [], "random_seed": 7}));
}

#[test]
fn a_profile_without_an_end_user_field_leaves_the_user_hint_out() {
    let profile = ChatProfile::compatible("Example Cloud").without_end_user();
    for dialect in [DialectId::AnthropicMessages, DialectId::OpenAiResponses] {
        let mut req = foreign(dialect);
        req.user = Some("user-42".into());
        let out = encode_with(&Request::Generate(req.clone()), profile).unwrap();
        assert!(out.get("user").is_none() && out.get("safety_identifier").is_none());
        let openai = encode_with(&Request::Generate(req), ChatProfile::OPENAI).unwrap();
        assert_eq!(openai["safety_identifier"], "user-42");
    }
    for key in ["user", "safety_identifier"] {
        let out = encode_with(
            &chat(json!({"model": "m", "messages": [], key: "user-42"})),
            profile,
        )
        .unwrap();
        assert_eq!(out, json!({"model": "m", "messages": []}), "{key}");
    }
}

#[test]
fn refused_fields_fail_before_anything_is_sent_and_name_the_field() {
    let err = encode_with(
        &chat(json!({"model": "m", "messages": [], "logit_bias": {"50256": -100}})),
        STRICT,
    )
    .unwrap_err();
    assert_eq!(
        refusal(&err),
        (
            Some("logit_bias"),
            Some("logit_bias"),
            "`logit_bias` is not supported by Example Cloud; remove it from the request."
                .to_owned()
        )
    );

    let err = encode_with(
        &chat(json!({"model": "m", "messages": [
            {"role": "system", "content": "Be brief."},
            {"role": "user", "content": "Hi", "name": "alice"}
        ]})),
        STRICT,
    )
    .unwrap_err();
    assert_eq!(
        refusal(&err),
        (
            Some("name"),
            Some("messages[1].name"),
            "`messages[1].name` is not supported by Example Cloud; remove it from the request."
                .to_owned()
        )
    );

    let tool_named = json!({"model": "m", "messages": [
        {"role": "user", "content": "Weather?"},
        {"role": "assistant", "content": null, "tool_calls": [
            {"id": "D681PevKs", "type": "function", "function": {"name": "get_weather", "arguments": "{}"}}
        ]},
        {"role": "tool", "tool_call_id": "D681PevKs", "name": "get_weather", "content": "18C"}
    ]});
    let out = encode_with(&chat(tool_named.clone()), STRICT).unwrap();
    assert_eq!(out["messages"][2]["name"], "get_weather");
}

#[test]
fn a_strict_profile_carries_a_foreign_request_without_refusing_it() {
    let mut req = foreign(DialectId::AnthropicMessages);
    req.system = vec![kaeso_kpp::Block::text("Be brief.")];
    req.sampling.max_output_tokens = Some(64);
    req.user = Some("user-42".into());
    req.stream = StreamMode::on(true);
    let out = encode_with(&Request::Generate(req), STRICT).unwrap();
    assert_eq!(
        out,
        json!({
            "model": "example-model",
            "messages": [
                {"role": "system", "content": "Be brief."},
                {"role": "user", "content": "Hi"}
            ],
            "max_tokens": 64,
            "stream": true,
            "stream_options": {"include_usage": true}
        })
    );
}

#[test]
fn rewritten_keys_never_leak_back_through_conflicts_or_explicit_nulls() {
    let err = encode_with(
        &chat(json!({"model": "m", "messages": [], "max_completion_tokens": 20, "max_tokens": 50})),
        STRICT,
    )
    .unwrap_err();
    assert_eq!(
        refusal(&err),
        (
            Some("max_tokens"),
            Some("max_tokens"),
            "Example Cloud takes one output limit; send `max_tokens` or `max_completion_tokens`, not both with different values."
                .to_owned()
        )
    );

    let out = encode_with(
        &chat(json!({"model": "m", "messages": [], "user": "a", "safety_identifier": "b"})),
        STRICT,
    )
    .unwrap();
    assert_eq!(out, json!({"model": "m", "messages": []}));

    let out = encode_with(
        &chat(json!({
            "model": "m", "messages": [],
            "seed": null, "user": null, "safety_identifier": null, "max_completion_tokens": null
        })),
        STRICT,
    )
    .unwrap();
    assert_eq!(
        out,
        json!({"model": "m", "messages": [], "random_seed": null, "max_tokens": null})
    );
}

#[test]
fn a_refused_message_field_from_another_format_is_named_where_the_app_wrote_it() {
    let mut req = foreign(DialectId::OpenAiResponses);
    req.messages = vec![
        Message::user("Tell me a secret."),
        Message::new(
            kaeso_kpp::Role::Assistant,
            vec![kaeso_kpp::Block::Refusal(kaeso_kpp::RefusalBlock::new(
                "I cannot share that.",
            ))],
        ),
        Message::user("Then say hello."),
    ];
    let err = encode_with(&Request::Generate(req.clone()), STRICT).unwrap_err();
    assert_eq!(
        refusal(&err),
        (
            Some("refusal block"),
            Some("messages[1].blocks[0]"),
            "`refusal block` is not supported by Example Cloud; remove the refusal from the conversation history or replace it with text."
                .to_owned()
        )
    );
    let openai = encode_with(&Request::Generate(req), ChatProfile::OPENAI).unwrap();
    assert_eq!(openai["messages"][1]["refusal"], "I cannot share that.");

    let err = encode_with(
        &chat(json!({"model": "m", "messages": [
            {"role": "user", "content": "Tell me a secret."},
            {"role": "assistant", "content": null, "refusal": "I cannot share that."}
        ]})),
        STRICT,
    )
    .unwrap_err();
    assert_eq!(refusal(&err).1, Some("messages[1].refusal"));
}
