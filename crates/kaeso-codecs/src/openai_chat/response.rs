use indexmap::IndexMap;
use kaeso_kpp::{
    Block, Citation, Extras, KppError, Origin, RawItem, RawJson, RawUsage, ReasoningBlock, Reply,
    RequestKind, Response, StopReason, TextBlock, Usage,
};
use serde_json::Value;

use super::DIALECT;
use super::convert::{
    add_nested, decode_tool_call, encode_tool_call, ext_nested, ext_with, extras_of,
    is_tool_call_item, parse_ext, to_raw,
};
use super::profile::ReplyReasoning;
use super::wire::{
    ChatResponse, Choice, CompletionTokensDetails, PromptTokensDetails, ResponseMessage, UsageWire,
};
use crate::codec::{EgressCtx, EncodeCtx};
use crate::cross::{ReplyPolicy, reply_policy};
use crate::wire::{Ext, StringOr, to_body};

pub(crate) const FORM_OBJECT: &str = "object";
pub(crate) const FORM_CONTENT_PARTS: &str = "content_parts";
pub(crate) const FORM_THINKING: &str = "thinking";
pub(crate) const FORM_REASONING_FIELD: &str = "reasoning_field";
pub(crate) const CHOICE: &str = "choice";
pub(crate) const MESSAGE: &str = "choice.message";

pub(crate) fn decode_response(body: &[u8], ctx: &EgressCtx) -> Result<Reply, KppError> {
    if ctx.kind != RequestKind::Generate {
        return Err(KppError::unsupported_by(
            super::kind_feature(ctx.kind),
            DIALECT,
            "Chat Completions only answers generation requests.",
        ));
    }
    let origin = ctx.origin(DIALECT);
    let wire: Ext<ChatResponse> = Ext::from_slice(body).map_err(|e| {
        KppError::provider(format!(
            "The provider sent a response that is not a valid OpenAI Chat Completions response: {}.",
            crate::wire::without_position(&e)
        ))
    })?;
    let Ext {
        known,
        extra,
        verbatim,
        ..
    } = wire;
    let mut extras = extras_of(&origin, extra, verbatim);
    match known.object.as_deref() {
        Some("chat.completion") => {}
        Some(other) => {
            extras
                .form
                .insert(FORM_OBJECT.to_owned(), Value::String(other.to_owned()));
        }
        None => {
            extras.form.insert(FORM_OBJECT.to_owned(), Value::Null);
        }
    }
    let raw_choice = match known.choices.as_slice() {
        [one] => one,
        [] => {
            return Err(KppError::provider(
                "The provider sent an OpenAI Chat Completions response without any choice.",
            ));
        }
        _ => {
            return Err(KppError::unsupported_by(
                "n",
                DIALECT,
                "The provider returned several choices, but Kaeso carries exactly one; do not request `n` > 1.",
            ));
        }
    };
    let choice: Ext<Choice> = parse_ext(raw_choice.as_raw(), "choice", true)?;
    let Ext {
        known: choice,
        extra: c_extra,
        verbatim: c_verbatim,
        ..
    } = choice;
    if choice.index != 0 {
        extras
            .fields
            .insert(format!("{CHOICE}.index"), to_raw(&choice.index)?);
    }
    add_nested(&mut extras, CHOICE, c_extra, c_verbatim);
    let Ext {
        known: message,
        extra: mut m_extra,
        verbatim: m_verbatim,
        ..
    } = choice.message;
    let reasoning = take_reasoning_field(ctx.chat_profile.reply.reasoning_fields, &mut m_extra);
    add_nested(&mut extras, MESSAGE, m_extra, m_verbatim);

    let name_back = |n: &str| ctx.original_tool_name(n).to_owned();
    let mut blocks = Vec::new();
    if let Some((field, text)) = reasoning {
        let mut block = ReasoningBlock::new(origin.clone());
        block.text = vec![text];
        block.extras = Extras::new(origin.clone());
        block
            .extras
            .form
            .insert(FORM_REASONING_FIELD.to_owned(), Value::from(field));
        blocks.push(Block::Reasoning(block));
    }
    let citations: Vec<Citation> = message
        .annotations
        .iter()
        .map(|a| Citation::new(origin.clone(), a.clone()))
        .collect();
    match message.content {
        Some(StringOr::String(text)) => {
            let mut block = TextBlock::new(text);
            block.citations = citations;
            block.extras = Extras::new(origin.clone());
            blocks.push(Block::Text(block));
        }
        Some(StringOr::List(parts)) => {
            extras
                .form
                .insert(FORM_CONTENT_PARTS.to_owned(), Value::Bool(true));
            let thinking = ctx.chat_profile.reply.reasoning == ReplyReasoning::ThinkingParts;
            for part in parts {
                if let Some(text) = plain_text_part(&part) {
                    let mut block = TextBlock::new(text);
                    block.extras = Extras::new(origin.clone());
                    blocks.push(Block::Text(block));
                } else if let Some(thought) = thinking.then(|| thinking_part(&part)).flatten() {
                    let mut block = ReasoningBlock::new(origin.clone());
                    block.text = thought.text;
                    block.extras = Extras::new(origin.clone());
                    block
                        .extras
                        .form
                        .insert(FORM_THINKING.to_owned(), Value::Object(thought.rest));
                    blocks.push(Block::Reasoning(block));
                } else {
                    blocks.push(Block::Unknown(RawItem::new(origin.clone(), part)));
                }
            }
            if !citations.is_empty() {
                extras.fields.insert(
                    format!("{MESSAGE}.annotations"),
                    to_raw(&message.annotations)?,
                );
            }
        }
        None if !citations.is_empty() => {
            extras.fields.insert(
                format!("{MESSAGE}.annotations"),
                to_raw(&message.annotations)?,
            );
        }
        None => {}
    }
    if let Some(refusal) = message.refusal {
        let mut block = kaeso_kpp::RefusalBlock::new(refusal);
        block.extras = Extras::new(origin.clone());
        blocks.push(Block::Refusal(block));
    }
    for (i, call) in message.tool_calls.iter().enumerate() {
        let generated = super::stream::generated_call_id(&known.id, i);
        blocks.push(decode_tool_call(
            call,
            &origin,
            true,
            &name_back,
            Some(generated),
        )?);
    }
    let Some(finish) = choice.finish_reason else {
        return Err(KppError::provider(
            "The provider sent an OpenAI Chat Completions response without a `finish_reason`.",
        ));
    };
    let mut response = Response::new(known.id, known.model, blocks);
    response.stop = decode_finish(&finish);
    if let Some(usage) = known.usage {
        response.usage = decode_usage(&usage, &origin)?;
    }
    response.extras = extras;
    Ok(Reply::Generate(response))
}

