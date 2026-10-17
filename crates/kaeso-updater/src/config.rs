use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use kaeso_update::protocol::TOKEN_MIN_LEN;
use secrecy::{ExposeSecret, SecretString};
use url::Url;

use crate::compose::ComposeFile;
use crate::engine::Settings;
use crate::files::{MaintenanceFlag, StateFile};

pub const COMPOSE_FILE: &str = "/kaeso/compose.yaml";
pub const MAINTENANCE_DIR: &str = "/maintenance";
pub const STATE_DIR: &str = "/state";
pub const DEFAULT_LISTEN: &str = "0.0.0.0:8081";
pub const DEFAULT_DOCKER: &str = "http://docker-proxy:2375";

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("KAESO_UPDATER_TOKEN or KAESO_UPDATER_TOKEN_FILE must be set")]
    NoToken,
    #[error("KAESO_UPDATER_TOKEN and KAESO_UPDATER_TOKEN_FILE are both set; use one of them")]
    TwoTokens,
    #[error("the token file cannot be read: {0}")]
    TokenFile(String),
    #[error(
        "the updater token must be at least {TOKEN_MIN_LEN} characters; generate one with `openssl rand -hex 32`"
    )]
    ShortToken,
    #[error("{0} is not a valid address")]
    Address(&'static str),
    #[error("HOSTNAME is not set; run the updater as a Compose service")]
    NoHostname,
}

pub fn healthcheck_address(listen: Option<&str>) -> String {
    let listen = listen
        .filter(|v| !v.trim().is_empty())
        .and_then(|v| v.parse::<SocketAddr>().ok())
        .or_else(|| DEFAULT_LISTEN.parse().ok());
    match listen {
        Some(addr) if addr.ip().is_unspecified() => {
            let local = if addr.is_ipv6() { "[::1]" } else { "127.0.0.1" };
            format!("{local}:{}", addr.port())
        }
        Some(addr) => addr.to_string(),
        None => "127.0.0.1:8081".to_owned(),
    }
}

#[derive(Debug)]
pub struct Config {
    pub listen: SocketAddr,
    pub token: SecretString,
    pub docker: Url,
    pub hostname: String,
}

impl Config {
    pub fn load(
        var: impl Fn(&str) -> Option<String>,
        read: impl Fn(&Path) -> std::io::Result<String>,
    ) -> Result<Config, ConfigError> {
        let set = |name: &str| var(name).filter(|v| !v.trim().is_empty());
        let token = match (set("KAESO_UPDATER_TOKEN"), set("KAESO_UPDATER_TOKEN_FILE")) {
            (Some(_), Some(_)) => return Err(ConfigError::TwoTokens),
            (Some(t), None) => t,
            (None, Some(path)) => read(Path::new(&path))
                .map_err(|e| ConfigError::TokenFile(e.to_string()))?
                .trim_end_matches(['\n', '\r'])
                .to_owned(),
            (None, None) => return Err(ConfigError::NoToken),
        };
        let token = SecretString::from(token);
        if token.expose_secret().trim().len() < TOKEN_MIN_LEN {
            return Err(ConfigError::ShortToken);
        }
        let listen = set("KAESO_UPDATER_LISTEN")
            .unwrap_or_else(|| DEFAULT_LISTEN.to_owned())
            .parse()
            .map_err(|_| ConfigError::Address("KAESO_UPDATER_LISTEN"))?;
        let docker = Url::parse(
            &set("KAESO_UPDATER_DOCKER_HOST").unwrap_or_else(|| DEFAULT_DOCKER.to_owned()),
        )
        .ok()
        .filter(|u| matches!(u.scheme(), "http" | "https"))
        .ok_or(ConfigError::Address("KAESO_UPDATER_DOCKER_HOST"))?;
        let hostname = set("HOSTNAME").ok_or(ConfigError::NoHostname)?;
        Ok(Config {
            listen,
            token,
            docker,
            hostname,
        })
    }

