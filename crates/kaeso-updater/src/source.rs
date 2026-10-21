use std::time::Duration;

use kaeso_update::Version;
use kaeso_update::manifest::Image;
use reqwest::StatusCode;
use reqwest::header::{ACCEPT, AUTHORIZATION};
use serde_json::Value;
use sha2::{Digest, Sha256};
use url::Url;

use crate::verify::{VerifyError, hex};

const MAX_ATTESTATIONS: usize = 4 << 20;
const MAX_INDEX: usize = 1 << 20;
const INDEX_TYPES: &str = "application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.list.v2+json, application/vnd.oci.image.manifest.v1+json, application/vnd.docker.distribution.manifest.v2+json";
const REGISTRY_HOST: &str = "ghcr.io";

#[derive(Clone, Debug)]
pub struct Endpoints {
    pub releases: Url,
    pub attestations: Url,
    pub registry: Url,
}

impl Endpoints {
    pub fn github() -> Result<Endpoints, url::ParseError> {
        Ok(Endpoints {
            releases: Url::parse(kaeso_update::DOWNLOADS)?,
            attestations: Url::parse(
                "https://api.github.com/repos/logicplanes/kaeso/attestations/",
            )?,
            registry: Url::parse("https://ghcr.io/")?,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("{0} cannot be fetched ({1})")]
    Unreachable(String, &'static str),
    #[error("{0} answered {1}")]
    Status(String, u16),
    #[error("{0} is larger than expected")]
    TooLarge(String),
    #[error("{0} does not match its digest")]
    Mismatch(String),
    #[error("{0} is not a valid address")]
    Address(String),
}

// "sha256:…" alone would be read as an absolute URL with the scheme `sha256`, so the
// digest joins as a relative path.
fn attestations_url(endpoints: &Endpoints, sha256: &str) -> Result<Url, url::ParseError> {
    endpoints.attestations.join(&format!("./sha256:{sha256}"))
}

#[derive(Clone, Debug)]
pub struct Source {
    http: reqwest::Client,
    endpoints: Endpoints,
}

fn kind(e: &reqwest::Error) -> &'static str {
    if e.is_timeout() {
        "timed out"
    } else if e.is_connect() {
        "no connection"
    } else if e.is_redirect() {
        "redirected away from HTTPS"
    } else {
        "request failed"
    }
}

impl Source {
    pub fn new(endpoints: Endpoints) -> Result<Source, reqwest::Error> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .connect_timeout(Duration::from_secs(10))
            .user_agent(concat!("kaeso-updater/", env!("CARGO_PKG_VERSION")))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                let downgrade = attempt.url().scheme() != "https"
                    && attempt.previous().iter().any(|u| u.scheme() == "https");
                if downgrade || attempt.previous().len() > 5 {
                    attempt.stop()
                } else {
                    attempt.follow()
                }
            }))
            .build()?;
        Ok(Source { http, endpoints })
    }

    async fn get(
        &self,
        what: &str,
        req: reqwest::RequestBuilder,
        max: usize,
    ) -> Result<(StatusCode, Vec<u8>), SourceError> {
        let mut resp = req
            .send()
            .await
            .map_err(|e| SourceError::Unreachable(what.to_owned(), kind(&e)))?;
        let status = resp.status();
        let mut body = Vec::new();
        while let Some(chunk) = resp
            .chunk()
            .await
            .map_err(|e| SourceError::Unreachable(what.to_owned(), kind(&e)))?
        {
            if body.len() + chunk.len() > max {
                return Err(SourceError::TooLarge(what.to_owned()));
            }
            body.extend_from_slice(&chunk);
        }
        Ok((status, body))
    }

    pub async fn asset(
        &self,
        version: &Version,
        name: &str,
        max: usize,
    ) -> Result<Vec<u8>, SourceError> {
        let what = format!("{name} of {version}");
        let url = self
            .endpoints
            .releases
            .join(&format!("v{version}/{name}"))
            .map_err(|_| SourceError::Address(what.clone()))?;
        let (status, body) = self
            .get(
                &what,
                self.http
                    .get(url)
                    .header(ACCEPT, "application/octet-stream"),
                max,
            )
            .await?;
        if !status.is_success() {
            return Err(SourceError::Status(what, status.as_u16()));
        }
        Ok(body)
    }

    pub async fn attestations(&self, sha256: &str) -> Result<Vec<u8>, VerifyError> {
        let what = format!("sha256:{sha256}");
        let fetch = |e: SourceError| VerifyError::Fetch(what.clone(), e.to_string());
        let url = attestations_url(&self.endpoints, sha256)
            .map_err(|_| fetch(SourceError::Address(what.clone())))?;
        let req = self
            .http
            .get(url)
            .header(ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");
        let (status, body) = self
            .get(&what, req, MAX_ATTESTATIONS)
            .await
            .map_err(fetch)?;
        match status {
            StatusCode::NOT_FOUND => Err(VerifyError::Missing(what)),
            s if s.is_success() => Ok(body),
            s => Err(fetch(SourceError::Status(what.clone(), s.as_u16()))),
        }
    }

    pub async fn index(&self, image: &Image) -> Result<Vec<u8>, SourceError> {
        let what = image.reference();
        let repository = image
            .name
            .strip_prefix(REGISTRY_HOST)
            .and_then(|r| r.strip_prefix('/'))
            .ok_or_else(|| SourceError::Address(what.clone()))?;
        let mut token_url = self
            .endpoints
            .registry
            .join("token")
            .map_err(|_| SourceError::Address(what.clone()))?;
        token_url
            .query_pairs_mut()
            .append_pair("scope", &format!("repository:{repository}:pull"))
            .append_pair("service", REGISTRY_HOST);
        let (status, body) = self.get(&what, self.http.get(token_url), 64 << 10).await?;
        if !status.is_success() {
            return Err(SourceError::Status(
                format!("the registry token for {what}"),
                status.as_u16(),
            ));
        }
        let token = serde_json::from_slice::<Value>(&body)
            .ok()
            .and_then(|v| v.get("token").and_then(Value::as_str).map(str::to_owned))
            .ok_or_else(|| {
                SourceError::Status(format!("the registry token for {what}"), status.as_u16())
            })?;
        let url = self
            .endpoints
            .registry
            .join(&format!("v2/{repository}/manifests/{}", image.digest))
            .map_err(|_| SourceError::Address(what.clone()))?;
        let req = self
            .http
            .get(url)
            .header(ACCEPT, INDEX_TYPES)
            .header(AUTHORIZATION, format!("Bearer {token}"));
        let (status, body) = self.get(&what, req, MAX_INDEX).await?;
        if !status.is_success() {
            return Err(SourceError::Status(what, status.as_u16()));
        }
        if hex(&Sha256::digest(&body)) != image.hex() {
            return Err(SourceError::Mismatch(what));
        }
        Ok(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attestations_are_asked_for_on_the_repository_by_digest() {
        let url = attestations_url(&Endpoints::github().unwrap(), "ebafa65").unwrap();
        assert_eq!(
            url.as_str(),
            "https://api.github.com/repos/logicplanes/kaeso/attestations/sha256:ebafa65"
        );
        let joined = Endpoints::github()
            .unwrap()
            .attestations
            .join("sha256:ebafa65")
            .unwrap();
        assert_eq!(joined.scheme(), "sha256", "why the digest needs `./`");
    }
}
