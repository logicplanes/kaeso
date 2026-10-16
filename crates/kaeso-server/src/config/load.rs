use std::path::{Path, PathBuf};

use toml::{Table, Value};

use super::Config;
use super::settings::{Kind, REQUIRED, SETTINGS, Setting, setting};

pub trait Source {
    fn var(&self, name: &str) -> Option<String>;
    fn read_file(&self, path: &Path) -> std::io::Result<String>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ProcessSource;

impl Source for ProcessSource {
    fn var(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }
    fn read_file(&self, path: &Path) -> std::io::Result<String> {
        std::fs::read_to_string(path)
    }
}

#[derive(Clone, Debug, Default)]
pub struct MapSource {
    vars: std::collections::HashMap<String, String>,
    files: std::collections::HashMap<PathBuf, String>,
}

impl MapSource {
    pub fn new() -> MapSource {
        MapSource::default()
    }

    #[must_use]
    pub fn set(mut self, name: &str, value: impl Into<String>) -> MapSource {
        self.vars.insert(name.to_owned(), value.into());
        self
    }

    #[must_use]
    pub fn file(mut self, path: &str, contents: impl Into<String>) -> MapSource {
        self.files.insert(PathBuf::from(path), contents.into());
        self
    }
}

impl Source for MapSource {
    fn var(&self, name: &str) -> Option<String> {
        self.vars.get(name).cloned()
    }
    fn read_file(&self, path: &Path) -> std::io::Result<String> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no such file"))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("{env} ({key}) is required: {description}")]
    Missing {
        key: &'static str,
        env: &'static str,
        description: &'static str,
    },
    #[error("{env} ({key}) {reason}")]
    Invalid {
        key: String,
        env: String,
        reason: String,
    },
    #[error("cannot read {what} {path}: {reason}")]
    File {
        what: String,
        path: PathBuf,
        reason: String,
    },
    #[error(
        "unknown setting `{key}` in {path}; see https://docs.kaeso.app/self-hosting/configuration for every setting"
    )]
    UnknownKey { key: String, path: PathBuf },
}

impl ConfigError {
    pub(crate) fn invalid(key: &str, reason: String) -> ConfigError {
        let env = setting(key).map_or_else(|| key.to_owned(), |s| s.env.to_owned());
        ConfigError::Invalid {
            key: key.to_owned(),
            env,
            reason,
        }
    }
}

pub fn load(source: &dyn Source) -> Result<Config, ConfigError> {
    let mut table = Table::new();
    for s in SETTINGS {
        if let Some(default) = s.default {
            insert(&mut table, s.key, typed(s, default)?);
        }
    }
    if let Some(path) = source.var("KAESO_CONFIG") {
        let path = PathBuf::from(path);
        let text = source.read_file(&path).map_err(|e| ConfigError::File {
            what: "the configuration file (KAESO_CONFIG)".into(),
            path: path.clone(),
            reason: e.to_string(),
        })?;
        let file: Table = toml::from_str(&text).map_err(|e| ConfigError::File {
            what: "the configuration file (KAESO_CONFIG)".into(),
            path: path.clone(),
            reason: e.message().to_owned(),
        })?;
        let mut keys = Vec::new();
        flatten("", &file, &mut keys);
        for (key, value) in keys {
            if setting(&key).is_none() {
                return Err(ConfigError::UnknownKey { key, path });
            }
            insert(&mut table, &key, value);
        }
    }
    for s in SETTINGS {
        if let Some(value) = env_value(source, s)? {
            insert(&mut table, s.key, typed(s, &value)?);
        }
    }
    for key in REQUIRED {
        if get(&table, key).is_none() {
            let s = setting(key).ok_or_else(|| ConfigError::invalid(key, "is unknown".into()))?;
            return Err(ConfigError::Missing {
                key: s.key,
                env: s.env,
                description: s.description,
            });
        }
    }
    let config: Config = Value::Table(table)
        .try_into()
        .map_err(|e: toml::de::Error| {
            let message = e.message().to_owned();
            let key = SETTINGS
                .iter()
                .rev()
                .find(|s| message.contains(s.key.rsplit('.').next().unwrap_or(s.key)))
                .map_or("configuration", |s| s.key);
            ConfigError::invalid(key, format!("is invalid: {message}"))
        })?;
    config.validate()?;
    Ok(config)
}

