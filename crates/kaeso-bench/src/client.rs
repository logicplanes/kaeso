use std::net::SocketAddr;

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::client::conn::http1::SendRequest;
use hyper::header::{AUTHORIZATION, CONTENT_TYPE, COOKIE, HOST, HeaderMap, ORIGIN};
use hyper::{Method, Request, Response};
use hyper_util::rt::TokioIo;
use tokio::net::TcpSocket;
use tokio::task::JoinHandle;

use crate::Result;

pub(crate) struct Conn {
    sender: SendRequest<Full<Bytes>>,
    task: JoinHandle<()>,
    addr: SocketAddr,
}

impl Drop for Conn {
    fn drop(&mut self) {
        self.task.abort();
    }
}

// Every client closes with a reset instead of a FIN: a run opens tens of thousands of
// connections, and the TIME_WAIT a FIN leaves would use up the ephemeral ports.
pub(crate) async fn connect(addr: SocketAddr, recv_buffer: Option<u32>) -> Result<Conn> {
    let socket = TcpSocket::new_v4()?;
    if let Some(size) = recv_buffer {
        socket.set_recv_buffer_size(size)?;
    }
    socket.set_zero_linger()?;
    let stream = socket.connect(addr).await?;
    stream.set_nodelay(true)?;
    let (sender, conn) = hyper::client::conn::http1::handshake(TokioIo::new(stream)).await?;
    let task = tokio::spawn(async move {
        let _ = conn.await;
    });
    Ok(Conn { sender, task, addr })
}

pub(crate) struct Call<'a> {
    pub(crate) method: Method,
    pub(crate) path: &'a str,
    pub(crate) bearer: Option<&'a str>,
    pub(crate) origin: Option<&'a str>,
    pub(crate) cookie: Option<&'a str>,
    pub(crate) body: Option<Bytes>,
}

impl Conn {
    pub(crate) async fn send(&mut self, call: Call<'_>) -> Result<Response<Incoming>> {
        let mut req = Request::builder()
            .method(call.method)
            .uri(call.path)
            .header(HOST, self.addr.to_string());
        if let Some(key) = call.bearer {
            req = req.header(AUTHORIZATION, format!("Bearer {key}"));
        }
        if let Some(origin) = call.origin {
            req = req.header(ORIGIN, origin);
        }
        if let Some(cookie) = call.cookie {
            req = req.header(COOKIE, cookie);
        }
        let body = match call.body {
            Some(b) => {
                req = req.header(CONTENT_TYPE, "application/json");
                b
            }
            None => Bytes::new(),
        };
        self.sender.ready().await?;
        Ok(self.sender.send_request(req.body(Full::new(body))?).await?)
    }
}

pub(crate) async fn read_all(resp: Response<Incoming>) -> Result<(u16, HeaderMap, Bytes)> {
    let status = resp.status().as_u16();
    let (parts, body) = resp.into_parts();
    let bytes = body.collect().await?.to_bytes();
    Ok((status, parts.headers, bytes))
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Timing {
    pub(crate) kaeso_ms: Option<f64>,
    pub(crate) upstream_ms: Option<f64>,
}

pub(crate) fn server_timing(headers: &HeaderMap) -> Timing {
    let mut timing = Timing::default();
    for value in headers.get_all("server-timing") {
        let Ok(text) = value.to_str() else { continue };
        for metric in text.split(',') {
            let mut parts = metric.trim().split(';');
            let name = parts.next().unwrap_or_default().trim();
            let dur = parts
                .filter_map(|p| p.trim().strip_prefix("dur="))
                .find_map(|d| d.parse::<f64>().ok());
            match name {
                "kaeso" => timing.kaeso_ms = dur,
                "upstream" => timing.upstream_ms = dur,
                _ => {}
            }
        }
    }
    timing
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_timing_is_read_by_metric_name() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "server-timing",
            "kaeso;dur=0.412, upstream;dur=153.2"
                .parse()
                .unwrap_or_else(|_| unreachable!()),
        );
        assert_eq!(
            server_timing(&headers),
            Timing {
                kaeso_ms: Some(0.412),
                upstream_ms: Some(153.2)
            }
        );
        headers.insert(
            "server-timing",
            "kaeso;desc=\"x\";dur=2"
                .parse()
                .unwrap_or_else(|_| unreachable!()),
        );
        assert_eq!(server_timing(&headers).kaeso_ms, Some(2.0));
        assert_eq!(server_timing(&headers).upstream_ms, None);
    }
}
