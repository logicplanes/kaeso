use kaeso_kpp::{
    Block, Extras, FunctionTool, GenerateRequest, JsonSchemaSpec, KppError, Message, Origin,
    OutputSpec, RawItem, RawJson, ReasoningBlock, ReasoningConfig, ReasoningEffort, Request,
    RequestKind, Role, StreamMode, ToolChoice, ToolDef, ToolResultBlock,
};
use serde_json::Value;
use serde_json::value::RawValue;

use super::DIALECT;
use super::convert::{
    PartContext, add_nested, decode_part, decode_tool_call, extras_of, form_str, parse_ext, to_raw,
};
use super::wire::{
    AssistantMessage, ChatRequest, FunctionToolWire, InstructionMessage, NamedToolChoice,
    ResponseFormat, ToolMessage, UserMessage, role_tag,
};
use crate::codec::{HeaderList, is_forwarded_header};
use crate::limits::{Form, Scope};
use crate::wire::{Ext, StringOr, type_tag, verbatim_into_form};

pub(crate) const FORM_MAX_TOKENS: &str = "max_tokens";
pub(crate) const FORM_STOP: &str = "stop";
pub(crate) const FORM_N: &str = "n";
pub(crate) const FORM_STREAM_FALSE: &str = "stream_false";
pub(crate) const FORM_STREAM_OPTIONS: &str = "stream_options";
pub(crate) const FORM_STREAM_OPTIONS_OFF: &str = "stream_options_off";
pub(crate) const FORM_USER: &str = "user";
pub(crate) const FORM_RESPONSE_FORMAT_TEXT: &str = "response_format_text";
pub(crate) const FORM_SCHEMA_ABSENT: &str = "schema_absent";
pub(crate) const FORM_MESSAGE: &str = "message";
pub(crate) const FORM_ROLE: &str = "role";
pub(crate) const FORM_CONTENT: &str = "content";
pub(crate) const FUNCTION_ROLE: &str = "function";
pub(crate) const FORM_REPLAYED: &str = "replayed";
pub(crate) const REPLAYED_REASONING: &[&str] =
    &["reasoning_content", "reasoning", "reasoning_details"];

pub(crate) const KEY_MAX_TOKENS_CONFLICT: &str = "max_tokens (differs from max_completion_tokens)";
pub(crate) const KEY_USER_CONFLICT: &str = "user (sent alongside safety_identifier)";
pub(crate) const CONFLICT_KEYS: &[(&str, &str)] = &[
    (KEY_MAX_TOKENS_CONFLICT, "max_tokens"),
    (KEY_USER_CONFLICT, "user"),
];

pub(crate) const MESSAGE_PREFIX: &str = "message";

pub(crate) fn decode_request(
    kind: RequestKind,
    body: &[u8],
    headers: &HeaderList,
) -> Result<Request, KppError> {
    if kind != RequestKind::Generate {
        return Err(KppError::unsupported_by(
            super::kind_feature(kind),
            DIALECT,
            "OpenAI Chat Completions only has `POST /v1/chat/completions`.",
        ));
    }
    let origin = Origin::new(DIALECT);
    let wire: Ext<ChatRequest> = Ext::from_slice(body).map_err(|e| describe(body, &e))?;
    let mut req = decode_generate(wire, &origin)?;
    for (name, value) in headers.iter() {
        if is_forwarded_header(DIALECT, name) {
            req.extras.insert_header(name, value);
        }
    }
    Ok(Request::Generate(req))
}

