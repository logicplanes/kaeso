#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use kaeso_update::RELEASE_WORKFLOW;
use kaeso_updater::source::{Endpoints, Source};
use kaeso_updater::verify::{NO_PROMISE, Sigstore, Subject, Trust, VerifyError};

const GATEWAY_ATTESTATIONS: &[u8] = include_bytes!("fixtures/attestations-gateway-0.1.0.json");
const CONSOLE_ATTESTATIONS: &[u8] = include_bytes!("fixtures/attestations-console-0.1.0.json");
const GATEWAY_INDEX: &[u8] = include_bytes!("fixtures/index-gateway-0.1.0.json");
const CONSOLE_INDEX: &[u8] = include_bytes!("fixtures/index-console-0.1.0.json");
const COMMIT: &str = "5388ad0b98aa3d957fea99e200259842bf746680";
const GATEWAY: &str = "ghcr.io/logicplanes/kaeso-gateway";
const CONSOLE: &str = "ghcr.io/logicplanes/kaeso-console";

fn sigstore(identity: &str) -> Sigstore {
    Sigstore::new(
        Trust::embedded().unwrap(),
        identity,
        Source::new(Endpoints::github().unwrap()).unwrap(),
    )
}

fn subject<'a>(name: &'a str, content: &'a [u8], commit: &'a str) -> Subject<'a> {
    Subject {
        name,
        content,
        commit,
    }
}

