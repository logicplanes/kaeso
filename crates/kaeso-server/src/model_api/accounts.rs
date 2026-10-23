use kaeso_core::provider::{AccountSignIn, Provider};
use kaeso_kpp::{DialectId, ErrorKind, GenerateRequest, KppError, RawJson, Request, StreamMode};

use super::errors::error;

// SIWC "Preview limitations": these Responses fields are refused for plan usage, as is
// previous_response_id over HTTP, so a request with any of them would fail at OpenAI.
const UNSUPPORTED: &[&str] = &[
    "background",
    "conversation",
    "max_output_tokens",
    "max_tool_calls",
    "metadata",
    "moderation",
    "multi_agent",
    "prompt",
    "prompt_cache_retention",
    "safety_identifier",
    "temperature",
    "top_logprobs",
    "top_p",
    "truncation",
    "user",
    "previous_response_id",
    "store",
];

// The same page wants `input` as an array and refuses `system` message items, so a
// Responses app's string input and system items are written in the default form:
// an input array, and `instructions` or a developer message.
const NATIVE_FORMS: &[&str] = &["input", "system_items", "instructions"];

pub(super) fn prepare(
    req: &mut Request,
    account: AccountSignIn,
    provider: &Provider,
) -> Result<DialectId, KppError> {
    let product = account.product;
    let provider = provider.name;
    match req {
        Request::Embed(_) => Err(KppError::unsupported(
            "embeddings",
            format!("A {product} account can't create embeddings. Use an API key for {provider} in this project instead."),
        )
        .with_status(400)
        .with_param("model")),
        Request::CountTokens(_) => Err(KppError::unsupported(
            "token counting",
            format!("A {product} account can't count tokens. Use an API key for {provider} in this project instead."),
        )
        .with_status(400)
        .with_param("model")),
        Request::Generate(g) => {
            for_plan(g);
            Ok(DialectId::OpenAiResponses)
        }
        _ => Ok(DialectId::OpenAiResponses),
    }
}

fn for_plan(g: &mut GenerateRequest) {
    g.sampling.max_output_tokens = None;
    g.sampling.temperature = None;
    g.sampling.top_p = None;
    g.user = None;
    g.extras
        .fields
        .retain(|name, _| !UNSUPPORTED.contains(&name.as_str()));
    g.stream = StreamMode::on(true);
    if g.extras.is_from(DialectId::OpenAiResponses) {
        g.extras
            .form
            .retain(|name, _| !NATIVE_FORMS.contains(&name.as_str()));
        if let Ok(off) = RawJson::from_serialize(&false) {
            g.extras.fields.insert("store".to_owned(), off);
        }
    }
}

pub(super) fn usage_limit(account: Option<AccountSignIn>, err: KppError) -> KppError {
    plan_limit(account, &err).unwrap_or(err)
}

pub(super) fn plan_limit(account: Option<AccountSignIn>, err: &KppError) -> Option<KppError> {
    match account.and_then(|a| a.usage_url.map(|url| (a.product, url))) {
        Some((product, url)) if err.kind == ErrorKind::Billing => Some(error(
            ErrorKind::RateLimited,
            429,
            format!("Your {product} plan's usage limit is reached. Review it at {url}."),
        )),
        _ => None,
    }
}
