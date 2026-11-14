use kaeso_kpp::{
    AudioFormat, DialectId, Extras, GenerateRequest, Message, Modality, Request, RequestKind,
};
use serde_json::{Value, json};

use super::*;
use crate::test_support::assert_json_eq;

fn round_trip(generation_config: Value) -> GenerateRequest {
    let body = json!({
        "contents": [{"role": "user", "parts": [{"text": "Hi."}]}],
        "generationConfig": generation_config
    });
    let bytes = serde_json::to_vec(&body).unwrap();
    let req = crate::dispatch::decode_gemini_request(
        "gemini-x",
        GeminiAction::GenerateContent,
        &bytes,
        &HeaderList::new(),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let mut ctx = EgressCtx::new(&req);
    let enc = Gemini::encode_request(&req, &mut ctx).unwrap_or_else(|e| panic!("{e}"));
    assert_json_eq("speech request", &bytes, &enc.body);
    match req {
        Request::Generate(r) => r,
        other => panic!("{other:?}"),
    }
}

fn foreign(f: impl FnOnce(&mut GenerateRequest)) -> Request {
    let mut r = GenerateRequest::new("gemini-x").with_message(Message::user("Hi."));
    r.extras = Extras::new(DialectId::OpenAiChat);
    f(&mut r);
    Request::Generate(r)
}

#[test]
fn empty_and_unknown_response_modalities_go_back_as_written() {
    let r = round_trip(json!({"responseModalities": []}));
    assert!(r.output_modalities.is_empty());
    assert!(r.audio_output.is_none());

    let r = round_trip(json!({"responseModalities": ["TEXT", "SOUND"]}));
    assert_eq!(
        r.output_modalities,
        vec![Modality::Text, Modality::Other("SOUND".into())]
    );
}

#[test]
fn speech_config_without_a_voice_round_trips() {
    let r = round_trip(json!({
        "responseModalities": ["AUDIO"],
        "speechConfig": {"languageCode": "de-DE"}
    }));
    let audio = r.audio_output.unwrap();
    assert_eq!(audio.voice, None);
    assert_eq!(audio.format, None);
}

#[test]
fn a_voice_named_outside_the_prebuilt_config_round_trips() {
    let r = round_trip(json!({
        "responseModalities": ["AUDIO"],
        "speechConfig": {"voiceConfig": {"voice": "Puck"}}
    }));
    assert_eq!(r.audio_output.unwrap().voice.as_deref(), Some("Puck"));

    let r = round_trip(json!({
        "responseModalities": ["AUDIO"],
        "speechConfig": {"voiceConfig": {
            "prebuiltVoiceConfig": {"voiceName": "Kore"},
            "voice": "Puck"
        }}
    }));
    assert_eq!(r.audio_output.unwrap().voice.as_deref(), Some("Kore"));
}

#[test]
fn audio_response_formats_kpp_names_are_typed() {
    for (format, typed) in [
        (json!({"mimeType": "AUDIO_WAV"}), AudioFormat::Wav),
        (json!({"mimeType": "AUDIO_MP3"}), AudioFormat::Mp3),
        (json!({"mimeType": "AUDIO_L16"}), AudioFormat::Pcm16),
        (
            json!({"mimeType": "AUDIO_L16", "sampleRate": 24000}),
            AudioFormat::Pcm16,
        ),
    ] {
        let r = round_trip(json!({
            "responseModalities": ["AUDIO"],
            "responseFormat": {"audio": format}
        }));
        assert_eq!(r.audio_output.unwrap().format, Some(typed), "{format}");
        assert!(
            !r.extras
                .fields
                .contains_key("generationConfig.responseFormat")
        );
    }
}

#[test]
fn response_formats_kpp_cannot_name_stay_as_written() {
    for format in [
        json!([{"audio": {"mimeType": "AUDIO_MP3"}}]),
        json!({"image": {"mimeType": "IMAGE_PNG"}}),
        json!({"audio": {"mimeType": "AUDIO_MP3"}, "image": {"mimeType": "IMAGE_PNG"}}),
        json!({"audio": {"mimeType": "AUDIO_MP3", "bitRate": 64}}),
        json!({"audio": {"mimeType": "AUDIO_MULAW"}}),
        json!({"audio": {"mimeType": "AUDIO_WAV", "sampleRate": 24000}}),
        json!({"audio": {"mimeType": "AUDIO_L16", "sampleRate": 16000}}),
        json!({"audio": {"sampleRate": 24000}}),
    ] {
        let r = round_trip(json!({
            "responseModalities": ["AUDIO"],
            "responseFormat": format
        }));
        assert!(r.audio_output.is_none(), "{format}");
        assert_eq!(
            r.extras.fields["generationConfig.responseFormat"]
                .to_value()
                .unwrap(),
            format
        );
    }
}

#[test]
fn a_wav_voice_from_another_format_becomes_gemini_speech_settings() {
    let req = foreign(|r| {
        r.output_modalities = vec![Modality::Text, Modality::Audio];
        let mut audio = kaeso_kpp::AudioOutput::default();
        audio.voice = Some("Kore".into());
        audio.format = Some(AudioFormat::Wav);
        r.audio_output = Some(audio);
    });
    let mut ctx = EgressCtx::new(&req);
    Gemini::check(&req, &ctx).unwrap();
    let enc = Gemini::encode_request(&req, &mut ctx).unwrap();
    let body: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(
        body["generationConfig"],
        json!({
            "responseModalities": ["AUDIO"],
            "speechConfig": {"voiceConfig": {"prebuiltVoiceConfig": {"voiceName": "Kore"}}},
            "responseFormat": {"audio": {"mimeType": "AUDIO_WAV"}}
        })
    );
}

#[test]
fn an_output_modality_gemini_lacks_is_refused_naming_the_app_field() {
    let req = foreign(|r| r.output_modalities = vec![Modality::Other("sound".into())]);
    let err = Gemini::check(
        &req,
        &EgressCtx::for_kind(RequestKind::Generate, kaeso_kpp::StreamMode::Off),
    )
    .unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("output modality `sound`"));
    assert_eq!(err.param.as_deref(), Some("modalities"));
    assert!(
        err.message
            .contains("Gemini returns text, images and audio."),
        "{}",
        err.message
    );
}
