/// What a provider that speaks a form of OpenAI's transcription API
/// (`POST /v1/audio/transcriptions` and `/v1/audio/translations`) takes.
/// [`TranscriptionsProfile::OPENAI`] is OpenAI itself. Every other profile names its source on
/// the OpenAI Transcriptions mapping page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct TranscriptionsProfile {
    /// The provider's name in error messages.
    pub provider: &'static str,
    /// The transcriptions path, after the provider's base URL.
    pub transcriptions_path: &'static str,
    /// The translations path, when the provider translates.
    pub translations_path: Option<&'static str>,
    pub body: TranscriptionBody,
    /// The `response_format` values the provider documents. `verbose_json` stands for a reply
    /// with timed segments or words, and `json` for one with the text.
    pub formats: &'static [&'static str],
    /// Whether the provider takes `response_format`; one without answers one JSON form.
    pub format_field: bool,
    pub language: LanguageField,
    pub timestamps: TimestampsField,
    /// Whether the provider times single words, not only segments.
    pub words: bool,
    /// Whether the provider streams transcript events.
    pub streams: bool,
    /// The form field that takes a link to the audio instead of an upload.
    pub url_field: Option<&'static str>,
    /// The form field that takes a file id the provider issued.
    pub file_id_field: Option<&'static str>,
    /// Whether the provider takes `prompt`.
    pub prompt: bool,
    /// Whether the provider takes `temperature`.
    pub temperature: bool,
    /// Kaeso-typed fields from other providers' forms that this one takes: `diarize`.
    pub extensions: &'static [&'static str],
}

/// The body a provider takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TranscriptionBody {
    /// A `multipart/form-data` upload.
    Multipart,
    /// Vercel AI Gateway's transcription model protocol: JSON with base64 `audio` and its
    /// `mediaType`, the model in the `ai-model-id` header.
    AiSdk,
}

/// Where a provider takes the language spoken.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LanguageField {
    /// The `language` form field, optional.
    Field,
    /// The `language` form field, which the provider requires.
    Required,
    /// A `language` HTTP header, as MiniMax takes it.
    Header,
    /// Nowhere: the provider detects it.
    None,
}

/// How a provider takes the timing a transcript should have.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TimestampsField {
    /// A form field of granularities, one part per value, such as `timestamp_granularities[]`.
    Granularities(&'static str),
    /// MiniMax's `timestamp_level`: `sentence` or `word`.
    Level,
    /// None: every reply has timed words.
    Always,
    /// None: replies have no timing.
    None,
}

impl TranscriptionsProfile {
    /// OpenAI, from `CreateTranscriptionRequest` and `CreateTranslationRequest` at
    /// openai-openapi `926a7b8`.
    pub const OPENAI: TranscriptionsProfile = TranscriptionsProfile {
        provider: "OpenAI",
        transcriptions_path: "/v1/audio/transcriptions",
        translations_path: Some("/v1/audio/translations"),
        body: TranscriptionBody::Multipart,
        formats: &[
            "json",
            "text",
            "srt",
            "verbose_json",
            "vtt",
            "diarized_json",
        ],
        format_field: true,
        language: LanguageField::Field,
        timestamps: TimestampsField::Granularities("timestamp_granularities[]"),
        words: true,
        streams: true,
        url_field: None,
        file_id_field: None,
        prompt: true,
        temperature: true,
        extensions: &[],
    };

    /// Groq's Whisper models: an audio link in `url`, no subtitles and no streams.
    pub const GROQ: TranscriptionsProfile = TranscriptionsProfile {
        provider: "Groq",
        formats: &["json", "text", "verbose_json"],
        streams: false,
        url_field: Some("url"),
        ..TranscriptionsProfile::OPENAI
    };

    /// Together AI's Whisper, Parakeet and Nemotron models: `json` and `verbose_json`, a link in
    /// `file`, speakers with `diarize`.
    pub const TOGETHER: TranscriptionsProfile = TranscriptionsProfile {
        provider: "Together AI",
        formats: &["json", "verbose_json"],
        timestamps: TimestampsField::Granularities("timestamp_granularities"),
        streams: false,
        url_field: Some("file"),
        extensions: &["diarize"],
        ..TranscriptionsProfile::OPENAI
    };

