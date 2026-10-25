use kaeso_kpp::{
    Base64Source, Block, CacheHint, Citation, DocumentBlock, Extras, ImageBlock, Opaque,
    OpaqueKind, Origin, RawItem, RawJson, ReasoningBlock, RefusalBlock, Source, TextBlock,
    ToolCallBlock, ToolResultBlock,
};
use serde_json::value::RawValue;

use super::DIALECT;
use super::node::Node;
use super::overrides::Overrides;
use super::wire;
use crate::cross::{ReplyPolicy, opaque_replayable, reply_policy};
use crate::wire::{StringOr, type_tag};

pub(super) const BREAKPOINT: &str = "breakpoint";
pub(super) const ARGS_STRING: &str = "args_string";
pub(super) const PARTS: &str = "parts";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TextKind {
    Input,
    AssistantInput,
    Output,
}

pub(super) fn parse_data_url(url: &str) -> Option<(String, String)> {
    let parsed = crate::wire::parse_data_url(url).filter(|d| d.header.is_none())?;
    Some((parsed.media_type.to_owned(), parsed.data.to_owned()))
}

pub(super) fn data_url(source: &Base64Source) -> String {
    format!("data:{};base64,{}", source.media_type, source.data)
}

fn default_filename(media_type: &str) -> String {
    let ext = match media_type {
        "application/pdf" => "pdf",
        "text/plain" => "txt",
        "text/markdown" => "md",
        "text/csv" => "csv",
        "application/json" => "json",
        _ => "bin",
    };
    format!("document.{ext}")
}

pub(super) fn is_plain_text(block: &Block) -> Option<&str> {
    match block {
        Block::Text(t)
            if t.citations.is_empty()
                && !is_breakpoint(t.cache.as_ref())
                && (t.extras.is_empty() || !t.extras.is_from(DIALECT)) =>
        {
            Some(&t.text)
        }
        _ => None,
    }
}

fn is_breakpoint(cache: Option<&CacheHint>) -> bool {
    cache.is_some_and(|c| c.extras.form_flag(DIALECT, BREAKPOINT))
}

fn breakpoint_node(cache: Option<&CacheHint>) -> Option<Node> {
    is_breakpoint(cache).then(|| Node::obj().with("mode", Node::str("explicit")))
}

fn breakpoint_from(value: Option<&serde_json::Value>, origin: &Origin) -> Option<CacheHint> {
    match value {
        Some(serde_json::Value::Object(_)) => {
            let mut hint = CacheHint::new(None);
            hint.extras = Extras::new(origin.clone()).with_form(BREAKPOINT, true.into());
            Some(hint)
        }
        _ => None,
    }
}

fn annotations(text: &TextBlock) -> Node {
    Node::Arr(
        text.citations
            .iter()
            .filter(|c| reply_policy(Some(&c.origin), DIALECT) == ReplyPolicy::Keep)
            .map(|c| Node::raw(&c.raw))
            .collect(),
    )
}

pub(super) fn text_part(t: &TextBlock, kind: TextKind) -> Node {
    let node = match kind {
        TextKind::Input => Node::obj()
            .with("type", Node::str("input_text"))
            .with("text", Node::str(&t.text)),
        TextKind::AssistantInput => Node::obj()
            .with("type", Node::str("output_text"))
            .with("text", Node::str(&t.text))
            .with("annotations", annotations(t)),
        TextKind::Output => Node::obj()
            .with("type", Node::str("output_text"))
            .with("annotations", annotations(t))
            .with("logprobs", Node::Arr(Vec::new()))
            .with("text", Node::str(&t.text)),
    };
    node.with_opt("prompt_cache_breakpoint", breakpoint_node(t.cache.as_ref()))
}

pub(super) fn refusal_part(text: &str) -> Node {
    Node::obj()
        .with("type", Node::str("refusal"))
        .with("refusal", Node::str(text))
}

