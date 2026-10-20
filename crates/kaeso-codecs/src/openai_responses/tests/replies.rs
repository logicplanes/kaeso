use kaeso_kpp::{
    Citation, CountTokensResponse, ErrorKind, OpaqueKind, ReasoningBlock, RefusalBlock, StopReason,
    TextBlock, Usage,
};
use serde_json::json;

use super::*;

#[test]
fn every_response_fixture_round_trips() {
    for (name, body) in fixtures(DIR, "responses", "json") {
        if FAILED_RESPONSES.contains(&name.as_str()) {
            continue;
        }
        let resp = decode_resp(&body);
        let out = OpenAiResponses::encode_response(
            &Reply::Generate(resp),
            &EncodeCtx::new(RequestKind::Generate, kaeso_kpp::StreamMode::Off),
        )
        .unwrap();
        assert_json_eq(&name, &body, &out);
    }
    for (name, body) in fixtures(DIR, "responses/input_tokens", "json") {
        let ctx = EgressCtx::for_kind(RequestKind::CountTokens, kaeso_kpp::StreamMode::Off);
        let reply = OpenAiResponses::decode_response(200, &HeaderList::new(), &body, &ctx).unwrap();
        let out = OpenAiResponses::encode_response(
            &reply,
            &EncodeCtx::new(RequestKind::CountTokens, kaeso_kpp::StreamMode::Off),
        )
        .unwrap();
        assert_json_eq(&name, &body, &out);
    }
}

#[test]
fn every_error_fixture_round_trips_with_status_and_retry_after() {
    for (name, body) in fixtures(DIR, "errors", "json") {
        let meta = sidecar(DIR, "errors", &name, "meta").unwrap();
        let status = u16::try_from(meta["status"].as_u64().unwrap()).unwrap();
        let headers: HeaderList = meta
            .get("headers")
            .and_then(Value::as_object)
            .map(|m| {
                m.iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned()))
                    .collect()
            })
            .unwrap_or_default();
        let err = OpenAiResponses::decode_error(status, &headers, &body);
        let out = OpenAiResponses::encode_error(&err);
        assert_eq!(out.status, status, "{name}");
        assert_json_eq(&name, &encoded_error(&meta, &body), &out.body);
        assert_eq!(
            out.headers.get("retry-after"),
            headers.get("retry-after"),
            "{name}"
        );
    }
}

fn response(name: &str) -> Response {
    decode_resp(&fixture("responses", name, "json"))
}

fn echo_ctx(req: Option<Request>) -> EncodeCtx {
    let mut ctx = EncodeCtx::new(RequestKind::Generate, kaeso_kpp::StreamMode::Off)
        .with_created_at(1_800_000_000);
    if let Some(r) = req {
        ctx = EncodeCtx::for_request(r).with_created_at(1_800_000_000);
    }
    ctx
}

