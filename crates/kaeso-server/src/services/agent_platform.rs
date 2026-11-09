use kaeso_codecs::EgressCtx;
use kaeso_codecs::gemini::GeminiProfile;
use kaeso_core::google::{self, AGENT_PLATFORM, EXPRESS_MODELS_PATH};
use kaeso_core::provider::{AuthMethod, AuthStyle};
use kaeso_core::providers::ProviderRef;
use kaeso_kpp::{DialectId, ErrorKind, KppError};
use secrecy::SecretString;
use url::Url;

use crate::model_api::errors::error;
use crate::services::credentials::Resolved;
use crate::services::google_auth::{self, TokenError};
use crate::state::AppState;

#[derive(Clone)]
pub struct Egress {
    pub base: Url,
    pub auth: AuthStyle,
    pub secret: SecretString,
    pub gemini: GeminiProfile,
    pub base_path: Option<String>,
}

impl Egress {
    pub fn apply(&self, ctx: EgressCtx) -> EgressCtx {
        let ctx = ctx.with_gemini_profile(self.gemini.clone());
        match &self.base_path {
            Some(path) => ctx.with_base_path(path.clone()),
            None => ctx,
        }
    }
}

// Mistral serves its models on the platform through `rawPredict` in Mistral's own format, a
// path Kaeso doesn't build yet
// (https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/partner-models/mistral).
const UNSERVED: &[&str] = &["mistral-", "codestral-"];

fn refused(message: String) -> EgressError {
    EgressError::Refused(error(ErrorKind::InvalidRequest, 400, message).with_param("model"))
}

impl std::fmt::Debug for Egress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Egress")
            .field("base", &self.base.as_str())
            .finish_non_exhaustive()
    }
}

pub fn is_agent_platform(provider: &ProviderRef) -> bool {
    provider.built_in().is_some_and(|p| p.id == AGENT_PLATFORM)
}

const KEY_REFUSED: &str = "The provider refused this project's provider key. Ask an organization admin to check it under Providers.";

#[derive(Debug)]
pub enum EgressError {
    Token(TokenError),
    Setup(KppError),
    Refused(KppError),
}

impl EgressError {
    pub fn refusal(&self) -> Option<(u16, &str)> {
        match self {
            EgressError::Token(TokenError::Refused { status, reason }) => Some((*status, reason)),
            EgressError::Token(TokenError::Sign) => Some((400, "unsigned")),
            _ => None,
        }
    }

    pub fn refuses_key(&self) -> bool {
        matches!(
            self,
            EgressError::Token(TokenError::Refused { .. } | TokenError::Sign)
        )
    }

    pub fn into_kpp(self) -> KppError {
        match self {
            EgressError::Token(e) => token_error(&e),
            EgressError::Setup(e) | EgressError::Refused(e) => e,
        }
    }
}

// A base URL the operator set (such as a Private Service Connect endpoint) serves every
// location as it is; otherwise each location has its own host (the platform's
// "Deployments and endpoints" page).
fn base(state: &AppState, provider: &ProviderRef, location: &str) -> Result<Url, EgressError> {
    let configured = crate::model_api::pipeline::base_url(state, provider);
    let default = provider.built_in().map(|p| p.default_base_url);
    match configured {
        Some(url) if Some(url.as_str().trim_end_matches('/')) != default => Ok(url.clone()),
        _ => Url::parse(&format!("https://{}", google::host(location))).map_err(|_| {
            EgressError::Setup(error(
                ErrorKind::ProviderError,
                500,
                "No base URL is configured for this provider.",
            ))
        }),
    }
}

pub async fn egress(
    state: &AppState,
    provider: &ProviderRef,
    credential: &Resolved,
    dialect: DialectId,
    model: &str,
) -> Result<Option<Egress>, EgressError> {
    if !is_agent_platform(provider) || credential.is_keyless() {
        return Ok(None);
    }
    if UNSERVED.iter().any(|prefix| model.starts_with(prefix)) {
        return Err(refused(format!(
            "Kaeso doesn't call Mistral models on {}; use the Mistral provider for `{model}`.",
            provider.name()
        )));
    }
    let gemini = provider.gemini().with_role_on_every_content(true);
    let Some(account) = credential
        .google
        .as_deref()
        .filter(|_| credential.method == AuthMethod::GoogleServiceAccount)
    else {
        // Express mode serves only Google's Gemini models
        // (https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/start/express-mode/overview).
        if dialect != DialectId::Gemini {
            return Err(refused(format!(
                "`{model}` needs a service account key; express mode API keys reach Gemini models only."
            )));
        }
        return Ok(Some(Egress {
            base: base(state, provider, google::GLOBAL)?,
            auth: AuthStyle::XGoogApiKey,
            secret: credential.secret.clone(),
            gemini: gemini.with_models_path(EXPRESS_MODELS_PATH),
            base_path: None,
        }));
    };
    let project = account.key.project_id();
    // Gemini Omni's video generation is served only in the global location
    // (https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/video/generate-videos-from-text).
    let location = if dialect == DialectId::GeminiInteractions {
        google::GLOBAL
    } else {
        account.location.as_str()
    };
    let base_path = match dialect {
        DialectId::GeminiInteractions => Some(google::interactions_base(project)),
        DialectId::AnthropicMessages => {
            Some(google::publisher_path(project, location, "anthropic"))
        }
        DialectId::OpenAiChat => Some(google::openapi_path(project, location)),
        _ => None,
    };
    let token =
        google_auth::access_token(state, credential.id.uuid(), credential.revision, account)
            .await
            .map_err(EgressError::Token)?;
    Ok(Some(Egress {
        base: base(state, provider, location)?,
        auth: AuthStyle::Bearer,
        secret: token,
        gemini: gemini.with_models_path(google::publisher_path(project, location, "google")),
        base_path,
    }))
}