fn image_part(b: &ImageBlock) -> Option<Node> {
    let node = Node::obj().with("type", Node::str("input_image"));
    let node = match &b.source {
        Source::Url(u) => node.with("image_url", Node::str(u)),
        Source::Base64(s) => node.with("image_url", Node::str(data_url(s))),
        Source::FileId(f) => node.with("file_id", Node::str(&f.id)),
        _ => return None,
    };
    Some(node.with_opt("prompt_cache_breakpoint", breakpoint_node(b.cache.as_ref())))
}

fn file_part(b: &DocumentBlock) -> Option<Node> {
    let node = Node::obj().with("type", Node::str("input_file"));
    let node = match &b.source {
        Source::Url(u) => node.with("file_url", Node::str(u)),
        Source::Base64(s) => node
            .with("filename", Node::str(default_filename(&s.media_type)))
            .with("file_data", Node::str(data_url(s))),
        Source::FileId(f) => node.with("file_id", Node::str(&f.id)),
        _ => return None,
    };
    Some(node.with_opt("prompt_cache_breakpoint", breakpoint_node(b.cache.as_ref())))
}

pub(super) fn canonical_part(block: &Block, kind: TextKind) -> Option<Node> {
    match block {
        Block::Text(t) => Some(text_part(t, kind)),
        Block::Refusal(r) => Some(refusal_part(&r.text)),
        Block::Image(b) => image_part(b),
        Block::Document(b) => file_part(b),
        Block::Unknown(item) if item.origin.is(DIALECT) => Some(Node::raw(&item.raw)),
        _ => None,
    }
}

pub(super) fn encode_part(block: &Block, kind: TextKind) -> Option<Node> {
    let mut node = canonical_part(block, kind)?;
    if let Some(extras) = block.extras() {
        super::overrides::apply(extras, DIALECT, "", &[], &mut node);
    }
    Some(node)
}

pub(super) fn is_content(block: &Block) -> bool {
    matches!(
        block,
        Block::Text(_)
            | Block::Refusal(_)
            | Block::Image(_)
            | Block::Document(_)
            | Block::Unknown(_)
    )
}

fn unknown(raw: &RawValue, origin: &Origin) -> Block {
    Block::Unknown(RawItem::new(origin.clone(), RawJson::from(raw)))
}

pub(super) fn decode_part(raw: &RawValue, kind: TextKind, origin: &Origin) -> Block {
    let tag = type_tag(raw).ok().flatten();
    let typed = match tag.as_deref() {
        Some("input_text" | "output_text") => serde_json::from_str::<wire::TextPart>(raw.get())
            .ok()
            .map(|p| {
                let mut t = TextBlock::new(p.text);
                t.citations = p
                    .annotations
                    .unwrap_or_default()
                    .iter()
                    .map(|a| Citation::new(origin.clone(), RawJson::from(&**a)))
                    .collect();
                t.cache = breakpoint_from(p.prompt_cache_breakpoint.as_ref(), origin);
                Block::Text(t)
            }),
        Some("refusal") => serde_json::from_str::<wire::RefusalPart>(raw.get())
            .ok()
            .map(|p| Block::Refusal(RefusalBlock::new(p.refusal))),
        Some("input_image") => serde_json::from_str::<wire::ImagePart>(raw.get())
            .ok()
            .and_then(|p| {
                let source = match (p.image_url, p.file_id) {
                    (Some(url), None) => match parse_data_url(&url) {
                        Some((mt, data)) => Source::base64(mt, data),
                        None => Source::Url(url),
                    },
                    (None, Some(id)) => Source::file_id(origin.clone(), id),
                    _ => return None,
                };
                let mut b = ImageBlock::new(source);
                b.cache = breakpoint_from(p.prompt_cache_breakpoint.as_ref(), origin);
                Some(Block::Image(b))
            }),
        Some("input_file") => serde_json::from_str::<wire::FilePart>(raw.get())
            .ok()
            .and_then(|p| {
                let source = match (p.file_data, p.file_url, p.file_id) {
                    (Some(data), None, None) => {
                        let (mt, data) = parse_data_url(&data)?;
                        Source::base64(mt, data)
                    }
                    (None, Some(url), None) => Source::Url(url),
                    (None, None, Some(id)) => Source::file_id(origin.clone(), id),
                    _ => return None,
                };
                let mut b = DocumentBlock::new(source);
                b.cache = breakpoint_from(p.prompt_cache_breakpoint.as_ref(), origin);
                Some(Block::Document(b))
            }),
        _ => None,
    };
    let Some(mut block) = typed else {
        return unknown(raw, origin);
    };
    set_extras(&mut block, Extras::new(origin.clone()));
    if let Some(canon) = canonical_part(&block, kind) {
        let overrides = Overrides::diff(&canon, raw, &[]);
        if let Some(extras) = extras_mut(&mut block) {
            overrides.record(extras, "");
        }
    }
    block
}

