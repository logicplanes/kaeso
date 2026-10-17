use std::collections::BTreeMap;

use aws_lc_rs::signature::{ECDSA_P256_SHA256_ASN1, UnparsedPublicKey};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use futures::future::BoxFuture;
use rustls_pki_types::CertificateDer;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sigstore::bundle::Bundle;
use sigstore::bundle::verify::Verifier;
use sigstore::bundle::verify::policy::Identity;
use sigstore::trust::ManualTrustRoot;

use crate::source::Source;

pub const GITHUB_ISSUER: &str = "https://token.actions.githubusercontent.com";
pub const STATEMENT: &str = "https://in-toto.io/Statement/v1";
pub const PROVENANCE: &str = "https://slsa.dev/provenance/v1";
pub const WORKFLOW_REPOSITORY: &str = "https://github.com/logicplanes/kaeso";
pub const WORKFLOW_PATH: &str = ".github/workflows/release.yml";
pub const WORKFLOW_REF: &str = "refs/heads/prod";

const TRUSTED_ROOT: &str = include_str!("../data/trusted_root.json");
const P256_SPKI_PREFIX: &[u8] = &[
    0x30, 0x59, 0x30, 0x13, 0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01, 0x06, 0x08, 0x2a,
    0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07, 0x03, 0x42, 0x00,
];

#[derive(Clone, Copy, Debug)]
pub struct Subject<'a> {
    pub name: &'a str,
    pub content: &'a [u8],
    pub commit: &'a str,
}

impl Subject<'_> {
    pub fn sha256(&self) -> String {
        hex(&Sha256::digest(self.content))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("{0} has no attestation")]
    Missing(String),
    #[error("the attestations of {0} cannot be fetched: {1}")]
    Fetch(String, String),
    #[error("no attestation of {name} comes from Kaeso's release workflow: {why}")]
    Untrusted { name: String, why: String },
    #[error("the trust root does not parse: {0}")]
    TrustRoot(String),
}

pub trait Verify: Send + Sync {
    fn verify<'a>(&'a self, subject: Subject<'a>) -> BoxFuture<'a, Result<(), VerifyError>>;
}

#[derive(Clone, Debug)]
struct RekorKey {
    id: Vec<u8>,
    point: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct Trust {
    fulcio: Vec<Vec<u8>>,
    ctfe: BTreeMap<String, Vec<u8>>,
    rekor: Vec<RekorKey>,
}

fn b64(value: &Value) -> Option<Vec<u8>> {
    STANDARD.decode(value.as_str()?).ok()
}

impl Trust {
    pub fn embedded() -> Result<Trust, VerifyError> {
        Trust::parse(TRUSTED_ROOT)
    }

    pub fn parse(text: &str) -> Result<Trust, VerifyError> {
        let bad = |what: &str| VerifyError::TrustRoot(what.to_owned());
        let root: Value = serde_json::from_str(text).map_err(|e| bad(&e.to_string()))?;
        let list = |key: &str| {
            root.get(key)
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        };
        let fulcio: Vec<Vec<u8>> = list("certificateAuthorities")
            .iter()
            .flat_map(|ca| {
                ca.pointer("/certChain/certificates")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default()
            })
            .filter_map(|c| c.get("rawBytes").and_then(b64))
            .collect();
        let keys = |key: &str| -> Vec<(Vec<u8>, Vec<u8>)> {
            list(key)
                .iter()
                .filter_map(|log| {
                    let id = log.pointer("/logId/keyId").and_then(b64)?;
                    let spki = log.pointer("/publicKey/rawBytes").and_then(b64)?;
                    Some((id, spki))
                })
                .collect()
        };
        let ctfe: BTreeMap<String, Vec<u8>> = keys("ctlogs")
            .into_iter()
            .map(|(id, spki)| (hex(&id), spki))
            .collect();
        let rekor = keys("tlogs")
            .into_iter()
            .filter_map(|(id, spki)| {
                let point = spki.strip_prefix(P256_SPKI_PREFIX)?.to_vec();
                Some(RekorKey { id, point })
            })
            .collect::<Vec<_>>();
        if fulcio.is_empty() || ctfe.is_empty() || rekor.is_empty() {
            return Err(bad(
                "it names no certificate authority, CT log or Rekor key",
            ));
        }
        Ok(Trust {
            fulcio,
            ctfe,
            rekor,
        })
    }

    fn verifier(&self) -> Result<Verifier, String> {
        let root = ManualTrustRoot {
            fulcio_certs: self
                .fulcio
                .iter()
                .map(|der| CertificateDer::from(der.clone()))
                .collect(),
            rekor_keys: BTreeMap::new(),
            ctfe_keys: self.ctfe.clone(),
        };
        Verifier::new(Default::default(), root).map_err(|e| e.to_string())
    }
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Serialize)]
struct SignedEntry<'a> {
    body: &'a str,
    #[serde(rename = "integratedTime")]
    integrated_time: i64,
    #[serde(rename = "logID")]
    log_id: String,
    #[serde(rename = "logIndex")]
    log_index: i64,
}

fn number(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::String(s) => s.parse().ok(),
        Value::Number(n) => n.as_i64(),
        _ => None,
    }
}

