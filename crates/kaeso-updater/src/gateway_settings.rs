use std::path::{Path, PathBuf};

use kaeso_update::settings::{self, SMTP_PASSWORD, Setting};
use serde_json::Value;

const CONFIG_VAR: &str = "KAESO_CONFIG";

#[derive(Debug, PartialEq, Eq)]
pub enum Check {
    Fine,
    Refused(Vec<String>),
    Unreadable(String),
}

// `docker compose config` has resolved `.env`, the override files and every service's env_file
// into `environment`, so it holds exactly what the gateway's container gets.
fn environment(gateway: &Value, name: &str) -> Option<String> {
    let value = match gateway.get("environment")?.get(name)? {
        Value::String(s) => s.clone(),
        Value::Null => return None,
        other => other.to_string(),
    };
    (!value.is_empty()).then_some(value)
}

fn mounted(config: &Value, gateway: &Value, path: &str) -> Option<Mounted> {
    let target = Path::new(path);
    let list = |field: &str| {
        gateway
            .get(field)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    // Docker mounts the deepest bind over the others, so the longest target holds the file.
    let bind = list("volumes")
        .into_iter()
        .filter(|v| v.get("type").and_then(Value::as_str) == Some("bind"))
        .filter_map(|v| {
            let source = PathBuf::from(v.get("source")?.as_str()?);
            let at = PathBuf::from(v.get("target")?.as_str()?);
            let rest = target.strip_prefix(&at).ok()?;
            let file = if rest.as_os_str().is_empty() {
                source
            } else {
                source.join(rest)
            };
            Some((at.components().count(), file))
        })
        .max_by_key(|(depth, _)| *depth);
    if let Some((_, file)) = bind {
        return Some(Mounted::File(file));
    }
    for (field, default_dir) in [("configs", "/"), ("secrets", "/run/secrets/")] {
        for entry in list(field) {
            let Some(name) = entry.get("source").and_then(Value::as_str) else {
                continue;
            };
            let at = entry
                .get("target")
                .and_then(Value::as_str)
                .map_or_else(|| format!("{default_dir}{name}"), str::to_owned);
            if Path::new(&at) != target {
                continue;
            }
            let top = config.get(field).and_then(|t| t.get(name))?;
            if let Some(content) = top.get("content").and_then(Value::as_str) {
                return Some(Mounted::Content(content.to_owned()));
            }
            return top
                .get("file")
                .and_then(Value::as_str)
                .map(|f| Mounted::File(PathBuf::from(f)));
        }
    }
    None
}

enum Mounted {
    File(PathBuf),
    Content(String),
}

fn file_table(
    config: &Value,
    gateway: &Value,
    read: &dyn Fn(&Path) -> std::io::Result<String>,
) -> Result<Option<toml::Table>, String> {
    let Some(path) = environment(gateway, CONFIG_VAR) else {
        return Ok(None);
    };
    let unreadable = |why: String| {
        format!(
            "the gateway's configuration file {path} ({CONFIG_VAR}) can't be read from the install's folder ({why}), so the update can't check its settings before it switches"
        )
    };
    let text = match mounted(config, gateway, &path) {
        Some(Mounted::Content(text)) => text,
        Some(Mounted::File(file)) => {
            read(&file).map_err(|e| unreadable(format!("{}: {e}", file.display())))?
        }
        None => {
            return Err(unreadable(
                "no bind mount, config or secret holds it".into(),
            ));
        }
    };
    toml::from_str(&text)
        .map(Some)
        .map_err(|e| unreadable(e.message().to_owned()))
}

pub fn check(config: &Value, read: &dyn Fn(&Path) -> std::io::Result<String>) -> Check {
    let Some(gateway) = config.get("services").and_then(|s| s.get("gateway")) else {
        return Check::Fine;
    };
    let file = match file_table(config, gateway, read) {
        Ok(file) => file,
        Err(why) => return Check::Unreadable(why),
    };
    let value = |s: Setting| {
        environment(gateway, s.env)
            .or_else(|| {
                (s == SMTP_PASSWORD)
                    .then(|| environment(gateway, &format!("{}_FILE", s.env)))
                    .flatten()
            })
            .or_else(|| {
                file.as_ref()?
                    .get(s.table())?
                    .get(s.field())?
                    .as_str()
                    .map(str::to_owned)
            })
    };
    let refused = settings::refused(value);
    if refused.is_empty() {
        Check::Fine
    } else {
        Check::Refused(refused)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::json;

    use super::*;

    fn checked(gateway: Value, files: &[(&str, &str)]) -> Check {
        checked_in(json!({ "services": { "gateway": gateway } }), files)
    }

    fn checked_in(config: Value, files: &[(&str, &str)]) -> Check {
        let files: HashMap<PathBuf, String> = files
            .iter()
            .map(|(p, t)| (PathBuf::from(p), (*t).to_owned()))
            .collect();
        check(&config, &|path: &Path| {
            files
                .get(path)
                .cloned()
                .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))
        })
    }

    fn named(check: Check) -> Vec<String> {
        match check {
            Check::Refused(found) => found
                .iter()
                .map(|r| r.split(' ').next().unwrap().to_owned())
                .collect(),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_compose_install_as_shipped_passes() {
        let gateway = json!({ "environment": {
            "KAESO_SMTP_USER": "",
            "KAESO_SMTP_PASSWORD": "",
            "KAESO_MAIL_TRANSPORT": "log",
            "KAESO_CONFIG": null,
        }});
        assert_eq!(checked(gateway, &[]), Check::Fine);
        assert_eq!(checked_in(json!({ "services": {} }), &[]), Check::Fine);
    }

    #[test]
    fn settings_in_the_environment_are_refused_in_the_gateways_words() {
        let gateway = json!({ "environment": {
            "KAESO_KPMR_INTERVAL": "1m",
            "KAESO_SMTP_USER": "kaeso",
            "KAESO_SMTP_PASSWORD": "",
        }});
        let Check::Refused(found) = checked(gateway, &[]) else {
            panic!()
        };
        assert_eq!(
            found,
            settings::refused(|s| match s.env {
                "KAESO_KPMR_INTERVAL" => Some("1m".into()),
                "KAESO_SMTP_USER" => Some("kaeso".into()),
                _ => None,
            })
        );
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn a_password_file_counts_as_a_password() {
        let gateway = json!({ "environment": {
            "KAESO_SMTP_USER": "kaeso",
            "KAESO_SMTP_PASSWORD_FILE": "/run/secrets/smtp",
        }});
        assert_eq!(checked(gateway, &[]), Check::Fine);
    }

    #[test]
    fn the_configuration_file_is_read_where_the_gateway_reads_it() {
        let toml = "[kpmr]\ninterval = \"2m\"\n\n[mcp_servers]\nsync_interval = \"10s\"\n";
        let bind = json!({
            "environment": { "KAESO_CONFIG": "/etc/kaeso/kaeso.toml" },
            "volumes": [
                { "type": "volume", "source": "data", "target": "/data" },
                { "type": "bind", "source": "/srv/kaeso/conf", "target": "/etc/kaeso" },
                { "type": "bind", "source": "/srv/kaeso", "target": "/etc" },
            ],
        });
        assert_eq!(
            named(checked(bind, &[("/srv/kaeso/conf/kaeso.toml", toml)])),
            ["KAESO_KPMR_INTERVAL", "KAESO_MCP_SERVERS_SYNC_INTERVAL"]
        );
        let file_bind = json!({
            "environment": { "KAESO_CONFIG": "/kaeso.toml" },
            "volumes": [{ "type": "bind", "source": "/srv/kaeso/kaeso.toml", "target": "/kaeso.toml" }],
        });
        assert_eq!(
            named(checked(
                file_bind,
                &[("/srv/kaeso/kaeso.toml", "[smtp]\nuser = \"kaeso\"\n")]
            )),
            ["KAESO_SMTP_PASSWORD"]
        );
        let configs = json!({
            "services": { "gateway": {
                "environment": { "KAESO_CONFIG": "/kaeso" },
                "configs": [{ "source": "kaeso" }],
            }},
            "configs": { "kaeso": { "file": "/srv/kaeso/kaeso.toml" } },
        });
        assert_eq!(
            named(checked_in(configs, &[("/srv/kaeso/kaeso.toml", toml)])),
            ["KAESO_KPMR_INTERVAL", "KAESO_MCP_SERVERS_SYNC_INTERVAL"]
        );
        let content = json!({
            "services": { "gateway": {
                "environment": { "KAESO_CONFIG": "/etc/kaeso.toml" },
                "secrets": [{ "source": "kaeso", "target": "/etc/kaeso.toml" }],
            }},
            "secrets": { "kaeso": { "content": "[kpmr]\ninterval = \"auto\"\n" } },
        });
        assert_eq!(checked_in(content, &[]), Check::Fine);
    }

    #[test]
    fn the_environment_wins_over_the_file_as_in_the_gateway() {
        let gateway = json!({
            "environment": { "KAESO_CONFIG": "/kaeso.toml", "KAESO_KPMR_INTERVAL": "1h" },
            "volumes": [{ "type": "bind", "source": "/srv/kaeso/kaeso.toml", "target": "/kaeso.toml" }],
        });
        let file = "[kpmr]\ninterval = \"1m\"\n";
        assert_eq!(
            checked(gateway, &[("/srv/kaeso/kaeso.toml", file)]),
            Check::Fine
        );
    }

    #[test]
    fn a_file_the_runner_cannot_read_stops_the_update() {
        let elsewhere = json!({ "environment": { "KAESO_CONFIG": "/etc/kaeso.toml" } });
        let Check::Unreadable(why) = checked(elsewhere, &[]) else {
            panic!()
        };
        assert!(why.contains("/etc/kaeso.toml (KAESO_CONFIG)"), "{why}");
        assert!(
            why.contains("no bind mount, config or secret holds it"),
            "{why}"
        );
        let missing = json!({
            "environment": { "KAESO_CONFIG": "/kaeso.toml" },
            "volumes": [{ "type": "bind", "source": "/srv/kaeso/kaeso.toml", "target": "/kaeso.toml" }],
        });
        assert!(matches!(
            checked(missing.clone(), &[]),
            Check::Unreadable(_)
        ));
        assert!(matches!(
            checked(missing, &[("/srv/kaeso/kaeso.toml", "[kpmr")]),
            Check::Unreadable(_)
        ));
    }
}
