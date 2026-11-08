use kaeso_kpp::{DialectId, KppError, ToolChoice};

use crate::adapt::Draft;
use crate::cross::{self, Setting};

/// How a provider's Anthropic Messages API differs from Anthropic's own.
///
/// [`MessagesProfile::ANTHROPIC`] sends a request as the app wrote it. Other providers
/// start from [`MessagesProfile::compatible`] and refuse what they accept without acting
/// on it, so an app never gets an answer that quietly ignored part of its request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct MessagesProfile {
    /// The provider's name, used in error messages.
    pub provider: &'static str,
    /// Request fields the provider ignores, by their Anthropic names: `top_k`,
    /// `stop_sequences`, or a top-level field an Anthropic app sent, such as `container`.
    /// A request that sets one is refused, except a field that may be left out when the
    /// request crosses formats, such as Claude Code's `context_management` that only clears
    /// thinking, which is left out and named in `left_out`.
    pub refused_fields: &'static [&'static str],
    /// Sampling fields the provider always sets to one value of its own, with that value:
    /// `temperature` or `top_p`. A request with another value is refused.
    pub fixed_sampling: &'static [(&'static str, FixedValue)],
    /// Whether `tool_choice` may force a tool or forbid tools; otherwise only `auto` is sent,
    /// and any other choice is refused.
    pub forced_tool_choice: bool,
    /// Whether the provider serves the Messages API as Google's `rawPredict`, as Gemini
    /// Enterprise Agent Platform does for Claude models: a generation goes to
    /// `{base_path}/{model}:rawPredict` (`:streamRawPredict` when it streams) without
    /// `model` in the body, a token count to `{base_path}/count-tokens:rawPredict` with
    /// it, and both bodies carry the platform's `anthropic_version`
    /// ([`EgressCtx::base_path`](crate::EgressCtx::base_path)).
    pub raw_predict: bool,
    /// The media types of the `video` content blocks the provider takes, as MiniMax does
    /// (`{"type": "video", "source": …}`); empty for a provider without video blocks, which
    /// refuses them. URL sources pass whatever they point to.
    pub video_media_types: &'static [&'static str],
}

/// The video media types MiniMax takes in a `video` block's base64 source: `MediaSource`'s
/// video formats in MiniMax's OpenAPI document
/// (<https://platform.minimax.io/docs/api-reference/text/api/openapi-chat-anthropic.json>).
pub const MINIMAX_VIDEO_MEDIA_TYPES: &[&str] = &[
    "video/mp4",
    "video/avi",
    "video/x-msvideo",
    "video/mov",
    "video/x-matroska",
];

/// A sampling value a provider always uses, in thousandths, so a profile stays `Eq`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct FixedValue {
    /// The value times 1000, such as `950` for `0.95`.
    pub thousandths: u32,
}

impl FixedValue {
    /// The value `thousandths / 1000`.
    pub const fn thousandths(thousandths: u32) -> Self {
        FixedValue { thousandths }
    }

    fn holds(self, value: f64) -> bool {
        (value * 1000.0 - f64::from(self.thousandths)).abs() < 1e-6
    }

    fn written(self) -> String {
        let whole = self.thousandths / 1000;
        let rest = self.thousandths % 1000;
        if rest == 0 {
            whole.to_string()
        } else {
            format!("{whole}.{rest:03}")
                .trim_end_matches('0')
                .to_owned()
        }
    }
}

impl MessagesProfile {
    /// Anthropic's Messages API.
    pub const ANTHROPIC: MessagesProfile = MessagesProfile {
        provider: "Anthropic",
        refused_fields: &[],
        fixed_sampling: &[],
        forced_tool_choice: true,
        raw_predict: false,
        video_media_types: &[],
    };

    /// Another provider's Messages API, as Anthropic's until a setting says otherwise.
    pub const fn compatible(provider: &'static str) -> Self {
        MessagesProfile {
            provider,
            ..MessagesProfile::ANTHROPIC
        }
    }

    /// Refuses these fields; each call replaces the list.
    #[must_use]
    pub const fn refusing(mut self, fields: &'static [&'static str]) -> Self {
        self.refused_fields = fields;
        self
    }

    /// The provider samples at these values whatever the request says; each call replaces
    /// the list.
    #[must_use]
    pub const fn sampling_fixed_at(
        mut self,
        values: &'static [(&'static str, FixedValue)],
    ) -> Self {
        self.fixed_sampling = values;
        self
    }

    /// The provider serves the Messages API as Google's `rawPredict`
    /// ([`MessagesProfile::raw_predict`]).
    #[must_use]
    pub const fn raw_predict(mut self) -> Self {
        self.raw_predict = true;
        self
    }

    /// The provider takes `video` content blocks of these media types
    /// ([`MessagesProfile::video_media_types`]).
    #[must_use]
    pub const fn with_video(mut self, media_types: &'static [&'static str]) -> Self {
        self.video_media_types = media_types;
        self
    }

    /// The provider chooses tools itself: only `tool_choice` `auto` is sent.
    #[must_use]
    pub const fn auto_tool_choice_only(mut self) -> Self {
        self.forced_tool_choice = false;
        self
    }

    fn refused(&self, param: &str) -> KppError {
        KppError::unsupported(
            param,
            format!(
                "`{param}` is not supported by {}; remove it from the request.",
                self.provider
            ),
        )
        .with_param(param)
    }
}

impl Default for MessagesProfile {
    fn default() -> Self {
        MessagesProfile::ANTHROPIC
    }
}

const D: DialectId = DialectId::AnthropicMessages;

pub(crate) fn apply(d: &mut Draft<'_>, profile: &MessagesProfile) -> Result<(), KppError> {
    let r = d.get();
    for (field, value) in profile.fixed_sampling {
        let sent = match *field {
            "temperature" => r.sampling.temperature,
            "top_p" => r.sampling.top_p,
            _ => None,
        };
        if let Some(sent) = sent
            && !value.holds(sent)
        {
            let at = value.written();
            return Err(KppError::unsupported(
                *field,
                format!(
                    "{} always samples at `{field}` {at}; remove `{field}` or set it to {at}.",
                    profile.provider
                ),
            )
            .with_param(*field));
        }
    }
    if !profile.forced_tool_choice
        && let Some(choice) = &r.tool_choice
        && !matches!(choice, ToolChoice::Auto)
    {
        return Err(KppError::unsupported(
            "tool_choice",
            format!(
                "{} chooses tools itself; remove `tool_choice` or set it to `auto`.",
                profile.provider
            ),
        )
        .with_param("tool_choice"));
    }
    let own = r.extras.is_from(D);
    let mut left_out = Vec::new();
    for field in profile.refused_fields {
        match *field {
            "top_k" if r.sampling.top_k.is_some() => return Err(profile.refused("top_k")),
            "stop_sequences" if !r.sampling.stop.is_empty() => {
                return Err(profile.refused(cross::app_field(r, Setting::StopSequences, D)));
            }
            "top_k" | "stop_sequences" => {}
            other if own => {
                let Some(value) = r.extras.fields.get(other) else {
                    continue;
                };
                if cross::is_droppable_value(D, "request", other, value) {
                    left_out.push(other.to_owned());
                    continue;
                }
                let mut err = profile.refused(other);
                if let Some(hint) = cross::claude_code_hint(D, "request", other) {
                    err.message = format!("{} {hint}", err.message);
                }
                return Err(err);
            }
            _ => {}
        }
    }
    for field in left_out {
        d.edit().extras.fields.shift_remove(&field);
        d.leave_out(field);
    }
    Ok(())
}
