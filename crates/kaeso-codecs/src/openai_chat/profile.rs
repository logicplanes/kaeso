use indexmap::IndexMap;
use kaeso_kpp::{Block, GenerateRequest, KppError, Role};

use super::DIALECT;
use super::wire::ChatRequest;
use crate::wire::Ext;
use crate::{ToolPatterns, ToolSchemaRoot};
use serde_json::value::RawValue;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ChatProfile {
    pub provider: &'static str,
    pub request: RequestProfile,
    pub reply: ReplyProfile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct RequestProfile {
    pub output_limit: OutputLimit,
    pub developer_role: bool,
    pub seed: SeedField,
    pub end_user: bool,
    pub end_user_field: EndUserField,
    pub assistant_prefill: bool,
    pub verbosity: bool,
    pub cache_key: bool,
    pub message_bookkeeping: bool,
    pub greedy_top_p: bool,
    pub stop_list: bool,
    pub partial_prefill: bool,
    pub thinking_switch: bool,
    /// Whether the provider turns thinking on and off with a `thinking` object,
    /// `{"type": "enabled"}` or `{"type": "disabled"}`, as Volcengine Ark does, instead of
    /// `reasoning_effort` alone.
    pub thinking_type: bool,
    /// Whether the provider accepts OpenRouter's unified `reasoning` object (`effort`,
    /// `max_tokens`, `enabled`, `exclude`) on Chat Completions requests.
    pub reasoning_object: bool,
    pub leading_system_only: bool,
    pub replay_reasoning_as_read: bool,
    pub tool_schema_root: ToolSchemaRoot,
    pub tool_patterns: ToolPatterns,
    pub tool_parameters: bool,
    /// Fields the provider would drop or misread, refused before sending. `parent.child` names
    /// a key inside an object field, such as `web_search_options.user_location`.
    pub refused_fields: &'static [&'static str],
    /// Values of a top-level string field the provider doesn't take in OpenAI's meaning, such
    /// as `service_tier: "priority"` for a provider with tiers of its own.
    pub refused_values: &'static [(&'static str, &'static [&'static str])],
    pub refused_message_fields: &'static [&'static str],
    /// The `input_audio.format` values the provider takes, or `None` for any value.
    pub input_audio_formats: Option<&'static [&'static str]>,
    /// Whether the provider takes videos as `video_url` content parts, which OpenAI's API
    /// doesn't have; otherwise video blocks are refused.
    pub video_url: bool,
    /// Where the provider serves Chat Completions, `/v1/chat/completions` unless it uses a path
    /// of its own, such as Perplexity's `/v1/sonar`.
    pub path: &'static str,
    /// Whether the provider takes `stream_options`. One that reports usage in every stream
    /// without being asked gets none; an app's `include_usage` still decides whether the app
    /// sees it, and any other option is refused.
    pub stream_options: bool,
    /// Whether the provider writes a reasoning model's chain of thought into `content` unless
    /// the request sets `separate_reasoning: true`, as Novita AI does; Kaeso then sets it, so
    /// the reasoning comes back in its own field.
    pub separate_reasoning: bool,
    /// Keys the encoder writes into a provider's own option objects when the request doesn't
    /// set them, such as Venice's `venice_parameters.include_venice_system_prompt: false`.
    pub option_defaults: &'static [OptionDefault],
}

/// A key of a provider's option object that the encoder writes when the request leaves it
/// out ([`ChatProfile::with_option_defaults`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct OptionDefault {
    /// The top-level object, such as `venice_parameters`.
    pub object: &'static str,
    /// The key inside it.
    pub key: &'static str,
    /// The value as JSON text, such as `false`.
    pub value: &'static str,
}

