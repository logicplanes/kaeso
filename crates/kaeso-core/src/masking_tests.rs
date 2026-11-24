#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use super::*;

fn rules(detectors: &[(Detector, RuleAction)]) -> Rules {
    Rules {
        detectors: detectors.iter().copied().collect(),
        ..Rules::default()
    }
}

fn masked(detector: Detector, text: &str) -> Option<String> {
    let scanner = rules(&[(detector, RuleAction::Mask)]).compile().unwrap();
    scanner.masker().text(text)
}

fn finds(detector: Detector, yes: &[&str], no: &[&str]) {
    for t in yes {
        assert!(masked(detector, t).is_some(), "{detector:?} finds {t}");
    }
    for t in no {
        assert_eq!(masked(detector, t), None, "{detector:?} leaves {t}");
    }
}

#[test]
fn secrets_are_the_key_shapes_kaeso_knows() {
    finds(
        Detector::Secrets,
        &[
            "key sk-ant-api03-abcdefghijklmnopqrstuvwxyz",
            "sk-kso-0123456789abcdef0123456789abcdef",
            "gsk_0123456789abcdefABCDEF",
            "AKIAIOSFODNN7EXAMPLE",
            "ghp_0123456789abcdefghijklmnopqrstuvwxyzAB",
            "github_pat_11ABCDEFG0123456789_abcdefghijklmnop",
            "glpat-0123456789abcdefghij",
            "-----BEGIN RSA PRIVATE KEY-----\nMIIEow\n-----END RSA PRIVATE KEY-----",
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NSJ9.c2lnbmF0dXJlLXNpZ25lZA",
        ],
        &[
            "sk-short",
            "the task-force met",
            "AKIA0123",
            "-----BEGIN PUBLIC KEY-----\nMIIB\n-----END PUBLIC KEY-----",
        ],
    );
    assert_eq!(
        masked(
            Detector::Secrets,
            "use sk-ant-api03-abcdefghijklmnopqrstuvwxyz now"
        )
        .unwrap(),
        "use [API_KEY_1] now"
    );
    assert_eq!(
        masked(
            Detector::Secrets,
            "-----BEGIN PRIVATE KEY-----\nabc\n-----END PRIVATE KEY-----"
        )
        .unwrap(),
        "[PRIVATE_KEY_1]"
    );
}

#[test]
fn personal_data_has_a_checkable_form() {
    finds(
        Detector::Email,
        &["mail ada@example.com", "a.b+c@mail.example.co.uk"],
        &["ada@", "@example.com", "ada at example dot com"],
    );
    finds(
        Detector::Phone,
        &["call +4930123456", "(+14155550123)"],
        &["call 030 123456", "+0123456789", "+123", "x+4930123456"],
    );
    finds(
        Detector::Iban,
        &["DE89370400440532013000", "IBAN GB82 WEST 1234 5698 7654 32"],
        &["DE89370400440532013001", "GB82 WEST 1234 5698 7654 33"],
    );
    finds(
        Detector::Card,
        &[
            "4111111111111111",
            "card 4111 1111 1111 1111",
            "5500-0000-0000-0004",
        ],
        &["4111111111111112", "1234 5678 9012", "order 2026111612"],
    );
    finds(
        Detector::Ip,
        &["from 192.0.2.7", "2001:db8::1", "fe80::1ff:fe23:4567:890a"],
        &["999.1.1.1", "version 1.2.3", "a:b", "12:30"],
    );
}

#[test]
fn the_same_value_keeps_its_placeholder_within_a_request() {
    let scanner = rules(&[(Detector::Email, RuleAction::Mask)])
        .compile()
        .unwrap();
    let mut m = scanner.masker();
    assert_eq!(
        m.text("ada@example.com and bob@example.com").unwrap(),
        "[EMAIL_1] and [EMAIL_2]"
    );
    assert_eq!(m.text("again ada@example.com").unwrap(), "again [EMAIL_1]");
    assert_eq!(m.found["email"].count, 3);
    assert_eq!(m.found["email"].detector, "email");
    assert_eq!(
        m.logged(),
        [
            ("ada@example.com".to_owned(), "[EMAIL_1]".to_owned()),
            ("bob@example.com".to_owned(), "[EMAIL_2]".to_owned())
        ]
    );
    assert_eq!(
        m.restorer(),
        None,
        "nothing is restored unless the rules ask"
    );
}