pub(crate) fn encode_response(reply: &Reply, ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
    let Reply::Generate(resp) = reply else {
        return Err(KppError::unsupported_by(
            super::kind_feature(reply.kind()),
            DIALECT,
            "A Chat Completions app can only receive chat completions.",
        ));
    };
    let own = resp.extras.is_from(DIALECT);
    let as_parts = own && resp.extras.form_flag(DIALECT, FORM_CONTENT_PARTS);
    let mut parts: Vec<RawJson> = Vec::new();
    let mut text: Option<String> = None;
    let mut citations = Vec::new();
    let mut refusal: Option<String> = None;
    let mut tool_calls = Vec::new();
    let mut reasoning: IndexMap<String, String> = IndexMap::new();
    for block in &resp.blocks {
        match block {
            Block::Reasoning(r) if r.extras.is_from(DIALECT) && reasoning_field(r).is_some() => {
                if let Some(field) = reasoning_field(r) {
                    reasoning
                        .entry(field.to_owned())
                        .or_default()
                        .push_str(&r.text.concat());
                }
            }
            Block::Text(t) => {
                if as_parts {
                    parts.push(to_raw(
                        &serde_json::json!({"type": "text", "text": t.text}),
                    )?);
                } else {
                    text.get_or_insert_with(String::new).push_str(&t.text);
                }
                citations.extend(
                    t.citations
                        .iter()
                        .filter(|c| c.origin.is(DIALECT))
                        .map(|c| c.raw.clone()),
                );
            }
            Block::Refusal(r) => refusal.get_or_insert_with(String::new).push_str(&r.text),
            Block::ToolCall(call) => tool_calls.push(encode_tool_call(call, &call.name)?),
            Block::Unknown(item)
                if reply_policy(Some(&item.origin), DIALECT) == ReplyPolicy::Keep
                    && is_tool_call_item(item) =>
            {
                tool_calls.push(item.raw.clone());
            }
            Block::Unknown(item) if as_parts && item.origin.is(DIALECT) => {
                parts.push(item.raw.clone());
            }
            Block::Reasoning(r) if as_parts && r.extras.is_from(DIALECT) => {
                if let Some(Value::Object(rest)) = r.extras.form_value(DIALECT, FORM_THINKING) {
                    parts.push(encode_thinking_part(&r.text, rest)?);
                }
            }
            Block::Reasoning(_)
            | Block::Unknown(_)
            | Block::Image(_)
            | Block::Document(_)
            | Block::ToolResult(_) => {}
            _ => {}
        }
    }
    let content = if as_parts {
        Some(StringOr::List(parts))
    } else {
        text.map(StringOr::String)
    };
    let message = ResponseMessage {
        role: "assistant".to_owned(),
        content,
        refusal,
        tool_calls,
        annotations: citations,
    };
    let mut message = ext_nested(message, &resp.extras, MESSAGE);
    for (field, text) in reasoning {
        let text = to_raw(&text)?;
        message.extra.entry(field).or_insert(text);
    }
    let mut choice = ext_nested(
        Choice {
            index: 0,
            message: Ext::new(ResponseMessage::default()),
            finish_reason: Some(encode_finish(&resp.stop, own).to_owned()),
        },
        &resp.extras,
        CHOICE,
    );
    let mut wire = ext_with(
        ChatResponse {
            id: resp.id.clone(),
            object: Some("chat.completion".to_owned()),
            model: resp.model.clone(),
            choices: Vec::new(),
            usage: Some(to_raw(&encode_usage(&resp.usage))?),
        },
        &resp.extras,
    );
    if own {
        if let Some(index) = choice.extra.shift_remove("index") {
            choice.known.index = index.parse().unwrap_or(0);
        }
        match resp.extras.form_value(DIALECT, FORM_OBJECT) {
            Some(Value::String(object)) => wire.known.object = Some(object.clone()),
            Some(_) => wire.known.object = None,
            None => {}
        }
    } else {
        let null = kaeso_kpp::RawJson::null;
        if message.known.content.is_none() {
            message.verbatim.insert("content".to_owned(), null());
        }
        if message.known.refusal.is_none() {
            message.verbatim.insert("refusal".to_owned(), null());
        }
        if message.known.annotations.is_empty() {
            message.verbatim.insert(
                "annotations".to_owned(),
                kaeso_kpp::RawJson::from_string("[]".to_owned())
                    .unwrap_or_else(|_| kaeso_kpp::RawJson::null()),
            );
        }
        choice.verbatim.insert("logprobs".to_owned(), null());
    }
    if !wire.extra.contains_key("created") {
        wire.extra
            .insert("created".to_owned(), to_raw(&ctx.created_at.unwrap_or(0))?);
    }
    choice.known.message = message;
    wire.known.choices = vec![to_raw(&choice)?];
    if own && resp.usage == Usage::default() && resp.usage.raw.is_none() {
        wire.known.usage = None;
    }
    to_body(&wire)
}

