use std::collections::BTreeSet;

use kaeso_kpp::{
    Block, DialectId, DocumentBlock, EmbedInput, EmbedRequest, FunctionTool, GenerateRequest,
    JsonSchemaSpec, Message, OutputSpec, RawJson, ReasoningConfig, ReasoningEffort, ReasoningMode,
    Request, Role, Source, ToolChoice, ToolDef,
};
use rust_decimal_macros::dec;
use uuid::Uuid;

use super::capability::{effort_levels, model_info, prepare};
use super::resolve::{Access, AccessLevel, Alias, Catalogue, ResolveError, Resolved, Via};
use super::route::Step;
use super::*;
use crate::id::CatalogueModelId;
use crate::provider;

fn snapshot() -> kpmr::Catalogue {
    kpmr::embedded().unwrap()
}

fn entry(provider: &str, model: &str) -> Entry {
    Entry {
        id: Uuid::now_v7(),
        provider: provider.into(),
        model_id: model.into(),
        overrides: Fields::default(),
        custom: false,
        delisted: false,
        last_known: None,
        availability: None,
        availability_checked_at: None,
    }
}

fn eff(e: &Entry) -> Effective {
    effective(e, &snapshot(), &OrgProviders::default()).unwrap()
}

fn catalogue(providers: &str) -> Vec<u8> {
    format!(r#"{{"format":1,"provider_ids":[],"providers":{providers}}}"#).into_bytes()
}

fn feed(pricing: &str) -> Vec<u8> {
    catalogue(&format!(
        r#"{{"openai":{{"id":"openai","models":{{"gpt-4o":{{"id":"gpt-4o","pricing":{pricing},"limits":{{"context":1,"output":1}}}}}}}}}}"#
    ))
}

#[test]
fn a_price_a_feed_zeroes_leaves_the_model_unpriced_for_a_spend_budget() {
    let ids = crate::provider::known_ids();
    let parsed = kpmr::parse(
        &feed(r#"{"unit":"usd_per_million_tokens","input":"0","output":"0"}"#),
        &ids,
    )
    .unwrap();
    let e = entry("openai", "gpt-4o");
    let zeroed = effective(&e, &parsed.catalogue, &OrgProviders::default()).unwrap();
    assert_eq!(zeroed.fields.input_price, Some(dec!(0)));
    let refused = crate::cost::unpriced_model(
        &zeroed.fields,
        &e.overrides,
        kaeso_kpp::RequestKind::Generate,
        "gpt-4o",
    );
    assert_eq!(refused.map(|u| u.feature), Some("gpt-4o".to_owned()));
    for bad in [
        r#"{"unit":"usd_per_million_tokens","input":"-1"}"#,
        r#"{"unit":"usd_per_million_tokens","input":"1000001"}"#,
        r#"{"unit":"usd_per_million_tokens","input":"x"}"#,
        r#"{"unit":"usd_per_million_tokens","input":0.5}"#,
    ] {
        let p = kpmr::parse(&feed(bad), &ids).unwrap();
        assert_eq!(p.skipped, ["openai/gpt-4o"], "{bad}");
    }
}

#[test]
fn every_anthropic_model_in_the_shipped_catalogue_says_whether_it_takes_adaptive_thinking() {
    let snap = snapshot();
    for (id, model) in &snap.providers["anthropic"].models {
        assert!(
            Fields::from_kpmr(model).adaptive_thinking.is_some(),
            "anthropic/{id} has no reasoning.adaptive_thinking"
        );
    }
    let haiku = Fields::from_kpmr(snap.model("anthropic", "claude-haiku-5-5").unwrap());
    assert_eq!(
        (haiku.adaptive_thinking, haiku.thinking_binding),
        (Some(true), Some(true))
    );
    let fable = Fields::from_kpmr(snap.model("anthropic", "claude-fable-5-1").unwrap());
    assert_eq!(fable.forced_tool_choice, Some(false));
    let mini = Fields::from_kpmr(snap.model("openai", "gpt-5.4-mini").unwrap());
    assert_eq!(mini.egress_dialect, Some(DialectId::OpenAiResponses));
    let cohere = Fields::from_kpmr(snap.model("cohere", "north-mini-code-1-0").unwrap());
    assert_eq!(
        cohere.efforts.as_deref(),
        Some(&["none".to_owned(), "high".to_owned()][..])
    );
}

#[test]
fn from_kpmr_keeps_the_effort_levels_this_gateway_knows_and_derives_the_dialect_of_embeddings() {
    let ids = crate::provider::known_ids();
    let feed = catalogue(
        r#"{"openai":{"id":"openai","models":{
        "a":{"id":"a","reasoning":{"efforts":["low","galactic"],"adaptive_thinking":null,"thinking_binding":null},"capabilities":{"forced_tool_choice":false}},
        "b":{"id":"b","kind":"embedding","gateway":{"egress_dialect":null}},
        "c":{"id":"c","kind":"embedding","gateway":{"egress_dialect":"openai_responses"}}
    }}}"#,
    );
    let parsed = kpmr::parse(&feed, &ids).unwrap();
    assert!(parsed.skipped.is_empty(), "{:?}", parsed.skipped);
    let fields = |id: &str| Fields::from_kpmr(parsed.catalogue.model("openai", id).unwrap());
    let a = fields("a");
    assert_eq!(a.efforts.as_deref(), Some(&["low".to_owned()][..]));
    assert_eq!(a.forced_tool_choice, Some(false));
    assert_eq!(a.egress_dialect, None);
    assert_eq!(
        fields("b").egress_dialect,
        Some(DialectId::OpenAiEmbeddings)
    );
    assert_eq!(
        fields("c").egress_dialect,
        Some(DialectId::OpenAiResponses),
        "KPMR's dialect wins over the one Kaeso derives"
    );
}

#[test]
fn overrides_win_over_kpmrs_values() {
    let mut e = entry("anthropic", "claude-haiku-4-5");
    let base = eff(&e);
    assert_eq!(base.fields.input_price, Some(dec!(1)));
    assert_eq!(base.fields.output_limit, Some(64_000));
    assert_eq!(base.fields.adaptive_thinking, Some(false));
    assert_eq!(base.fields.efforts.as_deref(), Some(&[][..]));
    assert_eq!(base.dialect, DialectId::AnthropicMessages);
    e.overrides.input_price = Some(dec!(0.8));
    e.overrides.adaptive_thinking = Some(true);
    let over = eff(&e);
    assert_eq!(over.fields.input_price, Some(dec!(0.8)));
    assert_eq!(over.fields.output_price, Some(dec!(5)));
    assert_eq!(over.fields.adaptive_thinking, Some(true));
    let opus = eff(&entry("anthropic", "claude-opus-4-6"));
    assert_eq!(
        opus.fields.efforts.unwrap(),
        ["low", "medium", "high", "max"]
    );
    assert_eq!(
        eff(&entry("openai", "text-embedding-3-small")).dialect,
        DialectId::OpenAiEmbeddings
    );
    assert_eq!(
        eff(&entry("openai", "gpt-4o")).dialect,
        DialectId::OpenAiResponses
    );
}

#[test]
fn the_one_hour_write_price_follows_an_override_then_an_overridden_input_price_then_kpmr() {
    let ids = crate::provider::known_ids();
    let feed = catalogue(
        r#"{"anthropic":{"id":"anthropic","models":{
        "listed":{"id":"listed","pricing":{"unit":"usd_per_million_tokens","input":"3","cache_write":"3.75","cache_write_1h":"6.5"}},
        "unlisted":{"id":"unlisted","pricing":{"unit":"usd_per_million_tokens","input":"3","cache_write":"3.75","cache_write_1h":null}}
    }}}"#,
    );
    let parsed = kpmr::parse(&feed, &ids).unwrap();
    let one_hour = |model: &str, overrides: Fields| {
        let mut e = entry("anthropic", model);
        e.overrides = overrides;
        let x = effective(&e, &parsed.catalogue, &OrgProviders::default()).unwrap();
        crate::cost::one_hour_cache_write_price(&x.fields)
    };
    assert_eq!(
        one_hour("listed", Fields::default()),
        Some(dec!(6.5)),
        "KPMR's price"
    );
    assert_eq!(
        one_hour(
            "listed",
            Fields {
                cache_write_1h_price: Some(dec!(9)),
                input_price: Some(dec!(1)),
                ..Fields::default()
            }
        ),
        Some(dec!(9)),
        "an override of the one-hour price wins"
    );
    assert_eq!(
        one_hour(
            "listed",
            Fields {
                input_price: Some(dec!(2)),
                ..Fields::default()
            }
        ),
        Some(dec!(4)),
        "an overridden input price keeps twice itself"
    );
    assert_eq!(
        one_hour("unlisted", Fields::default()),
        Some(dec!(6)),
        "twice the input price"
    );
    assert_eq!(one_hour("absent", Fields::default()), None);
}

fn tiered_feed() -> kpmr::Catalogue {
    let feed = catalogue(
        r#"{"openai":{"id":"openai","models":{
        "astra":{"id":"astra","pricing":{"unit":"usd_per_million_tokens","input":"10","output":"50","cache_read":"1","cache_write":"12.5",
            "tiers":[{"context_above":272000,"input":"20","output":"75","cache_read":"2","cache_write":"25"}]}}
    }}}"#,
    );
    kpmr::parse(&feed, &crate::provider::known_ids())
        .unwrap()
        .catalogue
}

fn astra_tier() -> PriceTier {
    PriceTier {
        context_above: 272_000,
        input_price: Some(dec!(20)),
        output_price: Some(dec!(75)),
        cache_read_price: Some(dec!(2)),
        cache_write_price: Some(dec!(25)),
        cache_write_1h_price: None,
    }
}

#[test]
fn kpmrs_long_context_prices_stay_until_an_override_sets_a_price_without_them() {
    let snap = tiered_feed();
    let tiers = |overrides: Fields| {
        let mut e = entry("openai", "astra");
        e.overrides = overrides;
        effective(&e, &snap, &OrgProviders::default())
            .unwrap()
            .fields
            .price_tiers
    };
    assert_eq!(tiers(Fields::default()), Some(vec![astra_tier()]));
    assert_eq!(
        tiers(Fields {
            display_name: Some("Astra".into()),
            context_limit: Some(400_000),
            ..Fields::default()
        }),
        Some(vec![astra_tier()]),
        "fields other than prices keep them"
    );
    for overrides in [
        Fields {
            input_price: Some(dec!(8)),
            ..Fields::default()
        },
        Fields {
            output_price: Some(dec!(40)),
            ..Fields::default()
        },
        Fields {
            cache_read_price: Some(dec!(0.5)),
            ..Fields::default()
        },
        Fields {
            cache_write_price: Some(dec!(10)),
            ..Fields::default()
        },
    ] {
        assert_eq!(tiers(overrides.clone()), None, "{overrides:?}");
    }
    let own = vec![PriceTier {
        context_above: 128_000,
        input_price: Some(dec!(9)),
        ..PriceTier::default()
    }];
    assert_eq!(
        tiers(Fields {
            input_price: Some(dec!(8)),
            price_tiers: Some(own.clone()),
            ..Fields::default()
        }),
        Some(own),
        "tiers set with the prices are kept"
    );
    assert_eq!(
        tiers(Fields {
            price_tiers: Some(Vec::new()),
            ..Fields::default()
        }),
        Some(Vec::new()),
        "an empty list turns KPMR's tiers off"
    );
    let one_hour = tiers(Fields {
        cache_write_1h_price: Some(dec!(30)),
        ..Fields::default()
    });
    assert_eq!(
        one_hour,
        Some(vec![PriceTier {
            cache_write_1h_price: Some(dec!(30)),
            ..astra_tier()
        }]),
        "a one-hour price set by hand holds for long prompts too"
    );
    assert_eq!(
        preview("openai", "astra", &snap).unwrap().price_tiers,
        Some(vec![astra_tier()])
    );
}

#[test]
fn a_custom_providers_model_has_only_the_tiers_set_by_hand() {
    let org = OrgProviders::new(vec![custom_provider("openai", DialectId::OpenAiChat)]);
    let mut e = entry("openai", "astra");
    e.custom = true;
    e.overrides = Fields {
        input_price: Some(dec!(1)),
        output_price: Some(dec!(2)),
        ..Fields::default()
    };
    let fields = |e: &Entry| effective(e, &tiered_feed(), &org).unwrap().fields;
    assert_eq!(fields(&e).price_tiers, None, "KPMR's tiers never reach it");
    e.overrides.price_tiers = Some(vec![PriceTier {
        context_above: 32_000,
        input_price: Some(dec!(3)),
        ..PriceTier::default()
    }]);
    e.overrides.cache_write_1h_price = Some(dec!(2.5));
    assert_eq!(
        fields(&e).price_tiers,
        Some(vec![PriceTier {
            context_above: 32_000,
            input_price: Some(dec!(3)),
            cache_write_1h_price: Some(dec!(2.5)),
            ..PriceTier::default()
        }])
    );
}

#[test]
fn delisted_models_keep_their_last_known_values() {
    let snap = snapshot();
    let mut e = entry("openai", "gone-model");
    let mut last = snap.model("openai", "gpt-4o").unwrap().clone();
    last.id = "gone-model".into();
    e.last_known = Some(last);
    e.delisted = true;
    let x = effective(&e, &snap, &OrgProviders::default()).unwrap();
    assert_eq!(x.fields.input_price, Some(dec!(2.5)));
    let bare = effective(&entry("openai", "unknown"), &snap, &OrgProviders::default()).unwrap();
    assert_eq!(bare.fields, Fields::default());
}

#[test]
fn kpmr_marks_entries_deprecated_also_after_delisting() {
    let snap = snapshot();
    assert!(entry("openai", "gpt-4").is_deprecated(&snap));
    assert!(!entry("anthropic", "claude-sonnet-5-5").is_deprecated(&snap));
    assert!(!entry("openai", "unknown").is_deprecated(&snap));
    let mut gone = entry("openai", "gone-model");
    let mut last = snap.model("openai", "gpt-4").unwrap().clone();
    last.id = "gone-model".into();
    gone.last_known = Some(last);
    gone.delisted = true;
    assert!(gone.is_deprecated(&snap));
}

#[test]
fn availability_reads_back_what_it_writes() {
    for a in [Availability::Available, Availability::Unavailable] {
        assert_eq!(Availability::parse(a.as_str()), Some(a));
    }
    assert_eq!(Availability::parse("unchecked"), None);
    let mut e = entry("anthropic", "claude-sonnet-4-5");
    assert!(!e.is_unavailable());
    e.availability = Some(Availability::Unavailable);
    assert!(e.is_unavailable());
    let cached = serde_json::json!({
        "id": e.id, "provider": "anthropic", "model_id": "claude-sonnet-4-5",
        "overrides": {}, "custom": false, "delisted": false, "last_known": null
    });
    let old: Entry = serde_json::from_value(cached).unwrap();
    assert_eq!(old.availability, None);
}

