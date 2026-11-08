#![cfg(test)]

use kaeso_codecs::anthropic_messages::{MINIMAX_VIDEO_MEDIA_TYPES, MessagesProfile};
use kaeso_codecs::gemini::GeminiAction;
use kaeso_codecs::openai_chat::ChatProfile;
use kaeso_codecs::{EgressCtx, HeaderList, dispatch};
use kaeso_kpp::{
    Block, DialectId, GenerateRequest, KppError, Message, Request, RequestKind, Role, Source,
};
use serde_json::{Value, json};

const A: DialectId = DialectId::AnthropicMessages;
const C: DialectId = DialectId::OpenAiChat;
const R: DialectId = DialectId::OpenAiResponses;
const G: DialectId = DialectId::Gemini;

fn decode(dialect: DialectId, body: &Value) -> Request {
    let bytes = serde_json::to_vec(body).unwrap();
    if dialect == G {
        return dispatch::decode_gemini_request(
            "gemini-3.5-flash",
            GeminiAction::GenerateContent,
            &bytes,
            &HeaderList::new(),
        )
        .unwrap();
    }
    dispatch::decode_request(dialect, RequestKind::Generate, &bytes, &HeaderList::new()).unwrap()
}

fn encode_with(dialect: DialectId, req: &Request, ctx: &mut EgressCtx) -> Result<Value, KppError> {
    let enc = dispatch::encode_request(dialect, req, ctx)?;
    Ok(serde_json::from_slice(&enc.body).unwrap())
}

fn encode(dialect: DialectId, req: &Request) -> Result<Value, KppError> {
    encode_with(dialect, req, &mut EgressCtx::new(req))
}

fn user(blocks: Vec<Block>) -> Request {
    let mut r = GenerateRequest::new("m").with_message(Message::new(Role::User, blocks));
    r.sampling.max_output_tokens = Some(256);
    Request::Generate(r)
}

fn blocks(req: &Request) -> &[Block] {
    let Request::Generate(r) = req else {
        panic!("{req:?}")
    };
    &r.messages[0].blocks
}

fn refused(result: Result<Value, KppError>) -> KppError {
    let err = result.unwrap_err();
    assert!(err.unsupported_feature().is_some(), "{err:?}");
    err
}

const MP3: &str = "SUQzBAAAAAAAI1RTU0U=";

#[test]
fn chat_input_audio_is_an_audio_block_that_reaches_gemini_inline() {
    let req = decode(
        C,
        &json!({
            "model": "gpt-audio-mini",
            "messages": [{"role": "user", "content": [
                {"type": "input_audio", "input_audio": {"data": MP3, "format": "mp3"}},
                {"type": "text", "text": "Who speaks?"}
            ]}]
        }),
    );
    let [Block::Audio(audio), Block::Text(_)] = blocks(&req) else {
        panic!("{req:?}")
    };
    assert_eq!(audio.source, Source::base64("audio/mpeg", MP3));
    let sent = encode(G, &req).unwrap();
    assert_eq!(
        sent["contents"][0]["parts"][0],
        json!({"inlineData": {"mimeType": "audio/mpeg", "data": MP3}})
    );
}

#[test]
fn gemini_audio_reaches_chat_with_its_format_and_responses_and_anthropic_refuse_it() {
    let req = decode(
        G,
        &json!({"contents": [{"role": "user", "parts": [
            {"inlineData": {"mimeType": "audio/mp3", "data": MP3}},
            {"text": "Transcribe."}
        ]}]}),
    );
    let sent = encode(C, &req).unwrap();
    assert_eq!(
        sent["messages"][0]["content"][0],
        json!({"type": "input_audio", "input_audio": {"data": MP3, "format": "mp3"}})
    );
    let err = refused(encode(R, &req));
    assert_eq!(err.unsupported_feature(), Some("audio block"));
    assert_eq!(err.param.as_deref(), Some("messages[0].blocks[0]"));
    let mut anthropic = req.clone();
    if let Request::Generate(r) = &mut anthropic {
        r.sampling.max_output_tokens = Some(64);
    }
    let err = refused(encode(A, &anthropic));
    assert!(err.message.contains("transcript"), "{}", err.message);
}

