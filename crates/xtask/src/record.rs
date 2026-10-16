use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::Value;
use time::OffsetDateTime;
use time::macros::format_description;

use kaeso_codecs::{
    anthropic_messages, is_forwarded_header, openai_chat, openai_embeddings, openai_responses,
};
use kaeso_kpp::DialectId;

use crate::error::{Error, read, read_text, write};
use crate::json::pretty;
use crate::signed_urls::strip_signed_queries;

const FIXTURES: &str = "crates/kaeso-codecs/fixtures";
const SKIP_PREFIXES: &[&str] = &["stateful-", "unknown-", "n-2", "input-audio"];
const TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
#[value(rename_all = "snake_case")]
pub(crate) enum Dialect {
    AnthropicMessages,
    OpenaiChat,
    OpenaiResponses,
    OpenaiEmbeddings,
}

impl Dialect {
    fn id(self) -> DialectId {
        match self {
            Self::AnthropicMessages => DialectId::AnthropicMessages,
            Self::OpenaiChat => DialectId::OpenAiChat,
            Self::OpenaiResponses => DialectId::OpenAiResponses,
            Self::OpenaiEmbeddings => DialectId::OpenAiEmbeddings,
        }
    }

    fn folder(self) -> &'static str {
        self.id().as_str()
    }

    fn is_anthropic(self) -> bool {
        self == Self::AnthropicMessages
    }

    fn path(self) -> &'static str {
        match self {
            Self::AnthropicMessages => anthropic_messages::MESSAGES_PATH,
            Self::OpenaiChat => openai_chat::CHAT_COMPLETIONS_PATH,
            Self::OpenaiResponses => openai_responses::RESPONSES_PATH,
            Self::OpenaiEmbeddings => openai_embeddings::EMBEDDINGS_PATH,
        }
    }

    fn count(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::AnthropicMessages => {
                Some((anthropic_messages::COUNT_TOKENS_PATH, "count_tokens"))
            }
            Self::OpenaiResponses => Some((openai_responses::INPUT_TOKENS_PATH, "input_tokens")),
            Self::OpenaiChat | Self::OpenaiEmbeddings => None,
        }
    }
}

#[derive(clap::Args)]
pub(crate) struct Args {
    #[arg(value_enum, help = "The dialect whose request fixtures are sent")]
    dialect: Dialect,
    #[arg(help = "Request fixture names, without .json")]
    names: Vec<String>,
    #[arg(long, help = "Record every request fixture of the dialect")]
    all: bool,
    #[arg(long, help = "Use the token-count endpoint and its fixtures")]
    count: bool,
    #[arg(long, help = "Print what would be sent and send nothing")]
    dry_run: bool,
}

impl Args {
    pub(crate) fn dialect(&self) -> Dialect {
        self.dialect
    }
}

pub(crate) struct Endpoint {
    base_url: String,
    key_variable: &'static str,
    key: Option<String>,
}

impl Endpoint {
    pub(crate) fn from_env(dialect: Dialect) -> Self {
        let (variable, default, key_variable) = if dialect.is_anthropic() {
            (
                "ANTHROPIC_BASE_URL",
                "https://api.anthropic.com",
                "ANTHROPIC_API_KEY",
            )
        } else {
            (
                "OPENAI_BASE_URL",
                "https://api.openai.com",
                "OPENAI_API_KEY",
            )
        };
        Self {
            base_url: std::env::var(variable).unwrap_or_else(|_| default.to_owned()),
            key_variable,
            key: std::env::var(key_variable).ok().filter(|k| !k.is_empty()),
        }
    }
}

struct Recorder<'a> {
    root: &'a Path,
    fixtures: PathBuf,
    dialect: Dialect,
    count: Option<(&'static str, &'static str)>,
    url: String,
    key: Option<&'a str>,
    client: reqwest::Client,
}