#[tokio::test]
async fn the_released_images_of_0_1_0_verify_against_the_release_workflow() {
    let s = sigstore(RELEASE_WORKFLOW);
    s.check(
        GATEWAY_ATTESTATIONS,
        subject(GATEWAY, GATEWAY_INDEX, COMMIT),
    )
    .await
    .unwrap();
    s.check(
        CONSOLE_ATTESTATIONS,
        subject(CONSOLE, CONSOLE_INDEX, COMMIT),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn another_image_another_commit_or_another_workflow_is_untrusted() {
    let s = sigstore(RELEASE_WORKFLOW);
    for (attestations, subject) in [
        (
            GATEWAY_ATTESTATIONS,
            subject(GATEWAY, CONSOLE_INDEX, COMMIT),
        ),
        (
            GATEWAY_ATTESTATIONS,
            subject(CONSOLE, GATEWAY_INDEX, COMMIT),
        ),
        (
            GATEWAY_ATTESTATIONS,
            subject(
                GATEWAY,
                GATEWAY_INDEX,
                "0000000000000000000000000000000000000000",
            ),
        ),
        (
            CONSOLE_ATTESTATIONS,
            subject(GATEWAY, GATEWAY_INDEX, COMMIT),
        ),
    ] {
        let err = s.check(attestations, subject).await.unwrap_err();
        assert!(matches!(err, VerifyError::Untrusted { .. }), "{err}");
    }
    let forked =
        sigstore("https://github.com/someone/kaeso/.github/workflows/release.yml@refs/heads/prod");
    let err = forked
        .check(
            GATEWAY_ATTESTATIONS,
            subject(GATEWAY, GATEWAY_INDEX, COMMIT),
        )
        .await
        .unwrap_err();
    assert!(err.to_string().contains("SubjectAltName"), "{err}");
}

fn tampered(change: impl Fn(&mut serde_json::Value)) -> Vec<u8> {
    let mut doc: serde_json::Value = serde_json::from_slice(GATEWAY_ATTESTATIONS).unwrap();
    change(&mut doc["attestations"][0]["bundle"]);
    serde_json::to_vec(&doc).unwrap()
}

#[tokio::test]
async fn a_tampered_signature_statement_or_log_entry_is_untrusted() {
    let s = sigstore(RELEASE_WORKFLOW);
    let cases = [
        tampered(|b| {
            let sig = b["dsseEnvelope"]["signatures"][0]["sig"]
                .as_str()
                .unwrap()
                .to_owned();
            let flipped = if sig.starts_with('M') {
                sig.replacen('M', "N", 1)
            } else {
                sig.replacen(|_| true, "M", 1)
            };
            b["dsseEnvelope"]["signatures"][0]["sig"] = flipped.into();
        }),
        tampered(|b| {
            b["verificationMaterial"]["tlogEntries"][0]["integratedTime"] = "1792193617".into();
        }),
        tampered(|b| {
            b["verificationMaterial"]["tlogEntries"][0]["logId"]["keyId"] =
                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".into();
        }),
    ];
    for doc in cases {
        let err = s
            .check(&doc, subject(GATEWAY, GATEWAY_INDEX, COMMIT))
            .await
            .unwrap_err();
        assert!(matches!(err, VerifyError::Untrusted { .. }), "{err}");
    }
}

#[tokio::test]
async fn no_attestation_at_all_is_its_own_refusal() {
    let s = sigstore(RELEASE_WORKFLOW);
    let err = s
        .check(
            br#"{"attestations":[]}"#,
            subject(GATEWAY, GATEWAY_INDEX, COMMIT),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, VerifyError::Missing(_)), "{err}");
}

#[test]
fn a_trust_root_without_keys_is_refused() {
    assert!(Trust::parse("{}").is_err());
    assert!(Trust::parse("not json").is_err());
    assert!(Trust::embedded().is_ok());
}

#[tokio::test]
async fn a_bundle_without_a_signed_entry_timestamp_names_what_it_is() {
    let s = sigstore(RELEASE_WORKFLOW);
    let doc = tampered(|b| {
        b["verificationMaterial"]["tlogEntries"][0]
            .as_object_mut()
            .unwrap()
            .remove("inclusionPromise");
    });
    let err = s
        .check(&doc, subject(GATEWAY, GATEWAY_INDEX, COMMIT))
        .await
        .unwrap_err();
    assert!(err.to_string().contains(NO_PROMISE), "{err}");
}

const TRUSTED_ROOT: &str = include_str!("../data/trusted_root.json");

fn root(change: impl Fn(&mut serde_json::Value)) -> Trust {
    let mut doc: serde_json::Value = serde_json::from_str(TRUSTED_ROOT).unwrap();
    change(&mut doc);
    Trust::parse(&doc.to_string()).unwrap()
}

async fn check_with(trust: Trust) -> Result<(), VerifyError> {
    Sigstore::new(
        trust,
        RELEASE_WORKFLOW,
        Source::new(Endpoints::github().unwrap()).unwrap(),
    )
    .check(
        GATEWAY_ATTESTATIONS,
        subject(GATEWAY, GATEWAY_INDEX, COMMIT),
    )
    .await
}

#[tokio::test]
async fn keys_and_authorities_count_only_within_their_validity() {
    let later = "2099-01-01T00:00:00Z";
    let rekor = root(|r| {
        for log in r["tlogs"].as_array_mut().unwrap() {
            log["publicKey"]["validFor"]["start"] = later.into();
        }
    });
    let err = check_with(rekor).await.unwrap_err();
    assert!(
        err.to_string().contains("outside its log key's validity"),
        "{err}"
    );
    let fulcio = root(|r| {
        for ca in r["certificateAuthorities"].as_array_mut().unwrap() {
            ca["validFor"]["end"] = "2022-01-01T00:00:00Z".into();
        }
    });
    let err = check_with(fulcio).await.unwrap_err();
    assert!(err.to_string().contains("for the time of signing"), "{err}");
    let ct = root(|r| {
        for log in r["ctlogs"].as_array_mut().unwrap() {
            log["publicKey"]["validFor"]["start"] = later.into();
        }
    });
    let err = check_with(ct).await.unwrap_err();
    assert!(err.to_string().contains("for the time of signing"), "{err}");
    let mut broken: serde_json::Value = serde_json::from_str(TRUSTED_ROOT).unwrap();
    broken["certificateAuthorities"][0]
        .as_object_mut()
        .unwrap()
        .remove("validFor");
    assert!(Trust::parse(&broken.to_string()).is_err());
}

#[tokio::test]
async fn attestations_are_fetched_from_the_repository_by_digest() {
    let app = axum::Router::new().route(
        "/repos/logicplanes/kaeso/attestations/{subject}",
        axum::routing::get(
            |axum::extract::Path(subject): axum::extract::Path<String>| async move {
                if subject == "sha256:ebafa65" {
                    (axum::http::StatusCode::OK, r#"{"attestations":[]}"#)
                } else {
                    (axum::http::StatusCode::NOT_FOUND, "")
                }
            },
        ),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await });
    let base = url::Url::parse(&format!("http://{addr}/")).unwrap();
    let source = Source::new(Endpoints {
        attestations: base.join("repos/logicplanes/kaeso/attestations/").unwrap(),
        ..Endpoints::github().unwrap()
    })
    .unwrap();
    assert_eq!(
        source.attestations("ebafa65").await.unwrap(),
        br#"{"attestations":[]}"#
    );
    assert!(matches!(
        source.attestations("0000").await,
        Err(VerifyError::Missing(_))
    ));
}