#[test]
fn patches_set_and_remove_overrides() {
    let mut f = Fields {
        input_price: Some(dec!(1)),
        ..Fields::default()
    };
    let patch =
        serde_json::json!({ "input_price": null, "output_price": "2.50", "efforts": ["low"] });
    f.patch(patch.as_object().unwrap()).unwrap();
    assert_eq!(f.input_price, None);
    assert_eq!(f.output_price, Some(dec!(2.5)));
    assert_eq!(f.set_names(), ["output_price", "efforts"]);
    let bad = serde_json::json!({ "colour": "red" });
    assert_eq!(
        f.patch(bad.as_object().unwrap()).unwrap_err().field,
        "colour"
    );
    let wrong = serde_json::json!({ "context_limit": "big" });
    assert_eq!(
        f.patch(wrong.as_object().unwrap()).unwrap_err().field,
        "context_limit"
    );
    for refused in ["+5", "5.", "1_000", " 5", "free"] {
        let price = serde_json::json!({ "efforts": ["low"], "cache_read_price": refused });
        let error = f.patch(price.as_object().unwrap()).unwrap_err();
        assert_eq!(error.field, "cache_read_price", "{refused:?}");
        assert_eq!(
            error.message,
            "`cache_read_price` has a value of the wrong type or form."
        );
    }
    assert_eq!(serde_json::to_value(&f).unwrap()["output_price"], "2.5");
}

#[test]
fn long_context_prices_are_set_by_hand_and_checked() {
    let openai = &ProviderRef::BuiltIn(provider::provider("openai").unwrap());
    let mut f = Fields::default();
    let patch = serde_json::json!({ "price_tiers": [
        { "context_above": 128000, "input_price": "4", "output_price": 16 },
        { "context_above": 256000, "input_price": "6", "cache_write_1h_price": null }
    ]});
    f.patch(patch.as_object().unwrap()).unwrap();
    assert_eq!(f.validate(openai), Ok(()));
    assert_eq!(f.set_names(), ["price_tiers"]);
    let tiers = f.price_tiers.clone().unwrap();
    assert_eq!(tiers[0].output_price, Some(dec!(16)));
    assert_eq!(tiers[1].input_price, Some(dec!(6)));
    assert_eq!(
        serde_json::to_value(&f).unwrap()["price_tiers"],
        serde_json::json!([
            { "context_above": 128000, "input_price": "4", "output_price": "16" },
            { "context_above": 256000, "input_price": "6" }
        ])
    );
    for wrong in [
        serde_json::json!({ "price_tiers": [{ "input_price": "4" }] }),
        serde_json::json!({ "price_tiers": [{ "context_above": 1, "input": "4" }] }),
        serde_json::json!({ "price_tiers": [{ "context_above": 1, "input_price": 0.5 }] }),
        serde_json::json!({ "price_tiers": { "context_above": 1 } }),
    ] {
        let error = f.patch(wrong.as_object().unwrap()).unwrap_err();
        assert_eq!(error.field, "price_tiers", "{wrong}");
        assert_eq!(
            error.message,
            "`price_tiers` has a value of the wrong type or form."
        );
    }
    let tier = |context_above: u64, input: Decimal| PriceTier {
        context_above,
        input_price: Some(input),
        ..PriceTier::default()
    };
    let check = |tiers: Vec<PriceTier>| {
        Fields {
            price_tiers: Some(tiers),
            ..Fields::default()
        }
        .validate(openai)
    };
    let order = "`price_tiers` takes up to 16 tiers, their `context_above` positive and rising.";
    for refused in [
        vec![tier(256_000, dec!(6)), tier(128_000, dec!(4))],
        vec![tier(128_000, dec!(4)), tier(128_000, dec!(6))],
        vec![tier(0, dec!(4))],
        (1..=17).map(|n| tier(n * 1000, dec!(1))).collect(),
    ] {
        let error = check(refused).unwrap_err();
        assert_eq!(
            (error.field.as_str(), error.message.as_str()),
            ("price_tiers", order)
        );
    }
    assert_eq!(
        check((1..=16).map(|n| tier(n * 1000, dec!(1))).collect()),
        Ok(())
    );
    assert_eq!(check(Vec::new()), Ok(()));
    let error = check(vec![tier(1000, dec!(0.0000000001))]).unwrap_err();
    assert_eq!(
        error.message,
        "`price_tiers` must be 0 to 1,000,000 USD per million tokens, at most 9 decimals."
    );
}

#[test]
fn values_are_checked() {
    let openai = &ProviderRef::BuiltIn(provider::provider("openai").unwrap());
    let anthropic = &ProviderRef::BuiltIn(provider::provider("anthropic").unwrap());
    let ok = Fields {
        input_modalities: Some(vec!["text".into(), "image".into()]),
        efforts: Some(vec!["low".into()]),
        egress_dialect: Some(DialectId::OpenAiChat),
        ..Fields::default()
    };
    assert_eq!(ok.validate(openai), Ok(()));
    let field = |f: Fields, p| f.validate(p).unwrap_err().field;
    assert_eq!(
        field(
            Fields {
                egress_dialect: Some(DialectId::OpenAiChat),
                ..Fields::default()
            },
            anthropic
        ),
        "egress_dialect"
    );
    assert_eq!(
        field(
            Fields {
                input_modalities: Some(vec!["smell".into()]),
                ..Fields::default()
            },
            openai
        ),
        "input_modalities"
    );
    assert_eq!(
        field(
            Fields {
                efforts: Some(vec!["low".into(), "low".into()]),
                ..Fields::default()
            },
            openai
        ),
        "efforts"
    );
    assert_eq!(
        field(
            Fields {
                adaptive_thinking: Some(true),
                ..Fields::default()
            },
            openai
        ),
        "adaptive_thinking"
    );
    assert_eq!(
        field(
            Fields {
                input_price: Some(dec!(0.0000000001)),
                ..Fields::default()
            },
            openai
        ),
        "input_price"
    );
    assert_eq!(
        field(
            Fields {
                release_date: Some("soon".into()),
                ..Fields::default()
            },
            openai
        ),
        "release_date"
    );
    let err = Fields::default().validate_custom().unwrap_err();
    assert_eq!(err.field, "input_modalities");
    assert!(err.message.contains("egress_dialect"));
    let custom = Fields {
        input_modalities: Some(vec!["text".into()]),
        output_modalities: Some(vec!["text".into()]),
        context_limit: Some(8000),
        output_limit: Some(1000),
        input_price: Some(dec!(1)),
        output_price: Some(dec!(2)),
        egress_dialect: Some(DialectId::OpenAiChat),
        ..Fields::default()
    };
    assert_eq!(custom.validate_custom(), Ok(()));
}

fn alias(name: impl Into<String>, target: Uuid) -> Alias {
    Alias {
        id: target,
        name: name.into(),
        route: Step::model(CatalogueModelId::from_uuid(target)),
    }
}

fn target(catalogue: &Catalogue, name: &str) -> Result<(Uuid, Option<Via>), ResolveError> {
    catalogue.resolve(name).map(|r| match r {
        Resolved::Model(e, v) => (e.id, Some(v)),
        Resolved::Alias(a) => (a.route.primary().map_or(Uuid::nil(), |m| m.uuid()), None),
    })
}

#[test]
fn names_resolve_alias_then_prefixed_then_native() {
    let gpt = entry("openai", "gpt-4o");
    let claude = entry("anthropic", "claude-sonnet-4-5");
    let entries = vec![gpt.clone(), claude.clone()];
    let aliases = vec![alias("chat-default", claude.id), alias("gpt-4o", claude.id)];
    let catalogue = Catalogue::new(entries.clone(), aliases);
    let r = |n: &str| target(&catalogue, n);
    assert_eq!(r("chat-default"), Ok((claude.id, None)));
    assert_eq!(r("gpt-4o"), Ok((claude.id, None)));
    assert_eq!(r("openai/gpt-4o"), Ok((gpt.id, Some(Via::Prefixed))));
    assert_eq!(r("claude-sonnet-4-5"), Ok((claude.id, Some(Via::Native))));
    assert!(matches!(r("gpt-5"), Err(ResolveError::NotInCatalogue(_))));
    assert!(r("anthropic/claude-opus-4-6").is_err());
    let twins = Catalogue::new(
        vec![entry("openai", "same"), entry("anthropic", "same")],
        vec![],
    );
    let err = twins.resolve("same").unwrap_err();
    assert!(err.to_string().contains("openai/same"), "{err}");
    assert_eq!(twins.sharing("same"), 2);
    assert!(catalogue.shadows("gpt-4o", &snapshot()));
    assert!(catalogue.shadows("openai/gpt-4o", &snapshot()));
    assert!(Catalogue::default().shadows("gpt-5", &snapshot()));
    assert!(!catalogue.shadows("chat-default", &snapshot()));
    assert_eq!(
        catalogue
            .aliases_of(claude.id)
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>(),
        ["chat-default", "gpt-4o"]
    );
    assert_eq!(catalogue.aliases_of(gpt.id).count(), 0);
    assert_eq!(catalogue.entry(gpt.id), Some(&gpt));
    assert!(super::resolve::validate_alias_name("a/b").is_err());
    assert!(super::resolve::validate_alias_name("fast chat").is_err());
    assert!(super::resolve::validate_alias_name("gpt-4o\u{200B}").is_err());
    assert!(super::resolve::validate_alias_name("chat\u{202E}tset").is_err());
}

#[test]
fn model_ids_with_a_slash_resolve_prefixed_and_native() {
    let oss = entry("groq", "openai/gpt-oss-20b");
    let qwen = entry("groq", "qwen/qwen3.8-27b");
    let entries = vec![oss.clone(), qwen.clone()];
    let catalogue_entries = Catalogue::new(entries.clone(), vec![]);
    let r = |n: &str| catalogue_entries.resolve_model(n).map(|(e, v)| (e.id, v));
    assert_eq!(r("groq/openai/gpt-oss-20b"), Ok((oss.id, Via::Prefixed)));
    assert_eq!(r("openai/gpt-oss-20b"), Ok((oss.id, Via::Native)));
    assert_eq!(r("qwen/qwen3.8-27b"), Ok((qwen.id, Via::Native)));
    assert!(matches!(
        r("groq/gpt-oss-20b"),
        Err(ResolveError::NotInCatalogue(_))
    ));
    let gpt = entry("openai", "gpt-oss-20b");
    let both = vec![oss.clone(), gpt.clone()];
    let catalogue_both = Catalogue::new(both.clone(), vec![]);
    let r = |n: &str| catalogue_both.resolve_model(n).map(|(e, v)| (e.id, v));
    assert_eq!(r("openai/gpt-oss-20b"), Ok((gpt.id, Via::Prefixed)));
    assert_eq!(r("groq/openai/gpt-oss-20b"), Ok((oss.id, Via::Prefixed)));
}

#[test]
fn model_ids_with_slashes_resolve_natively_and_with_their_provider_prefix() {
    let qwen = entry("togetherai", "Qwen/Qwen3.5-9B");
    let oss = entry("togetherai", "openai/gpt-oss-120b");
    let entries = vec![qwen.clone(), oss.clone(), entry("openai", "gpt-4o")];
    let catalogue_entries = Catalogue::new(entries.clone(), vec![]);
    let r = |n: &str| catalogue_entries.resolve_model(n).map(|(e, v)| (e.id, v));
    assert_eq!(r("Qwen/Qwen3.5-9B"), Ok((qwen.id, Via::Native)));
    assert_eq!(
        r("togetherai/Qwen/Qwen3.5-9B"),
        Ok((qwen.id, Via::Prefixed))
    );
    assert_eq!(r("openai/gpt-oss-120b"), Ok((oss.id, Via::Native)));
    assert_eq!(
        r("togetherai/openai/gpt-oss-120b"),
        Ok((oss.id, Via::Prefixed))
    );
    assert!(matches!(
        r("togetherai/Qwen3.5-9B"),
        Err(ResolveError::NotInCatalogue(_))
    ));
}

#[test]
fn model_ids_with_slashes_and_colons_resolve_natively_or_with_their_provider_prefix() {
    let free = entry("openrouter", "nvidia/nemotron-3.5-lightning:free");
    let sonnet = entry("openrouter", "anthropic/claude-sonnet-4.5");
    let entries = vec![free.clone(), sonnet.clone()];
    let catalogue_entries = Catalogue::new(entries.clone(), vec![]);
    let r = |n: &str| catalogue_entries.resolve_model(n).map(|(e, v)| (e.id, v));
    assert_eq!(
        r("nvidia/nemotron-3.5-lightning:free"),
        Ok((free.id, Via::Native))
    );
    assert_eq!(
        r("openrouter/nvidia/nemotron-3.5-lightning:free"),
        Ok((free.id, Via::Prefixed))
    );
    assert_eq!(
        r("anthropic/claude-sonnet-4.5"),
        Ok((sonnet.id, Via::Native))
    );
    assert!(matches!(
        r("nvidia/nemotron-3.5-lightning"),
        Err(ResolveError::NotInCatalogue(_))
    ));
}

#[test]
fn a_provider_prefix_wins_over_a_native_id_that_reads_the_same() {
    let routed = entry("openrouter", "openai/gpt-4o");
    let direct = entry("openai", "gpt-4o");
    let both = vec![routed.clone(), direct.clone()];
    let catalogue_both = Catalogue::new(both.clone(), vec![]);
    let r = |n: &str| catalogue_both.resolve_model(n).map(|(e, v)| (e.id, v));
    assert_eq!(r("openai/gpt-4o"), Ok((direct.id, Via::Prefixed)));
    assert_eq!(
        r("openrouter/openai/gpt-4o"),
        Ok((routed.id, Via::Prefixed))
    );
    assert_eq!(r("gpt-4o"), Ok((direct.id, Via::Native)));
    let only = vec![routed.clone()];
    let catalogue_only = Catalogue::new(only.clone(), vec![]);
    let r = |n: &str| catalogue_only.resolve_model(n).map(|(e, v)| (e.id, v));
    assert_eq!(r("openai/gpt-4o"), Ok((routed.id, Via::Native)));
}

