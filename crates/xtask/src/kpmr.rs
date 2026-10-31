use std::fmt::Write as _;
use std::io::Write;
use std::path::{Path, PathBuf};

use kaeso_core::kpmr::{self, TrustedKey};
use kaeso_core::provider::{PROVIDERS, known_ids};
use sha2::{Digest, Sha256};
use time::format_description::well_known::Rfc3339;

use crate::error::{Error, read_text, write};

pub(crate) struct Paths {
    catalogue: PathBuf,
    readme: PathBuf,
}

impl Paths {
    pub(crate) fn in_repository(root: &Path) -> Self {
        let data = root.join("crates/kaeso-core/data");
        Self {
            catalogue: data.join("kpmr.json"),
            readme: data.join("README.md"),
        }
    }
}

pub(crate) fn trusted_keys(extra: Option<&str>) -> Result<Vec<TrustedKey>, Error> {
    let mut keys = vec![TrustedKey::built_in().map_err(|e| Error::Invalid(e.to_string()))?];
    for entry in extra
        .unwrap_or("")
        .split(',')
        .filter(|e| !e.trim().is_empty())
    {
        keys.push(TrustedKey::parse(entry).map_err(|e| Error::Invalid(e.to_string()))?);
    }
    Ok(keys)
}

fn with_providers(url: &str) -> Result<reqwest::Url, Error> {
    let mut url = reqwest::Url::parse(url).map_err(|e| Error::Invalid(format!("{url}: {e}")))?;
    let mut ids: Vec<&str> = PROVIDERS.iter().map(|p| p.id).collect();
    ids.sort_unstable();
    url.query_pairs_mut()
        .append_pair("providers", &ids.join(","));
    Ok(url)
}

