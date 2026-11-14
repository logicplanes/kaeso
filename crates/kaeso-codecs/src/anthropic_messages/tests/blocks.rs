use kaeso_kpp::{ErrorKind, ImageBlock, JsonBlock, RawItem, RefusalBlock};

use super::*;
use crate::anthropic_messages::blocks::{BlockEncoder, Side};

fn round_trip(content: Value) -> GenerateRequest {
    let body = json!({
        "model": "claude-sonnet-5-5",
        "max_tokens": 64,
        "messages": [{"role": "user", "content": content}]
    });
    let r = generate(body.clone());
    let sent = encode_value(r.clone());
    assert_eq!(sent["messages"], body["messages"]);
    r
}

fn decode_err(content: Value) -> KppError {
    let body = json!({
        "model": "m",
        "max_tokens": 1,
        "messages": [{"role": "user", "content": content}]
    });
    AnthropicMessages::decode_request(
        RequestKind::Generate,
        &serde_json::to_vec(&body).unwrap(),
        &HeaderList::new(),
    )
    .unwrap_err()
}

fn encoder_error(side: Side, block: Block) -> KppError {
    BlockEncoder::new(Origin::new(A), side)
        .block(&block)
        .unwrap_err()
}

fn own_item(value: Value) -> RawItem {
    RawItem::new(A, RawJson::from_serialize(&value).unwrap())
}

#[test]
fn documents_with_citations_disabled_or_unset_go_back_as_written() {
    let r = round_trip(json!([
        {"type": "document", "source": {"type": "url", "url": "https://example.com/a.pdf"}, "citations": {"enabled": false}},
        {"type": "document", "source": {"type": "url", "url": "https://example.com/b.pdf"}, "citations": {}},
        {"type": "text", "text": "Compare them."}
    ]));
    for block in &r.messages[0].blocks[..2] {
        let Block::Document(d) = block else {
            panic!("{block:?}")
        };
        assert!(!d.citations);
    }
}

#[test]
fn a_video_with_a_source_kaeso_does_not_know_stays_raw() {
    let r = round_trip(json!([
        {"type": "video", "source": {"type": "stream", "id": "v1"}},
        {"type": "text", "text": "What happens?"}
    ]));
    assert!(matches!(&r.messages[0].blocks[0], Block::Unknown(_)));
}

#[test]
fn an_empty_tool_result_list_goes_back_as_a_list() {
    let body = json!({
        "model": "claude-sonnet-5-5",
        "max_tokens": 64,
        "messages": [
            {"role": "user", "content": "Run it."},
            {"role": "assistant", "content": [{"type": "tool_use", "id": "toolu_1", "name": "run", "input": {}}]},
            {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "toolu_1", "content": []}]}
        ]
    });
    let r = generate(body.clone());
    let Block::ToolResult(result) = &r.messages[2].blocks[0] else {
        panic!()
    };
    assert!(result.content.is_empty());
    assert_eq!(encode_value(r)["messages"], body["messages"]);
}

#[test]
fn a_cache_hint_of_another_type_and_lifetime_goes_back_as_written() {
    let r = round_trip(json!([
        {"type": "text", "text": "long context", "cache_control": {"type": "persistent", "ttl": "10m"}}
    ]));
    let Block::Text(t) = &r.messages[0].blocks[0] else {
        panic!()
    };
    assert_eq!(
        t.cache.as_ref().unwrap().ttl,
        Some(CacheTtl::Other("10m".into()))
    );
}

#[test]
fn sources_kaeso_cannot_read_are_invalid_requests() {
    let err = decode_err(json!([{"type": "image", "source": {"type": "url"}}]));
    assert_eq!(err.kind, ErrorKind::InvalidRequest);
    assert!(
        err.message
            .contains("invalid `source`: missing field `url`"),
        "{}",
        err.message
    );
    let err = decode_err(json!([
        {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "not base64!"}}
    ]));
    assert_eq!(err.kind, ErrorKind::InvalidRequest);
    assert!(
        err.message.contains("`source.data` is not valid base64"),
        "{}",
        err.message
    );
}

#[test]
fn only_anthropic_tool_changes_are_read_as_tool_changes() {
    let addition =
        json!({"type": "tool_addition", "tool": {"type": "tool_reference", "name": "Read"}});
    let foreign = RawItem::new(CHAT, RawJson::from_serialize(&addition).unwrap());
    assert!(tool_change(&foreign).is_none());
    assert!(tool_change(&own_item(json!({"type": "server_note", "text": "x"}))).is_none());
    assert!(matches!(
        tool_change(&own_item(addition)),
        Some(Ok(ToolChange::Added(_)))
    ));
    let Some(Err(reason)) = tool_change(&own_item(
        json!({"type": "tool_addition", "tool": {"type": "tool_definition"}}),
    )) else {
        panic!()
    };
    assert_eq!(reason, "a `tool_definition` without a `definition`");
}

#[test]
fn only_a_tool_reference_reads_as_the_name_of_a_tool() {
    let (name, _) = tool_reference(&own_item(
        json!({"type": "tool_reference", "tool_name": "Read"}),
    ))
    .unwrap();
    assert_eq!(name, "Read");
    assert!(
        tool_reference(&own_item(
            json!({"type": "tool_handle", "tool_name": "Read"})
        ))
        .is_none()
    );
}

#[test]
fn content_anthropic_has_no_block_for_is_refused_when_writing_a_request() {
    let cases = [
        (Block::Refusal(RefusalBlock::new("no")), "refusal block"),
        (
            Block::Unknown(RawItem::new(CHAT, raw(r#"{"type":"input_audio"}"#))),
            "unknown content block",
        ),
        (Block::Json(JsonBlock::new(raw(r#"{"a":1}"#))), "json block"),
        (
            Block::image(Source::Url("data:image/png,raw".into())),
            "data URL without base64 encoding",
        ),
        (
            Block::image(Source::text("text/plain", "x")),
            "text image source",
        ),
    ];
    for (block, feature) in cases {
        let err = encoder_error(Side::Request, block);
        assert_eq!(err.unsupported_feature(), Some(feature), "{err:?}");
    }
}

#[test]
fn a_reply_cannot_point_at_a_file_of_another_provider() {
    let mut image = ImageBlock::new(Source::file_id(CHAT, "file-1"));
    image.extras = Extras::new(A);
    let err = encoder_error(Side::Reply, Block::Image(image));
    assert_eq!(
        err.unsupported_feature(),
        Some("file id from another provider")
    );
    assert!(
        err.message
            .contains("Provider file ids only work with the provider that issued them."),
        "{}",
        err.message
    );
}