#[test]
fn every_name_form_of_a_model_a_key_may_not_use_resolves_to_it_or_to_nothing() {
    let allowed = entry("openai", "gpt-4o-mini");
    let forbidden = entry("openai", "gpt-4o");
    let routed = entry("openrouter", "openai/gpt-4o");
    let cat = Catalogue::new(
        vec![allowed.clone(), forbidden.clone(), routed.clone()],
        vec![
            alias("cheap", allowed.id),
            alias("gpt-4o-mini-x", forbidden.id),
        ],
    );
    let key: BTreeSet<Uuid> = [allowed.id].into();
    let access = Access {
        member: None,
        key: Some(&key),
    };
    for name in [
        "gpt-4o",
        "openai/gpt-4o",
        "openrouter/openai/gpt-4o",
        "gpt-4o-mini-x",
        "GPT-4O",
        "openai/GPT-4o",
        "gpt\u{2010}4o",
        "gpt-4o\u{200b}",
        " gpt-4o",
        "openai//gpt-4o",
        "openai/gpt-4o/",
    ] {
        if let Ok((e, _)) = target(&cat, name) {
            assert_ne!(e, allowed.id, "{name} resolved to the allowed model");
            assert_eq!(access.check(e), Err(AccessLevel::Key), "{name} passed");
        }
    }
    assert!(access.allows(target(&cat, "cheap").unwrap().0));
}

#[test]
fn access_lists_narrow_level_by_level() {
    let (a, b, c) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
    let all = Access::default();
    assert!(all.allows(a));
    let member: BTreeSet<Uuid> = [a, b].into();
    let key: BTreeSet<Uuid> = [b].into();
    let narrowed = Access {
        member: Some(&member),
        key: Some(&key),
    };
    assert_eq!(narrowed.check(a), Err(AccessLevel::Key));
    assert_eq!(narrowed.check(b), Ok(()));
    assert_eq!(narrowed.check(c), Err(AccessLevel::Member));
    assert_eq!(super::resolve::check_subset(&key, Some(&member)), Ok(()));
    assert_eq!(
        super::resolve::check_subset(&[c].into(), Some(&member)),
        Err(c)
    );
    assert_eq!(super::resolve::check_subset(&[c].into(), None), Ok(()));
}

fn user(blocks: Vec<Block>) -> GenerateRequest {
    GenerateRequest::new("m").with_message(Message::new(Role::User, blocks))
}

fn with_effort(effort: &str) -> GenerateRequest {
    let mut g = user(vec![Block::text("hi")]);
    let mut r = ReasoningConfig::default();
    r.effort = Some(ReasoningEffort::from_wire(effort));
    g.reasoning = Some(r);
    g
}

fn check(g: GenerateRequest, ingress: DialectId, e: &Effective) -> Result<Request, String> {
    let mut req = Request::Generate(g);
    prepare(&mut req, ingress, e)
        .map(|_| req)
        .map_err(|u| u.message)
}

#[test]
fn capabilities_are_checked_before_sending() {
    let gpt35 = eff(&entry("openai", "gpt-3.5-turbo"));
    let gpt4o = eff(&entry("openai", "gpt-4o"));
    let image = user(vec![Block::image(
        Source::base64("image/png", "AAAA").unwrap(),
    )]);
    let err = check(image.clone(), DialectId::OpenAiChat, &gpt35).unwrap_err();
    assert!(
        err.contains("gpt-3.5-turbo") && err.contains("images"),
        "{err}"
    );
    assert!(check(image, DialectId::OpenAiChat, &gpt4o).is_ok());
    let pdf = user(vec![Block::Document(DocumentBlock::new(
        Source::base64("application/pdf", "AAAA").unwrap(),
    ))]);
    let mini = eff(&entry("openai", "gpt-5-mini"));
    assert!(
        check(pdf.clone(), DialectId::AnthropicMessages, &mini)
            .unwrap_err()
            .contains("PDF")
    );
    assert!(check(pdf, DialectId::AnthropicMessages, &gpt4o).is_ok());
    let tools =
        user(vec![Block::text("hi")]).with_tool(ToolDef::Function(FunctionTool::new("f", None)));
    assert!(
        check(tools, DialectId::OpenAiChat, &gpt35)
            .unwrap_err()
            .contains("tool calling")
    );
    let mut schema = user(vec![Block::text("hi")]);
    schema.output = OutputSpec::JsonSchema(JsonSchemaSpec::new(RawJson::empty_object()));
    assert!(
        check(schema, DialectId::OpenAiChat, &gpt35)
            .unwrap_err()
            .contains("structured output")
    );
    assert!(
        check(
            with_budget(2_048, ReasoningMode::Enabled),
            DialectId::AnthropicMessages,
            &gpt4o
        )
        .unwrap_err()
        .contains("reasoning")
    );
    let mut temp = user(vec![Block::text("hi")]);
    temp.sampling.temperature = Some(0.2);
    let gpt5 = eff(&entry("openai", "gpt-5"));
    let (req, names) = left_out(temp.clone(), DialectId::OpenAiChat, &gpt5);
    assert_eq!(names, ["temperature"]);
    let Request::Generate(g) = &req else { panic!() };
    assert_eq!(g.sampling.temperature, None);
    temp.sampling.temperature = Some(1.0);
    assert!(left_out(temp, DialectId::OpenAiChat, &gpt5).1.is_empty());
    let embed = eff(&entry("openai", "text-embedding-3-small"));
    assert!(
        check(user(vec![Block::text("x")]), DialectId::OpenAiChat, &embed)
            .unwrap_err()
            .contains("embeddings model")
    );
    let mut e = Request::Embed(EmbedRequest::new("m", vec![EmbedInput::Text("x".into())]));
    assert_eq!(
        prepare(&mut e, DialectId::OpenAiEmbeddings, &embed)
            .unwrap()
            .dialect,
        DialectId::OpenAiEmbeddings
    );
    assert!(prepare(&mut e, DialectId::OpenAiEmbeddings, &gpt4o).is_err());
}

#[test]
fn audio_and_video_need_a_model_that_takes_them() {
    let gpt4o = eff(&entry("openai", "gpt-4o"));
    let gemini = eff(&entry("google", "gemini-3.5-flash"));
    let minimax = eff(&entry("minimax", "MiniMax-M3"));
    let audio = user(vec![Block::audio(
        Source::base64("audio/wav", "UklGRg==").unwrap(),
    )]);
    let video = user(vec![Block::video(Source::Url(
        "https://www.youtube.com/watch?v=UTdfxFyOQTI".into(),
    ))]);
    let err = check(audio.clone(), DialectId::OpenAiChat, &gpt4o).unwrap_err();
    assert_eq!(err, "The model `gpt-4o` does not accept audio.");
    assert!(check(audio.clone(), DialectId::OpenAiChat, &gemini).is_ok());
    assert_eq!(
        check(audio, DialectId::Gemini, &minimax).unwrap_err(),
        "The model `MiniMax-M3` does not accept audio."
    );
    assert_eq!(
        check(video.clone(), DialectId::Gemini, &gpt4o).unwrap_err(),
        "The model `gpt-4o` does not accept video."
    );
    assert!(check(video.clone(), DialectId::Gemini, &minimax).is_ok());
    let mut req = Request::Generate(video);
    let refused = prepare(&mut req, DialectId::Gemini, &gpt4o).unwrap_err();
    assert_eq!(refused.capability, "video input");
    assert_eq!(refused.param, Some("contents"));
}

#[test]
fn audio_and_image_output_need_a_model_that_writes_them() {
    use kaeso_kpp::{AudioOutput, Modality};
    let tts = eff(&entry("google", "gemini-3.1-flash-tts-preview"));
    let flash = eff(&entry("google", "gemini-3.5-flash"));
    let image = eff(&entry("google", "gemini-3.1-flash-image"));
    let mut speech = user(vec![Block::text("Say hi.")]);
    speech.output_modalities = vec![Modality::Text, Modality::Audio];
    speech.audio_output = Some(AudioOutput::new(Some("Kore".into()), None));
    assert!(check(speech.clone(), DialectId::OpenAiChat, &tts).is_ok());
    assert_eq!(
        check(speech.clone(), DialectId::OpenAiChat, &flash).unwrap_err(),
        "The model `gemini-3.5-flash` does not produce audio."
    );
    let mut req = Request::Generate(speech);
    assert_eq!(
        prepare(&mut req, DialectId::Gemini, &flash)
            .unwrap_err()
            .param,
        Some("generationConfig.responseModalities")
    );
    assert_eq!(
        check(user(vec![Block::text("Hi")]), DialectId::OpenAiChat, &tts).unwrap_err(),
        "The model `gemini-3.1-flash-tts-preview` does not produce text."
    );
    let mut draw = user(vec![Block::text("Draw a circle.")]);
    draw.output_modalities = vec![Modality::Text, Modality::Image];
    assert!(check(draw.clone(), DialectId::Gemini, &image).is_ok());
    assert_eq!(
        check(draw, DialectId::Gemini, &flash).unwrap_err(),
        "The model `gemini-3.5-flash` does not produce images."
    );
}

fn effort_of(req: &Request) -> Option<&ReasoningEffort> {
    match req {
        Request::Generate(g) => g.reasoning.as_ref().and_then(|r| r.effort.as_ref()),
        _ => None,
    }
}

fn kept(effort: &str, ingress: DialectId, e: &Effective) {
    let req = check(with_effort(effort), ingress, e).unwrap();
    assert_eq!(
        effort_of(&req),
        Some(&ReasoningEffort::from_wire(effort)),
        "{effort}"
    );
}

fn left_out(g: GenerateRequest, ingress: DialectId, e: &Effective) -> (Request, Vec<String>) {
    let mut req = Request::Generate(g);
    let plan = prepare(&mut req, ingress, e).unwrap();
    (req, plan.left_out)
}

fn effort_left_out(effort: &str, ingress: DialectId, e: &Effective) -> Vec<String> {
    let (req, left_out) = left_out(with_effort(effort), ingress, e);
    assert_eq!(effort_of(&req), None, "{effort}");
    left_out
}

fn refused(effort: &str, ingress: DialectId, e: &Effective) -> String {
    let err = check(with_effort(effort), ingress, e).unwrap_err();
    assert!(err.contains(&format!("`{effort}` ")), "{effort}: {err}");
    err
}

fn nearest(
    effort: &str,
    ingress: DialectId,
    e: &Effective,
) -> (Option<ReasoningEffort>, Vec<String>) {
    let (level, _, adapted) = adapted(with_effort(effort), ingress, e);
    (level, adapted)
}

#[test]
fn an_effort_the_model_does_not_list_becomes_the_nearest_level_it_lists() {
    use DialectId::{AnthropicMessages as A, OpenAiChat as C, OpenAiResponses as R};
    let gpt5 = eff(&entry("openai", "gpt-5"));
    assert_eq!(
        nearest("xhigh", C, &gpt5),
        (
            Some(ReasoningEffort::High),
            vec!["reasoning_effort".to_owned()]
        )
    );
    kept("minimal", C, &gpt5);
    let haiku = eff(&entry("anthropic", "claude-haiku-4-5"));
    assert_eq!(
        effort_levels(&haiku, C),
        ["low", "medium", "high", "xhigh", "max"]
    );
    kept("medium", C, &haiku);
    kept("max", C, &haiku);
    assert_eq!(nearest("minimal", C, &haiku).0, Some(ReasoningEffort::Low));
    assert_eq!(effort_left_out("low", A, &haiku), ["output_config.effort"]);
    let sonnet46 = eff(&entry("anthropic", "claude-sonnet-4-6"));
    assert_eq!(
        nearest("xhigh", R, &sonnet46),
        (
            Some(ReasoningEffort::High),
            vec!["reasoning.effort".to_owned()]
        ),
        "the lower level on a tie"
    );
    kept("max", R, &sonnet46);
    kept("none", R, &sonnet46);
    refused("turbo", R, &sonnet46);
    let k3 = eff(&entry("moonshotai", "kimi-k3"));
    assert_eq!(
        nearest("medium", A, &k3),
        (
            Some(ReasoningEffort::Low),
            vec!["output_config.effort".to_owned()]
        )
    );
    let reasoning = eff(&entry("cohere", "command-a-reasoning-08-2025"));
    assert_eq!(effort_levels(&reasoning, C), ["none", "high"]);
    assert_eq!(
        nearest("low", C, &reasoning).0,
        Some(ReasoningEffort::High),
        "never `none` on a tie, which would turn reasoning off"
    );
    assert_eq!(
        nearest("medium", C, &reasoning).0,
        Some(ReasoningEffort::High)
    );
}

fn claude_code_system_effort(effort: &str) -> GenerateRequest {
    let mut g = with_effort(effort);
    let mut m = Message::new(Role::System, vec![Block::text("Stay brief.")]);
    m.extras = kaeso_kpp::Extras::new(kaeso_kpp::Origin::new(DialectId::AnthropicMessages));
    m.extras.fields.insert(
        "output_config".to_owned(),
        RawJson::from_serialize(&serde_json::json!({"effort": effort})).unwrap(),
    );
    g.messages.push(m);
    g
}

fn system_output_config(req: &Request) -> Option<serde_json::Value> {
    let Request::Generate(g) = req else { panic!() };
    g.messages[1]
        .extras
        .fields
        .get("output_config")
        .map(|v| v.to_value().unwrap())
}

#[test]
fn the_effort_of_a_mid_conversation_system_message_follows_the_requests() {
    use DialectId::AnthropicMessages as A;
    let flash = eff(&entry("alibaba", "qwen3.7-flash"));
    let (req, names) = left_out(claude_code_system_effort("high"), A, &flash);
    assert_eq!(
        names,
        ["output_config.effort", "messages[1].output_config.effort"]
    );
    assert_eq!(system_output_config(&req), None);

    let k3 = eff(&entry("moonshotai", "kimi-k3"));
    let mut req = Request::Generate(claude_code_system_effort("medium"));
    let plan = prepare(&mut req, A, &k3).unwrap();
    assert_eq!(plan.adapted, ["output_config.effort"]);
    assert_eq!(
        system_output_config(&req),
        Some(serde_json::json!({"effort": "low"}))
    );
    assert_eq!(effort_of(&req), Some(&ReasoningEffort::Low));

    let mut other = claude_code_system_effort("low");
    other.reasoning.as_mut().unwrap().effort = Some(ReasoningEffort::High);
    let mut req = Request::Generate(other);
    prepare(&mut req, A, &k3).unwrap();
    assert_eq!(
        system_output_config(&req),
        Some(serde_json::json!({"effort": "low"})),
        "a message effort that differs from the request's stays as sent"
    );
}

