use kaeso_kpp::{DialectId, ErrorKind, ModelInfo, ModelLifecycle, ModelList, ModelsQuery};
use serde_json::{Value, json};

use crate::anthropic_messages::AnthropicMessages;
use crate::openai_chat::OpenAiChat;
use crate::openai_embeddings::OpenAiEmbeddings;
use crate::openai_responses::OpenAiResponses;
use crate::test_support::{assert_json_eq, fixtures};
use crate::{Codec, HeaderList, dispatch};

const A: DialectId = DialectId::AnthropicMessages;
const OPENAI: [DialectId; 3] = [
    DialectId::OpenAiChat,
    DialectId::OpenAiResponses,
    DialectId::OpenAiEmbeddings,
];

fn h() -> HeaderList {
    HeaderList::new()
}

fn fixture(dialect: &str, name: &str) -> Vec<u8> {
    fixtures(dialect, "models", "json")
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap_or_else(|| panic!("no fixture {dialect}/models/{name}"))
        .1
}

fn json_of(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).unwrap()
}

fn list(dialect: DialectId, body: &[u8]) -> ModelList {
    dispatch::decode_models(dialect, 200, &h(), body).unwrap()
}

fn model(dialect: DialectId, body: &[u8]) -> ModelInfo {
    dispatch::decode_model(dialect, 200, &h(), body).unwrap()
}

fn query(dialect: DialectId, q: &str) -> ModelsQuery {
    dispatch::decode_models_query(dialect, q).unwrap()
}

fn query_err(dialect: DialectId, q: &str) -> kaeso_kpp::KppError {
    dispatch::decode_models_query(dialect, q).unwrap_err()
}

fn round_trip(dialect: DialectId, name: &str, body: &[u8]) {
    let stem = name.rsplit('/').next().unwrap_or(name);
    let out = if stem.starts_with("retrieve") {
        dispatch::encode_model(dialect, &model(dialect, body)).unwrap()
    } else {
        dispatch::encode_models(dialect, &list(dialect, body)).unwrap()
    };
    assert_json_eq(&format!("{dialect}:{name}"), body, &out);
}

#[test]
fn every_anthropic_models_fixture_round_trips() {
    let all = fixtures("anthropic_messages", "models", "json");
    assert!(all.len() >= 8);
    for (name, body) in all {
        round_trip(A, &name, &body);
    }
}

#[test]
fn every_openai_models_fixture_round_trips_in_all_three_openai_codecs() {
    for (name, body) in fixtures("openai_chat", "models", "json") {
        for d in OPENAI {
            round_trip(d, &name, &body);
        }
    }
}

#[test]
fn round_trips_through_the_codec_trait_directly() {
    let body = fixture("openai_chat", "list");
    let l = OpenAiResponses::decode_models(200, &h(), &body).unwrap();
    assert_json_eq(
        "responses",
        &body,
        &OpenAiEmbeddings::encode_models(&l).unwrap(),
    );
    let body = fixture("openai_chat", "retrieve");
    let m = OpenAiEmbeddings::decode_model(200, &h(), &body).unwrap();
    assert_json_eq("embeddings", &body, &OpenAiChat::encode_model(&m).unwrap());
    let body = fixture("anthropic_messages", "retrieve");
    let m = AnthropicMessages::decode_model(200, &h(), &body).unwrap();
    assert_json_eq(
        "anthropic",
        &body,
        &AnthropicMessages::encode_model(&m).unwrap(),
    );
}

#[test]
fn unusual_spellings_round_trip() {
    let anthropic_lists = [
        json!({"data": [], "first_id": null, "has_more": null}),
        json!({"data": [], "has_more": false, "last_id": "x"}),
        json!({"data": [{"id": "m", "type": "model_v2"}], "first_id": "m", "has_more": false, "last_id": "m"}),
        json!({"data": [{"id": "m", "type": "model", "display_name": null, "created_at": null,
            "lifecycle": null, "deprecated_at": null, "retires_at": null, "max_input_tokens": null,
            "max_tokens": null, "capabilities": null, "line": null}], "first_id": null, "has_more": true, "last_id": null}),
    ];
    for v in anthropic_lists {
        let body = serde_json::to_vec(&v).unwrap();
        round_trip(A, "list-inline", &body);
    }
    let openai_lists = [
        json!({"object": "list", "data": []}),
        json!({"object": "page", "data": [{"id": "m", "object": "model"}]}),
        json!({"object": "list", "data": [{"id": "m", "object": "model.v2", "created": null, "owned_by": null, "shutdown_date": null}]}),
        json!({"data": [{"id": "vendor/m:free"}]}),
        json!({"object": null, "data": [{"id": "m", "object": null}]}),
    ];
    for v in openai_lists {
        let body = serde_json::to_vec(&v).unwrap();
        for d in OPENAI {
            round_trip(d, "list-inline", &body);
        }
    }
}

#[test]
fn a_chatgpt_plan_listing_names_models_by_slug_and_keeps_their_visibility() {
    let body = fixture("openai_chat", "chatgpt-plan-list");
    for d in OPENAI {
        let l = list(d, &body);
        let ids: Vec<&str> = l.models.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(
            ids,
            ["gpt-6.1-sol", "gpt-6.1-sol-mini", "gpt-6.1-sol-review"]
        );
        let visibility: Vec<&str> = l
            .models
            .iter()
            .map(|m| m.extras.fields["visibility"].get())
            .collect();
        assert_eq!(visibility, [r#""list""#, r#""list""#, r#""hide""#]);
        assert_eq!(
            l.models[0].extras.fields["display_name"].get(),
            r#""GPT-6.1 Sol""#
        );
    }
    let nameless = br#"{"models": [{"display_name": "No slug"}]}"#;
    let err = dispatch::decode_models(DialectId::OpenAiResponses, 200, &h(), nameless).unwrap_err();
    assert_eq!(err.kind, ErrorKind::ProviderError);
}

#[test]
fn an_openrouter_listing_without_object_fields_decodes_with_its_own_fields_as_extras() {
    let body = fixture("openai_chat", "providers/openrouter/list-recorded");
    let l = list(DialectId::OpenAiChat, &body);
    assert!(!l.has_more);
    let ids: Vec<&str> = l.models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "deepseek/deepseek-v4.1-flash-1017",
            "elorian/glimpse",
            "nvidia/nemotron-3.5-lightning:free"
        ]
    );
    let free = &l.models[2];
    assert_eq!(free.owned_by, None);
    assert!(free.created_at.is_some());
    for field in ["pricing", "context_length", "supported_parameters"] {
        assert!(free.extras.fields.contains_key(field), "{field}");
    }
    assert!(l.extras.fields.contains_key("total_count"));
    let not_found = fixtures("openai_chat", "errors", "json")
        .into_iter()
        .find(|(n, _)| n == "providers/openrouter/retrieve-not-found")
        .unwrap()
        .1;
    let e = dispatch::decode_model(DialectId::OpenAiChat, 404, &h(), &not_found).unwrap_err();
    assert_eq!(
        (e.kind, e.message.as_str()),
        (ErrorKind::NotFound, "Not Found")
    );
    let own = ModelList::new(vec![ModelInfo::new("nvidia/nemotron-3.5-lightning:free")]);
    let served: Value =
        serde_json::from_slice(&dispatch::encode_models(DialectId::OpenAiChat, &own).unwrap())
            .unwrap();
    assert_eq!(served["object"], "list");
    assert_eq!(served["data"][0]["object"], "model");
}

#[test]
fn a_together_ai_listing_that_is_a_bare_array_decodes_and_writes_back_as_one() {
    let body = fixture("openai_chat", "providers/togetherai/list-recorded");
    let l = list(DialectId::OpenAiChat, &body);
    let ids: Vec<&str> = l.models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "deepseek-ai/DeepSeek-V4.1-Flash-1017",
            "deepseek-ai/DeepSeek-V4.1-Flash",
            "Qwen/Qwen3.5-9B",
            "BAAI/bge-base-en-v1.5"
        ]
    );
    assert!(l.models[0].extras.fields.contains_key("pricing"));
    assert!(!l.has_more);
    let back = json_of(&dispatch::encode_models(DialectId::OpenAiChat, &l).unwrap());
    assert!(back.is_array());
    let own = ModelList::new(vec![ModelInfo::new("Qwen/Qwen3.5-9B")]);
    let served = json_of(&dispatch::encode_models(DialectId::OpenAiChat, &own).unwrap());
    assert_eq!(served["object"], "list");
    assert!(
        dispatch::decode_models(
            DialectId::OpenAiChat,
            200,
            &h(),
            b"[{\"object\": \"model\"}]"
        )
        .is_err()
    );
}

