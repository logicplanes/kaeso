use kaeso_kpp::{
    DialectId, GenerateRequest, ModelInfo, ReasoningEffort, ReasoningMode, Request, ToolChoice,
};

use kaeso_codecs::cross::reasoning_map;

use super::Effective;

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct Unsupported {
    pub capability: &'static str,
    pub param: Option<&'static str>,
    pub message: String,
}

fn refuse(capability: &'static str, param: Option<&'static str>, message: String) -> Unsupported {
    Unsupported {
        capability,
        param,
        message,
    }
}

fn app_param(ingress: DialectId, field: &'static str) -> &'static str {
    use DialectId::{AnthropicMessages as A, OpenAiResponses as R};
    match (field, ingress) {
        ("budget", _) => "thinking.budget_tokens",
        ("effort", A) => "output_config.effort",
        ("effort", R) => "reasoning.effort",
        ("effort", _) => "reasoning_effort",
        ("format", A) => "output_config.format",
        ("format", R) => "text.format",
        ("format", _) => "response_format",
        ("messages", R) => "input",
        ("reasoning", A) => "thinking",
        ("reasoning", R) => "reasoning",
        ("reasoning", _) => "reasoning_effort",
        (other, _) => other,
    }
}

fn forced_choice(choice: Option<&ToolChoice>, ingress: DialectId) -> Option<&'static str> {
    let anthropic = ingress == DialectId::AnthropicMessages;
    match choice? {
        ToolChoice::Required if anthropic => Some("`tool_choice` `any`"),
        ToolChoice::Tool(_) if anthropic => Some("`tool_choice` `tool`"),
        ToolChoice::Required => Some("`tool_choice` `required`"),
        ToolChoice::Tool(_) => Some("a named function in `tool_choice`"),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    pub dialect: DialectId,
    pub adaptive_thinking: Option<bool>,
    pub reasoning: Option<bool>,
}

pub fn prepare(
    req: &mut Request,
    ingress: DialectId,
    eff: &Effective,
) -> Result<Plan, Unsupported> {
    let name = eff.model_id.as_str();
    let adaptive_thinking = eff
        .provider
        .dialects()
        .contains(&DialectId::AnthropicMessages)
        .then_some(eff.fields.adaptive_thinking)
        .flatten();
    match req {
        Request::Embed(_) => {
            if !eff.is_embeddings() {
                return Err(refuse(
                    "embeddings",
                    Some("model"),
                    format!(
                        "The model `{name}` is a generation model; it cannot create embeddings."
                    ),
                ));
            }
            Ok(Plan {
                dialect: eff.dialect,
                adaptive_thinking: None,
                reasoning: None,
            })
        }
        Request::Generate(g) => {
            check_generation(g, ingress, eff)?;
            let dialect = eff.dialect;
            if dialect == DialectId::AnthropicMessages && g.sampling.max_output_tokens.is_none() {
                g.sampling.max_output_tokens = eff
                    .fields
                    .output_limit
                    .map(|l| u32::try_from(l).unwrap_or(u32::MAX));
            }
            Ok(Plan {
                dialect,
                adaptive_thinking,
                reasoning: eff.fields.reasoning,
            })
        }
        Request::CountTokens(g) => {
            let Some(dialect) = eff.provider.count_tokens() else {
                return Err(refuse(
                    "token counting",
                    Some("model"),
                    format!("The model `{name}` cannot count tokens."),
                ));
            };
            check_generation(g, ingress, eff)?;
            Ok(Plan {
                dialect,
                adaptive_thinking,
                reasoning: eff.fields.reasoning,
            })
        }
        _ => Err(refuse(
            "endpoint",
            None,
            format!("The model `{name}` cannot serve this endpoint."),
        )),
    }
}

fn check_generation(
    g: &GenerateRequest,
    ingress: DialectId,
    eff: &Effective,
) -> Result<(), Unsupported> {
    let name = eff.model_id.as_str();
    if eff.is_embeddings() {
        return Err(refuse(
            "generation",
            Some("model"),
            format!("The model `{name}` is an embeddings model; it cannot generate text."),
        ));
    }
    if eff
        .fields
        .output_modalities
        .as_ref()
        .is_some_and(|o| !o.iter().any(|m| m == "text"))
    {
        return Err(refuse(
            "text output",
            Some("model"),
            format!("The model `{name}` does not produce text."),
        ));
    }
    let caps = g.required_capabilities();
    let messages = Some(app_param(ingress, "messages"));
    if caps.images && !eff.accepts("image") {
        return Err(refuse(
            "image input",
            messages,
            format!("The model `{name}` does not accept images."),
        ));
    }
    if caps.pdf && !eff.accepts("pdf") {
        return Err(refuse(
            "pdf input",
            messages,
            format!("The model `{name}` does not accept PDF documents."),
        ));
    }
    if caps.tools && eff.fields.tool_call != Some(true) {
        return Err(refuse(
            "tools",
            Some("tools"),
            format!("The model `{name}` does not support tool calling."),
        ));
    }
    if eff.fields.forced_tool_choice == Some(false)
        && let Some(words) = forced_choice(g.tool_choice.as_ref(), ingress)
    {
        return Err(refuse(
            "forced tool choice",
            Some("tool_choice"),
            format!(
                "The model `{name}` does not accept {words}; send `auto` and name the tool in the prompt."
            ),
        ));
    }
    if caps.json_schema && eff.fields.structured_output != Some(true) {
        return Err(refuse(
            "structured output",
            Some(app_param(ingress, "format")),
            format!("The model `{name}` does not support structured output (a JSON schema)."),
        ));
    }
    if caps.reasoning && eff.fields.reasoning != Some(true) {
        return Err(refuse(
            "reasoning",
            Some(app_param(ingress, "reasoning")),
            format!("The model `{name}` does not support reasoning."),
        ));
    }
    if eff.fields.adaptive_thinking == Some(false)
        && g.reasoning
            .as_ref()
            .is_some_and(|r| r.mode == Some(ReasoningMode::Adaptive))
    {
        return Err(refuse(
            "adaptive thinking",
            Some("thinking"),
            format!(
                "The model `{name}` has no adaptive thinking; send `enabled` thinking with `budget_tokens`."
            ),
        ));
    }
    if let Some(t) = g.sampling.temperature
        && eff.fields.temperature == Some(false)
        && (t - 1.0).abs() > f64::EPSILON
        && !kpp_turns_thinking_on(g, ingress, eff)
    {
        return Err(refuse(
            "temperature",
            Some("temperature"),
            format!("The model `{name}` does not accept `temperature` (only the default of 1)."),
        ));
    }
    if let Some(effort) = g.reasoning.as_ref().and_then(|r| r.effort.as_ref()) {
        check_effort(effort, ingress, eff)?;
    } else if let Some(budget) = budget_sent_as_effort(g, eff) {
        check_budget(budget, ingress, eff)?;
    }
    Ok(())
}

fn budget_sent_as_effort(g: &GenerateRequest, eff: &Effective) -> Option<u32> {
    let r = g.reasoning.as_ref()?;
    let openai = matches!(
        eff.dialect,
        DialectId::OpenAiChat | DialectId::OpenAiResponses
    );
    (openai && r.mode != Some(ReasoningMode::Disabled))
        .then_some(r.budget_tokens)
        .flatten()
}

fn check_budget(budget: u32, ingress: DialectId, eff: &Effective) -> Result<(), Unsupported> {
    let levels = effort_levels(eff, ingress);
    let effort = reasoning_map::effort_for_budget(budget);
    if levels.is_empty() || levels.iter().any(|l| l == effort.as_wire()) {
        return Ok(());
    }
    let bands = [
        ("low", 0),
        ("medium", reasoning_map::MEDIUM_FROM),
        ("high", reasoning_map::HIGH_FROM),
    ];
    let supported = |band: &&(&str, u32)| levels.iter().any(|l| l == band.0);
    let at = bands
        .iter()
        .position(|(level, _)| *level == effort.as_wire())
        .unwrap_or(0);
    let above = bands.iter().skip(at + 1).find(supported);
    let below = bands
        .iter()
        .take(at)
        .rposition(|band| supported(&band))
        .and_then(|i| bands.get(i + 1));
    let model = &eff.model_id;
    let effort = effort.as_wire();
    let message = match (above, below) {
        (Some((_, min)), _) => format!(
            "A `budget_tokens` of {budget} means `{effort}`; `{model}` needs at least {min}."
        ),
        (None, Some((_, max))) => format!(
            "A `budget_tokens` of {budget} means `{effort}`; `{model}` needs less than {max}."
        ),
        (None, None) => format!(
            "A `budget_tokens` of {budget} means `{effort}`; `{model}` needs `thinking` off."
        ),
    };
    Err(refuse(
        "effort",
        Some(app_param(ingress, "budget")),
        message,
    ))
}

fn kpp_turns_thinking_on(g: &GenerateRequest, ingress: DialectId, eff: &Effective) -> bool {
    let Some(r) = &g.reasoning else {
        return false;
    };
    ingress != DialectId::AnthropicMessages
        && eff.dialect == DialectId::AnthropicMessages
        && (r
            .effort
            .as_ref()
            .is_some_and(|e| !matches!(e, ReasoningEffort::None | ReasoningEffort::Minimal))
            || matches!(
                r.mode,
                Some(ReasoningMode::Enabled | ReasoningMode::Adaptive)
            )
            || r.budget_tokens.is_some()
            || (r.summary.is_some() && r.effort.is_none() && eff.fields.reasoning == Some(true)))
}

pub fn effort_levels(eff: &Effective, ingress: DialectId) -> Vec<String> {
    let mut levels = eff.fields.efforts.clone().unwrap_or_default();
    if eff.dialect == DialectId::AnthropicMessages
        && eff.fields.adaptive_thinking == Some(false)
        && ingress != DialectId::AnthropicMessages
        && eff.fields.reasoning == Some(true)
    {
        for e in ["low", "medium", "high", "xhigh", "max"] {
            if !levels.iter().any(|l| l == e) {
                levels.push(e.to_owned());
            }
        }
    }
    levels
}

fn check_effort(
    effort: &ReasoningEffort,
    ingress: DialectId,
    eff: &Effective,
) -> Result<(), Unsupported> {
    let name = eff.model_id.as_str();
    let wire = effort.as_wire();
    let levels = effort_levels(eff, ingress);
    if *effort == ReasoningEffort::None
        && (eff.dialect == DialectId::AnthropicMessages
            || levels.is_empty()
            || levels.iter().any(|l| l == "none"))
    {
        return Ok(());
    }
    if levels.iter().any(|l| l == wire) {
        return Ok(());
    }
    let message = if levels.is_empty() {
        format!("The model `{name}` does not accept an effort level; `{wire}` cannot be sent.")
    } else {
        format!(
            "The model `{name}` supports the effort levels {}; `{wire}` is not one of them.",
            levels
                .iter()
                .map(|l| format!("`{l}`"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    Err(refuse(
        "effort",
        Some(app_param(ingress, "effort")),
        message,
    ))
}

pub fn model_info(id: &str, eff: &Effective) -> ModelInfo {
    let mut info = ModelInfo::new(id);
    info.display_name = Some(eff.display_name().to_owned());
    info.created_at = eff.fields.release_date.as_deref().and_then(release_seconds);
    info.owned_by = Some(eff.provider.id().to_owned());
    info.context_window = eff.fields.context_limit;
    info.max_output_tokens = eff.fields.output_limit;
    info
}

fn release_seconds(date: &str) -> Option<u64> {
    let format = time::macros::format_description!("[year]-[month]-[day]");
    let d = time::Date::parse(date, &format).ok()?;
    u64::try_from(d.midnight().assume_utc().unix_timestamp()).ok()
}
