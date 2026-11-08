use std::time::Duration;

use futures::{Stream, StreamExt};
use kaeso_update::manifest::Image;
use reqwest::{Method, StatusCode};
use serde_json::{Value, json};
use url::Url;

pub const API_VERSION: &str = "v1.44";
pub const PROJECT_LABEL: &str = "com.docker.compose.project";
pub const SERVICE_LABEL: &str = "com.docker.compose.service";
pub const ONEOFF_LABEL: &str = "com.docker.compose.oneoff";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed {
    pub id: String,
    pub name: String,
}
const MAX_BODY: usize = 16 << 20;

#[derive(Debug, thiserror::Error)]
pub enum DockerError {
    #[error("Docker cannot be reached ({0})")]
    Unreachable(&'static str),
    #[error("Docker refused to {what}: {status} {message}")]
    Refused {
        what: String,
        status: u16,
        message: String,
    },
    #[error("Docker's answer to {0} does not parse")]
    Parse(String),
    #[error("the updater refuses to {what}: {why}")]
    Forbidden {
        what: String,
        why: crate::guard::Refused,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timeouts {
    pub quick: Duration,
    pub slow: Duration,
    pub stop_grace: Duration,
    pub pull_idle: Duration,
}

impl Default for Timeouts {
    fn default() -> Timeouts {
        Timeouts {
            quick: Duration::from_secs(30),
            slow: Duration::from_secs(600),
            stop_grace: Duration::from_secs(15),
            pull_idle: Duration::from_secs(120),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Docker {
    http: reqwest::Client,
    base: String,
    timeouts: Timeouts,
}

fn kind(e: &reqwest::Error) -> &'static str {
    if e.is_timeout() {
        "timed out"
    } else if e.is_connect() {
        "no connection"
    } else {
        "request failed"
    }
}

fn encode(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

impl Docker {
    pub fn new(host: &Url) -> Result<Docker, reqwest::Error> {
        Docker::with_timeouts(host, Timeouts::default())
    }

    pub fn with_timeouts(host: &Url, timeouts: Timeouts) -> Result<Docker, reqwest::Error> {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let base = format!("{}/{API_VERSION}/", host.as_str().trim_end_matches('/'));
        Ok(Docker {
            http,
            base,
            timeouts,
        })
    }

    fn request(
        &self,
        method: Method,
        what: &str,
        path: &str,
    ) -> Result<reqwest::RequestBuilder, DockerError> {
        let url = Url::parse(&format!("{}{path}", self.base))
            .map_err(|_| DockerError::Parse(what.to_owned()))?;
        Ok(self.http.request(method, url))
    }

    async fn call(
        &self,
        method: Method,
        what: &str,
        path: &str,
        body: Option<&Value>,
        timeout: Duration,
    ) -> Result<(StatusCode, Vec<u8>), DockerError> {
        let mut req = self.request(method, what, path)?.timeout(timeout);
        if let Some(b) = body {
            let bytes = serde_json::to_vec(b).map_err(|_| DockerError::Parse(what.to_owned()))?;
            req = req
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(bytes);
        }
        let mut resp = req
            .send()
            .await
            .map_err(|e| DockerError::Unreachable(kind(&e)))?;
        let status = resp.status();
        let mut out = Vec::new();
        while let Some(chunk) = resp
            .chunk()
            .await
            .map_err(|e| DockerError::Unreachable(kind(&e)))?
        {
            if out.len() + chunk.len() > MAX_BODY {
                return Err(DockerError::Parse(what.to_owned()));
            }
            out.extend_from_slice(&chunk);
        }
        Ok((status, out))
    }

    async fn answered(
        &self,
        method: Method,
        what: &str,
        path: &str,
        body: Option<&Value>,
        ok: &[StatusCode],
        timeout: Duration,
    ) -> Result<Vec<u8>, DockerError> {
        let (status, out) = self.call(method, what, path, body, timeout).await?;
        if status.is_success() || ok.contains(&status) {
            return Ok(out);
        }
        Err(refused(what, status, &out))
    }

    fn quick(&self) -> Duration {
        self.timeouts.quick
    }

    fn slow(&self) -> Duration {
        self.timeouts.slow
    }

    fn json(what: &str, bytes: &[u8]) -> Result<Value, DockerError> {
        serde_json::from_slice(bytes).map_err(|_| DockerError::Parse(what.to_owned()))
    }

    pub async fn version(&self) -> Result<String, DockerError> {
        let out = self
            .answered(
                Method::GET,
                "report its version",
                "version",
                None,
                &[],
                self.quick(),
            )
            .await?;
        let v = Self::json("version", &out)?;
        Ok(v.get("Version")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned())
    }

    pub async fn containers(
        &self,
        project: &str,
        service: &str,
    ) -> Result<Vec<Listed>, DockerError> {
        let filters = json!({
            "label": [
                format!("{PROJECT_LABEL}={project}"),
                format!("{SERVICE_LABEL}={service}"),
                format!("{ONEOFF_LABEL}=False"),
            ]
        })
        .to_string();
        let path = format!("containers/json?all=true&filters={}", encode(&filters));
        let out = self
            .answered(
                Method::GET,
                "list containers",
                &path,
                None,
                &[],
                self.quick(),
            )
            .await?;
        Ok(listed(&Self::json("containers", &out)?))
    }

    pub async fn labeled(&self, label: &str) -> Result<Vec<Listed>, DockerError> {
        let filters = json!({ "label": [label] }).to_string();
        let path = format!("containers/json?all=true&filters={}", encode(&filters));
        let out = self
            .answered(
                Method::GET,
                "list containers",
                &path,
                None,
                &[],
                self.quick(),
            )
            .await?;
        Ok(listed(&Self::json("containers", &out)?))
    }

    pub async fn logs(&self, id: &str, tail: u32) -> Result<String, DockerError> {
        let out = self
            .answered(
                Method::GET,
                "read a container's logs",
                &format!(
                    "containers/{}/logs?stdout=true&stderr=true&tail={tail}",
                    encode(id)
                ),
                None,
                &[],
                self.quick(),
            )
            .await?;
        Ok(demultiplex(&out))
    }

    pub async fn create_runner(&self, spec: &Value) -> Result<String, DockerError> {
        crate::guard::runner(spec).map_err(|why| DockerError::Forbidden {
            what: "create the compose runner".into(),
            why,
        })?;
        let out = self
            .answered(
                Method::POST,
                "create a container",
                "containers/create",
                Some(spec),
                &[],
                self.slow(),
            )
            .await?;
        Self::json("created container", &out)?
            .get("Id")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| DockerError::Parse("created container".into()))
    }

    pub async fn inspect(&self, id: &str) -> Result<Value, DockerError> {
        let out = self
            .answered(
                Method::GET,
                "inspect a container",
                &format!("containers/{}/json", encode(id)),
                None,
                &[],
                self.quick(),
            )
            .await?;
        Self::json("container", &out)
    }

    pub async fn image(&self, reference: &str) -> Result<Option<Value>, DockerError> {
        let (status, out) = self
            .call(
                Method::GET,
                "inspect an image",
                &format!("images/{reference}/json"),
                None,
                self.quick(),
            )
            .await?;
        match status {
            StatusCode::NOT_FOUND => Ok(None),
            s if s.is_success() => Self::json("image", &out).map(Some),
            s => Err(DockerError::Refused {
                what: "inspect an image".into(),
                status: s.as_u16(),
                message: String::new(),
            }),
        }
    }

    pub async fn pull(&self, image: &Image) -> Result<(), DockerError> {
        let what = format!("pull {}", image.reference());
        crate::guard::image(image).map_err(|why| DockerError::Forbidden {
            what: what.clone(),
            why,
        })?;
        let path = format!(
            "images/create?fromImage={}&tag={}",
            encode(&image.name),
            encode(&image.digest)
        );
        let idle = self.timeouts.pull_idle;
        let req = self.request(Method::POST, &what, &path)?.send();
        let resp = tokio::time::timeout(idle, req)
            .await
            .map_err(|_| DockerError::Unreachable("timed out"))?
            .map_err(|e| DockerError::Unreachable(kind(&e)))?;
        let status = resp.status();
        if !status.is_success() {
            let out = tokio::time::timeout(idle, resp.bytes())
                .await
                .map_err(|_| DockerError::Unreachable("timed out"))??;
            return Err(refused(&what, status, &out));
        }
        pulled(&what, resp.bytes_stream(), idle).await
    }

    pub async fn tag(
        &self,
        reference: &str,
        repository: &str,
        tag: &str,
    ) -> Result<(), DockerError> {
        let path = format!(
            "images/{reference}/tag?repo={}&tag={}",
            encode(repository),
            encode(tag)
        );
        self.answered(Method::POST, "tag an image", &path, None, &[], self.slow())
            .await
            .map(|_| ())
    }

    pub async fn create(
        &self,
        name: &str,
        spec: &Value,
        replaced: &Value,
    ) -> Result<String, DockerError> {
        crate::guard::container(name, spec, replaced).map_err(|why| DockerError::Forbidden {
            what: format!("create {name}"),
            why,
        })?;
        let out = self
            .answered(
                Method::POST,
                "create a container",
                &format!("containers/create?name={}", encode(name)),
                Some(spec),
                &[],
                self.slow(),
            )
            .await?;
        Self::json("created container", &out)?
            .get("Id")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| DockerError::Parse("created container".into()))
    }

    pub async fn start(&self, id: &str) -> Result<(), DockerError> {
        self.answered(
            Method::POST,
            "start a container",
            &format!("containers/{}/start", encode(id)),
            None,
            &[StatusCode::NOT_MODIFIED],
            self.quick(),
        )
        .await
        .map(|_| ())
    }

    pub async fn stop(&self, id: &str, seconds: u32) -> Result<(), DockerError> {
        self.answered(
            Method::POST,
            "stop a container",
            &format!("containers/{}/stop?t={seconds}", encode(id)),
            None,
            &[StatusCode::NOT_MODIFIED],
            Duration::from_secs(u64::from(seconds)) + self.timeouts.stop_grace,
        )
        .await
        .map(|_| ())
    }

    pub async fn rename(&self, id: &str, name: &str) -> Result<(), DockerError> {
        self.answered(
            Method::POST,
            "rename a container",
            &format!("containers/{}/rename?name={}", encode(id), encode(name)),
            None,
            &[],
            self.quick(),
        )
        .await
        .map(|_| ())
    }

    pub async fn remove(&self, id: &str) -> Result<(), DockerError> {
        self.answered(
            Method::DELETE,
            "remove a container",
            &format!("containers/{}?force=true", encode(id)),
            None,
            &[StatusCode::NOT_FOUND],
            self.slow(),
        )
        .await
        .map(|_| ())
    }
}

fn listed(list: &Value) -> Vec<Listed> {
    list.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|c| {
                    let id = c.get("Id").and_then(Value::as_str)?.to_owned();
                    let name = c
                        .get("Names")
                        .and_then(Value::as_array)
                        .and_then(|n| n.first())
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .trim_start_matches('/')
                        .to_owned();
                    Some(Listed { id, name })
                })
                .collect()
        })
        .unwrap_or_default()
}

// Without a TTY, Docker frames each chunk of a log with an 8-byte header: the stream, three zero
// bytes and the length, big-endian (https://docs.docker.com/reference/api/engine/version/v1.44/#tag/Container/operation/ContainerAttach).
fn demultiplex(body: &[u8]) -> String {
    let mut text = Vec::with_capacity(body.len());
    let mut rest = body;
    while let Some((&[stream, 0, 0, 0, a, b, c, d], after)) = rest.split_first_chunk::<8>()
        && matches!(stream, 0..=2)
    {
        let len = u32::from_be_bytes([a, b, c, d]) as usize;
        let (frame, next) = after.split_at(len.min(after.len()));
        text.extend_from_slice(frame);
        rest = next;
    }
    text.extend_from_slice(rest);
    String::from_utf8_lossy(&text).into_owned()
}

fn refused(what: &str, status: StatusCode, body: &[u8]) -> DockerError {
    let message = serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|v| v.get("message").and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_default();
    DockerError::Refused {
        what: what.to_owned(),
        status: status.as_u16(),
        message,
    }
}

// Docker streams a pull's progress as it goes, so a pull may take as long as the images need
// while a registry or daemon that goes quiet for `idle` ends it.
async fn pulled<S, B, E>(what: &str, mut events: S, idle: Duration) -> Result<(), DockerError>
where
    S: Stream<Item = Result<B, E>> + Unpin,
    B: AsRef<[u8]>,
    E: Into<DockerError>,
{
    let mut line = Vec::new();
    loop {
        let next = tokio::time::timeout(idle, events.next())
            .await
            .map_err(|_| DockerError::Unreachable("timed out"))?;
        let Some(chunk) = next else {
            return pull_error(what, &line);
        };
        let chunk = chunk.map_err(Into::into)?;
        let mut parts = chunk.as_ref().split(|b| *b == b'\n');
        if let Some(first) = parts.next() {
            line.extend_from_slice(first);
        }
        for part in parts {
            pull_error(what, &line)?;
            line.clear();
            line.extend_from_slice(part);
        }
        if line.len() > MAX_BODY {
            return Err(DockerError::Parse(what.to_owned()));
        }
    }
}

fn pull_error(what: &str, line: &[u8]) -> Result<(), DockerError> {
    match serde_json::from_slice::<Value>(line)
        .ok()
        .as_ref()
        .and_then(|event| event.get("error"))
        .and_then(Value::as_str)
    {
        Some(message) => Err(DockerError::Refused {
            what: what.to_owned(),
            status: 200,
            message: message.to_owned(),
        }),
        None => Ok(()),
    }
}

impl From<reqwest::Error> for DockerError {
    fn from(e: reqwest::Error) -> DockerError {
        DockerError::Unreachable(kind(&e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Events = tokio::sync::mpsc::UnboundedSender<&'static [u8]>;

    fn events() -> (
        Events,
        impl Stream<Item = Result<&'static [u8], DockerError>> + Unpin,
    ) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<&'static [u8]>();
        let stream =
            futures::stream::unfold(
                rx,
                |mut rx| async move { rx.recv().await.map(|b| (Ok(b), rx)) },
            );
        (tx, Box::pin(stream))
    }

    #[test]
    fn logs_are_read_from_docker_frames_and_as_they_are_without_them() {
        let mut framed = vec![1, 0, 0, 0, 0, 0, 0, 6];
        framed.extend_from_slice(b"pulled");
        framed.extend_from_slice(&[2, 0, 0, 0, 0, 0, 0, 7]);
        framed.extend_from_slice(b"\nfailed");
        assert_eq!(demultiplex(&framed), "pulled\nfailed");
        assert_eq!(demultiplex(b"plain text\n"), "plain text\n");
        assert_eq!(
            demultiplex(&[1, 0, 0, 0, 0, 0, 0, 9, b'c', b'u', b't']),
            "cut"
        );
        assert_eq!(demultiplex(b""), "");
    }

    #[tokio::test(start_paused = true)]
    async fn a_pull_that_keeps_reporting_progress_takes_as_long_as_it_needs() {
        let (tx, stream) = events();
        let pull = tokio::spawn(pulled("pull", stream, Duration::from_secs(120)));
        for _ in 0..20 {
            tokio::time::advance(Duration::from_secs(100)).await;
            assert!(tx.send(b"{\"status\":\"Downloading\"}\n{\"sta").is_ok());
            assert!(tx.send(b"tus\":\"Extracting\"}\n").is_ok());
        }
        drop(tx);
        assert!(matches!(pull.await, Ok(Ok(()))));
    }

    #[tokio::test(start_paused = true)]
    async fn a_pull_that_goes_quiet_ends_after_the_idle_timeout() {
        let (tx, stream) = events();
        let started = tokio::time::Instant::now();
        let pull = tokio::spawn(pulled("pull", stream, Duration::from_secs(120)));
        tokio::time::advance(Duration::from_secs(60)).await;
        assert!(tx.send(b"{\"status\":\"Pulling\"}\n").is_ok());
        let ended = pull.await.unwrap();
        assert!(matches!(ended, Err(DockerError::Unreachable("timed out"))));
        assert_eq!(started.elapsed(), Duration::from_secs(180));
        drop(tx);
    }

    #[tokio::test(start_paused = true)]
    async fn an_error_in_the_progress_fails_the_pull_even_on_its_last_line() {
        for last in [
            &b"{\"status\":\"Pulling\"}\n{\"error\":\"denied\"}\n"[..],
            &b"{\"status\":\"Pulling\"}\n{\"error\":\"denied\"}"[..],
        ] {
            let (tx, stream) = events();
            assert!(tx.send(last).is_ok());
            drop(tx);
            let err = pulled("pull it", stream, Duration::from_secs(120))
                .await
                .unwrap_err();
            assert_eq!(err.to_string(), "Docker refused to pull it: 200 denied");
        }
    }
}
