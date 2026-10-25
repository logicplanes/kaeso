/// How a provider's OpenAI Responses API differs from OpenAI's own.
///
/// [`ResponsesProfile::OPENAI`] sends a request as the app wrote it. Other providers
/// start from [`ResponsesProfile::compatible`] and switch off what they refuse.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ResponsesProfile {
    /// The provider's name, used in error messages.
    pub provider: &'static str,
    /// Whether `namespace` tools reach the provider; otherwise they become function tools.
    pub tool_namespaces: bool,
    /// The shape of the hosted `web_search` tool the provider takes.
    pub web_search: WebSearchTool,
    /// How reasoning items the app sends back are written.
    pub reasoning_replay: ReasoningReplay,
    /// Whether reasoning items with OpenAI's `encrypted_content` reach the provider;
    /// otherwise they are left out.
    pub openai_encrypted_reasoning: bool,
}

/// The hosted `web_search` tool a Responses provider takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum WebSearchTool {
    /// OpenAI's tool, sent as the app wrote it.
    AsSent,
    /// xAI's tool: `filters`, `enable_image_understanding` and `enable_image_search`, and
    /// no `external_web_access`.
    Xai,
}

/// How reasoning items the app sends back reach a Responses provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ReasoningReplay {
    /// As the app wrote them.
    AsSent,
    /// Without an explicit `content: null`.
    WithoutNullContent,
}

impl ResponsesProfile {
    /// OpenAI's Responses API.
    pub const OPENAI: ResponsesProfile = ResponsesProfile {
        provider: "OpenAI",
        tool_namespaces: true,
        web_search: WebSearchTool::AsSent,
        reasoning_replay: ReasoningReplay::AsSent,
        openai_encrypted_reasoning: true,
    };

    /// Another provider's Responses API: as OpenAI's, but without tool namespaces, which
    /// only OpenAI documents.
    pub const fn compatible(provider: &'static str) -> Self {
        ResponsesProfile {
            provider,
            tool_namespaces: false,
            ..ResponsesProfile::OPENAI
        }
    }

    /// The provider takes xAI's hosted `web_search` tool.
    #[must_use]
    pub const fn xai_web_search(mut self) -> Self {
        self.web_search = WebSearchTool::Xai;
        self
    }

    /// The provider refuses reasoning items with `content: null`.
    #[must_use]
    pub const fn without_null_reasoning_content(mut self) -> Self {
        self.reasoning_replay = ReasoningReplay::WithoutNullContent;
        self
    }

    /// The provider refuses OpenAI's encrypted reasoning, so reasoning items the app sends
    /// back with it are left out.
    #[must_use]
    pub const fn without_openai_encrypted_reasoning(mut self) -> Self {
        self.openai_encrypted_reasoning = false;
        self
    }
}
