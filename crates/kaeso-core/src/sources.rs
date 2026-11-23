//! Sources: clients that report the requests and tool calls they make without Kaeso, such as a
//! coding agent calling its provider directly. Kaeso observes what a source sends; it never sits in
//! the source's path and can't refuse its calls.

use std::fmt;
use std::str::FromStr;

use crate::keys::{KeyKind, NewSecret};

/// The start of every source key: `sk-kso-src-` and 48 letters and digits.
pub const KEY_PREFIX: &str = "sk-kso-src-";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SourceKind {
    ClaudeCode,
    Codex,
    GeminiCli,
    Antigravity,
    OpenTelemetry,
    Http,
    Litellm,
    Openrouter,
}

impl SourceKind {
    pub const ALL: [SourceKind; 8] = [
        SourceKind::ClaudeCode,
        SourceKind::Codex,
        SourceKind::GeminiCli,
        SourceKind::Antigravity,
        SourceKind::OpenTelemetry,
        SourceKind::Http,
        SourceKind::Litellm,
        SourceKind::Openrouter,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            SourceKind::ClaudeCode => "claude_code",
            SourceKind::Codex => "codex",
            SourceKind::GeminiCli => "gemini_cli",
            SourceKind::Antigravity => "antigravity",
            SourceKind::OpenTelemetry => "opentelemetry",
            SourceKind::Http => "http",
            SourceKind::Litellm => "litellm",
            SourceKind::Openrouter => "openrouter",
        }
    }

    /// The name people know the client by.
    pub const fn name(self) -> &'static str {
        match self {
            SourceKind::ClaudeCode => "Claude Code",
            SourceKind::Codex => "Codex",
            SourceKind::GeminiCli => "Gemini CLI",
            SourceKind::Antigravity => "Antigravity",
            SourceKind::OpenTelemetry => "OpenTelemetry",
            SourceKind::Http => "HTTP",
            SourceKind::Litellm => "LiteLLM",
            SourceKind::Openrouter => "OpenRouter",
        }
    }

    /// The provider a client calls when its events don't name one.
    pub const fn provider(self) -> Option<&'static str> {
        match self {
            SourceKind::ClaudeCode => Some("anthropic"),
            SourceKind::Codex => Some("openai"),
            SourceKind::GeminiCli | SourceKind::Antigravity => Some("google"),
            SourceKind::Openrouter => Some("openrouter"),
            SourceKind::OpenTelemetry | SourceKind::Http | SourceKind::Litellm => None,
        }
    }
}

impl fmt::Display for SourceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not a source type")]
pub struct KindParseError(pub String);

impl FromStr for SourceKind {
    type Err = KindParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        SourceKind::ALL
            .into_iter()
            .find(|k| k.as_str() == s)
            .ok_or_else(|| KindParseError(s.to_owned()))
    }
}

/// A new source key, made from the same randomness as an API key.
pub fn generate(random: impl FnMut() -> [u8; 64]) -> NewSecret {
    let made = crate::keys::generate(KeyKind::Project, random);
    let full = {
        use secrecy::ExposeSecret;
        let random = made
            .secret
            .expose_secret()
            .strip_prefix(crate::keys::PROJECT_PREFIX)
            .unwrap_or_default();
        format!("{KEY_PREFIX}{random}")
    };
    NewSecret {
        hash: crate::secret::hash(&full),
        secret: secrecy::SecretString::from(full),
        prefix: made.prefix,
        last4: made.last4,
    }
}

/// Whether a presented value has the shape of a source key.
pub fn is_key(presented: &str) -> bool {
    presented.strip_prefix(KEY_PREFIX).is_some_and(|rest| {
        rest.len() == crate::keys::RANDOM_LEN && rest.bytes().all(|b| b.is_ascii_alphanumeric())
    })
}

