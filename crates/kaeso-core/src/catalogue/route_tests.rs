use std::collections::BTreeSet;

use kaeso_codecs::HeaderList;
use serde_json::json;
use uuid::Uuid;

use super::*;

fn id(n: u128) -> CatalogueModelId {
    CatalogueModelId::from_uuid(Uuid::from_u128(n))
}

fn route(value: serde_json::Value) -> Step {
    serde_json::from_value(value).unwrap()
}

fn m(n: u128) -> String {
    id(n).to_string()
}

struct World {
    headers: HeaderList,
    uses: BTreeSet<Feature>,
    format: AppFormat,
    input_tokens: u64,
    key: Option<ApiKeyId>,
    request: Uuid,
    caller: Option<Uuid>,
}

impl World {
    fn new() -> World {
        World {
            headers: HeaderList::new(),
            uses: BTreeSet::new(),
            format: AppFormat::OpenaiChat,
            input_tokens: 100,
            key: None,
            request: Uuid::from_u128(7),
            caller: None,
        }
    }

    fn facts(&self) -> Facts<'_> {
        Facts {
            alias: Uuid::from_u128(42),
            request: self.request,
            caller: self.caller,
            format: Some(self.format),
            headers: &self.headers,
            uses: &self.uses,
            input_tokens: self.input_tokens,
            key: self.key,
            member: None,
            service_account: None,
        }
    }
}

fn planned(step: &Step, w: &World) -> Vec<(CatalogueModelId, String)> {
    step.plan(&w.facts())
        .into_iter()
        .map(|p| (p.step.model, p.path))
        .collect()
}

#[test]
fn a_model_step_reads_with_its_defaults_and_writes_them_back() {
    let step = route(json!({ "type": "model", "model": m(1) }));
    assert_eq!(step, Step::model(id(1)));
    assert_eq!(
        serde_json::to_value(&step).unwrap(),
        json!({
            "type": "model",
            "model": m(1),
            "fallback": null,
            "fall_back_on": ["unavailable", "limited", "unsupported"],
            "timeout_seconds": null
        })
    );
}

