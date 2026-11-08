/// What a provider that speaks a form of OpenAI's speech API (`POST /v1/audio/speech`)
/// takes: its path and body form, voices, formats and the fields it lacks.
/// [`SpeechProfile::OPENAI`] is OpenAI itself. Every other profile names its source on the
/// OpenAI Speech mapping page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct SpeechProfile {
    /// The provider's name in error messages.
    pub provider: &'static str,
    /// The speech path, after the provider's base URL.
    pub path: &'static str,
    pub body: SpeechBody,
    pub reply: SpeechReply,
    /// The voice names the provider documents, or `None` when its voices depend on the model
    /// and any name is sent as it is.
    pub voices: Option<&'static [&'static str]>,
    /// Whether a voice may be named by its id, as OpenAI's `{"id": "voice_…"}`.
    pub voice_ids: bool,
    /// The `response_format` values the provider documents.
    pub formats: &'static [&'static str],
    /// The format the provider answers in when the request names none.
    pub default_format: &'static str,
    /// How the provider streams audio events, if it does.
    pub events: Option<SpeechEvents>,
    /// Models, by id prefix, that stream no events, as OpenAI's `tts-1` and `tts-1-hd`.
    pub sse_refused: &'static [&'static str],
    /// Whether the provider reports usage only in its events, so that Kaeso asks for events
    /// even when the app wants the audio itself.
    pub usage_in_events: bool,
    /// Whether the provider takes `instructions`.
    pub instructions: bool,
    /// Whether the provider takes `speed`.
    pub speed: bool,
    /// Kaeso-typed fields from other providers' forms that this one takes: `sample_rate`,
    /// `language` and `seed`.
    pub extensions: &'static [&'static str],
}

/// The body a provider takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SpeechBody {
    /// OpenAI's `CreateSpeechRequest`: `model`, `input`, `voice`, `response_format`, `speed`.
    OpenAi,
    /// OpenAI's body with the voice in `voice_id`, as Mistral's `SpeechRequest`.
    VoiceId,
    /// SpaceX's `POST /v1/tts`: `text`, `voice_id`, a required `language` and
    /// `output_format {codec, sample_rate}`, without a model.
    Xai,
    /// Vercel AI Gateway's speech model protocol: the model in the `ai-model-id` header, and
    /// `text`, `voice`, `outputFormat`, `instructions`, `speed`, `language` in the body.
    AiSdk,
}

/// What a provider answers with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SpeechReply {
    /// The audio itself, sent as it is made.
    Bytes,
    /// A JSON object with the audio base64-encoded under this key.
    JsonBase64(&'static str),
}

/// How a provider streams audio as server-sent events.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SpeechEvents {
    /// `stream_format: "sse"`; `speech.audio.delta` events carry `audio`.
    StreamFormat,
    /// `stream: true`; `speech.audio.delta` events carry the audio under this key.
    StreamFlag(&'static str),
}

impl SpeechProfile {
    /// OpenAI, from `CreateSpeechRequest` at openai-openapi `926a7b8`.
    pub const OPENAI: SpeechProfile = SpeechProfile {
        provider: "OpenAI",
        path: "/v1/audio/speech",
        body: SpeechBody::OpenAi,
        reply: SpeechReply::Bytes,
        voices: Some(crate::media::OPENAI_VOICES),
        voice_ids: true,
        formats: &["mp3", "opus", "aac", "flac", "wav", "pcm"],
        default_format: "mp3",
        events: Some(SpeechEvents::StreamFormat),
        sse_refused: &["tts-1"],
        usage_in_events: true,
        instructions: true,
        speed: true,
        extensions: &[],
    };

    /// Groq's Orpheus models. The Orpheus guide names `wav` as the only format, while the API
    /// reference lists more; Kaeso takes the guide's narrower word.
    pub const GROQ: SpeechProfile = SpeechProfile {
        provider: "Groq",
        voices: None,
        voice_ids: false,
        formats: &["wav"],
        default_format: "wav",
        events: None,
        sse_refused: &[],
        usage_in_events: false,
        instructions: false,
        extensions: &["sample_rate"],
        ..SpeechProfile::OPENAI
    };

