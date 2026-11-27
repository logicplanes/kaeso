use kaeso_core::provider::{AccountSignIn, AuthMethod};
use kaeso_kpp::{DialectId, ErrorKind, KppError, Request, RequestKind};

use super::errors::error;

pub(super) struct Prepared {
    pub(super) dialect: DialectId,
    // The plan's route refuses the output limit, so Kaeso ends the reply there itself; the
    // app's field that set it is named in `kaeso-adapted`.
    pub(super) output_limit: Option<(u64, &'static str)>,
}

pub(super) fn prepare(
    req: &mut Request,
    account: AccountSignIn,
    provider: &str,
    ingress: DialectId,
) -> Result<Prepared, KppError> {
    let (capability, cannot) = match req {
        Request::Generate(g) => {
            let output_limit = g.sampling.max_output_tokens.map(|limit| {
                (
                    u64::from(limit),
                    kaeso_core::catalogue::capability::output_param(g, ingress),
                )
            });
            kaeso_codecs::openai_responses::for_chatgpt_plan(g);
            return Ok(Prepared {
                dialect: DialectId::OpenAiResponses,
                output_limit,
            });
        }
        Request::Embed(_) => ("embeddings", "create embeddings"),
        Request::CountTokens(_) => ("token counting", "count tokens"),
        Request::Image(_) => ("images", "make images"),
        Request::Speech(_) => ("speech", "create speech"),
        Request::Transcription(_) => ("transcription", "transcribe audio"),
        Request::Decide(_) => ("decisions", "make decisions"),
        Request::Rerank(_) => ("rerank", "rerank documents"),
        _ => ("endpoint", "serve this endpoint"),
    };
    let product = account.product;
    Err(KppError::unsupported(
        capability,
        format!("A {product} account can't {cannot}. Use an API key for {provider} in this project instead."),
    )
    .with_status(400)
    .with_param("model"))
}

pub(super) fn media_api(kind: RequestKind) -> Option<&'static str> {
    match kind {
        RequestKind::Image => Some("Images API"),
        RequestKind::Speech => Some("Speech API"),
        RequestKind::Transcription => Some("Transcriptions API"),
        RequestKind::Video => Some("Videos API"),
        RequestKind::Decide => Some("Decisions API"),
        _ => None,
    }
}

pub(crate) fn text_only(method: AuthMethod) -> bool {
    method == AuthMethod::ChatGpt
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
        Some((product, url)) if err.kind == ErrorKind::Billing => {
            // A plan's usage limit lasts until the plan resets, hours later; a wait the provider
            // named still holds the account that long.
            let mut limited = error(
                ErrorKind::RateLimited,
                429,
                format!("Your {product} plan's usage limit is reached. Review it at {url}."),
            )
            .with_retry(kaeso_kpp::Retry::AfterReset);
            limited.retry_after = err.retry_after;
            Some(limited)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn a_plan_serves_generation_only_and_says_what_needs_an_api_key() {
        let mut video = Request::Video(kaeso_kpp::VideoRequest::new(
            "m",
            kaeso_kpp::VideoOperation::Generate,
        ));
        let Err(err) = prepare(&mut video, CHATGPT, "OpenAI", DialectId::OpenAiChat) else {
            panic!("a plan serves no videos")
        };
        assert_eq!(
            err.message,
            "A ChatGPT account can't serve this endpoint. Use an API key for OpenAI in this project instead."
        );
        assert_eq!(
            (err.status, err.param.as_deref()),
            (Some(400), Some("model"))
        );
        assert_eq!(media_api(RequestKind::Video), Some("Videos API"));
        assert_eq!(media_api(RequestKind::Generate), None);
        assert!(text_only(AuthMethod::ChatGpt));
        assert!(!text_only(AuthMethod::Grok) && !text_only(AuthMethod::ApiKey));
    }

    #[test]
    fn a_plan_makes_no_decisions_and_names_the_decisions_api() {
        let mut decide = Request::Decide(kaeso_kpp::DecideRequest::new(
            "gpt-6-luna",
            kaeso_kpp::DecisionInput::Text("state".into()),
        ));
        let Err(err) = prepare(&mut decide, CHATGPT, "OpenAI", DialectId::OpenAiChat) else {
            panic!("a plan makes no decisions")
        };
        assert_eq!(
            err.message,
            "A ChatGPT account can't make decisions. Use an API key for OpenAI in this project instead."
        );
        assert_eq!(media_api(RequestKind::Decide), Some("Decisions API"));
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
        assert_eq!(
            (err.retry, err.retry_after),
            (Some(kaeso_kpp::Retry::AfterReset), None)
        );
        let stated = account_error(
            Some(CHATGPT),
            KppError::new(ErrorKind::Billing, "usage_limit_reached")
                .with_retry_after(std::time::Duration::from_secs(7_200)),
        );
        assert_eq!(
            stated.retry_after,
            Some(std::time::Duration::from_secs(7_200))
        );
    }
}
