#![cfg(test)]

//! Every decision fixture of one format, crossed to each other decision format: a request must
//! keep its questions, options and input, a reply its answers, probabilities and refusals, or
//! be refused by name. The outcomes are pinned as snapshots.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use kaeso_codecs::openai_decisions::DecisionsProfile;
use kaeso_codecs::system_one::SystemOneProfile;
use kaeso_codecs::{EgressCtx, EncodeCtx, EncodedRequest, HeaderList, dispatch};
use kaeso_kpp::{
    AnswerValue, Block, ChoiceValue, DecideRequest, DecisionInput, Description, DialectId,
    Instructions, KppError, QuestionKind, Reply, Request, RequestKind,
};
use serde_json::{Value, json};

const DECISIONS: [DialectId; 3] = [
    DialectId::SystemOne,
    DialectId::OpenAiDecisions,
    DialectId::GeminiDecisions,
];

const PROVIDERS: &str = "providers";

fn fixtures_dir(dialect: DialectId) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(dialect.as_str())
}

fn files(dir: &Path, prefix: &str) -> Vec<(String, Vec<u8>)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("json"))
        .filter(|p| {
            let n = p.file_name().unwrap().to_str().unwrap();
            !n.ends_with(".headers.json") && !n.ends_with(".meta.json")
        })
        .map(|p| {
            (
                format!("{prefix}{}", p.file_stem().unwrap().to_str().unwrap()),
                std::fs::read(&p).unwrap(),
            )
        })
        .collect()
}

fn fixtures(dialect: DialectId, folder: &str) -> Vec<(String, Vec<u8>)> {
    let mut out = files(&fixtures_dir(dialect).join(folder), "");
    let providers = fixtures_dir(dialect).join(PROVIDERS);
    if providers.is_dir() {
        for provider in std::fs::read_dir(&providers).unwrap() {
            let provider = provider.unwrap().path();
            let id = provider.file_name().unwrap().to_str().unwrap().to_owned();
            out.extend(files(&provider.join(folder), &format!("{PROVIDERS}/{id}/")));
        }
    }
    out.sort();
    assert!(!out.is_empty(), "no {folder} fixtures for {dialect}");
    out
}

fn path_of(dialect: DialectId, folder: &str, name: &str, suffix: &str) -> PathBuf {
    match name
        .strip_prefix(PROVIDERS)
        .and_then(|rest| rest.strip_prefix('/'))
        .and_then(|rest| rest.split_once('/'))
    {
        Some((id, stem)) => fixtures_dir(dialect)
            .join(PROVIDERS)
            .join(id)
            .join(folder)
            .join(format!("{stem}{suffix}")),
        None => fixtures_dir(dialect)
            .join(folder)
            .join(format!("{name}{suffix}")),
    }
}

fn headers(dialect: DialectId, name: &str) -> HeaderList {
    match std::fs::read(path_of(dialect, "requests", name, ".headers.json")) {
        Ok(b) => {
            let v: Value = serde_json::from_slice(&b).unwrap();
            v.as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned()))
                .collect()
        }
        Err(_) => HeaderList::new(),
    }
}

fn decode(dialect: DialectId, body: &[u8], headers: &HeaderList) -> Result<Request, KppError> {
    dispatch::decode_request(dialect, RequestKind::Decide, body, headers)
}

// Providers of every format with the profiles that check no provider's own limits, as a custom
// provider has them.
fn egress(req: &Request, system_one: SystemOneProfile) -> EgressCtx {
    EgressCtx::new(req)
        .with_system_one_profile(system_one)
        .with_decisions_profile(DecisionsProfile::GENERIC)
}

fn encode(dialect: DialectId, req: &Request) -> Result<(EncodedRequest, EgressCtx), KppError> {
    encode_to(dialect, req, SystemOneProfile::GENERIC)
}

fn encode_to(
    dialect: DialectId,
    req: &Request,
    system_one: SystemOneProfile,
) -> Result<(EncodedRequest, EgressCtx), KppError> {
    let mut ctx = egress(req, system_one);
    dispatch::check(dialect, req, &egress(req, system_one))?;
    let enc = dispatch::encode_request(dialect, req, &mut ctx)?;
    Ok((enc, ctx))
}

// A provider's reply is read with its own profile, such as Cloudflare's envelope.
fn profile_of(name: &str) -> SystemOneProfile {
    if name.starts_with("providers/cloudflare-ai-gateway/") {
        SystemOneProfile::CLOUDFLARE_WORKERS_AI
    } else {
        SystemOneProfile::GENERIC
    }
}

// A recorded reply answers the request of the same name without `recorded-`.
fn request_of(reply: &str) -> String {
    match reply.rsplit_once('/') {
        Some((folder, stem)) => format!(
            "{folder}/{}",
            stem.strip_prefix("recorded-").unwrap_or(stem)
        ),
        None => reply.strip_prefix("recorded-").unwrap_or(reply).to_owned(),
    }
}

