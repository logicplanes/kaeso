use std::sync::Arc;
use std::time::Duration;

use kaeso_store::StoreError;
use kaeso_store::pg::agents::{self, Client};
use kaeso_upstream::UpstreamError;
use kaeso_upstream::mcp::Limits;
use serde::Deserialize;
use url::Url;
use uuid::Uuid;

use super::oauth::redirect_uri_allowed;
use crate::state::AppState;

// Client ID Metadata Documents are a few hundred bytes; the cap keeps a client from making Kaeso
// download much on someone else's behalf.
const LIMITS: Limits = Limits {
    first_byte: Duration::from_secs(5),
    idle: Duration::from_secs(5),
    max_body: 5 * 1024,
};
const MAX_URL: usize = 2048;
const MAX_NAME: usize = 100;
const MAX_REDIRECT_URIS: usize = 10;

#[derive(Debug, Deserialize)]
struct Document {
    client_id: String,
    #[serde(default)]
    client_name: Option<String>,
    #[serde(default)]
    redirect_uris: Vec<String>,
    #[serde(default)]
    token_endpoint_auth_method: Option<String>,
    #[serde(default)]
    software_id: Option<String>,
    #[serde(default)]
    software_version: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    Address,
    Unreachable,
    Document,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Described {
    pub name: String,
    pub redirect_uris: Vec<String>,
    pub software_id: Option<String>,
    pub software_version: Option<String>,
}

pub fn is_document_url(client_id: &str) -> bool {
    client_id.starts_with("https://")
}

// draft-ietf-oauth-client-id-metadata-document §3: an https URL with a path, without a fragment,
// credentials or dot segments, compared as written.
pub fn valid_url(raw: &str) -> Option<Url> {
    if raw.len() > MAX_URL || raw.contains("/./") || raw.contains("/../") {
        return None;
    }
    let url = Url::parse(raw).ok()?;
    let fine = url.scheme() == "https"
        && url.host_str().is_some()
        && url.path() != "/"
        && url.fragment().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && url.as_str() == raw;
    fine.then_some(url)
}

fn clean(s: Option<String>, max: usize) -> Option<String> {
    s.map(|s| {
        s.trim()
            .chars()
            .filter(|c| !c.is_control())
            .take(max)
            .collect::<String>()
    })
    .filter(|s| !s.is_empty())
}

pub fn describe(url: &Url, body: &[u8]) -> Result<Described, Refused> {
    let d: Document = serde_json::from_slice(body).map_err(|_| Refused::Document)?;
    // The document must name itself, so a page can't pass off another client's document as its own.
    if d.client_id != url.as_str() {
        return Err(Refused::Document);
    }
    if d.redirect_uris.is_empty()
        || d.redirect_uris.len() > MAX_REDIRECT_URIS
        || !d.redirect_uris.iter().all(|u| redirect_uri_allowed(u))
    {
        return Err(Refused::Document);
    }
    // Kaeso takes public clients only; a document asking for a secret or a signed assertion names
    // a way to authenticate it can't check.
    if d.token_endpoint_auth_method
        .as_deref()
        .is_some_and(|m| m != "none")
    {
        return Err(Refused::Document);
    }
    Ok(Described {
        name: clean(d.client_name, MAX_NAME)
            .unwrap_or_else(|| url.host_str().unwrap_or_default().to_owned()),
        redirect_uris: d.redirect_uris,
        software_id: clean(d.software_id, MAX_NAME),
        software_version: clean(d.software_version, MAX_NAME),
    })
}

pub async fn remember(
    state: &AppState,
    url: &str,
    described: Described,
) -> Result<Client, StoreError> {
    let now = state.clock.now();
    let client = agents::put_metadata_client(
        &state.pg,
        url,
        &Client {
            id: Uuid::now_v7(),
            name: described.name,
            redirect_uris: described.redirect_uris,
            software_id: described.software_id,
            software_version: described.software_version,
            created_at: now,
        },
        now,
    )
    .await?;
    state
        .caches
        .client_documents
        .insert(url.to_owned(), Arc::new(client.clone()))
        .await;
    Ok(client)
}

// Every fetch goes through the same reach guard as MCP servers, so a client id can't point Kaeso
// at its own network.
pub async fn client(state: &AppState, raw: &str) -> Result<Result<Client, Refused>, StoreError> {
    let Some(url) = valid_url(raw) else {
        return Ok(Err(Refused::Document));
    };
    if let Some(c) = state.caches.client_documents.get(raw).await {
        if agents::get_client(&state.pg, c.id).await?.is_some() {
            return Ok(Ok((*c).clone()));
        }
        // Registrations without a sign-in are trimmed by count, and a document's row counts among
        // them until someone approves its client; the document read minutes ago still describes it.
        return remember(
            state,
            raw,
            Described {
                name: c.name.clone(),
                redirect_uris: c.redirect_uris.clone(),
                software_id: c.software_id.clone(),
                software_version: c.software_version.clone(),
            },
        )
        .await
        .map(Ok);
    }
    let (status, body) = match state.public_mcp.get_json(&url, LIMITS).await {
        Ok(answer) => answer,
        Err(UpstreamError::Refused(_)) => return Ok(Err(Refused::Address)),
        Err(e) => {
            tracing::info!(error = %e, "a client metadata document couldn't be fetched");
            return Ok(Err(Refused::Unreachable));
        }
    };
    if status != 200 {
        return Ok(Err(Refused::Unreachable));
    }
    let described = match describe(&url, &body) {
        Ok(d) => d,
        Err(r) => return Ok(Err(r)),
    };
    remember(state, raw, described).await.map(Ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    const URL: &str = "https://claude.example/oauth/mcp-client-metadata.json";

    fn url() -> Url {
        Url::parse(URL).unwrap_or_else(|_| unreachable!())
    }

    #[test]
    fn only_plain_https_urls_with_a_path_name_a_client() {
        assert!(valid_url(URL).is_some());
        for bad in [
            "http://claude.example/client.json",
            "https://claude.example/",
            "https://claude.example/client.json#x",
            "https://me:pw@claude.example/client.json",
            "https://claude.example/a/../client.json",
            "https://CLAUDE.example/client.json",
        ] {
            assert!(valid_url(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn a_document_must_name_itself_and_a_public_client() {
        let good = format!(
            r#"{{"client_id":"{URL}","client_name":"Claude","redirect_uris":["https://claude.example/api/mcp/auth_callback"]}}"#
        );
        let described = describe(&url(), good.as_bytes());
        assert_eq!(described.map(|d| d.name), Ok("Claude".to_owned()));
        for bad in [
            r#"{"client_id":"https://other.example/client.json","redirect_uris":["https://claude.example/cb"]}"#.to_owned(),
            format!(r#"{{"client_id":"{URL}","redirect_uris":[]}}"#),
            format!(r#"{{"client_id":"{URL}","redirect_uris":["javascript:alert(1)"]}}"#),
            format!(
                r#"{{"client_id":"{URL}","redirect_uris":["https://claude.example/cb"],"token_endpoint_auth_method":"private_key_jwt"}}"#
            ),
            "not json".to_owned(),
        ] {
            assert_eq!(describe(&url(), bad.as_bytes()), Err(Refused::Document), "{bad}");
        }
        let unnamed =
            format!(r#"{{"client_id":"{URL}","redirect_uris":["https://claude.example/cb"]}}"#);
        assert_eq!(
            describe(&url(), unnamed.as_bytes()).map(|d| d.name),
            Ok("claude.example".to_owned())
        );
    }
}
