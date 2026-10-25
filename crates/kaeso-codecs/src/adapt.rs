use std::borrow::Cow;

use kaeso_kpp::{DialectId, GenerateRequest, KppError, Request};

use crate::codec::EgressCtx;
use crate::cross::is_allowlisted;
use crate::openai_responses::namespaces;
use crate::web_search;

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
        && let Some(stripped) = without_bookkeeping(changed.as_ref().unwrap_or(r))
    {
        changed = Some(stripped);
    }
    Ok(match changed {
        None => Cow::Borrowed(req),
        Some(g) if matches!(req, Request::CountTokens(_)) => Cow::Owned(Request::CountTokens(g)),
        Some(g) => Cow::Owned(Request::Generate(g)),
    })
}

fn without_bookkeeping(r: &GenerateRequest) -> Option<GenerateRequest> {
    let bookkeeping = |key: &String| is_allowlisted(DialectId::OpenAiChat, "message", key);
    let own = |m: &kaeso_kpp::Message| m.extras.is_from(DialectId::OpenAiChat);
    if !r
        .messages
        .iter()
        .any(|m| own(m) && m.extras.fields.keys().any(bookkeeping))
    {
        return None;
    }
    let mut stripped = r.clone();
    for message in stripped.messages.iter_mut().filter(|m| own(m)) {
        message.extras.fields.retain(|key, _| !bookkeeping(key));
    }
    Some(stripped)
}