pub(super) fn extras_mut(block: &mut Block) -> Option<&mut Extras> {
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

fn set_extras(block: &mut Block, extras: Extras) {
    if let Some(e) = extras_mut(block) {
        *e = extras;
    }
}

pub(super) fn decode_parts(
    content: &StringOr<Box<RawValue>>,
    kind: TextKind,
    origin: &Origin,
) -> Vec<Block> {
    match content {
        StringOr::String(s) => vec![Block::Text(TextBlock::new(s.clone()))],
        StringOr::List(parts) => parts.iter().map(|p| decode_part(p, kind, origin)).collect(),
    }
}

pub(super) fn single_string(blocks: &[&Block]) -> Option<String> {
    match blocks {
        [b] => is_plain_text(b).map(str::to_owned),
        _ => None,
    }
}

pub(super) fn content_node(blocks: &[&Block], kind: TextKind, owner: &Extras) -> Node {
    if !owner.form_flag(DIALECT, PARTS)
        && let Some(s) = single_string(blocks)
    {
        return Node::str(s);
    }
    Node::Arr(blocks.iter().filter_map(|b| encode_part(b, kind)).collect())
}

pub(super) fn arguments_from(text: String, extras: &mut Extras) -> RawJson {
    if serde_json::from_str::<&RawValue>(&text).is_ok()
        && let Ok(raw) = RawJson::from_string(text.clone())
    {
        return raw;
    }
    extras.form.insert(ARGS_STRING.to_owned(), true.into());
    RawJson::from_serialize(&text).unwrap_or_else(|_| RawJson::null())
}

pub(super) fn arguments_text(call: &ToolCallBlock) -> String {
    let wrapped = call.extras.form_flag(DIALECT, ARGS_STRING)
        || (!call.extras.is_from(DIALECT) && call.arguments.get().trim_start().starts_with('"'));
    if wrapped && let Ok(s) = call.arguments.parse::<String>() {
        return s;
    }
    call.arguments.get().to_owned()
}

pub(super) fn function_call_item(
    call_id: &str,
    name: &str,
    arguments: &str,
    id_status: Option<(&str, &str)>,
) -> Node {
    let mut node = Node::obj();
    if let Some((id, _)) = id_status {
        node.insert("id", Node::str(id));
    }
    node.insert("type", Node::str("function_call"));
    if let Some((_, status)) = id_status {
        node.insert("status", Node::str(status));
    }
    node.with("arguments", Node::str(arguments))
        .with("call_id", Node::str(call_id))
        .with("name", Node::str(name))
}

pub(super) fn decode_function_call(
    raw: &RawValue,
    origin: &Origin,
    id_status: Option<(&str, &str)>,
) -> Option<ToolCallBlock> {
    let w: wire::FunctionCallItem = serde_json::from_str(raw.get()).ok()?;
    let mut extras = Extras::new(origin.clone());
    let args = arguments_from(w.arguments.clone(), &mut extras);
    let canon = function_call_item(&w.call_id, &w.name, &w.arguments, id_status);
    Overrides::diff(&canon, raw, &[]).record(&mut extras, "");
    strip_kaeso_id(&mut extras);
    let mut call = ToolCallBlock::new(w.call_id, w.name, args);
    call.extras = extras;
    Some(call)
}

pub(super) fn function_call_output_item(call_id: &str, result: &ToolResultBlock) -> Node {
    let blocks: Vec<&Block> = result.content.iter().collect();
    let output = if blocks.is_empty() && !result.extras.form_flag(DIALECT, PARTS) {
        Node::str("")
    } else {
        content_node(&blocks, TextKind::Input, &result.extras)
    };
    Node::obj()
        .with("type", Node::str("function_call_output"))
        .with("call_id", Node::str(call_id))
        .with("output", output)
}

pub(super) fn decode_function_call_output(
    raw: &RawValue,
    origin: &Origin,
) -> Option<ToolResultBlock> {
    let w: wire::FunctionCallOutputItem = serde_json::from_str(raw.get()).ok()?;
    let content = decode_parts(&w.output, TextKind::Input, origin);
    let mut result = ToolResultBlock::new(w.call_id, content);
    result.extras = Extras::new(origin.clone());
    if matches!(w.output, StringOr::List(_)) {
        let blocks: Vec<&Block> = result.content.iter().collect();
        if blocks.is_empty() || single_string(&blocks).is_some() {
            result.extras.form.insert(PARTS.to_owned(), true.into());
        }
    }
    let canon = function_call_output_item(&result.call_id, &result);
    Overrides::diff(&canon, raw, &["output"]).record(&mut result.extras, "");
    Some(result)
}

pub(super) fn reasoning_item(
    id: Option<&str>,
    summary: &[String],
    encrypted: Option<&str>,
    text: &[String],
) -> Node {
    let part = |ty: &str, t: &str| {
        Node::obj()
            .with("type", Node::str(ty))
            .with("text", Node::str(t))
    };
    let mut node = Node::obj();
    if let Some(id) = id {
        node.insert("id", Node::str(id));
    }
    node.insert("type", Node::str("reasoning"));
    node.insert(
        "summary",
        Node::Arr(summary.iter().map(|s| part("summary_text", s)).collect()),
    );
    if let Some(enc) = encrypted {
        node.insert("encrypted_content", Node::str(enc));
    }
    if !text.is_empty() {
        node.insert(
            "content",
            Node::Arr(text.iter().map(|s| part("reasoning_text", s)).collect()),
        );
    }
    node
}

pub(super) const KAESO_ID: &str = "kaeso_id";

pub(super) fn reasoning_sendable(block: &ReasoningBlock) -> bool {
    !block.extras.form_flag(DIALECT, KAESO_ID)
        || block
            .opaque_of(OpaqueKind::OpenAiEncryptedReasoning)
            .is_some()
}

// OpenAI's `encrypted_content` is a Fernet token: the version byte 0x80, then a big-endian
// timestamp, so it starts with `gAAAAA` (https://github.com/fernet/spec/blob/master/Spec.md).
pub(super) fn has_openai_encryption(block: &ReasoningBlock) -> bool {
    block
        .opaque
        .iter()
        .any(|o| o.kind == OpaqueKind::OpenAiEncryptedReasoning && o.data.starts_with("gAAAAA"))
}

pub(super) fn is_kaeso_id(id: &str) -> bool {
    ["msg_kaeso_", "fc_kaeso_", "rs_kaeso_"]
        .iter()
        .any(|p| id.starts_with(p))
}

pub(super) fn strip_kaeso_id(extras: &mut Extras) {
    if let Some(serde_json::Value::Object(set)) = extras.form.get_mut("set")
        && set
            .get("id")
            .and_then(serde_json::Value::as_str)
            .is_some_and(is_kaeso_id)
    {
        set.remove("id");
    }
}

fn opaque_for<'a>(block: &'a ReasoningBlock, kind: OpaqueKind, target: &Origin) -> Option<&'a str> {
    block
        .opaque
        .iter()
        .find(|o| o.kind == kind && opaque_replayable(o, target))
        .map(|o| o.data.as_str())
}

