#![cfg(test)]

use kaeso_codecs::gemini::GeminiAction;
use kaeso_codecs::{EgressCtx, EncodeCtx, HeaderList, dispatch};
use kaeso_kpp::{
    AudioDelta, AudioFormat, AudioHeader, AudioOutput, Block, BlockDelta, BlockHeader, BlockStart,
    BlockStop, Delta, DialectId, GenerateRequest, KppError, Message, Modality, PCM16_MEDIA_TYPE,
    Reply, Request, RequestKind, Response, ResponseDelta, ResponseStart, ResponseStop, Role,
    Source, StopReason, StreamEvent, StreamMode,
};
use serde_json::{Value, json};

const A: DialectId = DialectId::AnthropicMessages;
const C: DialectId = DialectId::OpenAiChat;
const R: DialectId = DialectId::OpenAiResponses;
const G: DialectId = DialectId::Gemini;

fn decode(dialect: DialectId, body: &Value) -> Request {
    let bytes = serde_json::to_vec(body).unwrap();
    if dialect == G {
        let action = if body.get("stream").is_some() {
            GeminiAction::StreamGenerateContent
        } else {
            GeminiAction::GenerateContent
        };
        return dispatch::decode_gemini_request(
            "gemini-3.8-flash-tts",
            action,
            &bytes,
            &HeaderList::new(),
        )
        .unwrap();
    }
    dispatch::decode_request(dialect, RequestKind::Generate, &bytes, &HeaderList::new()).unwrap()
}

fn encode(dialect: DialectId, req: &Request) -> Result<(Value, EgressCtx), KppError> {
    let mut ctx = EgressCtx::new(req);
    let enc = dispatch::encode_request(dialect, req, &mut ctx)?;
    Ok((serde_json::from_slice(&enc.body).unwrap(), ctx))
}

fn chat_speech(voice: &str, format: &str, stream: bool) -> Value {
    let mut body = json!({
        "model": "gemini-3.8-flash-tts",
        "modalities": ["text", "audio"],
        "audio": {"voice": voice, "format": format},
        "messages": [{"role": "user", "content": "Say hi."}]
    });
    if stream {
        body["stream"] = json!(true);
    }
    body
}

#[test]
fn a_chat_apps_audio_request_reaches_gemini_as_speech() {
    let req = decode(C, &chat_speech("Kore", "pcm16", true));
    let Request::Generate(g) = &req else { panic!() };
    assert_eq!(g.output_modalities, [Modality::Text, Modality::Audio]);
    let audio = g.audio_output.as_ref().unwrap();
    assert_eq!(audio.voice.as_deref(), Some("Kore"));
    assert_eq!(audio.format, Some(AudioFormat::Pcm16));
    let (sent, _) = encode(G, &req).unwrap();
    assert_eq!(
        sent["generationConfig"],
        json!({
            "responseModalities": ["AUDIO"],
            "speechConfig": {"voiceConfig": {"prebuiltVoiceConfig": {"voiceName": "Kore"}}},
            "responseFormat": {"audio": {"mimeType": "AUDIO_L16"}}
        })
    );
}

#[test]
fn voices_and_formats_cross_only_where_the_target_documents_them() {
    let err = encode(G, &decode(C, &chat_speech("marin", "wav", false))).unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("voice `marin`"));
    assert_eq!(err.param.as_deref(), Some("audio.voice"));
    let err = encode(G, &decode(C, &chat_speech("Kore", "flac", false))).unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("audio format `flac`"));
    assert_eq!(err.param.as_deref(), Some("audio.format"));

    let gemini = |voice: &str| {
        json!({
            "contents": [{"role": "user", "parts": [{"text": "Say hi."}]}],
            "generationConfig": {
                "responseModalities": ["AUDIO"],
                "speechConfig": {"voiceConfig": {"prebuiltVoiceConfig": {"voiceName": voice}}}
            }
        })
    };
    let err = encode(C, &decode(G, &gemini("Kore"))).unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("voice `Kore`"));
    assert_eq!(err.param.as_deref(), Some("generationConfig.speechConfig"));
    let (sent, ctx) = encode(C, &decode(G, &gemini("alloy"))).unwrap();
    assert_eq!(sent["modalities"], json!(["text", "audio"]));
    assert_eq!(sent["audio"], json!({"voice": "alloy", "format": "wav"}));
    assert_eq!(ctx.audio_format, Some(AudioFormat::Wav));

    let mut silent = decode(G, &gemini("alloy"));
    if let Request::Generate(g) = &mut silent {
        g.audio_output = None;
    }
    let err = encode(C, &silent).unwrap_err();
    assert_eq!(
        err.unsupported_feature(),
        Some("audio output without a voice")
    );
}

