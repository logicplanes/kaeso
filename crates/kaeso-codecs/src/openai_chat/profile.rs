use indexmap::IndexMap;
use kaeso_kpp::{Block, GenerateRequest, KppError, Role};

use super::DIALECT;
use super::wire::ChatRequest;
use crate::wire::Ext;
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
    pub assistant_prefill: bool,
    pub verbosity: bool,
    pub refused_fields: &'static [&'static str],
    pub refused_message_fields: &'static [&'static str],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ReplyProfile {
    pub reasoning: ReplyReasoning,
    pub reasoning_fields: &'static [&'static str],
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
        assistant_prefill: true,
        verbosity: true,
        refused_fields: &[],
        refused_message_fields: &[],
    };
}

impl ReplyProfile {
    pub const OPENAI: ReplyProfile = ReplyProfile {
        reasoning: ReplyReasoning::AsSent,
        reasoning_fields: &[],
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
        let mut profile = ChatProfile {
            provider,
            ..ChatProfile::OPENAI
        };
        // `verbosity` is OpenAI's; providers that reject unknown fields refuse it.
        profile.request.verbosity = false;
        profile
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
    pub const fn thinking_parts(mut self) -> Self {
        self.reply.reasoning = ReplyReasoning::ThinkingParts;
        self
    }

    #[must_use]
    pub const fn reasoning_fields(mut self, fields: &'static [&'static str]) -> Self {
        self.reply.reasoning_fields = fields;
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

    pub(crate) fn refuse(
        &self,
        req: &GenerateRequest,
        wire: &Ext<ChatRequest>,
    ) -> Result<(), KppError> {
        let rules = &self.request;
        if !rules.assistant_prefill {
            self.refuse_prefill(req)?;
        }
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
        "response_format" => w.response_format.is_some(),
        "tools" => !w.tools.is_empty(),
        "tool_choice" => w.tool_choice.is_some(),
        "parallel_tool_calls" => w.parallel_tool_calls.is_some(),
        "user" => w.user.is_some(),
        "safety_identifier" => w.safety_identifier.is_some(),
        "stream" => w.stream.is_some(),
        "stream_options" => w.stream_options.is_some(),
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