impl OptionDefault {
    pub const fn new(object: &'static str, key: &'static str, value: &'static str) -> Self {
        OptionDefault { object, key, value }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ReplyProfile {
    pub reasoning: ReplyReasoning,
    pub reasoning_fields: &'static [&'static str],
    /// The message and delta field with the provider's encrypted original of the reasoning,
    /// which must go back unchanged with the assistant message, as Volcengine Ark's
    /// `encrypted_content`. A Chat app gets and sends it as written; for the other formats it
    /// travels with the reasoning block and goes back in this field.
    pub encrypted_reasoning: Option<&'static str>,
    pub error_detail: &'static [&'static str],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ReplyReasoning {
    AsSent,
    ThinkingParts,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum OutputLimit {
    AsWritten,
    MaxTokens,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum EndUserField {
    SafetyIdentifier,
    User,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SeedField {
    Seed,
    RandomSeed,
}

impl RequestProfile {
    pub const OPENAI: RequestProfile = RequestProfile {
        output_limit: OutputLimit::AsWritten,
        developer_role: true,
        seed: SeedField::Seed,
        end_user: true,
        end_user_field: EndUserField::SafetyIdentifier,
        assistant_prefill: true,
        verbosity: true,
        cache_key: true,
        message_bookkeeping: false,
        greedy_top_p: false,
        stop_list: false,
        partial_prefill: false,
        thinking_switch: false,
        thinking_type: false,
        reasoning_object: false,
        leading_system_only: false,
        replay_reasoning_as_read: false,
        tool_schema_root: ToolSchemaRoot::AsSent,
        tool_patterns: ToolPatterns::AsSent,
        tool_parameters: false,
        refused_fields: &[],
        refused_values: &[],
        refused_message_fields: &[],
        input_audio_formats: Some(&["wav", "mp3"]),
        video_url: false,
        path: super::CHAT_COMPLETIONS_PATH,
        stream_options: true,
        separate_reasoning: false,
        option_defaults: &[],
    };
}

impl ReplyProfile {
    pub const OPENAI: ReplyProfile = ReplyProfile {
        reasoning: ReplyReasoning::AsSent,
        reasoning_fields: &[],
        encrypted_reasoning: None,
        error_detail: &[],
    };
}

impl ChatProfile {
    pub const OPENAI: ChatProfile = ChatProfile {
        provider: "OpenAI",
        request: RequestProfile::OPENAI,
        reply: ReplyProfile::OPENAI,
    };

    pub const fn compatible(provider: &'static str) -> Self {
        // `verbosity` is OpenAI's; providers that reject unknown fields refuse it.
        ChatProfile {
            provider,
            ..ChatProfile::OPENAI
        }
        .without_verbosity()
    }

    #[must_use]
    pub const fn without_verbosity(mut self) -> Self {
        self.request.verbosity = false;
        self
    }

    /// The provider takes OpenAI's `verbosity`, which [`ChatProfile::compatible`] leaves out.
    #[must_use]
    pub const fn with_verbosity(mut self) -> Self {
        self.request.verbosity = true;
        self
    }

    #[must_use]
    pub const fn without_cache_key(mut self) -> Self {
        self.request.cache_key = false;
        self
    }

    #[must_use]
    pub const fn max_tokens(mut self) -> Self {
        self.request.output_limit = OutputLimit::MaxTokens;
        self
    }

    #[must_use]
    pub const fn system_role(mut self) -> Self {
        self.request.developer_role = false;
        self
    }

    #[must_use]
    pub const fn random_seed(mut self) -> Self {
        self.request.seed = SeedField::RandomSeed;
        self
    }

    #[must_use]
    pub const fn end_user_as_user(mut self) -> Self {
        self.request.end_user_field = EndUserField::User;
        self
    }

    #[must_use]
    pub const fn without_end_user(mut self) -> Self {
        self.request.end_user = false;
        self
    }

    #[must_use]
    pub const fn without_prefill(mut self) -> Self {
        self.request.assistant_prefill = false;
        self
    }

    #[must_use]
    pub const fn with_message_bookkeeping(mut self) -> Self {
        self.request.message_bookkeeping = true;
        self
    }

    #[must_use]
    pub const fn greedy_top_p(mut self) -> Self {
        self.request.greedy_top_p = true;
        self
    }

    #[must_use]
    pub const fn stop_list(mut self) -> Self {
        self.request.stop_list = true;
        self
    }

    #[must_use]
    pub const fn partial_prefill(mut self) -> Self {
        self.request.partial_prefill = true;
        self
    }

    #[must_use]
    pub const fn thinking_switch(mut self) -> Self {
        self.request.thinking_switch = true;
        self
    }

    /// The provider switches thinking with a `thinking` object ([`RequestProfile::thinking_type`]).
    #[must_use]
    pub const fn thinking_type(mut self) -> Self {
        self.request.thinking_type = true;
        self
    }

    /// The provider takes OpenRouter's `reasoning` object ([`RequestProfile::reasoning_object`]).
    #[must_use]
    pub const fn with_reasoning_object(mut self) -> Self {
        self.request.reasoning_object = true;
        self
    }

    #[must_use]
    pub const fn leading_system_only(mut self) -> Self {
        self.request.leading_system_only = true;
        self
    }

    #[must_use]
    pub const fn replay_reasoning_as_read(mut self) -> Self {
        self.request.replay_reasoning_as_read = true;
        self
    }

    #[must_use]
    pub const fn tool_schema_root(mut self, root: ToolSchemaRoot) -> Self {
        self.request.tool_schema_root = root;
        self
    }

    #[must_use]
    pub const fn tool_patterns(mut self, patterns: ToolPatterns) -> Self {
        self.request.tool_patterns = patterns;
        self
    }

    #[must_use]
    pub const fn with_tool_parameters(mut self) -> Self {
        self.request.tool_parameters = true;
        self
    }

    #[must_use]
    pub const fn thinking_parts(mut self) -> Self {
        self.reply.reasoning = ReplyReasoning::ThinkingParts;
        self
    }

    #[must_use]
    pub const fn reasoning_fields(mut self, fields: &'static [&'static str]) -> Self {
        self.reply.reasoning_fields = fields;
        self
    }

    /// The provider sends and takes back encrypted reasoning in `field`
    /// ([`ReplyProfile::encrypted_reasoning`]).
    #[must_use]
    pub const fn encrypted_reasoning_field(mut self, field: &'static str) -> Self {
        self.reply.encrypted_reasoning = Some(field);
        self
    }

    #[must_use]
    pub const fn error_detail(mut self, path: &'static [&'static str]) -> Self {
        self.reply.error_detail = path;
        self
    }

    #[must_use]
    pub const fn refusing(mut self, fields: &'static [&'static str]) -> Self {
        self.request.refused_fields = fields;
        self
    }

    /// Refuses these values of top-level string fields ([`RequestProfile::refused_values`]).
    #[must_use]
    pub const fn refusing_values(
        mut self,
        values: &'static [(&'static str, &'static [&'static str])],
    ) -> Self {
        self.request.refused_values = values;
        self
    }

    /// The provider takes any `input_audio.format` that names a media type, as OpenRouter
    /// passes the format on to the model's own provider.
    #[must_use]
    pub const fn any_audio_format(mut self) -> Self {
        self.request.input_audio_formats = None;
        self
    }

    /// The provider takes `video_url` content parts ([`RequestProfile::video_url`]).
    #[must_use]
    pub const fn with_video_url(mut self) -> Self {
        self.request.video_url = true;
        self
    }

    /// The provider serves Chat Completions at `path` ([`RequestProfile::path`]).
    #[must_use]
    pub const fn at_path(mut self, path: &'static str) -> Self {
        self.request.path = path;
        self
    }

    /// The provider takes no `stream_options` ([`RequestProfile::stream_options`]).
    #[must_use]
    pub const fn without_stream_options(mut self) -> Self {
        self.request.stream_options = false;
        self
    }

    /// The provider returns reasoning in its own field only when asked
    /// ([`RequestProfile::separate_reasoning`]).
    #[must_use]
    pub const fn separate_reasoning(mut self) -> Self {
        self.request.separate_reasoning = true;
        self
    }

    /// The encoder writes each of these keys into its option object when the request doesn't
    /// set it: it adds the object when the request has none, and leaves an object the app
    /// wrote as something else than a JSON object as written.
    #[must_use]
    pub const fn with_option_defaults(mut self, defaults: &'static [OptionDefault]) -> Self {
        self.request.option_defaults = defaults;
        self
    }

    #[must_use]
    pub const fn refusing_in_messages(mut self, fields: &'static [&'static str]) -> Self {
        self.request.refused_message_fields = fields;
        self
    }

    pub(crate) fn checked_seed(&self, seed: i64) -> Result<i64, KppError> {
        if self.request.seed == SeedField::RandomSeed && seed < 0 {
            return Err(KppError::unsupported(
                "seed",
                format!(
                    "{} takes a `seed` of 0 or more; send a seed that is not negative.",
                    self.provider
                ),
            )
            .with_param("seed"));
        }
        Ok(seed)
    }

    pub(crate) fn sent_top_p(&self, temperature: Option<f64>, top_p: Option<f64>) -> Option<f64> {
        if self.request.greedy_top_p && temperature == Some(0.0) {
            Some(1.0)
        } else {
            top_p
        }
    }

    pub(crate) fn refuse(
        &self,
        req: &GenerateRequest,
        wire: &Ext<ChatRequest>,
    ) -> Result<(), KppError> {
        let rules = &self.request;
        if !rules.assistant_prefill {
            self.refuse_prefill(req)?;
        }
        self.refuse_audio_formats(req)?;
        if let Some(field) = rules
            .refused_fields
            .iter()
            .find(|f| sends_known(&wire.known, f))
        {
            return Err(self.refused(field, field));
        }
        if let Some(field) = wire
            .verbatim
            .keys()
            .chain(wire.extra.keys())
            .find(|k| rules.refused_fields.contains(&k.as_str()))
        {
            return Err(self.refused(field, field));
        }
        let sent = |key: &str| wire.verbatim.get(key).or_else(|| wire.extra.get(key));
        for field in rules.refused_fields {
            let Some((parent, child)) = field.split_once('.') else {
                continue;
            };
            let inside = sent(parent)
                .and_then(|v| v.parse::<serde_json::Map<String, serde_json::Value>>().ok())
                .is_some_and(|object| object.contains_key(child));
            if inside {
                return Err(self.refused(field, field));
            }
        }
        for (field, values) in rules.refused_values {
            if let Some(value) = sent(field).and_then(|v| v.parse::<String>().ok())
                && values.contains(&value.as_str())
            {
                return Err(KppError::unsupported(
                    format!("{field} `{value}`"),
                    format!(
                        "{} does not take `{field}: \"{value}\"`; send one of its own values or leave `{field}` out.",
                        self.provider
                    ),
                )
                .with_param(*field));
            }
        }
        if rules.refused_message_fields.is_empty() {
            return Ok(());
        }
        if !req.extras.is_from(DIALECT) {
            return self.refuse_foreign_blocks(req);
        }
        for (index, message) in wire.known.messages.iter().enumerate() {
            let message: IndexMap<String, &RawValue> = parse(message.get().as_bytes())?;
            if role(&message).as_deref() == Some("tool") {
                continue;
            }
            if let Some(field) = message
                .keys()
                .find(|k| rules.refused_message_fields.contains(&k.as_str()))
            {
                return Err(self.refused(field, &format!("messages[{index}].{field}")));
            }
        }
        Ok(())
    }

    fn refuse_audio_formats(&self, req: &GenerateRequest) -> Result<(), KppError> {
        let Some(formats) = self.request.input_audio_formats else {
            return Ok(());
        };
        for (i, message) in req.messages.iter().enumerate() {
            for (j, block) in message.blocks.iter().enumerate() {
                let Block::Audio(audio) = block else { continue };
                let Some(format) = audio
                    .source
                    .media_type()
                    .and_then(crate::media::chat_audio_format)
                else {
                    continue;
                };
                if !formats.contains(&format) {
                    return Err(KppError::unsupported(
                        format!("audio format `{format}`"),
                        format!(
                            "{} takes `input_audio` in the formats {}; send the audio in one of them.",
                            self.provider,
                            formats.join(", ")
                        ),
                    )
                    .with_param(format!("messages[{i}].blocks[{j}]")));
                }
            }
        }
        Ok(())
    }

    fn refuse_prefill(&self, req: &GenerateRequest) -> Result<(), KppError> {
        let Some(last) = req.messages.last() else {
            return Ok(());
        };
        let index = req.messages.len() - 1;
        let calls_tools = last.blocks.iter().any(|b| matches!(b, Block::ToolCall(_)));
        let own_prefix = last.extras.is_from(DIALECT)
            && last
                .extras
                .fields
                .get("prefix")
                .is_some_and(|p| p.get().trim() == "true");
        if last.role != Role::Assistant || calls_tools || own_prefix {
            return Ok(());
        }
        Err(KppError::unsupported(
            "assistant prefill",
            format!(
                "{} cannot continue a prefilled assistant message; end the conversation with a user message.",
                self.provider
            ),
        )
        .with_param(format!("messages[{index}]")))
    }

    fn refuse_foreign_blocks(&self, req: &GenerateRequest) -> Result<(), KppError> {
        if !self.request.refused_message_fields.contains(&"refusal") {
            return Ok(());
        }
        for (i, message) in req.messages.iter().enumerate() {
            if let Some(j) = message
                .blocks
                .iter()
                .position(|b| matches!(b, Block::Refusal(_)))
            {
                return Err(KppError::unsupported(
                    "refusal block",
                    format!(
                        "`refusal block` is not supported by {}; remove the refusal from the conversation history or replace it with text.",
                        self.provider
                    ),
                )
                .with_param(format!("messages[{i}].blocks[{j}]")));
            }
        }
        Ok(())
    }

    fn refused(&self, field: &str, param: &str) -> KppError {
        KppError::unsupported(
            field,
            format!(
                "`{param}` is not supported by {}; remove it from the request.",
                self.provider
            ),
        )
        .with_param(param)
    }
}