pub(crate) async fn run(
    root: &Path,
    args: &Args,
    endpoint: &Endpoint,
    out: &mut impl Write,
) -> Result<(), Error> {
    let count = match (args.count, args.dialect.count()) {
        (false, _) => None,
        (true, Some(count)) => Some(count),
        (true, None) => {
            return Err(Error::Invalid(format!(
                "{} has no token-count endpoint",
                args.dialect.folder()
            )));
        }
    };
    if endpoint.key.is_none() && !args.dry_run {
        return Err(Error::Invalid(format!(
            "{} is not set",
            endpoint.key_variable
        )));
    }
    let fixtures = root.join(FIXTURES);
    let mut folder = fixtures.join(args.dialect.folder()).join("requests");
    if let Some((_, sub)) = count {
        folder.push(sub);
    }
    let selected = select(request_fixtures(&folder)?, &args.names, args.all)?;
    let recorder = Recorder {
        root,
        fixtures,
        dialect: args.dialect,
        count,
        url: format!(
            "{}{}",
            endpoint.base_url.trim_end_matches('/'),
            count.map_or(args.dialect.path(), |(path, _)| path)
        ),
        key: endpoint.key.as_deref().filter(|_| !args.dry_run),
        client: reqwest::Client::builder().timeout(TIMEOUT).build()?,
    };
    for fixture in selected {
        recorder.record(&fixture, out).await?;
    }
    Ok(())
}

fn request_fixtures(folder: &Path) -> Result<Vec<PathBuf>, Error> {
    let entries = std::fs::read_dir(folder).map_err(|source| Error::File {
        path: folder.to_owned(),
        source,
    })?;
    let mut found: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension().is_some_and(|e| e == "json")
                && !p.to_string_lossy().ends_with(".headers.json")
                && !p.to_string_lossy().ends_with(".meta.json")
        })
        .collect();
    found.sort();
    Ok(found)
}

fn stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn select(available: Vec<PathBuf>, names: &[String], all: bool) -> Result<Vec<PathBuf>, Error> {
    if all {
        return Ok(available
            .into_iter()
            .filter(|p| !SKIP_PREFIXES.iter().any(|s| stem(p).starts_with(s)))
            .collect());
    }
    if names.is_empty() {
        return Err(Error::Invalid(
            "name at least one fixture, or pass --all".to_owned(),
        ));
    }
    let wanted: BTreeSet<&str> = names.iter().map(String::as_str).collect();
    let chosen: Vec<PathBuf> = available
        .into_iter()
        .filter(|p| wanted.contains(stem(p).as_str()))
        .collect();
    let known: BTreeSet<String> = chosen.iter().map(|p| stem(p)).collect();
    let missing: Vec<&str> = wanted.into_iter().filter(|w| !known.contains(*w)).collect();
    if missing.is_empty() {
        Ok(chosen)
    } else {
        Err(Error::Invalid(format!(
            "unknown fixtures: {}",
            missing.join(", ")
        )))
    }
}

