#![cfg(test)]

use kaeso_codecs::openai_decisions::DecisionsProfile;
use kaeso_codecs::system_one::SystemOneProfile;
use kaeso_codecs::{EgressCtx, EncodeCtx, HeaderList, dispatch};
use kaeso_kpp::{
    Answer, AnswerValue, Block, Choice, ChoiceAnswer, ChoiceOption, ChoiceValue, DecideRequest,
    DecideResponse, DecisionInput, Description, DialectId, FunctionTool, GenerateRequest,
    Instructions, Level, Message, Predicate, PredicateAnswer, Question, QuestionKind, RawJson,
    Reply, Request, RequestKind, Response, Role, Score, ScoreAnswer, StopReason, StreamMode,
    ToolDef, Usage,
};
use proptest::prelude::*;

const GENERATION: [DialectId; 4] = [
    DialectId::AnthropicMessages,
    DialectId::OpenAiChat,
    DialectId::OpenAiResponses,
    DialectId::Gemini,
];

const DECISIONS: [DialectId; 3] = [
    DialectId::SystemOne,
    DialectId::OpenAiDecisions,
    DialectId::GeminiDecisions,
];

fn text() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9 .,!?'\"\\\\é✓\n]{1,40}"
}

fn args() -> impl Strategy<Value = RawJson> {
    (text(), any::<i32>(), any::<bool>()).prop_map(|(s, n, b)| {
        RawJson::from_serialize(&serde_json::json!({"q": s, "n": n, "flag": b})).unwrap()
    })
}

#[derive(Clone, Debug)]
enum Turn {
    Text(String, String),
    Tool(String, RawJson, String),
}

fn turn() -> impl Strategy<Value = Turn> {
    prop_oneof![
        (text(), text()).prop_map(|(q, a)| Turn::Text(q, a)),
        (text(), args(), text()).prop_map(|(q, a, r)| Turn::Tool(q, a, r)),
    ]
}