fn pretty(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(bytes).into()))
}

fn refused(e: &KppError) -> Value {
    json!({"unsupported": e.unsupported_feature(), "kind": e.kind.as_str(), "message": e.message, "param": e.param})
}

fn snapshot_name(kind: &str, a: DialectId, b: DialectId) -> String {
    format!("{kind}__{}__to__{}", a.as_str(), b.as_str())
}

fn decided(req: &Request) -> &DecideRequest {
    match req {
        Request::Decide(r) => r,
        other => panic!("{other:?}"),
    }
}

// JSON as written; `text_meaning` drops its whitespace.
fn compact(raw: &kaeso_kpp::RawJson) -> String {
    raw.get().to_owned()
}

// Text as a model reads it, with asset references taken out (each counts as an image) and
// without what formats and separators add around it: whitespace, and the brackets, quotes,
// commas and escapes of a state list written as text or as parts.
fn text_meaning(text: &str, r: &DecideRequest, images: &mut usize) -> String {
    let mut text = text.to_owned();
    for reference in r.assets.keys() {
        *images += text.matches(reference.as_str()).count();
        text = text.replace(reference.as_str(), "");
    }
    text.replace("\\n", " ")
        .replace("\\t", " ")
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '[' | ']' | '"' | ',' | '\\'))
        .collect()
}

fn input_meaning(r: &DecideRequest) -> Value {
    let mut images = 0usize;
    let mut media = Vec::new();
    let text = match &r.input {
        DecisionInput::Text(t) => text_meaning(t, r, &mut images),
        DecisionInput::Json(raw) => text_meaning(&compact(raw), r, &mut images),
        DecisionInput::Messages(messages) => {
            let mut text = String::new();
            for m in messages {
                for b in &m.blocks {
                    match b {
                        Block::Text(t) => text.push_str(&text_meaning(&t.text, r, &mut images)),
                        Block::Json(j) => {
                            text.push_str(&text_meaning(&compact(&j.data), r, &mut images))
                        }
                        other => media.push(other.kind().to_owned()),
                    }
                }
            }
            text
        }
        _ => String::new(),
    };
    media.extend(std::iter::repeat_n("image".to_owned(), images));
    media.sort();
    json!({"text": text, "media": media})
}

fn option_text(value: &ChoiceValue) -> String {
    match value {
        ChoiceValue::Text(t) => t.clone(),
        ChoiceValue::Bool(b) => b.to_string(),
        _ => String::new(),
    }
}

fn question_meaning(r: &DecideRequest) -> Value {
    r.questions
        .iter()
        .map(|q| match &q.kind {
            QuestionKind::Choice(c) => json!({
                "type": "choice",
                "options": c.options.iter().map(|o| option_text(&o.value)).collect::<Vec<_>>(),
            }),
            QuestionKind::Score(s) => json!({"type": "score", "levels": s.levels.len()}),
            other => json!({"type": other.as_str()}),
        })
        .collect()
}

fn request_meaning(r: &DecideRequest) -> Value {
    json!({"model": r.model, "input": input_meaning(r), "questions": question_meaning(r)})
}

fn reply_meaning(reply: &Reply) -> Value {
    let Reply::Decide(r) = reply else {
        return Value::Null;
    };
    r.answers
        .iter()
        .map(|a| match &a.value {
            AnswerValue::Predicate(p) => json!({"predicate": p.probability}),
            AnswerValue::Choice(c) => {
                json!({"choice": c.option, "probabilities": c.probabilities})
            }
            AnswerValue::Score(s) => json!({"score": s.score, "probabilities": s.probabilities}),
            AnswerValue::Box(b) => json!({"box": [b.top, b.bottom, b.left, b.right]}),
            AnswerValue::Refusal(_) => json!("refusal"),
            _ => Value::Null,
        })
        .collect()
}

#[test]
fn decision_requests_cross_every_decision_format() {
    for a in DECISIONS {
        let reqs = fixtures(a, "requests");
        for b in DECISIONS.into_iter().filter(|b| *b != a) {
            let mut outcomes = BTreeMap::new();
            for (name, body) in &reqs {
                let req = match decode(a, body, &headers(a, name)) {
                    Ok(r) => r,
                    Err(e) => {
                        outcomes.insert(name.clone(), json!({"rejected_by_source": e.message}));
                        continue;
                    }
                };
                let outcome = match encode(b, &req) {
                    Ok((enc, ctx)) => {
                        let back = decode(b, &enc.body, &enc.headers).unwrap_or_else(|e| {
                            panic!("{a}->{b} {name}: target cannot decode its own output: {e}")
                        });
                        assert_eq!(
                            request_meaning(decided(&back)),
                            request_meaning(decided(&req)),
                            "{a}->{b} {name}: the request's meaning changed",
                        );
                        json!({
                            "path": enc.path,
                            "body": pretty(&enc.body),
                            "adapted": ctx.adapted,
                            "left_out": ctx.left_out,
                        })
                    }
                    Err(e) => {
                        assert!(
                            e.unsupported_feature().is_some(),
                            "{a}->{b} {name}: refused without a feature name: {e:?}"
                        );
                        refused(&e)
                    }
                };
                outcomes.insert(name.clone(), outcome);
            }
            insta::assert_json_snapshot!(snapshot_name("request", a, b), outcomes);
        }
    }
}

