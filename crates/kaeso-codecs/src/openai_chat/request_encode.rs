use indexmap::IndexMap;
use kaeso_kpp::{
    Block, Extras, GenerateRequest, KppError, Message, OutputSpec, RawJson, Role, StreamMode,
    TextBlock, ToolChoice, ToolDef, ToolResultBlock,
};
use serde_json::Value;

use super::DIALECT;
use super::convert::{
    FORM_REFUSAL_PART, encode_file_part, encode_image_part, encode_refusal_part, encode_text_part,
    encode_tool_call, ext_nested, ext_with, is_tool_call_item, nested_keys, safe_tool_name, to_raw,
};
use super::profile::{ChatProfile, OutputLimit, SeedField};
use super::request::{
    CONFLICT_KEYS, FORM_CONTENT, FORM_MAX_TOKENS, FORM_MESSAGE, FORM_N, FORM_RESPONSE_FORMAT_TEXT,
    FORM_ROLE, FORM_SCHEMA_ABSENT, FORM_STOP, FORM_STREAM_FALSE, FORM_STREAM_OPTIONS,
    FORM_STREAM_OPTIONS_OFF, FORM_USER, FUNCTION_ROLE, KEY_MAX_TOKENS_CONFLICT, MESSAGE_PREFIX,
};
use super::wire::{
    AssistantMessage, ChatRequest, FunctionObject, FunctionToolWire, InstructionMessage,
    JsonSchemaWire, ResponseFormat, StreamOptions, ToolMessage, UserMessage,
};
use crate::codec::EgressCtx;
use crate::wire::{Ext, StringOr};

pub(crate) const DEFAULT_SCHEMA_NAME: &str = "response";

pub(crate) fn encode_generate(
    req: &GenerateRequest,
    ctx: &mut EgressCtx,
) -> Result<Ext<ChatRequest>, KppError> {
    let own = req.extras.is_from(DIALECT);
    let form = |key: &str| req.extras.form_value(DIALECT, key);
    let profile = ctx.chat_profile;
    let rules = profile.request;
    let mut names = Names { ctx };
    let mut w = ChatRequest {
        model: req.model.clone(),
        ..ChatRequest::default()
    };

    let s = &req.sampling;
    let number = |value: Option<f64>, key: &str| {
        crate::wire::spelled_number(value, key, &req.extras, DIALECT)
    };
    w.temperature = number(s.temperature, "temperature");
    let top_p = profile.checked_top_p(s.temperature, s.top_p)?;
    w.top_p = number(top_p, "top_p");
    let seed = s.seed.map(|seed| profile.checked_seed(seed)).transpose()?;
    if rules.seed == SeedField::Seed {
        w.seed = seed;
    }
    w.frequency_penalty = number(s.frequency_penalty, "frequency_penalty");
    w.presence_penalty = number(s.presence_penalty, "presence_penalty");
    if let Some(max) = s.max_output_tokens {
        match form(FORM_MAX_TOKENS).and_then(Value::as_str) {
            _ if rules.output_limit == OutputLimit::MaxTokens => w.max_tokens = Some(max),
            Some("max_tokens") => w.max_tokens = Some(max),
            Some("both") => {
                w.max_tokens = Some(max);
                w.max_completion_tokens = Some(max);
            }
            _ => w.max_completion_tokens = Some(max),
        }
    }
    let stop_form = form(FORM_STOP).and_then(Value::as_str);
    w.stop = match (s.stop.as_slice(), stop_form) {
        ([], Some("array")) => Some(StringOr::List(Vec::new())),
        ([], _) => None,
        ([one], Some("string")) => Some(StringOr::String(one.clone())),
        (stops, _) => Some(StringOr::List(stops.to_vec())),
    };
    if form(FORM_N).is_some() {
        w.n = Some(1);
    }
    if let Some(user) = req.user.as_ref().filter(|_| rules.end_user) {
        if own && req.extras.form_flag(DIALECT, FORM_USER) {
            w.user = Some(user.clone());
        } else {
            w.safety_identifier = Some(user.clone());
        }
    }
    encode_stream(req, own, &mut w)?;
    w.reasoning_effort = req
        .reasoning
        .as_ref()
        .and_then(crate::cross::reasoning_map::openai_effort)
        .map(|e| e.as_wire().to_owned());
    w.response_format = encode_response_format(req)?;
    for tool in &req.tools {
        match tool {
            ToolDef::Function(f) => {
                let function = FunctionObject {
                    name: names.send(&f.name),
                    description: f.description.clone(),
                    parameters: f.parameters.clone(),
                    strict: f.strict,
                };
                let wire = FunctionToolWire {
                    ty: "function".to_owned(),
                    function: ext_nested(function, &f.extras, "function"),
                };
                w.tools.push(to_raw(&ext_with(wire, &f.extras))?);
            }
            ToolDef::Unknown(item) if item.origin.is(DIALECT) => w.tools.push(item.raw.clone()),
            _ => {
                return Err(KppError::unsupported_by(
                    "tool",
                    DIALECT,
                    "Only function tools can be sent to it.",
                ));
            }
        }
    }
    w.tool_choice = match &req.tool_choice {
        None => None,
        Some(ToolChoice::Auto) => Some(to_raw("auto")?),
        Some(ToolChoice::None) => Some(to_raw("none")?),
        Some(ToolChoice::Required) => Some(to_raw("required")?),
        Some(ToolChoice::Tool(name)) => Some(to_raw(&serde_json::json!({
            "type": "function",
            "function": {"name": names.send(name)}
        }))?),
        Some(ToolChoice::Unknown(item)) if item.origin.is(DIALECT) => Some(item.raw.clone()),
        Some(_) => {
            return Err(KppError::unsupported_by(
                "tool_choice",
                DIALECT,
                "Use `auto`, `none`, `required` or a single named function.",
            ));
        }
    };
    w.parallel_tool_calls = req.parallel_tool_calls;

    encode_system(&req.system, rules.developer_role, &mut w.messages)?;
    for message in &req.messages {
        encode_message(message, &mut names, &mut w.messages)?;
    }

    let (extra, verbatim) = super::convert::own_keys(&req.extras);
    let extra = extra
        .into_iter()
        .map(|(key, value)| {
            let wire = CONFLICT_KEYS
                .iter()
                .find(|(stored, _)| *stored == key)
                .map_or(key, |(_, wire)| (*wire).to_owned());
            (wire, value)
        })
        .collect();
    let mut out = Ext::with(w, extra, verbatim);
    apply_profile(&profile, seed, &req.extras, &mut out)?;
    Ok(out)
}

