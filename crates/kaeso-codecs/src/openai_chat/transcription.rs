//! A transcription as a Chat Completions call with an `input_audio` part, as QwenCloud serves
//! `qwen3-asr-flash` on its OpenAI-compatible mode (Qwen-ASR API reference,
//! <https://www.alibabacloud.com/help/en/model-studio/qwen-asr-api-reference>, read 2026-11-08):
//! the audio as a data URL in `input_audio.data`, the language in `asr_options`.

use kaeso_kpp::{
    KppError, Origin, Reply, Source, StreamMode, TranscriptFormat, TranscriptionOperation,
    TranscriptionRequest, TranscriptionResponse, Usage,
};
use serde_json::{Value, json};

use super::DIALECT;
use crate::codec::{EgressCtx, EncodedRequest, HeaderList};

fn refused(feature: &str, param: &str, hint: &str) -> KppError {
    KppError::unsupported_by(
        feature.to_owned(),
        DIALECT,
        format!("QwenCloud's speech recognition does not take {feature}; {hint}"),
    )
    .with_param(param.to_owned())
}

pub(super) fn check(r: &TranscriptionRequest) -> Result<(), KppError> {
    crate::cross::check_extras(&r.extras, DIALECT, "", "request")?;
    if r.operation == TranscriptionOperation::Translate {
        return Err(refused(
            "translations",
            "model",
            "transcribe the audio and translate the text instead.",
        ));
    }
    if !matches!(r.audio.source, Source::Base64(_) | Source::Url(_)) {
        return Err(refused("file ids", "file", "upload the file."));
    }
    if !matches!(
        r.format_or_default(),
        TranscriptFormat::Json | TranscriptFormat::Text
    ) {
        return Err(refused(
            &format!("`response_format: {}`", r.format_or_default().as_wire()),
            "response_format",
            "it returns no timestamps through this API; ask for `json` or `text`.",
        ));
    }
    let unsupported = [
        ("prompt", r.prompt.is_some()),
        ("temperature", r.temperature.is_some()),
        ("timestamp_granularities", !r.timestamps.is_empty()),
        ("diarize", r.diarize.is_some()),
        ("num_speakers", r.speakers.is_some()),
    ];
    if let Some((field, _)) = unsupported.iter().find(|(_, set)| *set) {
        return Err(refused(&format!("`{field}`"), field, "remove it."));
    }
    if let Some(key) = r.extras.fields.keys().next() {
        return Err(refused(&format!("`{key}`"), key, "remove it."));
    }
    Ok(())
}

pub(super) fn encode(
    r: &TranscriptionRequest,
    ctx: &mut EgressCtx,
) -> Result<EncodedRequest, KppError> {
    check(r)?;
    ctx.stream = StreamMode::Off;
    ctx.transcript_format = Some(r.format_or_default());
    let data = match &r.audio.source {
        Source::Base64(b) => b.data_url(),
        Source::Url(u) => u.clone(),
        _ => String::new(),
    };
    let mut body = json!({
        "model": r.model,
        "messages": [{"role": "user", "content": [{"type": "input_audio", "input_audio": {"data": data}}]}],
        "stream": false,
    });
    if let (Some(l), Value::Object(m)) = (&r.language, &mut body) {
        m.insert("asr_options".to_owned(), json!({ "language": l }));
    }
    let path = match &ctx.base_path {
        Some(base) => format!("{base}/chat/completions"),
        None => super::CHAT_COMPLETIONS_PATH.to_owned(),
    };
    Ok(EncodedRequest::post_json(
        path,
        HeaderList::new().with("content-type", "application/json"),
        crate::wire::to_body(&body)?,
    ))
}

