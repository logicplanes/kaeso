use std::fmt::Write as _;
use std::io::Write;
use std::path::{Path, PathBuf};

use kaeso_core::provider::PROVIDERS;
use serde::Serialize;
use serde_json::ser::{PrettyFormatter, Serializer};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use time::{Duration, OffsetDateTime};

use crate::error::{Error, read_text, write};

const FETCHED_AT: &str = "pub const EMBEDDED_FETCHED_AT: OffsetDateTime = datetime!(";

pub(crate) struct Paths {
    snapshot: PathBuf,
    readme: PathBuf,
    source: PathBuf,
}

impl Paths {
    pub(crate) fn in_repository(root: &Path) -> Self {
        let core = root.join("crates/kaeso-core");
        Self {
            snapshot: core.join("data/models-dev.json"),
            readme: core.join("data/README.md"),
            source: core.join("src/models_dev.rs"),
        }
    }
}

pub(crate) async fn refresh(paths: &Paths, url: &str, out: &mut impl Write) -> Result<(), Error> {
    let api = reqwest::get(url).await?.error_for_status()?.bytes().await?;
    let now = OffsetDateTime::now_utc();
    let fetched = now - Duration::nanoseconds(i64::from(now.nanosecond()));
    let (snapshot, models) = filter(&api)?;
    let digest = hex(&Sha256::digest(snapshot.as_bytes()));
    let stamp = fetched.format(&Rfc3339)?;
    let readme = readme(&read_text(&paths.readme)?, &stamp, models, &digest)?;
    let source = fetched_at(&read_text(&paths.source)?, fetched)?;
    write(&paths.snapshot, &snapshot)?;
    write(&paths.readme, readme)?;
    write(&paths.source, source)?;
    writeln!(
        out,
        "{}: {models} models, fetched {stamp}, sha256 {digest}",
        paths.snapshot.display()
    )?;
    Ok(())
}

fn filter(api: &[u8]) -> Result<(String, usize), Error> {
    let mut all: Map<String, Value> =
        serde_json::from_slice(api).map_err(|e| Error::json("the models.dev catalogue", e))?;
    let kept: Map<String, Value> = PROVIDERS
        .iter()
        .filter_map(|p| all.remove(p.id).map(|v| (p.id.to_owned(), v)))
        .collect();
    let models = kept
        .values()
        .filter_map(|p| p.get("models").and_then(Value::as_object))
        .map(Map::len)
        .sum();
    let mut bytes = Vec::new();
    Value::Object(kept)
        .serialize(&mut Serializer::with_formatter(
            &mut bytes,
            PrettyFormatter::with_indent(b" "),
        ))
        .map_err(|e| Error::json("the filtered snapshot", e))?;
    bytes.push(b'\n');
    kaeso_core::models_dev::parse(&bytes, &kaeso_core::provider::known_ids())
        .map_err(|e| Error::Invalid(format!("the gateway cannot read the new snapshot: {e}")))?;
    let snapshot = String::from_utf8(bytes)
        .map_err(|e| Error::Invalid(format!("the snapshot is not UTF-8: {e}")))?;
    Ok((snapshot, models))
}

fn readme(text: &str, stamp: &str, models: usize, digest: &str) -> Result<String, Error> {
    let (mut fetched, mut content, mut sha) = (false, false, false);
    let mut lines = Vec::new();
    for line in text.split('\n') {
        let counted = line
            .strip_prefix("| Content |")
            .and_then(|_| line.rsplit_once("; "))
            .filter(|(_, tail)| tail.ends_with(" models |"));
        if line.starts_with("| Fetched |") {
            fetched = true;
            lines.push(format!("| Fetched | {stamp} |"));
        } else if line.starts_with("| SHA-256 |") {
            sha = true;
            lines.push(format!("| SHA-256 | `{digest}` |"));
        } else if let Some((head, _)) = counted {
            content = true;
            lines.push(format!("{head}; {models} models |"));
        } else {
            lines.push(line.to_owned());
        }
    }
    if fetched && content && sha {
        Ok(lines.join("\n"))
    } else {
        Err(Error::Invalid(
            "the data README lacks its Fetched, Content or SHA-256 row".to_owned(),
        ))
    }
}

