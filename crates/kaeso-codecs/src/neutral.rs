use kaeso_kpp::{DialectId, Modality};
use serde_json::Value;

use crate::adapt::Draft;
use crate::codec::EgressCtx;
use crate::cross::{self, Setting};
use crate::wire::VERBATIM_FORM_KEY;

const MODALITIES: &str = "modalities";

// Chat apps such as NextChat send OpenAI's sampling fields on every request at their default
// values. A value the API uses when the field is absent asks for nothing, so where the target
// or its provider would refuse the field it is left out and named instead.
pub(crate) fn left_out(d: &mut Draft<'_>, target: DialectId, ctx: &EgressCtx) {
    penalties(d, target, ctx);
    extras(d, target, ctx);
    if target == DialectId::OpenAiChat {
        text_only(d, ctx);
        refused_nulls(d, ctx);
    }
}

fn takes_penalty(target: DialectId, ctx: &EgressCtx, field: &str) -> bool {
    match target {
        DialectId::OpenAiChat => !ctx.chat_profile.request.refused_fields.contains(&field),
        DialectId::Gemini => true,
        _ => false,
    }
}

fn penalties(d: &mut Draft<'_>, target: DialectId, ctx: &EgressCtx) {
    for (setting, field) in [
        (Setting::FrequencyPenalty, "frequency_penalty"),
        (Setting::PresencePenalty, "presence_penalty"),
    ] {
        let s = &d.get().sampling;
        let value = match setting {
            Setting::FrequencyPenalty => s.frequency_penalty,
            _ => s.presence_penalty,
        };
        if !value.is_some_and(|v| v == 0.0) || takes_penalty(target, ctx, field) {
            continue;
        }
        let name = cross::app_field(d.get(), setting, target);
        let s = &mut d.edit().sampling;
        match setting {
            Setting::FrequencyPenalty => s.frequency_penalty = None,
            _ => s.presence_penalty = None,
        }
        d.leave_out(name.to_owned());
    }
}

fn refused_by_provider(target: DialectId, ctx: &EgressCtx, key: &str) -> bool {
    match target {
        DialectId::OpenAiChat => ctx.chat_profile.request.refused_fields.contains(&key),
        DialectId::AnthropicMessages => ctx.messages_profile.refused_fields.contains(&key),
        _ => false,
    }
}

fn extras(d: &mut Draft<'_>, target: DialectId, ctx: &EgressCtx) {
    let extras = &d.get().extras;
    let Some(origin) = extras.origin.as_ref().map(|o| o.dialect) else {
        return;
    };
    let own = origin == target;
    let neutral: Vec<String> = extras
        .fields
        .iter()
        .filter(|(key, value)| cross::is_neutral_value(origin, key, value))
        .filter(|(key, _)| !own || refused_by_provider(target, ctx, key))
        .map(|(key, _)| key.clone())
        .collect();
    if neutral.is_empty() {
        return;
    }
    let fields = &mut d.edit().extras.fields;
    for key in &neutral {
        fields.shift_remove(key);
    }
    for key in neutral {
        // The Responses encoder sends `store: false` with every request from another format.
        if !(target == DialectId::OpenAiResponses && key == "store") {
            d.leave_out(key);
        }
    }
}

fn text_only(d: &mut Draft<'_>, ctx: &EgressCtx) {
    let r = d.get();
    if r.output_modalities == [Modality::Text]
        && ctx
            .chat_profile
            .request
            .refused_fields
            .contains(&MODALITIES)
    {
        let name = cross::app_field(r, Setting::Modalities, DialectId::OpenAiChat);
        d.edit().output_modalities.clear();
        d.leave_out(name.to_owned());
    }
}