#[test]
fn a_vercel_listing_keeps_its_own_fields_and_refuses_a_wrong_key() {
    let body = fixture("openai_chat", "providers/vercel/list-recorded");
    let l = list(DialectId::OpenAiChat, &body);
    assert!(!l.has_more);
    let ids: Vec<&str> = l.models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "anthropic/claude-haiku-5.5",
            "google/gemini-embedding-001",
            "nvidia/nemotron-3.5-lightning-free"
        ]
    );
    let free = &l.models[2];
    assert_eq!(free.owned_by.as_deref(), Some("nvidia"));
    assert!(free.created_at.is_some());
    for field in ["pricing", "context_window", "max_tokens", "type", "tags"] {
        assert!(free.extras.fields.contains_key(field), "{field}");
    }
    assert_eq!(l.models[1].extras.fields["type"].get(), r#""embedding""#);
    assert_json_eq(
        "providers/vercel/list-recorded",
        &body,
        &dispatch::encode_models(DialectId::OpenAiChat, &l).unwrap(),
    );
    let one = model(
        DialectId::OpenAiChat,
        &fixture("openai_chat", "providers/vercel/retrieve-recorded"),
    );
    assert_eq!(one.id, "nvidia/nemotron-3.5-lightning-free");
    let errors = fixtures("openai_chat", "errors", "json");
    let error = |name: &str| {
        errors
            .iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("no error fixture {name}"))
            .1
            .clone()
    };
    let e = dispatch::decode_model(
        DialectId::OpenAiChat,
        404,
        &h(),
        &error("providers/vercel/retrieve-not-found"),
    )
    .unwrap_err();
    assert_eq!(
        (e.kind, e.message.as_str()),
        (
            ErrorKind::NotFound,
            "The model 'kaeso-example/no-such-model' does not exist"
        )
    );
    let e = dispatch::decode_models(
        DialectId::OpenAiChat,
        401,
        &h(),
        &error("providers/vercel/list-invalid-api-key"),
    )
    .unwrap_err();
    assert_eq!(e.kind, ErrorKind::Authentication);
}

#[test]
fn a_cohere_listing_decodes_and_refuses_a_wrong_key_in_cohere_words() {
    let body = fixture("openai_chat", "providers/cohere/list-recorded");
    let l = list(DialectId::OpenAiChat, &body);
    assert!(!l.has_more);
    assert_eq!(l.models.len(), 37);
    assert!(
        l.models
            .iter()
            .all(|m| m.owned_by.as_deref() == Some("cohere"))
    );
    for id in [
        "command-a-03-2025",
        "command-a-reasoning-08-2025",
        "embed-v4.0",
    ] {
        assert!(l.models.iter().any(|m| m.id == id), "{id}");
    }
    assert_json_eq(
        "providers/cohere/list-recorded",
        &body,
        &dispatch::encode_models(DialectId::OpenAiChat, &l).unwrap(),
    );
    let one = model(
        DialectId::OpenAiChat,
        &fixture("openai_chat", "providers/cohere/retrieve-recorded"),
    );
    assert_eq!(one.id, "command-a-03-2025");
    let errors = fixtures("openai_chat", "errors", "json");
    let error = |name: &str| {
        errors
            .iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("no error fixture {name}"))
            .1
            .clone()
    };
    let e = dispatch::decode_model(
        DialectId::OpenAiChat,
        404,
        &h(),
        &error("providers/cohere/retrieve-not-found"),
    )
    .unwrap_err();
    assert_eq!(e.kind, ErrorKind::NotFound);
    assert!(
        e.message
            .contains("model 'kaeso-example-no-such-model' not found"),
        "{}",
        e.message
    );
    let e = dispatch::decode_models(
        DialectId::OpenAiChat,
        401,
        &h(),
        &error("providers/cohere/list-invalid-api-key"),
    )
    .unwrap_err();
    assert_eq!(e.kind, ErrorKind::Authentication);
    assert!(
        e.message.contains("Incorrect API key provided"),
        "{}",
        e.message
    );
}

#[test]
fn a_zai_listing_refuses_a_wrong_key_in_its_platform_envelope_with_http_200() {
    let body = fixtures("anthropic_messages", "errors", "json")
        .into_iter()
        .find(|(n, _)| n == "providers/zai/list-invalid-api-key")
        .unwrap()
        .1;
    let e = dispatch::decode_models(A, 200, &h(), &body).unwrap_err();
    assert_eq!(
        (e.kind, e.message.as_str(), e.status),
        (ErrorKind::Authentication, "身份验证失败。", Some(200))
    );
    let e = dispatch::decode_model(A, 200, &h(), &body).unwrap_err();
    assert_eq!(e.kind, ErrorKind::Authentication);
}

#[test]
fn anthropic_fields_map_to_kpp() {
    let l = list(A, &fixture("anthropic_messages", "list"));
    assert!(!l.has_more);
    assert_eq!(l.models.len(), 4);
    let opus = &l.models[0];
    assert_eq!(opus.id, "claude-opus-5");
    assert_eq!(opus.display_name.as_deref(), Some("Claude Opus 5"));
    assert_eq!(opus.created_at, Some(1_784_851_200));
    assert_eq!(opus.lifecycle, Some(ModelLifecycle::Active));
    assert_eq!(opus.context_window, Some(1_000_000));
    assert_eq!(opus.max_output_tokens, Some(128_000));
    assert_eq!((opus.deprecated_at, opus.retires_at), (None, None));
    assert!(opus.extras.fields.contains_key("capabilities"));
    assert_eq!(
        opus.extras.fields["line"].to_value().unwrap(),
        json!("opus")
    );
    assert!(opus.extras.form.is_empty(), "{:?}", opus.extras.form);
    let old = &l.models[3];
    assert_eq!(old.lifecycle, Some(ModelLifecycle::Deprecated));
    assert_eq!(old.deprecated_at, Some(1_781_049_600));
    assert_eq!(old.retires_at, Some(1_796_860_800));

    let m = model(A, &fixture("anthropic_messages", "retrieve-date-spellings"));
    assert_eq!(
        m.created_at,
        Some(1_754_411_400),
        "-07:00 offset applied, .250 cut"
    );
    assert_eq!(m.deprecated_at, Some(1_767_571_200));
    assert_eq!(m.lifecycle, Some(ModelLifecycle::Retired));
    assert_eq!(
        m.extras.form["spelling"],
        json!({"created_at": "2025-08-05T09:30:00.250-07:00",
               "deprecated_at": "2026-01-05t00:00:00+00:00",
               "retires_at": "2026-05-05T00:00:00.000Z"})
    );

    let legacy = model(A, &fixture("anthropic_messages", "retrieve-legacy"));
    assert_eq!(legacy.lifecycle, None);
    assert_eq!(
        legacy.extras.form["none"],
        json!({"lifecycle": "absent", "deprecated_at": "absent", "retires_at": "absent"})
    );
    assert_eq!(legacy.extras.fields["capabilities"].get(), "null");
    assert!(!legacy.extras.fields.contains_key("line"));
}

#[test]
fn unknown_fields_and_values_are_kept_as_extras() {
    let l = list(A, &fixture("anthropic_messages", "unknown-fields"));
    assert!(l.extras.is_from(A));
    assert_eq!(l.extras.fields["x_future_list_field"].get(), "7");
    let m = &l.models[0];
    assert!(m.extras.fields.contains_key("x_future_field"));
    assert_eq!(
        m.lifecycle,
        Some(ModelLifecycle::Other("sunsetting".into()))
    );
    assert_eq!(m.extras.fields["line"].get(), "\"x-future-line\"");

    for d in OPENAI {
        let l = list(d, &fixture("openai_chat", "unknown-fields"));
        assert!(l.extras.is_from(d));
        assert_eq!(l.extras.fields["x_future_list_field"].get(), "true");
        assert!(l.models[0].extras.fields.contains_key("x_future_field"));
        assert_eq!(
            l.models[1].extras.form["none"],
            json!({"shutdown_date": "absent"})
        );
    }
}

#[test]
fn openai_fields_map_to_kpp() {
    let m = model(DialectId::OpenAiChat, &fixture("openai_chat", "retrieve"));
    assert_eq!(m.id, "gpt-6-astra");
    assert_eq!(m.created_at, Some(1_686_935_002));
    assert_eq!(m.owned_by.as_deref(), Some("openai"));
    assert_eq!(
        m.retires_at,
        Some(1_792_713_600),
        "2026-10-23 at midnight UTC"
    );
    assert!(m.extras.form.is_empty());
    let l = list(DialectId::OpenAiChat, &fixture("openai_chat", "list"));
    assert_eq!(l.models.len(), 3);
    assert_eq!(l.models[0].retires_at, None);
    assert!(!l.has_more);
}

#[test]
fn received_first_and_last_ids_are_kept_only_while_the_data_matches() {
    let body = fixture("anthropic_messages", "list-doc");
    let mut l = list(A, &body);
    assert_eq!(
        l.extras.form["first_id"],
        json!({"for": "claude-opus-5", "sent": "first_id"})
    );
    l.models.push(ModelInfo::new("claude-extra"));
    let out = json_of(&dispatch::encode_models(A, &l).unwrap());
    assert_eq!(out["first_id"], json!("first_id"));
    assert_eq!(out["last_id"], json!("claude-extra"));
    l.models.clear();
    let out = json_of(&dispatch::encode_models(A, &l).unwrap());
    assert_eq!(
        (&out["first_id"], &out["last_id"]),
        (&Value::Null, &Value::Null)
    );
}

