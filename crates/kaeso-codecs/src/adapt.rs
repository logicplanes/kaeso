use std::borrow::Cow;

use kaeso_kpp::{Block, DialectId, GenerateRequest, KppError, Message, Request, Role};

use crate::codec::EgressCtx;
use crate::cross::check_extras;
use crate::openai_responses::{ReasoningReplay, history, namespaces, replay};
use crate::{verbosity, web_search};

pub(crate) struct Draft<'a> {
    base: &'a GenerateRequest,
    edited: Option<GenerateRequest>,
    left_out: Vec<String>,
}

impl<'a> Draft<'a> {
    pub(crate) fn new(base: &'a GenerateRequest) -> Self {
        Draft {
            base,
            edited: None,
            left_out: Vec::new(),
        }
    }

    pub(crate) fn get(&self) -> &GenerateRequest {
        self.edited.as_ref().unwrap_or(self.base)
    }

    pub(crate) fn edit(&mut self) -> &mut GenerateRequest {
        self.edited.get_or_insert_with(|| self.base.clone())
    }

    pub(crate) fn leave_out(&mut self, path: String) {
        self.left_out.push(path);
    }
}

pub(crate) struct Adapted<'a> {
    pub(crate) req: Cow<'a, Request>,
    pub(crate) left_out: Vec<String>,
}

impl std::ops::Deref for Adapted<'_> {
    type Target = Request;

    fn deref(&self) -> &Request {
        &self.req
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
        });
    };
    let profile = ctx.responses_profile;
    let mut draft = Draft::new(r);
    match target {
        DialectId::AnthropicMessages => {
            web_search::for_anthropic(&mut draft)?;
            history::as_function_calls(&mut draft)?;
            fold_later_system(&mut draft, target)?;
        }
        DialectId::OpenAiChat => {
            web_search::for_chat(&mut draft)?;
            history::as_function_calls(&mut draft)?;
        }
        DialectId::OpenAiResponses => {
            web_search::for_responses(&mut draft, profile)?;
            if profile.reasoning_replay == ReasoningReplay::WithoutNullContent {
                replay::without_null_reasoning_content(&mut draft);
            }
        }
        _ => {}
    }
    if !ctx.takes_tool_namespaces(target) {
        namespaces::flatten(&mut draft, target)?;
    }
    verbosity::left_out(&mut draft, target, &ctx.chat_profile);
    let Draft {
        edited, left_out, ..
    } = draft;
    let req = match edited {
        None => Cow::Borrowed(req),
        Some(g) if matches!(req, Request::CountTokens(_)) => Cow::Owned(Request::CountTokens(g)),
        Some(g) => Cow::Owned(Request::Generate(g)),
    };
    Ok(Adapted { req, left_out })
}

// Anthropic takes a system message only right after a user turn and right before an
// assistant turn, and not on every model (Claude Sonnet 5 refuses them), while OpenAI apps
// place developer messages anywhere, typically before the user turn they introduce.
// https://platform.claude.com/docs/en/build-with-claude/mid-conversation-system-messages
fn fold_later_system(d: &mut Draft<'_>, target: DialectId) -> Result<(), KppError> {
    if d.get().extras.is_from(target) {
        return Ok(());
    }
    let mut any = false;
    for (i, m) in d.get().messages.iter().enumerate() {
        if m.role == Role::System {
            check_extras(&m.extras, target, &format!("messages[{i}]"), "message")?;
            any = true;
        }
    }
    if !any {
        return Ok(());
    }
    let r = d.edit();
    let mut held: Vec<Block> = Vec::new();
    let mut out: Vec<Message> = Vec::with_capacity(r.messages.len());
    for mut m in std::mem::take(&mut r.messages) {
        if m.role == Role::System {
            held.extend(m.blocks);
            continue;
        }
        if !held.is_empty() && m.role == Role::User {
            let at = m
                .blocks
                .iter()
                .take_while(|b| matches!(b, Block::ToolResult(_)))
                .count();
            m.blocks.splice(at..at, held.drain(..));
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