fn describe(body: &[u8], err: &serde_json::Error) -> KppError {
    type Check = fn(&Value) -> bool;
    let string: Check = |v| v.is_null() || v.is_string();
    let int: Check = |v| v.is_null() || v.as_u64().is_some_and(|n| u32::try_from(n).is_ok());
    let count: Check = |v| v.is_null() || v.as_u64().is_some();
    let signed: Check = |v| v.is_null() || v.as_i64().is_some();
    let number: Check = |v| v.is_null() || v.is_number();
    let boolean: Check = |v| v.is_null() || v.is_boolean();
    let array: Check = |v| v.is_array();
    let list: Check = |v| v.is_null() || v.is_array();
    let object: Check = |v| v.is_null() || v.is_object();
    let stop: Check = |v| {
        v.is_null() || v.is_string() || v.as_array().is_some_and(|a| a.iter().all(Value::is_string))
    };
    let choice: Check = |v| v.is_null() || v.is_string() || v.is_object();
    let fields: &[(&str, Check, &str)] = &[
        ("model", |v| v.is_string(), "a string"),
        ("messages", array, "an array of messages"),
        ("temperature", number, "a number"),
        ("top_p", number, "a number"),
        ("frequency_penalty", number, "a number"),
        ("presence_penalty", number, "a number"),
        (
            "max_completion_tokens",
            int,
            "an integer from 0 to 4294967295",
        ),
        ("max_tokens", int, "an integer from 0 to 4294967295"),
        ("n", count, "a positive integer"),
        ("seed", signed, "an integer"),
        ("stop", stop, "a string or an array of strings"),
        ("reasoning_effort", string, "a string"),
        ("user", string, "a string"),
        ("safety_identifier", string, "a string"),
        ("parallel_tool_calls", boolean, "true or false"),
        ("stream", boolean, "true or false"),
        ("stream_options", object, "an object"),
        ("tools", list, "an array of tools"),
        ("tool_choice", choice, "a string or an object"),
        ("response_format", object, "an object"),
    ];
    let invalid = |text: String| {
        KppError::invalid_request(format!(
            "The request body is not a valid OpenAI Chat Completions request: {text}."
        ))
    };
    let Ok(Value::Object(map)) = serde_json::from_slice::<Value>(body) else {
        return invalid(crate::wire::without_position(err));
    };
    for (name, check, expected) in fields {
        match map.get(*name) {
            Some(v) if !check(v) => {
                return invalid(format!("`{name}` must be {expected}")).with_param(*name);
            }
            None if matches!(*name, "model" | "messages") => {
                return invalid(format!("`{name}` is required")).with_param(*name);
            }
            _ => {}
        }
    }
    invalid(crate::wire::without_position(err))
}

