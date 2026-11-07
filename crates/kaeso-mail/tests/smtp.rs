use std::time::Duration;

use kaeso_mail::notices::{
    self, Because, DigestItem, Fact, Install, Links, Notes, ProjectUse, Update,
};
use kaeso_mail::templates::{self, Email};
use kaeso_mail::{
    Actor, Budget, BudgetAlert, BudgetScope, Instance, LimitKind, LimitPeriod, Locale, Mailer,
    SmtpSecurity, SmtpSettings, Transport,
};
use serde_json::Value;
use time::macros::{date, datetime};

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
                    notices::Place {
                        organization: "Acme",
                        project: Some("Default project"),
                    },
                    expires.midnight(),
                    "UTC",
                    url,
                ),
                templates::budget_alert(
                    l,
                    to,
                    &BudgetAlert {
                        organization: "Acme",
                        project: "Checkout",
                        scope: BudgetScope::Project,
                        budget: &budget,
                        percent: 80,
                        used: "40",
                        resets: Some(date!(2026 - 12 - 01).midnight()),
                    },
                    url,
                ),
                templates::test(l, to),
            ]
            .into_iter()
            .chain(reports(l, to))
        })
        .collect()
}

fn reports(l: Locale, to: &str) -> Vec<Email> {
    let links = Links {
        settings: "https://kaeso.example.com/settings/account/notifications".to_owned(),
        unsubscribe: None,
    };
    let open = "https://kaeso.example.com/settings/instance/updates";
    let project = |name: &str, requests: Option<&str>, cost: &str| ProjectUse {
        organization: "Acme".to_owned(),
        name: name.to_owned(),
        requests: requests.map(str::to_owned),
        cost: cost.to_owned(),
    };
    let facts = [
        Fact::WeeklySummary {
            from: date!(2026 - 11 - 02),
            to: date!(2026 - 11 - 08),
            requests: "12480".to_owned(),
            tool_calls: "312".to_owned(),
            cost: "41.20".to_owned(),
            projects: vec![project("Checkout", Some("9120"), "30.10")],
            top_models: vec!["gpt-4o".to_owned()],
        },
        Fact::CostReport {
            month: date!(2026 - 10 - 01),
            total: "412.50",
            projects: vec![project("Checkout", None, "300.00")],
        },
        Fact::InstanceUpdate {
            version: "0.6.0",
            change: Update::Available {
                running: "0.5.6",
                install: Install::Window {
                    at: datetime!(2026 - 11 - 09 03:00),
                    zone: "UTC",
                },
            },
            notes: Some(Notes {
                markdown: "### Added\n\n- Cerebras is a built-in provider [#642](https://github.com/logicplanes/kaeso/pull/642)\n",
                url: "https://github.com/logicplanes/kaeso/releases/tag/v0.6.0",
            }),
        },
    ];
    facts
        .iter()
        .map(|fact| {
            notices::notice(
                l,
                to,
                None,
                fact,
                Because::Right("usage.view"),
                open,
                &links,
            )
        })
        .chain([notices::digest(
            l,
            to,
            &[DigestItem {
                line: "Grace Hopper left".to_owned(),
                organization: Some("Acme".to_owned()),
                project: None,
            }],
            open,
            &links,
        )])
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
        let html = msg["HTML"].as_str().unwrap_or_default();
        let referenced: Vec<&str> = html
            .match_indices("src=\"cid:")
            .filter_map(|(at, _)| html[at + 9..].split('"').next())
            .collect();
        assert!(!referenced.is_empty(), "{subject}: no HTML images");
        let inline: Vec<&str> = msg["Inline"]
            .as_array()
            .map(|a| a.iter().filter_map(|p| p["ContentID"].as_str()).collect())
            .unwrap_or_default();
        let mut wanted = referenced.clone();
        wanted.sort_unstable();
        wanted.dedup();
        let mut attached = inline.clone();
        attached.sort_unstable();
        assert_eq!(
            attached, wanted,
            "{subject}: every inline image is shown and every shown image is inline"
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
