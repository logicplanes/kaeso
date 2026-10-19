use std::path::{Path, PathBuf};

use kaeso_kpp::{RequestKind, StreamMode};

use crate::EgressCtx;
use crate::testing::{json_bytes_diff, sse_diff};

pub(crate) fn egress() -> EgressCtx {
    EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off)
}

pub(crate) fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

pub(crate) fn fixtures(dialect: &str, folder: &str, extension: &str) -> Vec<(String, Vec<u8>)> {
    fixtures_in(&fixtures_dir(), dialect, folder, extension)
}

fn fixtures_in(
    root: &Path,
    dialect: &str,
    folder: &str,
    extension: &str,
) -> Vec<(String, Vec<u8>)> {
    let dir = root.join(dialect).join(folder);
    let mut out = files(&dir, extension, "");
    let providers = root.join(dialect).join(PROVIDERS);
    if providers.is_dir() {
        for provider in std::fs::read_dir(&providers).unwrap() {
            let provider = provider.unwrap().path();
            let own = provider.join(folder);
            if own.is_dir() {
                let id = provider.file_name().unwrap().to_str().unwrap();
                out.extend(files(&own, extension, &format!("{PROVIDERS}/{id}/")));
            }
        }
    }
    out.sort();
    assert!(
        !out.is_empty(),
        "no *.{extension} fixtures in {}",
        dir.display()
    );
    out
}

const PROVIDERS: &str = "providers";

fn files(dir: &Path, extension: &str, prefix: &str) -> Vec<(String, Vec<u8>)> {
    std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|p| p.is_file())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some(extension))
        .filter(|p| {
            let name = p.file_name().unwrap().to_str().unwrap();
            !name.ends_with(".headers.json") && !name.ends_with(".meta.json")
        })
        .map(|p| {
            let name = p.file_stem().unwrap().to_str().unwrap();
            (format!("{prefix}{name}"), std::fs::read(&p).unwrap())
        })
        .collect()
}

fn location(root: &Path, dialect: &str, folder: &str, name: &str) -> PathBuf {
    match name
        .strip_prefix(PROVIDERS)
        .and_then(|rest| rest.strip_prefix('/'))
        .and_then(|rest| rest.split_once('/'))
    {
        Some((id, stem)) => root
            .join(dialect)
            .join(PROVIDERS)
            .join(id)
            .join(folder)
            .join(stem),
        None => root.join(dialect).join(folder).join(name),
    }
}

pub(crate) fn sidecar(
    dialect: &str,
    folder: &str,
    name: &str,
    suffix: &str,
) -> Option<serde_json::Value> {
    let mut path = location(&fixtures_dir(), dialect, folder, name).into_os_string();
    path.push(format!(".{suffix}.json"));
    std::fs::read(&path)
        .ok()
        .map(|b| serde_json::from_slice(&b).unwrap())
}

#[track_caller]
pub(crate) fn assert_json_eq(name: &str, expected: &[u8], actual: &[u8]) {
    if let Some(d) = json_bytes_diff(expected, actual) {
        panic!(
            "{name}: round trip differs at {d}\n--- actual ---\n{}",
            String::from_utf8_lossy(actual)
        );
    }
}

#[track_caller]
pub(crate) fn assert_sse_eq(name: &str, expected: &[u8], actual: &[u8]) {
    if let Some(d) = sse_diff(expected, actual) {
        panic!(
            "{name}: stream round trip differs: {d}\n--- actual ---\n{}",
            String::from_utf8_lossy(actual)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_recordings_join_the_dialect_fixtures_under_their_provider() {
        let root = std::env::temp_dir().join(format!("kaeso-fixtures-{}", std::process::id()));
        let write = |path: &str, body: &str| {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, body).unwrap();
        };
        write("openai_chat/responses/basic.json", "{}");
        write("openai_chat/responses/RECORDED.md", "");
        write(
            "openai_chat/providers/groq/responses/recorded-basic.json",
            "[]",
        );
        write(
            "openai_chat/providers/groq/responses/recorded-basic.meta.json",
            "{}",
        );
        write("openai_chat/providers/groq/streams/recorded-basic.sse", "");
        write(
            "openai_chat/providers/zai/responses/recorded-basic.json",
            "1",
        );
        let names: Vec<(String, Vec<u8>)> = fixtures_in(&root, "openai_chat", "responses", "json");
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(
            names,
            [
                ("basic".to_owned(), b"{}".to_vec()),
                ("providers/groq/recorded-basic".to_owned(), b"[]".to_vec()),
                ("providers/zai/recorded-basic".to_owned(), b"1".to_vec()),
            ]
        );
        assert_eq!(
            location(&root, "openai_chat", "errors", "providers/groq/recorded-x"),
            root.join("openai_chat/providers/groq/errors/recorded-x")
        );
        assert_eq!(
            location(&root, "openai_chat", "requests/count_tokens", "basic"),
            root.join("openai_chat/requests/count_tokens/basic")
        );
    }
}