fn decode_generate(wire: Ext<ChatRequest>, origin: &Origin) -> Result<GenerateRequest, KppError> {
    let Ext {
        known: w,
        extra,
        verbatim,
        ..
    } = wire;
    let mut req = GenerateRequest::new(w.model);
    req.extras = extras_of(origin, extra, verbatim);
    crate::wire::note_integers(
        &mut req.extras,
        &[
            ("temperature", w.temperature.as_ref()),
            ("top_p", w.top_p.as_ref()),
            ("frequency_penalty", w.frequency_penalty.as_ref()),
            ("presence_penalty", w.presence_penalty.as_ref()),
        ],
    );
    let form = &mut req.extras.form;

    match w.n {
        None => {}
        Some(1) => {
            form.insert(FORM_N.to_owned(), Value::from(1));
        }
        Some(0) => {
            return Err(KppError::invalid_request("`n` must be at least 1.").with_param("n"));
        }
        Some(_) => {
            return Err(KppError::unsupported_by(
                "n",
                DIALECT,
                "Kaeso returns exactly one choice per request; send `n` > 1 as separate requests.",
            )
            .with_param("n"));
        }
    }

    let float = |n: &Option<serde_json::Number>| n.as_ref().and_then(serde_json::Number::as_f64);
    let s = &mut req.sampling;
    s.temperature = float(&w.temperature);
    s.top_p = float(&w.top_p);
    s.seed = w.seed;
    s.frequency_penalty = float(&w.frequency_penalty);
    s.presence_penalty = float(&w.presence_penalty);
    match (w.max_completion_tokens, w.max_tokens) {
        (Some(a), None) => s.max_output_tokens = Some(a),
        (None, Some(b)) => {
            s.max_output_tokens = Some(b);
            form.insert(FORM_MAX_TOKENS.to_owned(), form_str("max_tokens"));
        }
        (Some(a), Some(b)) => {
            s.max_output_tokens = Some(a);
            if a == b {
                form.insert(FORM_MAX_TOKENS.to_owned(), form_str("both"));
            } else {
                req.extras
                    .fields
                    .insert(KEY_MAX_TOKENS_CONFLICT.to_owned(), to_raw(&b)?);
            }
        }
        (None, None) => {}
    }
    let form = &mut req.extras.form;
    match w.stop {
        Some(StringOr::String(stop)) => {
            req.sampling.stop = vec![stop];
            form.insert(FORM_STOP.to_owned(), form_str("string"));
        }
        Some(StringOr::List(stops)) => {
            if stops.is_empty() {
                form.insert(FORM_STOP.to_owned(), form_str("array"));
            }
            req.sampling.stop = stops;
        }
        None => {}
    }

    match (w.user, w.safety_identifier) {
        (user, Some(id)) => {
            req.user = Some(id);
            if let Some(user) = user {
                req.extras
                    .fields
                    .insert(KEY_USER_CONFLICT.to_owned(), to_raw(&user)?);
            }
        }
        (Some(user), None) => {
            req.user = Some(user);
            form.insert(FORM_USER.to_owned(), Value::Bool(true));
        }
        (None, None) => {}
    }

    decode_stream(&mut req, w.stream, w.stream_options)?;

    if let Some(effort) = w.reasoning_effort {
        let mut reasoning = ReasoningConfig::default();
        reasoning.effort = Some(ReasoningEffort::from_wire(&effort));
        reasoning.extras = Extras::new(origin.clone());
        req.reasoning = Some(reasoning);
    }
    if let Some(format) = w.response_format {
        decode_response_format(&mut req, &format, origin)?;
    }
    for tool in &w.tools {
        req.tools.push(decode_tool(tool, origin)?);
    }
    req.tool_choice = w
        .tool_choice
        .map(|c| decode_tool_choice(&c, origin))
        .transpose()?;
    req.parallel_tool_calls = w.parallel_tool_calls;

    decode_messages(&mut req, &w.messages, origin)?;
    Ok(req)
}

fn decode_stream(
    req: &mut GenerateRequest,
    stream: Option<bool>,
    options: Option<Ext<super::wire::StreamOptions>>,
) -> Result<(), KppError> {
    if stream == Some(true) {
        let include_usage = match options {
            None => {
                req.extras
                    .form
                    .insert(FORM_STREAM_OPTIONS.to_owned(), form_str("absent"));
                false
            }
            Some(options) => {
                let Ext {
                    known,
                    extra,
                    verbatim,
                    ..
                } = options;
                add_nested(&mut req.extras, "stream_options", extra, verbatim);
                if known.include_usage.is_none() {
                    req.extras
                        .form
                        .insert(FORM_STREAM_OPTIONS.to_owned(), form_str("no_include_usage"));
                }
                known.include_usage.unwrap_or(false)
            }
        };
        req.stream = StreamMode::on(include_usage);
    } else {
        if stream == Some(false) {
            req.extras
                .form
                .insert(FORM_STREAM_FALSE.to_owned(), Value::Bool(true));
        }
        if let Some(options) = options {
            let value = to_raw(&options)?.to_value().unwrap_or(Value::Null);
            req.extras
                .form
                .insert(FORM_STREAM_OPTIONS_OFF.to_owned(), value);
        }
    }
    Ok(())
}