#[test]
fn a_changed_date_wins_over_its_stale_spelling() {
    let mut m = model(A, &fixture("anthropic_messages", "retrieve-date-spellings"));
    m.created_at = Some(0);
    m.retires_at = m.retires_at.map(|s| s + 1);
    let out = json_of(&dispatch::encode_model(A, &m).unwrap());
    assert_eq!(out["created_at"], json!("1970-01-01T00:00:00Z"));
    assert_eq!(out["retires_at"], json!("2026-05-05T00:00:01Z"));
    assert_eq!(out["deprecated_at"], json!("2026-01-05t00:00:00+00:00"));
}

#[test]
fn models_without_extras_get_documented_defaults() {
    let bare = ModelInfo::new("m-1");
    let a = json_of(&dispatch::encode_model(A, &bare).unwrap());
    assert_eq!(
        a,
        json!({"id": "m-1", "type": "model", "display_name": "m-1",
               "created_at": "1970-01-01T00:00:00Z", "lifecycle": "active",
               "deprecated_at": null, "retires_at": null, "max_input_tokens": null,
               "max_tokens": null, "capabilities": null, "line": null})
    );
    for d in OPENAI {
        let o = json_of(&dispatch::encode_model(d, &bare).unwrap());
        assert_eq!(
            o,
            json!({"id": "m-1", "object": "model", "created": 0, "owned_by": "kaeso", "shutdown_date": null})
        );
    }

    let mut full = ModelInfo::new("m-2");
    full.display_name = Some("Model Two".into());
    full.created_at = Some(1_700_000_000);
    full.owned_by = Some("acme".into());
    full.lifecycle = Some(ModelLifecycle::Deprecated);
    full.deprecated_at = Some(1_750_000_000);
    full.retires_at = Some(1_792_713_600 + 3600);
    full.context_window = Some(200_000);
    full.max_output_tokens = Some(8_192);
    let a = json_of(&dispatch::encode_model(A, &full).unwrap());
    assert_eq!(a["display_name"], json!("Model Two"));
    assert_eq!(a["created_at"], json!("2023-11-14T22:13:20Z"));
    assert_eq!(a["lifecycle"], json!("deprecated"));
    assert_eq!(a["deprecated_at"], json!("2025-06-15T15:06:40Z"));
    assert_eq!(a["retires_at"], json!("2026-10-23T01:00:00Z"));
    assert_eq!(
        (&a["max_input_tokens"], &a["max_tokens"]),
        (&json!(200_000), &json!(8_192))
    );
    assert!(a.get("owned_by").is_none());
    let o = json_of(&dispatch::encode_model(DialectId::OpenAiChat, &full).unwrap());
    assert_eq!(
        o,
        json!({"id": "m-2", "object": "model", "created": 1_700_000_000, "owned_by": "acme",
               "shutdown_date": "2026-10-23", "name": "Model Two", "context_length": 200_000,
               "top_provider": {"context_length": 200_000, "max_completion_tokens": 8_192}}),
        "the time of day of retires_at is cut off"
    );

    let mut page = ModelList::new(vec![full]);
    page.has_more = true;
    let a = json_of(&dispatch::encode_models(A, &page).unwrap());
    assert_eq!(
        (&a["first_id"], &a["has_more"], &a["last_id"]),
        (&json!("m-2"), &json!(true), &json!("m-2"))
    );
    let o = json_of(&dispatch::encode_models(DialectId::OpenAiResponses, &page).unwrap());
    let mut keys: Vec<_> = o.as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(keys, ["data", "object"]);
}

#[test]
fn anthropic_list_reaches_openai_apps_and_back() {
    let a = list(A, &fixture("anthropic_messages", "list"));
    let o = json_of(&dispatch::encode_models(DialectId::OpenAiChat, &a).unwrap());
    assert_eq!(o["object"], json!("list"));
    let first = &o["data"][0];
    let limits = (&a.models[0].context_window, &a.models[0].max_output_tokens);
    assert_eq!(
        first,
        &json!({"id": "claude-opus-5", "object": "model", "created": 1_784_851_200,
                "owned_by": "kaeso", "shutdown_date": null, "name": "Claude Opus 5",
                "context_length": limits.0,
                "top_provider": {"context_length": limits.0, "max_completion_tokens": limits.1}}),
        "no Anthropic extras (capabilities, line) reach an OpenAI app"
    );
    assert_eq!(o["data"][3]["shutdown_date"], json!("2026-12-10"));

    let back = list(DialectId::OpenAiChat, &serde_json::to_vec(&o).unwrap());
    let again = json_of(&dispatch::encode_models(A, &back).unwrap());
    let m = &again["data"][3];
    assert_eq!(m["id"], json!("claude-sonnet-4-5-20250929"));
    assert_eq!(
        m["created_at"],
        json!("2025-09-29T00:00:00Z"),
        "same instant as received"
    );
    assert_eq!(m["retires_at"], json!("2026-12-10T00:00:00Z"));
    assert_eq!(
        m["display_name"],
        json!("claude-sonnet-4-5-20250929"),
        "OpenAI's `name` is written, not read: the id is the fallback"
    );
    assert_eq!(
        m["lifecycle"],
        json!("active"),
        "lifecycle lost in OpenAI, documented default"
    );
    assert_eq!(
        (&m["capabilities"], &m["line"]),
        (&Value::Null, &Value::Null)
    );
    assert_eq!(
        (&again["first_id"], &again["last_id"], &again["has_more"]),
        (
            &json!("claude-opus-5"),
            &json!("claude-sonnet-4-5-20250929"),
            &json!(false)
        )
    );
}

#[test]
fn openai_list_reaches_anthropic_apps() {
    let o = list(DialectId::OpenAiEmbeddings, &fixture("openai_chat", "list"));
    let a = json_of(&dispatch::encode_models(A, &o).unwrap());
    let m = &a["data"][2];
    assert_eq!(m["id"], json!("model-id-2"));
    assert_eq!(m["created_at"], json!("2023-06-16T17:03:22Z"));
    assert_eq!(m["retires_at"], json!("2026-10-23T00:00:00Z"));
    assert_eq!(m["display_name"], json!("model-id-2"));
    assert!(m.get("owned_by").is_none());
    assert_eq!(a["first_id"], json!("model-id-0"));
    assert_eq!(a["last_id"], json!("model-id-2"));

    let u = list(
        DialectId::OpenAiChat,
        &fixture("openai_chat", "unknown-fields"),
    );
    let a = json_of(&dispatch::encode_models(A, &u).unwrap());
    assert!(a.get("x_future_list_field").is_none());
    assert!(a["data"][0].get("x_future_field").is_none());
    assert_eq!(
        a["data"][1]["retires_at"],
        Value::Null,
        "OpenAI's `absent` marker is not Anthropic's"
    );
}

#[test]
fn openai_extras_are_shared_by_the_three_openai_codecs() {
    let body = fixture("openai_chat", "unknown-fields");
    let from_responses = list(DialectId::OpenAiResponses, &body);
    for d in OPENAI {
        let out = dispatch::encode_models(d, &from_responses).unwrap();
        assert_json_eq(&format!("{d}"), &body, &out);
    }
    let a = list(A, &fixture("anthropic_messages", "unknown-fields"));
    let o = json_of(&dispatch::encode_models(DialectId::OpenAiChat, &a).unwrap());
    assert!(o.get("x_future_list_field").is_none());
    assert!(o["data"][0].get("x_future_field").is_none());
}

#[test]
fn anthropic_query_parameters_decode() {
    let q = query(A, "limit=2&after_id=claude-opus-5");
    assert_eq!(
        (q.limit, q.after_id.as_deref(), q.before_id),
        (Some(2), Some("claude-opus-5"), None)
    );
    assert!(q.extras.is_empty());
    let q = query(A, "?before_id=claude%2Dhaiku+x&limit=1000");
    assert_eq!(
        (q.limit, q.before_id.as_deref()),
        (Some(1000), Some("claude-haiku x"))
    );
    let q = query(A, "lifecycle%5B%5D=active&lifecycle%5B%5D=retired&limit=20");
    assert_eq!(
        q.lifecycle,
        [ModelLifecycle::Active, ModelLifecycle::Retired]
    );
    let q = query(
        A,
        "lifecycle=deprecated&lifecycle[]=active&lifecycle=retired",
    );
    assert_eq!(q.lifecycle.len(), 3);
    let q = query(A, "");
    assert_eq!(q, ModelsQuery::default());
}

#[test]
fn unknown_query_parameters_become_dialect_bound_extras() {
    let q = query(A, "beta=true&x%20y=1&x%20y=2&limit=5");
    assert!(q.extras.is_from(A));
    assert_eq!(q.extras.fields["query.beta"].get(), "\"true\"");
    assert_eq!(
        q.extras.fields["query.x y"].to_value().unwrap(),
        json!(["1", "2"])
    );
    for d in OPENAI {
        let q = query(d, "limit=5&after=abc&flag");
        assert!(q.extras.is_from(d));
        assert_eq!(q.limit, None, "OpenAI documents no `limit`");
        assert_eq!(q.extras.fields["query.limit"].get(), "\"5\"");
        assert_eq!(q.extras.fields["query.after"].get(), "\"abc\"");
        assert_eq!(q.extras.fields["query.flag"].get(), "\"\"");
        assert!(query(d, "").extras.is_empty());
        assert_eq!(query_err(d, "a=%G0").kind, ErrorKind::InvalidRequest);
    }
}

