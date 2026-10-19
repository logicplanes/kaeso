use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::de::{Deserialize, Deserializer, MapAccess, Visitor};
use serde_json::Value;
use serde_json::value::RawValue;
use time::OffsetDateTime;
use time::macros::format_description;

use kaeso_codecs::{
    anthropic_messages, is_forwarded_header, openai_chat, openai_embeddings, openai_responses,
};
use kaeso_core::provider::{AuthStyle, Provider, lookup};
use kaeso_kpp::DialectId;

use crate::error::{Error, read, read_text, write};
use crate::json::pretty;
use crate::signed_urls::strip_signed_queries;

const FIXTURES: &str = "crates/kaeso-codecs/fixtures";
const PROVIDERS_FOLDER: &str = "providers";
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

    fn default_provider(self) -> &'static str {
        if self.is_anthropic() {
            "anthropic"
        } else {
            "openai"
        }
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
    #[arg(
        long,
        value_name = "ID",
        help = "The provider to record, by its id in PROVIDERS [default: anthropic for anthropic_messages, openai for the others]"
    )]
    provider: Option<String>,
    #[arg(
        long,
        help = "Send this model instead of the one each request fixture names"
    )]
    model: Option<String>,
}

impl Args {
    pub(crate) fn provider(&self) -> Result<&'static Provider, Error> {
        let id = self
            .provider
            .as_deref()
            .unwrap_or(self.dialect.default_provider());
        lookup(id).map_err(|e| Error::Invalid(e.to_string()))
    }
}

pub(crate) struct Endpoint {
    provider: &'static Provider,
    base_url: String,
    key_variable: String,
    key: Option<String>,
}

impl Endpoint {
    pub(crate) fn from_env(provider: &'static Provider) -> Self {
        Self::from_variables(provider, |name| std::env::var(name).ok())
    }

    fn from_variables(
        provider: &'static Provider,
        variable: impl Fn(&str) -> Option<String>,
    ) -> Self {
        let segment = provider.provider_id().env_segment();
        let key_variable = format!("{segment}_API_KEY");
        Self {
            provider,
            base_url: variable(&format!("{segment}_BASE_URL"))
                .filter(|u| !u.is_empty())
                .unwrap_or_else(|| provider.default_base_url.to_owned()),
            key: variable(&key_variable).filter(|k| !k.is_empty()),
            key_variable,
        }
    }
}

struct Recorder<'a> {
    root: &'a Path,
    fixtures: PathBuf,
    target: PathBuf,
    dialect: Dialect,
    provider: &'static Provider,
    count: Option<(&'static str, &'static str)>,
    url: String,
    model: Option<&'a str>,
    key: Option<&'a str>,
    client: reqwest::Client,
}

