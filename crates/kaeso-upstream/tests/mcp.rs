#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use kaeso_mcp::Version;
use kaeso_testkit::mcp::{Era, MockMcp, Options};
use kaeso_upstream::UpstreamError;
use kaeso_upstream::guard::{Lookup, Reach};
use kaeso_upstream::mcp::{Client, Limits, McpError, McpUpstream, SecretHeader};
use secrecy::SecretString;
use serde_json::json;
use url::Url;

const LIMITS: Limits = Limits {
    first_byte: Duration::from_secs(5),
    idle: Duration::from_secs(5),
    max_body: 1 << 20,
};

fn loopback_allowed() -> McpUpstream {
    McpUpstream::new(
        Reach::networks(vec!["127.0.0.0/8".parse().unwrap()]),
        Lookup::default(),
        Duration::from_secs(2),
        "kaeso/0.6.0",
    )
}

fn resolving_to(reach: Reach, addr: &'static str) -> McpUpstream {
    let lookup = Lookup::new(move |_, port| {
        let ip: IpAddr = addr.parse().unwrap();
        Box::pin(async move { Ok(vec![SocketAddr::new(ip, port)]) })
    });
    McpUpstream::new(reach, lookup, Duration::from_secs(2), "kaeso/0.6.0")
}

fn client<'a>(up: &'a McpUpstream, url: &'a Url) -> Client<'a> {
    Client {
        upstream: up,
        url,
        secret: None,
        limits: LIMITS,
    }
}

#[tokio::test]
async fn a_modern_server_is_spoken_to_without_a_session() {
    let mock = MockMcp::start(Options {
        era: Era::Modern,
        ..Options::default()
    })
    .await;
    let up = loopback_allowed();
    let url = Url::parse(&mock.url()).unwrap();
    let c = client(&up, &url);
    let hs = c.handshake().await.unwrap();
    assert_eq!(hs.version, Version::V2026_07_28);
    assert_eq!(hs.session, None);
    assert_eq!(hs.server_info.as_ref().unwrap()["name"], "mock-x");
    assert_eq!(hs.instructions.as_deref(), Some("Posts to X."));
    let tools = c.list_tools(&hs).await.unwrap();
    assert_eq!(tools.len(), 3);
    assert_eq!(tools[0]["name"], "create_post");
    let seen = mock.seen();
    assert_eq!(seen[0].method, "server/discover");
    assert_eq!(seen[0].protocol_version.as_deref(), Some("2026-07-28"));
    assert_eq!(seen[0].mcp_method.as_deref(), Some("server/discover"));
    let list = &seen[1];
    assert_eq!(list.mcp_method.as_deref(), Some("tools/list"));
    assert_eq!(list.session, None);
    let meta = &list.body["params"]["_meta"];
    assert_eq!(
        meta["io.modelcontextprotocol/protocolVersion"],
        "2026-07-28"
    );
    assert_eq!(meta["io.modelcontextprotocol/clientInfo"]["name"], "kaeso");
    assert_eq!(
        meta["io.modelcontextprotocol/clientInfo"]["version"],
        "0.6.0"
    );
}

#[tokio::test]
async fn an_older_server_gets_initialize_and_its_session() {
    let mock = MockMcp::start(Options {
        era: Era::Legacy,
        page_size: 2,
        ..Options::default()
    })
    .await;
    let up = loopback_allowed();
    let url = Url::parse(&mock.url()).unwrap();
    let c = client(&up, &url);
    let hs = c.handshake().await.unwrap();
    assert_eq!(hs.version, Version::V2025_11_25);
    let session = hs.session.clone().unwrap();
    assert_eq!(c.list_tools(&hs).await.unwrap().len(), 3);
    let seen = mock.seen();
    let methods: Vec<&str> = seen.iter().map(|s| s.method.as_str()).collect();
    assert_eq!(
        methods,
        [
            "server/discover",
            "initialize",
            "notifications/initialized",
            "tools/list",
            "tools/list"
        ]
    );
    assert_eq!(seen[1].protocol_version, None);
    for later in &seen[2..] {
        assert_eq!(later.session.as_deref(), Some(session.as_str()));
        assert_eq!(later.protocol_version.as_deref(), Some("2025-11-25"));
        assert!(later.body["params"].get("_meta").is_none());
    }
    assert_eq!(seen[4].body["params"]["cursor"], "2");

    mock.end_sessions();
    let gone = c.request(&hs, "tools/list", None, "after").await;
    assert!(matches!(gone, Err(McpError::SessionGone)), "{gone:?}");
}

#[tokio::test]
async fn a_server_of_both_eras_is_spoken_to_in_the_newest() {
    let mock = MockMcp::start(Options::default()).await;
    let up = loopback_allowed();
    let url = Url::parse(&mock.url()).unwrap();
    let hs = client(&up, &url).handshake().await.unwrap();
    assert_eq!(hs.version, Version::V2026_07_28);
    assert_eq!(mock.seen().len(), 1);
}