// A field sent as `null` or `[]` is form: other formats never see it, and a Chat provider that
// refuses the field doesn't get it either.
fn refused_nulls(d: &mut Draft<'_>, ctx: &EgressCtx) {
    let refused = ctx.chat_profile.request.refused_fields;
    let r = d.get();
    let sends_refused_null = r.extras.is_from(DialectId::OpenAiChat)
        && matches!(
            r.extras.form.get(VERBATIM_FORM_KEY),
            Some(Value::Object(nulls)) if nulls.keys().any(|k| refused.contains(&k.as_str()))
        );
    if !sends_refused_null {
        return;
    }
    let form = &mut d.edit().extras.form;
    if let Some(Value::Object(nulls)) = form.get_mut(VERBATIM_FORM_KEY) {
        nulls.retain(|key, _| !refused.contains(&key.as_str()));
        if nulls.is_empty() {
            form.shift_remove(VERBATIM_FORM_KEY);
        }
    }
}

#[cfg(test)]
mod tests {
    use kaeso_kpp::{DialectId, KppError, Request, RequestKind};
    use serde_json::{Value, json};

    use crate::anthropic_messages::MessagesProfile;
    use crate::codec::{EgressCtx, HeaderList};
    use crate::dispatch;
    use crate::openai_chat::ChatProfile;

    const A: DialectId = DialectId::AnthropicMessages;
    const B: DialectId = DialectId::BedrockConverse;
    const C: DialectId = DialectId::OpenAiChat;
    const G: DialectId = DialectId::Gemini;
    const R: DialectId = DialectId::OpenAiResponses;

    fn decoded(dialect: DialectId, body: Value) -> Request {
        dispatch::decode_request(
            dialect,
            RequestKind::Generate,
            body.to_string().as_bytes(),
            &HeaderList::new(),
        )
        .unwrap()
    }

    fn chat(fields: Value) -> Request {
        let mut body = json!({
            "model": "m",
            "max_completion_tokens": 100,
            "messages": [{"role": "user", "content": "Hi"}]
        });
        for (k, v) in fields.as_object().unwrap() {
            body[k] = v.clone();
        }
        decoded(C, body)
    }

    fn sent_with(
        target: DialectId,
        req: &Request,
        ctx: EgressCtx,
    ) -> Result<(Value, Vec<String>), KppError> {
        let mut ctx = ctx;
        dispatch::check(target, req, &ctx)?;
        let out = dispatch::encode_request(target, req, &mut ctx)?;
        Ok((serde_json::from_slice(&out.body).unwrap(), ctx.left_out))
    }

    fn sent(target: DialectId, req: &Request) -> Result<(Value, Vec<String>), KppError> {
        sent_with(target, req, EgressCtx::new(req))
    }