fn apply_profile(
    profile: &ChatProfile,
    seed: Option<i64>,
    extras: &Extras,
    out: &mut Ext<ChatRequest>,
) -> Result<(), KppError> {
    if profile.request.greedy_top_p && out.known.top_p.is_some() {
        out.verbatim.shift_remove("top_p");
    }
    if profile.request.output_limit == OutputLimit::MaxTokens {
        if extras
            .fields_for(DIALECT)
            .is_some_and(|f| f.contains_key(KEY_MAX_TOKENS_CONFLICT))
        {
            return Err(KppError::unsupported(
                "max_tokens",
                format!(
                    "{} takes one output limit; send `max_tokens` or `max_completion_tokens`, not both with different values.",
                    profile.provider
                ),
            )
            .with_param("max_tokens"));
        }
        rename_null(
            &mut out.verbatim,
            "max_completion_tokens",
            Some("max_tokens"),
        );
    }
    if profile.request.seed == SeedField::RandomSeed {
        let sent_seed = seed.is_some() || out.verbatim.contains_key("seed");
        let sent_random_seed =
            out.extra.contains_key("random_seed") || out.verbatim.contains_key("random_seed");
        if sent_seed && sent_random_seed {
            return Err(KppError::unsupported(
                "seed",
                format!(
                    "{} takes one seed; send `seed` or `random_seed`, not both.",
                    profile.provider
                ),
            )
            .with_param("seed"));
        }
        rename_null(&mut out.verbatim, "seed", Some("random_seed"));
        if let Some(seed) = seed {
            out.extra.insert("random_seed".to_owned(), to_raw(&seed)?);
        }
    }
    if !profile.request.end_user {
        out.extra.shift_remove("user");
        rename_null(&mut out.verbatim, "user", None);
        rename_null(&mut out.verbatim, "safety_identifier", None);
    }
    Ok(())
}