#[test]
fn adaptive_thinking_becomes_a_budget_on_a_claude_model_without_it() {
    use DialectId::AnthropicMessages as A;
    let haiku45 = eff(&entry("anthropic", "claude-haiku-4-5"));
    let mut req = Request::Generate(adaptive(Some("high")));
    let plan = prepare(&mut req, A, &haiku45).unwrap();
    assert_eq!(plan.adapted, ["thinking"]);
    assert_eq!(plan.left_out, ["output_config.effort"]);
    let Request::Generate(g) = &req else { panic!() };
    let r = g.reasoning.as_ref().unwrap();
    assert_eq!(
        (r.mode.clone(), r.effort.as_ref(), r.budget_tokens),
        (Some(ReasoningMode::Enabled), None, Some(24_576))
    );
    assert!(!r.extras.fields.contains_key("display"));

    let mut req = Request::Generate(adaptive(None));
    prepare(&mut req, A, &haiku45).unwrap();
    let Request::Generate(g) = &req else { panic!() };
    assert_eq!(g.reasoning.as_ref().unwrap().budget_tokens, Some(8_192));

    let mut req = Request::Generate(adaptive(Some("none")));
    prepare(&mut req, A, &haiku45).unwrap();
    let Request::Generate(g) = &req else { panic!() };
    assert_eq!(
        g.reasoning.as_ref().unwrap().mode,
        Some(ReasoningMode::Disabled)
    );

    let sonnet55 = eff(&entry("anthropic", "claude-sonnet-5-5"));
    let mut req = Request::Generate(adaptive(Some("high")));
    let plan = prepare(&mut req, A, &sonnet55).unwrap();
    assert!(
        plan.adapted.is_empty(),
        "a model with adaptive thinking keeps it"
    );
}

#[test]
fn an_effort_or_adaptive_thinking_a_model_cannot_take_is_left_out() {
    use DialectId::{AnthropicMessages as A, OpenAiResponses as R};
    let claude_code = || {
        let mut g = with_effort("high");
        let r = g.reasoning.as_mut().unwrap();
        r.mode = Some(ReasoningMode::Adaptive);
        r.extras.fields.insert(
            "display".to_owned(),
            RawJson::from_serialize("omitted").unwrap(),
        );
        g
    };
    let command_a = eff(&entry("cohere", "command-a-03-2025"));
    let (req, names) = left_out(claude_code(), A, &command_a);
    assert_eq!(names, ["output_config.effort", "thinking"]);
    let Request::Generate(g) = &req else { panic!() };
    assert_eq!(g.reasoning, None);

    let flash = eff(&entry("alibaba", "qwen3.7-flash"));
    let (req, names) = left_out(claude_code(), A, &flash);
    assert_eq!(names, ["output_config.effort"]);
    let Request::Generate(g) = &req else { panic!() };
    let r = g.reasoning.as_ref().unwrap();
    assert_eq!(
        (&r.mode, &r.effort),
        (&Some(ReasoningMode::Adaptive), &None)
    );

    let mut codex = with_effort("high");
    let r = codex.reasoning.as_mut().unwrap();
    r.summary = Some(kaeso_kpp::ReasoningSummary::Auto);
    r.include_encrypted = true;
    let gpt4o = eff(&entry("openai", "gpt-4o"));
    let (req, names) = left_out(codex, R, &gpt4o);
    assert_eq!(names, ["reasoning.effort"]);
    let Request::Generate(g) = &req else { panic!() };
    let r = g.reasoning.as_ref().unwrap();
    assert_eq!(r.summary, Some(kaeso_kpp::ReasoningSummary::Auto));
    assert!(r.include_encrypted);

    kept("none", R, &gpt4o);
    assert!(left_out(with_effort("none"), R, &gpt4o).1.is_empty());
    let (_, names) = left_out(user(vec![Block::text("hi")]), A, &command_a);
    assert!(names.is_empty());
    assert_eq!(
        check(with_budget(16_384, ReasoningMode::Enabled), A, &command_a).unwrap_err(),
        "The model `command-a-03-2025` does not support reasoning."
    );
}

#[test]
fn an_anthropic_end_user_id_longer_than_the_provider_takes_is_hashed() {
    use DialectId::{AnthropicMessages as A, OpenAiChat as C};
    let long = format!(
        r#"{{"device_id":"{}","account_uuid":"","session_id":"0193a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b"}}"#,
        "a".repeat(64)
    );
    assert_eq!(long.len(), 150);
    let sent = |user: &str, ingress: DialectId, e: &Effective| {
        let mut g = user_request();
        g.user = Some(user.to_owned());
        let mut req = Request::Generate(g);
        let plan = prepare(&mut req, ingress, e).unwrap();
        let Request::Generate(g) = req else { panic!() };
        (g.user.unwrap(), plan.adapted)
    };
    let gpt = eff(&entry("openai", "gpt-4o"));
    let (user, adapted) = sent(&long, A, &gpt);
    assert_eq!(adapted, ["metadata.user_id"]);
    assert_eq!(user.len(), 64);
    assert!(user.chars().all(|c| c.is_ascii_hexdigit()), "{user}");
    assert_eq!(sent(&long, A, &gpt).0, user, "the same id hashes the same");
    let fits = "u".repeat(128);
    assert_eq!(sent(&fits, A, &gpt), (fits.clone(), Vec::new()));
    assert_eq!(sent(&long, C, &gpt), (long.clone(), Vec::new()));
    let haiku = eff(&entry("anthropic", "claude-haiku-4-5"));
    assert_eq!(sent(&long, A, &haiku), (long.clone(), Vec::new()));
}

#[test]
fn an_output_limit_above_the_models_is_lowered_when_the_request_crosses_formats() {
    use DialectId::{AnthropicMessages as A, OpenAiChat as C, OpenAiResponses as R};
    let command_a = eff(&entry("cohere", "command-a-03-2025"));
    assert_eq!(command_a.fields.output_limit, Some(8000));
    let asked = |n: u32, ingress: DialectId| {
        let mut g = user_request();
        g.sampling.max_output_tokens = Some(n);
        let mut req = Request::Generate(g);
        let plan = prepare(&mut req, ingress, &command_a).unwrap();
        let Request::Generate(g) = req else { panic!() };
        (g.sampling.max_output_tokens, plan.adapted)
    };
    assert_eq!(
        asked(32_000, A),
        (Some(8000), vec!["max_tokens".to_owned()])
    );
    assert_eq!(
        asked(32_000, R),
        (Some(8000), vec!["max_output_tokens".to_owned()])
    );
    assert_eq!(asked(4_000, A), (Some(4000), Vec::new()));
    assert_eq!(asked(32_000, C), (Some(32_000), Vec::new()));
}

fn user_request() -> GenerateRequest {
    user(vec![Block::text("hi")])
}

fn with_budget(budget: u32, mode: ReasoningMode) -> GenerateRequest {
    let mut g = user(vec![Block::text("hi")]);
    let mut r = ReasoningConfig::default();
    r.mode = Some(mode);
    r.budget_tokens = Some(budget);
    g.reasoning = Some(r);
    g
}

#[test]
fn a_thinking_budget_that_becomes_an_effort_the_model_lacks_is_refused() {
    let small = eff(&entry("mistral", "mistral-small-latest"));
    let a = DialectId::AnthropicMessages;
    assert_eq!(
        check(with_budget(2_048, ReasoningMode::Enabled), a, &small).unwrap_err(),
        "A `budget_tokens` of 2048 means `low`; `mistral-small-latest` needs at least 16384."
    );
    let mut req = Request::Generate(with_budget(2_048, ReasoningMode::Enabled));
    assert_eq!(
        prepare(&mut req, a, &small).unwrap_err().param,
        Some("thinking.budget_tokens")
    );
    assert!(check(with_budget(16_384, ReasoningMode::Enabled), a, &small).is_ok());
    assert!(check(with_budget(2_048, ReasoningMode::Disabled), a, &small).is_ok());
    let gpt5 = eff(&entry("openai", "gpt-5"));
    assert!(check(with_budget(2_048, ReasoningMode::Enabled), a, &gpt5).is_ok());
    let haiku = eff(&entry("anthropic", "claude-haiku-4-5"));
    assert!(check(with_budget(2_048, ReasoningMode::Enabled), a, &haiku).is_ok());
    let with_efforts = |efforts: &[&str]| {
        let mut e = entry("openai", "gpt-5");
        e.overrides.efforts = Some(efforts.iter().map(|&l| l.to_owned()).collect());
        eff(&e)
    };
    assert_eq!(
        check(
            with_budget(20_000, ReasoningMode::Enabled),
            a,
            &with_efforts(&["low"])
        )
        .unwrap_err(),
        "A `budget_tokens` of 20000 means `high`; `gpt-5` needs less than 4096."
    );
    assert_eq!(
        check(
            with_budget(2_048, ReasoningMode::Enabled),
            a,
            &with_efforts(&["none"])
        )
        .unwrap_err(),
        "A `budget_tokens` of 2048 means `low`; `gpt-5` needs `thinking` off."
    );
    assert!(
        check(
            with_budget(8_000, ReasoningMode::Enabled),
            a,
            &with_efforts(&["low", "high"])
        )
        .unwrap_err()
        .ends_with("needs at least 16384.")
    );
}

#[test]
fn mistral_models_speak_chat_completions_and_mistral_embed_embeds() {
    let small = eff(&entry("mistral", "mistral-small-latest"));
    assert_eq!(small.dialect, DialectId::OpenAiChat);
    assert_eq!(small.provider.chat().provider, "Mistral");
    assert!(!small.is_embeddings());
    let embed = eff(&entry("mistral", "mistral-embed"));
    assert_eq!(embed.dialect, DialectId::OpenAiEmbeddings);
    assert!(embed.is_embeddings());
    assert_eq!(
        effort_levels(&small, DialectId::AnthropicMessages),
        ["none", "high"]
    );
    kept("high", DialectId::AnthropicMessages, &small);
    kept("none", DialectId::OpenAiResponses, &small);
    assert_eq!(
        nearest("medium", DialectId::AnthropicMessages, &small).0,
        Some(ReasoningEffort::High)
    );
}

#[test]
fn vercel_models_speak_chat_completions_and_the_ones_kpmr_marks_as_embeddings_embed() {
    let free = eff(&entry("vercel", "nvidia/nemotron-3.5-lightning-free"));
    assert_eq!(free.dialect, DialectId::OpenAiChat);
    assert_eq!(free.provider.chat().provider, "Vercel");
    assert_eq!(
        (free.fields.input_price, free.fields.context_limit),
        (Some(dec!(0)), Some(1_000_000))
    );
    let embed = eff(&entry("vercel", "google/gemini-embedding-001"));
    assert_eq!(embed.dialect, DialectId::OpenAiEmbeddings);
    assert!(embed.is_embeddings());
    let mut sent_as = entry("vercel", "nvidia/nemotron-3.5-lightning-free");
    sent_as.overrides.egress_dialect = Some(DialectId::AnthropicMessages);
    assert_eq!(eff(&sent_as).dialect, DialectId::AnthropicMessages);

    let ids = crate::provider::known_ids();
    let feed = catalogue(
        r#"{"anthropic":{"id":"anthropic","models":{"x-embed":{"id":"x-embed","kind":"embedding"}}},
        "openai":{"id":"openai","models":{"y-embed":{"id":"y-embed","kind":"embedding"}}}}"#,
    );
    let parsed = kpmr::parse(&feed, &ids).unwrap();
    let dialect = |provider: &str, model: &str| {
        effective(
            &entry(provider, model),
            &parsed.catalogue,
            &OrgProviders::default(),
        )
        .unwrap()
        .dialect
    };
    assert_eq!(dialect("openai", "y-embed"), DialectId::OpenAiEmbeddings);
    assert_eq!(dialect("anthropic", "x-embed"), DialectId::OpenAiEmbeddings);
}

#[test]
fn cohere_models_speak_chat_completions_and_think_at_none_or_high() {
    let command = eff(&entry("cohere", "command-a-03-2025"));
    assert_eq!(command.dialect, DialectId::OpenAiChat);
    assert_eq!(command.provider.chat().provider, "Cohere");
    assert!(!command.is_embeddings());
    assert_eq!(
        (
            command.fields.input_price,
            command.fields.output_price,
            command.fields.context_limit
        ),
        (Some(dec!(2.5)), Some(dec!(10)), Some(256_000))
    );
    let reasoning = eff(&entry("cohere", "command-a-reasoning-08-2025"));
    assert_eq!(
        effort_levels(&reasoning, DialectId::AnthropicMessages),
        ["none", "high"]
    );
    kept("high", DialectId::OpenAiChat, &reasoning);
    kept("none", DialectId::OpenAiResponses, &reasoning);
    assert_eq!(
        nearest("medium", DialectId::OpenAiChat, &reasoning).0,
        Some(ReasoningEffort::High)
    );
}

#[test]
fn kimi_k3_models_think_at_the_effort_levels_moonshot_documents() {
    for model in ["kimi-k3", "kimi-k3.1-pro", "kimi-k3.1-pro-highspeed"] {
        let k3 = eff(&entry("moonshotai", model));
        assert_eq!(k3.dialect, DialectId::OpenAiChat, "{model}");
        assert_eq!(k3.provider.chat().provider, "Moonshot");
        assert_eq!(
            effort_levels(&k3, DialectId::OpenAiChat),
            ["low", "high", "max"],
            "{model}"
        );
        kept("low", DialectId::AnthropicMessages, &k3);
        kept("max", DialectId::OpenAiResponses, &k3);
        assert_eq!(
            nearest("medium", DialectId::OpenAiChat, &k3).0,
            Some(ReasoningEffort::Low),
            "{model}"
        );
        assert_eq!(
            adapted(with_effort("none"), DialectId::OpenAiChat, &k3).0,
            Some(ReasoningEffort::Low)
        );
    }
    let k26 = eff(&entry("moonshotai", "kimi-k2.6"));
    kept("none", DialectId::OpenAiChat, &k26);
    assert_eq!(
        effort_left_out("high", DialectId::OpenAiChat, &k26),
        ["reasoning_effort"]
    );
}

