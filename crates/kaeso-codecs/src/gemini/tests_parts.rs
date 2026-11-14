use kaeso_kpp::{
    AudioBlock, Block, DialectId, DocumentBlock, Extras, GenerateRequest, ImageBlock, JsonBlock,
    Message, Opaque, OpaqueKind, Origin, RawItem, RawJson, ReasoningBlock, Request, Role, Source,
    VideoBlock,
};
use serde_json::{Value, json};

use super::parts::PartEncoder;
use super::*;
use crate::test_support::assert_json_eq;

fn round_trip(contents: Value) -> GenerateRequest {
    let bytes = serde_json::to_vec(&json!({ "contents": contents })).unwrap();
    let req = crate::dispatch::decode_gemini_request(
        "gemini-x",
        GeminiAction::GenerateContent,
        &bytes,
        &HeaderList::new(),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let mut ctx = EgressCtx::new(&req);
    let enc = Gemini::encode_request(&req, &mut ctx).unwrap_or_else(|e| panic!("{e}"));
    assert_json_eq("parts request", &bytes, &enc.body);
    match req {
        Request::Generate(r) => r,
        other => panic!("{other:?}"),
    }
}

fn sent(blocks: Vec<Block>) -> Result<Value, kaeso_kpp::KppError> {
    let mut r = GenerateRequest::new("gemini-x").with_message(Message::new(Role::User, blocks));
    r.extras = Extras::new(DialectId::OpenAiChat);
    let req = Request::Generate(r);
    let mut ctx = EgressCtx::new(&req);
    Gemini::encode_request(&req, &mut ctx).map(|e| serde_json::from_slice(&e.body).unwrap())
}

fn url(path: &str) -> Source {
    Source::Url(format!("https://example.com/{path}"))
}

#[test]
fn media_urls_from_other_formats_take_the_mime_type_of_their_extension() {
    let cases: Vec<(Block, &str)> = [
        ("a.png", "image/png"),
        ("a.JPG", "image/jpeg"),
        ("a.jpeg", "image/jpeg"),
        ("a.webp", "image/webp"),
        ("a.gif", "image/gif"),
        ("a.heic", "image/heic"),
        ("a.heif", "image/heif"),
    ]
    .into_iter()
    .map(|(p, m)| (Block::Image(ImageBlock::new(url(p))), m))
    .chain(
        [
            ("r.pdf?sig=1", "application/pdf"),
            ("n.txt#top", "text/plain"),
        ]
        .into_iter()
        .map(|(p, m)| (Block::Document(DocumentBlock::new(url(p))), m)),
    )
    .chain(
        [
            ("s.wav", "audio/wav"),
            ("s.mp3", "audio/mp3"),
            ("s.aiff", "audio/aiff"),
            ("s.aac", "audio/aac"),
            ("s.ogg", "audio/ogg"),
            ("s.flac", "audio/flac"),
            ("s.m4a", "audio/m4a"),
            ("s.opus", "audio/opus"),
            ("s.webm", "audio/webm"),
        ]
        .into_iter()
        .map(|(p, m)| (Block::Audio(AudioBlock::new(url(p))), m)),
    )
    .chain(
        [
            ("v.mp4", "video/mp4"),
            ("v.mpeg", "video/mpeg"),
            ("v.mpg", "video/mpg"),
            ("v.mov", "video/mov"),
            ("v.avi", "video/avi"),
            ("v.flv", "video/x-flv"),
            ("v.webm", "video/webm"),
            ("v.wmv", "video/wmv"),
            ("v.3gp", "video/3gpp"),
            ("v.3gpp", "video/3gpp"),
        ]
        .into_iter()
        .map(|(p, m)| (Block::Video(VideoBlock::new(url(p))), m)),
    )
    .collect();
    let (blocks, mimes): (Vec<Block>, Vec<&str>) = cases.into_iter().unzip();
    let body = sent(blocks).unwrap();
    let parts = body["contents"][0]["parts"].as_array().unwrap();
    let got: Vec<&str> = parts
        .iter()
        .map(|p| p["fileData"]["mimeType"].as_str().unwrap())
        .collect();
    assert_eq!(got, mimes);
}

#[test]
fn a_document_url_with_an_extension_gemini_lacks_goes_without_a_mime_type() {
    let body = sent(vec![Block::Document(DocumentBlock::new(url("data.csv")))]).unwrap();
    assert_eq!(
        body["contents"][0]["parts"][0],
        json!({"fileData": {"fileUri": "https://example.com/data.csv"}})
    );
}

#[test]
fn a_file_id_from_another_provider_is_refused_with_its_path() {
    let target = Origin::from(DialectId::Gemini);
    let mut enc = PartEncoder::new(&target);
    let err = enc
        .block(
            &Block::image(Source::file_id(DialectId::OpenAiChat, "file-1")),
            "messages[0].content[0]",
        )
        .unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("image source"));
    assert_eq!(err.param.as_deref(), Some("messages[0].content[0]"));
    assert_eq!(
        err.message,
        "`image source` is not supported by Gemini API. Send the file as base64 or as a URL."
    );
}

#[test]
fn a_json_block_is_refused_as_a_gemini_part() {
    let target = Origin::from(DialectId::Gemini);
    let mut enc = PartEncoder::new(&target);
    let err = enc
        .block(
            &Block::Json(JsonBlock::new(
                RawJson::from_string(r#"{"a":1}"#.into()).unwrap(),
            )),
            "input[0]",
        )
        .unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("json block"));
    assert_eq!(err.param.as_deref(), Some("input[0]"));
}

