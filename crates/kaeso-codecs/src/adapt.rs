use std::borrow::Cow;

use kaeso_kpp::{
    Block, DialectId, GenerateRequest, KppError, Message, RawJson, Request, Role, ToolCallBlock,
    ToolChoice,
};
use serde_json::{Map, Value};

use crate::codec::EgressCtx;
use crate::cross::{CHAT_ASSISTANT_BOOKKEEPING, check_extras, is_chat_bookkeeping};
use crate::openai_responses::{InputItems, ReasoningReplay, history, namespaces, replay};
use crate::wire::VERBATIM_FORM_KEY;
use crate::{tool_roots, tool_search, verbosity, web_search};

const INPUT: &str = "input";

pub(crate) struct Draft<'a> {
    base: &'a GenerateRequest,
    edited: Option<GenerateRequest>,
    left_out: Vec<String>,
    adapted: Vec<String>,
    search_limit: Option<(&'static str, u64)>,
}

impl<'a> Draft<'a> {
    pub(crate) fn new(base: &'a GenerateRequest) -> Self {
        Draft {
            base,
            edited: None,
            left_out: Vec::new(),
            adapted: Vec::new(),
            search_limit: None,
        }
    }

    pub(crate) fn original(&self) -> &'a GenerateRequest {
        self.base
    }

    pub(crate) fn get(&self) -> &GenerateRequest {
        self.edited.as_ref().unwrap_or(self.base)
    }

    pub(crate) fn edit(&mut self) -> &mut GenerateRequest {
        self.edited.get_or_insert_with(|| self.base.clone())
    }

    pub(crate) fn replace(&mut self, edited: GenerateRequest) {
        self.edited = Some(edited);
    }

    pub(crate) fn leave_out(&mut self, path: String) {
        self.left_out.push(path);
    }

    pub(crate) fn adapt(&mut self, path: String) {
        self.adapted.push(path);
    }

    pub(crate) fn set_search_limit(&mut self, field: &'static str, limit: u64) {
        self.search_limit = Some((field, limit));
    }
}

pub(crate) struct Adapted<'a> {
    pub(crate) req: Cow<'a, Request>,
    pub(crate) left_out: Vec<String>,
    pub(crate) adapted: Vec<String>,
    pub(crate) search_limit: Option<(&'static str, u64)>,
}

impl std::ops::Deref for Adapted<'_> {
    type Target = Request;

    fn deref(&self) -> &Request {
        &self.req
    }
}

// Only OpenAI's Responses API takes `tool_choice` in a request without tools, such as a Codex
// helper call or a request whose tools were all left out on the way. `auto` and `none` change
// nothing there and are left out; a choice that forces a tool can't be met.
fn tool_choice_without_tools(d: &mut Draft<'_>) -> Result<(), KppError> {
    let r = d.get();
    if !r.tools.is_empty() {
        return Ok(());
    }
    match &r.tool_choice {
        Some(ToolChoice::Auto | ToolChoice::None) => {
            d.edit().tool_choice = None;
            d.leave_out("tool_choice".to_owned());
            Ok(())
        }
        Some(ToolChoice::Required | ToolChoice::Tool(_)) => Err(KppError::invalid_request(
            "`tool_choice` asks for a tool, but the request has no tools; send the tools or leave `tool_choice` out.",
        )
        .with_param("tool_choice")),
        _ => Ok(()),
    }
}

