use std::fs;
use std::path::Path;

use kaeso_mcp::Message;
use serde_json::Value;

#[test]
fn every_fixture_parses_and_round_trips() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    let mut seen = 0;
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let bytes = fs::read(&path).unwrap();
        let message = Message::parse(&bytes)
            .unwrap_or_else(|e| panic!("{} doesn't parse: {}", path.display(), e.message));
        let written = message.to_vec();
        let original: Value = serde_json::from_slice(&bytes).unwrap();
        let back: Value = serde_json::from_slice(&written).unwrap();
        assert_eq!(back, original, "{} lost something", path.display());
        assert_eq!(Message::parse(&written).unwrap().to_vec(), written);
        seen += 1;
    }
    assert_eq!(seen, 10);
}
