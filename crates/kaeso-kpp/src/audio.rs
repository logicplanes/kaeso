use serde::{Deserialize, Serialize};

use crate::block::Source;
use crate::error::KppError;
use crate::extras::Extras;
use crate::request::{AudioFormat, StreamMode};
use crate::response::Usage;

/// What a request for generated audio asks for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SpeechOperation {
    /// Speech from text, in one voice.
    #[default]
    Speech,
    /// A dialogue: lines of text, each in its own voice.
    Dialogue,
    /// A sound effect from a description.
    SoundEffect,
    /// Music from a description.
    Music,
    /// Speech from given audio, in another voice.
    VoiceChange,
}

impl SpeechOperation {
    /// The operation's name in logs and messages.
    pub const fn as_str(self) -> &'static str {
        match self {
            SpeechOperation::Speech => "speech",
            SpeechOperation::Dialogue => "dialogue",
            SpeechOperation::SoundEffect => "sound_effect",
            SpeechOperation::Music => "music",
            SpeechOperation::VoiceChange => "voice_change",
        }
    }
}

/// The voice to speak in: one of the provider's named voices, or a voice by its id.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Voice {
    /// A voice the provider names, such as OpenAI's `alloy` or Gemini's `Kore`.
    Named(String),
    /// A voice by its id at the provider, such as a custom voice.
    Id(String),
}

impl Voice {
    /// The name or id as written.
    pub fn as_str(&self) -> &str {
        match self {
            Voice::Named(s) | Voice::Id(s) => s,
        }
    }
}

/// One line of a dialogue.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DialogueLine {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice: Option<Voice>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl DialogueLine {
    pub fn new(text: impl Into<String>, voice: Option<Voice>) -> Self {
        DialogueLine {
            text: text.into(),
            voice,
            extras: Extras::default(),
        }
    }
}

/// Audio a request sends: inline bytes, a URL or a provider file id.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AudioInput {
    pub source: Source,
    /// The file name an upload carried.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl AudioInput {
    pub fn new(source: Source) -> Self {
        AudioInput {
            source,
            filename: None,
            extras: Extras::default(),
        }
    }

    #[must_use]
    pub fn with_filename(mut self, filename: impl Into<String>) -> Self {
        self.filename = Some(filename.into());
        self
    }

    /// The size of inline bytes, from their base64 length; `None` for a link or a file id.
    pub fn byte_len(&self) -> Option<u64> {
        match &self.source {
            Source::Base64(b) => {
                let data = b.data.trim_end_matches('=');
                Some((data.len() as u64).saturating_mul(3) / 4)
            }
            _ => None,
        }
    }
}

