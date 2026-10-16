use kaeso_kpp::{
    Block, Extras, GenerateRequest, KppError, Origin, OutputSpec, ReasoningEffort, ReasoningMode,
    Request, Role, Source, ToolDef,
};

use super::common::{D, is_data_url, parse_base64_data_url, request_kind_name};
use crate::cross::check_foreign;

const IMAGE_MEDIA_TYPES: [&str; 4] = ["image/jpeg", "image/png", "image/gif", "image/webp"];

pub(crate) fn check(req: &Request, target: &Origin) -> Result<(), KppError> {
    check_foreign(req, target)?;
    let (r, generate) = match req {
        Request::Generate(r) => (r, true),
        Request::CountTokens(r) => (r, false),
        other => {
            return Err(KppError::unsupported_by(
                request_kind_name(other.kind()),
                D,
                "Anthropic Messages serves message generation and token counting only.",
            ));
        }
    };
    if generate && r.sampling.max_output_tokens.is_none() {
        return Err(KppError::invalid_request(
            "Anthropic Messages requires `max_tokens`. Set a maximum number of output tokens for the request.",
        )
        .with_param("max_tokens"));
    }
    if !r.extras.is_from(D)
        && let Some(i) = r.messages.iter().position(|m| m.role == Role::System)
    {
        return Err(KppError::unsupported_by(
            "system message",
            D,
            "Kaeso sends mid-conversation system messages to Anthropic only from Anthropic Messages requests. Move the instructions into the leading system prompt or into a user message.",
        )
        .with_param(format!("messages[{i}]")));
    }
    if generate {
        check_output(r)?;
        check_sampling(r)?;
        check_reasoning(r)?;
    }
    for (i, b) in r.system.iter().enumerate() {
        let param = format!("system[{i}]");
        if !matches!(b, Block::Text(_) | Block::Unknown(_)) {
            return Err(KppError::unsupported_by(
                format!("{} block in the system prompt", b.kind()),
                D,
                "The Anthropic system prompt holds text only.",
            )
            .with_param(param));
        }
        check_block(b, &param)?;
    }
    for (mi, m) in r.messages.iter().enumerate() {
        for (bi, b) in m.blocks.iter().enumerate() {
            check_block(b, &format!("messages[{mi}].blocks[{bi}]"))?;
        }
    }
    for (i, t) in r.tools.iter().enumerate() {
        if let ToolDef::Function(f) = t
            && f.parameters.as_ref().is_some_and(|p| !p.is_object())
        {
            return Err(KppError::unsupported_by(
                "tool parameters that are not a JSON object schema",
                D,
                "Anthropic requires `input_schema` to be an object schema.",
            )
            .with_param(format!("tools[{i}]")));
        }
    }
    Ok(())
}

pub(crate) fn check_output(r: &GenerateRequest) -> Result<(), KppError> {
    match &r.output {
        OutputSpec::JsonObject => Err(KppError::unsupported_by(
            "json_object output",
            D,
            "Anthropic has no JSON mode without a schema. Send a JSON schema (structured outputs) instead.",
        )
        .with_param("output")),
        OutputSpec::JsonSchema(spec) if spec.description.is_some() => Err(KppError::unsupported_by(
            "output schema description",
            D,
            "Anthropic structured outputs take a schema only. Move the description into the prompt or the schema's own `description`.",
        )
        .with_param("output")),
        OutputSpec::JsonSchema(spec) if spec.strict == Some(false) => {
            Err(KppError::unsupported_by(
                "non-strict output schema",
                D,
                "Anthropic always enforces the output schema. Set `strict: true` or remove it.",
            )
            .with_param("output"))
        }
        _ => Ok(()),
    }
}

fn check_sampling(r: &GenerateRequest) -> Result<(), KppError> {
    let s = &r.sampling;
    let unsupported = [
        ("seed", s.seed.is_some()),
        ("frequency_penalty", s.frequency_penalty.is_some()),
        ("presence_penalty", s.presence_penalty.is_some()),
    ];
    if let Some((name, _)) = unsupported.iter().find(|(_, set)| *set) {
        return Err(KppError::unsupported_by(
            *name,
            D,
            &format!("Remove `{name}` or route the request to an OpenAI model."),
        )
        .with_param(*name));
    }
    if let Some(t) = s.temperature
        && !(0.0..=1.0).contains(&t)
    {
        return Err(KppError::unsupported_by(
            format!("temperature {t}"),
            D,
            "Anthropic accepts a temperature from 0 to 1.",
        )
        .with_param("temperature"));
    }
    Ok(())
}

fn base64_media_type(source: &Source, extras: &Extras) -> Result<Option<String>, ()> {
    match source {
        Source::Base64(s) => Ok(Some(s.media_type.clone())),
        Source::Url(u) if is_data_url(u) && !extras.is_from(D) => {
            parse_base64_data_url(u).map(|(m, _)| Some(m)).ok_or(())
        }
        _ => Ok(None),
    }
}

