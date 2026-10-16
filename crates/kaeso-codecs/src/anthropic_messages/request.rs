use kaeso_kpp::{
    Block, Extras, FunctionTool, GenerateRequest, JsonSchemaSpec, KppError, Message, OpaqueKind,
    Origin, OutputSpec, RawItem, RawJson, ReasoningBlock, ReasoningConfig, ReasoningEffort,
    ReasoningMode, Request, RequestKind, Role, StreamMode, ToolChoice, ToolDef,
};
use serde_json::Value;

use super::blocks::{
    BlockEncoder, Dec, Side, decode_block, decode_cache, decode_content, empty_parameters, own_ext,
};
use super::check::{check_output, check_reasoning};
use super::common::{D, internal, nested_ext, no_position, request_kind_name, to_raw};
use super::wire::{
    JsonFormatW, MessageW, MetadataW, OutputConfigW, Raw, RequestW, ThinkingConfigW, ToolChoiceW,
    ToolW,
};
use super::{ANTHROPIC_VERSION, COUNT_TOKENS_PATH, MESSAGES_PATH};
use crate::codec::{EgressCtx, EncodedRequest, HeaderList, is_forwarded_header};
use crate::cross::{reasoning_map, reasoning_replayable};
use crate::wire::{
    Ext, StringOr, add_nested_extra, extras_from, nested_extra_for, to_body, type_tag,
    verbatim_from_form, verbatim_into_form,
};

const OBJECTS_FORM: &str = "objects";
const SYSTEM_FORM: &str = "system";

fn invalid_body(detail: impl std::fmt::Display) -> KppError {
    KppError::invalid_request(format!(
        "The request body is not a valid Anthropic Messages request: {detail}."
    ))
}

fn parse_body(body: &[u8]) -> Result<Ext<RequestW>, KppError> {
    let raw: &serde_json::value::RawValue =
        serde_json::from_slice(body).map_err(|_| invalid_body("the body is not valid JSON"))?;
    let object: serde_json::Map<String, Value> = serde_json::from_str(raw.get())
        .map_err(|_| invalid_body("the body must be a JSON object"))?;
    Ext::from_raw(raw).map_err(|e| describe(&object, &e))
}

fn describe(object: &serde_json::Map<String, Value>, err: &serde_json::Error) -> KppError {
    type Check = fn(&Value) -> bool;
    let int: Check = |v| v.as_u64().is_some_and(|n| u32::try_from(n).is_ok());
    let fields: &[(&str, Check, &str)] = &[
        ("model", Value::is_string, "a string"),
        ("messages", Value::is_array, "an array of messages"),
        ("max_tokens", int, "a non-negative integer"),
        (
            "system",
            |v| v.is_string() || v.is_array(),
            "a string or an array of text blocks",
        ),
        ("metadata", Value::is_object, "an object"),
        ("output_config", Value::is_object, "an object"),
        (
            "stop_sequences",
            |v| v.as_array().is_some_and(|a| a.iter().all(Value::is_string)),
            "an array of strings",
        ),
        ("stream", Value::is_boolean, "true or false"),
        ("temperature", Value::is_number, "a number"),
        ("top_p", Value::is_number, "a number"),
        ("top_k", int, "a non-negative integer"),
        ("thinking", Value::is_object, "an object"),
        ("tool_choice", Value::is_object, "an object"),
        ("tools", Value::is_array, "an array"),
    ];
    for (name, check, expected) in fields {
        match object.get(*name) {
            None if *name == "model" || *name == "messages" => {
                return invalid_body(format!("`{name}` is required")).with_param(*name);
            }
            Some(v) if !check(v) => {
                return invalid_body(format!("`{name}` must be {expected}")).with_param(*name);
            }
            _ => {}
        }
    }
    if let Some(messages) = object.get("messages").and_then(Value::as_array) {
        for (i, m) in messages.iter().enumerate() {
            let param = format!("messages[{i}]");
            let problem = match m {
                Value::Object(m) if !m.get("role").is_some_and(Value::is_string) => {
                    "needs a `role` string"
                }
                Value::Object(m)
                    if !m
                        .get("content")
                        .is_some_and(|c| c.is_string() || c.is_array()) =>
                {
                    "needs `content` as a string or an array of blocks"
                }
                Value::Object(_) => continue,
                _ => "must be an object",
            };
            return invalid_body(format!("message {i} {problem}")).with_param(param);
        }
    }
    invalid_body(no_position(err))
}

