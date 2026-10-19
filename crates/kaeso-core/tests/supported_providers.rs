#![allow(clippy::unwrap_used)]

use kaeso_core::provider::{PROVIDERS, Provider};
use kaeso_kpp::DialectId;

const PAGE: &str = "concepts/supported-providers.md";

struct Language {
    dir: &'static str,
    separator: &'static str,
    token_counts: &'static str,
}

const LANGUAGES: [Language; 3] = [
    Language {
        dir: "en",
        separator: ", ",
        token_counts: "token counts",
    },
    Language {
        dir: "de",
        separator: ", ",
        token_counts: "Token-Zählung",
    },
    Language {
        dir: "zh-CN",
        separator: "、",
        token_counts: "Token 计数",
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

fn expected_row(p: &Provider, language: &Language) -> (String, String) {
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
    (
        format!("{} `{}`", p.name, p.id),
        formats.join(language.separator),
    )
}

fn table_rows(text: &str) -> Vec<(String, String)> {
    text.lines()
        .skip_while(|line| !line.starts_with("| ---"))
        .skip(1)
        .take_while(|line| line.starts_with('|'))
        .map(|line| {
            let mut cells = line.trim_matches('|').split(" | ").map(str::trim);
            let mut next = || cells.next().unwrap_or_default().to_owned();
            (next(), next())
        })
        .collect()
}

#[test]
fn the_supported_providers_page_lists_every_provider_by_name_with_its_formats() {
    let mut providers: Vec<&Provider> = PROVIDERS.iter().collect();
    providers.sort_by_key(|p| p.name.to_lowercase());
    for language in &LANGUAGES {
        let (path, text) = page(language);
        let expected: Vec<(String, String)> = providers
            .iter()
            .map(|p| expected_row(p, language))
            .collect();
        assert_eq!(
            table_rows(&text),
            expected,
            "{path} has one row per provider in crates/kaeso-core/src/provider.rs, sorted by name: \
             the name and the id, then the formats, with the default first"
        );
    }
}