fn env_value(source: &dyn Source, s: &Setting) -> Result<Option<String>, ConfigError> {
    let direct = source.var(s.env).filter(|v| !v.is_empty());
    if !s.secret {
        return Ok(direct);
    }
    let file_var = format!("{}_FILE", s.env);
    let Some(path) = source.var(&file_var).filter(|v| !v.is_empty()) else {
        return Ok(direct);
    };
    if direct.is_some() {
        return Err(ConfigError::Invalid {
            key: s.key.to_owned(),
            env: s.env.to_owned(),
            reason: format!("and {file_var} are both set; use one of them"),
        });
    }
    let path = PathBuf::from(path);
    let contents = source.read_file(&path).map_err(|e| ConfigError::File {
        what: format!("the secret file of {} ({file_var})", s.env),
        path,
        reason: e.to_string(),
    })?;
    Ok(Some(contents.trim_end_matches(['\n', '\r']).to_owned()))
}

fn typed(s: &Setting, raw: &str) -> Result<Value, ConfigError> {
    let invalid = |reason: &str| ConfigError::Invalid {
        key: s.key.to_owned(),
        env: s.env.to_owned(),
        reason: reason.to_owned(),
    };
    Ok(match s.kind {
        Kind::Text => Value::String(raw.to_owned()),
        Kind::Bool => match raw.trim() {
            "true" | "1" | "yes" | "on" => Value::Boolean(true),
            "false" | "0" | "no" | "off" => Value::Boolean(false),
            _ => return Err(invalid("must be true or false")),
        },
        Kind::Integer => Value::Integer(
            raw.trim()
                .parse::<u32>()
                .map(i64::from)
                .map_err(|_| invalid("must be a whole number"))?,
        ),
    })
}

fn insert(table: &mut Table, key: &str, value: Value) {
    let mut parts = key.split('.').peekable();
    let mut current = table;
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            current.insert(part.to_owned(), value);
            return;
        }
        let entry = current
            .entry(part.to_owned())
            .or_insert_with(|| Value::Table(Table::new()));
        if !entry.is_table() {
            *entry = Value::Table(Table::new());
        }
        let Value::Table(next) = entry else { return };
        current = next;
    }
}

fn get<'a>(table: &'a Table, key: &str) -> Option<&'a Value> {
    let mut parts = key.split('.');
    let first = parts.next()?;
    parts.try_fold(table.get(first)?, |v, p| v.as_table()?.get(p))
}

