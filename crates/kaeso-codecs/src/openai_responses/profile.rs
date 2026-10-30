use crate::ToolSchemaRoot;

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
    /// What the provider takes at the root of a function tool's parameter schema.
    pub tool_schema_root: ToolSchemaRoot,
    /// Which input item types reach the provider.
    pub input_items: InputItems,
    /// Whether the provider takes `tool_choice` in a request without tools; otherwise `auto`
    /// and `none` are left out of such a request and a choice that forces a tool is refused.
    pub tool_choice_without_tools: bool,
}

/// Which input item types a Responses provider takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum InputItems {
    /// Every type, as the app wrote it.
    AsSent,
    /// Every type but Codex's `agent_message`, which only OpenAI reads: each becomes a message.
    WithoutAgentMessages,
    /// Only these types. `agent_message` becomes a message, Codex's `local_shell_call` and
    /// `local_shell_call_output` become a function call and its output, and any other type
    /// is refused before the request is sent.
    Only(&'static [&'static str]),
}

/// The input item types xAI's Responses API takes: the types its 422 lists for any other
/// type (from a production refusal on 2026-10-29), and `custom_tool_call_output`, which
/// that list leaves out but xAI answered in
/// `providers/xai/responses/recorded-codex-gpt-history.json`.
pub const XAI_INPUT_ITEMS: &[&str] = &[
    "message",
    "reasoning",
    "function_call",
    "function_call_output",
    "shell_call",
    "shell_call_output",
    "web_search_call",
    "file_search_call",
    "code_interpreter_call",
    "mcp_call",
    "custom_tool_call",
    "custom_tool_call_output",
    "image_generation_call",
    "compaction",
];

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
        tool_schema_root: ToolSchemaRoot::AsSent,
        input_items: InputItems::AsSent,
        tool_choice_without_tools: true,
    };

    /// Another provider's Responses API: as OpenAI's, but without tool namespaces, Codex's
    /// agent messages and `tool_choice` in a request without tools, which only OpenAI takes.
    pub const fn compatible(provider: &'static str) -> Self {
        ResponsesProfile {
            provider,
            tool_namespaces: false,
            input_items: InputItems::WithoutAgentMessages,
            tool_choice_without_tools: false,
            ..ResponsesProfile::OPENAI
        }
    }

    /// The provider takes only xAI's input item types, [`XAI_INPUT_ITEMS`].
    #[must_use]
    pub const fn xai_input_items(mut self) -> Self {
        self.input_items = InputItems::Only(XAI_INPUT_ITEMS);
        self
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

    /// What the provider takes at the root of a function tool's parameter schema; Kaeso
    /// merges other roots into one object schema.
    #[must_use]
    pub const fn tool_schema_root(mut self, root: ToolSchemaRoot) -> Self {
        self.tool_schema_root = root;
        self
    }
}