#[test]
fn block_and_flag_change_nothing_and_say_what_they_found() {
    let scanner = rules(&[
        (Detector::Card, RuleAction::Block),
        (Detector::Email, RuleAction::Flag),
    ])
    .compile()
    .unwrap();
    let mut m = scanner.masker();
    assert_eq!(m.text("pay 4111111111111111 for ada@example.com"), None);
    assert_eq!(m.blocked.as_deref(), Some("card"));
    assert_eq!(m.found["email"].action, Some(RuleAction::Flag));
    assert!(m.logged().is_empty());

    let blocked_logged = Rules {
        mask_log: true,
        ..rules(&[(Detector::Card, RuleAction::Block)])
    }
    .compile()
    .unwrap();
    let mut m = blocked_logged.masker();
    assert_eq!(m.text("pay 4111111111111111"), None);
    assert_eq!(
        m.logged(),
        [("4111111111111111".to_owned(), "[CARD_1]".to_owned())]
    );

    let logged = Rules {
        mask_log: true,
        ..rules(&[(Detector::Email, RuleAction::Flag)])
    }
    .compile()
    .unwrap();
    let mut m = logged.masker();
    assert_eq!(
        m.text("ada@example.com"),
        None,
        "the request goes out unchanged"
    );
    assert_eq!(
        m.logged(),
        [("ada@example.com".to_owned(), "[EMAIL_1]".to_owned())]
    );
}

#[test]
fn a_card_or_iban_next_to_other_groups_is_still_found() {
    let scanner = rules(&[
        (Detector::Card, RuleAction::Mask),
        (Detector::Iban, RuleAction::Mask),
    ])
    .compile()
    .unwrap();
    for (text, masked) in [
        ("4111 1111 1111 1111 123", "[CARD_1] 123"),
        ("4111111111111111 123", "[CARD_1] 123"),
        ("4111-1111-1111-1111 12/29 123", "[CARD_1] 12/29 123"),
        ("id 7 4111 1111 1111 1111", "id 7 [CARD_1]"),
        ("ES91 2100 0418 4502 0005 1332 PAID", "[IBAN_1] PAID"),
        ("ES91 2100 0418 4502 0005 1332 PAID OK", "[IBAN_1] PAID OK"),
        (
            "ES91 2100 0418 4502 0005 1332 PAID 2026",
            "[IBAN_1] PAID 2026",
        ),
        ("ref 501 4111 1111 1111 1111", "ref 501 [CARD_1]"),
    ] {
        assert_eq!(scanner.masker().text(text).as_deref(), Some(masked));
    }
    assert_eq!(masked(Detector::Card, "1234 5678 9012 3456 789"), None);
    assert_eq!(
        masked(Detector::Card, "3 1 4 1 5 9 2 6 5 3 5 8 9 7 9 3 2 3 8"),
        None
    );
    assert_eq!(
        masked(Detector::Iban, "ES91 2100 0418 4502 0005 1332 2026"),
        None
    );
    assert_eq!(
        masked(Detector::Iban, "DE89 3704 0044 0532 0130 01 OK"),
        None
    );
}

#[test]
fn overlapping_finds_keep_the_earliest_and_longest() {
    let scanner = rules(&[
        (Detector::Email, RuleAction::Mask),
        (Detector::Ip, RuleAction::Mask),
    ])
    .compile()
    .unwrap();
    assert_eq!(
        scanner.masker().text("root@192.0.2.7.example.com").unwrap(),
        "[EMAIL_1]"
    );
}

