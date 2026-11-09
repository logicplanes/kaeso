//! ElevenLabs' realtime WebSocket APIs, read without I/O: which socket a path opens, what an
//! app's message carries for billing and which credentials it names, and the error message a
//! socket closes with. Sources, read 2026-11-09: ElevenLabs' WebSocket references
//! (<https://elevenlabs.io/docs/websockets>), the text to dialogue WebSocket
//! (<https://elevenlabs.io/docs/api-reference/text-to-dialogue/ttd-websocket>), and the
//! official `elevenlabs` Python SDK 2.77.0 (`realtime_tts.py`, `realtime/` and the
//! `text_to_dialogue_websocket_*` and `websocket_tts_client_message_multi` types).

use kaeso_kpp::{ErrorKind, KppError};
use serde_json::{Map, Value, json};

/// Which of ElevenLabs' realtime sockets a path opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RealtimeApi {
    /// `/v1/text-to-speech/{voice_id}/stream-input`.
    TextToSpeech,
    /// `/v1/text-to-speech/{voice_id}/multi-stream-input`, several contexts on one socket.
    TextToSpeechContexts,
    /// `/v1/text-to-dialogue/stream-input`.
    TextToDialogue,
    /// `/v1/text-to-dialogue/multi-stream-input`.
    TextToDialogueContexts,
    /// `/v1/speech-to-text/realtime`.
    SpeechToText,
}

/// The fields an app's first message may carry its credential in. Kaeso reads the Kaeso key
/// from them and never passes them on.
pub const CREDENTIAL_FIELDS: &[&str] = &["xi_api_key", "authorization", "single_use_token"];

/// The query parameters that carry credentials, which Kaeso doesn't take.
pub const CREDENTIAL_PARAMS: &[&str] =
    &["xi_api_key", "authorization", "single_use_token", "token"];

impl RealtimeApi {
    /// The socket a path opens, and the voice it names.
    pub fn from_path(path: &str) -> Option<(RealtimeApi, Option<String>)> {
        let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
        match parts.as_slice() {
            ["v1", "text-to-speech", voice, "stream-input"] => {
                Some((RealtimeApi::TextToSpeech, Some((*voice).to_owned())))
            }
            ["v1", "text-to-speech", voice, "multi-stream-input"] => {
                Some((RealtimeApi::TextToSpeechContexts, Some((*voice).to_owned())))
            }
            ["v1", "text-to-dialogue", "stream-input"] => Some((RealtimeApi::TextToDialogue, None)),
            ["v1", "text-to-dialogue", "multi-stream-input"] => {
                Some((RealtimeApi::TextToDialogueContexts, None))
            }
            ["v1", "speech-to-text", "realtime"] => Some((RealtimeApi::SpeechToText, None)),
            _ => None,
        }
    }

    /// The path at ElevenLabs, with the voice as one path segment.
    pub fn path(self, voice: Option<&str>) -> Result<String, KppError> {
        let voice = || {
            voice
                .filter(|v| super::request::is_voice_id(v))
                .ok_or_else(|| {
                    KppError::invalid_request(
                        "Name an ElevenLabs voice by its id, as `GET /v1/voices` lists them.",
                    )
                    .with_param("voice_id")
                })
        };
        Ok(match self {
            RealtimeApi::TextToSpeech => format!("/v1/text-to-speech/{}/stream-input", voice()?),
            RealtimeApi::TextToSpeechContexts => {
                format!("/v1/text-to-speech/{}/multi-stream-input", voice()?)
            }
            RealtimeApi::TextToDialogue => "/v1/text-to-dialogue/stream-input".to_owned(),
            RealtimeApi::TextToDialogueContexts => {
                "/v1/text-to-dialogue/multi-stream-input".to_owned()
            }
            RealtimeApi::SpeechToText => "/v1/speech-to-text/realtime".to_owned(),
        })
    }

    /// Whether the socket makes speech from text, rather than text from speech.
    pub fn speaks(self) -> bool {
        self != RealtimeApi::SpeechToText
    }

    /// Whether ElevenLabs takes an API key in the first message of this socket.
    pub fn takes_key_in_first_message(self) -> bool {
        self.speaks()
    }
}

/// What one of an app's messages carries.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ClientMessage {
    /// The message as it goes on to ElevenLabs, without credentials.
    pub forwarded: String,
    /// The credential the message named, from its first message's credential fields.
    pub credential: Option<String>,
    /// The characters of text the message asks to speak.
    pub characters: u64,
    /// The bytes of audio the message sends to transcribe.
    pub audio_bytes: u64,
    /// The sample rate the message names for its audio.
    pub sample_rate: Option<u32>,
}

fn bad(detail: &str) -> KppError {
    KppError::invalid_request(format!("Send each message as a JSON object; {detail}."))
}

fn chars(v: Option<&Value>) -> u64 {
    v.and_then(Value::as_str)
        .map_or(0, |s| s.chars().count() as u64)
}

