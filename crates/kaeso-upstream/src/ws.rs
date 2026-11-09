use std::sync::{Arc, LazyLock};
use std::time::Duration;

use secrecy::{ExposeSecret, SecretString};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use url::Url;

pub use tokio_tungstenite::tungstenite::Message;
pub use tokio_tungstenite::tungstenite::protocol::CloseFrame;
pub use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;

/// Any byte stream a provider's socket runs over: plain TCP in tests, TLS otherwise.
pub trait Io: AsyncRead + AsyncWrite + Unpin + Send + Sync {}

impl<T: AsyncRead + AsyncWrite + Unpin + Send + Sync> Io for T {}

pub type Socket = WebSocketStream<Box<dyn Io>>;

#[derive(Debug, thiserror::Error)]
pub enum WsError {
    #[error("cannot reach the provider")]
    Connect(#[source] std::io::Error),
    #[error("the provider did not answer in time")]
    Timeout,
    #[error("the TLS handshake with the provider failed")]
    Tls(#[source] std::io::Error),
    #[error("cannot set up TLS for providers")]
    Setup(#[source] rustls::Error),
    #[error("the provider's address is not a WebSocket address")]
    Address,
    /// The provider answered the upgrade with an HTTP response instead of a socket.
    #[error("the provider refused the socket with HTTP {status}")]
    Refused { status: u16, body: Vec<u8> },
    #[error("the WebSocket handshake with the provider failed")]
    Handshake(#[source] Box<tokio_tungstenite::tungstenite::Error>),
}

/// How a provider's socket is opened.
#[derive(Debug)]
pub struct Dial<'a> {
    /// `wss://` or, for tests and local servers, `ws://`.
    pub url: &'a Url,
    /// The auth header and its value; marked sensitive so it never reaches a log.
    pub auth: Option<(&'static str, &'a SecretString)>,
    pub user_agent: &'a str,
    pub connect_timeout: Duration,
    /// The largest message either side may send, in bytes.
    pub max_message: usize,
}

static TLS: LazyLock<Result<Arc<rustls::ClientConfig>, rustls::Error>> = LazyLock::new(|| {
    use rustls_platform_verifier::BuilderVerifierExt as _;
    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let mut config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .with_platform_verifier()?
        .with_no_client_auth();
    // A WebSocket upgrade is an HTTP/1.1 request (RFC 6455, section 4.1).
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(Arc::new(config))
});

/// Opens a WebSocket to a built-in provider, with the provider's key in a header. The
/// provider's own base URL is the operator's setting, so it isn't held to the guard's reach.
pub async fn dial(d: Dial<'_>) -> Result<Socket, WsError> {
    let host = d.url.host_str().ok_or(WsError::Address)?.to_owned();
    let tls = match d.url.scheme() {
        "wss" => true,
        "ws" => false,
        _ => return Err(WsError::Address),
    };
    let port = d.url.port_or_known_default().ok_or(WsError::Address)?;
    let connecting = async {
        let tcp = TcpStream::connect((host.trim_matches(['[', ']']), port))
            .await
            .map_err(WsError::Connect)?;
        let _ = tcp.set_nodelay(true);
        let io: Box<dyn Io> = if tls {
            let config = TLS.as_ref().map_err(|e| WsError::Setup(e.clone()))?.clone();
            let name = rustls_pki_types::ServerName::try_from(host.clone())
                .map_err(|_| WsError::Address)?;
            let stream = tokio_rustls::TlsConnector::from(config)
                .connect(name, tcp)
                .await
                .map_err(WsError::Tls)?;
            Box::new(stream)
        } else {
            Box::new(tcp)
        };
        Ok::<_, WsError>(io)
    };
    let io = tokio::time::timeout(d.connect_timeout, connecting)
        .await
        .map_err(|_| WsError::Timeout)??;
    let mut request = d
        .url
        .as_str()
        .into_client_request()
        .map_err(|e| WsError::Handshake(Box::new(e)))?;
    let headers = request.headers_mut();
    if let Ok(v) = HeaderValue::from_str(d.user_agent) {
        headers.insert("user-agent", v);
    }
    if let Some((name, secret)) = d.auth
        && let Ok(mut v) = HeaderValue::from_str(secret.expose_secret())
    {
        v.set_sensitive(true);
        headers.insert(name, v);
    }
    let config = WebSocketConfig::default()
        .max_message_size(Some(d.max_message))
        .max_frame_size(Some(d.max_message));
    let handshake = tokio_tungstenite::client_async_with_config(request, io, Some(config));
    match tokio::time::timeout(d.connect_timeout, handshake).await {
        Err(_) => Err(WsError::Timeout),
        Ok(Ok((socket, _))) => Ok(socket),
        Ok(Err(tokio_tungstenite::tungstenite::Error::Http(resp))) => {
            let status = resp.status().as_u16();
            let body = resp.into_body().unwrap_or_default();
            Err(WsError::Refused { status, body })
        }
        Ok(Err(e)) => Err(WsError::Handshake(Box::new(e))),
    }
}

#[cfg(test)]
mod tests {
    use futures::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;

    use super::*;

    #[tokio::test]
    async fn the_key_goes_in_its_header_and_messages_flow_both_ways() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut seen = None;
            let mut ws = tokio_tungstenite::accept_hdr_async(
                tcp,
                |req: &tokio_tungstenite::tungstenite::handshake::server::Request, resp| {
                    seen = req
                        .headers()
                        .get("xi-api-key")
                        .map(|v| v.to_str().unwrap().to_owned());
                    Ok(resp)
                },
            )
            .await
            .unwrap();
            let msg = ws.next().await.unwrap().unwrap();
            ws.send(msg).await.unwrap();
            seen
        });
        let url = Url::parse(&format!("ws://{addr}/v1/speech-to-text/realtime")).unwrap();
        let key = SecretString::from("sk_test");
        let mut socket = dial(Dial {
            url: &url,
            auth: Some(("xi-api-key", &key)),
            user_agent: "kaeso-test",
            connect_timeout: Duration::from_secs(5),
            max_message: 1024,
        })
        .await
        .unwrap();
        socket.send(Message::text("hi")).await.unwrap();
        assert_eq!(socket.next().await.unwrap().unwrap(), Message::text("hi"));
        assert_eq!(server.await.unwrap().as_deref(), Some("sk_test"));
    }

    #[tokio::test]
    async fn an_http_answer_to_the_upgrade_is_a_refusal_with_its_status() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let (mut tcp, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 2048];
            let _ = tcp.read(&mut buf).await;
            let _ = tcp
                .write_all(b"HTTP/1.1 401 Unauthorized\r\ncontent-length: 2\r\n\r\nno")
                .await;
        });
        let url = Url::parse(&format!("ws://{addr}/")).unwrap();
        let err = dial(Dial {
            url: &url,
            auth: None,
            user_agent: "kaeso-test",
            connect_timeout: Duration::from_secs(5),
            max_message: 1024,
        })
        .await
        .err()
        .unwrap();
        assert!(
            matches!(err, WsError::Refused { status: 401, .. }),
            "{err:?}"
        );
        let ftp = Url::parse("ftp://example.com/").unwrap();
        assert!(matches!(
            dial(Dial {
                url: &ftp,
                auth: None,
                user_agent: "kaeso-test",
                connect_timeout: Duration::from_secs(1),
                max_message: 1024,
            })
            .await
            .err(),
            Some(WsError::Address)
        ));
    }
}