pub(crate) fn decode_request(
    kind: RequestKind,
    body: &[u8],
    headers: &HeaderList,
) -> Result<Request, KppError> {
    let origin = Origin::new(D);
    let w = parse_body(body)?;
    let Ext {
        known: w,
        extra,
        verbatim,
    } = w;
    let invalid = |param: String| {
        move |e: String| {
            KppError::invalid_request(format!(
                "The request body is not a valid Anthropic Messages request: {e}."
            ))
            .with_param(param)
        }
    };
    let dec = Dec {
        origin: &origin,
        ctx: None,
    };

    let mut req = GenerateRequest::new(w.model);
    req.extras = extras_from(&origin, extra, verbatim);
    for (name, value) in headers.iter() {
        if !is_forwarded_header(D, name) {
            continue;
        }
        if name.eq_ignore_ascii_case("anthropic-version") && value.trim() == ANTHROPIC_VERSION {
            continue;
        }
        req.extras.insert_header(name, value);
    }

    match w.system {
        None => {}
        Some(StringOr::String(s)) => {
            req.system = vec![kaeso_kpp::Block::text(s)];
            req.extras.form.insert(SYSTEM_FORM.into(), "string".into());
        }
        Some(StringOr::List(list)) => {
            if list.is_empty() {
                req.extras.form.insert(SYSTEM_FORM.into(), "array".into());
            }
            for (i, b) in list.iter().enumerate() {
                req.system
                    .push(decode_block(b, dec).map_err(invalid(format!("system[{i}]")))?);
            }
        }
    }

    for (i, m) in w.messages.into_iter().enumerate() {
        let role = match m.known.role.as_str() {
            "user" => Role::User,
            "assistant" => Role::Assistant,
            "system" => Role::System,
            other => {
                return Err(KppError::invalid_request(format!(
                    "Message {i} has role `{other}`; Anthropic Messages accepts `user`, `assistant` and `system`."
                ))
                .with_param(format!("messages[{i}].role")));
            }
        };
        let mut extras = extras_from(&origin, m.extra, m.verbatim);
        let blocks = decode_content(m.known.content, dec, &mut extras, "")
            .map_err(invalid(format!("messages[{i}].content")))?;
        let mut message = Message::new(role, blocks);
        message.extras = extras;
        req.messages.push(message);
    }

    req.sampling.max_output_tokens = w.max_tokens;
    req.sampling.temperature = w.temperature;
    req.sampling.top_p = w.top_p;
    req.sampling.top_k = w.top_k;
    req.sampling.stop = w.stop_sequences;
    if w.stream == Some(true) {
        req.stream = StreamMode::On {
            include_usage: true,
        };
    }

    let mut objects = Vec::new();
    if let Some(m) = w.metadata {
        req.user = m.known.user_id;
        add_nested_extra(&mut req.extras, "metadata", m.extra);
        verbatim_into_form(&mut req.extras, "metadata", m.verbatim);
        objects.push(Value::from("metadata"));
    }

    let mut effort = None;
    if let Some(oc) = w.output_config {
        effort = oc.known.effort.map(|e| ReasoningEffort::from_wire(&e));
        if let Some(format) = oc.known.format {
            match decode_format(&format, &origin) {
                Some(spec) => req.output = OutputSpec::JsonSchema(spec),
                None => {
                    req.extras
                        .fields
                        .insert("output_config.format".into(), RawJson::from(format));
                }
            }
        }
        add_nested_extra(&mut req.extras, "output_config", oc.extra);
        verbatim_into_form(&mut req.extras, "output_config", oc.verbatim);
        objects.push(Value::from("output_config"));
    }
    if !objects.is_empty() {
        req.extras
            .form
            .insert(OBJECTS_FORM.into(), Value::Array(objects));
    }

    if w.thinking.is_some() || effort.is_some() {
        let mut reasoning = ReasoningConfig::default();
        reasoning.extras = Extras::new(origin.clone());
        if let Some(t) = w.thinking {
            reasoning.mode = Some(match t.known.ty.as_str() {
                "enabled" => ReasoningMode::Enabled,
                "disabled" => ReasoningMode::Disabled,
                "adaptive" => ReasoningMode::Adaptive,
                other => ReasoningMode::Other(other.to_owned()),
            });
            reasoning.budget_tokens = t.known.budget_tokens;
            reasoning.extras = extras_from(&origin, t.extra, t.verbatim);
        }
        reasoning.effort = effort;
        req.reasoning = Some(reasoning);
    }

    if let Some(tc) = w.tool_choice {
        let choice = match (tc.known.ty.as_str(), &tc.known.name) {
            ("auto", None) => Some(ToolChoice::Auto),
            ("any", None) => Some(ToolChoice::Required),
            ("none", None) => Some(ToolChoice::None),
            ("tool", Some(name)) => Some(ToolChoice::Tool(name.clone())),
            _ => None,
        };
        match choice {
            Some(choice) => {
                req.tool_choice = Some(choice);
                req.parallel_tool_calls = tc.known.disable_parallel_tool_use.map(|d| !d);
                add_nested_extra(&mut req.extras, "tool_choice", tc.extra);
                verbatim_into_form(&mut req.extras, "tool_choice", tc.verbatim);
            }
            None => {
                let raw = to_raw(&tc)?;
                req.tool_choice = Some(ToolChoice::Unknown(RawItem::new(
                    origin.clone(),
                    RawJson::from(raw),
                )));
            }
        }
    }

    for (i, raw) in w.tools.iter().enumerate() {
        let ty = type_tag(raw).ok().flatten();
        if ty.is_none() || ty.as_deref() == Some("custom") {
            let t: Ext<ToolW> = Ext::from_raw(raw).map_err(|e| {
                KppError::invalid_request(format!(
                    "The request body is not a valid Anthropic Messages request: invalid tool: {}.",
                    no_position(&e)
                ))
                .with_param(format!("tools[{i}]"))
            })?;
            let mut f = FunctionTool::new(t.known.name, Some(RawJson::from(t.known.input_schema)));
            f.description = t.known.description;
            f.strict = t.known.strict;
            f.cache = t.known.cache_control.map(|c| decode_cache(c, &origin));
            f.extras = extras_from(&origin, t.extra, t.verbatim);
            req.tools.push(ToolDef::Function(f));
        } else {
            req.tools.push(ToolDef::Unknown(RawItem::new(
                origin.clone(),
                RawJson::from(raw.as_ref()),
            )));
        }
    }

    match kind {
        RequestKind::Generate => Ok(Request::Generate(req)),
        RequestKind::CountTokens => Ok(Request::CountTokens(req)),
        other => Err(KppError::unsupported_by(
            request_kind_name(other),
            D,
            "Anthropic Messages serves message generation (`/v1/messages`) and token counting (`/v1/messages/count_tokens`) only.",
        )),
    }
}