#[test]
fn text_only_formats_refuse_audio_and_image_output() {
    let mut speech = decode(C, &chat_speech("marin", "wav", false));
    if let Request::Generate(g) = &mut speech {
        g.sampling.max_output_tokens = Some(64);
    }
    for target in [A, R] {
        let err = encode(target, &speech).unwrap_err();
        assert_eq!(err.unsupported_feature(), Some("audio output"), "{target}");
        assert_eq!(err.param.as_deref(), Some("modalities"));
    }
    let image = json!({
        "contents": [{"role": "user", "parts": [{"text": "Draw a circle."}]}],
        "generationConfig": {"responseModalities": ["TEXT", "IMAGE"], "maxOutputTokens": 64}
    });
    let image = decode(G, &image);
    for target in [A, R, C] {
        let err = encode(target, &image).unwrap_err();
        assert_eq!(err.unsupported_feature(), Some("image output"), "{target}");
        assert_eq!(
            err.param.as_deref(),
            Some("generationConfig.responseModalities")
        );
    }
}

fn gemini_reply(parts: Value) -> Reply {
    let body = json!({
        "candidates": [{"content": {"parts": parts, "role": "model"}, "finishReason": "STOP", "index": 0}],
        "usageMetadata": {"promptTokenCount": 4, "candidatesTokenCount": 27, "totalTokenCount": 31},
        "modelVersion": "gemini-3.8-flash-tts",
        "responseId": "resp_tts_1"
    });
    let ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off);
    dispatch::decode_response(
        G,
        200,
        &HeaderList::new(),
        &serde_json::to_vec(&body).unwrap(),
        &ctx,
    )
    .unwrap()
}

fn chat_reply(reply: &Reply) -> Value {
    let ctx = EncodeCtx::new(RequestKind::Generate, StreamMode::Off).with_created_at(1_791_734_800);
    serde_json::from_slice(&dispatch::encode_response(C, reply, &ctx).unwrap()).unwrap()
}

#[test]
fn geminis_speech_reaches_a_chat_app_as_message_audio() {
    let reply = gemini_reply(json!([
        {"inlineData": {"mimeType": PCM16_MEDIA_TYPE, "data": "AAAA"}},
        {"inlineData": {"mimeType": "audio/L16;codec=pcm;rate=24000", "data": "AQ=="}}
    ]));
    let message = &chat_reply(&reply)["choices"][0]["message"];
    assert_eq!(
        message["audio"],
        json!({"id": "audio_resp_tts_1", "expires_at": 1_791_734_800, "data": "AAAAAQ==", "transcript": ""})
    );
    assert_eq!(message["content"], Value::Null);

    let wav = gemini_reply(json!([
        {"inlineData": {"mimeType": "audio/wav", "data": "UklGRg=="}},
        {"inlineData": {"mimeType": "audio/wav", "data": "UklGRh=="}}
    ]));
    assert_eq!(
        chat_reply(&wav)["choices"][0]["message"]["audio"]["data"],
        "UklGRg=="
    );
    for target in [A, R] {
        let ctx = EncodeCtx::new(RequestKind::Generate, StreamMode::Off);
        let out: Value =
            serde_json::from_slice(&dispatch::encode_response(target, &reply, &ctx).unwrap())
                .unwrap();
        assert!(!out.to_string().contains("AAAA"), "{target}: {out}");
    }
}

#[test]
fn a_chat_audio_reply_reaches_a_gemini_app_inline_with_its_requested_format() {
    let req = decode(C, &chat_speech("marin", "mp3", false));
    let (_, ctx) = encode(C, &req).unwrap();
    let body = json!({
        "id": "chatcmpl-1", "object": "chat.completion", "created": 1, "model": "gpt-audio-mini",
        "choices": [{"index": 0, "finish_reason": "stop", "message": {
            "role": "assistant", "content": null,
            "audio": {"id": "audio_1", "expires_at": 2, "data": "SUQz", "transcript": "Hi."}
        }}]
    });
    let reply = dispatch::decode_response(
        C,
        200,
        &HeaderList::new(),
        body.to_string().as_bytes(),
        &ctx,
    )
    .unwrap();
    let Reply::Generate(r) = &reply else { panic!() };
    let [Block::Audio(audio)] = r.blocks.as_slice() else {
        panic!("{:?}", r.blocks)
    };
    assert_eq!(audio.source, Source::base64("audio/mpeg", "SUQz").unwrap());
    assert_eq!(audio.transcript.as_deref(), Some("Hi."));
    let ectx = EncodeCtx::new(RequestKind::Generate, StreamMode::Off);
    let out: Value =
        serde_json::from_slice(&dispatch::encode_response(G, &reply, &ectx).unwrap()).unwrap();
    assert_eq!(
        out["candidates"][0]["content"]["parts"],
        json!([{"inlineData": {"mimeType": "audio/mpeg", "data": "SUQz"}}])
    );
}