    /// Together AI's `AudioSpeechRequest`: Orpheus, Cartesia Sonic and Kokoro, voices per model.
    pub const TOGETHER: SpeechProfile = SpeechProfile {
        provider: "Together AI",
        voices: None,
        voice_ids: false,
        formats: &["mp3", "wav"],
        default_format: "wav",
        events: None,
        sse_refused: &[],
        usage_in_events: false,
        instructions: false,
        speed: false,
        extensions: &["sample_rate", "language"],
        ..SpeechProfile::OPENAI
    };

    /// Mistral's Voxtral TTS: `voice_id`, a JSON reply with base64 `audio_data`, and
    /// `speech.audio.*` events with `stream: true`.
    pub const MISTRAL: SpeechProfile = SpeechProfile {
        provider: "Mistral",
        body: SpeechBody::VoiceId,
        reply: SpeechReply::JsonBase64("audio_data"),
        voices: None,
        voice_ids: true,
        formats: &["pcm", "wav", "mp3", "flac", "opus"],
        default_format: "mp3",
        events: Some(SpeechEvents::StreamFlag("audio_data")),
        sse_refused: &[],
        usage_in_events: false,
        instructions: false,
        speed: false,
        extensions: &[],
        ..SpeechProfile::OPENAI
    };

    /// OpenRouter's `SpeechRequest`: `mp3` or `pcm`, `pcm` when none is named.
    pub const OPENROUTER: SpeechProfile = SpeechProfile {
        provider: "OpenRouter",
        voices: None,
        voice_ids: false,
        formats: &["mp3", "pcm"],
        default_format: "pcm",
        events: None,
        sse_refused: &[],
        usage_in_events: false,
        ..SpeechProfile::OPENAI
    };

    /// SpaceX's `POST /v1/tts`.
    pub const XAI: SpeechProfile = SpeechProfile {
        provider: "SpaceX",
        path: "/v1/tts",
        body: SpeechBody::Xai,
        voices: Some(XAI_VOICES),
        voice_ids: true,
        formats: &["mp3", "wav", "pcm"],
        default_format: "mp3",
        events: None,
        sse_refused: &[],
        usage_in_events: false,
        instructions: false,
        extensions: &["sample_rate", "language"],
        ..SpeechProfile::OPENAI
    };

    /// Vercel AI Gateway's speech models, `POST /v4/ai/speech-model`, whose reply carries the
    /// audio base64-encoded under `audio`. Only `mp3` is documented.
    pub const VERCEL: SpeechProfile = SpeechProfile {
        provider: "Vercel",
        path: "/v4/ai/speech-model",
        body: SpeechBody::AiSdk,
        reply: SpeechReply::JsonBase64("audio"),
        voices: None,
        voice_ids: true,
        formats: &["mp3"],
        default_format: "mp3",
        events: None,
        sse_refused: &[],
        usage_in_events: false,
        extensions: &["language"],
        ..SpeechProfile::OPENAI
    };

    /// Whether the provider takes the Kaeso-typed field `name` beyond OpenAI's.
    pub fn takes(&self, name: &str) -> bool {
        self.extensions.contains(&name)
    }
}

// The voice ids of SpaceX's `GET /v1/tts/voices` reference example (docs.x.ai, read
// 2026-11-08); custom voice ids pass as ids.
const XAI_VOICES: &[&str] = &[
    "carina", "zagan", "helix", "orion", "luna", "iris", "altair", "zenith", "perseus", "helios",
    "lux", "kepler", "rigel", "cosmo", "celeste", "ursa", "sirius", "lumen", "castor", "naksh",
    "atlas", "ara", "eve", "leo", "rex", "sal",
];
