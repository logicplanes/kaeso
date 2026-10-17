use std::sync::LazyLock;
use std::time::Duration;

use kaeso_update::protocol::{self, Job, StartRequest, Status};
use reqwest::StatusCode;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderValue};
use secrecy::{ExposeSecret, SecretString};
use url::Url;

use crate::state::AppState;

const MAX_BODY: usize = 64 * 1024;

static HTTP: LazyLock<Option<reqwest::Client>> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .connect_timeout(Duration::from_secs(2))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("kaeso-gateway/", env!("CARGO_PKG_VERSION")))
        .build()
        .inspect_err(|e| tracing::error!(error = %e, "cannot build the updater client"))
        .ok()
});

#[derive(Debug, thiserror::Error)]
pub enum UpdaterError {
    #[error("the updater cannot be reached: {0}")]
    Unreachable(String),
    #[error("the updater refused the token")]
    Unauthorized,
    #[error("the updater is busy with another update")]
    Busy,
    #[error("the updater answered {0}")]
    Status(u16),
    #[error("the updater's answer does not parse: {0}")]
    Protocol(String),
}

#[derive(Clone, Copy, Debug)]
pub struct Updater<'a> {
    url: &'a Url,
    token: &'a SecretString,
}

pub fn of(state: &AppState) -> Option<Updater<'_>> {
    state.config.updater.token.as_ref().map(|token| Updater {
        url: &state.config.updater.url,
        token,
    })
}

impl Updater<'_> {
    fn endpoint(&self, path: &str) -> Result<Url, UpdaterError> {
        self.url
            .join(path)
            .map_err(|e| UpdaterError::Unreachable(e.to_string()))
    }

    fn bearer(&self) -> Result<HeaderValue, UpdaterError> {
        let mut value = HeaderValue::from_str(&format!("Bearer {}", self.token.expose_secret()))
            .map_err(|_| UpdaterError::Unauthorized)?;
        value.set_sensitive(true);
        Ok(value)
    }

    async fn send(
        &self,
        req: reqwest::RequestBuilder,
    ) -> Result<(StatusCode, Vec<u8>), UpdaterError> {
        let mut resp = req
            .header(AUTHORIZATION, self.bearer()?)
            .send()
            .await
            .map_err(|e| UpdaterError::Unreachable(without_url(&e)))?;
        let status = resp.status();
        let mut body = Vec::new();
        while let Some(chunk) = resp
            .chunk()
            .await
            .map_err(|e| UpdaterError::Unreachable(without_url(&e)))?
        {
            if body.len() + chunk.len() > MAX_BODY {
                return Err(UpdaterError::Protocol("the answer is too large".into()));
            }
            body.extend_from_slice(&chunk);
        }
        Ok((status, body))
    }

    fn client() -> Result<&'static reqwest::Client, UpdaterError> {
        HTTP.as_ref()
            .ok_or_else(|| UpdaterError::Unreachable("no HTTP client".into()))
    }

    pub async fn status(&self) -> Result<Status, UpdaterError> {
        let (status, body) = self
            .send(Self::client()?.get(self.endpoint("v1/status")?))
            .await?;
        parse(status, &body)
    }

    pub async fn start(&self, req: &StartRequest) -> Result<Job, UpdaterError> {
        let body = serde_json::to_vec(req).map_err(|e| UpdaterError::Protocol(e.to_string()))?;
        let post = Self::client()?
            .post(self.endpoint("v1/updates")?)
            .header(CONTENT_TYPE, "application/json")
            .body(body);
        let (status, body) = self.send(post).await?;
        match status {
            StatusCode::CONFLICT => Err(UpdaterError::Busy),
            _ => parse(status, &body),
        }
    }

    pub async fn job(&self, id: &str) -> Result<Option<Job>, UpdaterError> {
        let (status, body) = self
            .send(Self::client()?.get(self.endpoint(&format!("v1/updates/{id}"))?))
            .await?;
        match status {
            StatusCode::NOT_FOUND => Ok(None),
            _ => parse(status, &body).map(Some),
        }
    }
}

fn parse<T: serde::de::DeserializeOwned>(
    status: StatusCode,
    body: &[u8],
) -> Result<T, UpdaterError> {
    match status {
        s if s.is_success() => {
            serde_json::from_slice(body).map_err(|e| UpdaterError::Protocol(e.to_string()))
        }
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(UpdaterError::Unauthorized),
        s => Err(UpdaterError::Status(s.as_u16())),
    }
}

fn without_url(e: &reqwest::Error) -> String {
    let kind = if e.is_timeout() {
        "timed out"
    } else if e.is_connect() {
        "connection refused or no route"
    } else {
        "request failed"
    };
    kind.to_owned()
}

pub fn speaks(status: &Status, required: u32) -> bool {
    status.protocol >= required && status.protocol >= protocol::VERSION
}