impl Recorder<'_> {
    fn relative<'p>(&self, path: &'p Path) -> &'p Path {
        path.strip_prefix(self.root).unwrap_or(path)
    }

    async fn record(&self, fixture: &Path, out: &mut impl Write) -> Result<(), Error> {
        let body = read(fixture)?;
        let payload: Value = serde_json::from_slice(&body)
            .map_err(|e| Error::json(self.relative(fixture).display(), e))?;
        let model = match payload.get("model") {
            Some(Value::String(m)) => m.clone(),
            Some(other) => other.to_string(),
            None => "?".to_owned(),
        };
        let streaming = payload.get("stream").is_some_and(truthy);
        writeln!(
            out,
            "{} -> POST {}{}",
            self.relative(fixture).display(),
            self.url,
            if streaming { " (stream)" } else { "" }
        )?;
        let Some(key) = self.key else {
            return Ok(());
        };
        let response = self
            .client
            .post(&self.url)
            .headers(self.headers(fixture, key)?)
            .body(body)
            .send()
            .await?;
        let status = response.status();
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let received = response.bytes().await?;
        let data = strip_signed_queries(&received);
        let name = stem(fixture);
        let base = self.fixtures.join(self.dialect.folder());
        let (folder, file) = if !status.is_success() {
            let folder = base.join("errors");
            let file = match pretty(&data) {
                Ok(text) => {
                    let file = folder.join(format!("recorded-{name}.json"));
                    write(&file, text)?;
                    file
                }
                Err(_) => {
                    let file = folder.join(format!("recorded-{name}.txt"));
                    write(&file, &data)?;
                    file
                }
            };
            write(
                &folder.join(format!("recorded-{name}.meta.json")),
                meta(status.as_u16(), retry_after.as_deref())?,
            )?;
            (folder, file)
        } else if streaming {
            let folder = base.join("streams");
            let file = folder.join(format!("recorded-{name}.sse"));
            write(&file, &data)?;
            (folder, file)
        } else {
            let folder = match self.count {
                Some((_, sub)) => base.join("responses").join(sub),
                None => base.join("responses"),
            };
            let file = folder.join(format!("recorded-{name}.json"));
            let text = pretty(&data).map_err(|e| Error::json("the response", e))?;
            write(&file, text)?;
            (folder, file)
        };
        self.provenance(&folder, &file, fixture, &model)?;
        writeln!(out, "  wrote {}", self.relative(&file).display())?;
        Ok(())
    }

    fn headers(&self, fixture: &Path, key: &str) -> Result<HeaderMap, Error> {
        let mut headers = vec![("content-type".to_owned(), "application/json".to_owned())];
        if self.dialect.is_anthropic() {
            headers.push(("x-api-key".to_owned(), key.to_owned()));
            headers.push((
                "anthropic-version".to_owned(),
                anthropic_messages::ANTHROPIC_VERSION.to_owned(),
            ));
        } else {
            headers.push(("authorization".to_owned(), format!("Bearer {key}")));
        }
        let sidecar = fixture.with_file_name(format!("{}.headers.json", stem(fixture)));
        if sidecar.exists() {
            let extra: serde_json::Map<String, Value> = serde_json::from_slice(&read(&sidecar)?)
                .map_err(|e| Error::json(self.relative(&sidecar).display(), e))?;
            for (name, value) in extra {
                let name = name.to_ascii_lowercase();
                if is_forwarded_header(self.dialect.id(), &name) {
                    let value = value.as_str().ok_or_else(|| {
                        Error::Invalid(format!(
                            "{}: `{name}` is not a string",
                            self.relative(&sidecar).display()
                        ))
                    })?;
                    headers.push((name, value.to_owned()));
                }
            }
        }
        let mut map = HeaderMap::new();
        for (name, value) in headers {
            let header = HeaderName::try_from(name.as_str())
                .map_err(|e| Error::Invalid(format!("header `{name}`: {e}")))?;
            let value = HeaderValue::try_from(value)
                .map_err(|e| Error::Invalid(format!("header `{name}`: {e}")))?;
            map.insert(header, value);
        }
        Ok(map)
    }

    fn provenance(
        &self,
        folder: &Path,
        file: &Path,
        fixture: &Path,
        model: &str,
    ) -> Result<(), Error> {
        let index = folder.join("RECORDED.md");
        let mut text = if index.exists() {
            read_text(&index)?
        } else {
            "# Recorded fixtures\n\nWritten by `cargo xtask record`. Provenance: live API.\n\n| File | Request fixture | Model | Recorded (UTC) |\n|---|---|---|---|\n".to_owned()
        };
        let when = OffsetDateTime::now_utc()
            .format(format_description!("[year]-[month]-[day] [hour]:[minute]"))?;
        let name = file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let request = fixture.strip_prefix(&self.fixtures).unwrap_or(fixture);
        text.push_str(&format!(
            "| `{name}` | `{}` | `{model}` | {when} |\n",
            request.display()
        ));
        write(&index, text)
    }
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