#[test]
fn duties_fill_what_kpp_cannot_know() {
    let haiku = eff(&entry("anthropic", "claude-haiku-4-5"));
    let mut req = Request::Generate(user(vec![Block::text("hi")]));
    let plan = prepare(&mut req, DialectId::OpenAiChat, &haiku).unwrap();
    assert_eq!(plan.dialect, DialectId::AnthropicMessages);
    assert_eq!(plan.adaptive_thinking, Some(false));
    assert_eq!(plan.thinking_binding, None);
    for model in [
        "claude-fable-5-1",
        "claude-opus-5-5",
        "claude-sonnet-5-5",
        "claude-haiku-5-5",
    ] {
        let checks = eff(&entry("anthropic", model));
        let mut req = Request::Generate(user(vec![Block::text("hi")]));
        let plan = prepare(&mut req, DialectId::OpenAiChat, &checks).unwrap();
        assert_eq!(plan.thinking_binding, Some(true), "{model}");
    }
    let Request::Generate(g) = &req else { panic!() };
    assert_eq!(g.sampling.max_output_tokens, Some(64_000));
    let mut own = user(vec![Block::text("hi")]);
    own.sampling.max_output_tokens = Some(100);
    let mut req = Request::Generate(own);
    prepare(&mut req, DialectId::OpenAiChat, &haiku).unwrap();
    let Request::Generate(g) = &req else { panic!() };
    assert_eq!(g.sampling.max_output_tokens, Some(100));
    let gpt = eff(&entry("openai", "gpt-4o"));
    let mut req = Request::Generate(user(vec![Block::text("hi")]));
    let plan = prepare(&mut req, DialectId::AnthropicMessages, &gpt).unwrap();
    assert_eq!(
        plan,
        super::capability::Plan {
            dialect: DialectId::OpenAiResponses,
            adaptive_thinking: None,
            thinking_binding: None,
            reasoning: Some(false),
            left_out: Vec::new(),
            adapted: Vec::new(),
        }
    );
    let Request::Generate(g) = &req else { panic!() };
    assert_eq!(g.sampling.max_output_tokens, None);
    let mut chat_entry = entry("openai", "gpt-4o");
    chat_entry.overrides.egress_dialect = Some(DialectId::OpenAiChat);
    let mut req = Request::CountTokens(user(vec![Block::text("hi")]));
    assert_eq!(
        prepare(&mut req, DialectId::OpenAiChat, &eff(&chat_entry))
            .unwrap()
            .dialect,
        DialectId::OpenAiResponses
    );
    let info = model_info("claude-haiku-4-5", &haiku);
    assert_eq!(
        info.display_name.as_deref(),
        Some("Claude Haiku 4.5 (latest)")
    );
    assert_eq!(info.created_at, Some(1_760_486_400));
    assert_eq!(info.owned_by.as_deref(), Some("anthropic"));
    assert_eq!(info.max_output_tokens, Some(64_000));
}

#[test]
fn a_provider_without_token_counting_refuses_count_requests_and_still_generates() {
    let mut e = eff(&entry("openai", "gpt-4o"));
    e.provider = ProviderRef::BuiltIn(Box::leak(Box::new(provider::Provider {
        count_tokens: None,
        ..e.provider.built_in().unwrap().clone()
    })));
    for ingress in [
        DialectId::AnthropicMessages,
        DialectId::OpenAiChat,
        DialectId::OpenAiResponses,
    ] {
        let mut count = Request::CountTokens(user(vec![Block::text("hi")]));
        let u = prepare(&mut count, ingress, &e).unwrap_err();
        assert_eq!(u.capability, "token counting");
        assert_eq!(u.param, Some("model"));
        assert_eq!(u.message, "The model `gpt-4o` cannot count tokens.");
        let mut generate = Request::Generate(user(vec![Block::text("hi")]));
        assert!(prepare(&mut generate, ingress, &e).is_ok(), "{ingress}");
    }
}

fn refusal(
    g: GenerateRequest,
    ingress: DialectId,
    e: &Effective,
) -> super::capability::Unsupported {
    let mut req = Request::Generate(g);
    prepare(&mut req, ingress, e).unwrap_err()
}

fn with_choice(choice: ToolChoice) -> GenerateRequest {
    let mut g =
        user(vec![Block::text("hi")]).with_tool(ToolDef::Function(FunctionTool::new("f", None)));
    g.tool_choice = Some(choice);
    g
}

#[test]
fn forced_tool_choice_is_refused_in_the_apps_words_where_the_model_rejects_it() {
    use DialectId::{AnthropicMessages as A, OpenAiChat as C, OpenAiResponses as R};
    for model in ["claude-fable-5-1", "claude-opus-5-5", "claude-sonnet-5-5"] {
        let e = eff(&entry("anthropic", model));
        assert_eq!(e.fields.forced_tool_choice, Some(false), "{model}");
        let cases = [
            (C, ToolChoice::Required, "`tool_choice` `required`"),
            (
                C,
                ToolChoice::Tool("f".into()),
                "a named function in `tool_choice`",
            ),
            (R, ToolChoice::Required, "`tool_choice` `required`"),
            (
                R,
                ToolChoice::Tool("f".into()),
                "a named function in `tool_choice`",
            ),
            (A, ToolChoice::Required, "`tool_choice` `any`"),
            (A, ToolChoice::Tool("f".into()), "`tool_choice` `tool`"),
        ];
        for (ingress, choice, words) in cases {
            let u = refusal(with_choice(choice.clone()), ingress, &e);
            assert_eq!(u.capability, "forced tool choice");
            assert_eq!(u.param, Some("tool_choice"));
            assert_eq!(
                u.message,
                format!(
                    "The model `{model}` does not accept {words}; send `auto` and name the tool in the prompt."
                )
            );
            let mut count = Request::CountTokens(with_choice(choice));
            assert!(
                prepare(&mut count, ingress, &e).is_err(),
                "{model} {ingress}"
            );
        }
        for choice in [ToolChoice::Auto, ToolChoice::None] {
            for ingress in [A, C, R] {
                assert!(check(with_choice(choice.clone()), ingress, &e).is_ok());
            }
        }
    }
    for model in [
        "claude-fable-5",
        "claude-opus-5",
        "claude-sonnet-4-6",
        "claude-haiku-5-5",
    ] {
        let e = eff(&entry("anthropic", model));
        assert_eq!(e.fields.forced_tool_choice, None, "{model}");
        assert!(
            check(with_choice(ToolChoice::Required), C, &e).is_ok(),
            "{model}"
        );
    }
    let mut e = entry("anthropic", "claude-opus-5-5");
    e.overrides.forced_tool_choice = Some(true);
    assert!(check(with_choice(ToolChoice::Required), C, &eff(&e)).is_ok());
}

#[test]
fn capability_refusals_name_the_apps_own_field() {
    use DialectId::{AnthropicMessages as A, OpenAiChat as C, OpenAiResponses as R};
    let opus55 = eff(&entry("anthropic", "claude-opus-5-5"));
    for (ingress, param) in [
        (C, "reasoning_effort"),
        (R, "reasoning.effort"),
        (A, "output_config.effort"),
    ] {
        let u = refusal(with_effort("turbo"), ingress, &opus55);
        assert_eq!(u.param, Some(param), "{ingress}");
        assert_eq!(
            u.message,
            "The model `claude-opus-5-5` supports the effort levels `low`, `medium`, `high`, `xhigh`, `max`; `turbo` is not one of them."
        );
    }
    let gpt35 = eff(&entry("openai", "gpt-3.5-turbo"));
    let mut schema = user(vec![Block::text("hi")]);
    schema.output = OutputSpec::JsonSchema(JsonSchemaSpec::new(RawJson::empty_object()));
    for (ingress, param) in [
        (C, "response_format"),
        (R, "text.format"),
        (A, "output_config.format"),
    ] {
        assert_eq!(refusal(schema.clone(), ingress, &gpt35).param, Some(param));
    }
    let image = user(vec![Block::image(
        Source::base64("image/png", "AAAA").unwrap(),
    )]);
    for (ingress, param) in [(C, "messages"), (R, "input"), (A, "messages")] {
        assert_eq!(refusal(image.clone(), ingress, &gpt35).param, Some(param));
    }
    let gpt4o = eff(&entry("openai", "gpt-4o"));
    assert_eq!(
        refusal(with_budget(2_048, ReasoningMode::Enabled), A, &gpt4o).param,
        Some("thinking")
    );
}

fn adaptive(effort: Option<&str>) -> GenerateRequest {
    let mut g = user(vec![Block::text("hi")]);
    let mut r = ReasoningConfig::default();
    r.mode = Some(ReasoningMode::Adaptive);
    r.effort = effort.map(ReasoningEffort::from_wire);
    g.reasoning = Some(r);
    g
}

#[test]
fn every_claude_model_without_adaptive_thinking_gets_a_budget_instead() {
    for model in [
        "claude-opus-4-5",
        "claude-opus-4-5-20251101",
        "claude-haiku-4-5",
        "claude-sonnet-4-5",
    ] {
        let e = eff(&entry("anthropic", model));
        let mut req = Request::Generate(adaptive(None));
        let plan = prepare(&mut req, DialectId::AnthropicMessages, &e).unwrap();
        assert_eq!(plan.adapted, ["thinking"], "{model}");
        let Request::Generate(g) = &req else { panic!() };
        assert_eq!(
            g.reasoning.as_ref().unwrap().mode,
            Some(ReasoningMode::Enabled),
            "{model}"
        );
    }
    let opus45 = eff(&entry("anthropic", "claude-opus-4-5"));
    assert!(check(with_effort("high"), DialectId::AnthropicMessages, &opus45).is_ok());
    assert!(check(with_effort("high"), DialectId::OpenAiChat, &opus45).is_ok());
    let haiku45 = eff(&entry("anthropic", "claude-haiku-4-5"));
    assert_eq!(
        effort_left_out("low", DialectId::AnthropicMessages, &haiku45),
        ["output_config.effort"]
    );
    assert!(check(with_effort("low"), DialectId::OpenAiResponses, &haiku45).is_ok());
    let sonnet46 = eff(&entry("anthropic", "claude-sonnet-4-6"));
    assert!(
        check(
            adaptive(Some("high")),
            DialectId::AnthropicMessages,
            &sonnet46
        )
        .is_ok()
    );
}

#[derive(Debug, PartialEq, Eq)]
enum Found {
    Alias(String),
    Model(*const Entry, Via),
}

fn found(r: Result<Resolved<'_>, ResolveError>) -> Result<Found, ResolveError> {
    r.map(|r| match r {
        Resolved::Alias(a) => Found::Alias(a.name.clone()),
        Resolved::Model(e, v) => Found::Model(std::ptr::from_ref(e), v),
    })
}

fn linear_resolve(name: &str, entries: &[Entry], aliases: &[Alias]) -> Result<Found, ResolveError> {
    if let Some(alias) = aliases.iter().find(|a| a.name == name) {
        return Ok(Found::Alias(alias.name.clone()));
    }
    linear_model(name, entries).map(|(e, v)| Found::Model(std::ptr::from_ref(e), v))
}

fn linear_model<'a>(name: &str, entries: &'a [Entry]) -> Result<(&'a Entry, Via), ResolveError> {
    if let Some((provider, model)) = name.split_once('/')
        && let Some(entry) = entries
            .iter()
            .find(|e| e.provider == provider && e.model_id == model)
    {
        return Ok((entry, Via::Prefixed));
    }
    let native: Vec<&Entry> = entries.iter().filter(|e| e.model_id == name).collect();
    match native.as_slice() {
        [] => Err(ResolveError::NotInCatalogue(name.to_owned())),
        [one] => Ok((one, Via::Native)),
        many => Err(ResolveError::Ambiguous {
            name: name.to_owned(),
            providers: many.iter().map(|e| e.provider.clone()).collect(),
        }),
    }
}

fn linear_shadows(name: &str, entries: &[Entry], snapshot: &kpmr::Catalogue) -> bool {
    entries
        .iter()
        .any(|e| e.model_id == name || format!("{}/{}", e.provider, e.model_id) == name)
        || snapshot
            .providers
            .values()
            .any(|p| p.models.contains_key(name))
}

mod indexed {
    use proptest::prelude::*;

    use super::*;

    const PROVIDERS: &[&str] = &["openai", "anthropic", "deepseek", "a-b"];
    const MODELS: &[&str] = &["m1", "m2", "gpt-4o", "x/y", "m1/z", ""];
    const NAMES: &[&str] = &["fast", "m1", "gpt-4o", "openai/m1", "x/y"];

    fn id(n: u8) -> Uuid {
        Uuid::from_u128(u128::from(n))
    }

    fn entries() -> impl Strategy<Value = Vec<Entry>> {
        prop::collection::vec(
            (
                0u8..12,
                prop::sample::select(PROVIDERS),
                prop::sample::select(MODELS),
            )
                .prop_map(|(n, p, m)| Entry {
                    id: id(n),
                    ..entry(p, m)
                }),
            0..10,
        )
    }

    fn aliases() -> impl Strategy<Value = Vec<Alias>> {
        prop::collection::vec(
            (prop::sample::select(NAMES), 0u8..14).prop_map(|(name, n)| alias(name, id(n))),
            0..6,
        )
    }

    fn names() -> impl Strategy<Value = String> {
        prop_oneof![
            prop::sample::select(NAMES).prop_map(str::to_owned),
            prop::sample::select(MODELS).prop_map(str::to_owned),
            (
                prop::sample::select(PROVIDERS),
                prop::sample::select(MODELS)
            )
                .prop_map(|(p, m)| format!("{p}/{m}")),
            "[a-z/]{0,6}",
        ]
    }

    proptest! {
        #[test]
        fn the_index_resolves_every_name_as_the_linear_scan_did(
            entries in entries(),
            aliases in aliases(),
            name in names(),
        ) {
            let catalogue = Catalogue::new(entries, aliases);
            prop_assert_eq!(
                found(catalogue.resolve(&name)),
                linear_resolve(&name, catalogue.entries(), catalogue.aliases())
            );
            let empty = kpmr::Catalogue::default();
            prop_assert_eq!(
                catalogue.shadows(&name, &empty),
                linear_shadows(&name, catalogue.entries(), &empty)
            );
            prop_assert_eq!(
                catalogue.sharing(&name),
                catalogue.entries().iter().filter(|e| e.model_id == name).count()
            );
            for e in catalogue.entries() {
                prop_assert_eq!(
                    catalogue.entry(e.id).map(std::ptr::from_ref),
                    catalogue.entries().iter().find(|x| x.id == e.id).map(std::ptr::from_ref)
                );
                let of: Vec<&Alias> = catalogue.aliases_of(e.id).collect();
                let scanned: Vec<&Alias> =
                    catalogue.aliases().iter().filter(|a| a.route.uses(CatalogueModelId::from_uuid(e.id))).collect();
                prop_assert_eq!(of, scanned);
            }
        }
    }

    #[test]
    #[ignore = "timing; run with --ignored --nocapture"]
    fn resolving_against_a_large_catalogue_is_timed() {
        let entries: Vec<Entry> = (0..20_000)
            .map(|i| entry(&format!("p{}", i % 200), &format!("model-{}", i / 2)))
            .collect();
        let aliases: Vec<Alias> = (0..2_000)
            .map(|i| alias(format!("alias-{i}"), entries[i * 10].id))
            .collect();
        let names: Vec<String> = (0..1_000)
            .flat_map(|i| {
                [
                    format!("alias-{}", i * 2),
                    format!("p{}/model-{}", (i * 20) % 200, i * 10),
                    format!("model-{}", i * 10),
                    format!("missing-{i}"),
                ]
            })
            .collect();
        let started = std::time::Instant::now();
        let catalogue = Catalogue::new(entries, aliases);
        let built = started.elapsed();
        let time = |f: &dyn Fn(&str) -> bool| {
            let started = std::time::Instant::now();
            let found = names.iter().filter(|n| f(n)).count();
            (
                started.elapsed() / u32::try_from(names.len()).unwrap(),
                found,
            )
        };
        let (linear, linear_found) =
            time(&|n| linear_resolve(n, catalogue.entries(), catalogue.aliases()).is_ok());
        let (indexed, indexed_found) = time(&|n| catalogue.resolve(n).is_ok());
        assert_eq!(linear_found, indexed_found);
        println!(
            "20000 entries, 2000 aliases, {} names: index built in {built:?}; per name: linear {linear:?}, indexed {indexed:?}",
            names.len()
        );
    }
}

