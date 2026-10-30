#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::http::{Method, StatusCode};
use common::{App, Owner};
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};

async fn project(app: &App, owner: &Owner) -> String {
    let r = app
        .get(&format!("/orgs/{}/projects", owner.org_id), &owner.cookie)
        .await;
    r.body["data"][0]["id"].as_str().unwrap().to_owned()
}

fn embedded_model_count() -> usize {
    kaeso_core::models_dev::embedded().unwrap().model_count()
}

fn base(owner: &Owner, project: &str) -> String {
    format!("/orgs/{}/projects/{project}", owner.org_id)
}

async fn add(app: &App, owner: &Owner, project: &str, body: Value) -> Value {
    let r = app
        .post(
            &format!("{}/models", base(owner, project)),
            body,
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    r.body
}

async fn patch(app: &App, owner: &Owner, path: &str, body: Value) -> common::Resp {
    app.call(Method::PATCH, path, Some(body), Some(&owner.cookie))
        .await
}

#[tokio::test]
async fn entries_store_only_overrides_and_merge_with_models_dev() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let p = project(&app, &owner).await;
    let b = base(&owner, &p);

    let status = app.get("/instance/models-dev", &owner.cookie).await;
    assert_eq!(status.body["source"], "embedded", "{:?}", status.body);
    assert_eq!(status.body["model_count"], embedded_model_count());

    let hits = app
        .get(
            &format!("{b}/models/search?q=haiku&provider=anthropic"),
            &owner.cookie,
        )
        .await;
    let ids: Vec<&str> = hits.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["model_id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"claude-haiku-4-5"), "{ids:?}");
    let preview = app
        .get(
            &format!("{b}/models/preview?provider=anthropic&model=claude-haiku-4-5"),
            &owner.cookie,
        )
        .await;
    assert_eq!(preview.body["fields"]["input_price"], "1");
    assert_eq!(preview.body["fields"]["adaptive_thinking"], false);
    assert_eq!(preview.body["egress_dialect"], "anthropic_messages");
    let missing = app
        .get(
            &format!("{b}/models/preview?provider=openai&model=nope"),
            &owner.cookie,
        )
        .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);

    let haiku = add(
        &app,
        &owner,
        &p,
        json!({
            "provider": "anthropic", "model_id": "claude-haiku-4-5",
            "overrides": { "input_price": "0.80", "display_name": "Support model" }
        }),
    )
    .await;
    assert!(haiku["id"].as_str().unwrap().starts_with("model_"));
    assert_eq!(
        haiku["overrides"],
        json!({ "display_name": "Support model", "input_price": "0.8" })
    );
    assert_eq!(haiku["effective"]["input_price"], "0.8");
    assert_eq!(haiku["effective"]["output_price"], "5");
    assert_eq!(haiku["effective"]["output_limit"], 64000);
    assert_eq!(haiku["listed"], true);
    let entry = format!("{b}/models/{}", haiku["id"].as_str().unwrap());

    let stored: Value = sqlx::query_scalar("SELECT overrides FROM catalogue_models")
        .fetch_one(&app.state.pg)
        .await
        .unwrap();
    assert_eq!(
        stored,
        json!({ "display_name": "Support model", "input_price": "0.8" })
    );

    let r = patch(
        &app,
        &owner,
        &entry,
        json!({ "overrides": { "input_price": null, "timeout_seconds": 120 } }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["effective"]["input_price"], "1");
    assert_eq!(
        r.body["overrides"],
        json!({ "display_name": "Support model", "timeout_seconds": 120 })
    );
    let bad = patch(
        &app,
        &owner,
        &entry,
        json!({ "overrides": { "egress_dialect": "openai_chat" } }),
    )
    .await;
    assert_eq!(
        bad.body["error"]["param"], "egress_dialect",
        "{:?}",
        bad.body
    );
    let bad = patch(
        &app,
        &owner,
        &entry,
        json!({ "overrides": { "colour": 1 } }),
    )
    .await;
    assert_eq!(bad.body["error"]["param"], "colour");

    let dup = app
        .post(
            &format!("{b}/models"),
            json!({ "provider": "anthropic", "model_id": "claude-haiku-4-5" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(dup.status, StatusCode::CONFLICT);
    for dots in ["..", ".", "a/../b", "openai/./x", "../"] {
        let r = app
            .post(
                &format!("{b}/models"),
                json!({
                    "provider": "openai", "model_id": dots, "custom": true,
                    "overrides": {
                        "input_modalities": ["text"], "output_modalities": ["text"],
                        "context_limit": 32000, "output_limit": 4000,
                        "input_price": "1", "output_price": "1", "egress_dialect": "openai_chat"
                    }
                }),
                Some(&owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{dots}: {:?}", r.body);
        assert_eq!(r.body["error"]["param"], "model_id");
        assert_eq!(
            r.body["error"]["message"],
            "`model_id` can't be `.` or `..`, or have either between slashes."
        );
    }
    let unknown = app
        .post(
            &format!("{b}/models"),
            json!({ "provider": "openai", "model_id": "gpt-internal" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(unknown.body["error"]["param"], "model_id");
    let incomplete = app.post(&format!("{b}/models"), json!({ "provider": "openai", "model_id": "gpt-internal", "custom": true, "overrides": { "input_price": "1" } }), Some(&owner.cookie)).await;
    assert_eq!(
        incomplete.body["error"]["param"], "input_modalities",
        "{:?}",
        incomplete.body
    );
    assert!(
        incomplete.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("egress_dialect")
    );
    let custom = add(
        &app,
        &owner,
        &p,
        json!({
            "provider": "openai", "model_id": "gpt-internal", "custom": true,
            "overrides": {
                "input_modalities": ["text"], "output_modalities": ["text"],
                "context_limit": 32000, "output_limit": 4000,
                "input_price": "0.5", "output_price": "1.5", "egress_dialect": "openai_chat"
            }
        }),
    )
    .await;
    assert_eq!(custom["custom"], true);
    assert_eq!(custom["egress_dialect"], "openai_chat");
    for refused in ["+0.5", "5.", "1_000", " 1", "1 ", "-1", "free"] {
        let r = app
            .post(
                &format!("{b}/models"),
                json!({
                    "provider": "openai", "model_id": "gpt-bad-price", "custom": true,
                    "overrides": { "input_price": refused }
                }),
                Some(&owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{refused:?}");
        assert_eq!(r.body["error"]["param"], "input_price", "{refused:?}");
        let message = r.body["error"]["message"].as_str().unwrap();
        assert_eq!(
            message, "`input_price` has a value of the wrong type or form.",
            "{refused:?}"
        );
        assert!(message.chars().count() <= 90, "{message}");
    }
    let r = patch(
        &app,
        &owner,
        &format!("{b}/models/{}", custom["id"].as_str().unwrap()),
        json!({ "overrides": { "output_price": null } }),
    )
    .await;
    assert_eq!(r.body["error"]["param"], "output_price", "{:?}", r.body);

    let current = format!("{b}/models/search?provider=anthropic&current=true&limit=1");
    let before = app.get(&current, &owner.cookie).await;
    assert_eq!(before.body["total"], 17, "{:?}", before.body);
    assert_eq!(before.body["not_in_catalogue"], 16);
    assert_eq!(before.body["has_more"], true);
    let first = &before.body["data"][0];
    assert_eq!(before.body["data"].as_array().unwrap().len(), 1);
    assert!(first["input_price"].is_string(), "{first:?}");
    assert!(first["output_price"].is_string(), "{first:?}");
    assert!(first["context_limit"].as_u64().unwrap() > 0, "{first:?}");
    let all = app
        .get(
            &format!("{b}/models/search?provider=anthropic&include_deprecated=true&limit=200"),
            &owner.cookie,
        )
        .await;
    assert_eq!(all.body["has_more"], false);
    assert_eq!(
        all.body["total"].as_u64().unwrap(),
        all.body["data"].as_array().unwrap().len() as u64
    );

    let r = app
        .post(
            &format!("{b}/models/batch"),
            json!({ "provider": "anthropic", "all_current": true }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    assert_eq!(r.body["skipped"], json!(["claude-haiku-4-5"]));
    assert_eq!(r.body["added"].as_array().unwrap().len(), 16);
    let list = app.get(&format!("{b}/models"), &owner.cookie).await;
    assert_eq!(list.body["data"].as_array().unwrap().len(), 18);
    let after = app.get(&current, &owner.cookie).await;
    assert_eq!(after.body["not_in_catalogue"], 0, "{:?}", after.body);

    let audit = app
        .get(&format!("/orgs/{}/audit-log", owner.org_id), &owner.cookie)
        .await;
    let text = audit.body.to_string();
    assert!(text.contains("catalogue_model.create") && text.contains("catalogue_model.update"));
}

#[tokio::test]
async fn aliases_resolve_first_and_warn_when_they_shadow() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let p = project(&app, &owner).await;
    let b = base(&owner, &p);
    let sonnet = add(
        &app,
        &owner,
        &p,
        json!({ "provider": "anthropic", "model_id": "claude-sonnet-4-5" }),
    )
    .await;
    let gpt = add(
        &app,
        &owner,
        &p,
        json!({ "provider": "openai", "model_id": "gpt-4o" }),
    )
    .await;
    let sonnet_id = sonnet["id"].as_str().unwrap();
    let r = app
        .post(
            &format!("{b}/aliases"),
            json!({ "name": "chat-default", "catalogue_model_id": sonnet_id }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    assert_eq!(r.body["shadows_model"], false);
    assert_eq!(r.body["target"], "anthropic/claude-sonnet-4-5");
    let shadow = app
        .post(
            &format!("{b}/aliases"),
            json!({ "name": "gpt-5", "catalogue_model_id": sonnet_id }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(shadow.body["shadows_model"], true);
    let dup = app
        .post(
            &format!("{b}/aliases"),
            json!({ "name": "chat-default", "catalogue_model_id": sonnet_id }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(dup.status, StatusCode::CONFLICT);
    let slash = app
        .post(
            &format!("{b}/aliases"),
            json!({ "name": "a/b", "catalogue_model_id": sonnet_id }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(slash.body["error"]["param"], "name");

    let project_id: kaeso_core::id::ProjectId = p.parse().unwrap();
    let models = kaeso_server::services::catalogue::project_models(&app.state, project_id.uuid())
        .await
        .unwrap();
    let resolve = |n: &str| models.catalogue.resolve(n).map(|(e, _)| e.model_id.clone());
    assert_eq!(resolve("chat-default").unwrap(), "claude-sonnet-4-5");
    assert_eq!(resolve("gpt-5").unwrap(), "claude-sonnet-4-5");
    assert_eq!(resolve("openai/gpt-4o").unwrap(), "gpt-4o");
    assert!(resolve("claude-opus-4-6").is_err());

    let alias = r.body["id"].as_str().unwrap();
    let moved = patch(
        &app,
        &owner,
        &format!("{b}/aliases/{alias}"),
        json!({ "catalogue_model_id": gpt["id"] }),
    )
    .await;
    assert_eq!(moved.body["target"], "openai/gpt-4o", "{:?}", moved.body);
    let models = kaeso_server::services::catalogue::project_models(&app.state, project_id.uuid())
        .await
        .unwrap();
    assert_eq!(
        models.catalogue.resolve("chat-default").unwrap().0.model_id,
        "gpt-4o"
    );
    let gone = app
        .call(
            Method::DELETE,
            &format!("{b}/models/{}", gpt["id"].as_str().unwrap()),
            None,
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(gone.status, StatusCode::NO_CONTENT);
    let aliases = app.get(&format!("{b}/aliases"), &owner.cookie).await;
    let names: Vec<&str> = aliases.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["gpt-5"]);
}

#[tokio::test]
async fn model_access_narrows_from_catalogue_to_member_to_key() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let p = project(&app, &owner).await;
    let b = base(&owner, &p);
    let a = add(
        &app,
        &owner,
        &p,
        json!({ "provider": "openai", "model_id": "gpt-4o" }),
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let m = add(
        &app,
        &owner,
        &p,
        json!({ "provider": "openai", "model_id": "gpt-4o-mini" }),
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let c = add(
        &app,
        &owner,
        &p,
        json!({ "provider": "anthropic", "model_id": "claude-haiku-4-5" }),
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let member = format!("{b}/members/{}/models", owner.user_id);
    let r = app.get(&member, &owner.cookie).await;
    assert_eq!(r.body["all"], true, "{:?}", r.body);
    assert_eq!(r.body["effective"].as_array().unwrap().len(), 3);
    let r = patch(&app, &owner, &member, json!({ "models": [a, m] })).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["all"], false);
    let r = patch(
        &app,
        &owner,
        &member,
        json!({ "models": ["model_00000000000000000000000000"] }),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    let r = patch(&app, &owner, &member, json!({ "all": true, "models": [a] })).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);

    let outside = app
        .post(
            &format!("{b}/keys"),
            json!({ "name": "k", "models": [c] }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(
        outside.body["error"]["param"], "models",
        "{:?}",
        outside.body
    );
    let key = app
        .post(
            &format!("{b}/keys"),
            json!({ "name": "k", "models": [a, m] }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(key.status, StatusCode::CREATED, "{:?}", key.body);
    let key_id = key.body["id"].as_str().unwrap().to_owned();
    let key_models = format!("{b}/keys/{key_id}/models");
    let r = app.get(&key_models, &owner.cookie).await;
    assert_eq!(
        r.body["models"].as_array().unwrap().len(),
        2,
        "{:?}",
        r.body
    );

    patch(&app, &owner, &member, json!({ "models": [a] })).await;
    let r = app.get(&key_models, &owner.cookie).await;
    assert_eq!(r.body["models"], json!([a]), "{:?}", r.body);
    assert_eq!(r.body["effective"], json!([a]));

    let project_uuid = p.parse::<kaeso_core::id::ProjectId>().unwrap().uuid();
    let key_uuid = key_id.parse::<kaeso_core::id::ApiKeyId>().unwrap().uuid();
    let user_uuid = owner
        .user_id
        .parse::<kaeso_core::id::UserId>()
        .unwrap()
        .uuid();
    let models = kaeso_server::services::catalogue::project_models(&app.state, project_uuid)
        .await
        .unwrap();
    let access = models.access(Some(key_uuid), Some(user_uuid), None);
    let id = |s: &str| {
        s.parse::<kaeso_core::id::CatalogueModelId>()
            .unwrap()
            .uuid()
    };
    assert_eq!(access.check(id(&a)), Ok(()));
    assert_eq!(
        access.check(id(&c)),
        Err(kaeso_core::catalogue::resolve::AccessLevel::Member)
    );
    patch(&app, &owner, &member, json!({ "all": true })).await;
    let models = kaeso_server::services::catalogue::project_models(&app.state, project_uuid)
        .await
        .unwrap();
    let access = models.access(Some(key_uuid), Some(user_uuid), None);
    assert_eq!(
        access.check(id(&c)),
        Err(kaeso_core::catalogue::resolve::AccessLevel::Key)
    );

    let r = app
        .call(
            Method::DELETE,
            &format!("{b}/models/{a}"),
            None,
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let r = app.get(&key_models, &owner.cookie).await;
    assert_eq!(r.body["models"], json!([]), "{:?}", r.body);
    let (left,): (i64,) = sqlx::query_as("SELECT count(*) FROM model_access")
        .fetch_one(&app.state.pg)
        .await
        .unwrap();
    assert_eq!(left, 0);
}

async fn stub_models_dev(body: Arc<Mutex<(u16, String)>>) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let router = axum::Router::new().route(
        "/api.json",
        axum::routing::get(move || {
            let body = body.clone();
            async move {
                let (status, text) = body.lock().unwrap().clone();
                (StatusCode::from_u16(status).unwrap(), text)
            }
        }),
    );
    tokio::spawn(async move { axum::serve(listener, router).await });
    format!("http://{addr}/api.json")
}

#[tokio::test]
async fn syncs_audit_price_changes_and_keep_delisted_models() {
    let s = TestServices::start().await;
    let mut data: Value = serde_json::from_str(kaeso_core::models_dev::EMBEDDED).unwrap();
    let body = Arc::new(Mutex::new((200u16, data.to_string())));
    let url = stub_models_dev(body.clone()).await;
    let app = App::start_with(&s, &[("KAESO_MODELS_DEV_URL", url.as_str())]).await;
    let other = app.second_instance(&s).await;
    let owner = app.setup().await;
    let p = project(&app, &owner).await;
    let b = base(&owner, &p);
    let haiku = add(
        &app,
        &owner,
        &p,
        json!({ "provider": "anthropic", "model_id": "claude-haiku-4-5" }),
    )
    .await;
    let gpt = add(
        &app,
        &owner,
        &p,
        json!({ "provider": "openai", "model_id": "gpt-4o" }),
    )
    .await;
    let fixed = add(&app, &owner, &p, json!({ "provider": "anthropic", "model_id": "claude-sonnet-4-5", "overrides": { "input_price": "3", "output_price": "15" } })).await;

    data["anthropic"]["models"]["claude-haiku-4-5"]["cost"]["input"] = json!(1.5);
    data["anthropic"]["models"]["claude-sonnet-4-5"]["cost"]["input"] = json!(4);
    data["openai"]["models"]
        .as_object_mut()
        .unwrap()
        .remove("gpt-4o");
    data["openai"]["models"]["gpt-new"] =
        json!({ "id": "gpt-new", "name": "GPT New", "cost": { "input": 1, "output": 2 } });
    data["future_provider"] = json!({ "id": "future_provider", "models": {} });
    body.lock().unwrap().1 = data.to_string();

    let r = app
        .post("/instance/models-dev/sync", json!({}), Some(&owner.cookie))
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["source"], url);
    assert_eq!(r.body["last_error"], Value::Null);
    assert_eq!(r.body["model_count"], embedded_model_count());

    let h = app
        .get(
            &format!("{b}/models/{}", haiku["id"].as_str().unwrap()),
            &owner.cookie,
        )
        .await;
    assert_eq!(h.body["effective"]["input_price"], "1.5");
    let g = app
        .get(
            &format!("{b}/models/{}", gpt["id"].as_str().unwrap()),
            &owner.cookie,
        )
        .await;
    assert_eq!(g.body["listed"], false, "{:?}", g.body);
    assert_eq!(
        g.body["effective"]["input_price"], "2.5",
        "last known values are kept"
    );

    let instance = app
        .get(
            "/instance/audit-log?action=models_dev.price_change",
            &owner.cookie,
        )
        .await;
    assert_eq!(
        instance.body["data"].as_array().unwrap().len(),
        2,
        "{:?}",
        instance.body
    );
    assert_eq!(instance.body["data"][0]["actor"]["kind"], "system");
    let org = app
        .get(
            &format!(
                "/orgs/{}/audit-log?action=catalogue_model.price_change",
                owner.org_id
            ),
            &owner.cookie,
        )
        .await;
    let events = org.body["data"].as_array().unwrap();
    assert_eq!(events.len(), 1, "{:?}", org.body);
    assert_eq!(events[0]["target_id"], haiku["id"]);
    assert_eq!(events[0]["before"]["input_price"], "1");
    assert_eq!(events[0]["after"]["input_price"], "1.5");
    let f = app
        .get(
            &format!("{b}/models/{}", fixed["id"].as_str().unwrap()),
            &owner.cookie,
        )
        .await;
    assert_eq!(
        f.body["effective"]["input_price"], "3",
        "overrides win over models.dev"
    );

    let mut seen = false;
    for _ in 0..100 {
        if other.state.models_dev.current().source == url {
            seen = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(seen, "the other instance never reloaded");
    assert!(
        other
            .state
            .models_dev
            .current()
            .snapshot
            .model("openai", "gpt-new")
            .is_some()
    );

    let r = app
        .post("/instance/models-dev/sync", json!({}), Some(&owner.cookie))
        .await;
    assert_eq!(r.body["last_error"], Value::Null);
    *body.lock().unwrap() = (500, "down".into());
    let r = app
        .post("/instance/models-dev/sync", json!({}), Some(&owner.cookie))
        .await;
    assert_eq!(r.status, StatusCode::SERVICE_UNAVAILABLE, "{:?}", r.body);
    let status = app.get("/instance/models-dev", &owner.cookie).await;
    assert!(
        status.body["last_error"].as_str().unwrap().contains("500"),
        "{:?}",
        status.body
    );
    assert_eq!(status.body["source"], url);
    *body.lock().unwrap() = (200, "{\"not\": \"models\"}".into());
    let r = app
        .post("/instance/models-dev/sync", json!({}), Some(&owner.cookie))
        .await;
    assert_eq!(r.status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(
        app.state
            .models_dev
            .current()
            .snapshot
            .model("openai", "gpt-new")
            .is_some()
    );

    let mut back: Value = serde_json::from_str(kaeso_core::models_dev::EMBEDDED).unwrap();
    back["openai"]["models"]["extra"] = json!({ "id": "extra" });
    *body.lock().unwrap() = (200, back.to_string());
    app.post("/instance/models-dev/sync", json!({}), Some(&owner.cookie))
        .await
        .expect_status(StatusCode::OK);
    let g = app
        .get(
            &format!("{b}/models/{}", gpt["id"].as_str().unwrap()),
            &owner.cookie,
        )
        .await;
    assert_eq!(g.body["listed"], true, "{:?}", g.body);
}

async fn run_sync_job(app: &App) -> String {
    let job = kaeso_server::jobs::all()
        .into_iter()
        .find(|j| j.name == "models_dev_sync")
        .unwrap();
    kaeso_server::jobs::run_once(&app.state, job)
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn the_sync_job_runs_once_per_interval_and_an_hour_after_a_failure() {
    let s = TestServices::start().await;
    let body = Arc::new(Mutex::new((
        200u16,
        kaeso_core::models_dev::EMBEDDED.to_owned(),
    )));
    let url = stub_models_dev(body.clone()).await;
    let app = App::start_with(
        &s,
        &[
            ("KAESO_MODELS_DEV_URL", url.as_str()),
            ("KAESO_MODELS_DEV_SYNC", "true"),
            ("KAESO_MODELS_DEV_INTERVAL", "6h"),
        ],
    )
    .await;
    let due = kaeso_core::models_dev::EMBEDDED_FETCHED_AT + time::Duration::hours(6);
    app.clock.set(
        due.max(
            time::OffsetDateTime::now_utc()
                .replace_nanosecond(0)
                .unwrap(),
        ),
    );

    assert_eq!(run_sync_job(&app).await, "unchanged");
    assert_eq!(run_sync_job(&app).await, "not due");
    app.clock
        .advance(time::Duration::hours(6) - time::Duration::SECOND);
    assert_eq!(run_sync_job(&app).await, "not due");
    app.clock.advance(time::Duration::SECOND);

    let mut changed: Value = serde_json::from_str(kaeso_core::models_dev::EMBEDDED).unwrap();
    changed["openai"]["models"]["gpt-new"] = json!({ "id": "gpt-new", "name": "GPT New" });
    body.lock().unwrap().1 = changed.to_string();
    assert_eq!(
        run_sync_job(&app).await,
        "applied: 0 price changes, 0 delisted, 0 listed again"
    );
    assert!(
        app.state
            .models_dev
            .current()
            .snapshot
            .model("openai", "gpt-new")
            .is_some()
    );
    assert_eq!(run_sync_job(&app).await, "not due");

    app.clock.advance(time::Duration::hours(6));
    body.lock().unwrap().0 = 500;
    let job = kaeso_server::jobs::all()
        .into_iter()
        .find(|j| j.name == "models_dev_sync")
        .unwrap();
    assert!(kaeso_server::jobs::run_once(&app.state, job).await.is_err());
    app.clock
        .advance(time::Duration::hours(1) - time::Duration::SECOND);
    assert_eq!(run_sync_job(&app).await, "not due");
    app.clock.advance(time::Duration::SECOND);
    body.lock().unwrap().0 = 200;
    assert_eq!(run_sync_job(&app).await, "unchanged");
}

async fn store_as_an_older_build(
    app: &App,
    body: &str,
    source: &str,
    fetched_at: time::OffsetDateTime,
) {
    let without_mistral: Vec<_> = kaeso_core::provider::known_ids()
        .into_iter()
        .filter(|id| id.as_str() != "mistral")
        .collect();
    let old = kaeso_core::models_dev::parse(body.as_bytes(), &without_mistral)
        .unwrap()
        .snapshot;
    sqlx::query(
        "UPDATE models_dev_snapshot SET fetched_at = $1, source_url = $2, \
         content_hash = sha256(convert_to($3, 'UTF8')), format = NULL, data = $4, model_count = $5, \
         last_attempt_at = $1, last_error = NULL WHERE id = 1",
    )
    .bind(fetched_at)
    .bind(source)
    .bind(body)
    .bind(serde_json::to_value(&old).unwrap())
    .bind(i32::try_from(old.model_count()).unwrap())
    .execute(&app.state.pg)
    .await
    .unwrap();
}

fn mistral_models(app: &App) -> usize {
    app.state
        .models_dev
        .current()
        .snapshot
        .providers
        .get("mistral")
        .map_or(0, |p| p.models.len())
}

#[tokio::test]
async fn an_upgrade_that_adds_a_provider_completes_newer_stored_data_and_syncs_it_again() {
    let s = TestServices::start().await;
    let mut doc: Value = serde_json::from_str(kaeso_core::models_dev::EMBEDDED).unwrap();
    doc["anthropic"]["models"]["claude-haiku-4-5"]["cost"]["input"] = json!(1.5);
    let served = doc.to_string();
    let body = Arc::new(Mutex::new((200u16, served.clone())));
    let url = stub_models_dev(body.clone()).await;
    let app = App::start_with(
        &s,
        &[
            ("KAESO_MODELS_DEV_URL", url.as_str()),
            ("KAESO_MODELS_DEV_SYNC", "true"),
            ("KAESO_MODELS_DEV_INTERVAL", "24h"),
        ],
    )
    .await;
    let now = (kaeso_core::models_dev::EMBEDDED_FETCHED_AT + time::Duration::DAY).max(
        time::OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .unwrap(),
    );
    app.clock.set(now);
    let fetched_at = now - time::Duration::HOUR;
    store_as_an_older_build(&app, &served, &url, fetched_at).await;
    let built_in = kaeso_core::models_dev::embedded().unwrap().providers["mistral"]
        .models
        .len();
    assert!(built_in > 0);

    kaeso_server::services::models_dev::init(&app.state)
        .await
        .unwrap();
    let loaded = app.state.models_dev.current();
    assert_eq!(loaded.source, url);
    assert_eq!(loaded.fetched_at, fetched_at);
    assert_eq!(mistral_models(&app), built_in);
    let haiku = loaded
        .snapshot
        .model("anthropic", "claude-haiku-4-5")
        .unwrap();
    assert_eq!(
        haiku.cost.as_ref().unwrap().input.unwrap().to_string(),
        "1.5",
        "the stored data stays in use for the providers it has"
    );

    body.lock().unwrap().0 = 500;
    let job = kaeso_server::jobs::all()
        .into_iter()
        .find(|j| j.name == "models_dev_sync")
        .unwrap();
    assert!(kaeso_server::jobs::run_once(&app.state, job).await.is_err());
    assert_eq!(run_sync_job(&app).await, "not due");
    app.clock.advance(time::Duration::HOUR);
    body.lock().unwrap().0 = 200;
    assert_eq!(
        run_sync_job(&app).await,
        "applied: 0 price changes, 0 delisted, 0 listed again",
        "the same models.dev data is applied again for the new provider"
    );
    assert_eq!(mistral_models(&app), built_in);
    assert_eq!(run_sync_job(&app).await, "not due");
    app.clock.advance(time::Duration::DAY);
    assert_eq!(run_sync_job(&app).await, "unchanged");

    kaeso_server::services::models_dev::init(&app.state)
        .await
        .unwrap();
    assert_eq!(app.state.models_dev.current().source, url);
    assert_eq!(mistral_models(&app), built_in);
}

#[tokio::test]
async fn an_older_builds_copy_of_the_built_in_snapshot_is_replaced() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    store_as_an_older_build(
        &app,
        kaeso_core::models_dev::EMBEDDED,
        kaeso_server::services::models_dev::EMBEDDED_SOURCE,
        kaeso_core::models_dev::EMBEDDED_FETCHED_AT,
    )
    .await;

    kaeso_server::services::models_dev::init(&app.state)
        .await
        .unwrap();
    assert!(mistral_models(&app) > 0);
    let (count, formatted): (i32, bool) =
        sqlx::query_as("SELECT model_count, format IS NOT NULL FROM models_dev_snapshot")
            .fetch_one(&app.state.pg)
            .await
            .unwrap();
    assert_eq!(usize::try_from(count).unwrap(), embedded_model_count());
    assert!(formatted);
}