fn fetched_at(source: &str, fetched: OffsetDateTime) -> Result<String, Error> {
    let stamp = fetched.format(format_description!(
        "[year]-[month]-[day] [hour]:[minute]:[second]"
    ))?;
    let mut found = false;
    let lines: Vec<String> = source
        .split('\n')
        .map(|line| {
            if line.starts_with(FETCHED_AT) {
                found = true;
                format!("{FETCHED_AT}{stamp} UTC);")
            } else {
                line.to_owned()
            }
        })
        .collect();
    if !found {
        return Err(Error::Invalid(
            "crates/kaeso-core/src/models_dev.rs lacks EMBEDDED_FETCHED_AT".to_owned(),
        ));
    }
    Ok(lines.join("\n"))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::with_capacity(64), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::*;
    use crate::testing::{Reply, TempDir, serve_once};

    const README: &str = "# Model data\n\n| | |\n|---|---|\n| Source | `x` |\n| Fetched | 2026-01-01T00:00:00Z |\n| Content | the providers Kaeso calls (`anthropic`, `openai`), unchanged otherwise; 3 models |\n| SHA-256 | `00` |\n";

    const API: &str = r#"{"zeta":{"id":"zeta","models":{"z":{"id":"z"}}},"openai":{"name":"OpenAI","id":"openai","models":{"gpt-b":{"id":"gpt-b","cost":{"input":1.25,"output":10}},"gpt-a":{"id":"gpt-a","name":"é"}}},"anthropic":{"id":"anthropic","models":{"claude-x":{"id":"claude-x"}}}}"#;

    #[test]
    fn the_shipped_snapshot_is_what_filter_writes() {
        let shipped = include_str!("../../kaeso-core/data/models-dev.json");
        let (snapshot, models) = filter(shipped.as_bytes()).unwrap();
        assert_eq!(snapshot, shipped);
        let readme = include_str!("../../kaeso-core/data/README.md");
        let digest = hex(&Sha256::digest(shipped.as_bytes()));
        assert!(readme.contains(&format!("| SHA-256 | `{digest}` |")));
        assert!(readme.contains(&format!("; {models} models |")));
    }

    #[test]
    fn filter_keeps_the_providers_kaeso_calls_with_sorted_keys() {
        let (snapshot, models) = filter(API.as_bytes()).unwrap();
        assert_eq!(models, 3);
        assert!(!snapshot.contains("zeta"));
        assert!(snapshot.starts_with("{\n \"anthropic\": {\n  \"id\": \"anthropic\",\n"));
        assert!(
            snapshot.contains("\"gpt-a\": {\n    \"id\": \"gpt-a\",\n    \"name\": \"é\"\n   },")
        );
        assert!(snapshot.ends_with("}\n"));
        assert!(filter(b"[]").is_err());
        assert!(filter(br#"{"openai":{"id":"openai","models":[]}}"#).is_err());
    }

    #[test]
    fn readme_rows_are_rewritten_and_required() {
        let text = readme(README, "2026-10-16T08:00:00Z", 70, "abc").unwrap();
        assert!(text.contains("| Fetched | 2026-10-16T08:00:00Z |\n"));
        assert!(text.contains("unchanged otherwise; 70 models |\n"));
        assert!(text.contains("| SHA-256 | `abc` |\n"));
        assert!(text.contains("| Source | `x` |\n"));
        assert!(readme("# Model data\n", "x", 1, "y").is_err());
    }

    #[test]
    fn the_fetch_time_constant_is_rewritten() {
        let source = "use x;\n\npub const EMBEDDED_FETCHED_AT: OffsetDateTime = datetime!(2026-10-11 20:44:44 UTC);\n";
        let updated = fetched_at(source, datetime!(2026-10-16 08:09:10 UTC)).unwrap();
        assert_eq!(
            updated,
            "use x;\n\npub const EMBEDDED_FETCHED_AT: OffsetDateTime = datetime!(2026-10-16 08:09:10 UTC);\n"
        );
        assert!(fetched_at("fn main() {}\n", datetime!(2026-10-16 08:09:10 UTC)).is_err());
    }

    #[tokio::test]
    async fn refresh_writes_the_snapshot_readme_and_constant() {
        let dir = TempDir::new();
        dir.file("crates/kaeso-core/data/README.md", README);
        dir.file(
            "crates/kaeso-core/src/models_dev.rs",
            "pub const EMBEDDED_FETCHED_AT: OffsetDateTime = datetime!(2026-01-01 00:00:00 UTC);\n",
        );
        let served = serve_once(Reply {
            status: 200,
            headers: vec![("content-type", "application/json")],
            body: API.as_bytes().to_vec(),
        })
        .await;
        let mut out = Vec::new();
        let paths = Paths::in_repository(dir.path());
        refresh(&paths, &format!("{}/api.json", served.url), &mut out)
            .await
            .unwrap();
        assert!(served.request.await.unwrap().starts_with("GET /api.json "));
        let snapshot = dir.read("crates/kaeso-core/data/models-dev.json");
        assert_eq!(snapshot, filter(API.as_bytes()).unwrap().0);
        let digest = hex(&Sha256::digest(snapshot.as_bytes()));
        let readme = dir.read("crates/kaeso-core/data/README.md");
        assert!(readme.contains(&format!("| SHA-256 | `{digest}` |")));
        assert!(readme.contains("; 3 models |"));
        assert!(!readme.contains("2026-01-01T00:00:00Z"));
        assert!(
            !dir.read("crates/kaeso-core/src/models_dev.rs")
                .contains("2026-01-01 00:00:00")
        );
        let report = String::from_utf8(out).unwrap();
        assert!(report.contains(": 3 models, fetched "), "{report}");
        assert!(report.ends_with(&format!("sha256 {digest}\n")), "{report}");
    }

    #[tokio::test]
    async fn refresh_fails_on_an_http_error_and_writes_nothing() {
        let dir = TempDir::new();
        let served = serve_once(Reply {
            status: 503,
            headers: vec![],
            body: b"down".to_vec(),
        })
        .await;
        let paths = Paths::in_repository(dir.path());
        let err = refresh(&paths, &served.url, &mut Vec::new())
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Http(_)), "{err}");
        assert!(!dir.path().join("crates").exists());
    }
}