fn decode_format(raw: &Raw, origin: &Origin) -> Option<JsonSchemaSpec> {
    if type_tag(raw).ok().flatten().as_deref() != Some("json_schema") {
        return None;
    }
    let f: Ext<JsonFormatW> = Ext::from_raw(raw).ok()?;
    let mut spec = JsonSchemaSpec::new(RawJson::from(f.known.schema));
    spec.extras = extras_from(origin, f.extra, f.verbatim);
    Some(spec)
}

pub(crate) fn encode_request(
    req: &Request,
    ctx: &mut EgressCtx,
) -> Result<EncodedRequest, KppError> {
    let (r, path) = match req {
        Request::Generate(r) => (r, MESSAGES_PATH),
        Request::CountTokens(r) => (r, COUNT_TOKENS_PATH),
        other => {
            return Err(KppError::unsupported_by(
                request_kind_name(other.kind()),
                D,
                "Anthropic Messages serves message generation (`/v1/messages`) and token counting (`/v1/messages/count_tokens`) only.",
            ));
        }
    };
    let own = r.extras.is_from(D);
    let count_only = matches!(req, Request::CountTokens(_)) && !own;
    let mut enc = BlockEncoder::new(ctx.origin(D), Side::Request);
    let mut w = RequestW {
        model: r.model.clone(),
        ..RequestW::default()
    };

    let system_form = r.extras.form_str(D, SYSTEM_FORM);
    w.system = if system_form == Some("string")
        && let [kaeso_kpp::Block::Text(t)] = r.system.as_slice()
        && t.cache.is_none()
        && t.citations.is_empty()
        && t.extras.is_empty()
    {
        Some(StringOr::String(t.text.clone()))
    } else {
        let list = enc.blocks(&r.system)?;
        (!list.is_empty() || system_form == Some("array")).then_some(StringOr::List(list))
    };

    for m in &r.messages {
        let role = match m.role {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::System => "system",
            _ => {
                return Err(KppError::unsupported_by(
                    "message role",
                    D,
                    "This version of Kaeso does not know the role of this message; Anthropic accepts user, assistant and system messages.",
                ));
            }
        };
        let Some(content) = enc.content(&m.blocks, &m.extras, "")? else {
            continue;
        };
        if matches!(&content, StringOr::List(l) if l.is_empty()) && !m.blocks.is_empty() {
            continue;
        }
        w.messages.push(own_ext(
            MessageW {
                role: role.to_owned(),
                content,
            },
            &m.extras,
        ));
    }

    if !count_only {
        w.max_tokens = r.sampling.max_output_tokens;
        w.temperature = r.sampling.temperature;
        w.top_p = r.sampling.top_p;
        w.top_k = r.sampling.top_k;
        w.stop_sequences = r.sampling.stop.clone();
        if !r.stream.is_off() {
            w.stream = Some(true);
        }
    }

    let objects: Vec<&str> = r
        .extras
        .form_value(D, OBJECTS_FORM)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let present = |name: &str| {
        objects.contains(&name)
            || !nested_extra_for(&r.extras, D, name).is_empty()
            || !verbatim_from_form(&r.extras, D, name).is_empty()
    };

    let user = if count_only { None } else { r.user.clone() };
    if user.is_some() || present("metadata") {
        w.metadata = Some(nested_ext(
            MetadataW { user_id: user },
            &r.extras,
            "metadata",
        ));
    }

    let is_count = matches!(req, Request::CountTokens(_));
    let reasoning = r
        .reasoning
        .as_ref()
        .filter(|_| !is_count || check_reasoning(r).is_ok());
    let output = if !is_count || check_output(r).is_ok() {
        &r.output
    } else {
        &OutputSpec::Text
    };
    let mut plan = match reasoning {
        Some(c) => thinking_plan(c, r.sampling.max_output_tokens, ctx, is_count)?,
        None => ThinkingPlan::default(),
    };
    let enabled = plan
        .thinking
        .as_ref()
        .is_some_and(|t| t.known.ty == "enabled");
    let forced = matches!(
        r.tool_choice,
        Some(ToolChoice::Required | ToolChoice::Tool(_))
    );
    if enabled
        && (tool_turn_lacks_thinking(r, &ctx.origin(D))
            || (forced && !reasoning.is_some_and(|c| c.extras.is_from(D))))
    {
        plan = ThinkingPlan::default();
    }
    if let Some(m) = plan.max_tokens {
        w.max_tokens = Some(m);
    }
    let foreign_thinking = plan
        .thinking
        .as_ref()
        .is_some_and(|t| t.known.ty == "enabled" || t.known.ty == "adaptive")
        && reasoning.is_some_and(|c| !c.extras.is_from(D));
    if foreign_thinking {
        w.temperature = None;
        w.top_p = None;
        w.top_k = None;
    }
    let (thinking, effort) = (plan.thinking, plan.effort);
    let format = match output {
        OutputSpec::JsonSchema(spec) => Some(to_raw(&own_ext(
            JsonFormatW {
                ty: "json_schema".into(),
                schema: spec.schema.clone().into_inner(),
            },
            &spec.extras,
        ))?),
        _ => None,
    };
    if effort.is_some() || format.is_some() || present("output_config") {
        w.output_config = Some(nested_ext(
            OutputConfigW { effort, format },
            &r.extras,
            "output_config",
        ));
    }
    w.thinking = thinking;

    w.tool_choice = match &r.tool_choice {
        Some(ToolChoice::Unknown(item)) => {
            Some(serde_json::from_str::<Ext<ToolChoiceW>>(item.raw.get()).map_err(internal)?)
        }
        Some(choice) => {
            let (ty, name) = match choice {
                ToolChoice::Auto => ("auto", None),
                ToolChoice::Required => ("any", None),
                ToolChoice::None => ("none", None),
                ToolChoice::Tool(n) => ("tool", Some(enc.name(n))),
                _ => ("auto", None),
            };
            let disable = match choice {
                ToolChoice::None if !own => None,
                _ => r.parallel_tool_calls.map(|p| !p),
            };
            Some(nested_ext(
                ToolChoiceW {
                    ty: ty.to_owned(),
                    name,
                    disable_parallel_tool_use: disable,
                },
                &r.extras,
                "tool_choice",
            ))
        }
        None if r.parallel_tool_calls == Some(false) => Some(Ext::new(ToolChoiceW {
            ty: "auto".into(),
            name: None,
            disable_parallel_tool_use: Some(true),
        })),
        None => None,
    };

    for t in &r.tools {
        match t {
            ToolDef::Function(f) => {
                let w_tool = ToolW {
                    name: enc.name(&f.name),
                    description: f.description.clone(),
                    input_schema: f
                        .parameters
                        .as_ref()
                        .map_or_else(empty_parameters, |p| p.clone().into_inner()),
                    cache_control: f.cache.as_ref().map(super::blocks::encode_cache),
                    strict: f.strict,
                    ..ToolW::default()
                };
                w.tools.push(to_raw(&own_ext(w_tool, &f.extras))?);
            }
            ToolDef::Unknown(item) => w.tools.push(item.raw.clone().into_inner()),
            _ => {
                return Err(KppError::unsupported_by(
                    "tool definition",
                    D,
                    "This version of Kaeso cannot write this kind of tool as an Anthropic tool.",
                ));
            }
        }
    }

    let body = to_body(&own_ext(w, &r.extras))?;
    ctx.tool_ids.extend(enc.rewrites.ids);
    ctx.tool_names.extend(enc.rewrites.names);
    Ok(EncodedRequest::post_json(
        path,
        request_headers(&r.extras),
        body,
    ))
}