#[test]
fn custom_patterns_are_checked_and_named() {
    let custom = |name: &str, pattern: &str| Rules {
        custom: vec![CustomRule {
            name: name.to_owned(),
            pattern: pattern.to_owned(),
            action: RuleAction::Mask,
        }],
        ..Rules::default()
    };
    let scanner = custom("order_id", r"ORD-\d{6}").compile().unwrap();
    let mut m = scanner.masker();
    assert_eq!(m.text("see ORD-123456").unwrap(), "see [ORDER_ID_1]");
    assert_eq!(m.found["order_id"].detector, "custom");
    assert_eq!(
        custom("Order", "x").compile().unwrap_err(),
        RulesError::Name(0)
    );
    assert_eq!(
        custom("email", "x").compile().unwrap_err(),
        RulesError::Name(0)
    );
    assert_eq!(
        custom("a", "").compile().unwrap_err(),
        RulesError::Length(0)
    );
    assert_eq!(
        custom("a", &"a".repeat(MAX_PATTERN + 1))
            .compile()
            .unwrap_err(),
        RulesError::Length(0)
    );
    assert_eq!(
        custom("a", "(").compile().unwrap_err(),
        RulesError::Pattern(0)
    );
    assert_eq!(
        custom("a", r"\w{1000}{1000}").compile().unwrap_err(),
        RulesError::Pattern(0),
        "a pattern whose program grows past the limit is refused"
    );
    let mut twice = custom("a", "x");
    twice.custom.push(twice.custom[0].clone());
    assert_eq!(twice.compile().unwrap_err(), RulesError::Repeated(1));
    let mut many = custom("a", "x");
    many.custom = (0..=MAX_CUSTOM)
        .map(|i| CustomRule {
            name: format!("p{i}"),
            pattern: "x".to_owned(),
            action: RuleAction::Flag,
        })
        .collect();
    assert_eq!(many.compile().unwrap_err(), RulesError::TooMany);
    assert_eq!(Rules::default().compile().unwrap_err(), RulesError::Empty);
}

#[test]
fn restoring_puts_values_back_also_across_pieces() {
    let scanner = Rules {
        restore: true,
        ..rules(&[(Detector::Email, RuleAction::Mask)])
    }
    .compile()
    .unwrap();
    let mut m = scanner.masker();
    m.text("ada@example.com").unwrap();
    let r = m.restorer().unwrap();
    assert_eq!(r.restore("Hi [EMAIL_1]!"), "Hi ada@example.com!");
    assert_eq!(r.longest(), "[EMAIL_1]".len());
    let mut s = r.stream();
    let mut out = String::new();
    for piece in ["Write to [EM", "AIL_", "1] or [", "x] today"] {
        out.push_str(&s.push(piece));
    }
    out.push_str(&s.finish());
    assert_eq!(out, "Write to ada@example.com or [x] today");
    let mut s = r.stream();
    assert_eq!(s.push("a [EMA"), "a ");
    assert_eq!(s.finish(), "a [EMA".get(2..).unwrap());
}

#[test]
fn logged_text_is_masked_also_where_json_escapes_the_value() {
    let pairs = vec![
        ("ada@example.com".to_owned(), "[EMAIL_1]".to_owned()),
        ("line\none".to_owned(), "[SECRET_1]".to_owned()),
    ];
    assert_eq!(
        mask_text(r#"{"a":"ada@example.com","b":"line\none"}"#, &pairs),
        r#"{"a":"[EMAIL_1]","b":"[SECRET_1]"}"#
    );
}

#[test]
fn checks_hold_for_their_edge_cases() {
    assert!(!luhn("12"));
    assert!(!iban("DE89!70400440532013000"));
    assert!(!iban("DE89"));
    assert!(passes(Check::Ipv6, "::1"));
    assert!(!passes(Check::Ipv6, "1:2"));
    assert_eq!(
        Detector::ALL.map(Detector::as_str),
        ["secrets", "email", "phone", "iban", "card", "ip"]
    );
    assert_eq!(
        [RuleAction::Mask, RuleAction::Block, RuleAction::Flag].map(RuleAction::as_str),
        ["mask", "block", "flag"]
    );
}