/// A request for generated audio: speech, a dialogue, a sound effect, music, or speech in
/// another voice.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SpeechRequest {
    pub model: String,
    #[serde(default)]
    pub operation: SpeechOperation,
    /// The text to speak, or the description of a sound effect or of music.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<String>,
    /// The lines of a dialogue.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dialogue: Vec<DialogueLine>,
    /// The audio to speak again in another voice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<AudioInput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice: Option<Voice>,
    /// How to speak: tone, pace, accent, as free text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    /// The container and codec of the audio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<AudioFormat>,
    /// The sample rate in hertz.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<u32>,
    /// The bit rate in kilobits per second, for compressed formats.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bitrate: Option<u32>,
    /// The speaking rate, where 1 is the voice's usual pace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<f64>,
    /// The language of the text, as an ISO 639 code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
    /// The length of a sound effect or of music, in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    /// Whether the reply also gives the time at which each character is spoken.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub timestamps: bool,
    /// `On` asks for audio events; `Off` for the audio itself, which a provider may still send
    /// as it is made.
    #[serde(default, skip_serializing_if = "StreamMode::is_off")]
    pub stream: StreamMode,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl SpeechRequest {
    pub fn new(model: impl Into<String>, operation: SpeechOperation) -> Self {
        SpeechRequest {
            model: model.into(),
            operation,
            ..SpeechRequest::default()
        }
    }

    #[must_use]
    pub fn with_input(mut self, input: impl Into<String>) -> Self {
        self.input = Some(input.into());
        self
    }

    #[must_use]
    pub fn with_voice(mut self, voice: Voice) -> Self {
        self.voice = Some(voice);
        self
    }

    /// The characters of text the request asks to speak, which providers bill by.
    pub fn characters(&self) -> u64 {
        let input = self.input.as_deref().map_or(0, |s| s.chars().count());
        let lines: usize = self.dialogue.iter().map(|l| l.text.chars().count()).sum();
        (input + lines) as u64
    }

    /// Checks what every request for audio needs, whatever its format.
    pub fn validate(&self) -> Result<(), KppError> {
        let invalid = |message: &str, param: &str| {
            Err(KppError::invalid_request(message.to_owned()).with_param(param.to_owned()))
        };
        if self.model.is_empty() {
            return invalid("`model` must not be empty.", "model");
        }
        let input = self.input.as_deref().is_some_and(|s| !s.is_empty());
        match self.operation {
            SpeechOperation::Speech | SpeechOperation::SoundEffect if !input => {
                return invalid("`input` must not be empty.", "input");
            }
            SpeechOperation::Dialogue if self.dialogue.is_empty() => {
                return invalid("Send at least one line of dialogue.", "inputs");
            }
            SpeechOperation::VoiceChange if self.audio.is_none() => {
                return invalid("Send the audio to speak in another voice.", "audio");
            }
            _ => {}
        }
        if self.speed.is_some_and(|s| !(s.is_finite() && s > 0.0)) {
            return invalid("`speed` must be a positive number.", "speed");
        }
        if self.duration.is_some_and(|d| !(d.is_finite() && d > 0.0)) {
            return invalid(
                "The duration must be a positive number of seconds.",
                "duration",
            );
        }
        Ok(())
    }

    /// What a model must be able to do to serve the request.
    pub fn required_capabilities(&self) -> crate::Capabilities {
        crate::Capabilities {
            audio_input: self.audio.is_some(),
            audio_output: true,
            streaming: !self.stream.is_off(),
            ..crate::Capabilities::default()
        }
    }
}

/// When each character of spoken text starts and ends, in seconds from the start of the audio.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SpeechAlignment {
    pub characters: Vec<String>,
    pub starts: Vec<f64>,
    pub ends: Vec<f64>,
}

impl SpeechAlignment {
    pub fn new(characters: Vec<String>, starts: Vec<f64>, ends: Vec<f64>) -> Self {
        SpeechAlignment {
            characters,
            starts,
            ends,
        }
    }
}

/// The reply to a request for audio.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SpeechResponse {
    pub model: String,
    /// The audio, base64-encoded with its media type.
    pub audio: Source,
    /// When each character is spoken, for a request that asked for timestamps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alignment: Option<SpeechAlignment>,
    /// The characters the provider billed, when it says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub characters: Option<u64>,
    pub usage: Usage,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl SpeechResponse {
    pub fn new(model: impl Into<String>, audio: Source) -> Self {
        SpeechResponse {
            model: model.into(),
            audio,
            alignment: None,
            characters: None,
            usage: Usage::default(),
            extras: Extras::default(),
        }
    }
}

/// One event of streamed audio: a piece of the audio, or the end with its usage.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SpeechEvent {
    /// A piece of the audio, base64-encoded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<String>,
    /// The media type of the audio, on the first piece when the provider names it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    /// When each character of this piece is spoken.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alignment: Option<SpeechAlignment>,
    /// Whether the audio is complete.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub done: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub characters: Option<u64>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl SpeechEvent {
    /// A piece of audio, base64-encoded.
    pub fn audio(data: impl Into<String>) -> Self {
        SpeechEvent {
            audio: Some(data.into()),
            ..SpeechEvent::default()
        }
    }

    /// The end of the audio.
    pub fn done(usage: Option<Usage>) -> Self {
        SpeechEvent {
            done: true,
            usage,
            ..SpeechEvent::default()
        }
    }
}

