mod decoder;
mod deltas;
mod encoder;

pub(super) use decoder::Decoder;
pub(super) use encoder::Encoder;

use std::collections::BTreeMap;

use kaeso_kpp::{
    BlockHeader, Extras, KppError, OpaqueKind, ReasoningBlock, StopReason, StreamEvent, Usage,
};
use serde_json::Value;

use super::DIALECT;
use super::namespaces::Namespaces;
use super::node::Node;
use super::parts::{generated_id, reasoning_item, web_search_call_item};
use super::response::{
    NATIVE, message_item, message_status, response_object, status_for, usage_node,
};
use crate::cross::{ReplyPolicy, reasoning_reply_policy, reply_opaque_visible};
use crate::web_search;
use crate::wire::NO_ARGUMENTS;

pub(super) const NEW_ITEM: &str = "new_item";
pub(crate) const NEW_PART: &str = "new_part";
pub(super) const PART: &str = "part";
pub(super) const SKIP: &str = "skip";
pub(super) const EVENT_KEY: &str = "~event";
pub(super) const SSE_KEY: &str = "~sse";

pub(super) type Emit<'a> = dyn FnMut(&str, Node, i64) -> Result<Option<Node>, KppError> + 'a;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PartKind {
    Text,
    Refusal,
    Unknown,
}

#[derive(Clone, Debug)]
struct Msg {
    output_index: u32,
    id: String,
    parts: Vec<Option<Node>>,
    open: usize,
}

#[derive(Clone, Debug)]
enum Blk {
    Part {
        kind: PartKind,
        output_index: u32,
        item_id: String,
        content_index: u32,
        text: String,
        annotations: Vec<Node>,
        logprobs: Vec<Node>,
        header: Option<Node>,
    },
    Call {
        output_index: u32,
        id: String,
        call_id: String,
        name: String,
        args: String,
    },
    Reasoning {
        output_index: u32,
        id: String,
        summaries: Vec<String>,
        open_summary: Option<u32>,
        texts: Vec<String>,
        open_text: Option<u32>,
        encrypted: Option<String>,
        carry: Option<Box<ReasoningBlock>>,
    },
    Unknown {
        output_index: u32,
        header: Node,
        last: Option<Node>,
    },
    Search(Search),
}

#[derive(Clone, Debug)]
struct Search {
    output_index: u32,
    id: String,
    tool_use_id: String,
    query: Option<String>,
    input: String,
}

#[derive(Clone, Debug)]
pub(super) struct Engine {
    native: bool,
    started: bool,
    response_id: String,
    model: String,
    start: Option<Node>,
    seq: i64,
    next_output: u32,
    msg: Option<Msg>,
    blocks: BTreeMap<u32, Blk>,
    hidden: Vec<u32>,
    done: Vec<(u32, Node)>,
    stop: Option<StopReason>,
    usage: Option<Usage>,
    echo: Vec<(String, Node)>,
    created_at: u64,
    terminated: bool,
    namespaces: Namespaces,
    searches: Vec<Search>,
}

fn int(n: u32) -> Node {
    Node::int(u64::from(n))
}

fn sc(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_owned()
    } else if name.is_empty() {
        prefix.to_owned()
    } else {
        format!("{prefix}-{}", name.trim_start_matches('@'))
    }
}

fn ev(ty: &str) -> Node {
    Node::obj().with("type", Node::str(ty))
}

impl Engine {
    pub(super) fn new(
        echo: Vec<(String, Node)>,
        created_at: u64,
        namespaces: Namespaces,
    ) -> Engine {
        Engine {
            native: false,
            started: false,
            response_id: String::new(),
            model: String::new(),
            start: None,
            seq: 0,
            next_output: 0,
            msg: None,
            blocks: BTreeMap::new(),
            hidden: Vec::new(),
            done: Vec::new(),
            stop: None,
            usage: None,
            echo,
            created_at,
            terminated: false,
            namespaces,
            searches: Vec::new(),
        }
    }

    pub(super) fn open(&self) -> bool {
        self.started && !self.terminated
    }

    pub(super) fn terminated(&self) -> bool {
        self.terminated
    }

