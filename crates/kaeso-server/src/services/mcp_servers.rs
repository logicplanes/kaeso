use std::time::Duration;

use kaeso_mcp::names;
use kaeso_upstream::UpstreamError;
use kaeso_upstream::guard;
use kaeso_upstream::mcp::{Client, Limits, McpError, SecretHeader};
use secrecy::SecretString;
use serde_json::Value;
use url::Url;

use crate::services::custom_providers::Reachability;
use crate::services::mcp_connections::{self, SignInError};
use crate::state::AppState;

pub const MAX_NAME: usize = 100;
pub const MAX_SCOPES: usize = 30;
const MAX_URL: usize = 2048;
const MAX_SCOPE: usize = 200;
const PROBE: Limits = Limits {
    first_byte: Duration::from_secs(15),
    idle: Duration::from_secs(15),
    max_body: 4 * 1024 * 1024,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UrlProblem {
    Length,
    Scheme,
    Host,
    Credentials,
    Fragment,
}

pub fn parse_url(raw: &str) -> Result<Url, UrlProblem> {
    let raw = raw.trim();
    if raw.is_empty() || raw.len() > MAX_URL {
        return Err(UrlProblem::Length);
    }
    let url = Url::parse(raw).map_err(|_| UrlProblem::Host)?;
    if !matches!(url.scheme(), "https" | "http") {
        return Err(UrlProblem::Scheme);
    }
    if url.host_str().is_none_or(str::is_empty) {
        return Err(UrlProblem::Host);
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(UrlProblem::Credentials);
    }
    if url.fragment().is_some() {
        return Err(UrlProblem::Fragment);
    }
    Ok(url)
}

pub fn clean_name(raw: &str) -> Option<String> {
    let name = raw.trim();
    (!name.is_empty() && name.chars().count() <= MAX_NAME && !name.chars().any(char::is_control))
        .then(|| name.to_owned())
}

pub fn valid_scopes(scopes: &[String]) -> bool {
    scopes.len() <= MAX_SCOPES
        && scopes.iter().all(|s| {
            !s.is_empty()
                && s.len() <= MAX_SCOPE
                && s.bytes()
                    .all(|b| b.is_ascii_graphic() && b != b'"' && b != b'\\')
        })
}

pub fn suggest_handle(from: &str) -> String {
    let mut out = String::new();
    for c in from.chars().flat_map(char::to_lowercase) {
        if out.len() >= names::MAX_HANDLE {
            break;
        }
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-');
    if out.is_empty() {
        "server".to_owned()
    } else {
        out.to_owned()
    }
}

pub async fn reachability(state: &AppState, url: &Url) -> Reachability {
    let (Some(host), Some(port)) = (url.host_str(), url.port_or_known_default()) else {
        return Reachability::Refused;
    };
    let reach = state.mcp.reach_for(url);
    match guard::resolve(host, port, &reach, state.mcp.lookup()).await {
        Ok(_) => Reachability::Allowed,
        Err(guard::ResolveError::Refused(_)) => Reachability::Refused,
        Err(guard::ResolveError::Lookup { .. }) => Reachability::Unknown,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeProblem {
    Refused,
    Unreachable,
    Timeout,
    Status(u16),
    NotMcp,
    HeaderRefused,
    SignIn,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OAuthOffer {
    pub issuer: String,
    pub client_metadata_documents: bool,
    pub registration: bool,
    pub scopes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SignIn {
    None,
    Header,
    OAuth(OAuthOffer),
}

#[derive(Debug, Default)]
pub struct Probe {
    pub problem: Option<ProbeProblem>,
    pub protocol_version: Option<&'static str>,
    pub server_info: Option<Value>,
    pub instructions: Option<String>,
    pub sign_in: Option<SignIn>,
    pub tools: Option<Vec<Value>>,
}

fn problem_of(e: &McpError) -> ProbeProblem {
    match e {
        McpError::Transport(UpstreamError::Refused(_)) => ProbeProblem::Refused,
        McpError::Transport(UpstreamError::Timeout(_)) => ProbeProblem::Timeout,
        McpError::Transport(_) => ProbeProblem::Unreachable,
        McpError::Status(s) | McpError::Unauthorized { status: s, .. } => ProbeProblem::Status(*s),
        McpError::Rpc { .. } | McpError::Protocol(_) | McpError::SessionGone => {
            ProbeProblem::NotMcp
        }
    }
}

// Nothing a probe learns is stored: it only tells the person what adding the server would take.
pub async fn probe(state: &AppState, url: &Url, header: Option<(&str, &SecretString)>) -> Probe {
    let client = Client {
        upstream: &state.mcp,
        url,
        secret: header.map(|(name, value)| SecretHeader { name, value }),
        limits: PROBE,
    };
    let mut out = Probe::default();
    match client.handshake().await {
        Ok(hs) => {
            out.protocol_version = Some(hs.version.as_str());
            out.server_info.clone_from(&hs.server_info);
            out.instructions.clone_from(&hs.instructions);
            out.sign_in = Some(if header.is_some() {
                SignIn::Header
            } else {
                SignIn::None
            });
            match client.list_tools(&hs).await {
                Ok(tools) => out.tools = Some(tools),
                Err(e) => out.problem = Some(problem_of(&e)),
            }
        }
        Err(McpError::Unauthorized { challenge, .. }) if header.is_none() => {
            match mcp_connections::discover(state, url, challenge.as_deref()).await {
                Ok(auth) => {
                    out.sign_in = Some(SignIn::OAuth(OAuthOffer {
                        issuer: auth.issuer,
                        client_metadata_documents: auth.cimd,
                        registration: auth.registration_endpoint.is_some(),
                        scopes: auth.scopes,
                    }));
                }
                Err(SignInError::NoAuthServer) => out.sign_in = Some(SignIn::Header),
                Err(e) => {
                    tracing::info!(error = %e, "an MCP server's sign-in can't be used");
                    out.problem = Some(ProbeProblem::SignIn);
                }
            }
        }
        Err(McpError::Unauthorized { .. }) => {
            out.sign_in = Some(SignIn::Header);
            out.problem = Some(ProbeProblem::HeaderRefused);
        }
        Err(e) => {
            tracing::info!(error = %e, "an MCP server couldn't be probed");
            out.problem = Some(problem_of(&e));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_are_suggested_from_names() {
        assert_eq!(suggest_handle("X (Twitter)"), "x-twitter");
        assert_eq!(suggest_handle("  Linear  "), "linear");
        assert_eq!(suggest_handle("Ünïcode only ✓"), "n-code-only");
        assert_eq!(suggest_handle("!!!"), "server");
        let long = suggest_handle("A very long name for a server somewhere");
        assert!(
            long.len() <= names::MAX_HANDLE && names::valid_handle(&long),
            "{long}"
        );
    }

    #[test]
    fn addresses_need_a_host_and_no_credentials() {
        assert!(parse_url("https://mcp.example.com/mcp?x=1").is_ok());
        assert_eq!(
            parse_url("ftp://x.example").unwrap_err(),
            UrlProblem::Scheme
        );
        assert_eq!(parse_url("mcp.example.com").unwrap_err(), UrlProblem::Host);
        assert_eq!(
            parse_url("https://u:p@mcp.example.com").unwrap_err(),
            UrlProblem::Credentials
        );
        assert_eq!(
            parse_url("https://mcp.example.com/#x").unwrap_err(),
            UrlProblem::Fragment
        );
        assert_eq!(parse_url(" ").unwrap_err(), UrlProblem::Length);
        assert_eq!(
            parse_url(&format!("https://e.example/{}", "a".repeat(2048))).unwrap_err(),
            UrlProblem::Length
        );
    }

    #[test]
    fn names_and_scopes_are_checked() {
        assert_eq!(clean_name("  X  ").as_deref(), Some("X"));
        assert!(clean_name("").is_none());
        assert!(clean_name("a\nb").is_none());
        assert!(clean_name(&"a".repeat(101)).is_none());
        assert!(valid_scopes(&["tweet.write".into(), "users.read".into()]));
        assert!(!valid_scopes(&["two words".into()]));
        assert!(!valid_scopes(&[String::new()]));
        assert!(!valid_scopes(&vec!["s".to_owned(); 31]));
    }
}