pub(crate) fn check_reasoning(r: &GenerateRequest) -> Result<(), KppError> {
    let Some(c) = &r.reasoning else {
        return Ok(());
    };
    let own = c.extras.is_from(D);
    let fail = |feature: &str, hint: &str| {
        Err(KppError::unsupported_by(feature, D, hint).with_param("reasoning"))
    };
    if c.include_encrypted && own {
        return fail(
            "reasoning.include_encrypted",
            "Encrypted reasoning is an OpenAI Responses feature; Anthropic returns signed thinking blocks instead.",
        );
    }
    if c.summary.is_some() && own {
        return fail(
            "reasoning.summary",
            "Anthropic has no reasoning summary setting. Remove it.",
        );
    }
    match &c.effort {
        Some(ReasoningEffort::None)
            if !own
                && matches!(c.mode, None | Some(ReasoningMode::Disabled))
                && c.budget_tokens.is_none() => {}
        Some(e @ (ReasoningEffort::None | ReasoningEffort::Minimal)) => {
            return fail(
                &format!("reasoning effort `{}`", e.as_wire()),
                "Anthropic effort levels are low, medium, high, xhigh and max.",
            );
        }
        Some(ReasoningEffort::Other(e)) if !own => {
            return fail(
                &format!("reasoning effort `{e}`"),
                "Anthropic effort levels are low, medium, high, xhigh and max.",
            );
        }
        _ => {}
    }
    match (&c.mode, c.budget_tokens) {
        (Some(ReasoningMode::Enabled), None) => fail(
            "reasoning without budget_tokens",
            "Anthropic `enabled` thinking needs `budget_tokens`; Kaeso does not convert an effort level into a budget (D4). Set a budget, or use adaptive thinking with an effort level.",
        ),
        (Some(m @ (ReasoningMode::Adaptive | ReasoningMode::Disabled)), Some(_)) => fail(
            "reasoning.budget_tokens",
            &format!(
                "Anthropic `{}` thinking takes no budget. Remove `budget_tokens`, or use `enabled` thinking.",
                if *m == ReasoningMode::Adaptive {
                    "adaptive"
                } else {
                    "disabled"
                }
            ),
        ),
        (Some(ReasoningMode::Other(m)), _) if !own => fail(
            &format!("reasoning mode `{m}`"),
            "Anthropic thinking types are enabled, adaptive and disabled.",
        ),
        _ => Ok(()),
    }
}

fn check_block(block: &Block, param: &str) -> Result<(), KppError> {
    let fail = |feature: String, hint: &str| {
        Err(KppError::unsupported_by(feature, D, hint).with_param(param))
    };
    match block {
        Block::Refusal(_) => fail(
            "refusal block".into(),
            "Anthropic has no refusal content block. Remove the refusal from the conversation history or replace it with text.",
        ),
        Block::Image(b) => match (&b.source, base64_media_type(&b.source, &b.extras)) {
            (_, Err(())) => fail(
                "data URL without base64 encoding".into(),
                "Send the image as a base64 data URL or as base64 data.",
            ),
            (_, Ok(Some(m))) if !IMAGE_MEDIA_TYPES.contains(&m.as_str()) => fail(
                format!("image media type `{m}`"),
                "Anthropic accepts JPEG, PNG, GIF and WebP images.",
            ),
            (Source::Text(_), _) => fail(
                "text image source".into(),
                "Send the image as base64, a URL or a file id.",
            ),
            _ => Ok(()),
        },
        Block::Document(b) => match (&b.source, base64_media_type(&b.source, &b.extras)) {
            (_, Err(())) => fail(
                "data URL without base64 encoding".into(),
                "Send the document as a base64 data URL or as base64 data.",
            ),
            (_, Ok(Some(m))) if m != "application/pdf" => fail(
                format!("document media type `{m}`"),
                "Anthropic accepts PDF documents as base64, URL or file, and plain-text documents as text.",
            ),
            (Source::Text(s), _) if s.media_type != "text/plain" => fail(
                format!("text document media type `{}`", s.media_type),
                "Anthropic text documents must be `text/plain`.",
            ),
            _ => Ok(()),
        },
        Block::ToolCall(c) if !c.arguments.is_object() => fail(
            "tool call arguments that are not a JSON object".into(),
            "Anthropic `tool_use.input` must be a JSON object.",
        ),
        Block::ToolResult(t) => {
            for (i, b) in t.content.iter().enumerate() {
                let param = format!("{param}.content[{i}]");
                if !matches!(
                    b,
                    Block::Text(_) | Block::Image(_) | Block::Document(_) | Block::Unknown(_)
                ) {
                    return Err(KppError::unsupported_by(
                        format!("{} block in a tool result", b.kind()),
                        D,
                        "Anthropic tool results hold text, images and documents.",
                    )
                    .with_param(param));
                }
                check_block(b, &param)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
