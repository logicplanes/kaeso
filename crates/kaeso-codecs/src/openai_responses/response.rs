use kaeso_kpp::{
    Block, CountTokensResponse, Extras, KppError, Origin, RawUsage, Reply, Response, StopReason,
    Usage,
};
use serde_json::Value;
use serde_json::value::RawValue;

use super::DIALECT;
use super::node::{Node, entries_of};
use super::overrides::{Overrides, apply};
use super::parts::{
    TextKind, arguments_text, carried_summaries, decode_function_call, decode_part,
    decode_reasoning, encode_part, function_call_item, generated_id, reasoning_item,
    reasoning_node,
};
use super::request::echo_body;
use super::wire;
use crate::codec::{EgressCtx, EncodeCtx, HeaderList};
use crate::cross::{ReplyPolicy, reasoning_reply_policy};
use crate::wire::{to_body, type_tag};

pub(super) const NATIVE: &str = "native";
const MESSAGES: &str = "messages";
const BLOCKS: &str = "blocks";
const ITEM: &str = "item";

fn item_marker(block: &Block) -> Option<usize> {
    block
        .extras()?
        .form_value(DIALECT, ITEM)?
        .as_u64()
        .and_then(|i| usize::try_from(i).ok())
}

fn mark(blocks: &mut [Block], item: usize) {
    for b in blocks {
        if let Some(e) = super::parts::extras_mut(b)
            && e.is_from(DIALECT)
        {
            e.form.insert(ITEM.to_owned(), item.into());
        }
    }
}

pub(super) fn item_scope(i: usize) -> String {
    format!("@out{i}")
}

pub(super) fn decode_usage(raw: &RawValue, origin: &Origin) -> Option<Usage> {
    let w: wire::UsageWire = serde_json::from_str(raw.get()).ok()?;
    let cached = w
        .input_tokens_details
        .as_ref()
        .and_then(|d| d.cached_tokens)
        .unwrap_or(0);
    let written = w
        .input_tokens_details
        .as_ref()
        .and_then(|d| d.cache_write_tokens)
        .unwrap_or(0);
    let mut usage = Usage::new(
        w.input_tokens
            .saturating_sub(cached)
            .saturating_sub(written),
        w.output_tokens,
    );
    usage.cache_read_tokens = cached;
    usage.cache_write_tokens = written;
    usage.reasoning_tokens = w.output_tokens_details.and_then(|d| d.reasoning_tokens);
    usage.raw = serde_json::from_str::<Value>(raw.get())
        .ok()
        .map(|v| RawUsage::new(origin.clone(), v));
    Some(usage)
}

pub(super) fn usage_node(u: &Usage) -> Node {
    let input = u.total_input_tokens();
    let mut node = Node::obj()
        .with("input_tokens", Node::int(input))
        .with(
            "input_tokens_details",
            Node::obj()
                .with("cached_tokens", Node::int(u.cache_read_tokens))
                .with("cache_write_tokens", Node::int(u.cache_write_tokens)),
        )
        .with("output_tokens", Node::int(u.output_tokens))
        .with(
            "output_tokens_details",
            Node::obj().with(
                "reasoning_tokens",
                Node::int(u.reasoning_tokens.unwrap_or(0)),
            ),
        )
        .with(
            "total_tokens",
            Node::int(input.saturating_add(u.output_tokens)),
        );
    if let Some(Value::Object(raw)) = u.raw_for(DIALECT) {
        for (k, v) in raw {
            node.insert(k, Node::Val(v.clone()));
        }
    }
    node
}

pub(super) fn stop_for(status: Option<&str>, reason: Option<&str>, has_calls: bool) -> StopReason {
    match (status, reason) {
        (Some("incomplete"), Some("max_output_tokens")) => StopReason::MaxTokens,
        (Some("incomplete"), Some("content_filter")) => StopReason::ContentFilter,
        (Some("incomplete"), Some(r)) => StopReason::Other(r.to_owned()),
        (Some("incomplete"), None) => StopReason::Other("incomplete".to_owned()),
        (Some("completed") | None, _) if has_calls => StopReason::ToolUse,
        (Some("completed") | None, _) => StopReason::EndTurn,
        (Some(other), _) => StopReason::Other(other.to_owned()),
    }
}