impl Trust {
    fn check_promise(&self, bundle: &Value) -> Result<(), String> {
        let entries = bundle
            .pointer("/verificationMaterial/tlogEntries")
            .and_then(Value::as_array)
            .ok_or("the bundle has no transparency log entry")?;
        let [entry] = entries.as_slice() else {
            return Err("the bundle has more than one transparency log entry".into());
        };
        let id = entry
            .pointer("/logId/keyId")
            .and_then(b64)
            .ok_or("the log entry names no log")?;
        let key = self
            .rekor
            .iter()
            .find(|k| k.id == id)
            .ok_or("the log entry is from a log the trust root doesn't name")?;
        let body = entry
            .get("canonicalizedBody")
            .and_then(Value::as_str)
            .ok_or("the log entry has no body")?;
        let signed = SignedEntry {
            body,
            integrated_time: number(entry.get("integratedTime")).ok_or("no integrated time")?,
            log_id: hex(&id),
            log_index: number(entry.get("logIndex")).ok_or("no log index")?,
        };
        let payload = serde_json::to_vec(&signed).map_err(|e| e.to_string())?;
        let signature = entry
            .pointer("/inclusionPromise/signedEntryTimestamp")
            .and_then(b64)
            .ok_or("the log entry has no signed entry timestamp")?;
        UnparsedPublicKey::new(&ECDSA_P256_SHA256_ASN1, &key.point)
            .verify(&payload, &signature)
            .map_err(|_| "the transparency log's signature does not match".to_owned())
    }
}

fn check_statement(bundle: &Value, subject: &Subject<'_>) -> Result<(), String> {
    let payload = bundle
        .pointer("/dsseEnvelope/payload")
        .and_then(b64)
        .ok_or("the bundle has no signed statement")?;
    let statement: Value = serde_json::from_slice(&payload)
        .map_err(|e| format!("the statement does not parse: {e}"))?;
    if statement.get("_type").and_then(Value::as_str) != Some(STATEMENT) {
        return Err("the statement is not an in-toto statement".into());
    }
    if statement.get("predicateType").and_then(Value::as_str) != Some(PROVENANCE) {
        return Err("the statement is not SLSA build provenance".into());
    }
    let digest = subject.sha256();
    let names_it = statement
        .get("subject")
        .and_then(Value::as_array)
        .is_some_and(|subjects| {
            subjects.iter().any(|s| {
                s.get("name").and_then(Value::as_str) == Some(subject.name)
                    && s.pointer("/digest/sha256").and_then(Value::as_str) == Some(digest.as_str())
            })
        });
    if !names_it {
        return Err(format!(
            "the statement is not about {}@sha256:{digest}",
            subject.name
        ));
    }
    let workflow = statement
        .pointer("/predicate/buildDefinition/externalParameters/workflow")
        .ok_or("the provenance names no workflow")?;
    let field = |name: &str| workflow.get(name).and_then(Value::as_str);
    if field("repository") != Some(WORKFLOW_REPOSITORY)
        || field("path") != Some(WORKFLOW_PATH)
        || field("ref") != Some(WORKFLOW_REF)
    {
        return Err("the provenance names another workflow".into());
    }
    let built_from_commit = statement
        .pointer("/predicate/buildDefinition/resolvedDependencies")
        .and_then(Value::as_array)
        .is_some_and(|deps| {
            deps.iter().any(|d| {
                d.pointer("/digest/gitCommit").and_then(Value::as_str) == Some(subject.commit)
            })
        });
    if !built_from_commit {
        return Err(format!(
            "the provenance is not for commit {}",
            subject.commit
        ));
    }
    Ok(())
}

pub struct Sigstore {
    trust: Trust,
    identity: String,
    source: Source,
}

impl Sigstore {
    pub fn new(trust: Trust, identity: impl Into<String>, source: Source) -> Sigstore {
        Sigstore {
            trust,
            identity: identity.into(),
            source,
        }
    }

    async fn check_bundle(&self, bundle: &Value, subject: &Subject<'_>) -> Result<(), String> {
        check_statement(bundle, subject)?;
        self.trust.check_promise(bundle)?;
        let parsed: Bundle = serde_json::from_value(bundle.clone())
            .map_err(|e| format!("the bundle does not parse: {e}"))?;
        let verifier = self.trust.verifier()?;
        let policy = Identity::new(&self.identity, GITHUB_ISSUER);
        verifier
            .verify(subject.content, parsed, &policy, true)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn check(
        &self,
        attestations: &[u8],
        subject: Subject<'_>,
    ) -> Result<(), VerifyError> {
        let untrusted = |why: String| VerifyError::Untrusted {
            name: subject.name.to_owned(),
            why,
        };
        let doc: Value = serde_json::from_slice(attestations)
            .map_err(|e| untrusted(format!("the attestation list does not parse: {e}")))?;
        let bundles: Vec<&Value> = doc
            .get("attestations")
            .and_then(Value::as_array)
            .map(|list| list.iter().filter_map(|a| a.get("bundle")).collect())
            .unwrap_or_default();
        if bundles.is_empty() {
            return Err(VerifyError::Missing(subject.name.to_owned()));
        }
        let mut last = String::new();
        for bundle in bundles {
            match self.check_bundle(bundle, &subject).await {
                Ok(()) => return Ok(()),
                Err(why) => last = why,
            }
        }
        Err(untrusted(last))
    }
}

impl Verify for Sigstore {
    fn verify<'a>(&'a self, subject: Subject<'a>) -> BoxFuture<'a, Result<(), VerifyError>> {
        Box::pin(async move {
            let attestations = self.source.attestations(&subject.sha256()).await?;
            self.check(&attestations, subject).await
        })
    }
}
