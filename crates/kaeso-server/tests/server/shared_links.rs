#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

// Read-only links to usage, member stats and budgets that people without an account can open.

use crate::common;

use axum::http::{Method, StatusCode};
use common::uuid;
use common::v1::Env;
use kaeso_core::clock::Clock;
use kaeso_testkit::webhooks::MockReceiver;
use serde_json::{Value, json};

const LOOPBACK: (&str, &str) = ("KAESO_WEBHOOKS_ALLOWED_NETWORKS", "127.0.0.0/8");

async fn env() -> Env {
    common::v1::env(None, Some(&[LOOPBACK])).await
}

fn links(env: &Env) -> String {
    format!("/orgs/{}/shared-links", env.owner.org_id)
}

async fn allow(env: &Env, allow: &str, max_days: Value) {
    let r = env
        .app
        .call(
            Method::PUT,
            &format!("/orgs/{}/security/shared-links", env.owner.org_id),
            Some(json!({ "allow": allow, "max_days": max_days })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
}

async fn share(env: &Env, cookie: &str, body: Value) -> common::Resp {
    env.app.post(&links(env), body, Some(cookie)).await
}

async fn made(env: &Env, body: Value) -> (String, String) {
    let r = share(env, &env.owner.cookie, body).await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let url = r.body["url"].as_str().unwrap();
    let token = url.rsplit_once("/share/").unwrap().1.to_owned();
    (r.body["link"]["id"].as_str().unwrap().to_owned(), token)
}

async fn open(env: &Env, token: &str, cookie: Option<&str>) -> common::Resp {
    env.app
        .call(Method::GET, &format!("/share/{token}"), None, cookie)
        .await
}

async fn spend(env: &Env, day: &str, model: &str, requests: i64, cost: &str) {
    let format = time::macros::format_description!("[year]-[month]-[day]");
    kaeso_store::pg::limits::add_usage(
        &env.app.state.pg,
        &[kaeso_store::pg::limits::UsageRow {
            day: time::Date::parse(day, &format).unwrap(),
            org_id: uuid(&env.owner.org_id),
            project_id: uuid(&env.project),
            user_id: Some(uuid(&env.owner.user_id)),
            service_account_id: None,
            api_key_id: None,
            catalogue_model_id: None,
            model_id: model.into(),
            provider: "openai".into(),
            ingress_dialect: "openai_chat".into(),
            egress_dialect: "openai_chat".into(),
            credential_id: None,
            source_id: None,
            requests,
            errors: 0,
            input_tokens: 100,
            output_tokens: 10,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            reasoning_tokens: 0,
            cost: cost.parse().unwrap(),
            media: Default::default(),
        }],
    )
    .await
    .unwrap();
}

fn today(env: &Env) -> String {
    env.app.clock.now().date().to_string()
}

fn breakdown<'a>(view: &'a Value, by: &str) -> &'a Vec<Value> {
    view["breakdowns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["by"] == by)
        .unwrap_or_else(|| panic!("no {by} in {view:?}"))["rows"]
        .as_array()
        .unwrap()
}

#[tokio::test]
async fn anyone_with_the_link_sees_the_live_usage_without_what_it_hides() {
    let env = env().await;
    allow(&env, "anyone", json!(90)).await;
    let day = today(&env);
    spend(&env, &day, "gpt-4o", 7, "1.5").await;
    let (id, token) = made(
        &env,
        json!({
            "kind": "analytics", "project": env.project, "audience": "anyone",
            "days": 7, "expires_in_days": 30, "hide_members": true, "metric": "tokens",
        }),
    )
    .await;

    let r = open(&env, &token, None).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.headers.get("x-robots-tag").unwrap(), "noindex, nofollow");
    let v = &r.body;
    assert_eq!(v["kind"], "analytics");
    assert_eq!(v["title"], "Checkout usage");
    assert_eq!(
        (&v["organization"], &v["project"]),
        (&json!("Acme"), &json!("Checkout"))
    );
    assert_eq!(
        (v["live"].as_bool(), &v["metric"]),
        (Some(true), &json!("tokens"))
    );
    assert_eq!(v["to"], day.as_str());
    assert_eq!(v["costs"], false);
    assert_eq!(v["made_with_kaeso"], true);
    assert_eq!(v["days"][0]["key"], day.as_str());
    assert_eq!(v["days"][0]["requests"], 7);
    assert_eq!(v["days"][0]["cost"], Value::Null);
    assert_eq!(breakdown(v, "model")[0]["key"], "openai/gpt-4o");
    let member = &breakdown(v, "member")[0];
    assert_eq!(
        (&member["name"], &member["number"]),
        (&Value::Null, &json!(1))
    );
    assert_eq!(member["key"], Value::Null, "no ids of people");

    let listed = env.app.get(&links(&env), &env.owner.cookie).await;
    let row = &listed.body["data"][0];
    assert_eq!((&row["id"], &row["views"]), (&json!(id), &json!(1)));
    assert!(row["last_opened_at"].is_string());
    assert!(row.get("url").is_none(), "the address is shown once");
    let audit = env
        .app
        .get(
            &format!("/orgs/{}/audit-log", env.owner.org_id),
            &env.owner.cookie,
        )
        .await;
    let actions: Vec<&str> = audit.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["action"].as_str().unwrap())
        .collect();
    assert!(
        actions.contains(&"shared_link.create") && actions.contains(&"shared_link.open"),
        "{actions:?}"
    );
    env.app.state.in_flight.settled().await;
    assert!(
        r.body.to_string().find(&token).is_none(),
        "the view never repeats its token"
    );
}

