use std::borrow::Cow;

use kaeso_kpp::{DialectId, GenerateRequest, KppError, Request};

use crate::codec::EgressCtx;
use crate::openai_responses::namespaces;
use crate::{tool_search, web_search};

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
    if let Some(loaded) = tool_search::for_foreign(changed.as_ref().unwrap_or(r), target)? {
        changed = Some(loaded);
    }
    if !ctx.takes_tool_namespaces(target)
        && let Some(flat) = namespaces::flatten(changed.as_ref().unwrap_or(r), target)?
    {
        changed = Some(flat);
    }
    Ok(match changed {
        None => Cow::Borrowed(req),
        Some(g) if matches!(req, Request::CountTokens(_)) => Cow::Owned(Request::CountTokens(g)),
        Some(g) => Cow::Owned(Request::Generate(g)),
    })
}