pub(crate) async fn run(
    root: &Path,
    args: &Args,
    endpoint: &Endpoint,
    out: &mut impl Write,
) -> Result<(), Error> {
    let provider = endpoint.provider;
    let dialect = args.dialect.id();
    if !provider.dialects.contains(&dialect) {
        let spoken: Vec<&str> = provider.dialects.iter().map(|d| d.as_str()).collect();
        return Err(Error::Invalid(format!(
            "{} does not speak {dialect}; it speaks {}",
            provider.id,
            spoken.join(", ")
        )));
    }
    let count = match (args.count, args.dialect.count()) {
        (false, _) => None,
        (true, Some(_)) if provider.count_tokens != Some(dialect) => {
            return Err(Error::Invalid(format!(
                "{} has no token-count endpoint for {dialect}; record without --count",
                provider.id
            )));
        }
        (true, Some(count)) => Some(count),
        (true, None) => {
            return Err(Error::Invalid(format!(
                "{dialect} has no token-count endpoint"
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
    let canonical = fixtures.join(args.dialect.folder());
    let target = if provider.id == args.dialect.default_provider() {
        canonical.clone()
    } else {
        canonical.join(PROVIDERS_FOLDER).join(provider.id)
    };
    let mut available = requests_in(&canonical, count)?;
    if target != canonical {
        let own = target.join("requests");
        if own.is_dir() {
            available = merge(available, requests_in(&target, count)?)?;
        }
    }
    let selected = select(available, &args.names, args.all)?;
    let recorder = Recorder {
        root,
        fixtures,
        target,
        dialect: args.dialect,
        provider,
        count,
        url: format!(
            "{}{}",
            endpoint.base_url.trim_end_matches('/'),
            count.map_or(args.dialect.path(), |(path, _)| path)
        ),
        model: args.model.as_deref(),
        key: endpoint.key.as_deref().filter(|_| !args.dry_run),
        client: reqwest::Client::builder().timeout(TIMEOUT).build()?,
    };
    for fixture in selected {
        recorder.record(&fixture, out).await?;
    }
    Ok(())
}

fn requests_in(
    base: &Path,
    count: Option<(&'static str, &'static str)>,
) -> Result<Vec<PathBuf>, Error> {
    let mut folder = base.join("requests");
    if let Some((_, sub)) = count {
        folder.push(sub);
    }
    request_fixtures(&folder)
}

fn merge(canonical: Vec<PathBuf>, own: Vec<PathBuf>) -> Result<Vec<PathBuf>, Error> {
    let names: BTreeSet<String> = canonical.iter().map(|p| stem(p)).collect();
    if let Some(twin) = own.iter().find(|p| names.contains(&stem(p))) {
        return Err(Error::Invalid(format!(
            "{} has the name of a request fixture of the dialect; rename it",
            twin.display()
        )));
    }
    let mut all = canonical;
    all.extend(own);
    Ok(all)
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
        let mut body = read(fixture)?;
        let payload: Value = serde_json::from_slice(&body)
            .map_err(|e| Error::json(self.relative(fixture).display(), e))?;
        let model = match (self.model, payload.get("model")) {
            (Some(m), _) => {
                body = with_model(&body, m)
                    .map_err(|e| Error::json(self.relative(fixture).display(), e))?;
                m.to_owned()
            }
            (None, Some(Value::String(m))) => m.clone(),
            (None, Some(other)) => other.to_string(),
            (None, None) => "?".to_owned(),
        };
        let streaming = payload.get("stream").is_some_and(truthy);
        writeln!(
            out,
            "{} -> POST {}{}{}",
            self.relative(fixture).display(),
            self.url,
            self.model.map(|m| format!(" as {m}")).unwrap_or_default(),
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
        let base = &self.target;
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
        match self.provider.auth {
            AuthStyle::Bearer => {
                headers.push(("authorization".to_owned(), format!("Bearer {key}")))
            }
            AuthStyle::XApiKey => headers.push(("x-api-key".to_owned(), key.to_owned())),
        }
        if self.dialect.is_anthropic() {
            headers.push((
                "anthropic-version".to_owned(),
                anthropic_messages::ANTHROPIC_VERSION.to_owned(),
            ));
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
            format!(
                "# Recorded fixtures\n\nWritten by `cargo xtask record`. Provenance: live API of {} (`{}`).\n\n| File | Request fixture | Model | Recorded (UTC) |\n|---|---|---|---|\n",
                self.provider.name, self.provider.id
            )
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

struct Members(Vec<(String, Box<RawValue>)>);

impl<'de> Deserialize<'de> for Members {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Ordered;
        impl<'de> Visitor<'de> for Ordered {
            type Value = Members;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a JSON object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Members, A::Error> {
                let mut members = Vec::new();
                while let Some(member) = map.next_entry()? {
                    members.push(member);
                }
                Ok(Members(members))
            }
        }
        deserializer.deserialize_map(Ordered)
    }
}

fn with_model(body: &[u8], model: &str) -> Result<Vec<u8>, serde_json::Error> {
    let Members(members) = serde_json::from_slice(body)?;
    let model = serde_json::to_string(model)?;
    let mut fields = Vec::with_capacity(members.len() + 1);
    let mut named = false;
    for (name, value) in &members {
        let value = if name == "model" {
            named = true;
            model.as_str()
        } else {
            value.get()
        };
        fields.push(format!("{}:{value}", serde_json::to_string(name)?));
    }
    if !named {
        fields.push(format!("\"model\":{model}"));
    }
    Ok(format!("{{{}}}", fields.join(",")).into_bytes())
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

    static EXAMPLE: Provider = Provider {
        id: "example-cloud",
        name: "Example Cloud",
        default_base_url: "https://api.example.com/compat",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        embeddings: None,
        dialects: &[DialectId::OpenAiChat, DialectId::AnthropicMessages],
    };

    fn known(id: &str) -> &'static Provider {
        lookup(id).unwrap()
    }

    fn endpoint(provider: &'static Provider, base_url: &str, key: Option<&str>) -> Endpoint {
        Endpoint {
            provider,
            base_url: base_url.to_owned(),
            key_variable: format!("{}_API_KEY", provider.provider_id().env_segment()),
            key: key.map(str::to_owned),
        }
    }

    fn anthropic(base_url: &str, key: Option<&str>) -> Endpoint {
        endpoint(known("anthropic"), base_url, key)
    }

    #[test]
    fn dialect_names_and_endpoints() {
        assert_eq!(
            args(&["openai_responses", "--all"]).dialect,
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
    }

    #[test]
    fn the_provider_comes_from_the_registry_and_defaults_to_the_dialect_owner() {
        let id = |line: &[&str]| args(line).provider().map(|p| p.id);
        assert_eq!(id(&["anthropic_messages", "--all"]).unwrap(), "anthropic");
        assert_eq!(id(&["openai_chat", "--all"]).unwrap(), "openai");
        assert_eq!(id(&["openai_embeddings", "--all"]).unwrap(), "openai");
        assert_eq!(
            id(&["openai_chat", "--provider", "anthropic", "--all"]).unwrap(),
            "anthropic"
        );
        assert_eq!(
            id(&["openai_chat", "--provider", "example-cloud", "--all"])
                .unwrap_err()
                .to_string(),
            "`example-cloud` is not a provider Kaeso knows; the providers are openai, anthropic, nvidia"
        );
        assert_eq!(
            id(&["openai_chat", "--provider", "Example_Cloud", "--all"])
                .unwrap_err()
                .to_string(),
            "`Example_Cloud` is not a provider id: lowercase letters and digits, with single hyphens between them"
        );
    }

    #[test]
    fn keys_and_base_urls_are_named_after_the_provider_id() {
        let variables = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| {
                pairs
                    .iter()
                    .find(|(n, _)| *n == name)
                    .map(|(_, v)| (*v).to_owned())
            }
        };
        let set = Endpoint::from_variables(
            &EXAMPLE,
            variables(&[
                ("EXAMPLE_CLOUD_API_KEY", "sk-test"),
                ("EXAMPLE_CLOUD_BASE_URL", "http://127.0.0.1:9"),
                ("OPENAI_API_KEY", "sk-other"),
            ]),
        );
        assert_eq!(set.key_variable, "EXAMPLE_CLOUD_API_KEY");
        assert_eq!(set.key.as_deref(), Some("sk-test"));
        assert_eq!(set.base_url, "http://127.0.0.1:9");
        let unset = Endpoint::from_variables(
            &EXAMPLE,
            variables(&[
                ("EXAMPLE_CLOUD_API_KEY", ""),
                ("EXAMPLE_CLOUD_BASE_URL", ""),
            ]),
        );
        assert_eq!(unset.key, None);
        assert_eq!(unset.base_url, "https://api.example.com/compat");
        let openai = Endpoint::from_variables(known("openai"), variables(&[]));
        assert_eq!(
            (openai.key_variable.as_str(), openai.base_url.as_str()),
            ("OPENAI_API_KEY", "https://api.openai.com")
        );
        let anthropic = Endpoint::from_variables(known("anthropic"), variables(&[]));
        assert_eq!(
            (anthropic.key_variable.as_str(), anthropic.base_url.as_str()),
            ("ANTHROPIC_API_KEY", "https://api.anthropic.com")
        );
    }

    #[test]
    fn a_model_override_keeps_every_other_member_as_written() {
        let body = br#"{ "stream": true, "model": "gpt-4o", "temperature": 1.0, "n": 1e2, "messages": [ {"role": "user"} ] }"#;
        assert_eq!(
            String::from_utf8(with_model(body, "llama-\"x\"").unwrap()).unwrap(),
            r#"{"stream":true,"model":"llama-\"x\"","temperature":1.0,"n":1e2,"messages":[ {"role": "user"} ]}"#
        );
        assert_eq!(
            with_model(br#"{"input":"hi"}"#, "m").unwrap(),
            br#"{"input":"hi","model":"m"}"#
        );
        assert_eq!(with_model(b"{}", "m").unwrap(), br#"{"model":"m"}"#);
        assert!(with_model(b"[]", "m").is_err());
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
            &anthropic("https://api.example.com/", None),
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
            &anthropic("http://127.0.0.1:9", None),
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
            &endpoint(known("openai"), "http://127.0.0.1:9", Some("k")),
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
            &endpoint(known("openai"), "http://127.0.0.1:9", Some("k")),
            &mut Vec::new(),
        )
        .await;
        assert!(matches!(no_folder, Err(Error::File { .. })));
        let wrong_dialect = run(
            dir.path(),
            &args(&["anthropic_messages", "--all"]),
            &endpoint(known("openai"), "http://127.0.0.1:9", Some("k")),
            &mut Vec::new(),
        )
        .await;
        assert_eq!(
            wrong_dialect.unwrap_err().to_string(),
            "openai does not speak anthropic_messages; it speaks openai_responses, openai_chat, openai_embeddings"
        );
        let cannot_count = run(
            dir.path(),
            &args(&["anthropic_messages", "--count", "--all"]),
            &endpoint(&EXAMPLE, "http://127.0.0.1:9", Some("k")),
            &mut Vec::new(),
        )
        .await;
        assert_eq!(
            cannot_count.unwrap_err().to_string(),
            "example-cloud has no token-count endpoint for anthropic_messages; record without --count"
        );
    }

    #[tokio::test]
    async fn a_provider_dry_run_adds_its_own_requests_and_names_the_model() {
        let dir = repository();
        let own = "crates/kaeso-codecs/fixtures/openai_chat/providers/example-cloud/requests";
        dir.file(&format!("{own}/reasoning-format.json"), r#"{"model":"m"}"#);
        let mut out = Vec::new();
        run(
            dir.path(),
            &args(&["openai_chat", "--all", "--dry-run", "--model", "llama-4"]),
            &endpoint(&EXAMPLE, "https://api.example.com/compat", None),
            &mut out,
        )
        .await
        .unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "crates/kaeso-codecs/fixtures/openai_chat/requests/basic.json -> POST https://api.example.com/compat/v1/chat/completions as llama-4\n\
             crates/kaeso-codecs/fixtures/openai_chat/providers/example-cloud/requests/reasoning-format.json -> POST https://api.example.com/compat/v1/chat/completions as llama-4\n"
        );
        dir.file(&format!("{own}/basic.json"), r#"{"model":"m"}"#);
        let twin = run(
            dir.path(),
            &args(&["openai_chat", "basic", "--dry-run"]),
            &endpoint(&EXAMPLE, "https://api.example.com/compat", None),
            &mut Vec::new(),
        )
        .await;
        assert!(
            twin.unwrap_err()
                .to_string()
                .ends_with("providers/example-cloud/requests/basic.json has the name of a request fixture of the dialect; rename it")
        );
    }

    #[tokio::test]
    async fn another_provider_records_into_its_own_folder_with_its_auth_and_model() {
        let dir = repository();
        let served = serve_once(Reply {
            status: 200,
            headers: vec![("content-type", "application/json")],
            body: br#"{"id":"msg_2","type":"message","content":[]}"#.to_vec(),
        })
        .await;
        let mut out = Vec::new();
        run(
            dir.path(),
            &args(&["anthropic_messages", "text-simple", "--model", "glm-5"]),
            &endpoint(&EXAMPLE, &served.url, Some("sk-test")),
            &mut out,
        )
        .await
        .unwrap();
        let request = served.request.await.unwrap();
        assert!(
            request.starts_with("POST /v1/messages HTTP/1.1\r\n"),
            "{request}"
        );
        assert!(
            request.contains("authorization: Bearer sk-test\r\n"),
            "{request}"
        );
        assert!(!request.contains("x-api-key"), "{request}");
        assert!(
            request.contains("anthropic-version: 2023-06-01\r\n"),
            "{request}"
        );
        assert!(request.ends_with(r#"{"model":"glm-5","max_tokens":10,"messages":[]}"#));
        let folder =
            "crates/kaeso-codecs/fixtures/anthropic_messages/providers/example-cloud/responses";
        assert!(
            dir.read(&format!("{folder}/recorded-text-simple.json"))
                .contains("\"msg_2\"")
        );
        let index = dir.read(&format!("{folder}/RECORDED.md"));
        assert!(
            index.contains("Provenance: live API of Example Cloud (`example-cloud`).\n"),
            "{index}"
        );
        assert!(index.contains(
            "| `recorded-text-simple.json` | `anthropic_messages/requests/text-simple.json` | `glm-5` | "
        ));
        assert!(
            !dir.path()
                .join("crates/kaeso-codecs/fixtures/anthropic_messages/responses/recorded-text-simple.json")
                .exists()
        );
        assert!(
            String::from_utf8(out)
                .unwrap()
                .ends_with(&format!("  wrote {folder}/recorded-text-simple.json\n"))
        );
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
            &anthropic(&served.url, Some("sk-test")),
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
        assert!(index.starts_with(
            "# Recorded fixtures\n\nWritten by `cargo xtask record`. Provenance: live API of Anthropic (`anthropic`).\n"
        ));
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
            &anthropic(&served.url, Some("sk-test")),
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
            &anthropic(&served.url, Some("sk-test")),
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
            &anthropic(&served.url, Some("sk-test")),
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
            &anthropic(&served.url, Some("sk-test")),
            &mut Vec::new(),
        )
        .await;
        assert!(matches!(result, Err(Error::Json { .. })));
    }
}
