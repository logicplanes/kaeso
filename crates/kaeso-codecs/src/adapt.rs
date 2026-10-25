use std::borrow::Cow;

use kaeso_kpp::{DialectId, GenerateRequest, KppError, Request, Role};

use crate::codec::EgressCtx;
use crate::cross::check_extras;
use crate::openai_responses::{namespaces, replay};
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
        DialectId::OpenAiResponses => {
            web_search::for_responses(r, ctx.takes_web_search_access(target))?
        }
        DialectId::OpenAiChat => web_search::for_chat(r)?,
        _ => None,
    };
    if target == DialectId::AnthropicMessages
        && let Some(hoisted) = leading_system_hoisted(changed.as_ref().unwrap_or(r), target)?
    {
        changed = Some(hoisted);
    }
    if !ctx.takes_tool_namespaces(target)
        && let Some(flat) = namespaces::flatten(changed.as_ref().unwrap_or(r), target)?
    {
        changed = Some(flat);
    }
    if target == DialectId::OpenAiResponses
        && !ctx.takes_null_reasoning_content(target)
        && let Some(clean) = replay::without_null_reasoning_content(changed.as_ref().unwrap_or(r))
    {
        changed = Some(clean);
    }
    Ok(match changed {
        None => Cow::Borrowed(req),
        Some(g) if matches!(req, Request::CountTokens(_)) => Cow::Owned(Request::CountTokens(g)),
        Some(g) => Cow::Owned(Request::Generate(g)),
    })
}

fn leading_system_hoisted(
    r: &GenerateRequest,
    target: DialectId,
) -> Result<Option<GenerateRequest>, KppError> {
    let leading = r
        .messages
        .iter()
        .take_while(|m| m.role == Role::System && !m.extras.is_from(target))
        .count();
    if leading == 0 {
        return Ok(None);
    }
    let mut out = r.clone();
    for (i, m) in out.messages.drain(..leading).enumerate() {
        check_extras(&m.extras, target, &format!("messages[{i}]"), "message")?;
        out.system.extend(m.blocks);
    }
    Ok(Some(out))
}