pub(super) fn status_for(stop: &StopReason) -> (&'static str, Node) {
    let incomplete = |r: &str| ("incomplete", Node::obj().with("reason", Node::str(r)));
    match stop {
        StopReason::MaxTokens => incomplete("max_output_tokens"),
        StopReason::ContentFilter => incomplete("content_filter"),
        _ => ("completed", Node::null()),
    }
}

pub(super) fn message_status(stop: &StopReason) -> &'static str {
    status_for(stop).0
}

pub(super) fn message_item(id: &str, status: &str, content: Vec<Node>) -> Node {
    Node::obj()
        .with("id", Node::str(id))
        .with("type", Node::str("message"))
        .with("status", Node::str(status))
        .with("content", Node::Arr(content))
        .with("role", Node::str("assistant"))
}

pub(super) fn echo_fields(ctx: &EncodeCtx) -> Vec<(String, Node)> {
    let req = ctx.request.as_deref().and_then(echo_body);
    let pick =
        |k: &str, default: Node| -> Node { req.as_ref().and_then(|r| r.get(k)).unwrap_or(default) };
    let mut reasoning = pick("reasoning", Node::obj());
    for k in ["effort", "summary"] {
        if !reasoning.has(k) {
            reasoning.insert(k, Node::null());
        }
    }
    let format = Node::obj().with("format", Node::obj().with("type", Node::str("text")));
    vec![
        ("instructions".into(), pick("instructions", Node::null())),
        (
            "max_output_tokens".into(),
            pick("max_output_tokens", Node::null()),
        ),
        (
            "max_tool_calls".into(),
            pick("max_tool_calls", Node::null()),
        ),
        (
            "parallel_tool_calls".into(),
            pick("parallel_tool_calls", Node::bool(true)),
        ),
        ("previous_response_id".into(), Node::null()),
        (
            "prompt_cache_key".into(),
            pick("prompt_cache_key", Node::null()),
        ),
        ("reasoning".into(), reasoning),
        (
            "safety_identifier".into(),
            pick("safety_identifier", Node::null()),
        ),
        ("service_tier".into(), Node::str("default")),
        ("store".into(), Node::bool(false)),
        ("temperature".into(), pick("temperature", Node::float(1.0))),
        ("text".into(), pick("text", format)),
        ("tool_choice".into(), pick("tool_choice", Node::str("auto"))),
        ("tools".into(), pick("tools", Node::Arr(Vec::new()))),
        ("top_logprobs".into(), pick("top_logprobs", Node::int(0))),
        ("top_p".into(), pick("top_p", Node::float(1.0))),
        (
            "truncation".into(),
            pick("truncation", Node::str("disabled")),
        ),
        ("user".into(), pick("user", Node::null())),
        ("metadata".into(), pick("metadata", Node::obj())),
    ]
}

pub(super) fn response_object(
    id: &str,
    model: &str,
    status: &str,
    incomplete: Node,
    output: Node,
    usage: Node,
    echo: Option<(&[(String, Node)], u64)>,
) -> Node {
    let mut node = Node::obj()
        .with("id", Node::str(id))
        .with("object", Node::str("response"));
    if let Some((_, created_at)) = echo {
        node.insert("created_at", Node::int(created_at));
    }
    node.insert("status", Node::str(status));
    if let Some((_, created_at)) = echo {
        node.insert("background", Node::bool(false));
        let completed = (status == "completed").then(|| Node::int(created_at));
        node.insert("completed_at", completed.unwrap_or_else(Node::null));
    }
    node.insert("error", Node::null());
    node.insert("incomplete_details", incomplete);
    if let Some((fields, _)) = echo {
        for (k, v) in fields {
            node.insert(k, v.clone());
        }
    }
    node.with("model", Node::str(model))
        .with("output", output)
        .with("usage", usage)
}

