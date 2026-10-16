#![allow(clippy::unwrap_used)]

use std::collections::HashSet;

use kaeso_server::config::{MapSource, REQUIRED, SETTINGS, load};
use secrecy::ExposeSecret;

fn read(path: &str) -> String {
    std::fs::read_to_string(format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn cell(text: &str) -> String {
    text.replace('|', "\\|")
}

fn settings_reference() -> String {
    let mut lines = vec![
        "| Environment | TOML key | Type | Default | Meaning |".to_owned(),
        "|---|---|---|---|---|".to_owned(),
    ];
    for s in SETTINGS {
        let kind = if s.secret {
            format!("{}, secret", s.kind.doc_name())
        } else {
            s.kind.doc_name().to_owned()
        };
        let default = match s.default {
            Some(d) => format!("`{}`", cell(d)),
            None if REQUIRED.contains(&s.key) => "required".to_owned(),
            None => "—".to_owned(),
        };
        lines.push(format!(
            "| `{}` | `{}` | {kind} | {default} | {} |",
            s.env,
            s.key,
            cell(s.description)
        ));
    }
    lines.join("\n")
}

#[test]
fn the_settings_reference_is_current() {
    insta::assert_snapshot!("settings_reference", settings_reference());
}

#[test]
fn every_environment_name_is_the_prefixed_upper_case_key() {
    for s in SETTINGS {
        let expected = format!("KAESO_{}", s.key.to_uppercase().replace('.', "_"));
        assert_eq!(s.env, expected, "environment name of {}", s.key);
    }
}

#[test]
fn keys_and_environment_names_are_unique() {
    let mut keys = HashSet::new();
    let mut envs = HashSet::new();
    for s in SETTINGS {
        assert!(keys.insert(s.key), "{} is listed twice", s.key);
        assert!(envs.insert(s.env), "{} is listed twice", s.env);
    }
    for s in SETTINGS.iter().filter(|s| s.secret) {
        let file = format!("{}_FILE", s.env);
        assert!(
            !envs.contains(file.as_str()),
            "{file} is both a setting and the file variable of {}",
            s.env
        );
    }
}

#[test]
fn no_key_is_also_a_table_of_other_keys() {
    for s in SETTINGS {
        let table = format!("{}.", s.key);
        let nested: Vec<&str> = SETTINGS
            .iter()
            .filter(|o| o.key.starts_with(&table))
            .map(|o| o.key)
            .collect();
        assert!(
            nested.is_empty(),
            "{} is a value and a table of {nested:?}",
            s.key
        );
    }
}

#[test]
fn every_description_is_a_sentence() {
    for s in SETTINGS {
        let d = s.description;
        let starts_well = d
            .chars()
            .next()
            .is_some_and(|c| c.is_uppercase() || c == '`');
        assert!(
            starts_well,
            "description of {} starts with a capital or a code span",
            s.key
        );
        assert!(
            d.ends_with('.'),
            "description of {} ends with a period",
            s.key
        );
        assert_eq!(
            d,
            d.trim(),
            "description of {} has no surrounding space",
            s.key
        );
        assert!(!d.contains('\n'), "description of {} is one line", s.key);
        assert!(
            !d.contains("  "),
            "description of {} has no double space",
            s.key
        );
    }
}

#[test]
fn every_required_setting_exists_and_has_no_default() {
    for key in REQUIRED {
        let s = SETTINGS
            .iter()
            .find(|s| s.key == *key)
            .unwrap_or_else(|| panic!("required setting {key} is not a setting"));
        assert!(s.default.is_none(), "required setting {key} has a default");
    }
}

#[test]
fn secrets_have_no_default() {
    for s in SETTINGS.iter().filter(|s| s.secret) {
        assert!(s.default.is_none(), "secret {} has a default", s.key);
    }
}

fn flatten(prefix: &str, table: &toml::Table, out: &mut Vec<(String, String)>) {
    for (name, value) in table {
        let key = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}.{name}")
        };
        match value {
            toml::Value::Table(inner) => flatten(&key, inner, out),
            toml::Value::String(text) => out.push((key, text.clone())),
            other => out.push((key, other.to_string())),
        }
    }
}

#[test]
fn the_example_file_sets_every_setting_but_secrets_to_its_default() {
    let text = read("deploy/kaeso.example.toml");
    let mut values = Vec::new();
    flatten("", &toml::from_str(&text).unwrap(), &mut values);
    for s in SETTINGS {
        let value = values.iter().find(|(k, _)| k == s.key).map(|(_, v)| v);
        match (s.secret, s.default, value) {
            (true, _, None) => {}
            (true, _, Some(_)) => panic!("{} is a secret and must not be in the example", s.key),
            (false, Some(default), Some(v)) => assert_eq!(v, default, "{}", s.key),
            (false, None, Some(_)) => {}
            (false, _, None) => panic!("{} is missing from deploy/kaeso.example.toml", s.key),
        }
    }
    assert_eq!(
        values.len(),
        SETTINGS.iter().filter(|s| !s.secret).count(),
        "deploy/kaeso.example.toml sets something that is not a setting"
    );
}

#[test]
fn the_example_file_is_a_valid_configuration() {
    let source = MapSource::new()
        .set("KAESO_CONFIG", "/etc/kaeso/kaeso.toml")
        .file("/etc/kaeso/kaeso.toml", read("deploy/kaeso.example.toml"))
        .set(
            "KAESO_MASTER_KEYS",
            kaeso_crypto::generate_master_key().expose_secret(),
        )
        .set(
            "KAESO_POSTGRES_URL",
            "postgres://kaeso:secret@postgres:5432/kaeso",
        )
        .set(
            "KAESO_CLICKHOUSE_URL",
            "http://kaeso:secret@clickhouse:8123",
        )
        .set("KAESO_VALKEY_URL", "redis://valkey:6379");
    let config = load(&source).unwrap();
    assert_eq!(config.public_url.as_str(), "https://kaeso.example.com/");
}
