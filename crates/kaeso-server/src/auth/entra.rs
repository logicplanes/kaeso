use std::time::Duration;

use openidconnect::http::{self, header};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use url::Url;

use super::oidc::{FetchError, fetch};

/// A Microsoft Entra ID cloud: the host its issuers live on and its Microsoft Graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cloud {
    pub login: String,
    pub graph: String,
}

// The national clouds' sign-in and Graph hosts: https://learn.microsoft.com/en-us/graph/deployments
pub fn clouds() -> Vec<Cloud> {
    [
        (
            "https://login.microsoftonline.com",
            "https://graph.microsoft.com",
        ),
        (
            "https://login.microsoftonline.us",
            "https://graph.microsoft.us",
        ),
        (
            "https://login.partner.microsoftonline.cn",
            "https://microsoftgraph.chinacloudapi.cn",
        ),
    ]
    .into_iter()
    .map(|(login, graph)| Cloud {
        login: login.to_owned(),
        graph: graph.to_owned(),
    })
    .collect()
}

/// The Microsoft Graph of the Entra cloud an issuer belongs to; `None` for any other provider.
pub fn graph_for<'a>(clouds: &'a [Cloud], issuer: &str) -> Option<&'a str> {
    let issuer = Url::parse(issuer).ok()?;
    let origin = issuer.origin();
    clouds
        .iter()
        .find(|c| Url::parse(&c.login).is_ok_and(|l| l.origin() == origin))
        .map(|c| c.graph.as_str())
}

const MAX_PAGES: usize = 50;
const BUDGET: Duration = Duration::from_secs(15);

#[derive(Deserialize)]
struct Page {
    #[serde(default)]
    value: Vec<Member>,
    #[serde(rename = "@odata.nextLink")]
    next: Option<String>,
}

#[derive(Deserialize)]
struct Member {
    id: String,
    #[serde(rename = "@odata.type")]
    kind: Option<String>,
}

/// The object ids of every group the signed-in person is in, directly or through other groups,
/// read with the access token of their sign-in. `User.Read` is enough:
/// https://learn.microsoft.com/en-us/graph/api/user-list-transitivememberof
pub async fn member_groups(
    allow_private: bool,
    graph: &str,
    token: &SecretString,
) -> Result<Vec<String>, FetchError> {
    tokio::time::timeout(BUDGET, pages(allow_private, graph, token))
        .await
        .map_err(|_| FetchError::new("Microsoft Graph took too long"))?
}

async fn pages(
    allow_private: bool,
    graph: &str,
    token: &SecretString,
) -> Result<Vec<String>, FetchError> {
    let graph = graph.trim_end_matches('/');
    // `$select` and `$top` work without advanced queries, whose index can lag behind changes:
    // https://learn.microsoft.com/en-us/graph/aad-advanced-queries
    let mut url = format!("{graph}/v1.0/me/transitiveMemberOf?$select=id&$top=999");
    let mut groups = Vec::new();
    for _ in 0..MAX_PAGES {
        let request = http::Request::get(&url)
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", token.expose_secret()),
            )
            .header(header::ACCEPT, "application/json")
            .body(Vec::new())
            .map_err(|e| FetchError::new(e.to_string()))?;
        let response = fetch(allow_private, request).await?;
        if !response.status().is_success() {
            return Err(FetchError::new(format!(
                "Microsoft Graph answered {}",
                response.status()
            )));
        }
        let page: Page = serde_json::from_slice(response.body())
            .map_err(|e| FetchError::new(format!("Microsoft Graph's answer: {e}")))?;
        groups.extend(
            page.value
                .into_iter()
                .filter(|m| {
                    m.kind
                        .as_deref()
                        .is_none_or(|k| k == "#microsoft.graph.group")
                })
                .map(|m| m.id),
        );
        match page.next {
            None => return Ok(groups),
            // The next request carries the person's token, so it may only go to the same Graph.
            Some(next) if next.starts_with(&format!("{graph}/")) => url = next,
            Some(_) => {
                return Err(FetchError::new(
                    "Microsoft Graph pointed the next page elsewhere",
                ));
            }
        }
    }
    Err(FetchError::new(format!(
        "more than {MAX_PAGES} pages of memberships"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_entra_issuers_have_a_graph() {
        let clouds = clouds();
        assert_eq!(
            graph_for(
                &clouds,
                "https://login.microsoftonline.com/00000000-0000-0000-0000-000000000000/v2.0"
            ),
            Some("https://graph.microsoft.com")
        );
        assert_eq!(
            graph_for(
                &clouds,
                "https://login.microsoftonline.us/00000000-0000-0000-0000-000000000000/v2.0"
            ),
            Some("https://graph.microsoft.us")
        );
        assert_eq!(graph_for(&clouds, "https://id.example.com"), None);
        assert_eq!(
            graph_for(&clouds, "https://login.microsoftonline.com.example.com/x"),
            None
        );
        assert_eq!(graph_for(&clouds, "not a url"), None);
    }
}