#[tokio::test]
async fn costs_show_only_when_asked_for_by_someone_who_sees_them() {
    let env = env().await;
    spend(&env, &today(&env), "gpt-4o", 2, "0.75").await;
    let (_, token) = made(
        &env,
        json!({ "kind": "project_card", "project": env.project, "show_costs": true, "expires_in_days": 7 }),
    )
    .await;
    let r = open(&env, &token, Some(&env.owner.cookie)).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["costs"], true);
    assert_eq!(r.body["days"][0]["cost"], "0.75");
    assert_eq!(r.body["title"], "Checkout summary");

    // A member sees their own usage, but not its cost.
    let (eve, eve_id) = common::join(
        &env.app,
        &env.owner,
        json!({ "email": "eve@example.com", "role": "member" }),
        "Eve",
    )
    .await;
    let mine = json!({ "kind": "member", "member": eve_id, "expires_in_days": 7 });
    let refused = share(
        &env,
        &eve,
        json!({ "kind": "member", "member": eve_id, "show_costs": true, "expires_in_days": 7 }),
    )
    .await;
    assert_eq!(
        refused.status,
        StatusCode::BAD_REQUEST,
        "{:?}",
        refused.body
    );
    assert_eq!(refused.body["error"]["param"], "show_costs");
    let ok = share(&env, &eve, mine).await;
    assert_eq!(ok.status, StatusCode::CREATED, "{:?}", ok.body);
    assert_eq!(ok.body["link"]["title"], "Eve's usage");
    let others = share(
        &env,
        &eve,
        json!({ "kind": "member", "member": env.owner.user_id, "expires_in_days": 7 }),
    )
    .await;
    assert_eq!(others.status, StatusCode::FORBIDDEN, "{:?}", others.body);
    let project = share(
        &env,
        &eve,
        json!({ "kind": "analytics", "project": env.project, "expires_in_days": 7 }),
    )
    .await;
    assert_eq!(project.status, StatusCode::FORBIDDEN, "not in the project");
}