pub(super) fn reasoning_node(
    block: &ReasoningBlock,
    target: &Origin,
    generated_id: Option<&str>,
) -> Node {
    let id = opaque_for(block, OpaqueKind::OpenAiReasoningItemId, target).or(generated_id);
    let enc = opaque_for(block, OpaqueKind::OpenAiEncryptedReasoning, target);
    reasoning_item(id, &block.summary, enc, &block.text)
}

pub(super) fn carried_summaries(block: &ReasoningBlock) -> Vec<String> {
    let text = block.text.concat();
    (!text.is_empty())
        .then_some(text)
        .into_iter()
        .chain(block.summary.iter().cloned())
        .collect()
}

pub(super) fn decode_reasoning(raw: &RawValue, origin: &Origin) -> Option<ReasoningBlock> {
    let w: wire::ReasoningItem = serde_json::from_str(raw.get()).ok()?;
    if let Some(block) = w
        .encrypted_content
        .as_deref()
        .and_then(crate::continuity::unwrap_reasoning)
    {
        return Some(block);
    }
    let mut block = ReasoningBlock::new(origin.clone());
    block.summary = w
        .summary
        .unwrap_or_default()
        .into_iter()
        .map(|p| p.text)
        .collect();
    block.text = w
        .content
        .unwrap_or_default()
        .into_iter()
        .map(|p| p.text)
        .collect();
    if w.id.as_deref().is_some_and(is_kaeso_id) {
        block.extras.form.insert(KAESO_ID.to_owned(), true.into());
    }
    if let Some(id) = w.id.as_ref().filter(|id| !is_kaeso_id(id)) {
        block.opaque.push(Opaque::new(
            origin.clone(),
            OpaqueKind::OpenAiReasoningItemId,
            id,
        ));
    }
    if let Some(enc) = &w.encrypted_content {
        block.opaque.push(Opaque::new(
            origin.clone(),
            OpaqueKind::OpenAiEncryptedReasoning,
            enc,
        ));
    }
    let kaeso = std::mem::take(&mut block.extras.form);
    block.extras = Extras::new(origin.clone());
    block.extras.form = kaeso;
    let canon = reasoning_item(
        w.id.as_deref(),
        &block.summary,
        w.encrypted_content.as_deref(),
        &block.text,
    );
    Overrides::diff(&canon, raw, &[]).record(&mut block.extras, "");
    Some(block)
}