#[derive(Default)]
struct ThinkingPlan {
    thinking: Option<Ext<ThinkingConfigW>>,
    effort: Option<String>,
    max_tokens: Option<u32>,
}

fn thinking_plan(
    c: &ReasoningConfig,
    max_tokens: Option<u32>,
    ctx: &EgressCtx,
    is_count: bool,
) -> Result<ThinkingPlan, KppError> {
    let own = c.extras.is_from(D);
    let effort = c
        .effort
        .as_ref()
        .filter(|e| own || **e != ReasoningEffort::None);
    let unset = c.mode.is_none() && c.budget_tokens.is_none();
    let default_depth =
        !own && unset && c.effort.is_none() && c.summary.is_some() && ctx.reasoning == Some(true);
    let display = |thinking: &mut Ext<ThinkingConfigW>| -> Result<(), KppError> {
        if !own && !is_count && c.summary.is_some() {
            let value = RawJson::from_serialize("summarized").map_err(internal)?;
            thinking.extra.insert("display".to_owned(), value);
        }
        Ok(())
    };
    let budget = if default_depth {
        Some(reasoning_map::MEDIUM_BUDGET)
    } else {
        effort.and_then(reasoning_map::budget_for_effort)
    };
    if !own
        && ctx.adaptive_thinking == Some(false)
        && unset
        && let Some(budget) = budget
    {
        let (budget, raised) = reasoning_map::budget_within(budget, max_tokens);
        let mut thinking = own_ext(
            ThinkingConfigW {
                ty: "enabled".to_owned(),
                budget_tokens: Some(budget),
            },
            &c.extras,
        );
        display(&mut thinking)?;
        return Ok(ThinkingPlan {
            thinking: Some(thinking),
            effort: None,
            max_tokens: raised.filter(|_| !is_count),
        });
    }
    let ty = match &c.mode {
        Some(ReasoningMode::Disabled) => Some("disabled".to_owned()),
        Some(ReasoningMode::Adaptive) => Some("adaptive".to_owned()),
        Some(ReasoningMode::Other(m)) => Some(m.clone()),
        Some(_) => Some("enabled".to_owned()),
        None if c.budget_tokens.is_some() => Some("enabled".to_owned()),
        None if !own
            && matches!(
                effort,
                Some(
                    ReasoningEffort::Low
                        | ReasoningEffort::Medium
                        | ReasoningEffort::High
                        | ReasoningEffort::XHigh
                        | ReasoningEffort::Max
                )
            ) =>
        {
            Some("adaptive".to_owned())
        }
        None if default_depth => Some("adaptive".to_owned()),
        None => None,
    };
    let thinking = match ty {
        Some(ty) => {
            let on = ty == "enabled" || ty == "adaptive";
            let mut thinking = own_ext(
                ThinkingConfigW {
                    ty,
                    budget_tokens: c.budget_tokens,
                },
                &c.extras,
            );
            if on {
                display(&mut thinking)?;
            }
            Some(thinking)
        }
        None => None,
    };
    Ok(ThinkingPlan {
        thinking,
        effort: effort.map(|e| e.as_wire().to_owned()),
        max_tokens: None,
    })
}

