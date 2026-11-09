/// What a provider that speaks a form of OpenAI's Videos API takes: its path, body form and
/// the fields it lacks. [`VideosProfile::OPENAI`] is OpenAI itself. Every other profile names
/// its source on the OpenAI Videos mapping page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct VideosProfile {
    /// The provider's name in error messages.
    pub provider: &'static str,
    /// The path of the video jobs, after the provider's base URL; a job's own path is this
    /// path and its id.
    pub path: &'static str,
    pub body: VideosBody,
    /// Whether the provider documents `DELETE` on a job's path.
    pub deletes: bool,
}

/// The body a provider takes and the job it answers with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum VideosBody {
    /// OpenAI's `CreateVideoJsonBody` or `CreateVideoMultipartBody`, answered with a
    /// `VideoResource`.
    OpenAi,
    /// Together AI's `CreateVideoBody` (`width`, `height`, `resolution`, `ratio`, `seconds`,
    /// `seed`, `negative_prompt`, `generate_audio`, `media`), answered with its `VideoJob`.
    Together,
}

impl VideosProfile {
    /// OpenAI, from the Videos API at openai-openapi `926a7b8`. OpenAI shut its video models
    /// down on 2026-09-24, so no built-in provider sends video jobs in this form; it is the
    /// form apps speak.
    pub const OPENAI: VideosProfile = VideosProfile {
        provider: "OpenAI",
        path: "/v1/videos",
        body: VideosBody::OpenAi,
        deletes: true,
    };

    /// Together AI's `POST /v2/videos` and `GET /v2/videos/{id}`
    /// (<https://docs.together.ai/reference/create-videos>), which documents no delete.
    pub const TOGETHER: VideosProfile = VideosProfile {
        provider: "Together AI",
        path: "/v2/videos",
        body: VideosBody::Together,
        deletes: false,
    };
}
