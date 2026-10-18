#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use axum::http::StatusCode;
use common::App;
use kaeso_store::pg::updates::{self, ReleaseRow};
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};

const RUNNING: &str = env!("CARGO_PKG_VERSION");

fn release(version: &str) -> ReleaseRow {
    ReleaseRow {
        version: version.to_owned(),
        published_at: time::OffsetDateTime::UNIX_EPOCH,
        prerelease: false,
        notes: format!("## Added\n\n- Everything in {version}."),
        url: format!("https://github.com/logicplanes/kaeso/releases/tag/v{version}"),
        manifest: None,
        manifest_error: None,
        art: None,
    }
}

#[tokio::test]
async fn a_new_user_sees_nothing_and_an_old_one_sees_every_release_since() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;

    let fresh = app.get("/me/whats-new", &owner.cookie).await;
    assert_eq!(fresh.status, StatusCode::OK);
    assert_eq!(
        fresh.body,
        json!({ "current": RUNNING, "seen": null, "releases": [] })
    );

    let seen = app
        .post("/me/whats-new/seen", json!({}), Some(&owner.cookie))
        .await;
    assert_eq!(seen.status, StatusCode::NO_CONTENT);
    let after = app.get("/me/whats-new", &owner.cookie).await;
    assert_eq!(after.body["seen"], RUNNING);
    assert_eq!(after.body["releases"], json!([]));

    let mut tx = app.state.pg.begin().await.unwrap();
    let art = json!({ "version": 1, "seed": 42, "style": "albers", "palette": 3 });
    let drawn = ReleaseRow {
        art: Some(art.clone()),
        ..release("0.0.2")
    };
    updates::replace_releases(&mut tx, &[release("0.0.1"), drawn], None)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    sqlx::query("UPDATE users SET seen_version = '0.0.1'")
        .execute(&app.state.pg)
        .await
        .unwrap();
    let behind = app.get("/me/whats-new", &owner.cookie).await;
    let versions: Vec<&str> = behind.body["releases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["version"].as_str().unwrap())
        .collect();
    assert_eq!(versions, [RUNNING, "0.0.2"]);
    assert_eq!(behind.body["releases"][0]["published_at"], Value::Null);
    assert_eq!(behind.body["releases"][0]["art"], Value::Null);
    assert_eq!(behind.body["releases"][1]["art"], art);
    assert_eq!(
        behind.body["releases"][1]["notes"],
        "## Added\n\n- Everything in 0.0.2."
    );

    app.post("/me/whats-new/seen", json!({}), Some(&owner.cookie))
        .await;
    let caught_up = app.get("/me/whats-new", &owner.cookie).await;
    assert_eq!(caught_up.body["releases"], json!([]));
    s.cleanup().await;
}

#[test]
fn the_documented_artwork_fits_every_seed_and_palette() {
    let doc = serde_json::to_value(kaeso_server::platform::openapi()).unwrap();
    let art = &doc["components"]["schemas"]["ReleaseArtView"]["properties"];
    assert_eq!(art["seed"]["format"], "int64");
    assert_eq!(art["seed"]["minimum"], 0);
    assert_eq!(art["seed"]["maximum"], u64::from(u32::MAX));
    assert_eq!(art["palette"]["maximum"], 11);
    assert_eq!(art["version"]["minimum"], 1);
    assert_eq!(art["version"]["maximum"], 1);
}