#[test]
fn a_reply_leaves_out_unknown_parts_of_other_formats_and_keeps_its_own() {
    let target = Origin::from(DialectId::Gemini);
    let mut enc = PartEncoder::new(&target);
    enc.reply = true;
    let foreign = RawItem::new(
        DialectId::OpenAiChat,
        RawJson::from_string(r#"{"type":"input_audio"}"#.into()).unwrap(),
    );
    let own = RawItem::new(
        DialectId::Gemini,
        RawJson::from_string(r#"{"futurePart":1}"#.into()).unwrap(),
    );
    enc.block(&Block::Unknown(foreign), "candidates[0]")
        .unwrap();
    enc.block(&Block::Unknown(own), "candidates[0]").unwrap();
    assert_eq!(enc.finish(), vec![json!({"futurePart": 1})]);
}

#[test]
fn a_gemini_reasoning_block_with_two_signatures_is_written_as_a_thought() {
    let target = Origin::from(DialectId::Gemini);
    let mut block = ReasoningBlock::new(DialectId::Gemini);
    for sig in ["c2ln", "b3RoZXI="] {
        block.opaque.push(Opaque::new(
            DialectId::Gemini,
            OpaqueKind::GeminiThoughtSignature,
            sig,
        ));
    }
    let mut enc = PartEncoder::new(&target);
    enc.block(&Block::Reasoning(block), "contents[0].parts[0]")
        .unwrap();
    enc.block(&Block::text("answer"), "contents[0].parts[1]")
        .unwrap();
    assert_eq!(
        enc.finish(),
        vec![
            json!({"text": "", "thought": true, "thoughtSignature": "c2ln"}),
            json!({"text": "answer"}),
        ]
    );
}

#[test]
fn function_response_parts_gemini_cannot_read_stay_raw() {
    let r = round_trip(json!([
        {"role": "model", "parts": [{"functionCall": {"id": "c1", "name": "f"}}]},
        {"role": "user", "parts": [{"functionResponse": {
            "id": "c1",
            "name": "f",
            "response": {"output": "done"},
            "parts": [{"inlineData": {"mimeType": "image/png", "data": "not base64!"}}]
        }}]}
    ]));
    let Block::ToolResult(result) = &r.messages[1].blocks[0] else {
        panic!("{:?}", r.messages[1].blocks)
    };
    assert!(matches!(&result.content[0], Block::Text(t) if t.text == "done"));
    assert!(matches!(&result.content[1], Block::Unknown(_)));
}

#[test]
fn a_function_response_of_another_shape_reads_as_its_compact_json() {
    let r = round_trip(json!([
        {"role": "model", "parts": [{"functionCall": {"id": "c1", "name": "f"}}]},
        {"role": "user", "parts": [{"functionResponse": {
            "id": "c1",
            "name": "f",
            "response": {"result": "say \"hi there\"\\"}
        }}]}
    ]));
    let Block::ToolResult(result) = &r.messages[1].blocks[0] else {
        panic!()
    };
    let [Block::Text(t)] = result.content.as_slice() else {
        panic!("{:?}", result.content)
    };
    assert_eq!(t.text, r#"{"result":"say \"hi there\"\\"}"#);
}

#[test]
fn a_json_function_response_edited_to_plain_text_goes_back_as_output() {
    let mut r = round_trip(json!([
        {"role": "model", "parts": [{"functionCall": {"id": "c1", "name": "f"}}]},
        {"role": "user", "parts": [{"functionResponse": {
            "id": "c1",
            "name": "f",
            "response": {"result": 1}
        }}]}
    ]));
    let Block::ToolResult(result) = &mut r.messages[1].blocks[0] else {
        panic!()
    };
    result.content = vec![Block::text("plain")];
    let req = Request::Generate(r);
    let mut ctx = EgressCtx::new(&req);
    let enc = Gemini::encode_request(&req, &mut ctx).unwrap();
    let body: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(
        body["contents"][1]["parts"][0]["functionResponse"],
        json!({"id": "c1", "name": "f", "response": {"output": "plain"}})
    );
}

#[test]
fn audio_and_video_urls_with_an_extension_gemini_lacks_are_refused() {
    let err = sent(vec![Block::Audio(AudioBlock::new(url("s.mid")))]).unwrap_err();
    assert_eq!(
        err.unsupported_feature(),
        Some("audio URL without a known file extension")
    );
    let err = sent(vec![Block::Video(VideoBlock::new(url("v.mkv")))]).unwrap_err();
    assert_eq!(
        err.unsupported_feature(),
        Some("video URL without a known file extension")
    );
}

#[test]
fn a_tool_result_that_answers_no_call_needs_the_function_name() {
    let target = Origin::from(DialectId::Gemini);
    let mut enc = PartEncoder::new(&target);
    let err = enc
        .block(
            &Block::tool_result("call_9", vec![Block::text("done")], false),
            "contents[1].parts[0]",
        )
        .unwrap_err();
    assert_eq!(err.kind, kaeso_kpp::ErrorKind::InvalidRequest);
    assert_eq!(err.param.as_deref(), Some("contents[1].parts[0]"));
    assert_eq!(
        err.message,
        "The tool result for `call_9` answers no earlier tool call; Gemini needs the name of the function it answers."
    );
}