/// What a transcription request asks for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TranscriptionOperation {
    /// The text in the language spoken.
    #[default]
    Transcribe,
    /// The text in English, whatever the language spoken.
    Translate,
}

impl TranscriptionOperation {
    pub const fn as_str(self) -> &'static str {
        match self {
            TranscriptionOperation::Transcribe => "transcribe",
            TranscriptionOperation::Translate => "translate",
        }
    }
}

wire_enum!(
    /// The form of a transcript: JSON with the text, plain text, subtitles, or JSON with
    /// segments, words or speakers.
    TranscriptFormat {
        Json = "json",
        Text = "text",
        Srt = "srt",
        VerboseJson = "verbose_json",
        Vtt = "vtt",
        DiarizedJson = "diarized_json",
    }
);

impl TranscriptFormat {
    /// Whether the transcript is plain text rather than JSON.
    pub fn is_text(&self) -> bool {
        matches!(
            self,
            TranscriptFormat::Text | TranscriptFormat::Srt | TranscriptFormat::Vtt
        )
    }

    /// Whether the form needs the time of each segment.
    pub fn needs_timing(&self) -> bool {
        matches!(
            self,
            TranscriptFormat::Srt
                | TranscriptFormat::Vtt
                | TranscriptFormat::VerboseJson
                | TranscriptFormat::DiarizedJson
        )
    }
}

wire_enum!(
    /// How finely a transcript is timed.
    TimestampGranularity {
        Word = "word",
        Segment = "segment",
    }
);

/// A request to transcribe or translate audio.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TranscriptionRequest {
    pub model: String,
    #[serde(default)]
    pub operation: TranscriptionOperation,
    pub audio: AudioInput,
    /// The language spoken, as an ISO 639 code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Text that guides the transcript's spelling and style.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<TranscriptFormat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub timestamps: Vec<TimestampGranularity>,
    /// Whether to tell the speakers apart.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diarize: Option<bool>,
    /// The most speakers the audio holds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speakers: Option<u32>,
    #[serde(default, skip_serializing_if = "StreamMode::is_off")]
    pub stream: StreamMode,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl TranscriptionRequest {
    pub fn new(
        model: impl Into<String>,
        operation: TranscriptionOperation,
        audio: AudioInput,
    ) -> Self {
        TranscriptionRequest {
            model: model.into(),
            operation,
            audio,
            language: None,
            prompt: None,
            format: None,
            temperature: None,
            timestamps: Vec::new(),
            diarize: None,
            speakers: None,
            stream: StreamMode::Off,
            extras: Extras::default(),
        }
    }

    /// The transcript's form, `json` when the request names none.
    pub fn format_or_default(&self) -> TranscriptFormat {
        self.format.clone().unwrap_or(TranscriptFormat::Json)
    }

    /// Checks what every transcription request needs, whatever its format.
    pub fn validate(&self) -> Result<(), KppError> {
        let invalid = |message: &str, param: &str| {
            Err(KppError::invalid_request(message.to_owned()).with_param(param.to_owned()))
        };
        if self.model.is_empty() {
            return invalid("`model` must not be empty.", "model");
        }
        if self
            .temperature
            .is_some_and(|t| !(t.is_finite() && (0.0..=2.0).contains(&t)))
        {
            return invalid("`temperature` must be between 0 and 2.", "temperature");
        }
        if self.speakers == Some(0) {
            return invalid("The number of speakers must be at least 1.", "num_speakers");
        }
        Ok(())
    }

    /// What a model must be able to do to serve the request.
    pub fn required_capabilities(&self) -> crate::Capabilities {
        crate::Capabilities {
            audio_input: true,
            streaming: !self.stream.is_off(),
            ..crate::Capabilities::default()
        }
    }
}

/// A timed stretch of a transcript.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TranscriptSegment {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The start, in seconds from the start of the audio.
    pub start: f64,
    pub end: f64,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker: Option<String>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl TranscriptSegment {
    pub fn new(start: f64, end: f64, text: impl Into<String>) -> Self {
        TranscriptSegment {
            start,
            end,
            text: text.into(),
            ..TranscriptSegment::default()
        }
    }
}