/// The transcript in a chat completion: the message's text, the language of its `audio_info`
/// annotation, and usage with the audio's `seconds`.
pub(super) fn decode(body: &[u8], origin: &Origin) -> Result<Reply, KppError> {
    let v: Value = serde_json::from_slice(body).map_err(|e| {
        KppError::provider(format!(
            "The provider sent a transcript that is not JSON: {}.",
            crate::wire::without_position(&e)
        ))
    })?;
    let message = v.pointer("/choices/0/message");
    let text = message
        .and_then(|m| m.get("content"))
        .and_then(Value::as_str)
        .ok_or_else(|| KppError::provider("The provider's transcript has no text."))?;
    let mut resp = TranscriptionResponse::new(
        v.get("model").and_then(Value::as_str).unwrap_or_default(),
        text,
    );
    resp.language = message
        .and_then(|m| m.get("annotations"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|a| a.get("type").and_then(Value::as_str) == Some("audio_info"))
        .and_then(|a| a.get("language"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    if let Some(u) = v.get("usage") {
        let n = |k: &str| u.get(k).and_then(Value::as_u64).unwrap_or(0);
        let mut usage = Usage::new(n("prompt_tokens"), n("completion_tokens"));
        usage.input_audio_tokens = u
            .pointer("/prompt_tokens_details/audio_tokens")
            .and_then(Value::as_u64);
        usage.raw = Some(kaeso_kpp::RawUsage::new(origin.clone(), u.clone()));
        resp.usage = usage;
        resp.usage_seconds = u.get("seconds").and_then(Value::as_f64);
    }
    Ok(Reply::Transcription(resp))
}

#[cfg(test)]
mod tests {
    use kaeso_kpp::{AudioInput, ErrorKind, Request, TranscriptionOperation};

    use super::*;
    use crate::Codec;
    use crate::openai_chat::OpenAiChat;

    fn request() -> TranscriptionRequest {
        let mut r = TranscriptionRequest::new(
            "qwen3-asr-flash",
            TranscriptionOperation::Transcribe,
            AudioInput::new(Source::base64("audio/mpeg", "SUQz").unwrap()),
        );
        r.language = Some("zh".into());
        r
    }

    #[test]
    fn a_transcription_becomes_an_input_audio_chat_call_and_its_reply_a_transcript() {
        let req = Request::Transcription(request());
        let mut ctx = EgressCtx::new(&req)
            .with_provider("alibaba")
            .with_base_path("/v1");
        OpenAiChat::check(&req, &ctx).unwrap();
        let enc = OpenAiChat::encode_request(&req, &mut ctx).unwrap();
        assert_eq!(enc.path, "/v1/chat/completions");
        let body: Value = serde_json::from_slice(&enc.body).unwrap();
        assert_eq!(
            body["messages"][0]["content"][0]["input_audio"]["data"],
            "data:audio/mpeg;base64,SUQz"
        );
        assert_eq!(body["asr_options"], json!({"language": "zh"}));
        let reply = std::fs::read(format!(
            "{}/fixtures/openai_transcriptions/providers/alibaba/responses/chat.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let Reply::Transcription(t) =
            OpenAiChat::decode_response(200, &HeaderList::new(), &reply, &ctx).unwrap()
        else {
            panic!()
        };
        assert_eq!(t.text, "Welcome to Alibaba Cloud.");
        assert_eq!(t.language.as_deref(), Some("zh"));
        assert_eq!(
            (t.usage_seconds, t.usage.input_audio_tokens),
            (Some(1.0), Some(42))
        );
    }

    #[test]
    fn what_the_model_lacks_is_refused() {
        for f in [
            (|r: &mut TranscriptionRequest| r.prompt = Some("p".into())) as fn(&mut _),
            |r| r.format = Some(TranscriptFormat::Srt),
            |r| r.operation = TranscriptionOperation::Translate,
            |r| r.diarize = Some(true),
        ] {
            let mut r = request();
            f(&mut r);
            let err = check(&r).unwrap_err();
            assert!(matches!(err.kind, ErrorKind::Unsupported { .. }), "{err:?}");
        }
        let mut link = request();
        link.audio = AudioInput::new(Source::Url("https://example.com/a.mp3".into()));
        check(&link).unwrap();
    }
}