prop_compose! {
    fn request()(
        system in proptest::option::of(text()),
        turns in proptest::collection::vec(turn(), 1..6),
        last in text(),
        max_tokens in 16u32..4096,
        temperature in proptest::option::of(0.0f64..1.0),
    ) -> GenerateRequest {
        let schema = RawJson::from_string(r#"{"type":"object","properties":{"q":{"type":"string"}}}"#.into()).unwrap();
        let mut req = GenerateRequest::new("model-x").with_tool(ToolDef::Function(FunctionTool::new("lookup", Some(schema))));
        if let Some(s) = system {
            req = req.with_system(s);
        }
        for (i, t) in turns.into_iter().enumerate() {
            match t {
                Turn::Text(q, a) => {
                    req.messages.push(Message::user(q));
                    req.messages.push(Message::assistant(a));
                }
                Turn::Tool(q, a, r) => {
                    let id = format!("call_{i}");
                    req.messages.push(Message::user(q));
                    req.messages.push(Message::new(Role::Assistant, vec![Block::tool_call(&id, "lookup", a)]));
                    req.messages.push(Message::new(Role::User, vec![Block::tool_result(&id, vec![Block::text(r)], false)]));
                }
            }
        }
        req.messages.push(Message::user(last));
        req.sampling.max_output_tokens = Some(max_tokens);
        req.sampling.temperature = temperature;
        req
    }
}

fn essence(req: &GenerateRequest) -> serde_json::Value {
    let mut turns = Vec::new();
    for m in &req.messages {
        for b in &m.blocks {
            match b {
                Block::Text(t) => turns.push(serde_json::json!([m.role.as_str(), "text", t.text])),
                Block::ToolCall(c) => turns.push(serde_json::json!([
                    m.role.as_str(),
                    "call",
                    c.id,
                    c.name,
                    c.arguments.to_value().unwrap()
                ])),
                Block::ToolResult(r) => {
                    let text: String = r
                        .content
                        .iter()
                        .filter_map(|b| match b {
                            Block::Text(t) => Some(t.text.as_str()),
                            _ => None,
                        })
                        .collect();
                    turns.push(serde_json::json!([
                        m.role.as_str(),
                        "result",
                        r.call_id,
                        text
                    ]));
                }
                other => turns.push(serde_json::json!([m.role.as_str(), other.kind()])),
            }
        }
    }
    let system: String = req
        .system
        .iter()
        .filter_map(|b| match b {
            Block::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect();
    serde_json::json!({
        "model": req.model,
        "system": system,
        "turns": turns,
        "max": req.sampling.max_output_tokens,
        "temperature": req.sampling.temperature,
        "tools": req.tools.iter().map(|t| match t { ToolDef::Function(f) => f.name.clone(), _ => String::new() }).collect::<Vec<_>>(),
    })
}

fn round_trip(dialect: DialectId, req: &GenerateRequest) -> GenerateRequest {
    let request = Request::Generate(req.clone());
    dispatch::check(dialect, &request, &EgressCtx::new(&request))
        .unwrap_or_else(|e| panic!("{dialect}: {e}"));
    let mut ctx = EgressCtx::new(&request);
    let enc = dispatch::encode_request(dialect, &request, &mut ctx).unwrap();
    let back = if dialect == DialectId::Gemini {
        let segment = enc.path.rsplit('/').next().unwrap();
        let (model, action) = kaeso_codecs::gemini::split_model_action(segment).unwrap();
        dispatch::decode_gemini_request(&model, action, &enc.body, &HeaderList::new())
    } else {
        dispatch::decode_request(
            dialect,
            RequestKind::Generate,
            &enc.body,
            &HeaderList::new(),
        )
    }
    .unwrap();
    let Request::Generate(back) = back else {
        panic!("kind changed")
    };
    back
}

#[test]
fn a_temperature_with_seventeen_significant_digits_survives_every_dialect() {
    let schema =
        RawJson::from_string(r#"{"type":"object","properties":{"q":{"type":"string"}}}"#.into())
            .unwrap();
    let mut req = GenerateRequest::new("model-x")
        .with_tool(ToolDef::Function(FunctionTool::new("lookup", Some(schema))));
    req.messages.push(Message::user(" "));
    req.messages.push(Message::assistant("é"));
    req.messages.push(Message::user("?"));
    req.sampling.max_output_tokens = Some(16);
    req.sampling.temperature = Some(0.950_729_014_766_991_7);
    for d in GENERATION {
        let back = round_trip(d, &req);
        assert_eq!(back.sampling.temperature, req.sampling.temperature, "{d}");
        assert_eq!(essence(&back), essence(&req), "{d}");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::default())]

    #[test]
    fn requests_survive_every_dialect(req in request()) {
        for d in GENERATION {
            prop_assert_eq!(essence(&round_trip(d, &req)), essence(&req), "dialect {}", d);
        }
    }

    #[test]
    fn replies_survive_every_dialect(
        texts in proptest::collection::vec(text(), 1..3),
        call in proptest::option::of(args()),
        input in 0u64..100_000,
        output in 0u64..10_000,
    ) {
        let mut blocks: Vec<Block> = texts.iter().map(Block::text).collect();
        if let Some(a) = call.clone() {
            blocks.push(Block::tool_call("call_1", "lookup", a));
        }
        let mut resp = Response::new("resp_1", "model-x", blocks);
        resp.stop = if call.is_some() { StopReason::ToolUse } else { StopReason::EndTurn };
        resp.usage = Usage::new(input, output);
        let reply = Reply::Generate(resp);
        for d in GENERATION {
            let enc_ctx = EncodeCtx::new(RequestKind::Generate, StreamMode::Off).with_created_at(1);
            let out = dispatch::encode_response(d, &reply, &enc_ctx).unwrap();
            let ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off);
            let Reply::Generate(back) = dispatch::decode_response(d, 200, &HeaderList::new(), &out, &ctx).unwrap() else {
                panic!("kind changed")
            };
            let text: String = back.blocks.iter().filter_map(|b| match b { Block::Text(t) => Some(t.text.as_str()), _ => None }).collect();
            prop_assert_eq!(text, texts.concat(), "dialect {}", d);
            let calls: Vec<_> = back.blocks.iter().filter_map(|b| match b { Block::ToolCall(c) => Some(c.arguments.to_value().unwrap()), _ => None }).collect();
            prop_assert_eq!(calls, call.iter().map(|a| a.to_value().unwrap()).collect::<Vec<_>>(), "dialect {}", d);
            prop_assert_eq!(back.usage.input_tokens, input, "dialect {}", d);
            prop_assert_eq!(back.usage.output_tokens, output, "dialect {}", d);
        }
    }

    #[test]
    fn decision_requests_survive_every_decision_format(req in decide_request()) {
        for d in DECISIONS {
            let (_, back) = decide_round_trip(d, &req);
            prop_assert_eq!(decide_essence(&back), decide_essence(&req), "dialect {}", d);
        }
    }

    #[test]
    fn decision_replies_survive_every_decision_format(
        (req, reply) in decide_request().prop_flat_map(|req| {
            let answers: Vec<_> = req.questions.iter().map(answer_for).collect();
            (Just(req), answers, 0u64..100_000)
        }).prop_map(|(req, answers, input)| {
            let mut reply = DecideResponse::new(req.model.clone(), answers);
            reply.usage = Usage::new(input, 0);
            (req, reply)
        })
    ) {
        let reply = Reply::Decide(reply);
        for d in DECISIONS {
            let (ctx, app) = decide_round_trip(d, &req);
            let out = dispatch::encode_response(d, &reply, &EncodeCtx::for_request(Request::Decide(app)))
                .unwrap_or_else(|e| panic!("{d}: {e}"));
            let back = dispatch::decode_response(d, 200, &HeaderList::new(), &out, &ctx)
                .unwrap_or_else(|e| panic!("{d}: {e}"));
            let (Reply::Decide(want), Reply::Decide(got)) = (&reply, &back) else {
                panic!("kind changed")
            };
            prop_assert_eq!(&got.model, &want.model, "dialect {}", d);
            prop_assert_eq!(answers_essence(got), answers_essence(want), "dialect {}", d);
            prop_assert_eq!(got.usage.total_input_tokens(), want.usage.input_tokens, "dialect {}", d);
        }
    }
}

// The questions every decision format can say: named by Gemini's key rule, predicates without
// criteria, which OpenAI's format has no place for, and 2 to 10 options or levels.
fn decide_kind() -> impl Strategy<Value = QuestionKind> {
    prop_oneof![
        Just(QuestionKind::Predicate(Predicate::default())),
        proptest::collection::vec(
            ("[a-z][a-z0-9_]{0,8}", proptest::option::of(text())),
            2..=10
        )
        .prop_map(|options| {
            QuestionKind::Choice(Choice::new(
                options
                    .into_iter()
                    .enumerate()
                    .map(|(i, (key, d))| {
                        ChoiceOption::new(
                            ChoiceValue::Text(format!("{key}{i}")),
                            d.map(Description::Text),
                        )
                    })
                    .collect(),
            ))
        }),
        proptest::collection::vec(text(), 2..=10).prop_map(|levels| {
            QuestionKind::Score(Score::new(
                levels
                    .into_iter()
                    .map(|l| Level::new(None, Some(Description::Text(l))))
                    .collect(),
            ))
        }),
    ]
}

prop_compose! {
    fn decide_request()(
        input in text(),
        questions in proptest::collection::vec(("[a-z][a-z0-9_-]{0,20}", text(), decide_kind()), 1..6),
    ) -> DecideRequest {
        let mut req = DecideRequest::new("decider-x", DecisionInput::Text(input));
        for (i, (name, instructions, kind)) in questions.into_iter().enumerate() {
            req.questions.push(
                Question::new(Some(format!("{name}{i}")), kind)
                    .with_instructions(Instructions::Text(instructions)),
            );
        }
        req
    }
}

// Probabilities as a provider sends them: not normalised, some options or levels left out.
fn answer_for(q: &Question) -> BoxedStrategy<Answer> {
    let probabilities = |n: usize| proptest::collection::vec(proptest::option::of(0.0f64..1.0), n);
    match &q.kind {
        QuestionKind::Choice(c) => {
            let n = c.options.len();
            (0..n, probabilities(n))
                .prop_map(|(option, mut ps)| {
                    if let Some(slot) = ps.get_mut(option) {
                        slot.get_or_insert(0.5);
                    }
                    Answer::new(AnswerValue::Choice(ChoiceAnswer::new(option, ps)))
                })
                .boxed()
        }
        QuestionKind::Score(s) => {
            let n = s.levels.len();
            (0.0f64..9.0, probabilities(n))
                .prop_map(|(score, ps)| {
                    Answer::new(AnswerValue::Score(ScoreAnswer::new(score, ps)))
                })
                .boxed()
        }
        _ => (0.01f64..0.99)
            .prop_map(|p| Answer::new(AnswerValue::Predicate(PredicateAnswer::new(p))))
            .boxed(),
    }
}

fn decide_egress(req: &Request) -> EgressCtx {
    EgressCtx::new(req)
        .with_system_one_profile(SystemOneProfile::GENERIC)
        .with_decisions_profile(DecisionsProfile::GENERIC)
}

fn decide_round_trip(dialect: DialectId, req: &DecideRequest) -> (EgressCtx, DecideRequest) {
    let request = Request::Decide(req.clone());
    dispatch::check(dialect, &request, &decide_egress(&request))
        .unwrap_or_else(|e| panic!("{dialect}: {e}"));
    let mut ctx = decide_egress(&request);
    let enc = dispatch::encode_request(dialect, &request, &mut ctx).unwrap();
    let back = dispatch::decode_request(dialect, RequestKind::Decide, &enc.body, &enc.headers)
        .unwrap_or_else(|e| panic!("{dialect}: {e}"));
    let Request::Decide(back) = back else {
        panic!("kind changed")
    };
    (ctx, back)
}

fn description(d: Option<&Description>) -> String {
    match d {
        Some(Description::Text(t)) => t.clone(),
        Some(Description::Json(j)) => j.get().to_owned(),
        _ => String::new(),
    }
}

fn decide_essence(req: &DecideRequest) -> serde_json::Value {
    let questions: Vec<serde_json::Value> = req
        .questions
        .iter()
        .map(|q| {
            let instructions = match &q.instructions {
                Some(Instructions::Text(t)) => t.clone(),
                other => format!("{other:?}"),
            };
            let kind = match &q.kind {
                QuestionKind::Predicate(_) => serde_json::json!("predicate"),
                QuestionKind::Choice(c) => serde_json::json!(
                    c.options
                        .iter()
                        .map(|o| {
                            let ChoiceValue::Text(v) = &o.value else {
                                panic!("a boolean value")
                            };
                            (v.clone(), description(o.description.as_ref()))
                        })
                        .collect::<Vec<_>>()
                ),
                QuestionKind::Score(s) => serde_json::json!(
                    s.levels
                        .iter()
                        .map(|l| l
                            .label
                            .clone()
                            .unwrap_or_else(|| description(l.description.as_ref())))
                        .collect::<Vec<_>>()
                ),
                other => serde_json::json!(other.as_str()),
            };
            serde_json::json!([q.name, instructions, kind])
        })
        .collect();
    let DecisionInput::Text(input) = &req.input else {
        panic!("the input changed form")
    };
    serde_json::json!({"model": req.model, "input": input, "questions": questions})
}

fn answers_essence(r: &DecideResponse) -> serde_json::Value {
    serde_json::json!(
        r.answers
            .iter()
            .map(|a| match &a.value {
                AnswerValue::Predicate(p) => serde_json::json!(["predicate", p.probability]),
                AnswerValue::Choice(c) => serde_json::json!(["choice", c.option, c.probabilities]),
                AnswerValue::Score(s) => serde_json::json!(["score", s.score, s.probabilities]),
                other => serde_json::json!([other.as_str()]),
            })
            .collect::<Vec<_>>()
    )
}