    /// Mistral's Voxtral Transcribe: one JSON form with timed segments when asked, links in
    /// `file_url`, file ids in `file_id`, `transcription.*` events.
    pub const MISTRAL: TranscriptionsProfile = TranscriptionsProfile {
        provider: "Mistral",
        translations_path: None,
        formats: &["json", "verbose_json"],
        format_field: false,
        timestamps: TimestampsField::Granularities("timestamp_granularities"),
        words: false,
        url_field: Some("file_url"),
        file_id_field: Some("file_id"),
        prompt: false,
        extensions: &["diarize"],
        ..TranscriptionsProfile::OPENAI
    };

    /// Z.ai's GLM-ASR, served from the root of the host: JSON text only, no language.
    pub const ZAI: TranscriptionsProfile = TranscriptionsProfile {
        provider: "Z.ai",
        transcriptions_path: "/api/paas/v4/audio/transcriptions",
        translations_path: None,
        formats: &["json"],
        format_field: false,
        language: LanguageField::None,
        timestamps: TimestampsField::None,
        words: false,
        temperature: false,
        ..TranscriptionsProfile::OPENAI
    };

    /// SpaceX's `POST /v1/stt`: timed words in every reply, links in `url`, no streams.
    pub const XAI: TranscriptionsProfile = TranscriptionsProfile {
        provider: "SpaceX",
        transcriptions_path: "/v1/stt",
        translations_path: None,
        formats: &["json", "verbose_json"],
        format_field: false,
        timestamps: TimestampsField::Always,
        streams: false,
        url_field: Some("url"),
        prompt: false,
        temperature: false,
        extensions: &["diarize"],
        ..TranscriptionsProfile::OPENAI
    };

    /// OpenRouter's `POST /api/v1/audio/transcriptions`.
    pub const OPENROUTER: TranscriptionsProfile = TranscriptionsProfile {
        provider: "OpenRouter",
        translations_path: None,
        formats: &["json", "verbose_json"],
        streams: false,
        url_field: Some("source_url"),
        prompt: false,
        extensions: &["diarize"],
        ..TranscriptionsProfile::OPENAI
    };

    /// Cohere's `POST /v2/audio/transcriptions`, served from the root of the host: `language`
    /// is required, and the reply is the text alone.
    pub const COHERE: TranscriptionsProfile = TranscriptionsProfile {
        provider: "Cohere",
        transcriptions_path: "/v2/audio/transcriptions",
        translations_path: None,
        formats: &["json"],
        format_field: false,
        language: LanguageField::Required,
        timestamps: TimestampsField::None,
        words: false,
        streams: false,
        prompt: false,
        ..TranscriptionsProfile::OPENAI
    };

    /// MiniMax's `POST /v1/speech_to_text`, served from the root of the host: the language in
    /// a header, `timestamp_level`, subtitles and speakers in its timed forms.
    pub const MINIMAX: TranscriptionsProfile = TranscriptionsProfile {
        provider: "MiniMax",
        transcriptions_path: "/v1/speech_to_text",
        translations_path: None,
        formats: &["json", "verbose_json", "srt", "vtt"],
        language: LanguageField::Header,
        timestamps: TimestampsField::Level,
        prompt: false,
        temperature: false,
        ..TranscriptionsProfile::OPENAI
    };

    /// Vercel AI Gateway's transcription models, `POST /v4/ai/transcription-model`.
    pub const VERCEL: TranscriptionsProfile = TranscriptionsProfile {
        provider: "Vercel",
        transcriptions_path: "/v4/ai/transcription-model",
        translations_path: None,
        body: TranscriptionBody::AiSdk,
        formats: &["json"],
        format_field: false,
        language: LanguageField::None,
        timestamps: TimestampsField::None,
        words: false,
        streams: false,
        prompt: false,
        temperature: false,
        ..TranscriptionsProfile::OPENAI
    };

    /// Whether the provider takes the Kaeso-typed field `name` beyond OpenAI's.
    pub fn takes(&self, name: &str) -> bool {
        self.extensions.contains(&name)
    }

    pub(crate) fn has_format(&self, wire: &str) -> bool {
        self.formats.contains(&wire)
    }
}