pub(super) fn failed_error(w: &wire::ResponseObject, raw: Value, origin: &Origin) -> KppError {
    let code = w.error.as_ref().and_then(|e| e.code.clone());
    let message = w
        .error
        .as_ref()
        .and_then(|e| e.message.clone())
        .unwrap_or_else(|| "The provider reported that the response failed.".to_owned());
    let kind = code
        .as_deref()
        .and_then(super::error::kind_for_code)
        .unwrap_or(kaeso_kpp::ErrorKind::ProviderError);
    KppError::new(kind, message)
        .with_origin(origin.clone())
        .with_raw(raw)
}

pub(super) fn decode(
    status: u16,
    headers: &HeaderList,
    body: &[u8],
    ctx: &EgressCtx,
) -> Result<Reply, KppError> {
    if !(200..300).contains(&status) {
        return Err(super::error::decode(DIALECT, status, headers, body));
    }
    let origin = ctx.origin(DIALECT);
    let bad = |e: &dyn std::fmt::Display| {
        KppError::provider(format!(
            "The provider sent a response that is not a valid {} response: {e}.",
            DIALECT.display_name()
        ))
    };
    let raw: &RawValue = serde_json::from_slice(body).map_err(|e| bad(&e))?;
    match ctx.kind {
        kaeso_kpp::RequestKind::CountTokens => {
            let w: wire::TokenCounts = serde_json::from_str(raw.get()).map_err(|e| bad(&e))?;
            let mut out = CountTokensResponse::new(w.input_tokens);
            out.extras = Extras::new(origin);
            Overrides::diff(&count_node(w.input_tokens), raw, &[]).record(&mut out.extras, "");
            Ok(Reply::CountTokens(out))
        }
        kaeso_kpp::RequestKind::Generate => {
            let mut resp = decode_object(raw, &origin).map_err(|e| match e {
                Decoded::Bad(msg) => bad(&msg),
                Decoded::Failed(err) => err,
            })?;
            map_names_back(&mut resp, ctx);
            Ok(Reply::Generate(resp))
        }
        _ => Err(KppError::unsupported_by("embeddings", DIALECT, "")),
    }
}

fn map_names_back(resp: &mut Response, ctx: &EgressCtx) {
    for b in &mut resp.blocks {
        if let Block::ToolCall(c) = b {
            c.id = ctx.original_tool_id(&c.id).to_owned();
            c.name = ctx.original_tool_name(&c.name).to_owned();
        }
    }
}

fn count_node(n: u64) -> Node {
    Node::obj()
        .with("object", Node::str("response.input_tokens"))
        .with("input_tokens", Node::int(n))
}

pub(super) enum Decoded {
    Bad(String),
    Failed(KppError),
}