fn meta(status: u16, retry_after: Option<&str>) -> Result<String, Error> {
    let headers = match retry_after {
        Some(value) => format!(
            ",\"headers\":{{\"retry-after\":{}}}",
            Value::String(value.to_owned())
        ),
        None => String::new(),
    };
    pretty(format!("{{\"status\":{status}{headers}}}").as_bytes())
        .map_err(|e| Error::json("the error metadata", e))
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;
    use crate::testing::{Reply, TempDir, serve_once};

    #[derive(Parser)]
    struct Cli {
        #[command(flatten)]
        args: Args,
    }

    fn args(line: &[&str]) -> Args {
        Cli::try_parse_from(std::iter::once("record").chain(line.iter().copied()))
            .unwrap()
            .args
    }

    fn repository() -> TempDir {
        let dir = TempDir::new();
        let requests = "crates/kaeso-codecs/fixtures/anthropic_messages/requests";
        dir.file(
            &format!("{requests}/text-simple.json"),
            r#"{"model":"claude-haiku-4-5","max_tokens":10,"messages":[]}"#,
        );
        dir.file(
            &format!("{requests}/text-stream.json"),
            r#"{"model":"claude-haiku-4-5","stream":true}"#,
        );
        dir.file(
            &format!("{requests}/text-stream.headers.json"),
            r#"{"Anthropic-Beta":"interleaved-thinking-2025-05-14","x-ignored":"no"}"#,
        );
        dir.file(
            &format!("{requests}/unknown-field.json"),
            r#"{"model":"m"}"#,
        );
        dir.file(
            &format!("{requests}/count_tokens/basic.json"),
            r#"{"model":"claude-haiku-4-5"}"#,
        );
        dir.file(
            "crates/kaeso-codecs/fixtures/openai_chat/requests/basic.json",
            r#"{"model":"gpt-4o"}"#,
        );
        dir
    }

    fn endpoint(base_url: &str, key: Option<&str>) -> Endpoint {
        Endpoint {
            base_url: base_url.to_owned(),
            key_variable: "ANTHROPIC_API_KEY",
            key: key.map(str::to_owned),
        }
    }

    #[test]
    fn dialect_names_and_endpoints() {
        assert_eq!(
            args(&["openai_responses", "--all"]).dialect(),
            Dialect::OpenaiResponses
        );
        assert_eq!(Dialect::OpenaiChat.path(), "/v1/chat/completions");
        assert_eq!(Dialect::OpenaiEmbeddings.path(), "/v1/embeddings");
        assert_eq!(Dialect::OpenaiEmbeddings.folder(), "openai_embeddings");
        assert_eq!(
            Dialect::OpenaiResponses.count(),
            Some(("/v1/responses/input_tokens", "input_tokens"))
        );
        assert!(Cli::try_parse_from(["record", "openai"]).is_err());
        let openai = Endpoint::from_env(Dialect::OpenaiChat);
        assert_eq!(openai.key_variable, "OPENAI_API_KEY");
        assert_eq!(
            Endpoint::from_env(Dialect::AnthropicMessages).key_variable,
            "ANTHROPIC_API_KEY"
        );
    }

    #[test]
    fn selection_by_name_or_all() {
        let available: Vec<PathBuf> = ["a.json", "stateful-x.json", "n-2.json", "b.json"]
            .iter()
            .map(PathBuf::from)
            .collect();
        let all = select(available.clone(), &[], true).unwrap();
        assert_eq!(all, [PathBuf::from("a.json"), PathBuf::from("b.json")]);
        let named = select(
            available.clone(),
            &["b".to_owned(), "n-2".to_owned()],
            false,
        )
        .unwrap();
        assert_eq!(named, [PathBuf::from("n-2.json"), PathBuf::from("b.json")]);
        let missing = select(available.clone(), &["c".to_owned(), "a".to_owned()], false);
        assert_eq!(missing.unwrap_err().to_string(), "unknown fixtures: c");
        assert!(select(available, &[], false).is_err());
    }

    #[test]
    fn metadata_keeps_status_then_retry_after() {
        assert_eq!(meta(500, None).unwrap(), "{\n  \"status\": 500\n}\n");
        assert_eq!(
            meta(429, Some("20")).unwrap(),
            "{\n  \"status\": 429,\n  \"headers\": {\n    \"retry-after\": \"20\"\n  }\n}\n"
        );
    }

    #[test]
    fn truthiness_follows_the_payload() {
        for (value, expected) in [
            (Value::Null, false),
            (Value::Bool(true), true),
            (Value::from(0), false),
            (Value::from(1), true),
            (Value::from(""), false),
            (Value::from("yes"), true),
            (Value::Array(vec![]), false),
            (serde_json::json!({"a": 1}), true),
        ] {
            assert_eq!(truthy(&value), expected, "{value}");
        }
    }

    #[tokio::test]
    async fn a_dry_run_lists_requests_without_a_key() {
        let dir = repository();
        let mut out = Vec::new();
        run(
            dir.path(),
            &args(&["anthropic_messages", "--all", "--dry-run"]),
            &endpoint("https://api.example.com/", None),
            &mut out,
        )
        .await
        .unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "crates/kaeso-codecs/fixtures/anthropic_messages/requests/text-simple.json -> POST https://api.example.com/v1/messages\n\
             crates/kaeso-codecs/fixtures/anthropic_messages/requests/text-stream.json -> POST https://api.example.com/v1/messages (stream)\n"
        );
    }

    #[tokio::test]
    async fn refusals_before_anything_is_sent() {
        let dir = repository();
        let no_key = run(
            dir.path(),
            &args(&["anthropic_messages", "text-simple"]),
            &endpoint("http://127.0.0.1:9", None),
            &mut Vec::new(),
        )
        .await;
        assert_eq!(
            no_key.unwrap_err().to_string(),
            "ANTHROPIC_API_KEY is not set"
        );
        let no_count = run(
            dir.path(),
            &args(&["openai_chat", "--count", "--all"]),
            &endpoint("http://127.0.0.1:9", Some("k")),
            &mut Vec::new(),
        )
        .await;
        assert_eq!(
            no_count.unwrap_err().to_string(),
            "openai_chat has no token-count endpoint"
        );
        let no_folder = run(
            dir.path(),
            &args(&["openai_embeddings", "--all"]),
            &endpoint("http://127.0.0.1:9", Some("k")),
            &mut Vec::new(),
        )
        .await;
        assert!(matches!(no_folder, Err(Error::File { .. })));
    }

    #[tokio::test]
    async fn a_response_is_written_pretty_without_signed_queries_with_its_provenance() {
        let dir = repository();
        let served = serve_once(Reply {
            status: 200,
            headers: vec![("content-type", "application/json")],
            body: br#"{"id":"msg_1","type":"message","content":[{"type":"text","text":"https:\/\/b.s3.amazonaws.com\/a.pdf?X-Amz-Credential=AKIAIOSFODNN7EXAMPLE\u0026X-Amz-Signature=abc"}]}"#.to_vec(),
        })
        .await;
        let mut out = Vec::new();
        run(
            dir.path(),
            &args(&["anthropic_messages", "text-simple"]),
            &endpoint(&served.url, Some("sk-test")),
            &mut out,
        )
        .await
        .unwrap();
        let request = served.request.await.unwrap();
        assert!(
            request.starts_with("POST /v1/messages HTTP/1.1\r\n"),
            "{request}"
        );
        assert!(request.contains("x-api-key: sk-test\r\n"), "{request}");
        assert!(
            request.contains("anthropic-version: 2023-06-01\r\n"),
            "{request}"
        );
        assert!(request.ends_with(r#"{"model":"claude-haiku-4-5","max_tokens":10,"messages":[]}"#));
        let folder = "crates/kaeso-codecs/fixtures/anthropic_messages/responses";
        assert_eq!(
            dir.read(&format!("{folder}/recorded-text-simple.json")),
            "{\n  \"id\": \"msg_1\",\n  \"type\": \"message\",\n  \"content\": [\n    {\n      \"type\": \"text\",\n      \"text\": \"https:\\/\\/b.s3.amazonaws.com\\/a.pdf\"\n    }\n  ]\n}\n"
        );
        let index = dir.read(&format!("{folder}/RECORDED.md"));
        assert!(index.starts_with("# Recorded fixtures\n\nWritten by `cargo xtask record`."));
        assert!(index.contains(
            "| `recorded-text-simple.json` | `anthropic_messages/requests/text-simple.json` | `claude-haiku-4-5` | "
        ));
        assert!(
            String::from_utf8(out)
                .unwrap()
                .ends_with(&format!("  wrote {folder}/recorded-text-simple.json\n"))
        );
    }

    #[tokio::test]
    async fn a_stream_is_kept_byte_for_byte_without_signed_queries() {
        let dir = repository();
        let stream = "event: ping\r\ndata: {\"type\": \"ping\"}\r\n\r\n\
            data: {\"url\":\"https://b.s3.amazonaws.com/k?X-Amz-Credential=AKIAIOSFODNN7EXAMPLE&X-Amz-Signature=abc\"}\n\n";
        let served = serve_once(Reply {
            status: 200,
            headers: vec![("content-type", "text/event-stream")],
            body: stream.as_bytes().to_vec(),
        })
        .await;
        run(
            dir.path(),
            &args(&["anthropic_messages", "text-stream"]),
            &endpoint(&served.url, Some("sk-test")),
            &mut Vec::new(),
        )
        .await
        .unwrap();
        let request = served.request.await.unwrap();
        assert!(request.contains("anthropic-beta: interleaved-thinking-2025-05-14\r\n"));
        assert!(!request.contains("x-ignored"));
        assert_eq!(
            dir.read(
                "crates/kaeso-codecs/fixtures/anthropic_messages/streams/recorded-text-stream.sse"
            ),
            "event: ping\r\ndata: {\"type\": \"ping\"}\r\n\r\n\
             data: {\"url\":\"https://b.s3.amazonaws.com/k\"}\n\n"
        );
    }

    #[tokio::test]
    async fn errors_keep_their_status_and_retry_after() {
        let dir = repository();
        let served = serve_once(Reply {
            status: 429,
            headers: vec![("retry-after", "20")],
            body: br#"{"type":"error","error":{"type":"rate_limit_error"}}"#.to_vec(),
        })
        .await;
        run(
            dir.path(),
            &args(&["anthropic_messages", "--count", "basic"]),
            &endpoint(&served.url, Some("sk-test")),
            &mut Vec::new(),
        )
        .await
        .unwrap();
        assert!(
            served
                .request
                .await
                .unwrap()
                .starts_with("POST /v1/messages/count_tokens ")
        );
        let errors = "crates/kaeso-codecs/fixtures/anthropic_messages/errors";
        assert!(
            dir.read(&format!("{errors}/recorded-basic.json"))
                .contains("\"rate_limit_error\"")
        );
        assert_eq!(
            dir.read(&format!("{errors}/recorded-basic.meta.json")),
            "{\n  \"status\": 429,\n  \"headers\": {\n    \"retry-after\": \"20\"\n  }\n}\n"
        );
        assert!(dir.read(&format!("{errors}/RECORDED.md")).contains(
            "| `recorded-basic.json` | `anthropic_messages/requests/count_tokens/basic.json` |"
        ));
    }

    #[tokio::test]
    async fn a_body_that_is_not_json_is_kept_as_text() {
        let dir = repository();
        let served = serve_once(Reply {
            status: 502,
            headers: vec![],
            body: b"<html>bad gateway</html>".to_vec(),
        })
        .await;
        run(
            dir.path(),
            &args(&["anthropic_messages", "text-simple"]),
            &endpoint(&served.url, Some("sk-test")),
            &mut Vec::new(),
        )
        .await
        .unwrap();
        let errors = "crates/kaeso-codecs/fixtures/anthropic_messages/errors";
        assert_eq!(
            dir.read(&format!("{errors}/recorded-text-simple.txt")),
            "<html>bad gateway</html>"
        );
        assert_eq!(
            dir.read(&format!("{errors}/recorded-text-simple.meta.json")),
            "{\n  \"status\": 502\n}\n"
        );
    }

    #[tokio::test]
    async fn a_successful_body_that_is_not_json_fails() {
        let dir = repository();
        let served = serve_once(Reply {
            status: 200,
            headers: vec![],
            body: b"not json".to_vec(),
        })
        .await;
        let result = run(
            dir.path(),
            &args(&["anthropic_messages", "text-simple"]),
            &endpoint(&served.url, Some("sk-test")),
            &mut Vec::new(),
        )
        .await;
        assert!(matches!(result, Err(Error::Json { .. })));
    }
}
