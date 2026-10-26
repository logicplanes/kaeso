use kaeso_kpp::{
    Block, CacheHint, CacheTtl, Extras, FunctionTool, GenerateRequest, JsonSchemaSpec, KppError,
    Message, OpaqueKind, Origin, OutputSpec, RawItem, RawJson, ReasoningBlock, ReasoningConfig,
    ReasoningEffort, ReasoningMode, Request, RequestKind, Role, StreamMode, ToolChoice, ToolDef,
};
use serde_json::Value;

use super::blocks::{
    BlockEncoder, Dec, Side, decode_block, decode_cache, decode_content, empty_parameters, own_ext,
};
use super::check::{check_output, check_reasoning};
use super::common::{D, internal, nested_ext, request_kind_name, to_raw};
use super::wire::{
    JsonFormatW, MessageW, MetadataW, OutputConfigW, Raw, RequestW, ThinkingConfigW, ToolChoiceW,
    ToolW,
};
use super::{ANTHROPIC_VERSION, COUNT_TOKENS_PATH, MESSAGES_PATH};
use crate::codec::{EgressCtx, EncodedRequest, HeaderList, is_forwarded_header};
use crate::cross::{self, reasoning_map, reasoning_replayable};
use crate::wire::without_position;
use crate::wire::{
    Ext, StringOr, add_nested_extra, extras_from, nested_extra_for, to_body, type_tag,
    verbatim_from_form, verbatim_into_form,
};

const OBJECTS_FORM: &str = "objects";
const THINKING_BINDING_BETA: &str = "thinking-binding-controls-2026-08-01";
const SYSTEM_FORM: &str = "system";
const MAX_BREAKPOINTS: usize = 4;

fn invalid_body(detail: impl std::fmt::Display) -> KppError {
    KppError::invalid_request(format!(
        "The request body is not a valid Anthropic Messages request: {detail}."
    ))
}

