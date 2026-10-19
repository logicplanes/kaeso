use indexmap::IndexMap;
use kaeso_kpp::{Block, GenerateRequest, KppError, Role};

use super::DIALECT;
use serde_json::value::RawValue;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ChatProfile {
    pub provider: &'static str,
    pub output_limit: OutputLimit,
    pub developer_role: bool,
    pub seed: SeedField,
    pub end_user: bool,
    pub assistant_prefill: bool,
    pub refused_fields: &'static [&'static str],
    pub refused_message_fields: &'static [&'static str],
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

impl ChatProfile {
    pub const OPENAI: ChatProfile = ChatProfile {
        provider: "OpenAI",
        output_limit: OutputLimit::AsWritten,
        developer_role: true,
        seed: SeedField::Seed,
        end_user: true,
        assistant_prefill: true,
        refused_fields: &[],
        refused_message_fields: &[],
    };

    pub const fn compatible(provider: &'static str) -> Self {
        ChatProfile {
            provider,
            ..ChatProfile::OPENAI
        }
    }

    #[must_use]
    pub const fn max_tokens(mut self) -> Self {
        self.output_limit = OutputLimit::MaxTokens;
        self
    }

    #[must_use]
    pub const fn system_role(mut self) -> Self {
        self.developer_role = false;
        self
    }

    #[must_use]
    pub const fn random_seed(mut self) -> Self {
        self.seed = SeedField::RandomSeed;
        self
    }

    #[must_use]
    pub const fn without_end_user(mut self) -> Self {
        self.end_user = false;
        self
    }

    #[must_use]
    pub const fn without_prefill(mut self) -> Self {
        self.assistant_prefill = false;
        self
    }

    #[must_use]
    pub const fn refusing(mut self, fields: &'static [&'static str]) -> Self {
        self.refused_fields = fields;
        self
    }

    #[must_use]
    pub const fn refusing_in_messages(mut self, fields: &'static [&'static str]) -> Self {
        self.refused_message_fields = fields;
        self
    }

    pub(crate) fn checked_seed(&self, seed: i64) -> Result<i64, KppError> {
        if self.seed == SeedField::RandomSeed && seed < 0 {
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

    pub(crate) fn refuse(&self, req: &GenerateRequest, body: &[u8]) -> Result<(), KppError> {
        if !self.assistant_prefill {
            self.refuse_prefill(req)?;
        }
        if self.refused_fields.is_empty() && self.refused_message_fields.is_empty() {
            return Ok(());
        }
        let top: IndexMap<String, &RawValue> = parse(body)?;
        if let Some(field) = top
            .keys()
            .find(|k| self.refused_fields.contains(&k.as_str()))
        {
            return Err(self.refused(field, field));
        }
        if self.refused_message_fields.is_empty() {
            return Ok(());
        }
        if !req.extras.is_from(DIALECT) {
            return self.refuse_foreign_blocks(req);
        }
        let Some(messages) = top.get("messages") else {
            return Ok(());
        };
        let messages: Vec<IndexMap<String, &RawValue>> = parse(messages.get().as_bytes())?;
        for (index, message) in messages.iter().enumerate() {
            if role(message).as_deref() == Some("tool") {
                continue;
            }
            if let Some(field) = message
                .keys()
                .find(|k| self.refused_message_fields.contains(&k.as_str()))
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
        let own_prefix = last.extras.is_from(DIALECT) && last.extras.fields.contains_key("prefix");
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
        if !self.refused_message_fields.contains(&"refusal") {
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
