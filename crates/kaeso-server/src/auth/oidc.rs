use std::net::SocketAddr;
use std::time::Duration;

use kaeso_core::net::is_public;
use openidconnect::core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata};
use openidconnect::{
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, HttpRequest, HttpResponse, IssuerUrl,
    Nonce, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenResponse,
};

use crate::state::AppState;

#[derive(Debug, thiserror::Error)]
pub enum OidcError {
    #[error("the identity provider could not be reached: {0}")]
    Provider(String),
    #[error(
        "the identity provider's address is on a private network, which this install does not allow"
    )]
    PrivateNetwork,
    #[error("the identity provider's answer could not be verified: {0}")]
    Token(String),
    #[error("the SSO connection is misconfigured: {0}")]
    Config(String),
}

impl OidcError {
    pub fn message(&self) -> &'static str {
        match self {
            OidcError::Provider(_) => "The identity provider could not be reached.",
            OidcError::PrivateNetwork => {
                "The identity provider is on a private network, which this install doesn't allow."
            }
            OidcError::Token(_) => "The identity provider's answer could not be verified.",
            OidcError::Config(_) => "The SSO connection doesn't work with this identity provider.",
        }
    }
}

fn chain(e: &dyn std::error::Error) -> String {
    let mut message = e.to_string();
    let mut source = e.source();
    while let Some(s) = source {
        let part = s.to_string();
        if !message.contains(&part) {
            message.push_str(": ");
            message.push_str(&part);
        }
        source = s.source();
    }
    message
}

fn provider_error(e: &dyn std::error::Error) -> OidcError {
    let cause = chain(e);
    if cause.contains(&OidcError::PrivateNetwork.to_string()) {
        OidcError::PrivateNetwork
    } else {
        OidcError::Provider(cause)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct FetchError(String);

const MAX_ANSWER: usize = 1 << 20;

pub async fn fetch(allow_private: bool, request: HttpRequest) -> Result<HttpResponse, FetchError> {
    let uri = request.uri().clone();
    let host = uri
        .host()
        .ok_or_else(|| FetchError("the URL has no host".into()))?
        .to_owned();
    let https = uri.scheme_str() == Some("https");
    let port = uri.port_u16().unwrap_or(if https { 443 } else { 80 });
    let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), port))
        .await
        .map_err(|e| FetchError(format!("cannot resolve {host}: {e}")))?
        .collect();
    let addr = addrs
        .iter()
        .copied()
        .find(|a| allow_private || is_public(a.ip()))
        .ok_or_else(|| FetchError(OidcError::PrivateNetwork.to_string()))?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .resolve(&host, addr)
        .build()
        .map_err(|e| FetchError(e.to_string()))?;
    let (parts, body) = request.into_parts();
    let response = client
        .request(parts.method, uri.to_string())
        .headers(parts.headers)
        .body(body)
        .send()
        .await
        .map_err(|e| FetchError(format!("request to {host} failed: {e}")))?;
    let status = response.status();
    let headers = response.headers().clone();
    let mut response = response;
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| FetchError(e.to_string()))?
    {
        if bytes.len() + chunk.len() > MAX_ANSWER {
            return Err(FetchError(
                "the identity provider's answer is too large".into(),
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    let mut out = HttpResponse::new(bytes);
    *out.status_mut() = status;
    *out.headers_mut() = headers;
    Ok(out)
}

pub struct Provider {
    metadata: CoreProviderMetadata,
    client_id: String,
    client_secret: ClientSecret,
    redirect: RedirectUrl,
    allow_private: bool,
}

pub fn redirect_url(state: &AppState) -> String {
    format!("{}platform/v1/auth/oidc/callback", state.config.public_url)
}

impl Provider {
    pub async fn discover(
        state: &AppState,
        issuer: &str,
        client_id: &str,
        client_secret: String,
    ) -> Result<Provider, OidcError> {
        let allow_private = state.config.auth.oidc_allow_private_networks;
        let issuer = IssuerUrl::new(issuer.to_owned())
            .map_err(|e| OidcError::Config(format!("issuer: {e}")))?;
        let http = move |req: HttpRequest| fetch(allow_private, req);
        let metadata = CoreProviderMetadata::discover_async(issuer, &http)
            .await
            .map_err(|e| provider_error(&e))?;
        Ok(Provider {
            metadata,
            client_id: client_id.to_owned(),
            client_secret: ClientSecret::new(client_secret),
            redirect: RedirectUrl::new(redirect_url(state))
                .map_err(|e| OidcError::Config(e.to_string()))?,
            allow_private,
        })
    }

    fn client(
        &self,
    ) -> openidconnect::core::CoreClient<
        openidconnect::EndpointSet,
        openidconnect::EndpointNotSet,
        openidconnect::EndpointNotSet,
        openidconnect::EndpointNotSet,
        openidconnect::EndpointMaybeSet,
        openidconnect::EndpointMaybeSet,
    > {
        CoreClient::from_provider_metadata(
            self.metadata.clone(),
            ClientId::new(self.client_id.clone()),
            Some(self.client_secret.clone()),
        )
        .set_redirect_uri(self.redirect.clone())
    }

    pub fn start(&self) -> Started {
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let (url, state, nonce) = self
            .client()
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .add_scope(Scope::new("email".into()))
            .add_scope(Scope::new("profile".into()))
            .set_pkce_challenge(challenge)
            .url();
        Started {
            url: url.to_string(),
            state: state.secret().clone(),
            nonce: nonce.secret().clone(),
            pkce_verifier: verifier.secret().clone(),
        }
    }

    pub async fn finish(
        &self,
        code: &str,
        pkce_verifier: &str,
        nonce: &str,
    ) -> Result<Identity, OidcError> {
        let allow_private = self.allow_private;
        let http = move |req: HttpRequest| fetch(allow_private, req);
        let client = self.client();
        let response = client
            .exchange_code(AuthorizationCode::new(code.to_owned()))
            .map_err(|e| OidcError::Config(e.to_string()))?
            .set_pkce_verifier(PkceCodeVerifier::new(pkce_verifier.to_owned()))
            .request_async(&http)
            .await
            .map_err(|e| provider_error(&e))?;
        let id_token = response
            .id_token()
            .ok_or_else(|| OidcError::Token("no ID token in the answer".into()))?;
        let claims = id_token
            .claims(&client.id_token_verifier(), &Nonce::new(nonce.to_owned()))
            .map_err(|e| OidcError::Token(e.to_string()))?;
        if claims.email_verified() == Some(false) {
            return Err(OidcError::Token(
                "the email address is not verified by the identity provider".into(),
            ));
        }
        let email = claims
            .email()
            .map(|e| e.as_str().to_owned())
            .ok_or_else(|| {
                OidcError::Token(
                    "the ID token has no email address; grant the `email` scope".into(),
                )
            })?;
        let name = claims
            .name()
            .and_then(|n| n.get(None))
            .map(|n| n.as_str().to_owned());
        Ok(Identity {
            subject: claims.subject().as_str().to_owned(),
            email,
            name,
        })
    }
}

#[derive(Debug)]
pub struct Started {
    pub url: String,
    pub state: String,
    pub nonce: String,
    pub pkce_verifier: String,
}

#[derive(Debug, Clone)]
pub struct Identity {
    pub subject: String,
    pub email: String,
    pub name: Option<String>,
}
