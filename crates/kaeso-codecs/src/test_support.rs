use std::path::{Path, PathBuf};

use crate::testing::{json_bytes_diff, sse_diff};

pub(crate) fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

pub(crate) fn fixtures(dialect: &str, folder: &str, extension: &str) -> Vec<(String, Vec<u8>)> {
    let dir = fixtures_dir().join(dialect).join(folder);
    let mut out: Vec<(String, Vec<u8>)> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|p| p.is_file())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some(extension))
        .filter(|p| {
            let name = p.file_name().unwrap().to_str().unwrap();
            !name.ends_with(".headers.json") && !name.ends_with(".meta.json")
        })
        .map(|p| {
            let name = p.file_stem().unwrap().to_str().unwrap().to_owned();
            (name, std::fs::read(&p).unwrap())
        })
        .collect();
    out.sort();
    assert!(
        !out.is_empty(),
        "no *.{extension} fixtures in {}",
        dir.display()
    );
    out
}

pub(crate) fn sidecar(
    dialect: &str,
    folder: &str,
    name: &str,
    suffix: &str,
) -> Option<serde_json::Value> {
    let path = fixtures_dir()
        .join(dialect)
        .join(folder)
        .join(format!("{name}.{suffix}.json"));
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