#[test]
fn chat_audio_formats_follow_the_provider_profile() {
    let flac = user(vec![Block::audio(Source::base64("audio/flac", "ZkxhQw=="))]);
    let err = refused(encode(C, &flac));
    assert_eq!(err.unsupported_feature(), Some("audio format `flac`"));
    assert!(err.message.contains("wav, mp3"), "{}", err.message);

    let mut ctx = EgressCtx::new(&flac)
        .with_chat_profile(ChatProfile::compatible("OpenRouter").any_audio_format());
    let sent = encode_with(C, &flac, &mut ctx).unwrap();
    assert_eq!(
        sent["messages"][0]["content"][0]["input_audio"]["format"],
        "flac"
    );

    let webm = user(vec![Block::audio(Source::base64("audio/webm", "GkXf"))]);
    let err = refused(encode(C, &webm));
    assert_eq!(
        err.unsupported_feature(),
        Some("audio media type `audio/webm`")
    );
}

#[test]
fn chat_takes_audio_only_as_base64_in_user_messages_and_no_video() {
    let url = user(vec![Block::audio(Source::Url(
        "https://example.com/a.mp3".into(),
    ))]);
    assert_eq!(
        refused(encode(C, &url)).unsupported_feature(),
        Some("audio source")
    );
    let mut assistant = GenerateRequest::new("m")
        .with_message(Message::user("Say it."))
        .with_message(Message::new(
            Role::Assistant,
            vec![Block::audio(Source::base64("audio/wav", "UklGRg=="))],
        ));
    assistant.sampling.max_output_tokens = Some(16);
    let assistant = Request::Generate(assistant);
    assert_eq!(
        refused(encode(C, &assistant)).unsupported_feature(),
        Some("audio in assistant messages")
    );
    let video = user(vec![Block::video(Source::Url(
        "https://www.youtube.com/watch?v=UTdfxFyOQTI".into(),
    ))]);
    let err = refused(encode(C, &video));
    assert_eq!(err.unsupported_feature(), Some("video block"));
    assert!(err.message.contains("frames"), "{}", err.message);
}

#[test]
fn video_url_parts_go_only_to_chat_providers_that_document_them() {
    let body = json!({"model": "kimi-k3", "messages": [{"role": "user", "content": [
        {"type": "video_url", "video_url": {"url": "data:video/mp4;base64,AAAAIGZ0eXA="}},
        {"type": "video_url", "video_url": {"url": "ms://cvb1kle0"}},
        {"type": "text", "text": "Describe both."}
    ]}]});
    let req = decode(C, &body);
    let [Block::Video(inline), Block::Video(file), Block::Text(_)] = blocks(&req) else {
        panic!("{req:?}")
    };
    assert_eq!(inline.source, Source::base64("video/mp4", "AAAAIGZ0eXA="));
    assert_eq!(file.source, Source::Url("ms://cvb1kle0".into()));
    let mut ctx = EgressCtx::new(&req)
        .with_chat_profile(ChatProfile::compatible("Moonshot").with_video_url());
    assert_eq!(
        encode_with(C, &req, &mut ctx).unwrap()["messages"],
        body["messages"]
    );
    assert_eq!(
        refused(encode(C, &req)).unsupported_feature(),
        Some("video block")
    );
    let gemini = decode(
        G,
        &json!({"contents": [{"role": "user", "parts": [
            {"fileData": {"fileUri": "https://www.youtube.com/watch?v=UTdfxFyOQTI"}}
        ]}]}),
    );
    let mut ctx = EgressCtx::new(&gemini)
        .with_chat_profile(ChatProfile::compatible("OpenRouter").with_video_url());
    assert_eq!(
        encode_with(C, &gemini, &mut ctx).unwrap()["messages"][0]["content"][0],
        json!({"type": "video_url", "video_url": {"url": "https://www.youtube.com/watch?v=UTdfxFyOQTI"}})
    );
}

#[test]
fn chat_audio_parts_without_a_known_format_round_trip_as_written() {
    for part in [
        json!({"type": "input_audio", "input_audio": {"data": "AAAA", "format": "pcm16"}}),
        json!({"type": "input_audio", "input_audio": "AAAA"}),
    ] {
        let body = json!({"model": "voxtral-small-latest", "messages": [
            {"role": "user", "content": [part.clone()]}
        ]});
        let req = decode(C, &body);
        assert!(matches!(blocks(&req), [Block::Unknown(_)]), "{part}");
        assert_eq!(encode(C, &req).unwrap()["messages"], body["messages"]);
        assert!(encode(G, &req).is_err(), "{part}");
    }
}

