use kaeso_kpp::{DialectId, GenerateRequest, ModelInfo, ReasoningEffort, Request};

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
        .dialects
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
            check_generation(g, ingress, eff)?;
            Ok(Plan {
                dialect: eff.provider.count_tokens,
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
    if caps.images && !eff.accepts("image") {
        return Err(refuse(
            "image input",
            Some("messages"),
            format!("The model `{name}` does not accept images."),
        ));
    }
    if caps.pdf && !eff.accepts("pdf") {
        return Err(refuse(
            "pdf input",
            Some("messages"),
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
    if caps.json_schema && eff.fields.structured_output != Some(true) {
        return Err(refuse(
            "structured output",
            Some("response_format"),
            format!("The model `{name}` does not support structured output (a JSON schema)."),
        ));
    }
    if caps.reasoning && eff.fields.reasoning != Some(true) {
        return Err(refuse(
            "reasoning",
            Some("reasoning"),
            format!("The model `{name}` does not support reasoning."),
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
    }
    Ok(())
}

fn kpp_turns_thinking_on(g: &GenerateRequest, ingress: DialectId, eff: &Effective) -> bool {
    use kaeso_kpp::ReasoningMode;
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
    Err(refuse("effort", Some("reasoning_effort"), message))
}

pub fn model_info(id: &str, eff: &Effective) -> ModelInfo {
    let mut info = ModelInfo::new(id);
    info.display_name = Some(eff.display_name().to_owned());
    info.created_at = eff.fields.release_date.as_deref().and_then(release_seconds);
    info.owned_by = Some(eff.provider.id.to_owned());
    info.context_window = eff.fields.context_limit;
    info.max_output_tokens = eff.fields.output_limit;
    info
}

fn release_seconds(date: &str) -> Option<u64> {
    let format = time::macros::format_description!("[year]-[month]-[day]");
    let d = time::Date::parse(date, &format).ok()?;
    u64::try_from(d.midnight().assume_utc().unix_timestamp()).ok()
}
