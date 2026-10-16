use std::collections::BTreeSet;

use kaeso_kpp::{
    Block, DialectId, DocumentBlock, EmbedInput, EmbedRequest, FunctionTool, GenerateRequest,
    JsonSchemaSpec, Message, OutputSpec, RawJson, ReasoningConfig, ReasoningEffort, Request, Role,
    Source, ToolDef,
};
use rust_decimal_macros::dec;
use uuid::Uuid;

use super::capability::{effort_levels, model_info, prepare};
use super::resolve::{Access, AccessLevel, Alias, ResolveError, Via, resolve, shadows};
use super::*;

fn snapshot() -> models_dev::Snapshot {
    models_dev::embedded().unwrap()
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
    }
}

fn eff(e: &Entry) -> Effective {
    effective(e, &snapshot(), &Supplement::embedded().unwrap()).unwrap()
}

#[test]
fn the_shipped_supplement_is_valid_and_names_known_models() {
    let supplement = Supplement::embedded().unwrap();
    let snap = snapshot();
    for (provider, model) in supplement.models.keys() {
        assert!(
            snap.model(provider, model).is_some(),
            "{provider}/{model} is in the supplement but not in models.dev"
        );
    }
    for model in snap.providers["anthropic"].models.keys() {
        assert!(
            supplement
                .get("anthropic", model)
                .is_some_and(|f| f.adaptive_thinking.is_some()),
            "anthropic/{model} has no adaptive_thinking entry"
        );
    }
    assert!(Supplement::parse("[nope.x]\nadaptive_thinking = true").is_err());
    assert!(Supplement::parse("[openai.x]\nadaptive_thinking = true").is_err());
    assert!(Supplement::parse("[anthropic.x]\nunknown = 1").is_err());
}