pub(super) fn decode_object(raw: &RawValue, origin: &Origin) -> Result<Response, Decoded> {
    let w: wire::ResponseObject =
        serde_json::from_str(raw.get()).map_err(|e| Decoded::Bad(e.to_string()))?;
    if w.status.as_deref() == Some("failed") {
        let value = serde_json::from_str(raw.get()).unwrap_or(Value::Null);
        return Err(Decoded::Failed(failed_error(&w, value, origin)));
    }
    let items = w.output.as_deref().unwrap_or_default();
    let has_calls = items
        .iter()
        .any(|i| type_tag(i).ok().flatten().as_deref() == Some("function_call"));
    let stop = stop_for(
        w.status.as_deref(),
        w.incomplete_details
            .as_ref()
            .and_then(|d| d.reason.as_deref()),
        has_calls,
    );
    let mut extras = Extras::new(origin.clone()).with_form(NATIVE, true.into());
    let mut blocks = Vec::new();
    let mut ranges = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let index = u32::try_from(i).unwrap_or(u32::MAX);
        let ty = type_tag(item).ok().flatten();
        let decoded = match ty.as_deref() {
            Some("message") => decode_message(item, origin, &w.id, index, &stop).map(|(bs, ov)| {
                ranges.push(Value::from(vec![blocks.len(), bs.len()]));
                ov.record(&mut extras, &item_scope(i));
                bs
            }),
            Some("function_call") => {
                let id = generated_id("fc", &w.id, index);
                decode_function_call(item, origin, Some((&id, "completed")))
                    .map(|c| vec![Block::ToolCall(c)])
            }
            Some("reasoning") => decode_reasoning(item, origin).map(|r| vec![Block::Reasoning(r)]),
            _ => None,
        };
        let mut decoded = decoded.unwrap_or_else(|| {
            vec![Block::Unknown(kaeso_kpp::RawItem::new(
                origin.clone(),
                kaeso_kpp::RawJson::from(&**item),
            ))]
        });
        mark(&mut decoded, i);
        blocks.extend(decoded);
    }
    if !ranges.is_empty() {
        extras
            .form
            .insert(MESSAGES.to_owned(), Value::Array(ranges));
        extras.form.insert(BLOCKS.to_owned(), blocks.len().into());
    }
    let usage = w
        .usage
        .as_deref()
        .and_then(|u| decode_usage(u, origin))
        .unwrap_or_default();
    let mut resp = Response::new(w.id.clone(), w.model.clone(), blocks);
    resp.stop = stop;
    resp.usage = usage;
    resp.extras = extras;
    let canon = canonical(&resp, None);
    Overrides::diff(&canon, raw, &["output"]).record(&mut resp.extras, "");
    Ok(resp)
}

fn decode_message(
    raw: &RawValue,
    origin: &Origin,
    response_id: &str,
    index: u32,
    stop: &StopReason,
) -> Option<(Vec<Block>, Overrides)> {
    let entries = entries_of(raw)?;
    let role = entries
        .iter()
        .find(|(k, _)| k == "role")
        .map(|(_, v)| v.get());
    if role != Some("\"assistant\"") {
        return None;
    }
    let content = entries
        .iter()
        .find(|(k, _)| k == "content")
        .map(|(_, v)| *v)?;
    let parts: Vec<&RawValue> = serde_json::from_str(content.get()).ok()?;
    let blocks: Vec<Block> = parts
        .iter()
        .map(|p| decode_part(p, TextKind::Output, origin))
        .collect();
    let canon = message_item(
        &generated_id("msg", response_id, index),
        message_status(stop),
        Vec::new(),
    );
    Some((blocks, Overrides::diff(&canon, raw, &["content"])))
}

fn is_message_part(b: &Block) -> bool {
    matches!(b, Block::Text(_) | Block::Refusal(_))
}

