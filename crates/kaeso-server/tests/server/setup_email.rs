#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use crate::common;

use axum::http::StatusCode;
use common::App;
use kaeso_testkit::services::TestServices;
use serde_json::json;

const WITH_EMAIL: [(&str, &str); 3] = [
    ("KAESO_SETUP_EMAIL", "owner@example.com"),
    ("KAESO_MAIL_TRANSPORT", "smtp"),
    ("KAESO_SMTP_HOST", "mail.example.com"),
];

fn setup_token(app: &App) -> String {
    let sent = app.mail.all();
    let email = sent
        .iter()
        .rev()
        .find(|m| m.kind.as_str() == "setup_link")
        .unwrap_or_else(|| {
            panic!(
                "no setup email among {:?}",
                sent.iter().map(|m| m.kind.as_str()).collect::<Vec<_>>()
            )
        });
    assert_eq!(email.to, "owner@example.com");
    assert_eq!(email.subject, "Set up Kaeso");
    let link = email
        .text
        .split_whitespace()
        .find(|w| w.contains("/setup#token="))
        .unwrap_or_else(|| panic!("no setup link in {}", email.text));
    link.split("#token=").nth(1).unwrap().to_owned()
}

#[tokio::test]
async fn the_setup_link_is_emailed_at_every_start_until_the_first_user_exists() {
    let s = TestServices::start().await;
    let first = App::start_with(&s, &WITH_EMAIL).await;
    let earlier = setup_token(&first);
    let second = first.second_instance_with(&s, &WITH_EMAIL).await;
    let fresh = setup_token(&second);
    assert_ne!(earlier, fresh, "every start sends a fresh link");
    let set_up = |token: &str| json!({ "token": token, "email": "owner@example.com", "name": "Ada Okafor", "password": "correct horse battery staple", "org_name": "Acme" });
    // Two gateways that start together each email a link, and either works.
    let done = second.post("/auth/setup", set_up(&earlier), None).await;
    assert_eq!(done.status, StatusCode::OK, "{:?}", done.body);
    let refused = second.post("/auth/setup", set_up(&fresh), None).await;
    assert_eq!(refused.status, StatusCode::CONFLICT, "{:?}", refused.body);
    let sent_before = second.mail.all().len();
    let third = second.second_instance_with(&s, &WITH_EMAIL).await;
    assert_eq!(
        third.mail.all().len(),
        sent_before,
        "once a user exists, a start sends nothing"
    );
}

#[tokio::test]
async fn a_printed_setup_link_retires_the_emailed_ones() {
    let s = TestServices::start().await;
    let app = App::start_with(&s, &WITH_EMAIL).await;
    let emailed = setup_token(&app);
    let printed = kaeso_server::setup::issue_link(&app.state)
        .await
        .unwrap()
        .unwrap();
    let printed = printed.split("#token=").nth(1).unwrap().to_owned();
    let set_up = |token: &str| json!({ "token": token, "email": "owner@example.com", "name": "Ada Okafor", "password": "correct horse battery staple", "org_name": "Acme" });
    let refused = app.post("/auth/setup", set_up(&emailed), None).await;
    assert_eq!(
        refused.status,
        StatusCode::BAD_REQUEST,
        "{:?}",
        refused.body
    );
    let done = app.post("/auth/setup", set_up(&printed), None).await;
    assert_eq!(done.status, StatusCode::OK, "{:?}", done.body);
}

#[tokio::test]
async fn the_setting_needs_an_address_and_the_smtp_transport() {
    let s = TestServices::start().await;
    let refused = |extra: &'static [(&'static str, &'static str)]| {
        let s = &s;
        async move { common::config_result(s, extra).unwrap_err().to_string() }
    };
    assert!(
        refused(&[("KAESO_SETUP_EMAIL", "owner@example.com")])
            .await
            .contains("setup.email"),
        "the log transport can't deliver the link"
    );
    assert!(
        refused(&[
            ("KAESO_SETUP_EMAIL", "not-an-address"),
            ("KAESO_MAIL_TRANSPORT", "smtp"),
            ("KAESO_SMTP_HOST", "mail.example.com"),
        ])
        .await
        .contains("one email address")
    );
}