pub(crate) fn for_target<'a>(
    req: &'a Request,
    target: DialectId,
    ctx: &EgressCtx,
) -> Result<Adapted<'a>, KppError> {
    let (Request::Generate(r) | Request::CountTokens(r)) = req else {
        return Ok(Adapted {
            req: Cow::Borrowed(req),
            left_out: Vec::new(),
            adapted: Vec::new(),
            search_limit: None,
        });
    };
    let profile = ctx.responses_profile;
    let mut draft = Draft::new(r);
    match target {
        DialectId::AnthropicMessages => {
            web_search::for_anthropic(&mut draft)?;
            web_search::for_claude(&mut draft);
            history::agent_messages(&mut draft, "Anthropic")?;
            history::as_function_calls(&mut draft)?;
            object_arguments(&mut draft, target)?;
            fold_later_system(&mut draft, target)?;
        }
        DialectId::OpenAiChat => {
            web_search::for_chat(&mut draft)?;
            history::agent_messages(&mut draft, ctx.chat_profile.provider)?;
            history::as_function_calls(&mut draft)?;
            place_system_for_chat(&mut draft, target);
        }
        DialectId::OpenAiResponses => {
            web_search::for_responses(&mut draft, profile)?;
            if profile.reasoning_replay == ReasoningReplay::WithoutNullContent {
                replay::without_null_reasoning_content(&mut draft);
            }
            match profile.input_items {
                InputItems::AsSent => {}
                InputItems::WithoutAgentMessages => {
                    history::agent_messages(&mut draft, profile.provider)?;
                }
                InputItems::Only(allowed) => {
                    history::agent_messages(&mut draft, profile.provider)?;
                    history::only_items(&mut draft, allowed, profile.provider)?;
                }
            }
        }
        _ => {}
    }
    if let Some(loaded) = tool_search::for_foreign(draft.get(), target)? {
        draft.replace(loaded);
    }
    if !ctx.takes_tool_namespaces(target) {
        namespaces::flatten(&mut draft, target)?;
    }
    tool_roots::for_target(&mut draft, target, ctx)?;
    if !(target == DialectId::OpenAiResponses && profile.tool_choice_without_tools) {
        tool_choice_without_tools(&mut draft)?;
    }
    if target == DialectId::OpenAiChat
        && !ctx.chat_profile.request.message_bookkeeping
        && has_bookkeeping(draft.get())
    {
        strip_bookkeeping(draft.edit());
    }
    verbosity::left_out(&mut draft, target, &ctx.chat_profile);
    let Draft {
        edited,
        left_out,
        adapted,
        search_limit,
        ..
    } = draft;
    let req = match edited {
        None => Cow::Borrowed(req),
        Some(g) if matches!(req, Request::CountTokens(_)) => Cow::Owned(Request::CountTokens(g)),
        Some(g) => Cow::Owned(Request::Generate(g)),
    };
    Ok(Adapted {
        req,
        left_out,
        adapted,
        search_limit,
    })
}

fn takes_object(c: &ToolCallBlock, target: DialectId) -> bool {
    !c.extras.is_from(target) && !c.arguments.is_object()
}

fn calls_without_object(
    r: &GenerateRequest,
    target: DialectId,
) -> impl Iterator<Item = &ToolCallBlock> {
    r.messages
        .iter()
        .flat_map(|m| &m.blocks)
        .filter_map(move |b| match b {
            Block::ToolCall(c) if takes_object(c, target) => Some(c),
            _ => None,
        })
}

fn as_object(arguments: &RawJson) -> Result<RawJson, KppError> {
    let invalid = |e: serde_json::Error| {
        KppError::invalid_request(format!("A tool call's arguments cannot be sent: {e}."))
    };
    match arguments.to_value().map_err(invalid)? {
        Value::Null => Ok(RawJson::empty_object()),
        value => {
            RawJson::from_serialize(&Map::from_iter([(INPUT.to_owned(), value)])).map_err(invalid)
        }
    }
}

// Anthropic refuses a whole request when one `tool_use.input` in its history is not an
// object; calls other formats made with text, arrays or `null` would leave the thread stuck.
fn object_arguments(d: &mut Draft<'_>, target: DialectId) -> Result<(), KppError> {
    let ids: Vec<String> = calls_without_object(d.get(), target)
        .map(|c| c.id.clone())
        .collect();
    if ids.is_empty() {
        return Ok(());
    }
    for block in d
        .edit()
        .messages
        .iter_mut()
        .flat_map(|m| m.blocks.iter_mut())
    {
        if let Block::ToolCall(c) = block
            && takes_object(c, target)
        {
            c.arguments = as_object(&c.arguments)?;
        }
    }
    for id in ids {
        d.adapt(format!("tool call {id}"));
    }
    Ok(())
}

fn foreign_system(d: &Draft<'_>, target: DialectId) -> Result<bool, KppError> {
    if d.get().extras.is_from(target) {
        return Ok(false);
    }
    let mut any = false;
    for (i, m) in d.get().messages.iter().enumerate() {
        if m.role == Role::System {
            check_extras(&m.extras, target, &format!("messages[{i}]"), "message")?;
            any = true;
        }
    }
    Ok(any)
}

fn leading_results(m: &Message) -> usize {
    m.blocks
        .iter()
        .take_while(|b| matches!(b, Block::ToolResult(_)))
        .count()
}

