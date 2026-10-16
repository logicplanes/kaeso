#![allow(clippy::unwrap_used)]

use std::collections::HashSet;

use kaeso_core::provider::{PROVIDERS, ProviderId};
use kaeso_server::config::{
    MapSource, PROVIDER_FIELDS, REQUIRED, load, parse_provider_env, provider_env, provider_key,
    settings,
};
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
    for s in settings() {
        let kind = if s.secret {
            format!("{}, secret", s.kind.doc_name())
        } else {
            s.kind.doc_name().to_owned()
        };
        let default = match s.default {
            Some(d) => format!("`{}`", cell(d)),
            None if REQUIRED.contains(&s.key.as_ref()) => "required".to_owned(),
            None => "—".to_owned(),
        };
        lines.push(format!(
            "| `{}` | `{}` | {kind} | {default} | {} |",
            s.env,
            s.key,
            cell(&s.description)
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
    for s in settings() {
        let expected = format!("KAESO_{}", s.key.to_uppercase().replace(['.', '-'], "_"));
        assert_eq!(s.env, expected, "environment name of {}", s.key);
    }
}

#[test]
fn every_known_provider_has_every_provider_field_as_a_setting() {
    for p in PROVIDERS {
        for f in PROVIDER_FIELDS {
            let id = p.provider_id();
            let key = provider_key(&id, f);
            let s = settings()
                .iter()
                .find(|s| s.key == key)
                .unwrap_or_else(|| panic!("{key} is not a setting"));
            assert_eq!(s.env, provider_env(&id, f));
            assert_eq!(s.kind, f.kind);
            assert_eq!(s.default, (f.default)(p));
            assert!(!s.secret, "{key}");
        }
    }
    let provider_rows = settings()
        .iter()
        .filter(|s| s.key.starts_with("providers."))
        .count();
    assert_eq!(provider_rows, PROVIDERS.len() * PROVIDER_FIELDS.len());
}

#[test]
fn provider_fields_are_snake_case_and_none_ends_another() {
    for f in PROVIDER_FIELDS {
        let snake = f.name.split('_').all(|w| {
            !w.is_empty()
                && w.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        });
        assert!(snake, "{} is not snake case", f.name);
        for other in PROVIDER_FIELDS.iter().filter(|o| o.name != f.name) {
            assert!(
                !other.name.ends_with(&format!("_{}", f.name)),
                "`{}` ends in `_{}`, so a variable ending in it could name two settings",
                other.name,
                f.name
            );
        }
    }
    let names: HashSet<&str> = PROVIDER_FIELDS.iter().map(|f| f.name).collect();
    assert_eq!(names.len(), PROVIDER_FIELDS.len());
}

#[test]
fn provider_variables_map_back_to_exactly_one_provider_and_field() {
    for p in PROVIDERS {
        for f in PROVIDER_FIELDS {
            let env = provider_env(&p.provider_id(), f);
            let (provider, field) = parse_provider_env(&env).unwrap();
            assert_eq!((provider.id, field.name), (p.id, f.name), "{env}");
        }
    }
    let field = PROVIDER_FIELDS.first().unwrap();
    for (id, env) in [
        ("azure-openai", "KAESO_PROVIDERS_AZURE_OPENAI_BASE_URL"),
        ("together-ai", "KAESO_PROVIDERS_TOGETHER_AI_BASE_URL"),
        ("302ai", "KAESO_PROVIDERS_302AI_BASE_URL"),
    ] {
        let id: ProviderId = id.parse().unwrap();
        assert_eq!(provider_env(&id, field), env);
        assert_eq!(provider_key(&id, field), format!("providers.{id}.base_url"));
        let segment = env
            .strip_prefix("KAESO_PROVIDERS_")
            .and_then(|r| r.strip_suffix("_BASE_URL"))
            .unwrap();
        assert_eq!(ProviderId::from_env_segment(segment).unwrap(), id);
    }
}

#[test]
fn keys_and_environment_names_are_unique() {
    let mut keys = HashSet::new();
    let mut envs = HashSet::new();
    for s in settings() {
        assert!(keys.insert(s.key.as_ref()), "{} is listed twice", s.key);
        assert!(envs.insert(s.env.as_ref()), "{} is listed twice", s.env);
    }
    for s in settings().iter().filter(|s| s.secret) {
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
    for s in settings() {
        let table = format!("{}.", s.key);
        let nested: Vec<&str> = settings()
            .iter()
            .filter(|o| o.key.starts_with(&table))
            .map(|o| o.key.as_ref())
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
    for s in settings() {
        let d = s.description.as_ref();
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
        let s = settings()
            .iter()
            .find(|s| s.key == *key)
            .unwrap_or_else(|| panic!("required setting {key} is not a setting"));
        assert!(s.default.is_none(), "required setting {key} has a default");
    }
}

#[test]
fn secrets_have_no_default() {
    for s in settings().iter().filter(|s| s.secret) {
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
    for s in settings() {
        let value = values.iter().find(|(k, _)| *k == s.key).map(|(_, v)| v);
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
        settings().iter().filter(|s| !s.secret).count(),
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

#[test]
fn an_empty_file_value_trusts_no_proxy_and_an_empty_variable_keeps_the_default() {
    let base = || {
        MapSource::new()
            .set("KAESO_PUBLIC_URL", "https://kaeso.example.com")
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
            .set("KAESO_VALKEY_URL", "redis://valkey:6379")
            .set("KAESO_MAIL_TRANSPORT", "log")
    };
    let none = load(
        &base()
            .set("KAESO_CONFIG", "/etc/kaeso/kaeso.toml")
            .file("/etc/kaeso/kaeso.toml", "trusted_proxies = \"\"\n"),
    )
    .unwrap();
    assert!(none.trusted_proxies.is_empty());
    let default = load(&base().set("KAESO_TRUSTED_PROXIES", "")).unwrap();
    assert_eq!(default.trusted_proxies.len(), 6);
}