#[tokio::test]
async fn members_only_links_need_a_members_session_and_the_policy_holds() {
    let env = env().await;
    let (_, token) = made(&env, json!({ "kind": "analytics", "expires_in_days": 30 })).await;
    let r = open(&env, &token, None).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{:?}", r.body);
    assert_eq!(
        r.body["error"]["message"],
        "This link is for the organization's members; sign in to see it."
    );
    assert_eq!(
        open(&env, &token, Some(&env.owner.cookie)).await.status,
        StatusCode::OK
    );
    assert_eq!(
        open(&env, &token, Some(&env.owner.cookie)).await.body["project"],
        Value::Null
    );

    // The default policy allows members only, and links that last at most 90 days.
    let public = share(
        &env,
        &env.owner.cookie,
        json!({ "kind": "analytics", "audience": "anyone", "expires_in_days": 7 }),
    )
    .await;
    assert_eq!(public.status, StatusCode::FORBIDDEN, "{:?}", public.body);
    let forever = share(&env, &env.owner.cookie, json!({ "kind": "analytics" })).await;
    assert_eq!(forever.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        forever.body["error"]["message"],
        "The organization allows at most 90 days."
    );
    allow(&env, "anyone", Value::Null).await;
    let (_, public) = made(&env, json!({ "kind": "analytics", "audience": "anyone" })).await;
    assert_eq!(open(&env, &public, None).await.status, StatusCode::OK);

    // Tightened again, the public link needs a member; turned off, no link works.
    allow(&env, "members", json!(30)).await;
    assert_eq!(
        open(&env, &public, None).await.status,
        StatusCode::UNAUTHORIZED
    );
    allow(&env, "off", json!(30)).await;
    assert_eq!(
        open(&env, &public, Some(&env.owner.cookie)).await.status,
        StatusCode::NOT_FOUND
    );
    let off = share(
        &env,
        &env.owner.cookie,
        json!({ "kind": "analytics", "expires_in_days": 7 }),
    )
    .await;
    assert_eq!(off.status, StatusCode::FORBIDDEN);

    // Nor while the install turns shared links off.
    allow(&env, "anyone", json!(90)).await;
    let r = env
        .app
        .call(
            Method::PATCH,
            "/instance/settings",
            Some(json!({ "shared_links": false, "share_mark": false })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(
        (&r.body["shared_links"], &r.body["share_mark"]),
        (&json!(false), &json!(false))
    );
    assert_eq!(
        open(&env, &public, None).await.status,
        StatusCode::NOT_FOUND
    );
    env.app
        .call(
            Method::PATCH,
            "/instance/settings",
            Some(json!({ "shared_links": true })),
            Some(&env.owner.cookie),
        )
        .await;
    let back = open(&env, &public, None).await;
    assert_eq!(back.status, StatusCode::OK);
    assert_eq!(back.body["made_with_kaeso"], false);
}

#[tokio::test]
async fn expired_revoked_and_unknown_links_say_the_same() {
    let env = env().await;
    allow(&env, "anyone", json!(90)).await;
    let (_, soon) = made(
        &env,
        json!({ "kind": "analytics", "audience": "anyone", "expires_in_days": 1 }),
    )
    .await;
    let (id, revoked) = made(
        &env,
        json!({ "kind": "analytics", "audience": "anyone", "expires_in_days": 30 }),
    )
    .await;
    let r = env
        .app
        .call(
            Method::DELETE,
            &format!("{}/{id}", links(&env)),
            None,
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT, "{:?}", r.body);
    env.app.clock.advance(time::Duration::days(2));
    for token in [soon.as_str(), revoked.as_str(), "not-a-token"] {
        let r = open(&env, token, None).await;
        assert_eq!(r.status, StatusCode::NOT_FOUND, "{token}");
        assert_eq!(
            r.body["error"]["message"],
            "This shared link doesn't exist, has expired or was revoked."
        );
    }
    let listed = env.app.get(&links(&env), &env.owner.cookie).await;
    assert_eq!(listed.body["data"], json!([]));
}

#[tokio::test]
async fn a_link_stops_when_its_maker_can_no_longer_see_it_and_only_they_or_a_manager_revoke_it() {
    let env = env().await;
    allow(&env, "anyone", json!(90)).await;
    let (dev, dev_id) = common::join(
        &env.app,
        &env.owner,
        json!({ "email": "dev@example.com", "role": "developer" }),
        "Dev",
    )
    .await;
    let r = share(
        &env,
        &dev,
        json!({ "kind": "analytics", "audience": "anyone", "expires_in_days": 7 }),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let token = r.body["url"]
        .as_str()
        .unwrap()
        .rsplit_once("/share/")
        .unwrap()
        .1
        .to_owned();
    let (owners, _) = made(&env, json!({ "kind": "analytics", "expires_in_days": 7 })).await;

    // A maker without `usage.share.manage` sees and revokes only their own links.
    let theirs = env.app.get(&links(&env), &dev).await;
    assert_eq!(theirs.body["data"].as_array().unwrap().len(), 1);
    let r = env
        .app
        .call(
            Method::DELETE,
            &format!("{}/{owners}", links(&env)),
            None,
            Some(&dev),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{:?}", r.body);
    assert_eq!(
        env.app.get(&links(&env), &env.owner.cookie).await.body["data"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    assert_eq!(open(&env, &token, None).await.status, StatusCode::OK);
    let r = env
        .app
        .call(
            Method::PATCH,
            &format!("/orgs/{}/members/{dev_id}", env.owner.org_id),
            Some(json!({ "role": "member" })),
            Some(&env.owner.cookie),
        )
        .await;
    assert!(r.status.is_success(), "{:?}", r.body);
    assert_eq!(
        open(&env, &token, None).await.status,
        StatusCode::NOT_FOUND,
        "a member can't see the org's usage"
    );
    let r = env
        .app
        .call(
            Method::PATCH,
            &format!("/orgs/{}/members/{dev_id}", env.owner.org_id),
            Some(json!({ "role": "developer" })),
            Some(&env.owner.cookie),
        )
        .await;
    assert!(r.status.is_success(), "{:?}", r.body);
    assert_eq!(open(&env, &token, None).await.status, StatusCode::OK);
    let r = env
        .app
        .call(
            Method::DELETE,
            &format!("/orgs/{}/members/{dev_id}", env.owner.org_id),
            None,
            Some(&env.owner.cookie),
        )
        .await;
    assert!(r.status.is_success(), "{:?}", r.body);
    assert_eq!(
        open(&env, &token, None).await.status,
        StatusCode::NOT_FOUND,
        "the maker left"
    );
}

#[tokio::test]
async fn a_snapshot_keeps_the_numbers_it_was_made_with() {
    let env = env().await;
    let day = today(&env);
    spend(&env, &day, "gpt-4o", 3, "1").await;
    let (_, frozen) = made(
        &env,
        json!({ "kind": "analytics", "project": env.project, "live": false, "from": day, "to": day, "expires_in_days": 7, "show_costs": true }),
    )
    .await;
    let (_, live) = made(
        &env,
        json!({ "kind": "analytics", "project": env.project, "days": 1, "expires_in_days": 7 }),
    )
    .await;
    spend(&env, &day, "gpt-4o", 5, "2").await;
    let kept = open(&env, &frozen, Some(&env.owner.cookie)).await;
    assert_eq!(
        (
            kept.body["live"].as_bool(),
            &kept.body["days"][0]["requests"]
        ),
        (Some(false), &json!(3))
    );
    assert_eq!(kept.body["days"][0]["cost"], "1");
    let now = open(&env, &live, Some(&env.owner.cookie)).await;
    assert_eq!(now.body["days"][0]["requests"], 8);

    let wrong = share(
        &env,
        &env.owner.cookie,
        json!({ "kind": "analytics", "live": false, "from": day, "expires_in_days": 7 }),
    )
    .await;
    assert_eq!(wrong.status, StatusCode::BAD_REQUEST);
    assert_eq!(wrong.body["error"]["param"], "from");
}

#[tokio::test]
async fn a_budget_link_shows_the_projects_limits_and_spend_budgets_only_with_costs() {
    let env = env().await;
    for body in [
        json!({ "level": "project", "kind": "spend_budget", "period": "month", "amount": "50" }),
        json!({ "level": "project", "kind": "token_budget", "period": "month", "amount": 100000 }),
        json!({ "level": "member", "user": env.owner.user_id, "kind": "request_rate", "period": "minute", "amount": 60 }),
    ] {
        let r = env
            .app
            .post(
                &format!("/orgs/{}/projects/{}/limits", env.owner.org_id, env.project),
                body,
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    }
    let (_, plain) = made(&env, json!({ "kind": "budget", "project": env.project, "expires_in_days": 7, "hide_members": true })).await;
    let r = open(&env, &plain, Some(&env.owner.cookie)).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let kinds: Vec<&str> = r.body["limits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds.len(), 2, "{kinds:?}");
    assert!(!kinds.contains(&"spend_budget"));
    let member = r.body["limits"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["level"] == "member")
        .unwrap();
    assert_eq!(
        (&member["name"], &member["number"]),
        (&Value::Null, &json!(1))
    );
    let (_, priced) = made(&env, json!({ "kind": "budget", "project": env.project, "expires_in_days": 7, "show_costs": true })).await;
    let r = open(&env, &priced, Some(&env.owner.cookie)).await;
    let spend = r.body["limits"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["kind"] == "spend_budget")
        .unwrap();
    assert_eq!(
        (&spend["amount"], &spend["used"]),
        (&json!("50"), &json!("0"))
    );
    let member = r.body["limits"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["level"] == "member")
        .unwrap();
    assert_eq!(member["name"], "Owner");

    let missing = share(
        &env,
        &env.owner.cookie,
        json!({ "kind": "budget", "expires_in_days": 7 }),
    )
    .await;
    assert_eq!(missing.status, StatusCode::BAD_REQUEST);
    assert_eq!(missing.body["error"]["param"], "project");
    let kind = share(
        &env,
        &env.owner.cookie,
        json!({ "kind": "logs", "expires_in_days": 7 }),
    )
    .await;
    assert_eq!(
        kind.body["error"]["message"],
        "`logs` is not one of analytics, member, budget, project_card."
    );
}

#[tokio::test]
async fn the_preview_is_an_image_and_a_members_only_link_shows_no_numbers_in_it() {
    let env = env().await;
    allow(&env, "anyone", json!(90)).await;
    spend(&env, &today(&env), "gpt-4o", 3, "1").await;
    let (_, public) = made(
        &env,
        json!({ "kind": "analytics", "audience": "anyone", "expires_in_days": 7 }),
    )
    .await;
    let (_, members) = made(&env, json!({ "kind": "analytics", "expires_in_days": 7 })).await;
    let mut sizes = Vec::new();
    for token in [&public, &members] {
        let req = axum::http::Request::get(format!("/platform/v1/share/{token}/image.png"))
            .body(axum::body::Body::empty())
            .unwrap();
        let (status, headers, png) = env.app.raw_bytes(req).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers.get("content-type").unwrap(), "image/png");
        assert_eq!(&png[1..4], b"PNG");
        sizes.push(png.len());
    }
    assert_ne!(
        sizes[0], sizes[1],
        "the public link's chart differs from the plain cover"
    );
    let listed = env.app.get(&links(&env), &env.owner.cookie).await;
    assert!(
        listed.body["data"]
            .as_array()
            .unwrap()
            .iter()
            .all(|l| l["views"] == 0),
        "previews aren't opens"
    );
}

#[tokio::test]
async fn a_new_link_reaches_the_organizations_webhooks_as_share_created() {
    let env = env().await;
    allow(&env, "anyone", json!(90)).await;
    let receiver = MockReceiver::start().await;
    let r = env
        .app
        .post(
            &format!("/orgs/{}/webhooks", env.owner.org_id),
            json!({ "name": "Security", "url": receiver.url, "kinds": ["org_security"] }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    made(&env, json!({ "kind": "analytics", "audience": "anyone", "expires_in_days": 7, "title": "October" })).await;
    for _ in 0..200 {
        env.app.state.in_flight.settled().await;
        kaeso_server::services::webhooks::run(env.app.state.clone())
            .await
            .unwrap();
        if !receiver.got().is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let [got] = &receiver.got()[..] else {
        panic!("{:?}", receiver.got());
    };
    let event = got.json();
    assert_eq!(event["type"], "share.created");
    assert_eq!(event["data"]["title"], "October");
    assert_eq!(event["data"]["audience"], "anyone");
    assert_eq!(event["data"]["by"], "Owner");
}