fn rename_null(verbatim: &mut IndexMap<String, RawJson>, from: &str, to: Option<&str>) {
    if let Some(null) = verbatim.shift_remove(from)
        && let Some(to) = to
    {
        verbatim.entry(to.to_owned()).or_insert(null);
    }
}

struct Names<'a> {
    ctx: &'a mut EgressCtx,
}

impl Names<'_> {
    fn send(&mut self, name: &str) -> String {
        safe_tool_name(name, &mut self.ctx.tool_names)
    }
}

fn encode_stream(req: &GenerateRequest, own: bool, w: &mut ChatRequest) -> Result<(), KppError> {
    let form = |key: &str| req.extras.form_value(DIALECT, key);
    match req.stream {
        StreamMode::On { include_usage, .. } => {
            w.stream = Some(true);
            let spelling = form(FORM_STREAM_OPTIONS).and_then(Value::as_str);
            let (extra, verbatim) = nested_keys(&req.extras, "stream_options");
            w.stream_options = if !own {
                Some(Ext::new(StreamOptions {
                    include_usage: Some(true),
                }))
            } else {
                match spelling {
                    Some("absent") if !include_usage => None,
                    Some("absent") => Some(Ext::new(StreamOptions {
                        include_usage: Some(true),
                    })),
                    Some("no_include_usage") => Some(Ext::with(
                        StreamOptions {
                            include_usage: include_usage.then_some(true),
                        },
                        extra,
                        verbatim,
                    )),
                    _ => Some(Ext::with(
                        StreamOptions {
                            include_usage: Some(include_usage),
                        },
                        extra,
                        verbatim,
                    )),
                }
            };
        }
        _ => {
            if own && req.extras.form_flag(DIALECT, FORM_STREAM_FALSE) {
                w.stream = Some(false);
            }
            if let Some(value) = form(FORM_STREAM_OPTIONS_OFF) {
                let raw = to_raw(value)?;
                w.stream_options = Ext::from_raw(raw.as_raw()).ok();
            }
        }
    }
    Ok(())
}

fn encode_response_format(req: &GenerateRequest) -> Result<Option<RawJson>, KppError> {
    let nested = |ty: &str| {
        let format = ResponseFormat {
            ty: ty.to_owned(),
            json_schema: None,
        };
        to_raw(&ext_nested(format, &req.extras, "response_format"))
    };
    match &req.output {
        OutputSpec::Text => {
            if req.extras.form_flag(DIALECT, FORM_RESPONSE_FORMAT_TEXT) {
                Ok(Some(nested("text")?))
            } else {
                Ok(None)
            }
        }
        OutputSpec::JsonObject => Ok(Some(nested("json_object")?)),
        OutputSpec::JsonSchema(spec) => {
            let absent = spec.extras.form_flag(DIALECT, FORM_SCHEMA_ABSENT);
            let own = spec.extras.is_from(DIALECT);
            let js = JsonSchemaWire {
                name: match (&spec.name, own) {
                    (Some(name), _) => Some(name.clone()),
                    (None, true) => None,
                    (None, false) => Some(DEFAULT_SCHEMA_NAME.to_owned()),
                },
                description: spec.description.clone(),
                schema: (!absent).then(|| spec.schema.clone()),
                strict: spec.strict,
            };
            let format = ResponseFormat {
                ty: "json_schema".to_owned(),
                json_schema: Some(ext_nested(js, &spec.extras, "json_schema")),
            };
            Ok(Some(to_raw(&ext_with(format, &spec.extras))?))
        }
        _ => Err(KppError::unsupported_by(
            "response format",
            DIALECT,
            "Use text, a JSON object or a JSON schema.",
        )),
    }
}

fn instruction_role(written: Option<&str>, developer_role: bool) -> &str {
    match written {
        Some("developer") if !developer_role => "system",
        Some(role) => role,
        None => "system",
    }
}