fn decode_response_format(
    req: &mut GenerateRequest,
    raw: &RawJson,
    origin: &Origin,
) -> Result<(), KppError> {
    let ty = type_tag(raw.as_raw()).ok().flatten();
    if !matches!(ty.as_deref(), Some("text" | "json_object" | "json_schema")) {
        req.extras
            .fields
            .insert("response_format".to_owned(), raw.clone());
        return Ok(());
    }
    let format: Ext<ResponseFormat> = parse_ext(raw.as_raw(), "response_format", false)?;
    let Ext {
        known,
        extra,
        verbatim,
        ..
    } = format;
    match known.ty.as_str() {
        "json_schema" => {
            let Some(schema) = known.json_schema else {
                return Err(KppError::invalid_request(
                    "`response_format` of type `json_schema` needs a `json_schema` object.",
                )
                .with_param("response_format"));
            };
            let Ext {
                known: js,
                extra: js_extra,
                verbatim: js_verbatim,
                ..
            } = schema;
            let absent = js.schema.is_none();
            let mut spec = JsonSchemaSpec::new(js.schema.unwrap_or_else(RawJson::empty_object));
            spec.name = js.name;
            spec.description = js.description;
            spec.strict = js.strict;
            spec.extras = extras_of(origin, extra, verbatim);
            add_nested(&mut spec.extras, "json_schema", js_extra, js_verbatim);
            if absent {
                spec.extras
                    .form
                    .insert(FORM_SCHEMA_ABSENT.to_owned(), Value::Bool(true));
            }
            req.output = OutputSpec::JsonSchema(spec);
        }
        other => {
            if let Some(js) = known.json_schema {
                req.extras
                    .fields
                    .insert("response_format.json_schema".to_owned(), to_raw(&js)?);
            }
            add_nested(&mut req.extras, "response_format", extra, verbatim);
            if other == "json_object" {
                req.output = OutputSpec::JsonObject;
            } else {
                req.extras
                    .form
                    .insert(FORM_RESPONSE_FORMAT_TEXT.to_owned(), Value::Bool(true));
            }
        }
    }
    Ok(())
}

fn decode_tool(raw: &RawJson, origin: &Origin) -> Result<ToolDef, KppError> {
    if type_tag(raw.as_raw()).ok().flatten().as_deref() != Some("function") {
        return Ok(ToolDef::Unknown(RawItem::new(origin.clone(), raw.clone())));
    }
    let tool: Ext<FunctionToolWire> = parse_ext(raw.as_raw(), "function tool", false)?;
    let Ext {
        known,
        extra,
        verbatim,
        ..
    } = tool;
    let Ext {
        known: f,
        extra: f_extra,
        verbatim: f_verbatim,
        ..
    } = known.function;
    let mut def = FunctionTool::new(f.name, f.parameters);
    def.description = f.description;
    def.strict = f.strict;
    def.extras = extras_of(origin, extra, verbatim);
    add_nested(&mut def.extras, "function", f_extra, f_verbatim);
    Ok(ToolDef::Function(def))
}

fn decode_tool_choice(raw: &RawJson, origin: &Origin) -> Result<ToolChoice, KppError> {
    let unknown = || ToolChoice::Unknown(RawItem::new(origin.clone(), raw.clone()));
    if let Ok(mode) = raw.parse::<String>() {
        return Ok(match mode.as_str() {
            "auto" => ToolChoice::Auto,
            "none" => ToolChoice::None,
            "required" => ToolChoice::Required,
            _ => unknown(),
        });
    }
    if type_tag(raw.as_raw()).ok().flatten().as_deref() != Some("function") {
        return Ok(unknown());
    }
    let choice: Ext<NamedToolChoice> = parse_ext(raw.as_raw(), "tool_choice", false)?;
    let plain = choice.extra.is_empty()
        && choice.verbatim.is_empty()
        && choice.known.function.extra.is_empty()
        && choice.known.function.verbatim.is_empty();
    Ok(if plain {
        ToolChoice::Tool(choice.known.function.known.name)
    } else {
        unknown()
    })
}