fn tool_turn_lacks_thinking(r: &GenerateRequest, target: &Origin) -> bool {
    let last = r.messages.iter().rev().find(|m| m.role != Role::System);
    let returns_results = last.is_some_and(|m| {
        m.role == Role::User && m.blocks.iter().any(|b| matches!(b, Block::ToolResult(_)))
    });
    let Some(turn) = r.messages.iter().rev().find(|m| m.role == Role::Assistant) else {
        return false;
    };
    if !returns_results || !turn.blocks.iter().any(|b| matches!(b, Block::ToolCall(_))) {
        return false;
    }
    let sendable = |b: &ReasoningBlock| {
        reasoning_replayable(b, target)
            && (b.opaque_of(OpaqueKind::AnthropicRedactedThinking).is_some()
                || b.opaque_of(OpaqueKind::AnthropicThinkingSignature)
                    .is_some_and(|o| !o.data.is_empty()))
    };
    let first = turn
        .blocks
        .iter()
        .find(|b| !matches!(b, Block::Reasoning(rb) if !sendable(rb)));
    !matches!(first, Some(Block::Reasoning(_)))
}

fn request_headers(extras: &Extras) -> HeaderList {
    let mut headers = HeaderList::new().with("content-type", "application/json");
    let mut version = ANTHROPIC_VERSION.to_owned();
    let mut forwarded = Vec::new();
    if extras.is_from(D) {
        for (name, value) in extras.headers() {
            if name == "anthropic-version" {
                if is_newer_version(&value) {
                    version = value;
                }
            } else if is_forwarded_header(D, name) {
                forwarded.push((name.to_owned(), value));
            }
        }
    }
    headers.push("anthropic-version", version);
    for (name, value) in forwarded {
        headers.push(name, value);
    }
    headers
}

pub(crate) fn is_newer_version(value: &str) -> bool {
    let value = value.trim();
    let well_formed = value.len() == 10
        && value.bytes().enumerate().all(|(i, b)| match i {
            4 | 7 => b == b'-',
            _ => b.is_ascii_digit(),
        });
    let field = |range: std::ops::Range<usize>| value.get(range).and_then(|f| f.parse::<u8>().ok());
    let valid_date = matches!(field(5..7), Some(1..=12)) && matches!(field(8..10), Some(1..=31));
    well_formed && valid_date && value > ANTHROPIC_VERSION
}