/// Reads one of an app's text messages: takes its credential fields out and counts what it
/// asks ElevenLabs to make or hear.
pub fn read_client_message(api: RealtimeApi, text: &str) -> Result<ClientMessage, KppError> {
    let mut value: Value = serde_json::from_str(text).map_err(|_| bad("this one is not JSON"))?;
    let Some(object) = value.as_object_mut() else {
        return Err(bad("this one is not an object"));
    };
    let mut out = ClientMessage::default();
    for field in CREDENTIAL_FIELDS {
        if let Some(v) = object.remove(*field) {
            let s = v.as_str().map(str::trim).unwrap_or_default();
            let s = s.strip_prefix("Bearer ").unwrap_or(s).trim();
            if !s.is_empty() && out.credential.is_none() {
                out.credential = Some(s.to_owned());
            }
        }
    }
    match api {
        RealtimeApi::TextToSpeech | RealtimeApi::TextToSpeechContexts => {
            out.characters = chars(object.get("text"));
        }
        RealtimeApi::TextToDialogue | RealtimeApi::TextToDialogueContexts => {
            out.characters = object
                .get("inputs")
                .and_then(Value::as_array)
                .map_or(0, |inputs| {
                    inputs.iter().map(|i| chars(i.get("text"))).sum()
                });
        }
        RealtimeApi::SpeechToText => {
            if let Some(audio) = object.get("audio_base_64").and_then(Value::as_str) {
                out.audio_bytes = decoded_len(audio);
            }
            out.sample_rate = object
                .get("sample_rate")
                .and_then(Value::as_u64)
                .and_then(|r| u32::try_from(r).ok());
        }
    }
    out.forwarded = serde_json::to_string(&value).unwrap_or_default();
    Ok(out)
}

/// The length of base64 data once decoded, without decoding it.
fn decoded_len(b64: &str) -> u64 {
    (b64.trim_end_matches('=').len() as u64) * 3 / 4
}

/// How many bytes one second of audio in `audio_format` takes: 16-bit PCM or 8-bit µ-law, one
/// channel, as realtime speech to text takes them.
pub fn bytes_per_second(audio_format: &str) -> Option<u64> {
    let (codec, rate) = audio_format.split_once('_')?;
    let rate: u64 = rate.parse().ok()?;
    match codec {
        "pcm" => Some(rate * 2),
        "ulaw" => Some(rate),
        _ => None,
    }
}