pub(crate) async fn refresh(
    paths: &Paths,
    url: &str,
    keys: &[TrustedKey],
    out: &mut impl Write,
) -> Result<(), Error> {
    let resp = reqwest::Client::new()
        .get(with_providers(url)?)
        .header(reqwest::header::ACCEPT, kpmr::MEDIA_TYPE)
        .send()
        .await?
        .error_for_status()?;
    let signature = resp
        .headers()
        .get(kpmr::SIGNATURE_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let body = resp.bytes().await?;
    kpmr::verify(&body, signature.as_deref(), keys)
        .map_err(|e| Error::Invalid(format!("refusing the catalogue: {e}")))?;
    let parsed = kpmr::parse(&body, &known_ids())
        .map_err(|e| Error::Invalid(format!("the gateway cannot read the catalogue: {e}")))?;
    if !parsed.skipped.is_empty() {
        return Err(Error::Invalid(format!(
            "the gateway would skip these models: {}",
            parsed.skipped.join(", ")
        )));
    }
    let digest = hex(&Sha256::digest(&body));
    let rows = Rows::of(&parsed.catalogue, &digest)?;
    let readme = readme(&read_text(&paths.readme)?, &rows)?;
    write(&paths.catalogue, &body)?;
    write(&paths.readme, readme)?;
    writeln!(
        out,
        "{}: revision {}, {} models, sha256 {digest}",
        paths.catalogue.display(),
        parsed.catalogue.revision.as_deref().unwrap_or("unknown"),
        parsed.catalogue.model_count(),
    )?;
    Ok(())
}

struct Rows {
    revision: String,
    generated: String,
    content: String,
    sha: String,
}

impl Rows {
    fn of(catalogue: &kpmr::Catalogue, digest: &str) -> Result<Rows, Error> {
        let generated = match catalogue.generated_at {
            Some(at) => at.format(&Rfc3339)?,
            None => return Err(Error::Invalid("the catalogue has no generated_at".into())),
        };
        let revision = catalogue
            .revision
            .clone()
            .ok_or_else(|| Error::Invalid("the catalogue has no revision".into()))?;
        let names = catalogue
            .providers
            .keys()
            .map(|p| format!("`{p}`"))
            .collect::<Vec<_>>()
            .join(", ");
        Ok(Rows {
            revision: format!("| Revision | `{revision}` |"),
            generated: format!("| Generated | {generated} |"),
            content: format!(
                "| Content | the providers Kaeso calls ({names}); {} models |",
                catalogue.model_count()
            ),
            sha: format!("| SHA-256 | `{digest}` |"),
        })
    }
}

fn readme(text: &str, rows: &Rows) -> Result<String, Error> {
    let rows = [
        ("| Revision |", rows.revision.as_str()),
        ("| Generated |", rows.generated.as_str()),
        ("| Content |", rows.content.as_str()),
        ("| SHA-256 |", rows.sha.as_str()),
    ];
    let lines: Vec<&str> = text
        .split('\n')
        .map(|line| {
            rows.iter()
                .find(|(prefix, _)| line.starts_with(prefix))
                .map_or(line, |(_, row)| row)
        })
        .collect();
    if rows.iter().all(|(_, row)| lines.contains(row)) {
        Ok(lines.join("\n"))
    } else {
        Err(Error::Invalid(
            "the data README lacks its Revision, Generated, Content or SHA-256 row".to_owned(),
        ))
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::with_capacity(64), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

#[cfg(test)]
mod tests {
    use base64::Engine as _;
    use base64::engine::general_purpose::STANDARD;
    use ed25519_dalek::{Signer, SigningKey};

    use super::*;
    use crate::testing::{Reply, TempDir, serve_once};

    const README: &str = "# Model data\n\n| | |\n|---|---|\n| Source | `x` |\n| Revision | `old` |\n| Generated | 2026-01-01T00:00:00Z |\n| Content | the providers Kaeso calls (`anthropic`); 3 models |\n| SHA-256 | `00` |\n";

    const CATALOGUE: &str = r#"{"format":1,"revision":"2026-10-31T21:00:00Z-abc","generated_at":"2026-10-31T21:00:00Z","refresh_seconds":3600,"provider_ids":["anthropic","openai","zeta"],"providers":{"anthropic":{"id":"anthropic","models":{"claude-x":{"id":"claude-x"}}},"openai":{"id":"openai","name":"OpenAI","models":{"gpt-a":{"id":"gpt-a","name":"é"},"gpt-b":{"id":"gpt-b","pricing":{"unit":"usd_per_million_tokens","input":"1.25","output":"10"}}}}}}"#;

    fn test_key() -> (SigningKey, Vec<TrustedKey>) {
        let key = SigningKey::from_bytes(&[9; 32]);
        let public = STANDARD.encode(key.verifying_key().to_bytes());
        let keys = trusted_keys(Some(&format!("test-key:{public}"))).unwrap();
        (key, keys)
    }

    fn signed(key: &SigningKey, body: &[u8]) -> &'static str {
        let sig = STANDARD.encode(key.sign(body).to_bytes());
        Box::leak(format!("keyid=test-key, ed25519={sig}").into_boxed_str())
    }

    #[test]
    fn the_shipped_catalogue_and_the_readme_agree() {
        let shipped = include_str!("../../kaeso-core/data/kpmr.json");
        let parsed = kpmr::parse(shipped.as_bytes(), &known_ids()).unwrap();
        let digest = hex(&Sha256::digest(shipped.as_bytes()));
        let rows = Rows::of(&parsed.catalogue, &digest).unwrap();
        let readme = include_str!("../../kaeso-core/data/README.md");
        for row in [rows.revision, rows.generated, rows.content, rows.sha] {
            assert!(readme.contains(&format!("{row}\n")), "{row}");
        }
    }

    #[test]
    fn the_request_names_every_provider_kaeso_calls() {
        let url = with_providers("https://kpmr.example.com/v1/catalogue").unwrap();
        let query = url.query().unwrap();
        assert!(
            query.starts_with("providers=alibaba%2Canthropic%2C"),
            "{query}"
        );
        assert!(with_providers("not a url").is_err());
    }

    #[test]
    fn extra_keys_are_comma_separated_and_checked() {
        assert_eq!(trusted_keys(None).unwrap().len(), 1);
        assert_eq!(test_key().1.len(), 2);
        assert!(trusted_keys(Some("broken")).is_err());
    }

    #[test]
    fn readme_rows_are_rewritten_and_required() {
        let parsed = kpmr::parse(CATALOGUE.as_bytes(), &known_ids()).unwrap();
        let rows = Rows::of(&parsed.catalogue, "abc").unwrap();
        let text = readme(README, &rows).unwrap();
        assert!(text.contains("| Revision | `2026-10-31T21:00:00Z-abc` |\n"));
        assert!(text.contains("| Generated | 2026-10-31T21:00:00Z |\n"));
        assert!(text.contains(
            "| Content | the providers Kaeso calls (`anthropic`, `openai`); 3 models |\n"
        ));
        assert!(text.contains("| SHA-256 | `abc` |\n"));
        assert!(text.contains("| Source | `x` |\n"));
        assert!(readme("# Model data\n", &rows).is_err());
    }

    #[tokio::test]
    async fn refresh_writes_the_signed_catalogue_as_sent_and_the_readme() {
        let dir = TempDir::new();
        dir.file("crates/kaeso-core/data/README.md", README);
        let (key, keys) = test_key();
        let served = serve_once(Reply {
            status: 200,
            headers: vec![
                ("content-type", kpmr::MEDIA_TYPE),
                ("kpmr-signature", signed(&key, CATALOGUE.as_bytes())),
            ],
            body: CATALOGUE.as_bytes().to_vec(),
        })
        .await;
        let mut out = Vec::new();
        let paths = Paths::in_repository(dir.path());
        refresh(
            &paths,
            &format!("{}/v1/catalogue", served.url),
            &keys,
            &mut out,
        )
        .await
        .unwrap();
        let request = served.request.await.unwrap();
        assert!(
            request.starts_with("GET /v1/catalogue?providers=alibaba%2C"),
            "{request}"
        );
        assert!(
            request
                .to_ascii_lowercase()
                .contains("accept: application/vnd.kaeso.kpmr+json; version=1\r\n"),
            "{request}"
        );
        assert_eq!(dir.read("crates/kaeso-core/data/kpmr.json"), CATALOGUE);
        let digest = hex(&Sha256::digest(CATALOGUE.as_bytes()));
        let readme = dir.read("crates/kaeso-core/data/README.md");
        assert!(readme.contains(&format!("| SHA-256 | `{digest}` |")));
        assert!(readme.contains("; 3 models |"));
        let report = String::from_utf8(out).unwrap();
        assert!(
            report.ends_with(&format!(
                ": revision 2026-10-31T21:00:00Z-abc, 3 models, sha256 {digest}\n"
            )),
            "{report}"
        );
    }

    #[tokio::test]
    async fn refresh_refuses_a_catalogue_without_a_trusted_signature_and_writes_nothing() {
        let dir = TempDir::new();
        dir.file("crates/kaeso-core/data/README.md", README);
        let (_, keys) = test_key();
        let forger = SigningKey::from_bytes(&[4; 32]);
        for signature in [None, Some(signed(&forger, CATALOGUE.as_bytes()))] {
            let served = serve_once(Reply {
                status: 200,
                headers: signature
                    .map(|s| ("kpmr-signature", s))
                    .into_iter()
                    .collect(),
                body: CATALOGUE.as_bytes().to_vec(),
            })
            .await;
            let paths = Paths::in_repository(dir.path());
            let err = refresh(&paths, &served.url, &keys, &mut Vec::new())
                .await
                .unwrap_err();
            assert!(
                err.to_string().starts_with("refusing the catalogue"),
                "{err}"
            );
            assert!(!dir.path().join("crates/kaeso-core/data/kpmr.json").exists());
        }
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
        let err = refresh(&paths, &served.url, &test_key().1, &mut Vec::new())
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Http(_)), "{err}");
        assert!(!dir.path().join("crates").exists());
    }
}