#[test]
fn invalid_anthropic_queries_are_refused_naming_the_parameter() {
    let cases = [
        ("limit=0", "limit"),
        ("limit=1001", "limit"),
        ("limit=-1", "limit"),
        ("limit=abc", "limit"),
        ("limit=", "limit"),
        ("limit=2.5", "limit"),
        ("limit=99999999999", "limit"),
        ("limit=+5", "limit"),
        ("limit=%2B5", "limit"),
        ("limit= 5", "limit"),
        ("limit=1&limit=2", "limit"),
        ("lifecycle=sunset", "lifecycle"),
        ("lifecycle=active,deprecated", "lifecycle"),
        ("lifecycle[]=", "lifecycle"),
        (
            "lifecycle=active&lifecycle=active&lifecycle=deprecated&lifecycle=retired",
            "lifecycle",
        ),
        ("after_id=a&after_id=b", "after_id"),
        ("before_id=a&before_id=a", "before_id"),
        ("after_id=", "after_id"),
        ("before_id=", "before_id"),
        ("after_id=a&before_id=b", "before_id"),
    ];
    for (q, param) in cases {
        let e = query_err(A, q);
        assert_eq!(e.kind, ErrorKind::InvalidRequest, "{q}");
        assert_eq!(e.param.as_deref(), Some(param), "{q}");
        assert!(
            e.message.contains(&format!("`{param}`")),
            "{q}: {}",
            e.message
        );
    }
    let e = query_err(A, "limit=%ZZ");
    assert_eq!(e.kind, ErrorKind::InvalidRequest);
    assert!(e.message.contains("percent-encoding"), "{}", e.message);
}

#[test]
fn decoded_queries_page_a_decoded_list_end_to_end() {
    let all = list(A, &fixture("anthropic_messages", "list")).models;
    let q = query(A, "limit=2&after_id=claude-opus-5");
    let page = ModelList::page(&all, &q, Some(20)).unwrap();
    let out = dispatch::encode_models(A, &page).unwrap();
    assert_json_eq(
        "list-page",
        &fixture("anthropic_messages", "list-page"),
        &out,
    );

    let q = query(A, "limit=1&before_id=claude-haiku-4-5-20251001");
    let page = ModelList::page(&all, &q, Some(20)).unwrap();
    let out = json_of(&dispatch::encode_models(A, &page).unwrap());
    assert_eq!(
        (&out["first_id"], &out["last_id"], &out["has_more"]),
        (
            &json!("claude-sonnet-5"),
            &json!("claude-sonnet-5"),
            &json!(true)
        )
    );

    let q = query(A, "lifecycle[]=deprecated");
    let page = ModelList::page(&all, &q, Some(20)).unwrap();
    assert_eq!(page.models.len(), 1);
    assert_eq!(page.models[0].id, "claude-sonnet-4-5-20250929");

    let q = query(DialectId::OpenAiChat, "");
    let page = ModelList::page(&all, &q, None).unwrap();
    let out = json_of(&dispatch::encode_models(DialectId::OpenAiChat, &page).unwrap());
    assert_eq!(out["data"].as_array().unwrap().len(), 4);
}

#[test]
fn unknown_cursors_from_decoded_queries_are_refused() {
    let all = list(A, &fixture("anthropic_messages", "list")).models;
    for (q, param) in [
        ("after_id=claude-nonexistent", "after_id"),
        ("before_id=claude-nonexistent&limit=1", "before_id"),
    ] {
        let e = ModelList::page(&all, &query(A, q), Some(20)).unwrap_err();
        assert_eq!(
            (e.kind.clone(), e.param.as_deref()),
            (ErrorKind::InvalidRequest, Some(param))
        );
        let out = dispatch::encode_error(A, &e);
        assert_eq!(out.status, 400);
        assert_eq!(
            json_of(&out.body)["error"]["type"],
            json!("invalid_request_error")
        );
    }
}

#[test]
fn dates_past_year_9999_get_the_unknown_value() {
    let body = br#"{"object":"list","data":[{"id":"m","object":"model","created":1700000000000,"owned_by":"x"}]}"#;
    let o = list(DialectId::OpenAiChat, body);
    assert_eq!(o.models[0].created_at, Some(1_700_000_000_000));
    assert_json_eq(
        "same dialect",
        body,
        &dispatch::encode_models(DialectId::OpenAiChat, &o).unwrap(),
    );
    let a = json_of(&dispatch::encode_models(A, &o).unwrap());
    assert_eq!(a["data"][0]["created_at"], json!("1970-01-01T00:00:00Z"));
    let back = list(A, &serde_json::to_vec(&a).unwrap());
    assert_eq!(
        back.models[0].created_at,
        Some(0),
        "Anthropic's own decoder reads it"
    );

    let mut m = ModelInfo::new("far");
    m.created_at = Some(u64::MAX);
    m.deprecated_at = Some(u64::MAX);
    m.retires_at = Some(253_402_300_800);
    let a = json_of(&dispatch::encode_model(A, &m).unwrap());
    assert_eq!(
        (&a["created_at"], &a["deprecated_at"], &a["retires_at"]),
        (&json!("1970-01-01T00:00:00Z"), &Value::Null, &Value::Null)
    );
    for d in OPENAI {
        let o = json_of(&dispatch::encode_model(d, &m).unwrap());
        assert_eq!(
            (&o["created"], &o["shutdown_date"]),
            (&json!(u64::MAX), &Value::Null)
        );
    }
    let mut own = model(DialectId::OpenAiChat, &fixture("openai_chat", "retrieve"));
    own.retires_at = Some(u64::MAX);
    let o = json_of(&dispatch::encode_model(DialectId::OpenAiChat, &own).unwrap());
    assert_eq!(o["shutdown_date"], Value::Null);
    let mut own = model(A, &fixture("anthropic_messages", "retrieve"));
    own.created_at = Some(u64::MAX);
    own.retires_at = Some(u64::MAX);
    let a = json_of(&dispatch::encode_model(A, &own).unwrap());
    assert_eq!(
        (&a["created_at"], &a["retires_at"]),
        (&json!("1970-01-01T00:00:00Z"), &Value::Null)
    );
}

#[test]
fn non_2xx_statuses_decode_as_errors() {
    let not_found = std::fs::read(
        crate::test_support::fixtures_dir().join("anthropic_messages/errors/not-found.json"),
    )
    .unwrap();
    let e = dispatch::decode_models(A, 404, &h(), &not_found).unwrap_err();
    assert_eq!((e.kind, e.status), (ErrorKind::NotFound, Some(404)));
    let e = dispatch::decode_model(A, 404, &h(), &not_found).unwrap_err();
    assert_eq!(e.kind, ErrorKind::NotFound);
    let not_found = std::fs::read(
        crate::test_support::fixtures_dir().join("openai_chat/errors/model-not-found.json"),
    )
    .unwrap();
    for d in OPENAI {
        let e = dispatch::decode_models(d, 404, &h(), &not_found).unwrap_err();
        assert_eq!((e.kind, e.status), (ErrorKind::NotFound, Some(404)), "{d}");
        let e = dispatch::decode_model(d, 404, &h(), &not_found).unwrap_err();
        assert_eq!(e.kind, ErrorKind::NotFound, "{d}");
        assert!(dispatch::decode_model(d, 302, &h(), b"{}").is_err());
    }
    assert!(dispatch::decode_models(A, 500, &h(), b"oops").is_err());
}

