use std::time::Duration;

use kaeso_mail::templates::{self, Email};
use kaeso_mail::{
    Actor, Budget, BudgetScope, Instance, LimitKind, LimitPeriod, Locale, Mailer, SmtpSecurity,
    SmtpSettings, Transport,
};
use serde_json::Value;
use time::macros::date;

const MIN_SUPPORTED: f64 = 88.0;

fn every_email(to: &str) -> Vec<Email> {
    let url = "https://kaeso.example.com/auth/magic-link#token=kGq3VbX9yZ2mN7pR4sT8uW1aC5dE6fH0jK";
    let budget = Budget {
        kind: LimitKind::SpendBudget,
        period: LimitPeriod::Month,
        amount: "50".to_owned(),
    };
    let (due, expires) = (date!(2026 - 11 - 10), date!(2026 - 10 - 18));
    Locale::ALL
        .into_iter()
        .flat_map(|l| {
            [
                templates::invitation(l, to, "Ada Lovelace", "Acme", "developer", url),
                templates::instance_invitation(l, to, "Ada Lovelace", url),
                templates::magic_link(l, to, url),
                templates::password_reset(l, to, url),
                templates::email_change_confirmation(l, to, url),
                templates::signup(l, to, url),
                templates::password_changed(l, to),
                templates::two_factor_enabled(l, to),
                templates::two_factor_disabled(l, to),
                templates::two_factor_reset(l, to, Actor::Administrator),
                templates::two_factor_locked(
                    l,
                    to,
                    10,
                    "https://kaeso.example.com/auth/password-reset",
                ),
                templates::email_changed(l, to, "new@example.com"),
                templates::sessions_ended(l, to, Actor::Administrator),
                templates::account_deactivated(l, to),
                templates::org_scheduled_for_deletion(
                    l,
                    to,
                    "Acme",
                    "Ada Lovelace",
                    due.midnight(),
                    "UTC",
                    url,
                ),
                templates::ownership_transferred(l, to, "Acme", "Ada Lovelace", url),
                templates::key_expiry_warning(
                    l,
                    to,
                    "ci-deploy",
                    "Default project",
                    expires.midnight(),
                    "UTC",
                    url,
                ),
                templates::budget_alert(l, to, &budget, BudgetScope::Project, 80, url),
                templates::test(l, to),
            ]
        })
        .collect()
}

#[tokio::test]
async fn every_email_reaches_an_smtp_server_whole() {
    let smtp =
        std::env::var("KAESO_TEST_SMTP_URL").unwrap_or_else(|_| "smtp://127.0.0.1:51025".into());
    let api =
        std::env::var("KAESO_TEST_MAIL_API").unwrap_or_else(|_| "http://127.0.0.1:58025".into());
    let url = url::Url::parse(&smtp).unwrap();
    let mailer = Mailer::start(
        Transport::Smtp(SmtpSettings {
            host: url.host_str().unwrap().to_owned(),
            port: url.port().unwrap(),
            security: SmtpSecurity::None,
            credentials: None,
        }),
        "Kaeso <kaeso@example.com>",
        Instance::new("https://kaeso.example.com/"),
    )
    .unwrap();
    let to = format!("smtp-{}@example.com", std::process::id());
    let emails = every_email(&to);
    for email in &emails {
        mailer.send_now(email.clone()).await.unwrap();
    }
    let client = reqwest::Client::new();
    let get = |path: String| {
        let client = client.clone();
        async move {
            client
                .get(path)
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()
        }
    };
    let mut list = Value::Null;
    for _ in 0..50 {
        list = get(format!("{api}/api/v1/search?query=to:{to}&limit=100")).await;
        if list["messages_count"].as_u64().unwrap_or(0) >= emails.len() as u64 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let messages = list["messages"].as_array().cloned().unwrap_or_default();
    assert_eq!(messages.len(), emails.len(), "not every email arrived");
    for m in &messages {
        let id = m["ID"].as_str().unwrap();
        let subject = m["Subject"].as_str().unwrap_or_default().to_owned();
        assert!(
            emails.iter().any(|e| e.subject == subject),
            "unexpected subject {subject}"
        );
        let msg = get(format!("{api}/api/v1/message/{id}")).await;
        let text = msg["Text"].as_str().unwrap_or_default();
        assert!(
            [
                "Sent by Kaeso at kaeso.example.com",
                "Gesendet von Kaeso unter kaeso.example.com",
                "此邮件由 kaeso.example.com 上的 Kaeso 发送",
            ]
            .iter()
            .any(|footer| text.contains(footer)),
            "{subject}: no plain text"
        );
        assert!(
            msg["HTML"]
                .as_str()
                .unwrap_or_default()
                .contains("cid:wordmark-light@kaeso"),
            "{subject}: no HTML"
        );
        let inline: Vec<&str> = msg["Inline"]
            .as_array()
            .map(|a| a.iter().filter_map(|p| p["ContentID"].as_str()).collect())
            .unwrap_or_default();
        assert!(
            inline.contains(&"wordmark-light@kaeso") && inline.contains(&"wordmark-dark@kaeso"),
            "{subject}: inline images {inline:?}"
        );
        assert_eq!(
            msg["Attachments"].as_array().map_or(0, Vec::len),
            0,
            "{subject}: the images must not show as attachments"
        );
        let check = get(format!("{api}/api/v1/message/{id}/html-check")).await;
        let supported = check["Total"]["Supported"].as_f64().unwrap_or_default();
        assert!(
            supported >= MIN_SUPPORTED,
            "{subject}: Mailpit's HTML check reports {supported:.1}% supported: {}",
            check["Warnings"]
        );
    }
    client
        .delete(format!("{api}/api/v1/search?query=to:{to}"))
        .send()
        .await
        .unwrap();
}