fn custom_provider(id: &str, generation: DialectId) -> crate::providers::CustomProvider {
    crate::providers::CustomProvider::new(crate::providers::Draft {
        uuid: Uuid::now_v7(),
        id: provider::ProviderId::parse(id).unwrap(),
        name: "Internal proxy".into(),
        base_url: "https://proxy.example.com".into(),
        auth: crate::providers::CustomAuth::XApiKey,
        generation,
        embeddings: None,
        count_tokens: None,
        server: None,
        media: Vec::new(),
        decisions: None,
    })
    .unwrap()
}

#[test]
fn a_custom_providers_model_resolves_only_in_its_organization_with_the_values_set_by_hand() {
    let mut e = entry("anthropic", "claude-haiku-4-5");
    e.custom = true;
    e.overrides = Fields {
        input_price: Some(dec!(0.5)),
        output_price: Some(dec!(1)),
        context_limit: Some(8192),
        ..Fields::default()
    };
    let org = OrgProviders::new(vec![custom_provider(
        "anthropic",
        DialectId::AnthropicMessages,
    )]);
    let own = effective(&e, &snapshot(), &org).unwrap();
    assert_eq!(own.provider.name(), "Internal proxy");
    assert_eq!(own.dialect, DialectId::AnthropicMessages);
    assert_eq!(own.fields.context_limit, Some(8192));
    assert_eq!(
        own.fields.adaptive_thinking, None,
        "no KPMR value reaches a custom provider's model"
    );
    assert_eq!(own.fields.output_limit, None);

    let mut vllm = entry("my-vllm", "llama-3.3-70b");
    vllm.custom = true;
    let org = OrgProviders::new(vec![custom_provider("my-vllm", DialectId::OpenAiChat)]);
    assert_eq!(
        effective(&vllm, &snapshot(), &org).unwrap().dialect,
        DialectId::OpenAiChat
    );
    assert!(
        effective(&vllm, &snapshot(), &OrgProviders::default()).is_none(),
        "another organization cannot resolve the provider"
    );
}

#[test]
fn a_custom_provider_counts_tokens_only_when_it_names_its_generation_dialect() {
    let org = OrgProviders::new(vec![custom_provider(
        "my-proxy",
        DialectId::AnthropicMessages,
    )]);
    let mut e = entry("my-proxy", "claude-x");
    e.custom = true;
    let e = effective(&e, &snapshot(), &org).unwrap();
    let mut count = Request::CountTokens(user(vec![Block::text("hi")]));
    let u = prepare(&mut count, DialectId::AnthropicMessages, &e).unwrap_err();
    assert_eq!(u.message, "The model `claude-x` cannot count tokens.");
    assert!(
        Fields {
            egress_dialect: Some(DialectId::OpenAiChat),
            ..Fields::default()
        }
        .validate(&e.provider)
        .is_err(),
        "a custom provider speaks only the dialects it names"
    );
}

#[test]
fn alibaba_models_speak_chat_completions_and_take_effort_levels_only_where_listed() {
    let flash = eff(&entry("alibaba", "qwen3.7-flash"));
    assert_eq!(flash.dialect, DialectId::OpenAiChat);
    assert_eq!(flash.provider.chat().provider, "QwenCloud");
    assert!(!flash.is_embeddings());
    assert_eq!(
        (flash.fields.input_price, flash.fields.output_price),
        (Some(dec!(0.03)), Some(dec!(0.13)))
    );
    assert!(effort_levels(&flash, DialectId::OpenAiResponses).is_empty());
    kept("none", DialectId::OpenAiChat, &flash);
    assert_eq!(
        effort_left_out("low", DialectId::OpenAiResponses, &flash),
        ["reasoning.effort"]
    );
    let effort = eff(&entry("alibaba", "qwen3.8-flash"));
    assert_eq!(
        effort_levels(&effort, DialectId::OpenAiChat),
        ["low", "medium", "xhigh"]
    );
    kept("low", DialectId::OpenAiResponses, &effort);
}

#[test]
fn cerebras_models_speak_chat_completions_with_kpmrs_prices_limits_and_effort_levels() {
    let gpt_oss = eff(&entry("cerebras", "gpt-oss-120b"));
    assert_eq!(gpt_oss.dialect, DialectId::OpenAiChat);
    assert_eq!(gpt_oss.provider.chat().provider, "Cerebras");
    assert_eq!(
        (gpt_oss.fields.input_price, gpt_oss.fields.output_price),
        (Some(dec!(0.35)), Some(dec!(0.75)))
    );
    assert_eq!(
        (gpt_oss.fields.context_limit, gpt_oss.fields.output_limit),
        (Some(131_072), Some(40_960))
    );
    assert_eq!(
        effort_levels(&gpt_oss, DialectId::AnthropicMessages),
        ["low", "medium", "high"]
    );
    kept("high", DialectId::OpenAiResponses, &gpt_oss);
    assert_eq!(
        nearest("minimal", DialectId::OpenAiChat, &gpt_oss).0,
        Some(ReasoningEffort::Low)
    );
    let qwen = eff(&entry("cerebras", "qwen-3.8-27b"));
    assert_eq!(
        effort_levels(&qwen, DialectId::OpenAiChat),
        ["none", "low", "medium", "high"]
    );
    kept("none", DialectId::OpenAiResponses, &qwen);
}

fn adapted(
    g: GenerateRequest,
    ingress: DialectId,
    e: &Effective,
) -> (Option<ReasoningEffort>, Option<ReasoningMode>, Vec<String>) {
    let mut req = Request::Generate(g);
    let plan = prepare(&mut req, ingress, e).unwrap();
    let Request::Generate(g) = req else {
        unreachable!()
    };
    let r = g.reasoning.unwrap_or_default();
    (r.effort, r.mode, plan.adapted)
}

fn thinking_off() -> GenerateRequest {
    let mut g = user(vec![Block::text("hi")]);
    let mut r = ReasoningConfig::default();
    r.mode = Some(ReasoningMode::Disabled);
    g.reasoning = Some(r);
    g
}

#[test]
fn thinking_off_becomes_the_lowest_effort_of_a_model_that_always_reasons() {
    use DialectId::{AnthropicMessages as A, OpenAiChat as C, OpenAiResponses as R};
    let gpt_oss = eff(&entry("cerebras", "gpt-oss-120b"));
    assert_eq!(
        adapted(thinking_off(), A, &gpt_oss),
        (
            Some(ReasoningEffort::Low),
            None,
            vec!["thinking".to_owned()]
        )
    );
    assert_eq!(
        adapted(with_effort("none"), C, &gpt_oss),
        (
            Some(ReasoningEffort::Low),
            None,
            vec!["reasoning_effort".to_owned()]
        )
    );
    assert_eq!(
        adapted(with_effort("none"), R, &gpt_oss),
        (
            Some(ReasoningEffort::Low),
            None,
            vec!["reasoning.effort".to_owned()]
        )
    );
    let gpt5 = eff(&entry("openai", "gpt-5"));
    assert_eq!(
        adapted(with_effort("none"), C, &gpt5).0,
        Some(ReasoningEffort::Minimal),
        "the lowest level, not the first one listed"
    );

    let qwen = eff(&entry("cerebras", "qwen-3.8-27b"));
    assert_eq!(
        adapted(thinking_off(), A, &qwen).2,
        Vec::<String>::new(),
        "a model that lists `none` takes it"
    );
    kept("none", C, &qwen);
    let haiku = eff(&entry("anthropic", "claude-haiku-4-5"));
    assert_eq!(
        adapted(thinking_off(), C, &haiku),
        (None, Some(ReasoningMode::Disabled), Vec::new()),
        "Claude turns thinking off itself"
    );
    let gpt4o = eff(&entry("openai", "gpt-4o"));
    assert_eq!(
        adapted(thinking_off(), A, &gpt4o).2,
        Vec::<String>::new(),
        "a model that doesn't reason is left alone"
    );
    kept("high", C, &gpt_oss);
}

#[test]
fn zai_models_speak_anthropic_messages_with_kpmrs_prices_and_limits() {
    let glm = eff(&entry("zai", "glm-5.3"));
    assert_eq!(glm.dialect, DialectId::AnthropicMessages);
    assert_eq!(glm.provider.name(), "Z.ai");
    assert_eq!(
        (
            glm.fields.input_price,
            glm.fields.output_price,
            glm.fields.cache_read_price
        ),
        (Some(dec!(1.4)), Some(dec!(4.4)), Some(dec!(0.26)))
    );
    assert_eq!(
        (glm.fields.context_limit, glm.fields.output_limit),
        (Some(1_000_000), Some(131_072))
    );
    let flash = eff(&entry("zai", "glm-5.3-flash"));
    assert!(
        flash
            .fields
            .input_modalities
            .as_ref()
            .is_some_and(|m| m.iter().any(|m| m == "image"))
    );
}

#[test]
fn minimax_models_speak_anthropic_messages_with_kpmrs_prices_and_limits() {
    let m3 = eff(&entry("minimax", "MiniMax-M3"));
    assert_eq!(m3.dialect, DialectId::AnthropicMessages);
    assert_eq!(m3.provider.messages().provider, "MiniMax");
    assert_eq!(
        (
            m3.fields.input_price,
            m3.fields.output_price,
            m3.fields.cache_read_price
        ),
        (Some(dec!(0.3)), Some(dec!(1.2)), Some(dec!(0.06)))
    );
    assert_eq!(
        (m3.fields.context_limit, m3.fields.output_limit),
        (Some(1_000_000), Some(524_288))
    );
    let flash = eff(&entry("minimax", "MiniMax-M3.1-Flash"));
    assert_eq!(
        (flash.fields.input_price, flash.fields.output_price),
        (Some(dec!(0.15)), Some(dec!(0.6)))
    );
}

#[test]
fn google_models_speak_gemini_with_kpmrs_prices_and_their_embeddings_embed() {
    use DialectId::{AnthropicMessages as A, OpenAiResponses as R};
    let flash = eff(&entry("google", "gemini-3.5-flash-lite"));
    assert_eq!(flash.dialect, DialectId::Gemini);
    assert_eq!(
        flash.provider.gemini(),
        kaeso_codecs::gemini::GeminiProfile::GOOGLE_AI_STUDIO
    );
    assert!(!flash.is_embeddings());
    assert_eq!(
        (
            flash.fields.input_price,
            flash.fields.output_price,
            flash.fields.cache_read_price
        ),
        (Some(dec!(0.3)), Some(dec!(2.5)), Some(dec!(0.03)))
    );
    assert_eq!(
        (flash.fields.context_limit, flash.fields.output_limit),
        (Some(1_048_576), Some(65_536))
    );
    let pro = eff(&entry("google", "gemini-3.1-pro-preview"));
    assert_eq!(
        (pro.fields.input_price, pro.fields.output_price),
        (Some(dec!(2)), Some(dec!(12)))
    );
    for model in ["gemini-embedding-2", "gemini-embedding-001"] {
        let embed = eff(&entry("google", model));
        assert_eq!(embed.dialect, DialectId::GeminiEmbeddings, "{model}");
        assert!(embed.is_embeddings(), "{model}");
    }
    assert_eq!(
        eff(&entry("google", "gemini-embedding-2"))
            .fields
            .input_price,
        Some(dec!(0.2))
    );

    assert_eq!(
        effort_levels(&flash, A),
        ["minimal", "low", "medium", "high"]
    );
    kept("low", R, &flash);
    assert_eq!(
        nearest("minimal", R, &pro),
        (
            Some(ReasoningEffort::Low),
            vec!["reasoning.effort".to_owned()]
        )
    );
    assert_eq!(nearest("xhigh", A, &flash).0, Some(ReasoningEffort::High));
    let budgeted = eff(&entry("google", "gemini-2.5-flash"));
    assert_eq!(effort_left_out("high", R, &budgeted), ["reasoning.effort"]);
    for model in [&budgeted, &flash] {
        let sent = check(with_budget(2_048, ReasoningMode::Enabled), A, model).unwrap();
        let Request::Generate(g) = sent else {
            unreachable!()
        };
        let r = g.reasoning.unwrap();
        assert_eq!((r.budget_tokens, r.effort), (Some(2_048), None));
    }
}

#[test]
fn agent_platform_models_speak_gemini_and_its_embeddings_models_are_refused() {
    let flash = eff(&entry("gemini-agent-platform", "gemini-3.5-flash"));
    assert_eq!(flash.dialect, DialectId::Gemini);
    assert!(!flash.is_embeddings());
    assert!(flash.fields.input_price.is_some());
    let embed = eff(&entry("gemini-agent-platform", "gemini-embedding-001"));
    assert!(embed.is_embeddings());
    let mut req = Request::Embed(EmbedRequest::new(
        "gemini-embedding-001",
        vec![EmbedInput::Text("x".into())],
    ));
    let refused = prepare(&mut req, DialectId::OpenAiEmbeddings, &embed).unwrap_err();
    assert_eq!(
        refused.message,
        "Kaeso calls no embeddings models on Gemini Enterprise Agent Platform, so `gemini-embedding-001` can't create embeddings."
    );
    let mut generate = Request::Generate(GenerateRequest::new("gemini-embedding-001"));
    assert!(
        prepare(&mut generate, DialectId::OpenAiChat, &embed)
            .unwrap_err()
            .message
            .contains("embeddings model")
    );
}

