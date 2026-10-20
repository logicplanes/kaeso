use kaeso_kpp::{
    Block, Citation, Extras, KppError, Origin, RawUsage, Reply, RequestKind, Response, StopReason,
    TextBlock, Usage,
};
use serde_json::Value;

use super::DIALECT;
use super::convert::{
    add_nested, decode_tool_call, encode_tool_call, ext_nested, ext_with, extras_of,
    is_tool_call_item, parse_ext, to_raw,
};
use super::wire::{
    ChatResponse, Choice, CompletionTokensDetails, PromptTokensDetails, ResponseMessage, UsageWire,
};
use crate::codec::{EgressCtx, EncodeCtx};
use crate::cross::{ReplyPolicy, reply_policy};
use crate::wire::{Ext, to_body};

pub(crate) const FORM_OBJECT: &str = "object";
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
        extra: m_extra,
        verbatim: m_verbatim,
        ..
    } = choice.message;
    add_nested(&mut extras, MESSAGE, m_extra, m_verbatim);

    let name_back = |n: &str| ctx.original_tool_name(n).to_owned();
    let mut blocks = Vec::new();
    let citations: Vec<Citation> = message
        .annotations
        .iter()
        .map(|a| Citation::new(origin.clone(), a.clone()))
        .collect();
    match message.content {
        Some(text) => {
            let mut block = TextBlock::new(text);
            block.citations = citations;
            block.extras = Extras::new(origin.clone());
            blocks.push(Block::Text(block));
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
    for call in &message.tool_calls {
        blocks.push(decode_tool_call(call, &origin, true, &name_back)?);
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
    let mut text: Option<String> = None;
    let mut citations = Vec::new();
    let mut refusal: Option<String> = None;
    let mut tool_calls = Vec::new();
    for block in &resp.blocks {
        match block {
            Block::Text(t) => {
                text.get_or_insert_with(String::new).push_str(&t.text);
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
            Block::Reasoning(_)
            | Block::Unknown(_)
            | Block::Image(_)
            | Block::Document(_)
            | Block::ToolResult(_) => {}
            _ => {}
        }
    }
    let message = ResponseMessage {
        role: "assistant".to_owned(),
        content: text,
        refusal,
        tool_calls,
        annotations: citations,
    };
    let mut message = ext_nested(message, &resp.extras, MESSAGE);
    let mut choice = ext_nested(
        Choice {
            index: 0,
            message: Ext::new(ResponseMessage::default()),
            finish_reason: Some(encode_finish(&resp.stop).to_owned()),
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

pub(crate) fn encode_finish(stop: &StopReason) -> &str {
    match stop {
        StopReason::MaxTokens => "length",
        StopReason::ToolUse => "tool_calls",
        StopReason::ContentFilter | StopReason::Refusal => "content_filter",
        StopReason::Other(other) => other,
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
    let cached = details.and_then(|d| d.cached_tokens).unwrap_or(0);
    let written = details.and_then(|d| d.cache_write_tokens).unwrap_or(0);
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