/// A timed word of a transcript.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TranscriptWord {
    pub text: String,
    pub start: f64,
    pub end: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker: Option<String>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl TranscriptWord {
    pub fn new(text: impl Into<String>, start: f64, end: f64) -> Self {
        TranscriptWord {
            text: text.into(),
            start,
            end,
            ..TranscriptWord::default()
        }
    }
}

/// The reply to a transcription request.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TranscriptionResponse {
    pub model: String,
    /// The transcript: plain text, or the subtitles of `format`.
    pub text: String,
    /// The form `text` came in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<TranscriptFormat>,
    /// The language detected, as the provider names it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// The length of the audio in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub segments: Vec<TranscriptSegment>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub words: Vec<TranscriptWord>,
    pub usage: Usage,
    /// The seconds of audio the provider billed, for providers that bill by duration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage_seconds: Option<f64>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl TranscriptionResponse {
    pub fn new(model: impl Into<String>, text: impl Into<String>) -> Self {
        TranscriptionResponse {
            model: model.into(),
            text: text.into(),
            ..TranscriptionResponse::default()
        }
    }

    /// The length of the audio, from the reply's duration, its billed seconds or its last
    /// timed segment or word.
    pub fn audio_seconds(&self) -> Option<f64> {
        self.duration.or(self.usage_seconds).or_else(|| {
            let segments = self.segments.iter().map(|s| s.end);
            let words = self.words.iter().map(|w| w.end);
            segments.chain(words).reduce(f64::max)
        })
    }
}