fn decode_messages(
    req: &mut GenerateRequest,
    messages: &[RawJson],
    origin: &Origin,
) -> Result<(), KppError> {
    let mut leading = true;
    for (i, raw) in messages.iter().enumerate() {
        let param = || format!("messages[{i}]");
        let role = role_tag(raw.as_raw())
            .map_err(|e| invalid_message(i, &e.to_string()))?
            .ok_or_else(|| invalid_message(i, "the message has no `role`"))?;
        match role.as_str() {
            "system" | "developer" => {
                let msg: Ext<InstructionMessage> = parse_ext(raw.as_raw(), "system message", false)
                    .map_err(|e| e.with_param(param()))?;
                let blocks =
                    decode_content(msg.known.content.clone(), PartContext::TextOnly, origin)
                        .map_err(|e| with_param(e, param()))?;
                let spelling = content_spelling(&msg.known.content);
                if leading && matches!(blocks.first(), Some(b) if b.extras().is_some()) {
                    push_leading_system(req, msg, blocks, spelling, origin);
                } else {
                    leading = false;
                    let Ext {
                        known,
                        extra,
                        verbatim,
                        ..
                    } = msg;
                    let mut message = Message::new(Role::System, blocks);
                    message.extras = extras_of(origin, extra, verbatim);
                    if known.role == "developer" {
                        message
                            .extras
                            .form
                            .insert(FORM_ROLE.to_owned(), form_str("developer"));
                    }
                    mark_array(&mut message.extras, spelling);
                    req.messages.push(message);
                }
            }
            "user" => {
                leading = false;
                let msg: Ext<UserMessage> = parse_ext(raw.as_raw(), "user message", false)
                    .map_err(|e| e.with_param(param()))?;
                let Ext {
                    known,
                    extra,
                    verbatim,
                    ..
                } = msg;
                let spelling = content_spelling(&known.content);
                let blocks = decode_content(known.content, PartContext::User, origin)
                    .map_err(|e| with_param(e, param()))?;
                let mut message = Message::new(Role::User, blocks);
                message.extras = extras_of(origin, extra, verbatim);
                mark_array(&mut message.extras, spelling);
                req.messages.push(message);
            }
            "assistant" => {
                leading = false;
                let message = decode_assistant(raw, origin).map_err(|e| with_param(e, param()))?;
                req.messages.push(message);
            }
            "tool" => {
                leading = false;
                let block = decode_tool_message(raw, origin).map_err(|e| with_param(e, param()))?;
                let merge = req.messages.last().is_some_and(|m| {
                    m.role == Role::User
                        && !m.blocks.is_empty()
                        && m.blocks.iter().all(|b| matches!(b, Block::ToolResult(_)))
                        && i > 0
                        && previous_is_tool(messages, i)
                });
                match req.messages.last_mut() {
                    Some(last) if merge => last.blocks.push(block),
                    _ => {
                        let mut message = Message::new(Role::User, vec![block]);
                        message.extras = Extras::new(origin.clone());
                        req.messages.push(message);
                    }
                }
            }
            "function" => {
                leading = false;
                let mut message = Message::new(
                    Role::User,
                    vec![Block::Unknown(RawItem::new(origin.clone(), raw.clone()))],
                );
                message.extras =
                    Extras::new(origin.clone()).with_form(FORM_ROLE, form_str(FUNCTION_ROLE));
                req.messages.push(message);
            }
            other => {
                return Err(invalid_message(i, &format!("unknown role `{other}`")));
            }
        }
    }
    Ok(())
}

pub(crate) fn message_form(raw: &RawValue) -> Form {
    let parts = match role_tag(raw).ok().flatten().as_deref() {
        Some("system" | "developer" | "tool") => PartContext::TextOnly,
        Some("user") => PartContext::User,
        Some("assistant") => PartContext::Assistant,
        _ => return Form::Raw,
    };
    Form::Nests(Scope::Chat(parts))
}

fn previous_is_tool(messages: &[RawJson], i: usize) -> bool {
    i.checked_sub(1)
        .and_then(|p| messages.get(p))
        .and_then(|m| role_tag(m.as_raw()).ok().flatten())
        .is_some_and(|r| r == "tool")
}

fn invalid_message(i: usize, why: &str) -> KppError {
    KppError::invalid_request(format!(
        "Message {i} is not a valid OpenAI Chat Completions message: {why}."
    ))
    .with_param(format!("messages[{i}]"))
}