pub fn token_error(e: &TokenError) -> KppError {
    match e {
        TokenError::Refused { .. } | TokenError::Sign => {
            error(ErrorKind::ProviderError, 502, KEY_REFUSED)
        }
        TokenError::Unavailable { status } => {
            tracing::warn!(status, "Google's token endpoint is unavailable");
            error(
                ErrorKind::ProviderError,
                503,
                "Google can't sign in the provider key right now. Try again shortly.",
            )
        }
        TokenError::Unreachable(cause) => {
            tracing::warn!(error = %cause, "cannot reach Google's token endpoint");
            error(
                ErrorKind::ProviderError,
                502,
                "Kaeso cannot reach the provider. Try again shortly.",
            )
        }
        TokenError::Unreadable => {
            tracing::warn!("Google's token endpoint answered in a form Kaeso cannot read");
            error(
                ErrorKind::ProviderError,
                502,
                "Google can't sign in the provider key right now. Try again shortly.",
            )
        }
    }
}

// The platform's model listing is a v1beta1 method in another shape than Gemini's, so a model
// check counts the tokens of a one-word prompt instead: Google charges nothing for countTokens
// (https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/capabilities/get-token-count),
// and its answer tells a served model from an unknown one and a working key from a refused one.
pub async fn probe(
    state: &AppState,
    credential: &Resolved,
    eff: &kaeso_core::catalogue::Effective,
) -> crate::services::availability::Probe {
    use crate::services::availability::Probe;
    use kaeso_codecs::dispatch;
    use kaeso_kpp::{GenerateRequest, Message, Request};

    let Some(dialect) = eff
        .provider
        .count_tokens_for(eff.dialect)
        .filter(|d| *d == eff.dialect)
    else {
        return Probe::Inconclusive(KppError::provider(
            "the provider counts no tokens for this model's format",
        ));
    };
    let egress = match egress(state, &eff.provider, credential, dialect, &eff.model_id).await {
        Ok(Some(e)) => e,
        Ok(None) => return Probe::Inconclusive(KppError::provider("not an agent platform key")),
        Err(e) if e.refuses_key() => {
            let mut refused = e.into_kpp();
            refused.kind = ErrorKind::Authentication;
            return Probe::Failed(refused);
        }
        Err(e) => return Probe::Failed(e.into_kpp()),
    };
    let mut asked = GenerateRequest::new(eff.model_id.clone());
    asked.messages.push(Message::user("hi"));
    let req = Request::CountTokens(asked);
    let mut ctx = egress.apply(
        EgressCtx::new(&req)
            .with_provider(eff.provider.id())
            .with_messages_profile(eff.provider.messages()),
    );
    let encoded = match dispatch::encode_request(dialect, &req, &mut ctx) {
        Ok(e) => e,
        Err(e) => return Probe::Failed(e),
    };
    let wait = state.config.upstream.first_byte_timeout;
    let sent = state
        .upstream
        .send(kaeso_upstream::Call {
            base_url: &egress.base,
            auth: Some(egress.auth),
            guarded: false,
            credential: &egress.secret,
            request: kaeso_upstream::Outgoing::from(encoded),
            stream: false,
            timeouts: kaeso_upstream::Timeouts {
                request: wait,
                first_byte: wait,
                idle: wait,
            },
            max_body: state.config.body_limits.provider_response.as_usize(),
        })
        .await;
    let resp = match sent {
        Ok(r) => r,
        Err(e) => {
            return Probe::Inconclusive(crate::model_api::pipeline::upstream_error_from(
                &e,
                &eff.provider,
            ));
        }
    };
    let kaeso_upstream::ResponseBody::Full(body) = resp.body else {
        return Probe::Inconclusive(KppError::provider("the model check arrived as a stream"));
    };
    if (200..300).contains(&resp.status) {
        return Probe::Served;
    }
    let err = crate::model_api::pipeline::without_key_fragments(dispatch::decode_error(
        dialect,
        resp.status,
        &resp.headers,
        &body,
    ));
    match err.kind {
        ErrorKind::NotFound => Probe::Unserved,
        _ => Probe::Failed(err),
    }
}
