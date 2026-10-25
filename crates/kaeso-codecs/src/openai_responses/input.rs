use kaeso_kpp::{Block, Extras, Message, OpaqueKind, Origin, RawItem, RawJson, Role, TextBlock};
use serde_json::value::RawValue;
use serde_json::{Value, json};

use super::DIALECT;
use super::node::Node;
use super::overrides::{Overrides, apply};
use super::parts::{
    self, PARTS, TextKind, arguments_text, content_node, decode_function_call,
    decode_function_call_output, decode_parts, decode_reasoning, function_call_item,
    function_call_output_item, is_content, is_plain_text, reasoning_node, single_string,
};
use super::request::Names;
use super::wire;
use crate::cross::reasoning_replayable;
use crate::wire::{StringOr, type_tag};

pub(super) const RAW_ITEM: &str = "raw_item";
const EMPTY_AT: &str = "empty_at";
pub(super) const SYSTEM_ITEMS: &str = "system_items";
pub(super) const INSTRUCTIONS: &str = "instructions";

fn text_kind(role: Role) -> TextKind {
    match role {
        Role::Assistant => TextKind::AssistantInput,
        _ => TextKind::Input,
    }
}

fn wire_role(role: Role) -> &'static str {
    match role {
        Role::User => "user",
        Role::Assistant => "assistant",
        _ => "developer",
    }
}

fn message_item(role: Role, blocks: &[&Block], owner: &Extras) -> Node {
    Node::obj()
        .with("role", Node::str(wire_role(role)))
        .with("content", content_node(blocks, text_kind(role), owner))
}

enum Item {
    Msg {
        role: Role,
        blocks: Vec<Block>,
        extras: Extras,
        hoistable: Option<Value>,
    },
    Call(Block),
    Output(Block),
    Reasoning(Block),
    Raw(Block, Role),
}

fn raw_role(ty: Option<&str>) -> Role {
    match ty {
        Some(t) if t.ends_with("_call") || t == "mcp_list_tools" || t == "mcp_approval_request" => {
            Role::Assistant
        }
        _ => Role::User,
    }
}

fn decode_item(raw: &RawValue, origin: &Origin) -> Item {
    let ty = type_tag(raw).ok().flatten();
    let raw_item = || {
        Item::Raw(
            Block::Unknown(RawItem::new(origin.clone(), RawJson::from(raw))),
            raw_role(ty.as_deref()),
        )
    };
    let has_role = super::node::entries_of(raw).is_some_and(|e| e.iter().any(|(k, _)| k == "role"));
    match ty.as_deref() {
        Some("message") => decode_message(raw, origin).unwrap_or_else(raw_item),
        None if has_role => decode_message(raw, origin).unwrap_or_else(raw_item),
        Some("function_call") => decode_function_call(raw, origin, None)
            .map(|c| Item::Call(Block::ToolCall(c)))
            .unwrap_or_else(raw_item),
        Some("function_call_output") => decode_function_call_output(raw, origin)
            .map(|r| Item::Output(Block::ToolResult(r)))
            .unwrap_or_else(raw_item),
        Some("reasoning") => decode_reasoning(raw, origin)
            .map(|r| Item::Reasoning(Block::Reasoning(r)))
            .unwrap_or_else(raw_item),
        _ => raw_item(),
    }
}

fn decode_message(raw: &RawValue, origin: &Origin) -> Option<Item> {
    let m: wire::MessageItem = serde_json::from_str(raw.get()).ok()?;
    let role = match m.role.as_str() {
        "user" => Role::User,
        "assistant" => Role::Assistant,
        "system" | "developer" => Role::System,
        _ => return None,
    };
    let blocks = decode_parts(&m.content, text_kind(role), origin);
    let mut extras = Extras::new(origin.clone());
    let is_list = matches!(m.content, StringOr::List(_));
    if is_list {
        let refs: Vec<&Block> = blocks.iter().collect();
        if single_string(&refs).is_some() {
            extras.form.insert(PARTS.to_owned(), true.into());
        }
    }
    let refs: Vec<&Block> = blocks.iter().collect();
    let canon = message_item(role, &refs, &extras);
    let overrides = Overrides::diff(&canon, raw, &["content"]);
    let mut item_overrides = Extras::new(origin.clone());
    overrides.record(&mut item_overrides, "");
    let set = match item_overrides.form.get("set") {
        Some(Value::Object(set)) => Some(set),
        _ => None,
    };
    let id = set
        .and_then(|set| set.get("id"))
        .and_then(Value::as_str)
        .filter(|id| !super::parts::is_kaeso_id(id));
    let hoistable = (role == Role::System
        && blocks.iter().all(|b| matches!(b, Block::Text(_)))
        && item_overrides.fields.is_empty()
        && !item_overrides.form.contains_key("unset")
        && match item_overrides.form.get("set") {
            None => true,
            Some(Value::Object(set)) => set.keys().all(|k| k == "type" || k == "role" || k == "id"),
            Some(_) => false,
        })
    .then(|| {
        let mut desc = json!({
            "n": blocks.len(),
            "role": m.role,
            "type": super::node::entries_of(raw).is_some_and(|e| e.iter().any(|(k, _)| k == "type")),
            "parts": is_list,
        });
        if let (Some(id), Value::Object(d)) = (id, &mut desc) {
            d.insert("id".to_owned(), id.into());
        }
        desc
    });
    super::parts::strip_kaeso_id(&mut item_overrides);
    for (k, v) in item_overrides.form {
        extras.form.insert(k, v);
    }
    extras.fields = item_overrides.fields;
    Some(Item::Msg {
        role,
        blocks,
        extras,
        hoistable,
    })
}

