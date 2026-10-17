#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

mod common;

use axum::response::IntoResponse;
use base64::Engine;
use kaeso_codecs::dispatch;
use kaeso_kpp::{DialectId, ErrorKind, KppError};
use kaeso_server::platform::error::ApiError;
use serde_json::Value;
use sha2::{Digest, Sha256};

fn compose() -> String {
    std::fs::read_to_string(format!(
        "{}/../../deploy/compose.yaml",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn config(name: &str) -> String {
    let text = compose();
    let start = text
        .find(&format!("\n  {name}:\n    content: |\n"))
        .unwrap_or_else(|| panic!("compose.yaml has no config {name}"));
    let body = &text[start..];
    let body = &body[body.find("|\n").unwrap() + 2..];
    let mut out = String::new();
    for line in body.lines() {
        if line.is_empty() {
            out.push('\n');
        } else if let Some(rest) = line.strip_prefix("      ") {
            out.push_str(rest);
            out.push('\n');
        } else {
            break;
        }
    }
    out.trim_end_matches('\n').to_owned() + "\n"
}

fn responses(caddyfile: &str) -> Vec<(String, u16)> {
    caddyfile
        .lines()
        .filter_map(|l| {
            let rest = l.trim().strip_prefix("respond `")?;
            let (body, status) = rest.split_once("` ")?;
            Some((body.to_owned(), status.trim().parse().unwrap()))
        })
        .collect()
}

fn messages(caddyfile: &str) -> Vec<String> {
    caddyfile
        .lines()
        .filter_map(|l| {
            let l = l.trim().strip_prefix("vars ")?;
            let start = l.find('"')?;
            Some(l[start + 1..l.len() - 1].to_owned())
        })
        .collect()
}

#[test]
fn the_edge_answers_each_api_in_the_gateways_own_error_format() {
    let caddyfile = config("caddyfile");
    let bodies = responses(&caddyfile);
    assert_eq!(bodies.len(), 3, "{bodies:?}");
    let texts = messages(&caddyfile);
    assert_eq!(texts.len(), 2, "{texts:?}");
    for message in &texts {
        assert_eq!(common::copy::sent_problem(message), None, "{message}");
        let filled: Vec<(Value, u16)> = bodies
            .iter()
            .map(|(b, s)| {
                (
                    serde_json::from_str(&b.replace("{vars.message}", message)).unwrap(),
                    *s,
                )
            })
            .collect();
        let platform = ApiError::Unavailable(message.clone()).into_response();
        assert_eq!(filled[0].1, platform.status().as_u16());
        let platform_body = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(axum::body::to_bytes(platform.into_body(), 1 << 16))
            .unwrap();
        assert_eq!(
            filled[0].0,
            serde_json::from_slice::<Value>(&platform_body).unwrap()
        );
        for (dialect, (body, status)) in [
            (DialectId::AnthropicMessages, &filled[1]),
            (DialectId::OpenAiChat, &filled[2]),
        ] {
            let encoded = dispatch::encode_error(
                dialect,
                &KppError::new(ErrorKind::Overloaded, message.clone()),
            );
            assert_eq!(*status, encoded.status, "{dialect:?}");
            assert_eq!(
                *body,
                serde_json::from_slice::<Value>(&encoded.body).unwrap(),
                "{dialect:?}"
            );
        }
    }
}

fn inline(page: &str, tag: &str) -> String {
    let open = format!("<{tag}>");
    let start = page.find(&open).unwrap() + open.len();
    let end = page[start..].find(&format!("</{tag}>")).unwrap() + start;
    page[start..end].to_owned()
}

fn hash(text: &str) -> String {
    format!(
        "'sha256-{}'",
        base64::engine::general_purpose::STANDARD.encode(Sha256::digest(text.as_bytes()))
    )
}

#[test]
fn the_updating_page_is_allowed_exactly_its_own_style_and_script() {
    let page = config("updating_page");
    let caddyfile = config("caddyfile");
    let csp = caddyfile
        .lines()
        .find_map(|l| l.trim().strip_prefix("header Content-Security-Policy "))
        .unwrap()
        .trim_matches('"')
        .to_owned();
    assert!(csp.starts_with("default-src 'none';"), "{csp}");
    assert!(
        csp.contains(&format!("style-src {};", hash(&inline(&page, "style")))),
        "{csp}"
    );
    assert!(
        csp.contains(&format!("script-src {};", hash(&inline(&page, "script")))),
        "{csp}"
    );
    assert!(csp.contains("connect-src 'self';"));
    assert_eq!(page.matches("<style>").count(), 1);
    assert_eq!(page.matches("<script>").count(), 1);
}

#[test]
fn the_updating_page_loads_nothing_from_elsewhere_and_speaks_three_languages() {
    let page = config("updating_page");
    for forbidden in ["src=", "href=", "http:", "https:", "@import", "url("] {
        assert!(!page.contains(forbidden), "the page contains {forbidden}");
    }
    let script = inline(&page, "script");
    let keys = |lang: &str| -> Vec<String> {
        let start = script.find(&format!("{lang}: {{")).unwrap();
        let block = &script[start..start + script[start..].find("\n    }").unwrap()];
        block
            .lines()
            .skip(1)
            .filter_map(|l| l.trim().split_once(':').map(|(k, _)| k.to_owned()))
            .collect()
    };
    let english = keys("en");
    assert!(english.len() >= 10, "{english:?}");
    assert_eq!(keys("de"), english);
    assert_eq!(keys("\"zh-CN\""), english);
    assert!(page.starts_with("<!doctype html>\n<html lang=\"en\">"));
}