fn with_param(err: KppError, param: String) -> KppError {
    if err.param.is_some() {
        err
    } else {
        err.with_param(param)
    }
}

fn mark_array(extras: &mut Extras, spelling: &str) {
    if spelling == "array" {
        extras
            .form
            .insert(FORM_CONTENT.to_owned(), form_str("array"));
    }
}

fn content_spelling<T>(content: &StringOr<T>) -> &'static str {
    match content {
        StringOr::String(_) => "string",
        StringOr::List(_) => "array",
    }
}

fn decode_content(
    content: StringOr<RawJson>,
    ctx: PartContext,
    origin: &Origin,
) -> Result<Vec<Block>, KppError> {
    match content {
        StringOr::String(text) => Ok(vec![text_block(text, origin)]),
        StringOr::List(parts) => {
            if parts.is_empty() {
                return Err(KppError::invalid_request(
                    "Message `content` must be a string or a non-empty array of content parts.",
                ));
            }
            parts.iter().map(|p| decode_part(p, ctx, origin)).collect()
        }
    }
}

fn text_block(text: String, origin: &Origin) -> Block {
    let mut block = kaeso_kpp::TextBlock::new(text);
    block.extras = Extras::new(origin.clone());
    Block::Text(block)
}

fn push_leading_system(
    req: &mut GenerateRequest,
    msg: Ext<InstructionMessage>,
    mut blocks: Vec<Block>,
    spelling: &str,
    origin: &Origin,
) {
    let Ext {
        known,
        mut extra,
        mut verbatim,
        ..
    } = msg;
    super::convert::nulls_to_verbatim(&mut extra, &mut verbatim);
    if let Some(extras) = blocks.first_mut().and_then(block_extras_mut) {
        if extras.origin.is_none() {
            extras.origin = Some(origin.clone());
        }
        let mut marker = serde_json::Map::new();
        marker.insert("role".to_owned(), form_str(&known.role));
        marker.insert("content".to_owned(), form_str(spelling));
        extras
            .form
            .insert(FORM_MESSAGE.to_owned(), Value::Object(marker));
        crate::wire::add_nested_extra(extras, MESSAGE_PREFIX, extra);
        verbatim_into_form(extras, MESSAGE_PREFIX, verbatim);
    }
    req.system.extend(blocks);
}

pub(crate) fn block_extras_mut(block: &mut Block) -> Option<&mut Extras> {
    match block {
        Block::Text(b) => Some(&mut b.extras),
        Block::Image(b) => Some(&mut b.extras),
        Block::Document(b) => Some(&mut b.extras),
        Block::ToolCall(b) => Some(&mut b.extras),
        Block::ToolResult(b) => Some(&mut b.extras),
        Block::Reasoning(b) => Some(&mut b.extras),
        Block::Refusal(b) => Some(&mut b.extras),
        _ => None,
    }
}