pub(super) struct DecodedInput {
    pub(super) system: Vec<Block>,
    pub(super) system_items: Vec<Value>,
    pub(super) messages: Vec<Message>,
}

pub(super) fn decode_items(items: &[Box<RawValue>], origin: &Origin) -> DecodedInput {
    let mut decoded: Vec<Item> = items.iter().map(|r| decode_item(r, origin)).collect();
    let mut out = DecodedInput {
        system: Vec::new(),
        system_items: Vec::new(),
        messages: Vec::new(),
    };
    let leading = decoded
        .iter()
        .take_while(|i| {
            matches!(
                i,
                Item::Msg {
                    hoistable: Some(_),
                    ..
                }
            )
        })
        .count();
    for item in decoded.drain(..leading) {
        if let Item::Msg {
            blocks,
            hoistable: Some(desc),
            ..
        } = item
        {
            out.system.extend(blocks);
            out.system_items.push(desc);
        }
    }

    let mut current: Option<(Message, bool)> = None;
    let flush = |current: &mut Option<(Message, bool)>, out: &mut DecodedInput| {
        if let Some((m, _)) = current.take() {
            out.messages.push(m);
        }
    };
    for item in decoded {
        match item {
            Item::Raw(block, role) => {
                flush(&mut current, &mut out);
                let mut m = Message::new(role, vec![block]);
                m.extras = Extras::new(origin.clone()).with_form(RAW_ITEM, true.into());
                out.messages.push(m);
            }
            Item::Msg {
                role,
                blocks,
                mut extras,
                ..
            } => {
                let joinable = matches!(&current, Some((m, has_msg)) if m.role == role && !has_msg && role != Role::System);
                if !joinable {
                    flush(&mut current, &mut out);
                }
                let (m, has_msg) = current.get_or_insert_with(|| {
                    let mut m = Message::new(role, Vec::new());
                    m.extras = Extras::new(origin.clone());
                    (m, false)
                });
                if blocks.is_empty() {
                    extras
                        .form
                        .insert(EMPTY_AT.to_owned(), m.blocks.len().into());
                }
                m.blocks.extend(blocks);
                for (k, v) in extras.form {
                    m.extras.form.insert(k, v);
                }
                for (k, v) in extras.fields {
                    m.extras.fields.insert(k, v);
                }
                *has_msg = true;
            }
            Item::Call(block) | Item::Reasoning(block) | Item::Output(block) => {
                let role = if matches!(block, Block::ToolResult(_)) {
                    Role::User
                } else {
                    Role::Assistant
                };
                let joinable = matches!(&current, Some((m, _)) if m.role == role);
                if !joinable {
                    flush(&mut current, &mut out);
                }
                let (m, _) = current.get_or_insert_with(|| {
                    let mut m = Message::new(role, Vec::new());
                    m.extras = Extras::new(origin.clone());
                    (m, false)
                });
                m.blocks.push(block);
            }
        }
    }
    flush(&mut current, &mut out);
    out
}