pub(super) fn output_items(resp: &Response) -> Vec<Node> {
    let target = Origin::new(DIALECT);
    let unchanged = resp
        .extras
        .form_value(DIALECT, BLOCKS)
        .and_then(Value::as_u64)
        .is_some_and(|n| usize::try_from(n).ok() == Some(resp.blocks.len()));
    let ranges: Vec<(usize, usize)> = resp
        .extras
        .form_value(DIALECT, MESSAGES)
        .and_then(Value::as_array)
        .filter(|_| unchanged)
        .map(|list| {
            list.iter()
                .filter_map(|r| {
                    let at = usize::try_from(r.get(0)?.as_u64()?).ok()?;
                    let n = usize::try_from(r.get(1)?.as_u64()?).ok()?;
                    Some((at, n))
                })
                .collect()
        })
        .unwrap_or_default();
    let status = message_status(&resp.stop);
    let mut items = Vec::new();
    let mut i = 0;
    let mut used = vec![false; ranges.len()];
    loop {
        if let Some(r) = ranges
            .iter()
            .enumerate()
            .position(|(k, (at, _))| *at == i && !used.get(k).copied().unwrap_or(true))
        {
            if let Some(u) = used.get_mut(r) {
                *u = true;
            }
            let n = ranges.get(r).map_or(0, |(_, n)| *n);
            let index = items.len();
            let parts: Vec<Node> = resp
                .blocks
                .iter()
                .skip(i)
                .take(n)
                .filter_map(|b| encode_part(b, TextKind::Output))
                .collect();
            let id = generated_id("msg", &resp.id, u32::try_from(index).unwrap_or(u32::MAX));
            let mut node = message_item(&id, status, parts);
            apply(&resp.extras, DIALECT, &item_scope(index), &[], &mut node);
            items.push(node);
            i += n;
            continue;
        }
        let Some(block) = resp.blocks.get(i) else {
            break;
        };
        let index = item_marker(block).unwrap_or(items.len());
        let index32 = u32::try_from(index).unwrap_or(u32::MAX);
        if is_message_part(block) {
            let marker = item_marker(block);
            let next_range = ranges.iter().map(|(at, _)| *at).filter(|at| *at > i).min();
            let run: Vec<&Block> = resp
                .blocks
                .iter()
                .enumerate()
                .skip(i)
                .take_while(|(k, b)| {
                    is_message_part(b)
                        && item_marker(b) == marker
                        && next_range.is_none_or(|r| *k < r)
                })
                .map(|(_, b)| b)
                .collect();
            i += run.len();
            let parts = run
                .iter()
                .filter_map(|b| encode_part(b, TextKind::Output))
                .collect();
            let mut node = message_item(&generated_id("msg", &resp.id, index32), status, parts);
            if marker.is_some() {
                apply(&resp.extras, DIALECT, &item_scope(index), &[], &mut node);
            }
            items.push(node);
            continue;
        }
        i += 1;
        match block {
            Block::ToolCall(c) => {
                let id = generated_id("fc", &resp.id, index32);
                let mut node = function_call_item(
                    &c.id,
                    &c.name,
                    &arguments_text(c),
                    Some((&id, "completed")),
                );
                apply(&c.extras, DIALECT, "", &[], &mut node);
                items.push(node);
            }
            Block::Reasoning(r) => {
                let id = generated_id("rs", &resp.id, index32);
                match reasoning_reply_policy(r.origin.as_ref(), DIALECT) {
                    ReplyPolicy::Keep => {
                        let mut node = reasoning_node(r, &target, Some(&id));
                        apply(&r.extras, DIALECT, "", &[], &mut node);
                        items.push(node);
                    }
                    ReplyPolicy::Carry => items.push(reasoning_item(
                        Some(&id),
                        &carried_summaries(r),
                        Some(&crate::continuity::wrap_reasoning(r)),
                        &[],
                    )),
                    _ => {}
                }
            }
            Block::Unknown(item) if item.origin.is(DIALECT) => items.push(Node::raw(&item.raw)),
            _ => {}
        }
    }
    items
}

fn canonical(resp: &Response, echo: Option<(&[(String, Node)], u64)>) -> Node {
    let (status, incomplete) = status_for(&resp.stop);
    response_object(
        &resp.id,
        &resp.model,
        status,
        incomplete,
        Node::Arr(output_items(resp)),
        usage_node(&resp.usage),
        echo,
    )
}

pub(super) fn encode(reply: &Reply, ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
    match reply {
        Reply::Generate(resp) => {
            let native = resp.extras.form_flag(DIALECT, NATIVE);
            let echo = (!native).then(|| echo_fields(ctx));
            let created = ctx.created_at.unwrap_or(0);
            let mut node = canonical(resp, echo.as_deref().map(|e| (e, created)));
            apply(&resp.extras, DIALECT, "", &[], &mut node);
            to_body(&node)
        }
        Reply::CountTokens(c) => {
            let mut node = count_node(c.input_tokens);
            apply(&c.extras, DIALECT, "", &[], &mut node);
            to_body(&node)
        }
        Reply::Embed(_) => Err(KppError::unsupported_by(
            "embeddings response",
            DIALECT,
            "Embeddings are answered in the OpenAI Embeddings format.",
        )),
        _ => Err(KppError::unsupported_by("reply kind", DIALECT, "")),
    }
}