#[test]
fn malformed_bodies_are_provider_errors() {
    let cases: [(DialectId, bool, Value, &str); 10] = [
        (A, true, json!({"type": "model"}), "id"),
        (A, true, json!({"id": "m"}), "type"),
        (
            A,
            true,
            json!({"id": "m", "type": "model", "created_at": "yesterday"}),
            "created_at",
        ),
        (
            A,
            true,
            json!({"id": "m", "type": "model", "deprecated_at": "2025-01-01"}),
            "deprecated_at",
        ),
        (
            A,
            true,
            json!({"id": "m", "type": "model", "retires_at": "2025-01-01T00:00:00"}),
            "retires_at",
        ),
        (
            A,
            true,
            json!({"id": "m", "type": "model", "max_tokens": -1}),
            "invalid",
        ),
        (A, false, json!({"first_id": null}), "data"),
        (
            DialectId::OpenAiChat,
            true,
            json!({"id": "m", "object": "model", "shutdown_date": "2026-10-23T00:00:00Z"}),
            "shutdown_date",
        ),
        (
            DialectId::OpenAiChat,
            true,
            json!({"id": "m", "object": "model", "created": "now"}),
            "invalid",
        ),
        (
            DialectId::OpenAiChat,
            false,
            json!({"object": "list"}),
            "data",
        ),
    ];
    for (d, single, v, needle) in cases {
        let body = serde_json::to_vec(&v).unwrap();
        let e = if single {
            dispatch::decode_model(d, 200, &h(), &body).unwrap_err()
        } else {
            dispatch::decode_models(d, 200, &h(), &body).unwrap_err()
        };
        assert_eq!(e.kind, ErrorKind::ProviderError, "{v}");
        assert!(e.message.contains(needle), "{v}: {}", e.message);
        assert!(e.message.contains("model listing"), "{}", e.message);
    }
    for d in [A, DialectId::OpenAiChat] {
        assert!(dispatch::decode_models(d, 200, &h(), b"not json").is_err());
        assert!(dispatch::decode_models(d, 200, &h(), br#"{"data":[],"data":[]}"#).is_err());
    }
}

#[test]
fn unknown_model_ids_answer_404_in_the_app_dialect() {
    let err = kaeso_kpp::KppError::new(ErrorKind::NotFound, "The model `x` does not exist.")
        .with_status(404);
    let a = dispatch::encode_error(A, &err);
    assert_eq!(a.status, 404);
    assert_eq!(json_of(&a.body)["error"]["type"], json!("not_found_error"));
    let o = dispatch::encode_error(DialectId::OpenAiChat, &err);
    assert_eq!(o.status, 404);
    assert!(json_of(&o.body)["error"]["message"].is_string());
}

#[test]
fn a_model_is_retrieved_with_a_get_on_its_own_path() {
    let anthropic = dispatch::encode_model_request(A, "claude-sonnet-4-5").unwrap();
    assert_eq!(anthropic.method, "GET");
    assert_eq!(anthropic.path, "/v1/models/claude-sonnet-4-5");
    assert_eq!(
        anthropic.headers.get("anthropic-version"),
        Some("2023-06-01")
    );
    assert!(anthropic.body.is_empty());
    for d in OPENAI {
        let openai = dispatch::encode_model_request(d, "ft:gpt-4o-mini:org::abc").unwrap();
        assert_eq!(openai.method, "GET");
        assert_eq!(openai.path, "/v1/models/ft:gpt-4o-mini:org::abc");
        assert!(openai.headers.is_empty(), "{d}");
        assert!(openai.body.is_empty());
    }
}

#[test]
fn a_retrieved_model_id_stays_one_path_segment() {
    let req = dispatch::encode_model_request(A, "a/b c?d#e%f").unwrap();
    assert_eq!(req.path, "/v1/models/a%2Fb%20c%3Fd%23e%25f");
    let req = dispatch::encode_model_request(OPENAI[0], "modèle").unwrap();
    assert_eq!(req.path, "/v1/models/mod%C3%A8le");
    for (id, path) in [
        ("a/../../x", "/v1/models/a%2F..%2F..%2Fx"),
        ("x?y#z", "/v1/models/x%3Fy%23z"),
        ("...", "/v1/models/..."),
        ("v1.5", "/v1/models/v1.5"),
    ] {
        let req = dispatch::encode_model_request(DialectId::OpenAiResponses, id).unwrap();
        assert_eq!(req.path, path);
    }
}

#[test]
fn a_model_id_a_url_would_resolve_away_is_refused() {
    for dialect in [A, OPENAI[0]] {
        for id in [".", ".."] {
            let err = dispatch::encode_model_request(dialect, id).unwrap_err();
            assert_eq!(err.kind, ErrorKind::InvalidRequest);
            assert_eq!(err.param.as_deref(), Some("model"));
            assert_eq!(
                err.message,
                format!("The model id `{id}` can't be part of a URL; use the model's real id.")
            );
        }
    }
}

#[test]
fn a_model_listing_is_requested_with_a_get() {
    let anthropic = dispatch::encode_models_request(A, &ModelsQuery::default()).unwrap();
    assert_eq!(
        (anthropic.method, anthropic.path.as_str()),
        ("GET", "/v1/models")
    );
    assert_eq!(
        anthropic.headers.get("anthropic-version"),
        Some("2023-06-01")
    );
    assert!(anthropic.body.is_empty());
    for d in OPENAI {
        let openai = dispatch::encode_models_request(d, &ModelsQuery::default()).unwrap();
        assert_eq!((openai.method, openai.path.as_str()), ("GET", "/v1/models"));
        assert!(openai.headers.is_empty(), "{d}");
        assert!(openai.body.is_empty());
    }
}

#[test]
fn an_anthropic_listing_request_pages_with_its_query_parameters() {
    let mut query = ModelsQuery::default();
    query.limit = Some(1000);
    query.after_id = Some("claude haiku/4&5".to_owned());
    query.lifecycle = vec![ModelLifecycle::Active, ModelLifecycle::Deprecated];
    let req = dispatch::encode_models_request(A, &query).unwrap();
    assert_eq!(
        req.path,
        "/v1/models?limit=1000&after_id=claude%20haiku%2F4%265&lifecycle%5B%5D=active&lifecycle%5B%5D=deprecated"
    );
    let decoded = dispatch::decode_models_query(A, req.path.split_once('?').unwrap().1).unwrap();
    assert_eq!(
        (decoded.limit, decoded.after_id, decoded.lifecycle),
        (query.limit, query.after_id, query.lifecycle)
    );
    let mut before = ModelsQuery::default();
    before.before_id = Some("claude-opus-5".to_owned());
    assert_eq!(
        dispatch::encode_models_request(A, &before).unwrap().path,
        "/v1/models?before_id=claude-opus-5"
    );
}

#[test]
fn a_listing_request_refuses_what_the_provider_cannot_take() {
    let unknown = dispatch::decode_models_query(A, "x_future=1").unwrap();
    let e = dispatch::encode_models_request(A, &unknown).unwrap_err();
    assert_eq!(e.unsupported_feature(), Some("query parameters"));
    assert!(e.message.contains("Anthropic"), "{}", e.message);
    let mut paged = ModelsQuery::default();
    paged.after_id = Some("gpt-4o".to_owned());
    let mut limited = ModelsQuery::default();
    limited.limit = Some(5);
    let mut before = ModelsQuery::default();
    before.before_id = Some("gpt-4o".to_owned());
    let mut active = ModelsQuery::default();
    active.lifecycle = vec![ModelLifecycle::Active];
    let extra = dispatch::decode_models_query(OPENAI[0], "x_future=1").unwrap();
    for d in OPENAI {
        for (query, param) in [
            (&limited, "limit"),
            (&paged, "after_id"),
            (&before, "before_id"),
            (&active, "lifecycle"),
            (&extra, "query parameters"),
        ] {
            let e = dispatch::encode_models_request(d, query).unwrap_err();
            assert_eq!(e.unsupported_feature(), Some(param), "{d}");
            assert!(
                e.message.ends_with("send the listing without it."),
                "{}",
                e.message
            );
        }
    }
}

#[test]
fn an_ollama_cloud_listing_keeps_tagged_ids_and_its_codex_catalogue() {
    let body = fixture("openai_chat", "providers/ollama-cloud/list-recorded");
    let l = list(DialectId::OpenAiChat, &body);
    let ids: Vec<&str> = l.models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(
        ids,
        ["deepseek-v4.1-flash:1017", "gemma4:31b", "gpt-oss:20b"]
    );
    assert!(
        l.models
            .iter()
            .all(|m| m.owned_by.as_deref() == Some("ollama"))
    );
    assert!(!l.has_more);
    assert_json_eq(
        "providers/ollama-cloud/list-recorded",
        &body,
        &dispatch::encode_models(DialectId::OpenAiChat, &l).unwrap(),
    );
    let one = model(
        DialectId::OpenAiChat,
        &fixture("openai_chat", "providers/ollama-cloud/retrieve-recorded"),
    );
    assert_eq!(one.id, "gpt-oss:20b");
    let missing = fixtures("openai_chat", "errors", "json")
        .into_iter()
        .find(|(n, _)| n == "providers/ollama-cloud/retrieve-not-found")
        .unwrap()
        .1;
    let e = dispatch::decode_model(DialectId::OpenAiChat, 404, &h(), &missing).unwrap_err();
    assert_eq!(e.kind, ErrorKind::NotFound);
    assert!(e.message.contains("no-such-model:1b"), "{}", e.message);
}

#[test]
fn a_moonshot_listing_keeps_its_model_details_and_refuses_a_wrong_key() {
    let body = fixture("openai_chat", "providers/moonshotai/list-recorded");
    let l = list(DialectId::OpenAiChat, &body);
    assert!(!l.has_more);
    let mut ids: Vec<&str> = l.models.iter().map(|m| m.id.as_str()).collect();
    ids.sort_unstable();
    assert_eq!(
        ids,
        [
            "kimi-k2.6",
            "kimi-k2.7-code",
            "kimi-k2.7-code-highspeed",
            "kimi-k3",
            "kimi-k3.1-pro"
        ]
    );
    assert!(
        l.models
            .iter()
            .all(|m| m.owned_by.as_deref() == Some("moonshot"))
    );
    assert_json_eq(
        "providers/moonshotai/list-recorded",
        &body,
        &dispatch::encode_models(DialectId::OpenAiChat, &l).unwrap(),
    );
    let one = model(
        DialectId::OpenAiChat,
        &fixture("openai_chat", "providers/moonshotai/retrieve-recorded"),
    );
    assert_eq!(one.id, "kimi-k2.6");
    let errors = fixtures("openai_chat", "errors", "json");
    let error = |name: &str| {
        errors
            .iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("no error fixture {name}"))
            .1
            .clone()
    };
    let e = dispatch::decode_model(
        DialectId::OpenAiChat,
        404,
        &h(),
        &error("providers/moonshotai/retrieve-not-found"),
    )
    .unwrap_err();
    assert_eq!(e.kind, ErrorKind::NotFound);
    let e = dispatch::decode_models(
        DialectId::OpenAiChat,
        401,
        &h(),
        &error("providers/moonshotai/list-invalid-api-key"),
    )
    .unwrap_err();
    assert_eq!(e.kind, ErrorKind::Authentication);
    assert_eq!(e.message, "Invalid Authentication");
}

#[test]
fn an_alibaba_listing_decodes_with_its_paging_ids_and_refuses_a_wrong_key() {
    let body = fixture("openai_chat", "providers/alibaba/list-recorded");
    let l = list(DialectId::OpenAiChat, &body);
    assert!(!l.has_more);
    assert_eq!(l.models.len(), 102);
    assert!(
        l.models
            .iter()
            .all(|m| m.owned_by.as_deref() == Some("system"))
    );
    for id in [
        "qwen3.7-flash",
        "qwen3-vl-plus",
        "text-embedding-v4",
        "kimi-k3",
    ] {
        assert!(l.models.iter().any(|m| m.id == id), "{id}");
    }
    assert_json_eq(
        "providers/alibaba/list-recorded",
        &body,
        &dispatch::encode_models(DialectId::OpenAiChat, &l).unwrap(),
    );
    let one = model(
        DialectId::OpenAiChat,
        &fixture("openai_chat", "providers/alibaba/retrieve-recorded"),
    );
    assert_eq!(one.id, "qwen3.7-flash");
    let errors = fixtures("openai_chat", "errors", "json");
    let error = |name: &str| {
        errors
            .iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("no error fixture {name}"))
            .1
            .clone()
    };
    let e = dispatch::decode_model(
        DialectId::OpenAiChat,
        404,
        &h(),
        &error("providers/alibaba/retrieve-not-found"),
    )
    .unwrap_err();
    assert_eq!(e.kind, ErrorKind::NotFound);
    assert!(
        e.message.contains("kaeso-example-no-such-model"),
        "{}",
        e.message
    );
    let e = dispatch::decode_models(
        DialectId::OpenAiChat,
        401,
        &h(),
        &error("providers/alibaba/list-invalid-api-key"),
    )
    .unwrap_err();
    assert_eq!(e.kind, ErrorKind::Authentication);
}