fn parse_body(body: &[u8]) -> Result<Ext<RequestW>, KppError> {
    let raw: &serde_json::value::RawValue =
        serde_json::from_slice(body).map_err(|_| invalid_body("the body is not valid JSON"))?;
    Ext::from_raw(raw).map_err(|e| {
        match serde_json::from_str::<serde_json::Map<String, Value>>(raw.get()) {
            Ok(object) => describe(&object, &e),
            Err(_) if raw.get().starts_with('{') => invalid_body(without_position(&e)),
            Err(_) => invalid_body("the body must be a JSON object"),
        }
    })
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
    invalid_body(without_position(err))
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
    let dec = Dec::new(&origin, None);
    if kind == RequestKind::Generate && w.max_tokens.is_none() {
        return Err(invalid_body("`max_tokens` is required").with_param("max_tokens"));
    }

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
    crate::wire::note_integers(
        &mut req.extras,
        &[
            ("temperature", w.temperature.as_ref()),
            ("top_p", w.top_p.as_ref()),
        ],
    );
    req.sampling.temperature = w.temperature.as_ref().and_then(serde_json::Number::as_f64);
    req.sampling.top_p = w.top_p.as_ref().and_then(serde_json::Number::as_f64);
    req.sampling.top_k = w.top_k;
    req.sampling.stop = w.stop_sequences;
    if w.stream == Some(true) {
        req.stream = StreamMode::on(true);
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
            reasoning.mode = Some(ReasoningMode::from_wire(&t.known.ty));
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
                    without_position(&e)
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
    let adjusted;
    let r = if !own && cache_hints(r) > 0 {
        adjusted = app_hints_for_anthropic(r);
        &adjusted
    } else {
        r
    };
    let count_only = matches!(req, Request::CountTokens(_)) && !own;
    let mut enc = BlockEncoder::new(ctx.origin(D), Side::Request);
    let mut w = RequestW {
        model: r.model.clone(),
        ..RequestW::default()
    };

    let breakpoint = (matches!(req, Request::Generate(_)) && !own)
        .then(|| cross_format_breakpoint(r))
        .flatten();
    let marked_system;
    let system = match &breakpoint {
        Some(hint) => {
            marked_system = with_last_breakpoint(&r.system, hint);
            &marked_system
        }
        None => &r.system,
    };
    let last_cacheable_message = breakpoint.as_ref().and_then(|_| {
        r.messages
            .iter()
            .rposition(|m| m.blocks.iter().any(takes_cache_control))
    });
    let system_form = r.extras.form_str(D, SYSTEM_FORM);
    w.system = if system_form == Some("string")
        && let [kaeso_kpp::Block::Text(t)] = system.as_slice()
        && t.cache.is_none()
        && t.citations.is_empty()
        && t.extras.is_empty()
    {
        Some(StringOr::String(t.text.clone()))
    } else {
        let list = enc.blocks(system)?;
        (!list.is_empty() || system_form == Some("array")).then_some(StringOr::List(list))
    };

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
    let rebuilt = rebuilt_thinking(r, &mut plan, reasoning, ctx, forced, own || is_count)?;
    for (index, m) in r.messages.iter().enumerate() {
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
        let kept: Vec<Block>;
        let blocks = if rebuilt == Rebuilt::Dropped {
            kept = m
                .blocks
                .iter()
                .filter(|b| !matches!(b, Block::Reasoning(rb) if replays_as_thinking(rb, &ctx.origin(D))))
                .cloned()
                .collect();
            &kept
        } else {
            &m.blocks
        };
        let marked_message;
        let blocks = match &breakpoint {
            Some(hint) if last_cacheable_message == Some(index) => {
                marked_message = with_last_breakpoint(blocks, hint);
                &marked_message
            }
            _ => blocks,
        };
        let Some(content) = enc.content(blocks, &m.extras, "")? else {
            continue;
        };
        if matches!(&content, StringOr::List(l) if l.is_empty()) && !blocks.is_empty() {
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
        w.temperature =
            crate::wire::spelled_number(r.sampling.temperature, "temperature", &r.extras, D);
        w.top_p = crate::wire::spelled_number(r.sampling.top_p, "top_p", &r.extras, D);
        w.top_k = r.sampling.top_k;
        w.stop_sequences = r.sampling.stop.clone();
        if !r.stream.is_off() {
            w.stream = Some(true);
        }
    }
    if let Some(m) = plan.max_tokens {
        w.max_tokens = Some(m);
    }
    let thinking_on = plan
        .thinking
        .as_ref()
        .is_some_and(|t| t.known.ty == "enabled" || t.known.ty == "adaptive");
    if thinking_on && (rebuilt == Rebuilt::Bound || reasoning.is_some_and(|c| !c.extras.is_from(D)))
    {
        w.temperature = None;
        w.top_p = None;
        w.top_k = None;
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

    let last_function = r
        .tools
        .iter()
        .rposition(|t| matches!(t, ToolDef::Function(_)));
    for (i, t) in r.tools.iter().enumerate() {
        match t {
            ToolDef::Function(f) => {
                let cache = f
                    .cache
                    .as_ref()
                    .or(breakpoint.as_ref().filter(|_| last_function == Some(i)));
                let w_tool = ToolW {
                    name: enc.name(&f.name),
                    description: f.description.clone(),
                    input_schema: f
                        .parameters
                        .as_ref()
                        .map_or_else(empty_parameters, |p| p.clone().into_inner()),
                    cache_control: cache.map(super::blocks::encode_cache),
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
    let betas: &[&str] = match rebuilt {
        Rebuilt::Bound => &[THINKING_BINDING_BETA],
        _ => &[],
    };
    Ok(EncodedRequest::post_json(
        path,
        request_headers(&r.extras, betas),
        body,
    ))
}

fn cross_format_breakpoint(r: &GenerateRequest) -> Option<CacheHint> {
    if cache_hints(r) > 0 || cross::openai_cache_is_explicit(&r.extras) {
        return None;
    }
    let ttl = cross::openai_cache_outlives_five_minutes(&r.extras).then_some(CacheTtl::OneHour);
    Some(CacheHint::new(ttl))
}

fn cache_slot(b: &mut Block) -> Option<&mut Option<CacheHint>> {
    match b {
        Block::Text(t) if !t.text.is_empty() => Some(&mut t.cache),
        Block::Image(i) => Some(&mut i.cache),
        Block::Document(d) => Some(&mut d.cache),
        Block::ToolCall(c) => Some(&mut c.cache),
        Block::ToolResult(t) => Some(&mut t.cache),
        _ => None,
    }
}

fn takes_cache_control(b: &Block) -> bool {
    match b {
        Block::Text(t) => !t.text.is_empty(),
        Block::Image(_) | Block::Document(_) | Block::ToolCall(_) | Block::ToolResult(_) => true,
        _ => false,
    }
}

fn each_block_hint(blocks: &mut [Block], visit: &mut impl FnMut(&mut Option<CacheHint>)) {
    for b in blocks {
        if let Block::ToolResult(t) = b {
            each_block_hint(&mut t.content, visit);
        }
        if let Some(slot) = cache_slot(b) {
            visit(slot);
        }
    }
}

fn each_hint(r: &mut GenerateRequest, visit: &mut impl FnMut(&mut Option<CacheHint>)) {
    for t in &mut r.tools {
        if let ToolDef::Function(f) = t {
            visit(&mut f.cache);
        }
    }
    each_block_hint(&mut r.system, visit);
    for m in &mut r.messages {
        each_block_hint(&mut m.blocks, visit);
    }
}

fn cache_hints(r: &GenerateRequest) -> usize {
    fn count(blocks: &[Block]) -> usize {
        blocks
            .iter()
            .map(|b| {
                usize::from(b.cache().is_some())
                    + match b {
                        Block::ToolResult(t) => count(&t.content),
                        _ => 0,
                    }
            })
            .sum()
    }
    let tools = r
        .tools
        .iter()
        .filter(|t| matches!(t, ToolDef::Function(f) if f.cache.is_some()))
        .count();
    tools + count(&r.system) + r.messages.iter().map(|m| count(&m.blocks)).sum::<usize>()
}

fn clear_hints_on_empty_text(blocks: &mut [Block]) {
    for b in blocks {
        match b {
            Block::Text(t) if t.text.is_empty() => t.cache = None,
            Block::ToolResult(t) => clear_hints_on_empty_text(&mut t.content),
            _ => {}
        }
    }
}

fn app_hints_for_anthropic(r: &GenerateRequest) -> GenerateRequest {
    let mut r = r.clone();
    clear_hints_on_empty_text(&mut r.system);
    for m in &mut r.messages {
        clear_hints_on_empty_text(&mut m.blocks);
    }
    let mut surplus = cache_hints(&r).saturating_sub(MAX_BREAKPOINTS);
    let long = cross::openai_cache_outlives_five_minutes(&r.extras);
    each_hint(&mut r, &mut |slot| {
        if slot.is_none() {
            return;
        }
        if surplus > 0 {
            *slot = None;
            surplus -= 1;
        } else if let Some(hint) = slot
            && long
            && hint.ttl.is_none()
        {
            hint.ttl = Some(CacheTtl::OneHour);
        }
    });
    r
}

fn with_last_breakpoint(blocks: &[Block], hint: &CacheHint) -> Vec<Block> {
    let mut marked = blocks.to_vec();
    if let Some(slot) = marked.iter_mut().rev().find_map(cache_slot) {
        *slot = Some(hint.clone());
    }
    marked
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
        Some(mode) => Some(mode.as_wire().to_owned()),
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

fn continued_tool_turn(r: &GenerateRequest) -> Option<&Message> {
    let last = r.messages.iter().rev().find(|m| m.role != Role::System)?;
    let returns_results = last.role == Role::User
        && last
            .blocks
            .iter()
            .any(|b| matches!(b, Block::ToolResult(_)));
    let turn = r
        .messages
        .iter()
        .rev()
        .find(|m| m.role == Role::Assistant)?;
    (returns_results && turn.blocks.iter().any(|b| matches!(b, Block::ToolCall(_)))).then_some(turn)
}

fn tool_turn_lacks_thinking(r: &GenerateRequest, target: &Origin) -> bool {
    let Some(turn) = continued_tool_turn(r) else {
        return false;
    };
    let first = turn
        .blocks
        .iter()
        .find(|b| !matches!(b, Block::Reasoning(rb) if !replays_as_thinking(rb, target)));
    !matches!(first, Some(Block::Reasoning(_)))
}

fn replays_as_thinking(b: &ReasoningBlock, target: &Origin) -> bool {
    reasoning_replayable(b, target)
        && (b.opaque_of(OpaqueKind::AnthropicRedactedThinking).is_some()
            || b.opaque_of(OpaqueKind::AnthropicThinkingSignature)
                .is_some_and(|o| !o.data.is_empty()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Rebuilt {
    None,
    Bound,
    Dropped,
}

fn rebuilt_thinking(
    r: &GenerateRequest,
    plan: &mut ThinkingPlan,
    reasoning: Option<&ReasoningConfig>,
    ctx: &EgressCtx,
    forced: bool,
    own_or_count: bool,
) -> Result<Rebuilt, KppError> {
    let target = ctx.origin(D);
    let replays = r
        .messages
        .iter()
        .flat_map(|m| &m.blocks)
        .any(|b| matches!(b, Block::Reasoning(rb) if replays_as_thinking(rb, &target)));
    if own_or_count
        || !replays
        || ctx.thinking_binding != Some(true)
        || ctx.adaptive_thinking == Some(false)
    {
        return Ok(Rebuilt::None);
    }
    let off = reasoning.is_some_and(|c| {
        c.effort == Some(ReasoningEffort::None) || c.mode == Some(ReasoningMode::Disabled)
    });
    let thinking = match &mut plan.thinking {
        Some(t) if t.known.ty == "adaptive" => t,
        None if !forced && !off => plan.thinking.insert(Ext::new(ThinkingConfigW {
            ty: "adaptive".to_owned(),
            budget_tokens: None,
        })),
        Some(t) if t.known.ty == "enabled" && continued_tool_turn(r).is_some() => {
            *plan = ThinkingPlan::default();
            return Ok(Rebuilt::Dropped);
        }
        _ => return Ok(Rebuilt::Dropped),
    };
    let binding = RawJson::from_serialize(&serde_json::json!({
        "prefix_mismatch_behavior": "drop_block"
    }))
    .map_err(internal)?;
    thinking.extra.insert("block_binding".to_owned(), binding);
    Ok(Rebuilt::Bound)
}

fn request_headers(extras: &Extras, betas: &[&str]) -> HeaderList {
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
    let mut added = betas.join(",");
    for (name, value) in &mut forwarded {
        if name == "anthropic-beta" && !added.is_empty() {
            value.push(',');
            value.push_str(&std::mem::take(&mut added));
        }
    }
    if !added.is_empty() {
        forwarded.push(("anthropic-beta".to_owned(), added));
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