fn flatten(prefix: &str, table: &Table, out: &mut Vec<(String, Value)>) {
    for (k, v) in table {
        let key = if prefix.is_empty() {
            k.clone()
        } else {
            format!("{prefix}.{k}")
        };
        match v {
            Value::Table(t) if setting(&key).is_none() => flatten(&key, t, out),
            other => out.push((key, other.clone())),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use secrecy::ExposeSecret;

    use super::*;
    use crate::config::{AccountCreation, MailTransport, OrgCreation};

    #[derive(Default)]
    struct Map {
        vars: HashMap<String, String>,
        files: HashMap<PathBuf, String>,
    }

    impl Map {
        fn minimal() -> Map {
            let mut m = Map::default();
            for (k, v) in [
                ("KAESO_PUBLIC_URL", "https://kaeso.example.com"),
                ("KAESO_POSTGRES_URL", "postgres://kaeso@db/kaeso"),
                ("KAESO_CLICKHOUSE_URL", "http://kaeso:pw@clickhouse:8123"),
                ("KAESO_VALKEY_URL", "redis://valkey:6379"),
                ("KAESO_MASTER_KEYS", "k1:AAAA"),
                ("KAESO_MAIL_TRANSPORT", "log"),
            ] {
                m.vars.insert(k.into(), v.into());
            }
            m
        }
        fn set(mut self, k: &str, v: &str) -> Self {
            self.vars.insert(k.into(), v.into());
            self
        }
        fn file(mut self, p: &str, contents: &str) -> Self {
            self.files.insert(p.into(), contents.into());
            self
        }
    }

    impl Source for Map {
        fn var(&self, name: &str) -> Option<String> {
            self.vars.get(name).cloned()
        }
        fn read_file(&self, path: &Path) -> std::io::Result<String> {
            self.files
                .get(path)
                .cloned()
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "not found"))
        }
    }

    #[test]
    fn defaults_fill_everything_optional() {
        let c = load(&Map::minimal()).unwrap();
        assert_eq!(c.listen.to_string(), "0.0.0.0:8080");
        assert_eq!(c.metrics_listen.port(), 9100);
        assert_eq!(c.postgres.pool_size, 20);
        assert_eq!(c.upstream.connect_timeout.as_secs(), 10);
        assert_eq!(c.upstream.request_timeout.as_secs(), 600);
        assert_eq!(c.upstream.first_byte_timeout.as_secs(), 60);
        assert_eq!(c.upstream.stream_idle_timeout.as_secs(), 300);
        assert_eq!(c.streams.keepalive_interval.as_secs(), 15);
        assert_eq!(c.continuity.reasoning_ttl.as_secs(), 3600);
        assert_eq!(c.key_failures.per_address, 100);
        assert_eq!(c.key_failures.window.as_secs(), 60);
        assert_eq!(c.body_limits.model_api.0, 32 << 20);
        assert_eq!(c.body_limits.platform_api.0, 1 << 20);
        assert_eq!(c.logs.buffer_size.0, 256 << 20);
        assert_eq!(c.logs.flush_rows, 5000);
        assert_eq!(c.models_dev.interval.as_secs(), 24 * 3600);
        assert_eq!(c.instance.org_creation, OrgCreation::InstanceAdmins);
        assert_eq!(c.instance.account_creation, AccountCreation::InvitationOnly);
        assert_eq!(c.instance.metadata_retention_days, 90);
        assert_eq!(c.instance.content_retention_days, 30);
        assert!(c.instance.org_deletion);
        assert_eq!(c.instance.org_deletion_grace_days, 7);
        assert_eq!(c.mail.transport, MailTransport::Log);
        assert_eq!(
            c.providers.anthropic.base_url.as_str(),
            "https://api.anthropic.com/"
        );
        assert!(!c.is_local_development());
    }

    #[test]
    fn missing_required_settings_are_named() {
        let mut m = Map::minimal();
        m.vars.remove("KAESO_VALKEY_URL");
        let err = load(&m).unwrap_err().to_string();
        assert!(
            err.starts_with("KAESO_VALKEY_URL (valkey.url) is required"),
            "{err}"
        );
    }

    #[test]
    fn environment_wins_over_the_file() {
        let m = Map::minimal()
            .set("KAESO_CONFIG", "/etc/kaeso.toml")
            .set("KAESO_POSTGRES_POOL_SIZE", "7")
            .file(
                "/etc/kaeso.toml",
                "listen = \"127.0.0.1:9000\"\n[postgres]\npool_size = 3\n[providers.openai]\nbase_url = \"http://mock:1\"\n",
            );
        let c = load(&m).unwrap();
        assert_eq!(c.listen.port(), 9000);
        assert_eq!(c.postgres.pool_size, 7);
        assert_eq!(c.providers.openai.base_url.as_str(), "http://mock:1/");
    }

    #[test]
    fn unknown_file_keys_are_refused() {
        let m = Map::minimal()
            .set("KAESO_CONFIG", "/k.toml")
            .file("/k.toml", "[postgres]\npool = 3\n");
        let err = load(&m).unwrap_err().to_string();
        assert_eq!(
            err,
            "unknown setting `postgres.pool` in /k.toml; see https://docs.kaeso.app/self-hosting/configuration for every setting"
        );
    }

    #[test]
    fn secrets_come_from_files() {
        let mut m = Map::minimal()
            .set("KAESO_MASTER_KEYS_FILE", "/run/secrets/master_keys")
            .file("/run/secrets/master_keys", "k9:BBBB\n");
        m.vars.remove("KAESO_MASTER_KEYS");
        let c = load(&m).unwrap();
        assert_eq!(c.master_keys.expose_secret(), "k9:BBBB");
        let both = Map::minimal()
            .set("KAESO_MASTER_KEYS_FILE", "/run/secrets/master_keys")
            .file("/run/secrets/master_keys", "k9:BBBB");
        let err = load(&both).unwrap_err().to_string();
        assert!(err.contains("are both set"), "{err}");
        assert!(!err.contains("BBBB"));
    }

    #[test]
    fn invalid_values_name_the_setting_and_never_echo_secrets() {
        let err = load(&Map::minimal().set("KAESO_AUTH_OIDC", "maybe")).unwrap_err();
        assert!(
            err.to_string().starts_with("KAESO_AUTH_OIDC (auth.oidc)"),
            "{err}"
        );
        let err =
            load(&Map::minimal().set("KAESO_PUBLIC_URL", "http://kaeso.example.com")).unwrap_err();
        assert!(err.to_string().contains("must be an https URL"), "{err}");
        for url in [
            "https://admin:secret@kaeso.example.com",
            "https://kaeso.example.com/#top",
            "https://kaeso.example.com/console",
        ] {
            let err = load(&Map::minimal().set("KAESO_PUBLIC_URL", url)).unwrap_err();
            assert!(err.to_string().contains("bare origin"), "{url}: {err}");
            assert!(!err.to_string().contains("secret"), "{err}");
        }
        let err = load(&Map::minimal().set("KAESO_BODY_LIMITS_MODEL_API", "0")).unwrap_err();
        assert!(err.to_string().contains("larger than zero"), "{err}");
        let err = load(&Map::minimal().set("KAESO_SMTP_TLS", "sometimes")).unwrap_err();
        assert!(err.to_string().contains("smtp.tls"), "{err}");
        let err =
            load(&Map::minimal().set("KAESO_INSTANCE_CONTENT_RETENTION_DAYS", "120")).unwrap_err();
        assert!(
            err.to_string().contains("content is never kept longer"),
            "{err}"
        );
        let err = load(&Map::minimal().set("KAESO_MAIL_TRANSPORT", "smtp")).unwrap_err();
        assert!(err.to_string().starts_with("KAESO_SMTP_HOST"), "{err}");
    }

    #[test]
    fn keepalives_are_off_at_zero_and_bounded_otherwise() {
        for off in ["0", "0s"] {
            let c = load(&Map::minimal().set("KAESO_STREAMS_KEEPALIVE_INTERVAL", off)).unwrap();
            assert!(c.streams.keepalive_interval.is_zero());
        }
        let c = load(&Map::minimal().set("KAESO_STREAMS_KEEPALIVE_INTERVAL", "300s")).unwrap();
        assert_eq!(c.streams.keepalive_interval.as_secs(), 300);
        for bad in ["500ms", "301s", "1h"] {
            let err = load(&Map::minimal().set("KAESO_STREAMS_KEEPALIVE_INTERVAL", bad))
                .unwrap_err()
                .to_string();
            assert!(
                err.starts_with("KAESO_STREAMS_KEEPALIVE_INTERVAL (streams.keepalive_interval)"),
                "{err}"
            );
        }
    }

    #[test]
    fn reasoning_ttl_is_off_at_zero_and_bounded_otherwise() {
        let c = load(&Map::minimal().set("KAESO_CONTINUITY_REASONING_TTL", "0s")).unwrap();
        assert!(c.continuity.reasoning_ttl.is_zero());
        for bad in ["30s", "25h"] {
            let err = load(&Map::minimal().set("KAESO_CONTINUITY_REASONING_TTL", bad))
                .unwrap_err()
                .to_string();
            assert!(err.starts_with("KAESO_CONTINUITY_REASONING_TTL"), "{err}");
        }
    }

    #[test]
    fn the_key_failure_throttle_is_bounded() {
        let c = load(
            &Map::minimal()
                .set("KAESO_KEY_FAILURES_PER_ADDRESS", "0")
                .set("KAESO_KEY_FAILURES_WINDOW", "1h"),
        )
        .unwrap();
        assert_eq!(c.key_failures.per_address, 0);
        assert_eq!(c.key_failures.window.as_secs(), 3600);
        for bad in ["0s", "500ms", "61m"] {
            let err = load(&Map::minimal().set("KAESO_KEY_FAILURES_WINDOW", bad))
                .unwrap_err()
                .to_string();
            assert!(
                err.starts_with("KAESO_KEY_FAILURES_WINDOW (key_failures.window)"),
                "{err}"
            );
        }
    }

    #[test]
    fn account_and_org_creation_seed_without_allowed_domains() {
        let c = load(
            &Map::minimal()
                .set("KAESO_INSTANCE_ACCOUNT_CREATION", "everyone")
                .set("KAESO_INSTANCE_ORG_CREATION", "everyone"),
        )
        .unwrap();
        assert_eq!(c.instance.account_creation, AccountCreation::Everyone);
        assert_eq!(c.instance.org_creation, OrgCreation::Everyone);
        for (var, key) in [
            (
                "KAESO_INSTANCE_ACCOUNT_CREATION",
                "instance.account_creation",
            ),
            ("KAESO_INSTANCE_ORG_CREATION", "instance.org_creation"),
        ] {
            let err = load(&Map::minimal().set(var, "allowed_domains"))
                .unwrap_err()
                .to_string();
            assert!(
                err.contains(key) && err.contains("instance settings"),
                "{err}"
            );
        }
        assert!(load(&Map::minimal().set("KAESO_INSTANCE_ACCOUNT_CREATION", "anyone")).is_err());
    }

    #[test]
    fn localhost_is_development() {
        let c = load(&Map::minimal().set("KAESO_PUBLIC_URL", "http://localhost:8080")).unwrap();
        assert!(c.is_local_development());
        assert_eq!(c.public_origin(), "http://localhost:8080");
    }
}