#[test]
fn a_cerebras_listing_decodes_and_its_errors_keep_cerebras_message_without_an_error_object() {
    let body = fixture("openai_chat", "providers/cerebras/list-recorded");
    let l = list(DialectId::OpenAiChat, &body);
    assert!(!l.has_more);
    let ids: Vec<&str> = l.models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, ["gpt-oss-120b", "qwen-3.8-27b"]);
    assert!(
        l.models
            .iter()
            .all(|m| m.owned_by.as_deref() == Some("Cerebras"))
    );
    assert_json_eq(
        "providers/cerebras/list-recorded",
        &body,
        &dispatch::encode_models(DialectId::OpenAiChat, &l).unwrap(),
    );
    let one = model(
        DialectId::OpenAiChat,
        &fixture("openai_chat", "providers/cerebras/retrieve-recorded"),
    );
    assert_eq!(one.id, "gpt-oss-120b");
    let errors = fixtures("openai_chat", "errors", "json");
    let error = |name: &str| {
        errors
            .iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("no error fixture {name}"))
            .1
            .clone()
    };
    let e = dispatch::decode_model(
        DialectId::OpenAiChat,
        404,
        &h(),
        &error("providers/cerebras/retrieve-not-found"),
    )
    .unwrap_err();
    assert_eq!(e.kind, ErrorKind::NotFound);
    assert_eq!(
        e.message,
        "Model does not exist or you do not have access to it."
    );
    assert_eq!(e.param.as_deref(), Some("model"));
    let e = dispatch::decode_models(
        DialectId::OpenAiChat,
        401,
        &h(),
        &error("providers/cerebras/list-invalid-api-key"),
    )
    .unwrap_err();
    assert_eq!(e.kind, ErrorKind::Authentication);
    assert_eq!(e.message, "Wrong API Key");
}

#[test]
fn a_deepinfra_listing_decodes_with_its_metadata_and_its_refusals_have_their_kinds() {
    let body = fixture("openai_chat", "providers/deepinfra/list-recorded");
    let l = list(DialectId::OpenAiChat, &body);
    let ids: Vec<&str> = l.models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "BAAI/bge-m3",
            "openai/gpt-oss-120b",
            "deepseek-ai/DeepSeek-V4-Flash-0731"
        ]
    );
    assert_json_eq(
        "providers/deepinfra/list-recorded",
        &body,
        &dispatch::encode_models(DialectId::OpenAiChat, &l).unwrap(),
    );
    let errors = fixtures("openai_chat", "errors", "json");
    let error = |name: &str| {
        errors
            .iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("no error fixture {name}"))
            .1
            .clone()
    };
    let e = dispatch::decode_models(
        DialectId::OpenAiChat,
        401,
        &h(),
        &error("providers/deepinfra/recorded-invalid-api-key"),
    )
    .unwrap_err();
    assert_eq!(e.kind, ErrorKind::Authentication);
    assert_eq!(e.message, "User is not authorized to access this resource");
    let e = dispatch::decode_models(
        DialectId::OpenAiChat,
        403,
        &h(),
        &error("providers/deepinfra/missing-scope-doc"),
    )
    .unwrap_err();
    assert_eq!(
        (e.kind, e.message.as_str()),
        (
            ErrorKind::PermissionDenied,
            "The provider returned an error (HTTP 403): this API key does not have the 'inference' scope."
        )
    );
}

#[test]
fn a_novita_ai_listing_decodes_with_its_prices_and_its_refusals_and_unknown_models_have_their_kinds()
 {
    let body = fixture("openai_chat", "providers/novita-ai/list-recorded");
    let l = list(DialectId::OpenAiChat, &body);
    let ids: Vec<&str> = l.models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "tencent/hy4",
            "deepseek/deepseek-v4.1-flash",
            "meta-llama/llama-3.1-8b-instruct"
        ]
    );
    assert_json_eq(
        "providers/novita-ai/list-recorded",
        &body,
        &dispatch::encode_models(DialectId::OpenAiChat, &l).unwrap(),
    );
    let one = fixture(
        "openai_chat",
        "providers/novita-ai/retrieve-embedding-recorded",
    );
    let embedding = model(DialectId::OpenAiChat, &one);
    assert_eq!(embedding.id, "baai/bge-m3");
    assert_json_eq(
        "providers/novita-ai/retrieve-embedding-recorded",
        &one,
        &dispatch::encode_model(DialectId::OpenAiChat, &embedding).unwrap(),
    );
    let errors = fixtures("openai_chat", "errors", "json");
    let error = |name: &str| {
        errors
            .iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("no error fixture {name}"))
            .1
            .clone()
    };
    let e = dispatch::decode_models(
        DialectId::OpenAiChat,
        401,
        &h(),
        &error("providers/novita-ai/recorded-invalid-api-key"),
    )
    .unwrap_err();
    assert_eq!(
        (e.kind, e.message.as_str()),
        (
            ErrorKind::Authentication,
            "The provider returned an error (HTTP 401): failed to authenticate API key."
        )
    );
    let e = dispatch::decode_model(
        DialectId::OpenAiChat,
        404,
        &h(),
        &error("providers/novita-ai/recorded-model-not-found"),
    )
    .unwrap_err();
    assert_eq!(e.kind, ErrorKind::NotFound);
}

#[test]
fn a_volcengine_listing_keeps_its_own_fields_and_its_key_refusals_are_authentication_errors() {
    let body = fixture("openai_chat", "providers/volcengine/list-doc");
    let l = list(DialectId::OpenAiChat, &body);
    let ids: Vec<&str> = l.models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, ["doubao-seed-2-1-pro-260915"]);
    assert_eq!(l.models[0].created_at, Some(1789472816));
    assert_eq!(l.models[0].owned_by, None);
    let extras = serde_json::to_string(&l.models[0].extras).unwrap();
    for field in ["token_limits", "task_type", "features", "max_output_tokens"] {
        assert!(extras.contains(field), "{field}: {extras}");
    }
    assert_json_eq(
        "providers/volcengine/list-doc",
        &body,
        &dispatch::encode_models(DialectId::OpenAiChat, &l).unwrap(),
    );
    let errors = fixtures("openai_chat", "errors", "json");
    for (name, message) in [
        (
            "providers/volcengine/recorded-invalid-api-key",
            "The API key format is incorrect. Request id: 00000000000000000000000000000000000000000000000000000",
        ),
        (
            "providers/volcengine/recorded-unknown-api-key",
            "The API key doesn't exist. Request id: 00000000000000000000000000000000000000000000000000000",
        ),
    ] {
        let body = &errors.iter().find(|(n, _)| n == name).unwrap().1;
        let e = dispatch::decode_models(DialectId::OpenAiChat, 401, &h(), body).unwrap_err();
        assert_eq!(
            (e.kind, e.message.as_str()),
            (ErrorKind::Authentication, message)
        );
    }
}

