#![allow(clippy::unwrap_used)]

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use common::{App, ORIGIN, Owner, Resp};
use kaeso_core::clock::Clock;
use kaeso_mail::Locale;
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};

const PASSWORD: &str = "correct horse battery";

async fn call(
    app: &App,
    method: Method,
    path: &str,
    body: Value,
    cookie: Option<&str>,
    accept_language: Option<&str>,
) -> Resp {
    let mut req = Request::builder()
        .method(method)
        .uri(format!("/platform/v1{path}"))
        .header(header::ORIGIN, ORIGIN)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(c) = cookie {
        req = req.header(header::COOKIE, format!("__Host-kaeso_session={c}"));
    }
    if let Some(l) = accept_language {
        req = req.header(header::ACCEPT_LANGUAGE, l);
    }
    app.raw(
        req.body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap(),
    )
    .await
}

async fn choose(app: &App, cookie: &str, locale: &str) {
    let r = app
        .call(
            Method::PATCH,
            "/me",
            Some(json!({ "locale": locale })),
            Some(cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
}

async fn last(app: &App, to: &str, n: usize) -> (Locale, String) {
    app.wait_for_mail(to, n).await;
    let mail = app.mail.to(to).pop().unwrap();
    (mail.lang, mail.subject)
}

async fn reset(app: &App, email: &str, accept_language: Option<&str>) {
    let r = call(
        app,
        Method::POST,
        "/auth/password-reset",
        json!({ "email": email }),
        None,
        accept_language,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
}

async fn invite(app: &App, owner: &Owner, email: &str, accept_language: Option<&str>) {
    let r = call(
        app,
        Method::POST,
        &format!("/orgs/{}/invites", owner.org_id),
        json!({ "email": email, "preset": "developer" }),
        Some(&owner.cookie),
        accept_language,
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
}

#[tokio::test]
async fn a_users_choice_wins_then_the_browser_then_english() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let to = "owner@example.com";

    reset(&app, to, None).await;
    assert_eq!(
        last(&app, to, 1).await,
        (Locale::En, "Reset your Kaeso password".to_owned())
    );
    reset(&app, to, Some("de-DE,de;q=0.9,en;q=0.8")).await;
    assert_eq!(
        last(&app, to, 2).await,
        (Locale::De, "Kaeso-Passwort zurücksetzen".to_owned())
    );
    reset(&app, to, Some("fr-FR,fr;q=0.9,zh-CN;q=0.8,en;q=0.7")).await;
    assert_eq!(
        last(&app, to, 3).await,
        (Locale::ZhCn, "重置您的 Kaeso 密码".to_owned())
    );
    reset(&app, to, Some("fr-FR,fr;q=0.9")).await;
    assert_eq!(last(&app, to, 4).await.0, Locale::En);

    choose(&app, &owner.cookie, "zh-CN").await;
    reset(&app, to, Some("de")).await;
    assert_eq!(last(&app, to, 5).await.0, Locale::ZhCn);
    choose(&app, &owner.cookie, "de").await;
    let changed = call(
        &app,
        Method::POST,
        "/me/password",
        json!({ "current_password": PASSWORD, "new_password": "another long passphrase" }),
        Some(&owner.cookie),
        Some("zh-CN,en;q=0.5"),
    )
    .await;
    assert_eq!(changed.status, StatusCode::OK, "{:?}", changed.body);
    assert_eq!(
        last(&app, to, 6).await,
        (Locale::De, "Ihr Kaeso-Passwort wurde geändert".to_owned())
    );
    choose(&app, &changed.cookie().unwrap(), "en").await;
    let changed = call(
        &app,
        Method::POST,
        "/me/password",
        json!({ "current_password": "another long passphrase", "new_password": PASSWORD }),
        Some(&changed.cookie().unwrap()),
        Some("de"),
    )
    .await;
    assert_eq!(changed.status, StatusCode::OK, "{:?}", changed.body);
    assert_eq!(
        last(&app, to, 7).await,
        (Locale::En, "Your Kaeso password was changed".to_owned())
    );
    s.cleanup().await;
}

#[tokio::test]
async fn an_invitation_speaks_the_inviters_language() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;

    invite(&app, &owner, "ada@example.com", Some("zh-CN,zh;q=0.9")).await;
    let (lang, subject) = last(&app, "ada@example.com", 1).await;
    assert_eq!(
        (lang, subject.as_str()),
        (Locale::ZhCn, "Owner 邀请您加入 Kaeso 上的 Acme")
    );
    invite(&app, &owner, "grace@example.com", None).await;
    assert_eq!(
        last(&app, "grace@example.com", 1).await,
        (Locale::En, "Owner invited you to Acme on Kaeso".to_owned())
    );

    choose(&app, &owner.cookie, "de").await;
    invite(&app, &owner, "linus@example.com", Some("zh-CN")).await;
    assert_eq!(
        last(&app, "linus@example.com", 1).await,
        (
            Locale::De,
            "Owner hat Sie zu Acme auf Kaeso eingeladen".to_owned()
        )
    );
    for (to, role) in [
        ("ada@example.com", "以开发者身份加入"),
        ("grace@example.com", "as a developer"),
        ("linus@example.com", "als Entwickler beizutreten"),
    ] {
        let mail = app.mail.to(to).pop().unwrap();
        assert!(mail.text.contains(role), "{to}: {}", mail.text);
    }

    let token = app.last_token("linus@example.com");
    let accepted = app
        .post(
            "/auth/invites/accept",
            json!({ "token": token, "name": "Linus", "password": PASSWORD }),
            None,
        )
        .await;
    assert_eq!(accepted.status, StatusCode::OK, "{:?}", accepted.body);
    choose(&app, &accepted.cookie().unwrap(), "zh-CN").await;
    reset(&app, "linus@example.com", Some("de")).await;
    assert_eq!(last(&app, "linus@example.com", 2).await.0, Locale::ZhCn);
    s.cleanup().await;
}

#[tokio::test]
async fn emails_without_a_browser_follow_the_recipients_choice() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;

    choose(&app, &owner.cookie, "de").await;
    let projects = app
        .get(&format!("/orgs/{}/projects", owner.org_id), &owner.cookie)
        .await;
    let project = projects.body["data"][0]["id"].as_str().unwrap().to_owned();
    let r = app
        .post(
            &format!("/orgs/{}/projects/{project}/keys", owner.org_id),
            json!({ "name": "soon", "expires_in_days": 7 }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let job = kaeso_server::jobs::all()
        .into_iter()
        .find(|j| j.name == "key_expiry_warnings")
        .unwrap();
    kaeso_server::jobs::run_once(&app.state, job)
        .await
        .unwrap()
        .unwrap();
    let expires = (app.clock.now() + time::Duration::days(7)).date();
    assert_eq!(
        last(&app, "owner@example.com", 1).await,
        (
            Locale::De,
            format!(
                "Der API-Schlüssel soon läuft am {} ab",
                Locale::De.date(expires)
            )
        )
    );

    choose(&app, &owner.cookie, "zh-CN").await;
    invite(&app, &owner, "member@example.com", None).await;
    app.wait_for_mail("member@example.com", 1).await;
    let token = app.last_token("member@example.com");
    let accepted = app
        .post(
            "/auth/invites/accept",
            json!({ "token": token, "name": "Member", "password": PASSWORD }),
            None,
        )
        .await;
    assert_eq!(accepted.status, StatusCode::OK, "{:?}", accepted.body);
    let fresh = app
        .post(
            "/auth/password",
            json!({ "email": "owner@example.com", "password": PASSWORD }),
            None,
        )
        .await
        .cookie()
        .unwrap();
    let r = call(
        &app,
        Method::DELETE,
        &format!("/orgs/{}", owner.org_id),
        json!({}),
        Some(&fresh),
        Some("de-DE,de"),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let (lang, subject) = last(&app, "owner@example.com", 2).await;
    assert_eq!((lang, subject.as_str()), (Locale::ZhCn, "Acme 已计划删除"));
    let (lang, subject) = last(&app, "member@example.com", 2).await;
    assert_eq!(
        (lang, subject.as_str()),
        (Locale::En, "Acme is scheduled for deletion")
    );
    s.cleanup().await;
}