#[test]
fn agent_platform_models_go_in_the_format_of_their_family_and_count_where_it_counts() {
    let claude = eff(&entry("gemini-agent-platform", "claude-opus-4-5@20251101"));
    assert_eq!(claude.dialect, DialectId::AnthropicMessages);
    let llama = eff(&entry(
        "gemini-agent-platform",
        "meta/llama-4-maverick-17b-128e-instruct-maas",
    ));
    assert_eq!(llama.dialect, DialectId::OpenAiChat);
    assert_eq!(
        eff(&entry("gemini-agent-platform", "gemini-3.5-flash")).dialect,
        DialectId::Gemini
    );
    let p = &claude.provider;
    assert_eq!(
        p.count_tokens_for(DialectId::AnthropicMessages),
        Some(DialectId::AnthropicMessages)
    );
    assert_eq!(p.count_tokens_for(DialectId::OpenAiChat), None);
    assert_eq!(
        p.count_tokens_for(DialectId::Gemini),
        Some(DialectId::Gemini)
    );
    let mut count = Request::CountTokens(GenerateRequest::new("meta/llama"));
    assert!(prepare(&mut count, DialectId::AnthropicMessages, &llama).is_err());
    let mut count = Request::CountTokens(GenerateRequest::new("claude-opus-4-5@20251101"));
    assert_eq!(
        prepare(&mut count, DialectId::AnthropicMessages, &claude)
            .unwrap()
            .dialect,
        DialectId::AnthropicMessages
    );
    let mut sent_as = entry("gemini-agent-platform", "claude-opus-4-5@20251101");
    sent_as.overrides.egress_dialect = Some(DialectId::OpenAiChat);
    assert_eq!(eff(&sent_as).dialect, DialectId::OpenAiChat);
    let openai = eff(&entry("openai", "gpt-4o"));
    assert_eq!(
        openai.provider.count_tokens_for(DialectId::OpenAiChat),
        Some(DialectId::OpenAiResponses),
        "a provider without routes counts in its one format"
    );
}

fn image_feed() -> kpmr::Catalogue {
    let json = catalogue(
        r#"{"xai":{"id":"xai","models":{"grok-imagine-image-2.0":{"id":"grok-imagine-image-2.0","kind":"image","modalities":{"input":["text","image"],"output":["image"]},"pricing":{"unit":"usd_per_million_tokens","input_image":"8","output_image":"30","cache_read_image":"2","per_unit":[{"per":"image","of":"output","usd":"0.04","size":null,"quality":"low","resolution":"1K","audio":null,"estimate":false},{"per":"image","of":"input","usd":"0.01","estimate":true},{"per":"furlong","of":"output","usd":"1"},{"per":"image","of":"output"}]}}}}}"#,
    );
    kpmr::parse(&json, &crate::provider::known_ids())
        .unwrap()
        .catalogue
}

#[test]
fn kpmrs_media_prices_reach_the_catalogue_until_a_price_is_negotiated() {
    let snap = image_feed();
    let fields = |overrides: Fields| {
        let mut e = entry("xai", "grok-imagine-image-2.0");
        e.overrides = overrides;
        effective(&e, &snap, &OrgProviders::default())
            .unwrap()
            .fields
    };
    let listed = fields(Fields::default());
    assert_eq!(listed.input_image_price, Some(dec!(8)));
    assert_eq!(listed.output_image_price, Some(dec!(30)));
    assert_eq!(listed.cache_read_image_price, Some(dec!(2)));
    let units = listed.unit_prices.unwrap();
    assert_eq!(
        units.len(),
        3,
        "a unit price without `usd` is skipped, the rest kept"
    );
    assert_eq!(units[0].resolution.as_deref(), Some("1K"));
    assert!(units[1].estimate);
    let negotiated = fields(Fields {
        output_price: Some(dec!(1)),
        ..Fields::default()
    });
    assert_eq!(negotiated.output_image_price, None);
    assert_eq!(negotiated.unit_prices, None);
    let both = fields(Fields {
        output_price: Some(dec!(1)),
        output_image_price: Some(dec!(20)),
        unit_prices: Some(vec![]),
        ..Fields::default()
    });
    assert_eq!(both.output_image_price, Some(dec!(20)));
    assert_eq!(
        both.input_image_price,
        Some(dec!(8)),
        "one image price by hand keeps the others"
    );
    assert_eq!(both.unit_prices, Some(vec![]));
}