#[tokio::test]
async fn a_streamed_answer_is_read_past_its_progress() {
    let mock = MockMcp::start(Options {
        stream_calls: true,
        ..Options::default()
    })
    .await;
    let up = loopback_allowed();
    let url = Url::parse(&mock.url()).unwrap();
    let c = client(&up, &url);
    let hs = c.handshake().await.unwrap();
    let result = c
        .request(
            &hs,
            "tools/call",
            Some(json!({ "name": "create_post", "arguments": { "text": "hi" } })),
            "call-1",
        )
        .await
        .unwrap();
    let result: serde_json::Value = serde_json::from_slice(&result).unwrap();
    assert_eq!(
        result["content"][0]["text"],
        r#"create_post ran with {"text":"hi"}"#
    );
    assert_eq!(result["resultType"], "complete");
    let call = mock
        .seen()
        .into_iter()
        .find(|s| s.method == "tools/call")
        .unwrap();
    assert_eq!(call.mcp_name.as_deref(), Some("create_post"));
    let unknown = c
        .request(&hs, "tools/call", Some(json!({ "name": "nope" })), "call-2")
        .await;
    assert!(
        matches!(unknown, Err(McpError::Rpc { code: -32602, .. })),
        "{unknown:?}"
    );
}

#[tokio::test]
async fn a_server_that_wants_sign_in_says_where() {
    let mock = MockMcp::start(Options {
        token: Some("upstream-token".to_owned()),
        ..Options::default()
    })
    .await;
    let up = loopback_allowed();
    let url = Url::parse(&mock.url()).unwrap();
    let refused = client(&up, &url).handshake().await;
    let Err(McpError::Unauthorized { status, challenge }) = refused else {
        panic!("{refused:?}")
    };
    assert_eq!(status, 401);
    assert!(
        challenge
            .unwrap()
            .contains("/.well-known/oauth-protected-resource/mcp")
    );
    let token = SecretString::from("Bearer upstream-token");
    let signed_in = Client {
        secret: Some(SecretHeader {
            name: "authorization",
            value: &token,
        }),
        ..client(&up, &url)
    };
    let hs = signed_in.handshake().await.unwrap();
    assert_eq!(signed_in.list_tools(&hs).await.unwrap().len(), 3);
    assert!(mock.seen().iter().all(|s| {
        s.authorization
            .as_deref()
            .is_none_or(|a| a == "Bearer upstream-token")
    }));
}

fn refused(e: &McpError) -> bool {
    matches!(e, McpError::Transport(UpstreamError::Refused(_)))
}

#[tokio::test]
async fn private_and_loopback_servers_are_refused_without_the_setting() {
    let mock = MockMcp::start(Options::default()).await;
    let public_only = McpUpstream::new(
        Reach::public_only(),
        Lookup::default(),
        Duration::from_secs(2),
        "kaeso/0.6.0",
    );
    let port = mock.addr.port();
    for url in [
        mock.url(),
        format!("http://[::ffff:127.0.0.1]:{port}/mcp"),
        format!("http://[::1]:{port}/mcp"),
        "http://10.0.0.5/mcp".to_owned(),
        "http://192.168.1.20/mcp".to_owned(),
    ] {
        let url = Url::parse(&url).unwrap();
        let e = client(&public_only, &url).handshake().await.unwrap_err();
        assert!(refused(&e), "{url}: {e:?}");
    }
    for (addr, reach) in [
        ("10.0.0.5", Reach::public_only()),
        ("127.0.0.1", Reach::public_only()),
        ("::ffff:10.0.0.5", Reach::public_only()),
        ("169.254.169.254", Reach::public_only()),
        (
            "169.254.169.254",
            Reach::networks(vec!["0.0.0.0/0".parse().unwrap()]),
        ),
        ("fe80::1", Reach::networks(vec!["::/0".parse().unwrap()])),
        ("fd00:ec2::254", Reach::every_private_network()),
    ] {
        let up = resolving_to(reach, addr);
        let url = Url::parse("https://mcp.example.com/mcp").unwrap();
        let e = client(&up, &url).handshake().await.unwrap_err();
        assert!(refused(&e), "{addr}: {e:?}");
    }
}

#[tokio::test]
async fn link_local_and_metadata_literals_stay_refused_with_every_network_allowed() {
    let everything = McpUpstream::new(
        Reach::networks(vec!["0.0.0.0/0".parse().unwrap(), "::/0".parse().unwrap()]),
        Lookup::default(),
        Duration::from_secs(2),
        "kaeso/0.6.0",
    );
    for url in [
        "http://169.254.169.254/latest",
        "http://[fe80::1]/mcp",
        "http://100.100.100.200/mcp",
    ] {
        let url = Url::parse(url).unwrap();
        let e = client(&everything, &url).handshake().await.unwrap_err();
        assert!(refused(&e), "{url}: {e:?}");
    }
}

#[tokio::test]
async fn a_redirect_is_never_followed() {
    let mock = MockMcp::start(Options::default()).await;
    let up = loopback_allowed();
    let url = Url::parse(&mock.moved_url()).unwrap();
    let e = client(&up, &url).handshake().await.unwrap_err();
    assert!(matches!(e, McpError::Status(307)), "{e:?}");
}