#[test]
fn an_ovhcloud_listing_decodes_every_model_kind_and_its_wrong_key_and_unknown_model_are_errors() {
    let body = fixture("openai_chat", "providers/ovhcloud/list-recorded");
    let l = list(DialectId::OpenAiChat, &body);
    let ids: Vec<&str> = l.models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(
        ids,
        ["whisper-large-v3", "bge-m3", "gpt-oss-20b", "Qwen3.5-9B"]
    );
    assert_json_eq(
        "providers/ovhcloud/list-recorded",
        &body,
        &dispatch::encode_models(DialectId::OpenAiChat, &l).unwrap(),
    );
    let one = fixture("openai_chat", "providers/ovhcloud/retrieve-recorded");
    assert_json_eq(
        "providers/ovhcloud/retrieve-recorded",
        &one,
        &dispatch::encode_model(DialectId::OpenAiChat, &model(DialectId::OpenAiChat, &one))
            .unwrap(),
    );
    let errors = fixtures("openai_chat", "errors", "json");
    let error = |name: &str| {
        errors
            .iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("no error fixture {name}"))
            .1
            .clone()
    };
    let e = dispatch::decode_models(
        DialectId::OpenAiChat,
        403,
        &h(),
        &error("providers/ovhcloud/recorded-invalid-api-key"),
    )
    .unwrap_err();
    assert_eq!(
        (e.kind, e.message.as_str()),
        (
            ErrorKind::PermissionDenied,
            "The provider returned an error (HTTP 403): Forbidden: authentication failed.  Please generate a new one at https://kepler.ai.cloud.ovh.net/v1/oauth/ovh/authorize?iam_action=publicCloudProject:ai:endpoints/call."
        )
    );
    let e = dispatch::decode_model(
        DialectId::OpenAiChat,
        404,
        &h(),
        &error("providers/ovhcloud/recorded-unknown-model"),
    )
    .unwrap_err();
    assert_eq!(e.kind, ErrorKind::NotFound);
}

#[test]
fn a_tencent_tokenhub_listing_decodes_with_its_names_and_statuses_and_a_missing_key_is_refused() {
    let body = fixture("openai_chat", "providers/tencent-tokenhub/list-doc");
    let l = list(DialectId::OpenAiChat, &body);
    let ids: Vec<&str> = l.models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, ["hy4", "deepseek-v4-pro"]);
    assert_json_eq(
        "providers/tencent-tokenhub/list-doc",
        &body,
        &dispatch::encode_models(DialectId::OpenAiChat, &l).unwrap(),
    );
    let errors = fixtures("openai_chat", "errors", "json");
    let error = |name: &str| {
        errors
            .iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("no error fixture {name}"))
            .1
            .clone()
    };
    let e = dispatch::decode_models(
        DialectId::OpenAiChat,
        401,
        &h(),
        &error("providers/tencent-tokenhub/recorded-models-missing-api-key"),
    )
    .unwrap_err();
    assert_eq!(e.kind, ErrorKind::Authentication);
    assert!(
        e.message
            .starts_with("Authentication information is missing")
    );
}

#[test]
fn a_venice_listing_decodes_with_its_model_spec_and_its_string_errors_have_their_kinds() {
    let body = fixture("openai_chat", "providers/venice/list-recorded");
    let l = list(DialectId::OpenAiChat, &body);
    let ids: Vec<&str> = l.models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(
        ids,
        ["zai-org-glm-5-1", "venice-uncensored-1-2", "e2ee-glm-5-3-p"]
    );
    assert_json_eq(
        "providers/venice/list-recorded",
        &body,
        &dispatch::encode_models(DialectId::OpenAiChat, &l).unwrap(),
    );
    let errors = fixtures("openai_chat", "errors", "json");
    let error = |name: &str| {
        errors
            .iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("no error fixture {name}"))
            .1
            .clone()
    };
    let e = dispatch::decode_models(
        DialectId::OpenAiChat,
        401,
        &h(),
        &error("providers/venice/recorded-invalid-api-key"),
    )
    .unwrap_err();
    assert_eq!(
        (e.kind, e.message.as_str()),
        (ErrorKind::Authentication, "Authentication failed")
    );
}

#[test]
fn a_sambanova_listing_decodes_with_its_prices_and_its_wrong_keys_and_unknown_models_are_errors() {
    let body = fixture("openai_chat", "providers/sambanova/list-recorded");
    let l = list(DialectId::OpenAiChat, &body);
    let ids: Vec<&str> = l.models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "GLM-5.3",
            "Meta-Llama-3.3-70B-Instruct",
            "MiniMax-M3.1-Flash",
            "gemma-4-31B-it",
            "gpt-oss-120b"
        ]
    );
    assert_json_eq(
        "providers/sambanova/list-recorded",
        &body,
        &dispatch::encode_models(DialectId::OpenAiChat, &l).unwrap(),
    );
    let one = fixture("openai_chat", "providers/sambanova/retrieve-recorded");
    assert_json_eq(
        "providers/sambanova/retrieve-recorded",
        &one,
        &dispatch::encode_model(DialectId::OpenAiChat, &model(DialectId::OpenAiChat, &one))
            .unwrap(),
    );
    let errors = fixtures("openai_chat", "errors", "json");
    let error = |name: &str| {
        errors
            .iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("no error fixture {name}"))
            .1
            .clone()
    };
    let e = dispatch::decode_models(
        DialectId::OpenAiChat,
        401,
        &h(),
        &error("providers/sambanova/recorded-invalid-api-key"),
    )
    .unwrap_err();
    assert_eq!(e.kind, ErrorKind::Authentication);
    let e = dispatch::decode_model(
        DialectId::OpenAiChat,
        404,
        &h(),
        &error("providers/sambanova/recorded-embeddings-model-not-found"),
    )
    .unwrap_err();
    assert_eq!(
        (e.kind, e.param.as_deref()),
        (ErrorKind::NotFound, Some("model"))
    );
}

#[test]
fn a_standard_compute_listing_decodes_with_its_router_and_regional_models_and_its_errors_have_their_kinds()
 {
    let body = fixture("openai_chat", "providers/standardcompute/list-recorded");
    let l = list(DialectId::OpenAiChat, &body);
    let ids: Vec<&str> = l.models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids.len(), 74);
    assert_eq!(ids.first(), Some(&"StandardCompute"));
    for id in [
        "openai/gpt-5.6-luna:eu",
        "anthropic/claude-haiku-5-5",
        "z-ai/glm-5.3:us",
    ] {
        assert!(ids.contains(&id), "{id}");
    }
    assert_json_eq(
        "providers/standardcompute/list-recorded",
        &body,
        &dispatch::encode_models(DialectId::OpenAiChat, &l).unwrap(),
    );
    let errors = fixtures("openai_chat", "errors", "json");
    let error = |name: &str| {
        errors
            .iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("no error fixture {name}"))
            .1
            .clone()
    };
    let e = dispatch::decode_model(
        DialectId::OpenAiChat,
        404,
        &h(),
        &error("providers/standardcompute/recorded-retrieve-not-found"),
    )
    .unwrap_err();
    assert_eq!(e.kind, ErrorKind::NotFound);
    let e = dispatch::decode_models(
        DialectId::OpenAiChat,
        401,
        &h(),
        &error("providers/standardcompute/recorded-invalid-api-key"),
    )
    .unwrap_err();
    assert_eq!(e.kind, ErrorKind::Authentication);
    let kind = |status: u16, name: &str| {
        let e = dispatch::decode_error(
            DialectId::OpenAiChat,
            status,
            &h(),
            &error(&format!("providers/standardcompute/{name}")),
        );
        (e.kind, e.param)
    };
    assert_eq!(
        kind(402, "budget-exhausted-doc"),
        (ErrorKind::Billing, None)
    );
    assert_eq!(
        kind(400, "recorded-unknown-model"),
        (ErrorKind::InvalidRequest, Some("model".to_owned()))
    );
    assert_eq!(
        kind(400, "recorded-image-to-text-model"),
        (ErrorKind::InvalidRequest, Some("model".to_owned()))
    );
    assert_eq!(
        kind(400, "recorded-image-base64-detail"),
        (ErrorKind::InvalidRequest, None)
    );
}

#[test]
fn a_hugging_face_listing_decodes_with_its_providers_and_a_refused_token_is_an_error() {
    let body = fixture("openai_chat", "providers/huggingface/list-recorded");
    let l = list(DialectId::OpenAiChat, &body);
    let ids: Vec<&str> = l.models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "Qwen/Qwen3.8-27B",
            "deepseek-ai/DeepSeek-V4.1-Flash-1017",
            "openai/gpt-oss-120b"
        ]
    );
    assert_json_eq(
        "providers/huggingface/list-recorded",
        &body,
        &dispatch::encode_models(DialectId::OpenAiChat, &l).unwrap(),
    );
    let (_, error) = fixtures("openai_chat", "errors", "json")
        .into_iter()
        .find(|(n, _)| n == "providers/huggingface/recorded-invalid-api-key")
        .unwrap();
    let e = dispatch::decode_models(DialectId::OpenAiChat, 401, &h(), &error).unwrap_err();
    assert_eq!(
        (e.kind, e.message.as_str()),
        (ErrorKind::Authentication, "Invalid username or password.")
    );
}

