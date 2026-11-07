use std::fmt::Write;

use super::*;
use crate::rights::Right;

#[test]
fn there_are_23_kinds_grouped_by_area_in_page_order() {
    assert_eq!(Kind::ALL.len(), 23);
    let areas: Vec<Area> = Kind::ALL.iter().map(|k| k.area()).collect();
    let mut sorted = areas.clone();
    sorted.sort();
    assert_eq!(
        areas, sorted,
        "kinds are listed in the order of their areas"
    );
    for area in Area::ALL {
        assert!(areas.contains(area), "{area:?} has no kind");
    }
}

#[test]
fn keys_parse_back_and_descriptions_are_sentences() {
    for k in Kind::ALL {
        assert_eq!(k.as_str().parse::<Kind>(), Ok(*k));
        assert!(
            k.as_str()
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '_'),
            "{k}"
        );
        assert!(
            k.description().ends_with('.') && k.description().len() <= 100,
            "{k}"
        );
    }
    assert!("nothing".parse::<Kind>().is_err());
    for d in Delivery::ALL {
        assert_eq!(d.as_str().parse::<Delivery>(), Ok(*d));
    }
    assert!("weekly".parse::<Delivery>().is_err());
}

#[test]
fn security_kinds_are_always_sent_now() {
    for k in Kind::ALL {
        if k.area() == Area::Security {
            assert!(k.always(), "{k}");
        }
        if k.always() {
            assert_eq!(k.default_delivery(), Delivery::Now, "{k}");
        }
    }
}

#[test]
fn scheduled_kinds_are_on_or_off_and_the_rest_take_the_digest() {
    let scheduled: Vec<&str> = Kind::ALL
        .iter()
        .filter(|k| k.scheduled())
        .map(|k| k.as_str())
        .collect();
    assert_eq!(scheduled, ["weekly_summary", "cost_report"]);
    for k in Kind::ALL {
        assert!(!(k.scheduled() && k.always()), "{k}");
        assert!(
            k.deliveries().contains(&k.default_delivery()),
            "{k}'s default is one of its choices"
        );
        if k.scheduled() {
            assert_eq!(k.deliveries(), [Delivery::Now, Delivery::Off], "{k}");
            assert!(!k.allows(Delivery::Daily), "{k}");
            assert_eq!(k.chosen(Some(Delivery::Daily)), Delivery::Now, "{k}");
        } else if k.always() {
            assert!(Delivery::ALL.iter().all(|d| !k.allows(*d)), "{k}");
            assert_eq!(k.chosen(Some(Delivery::Off)), Delivery::Now, "{k}");
        } else {
            assert_eq!(k.deliveries(), Delivery::ALL, "{k}");
            assert_eq!(k.chosen(Some(Delivery::Daily)), Delivery::Daily, "{k}");
        }
        assert_eq!(k.chosen(None), k.default_delivery(), "{k}");
    }
}

#[test]
fn only_kinds_about_yourself_name_no_right() {
    let about_you: Vec<&str> = Kind::ALL
        .iter()
        .filter(|k| k.rights().is_empty())
        .map(|k| k.as_str())
        .collect();
    assert_eq!(about_you, ["account_security", "new_app", "access_changed"]);
    for k in Kind::ALL {
        for r in k.rights() {
            assert!(
                !matches!(r, Right::Account(_)),
                "{k} names an account right"
            );
        }
    }
}

fn reference() -> String {
    let mut out = String::from(
        "| Kind | Area | Name | Default | Choices | Always | Rights | Description |\n| --- | --- | --- | --- | --- | --- | --- | --- |\n",
    );
    for k in Kind::ALL {
        let rights: Vec<String> = k.rights().iter().map(|r| format!("`{r}`")).collect();
        let choices: Vec<String> = k.deliveries().iter().map(|d| d.to_string()).collect();
        let choices = if k.always() {
            String::new()
        } else {
            choices.join(", ")
        };
        let _ = writeln!(
            out,
            "| `{}` | {} | {} | {} | {} | {} | {} | {} |",
            k.as_str(),
            k.area().name(),
            k.name(),
            k.default_delivery(),
            choices,
            if k.always() { "✓" } else { "" },
            rights.join(", "),
            k.description()
        );
    }
    out
}

#[test]
fn the_catalogue_reference_is_current() {
    insta::assert_snapshot!("notifications_reference", reference());
}