fn encode_system(
    system: &[Block],
    developer_role: bool,
    out: &mut Vec<RawJson>,
) -> Result<(), KppError> {
    let mut groups: Vec<Vec<&Block>> = Vec::new();
    for block in system {
        let starts = block
            .extras()
            .is_some_and(|e| e.form_value(DIALECT, FORM_MESSAGE).is_some());
        match groups.last_mut() {
            Some(group) if !starts => group.push(block),
            _ => groups.push(vec![block]),
        }
    }
    for group in groups {
        let first = group.first().and_then(|b| b.extras());
        let marker = first.and_then(|e| e.form_value(DIALECT, FORM_MESSAGE));
        let role = instruction_role(
            marker.and_then(|m| m.get("role")).and_then(Value::as_str),
            developer_role,
        );
        let spelling = marker
            .and_then(|m| m.get("content"))
            .and_then(Value::as_str);
        let content = encode_text_content(&group, spelling)?;
        let message = InstructionMessage {
            role: role.to_owned(),
            content,
        };
        let wire = match first {
            Some(extras) => {
                let (extra, verbatim) = nested_keys(extras, MESSAGE_PREFIX);
                Ext::with(message, extra, verbatim)
            }
            None => Ext::new(message),
        };
        out.push(to_raw(&wire)?);
    }
    Ok(())
}

fn encode_text_content(
    blocks: &[&Block],
    spelling: Option<&str>,
) -> Result<StringOr<RawJson>, KppError> {
    if spelling != Some("array")
        && let [Block::Text(text)] = blocks
        && plain_text(text)
    {
        return Ok(StringOr::String(text.text.clone()));
    }
    if blocks.is_empty() && spelling != Some("array") {
        return Ok(StringOr::String(String::new()));
    }
    let mut parts = Vec::new();
    for block in blocks {
        parts.push(match block {
            Block::Text(t) => encode_text_part(t)?,
            Block::Unknown(item) if item.origin.is(DIALECT) => item.raw.clone(),
            other => {
                return Err(block_unsupported(
                    other,
                    "a system, developer or tool message",
                ));
            }
        });
    }
    Ok(StringOr::List(parts))
}

fn plain_text(text: &TextBlock) -> bool {
    let (extra, verbatim) = super::convert::own_keys(&text.extras);
    super::convert::encode_breakpoint(text.cache.as_ref()).is_none()
        && extra.is_empty()
        && verbatim.is_empty()
}

fn content_form(message: &Message) -> Option<&str> {
    message.extras.form_str(DIALECT, FORM_CONTENT)
}

fn encode_message(
    message: &Message,
    names: &mut Names<'_>,
    out: &mut Vec<RawJson>,
) -> Result<(), KppError> {
    match message.role {
        Role::System => {
            let role = instruction_role(
                message.extras.form_str(DIALECT, FORM_ROLE),
                names.ctx.chat_profile.request.developer_role,
            );
            let blocks: Vec<&Block> = message
                .blocks
                .iter()
                .filter(|b| !matches!(b, Block::Reasoning(_)))
                .collect();
            let content = encode_text_content(&blocks, content_form(message))?;
            let wire = InstructionMessage {
                role: role.to_owned(),
                content,
            };
            out.push(to_raw(&ext_with(wire, &message.extras))?);
        }
        Role::User => encode_user(message, out)?,
        Role::Assistant => out.push(encode_assistant(message, names)?),
        _ => {
            return Err(KppError::unsupported_by(
                "message role",
                DIALECT,
                "Messages must be user, assistant, system or developer messages.",
            ));
        }
    }
    Ok(())
}

fn encode_user(message: &Message, out: &mut Vec<RawJson>) -> Result<(), KppError> {
    if message.extras.form_str(DIALECT, FORM_ROLE) == Some(FUNCTION_ROLE)
        && let [Block::Unknown(item)] = message.blocks.as_slice()
        && item.origin.is(DIALECT)
    {
        out.push(item.raw.clone());
        return Ok(());
    }
    let mut rest: Vec<&Block> = Vec::new();
    for block in &message.blocks {
        match block {
            Block::ToolResult(result) => out.push(encode_tool_message(result)?),
            Block::Reasoning(_) => {}
            other => rest.push(other),
        }
    }
    if rest.is_empty() {
        return Ok(());
    }
    let spelling = content_form(message);
    let content = match rest.as_slice() {
        [Block::Text(text)] if spelling != Some("array") && plain_text(text) => {
            StringOr::String(text.text.clone())
        }
        blocks => {
            let mut parts = Vec::new();
            for block in blocks {
                parts.push(match block {
                    Block::Text(t) => encode_text_part(t)?,
                    Block::Image(i) => encode_image_part(i)?,
                    Block::Document(d) => encode_file_part(d)?,
                    Block::Unknown(item) if item.origin.is(DIALECT) => item.raw.clone(),
                    other => return Err(block_unsupported(other, "a user message")),
                });
            }
            StringOr::List(parts)
        }
    };
    let wire = UserMessage {
        role: "user".to_owned(),
        content,
    };
    out.push(to_raw(&ext_with(wire, &message.extras))?);
    Ok(())
}