pub(crate) fn decode_finish(finish: &str) -> StopReason {
    match finish {
        "stop" => StopReason::EndTurn,
        "length" => StopReason::MaxTokens,
        "tool_calls" => StopReason::ToolUse,
        "content_filter" => StopReason::ContentFilter,
        other => StopReason::Other(other.to_owned()),
    }
}

const FINISH_REASONS: &[&str] = &[
    "stop",
    "length",
    "tool_calls",
    "content_filter",
    "function_call",
];

pub(crate) fn encode_finish(stop: &StopReason, own: bool) -> &str {
    match stop {
        StopReason::MaxTokens => "length",
        StopReason::ToolUse => "tool_calls",
        StopReason::ContentFilter | StopReason::Refusal => "content_filter",
        StopReason::Other(other) if own || FINISH_REASONS.contains(&other.as_str()) => other,
        StopReason::Other(other) => match other.as_str() {
            "max_tokens" | "max_output_tokens" | "model_context_window_exceeded" => "length",
            "tool_use" => "tool_calls",
            "refusal" => "content_filter",
            _ => "stop",
        },
        _ => "stop",
    }
}

pub(crate) fn decode_usage(raw: &kaeso_kpp::RawJson, origin: &Origin) -> Result<Usage, KppError> {
    let wire: UsageWire = raw.parse().map_err(|e| {
        KppError::provider(format!(
            "The provider sent an OpenAI Chat Completions usage object that Kaeso cannot read: {}.",
            crate::wire::without_position(&e)
        ))
    })?;
    let value: Value = raw.to_value().unwrap_or(Value::Null);
    let mut usage = usage_from_wire(&wire);
    usage.raw = Some(RawUsage::new(origin.clone(), value));
    Ok(usage)
}

fn usage_from_wire(wire: &UsageWire) -> Usage {
    let details = wire.prompt_tokens_details.as_ref();
    let cached = details
        .and_then(|d| d.cached_tokens)
        .unwrap_or(0)
        .min(wire.prompt_tokens);
    let written = details
        .and_then(|d| d.cache_write_tokens)
        .unwrap_or(0)
        .min(wire.prompt_tokens.saturating_sub(cached));
    let reasoning = wire
        .completion_tokens_details
        .as_ref()
        .and_then(|d| d.reasoning_tokens);
    let mut usage = Usage::new(
        wire.prompt_tokens
            .saturating_sub(cached)
            .saturating_sub(written),
        output_tokens(wire, reasoning.unwrap_or(0)),
    );
    usage.cache_read_tokens = cached;
    usage.cache_write_tokens = written;
    usage.reasoning_tokens = reasoning;
    usage
}