#[test]
fn responses_decode_to_typed_blocks_stop_and_usage() {
    let r = response("text-input-string");
    assert_eq!(
        r.id,
        "resp_67ccd2bed1ec8190b14f964abc0542670bb6a6b452d3795b"
    );
    assert_eq!(r.model, "gpt-6-astra");
    assert_eq!(r.stop, StopReason::EndTurn);
    assert!(matches!(&r.blocks[..], [Block::Text(t)] if t.text.starts_with("In a peaceful grove")));
    assert_eq!(r.usage.input_tokens, 36);
    assert_eq!(r.usage.output_tokens, 87);

    let r = response("tools-parallel");
    assert_eq!(r.stop, StopReason::ToolUse);
    assert!(
        matches!(&r.blocks[..], [Block::Reasoning(_), Block::ToolCall(a), Block::ToolCall(b)]
        if a.id == "call_ZhPaNDVL6V7jzIneVoHYEMoQ" && b.arguments.get() == r#"{"city":"Tokyo","unit":"celsius"}"#)
    );

    let r = response("max-output-tokens-incomplete");
    assert_eq!(r.stop, StopReason::MaxTokens);

    let r = response("reasoning-encrypted");
    let Block::Reasoning(rs) = &r.blocks[0] else {
        panic!()
    };
    assert_eq!(rs.summary.len(), 1);
    assert!(rs.opaque_of(OpaqueKind::OpenAiEncryptedReasoning).is_some());
    assert!(rs.origin.as_ref().unwrap().provider.as_deref() == Some("openai"));
    assert_eq!(r.usage.input_tokens, 144);
    assert_eq!(r.usage.cache_read_tokens, 768);
    assert_eq!(r.usage.reasoning_tokens, Some(192));
    assert!(r.usage.raw.is_some());

    let r = response("refusal");
    assert!(matches!(&r.blocks[1], Block::Refusal(f) if f.text.starts_with("I'm sorry")));

    let r = response("web-search");
    assert!(matches!(&r.blocks[1], Block::Unknown(i) if i.origin.is(R)));
    let Block::Text(t) = &r.blocks[3] else {
        panic!()
    };
    assert_eq!(t.citations.len(), 1);
    assert!(t.citations[0].raw.get().contains("url_citation"));

    let r = response("unknown-output-item");
    assert!(matches!(&r.blocks[0], Block::Unknown(_)));
    assert!(r.extras.fields.contains_key("x_future_field"));
}

#[test]
fn a_failed_response_object_decodes_to_its_error() {
    let err = OpenAiResponses::decode_response(
        200,
        &HeaderList::new(),
        &fixture("responses", "failed", "json"),
        &gen_ctx(),
    )
    .unwrap_err();
    assert_eq!(err.kind, ErrorKind::ProviderError);
    assert!(err.message.starts_with("An error occurred"));
    assert!(err.raw.is_some());
    assert!(err.origin.as_ref().is_some_and(|o| o.is(R)));
    let encoded = OpenAiResponses::encode_error(&err);
    let body: Value = serde_json::from_slice(&encoded.body).unwrap();
    assert_eq!(body["error"]["message"], json!(err.message));
    assert!(body.get("object").is_none(), "{body}");
}

#[test]
fn partial_usage_keeps_the_counts_it_has() {
    let body = json!({
        "id": "resp_1",
        "object": "response",
        "created_at": 1,
        "status": "completed",
        "model": "gpt-5.5",
        "output": [],
        "usage": {"input_tokens": 3}
    });
    let r = decode_resp(&serde_json::to_vec(&body).unwrap());
    assert_eq!((r.usage.input_tokens, r.usage.output_tokens), (3, 0));
}

#[test]
fn non_2xx_statuses_decode_as_errors() {
    let err = OpenAiResponses::decode_response(
        429,
        &HeaderList::new().with("retry-after", "3"),
        &fixture("errors", "rate-limit-tokens-429", "json"),
        &gen_ctx(),
    )
    .unwrap_err();
    assert_eq!(err.kind, ErrorKind::RateLimited);
    assert_eq!(err.retry_after, Some(std::time::Duration::from_secs(3)));
    let err = OpenAiResponses::decode_response(200, &HeaderList::new(), b"not json", &gen_ctx())
        .unwrap_err();
    assert_eq!(err.kind, ErrorKind::ProviderError);
}

#[test]
fn error_kinds_of_the_fixtures() {
    let kind = |name: &str, status: u16| {
        OpenAiResponses::decode_error(status, &HeaderList::new(), &fixture("errors", name, "json"))
            .kind
    };
    assert_eq!(
        kind("context-length-exceeded-400", 400),
        ErrorKind::ContextTooLong
    );
    assert_eq!(kind("insufficient-quota-429", 429), ErrorKind::Billing);
    assert_eq!(kind("invalid-api-key-401", 401), ErrorKind::Authentication);
    assert_eq!(kind("model-not-found-404", 404), ErrorKind::NotFound);
    assert_eq!(kind("overloaded-503", 503), ErrorKind::Overloaded);
    assert_eq!(kind("server-error-500", 500), ErrorKind::ProviderError);
    assert_eq!(kind("slow-down-429", 429), ErrorKind::RateLimited);
    assert_eq!(
        kind("unsupported-region-403", 403),
        ErrorKind::PermissionDenied
    );
    assert_eq!(
        kind("unsupported-parameter-400", 400),
        ErrorKind::InvalidRequest
    );
    let e = OpenAiResponses::decode_error(
        400,
        &HeaderList::new(),
        &fixture("errors", "unsupported-parameter-400", "json"),
    );
    assert_eq!(e.param.as_deref(), Some("temperature"));
}

#[test]
fn kaeso_errors_get_the_openai_shape() {
    let out =
        OpenAiResponses::encode_error(&KppError::unsupported("top_k", "`top_k` is not supported."));
    assert_eq!(out.status, 400);
    let body: Value = serde_json::from_slice(&out.body).unwrap();
    assert_eq!(body["error"]["type"], "invalid_request_error");
    assert_eq!(body["error"]["message"], "`top_k` is not supported.");
    let out = OpenAiResponses::encode_error(
        &KppError::new(ErrorKind::Overloaded, "busy")
            .with_status(529)
            .with_origin(DialectId::AnthropicMessages),
    );
    assert_eq!(out.status, 503);
}

fn anthropic_reply() -> Response {
    let a = DialectId::AnthropicMessages;
    let mut thinking = ReasoningBlock::new(a);
    thinking.text.push("Think.".into());
    thinking.opaque.push(kaeso_kpp::Opaque::new(
        a,
        OpaqueKind::AnthropicThinkingSignature,
        "sig",
    ));
    let mut redacted = ReasoningBlock::new(a);
    redacted.opaque.push(kaeso_kpp::Opaque::new(
        a,
        OpaqueKind::AnthropicRedactedThinking,
        "xx",
    ));
    let mut cited = TextBlock::new("Paris.");
    cited
        .citations
        .push(Citation::new(a, raw(r#"{"type":"char_location"}"#)));
    let mut r = Response::new(
        "msg_01ABC",
        "claude-sonnet-4-6",
        vec![
            Block::Reasoning(thinking),
            Block::Reasoning(redacted),
            Block::Text(cited),
            Block::Unknown(kaeso_kpp::RawItem::new(
                a,
                raw(r#"{"type":"server_tool_use"}"#),
            )),
            Block::tool_call("toolu_1", "get_weather", raw(r#"{"city":"Paris"}"#)),
        ],
    );
    r.stop = StopReason::ToolUse;
    let mut u = Usage::new(10, 20);
    u.cache_read_tokens = 5;
    u.cache_write_tokens = 2;
    u.raw = Some(kaeso_kpp::RawUsage::new(a, json!({"input_tokens": 10})));
    r.usage = u;
    r.extras = Extras::new(a).with_field("container", raw("null"));
    r
}

#[test]
fn foreign_replies_become_complete_response_objects() {
    let req = OpenAiResponses::decode_request(
        RequestKind::Generate,
        &fixture("requests", "tools-function", "json"),
        &HeaderList::new(),
    )
    .unwrap();
    let out =
        OpenAiResponses::encode_response(&Reply::Generate(anthropic_reply()), &echo_ctx(Some(req)))
            .unwrap();
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["id"], "msg_01ABC");
    assert_eq!(v["object"], "response");
    assert_eq!(v["status"], "completed");
    assert_eq!(v["created_at"], 1_800_000_000);
    assert_eq!(v["completed_at"], 1_800_000_000);
    assert_eq!(v["model"], "claude-sonnet-4-6");
    assert_eq!(v["tool_choice"], "auto");
    assert_eq!(v["tools"][0]["name"], "get_current_weather");
    assert_eq!(v["store"], false);
    assert_eq!(v["temperature"], 1.0);
    assert!(v.get("container").is_none(), "foreign extras are left out");
    let out_items = v["output"].as_array().unwrap();
    assert_eq!(out_items.len(), 4, "{out_items:?}");
    let reply = anthropic_reply();
    for (i, summary) in [
        (0, json!([{"type": "summary_text", "text": "Think."}])),
        (1, json!([])),
    ] {
        let item = &out_items[i];
        assert_eq!(item["type"], "reasoning");
        assert!(
            item["id"].as_str().unwrap().starts_with("rs_kaeso_"),
            "{item}"
        );
        assert_eq!(item["summary"], summary);
        let Block::Reasoning(original) = &reply.blocks[i] else {
            panic!()
        };
        assert_eq!(
            crate::continuity::unwrap_reasoning(item["encrypted_content"].as_str().unwrap())
                .as_ref(),
            Some(original)
        );
    }
    let out_items: Vec<Value> = std::iter::once(Value::Null)
        .chain(out_items.iter().skip(2).cloned())
        .collect();
    assert_eq!(out_items[1]["type"], "message");
    assert!(
        out_items[1]["id"]
            .as_str()
            .unwrap()
            .starts_with("msg_kaeso_")
    );
    assert!(
        out_items[2]["id"]
            .as_str()
            .unwrap()
            .starts_with("fc_kaeso_")
    );
    assert_eq!(
        out_items[1]["content"],
        json!([{"type": "output_text", "annotations": [], "logprobs": [], "text": "Paris."}])
    );
    assert_eq!(out_items[2]["call_id"], "toolu_1");
    assert_eq!(out_items[2]["arguments"], r#"{"city":"Paris"}"#);
    assert_eq!(
        v["usage"],
        json!({"input_tokens": 17, "input_tokens_details": {"cached_tokens": 5, "cache_write_tokens": 2},
               "output_tokens": 20, "output_tokens_details": {"reasoning_tokens": 0}, "total_tokens": 37})
    );
    let out =
        OpenAiResponses::encode_response(&Reply::Generate(anthropic_reply()), &echo_ctx(None))
            .unwrap();
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["tools"], json!([]));
    assert_eq!(v["text"], json!({"format": {"type": "text"}}));
}

#[test]
fn stop_reasons_map_to_status() {
    for (stop, status, reason) in [
        (
            StopReason::MaxTokens,
            "incomplete",
            json!({"reason": "max_output_tokens"}),
        ),
        (
            StopReason::ContentFilter,
            "incomplete",
            json!({"reason": "content_filter"}),
        ),
        (
            StopReason::StopSequence("x".into()),
            "completed",
            Value::Null,
        ),
        (StopReason::PauseTurn, "completed", Value::Null),
    ] {
        let mut r = Response::new(
            "r",
            "m",
            vec![Block::text("x"), Block::Refusal(RefusalBlock::new("no"))],
        );
        r.stop = stop;
        let v: Value = serde_json::from_slice(
            &OpenAiResponses::encode_response(&Reply::Generate(r), &echo_ctx(None)).unwrap(),
        )
        .unwrap();
        assert_eq!(v["status"], status);
        assert_eq!(v["incomplete_details"], reason);
        assert_eq!(v["output"][0]["status"], status);
        assert_eq!(
            v["output"][0]["content"][1],
            json!({"type": "refusal", "refusal": "no"})
        );
    }
}

#[test]
fn count_tokens_replies() {
    let out = OpenAiResponses::encode_response(
        &Reply::CountTokens(CountTokensResponse::new(42)),
        &EncodeCtx::new(RequestKind::CountTokens, kaeso_kpp::StreamMode::Off),
    )
    .unwrap();
    assert_json_eq(
        "count",
        br#"{"object":"response.input_tokens","input_tokens":42}"#,
        &out,
    );
    let err = OpenAiResponses::encode_response(
        &Reply::Embed(kaeso_kpp::EmbedResponse::new("m", vec![])),
        &EncodeCtx::new(RequestKind::Embed, kaeso_kpp::StreamMode::Off),
    )
    .unwrap_err();
    assert!(err.unsupported_feature().is_some());
}

#[test]
fn kaeso_invented_item_ids_are_not_sent_back_to_providers() {
    let out =
        OpenAiResponses::encode_response(&Reply::Generate(anthropic_reply()), &echo_ctx(None))
            .unwrap();
    let v: Value = serde_json::from_slice(&out).unwrap();
    let mut input: Vec<Value> = vec![json!({"role": "user", "content": "Weather?"})];
    input.extend(v["output"].as_array().unwrap().iter().cloned());
    input.push(json!({"type": "function_call_output", "call_id": "toolu_1", "output": "rain"}));
    input.push(json!({"type": "reasoning", "id": "rs_kaeso_x_0", "summary": []}));
    input.push(json!({"type": "message", "id": "msg_0d9d96706c3569bb006acb6c7d9d6981", "role": "assistant", "status": "completed",
        "content": [{"type": "output_text", "text": "ok", "annotations": []}]}));
    let body = json!({"model": "gpt-5.5", "input": input});
    let req = OpenAiResponses::decode_request(
        RequestKind::Generate,
        &serde_json::to_vec(&body).unwrap(),
        &HeaderList::new(),
    )
    .unwrap();
    let mut ctx = EgressCtx::new(&req).with_provider("openai");
    let enc = OpenAiResponses::encode_request(&req, &mut ctx).unwrap();
    let sent = String::from_utf8(enc.body).unwrap();
    assert!(!sent.contains("_kaeso_"), "{sent}");
    assert!(
        sent.contains("msg_0d9d96706c3569bb006acb6c7d9d6981"),
        "provider ids stay"
    );
}

#[test]
fn dropping_blocks_keeps_item_boundaries_and_provider_ids() {
    let mut r = response("web-search");
    r.blocks.remove(0);
    let out = OpenAiResponses::encode_response(&Reply::Generate(r), &echo_ctx(None)).unwrap();
    let v: Value = serde_json::from_slice(&out).unwrap();
    let items = v["output"].as_array().unwrap();
    assert_eq!(items.len(), 3, "{items:?}");
    assert_eq!(items[0]["type"], "web_search_call");
    assert_eq!(items[1]["type"], "reasoning");
    assert_eq!(
        items[2]["id"],
        "msg_0d9d96706c3569bb006acb6c7d9d698190217eb9d7e719a0f5"
    );
    assert_eq!(items[2]["content"].as_array().unwrap().len(), 1);
    let mut r = response("tools-parallel");
    r.blocks.remove(0);
    let v: Value = serde_json::from_slice(
        &OpenAiResponses::encode_response(&Reply::Generate(r), &echo_ctx(None)).unwrap(),
    )
    .unwrap();
    assert_eq!(
        v["output"][0]["id"],
        "fc_00636e4c8fc87d8c006acb59d7192e81936a93e05a699b09c3"
    );
    assert_eq!(v["output"].as_array().unwrap().len(), 2);
}

#[test]
fn a_reply_whose_usage_cannot_be_read_is_a_provider_error_not_zero_usage() {
    let mut body: Value =
        serde_json::from_slice(&fixture("responses", "text-input-string", "json")).unwrap();
    body["usage"] = json!({"input_tokens": "many", "output_tokens": 3});
    let err = OpenAiResponses::decode_response(
        200,
        &HeaderList::new(),
        &serde_json::to_vec(&body).unwrap(),
        &gen_ctx(),
    )
    .unwrap_err();
    assert_eq!(err.kind, ErrorKind::ProviderError);
    assert!(err.message.contains("usage object"), "{}", err.message);
    body["usage"] = Value::Null;
    let resp = decode_resp(&serde_json::to_vec(&body).unwrap());
    assert_eq!(resp.usage, Usage::default());
}