pub(super) fn web_search_call_item(id: &str, status: &str, query: Option<&str>) -> Node {
    let node = Node::obj()
        .with("id", Node::str(id))
        .with("type", Node::str("web_search_call"))
        .with("status", Node::str(status));
    match query {
        Some(q) => node.with(
            "action",
            Node::obj()
                .with("type", Node::str("search"))
                .with("query", Node::str(q)),
        ),
        None => node,
    }
}

pub(super) fn generated_id(prefix: &str, response_id: &str, output_index: u32) -> String {
    let clean: String = response_id
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect();
    format!("{prefix}_kaeso_{clean}_{output_index}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_urls() {
        assert_eq!(
            parse_data_url("data:image/png;base64,AAAA"),
            Some(("image/png".into(), "AAAA".into()))
        );
        assert_eq!(parse_data_url("data:image/png;charset=x;base64,AA"), None);
        assert_eq!(parse_data_url("https://x"), None);
        assert_eq!(parse_data_url("data:text/plain,hi"), None);
        assert_eq!(default_filename("application/pdf"), "document.pdf");
        assert_eq!(generated_id("msg", "msg_01AB-c", 2), "msg_kaeso_msg01ABc_2");
        assert!(is_kaeso_id("fc_kaeso_x_1"));
        assert!(!is_kaeso_id("fc_0d9d96706c3569bb"));
    }
}
