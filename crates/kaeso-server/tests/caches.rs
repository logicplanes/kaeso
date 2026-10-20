#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use std::sync::Arc;

use kaeso_core::limits::{Kind, Limit, Period, Subject};
use kaeso_testkit::services::TestServices;
use tokio::sync::{Notify, oneshot};

fn limit(amount: i64) -> Vec<Limit> {
    vec![Limit {
        id: uuid::Uuid::nil(),
        subject: Subject::Project(uuid::Uuid::nil()),
        kind: Kind::TokenBudget,
        model: None,
        period: Period::Day,
        amount: amount.into(),
        hard: true,
        thresholds: Vec::new(),
    }]
}

async fn read(app: &common::App, key: &str, loaded: i64) -> i64 {
    let value = app
        .state
        .caches
        .project_limits
        .get_or_load(key, || async move { Ok::<_, ()>(Some(limit(loaded))) })
        .await
        .unwrap()
        .unwrap();
    value[0].amount.try_into().unwrap()
}

#[tokio::test]
async fn an_entry_loaded_before_an_invalidation_is_not_kept() {
    let s = TestServices::start().await;
    let app = common::App::start(&s).await;
    let other = app.second_instance(&s).await;
    let started = Arc::new(Notify::new());
    let (release, released) = oneshot::channel::<()>();
    let caches = &app.state.caches;
    let stale = caches.project_limits.get_or_load("race", {
        let started = started.clone();
        || async move {
            started.notify_one();
            released.await.unwrap();
            Ok::<_, ()>(Some(limit(1)))
        }
    });
    let revoke = async {
        started.notified().await;
        caches.project_limits.invalidate("race").await;
        release.send(()).unwrap();
    };
    let (stale, ()) = tokio::join!(stale, revoke);
    assert_eq!(stale.unwrap().unwrap()[0].amount, 1.into());
    assert_eq!(read(&app, "race", 2).await, 2, "this instance reloads");
    assert_eq!(
        read(&other, "race", 3).await,
        2,
        "the fresh entry is shared"
    );
}

#[tokio::test]
async fn shared_entries_need_this_installs_signature() {
    let s = TestServices::start().await;
    let app = common::App::start(&s).await;
    let other = app.second_instance(&s).await;
    assert_eq!(read(&app, "signed", 5).await, 5);
    assert_eq!(read(&other, "signed", 6).await, 5, "a signed entry is read");

    let forged = app.state.valkey.key("cache:plim:forged");
    let json = serde_json::to_vec(&limit(1_000_000)).unwrap();
    let mut bytes = vec![0_u8; 32];
    bytes.extend_from_slice(&json);
    for value in [json.clone(), bytes] {
        let mut conn = app.state.valkey.conn();
        let _: () = redis::cmd("SET")
            .arg(&forged)
            .arg(value)
            .arg("PX")
            .arg(30_000)
            .query_async(&mut conn)
            .await
            .unwrap();
        assert_eq!(read(&other, "forged", 7).await, 7);
        other.state.caches.project_limits.invalidate("forged").await;
    }
}
