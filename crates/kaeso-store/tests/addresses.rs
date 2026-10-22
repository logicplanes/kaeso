#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use kaeso_core::email::{canonical, same_address};
use kaeso_store::pg::users;
use kaeso_testkit::services::TestServices;

const WRITTEN: &[&str] = &[
    "ada@example.com",
    "ADA@Example.com",
    "kim@example.com",
    "\u{212A}im@example.com",
    "\u{212A}IM@example.com",
    "\u{FF4B}\u{FF49}\u{FF4D}@example.com",
    "e\u{301}va@example.com",
    "\u{E9}va@example.com",
    "\u{C9}VA@example.com",
    "E\u{301}VA@example.com",
    "\u{FB01}nn@example.com",
    "finn@example.com",
    "stra\u{DF}e@example.com",
    "STRASSE@example.com",
    "\u{39F}\u{394}\u{3A5}\u{3A3}@example.com",
    "\u{3BF}\u{3B4}\u{3C5}\u{3C2}@example.com",
    "admin@example.com",
    "ADM\u{130}N@example.com",
];

async fn email_key(pg: &sqlx::PgPool, address: &str) -> String {
    sqlx::query_scalar("SELECT email_key($1)")
        .bind(address)
        .fetch_one(pg)
        .await
        .unwrap()
}

#[tokio::test]
async fn the_database_folds_every_address_at_least_as_far_as_canonical() {
    let s = TestServices::start().await;
    kaeso_store::pg::migrate(&s.pg).await.unwrap();
    for written in WRITTEN.iter().filter(|w| !w.contains('\u{130}')) {
        assert_eq!(
            email_key(&s.pg, written).await,
            canonical(written),
            "{written}"
        );
    }
    assert_eq!(
        email_key(&s.pg, "ADM\u{130}N@example.com").await,
        "admin@example.com",
        "the database's lower() folds a dotted capital I to i"
    );
    assert_eq!(
        canonical("ADM\u{130}N@example.com"),
        "admi\u{307}n@example.com"
    );
    assert!(!same_address(
        "ADM\u{130}N@example.com",
        "admin@example.com"
    ));
    for a in WRITTEN {
        for b in WRITTEN {
            if same_address(a, b) {
                assert_eq!(
                    email_key(&s.pg, a).await,
                    email_key(&s.pg, b).await,
                    "{a} and {b} are one address, so the index finds one from the other"
                );
            }
        }
    }
    s.cleanup().await;
}

async fn store_as_before_the_upgrade(pg: &sqlx::PgPool, email: &str) -> uuid::Uuid {
    let id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO users (id, email, name) VALUES ($1, $2, 'Eva')")
        .bind(id)
        .bind(email)
        .execute(pg)
        .await
        .unwrap();
    id
}

fn new_user(email: &str) -> users::NewUser<'_> {
    users::NewUser {
        id: uuid::Uuid::now_v7(),
        email,
        name: "Someone",
        password_hash: None,
        email_verified_at: None,
        is_instance_admin: false,
    }
}

#[tokio::test]
async fn an_address_has_one_account_or_none_however_it_was_stored() {
    let s = TestServices::start().await;
    kaeso_store::pg::migrate(&s.pg).await.unwrap();
    let eva = store_as_before_the_upgrade(&s.pg, "e\u{301}va@example.com").await;
    for typed in ["\u{E9}va@example.com", "\u{C9}VA@example.com"] {
        let found = users::get_by_email(&s.pg, typed).await.unwrap().unwrap();
        assert_eq!(found.id, eva, "{typed}");
        assert_eq!(
            found.email.as_deref(),
            Some("e\u{301}va@example.com"),
            "the stored address is kept for delivery"
        );
    }
    assert!(
        !users::insert(&s.pg, &new_user("\u{C9}va@example.com"))
            .await
            .unwrap(),
        "sign-up and invitations refuse another account with the same canonical form"
    );
    assert!(
        users::email_taken(&s.pg, "\u{E9}VA@example.com", None)
            .await
            .unwrap()
    );
    assert!(
        !users::email_taken(&s.pg, "\u{E9}va@example.com", Some(eva))
            .await
            .unwrap()
    );
    let ada = new_user("ada@example.com");
    assert!(users::insert(&s.pg, &ada).await.unwrap());
    let now = time::OffsetDateTime::UNIX_EPOCH;
    assert!(
        !users::set_email(&s.pg, ada.id, "\u{E9}va@example.com", now)
            .await
            .unwrap(),
        "an email change refuses it too"
    );
    assert_eq!(
        users::get(&s.pg, ada.id)
            .await
            .unwrap()
            .unwrap()
            .email
            .as_deref(),
        Some("ada@example.com")
    );

    let twin = store_as_before_the_upgrade(&s.pg, "\u{E9}va@example.com").await;
    assert_ne!(twin, eva);
    for typed in ["e\u{301}va@example.com", "\u{E9}va@example.com"] {
        assert_eq!(
            users::get_by_email(&s.pg, typed).await.unwrap(),
            None,
            "two accounts share {typed}'s canonical form, so neither is picked"
        );
    }
    s.cleanup().await;
}