#[test]
fn decision_replies_cross_every_decision_format() {
    for a in DECISIONS {
        let bodies = fixtures(a, "responses");
        for b in DECISIONS.into_iter().filter(|b| *b != a) {
            let mut outcomes = BTreeMap::new();
            for (name, body) in &bodies {
                let asked = request_of(name);
                let request = std::fs::read(path_of(a, "requests", &asked, ".json"))
                    .unwrap_or_else(|_| panic!("{a} {name}: a reply needs its request"));
                let req = decode(a, &request, &headers(a, &asked))
                    .unwrap_or_else(|e| panic!("{a} {name}: {e}"));
                let (_, ctx_a) = encode_to(a, &req, profile_of(name))
                    .unwrap_or_else(|e| panic!("{a} {name}: {e}"));
                let reply = match dispatch::decode_response(
                    a,
                    200,
                    &HeaderList::new(),
                    body,
                    &ctx_a,
                ) {
                    Ok(r) => r,
                    Err(e) => {
                        let out = dispatch::encode_error(b, &e);
                        outcomes.insert(
                            name.clone(),
                            json!({"error": e.kind.as_str(), "status": out.status, "body": pretty(&out.body)}),
                        );
                        continue;
                    }
                };
                // The app of format B sent the request as B writes it.
                let app = match encode(b, &req) {
                    Ok((enc, _)) => decode(b, &enc.body, &enc.headers)
                        .unwrap_or_else(|e| panic!("{a}->{b} {name}: {e}")),
                    Err(e) => {
                        outcomes.insert(
                            name.clone(),
                            json!({"request_refused": e.unsupported_feature()}),
                        );
                        continue;
                    }
                };
                let outcome = match dispatch::encode_response(
                    b,
                    &reply,
                    &EncodeCtx::for_request(app.clone()),
                ) {
                    Ok(out) => {
                        let (_, ctx_b) =
                            encode(b, &app).unwrap_or_else(|e| panic!("{a}->{b} {name}: {e}"));
                        let back =
                            dispatch::decode_response(b, 200, &HeaderList::new(), &out, &ctx_b)
                                .unwrap_or_else(|e| {
                                    panic!(
                                        "{a}->{b} {name}: target cannot decode its own reply: {e}"
                                    )
                                });
                        assert_eq!(
                            reply_meaning(&back),
                            reply_meaning(&reply),
                            "{a}->{b} {name}: answers changed",
                        );
                        pretty(&out)
                    }
                    Err(e) => {
                        let out = dispatch::encode_error(b, &e);
                        json!({"error": e.kind.as_str(), "status": out.status, "body": pretty(&out.body)})
                    }
                };
                outcomes.insert(name.clone(), outcome);
            }
            insta::assert_json_snapshot!(snapshot_name("response", a, b), outcomes);
        }
    }
}

#[test]
fn levels_and_descriptions_keep_their_text_across_formats() {
    let body = json!({
        "model": "gemini-decision-1",
        "input": "x",
        "questions": {"s": {"type": "score", "instructions": "How?", "criteria": ["Low", "High"]}}
    });
    let req = decode(
        DialectId::GeminiDecisions,
        &serde_json::to_vec(&body).unwrap(),
        &HeaderList::new(),
    )
    .unwrap();
    for b in [DialectId::SystemOne, DialectId::OpenAiDecisions] {
        let (enc, _) = encode(b, &req).unwrap();
        let back = decode(b, &enc.body, &enc.headers).unwrap();
        let QuestionKind::Score(s) = &decided(&back).questions[0].kind else {
            panic!()
        };
        let texts: Vec<String> = s
            .levels
            .iter()
            .map(|l| match (&l.label, &l.description) {
                (Some(label), _) => label.clone(),
                (None, Some(Description::Text(t))) => t.clone(),
                _ => String::new(),
            })
            .collect();
        assert_eq!(texts, ["Low", "High"], "{b}");
        assert!(matches!(
            decided(&back).questions[0].instructions,
            Some(Instructions::Text(_))
        ));
    }
}