#[test]
fn media_prices_set_by_hand_are_checked() {
    let openai = ProviderRef::from(provider::provider("openai").unwrap());
    let unit = |per: &str, of: &str, usd: Decimal| UnitPrice {
        per: per.into(),
        of: of.into(),
        usd,
        ..UnitPrice::default()
    };
    let ok = Fields {
        input_image_price: Some(dec!(8)),
        unit_prices: Some(vec![
            unit("image", "output", dec!(0.04)),
            unit("megapixel", "output", dec!(0.08)),
        ]),
        ..Fields::default()
    };
    ok.validate(&openai).unwrap();
    for (fields, param) in [
        (
            Fields {
                output_image_price: Some(dec!(1000001)),
                ..Fields::default()
            },
            "output_image_price",
        ),
        (
            Fields {
                unit_prices: Some(vec![unit("frame", "output", dec!(1))]),
                ..Fields::default()
            },
            "unit_prices",
        ),
        (
            Fields {
                unit_prices: Some(vec![unit("image", "both", dec!(1))]),
                ..Fields::default()
            },
            "unit_prices",
        ),
        (
            Fields {
                unit_prices: Some(vec![unit("image", "output", dec!(0.0000000001))]),
                ..Fields::default()
            },
            "unit_prices",
        ),
        (
            Fields {
                unit_prices: Some(vec![unit("image", "output", dec!(1)); 33]),
                ..Fields::default()
            },
            "unit_prices",
        ),
    ] {
        assert_eq!(fields.validate(&openai).unwrap_err().field, param);
    }
    let mut patched = Fields::default();
    patched
        .patch(
            &serde_json::from_str(
                r#"{"unit_prices":[{"per":"image","of":"output","usd":"0.02","quality":"high"}]}"#,
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(
        patched.unit_prices.unwrap()[0].quality.as_deref(),
        Some("high")
    );
    let wrong = Fields::default().clone().patch(
        &serde_json::from_str(r#"{"unit_prices":[{"per":"image","of":"output"}]}"#).unwrap(),
    );
    assert!(wrong.is_err());
}

#[test]
fn image_requests_go_to_the_providers_image_api_and_need_an_image_model() {
    use kaeso_kpp::{ImageInput, ImageOperation, ImageRequest, Source};
    let draw =
        || Request::Image(ImageRequest::new("m", ImageOperation::Generate).with_prompt("A fox"));
    let mut req = draw();
    let image = eff(&entry("openai", "gpt-image-2"));
    assert_eq!(
        prepare(&mut req, DialectId::OpenAiImages, &image)
            .unwrap()
            .dialect,
        DialectId::OpenAiImages
    );
    let gemini = eff(&entry("google", "gemini-3.1-flash-image"));
    assert_eq!(
        prepare(&mut draw(), DialectId::OpenAiImages, &gemini)
            .unwrap()
            .dialect,
        DialectId::Gemini
    );
    let flash = eff(&entry("google", "gemini-3.5-flash"));
    let refused = prepare(&mut draw(), DialectId::OpenAiImages, &flash).unwrap_err();
    assert_eq!(
        refused.message,
        "The model `gemini-3.5-flash` does not produce images."
    );
    let claude = eff(&entry("anthropic", "claude-haiku-4-5"));
    let refused = prepare(&mut draw(), DialectId::OpenAiImages, &claude).unwrap_err();
    assert_eq!(
        (refused.capability, refused.param),
        ("images", Some("model"))
    );
    let mut text_only = eff(&entry("openai", "gpt-image-2"));
    text_only.fields.input_modalities = Some(vec!["text".into()]);
    let mut edit = Request::Image(
        ImageRequest::new("m", ImageOperation::Edit)
            .with_prompt("hat")
            .with_image(ImageInput::new(
                Source::base64("image/png", "AA==").unwrap(),
            )),
    );
    assert_eq!(
        prepare(&mut edit, DialectId::OpenAiImages, &text_only)
            .unwrap_err()
            .param,
        Some("image")
    );
}

#[test]
fn a_custom_image_model_needs_its_modalities_and_a_price() {
    let image = |input: Option<Vec<String>>, unit: bool| Fields {
        input_modalities: input,
        output_modalities: Some(vec!["image".into()]),
        unit_prices: unit.then(|| {
            vec![UnitPrice {
                per: "image".into(),
                of: "output".into(),
                usd: dec!(0.02),
                ..UnitPrice::default()
            }]
        }),
        ..Fields::default()
    };
    image(Some(vec!["text".into()]), true)
        .validate_custom()
        .unwrap();
    let e = image(Some(vec!["text".into()]), false)
        .validate_custom()
        .unwrap_err();
    assert_eq!(
        (e.field.as_str(), e.message.as_str()),
        ("unit_prices", "A custom model needs unit_prices.")
    );
    let e = image(None, false).validate_custom().unwrap_err();
    assert_eq!(
        e.message,
        "A custom model needs input_modalities, unit_prices."
    );
    let priced = Fields {
        output_image_price: Some(dec!(30)),
        ..image(Some(vec!["text".into()]), false)
    };
    priced.validate_custom().unwrap();
    let text = Fields {
        output_modalities: Some(vec!["text".into(), "image".into()]),
        ..image(Some(vec!["text".into()]), true)
    };
    assert_eq!(text.validate_custom().unwrap_err().field, "context_limit");
}

#[test]
fn custom_speech_and_transcription_models_need_their_modalities_and_a_price() {
    let model = |input: &str, output: &str, unit: Option<&str>| Fields {
        input_modalities: Some(vec![input.into()]),
        output_modalities: Some(vec![output.into()]),
        unit_prices: unit.map(|per| {
            vec![UnitPrice {
                per: per.into(),
                of: "input".into(),
                usd: dec!(0.006),
                ..UnitPrice::default()
            }]
        }),
        ..Fields::default()
    };
    model("text", "audio", Some("million_characters"))
        .validate_custom()
        .unwrap();
    model("audio", "text", Some("minute"))
        .validate_custom()
        .unwrap();
    assert_eq!(
        model("audio", "text", None)
            .validate_custom()
            .unwrap_err()
            .field,
        "unit_prices"
    );
    let chat_with_audio = Fields {
        input_modalities: Some(vec!["text".into(), "audio".into()]),
        ..model("audio", "text", Some("minute"))
    };
    assert_eq!(
        chat_with_audio.validate_custom().unwrap_err().field,
        "context_limit"
    );
}

#[test]
fn fireworks_models_speak_chat_completions_with_kpmrs_prices_and_its_embeddings_embed() {
    let flash = eff(&entry(
        "fireworks-ai",
        "accounts/fireworks/models/glm-5p3-flash",
    ));
    assert_eq!(flash.dialect, DialectId::OpenAiChat);
    assert_eq!(flash.provider.chat().provider, "Fireworks AI");
    assert_eq!(
        (
            flash.fields.input_price,
            flash.fields.output_price,
            flash.fields.cache_read_price
        ),
        (Some(dec!(0.15)), Some(dec!(0.5)), Some(dec!(0.03)))
    );
    assert_eq!(
        effort_levels(&flash, DialectId::OpenAiChat),
        ["low", "high", "max"]
    );
    let embed = eff(&entry(
        "fireworks-ai",
        "accounts/fireworks/models/qwen3-embedding-8b",
    ));
    assert_eq!(embed.dialect, DialectId::OpenAiEmbeddings);
    assert_eq!(embed.fields.input_price, Some(dec!(0.1)));
    let nomic = eff(&entry("fireworks-ai", "nomic-ai/nomic-embed-text-v1.5"));
    assert_eq!(
        (nomic.dialect, nomic.fields.input_price),
        (DialectId::OpenAiEmbeddings, Some(dec!(0.008)))
    );
    let mut sent_as = entry("fireworks-ai", "accounts/fireworks/models/kimi-k3");
    sent_as.overrides.egress_dialect = Some(DialectId::AnthropicMessages);
    assert_eq!(eff(&sent_as).dialect, DialectId::AnthropicMessages);
}

#[test]
fn nebius_models_speak_chat_completions_with_kpmrs_prices_and_its_embeddings_embed() {
    let glm = eff(&entry("nebius", "zai-org/GLM-5.3"));
    assert_eq!(glm.dialect, DialectId::OpenAiChat);
    assert_eq!(glm.provider.chat().provider, "Nebius Token Factory");
    assert_eq!(
        (
            glm.fields.input_price,
            glm.fields.output_price,
            glm.fields.cache_read_price
        ),
        (Some(dec!(1.4)), Some(dec!(4.4)), Some(dec!(0.26)))
    );
    assert_eq!(
        effort_levels(
            &eff(&entry("nebius", "moonshotai/Kimi-K3")),
            DialectId::OpenAiChat
        ),
        ["none", "low", "medium", "high", "max"]
    );
    let embed = eff(&entry("nebius", "Qwen/Qwen3-Embedding-8B"));
    assert_eq!(
        (embed.dialect, embed.fields.input_price),
        (DialectId::OpenAiEmbeddings, Some(dec!(0.01)))
    );
}

#[test]
fn sambanova_models_speak_chat_completions_and_count_tokens_in_messages() {
    let gpt_oss = eff(&entry("sambanova", "gpt-oss-120b"));
    assert_eq!(gpt_oss.dialect, DialectId::OpenAiChat);
    assert_eq!(gpt_oss.provider.chat().provider, "SambaNova");
    assert_eq!(
        (gpt_oss.fields.input_price, gpt_oss.fields.output_price),
        (Some(dec!(0.22)), Some(dec!(0.59)))
    );
    assert_eq!(
        effort_levels(&gpt_oss, DialectId::OpenAiChat),
        ["low", "medium", "high"]
    );
    let glm = eff(&entry("sambanova", "GLM-5.3"));
    assert_eq!(
        (glm.fields.input_price, glm.fields.cache_read_price),
        (Some(dec!(2.8)), Some(dec!(0.14)))
    );
    let provider = gpt_oss.provider.clone();
    assert_eq!(
        provider.count_tokens_for(DialectId::OpenAiChat),
        Some(DialectId::AnthropicMessages)
    );
    let mut sent_as = entry("sambanova", "GLM-5.3");
    sent_as.overrides.egress_dialect = Some(DialectId::AnthropicMessages);
    let messages = eff(&sent_as);
    assert_eq!(messages.dialect, DialectId::AnthropicMessages);
    assert_eq!(
        messages.provider.count_tokens_for(messages.dialect),
        Some(messages.dialect)
    );
}

#[test]
fn standard_compute_models_speak_chat_completions_without_prices_and_smart_routing_is_a_router() {
    let routing = eff(&entry("standardcompute", "standardcompute"));
    assert_eq!(routing.dialect, DialectId::OpenAiChat);
    assert_eq!(routing.provider.chat().provider, "Standard Compute");
    assert_eq!(
        routing.fields.display_name.as_deref(),
        Some("Smart routing")
    );
    assert_eq!(
        routing.provider.count_tokens_for(DialectId::OpenAiChat),
        None
    );
    let models = &snapshot().providers["standardcompute"].models;
    assert!(models["standardcompute"].is_router());
    assert_eq!(models.values().filter(|m| m.is_router()).count(), 1);
    for id in [
        "standardcompute",
        "anthropic/claude-sonnet-5.5",
        "openai/gpt-5.6-luna:eu",
        "z-ai/glm-5.3:us",
    ] {
        let m = eff(&entry("standardcompute", id));
        assert_eq!(
            (
                m.fields.input_price,
                m.fields.output_price,
                m.fields.cache_read_price
            ),
            (None, None, None),
            "{id}: Standard Compute bills a plan, not tokens"
        );
    }
    assert!(models["google/gemini-2.5-pro"].is_deprecated());
}

#[test]
fn cloudflare_models_go_out_in_the_format_cloudflare_lists_with_their_efforts_and_prices() {
    let claude = eff(&entry(
        "cloudflare-ai-gateway",
        "anthropic/claude-sonnet-5.5",
    ));
    assert_eq!(claude.dialect, DialectId::AnthropicMessages);
    assert_eq!(
        (claude.fields.input_price, claude.fields.output_price),
        (Some(dec!(2)), Some(dec!(10)))
    );
    let luna = eff(&entry("cloudflare-ai-gateway", "openai/gpt-5.6-luna"));
    assert_eq!(luna.dialect, DialectId::OpenAiResponses);
    let glm = eff(&entry("cloudflare-ai-gateway", "@cf/zai-org/glm-4.7-flash"));
    assert_eq!(glm.dialect, DialectId::OpenAiChat);
    assert_eq!(glm.provider.chat().provider, "Cloudflare AI Gateway");
    assert_eq!(glm.provider.count_tokens_for(DialectId::OpenAiChat), None);
    assert_eq!(
        effort_levels(&glm, DialectId::OpenAiChat),
        ["low", "medium", "high"]
    );
    assert_eq!(
        (glm.fields.input_price, glm.fields.output_price),
        (Some(dec!(0.0605)), Some(dec!(0.4)))
    );
    let qwen = eff(&entry("cloudflare-ai-gateway", "@cf/qwen/qwen3.8-27b"));
    assert_eq!(
        effort_levels(&qwen, DialectId::OpenAiChat),
        ["low", "medium", "xhigh"]
    );
    let snap = snapshot();
    assert!(entry("cloudflare-ai-gateway", "openai/o3-mini").is_deprecated(&snap));
    let scope = glm.provider.built_in().unwrap().account_scope.unwrap();
    assert_eq!(
        scope.base_path("0123456789abcdef0123456789abcdef"),
        "/client/v4/accounts/0123456789abcdef0123456789abcdef/ai/v1"
    );
}

#[test]
fn scaleway_models_speak_chat_completions_and_whisper_is_priced_by_the_minute() {
    let gpt_oss = eff(&entry("scaleway", "gpt-oss-120b"));
    assert_eq!(gpt_oss.dialect, DialectId::OpenAiChat);
    assert_eq!(gpt_oss.provider.chat().provider, "Scaleway");
    assert_eq!(
        effort_levels(&gpt_oss, DialectId::OpenAiChat),
        ["low", "medium", "high"]
    );
    let embed = eff(&entry("scaleway", "bge-multilingual-gemma2"));
    assert_eq!(embed.dialect, DialectId::OpenAiEmbeddings);
    let catalogue = snapshot();
    let whisper = catalogue.model("scaleway", "whisper-large-v3").unwrap();
    let per_unit = &whisper.pricing.as_ref().unwrap().per_unit;
    assert_eq!(per_unit.len(), 1);
    assert_eq!(per_unit[0].usd, dec!(0.0033645));
}

fn decide(questions: Vec<kaeso_kpp::QuestionKind>) -> Request {
    let mut d = kaeso_kpp::DecideRequest::new(
        "m",
        kaeso_kpp::DecisionInput::Text("A refund for order 7".into()),
    );
    for (i, kind) in questions.into_iter().enumerate() {
        d = d.with_question(kaeso_kpp::Question::new(Some(format!("q{i}")), kind));
    }
    Request::Decide(d)
}

fn yes_no() -> kaeso_kpp::QuestionKind {
    kaeso_kpp::QuestionKind::Predicate(kaeso_kpp::Predicate::new(None, None))
}

fn choice(n: usize) -> kaeso_kpp::QuestionKind {
    kaeso_kpp::QuestionKind::Choice(kaeso_kpp::Choice::new(
        (0..n)
            .map(|i| {
                kaeso_kpp::ChoiceOption::new(kaeso_kpp::ChoiceValue::Text(format!("o{i}")), None)
            })
            .collect(),
    ))
}

fn score(n: usize) -> kaeso_kpp::QuestionKind {
    kaeso_kpp::QuestionKind::Score(kaeso_kpp::Score::new(
        (0..n)
            .map(|i| kaeso_kpp::Level::new(Some(format!("l{i}")), None))
            .collect(),
    ))
}

#[test]
fn decision_models_come_from_kpmrs_kind_and_decisions_object() {
    let luna = eff(&entry("openai", "gpt-6-luna"));
    assert!(luna.decides() && !luna.decides_only());
    assert_eq!(luna.dialect, DialectId::OpenAiResponses);
    let gemini = eff(&entry("google", "gemini-decision-1"));
    assert!(gemini.decides() && gemini.decides_only());
    let d = gemini.fields.decisions.as_ref().unwrap();
    assert_eq!(d.max_questions, Some(1000));
    assert!(d.answers("predicate") && !d.answers("box"));
    let chat = eff(&entry("openai", "gpt-5.5"));
    assert!(!chat.decides() && !chat.decides_only());
    let bare = DecisionFields::default();
    assert!(bare.answers("score") && !bare.answers("box"));
    let m: kpmr::Model = serde_json::from_str(r#"{"id": "m", "kind": "decision"}"#).unwrap();
    assert_eq!(
        Fields::from_kpmr(&m).decisions,
        Some(DecisionFields {
            only: true,
            ..DecisionFields::default()
        })
    );
}

#[test]
fn decisions_go_to_the_providers_decisions_api_in_its_own_format() {
    for (provider, model, dialect) in [
        ("openai", "gpt-6-luna", DialectId::OpenAiDecisions),
        ("google", "gemini-decision-1", DialectId::GeminiDecisions),
        ("openrouter", "typesafe/jev-1.14", DialectId::SystemOne),
        ("vercel", "typesafe-ai/jev", DialectId::OpenAiDecisions),
        ("perplexity", "pplx-decider-v1.2", DialectId::SystemOne),
        ("typesafe", "jev-latest", DialectId::SystemOne),
        ("typesafe", "jev-1.13.0", DialectId::SystemOne),
    ] {
        let e = eff(&entry(provider, model));
        let plan = prepare(&mut decide(vec![yes_no()]), DialectId::SystemOne, &e).unwrap();
        assert_eq!(plan.dialect, dialect, "{provider}/{model}");
    }
}

#[test]
fn decisions_are_refused_where_the_model_or_its_provider_cannot_answer_them() {
    let refused = |provider: &str, model: &str, req: &mut Request| {
        let e = eff(&entry(provider, model));
        let u = prepare(req, DialectId::OpenAiDecisions, &e).unwrap_err();
        (u.capability, u.param, u.message)
    };
    assert_eq!(
        refused("openai", "gpt-5.5", &mut decide(vec![yes_no()])),
        (
            "decisions",
            Some("model"),
            "The model `gpt-5.5` is a generation model; it can't answer decision questions.".into()
        )
    );
    assert_eq!(
        refused("anthropic", "claude-haiku-4-5", &mut decide(vec![yes_no()])).2,
        "Anthropic offers no decisions API Kaeso can call, so `claude-haiku-4-5` can't make decisions."
    );
    assert_eq!(
        refused(
            "cloudflare-ai-gateway",
            "typesafe/jev",
            &mut decide(vec![yes_no()])
        )
        .2,
        "Cloudflare AI Gateway offers no decisions API Kaeso can call, so `typesafe/jev` can't make decisions."
    );
    assert_eq!(
        refused("openrouter", "openai/gpt-5", &mut decide(vec![yes_no()])).2,
        "The model `openai/gpt-5` is a generation model; it can't answer decision questions."
    );
    assert_eq!(
        refused(
            "google",
            "gemini-decision-1",
            &mut decide(vec![
                yes_no(),
                kaeso_kpp::QuestionKind::Box(kaeso_kpp::BoxQuestion::new())
            ])
        ),
        (
            "question type",
            Some("questions"),
            "The model `gemini-decision-1` does not answer box questions.".into()
        )
    );
    assert_eq!(
        refused(
            "typesafe",
            "jev-1.13.0",
            &mut decide(vec![
                yes_no(),
                kaeso_kpp::QuestionKind::Box(kaeso_kpp::BoxQuestion::new())
            ])
        )
        .2,
        "The model `jev-1.13.0` does not answer box questions."
    );
    let mut boxed = decide(vec![kaeso_kpp::QuestionKind::Box(
        kaeso_kpp::BoxQuestion::new(),
    )]);
    assert!(
        prepare(
            &mut boxed,
            DialectId::SystemOne,
            &eff(&entry("typesafe", "jev-1.14.0"))
        )
        .is_ok()
    );
    let mut many = decide((0..401).map(|_| yes_no()).collect());
    assert_eq!(
        refused("openai", "gpt-6-luna", &mut many).2,
        "The model `gpt-6-luna` answers at most 400 questions per request."
    );
    assert_eq!(
        refused(
            "perplexity",
            "pplx-decider-v1.2",
            &mut decide(vec![choice(256)])
        )
        .2,
        "The model `pplx-decider-v1.2` takes at most 255 options per choice question."
    );
    assert_eq!(
        refused("openai", "gpt-6-luna", &mut decide(vec![score(11)])).2,
        "The model `gpt-6-luna` takes at most 10 levels per score question."
    );
    let image = kaeso_kpp::Block::image(Source::base64("image/png", "AA==").unwrap());
    let mut with_image = decide(vec![yes_no()]);
    if let Request::Decide(d) = &mut with_image {
        d.input = kaeso_kpp::DecisionInput::Messages(vec![Message::new(Role::User, vec![image])]);
    }
    let mut text_only = eff(&entry("google", "gemini-decision-1"));
    text_only.fields.input_modalities = Some(vec!["text".into()]);
    let u = prepare(&mut with_image.clone(), DialectId::SystemOne, &text_only).unwrap_err();
    assert_eq!(
        (u.param, u.message.as_str()),
        (
            Some("state"),
            "The model `gemini-decision-1` does not accept images."
        )
    );
    let mut one_image = eff(&entry("google", "gemini-decision-1"));
    one_image.fields.decisions.as_mut().unwrap().max_images = Some(0);
    let u = prepare(&mut with_image, DialectId::GeminiDecisions, &one_image).unwrap_err();
    assert_eq!(
        (u.param, u.message.as_str()),
        (
            Some("input"),
            "The model `gemini-decision-1` takes at most 0 images per request."
        )
    );
}

#[test]
fn a_decision_only_model_refuses_chat_token_counts_and_embeddings() {
    let gemini = eff(&entry("google", "gemini-decision-1"));
    let message = "The model `gemini-decision-1` is a decision model; send it to `/v1/decisions` or `/v1/systemone`.";
    for mut req in [
        Request::Generate(user_request()),
        Request::CountTokens(user_request()),
        Request::Embed(EmbedRequest::new("m", vec![EmbedInput::Text("a".into())])),
    ] {
        let u = prepare(&mut req, DialectId::OpenAiChat, &gemini).unwrap_err();
        assert_eq!((u.param, u.message.as_str()), (Some("model"), message));
    }
    let jev = eff(&entry("typesafe", "jev-latest"));
    assert_eq!(jev.dialect, DialectId::SystemOne);
    let u = prepare(
        &mut Request::Generate(user_request()),
        DialectId::AnthropicMessages,
        &jev,
    )
    .unwrap_err();
    assert_eq!(
        u.message,
        "The model `jev-latest` is a decision model; send it to `/v1/decisions` or `/v1/systemone`."
    );
    let luna = eff(&entry("openai", "gpt-6-luna"));
    assert!(
        prepare(
            &mut Request::Generate(user_request()),
            DialectId::OpenAiResponses,
            &luna
        )
        .is_ok()
    );
}

#[test]
fn decisions_are_set_by_hand_only_where_the_provider_has_a_decisions_api() {
    let openrouter: ProviderRef = provider::provider("openrouter").unwrap().into();
    let anthropic: ProviderRef = provider::provider("anthropic").unwrap().into();
    let with = |d: DecisionFields| Fields {
        decisions: Some(d),
        ..Fields::default()
    };
    let only = DecisionFields {
        only: true,
        question_types: Some(vec!["choice".into(), "box".into()]),
        max_questions: Some(64),
        ..DecisionFields::default()
    };
    assert!(with(only.clone()).validate(&openrouter).is_ok());
    let off = ProviderRef::Custom(std::sync::Arc::new(custom_provider(
        "my-vllm",
        DialectId::OpenAiChat,
    )));
    assert_eq!(
        with(only.clone()).validate(&off).unwrap_err().message,
        "The custom provider `my-vllm` has no decisions API turned on; turn it on first."
    );
    let mut draft = custom_provider("my-vllm", DialectId::OpenAiChat).draft();
    draft.media = vec![crate::providers::MediaApi::Decisions];
    let on = ProviderRef::Custom(std::sync::Arc::new(
        crate::providers::CustomProvider::new(draft).unwrap(),
    ));
    assert!(with(only.clone()).validate(&on).is_ok());
    let e = with(only.clone()).validate(&anthropic).unwrap_err();
    assert_eq!(
        (e.field.as_str(), e.message.as_str()),
        (
            "decisions",
            "Anthropic offers no decisions API Kaeso can call; remove `decisions`."
        )
    );
    for (bad, message) in [
        (
            DecisionFields {
                question_types: Some(vec!["extract".into()]),
                ..DecisionFields::default()
            },
            "`extract` is not one of predicate, choice, score, box.",
        ),
        (
            DecisionFields {
                question_types: Some(vec![]),
                ..DecisionFields::default()
            },
            "`decisions.question_types` must list at least one type.",
        ),
        (
            DecisionFields {
                max_levels: Some(0),
                ..DecisionFields::default()
            },
            "The limits in `decisions` must be positive.",
        ),
    ] {
        assert_eq!(
            with(bad).validate(&openrouter).unwrap_err().message,
            message
        );
    }
    let mut patched = Fields::default();
    patched
        .patch(
            serde_json::json!({"decisions": {"only": true, "max_options": 26}})
                .as_object()
                .unwrap(),
        )
        .unwrap();
    assert_eq!(patched.decisions.as_ref().unwrap().max_options, Some(26));
    assert_eq!(patched.set_names(), ["decisions"]);
    let custom = Fields {
        input_modalities: Some(vec!["text".into()]),
        input_price: Some(dec!(0.042)),
        ..with(only)
    };
    assert!(custom.validate_custom().is_ok());
    let e = Fields {
        input_price: None,
        ..custom
    }
    .validate_custom()
    .unwrap_err();
    assert_eq!(e.message, "A custom model needs input_price.");
}