pub(super) fn encode_system(system: &[Block], req_extras: &Extras) -> (Option<String>, Vec<Node>) {
    let markers = req_extras
        .form_value(DIALECT, SYSTEM_ITEMS)
        .and_then(Value::as_array);
    let Some(markers) = markers else {
        if let [only] = system
            && let Some(text) = is_plain_text(only)
        {
            return (Some(text.to_owned()), Vec::new());
        }
        if system.is_empty() {
            return (None, Vec::new());
        }
        let blocks: Vec<&Block> = system.iter().collect();
        let item = Node::obj().with("role", Node::str("developer")).with(
            "content",
            Node::Arr(
                blocks
                    .iter()
                    .filter_map(|b| parts::encode_part(b, TextKind::Input))
                    .collect(),
            ),
        );
        return (None, vec![item]);
    };
    let mut rest = system.iter();
    let instructions = if req_extras.form_flag(DIALECT, INSTRUCTIONS) {
        rest.next().and_then(|b| match b {
            Block::Text(t) => Some(t.text.clone()),
            _ => None,
        })
    } else {
        None
    };
    let mut items = Vec::new();
    for desc in markers {
        let n = desc.get("n").and_then(Value::as_u64).unwrap_or(0);
        let blocks: Vec<&Block> = rest
            .by_ref()
            .take(usize::try_from(n).unwrap_or(0))
            .collect();
        let role = desc
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("developer");
        let mut item = Node::obj();
        if let Some(id) = desc.get("id").and_then(Value::as_str) {
            item.insert("id", Node::str(id));
        }
        if desc.get("type").and_then(Value::as_bool).unwrap_or(false) {
            item.insert("type", Node::str("message"));
        }
        item.insert("role", Node::str(role));
        let as_parts = desc.get("parts").and_then(Value::as_bool).unwrap_or(false);
        let content = match (as_parts, single_string(&blocks)) {
            (false, Some(s)) => Node::str(s),
            _ => Node::Arr(
                blocks
                    .iter()
                    .filter_map(|b| parts::encode_part(b, TextKind::Input))
                    .collect(),
            ),
        };
        items.push(item.with("content", content));
    }
    let leftover: Vec<&Block> = rest.collect();
    if !leftover.is_empty() {
        items.push(
            Node::obj().with("role", Node::str("developer")).with(
                "content",
                Node::Arr(
                    leftover
                        .iter()
                        .filter_map(|b| parts::encode_part(b, TextKind::Input))
                        .collect(),
                ),
            ),
        );
    }
    (instructions, items)
}

pub(super) fn encode_messages(
    messages: &[Message],
    target: &Origin,
    stateless: bool,
    names: &mut Names<'_>,
) -> Vec<Node> {
    let mut items = Vec::new();
    for m in messages {
        if m.extras.form_flag(DIALECT, RAW_ITEM)
            && let [Block::Unknown(item)] = m.blocks.as_slice()
        {
            items.push(Node::raw(&item.raw));
            continue;
        }
        let empty_at = m
            .extras
            .form_value(DIALECT, EMPTY_AT)
            .and_then(Value::as_u64)
            .and_then(|i| usize::try_from(i).ok());
        let mut first_run = true;
        let mut i = 0;
        let emit_message = |blocks: &[&Block], items: &mut Vec<Node>, first: bool| {
            let empty = Extras::default();
            let owner = if first { &m.extras } else { &empty };
            let mut node = message_item(m.role, blocks, owner);
            if first {
                apply(&m.extras, DIALECT, "", &[], &mut node);
            }
            items.push(node);
        };
        while i <= m.blocks.len() {
            if empty_at == Some(i) && first_run {
                emit_message(&[], &mut items, true);
                first_run = false;
            }
            let Some(block) = m.blocks.get(i) else { break };
            if is_content(block) {
                let run: Vec<&Block> = m
                    .blocks
                    .iter()
                    .skip(i)
                    .take_while(|b| is_content(b))
                    .collect();
                i += run.len();
                emit_message(&run, &mut items, first_run);
                first_run = false;
                continue;
            }
            i += 1;
            match block {
                Block::ToolCall(call) => {
                    let mut node = function_call_item(
                        &names.call_id(&call.id),
                        &names.tool_name(&call.name),
                        &arguments_text(call),
                        None,
                    );
                    apply(&call.extras, DIALECT, "", &[], &mut node);
                    items.push(node);
                }
                Block::ToolResult(result) => {
                    let mut node =
                        function_call_output_item(&names.call_id(&result.call_id), result);
                    apply(&result.extras, DIALECT, "", &[], &mut node);
                    items.push(node);
                }
                Block::Reasoning(r)
                    if reasoning_replayable(r, target)
                        && super::parts::reasoning_sendable(r)
                        && (!stateless
                            || r.opaque_of(OpaqueKind::OpenAiEncryptedReasoning).is_some()) =>
                {
                    let mut node = reasoning_node(r, target, None);
                    apply(&r.extras, DIALECT, "", &[], &mut node);
                    items.push(node);
                }
                _ => {}
            }
        }
    }
    items
}

pub(super) fn string_input(messages: &[Message]) -> Option<String> {
    match messages {
        [m] if m.role == Role::User && (m.extras.is_empty() || !m.extras.is_from(DIALECT)) => {
            match m.blocks.as_slice() {
                [b] => is_plain_text(b).map(str::to_owned),
                _ => None,
            }
        }
        _ => None,
    }
}

pub(super) fn string_message(text: String) -> Message {
    Message::new(Role::User, vec![Block::Text(TextBlock::new(text))])
}