#[test]
fn unknown_fields_and_types_are_refused() {
    for bad in [
        json!({ "type": "model", "model": m(1), "retries": 3 }),
        json!({ "type": "loop", "model": m(1) }),
        json!({ "type": "condition", "match": "all", "rules": [{ "field": "colour", "op": "in", "value": [] }], "then": { "type": "model", "model": m(1) }, "else": { "type": "model", "model": m(2) } }),
        json!({ "type": "model", "model": "openai/gpt-5" }),
        json!({ "type": "split", "by": "moon", "branches": [] }),
    ] {
        assert!(
            serde_json::from_value::<Step>(bad.clone()).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn a_full_route_round_trips_through_json() {
    let value = json!({
        "type": "condition",
        "match": "any",
        "rules": [
            { "field": "header", "name": "x-plan", "op": "equals", "value": "paid" },
            { "field": "format", "op": "not_in", "value": ["openai_embeddings"] },
            { "field": "feature", "op": "used", "value": "images" },
            { "field": "input_tokens", "op": "at_least", "value": 100000 },
            { "field": "key", "op": "in", "value": [ApiKeyId::from_uuid(Uuid::from_u128(5)).to_string()] },
            { "field": "member", "op": "not_in", "value": [UserId::from_uuid(Uuid::from_u128(6)).to_string()] },
            { "field": "service_account", "op": "in", "value": [ServiceAccountId::from_uuid(Uuid::from_u128(8)).to_string()] },
            { "field": "header", "name": "x-beta", "op": "present", "value": null }
        ],
        "then": {
            "type": "model", "model": m(1),
            "fallback": { "type": "model", "model": m(2), "fallback": null, "fall_back_on": ["unavailable"], "timeout_seconds": 20 },
            "fall_back_on": ["unavailable", "limited", "unsupported", "rejected"],
            "timeout_seconds": 10
        },
        "else": {
            "type": "split", "by": "caller",
            "branches": [
                { "weight": 90, "step": { "type": "model", "model": m(3), "fallback": null, "fall_back_on": ["unavailable", "limited", "unsupported"], "timeout_seconds": null } },
                { "weight": 10, "step": { "type": "model", "model": m(4), "fallback": null, "fall_back_on": ["unavailable", "limited", "unsupported"], "timeout_seconds": null } }
            ]
        }
    });
    let step = route(value.clone());
    assert_eq!(serde_json::to_value(&step).unwrap(), value);
    assert_eq!(step.validate(&|_| true), Ok(()));
    assert_eq!(step.models(), BTreeSet::from([id(1), id(2), id(3), id(4)]));
    assert!(step.uses(id(4)) && !step.uses(id(9)));
    assert_eq!(step.models_in_order(), [id(1), id(2), id(3), id(4)]);
    assert_eq!(step.primary(), Some(id(1)));
}

#[test]
fn validation_names_the_field_that_is_wrong() {
    let known = |m: CatalogueModelId| m != id(9);
    let cases = [
        (json!({ "type": "model", "model": m(9) }), "route.model"),
        (
            json!({ "type": "model", "model": m(1), "timeout_seconds": 0 }),
            "route.timeout_seconds",
        ),
        (
            json!({ "type": "model", "model": m(1), "timeout_seconds": 3601 }),
            "route.timeout_seconds",
        ),
        (
            json!({ "type": "model", "model": m(1), "fall_back_on": [], "fallback": { "type": "model", "model": m(2) } }),
            "route.fall_back_on",
        ),
        (
            json!({ "type": "model", "model": m(1), "fallback": { "type": "model", "model": m(9) } }),
            "route.fallback.model",
        ),
        (
            json!({ "type": "condition", "match": "all", "rules": [], "then": { "type": "model", "model": m(1) }, "else": { "type": "model", "model": m(2) } }),
            "route.rules",
        ),
        (
            json!({ "type": "condition", "match": "all", "rules": [{ "field": "header", "name": "X-Plan", "op": "equals", "value": "paid" }], "then": { "type": "model", "model": m(1) }, "else": { "type": "model", "model": m(2) } }),
            "route.rules.0.name",
        ),
        (
            json!({ "type": "condition", "match": "all", "rules": [{ "field": "header", "name": "authorization", "op": "contains", "value": "sk-" }], "then": { "type": "model", "model": m(1) }, "else": { "type": "model", "model": m(2) } }),
            "route.rules.0.name",
        ),
        (
            json!({ "type": "condition", "match": "all", "rules": [{ "field": "header", "name": "x-plan", "op": "equals" }], "then": { "type": "model", "model": m(1) }, "else": { "type": "model", "model": m(2) } }),
            "route.rules.0.value",
        ),
        (
            json!({ "type": "condition", "match": "all", "rules": [{ "field": "header", "name": "x-plan", "op": "present", "value": "x" }], "then": { "type": "model", "model": m(1) }, "else": { "type": "model", "model": m(2) } }),
            "route.rules.0.value",
        ),
        (
            json!({ "type": "condition", "match": "all", "rules": [{ "field": "header", "name": "x-plan", "op": "equals", "value": "a".repeat(201) }], "then": { "type": "model", "model": m(1) }, "else": { "type": "model", "model": m(2) } }),
            "route.rules.0.value",
        ),
        (
            json!({ "type": "condition", "match": "all", "rules": [{ "field": "format", "op": "in", "value": ["openai_chat", "openai_chat"] }], "then": { "type": "model", "model": m(1) }, "else": { "type": "model", "model": m(2) } }),
            "route.rules.0.value",
        ),
        (
            json!({ "type": "condition", "match": "all", "rules": [{ "field": "input_tokens", "op": "below", "value": 0 }], "then": { "type": "model", "model": m(1) }, "else": { "type": "model", "model": m(2) } }),
            "route.rules.0.value",
        ),
        (
            json!({ "type": "condition", "match": "all", "rules": [{ "field": "key", "op": "in", "value": [] }], "then": { "type": "model", "model": m(1) }, "else": { "type": "model", "model": m(2) } }),
            "route.rules.0.value",
        ),
        (
            json!({ "type": "condition", "match": "all", "rules": [{ "field": "feature", "op": "used", "value": "tools" }], "then": { "type": "model", "model": m(1) }, "else": { "type": "model", "model": m(9) } }),
            "route.else.model",
        ),
        (
            json!({ "type": "split", "by": "request", "branches": [{ "weight": 99, "step": { "type": "model", "model": m(1) } }] }),
            "route.branches",
        ),
        (
            json!({ "type": "split", "by": "request", "branches": [{ "weight": 50, "step": { "type": "model", "model": m(1) } }, { "weight": 40, "step": { "type": "model", "model": m(2) } }] }),
            "route.branches",
        ),
        (
            json!({ "type": "split", "by": "request", "branches": [{ "weight": 100, "step": { "type": "model", "model": m(1) } }, { "weight": 0, "step": { "type": "model", "model": m(2) } }] }),
            "route.branches.0.weight",
        ),
        (
            json!({ "type": "split", "by": "request", "branches": [{ "weight": 50, "step": { "type": "model", "model": m(1) } }, { "weight": 50, "step": { "type": "model", "model": m(9) } }] }),
            "route.branches.1.step.model",
        ),
    ];
    for (value, param) in cases {
        let err = route(value.clone()).validate(&known).unwrap_err();
        assert_eq!(err.param, param, "{value}");
        assert!(
            err.message.ends_with('.') && err.message.len() <= 90,
            "{}",
            err.message
        );
    }
}

#[test]
fn a_route_is_capped_in_size_and_depth() {
    let mut chain = Step::model(id(1));
    for _ in 0..11 {
        chain = Step::Model(ModelStep {
            fallback: Some(Box::new(chain)),
            ..match Step::model(id(1)) {
                Step::Model(m) => m,
                _ => unreachable!(),
            }
        });
    }
    assert_eq!(chain.validate(&|_| true), Ok(()));
    let deeper = Step::Model(ModelStep {
        fallback: Some(Box::new(chain)),
        ..match Step::model(id(1)) {
            Step::Model(m) => m,
            _ => unreachable!(),
        }
    });
    let err = deeper.validate(&|_| true).unwrap_err();
    assert_eq!(err.message, "A route is at most 12 steps deep.");
    let wide = Step::Split(SplitStep {
        by: SplitBy::Request,
        header: None,
        branches: (0..4)
            .map(|_| Branch {
                weight: 25,
                step: Step::Split(SplitStep {
                    by: SplitBy::Request,
                    header: None,
                    branches: (0..8)
                        .map(|i| Branch {
                            weight: if i == 0 { 30 } else { 10 },
                            step: Step::model(id(1)),
                        })
                        .collect(),
                }),
            })
            .collect(),
    });
    let err = wide.validate(&|_| true).unwrap_err();
    assert_eq!(err.message, "A route has at most 32 steps.");
}

#[test]
fn a_fallback_chain_plans_every_step_in_order() {
    let step = route(json!({
        "type": "model", "model": m(1),
        "fallback": { "type": "model", "model": m(2), "fallback": { "type": "model", "model": m(3) } }
    }));
    assert_eq!(
        planned(&step, &World::new()),
        [
            (id(1), "route".to_owned()),
            (id(2), "route.fallback".to_owned()),
            (id(3), "route.fallback.fallback".to_owned())
        ]
    );
}

#[test]
fn conditions_branch_on_headers_formats_features_tokens_and_callers() {
    let with = |rule: serde_json::Value| {
        route(json!({
            "type": "condition", "match": "all", "rules": [rule],
            "then": { "type": "model", "model": m(1) },
            "else": { "type": "model", "model": m(2) }
        }))
    };
    let took = |step: &Step, w: &World| planned(step, w)[0].0;
    let mut w = World::new();
    let paid =
        with(json!({ "field": "header", "name": "x-plan", "op": "equals", "value": "paid" }));
    assert_eq!(took(&paid, &w), id(2));
    w.headers.push("X-Plan", "paid");
    assert_eq!(took(&paid, &w), id(1));
    let not_paid =
        with(json!({ "field": "header", "name": "x-plan", "op": "not_equals", "value": "paid" }));
    assert_eq!(took(&not_paid, &w), id(2));
    let contains =
        with(json!({ "field": "header", "name": "x-plan", "op": "contains", "value": "ai" }));
    assert_eq!(took(&contains, &w), id(1));
    let present = with(json!({ "field": "header", "name": "x-plan", "op": "present" }));
    let absent = with(json!({ "field": "header", "name": "x-team", "op": "absent" }));
    assert_eq!((took(&present, &w), took(&absent, &w)), (id(1), id(1)));

    let anthropic = with(json!({ "field": "format", "op": "in", "value": ["anthropic_messages"] }));
    assert_eq!(took(&anthropic, &w), id(2));
    w.format = AppFormat::AnthropicMessages;
    assert_eq!(took(&anthropic, &w), id(1));

    let images = with(json!({ "field": "feature", "op": "used", "value": "images" }));
    let no_tools = with(json!({ "field": "feature", "op": "not_used", "value": "tools" }));
    assert_eq!((took(&images, &w), took(&no_tools, &w)), (id(2), id(1)));
    w.uses.insert(Feature::Images);
    assert_eq!(took(&images, &w), id(1));

    let long = with(json!({ "field": "input_tokens", "op": "at_least", "value": 100000 }));
    let short = with(json!({ "field": "input_tokens", "op": "below", "value": 100000 }));
    assert_eq!((took(&long, &w), took(&short, &w)), (id(2), id(1)));
    w.input_tokens = 100_000;
    assert_eq!((took(&long, &w), took(&short, &w)), (id(1), id(2)));

    let key = ApiKeyId::from_uuid(Uuid::from_u128(5));
    let keyed = with(json!({ "field": "key", "op": "in", "value": [key.to_string()] }));
    let others = with(json!({ "field": "key", "op": "not_in", "value": [key.to_string()] }));
    assert_eq!((took(&keyed, &w), took(&others, &w)), (id(2), id(1)));
    w.key = Some(key);
    assert_eq!((took(&keyed, &w), took(&others, &w)), (id(1), id(2)));
}

#[test]
fn any_needs_one_rule_and_all_needs_every_rule() {
    let both = |matches: &str| {
        route(json!({
            "type": "condition", "match": matches,
            "rules": [
                { "field": "header", "name": "x-plan", "op": "equals", "value": "paid" },
                { "field": "feature", "op": "used", "value": "tools" }
            ],
            "then": { "type": "model", "model": m(1) },
            "else": { "type": "model", "model": m(2) }
        }))
    };
    let mut w = World::new();
    w.headers.push("x-plan", "paid");
    assert_eq!(planned(&both("all"), &w)[0].0, id(2));
    assert_eq!(planned(&both("any"), &w)[0].0, id(1));
}

#[test]
fn a_split_follows_its_weights_and_keeps_a_caller_on_one_branch() {
    let split = |by: &str| {
        route(json!({
            "type": "split", "by": by,
            "branches": [
                { "weight": 80, "step": { "type": "model", "model": m(1) } },
                { "weight": 20, "step": { "type": "model", "model": m(2) } }
            ]
        }))
    };
    let per_request = split("request");
    let mut w = World::new();
    let mut second = 0;
    for n in 0..10_000u128 {
        w.request = Uuid::from_u128(n);
        if planned(&per_request, &w)[0].0 == id(2) {
            second += 1;
        }
    }
    assert!((1_700..2_300).contains(&second), "{second}");

    let per_caller = split("caller");
    w.caller = Some(Uuid::from_u128(77));
    let first = planned(&per_caller, &w)[0].clone();
    for n in 0..200u128 {
        w.request = Uuid::from_u128(n);
        assert_eq!(planned(&per_caller, &w)[0], first);
    }
    assert!(first.1.starts_with("route.branches."));
    w.caller = None;
    let mut seen = BTreeSet::new();
    for n in 0..200u128 {
        w.request = Uuid::from_u128(n);
        seen.insert(planned(&per_caller, &w)[0].0);
    }
    assert_eq!(
        seen.len(),
        2,
        "without a caller, a caller split draws per request"
    );
}

#[test]
fn a_split_by_header_keeps_each_value_on_one_branch() {
    let step = route(json!({
        "type": "split", "by": "header", "header": "x-user-id",
        "branches": [
            { "weight": 50, "step": { "type": "model", "model": m(1) } },
            { "weight": 50, "step": { "type": "model", "model": m(2) } }
        ]
    }));
    assert_eq!(step.validate(&|_| true), Ok(()));
    assert_eq!(
        serde_json::to_value(&step).unwrap()["header"],
        json!("x-user-id")
    );
    let mut w = World::new();
    let mut seen = BTreeSet::new();
    for user in 0..200u32 {
        w.headers = HeaderList::new().with("X-User-Id", format!("user-{user}"));
        let first = planned(&step, &w)[0].clone();
        for n in 0..20u128 {
            w.request = Uuid::from_u128(n);
            assert_eq!(planned(&step, &w)[0], first, "user-{user}");
        }
        seen.insert(first.0);
    }
    assert_eq!(seen.len(), 2, "different values land on both branches");

    w.headers = HeaderList::new();
    let mut drawn = BTreeSet::new();
    for n in 0..200u128 {
        w.request = Uuid::from_u128(n);
        drawn.insert(planned(&step, &w)[0].0);
    }
    assert_eq!(
        drawn.len(),
        2,
        "without the header, requests are drawn anew"
    );
}

#[test]
fn only_a_split_by_header_names_a_header_and_never_a_secret_one() {
    let split = |by: &str, header: serde_json::Value| {
        route(json!({
            "type": "split", "by": by, "header": header,
            "branches": [
                { "weight": 50, "step": { "type": "model", "model": m(1) } },
                { "weight": 50, "step": { "type": "model", "model": m(2) } }
            ]
        }))
    };
    let refused = |step: Step| {
        let e = step.validate(&|_| true).unwrap_err();
        (e.param, e.message)
    };
    assert_eq!(
        refused(split("header", json!(null))),
        (
            "route.header".to_owned(),
            "Name the header this split reads.".to_owned()
        )
    );
    assert_eq!(
        refused(split("caller", json!("x-user-id"))),
        (
            "route.header".to_owned(),
            "Only a split by header reads a header; remove `header`.".to_owned()
        )
    );
    assert_eq!(
        refused(split("header", json!("Authorization"))).1,
        "A header name is a lowercase HTTP token of up to 100 characters."
    );
    assert_eq!(
        refused(split("header", json!("cookie"))).1,
        "Routes can't read this header, since it carries credentials."
    );
    assert!(
        serde_json::to_value(split("request", json!(null)))
            .unwrap()
            .get("header")
            .is_none()
    );
}

#[test]
fn a_trace_names_every_decision_on_the_way_to_each_model() {
    let step = route(json!({
        "type": "condition", "match": "any",
        "rules": [
            { "field": "header", "name": "x-plan", "op": "equals", "value": "enterprise" },
            { "field": "input_tokens", "op": "at_least", "value": 100000 }
        ],
        "then": { "type": "model", "model": m(1), "fallback": { "type": "model", "model": m(2) } },
        "else": {
            "type": "split", "by": "request",
            "branches": [
                { "weight": 60, "step": { "type": "model", "model": m(3) } },
                { "weight": 40, "step": { "type": "model", "model": m(4) } }
            ]
        }
    }));
    let mut w = World::new();
    w.headers = HeaderList::new().with("x-plan", "enterprise");
    let traced = step.trace(&w.facts());
    assert!(matches!(
        traced.as_slice(),
        [
            Traced::Condition { path, rules, holds: true },
            Traced::Model { path: first, .. },
            Traced::Model { path: second, .. },
        ] if path == "route" && rules == &[true, false] && first == "route.then" && second == "route.then.fallback"
    ));

    w.headers = HeaderList::new();
    for n in 0..50u128 {
        w.request = Uuid::from_u128(n);
        let traced = step.trace(&w.facts());
        let [
            Traced::Condition {
                holds: false,
                rules,
                ..
            },
            Traced::Split {
                path,
                branch,
                weight,
                draw,
            },
            Traced::Model {
                path: model,
                step: chosen,
            },
        ] = traced.as_slice()
        else {
            panic!("{traced:?}");
        };
        assert_eq!(rules, &[false, false]);
        assert_eq!(path, "route.else");
        assert!(*draw < 100);
        assert_eq!(*branch, usize::from(*draw >= 60));
        assert_eq!(*weight, if *branch == 0 { 60 } else { 40 });
        assert_eq!(model, &format!("route.else.branches.{branch}.step"));
        assert_eq!(chosen.model, if *branch == 0 { id(3) } else { id(4) });
        assert_eq!(planned(&step, &w), [(chosen.model, model.clone())]);
    }
}

#[test]
fn a_split_turns_a_draw_into_a_branch_by_its_weights() {
    let Step::Split(split) = route(json!({
        "type": "split", "by": "request",
        "branches": [
            { "weight": 10, "step": { "type": "model", "model": m(1) } },
            { "weight": 30, "step": { "type": "model", "model": m(2) } },
            { "weight": 60, "step": { "type": "model", "model": m(3) } }
        ]
    })) else {
        unreachable!()
    };
    let branches: Vec<usize> = [0, 9, 10, 39, 40, 99]
        .into_iter()
        .map(|d| split.branch_for(d))
        .collect();
    assert_eq!(branches, [0, 0, 1, 1, 2, 2]);
}

#[test]
fn a_fallback_can_branch_too() {
    let step = route(json!({
        "type": "model", "model": m(1),
        "fallback": {
            "type": "condition", "match": "all",
            "rules": [{ "field": "feature", "op": "used", "value": "images" }],
            "then": { "type": "model", "model": m(2) },
            "else": { "type": "model", "model": m(3), "fallback": { "type": "model", "model": m(4) } }
        }
    }));
    let mut w = World::new();
    assert_eq!(
        planned(&step, &w).iter().map(|p| p.0).collect::<Vec<_>>(),
        [id(1), id(3), id(4)]
    );
    w.uses.insert(Feature::Images);
    assert_eq!(
        planned(&step, &w),
        [
            (id(1), "route".to_owned()),
            (id(2), "route.fallback.then".to_owned())
        ]
    );
}

#[test]
fn header_names_follow_http_tokens() {
    for good in ["x-plan", "x_team.1", "kaeso-route"] {
        assert!(valid_header_name(good), "{good}");
    }
    for bad in ["", "X-Plan", "x plan", "x:plan", &"x".repeat(101)] {
        assert!(!valid_header_name(bad), "{bad}");
    }
}

#[test]
fn features_come_from_what_the_request_uses() {
    use kaeso_kpp::{
        Block, FunctionTool, GenerateRequest, Message, Request, Role, Source, ToolDef,
    };
    let user = |blocks| GenerateRequest::new("fast").with_message(Message::new(Role::User, blocks));
    let plain = Request::Generate(user(vec![Block::text("hi")]));
    assert!(features_of(&plain).is_empty());
    let image = Request::Generate(
        user(vec![Block::image(Source::base64("image/png", "AAAA"))])
            .with_tool(ToolDef::Function(FunctionTool::new("f", None))),
    );
    assert_eq!(
        features_of(&image),
        BTreeSet::from([Feature::Images, Feature::Tools])
    );
    let counted = Request::CountTokens(user(vec![Block::image(Source::base64(
        "image/png",
        "AAAA",
    ))]));
    assert_eq!(features_of(&counted), BTreeSet::from([Feature::Images]));
}
