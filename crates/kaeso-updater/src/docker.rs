use std::time::Duration;

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

#[derive(Clone, Debug)]
pub struct Docker {
    http: reqwest::Client,
    base: String,
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
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(600))
            .connect_timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let base = format!("{}/{API_VERSION}/", host.as_str().trim_end_matches('/'));
        Ok(Docker { http, base })
    }

    async fn call(
        &self,
        method: Method,
        what: &str,
        path: &str,
        body: Option<&Value>,
    ) -> Result<(StatusCode, Vec<u8>), DockerError> {
        let url = Url::parse(&format!("{}{path}", self.base))
            .map_err(|_| DockerError::Parse(what.to_owned()))?;
        let mut req = self.http.request(method, url);
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
    ) -> Result<Vec<u8>, DockerError> {
        let (status, out) = self.call(method, what, path, body).await?;
        if status.is_success() || ok.contains(&status) {
            return Ok(out);
        }
        let message = serde_json::from_slice::<Value>(&out)
            .ok()
            .and_then(|v| v.get("message").and_then(Value::as_str).map(str::to_owned))
            .unwrap_or_default();
        Err(DockerError::Refused {
            what: what.to_owned(),
            status: status.as_u16(),
            message,
        })
    }

    fn json(what: &str, bytes: &[u8]) -> Result<Value, DockerError> {
        serde_json::from_slice(bytes).map_err(|_| DockerError::Parse(what.to_owned()))
    }

    pub async fn version(&self) -> Result<String, DockerError> {
        let out = self
            .answered(Method::GET, "report its version", "version", None, &[])
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
            .answered(Method::GET, "list containers", &path, None, &[])
            .await?;
        let list = Self::json("containers", &out)?;
        Ok(list
            .as_array()
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
            .unwrap_or_default())
    }

    pub async fn inspect(&self, id: &str) -> Result<Value, DockerError> {
        let out = self
            .answered(
                Method::GET,
                "inspect a container",
                &format!("containers/{}/json", encode(id)),
                None,
                &[],
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
        let out = self.answered(Method::POST, &what, &path, None, &[]).await?;
        for line in out.split(|b| *b == b'\n').filter(|l| !l.is_empty()) {
            if let Ok(event) = serde_json::from_slice::<Value>(line)
                && let Some(message) = event.get("error").and_then(Value::as_str)
            {
                return Err(DockerError::Refused {
                    what,
                    status: 200,
                    message: message.to_owned(),
                });
            }
        }
        Ok(())
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
        self.answered(Method::POST, "tag an image", &path, None, &[])
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
        )
        .await
        .map(|_| ())
    }
}