#[test]
fn overrides_win_then_models_dev_then_the_supplement() {
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
fn delisted_models_keep_their_last_known_values() {
    let snap = snapshot();
    let mut e = entry("openai", "gone-model");
    let mut last = snap.model("openai", "gpt-4o").unwrap().clone();
    last.id = "gone-model".into();
    e.last_known = Some(last);
    e.delisted = true;
    let x = effective(&e, &snap, &Supplement::default()).unwrap();
    assert_eq!(x.fields.input_price, Some(dec!(2.5)));
    let bare = effective(&entry("openai", "unknown"), &snap, &Supplement::default()).unwrap();
    assert_eq!(bare.fields, Fields::default());
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
    assert!(f.patch(wrong.as_object().unwrap()).is_err());
    assert_eq!(serde_json::to_value(&f).unwrap()["output_price"], "2.5");
}

#[test]
fn values_are_checked() {
    let openai = provider::provider("openai").unwrap();
    let anthropic = provider::provider("anthropic").unwrap();
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

#[test]
fn names_resolve_alias_then_prefixed_then_native() {
    let gpt = entry("openai", "gpt-4o");
    let claude = entry("anthropic", "claude-sonnet-4-5");
    let entries = vec![gpt.clone(), claude.clone()];
    let aliases = vec![
        Alias {
            name: "chat-default".into(),
            target: claude.id,
        },
        Alias {
            name: "gpt-4o".into(),
            target: claude.id,
        },
    ];
    let r = |n: &str| resolve(n, &entries, &aliases).map(|(e, v)| (e.id, v));
    assert_eq!(r("chat-default"), Ok((claude.id, Via::Alias)));
    assert_eq!(r("gpt-4o"), Ok((claude.id, Via::Alias)));
    assert_eq!(r("openai/gpt-4o"), Ok((gpt.id, Via::Prefixed)));
    assert_eq!(r("claude-sonnet-4-5"), Ok((claude.id, Via::Native)));
    assert!(matches!(r("gpt-5"), Err(ResolveError::NotInCatalogue(_))));
    assert!(r("anthropic/claude-opus-4-6").is_err());
    let twins = vec![entry("openai", "same"), entry("anthropic", "same")];
    let err = resolve("same", &twins, &[]).unwrap_err();
    assert!(err.to_string().contains("openai/same"), "{err}");
    assert!(shadows("gpt-4o", &entries, &snapshot()));
    assert!(shadows("gpt-5", &[], &snapshot()));
    assert!(!shadows("chat-default", &entries, &snapshot()));
    assert!(super::resolve::validate_alias_name("a/b").is_err());
    assert!(super::resolve::validate_alias_name("fast chat").is_err());
}

#[test]
fn access_lists_narrow_level_by_level() {
    let (a, b, c) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
    let all = Access::default();
    assert!(all.allows(a));
    let member: BTreeSet<Uuid> = [a, b].into();
    let key: BTreeSet<Uuid> = [b].into();
    let narrowed = Access {
        member: Some(member.clone()),
        key: Some(key.clone()),
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
    let image = user(vec![Block::image(Source::base64("image/png", "AAAA"))]);
    let err = check(image.clone(), DialectId::OpenAiChat, &gpt35).unwrap_err();
    assert!(
        err.contains("gpt-3.5-turbo") && err.contains("images"),
        "{err}"
    );
    assert!(check(image, DialectId::OpenAiChat, &gpt4o).is_ok());
    let pdf = user(vec![Block::Document(DocumentBlock::new(Source::base64(
        "application/pdf",
        "AAAA",
    )))]);
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
    let reasoning = with_effort("high");
    assert!(
        check(reasoning, DialectId::OpenAiChat, &gpt4o)
            .unwrap_err()
            .contains("reasoning")
    );
    let mut temp = user(vec![Block::text("hi")]);
    temp.sampling.temperature = Some(0.2);
    let gpt5 = eff(&entry("openai", "gpt-5"));
    assert!(
        check(temp.clone(), DialectId::OpenAiChat, &gpt5)
            .unwrap_err()
            .contains("temperature")
    );
    temp.sampling.temperature = Some(1.0);
    assert!(check(temp, DialectId::OpenAiChat, &gpt5).is_ok());
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
fn efforts_are_refused_not_replaced() {
    let gpt5 = eff(&entry("openai", "gpt-5"));
    let err = check(with_effort("xhigh"), DialectId::OpenAiChat, &gpt5).unwrap_err();
    assert!(
        err.contains("`minimal`, `low`, `medium`, `high`") && err.contains("`xhigh`"),
        "{err}"
    );
    assert!(check(with_effort("minimal"), DialectId::OpenAiChat, &gpt5).is_ok());
    assert!(check(with_effort("none"), DialectId::OpenAiChat, &gpt5).is_err());
    let haiku = eff(&entry("anthropic", "claude-haiku-4-5"));
    assert_eq!(
        effort_levels(&haiku, DialectId::OpenAiChat),
        ["low", "medium", "high", "xhigh", "max"]
    );
    assert!(check(with_effort("medium"), DialectId::OpenAiChat, &haiku).is_ok());
    assert!(check(with_effort("max"), DialectId::OpenAiChat, &haiku).is_ok());
    assert!(check(with_effort("minimal"), DialectId::OpenAiChat, &haiku).is_err());
    assert!(
        check(with_effort("low"), DialectId::AnthropicMessages, &haiku)
            .unwrap_err()
            .contains("does not accept an effort")
    );
    let sonnet46 = eff(&entry("anthropic", "claude-sonnet-4-6"));
    assert!(check(with_effort("xhigh"), DialectId::OpenAiResponses, &sonnet46).is_err());
    assert!(check(with_effort("max"), DialectId::OpenAiResponses, &sonnet46).is_ok());
    assert!(check(with_effort("none"), DialectId::OpenAiResponses, &sonnet46).is_ok());
    assert!(check(with_effort("turbo"), DialectId::OpenAiResponses, &sonnet46).is_err());
}

#[test]
fn duties_fill_what_kpp_cannot_know() {
    let haiku = eff(&entry("anthropic", "claude-haiku-4-5"));
    let mut req = Request::Generate(user(vec![Block::text("hi")]));
    let plan = prepare(&mut req, DialectId::OpenAiChat, &haiku).unwrap();
    assert_eq!(plan.dialect, DialectId::AnthropicMessages);
    assert_eq!(plan.adaptive_thinking, Some(false));
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
            reasoning: Some(false),
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
