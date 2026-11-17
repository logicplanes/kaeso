#![allow(clippy::unwrap_used)]

use kaeso_kpp::DialectId;

// The fuzz workspace builds on its own, so nothing else notices a dialect without a target; the
// fuzz workflow's own check only compares fuzz/Cargo.toml with its matrix.
#[test]
fn every_dialect_has_a_fuzz_target() {
    let root = format!("{}/../..", env!("CARGO_MANIFEST_DIR"));
    let manifest = std::fs::read_to_string(format!("{root}/fuzz/Cargo.toml")).unwrap();
    let missing: Vec<String> = DialectId::ALL
        .iter()
        .map(|d| format!("decode_{}", d.as_str()))
        .filter(|target| {
            !manifest.contains(&format!("name = \"{target}\""))
                || !std::path::Path::new(&format!("{root}/fuzz/fuzz_targets/{target}.rs")).exists()
        })
        .collect();
    assert!(missing.is_empty(), "no fuzz target for {missing:?}");
}