#[test]
fn youtube_and_file_urls_become_video_and_audio_for_gemini() {
    let req = decode(
        G,
        &json!({"contents": [{"role": "user", "parts": [
            {"fileData": {"fileUri": "https://www.youtube.com/watch?v=UTdfxFyOQTI"}},
            {"fileData": {"fileUri": "files/abc", "mimeType": "audio/wav"}}
        ]}]}),
    );
    let [Block::Video(video), Block::Audio(audio)] = blocks(&req) else {
        panic!("{req:?}")
    };
    assert_eq!(
        video.source,
        Source::Url("https://www.youtube.com/watch?v=UTdfxFyOQTI".into())
    );
    assert!(matches!(audio.source, Source::FileId(_)));

    let foreign = user(vec![
        Block::video(Source::Url("https://youtu.be/UTdfxFyOQTI".into())),
        Block::video(Source::Url("https://example.com/clip.MOV?x=1".into())),
        Block::audio(Source::Url("https://example.com/talk.flac".into())),
    ]);
    let sent = encode(G, &foreign).unwrap();
    assert_eq!(
        sent["contents"][0]["parts"],
        json!([
            {"fileData": {"fileUri": "https://youtu.be/UTdfxFyOQTI"}},
            {"fileData": {"fileUri": "https://example.com/clip.MOV?x=1", "mimeType": "video/mov"}},
            {"fileData": {"fileUri": "https://example.com/talk.flac", "mimeType": "audio/flac"}}
        ])
    );
    for (block, feature) in [
        (
            Block::audio(Source::Url("https://example.com/talk".into())),
            "audio URL without a known file extension",
        ),
        (
            Block::video(Source::Url("https://example.com/clip".into())),
            "video URL without a known file extension",
        ),
    ] {
        assert_eq!(
            refused(encode(G, &user(vec![block]))).unsupported_feature(),
            Some(feature)
        );
    }
}

#[test]
fn gemini_function_responses_keep_audio_raw_and_refuse_foreign_audio() {
    let calls = Message::new(
        Role::Assistant,
        vec![Block::tool_call(
            "call_1",
            "listen",
            kaeso_kpp::RawJson::empty_object(),
        )],
    );
    let mut r = GenerateRequest::new("m")
        .with_message(Message::user("Play it."))
        .with_message(calls)
        .with_message(Message::new(
            Role::User,
            vec![Block::tool_result(
                "call_1",
                vec![Block::audio(Source::base64("audio/wav", "UklGRg=="))],
                false,
            )],
        ));
    r.sampling.max_output_tokens = Some(32);
    let req = Request::Generate(r);
    let err = refused(encode(G, &req));
    assert_eq!(
        err.unsupported_feature(),
        Some("audio block in a tool result")
    );
}

#[test]
fn minimax_takes_video_blocks_and_anthropic_refuses_them() {
    let body = json!({
        "model": "MiniMax-M3",
        "max_tokens": 64,
        "messages": [{"role": "user", "content": [
            {"type": "video", "source": {"type": "base64", "media_type": "video/mp4", "data": "AAAAIGZ0eXA="}},
            {"type": "video", "source": {"type": "url", "url": "mm_file://12345"}, "cache_control": {"type": "ephemeral"}},
            {"type": "text", "text": "Compare the clips."}
        ]}]
    });
    let req = decode(A, &body);
    let [Block::Video(inline), Block::Video(file), Block::Text(_)] = blocks(&req) else {
        panic!("{req:?}")
    };
    assert_eq!(inline.source, Source::base64("video/mp4", "AAAAIGZ0eXA="));
    assert!(file.cache.is_some());

    let minimax = MessagesProfile::compatible("MiniMax").with_video(MINIMAX_VIDEO_MEDIA_TYPES);
    let mut ctx = EgressCtx::new(&req).with_messages_profile(minimax);
    let sent = encode_with(A, &req, &mut ctx).unwrap();
    assert_eq!(sent["messages"], body["messages"]);

    let err = refused(encode(A, &req));
    assert_eq!(err.unsupported_feature(), Some("video block"));
    assert_eq!(err.param.as_deref(), Some("messages[0].blocks[0]"));

    let webm = user(vec![Block::video(Source::base64("video/webm", "GkXf"))]);
    let mut ctx = EgressCtx::new(&webm).with_messages_profile(minimax);
    let err = refused(encode_with(A, &webm, &mut ctx));
    assert_eq!(
        err.unsupported_feature(),
        Some("video media type `video/webm`")
    );
    assert!(err.message.contains("MiniMax"), "{}", err.message);

    assert_eq!(
        refused(encode(G, &req)).param.as_deref(),
        Some("messages[0].blocks[1]")
    );
}
