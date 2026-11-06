use kaeso_core::provider::AccountSignIn;
use kaeso_kpp::{DialectId, ErrorKind, KppError, Request};

use super::errors::error;

pub(super) fn prepare(
    req: &mut Request,
    account: AccountSignIn,
    provider: &str,
) -> Result<DialectId, KppError> {
    let product = account.product;
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

// The ChatGPT backend refuses a model the plan doesn't include with 400 and this text, so
// Kaeso treats the model as not served to the account rather than the request as bad.
const NOT_IN_PLAN: &str = "is not supported when using Codex with a ChatGPT account";

pub(super) fn account_error(account: Option<AccountSignIn>, err: KppError) -> KppError {
    if let Some(limited) = plan_limit(account, &err) {
        return limited;
    }
    if account.is_some()
        && err.kind == ErrorKind::InvalidRequest
        && err.message.contains(NOT_IN_PLAN)
    {
        let mut unserved = err;
        unserved.kind = ErrorKind::NotFound;
        unserved.status = Some(404);
        return unserved;
    }
    err
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

#[cfg(test)]
mod tests {
    use super::*;
    use kaeso_core::provider::AuthMethod;

    const CHATGPT: AccountSignIn = AccountSignIn {
        method: AuthMethod::ChatGpt,
        product: "ChatGPT",
        usage_url: Some("https://usage.example.com"),
        experimental: false,
    };

    fn refused(message: &str) -> KppError {
        KppError::new(ErrorKind::InvalidRequest, message).with_status(400)
    }

    #[test]
    fn a_model_outside_the_plan_is_not_served_to_the_account() {
        let said =
            "The 'gpt-6-astra' model is not supported when using Codex with a ChatGPT account.";
        let err = account_error(Some(CHATGPT), refused(said));
        assert_eq!((err.kind, err.status), (ErrorKind::NotFound, Some(404)));
        assert_eq!(err.message, said);
    }

    #[test]
    fn other_refusals_and_api_keys_keep_their_error() {
        let bad = account_error(Some(CHATGPT), refused("Unknown parameter: `foo`."));
        assert_eq!(bad.kind, ErrorKind::InvalidRequest);
        let said =
            "The 'gpt-6-astra' model is not supported when using Codex with a ChatGPT account.";
        let key = account_error(None, refused(said));
        assert_eq!(
            (key.kind, key.status),
            (ErrorKind::InvalidRequest, Some(400))
        );
    }

    #[test]
    fn a_plan_out_of_usage_is_rate_limited_with_where_to_look() {
        let err = account_error(
            Some(CHATGPT),
            KppError::new(ErrorKind::Billing, "usage_limit_reached"),
        );
        assert_eq!(err.kind, ErrorKind::RateLimited);
        assert!(
            err.message.contains("https://usage.example.com"),
            "{}",
            err.message
        );
    }
}
