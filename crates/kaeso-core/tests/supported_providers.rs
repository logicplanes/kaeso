#![allow(clippy::unwrap_used)]

use kaeso_core::provider::{PROVIDERS, Provider};
use kaeso_kpp::DialectId;

const PAGE: &str = "concepts/supported-providers.md";

struct Language {
    dir: &'static str,
    separator: &'static str,
    token_counts: &'static str,
    key_separator: &'static str,
    no_prefix: &'static str,
}

const LANGUAGES: [Language; 3] = [
    Language {
        dir: "en",
        separator: ", ",
        token_counts: "token counts",
        key_separator: ", ",
        no_prefix: "no fixed prefix",
    },
    Language {
        dir: "de",
        separator: ", ",
        token_counts: "Token-Zählung",
        key_separator: ", ",
        no_prefix: "kein festes Präfix",
    },
    Language {
        dir: "zh-CN",
        separator: "、",
        token_counts: "Token 计数",
        key_separator: "，",
        no_prefix: "无固定前缀",
    },
];

fn page(language: &Language) -> (String, String) {
    let path = format!("apps/docs/content/{}/{PAGE}", language.dir);
    let full = format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR"));
    (path, std::fs::read_to_string(full).unwrap())
}

fn format_name(dialect: DialectId) -> &'static str {
    let name = dialect.display_name();
    name.split_once(' ').map_or(name, |(_, format)| format)
}

fn host(base_url: &str) -> &str {
    let rest = base_url
        .split_once("://")
        .map_or(base_url, |(_, rest)| rest);
    rest.split('/').next().unwrap_or(rest)
}

type Row = (String, String, String, String);

fn expected_row(p: &Provider, language: &Language) -> Row {
    let mut formats = vec![format_name(p.generation)];
    for d in p.dialects.iter().copied().filter(|d| *d != p.generation) {
        formats.push(format_name(d));
    }
    if let Some(d) = p.embeddings
        && !p.dialects.contains(&d)
    {
        formats.push(format_name(d));
    }
    if p.count_tokens.is_some() {
        formats.push(language.token_counts);
    }
    let prefix = p.key_prefix.map_or_else(
        || language.no_prefix.to_owned(),
        |prefix| format!("`{prefix}`"),
    );
    (
        format!("{} `{}`", p.name, p.id),
        formats.join(language.separator),
        format!("`{}`", host(p.default_base_url)),
        format!(
            "[{}]({}){}{prefix}",
            host(p.key_page.url),
            p.key_page.url,
            language.key_separator
        ),
    )
}

fn table_rows(text: &str) -> Vec<Row> {
    text.lines()
        .skip_while(|line| !line.starts_with("| ---"))
        .skip(1)
        .take_while(|line| line.starts_with('|'))
        .map(|line| {
            let mut cells = line.trim_matches('|').split(" | ").map(str::trim);
            let mut next = || cells.next().unwrap_or_default().to_owned();
            (next(), next(), next(), next())
        })
        .collect()
}

#[test]
fn the_supported_providers_page_lists_every_provider_by_name_with_its_formats_host_and_key() {
    let mut providers: Vec<&Provider> = PROVIDERS.iter().collect();
    providers.sort_by_key(|p| p.name.to_lowercase());
    for language in &LANGUAGES {
        let (path, text) = page(language);
        let expected: Vec<Row> = providers
            .iter()
            .map(|p| expected_row(p, language))
            .collect();
        assert_eq!(
            table_rows(&text),
            expected,
            "{path} has one row per provider in crates/kaeso-core/src/provider.rs, sorted by name: \
             the name and the id, the formats with the default first, the host of the default base URL, \
             then the key page linked by its host and the key's prefix"
        );
    }
}
