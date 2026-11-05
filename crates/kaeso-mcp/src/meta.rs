use serde::Deserialize;

use crate::raw::RawObject;

pub const KEY: &str = "_meta";
pub const PROTOCOL_VERSION: &str = "io.modelcontextprotocol/protocolVersion";
pub const CLIENT_CAPABILITIES: &str = "io.modelcontextprotocol/clientCapabilities";
pub const CLIENT_INFO: &str = "io.modelcontextprotocol/clientInfo";
pub const SERVER_INFO: &str = "io.modelcontextprotocol/serverInfo";
pub const LOG_LEVEL: &str = "io.modelcontextprotocol/logLevel";
pub const SUBSCRIPTION_ID: &str = "io.modelcontextprotocol/subscriptionId";
pub const TRACEPARENT: &str = "traceparent";
pub const TRACESTATE: &str = "tracestate";
pub const BAGGAGE: &str = "baggage";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Implementation {
    pub name: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
}

pub fn protocol_version(meta: &RawObject<'_>) -> Option<String> {
    meta.string(PROTOCOL_VERSION)
}

pub fn client_info(meta: &RawObject<'_>) -> Option<Implementation> {
    meta.get(CLIENT_INFO)
        .and_then(|raw| serde_json::from_str(raw.get()).ok())
}

pub fn has_client_capability(meta: &RawObject<'_>, capability: &str) -> bool {
    meta.get(CLIENT_CAPABILITIES)
        .and_then(|raw| RawObject::of(raw).ok())
        .is_some_and(|caps| caps.contains(capability))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_what_a_stateless_request_carries() {
        let body = br#"{
            "io.modelcontextprotocol/protocolVersion": "2026-07-28",
            "io.modelcontextprotocol/clientInfo": { "name": "claude-ai", "title": "Claude", "version": "1.0", "icons": [] },
            "io.modelcontextprotocol/clientCapabilities": { "elicitation": {} },
            "traceparent": "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01"
        }"#;
        let meta = RawObject::parse(body).unwrap();
        assert_eq!(protocol_version(&meta).as_deref(), Some("2026-07-28"));
        assert_eq!(
            client_info(&meta),
            Some(Implementation {
                name: "claude-ai".to_owned(),
                title: Some("Claude".to_owned()),
                version: Some("1.0".to_owned()),
            })
        );
        assert!(has_client_capability(&meta, "elicitation"));
        assert!(!has_client_capability(&meta, "sampling"));
        assert!(meta.contains(TRACEPARENT));
    }

    #[test]
    fn missing_or_malformed_values_read_as_absent() {
        let meta = RawObject::parse(br#"{"io.modelcontextprotocol/clientInfo": "x", "io.modelcontextprotocol/protocolVersion": 1}"#).unwrap();
        assert_eq!(protocol_version(&meta), None);
        assert_eq!(client_info(&meta), None);
        assert!(!has_client_capability(&meta, "elicitation"));
    }
}