fn encode_tool_message(result: &ToolResultBlock) -> Result<RawJson, KppError> {
    let blocks: Vec<&Block> = result.content.iter().collect();
    let spelling = result.extras.form_str(DIALECT, FORM_CONTENT);
    let wire = ToolMessage {
        role: "tool".to_owned(),
        content: encode_text_content(&blocks, spelling)?,
        tool_call_id: result.call_id.clone(),
    };
    to_raw(&ext_with(wire, &result.extras))
}

fn encode_assistant(message: &Message, names: &mut Names<'_>) -> Result<RawJson, KppError> {
    let mut content: Vec<&Block> = Vec::new();
    let mut refusal: Option<String> = None;
    let mut tool_calls = Vec::new();
    for block in &message.blocks {
        match block {
            Block::Text(_) => content.push(block),
            Block::Refusal(r) => {
                let part = r.extras.form_flag(DIALECT, FORM_REFUSAL_PART);
                if !part && refusal.is_none() {
                    refusal = Some(r.text.clone());
                } else {
                    content.push(block);
                }
            }
            Block::ToolCall(call) => {
                let name = names.send(&call.name);
                tool_calls.push(encode_tool_call(call, &name)?);
            }
            Block::Unknown(item) if item.origin.is(DIALECT) => {
                if is_tool_call_item(item) {
                    tool_calls.push(item.raw.clone());
                } else {
                    content.push(block);
                }
            }
            Block::Reasoning(_) => {}
            other => return Err(block_unsupported(other, "an assistant message")),
        }
    }
    let spelling = content_form(message);
    let annotations = content
        .iter()
        .filter_map(|b| match b {
            Block::Text(t) => Some(t.citations.iter()),
            _ => None,
        })
        .flatten()
        .filter(|c| c.origin.is(DIALECT))
        .map(|c| c.raw.clone())
        .collect();
    let mut wire = AssistantMessage {
        role: "assistant".to_owned(),
        refusal,
        tool_calls,
        annotations,
        ..AssistantMessage::default()
    };
    let mut wrapped = ext_with(AssistantMessage::default(), &message.extras);
    wire.content = match content.as_slice() {
        [] if !message.extras.is_from(DIALECT)
            && wire.tool_calls.is_empty()
            && wire.refusal.is_none() =>
        {
            Some(StringOr::String(String::new()))
        }
        [] => {
            if spelling != Some("absent") && !wrapped.verbatim.contains_key("content") {
                wrapped
                    .verbatim
                    .insert("content".to_owned(), RawJson::null());
            }
            None
        }
        [Block::Text(text)] if spelling != Some("array") && plain_text(text) => {
            Some(StringOr::String(text.text.clone()))
        }
        blocks => {
            let mut parts = Vec::new();
            for block in blocks {
                parts.push(match block {
                    Block::Text(t) => encode_text_part(t)?,
                    Block::Refusal(r) => encode_refusal_part(r)?,
                    Block::Unknown(item) => item.raw.clone(),
                    other => return Err(block_unsupported(other, "an assistant message")),
                });
            }
            Some(StringOr::List(parts))
        }
    };
    wrapped.known = wire;
    to_raw(&wrapped)
}

fn block_unsupported(block: &Block, place: &str) -> KppError {
    KppError::unsupported(
        format!("{} block in {place}", block.kind()),
        format!(
            "OpenAI Chat Completions cannot carry a {} block in {place}.",
            block.kind()
        ),
    )
}