fn decode_assistant(raw: &RawJson, origin: &Origin) -> Result<Message, KppError> {
    let msg: Ext<AssistantMessage> = parse_ext(raw.as_raw(), "assistant message", false)?;
    let Ext {
        known,
        extra,
        verbatim,
        ..
    } = msg;
    let content_absent = known.content.is_none() && !verbatim.contains_key("content");
    let mut extras = extras_of(origin, extra, verbatim);
    let mut blocks = Vec::new();
    if let Some(thought) = replayed_reasoning(&mut extras, origin) {
        blocks.push(Block::Reasoning(thought));
    }
    match known.content {
        Some(StringOr::String(text)) => {
            blocks.push(text_block(text, origin));
        }
        Some(StringOr::List(parts)) => {
            if parts.is_empty() {
                return Err(KppError::invalid_request(
                    "Assistant `content` must be a string, `null` or a non-empty array of content parts.",
                ));
            }
            for part in &parts {
                blocks.push(decode_part(part, PartContext::Assistant, origin)?);
            }
            extras
                .form
                .insert(FORM_CONTENT.to_owned(), form_str("array"));
        }
        None if content_absent => {
            extras
                .form
                .insert(FORM_CONTENT.to_owned(), form_str("absent"));
        }
        None => {}
    }
    if let Some(refusal) = known.refusal {
        let mut block = kaeso_kpp::RefusalBlock::new(refusal);
        block.extras = Extras::new(origin.clone());
        blocks.push(Block::Refusal(block));
    }
    if !known.annotations.is_empty() {
        let citations = known
            .annotations
            .iter()
            .map(|a| kaeso_kpp::Citation::new(origin.clone(), a.clone()));
        match blocks.iter_mut().find_map(|b| match b {
            Block::Text(t) => Some(t),
            _ => None,
        }) {
            Some(text) => text.citations.extend(citations),
            None => {
                extras
                    .fields
                    .insert("annotations".to_owned(), to_raw(&known.annotations)?);
            }
        }
    }
    for call in &known.tool_calls {
        blocks.push(decode_tool_call(
            call,
            origin,
            false,
            &|n| n.to_owned(),
            None,
        )?);
    }
    let mut message = Message::new(Role::Assistant, blocks);
    message.extras = extras;
    Ok(message)
}

fn replayed_reasoning(extras: &mut Extras, origin: &Origin) -> Option<ReasoningBlock> {
    let sent: Vec<(&str, Value)> = REPLAYED_REASONING
        .iter()
        .filter_map(|key| Some((*key, extras.fields.get(*key)?.to_value().ok()?)))
        .filter(|(key, value)| match *key {
            "reasoning_details" => value
                .as_array()
                .is_some_and(|items| items.iter().all(Value::is_object)),
            _ => value.is_string(),
        })
        .collect();
    let (text, summary) = sent.iter().find_map(|(key, value)| readable(key, value))?;
    let mut block = ReasoningBlock::new(origin.clone());
    block.text = text;
    block.summary = summary;
    block.extras = Extras::new(origin.clone());
    let mut fields = serde_json::Map::new();
    for (key, value) in sent {
        extras.fields.shift_remove(key);
        fields.insert(key.to_owned(), value);
    }
    block
        .extras
        .form
        .insert(FORM_REPLAYED.to_owned(), Value::Object(fields));
    Some(block)
}

// OpenRouter's `reasoning_details` items: `reasoning.text` (`text`), `reasoning.summary`
// (`summary`) and `reasoning.encrypted` (`data`, not readable).
fn readable(key: &str, value: &Value) -> Option<(Vec<String>, Vec<String>)> {
    let read = |items: &[Value], ty: &str, field: &str| -> Vec<String> {
        items
            .iter()
            .filter(|item| item.get("type").and_then(Value::as_str) == Some(ty))
            .filter_map(|item| item.get(field)?.as_str().map(str::to_owned))
            .filter(|text| !text.is_empty())
            .collect()
    };
    match (key, value) {
        ("reasoning_details", Value::Array(items)) => {
            let text = read(items, "reasoning.text", "text");
            let summary = read(items, "reasoning.summary", "summary");
            (!text.is_empty() || !summary.is_empty()).then_some((text, summary))
        }
        (_, Value::String(text)) if !text.is_empty() => Some((vec![text.clone()], Vec::new())),
        _ => None,
    }
}

fn decode_tool_message(raw: &RawJson, origin: &Origin) -> Result<Block, KppError> {
    let msg: Ext<ToolMessage> = parse_ext(raw.as_raw(), "tool message", false)?;
    let Ext {
        known,
        extra,
        verbatim,
        ..
    } = msg;
    let spelling = content_spelling(&known.content);
    let content = decode_content(known.content, PartContext::TextOnly, origin)?;
    let mut result = ToolResultBlock::new(known.tool_call_id, content);
    result.extras = extras_of(origin, extra, verbatim);
    mark_array(&mut result.extras, spelling);
    Ok(Block::ToolResult(result))
}
