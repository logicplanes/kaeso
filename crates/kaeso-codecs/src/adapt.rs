use std::borrow::Cow;

use kaeso_kpp::{DialectId, GenerateRequest, KppError, Message, Request, Role};
use serde_json::Value;

use crate::codec::EgressCtx;
use crate::cross::{CHAT_ASSISTANT_BOOKKEEPING, is_chat_bookkeeping};
use crate::openai_responses::namespaces;
use crate::web_search;
use crate::wire::VERBATIM_FORM_KEY;

pub(crate) fn for_target<'a>(
    req: &'a Request,
    target: DialectId,
    ctx: &EgressCtx,
) -> Result<Cow<'a, Request>, KppError> {
    let (Request::Generate(r) | Request::CountTokens(r)) = req else {
        return Ok(Cow::Borrowed(req));
    };
    let mut changed: Option<GenerateRequest> = match target {
        DialectId::AnthropicMessages => web_search::for_anthropic(r)?,
        DialectId::OpenAiResponses => web_search::for_responses(r),
        _ => None,
    };
    if !ctx.takes_tool_namespaces(target)
        && let Some(flat) = namespaces::flatten(changed.as_ref().unwrap_or(r), target)?
    {
        changed = Some(flat);
    }
    if target == DialectId::OpenAiChat
        && !ctx.chat_profile.request.message_bookkeeping
        && has_bookkeeping(changed.as_ref().unwrap_or(r))
    {
        let mut stripped = changed.take().unwrap_or_else(|| r.clone());
        strip_bookkeeping(&mut stripped);
        changed = Some(stripped);
    }
    Ok(match changed {
        None => Cow::Borrowed(req),
        Some(g) if matches!(req, Request::CountTokens(_)) => Cow::Owned(Request::CountTokens(g)),
        Some(g) => Cow::Owned(Request::Generate(g)),
    })
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