/// Token counts as a client reported them, before Kaeso reads them its own way: the input
/// without what came from or went into the cache, and the output with the reasoning in it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counted {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub reasoning: u64,
}

/// What a client's input count holds besides the uncached input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputCounts {
    /// Only the uncached input, as Anthropic and Claude Code count it.
    Uncached,
    /// The cached input too, as OpenAI, Gemini and the GenAI semantic conventions count it.
    WithCache,
}

/// What a client's output count holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputCounts {
    /// The reasoning too, as OpenAI and Anthropic count it.
    WithReasoning,
    /// Only the visible output; Gemini counts its thoughts apart.
    WithoutReasoning,
}

impl Counted {
    /// The counts as Kaeso keeps them: input without the cache, output with the reasoning.
    pub fn normalized(self, input: InputCounts, output: OutputCounts) -> Counted {
        let uncached = match input {
            InputCounts::Uncached => self.input,
            InputCounts::WithCache => self
                .input
                .saturating_sub(self.cache_read)
                .saturating_sub(self.cache_write),
        };
        let out = match output {
            OutputCounts::WithReasoning => self.output.max(self.reasoning),
            OutputCounts::WithoutReasoning => self.output.saturating_add(self.reasoning),
        };
        Counted {
            input: uncached,
            output: out,
            ..self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn random() -> [u8; 64] {
        let mut b = [0u8; 64];
        for (i, x) in b.iter_mut().enumerate() {
            *x = (i as u8).wrapping_mul(53).wrapping_add(7);
        }
        b
    }

    #[test]
    fn a_source_key_has_its_own_prefix_and_is_never_an_api_key() {
        use secrecy::ExposeSecret;
        let made = generate(random);
        let key = made.secret.expose_secret();
        assert!(key.starts_with("sk-kso-src-"), "{key}");
        assert_eq!(key.len(), KEY_PREFIX.len() + 48);
        assert!(is_key(key));
        assert_eq!(crate::keys::kind_of(key), None);
        assert_eq!(made.hash, crate::secret::hash(key));
        assert!(key.ends_with(&made.last4));
        assert!(!is_key("sk-kso-src-short"));
        assert!(!is_key(&format!("sk-kso-src-{}", "-".repeat(48))));
    }

    #[test]
    fn every_kind_reads_back_from_its_name() {
        for kind in SourceKind::ALL {
            assert_eq!(kind.as_str().parse::<SourceKind>(), Ok(kind));
        }
        assert!("claude-code".parse::<SourceKind>().is_err());
    }

    #[test]
    fn counts_with_the_cache_in_the_input_lose_it() {
        let openai = Counted {
            input: 1_000,
            output: 300,
            cache_read: 800,
            cache_write: 0,
            reasoning: 120,
        };
        assert_eq!(
            openai.normalized(InputCounts::WithCache, OutputCounts::WithReasoning),
            Counted {
                input: 200,
                output: 300,
                ..openai
            }
        );
        let gemini = Counted {
            input: 1_000,
            output: 300,
            cache_read: 400,
            cache_write: 0,
            reasoning: 50,
        };
        assert_eq!(
            gemini.normalized(InputCounts::WithCache, OutputCounts::WithoutReasoning),
            Counted {
                input: 600,
                output: 350,
                ..gemini
            }
        );
        let claude = Counted {
            input: 2,
            output: 62,
            cache_read: 11_873,
            cache_write: 16_236,
            reasoning: 0,
        };
        assert_eq!(
            claude.normalized(InputCounts::Uncached, OutputCounts::WithReasoning),
            claude
        );
    }

    #[test]
    fn counts_that_disagree_never_go_below_zero() {
        let odd = Counted {
            input: 10,
            output: 5,
            cache_read: 50,
            cache_write: 0,
            reasoning: 9,
        };
        let n = odd.normalized(InputCounts::WithCache, OutputCounts::WithReasoning);
        assert_eq!((n.input, n.output), (0, 9));
    }
}
