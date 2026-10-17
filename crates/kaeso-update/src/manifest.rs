use semver::Version;
use serde::{Deserialize, Serialize};

pub const ASSET: &str = "release.json";
pub const COMPOSE_ASSET: &str = "compose.yaml";
pub const SCHEMA: u32 = 1;
pub const MAX_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Service {
    Gateway,
    Console,
    Updater,
}

impl Service {
    pub const ALL: [Service; 3] = [Service::Gateway, Service::Console, Service::Updater];
    pub const SWITCHED: [Service; 2] = [Service::Gateway, Service::Console];

    pub fn as_str(self) -> &'static str {
        match self {
            Service::Gateway => "gateway",
            Service::Console => "console",
            Service::Updater => "updater",
        }
    }

    pub fn image(self) -> &'static str {
        match self {
            Service::Gateway => "ghcr.io/logicplanes/kaeso-gateway",
            Service::Console => "ghcr.io/logicplanes/kaeso-console",
            Service::Updater => "ghcr.io/logicplanes/kaeso-updater",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Image {
    pub name: String,
    pub digest: String,
}

impl Image {
    pub fn reference(&self) -> String {
        format!("{}@{}", self.name, self.digest)
    }

    pub fn hex(&self) -> &str {
        self.digest.strip_prefix("sha256:").unwrap_or(&self.digest)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Images {
    pub gateway: Image,
    pub console: Image,
    pub updater: Image,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asset {
    pub name: String,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub schema: u32,
    pub version: String,
    pub commit: String,
    pub manual: bool,
    pub updater_protocol: u32,
    pub images: Images,
    pub compose: Asset,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ManifestError {
    #[error("the release manifest is larger than 64 KiB")]
    TooLarge,
    #[error("the release manifest does not parse: {0}")]
    Malformed(String),
    #[error("the release manifest has schema {0}, which this version cannot read")]
    Schema(u64),
    #[error("the release manifest is for {found}, not {expected}")]
    Version { expected: String, found: String },
    #[error("the release manifest names `{0}` where a Kaeso image belongs")]
    Image(String),
    #[error("the release manifest has the malformed digest `{0}`")]
    Digest(String),
    #[error("the release manifest has a malformed commit")]
    Commit,
    #[error("the release manifest names `{0}` where compose.yaml belongs")]
    Compose(String),
}

impl Manifest {
    pub fn parse(bytes: &[u8], expected: &Version) -> Result<Manifest, ManifestError> {
        if bytes.len() > MAX_BYTES {
            return Err(ManifestError::TooLarge);
        }
        let value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|e| ManifestError::Malformed(e.to_string()))?;
        let schema = value
            .get("schema")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| ManifestError::Malformed("`schema` is missing".into()))?;
        if schema != u64::from(SCHEMA) {
            return Err(ManifestError::Schema(schema));
        }
        let manifest: Manifest =
            serde_json::from_value(value).map_err(|e| ManifestError::Malformed(e.to_string()))?;
        manifest.check(expected)?;
        Ok(manifest)
    }

    fn check(&self, expected: &Version) -> Result<(), ManifestError> {
        if Version::parse(&self.version).ok().as_ref() != Some(expected) {
            return Err(ManifestError::Version {
                expected: expected.to_string(),
                found: self.version.clone(),
            });
        }
        if self.commit.len() != 40 || !self.commit.bytes().all(is_lower_hex) {
            return Err(ManifestError::Commit);
        }
        for service in Service::ALL {
            let image = self.image(service);
            if image.name != service.image() {
                return Err(ManifestError::Image(image.name.clone()));
            }
            if !is_digest(&image.digest) {
                return Err(ManifestError::Digest(image.digest.clone()));
            }
        }
        if self.compose.name != COMPOSE_ASSET {
            return Err(ManifestError::Compose(self.compose.name.clone()));
        }
        if !is_sha256_hex(&self.compose.sha256) {
            return Err(ManifestError::Digest(self.compose.sha256.clone()));
        }
        Ok(())
    }

    pub fn image(&self, service: Service) -> &Image {
        match service {
            Service::Gateway => &self.images.gateway,
            Service::Console => &self.images.console,
            Service::Updater => &self.images.updater,
        }
    }
}

fn is_lower_hex(b: u8) -> bool {
    b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
}

pub fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(is_lower_hex)
}

pub fn is_digest(s: &str) -> bool {
    s.strip_prefix("sha256:").is_some_and(is_sha256_hex)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const D: &str = "1ea482d2b6192fad3cb7a7ae81ae8e5e87b63fe8fe55bf1cea04fe39d98685b1";

    fn valid() -> serde_json::Value {
        json!({
            "schema": 1,
            "version": "0.2.0",
            "commit": "5388ad0b98aa3d957fea99e200259842bf746680",
            "manual": false,
            "updater_protocol": 1,
            "images": {
                "gateway": { "name": "ghcr.io/logicplanes/kaeso-gateway", "digest": format!("sha256:{D}") },
                "console": { "name": "ghcr.io/logicplanes/kaeso-console", "digest": format!("sha256:{D}") },
                "updater": { "name": "ghcr.io/logicplanes/kaeso-updater", "digest": format!("sha256:{D}") },
            },
            "compose": { "name": "compose.yaml", "sha256": D },
            "notes": "fields this version doesn't know are ignored",
        })
    }

    fn parse(v: &serde_json::Value) -> Result<Manifest, ManifestError> {
        Manifest::parse(
            &serde_json::to_vec(v).unwrap(),
            &Version::parse("0.2.0").unwrap(),
        )
    }

    #[test]
    fn a_valid_manifest_parses_and_names_its_images() {
        let m = parse(&valid()).unwrap();
        assert_eq!(
            m.image(Service::Gateway).reference(),
            format!("ghcr.io/logicplanes/kaeso-gateway@sha256:{D}")
        );
        assert_eq!(m.image(Service::Console).hex(), D);
        assert_eq!(m.image(Service::Updater).name, Service::Updater.image());
        assert!(!m.manual);
    }

    #[test]
    fn every_broken_field_is_refused_with_its_reason() {
        type Case = (fn(&mut serde_json::Value), ManifestError);
        let cases: Vec<Case> = vec![
            (|v| v["schema"] = json!(2), ManifestError::Schema(2)),
            (
                |v| v["version"] = json!("0.2.1"),
                ManifestError::Version {
                    expected: "0.2.0".into(),
                    found: "0.2.1".into(),
                },
            ),
            (|v| v["commit"] = json!("main"), ManifestError::Commit),
            (
                |v| v["images"]["gateway"]["name"] = json!("docker.io/evil/gateway"),
                ManifestError::Image("docker.io/evil/gateway".into()),
            ),
            (
                |v| v["images"]["console"]["digest"] = json!("latest"),
                ManifestError::Digest("latest".into()),
            ),
            (
                |v| v["compose"]["name"] = json!("other.yaml"),
                ManifestError::Compose("other.yaml".into()),
            ),
            (
                |v| v["compose"]["sha256"] = json!("ABC"),
                ManifestError::Digest("ABC".into()),
            ),
        ];
        for (change, expected) in cases {
            let mut v = valid();
            change(&mut v);
            assert_eq!(parse(&v).unwrap_err(), expected);
        }
    }

    #[test]
    fn malformed_and_oversized_documents_are_refused() {
        let version = Version::parse("0.2.0").unwrap();
        assert!(matches!(
            Manifest::parse(b"not json", &version),
            Err(ManifestError::Malformed(_))
        ));
        assert!(matches!(
            Manifest::parse(b"{}", &version),
            Err(ManifestError::Malformed(_))
        ));
        let mut v = valid();
        v["images"].as_object_mut().unwrap().remove("updater");
        assert!(matches!(parse(&v), Err(ManifestError::Malformed(_))));
        assert_eq!(
            Manifest::parse(&vec![b' '; MAX_BYTES + 1], &version),
            Err(ManifestError::TooLarge)
        );
    }

    #[test]
    fn digests_are_lowercase_sha256() {
        assert!(is_digest(&format!("sha256:{D}")));
        assert!(!is_digest(D));
        assert!(!is_digest(&format!("sha256:{}", D.to_uppercase())));
        assert!(!is_digest("sha256:abc"));
    }

    #[test]
    fn services_name_their_images() {
        let names: Vec<&str> = Service::ALL.iter().map(|s| s.as_str()).collect();
        assert_eq!(names, ["gateway", "console", "updater"]);
        assert_eq!(Service::SWITCHED, [Service::Gateway, Service::Console]);
        assert_eq!(
            Service::Console.image(),
            "ghcr.io/logicplanes/kaeso-console"
        );
    }
}