#[test]
fn a_replayed_audio_reply_stays_at_openai_unless_its_transcript_comes_along() {
    let history = json!({
        "model": "gpt-audio-mini",
        "messages": [
            {"role": "user", "content": "Say hello."},
            {"role": "assistant", "audio": {"id": "audio_abc"}},
            {"role": "user", "content": "Again."}
        ]
    });
    let req = decode(C, &history);
    let (sent, _) = encode(C, &req).unwrap();
    assert_eq!(sent["messages"], history["messages"]);
    let err = encode(G, &req).unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("file id"));

    let mut spoken = kaeso_kpp::AudioBlock::new(Source::base64(PCM16_MEDIA_TYPE, "AAAA").unwrap());
    spoken.transcript = Some("Hello!".into());
    let mut r = GenerateRequest::new("gpt-audio-mini")
        .with_message(Message::user("Say hello."))
        .with_message(Message::new(Role::Assistant, vec![Block::Audio(spoken)]))
        .with_message(Message::user("Again."));
    r.output_modalities = vec![Modality::Audio];
    r.audio_output = Some(AudioOutput::new(Some("marin".into()), None));
    let (sent, _) = encode(C, &Request::Generate(r)).unwrap();
    assert_eq!(sent["messages"][1]["content"], "Hello!");
    assert_eq!(sent["audio"], json!({"voice": "marin", "format": "wav"}));
}

fn stream_bytes(dialect: DialectId, events: &[StreamEvent]) -> String {
    let ctx = EncodeCtx::new(RequestKind::Generate, StreamMode::on(true)).with_created_at(7);
    let mut enc = dispatch::stream_encoder(dialect, &ctx).unwrap();
    let mut out = Vec::new();
    for e in events {
        out.extend(enc.encode(e).unwrap());
    }
    out.extend(enc.finish().unwrap());
    String::from_utf8(out).unwrap()
}

#[test]
fn streamed_audio_crosses_between_chat_and_gemini_piece_by_piece() {
    let events = vec![
        StreamEvent::ResponseStart(ResponseStart::new("resp_1", "gemini-3.8-flash-tts")),
        StreamEvent::BlockStart(BlockStart::new(
            0,
            BlockHeader::Audio(AudioHeader::new(PCM16_MEDIA_TYPE)),
        )),
        StreamEvent::BlockDelta(BlockDelta::new(
            0,
            Delta::Audio(AudioDelta::new(Some("AAAA".into()), None)),
        )),
        StreamEvent::BlockDelta(BlockDelta::new(
            0,
            Delta::Audio(AudioDelta::new(None, Some("Hi".into()))),
        )),
        StreamEvent::BlockStop(BlockStop::new(0)),
        StreamEvent::ResponseDelta(ResponseDelta::new(Some(StopReason::EndTurn), None)),
        StreamEvent::ResponseStop(ResponseStop::new()),
    ];
    let chat = stream_bytes(C, &events);
    assert!(chat.contains(r#""audio":{"id":"audio_resp_1"}"#), "{chat}");
    assert!(chat.contains(r#""audio":{"data":"AAAA"}"#), "{chat}");
    assert!(chat.contains(r#""audio":{"transcript":"Hi"}"#), "{chat}");
    let gemini = stream_bytes(G, &events);
    let first: Value = serde_json::from_str(
        gemini
            .lines()
            .next()
            .and_then(|l| l.strip_prefix("data: "))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        first["candidates"][0]["content"]["parts"],
        json!([{"inlineData": {"mimeType": PCM16_MEDIA_TYPE, "data": "AAAA"}}])
    );
    assert!(!gemini.contains("Hi"), "{gemini}");
    for target in [A, R] {
        assert!(!stream_bytes(target, &events).contains("AAAA"), "{target}");
    }
}

#[test]
fn an_audio_only_reply_round_trips_through_the_logged_form() {
    let mut block = kaeso_kpp::AudioBlock::new(Source::base64("audio/wav", "UklGRg==").unwrap());
    block.transcript = Some("Hi.".into());
    let reply = Response::new("r", "m", vec![Block::Audio(block)]);
    let json = serde_json::to_value(&reply).unwrap();
    assert_eq!(
        json["blocks"],
        json!([{"audio": {"source": {"base64": {"media_type": "audio/wav", "data": "UklGRg=="}}, "transcript": "Hi."}}])
    );
    assert_eq!(serde_json::from_value::<Response>(json).unwrap(), reply);
}