#[test]
fn an_nvidia_listing_decodes_and_its_problem_documents_keep_nvidias_detail() {
    let body = fixture("openai_chat", "providers/nvidia/list-recorded");
    let l = list(DialectId::OpenAiChat, &body);
    let ids: Vec<&str> = l.models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "nvidia/nemotron-3-super-120b-a12b",
            "nvidia/nemotron-3.5-embed-1b",
            "openai/gpt-oss-20b",
            "z-ai/glm-5.3-flash"
        ]
    );
    assert_json_eq(
        "providers/nvidia/list-recorded",
        &body,
        &dispatch::encode_models(DialectId::OpenAiChat, &l).unwrap(),
    );
    let errors = fixtures("openai_chat", "errors", "json");
    let error = |name: &str| {
        errors
            .iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("no error fixture {name}"))
            .1
            .clone()
    };
    let e = dispatch::decode_models(
        DialectId::OpenAiChat,
        403,
        &h(),
        &error("providers/nvidia/recorded-invalid-api-key"),
    )
    .unwrap_err();
    assert_eq!(
        (e.kind, e.message.as_str()),
        (
            ErrorKind::PermissionDenied,
            "The provider returned an error (HTTP 403): Authorization failed."
        )
    );
    let e = dispatch::decode_error(
        DialectId::OpenAiChat,
        410,
        &h(),
        &error("providers/nvidia/recorded-retired-model"),
    );
    assert_eq!(e.kind, ErrorKind::NotFound);
    assert!(
        e.message.contains("has reached its end of life"),
        "{}",
        e.message
    );
}

#[test]
fn a_stepfun_listing_mixes_chat_and_speech_models_and_a_wrong_key_is_an_authentication_error() {
    let body = fixture("openai_chat", "providers/stepfun-ai/list-doc");
    let l = list(DialectId::OpenAiChat, &body);
    let ids: Vec<&str> = l.models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, ["step-5-preview", "stepaudio-2.5-tts"]);
    round_trip(
        DialectId::OpenAiChat,
        "providers/stepfun-ai/list-doc",
        &body,
    );
    let body = fixture("openai_chat", "providers/stepfun-ai/retrieve-doc");
    assert_eq!(model(DialectId::OpenAiChat, &body).id, "step-5-preview");
    round_trip(
        DialectId::OpenAiChat,
        "providers/stepfun-ai/retrieve-doc",
        &body,
    );
    let refused = fixtures("openai_chat", "errors", "json")
        .into_iter()
        .find(|(n, _)| n == "providers/stepfun-ai/recorded-invalid-api-key")
        .unwrap()
        .1;
    let e = dispatch::decode_models(DialectId::OpenAiChat, 401, &h(), &refused).unwrap_err();
    assert_eq!(
        (e.kind, e.message.as_str()),
        (ErrorKind::Authentication, "Incorrect API key provided")
    );
}

#[test]
fn a_jina_listing_and_model_decode_from_openrouters_shape_with_prefixed_ids() {
    let jina = |name: &str| {
        let dir = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/fixtures/openai_embeddings/providers/jina/models"
        );
        std::fs::read(format!("{dir}/{name}.json")).unwrap()
    };
    let body = jina("list-recorded");
    let l = list(DialectId::OpenAiEmbeddings, &body);
    let ids: Vec<&str> = l.models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "jina-ai/jina-reranker-v3.5",
            "jina-ai/jina-embeddings-v3",
            "jina-ai/jina-embeddings-v5-text-small"
        ]
    );
    round_trip(
        DialectId::OpenAiEmbeddings,
        "providers/jina/list-recorded",
        &body,
    );
    let body = jina("retrieve-recorded");
    assert_eq!(
        model(DialectId::OpenAiEmbeddings, &body).id,
        "jina-ai/jina-embeddings-v5-text-small"
    );
    round_trip(
        DialectId::OpenAiEmbeddings,
        "providers/jina/retrieve-recorded",
        &body,
    );
}

#[test]
fn a_qianfan_listing_decodes_and_its_codes_give_their_kinds() {
    let body = fixture("openai_chat", "providers/qianfan/list-doc");
    let l = list(DialectId::OpenAiChat, &body);
    assert_eq!(l.models[0].id, "deepseek-v3.2");
    assert_json_eq(
        "providers/qianfan/list-doc",
        &body,
        &dispatch::encode_models(DialectId::OpenAiChat, &l).unwrap(),
    );
    for (status, code, kind) in [
        (400, "tokens_too_long", ErrorKind::ContextTooLong),
        (400, "characters_too_long", ErrorKind::ContextTooLong),
        (403, "account_overdue", ErrorKind::Billing),
        (403, "model_offline", ErrorKind::NotFound),
        (401, "invalid_iam_token", ErrorKind::Authentication),
    ] {
        let body = format!(
            r#"{{"error":{{"code":"{code}","message":"m","type":"invalid_request_error"}},"id":"as-0"}}"#
        );
        let e = dispatch::decode_models(DialectId::OpenAiChat, status, &h(), body.as_bytes())
            .unwrap_err();
        assert_eq!(e.kind, kind, "{code}");
    }
}

#[test]
fn the_catalogue_description_reaches_openai_apps_in_openrouters_fields() {
    let mut m = ModelInfo::new("grok-4.7");
    m.display_name = Some("Grok 4.7".into());
    m.context_window = Some(2_000_000);
    m.max_output_tokens = Some(64_000);
    m.input_modalities = Some(vec!["text".into(), "image".into()]);
    m.output_modalities = Some(vec!["text".into()]);
    m.features = Some(vec![
        kaeso_kpp::ModelFeature::Tools,
        kaeso_kpp::ModelFeature::Reasoning,
        kaeso_kpp::ModelFeature::ReasoningEffort,
        kaeso_kpp::ModelFeature::StructuredOutput,
    ]);
    let mut prices = kaeso_kpp::ModelPrices::default();
    prices.input = Some("3.00".into());
    prices.output = Some("15".into());
    prices.cache_read = Some("0.075".into());
    prices.cache_write = Some("3.75".into());
    prices.reasoning = Some("15".into());
    m.prices = Some(prices);
    for d in OPENAI {
        let o = json_of(&dispatch::encode_model(d, &m).unwrap());
        assert_eq!(
            o,
            json!({"id": "grok-4.7", "object": "model", "created": 0, "owned_by": "kaeso",
                   "shutdown_date": null, "name": "Grok 4.7", "context_length": 2_000_000,
                   "architecture": {"modality": "text+image->text",
                                    "input_modalities": ["text", "image"],
                                    "output_modalities": ["text"]},
                   "pricing": {"prompt": "0.000003", "completion": "0.000015",
                               "input_cache_read": "0.000000075",
                               "input_cache_write": "0.00000375",
                               "internal_reasoning": "0.000015"},
                   "top_provider": {"context_length": 2_000_000, "max_completion_tokens": 64_000},
                   "supported_parameters": ["tools", "tool_choice", "reasoning", "reasoning_effort",
                                            "response_format", "structured_outputs"]}),
            "{d}"
        );
    }
    let mut none = ModelInfo::new("m");
    none.features = Some(Vec::new());
    let o = json_of(&dispatch::encode_model(DialectId::OpenAiChat, &none).unwrap());
    assert_eq!(
        o["supported_parameters"],
        json!([]),
        "known to take none of them"
    );

    // Every field Kaeso writes is one OpenRouter's recorded listing has, with the same type.
    fn same_shape(ours: &Value, theirs: &Value, at: &str) {
        match (ours, theirs) {
            (Value::Object(o), Value::Object(t)) => {
                for (k, v) in o {
                    let there = t
                        .get(k)
                        .unwrap_or_else(|| panic!("{at}.{k} is not OpenRouter's"));
                    same_shape(v, there, &format!("{at}.{k}"));
                }
            }
            (Value::Array(o), Value::Array(t)) => {
                if let (Some(a), Some(b)) = (o.first(), t.first()) {
                    same_shape(a, b, &format!("{at}[]"));
                }
            }
            (Value::Number(_), Value::Number(_))
            | (Value::String(_), Value::String(_))
            | (Value::Bool(_), Value::Bool(_)) => {}
            _ => panic!("{at}: {ours} is not of the type of OpenRouter's {theirs}"),
        }
    }
    let recorded = json_of(&fixture(
        "openai_chat",
        "providers/openrouter/list-recorded-prices",
    ));
    let theirs = &recorded["data"][0];
    let mut ours = json_of(&dispatch::encode_model(DialectId::OpenAiChat, &m).unwrap());
    for k in ["object", "owned_by", "shutdown_date"] {
        ours.as_object_mut().unwrap().remove(k);
    }
    same_shape(&ours, theirs, "model");
}

#[test]
fn prices_per_million_tokens_become_exact_prices_per_token() {
    for (per_million, per_token) in [
        ("3", "0.000003"),
        ("3.00", "0.000003"),
        ("0.075", "0.000000075"),
        ("15.5", "0.0000155"),
        ("0", "0"),
        ("0.000", "0"),
        ("1000000", "1"),
        ("2500000.5", "2.5000005"),
        (".5", "0.0000005"),
        ("12.", "0.000012"),
    ] {
        assert_eq!(
            crate::openai_models::per_token(per_million).as_deref(),
            Some(per_token),
            "{per_million}"
        );
    }
    for bad in ["", ".", "-1", "1e-6", "1.2.3", " 1", "x"] {
        assert_eq!(crate::openai_models::per_token(bad), None, "{bad:?}");
    }
}