    pub(super) fn started(&self) -> bool {
        self.started
    }

    pub(super) fn message_open(&self) -> bool {
        self.msg.is_some()
    }

    pub(super) fn saw_sequence(&mut self, seq: Option<i64>) {
        self.seq = seq.map_or(self.seq.saturating_add(1), |s| s.saturating_add(1));
    }

    fn emit(&mut self, em: &mut Emit<'_>, scope: &str, node: Node) -> Result<Node, KppError> {
        match em(scope, node.clone(), self.seq)? {
            Some(out) => {
                self.saw_sequence(out.get_i64("sequence_number"));
                Ok(out)
            }
            None => Ok(node),
        }
    }

    fn take_output(&mut self) -> u32 {
        let oi = self.next_output;
        self.next_output = self.next_output.saturating_add(1);
        oi
    }

    pub(super) fn step(&mut self, event: &StreamEvent, em: &mut Emit<'_>) -> Result<(), KppError> {
        if self.terminated && !matches!(event, StreamEvent::Extra(_)) {
            return Err(KppError::provider(
                "The stream continued after its response ended. This is a bug in Kaeso; please report it.",
            ));
        }
        match event {
            StreamEvent::ResponseStart(s) => {
                if self.started {
                    return Err(KppError::provider(
                        "The stream started a second response. This is a bug in Kaeso; please report it.",
                    ));
                }
                self.started = true;
                self.native = s.extras.form_flag(DIALECT, NATIVE);
                self.response_id = s.id.clone();
                self.model = s.model.clone();
                let usage = match (&s.usage, self.native) {
                    (Some(u), true) => usage_node(u),
                    _ => Node::null(),
                };
                let echo = (!self.native).then_some((self.echo.as_slice(), self.created_at));
                let obj = response_object(
                    &s.id,
                    &s.model,
                    "in_progress",
                    Node::null(),
                    Node::Arr(Vec::new()),
                    usage,
                    echo,
                );
                let created = self.emit(em, "", ev("response.created").with("response", obj))?;
                let start = created.get("response").unwrap_or_else(Node::obj);
                self.start = Some(start.clone());
                self.emit(
                    em,
                    "@in_progress",
                    ev("response.in_progress").with("response", start),
                )?;
                Ok(())
            }
            StreamEvent::BlockStart(b) => {
                self.require_start()?;
                let is_part = match &b.block {
                    BlockHeader::Text | BlockHeader::Refusal => true,
                    BlockHeader::Unknown(_) => b.extras.form_flag(DIALECT, PART),
                    _ => false,
                };
                if is_part {
                    return self.start_part(
                        b.index,
                        &b.block,
                        b.extras.form_flag(DIALECT, NEW_ITEM),
                        em,
                    );
                }
                match &b.block {
                    BlockHeader::ToolCall(h) => {
                        self.close_idle_message(em, "@close")?;
                        let oi = self.take_output();
                        let id = generated_id("fc", &self.response_id, oi);
                        let item = self.namespaces.call_item(
                            &h.id,
                            &h.name,
                            "",
                            Some((&id, "in_progress")),
                        );
                        let out = self.emit(
                            em,
                            "",
                            ev("response.output_item.added")
                                .with("output_index", int(oi))
                                .with("item", item),
                        )?;
                        let item = out.get("item");
                        self.blocks.insert(
                            b.index,
                            Blk::Call {
                                output_index: oi,
                                id: item.as_ref().and_then(|i| i.get_str("id")).unwrap_or(id),
                                call_id: h.id.clone(),
                                name: h.name.clone(),
                                args: String::new(),
                            },
                        );
                    }
                    BlockHeader::Reasoning(h)
                        if reasoning_reply_policy(h.origin.as_ref(), DIALECT)
                            == ReplyPolicy::Carry =>
                    {
                        self.close_idle_message(em, "@close")?;
                        let oi = self.take_output();
                        let id = generated_id("rs", &self.response_id, oi);
                        let item = reasoning_item(Some(&id), &[], None, &[]);
                        self.emit(
                            em,
                            "",
                            ev("response.output_item.added")
                                .with("output_index", int(oi))
                                .with("item", item),
                        )?;
                        let mut carry = ReasoningBlock::default();
                        carry.origin.clone_from(&h.origin);
                        carry.opaque.clone_from(&h.opaque);
                        self.blocks.insert(
                            b.index,
                            Blk::Reasoning {
                                output_index: oi,
                                id,
                                summaries: Vec::new(),
                                open_summary: None,
                                texts: Vec::new(),
                                open_text: None,
                                encrypted: None,
                                carry: Some(Box::new(carry)),
                            },
                        );
                    }
                    BlockHeader::Reasoning(h)
                        if reasoning_reply_policy(h.origin.as_ref(), DIALECT)
                            == ReplyPolicy::Keep =>
                    {
                        self.close_idle_message(em, "@close")?;
                        let oi = self.take_output();
                        let id = h
                            .opaque
                            .iter()
                            .find(|o| {
                                o.kind == OpaqueKind::OpenAiReasoningItemId
                                    && reply_opaque_visible(o, DIALECT)
                            })
                            .map_or_else(
                                || generated_id("rs", &self.response_id, oi),
                                |o| o.data.clone(),
                            );
                        let item = reasoning_item(Some(&id), &[], None, &[]);
                        let out = self.emit(
                            em,
                            "",
                            ev("response.output_item.added")
                                .with("output_index", int(oi))
                                .with("item", item),
                        )?;
                        let id = out.get("item").and_then(|i| i.get_str("id")).unwrap_or(id);
                        self.blocks.insert(
                            b.index,
                            Blk::Reasoning {
                                output_index: oi,
                                id,
                                summaries: Vec::new(),
                                open_summary: None,
                                texts: Vec::new(),
                                open_text: None,
                                encrypted: None,
                                carry: None,
                            },
                        );
                    }
                    BlockHeader::Unknown(item) if !item.origin.is(DIALECT) => {
                        if let Some(search) = web_search::anthropic_search(item) {
                            self.start_search(b.index, search, em)?;
                        } else {
                            if let Some((tool_use_id, found)) =
                                web_search::anthropic_search_result(item)
                            {
                                let status = if found { "completed" } else { "failed" };
                                self.finish_search(&tool_use_id, status, "", em)?;
                            }
                            self.hidden.push(b.index);
                        }
                    }
                    BlockHeader::Unknown(item) if item.origin.is(DIALECT) => {
                        self.close_idle_message(em, "@close")?;
                        let oi = self.take_output();
                        let header = Node::raw(&item.raw);
                        self.emit(
                            em,
                            "",
                            ev("response.output_item.added")
                                .with("output_index", int(oi))
                                .with("item", header.clone()),
                        )?;
                        self.blocks.insert(
                            b.index,
                            Blk::Unknown {
                                output_index: oi,
                                header,
                                last: None,
                            },
                        );
                    }
                    _ => self.hidden.push(b.index),
                }
                Ok(())
            }
            StreamEvent::BlockDelta(d) => self.delta(d, em),
            StreamEvent::BlockStop(s) => self.stop_block(s.index, "", em),
            StreamEvent::ResponseDelta(d) => {
                if let Some(stop) = &d.stop {
                    self.stop = Some(stop.clone());
                }
                if let Some(u) = &d.usage {
                    self.usage = Some(u.clone());
                }
                Ok(())
            }
            StreamEvent::ResponseStop(_) => {
                self.require_start()?;
                let open: Vec<u32> = self.blocks.keys().copied().collect();
                for index in open {
                    self.stop_block(index, &format!("@a{index}"), em)?;
                }
                let pending: Vec<String> = self
                    .searches
                    .iter()
                    .map(|s| s.tool_use_id.clone())
                    .collect();
                for tool_use_id in pending {
                    self.finish_search(&tool_use_id, "in_progress", "@search", em)?;
                }
                let stop = self.stop.clone().unwrap_or_default();
                if self.msg.is_some() {
                    self.close_message(em, "@close", message_status(&stop))?;
                }
                let (status, incomplete) = status_for(&stop);
                let mut obj = self.start.clone().unwrap_or_else(Node::obj);
                obj.insert("status", Node::str(status));
                if !self.native {
                    let done = (status == "completed").then(|| Node::int(self.created_at));
                    obj.insert("completed_at", done.unwrap_or_else(Node::null));
                }
                obj.insert("error", Node::null());
                obj.insert("incomplete_details", incomplete);
                let mut done = self.done.clone();
                done.sort_by_key(|(oi, _)| *oi);
                obj.insert(
                    "output",
                    Node::Arr(done.into_iter().map(|(_, n)| n).collect()),
                );
                obj.insert(
                    "usage",
                    self.usage.as_ref().map_or_else(Node::null, usage_node),
                );
                let ty = if status == "incomplete" {
                    "response.incomplete"
                } else {
                    "response.completed"
                };
                self.emit(em, "", ev(ty).with("response", obj))?;
                self.terminated = true;
                Ok(())
            }
            StreamEvent::Error(err) => {
                let is_event = |v: &Value| v.get("type").is_some_and(Value::is_string);
                let mine = err.origin.as_ref().is_some_and(|o| o.is(DIALECT));
                let events: Vec<Value> = match &err.raw {
                    Some(v) if mine && is_event(v) => vec![v.clone()],
                    Some(Value::Array(list))
                        if mine && !list.is_empty() && list.iter().all(is_event) =>
                    {
                        list.clone()
                    }
                    _ => vec![super::error::stream_error_event(err)],
                };
                for (i, e) in events.into_iter().enumerate() {
                    let scope = if i == 0 {
                        String::new()
                    } else {
                        format!("@e{i}")
                    };
                    self.emit(em, &scope, Node::Val(e))?;
                }
                self.terminated = true;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn require_start(&self) -> Result<(), KppError> {
        if self.started {
            Ok(())
        } else {
            Err(KppError::provider(
                "The stream sent content before the response started. This is a bug in Kaeso; please report it.",
            ))
        }
    }

    fn start_part(
        &mut self,
        index: u32,
        header: &BlockHeader,
        new_item: bool,
        em: &mut Emit<'_>,
    ) -> Result<(), KppError> {
        if new_item && self.msg.is_some() {
            self.close_message(em, "@close", "completed")?;
        }
        if self.msg.is_none() {
            let oi = self.take_output();
            let id = generated_id("msg", &self.response_id, oi);
            let item = message_item(&id, "in_progress", Vec::new());
            let out = self.emit(
                em,
                "@open",
                ev("response.output_item.added")
                    .with("output_index", int(oi))
                    .with("item", item),
            )?;
            let id = out.get("item").and_then(|i| i.get_str("id")).unwrap_or(id);
            self.msg = Some(Msg {
                output_index: oi,
                id,
                parts: Vec::new(),
                open: 0,
            });
        }
        let Some(msg) = self.msg.as_mut() else {
            return Ok(());
        };
        let content_index = u32::try_from(msg.parts.len()).unwrap_or(u32::MAX);
        let (output_index, item_id) = (msg.output_index, msg.id.clone());
        let (kind, part, header_node) = match header {
            BlockHeader::Text => (
                PartKind::Text,
                Node::obj()
                    .with("type", Node::str("output_text"))
                    .with("annotations", Node::Arr(Vec::new()))
                    .with("logprobs", Node::Arr(Vec::new()))
                    .with("text", Node::str("")),
                None,
            ),
            BlockHeader::Refusal => (PartKind::Refusal, super::parts::refusal_part(""), None),
            BlockHeader::Unknown(item) => {
                let n = Node::raw(&item.raw);
                (PartKind::Unknown, n.clone(), Some(n))
            }
            _ => return Ok(()),
        };
        msg.parts.push(None);
        msg.open += 1;
        self.emit(
            em,
            "",
            ev("response.content_part.added")
                .with("item_id", Node::str(&item_id))
                .with("output_index", int(output_index))
                .with("content_index", int(content_index))
                .with("part", part),
        )?;
        self.blocks.insert(
            index,
            Blk::Part {
                kind,
                output_index,
                item_id,
                content_index,
                text: String::new(),
                annotations: Vec::new(),
                logprobs: Vec::new(),
                header: header_node,
            },
        );
        Ok(())
    }

    fn close_idle_message(&mut self, em: &mut Emit<'_>, scope: &str) -> Result<(), KppError> {
        if self.msg.as_ref().is_some_and(|m| m.open == 0) {
            self.close_message(em, scope, "completed")?;
        }
        Ok(())
    }

    fn close_message(
        &mut self,
        em: &mut Emit<'_>,
        scope: &str,
        status: &str,
    ) -> Result<(), KppError> {
        let Some(msg) = self.msg.take() else {
            return Ok(());
        };
        let item = message_item(&msg.id, status, msg.parts.into_iter().flatten().collect());
        let out = self.emit(
            em,
            scope,
            ev("response.output_item.done")
                .with("output_index", int(msg.output_index))
                .with("item", item.clone()),
        )?;
        self.done
            .push((msg.output_index, out.get("item").unwrap_or(item)));
        Ok(())
    }

    fn stop_block(&mut self, index: u32, prefix: &str, em: &mut Emit<'_>) -> Result<(), KppError> {
        if let Some(pos) = self.hidden.iter().position(|i| *i == index) {
            self.hidden.remove(pos);
            return Ok(());
        }
        let Some(blk) = self.blocks.remove(&index) else {
            return Err(KppError::provider(format!(
                "The stream stopped block {index}, which never started. This is a bug in Kaeso; please report it."
            )));
        };
        match blk {
            Blk::Part {
                kind,
                output_index,
                item_id,
                content_index,
                text,
                annotations,
                logprobs,
                header,
            } => {
                let base = |ty: &str| {
                    ev(ty)
                        .with("item_id", Node::str(&item_id))
                        .with("output_index", int(output_index))
                        .with("content_index", int(content_index))
                };
                let part = match kind {
                    PartKind::Text => {
                        self.emit(
                            em,
                            &sc(prefix, "@done"),
                            base("response.output_text.done")
                                .with("text", Node::str(&text))
                                .with("logprobs", Node::Arr(logprobs.clone())),
                        )?;
                        Node::obj()
                            .with("type", Node::str("output_text"))
                            .with("annotations", Node::Arr(annotations))
                            .with("logprobs", Node::Arr(logprobs))
                            .with("text", Node::str(&text))
                    }
                    PartKind::Refusal => {
                        self.emit(
                            em,
                            &sc(prefix, "@done"),
                            base("response.refusal.done").with("refusal", Node::str(&text)),
                        )?;
                        super::parts::refusal_part(&text)
                    }
                    PartKind::Unknown => header.unwrap_or_else(Node::obj),
                };
                let out = self.emit(
                    em,
                    &sc(prefix, ""),
                    base("response.content_part.done").with("part", part.clone()),
                )?;
                if let Some(msg) = self.msg.as_mut() {
                    if let Some(slot) = msg
                        .parts
                        .get_mut(usize::try_from(content_index).unwrap_or(usize::MAX))
                    {
                        *slot = Some(out.get("part").unwrap_or(part));
                    }
                    msg.open = msg.open.saturating_sub(1);
                }
            }
            Blk::Call {
                output_index,
                id,
                call_id,
                name,
                mut args,
            } => {
                if !self.native && args.trim().is_empty() {
                    self.emit(
                        em,
                        "",
                        ev("response.function_call_arguments.delta")
                            .with("item_id", Node::str(&id))
                            .with("output_index", int(output_index))
                            .with("delta", Node::str(NO_ARGUMENTS)),
                    )?;
                    args.push_str(NO_ARGUMENTS);
                }
                self.emit(
                    em,
                    &sc(prefix, "@done"),
                    ev("response.function_call_arguments.done")
                        .with("item_id", Node::str(&id))
                        .with("output_index", int(output_index))
                        .with("arguments", Node::str(&args)),
                )?;
                let item =
                    self.namespaces
                        .call_item(&call_id, &name, &args, Some((&id, "completed")));
                self.item_done(em, prefix, output_index, item)?;
            }
            Blk::Reasoning {
                output_index,
                id,
                summaries,
                open_summary,
                texts,
                open_text,
                encrypted,
                carry,
            } => {
                let encrypted = match carry {
                    Some(block) => Some(crate::continuity::wrap_reasoning(&block)),
                    None => encrypted,
                };
                if let Some(si) = open_summary {
                    self.close_summary(em, prefix, &id, output_index, si, &summaries)?;
                }
                if let Some(ci) = open_text {
                    self.close_reasoning_text(em, prefix, &id, output_index, ci, &texts)?;
                }
                let item = reasoning_item(Some(&id), &summaries, encrypted.as_deref(), &texts);
                self.item_done(em, prefix, output_index, item)?;
            }
            Blk::Unknown {
                output_index,
                header,
                last,
            } => {
                self.item_done(em, prefix, output_index, last.unwrap_or(header))?;
            }
            Blk::Search(mut search) => {
                if search.query.is_none() {
                    search.query = serde_json::from_str::<Value>(&search.input)
                        .ok()
                        .as_ref()
                        .and_then(web_search::query_of);
                }
                self.searches.push(search);
            }
        }
        Ok(())
    }

    fn start_search(
        &mut self,
        index: u32,
        search: web_search::AnthropicSearch,
        em: &mut Emit<'_>,
    ) -> Result<(), KppError> {
        self.close_idle_message(em, "@close")?;
        let oi = self.take_output();
        let id = generated_id("ws", &self.response_id, oi);
        self.emit(
            em,
            "",
            ev("response.output_item.added")
                .with("output_index", int(oi))
                .with("item", web_search_call_item(&id, "in_progress", None)),
        )?;
        for ty in [
            "response.web_search_call.in_progress",
            "response.web_search_call.searching",
        ] {
            self.emit(
                em,
                "",
                ev(ty)
                    .with("item_id", Node::str(&id))
                    .with("output_index", int(oi)),
            )?;
        }
        self.blocks.insert(
            index,
            Blk::Search(Search {
                output_index: oi,
                id,
                tool_use_id: search.tool_use_id,
                query: search.query,
                input: String::new(),
            }),
        );
        Ok(())
    }

    fn finish_search(
        &mut self,
        tool_use_id: &str,
        status: &str,
        prefix: &str,
        em: &mut Emit<'_>,
    ) -> Result<(), KppError> {
        let Some(at) = self
            .searches
            .iter()
            .position(|s| s.tool_use_id == tool_use_id)
        else {
            return Ok(());
        };
        let search = self.searches.remove(at);
        if status == "completed" {
            self.emit(
                em,
                &sc(prefix, "@searched"),
                ev("response.web_search_call.completed")
                    .with("item_id", Node::str(&search.id))
                    .with("output_index", int(search.output_index)),
            )?;
        }
        let item = web_search_call_item(&search.id, status, search.query.as_deref());
        self.item_done(em, prefix, search.output_index, item)
    }

    fn item_done(
        &mut self,
        em: &mut Emit<'_>,
        prefix: &str,
        oi: u32,
        item: Node,
    ) -> Result<(), KppError> {
        let out = self.emit(
            em,
            &sc(prefix, ""),
            ev("response.output_item.done")
                .with("output_index", int(oi))
                .with("item", item.clone()),
        )?;
        self.done.push((oi, out.get("item").unwrap_or(item)));
        Ok(())
    }
}

pub(super) fn skipped(extras: &Extras, scope: &str) -> bool {
    matches!(
        extras.form_value(DIALECT, SKIP),
        Some(Value::Array(list)) if list.iter().any(|s| s.as_str() == Some(scope))
    )
}

pub(super) fn event_extras(event: &StreamEvent) -> Option<&Extras> {
    match event {
        StreamEvent::ResponseStart(e) => Some(&e.extras),
        StreamEvent::BlockStart(e) => Some(&e.extras),
        StreamEvent::BlockDelta(e) => Some(&e.extras),
        StreamEvent::BlockStop(e) => Some(&e.extras),
        StreamEvent::ResponseDelta(e) => Some(&e.extras),
        StreamEvent::ResponseStop(e) => Some(&e.extras),
        _ => None,
    }
}