/// The message a socket sends before it closes on `err`, in the socket's own error shape, and
/// the close code that follows it: 1008 for what the app can fix, 1011 for Kaeso's or the
/// provider's own failures (RFC 6455, section 7.4.1).
pub fn error_message(api: RealtimeApi, err: &KppError) -> (String, u16) {
    let code = match err.kind {
        ErrorKind::InvalidRequest
        | ErrorKind::Unsupported { .. }
        | ErrorKind::Authentication
        | ErrorKind::PermissionDenied
        | ErrorKind::Billing
        | ErrorKind::NotFound
        | ErrorKind::RateLimited
        | ErrorKind::RequestTooLarge
        | ErrorKind::ContextTooLong => 1008,
        _ => 1011,
    };
    let (_, _, wire_code) = super::error::wire_for(err);
    let body = if api == RealtimeApi::SpeechToText {
        let message_type = match err.kind {
            ErrorKind::Authentication | ErrorKind::PermissionDenied => "auth_error",
            ErrorKind::Billing => "quota_exceeded",
            ErrorKind::RateLimited => "rate_limited",
            ErrorKind::InvalidRequest | ErrorKind::Unsupported { .. } => "invalid_request",
            ErrorKind::RequestTooLarge => "chunk_size_exceeded",
            _ => "error",
        };
        json!({"message_type": message_type, "error": err.message})
    } else {
        let mut m = Map::new();
        m.insert("message".into(), err.message.clone().into());
        m.insert("error".into(), wire_code.into());
        m.insert("code".into(), code.into());
        m.insert(
            "param".into(),
            err.param.clone().map_or(Value::Null, Value::from),
        );
        Value::Object(m)
    };
    (body.to_string(), code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_name_their_socket_and_voice() {
        assert_eq!(
            RealtimeApi::from_path("/v1/text-to-speech/JBFqnCBsd6RMkjVDRZzb/stream-input"),
            Some((
                RealtimeApi::TextToSpeech,
                Some("JBFqnCBsd6RMkjVDRZzb".into())
            ))
        );
        assert_eq!(
            RealtimeApi::from_path("/v1/text-to-speech/v/multi-stream-input").map(|r| r.0),
            Some(RealtimeApi::TextToSpeechContexts)
        );
        assert_eq!(
            RealtimeApi::from_path("/v1/text-to-dialogue/multi-stream-input").map(|r| r.0),
            Some(RealtimeApi::TextToDialogueContexts)
        );
        assert_eq!(
            RealtimeApi::from_path("/v1/speech-to-text/realtime").map(|r| r.0),
            Some(RealtimeApi::SpeechToText)
        );
        assert_eq!(RealtimeApi::from_path("/v1/text-to-speech/v"), None);
        assert_eq!(RealtimeApi::from_path("/v1/convai/conversation"), None);
        assert_eq!(
            RealtimeApi::TextToSpeech.path(Some("abc123")).unwrap(),
            "/v1/text-to-speech/abc123/stream-input"
        );
        for voice in [Some(".."), Some("a/b"), Some(""), None] {
            let err = RealtimeApi::TextToSpeech.path(voice).unwrap_err();
            assert_eq!(err.param.as_deref(), Some("voice_id"));
        }
        assert!(RealtimeApi::TextToDialogue.path(None).is_ok());
        assert!(RealtimeApi::TextToDialogue.speaks() && !RealtimeApi::SpeechToText.speaks());
    }

    #[test]
    fn messages_lose_their_credentials_and_count_what_they_carry() {
        let m = read_client_message(
            RealtimeApi::TextToSpeech,
            r#"{"text":" ","xi_api_key":"sk-kso-abc","voice_settings":{"stability":0.5}}"#,
        )
        .unwrap();
        assert_eq!(m.credential.as_deref(), Some("sk-kso-abc"));
        assert_eq!(
            serde_json::from_str::<Value>(&m.forwarded).unwrap(),
            json!({"text": " ", "voice_settings": {"stability": 0.5}})
        );
        assert_eq!(m.characters, 1);
        let m = read_client_message(
            RealtimeApi::TextToSpeechContexts,
            r#"{"text":"Héllo ","context_id":"a","authorization":"Bearer sk-kso-x"}"#,
        )
        .unwrap();
        assert_eq!(
            (m.characters, m.credential.as_deref()),
            (6, Some("sk-kso-x"))
        );
        let m = read_client_message(
            RealtimeApi::TextToDialogue,
            r#"{"inputs":[{"text":"Hi.","voice_id":"a"},{"text":"Yes?","voice_id":"b"}]}"#,
        )
        .unwrap();
        assert_eq!(m.characters, 7);
        let m = read_client_message(
            RealtimeApi::SpeechToText,
            r#"{"message_type":"input_audio_chunk","audio_base_64":"AAAAAAAA","commit":false,"sample_rate":16000}"#,
        )
        .unwrap();
        assert_eq!((m.audio_bytes, m.sample_rate), (6, Some(16000)));
        assert!(read_client_message(RealtimeApi::SpeechToText, "[1]").is_err());
        assert!(read_client_message(RealtimeApi::SpeechToText, "not json").is_err());
    }

    #[test]
    fn audio_lengths_come_from_the_format() {
        assert_eq!(bytes_per_second("pcm_16000"), Some(32_000));
        assert_eq!(bytes_per_second("ulaw_8000"), Some(8_000));
        assert_eq!(bytes_per_second("mp3_44100_128"), None);
        assert_eq!(bytes_per_second("pcm"), None);
    }

    #[test]
    fn errors_take_each_sockets_shape() {
        let err = KppError::new(ErrorKind::RateLimited, "Rate limit reached.");
        let (body, code) = error_message(RealtimeApi::SpeechToText, &err);
        assert_eq!(code, 1008);
        assert_eq!(
            serde_json::from_str::<Value>(&body).unwrap(),
            json!({"message_type": "rate_limited", "error": "Rate limit reached."})
        );
        let err = KppError::invalid_request("Name the model.").with_param("model_id");
        let (body, code) = error_message(RealtimeApi::TextToDialogue, &err);
        assert_eq!(
            serde_json::from_str::<Value>(&body).unwrap(),
            json!({"message": "Name the model.", "error": "invalid_parameters", "code": 1008, "param": "model_id"})
        );
        assert_eq!(code, 1008);
        let (_, code) = error_message(RealtimeApi::TextToSpeech, &KppError::provider("down"));
        assert_eq!(code, 1011);
    }

    #[test]
    fn every_realtime_fixture_reads_and_forwards_without_its_credential() {
        let expect = [
            ("speech-to-text-chunk", RealtimeApi::SpeechToText, 0, 33),
            (
                "text-to-dialogue-inputs",
                RealtimeApi::TextToDialogue,
                30,
                0,
            ),
            (
                "text-to-speech-contexts",
                RealtimeApi::TextToSpeechContexts,
                13,
                0,
            ),
            ("text-to-speech-first", RealtimeApi::TextToSpeech, 1, 0),
            ("text-to-speech-text", RealtimeApi::TextToSpeech, 20, 0),
        ];
        let files = crate::test_support::fixtures("elevenlabs", "realtime", "json");
        assert_eq!(files.len(), expect.len());
        for ((name, body), (want, api, characters, audio)) in files.iter().zip(expect) {
            assert_eq!(name, want);
            let text = std::str::from_utf8(body).unwrap();
            let read = read_client_message(api, text).unwrap();
            assert_eq!(
                (read.characters, read.audio_bytes),
                (characters, audio),
                "{name}"
            );
            let mut sent: Value = serde_json::from_str(text).unwrap();
            if let Some(o) = sent.as_object_mut() {
                for f in CREDENTIAL_FIELDS {
                    o.remove(*f);
                }
            }
            crate::test_support::assert_json_eq(
                name,
                serde_json::to_vec(&sent).unwrap().as_slice(),
                read.forwarded.as_bytes(),
            );
        }
    }
}