    pub fn settings(&self) -> Settings {
        Settings {
            compose: ComposeFile::new(COMPOSE_FILE),
            maintenance: MaintenanceFlag::new(MAINTENANCE_DIR),
            state: StateFile::new(STATE_DIR),
            hostname: self.hostname.clone(),
            poll: Duration::from_secs(2),
            gateway_timeout: Duration::from_secs(600),
            console_timeout: Duration::from_secs(120),
            rollback_timeout: Duration::from_secs(300),
            docker_wait: Duration::from_secs(120),
            stop_seconds: 40,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef";

    fn load(vars: &[(&str, &str)]) -> Result<Config, ConfigError> {
        let map: HashMap<String, String> = vars
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Config::load(
            |name| map.get(name).cloned(),
            |path| match path.to_str() {
                Some("/run/secrets/updater_token") => Ok(format!("{TOKEN}\n")),
                _ => Err(std::io::Error::from(std::io::ErrorKind::NotFound)),
            },
        )
    }

    #[test]
    fn defaults_fill_everything_but_the_token() {
        let c = load(&[("KAESO_UPDATER_TOKEN", TOKEN), ("HOSTNAME", "abc123")]).unwrap();
        assert_eq!(c.listen.to_string(), DEFAULT_LISTEN);
        assert_eq!(c.docker.as_str(), "http://docker-proxy:2375/");
        assert_eq!(c.token.expose_secret(), TOKEN);
        let s = c.settings();
        assert_eq!(s.hostname, "abc123");
        assert_eq!(s.gateway_timeout, Duration::from_secs(600));
    }

    #[test]
    fn the_token_comes_from_a_file_and_must_be_long() {
        let c = load(&[
            ("KAESO_UPDATER_TOKEN_FILE", "/run/secrets/updater_token"),
            ("HOSTNAME", "h"),
        ])
        .unwrap();
        assert_eq!(c.token.expose_secret(), TOKEN);
        assert_eq!(
            load(&[("HOSTNAME", "h")]).unwrap_err(),
            ConfigError::NoToken
        );
        assert_eq!(
            load(&[("KAESO_UPDATER_TOKEN", "short"), ("HOSTNAME", "h")]).unwrap_err(),
            ConfigError::ShortToken
        );
        assert_eq!(
            load(&[
                ("KAESO_UPDATER_TOKEN", TOKEN),
                ("KAESO_UPDATER_TOKEN_FILE", "/x"),
                ("HOSTNAME", "h")
            ])
            .unwrap_err(),
            ConfigError::TwoTokens
        );
        assert!(matches!(
            load(&[("KAESO_UPDATER_TOKEN_FILE", "/missing"), ("HOSTNAME", "h")]),
            Err(ConfigError::TokenFile(_))
        ));
    }

    #[test]
    fn addresses_and_the_hostname_are_checked() {
        assert_eq!(
            load(&[
                ("KAESO_UPDATER_TOKEN", TOKEN),
                ("HOSTNAME", "h"),
                ("KAESO_UPDATER_LISTEN", "nope")
            ])
            .unwrap_err(),
            ConfigError::Address("KAESO_UPDATER_LISTEN")
        );
        assert_eq!(
            load(&[
                ("KAESO_UPDATER_TOKEN", TOKEN),
                ("HOSTNAME", "h"),
                ("KAESO_UPDATER_DOCKER_HOST", "unix:///var/run/docker.sock")
            ])
            .unwrap_err(),
            ConfigError::Address("KAESO_UPDATER_DOCKER_HOST")
        );
        assert_eq!(
            load(&[("KAESO_UPDATER_TOKEN", TOKEN)]).unwrap_err(),
            ConfigError::NoHostname
        );
    }

    #[test]
    fn the_healthcheck_asks_the_address_the_updater_listens_on() {
        assert_eq!(healthcheck_address(None), "127.0.0.1:8081");
        assert_eq!(healthcheck_address(Some("")), "127.0.0.1:8081");
        assert_eq!(healthcheck_address(Some("0.0.0.0:9000")), "127.0.0.1:9000");
        assert_eq!(healthcheck_address(Some("[::]:9001")), "[::1]:9001");
        assert_eq!(healthcheck_address(Some("10.0.0.5:8082")), "10.0.0.5:8082");
        assert_eq!(healthcheck_address(Some("nonsense")), "127.0.0.1:8081");
    }

    #[test]
    fn errors_never_contain_the_token() {
        for e in [
            ConfigError::ShortToken,
            ConfigError::TwoTokens,
            ConfigError::NoToken,
        ] {
            assert!(!e.to_string().contains(TOKEN));
        }
        let c = load(&[("KAESO_UPDATER_TOKEN", TOKEN), ("HOSTNAME", "h")]).unwrap();
        assert!(!format!("{c:?}").contains(TOKEN));
    }
}