fn output_tokens(wire: &UsageWire, reasoning: u64) -> u64 {
    let counted = wire.prompt_tokens.saturating_add(wire.completion_tokens);
    let beside = counted.saturating_add(reasoning);
    if reasoning > 0 && wire.total_tokens == beside && wire.total_tokens != counted {
        wire.completion_tokens.saturating_add(reasoning)
    } else {
        wire.completion_tokens
    }
}

pub(crate) fn encode_usage(usage: &Usage) -> Value {
    let prompt = usage.total_input_tokens();
    let mut details = PromptTokensDetails {
        cached_tokens: Some(usage.cache_read_tokens),
        cache_write_tokens: None,
    };
    if usage.cache_write_tokens > 0 {
        details.cache_write_tokens = Some(usage.cache_write_tokens);
    }
    let wire = UsageWire {
        prompt_tokens: prompt,
        completion_tokens: usage.output_tokens,
        total_tokens: prompt.saturating_add(usage.output_tokens),
        prompt_tokens_details: Some(details),
        completion_tokens_details: usage.reasoning_tokens.map(|r| CompletionTokensDetails {
            reasoning_tokens: Some(r),
        }),
    };
    let typed = serde_json::to_value(&wire).unwrap_or(Value::Null);
    let Some(raw) = usage.raw_for(DIALECT) else {
        return typed;
    };
    if let Ok(raw_wire) = serde_json::from_value::<UsageWire>(raw.clone()) {
        let mut same = usage_from_wire(&raw_wire);
        same.raw = usage.raw.clone();
        if same == *usage {
            return raw.clone();
        }
    }
    let mut merged = raw.clone();
    overlay(&mut merged, typed);
    merged
}

fn overlay(base: &mut Value, top: Value) {
    match (base, top) {
        (Value::Object(b), Value::Object(t)) => {
            for (k, v) in t {
                match b.get_mut(&k) {
                    Some(slot) => overlay(slot, v),
                    None => {
                        b.insert(k, v);
                    }
                }
            }
        }
        (slot, top) => *slot = top,
    }
}

pub(crate) fn plain_text_part(part: &RawJson) -> Option<String> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct TextOnly {
        #[serde(rename = "type")]
        ty: String,
        text: String,
    }
    serde_json::from_str::<TextOnly>(part.get())
        .ok()
        .filter(|p| p.ty == "text")
        .map(|p| p.text)
}

pub(crate) fn take_reasoning_field(
    fields: &[&'static str],
    extra: &mut IndexMap<String, RawJson>,
) -> Option<(&'static str, String)> {
    fields.iter().find_map(|field| {
        let text = extra
            .get(*field)?
            .parse::<String>()
            .ok()
            .filter(|t| !t.is_empty())?;
        extra.shift_remove(*field);
        Some((*field, text))
    })
}

fn reasoning_field(block: &ReasoningBlock) -> Option<&str> {
    block.extras.form_str(DIALECT, FORM_REASONING_FIELD)
}

pub(crate) struct Thought {
    pub(crate) text: Vec<String>,
    pub(crate) rest: serde_json::Map<String, Value>,
}

pub(crate) fn thinking_part(part: &RawJson) -> Option<Thought> {
    let Ok(Value::Object(mut fields)) = part.to_value() else {
        return None;
    };
    if fields.get("type").and_then(Value::as_str) != Some("thinking") {
        return None;
    }
    if fields
        .keys()
        .any(|k| !matches!(k.as_str(), "type" | "thinking" | "closed"))
    {
        return None;
    }
    if fields.get("closed").is_some_and(|c| !c.is_boolean()) {
        return None;
    }
    let Some(Value::Array(inner)) = fields.remove("thinking") else {
        return None;
    };
    let text = inner
        .iter()
        .map(|item| plain_text_part(&RawJson::from_serialize(item).ok()?))
        .collect::<Option<Vec<String>>>()?;
    fields.remove("type");
    Some(Thought { text, rest: fields })
}

pub(crate) fn encode_thinking_part(
    text: &[String],
    rest: &serde_json::Map<String, Value>,
) -> Result<RawJson, KppError> {
    let mut part = serde_json::Map::new();
    part.insert("type".to_owned(), Value::from("thinking"));
    part.insert(
        "thinking".to_owned(),
        Value::Array(
            text.iter()
                .map(|t| serde_json::json!({"type": "text", "text": t}))
                .collect(),
        ),
    );
    for (key, value) in rest {
        part.insert(key.clone(), value.clone());
    }
    to_raw(&Value::Object(part))
}
