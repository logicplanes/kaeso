use kaeso_kpp::{ErrorKind, RequestKind, StreamEvent, StreamMode};

use super::{ChatProfile, OpenAiChat};
use crate::codec::{Codec, EgressCtx, HeaderList};
use crate::dispatch;
use crate::test_support::{assert_json_eq, fixtures};

const UPSTREAM: ChatProfile =
    ChatProfile::compatible("OpenRouter").error_detail(&["error", "metadata", "raw"]);

fn ctx(profile: ChatProfile, stream: bool) -> EgressCtx {
    EgressCtx::for_kind(RequestKind::Generate, StreamMode::on(stream)).with_chat_profile(profile)
}

fn recorded_error(name: &str) -> Vec<u8> {
    let wanted = format!("providers/openrouter/{name}");
    fixtures("openai_chat", "errors", "json")
        .into_iter()
        .find(|(n, _)| *n == wanted)
        .map(|(_, body)| body)
        .unwrap()
}

#[test]
fn the_upstream_reason_a_profile_names_joins_the_message_and_chat_apps_get_the_body_as_sent() {
    let body = recorded_error("recorded-upstream-rate-limited");
    let headers = HeaderList::new().with("retry-after", "1");
    let err = dispatch::decode_provider_error(
        kaeso_kpp::DialectId::OpenAiChat,
        429,
        &headers,
        &body,
        &ctx(UPSTREAM, false),
    );
    assert_eq!(err.kind, ErrorKind::RateLimited);
    assert_eq!(
        err.message,
        "Provider returned error: liquid/lfm-2.5-2.6b:free is temporarily rate-limited upstream. Please retry shortly, or add your own key to accumulate your rate limits: https://openrouter.ai/settings/integrations"
    );
    assert!(err.retry_after.is_some());
    let out = OpenAiChat::encode_error(&err);
    assert_eq!(out.status, 429);
    assert_json_eq("chat app", &body, &out.body);

    let via_reply =
        OpenAiChat::decode_response(429, &headers, &body, &ctx(UPSTREAM, false)).unwrap_err();
    assert_eq!(via_reply.message, err.message);

    let plain = dispatch::decode_provider_error(
        kaeso_kpp::DialectId::OpenAiChat,
        429,
        &headers,
        &body,
        &ctx(ChatProfile::OPENAI, false),
    );
    assert_eq!(plain.message, "Provider returned error");
    assert_eq!(
        OpenAiChat::decode_error(429, &headers, &body).message,
        "Provider returned error"
    );
}

#[test]
fn errors_without_the_named_detail_keep_their_message() {
    let body = recorded_error("recorded-unknown-model");
    let err =
        OpenAiChat::decode_provider_error(400, &HeaderList::new(), &body, &ctx(UPSTREAM, false));
    assert_eq!(
        err.message,
        "kaeso-example/no-such-model:free is not a valid model ID"
    );
}

#[test]
fn a_streamed_error_gets_the_upstream_reason_too() {
    let chunk = String::from_utf8(recorded_error("recorded-routing-fields")).unwrap();
    let chunk: serde_json::Value = serde_json::from_str(&chunk).unwrap();
    let frame = format!("data: {chunk}\n\n");
    let mut decoder = OpenAiChat::stream_decoder(&ctx(UPSTREAM, true));
    let mut events = decoder.feed(frame.as_bytes());
    events.extend(decoder.finish());
    let Some(StreamEvent::Error(err)) = events.first() else {
        panic!("{events:?}")
    };
    assert!(
        err.message
            .starts_with("Provider returned error: google/gemma-4-31b-it:free is temporarily rate-limited upstream."),
        "{}",
        err.message
    );
}
