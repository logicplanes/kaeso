use kaeso_core::provider::{AccountSignIn, Provider};
use kaeso_kpp::{DialectId, ErrorKind, KppError, Request};

use super::errors::error;

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
            kaeso_codecs::openai_responses::for_chatgpt_plan(g);
            Ok(DialectId::OpenAiResponses)
        }
        _ => Ok(DialectId::OpenAiResponses),
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