/// One event of a streamed transcript: a piece of text, a finished segment, or the end.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TranscriptEvent {
    /// More text of the transcript.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delta: Option<String>,
    /// The segment the text belongs to, for providers that name it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segment_id: Option<String>,
    /// A finished segment, with its time and speaker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segment: Option<TranscriptSegment>,
    /// The whole transcript, at the end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub done: Option<Box<TranscriptionResponse>>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl TranscriptEvent {
    pub fn delta(text: impl Into<String>) -> Self {
        TranscriptEvent {
            delta: Some(text.into()),
            ..TranscriptEvent::default()
        }
    }

    pub fn segment(segment: TranscriptSegment) -> Self {
        TranscriptEvent {
            segment: Some(segment),
            ..TranscriptEvent::default()
        }
    }

    pub fn done(transcript: TranscriptionResponse) -> Self {
        TranscriptEvent {
            done: Some(Box::new(transcript)),
            ..TranscriptEvent::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav() -> AudioInput {
        AudioInput::new(Source::base64("audio/wav", "UklGRg==")).with_filename("a.wav")
    }

    #[test]
    fn speech_requests_need_what_their_operation_needs() {
        let param = |r: &SpeechRequest| r.validate().unwrap_err().param.unwrap();
        assert_eq!(
            param(&SpeechRequest::new("", SpeechOperation::Speech)),
            "model"
        );
        assert_eq!(
            param(&SpeechRequest::new("m", SpeechOperation::Speech)),
            "input"
        );
        let ok = SpeechRequest::new("m", SpeechOperation::Speech).with_input("Hi");
        ok.validate().unwrap();
        assert_eq!(
            param(&SpeechRequest::new("m", SpeechOperation::Dialogue)),
            "inputs"
        );
        let mut dialogue = SpeechRequest::new("m", SpeechOperation::Dialogue);
        dialogue
            .dialogue
            .push(DialogueLine::new("Hi", Some(Voice::Id("v1".into()))));
        dialogue.validate().unwrap();
        assert_eq!(
            param(&SpeechRequest::new("m", SpeechOperation::VoiceChange)),
            "audio"
        );
        SpeechRequest::new("m", SpeechOperation::Music)
            .validate()
            .unwrap();
        let mut fast = ok.clone();
        fast.speed = Some(0.0);
        assert_eq!(param(&fast), "speed");
        let mut long = ok.clone();
        long.duration = Some(f64::NAN);
        assert_eq!(param(&long), "duration");
        let mut change = SpeechRequest::new("m", SpeechOperation::VoiceChange);
        change.audio = Some(wav());
        let caps = change.required_capabilities();
        assert!(caps.audio_input && caps.audio_output && !caps.streaming);
        assert_eq!(dialogue.characters() + ok.characters(), 4);
        assert_eq!(SpeechOperation::SoundEffect.as_str(), "sound_effect");
        assert_eq!(Voice::Named("alloy".into()).as_str(), "alloy");
    }

    #[test]
    fn transcription_requests_check_their_ranges() {
        let ok = TranscriptionRequest::new("m", TranscriptionOperation::Transcribe, wav());
        ok.validate().unwrap();
        assert_eq!(ok.format_or_default(), TranscriptFormat::Json);
        let param = |r: &TranscriptionRequest| r.validate().unwrap_err().param.unwrap();
        let mut hot = ok.clone();
        hot.temperature = Some(3.0);
        assert_eq!(param(&hot), "temperature");
        let mut none = ok.clone();
        none.speakers = Some(0);
        assert_eq!(param(&none), "num_speakers");
        let mut nameless = ok.clone();
        nameless.model.clear();
        assert_eq!(param(&nameless), "model");
        assert!(ok.required_capabilities().audio_input);
        assert_eq!(wav().byte_len(), Some(4));
        assert_eq!(
            AudioInput::new(Source::Url("https://example.com/a.mp3".into())).byte_len(),
            None
        );
        assert_eq!(TranscriptionOperation::Translate.as_str(), "translate");
    }

    #[test]
    fn transcript_formats_round_trip_and_know_their_kind() {
        for s in [
            "json",
            "text",
            "srt",
            "verbose_json",
            "vtt",
            "diarized_json",
            "tsv",
        ] {
            assert_eq!(TranscriptFormat::from_wire(s).as_wire(), s);
        }
        assert!(TranscriptFormat::Srt.is_text() && !TranscriptFormat::Json.is_text());
        assert!(TranscriptFormat::Vtt.needs_timing() && !TranscriptFormat::Text.needs_timing());
        assert_eq!(
            TimestampGranularity::from_wire("word"),
            TimestampGranularity::Word
        );
    }

    #[test]
    fn the_audio_length_comes_from_the_best_field_the_reply_has() {
        let mut r = TranscriptionResponse::new("m", "hi");
        assert_eq!(r.audio_seconds(), None);
        r.words.push(TranscriptWord::new("hi", 0.0, 0.5));
        r.segments.push(TranscriptSegment::new(0.0, 0.4, "hi"));
        assert_eq!(r.audio_seconds(), Some(0.5));
        r.usage_seconds = Some(2.0);
        assert_eq!(r.audio_seconds(), Some(2.0));
        r.duration = Some(1.5);
        assert_eq!(r.audio_seconds(), Some(1.5));
    }

    #[test]
    fn events_have_small_stable_forms() {
        let json = serde_json::to_string(&SpeechEvent::audio("AAAA")).unwrap();
        assert_eq!(json, r#"{"audio":"AAAA"}"#);
        let json = serde_json::to_string(&SpeechEvent::done(None)).unwrap();
        assert_eq!(json, r#"{"done":true}"#);
        let json = serde_json::to_string(&TranscriptEvent::delta("Hel")).unwrap();
        assert_eq!(json, r#"{"delta":"Hel"}"#);
        let seg = TranscriptEvent::segment(TranscriptSegment::new(0.0, 1.0, "Hi"));
        assert!(serde_json::to_string(&seg).unwrap().contains("\"segment\""));
        let done = TranscriptEvent::done(TranscriptionResponse::new("m", "Hi"));
        assert_eq!(done.done.unwrap().text, "Hi");
        let r = SpeechResponse::new("m", Source::base64("audio/mpeg", "AA=="));
        let back: SpeechResponse =
            serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
        assert_eq!(back, r);
    }
}