// Anthropic takes a system message only right after a user turn and right before an
// assistant turn, and not on every model (Claude Sonnet 5 refuses them), while OpenAI apps
// place developer messages anywhere, typically before the user turn they introduce.
// https://platform.claude.com/docs/en/build-with-claude/mid-conversation-system-messages
fn fold_later_system(d: &mut Draft<'_>, target: DialectId) -> Result<(), KppError> {
    if !foreign_system(d, target)? {
        return Ok(());
    }
    let r = d.edit();
    let leading = r
        .messages
        .iter()
        .take_while(|m| m.role == Role::System)
        .count();
    for m in r.messages.drain(..leading) {
        r.system.extend(m.blocks);
    }
    let last = r.messages.len().saturating_sub(1);
    let mut held: Vec<Block> = Vec::new();
    let mut out: Vec<Message> = Vec::with_capacity(r.messages.len());
    for (i, mut m) in std::mem::take(&mut r.messages).into_iter().enumerate() {
        if m.role == Role::System {
            held.extend(m.blocks);
            continue;
        }
        if !held.is_empty() && m.role == Role::User {
            let at = leading_results(&m);
            m.blocks.splice(at..at, held.drain(..));
        } else if !held.is_empty() && m.role == Role::Assistant && i == last {
            // A final assistant message is a prefill that Anthropic continues; text after it
            // would turn it into a finished turn.
            match out.iter_mut().rev().find(|m| m.role == Role::User) {
                Some(user) => user.blocks.append(&mut held),
                None => out.push(Message::new(Role::User, std::mem::take(&mut held))),
            }
        }
        out.push(m);
    }
    if !held.is_empty() {
        match out.last_mut() {
            Some(last) if last.role == Role::User => last.blocks.append(&mut held),
            _ => out.push(Message::new(Role::User, held)),
        }
    }
    r.messages = out;
    Ok(())
}

fn calls_tools(m: &Message) -> bool {
    m.role == Role::Assistant && m.blocks.iter().any(|b| matches!(b, Block::ToolCall(_)))
}

// Chat Completions wants a call's `tool` messages right after the assistant message that
// made it: Mistral answers a system message between them with a 400
// `invalid_request_message_order` ("Unexpected role 'tool' after role 'system'").
fn place_system_for_chat(d: &mut Draft<'_>, target: DialectId) {
    let r = d.get();
    if r.extras.is_from(target) || !r.messages.iter().any(|m| m.role == Role::System) {
        return;
    }
    let r = d.edit();
    let mut out: Vec<Message> = Vec::with_capacity(r.messages.len());
    let mut deferred: Vec<Message> = Vec::new();
    let mut awaiting_results = false;
    for mut m in std::mem::take(&mut r.messages) {
        match m.role {
            Role::System if awaiting_results => deferred.push(m),
            Role::User if awaiting_results && !deferred.is_empty() => {
                let at = leading_results(&m);
                if at == m.blocks.len() {
                    out.push(m);
                    continue;
                }
                if at > 0 {
                    let rest = m.blocks.split_off(at);
                    out.push(m.clone());
                    m.blocks = rest;
                }
                out.append(&mut deferred);
                out.push(m);
                awaiting_results = false;
            }
            Role::System => out.push(m),
            _ => {
                out.append(&mut deferred);
                awaiting_results = calls_tools(&m);
                out.push(m);
            }
        }
    }
    out.append(&mut deferred);
    r.messages = out;
}

fn null_bookkeeping(message: &Message) -> bool {
    message
        .extras
        .form_value(DialectId::OpenAiChat, VERBATIM_FORM_KEY)
        .and_then(Value::as_object)
        .is_some_and(|nulls| nulls.keys().any(|key| is_chat_bookkeeping(message, key)))
}

fn has_bookkeeping(r: &GenerateRequest) -> bool {
    r.messages.iter().any(|m| {
        m.extras
            .fields
            .keys()
            .any(|key| is_chat_bookkeeping(m, key))
            || null_bookkeeping(m)
    })
}

fn strip_bookkeeping(r: &mut GenerateRequest) {
    for message in &mut r.messages {
        if message.role != Role::Assistant || !message.extras.is_from(DialectId::OpenAiChat) {
            continue;
        }
        let extras = &mut message.extras;
        extras
            .fields
            .retain(|key, _| !CHAT_ASSISTANT_BOOKKEEPING.contains(&key.as_str()));
        if let Some(Value::Object(nulls)) = extras.form.get_mut(VERBATIM_FORM_KEY) {
            nulls.retain(|key, _| !CHAT_ASSISTANT_BOOKKEEPING.contains(&key.as_str()));
            if nulls.is_empty() {
                extras.form.shift_remove(VERBATIM_FORM_KEY);
            }
        }
    }
}