    fn to_chat_provider(req: &Request, refusing: &'static [&'static str]) -> (Value, Vec<String>) {
        let profile = ChatProfile::compatible("Example Cloud").refusing(refusing);
        sent_with(C, req, EgressCtx::new(req).with_chat_profile(profile)).unwrap()
    }

    #[test]
    fn penalties_of_zero_are_left_out_where_the_target_has_no_penalties() {
        let req = chat(json!({"frequency_penalty": 0, "presence_penalty": 0.0}));
        for target in [A, B, R] {
            let (body, left_out) = sent(target, &req).unwrap_or_else(|e| panic!("{target}: {e}"));
            assert_eq!(
                left_out,
                ["frequency_penalty", "presence_penalty"],
                "{target}"
            );
            let text = body.to_string();
            assert!(!text.contains("enalty"), "{target}: {text}");
        }
        let (body, left_out) = sent(G, &req).unwrap();
        assert!(left_out.is_empty(), "{left_out:?}");
        assert_eq!(body["generationConfig"]["frequencyPenalty"], json!(0.0));
        let (body, left_out) = sent(C, &req).unwrap();
        assert!(left_out.is_empty(), "{left_out:?}");
        assert_eq!(
            (&body["frequency_penalty"], &body["presence_penalty"]),
            (&json!(0), &json!(0.0)),
            "a Chat provider gets them as written"
        );
        let (body, left_out) = to_chat_provider(&req, &["frequency_penalty", "presence_penalty"]);
        assert_eq!(left_out, ["frequency_penalty", "presence_penalty"]);
        assert!(body.get("frequency_penalty").is_none(), "{body}");
    }

    #[test]
    fn other_penalties_are_still_refused() {
        let req = chat(json!({"frequency_penalty": 0, "presence_penalty": 0.5}));
        for target in [A, B, R] {
            let e = sent(target, &req).unwrap_err();
            assert_eq!(e.param.as_deref(), Some("presence_penalty"), "{target}");
        }
        let profile = ChatProfile::compatible("Example Cloud").refusing(&["presence_penalty"]);
        let e = sent_with(C, &req, EgressCtx::new(&req).with_chat_profile(profile)).unwrap_err();
        assert_eq!(e.param.as_deref(), Some("presence_penalty"));
    }

    #[test]
    fn a_gemini_apps_penalties_are_named_by_its_own_fields() {
        let req = decoded(
            G,
            json!({
                "contents": [{"role": "user", "parts": [{"text": "Hi"}]}],
                "generationConfig": {"maxOutputTokens": 100, "presencePenalty": 0}
            }),
        );
        let (_, left_out) = sent(A, &req).unwrap();
        assert_eq!(left_out, ["generationConfig.presencePenalty"]);

        let mut text_only = decoded(
            G,
            json!({
                "contents": [{"role": "user", "parts": [{"text": "Hi"}]}],
                "generationConfig": {"maxOutputTokens": 100, "responseModalities": ["TEXT"]}
            }),
        );
        if let Request::Generate(g) = &mut text_only {
            g.model = "m".into();
        }
        let (body, left_out) = to_chat_provider(&text_only, &["modalities"]);
        assert_eq!(left_out, ["generationConfig.responseModalities"]);
        assert!(body.get("modalities").is_none(), "{body}");
    }

    #[test]
    fn a_chat_apps_neutral_values_cross_to_every_format() {
        let req = chat(json!({
            "logprobs": false, "top_logprobs": 0, "logit_bias": {}, "metadata": {},
            "store": false, "service_tier": "auto", "verbosity": "medium"
        }));
        let every = [
            "logit_bias",
            "logprobs",
            "metadata",
            "service_tier",
            "store",
            "top_logprobs",
            "verbosity",
        ];
        for target in [A, B, G] {
            let (body, left_out) = sent(target, &req).unwrap_or_else(|e| panic!("{target}: {e}"));
            assert_eq!(left_out, every, "{target}");
            for key in every {
                assert!(body.get(key).is_none(), "{target}: {body}");
            }
        }
        let (body, left_out) = sent(R, &req).unwrap();
        let without_store: Vec<&str> = every.into_iter().filter(|k| *k != "store").collect();
        assert_eq!(
            left_out, without_store,
            "Responses gets Kaeso's own `store`"
        );
        assert_eq!(body["store"], json!(false));
        assert!(body.get("metadata").is_none(), "{body}");
        let (body, left_out) = sent(C, &req).unwrap();
        assert!(left_out.is_empty(), "{left_out:?}");
        assert_eq!(body["service_tier"], "auto", "OpenAI gets them as written");
    }

    #[test]
    fn an_anthropic_apps_null_thread_crosses_to_every_format() {
        let req = decoded(
            A,
            json!({
                "model": "m", "max_tokens": 100, "thread": null,
                "messages": [{"role": "user", "content": "Hi"}]
            }),
        );
        for target in [C, R, G, B, A] {
            let (body, left_out) = sent(target, &req).unwrap_or_else(|e| panic!("{target}: {e}"));
            assert_eq!(left_out, ["thread"], "{target}");
            assert!(body.get("thread").is_none(), "{target}: {body}");
        }
    }

    #[test]
    fn other_values_of_those_fields_are_still_refused() {
        for (key, value) in [
            ("logprobs", json!(true)),
            ("top_logprobs", json!(2)),
            ("logit_bias", json!({"50256": -100})),
            ("metadata", json!({"tenant": "acme"})),
            ("store", json!(true)),
            ("service_tier", json!("default")),
            ("service_tier", json!("flex")),
            ("verbosity", json!("low")),
            ("web_search_options", json!({})),
        ] {
            let req = chat(json!({ key: value }));
            let e = sent(A, &req).unwrap_err();
            assert_eq!(e.unsupported_feature(), Some(key), "{key}: {value}");
        }
    }

    #[test]
    fn a_chat_provider_that_refuses_a_field_gets_its_neutral_value_left_out() {
        let req = chat(json!({
            "store": false, "metadata": {}, "service_tier": "auto", "logprobs": false,
            "logit_bias": {}, "modalities": ["text"]
        }));
        let refusing = &[
            "store",
            "metadata",
            "service_tier",
            "logprobs",
            "logit_bias",
            "modalities",
        ];
        let (body, left_out) = to_chat_provider(&req, refusing);
        assert_eq!(
            left_out,
            [
                "logit_bias",
                "logprobs",
                "metadata",
                "service_tier",
                "store",
                "modalities"
            ]
        );
        for key in refusing {
            assert!(body.get(*key).is_none(), "{key}: {body}");
        }
        let (body, left_out) = to_chat_provider(&req, &["store"]);
        assert_eq!(left_out, ["store"]);
        assert_eq!(
            body["metadata"],
            json!({}),
            "a field the provider takes stays"
        );
        assert_eq!(body["modalities"], json!(["text"]));

        let refused = chat(json!({"store": true}));
        let profile = ChatProfile::compatible("Example Cloud").refusing(&["store"]);
        let e = sent_with(
            C,
            &refused,
            EgressCtx::new(&refused).with_chat_profile(profile),
        )
        .unwrap_err();
        assert_eq!(e.param.as_deref(), Some("store"));
        let audio = chat(
            json!({"modalities": ["text", "audio"], "audio": {"voice": "marin", "format": "wav"}}),
        );
        let profile = ChatProfile::compatible("Example Cloud").refusing(&["modalities"]);
        let e =
            sent_with(C, &audio, EgressCtx::new(&audio).with_chat_profile(profile)).unwrap_err();
        assert_eq!(e.param.as_deref(), Some("modalities"));
    }

    #[test]
    fn a_refused_field_sent_as_null_is_not_sent() {
        let req = chat(json!({"store": null, "frequency_penalty": null, "seed": null}));
        let (body, left_out) = to_chat_provider(&req, &["store", "frequency_penalty"]);
        assert!(left_out.is_empty(), "{left_out:?}");
        assert!(body.get("store").is_none(), "{body}");
        assert!(body.get("frequency_penalty").is_none(), "{body}");
        assert_eq!(
            body.get("seed"),
            Some(&Value::Null),
            "a null the provider takes stays"
        );
        let (body, _) = to_chat_provider(&chat(json!({"store": null})), &["store"]);
        assert!(body.get("store").is_none(), "{body}");
    }

    #[test]
    fn a_refused_field_sent_as_an_empty_list_is_not_sent() {
        let req = chat(json!({"tools": []}));
        let (body, left_out) = to_chat_provider(&req, &["tools"]);
        assert!(left_out.is_empty(), "an empty list is form: {left_out:?}");
        assert!(body.get("tools").is_none(), "{body}");
        let (body, _) = to_chat_provider(&req, &["store"]);
        assert_eq!(body["tools"], json!([]), "a provider that takes it gets it");
    }

    #[test]
    fn an_anthropic_apps_service_tier_auto_crosses() {
        let req = decoded(
            A,
            json!({
                "model": "m", "max_tokens": 100, "service_tier": "auto",
                "messages": [{"role": "user", "content": "Hi"}]
            }),
        );
        for target in [C, R, G] {
            let (body, left_out) = sent(target, &req).unwrap_or_else(|e| panic!("{target}: {e}"));
            assert_eq!(left_out, ["service_tier"], "{target}");
            assert!(body.get("service_tier").is_none(), "{target}: {body}");
        }
        let (body, left_out) = sent(A, &req).unwrap();
        assert!(left_out.is_empty());
        assert_eq!(body["service_tier"], "auto");
        let profile = MessagesProfile::compatible("Example Cloud").refusing(&["service_tier"]);
        let (body, left_out) =
            sent_with(A, &req, EgressCtx::new(&req).with_messages_profile(profile)).unwrap();
        assert_eq!(left_out, ["service_tier"]);
        assert!(body.get("service_tier").is_none(), "{body}");
    }
}