impl Default for ChatProfile {
    fn default() -> Self {
        ChatProfile::OPENAI
    }
}

pub(crate) fn sends_known(w: &ChatRequest, key: &str) -> bool {
    match key {
        "model" => true,
        "messages" => !w.messages.is_empty(),
        "temperature" => w.temperature.is_some(),
        "top_p" => w.top_p.is_some(),
        "max_completion_tokens" => w.max_completion_tokens.is_some(),
        "max_tokens" => w.max_tokens.is_some(),
        "stop" => w.stop.is_some(),
        "seed" => w.seed.is_some(),
        "frequency_penalty" => w.frequency_penalty.is_some(),
        "presence_penalty" => w.presence_penalty.is_some(),
        "n" => w.n.is_some(),
        "reasoning_effort" => w.reasoning_effort.is_some(),
        "reasoning" => w.reasoning.is_some(),
        "response_format" => w.response_format.is_some(),
        "tools" => !w.tools.is_empty(),
        "tool_choice" => w.tool_choice.is_some(),
        "parallel_tool_calls" => w.parallel_tool_calls.is_some(),
        "user" => w.user.is_some(),
        "safety_identifier" => w.safety_identifier.is_some(),
        "stream" => w.stream.is_some(),
        "stream_options" => w.stream_options.is_some(),
        "modalities" => w.modalities.is_some(),
        "audio" => w.audio.is_some(),
        _ => false,
    }
}

fn role(message: &IndexMap<String, &RawValue>) -> Option<String> {
    serde_json::from_str(message.get("role")?.get()).ok()
}

fn parse<'a, T: serde::Deserialize<'a>>(bytes: &'a [u8]) -> Result<T, KppError> {
    serde_json::from_slice(bytes).map_err(|e| {
        KppError::provider(format!(
            "Kaeso could not read the request it built ({e}). This is a bug in Kaeso; please report it."
        ))
    })
}
