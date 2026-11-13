use base64::Engine as _;
use kaeso_kpp::{
    Answer, AnswerValue, Asset, Block, BoxAnswer, Choice, ChoiceAnswer, ChoiceOption, ChoiceValue,
    DecideRequest, DecideResponse, DecisionInput, Description, DialectId, ErrorKind, Extras,
    GenerateRequest, ImageBlock, JsonBlock, KppError, Level, Message, Origin, Predicate,
    PredicateAnswer, Question, QuestionKind, RawJson, Refusal, Reply, Request, RequestKind, Role,
    Score, ScoreAnswer, Source, StreamEvent, StreamMode,
};
use serde_json::{Value, json};

use super::*;
use crate::dispatch;
use crate::multipart::{FormField, FormWriter};
use crate::test_support::{assert_json_eq, fixtures, sidecar};

const DIR: &str = "system_one";

fn headers_for(folder: &str, name: &str) -> HeaderList {
    sidecar(DIR, folder, name, "headers")
        .and_then(|v| v.as_object().cloned())
        .map(|m| {
            m.into_iter()
                .map(|(k, v)| (k, v.as_str().unwrap_or_default().to_owned()))
                .collect()
        })
        .unwrap_or_default()
}

fn profile_for(name: &str) -> SystemOneProfile {
    match name.split('/').nth(1) {
        Some("openrouter") => SystemOneProfile::OPENROUTER,
        Some("perplexity") => SystemOneProfile::PERPLEXITY,
        Some("baseten") => SystemOneProfile::BASETEN,
        Some("cloudflare-ai-gateway") => SystemOneProfile::CLOUDFLARE_WORKERS_AI,
        // The OpenAPI document's own box example holds a 16×16 image, which TypeSafe refuses
        // (`a_typesafe_example_image_is_refused_before_sending` below).
        _ if name == "box" => SystemOneProfile::GENERIC,
        _ => SystemOneProfile::TYPESAFE,
    }
}

fn decode_with(body: &[u8], headers: &HeaderList) -> DecideRequest {
    match SystemOne::decode_request(RequestKind::Decide, body, headers) {
        Ok(Request::Decide(r)) => r,
        other => panic!("{other:?}"),
    }
}

fn decode(body: &[u8]) -> DecideRequest {
    decode_with(body, &HeaderList::new())
}

fn json_body(v: &Value) -> Vec<u8> {
    serde_json::to_vec(v).unwrap()
}

fn ctx(profile: SystemOneProfile, req: &Request) -> EgressCtx {
    EgressCtx::new(req)
        .with_provider("typesafe")
        .with_system_one_profile(profile)
}

fn encode(
    req: &DecideRequest,
    profile: SystemOneProfile,
) -> Result<(EncodedRequest, EgressCtx), KppError> {
    let req = Request::Decide(req.clone());
    let mut c = ctx(profile, &req);
    let enc = SystemOne::encode_request(&req, &mut c)?;
    Ok((enc, c))
}

fn body_of(enc: &EncodedRequest) -> Value {
    serde_json::from_slice(&enc.body).unwrap()
}

fn request_fixture(name: &str) -> (Vec<u8>, HeaderList) {
    let body = fixtures(DIR, "requests", "json")
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap_or_else(|| panic!("no request {name}"))
        .1;
    (body, headers_for("requests", name))
}

fn png(width: u32, height: u32) -> Vec<u8> {
    let mut b = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR".to_vec();
    b.extend(width.to_be_bytes());
    b.extend(height.to_be_bytes());
    b.extend([8, 2, 0, 0, 0, 0, 0, 0, 0]);
    b
}

fn jpeg(width: u16, height: u16) -> Vec<u8> {
    let mut b = vec![0xff, 0xd8, 0xff, 0xc0, 0x00, 0x11, 0x08];
    b.extend(height.to_be_bytes());
    b.extend(width.to_be_bytes());
    b.extend([3, 1, 0x22, 0]);
    b
}

fn webp(width: u32, height: u32) -> Vec<u8> {
    let mut b = b"RIFF\0\0\0\0WEBPVP8X\x0a\0\0\0\0\0\0\0".to_vec();
    b.extend(&(width - 1).to_le_bytes()[..3]);
    b.extend(&(height - 1).to_le_bytes()[..3]);
    b
}

fn noul(name: &str, instructions: &str) -> Question {
    Question::new(
        Some(name.to_owned()),
        QuestionKind::Predicate(Predicate::default()),
    )
    .with_instructions(kaeso_kpp::Instructions::Text(instructions.to_owned()))
}

fn with_image(state: Value, bytes: Vec<u8>, media: &str) -> DecideRequest {
    let raw = RawJson::from_serialize(&state).unwrap();
    DecideRequest::new("jev-latest", DecisionInput::Json(raw))
        .with_question(noul("seen", "Is a person in `photo`?"))
        .with_asset("img-1", Asset::new(Source::bytes(media, bytes)))
}

#[test]
fn every_request_fixture_round_trips() {
    for (name, body) in fixtures(DIR, "requests", "json") {
        let headers = headers_for("requests", &name);
        let req = decode_with(&body, &headers);
        let (enc, _) = encode(&req, profile_for(&name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_json_eq(&name, &body, &enc.body);
        for (k, v) in headers.iter() {
            assert_eq!(enc.headers.get(k), Some(v), "{name}: {k}");
        }
        let model = serde_json::from_slice::<Value>(&body).unwrap()["model"]
            .as_str()
            .unwrap()
            .to_owned();
        assert_eq!(
            enc.path,
            profile_for(&name).path.replace("{model}", &model),
            "{name}"
        );
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

// What an app gets for a provider's reply: Cloudflare's `result` without its envelope, with the
// model's full id.
fn as_apps_get(profile: SystemOneProfile, body: &[u8]) -> Vec<u8> {
    let Envelope::Result = profile.envelope else {
        return body.to_vec();
    };
    let mut result = serde_json::from_slice::<Value>(body).unwrap()["result"].take();
    if let ModelField::ShortName(prefix) = profile.model_field {
        result["model"] = format!("{prefix}{}", result["model"].as_str().unwrap()).into();
    }
    json_body(&result)
}

#[test]
fn every_response_fixture_round_trips_through_its_request() {
    for (name, body) in fixtures(DIR, "responses", "json") {
        let (req_body, headers) = request_fixture(&request_of(&name));
        let req = decode_with(&req_body, &headers);
        let profile = profile_for(&name);
        let (_, c) = encode(&req, profile).unwrap();
        let reply = SystemOne::decode_response(200, &HeaderList::new(), &body, &c)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let out = SystemOne::encode_response(&reply, &EncodeCtx::for_request(Request::Decide(req)))
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_json_eq(&name, &as_apps_get(profile, &body), &out);
    }
}

#[test]
fn responses_read_as_their_documented_values() {
    let (req, _) = request_fixture("score");
    let req = decode(&req);
    let (_, c) = encode(&req, SystemOneProfile::TYPESAFE).unwrap();
    let body = fixtures(DIR, "responses", "json")
        .into_iter()
        .find(|(n, _)| n == "score")
        .unwrap()
        .1;
    let Reply::Decide(r) = SystemOne::decode_response(200, &HeaderList::new(), &body, &c).unwrap()
    else {
        panic!()
    };
    assert_eq!(r.model, "jev-latest");
    assert_eq!((r.usage.input_tokens, r.usage.output_tokens), (120, 12));
    let answer = &r.answers[0];
    assert_eq!(answer.confidence, Some(0.55));
    assert_eq!(
        answer.value,
        AnswerValue::Score(ScoreAnswer::new(1.7, vec![Some(0.1), Some(0.1), Some(0.8)]))
    );
}

#[test]
fn error_fixtures_read_as_their_kinds_and_reach_apps_as_sent() {
    let expected: &[(&str, ErrorKind)] = &[
        ("validation-422", ErrorKind::InvalidRequest),
        ("validation-criteria-422", ErrorKind::InvalidRequest),
        ("images-not-enabled-400", ErrorKind::InvalidRequest),
        ("asset-unreferenced-400", ErrorKind::InvalidRequest),
        ("max-tokens-exceeded-400", ErrorKind::ContextTooLong),
        ("unauthorized-401", ErrorKind::Authentication),
        ("too-large-413", ErrorKind::RequestTooLarge),
        ("unsupported-encoding-415", ErrorKind::InvalidRequest),
        ("rate-limit-429", ErrorKind::RateLimited),
        ("overloaded-529", ErrorKind::Overloaded),
        (
            "providers/openrouter/payment-required-402",
            ErrorKind::Billing,
        ),
        (
            "providers/perplexity/invalid-model-400",
            ErrorKind::InvalidRequest,
        ),
        (
            "providers/perplexity/noul-needs-criteria-400",
            ErrorKind::InvalidRequest,
        ),
        (
            "providers/typesafe/recorded-invalid-key-401",
            ErrorKind::Authentication,
        ),
        (
            "providers/typesafe/recorded-missing-key-403",
            ErrorKind::Authentication,
        ),
        (
            "providers/cloudflare-ai-gateway/recorded-invalid-token-401",
            ErrorKind::Authentication,
        ),
        (
            "providers/cloudflare-ai-gateway/recorded-no-instructions-400",
            ErrorKind::InvalidRequest,
        ),
        (
            "providers/cloudflare-ai-gateway/recorded-one-level-422",
            ErrorKind::InvalidRequest,
        ),
        (
            "providers/cloudflare-ai-gateway/recorded-one-option-422",
            ErrorKind::InvalidRequest,
        ),
        (
            "providers/cloudflare-ai-gateway/recorded-too-many-images-422",
            ErrorKind::InvalidRequest,
        ),
        (
            "providers/cloudflare-ai-gateway/recorded-too-many-questions-422",
            ErrorKind::InvalidRequest,
        ),
        (
            "providers/cloudflare-ai-gateway/recorded-unknown-field-422",
            ErrorKind::InvalidRequest,
        ),
        (
            "providers/cloudflare-ai-gateway/recorded-unknown-gateway-400",
            ErrorKind::InvalidRequest,
        ),
        (
            "providers/cloudflare-ai-gateway/recorded-wrong-model-422",
            ErrorKind::InvalidRequest,
        ),
    ];
    let all = fixtures(DIR, "errors", "json");
    assert_eq!(all.len(), expected.len());
    for (name, kind) in expected {
        let body = &all.iter().find(|(n, _)| n == name).unwrap().1;
        let meta = sidecar(DIR, "errors", name, "meta").unwrap();
        let status = u16::try_from(meta["status"].as_u64().unwrap()).unwrap();
        let headers: HeaderList = meta
            .get("headers")
            .and_then(Value::as_object)
            .map(|m| {
                m.iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned()))
                    .collect()
            })
            .unwrap_or_default();
        let err = SystemOne::decode_error(status, &headers, body);
        assert_eq!(&err.kind, kind, "{name}");
        assert!(!err.message.is_empty(), "{name}");
        let out = SystemOne::encode_error(&err);
        if name.starts_with("providers/") && !name.starts_with("providers/typesafe/") {
            let sent: Value = serde_json::from_slice(&out.body).unwrap();
            assert_eq!(sent["detail"], err.message.as_str(), "{name}");
            continue;
        }
        assert_eq!(out.status, status, "{name}");
        assert_json_eq(name, body, &out.body);
        if status == 429 {
            assert_eq!(
                err.retry_after,
                Some(std::time::Duration::from_millis(1500))
            );
            assert_eq!(out.headers.get("retry-after"), Some("2"));
            assert_eq!(out.headers.get("retry-after-ms"), Some("1500"));
        }
    }
    let err = SystemOne::decode_error(422, &HeaderList::new(), br#"{"detail":[{"loc":["body","questions","urgency","score","criteria"],"msg":"too short","type":"too_short"}]}"#);
    assert_eq!(
        err.param.as_deref(),
        Some("questions.urgency.score.criteria")
    );
    assert_eq!(err.message, "`questions.urgency.score.criteria`: too short");
}

#[test]
fn kaeso_errors_reach_system_one_apps_in_its_shapes() {
    let schema = KppError::invalid_request("`questions` must be an object of named questions.")
        .with_param("questions[2].criteria");
    let out = SystemOne::encode_error(&schema);
    assert_eq!(out.status, 422);
    assert_eq!(
        serde_json::from_slice::<Value>(&out.body).unwrap(),
        json!({"detail": [{"loc": ["body", "questions", 2, "criteria"], "msg": schema.message, "type": "value_error"}]})
    );
    for (err, status) in [
        (KppError::invalid_request("Ask at least one question."), 400),
        (
            KppError::unsupported("box question", "Perplexity answers no box questions."),
            400,
        ),
        (KppError::new(ErrorKind::RequestTooLarge, "Too large."), 413),
        (
            KppError::new(ErrorKind::Overloaded, "Busy.").with_status(503),
            529,
        ),
        (KppError::new(ErrorKind::Authentication, "Bad key."), 401),
        (KppError::new(ErrorKind::ContextTooLong, "Too long."), 400),
    ] {
        let out = SystemOne::encode_error(&err);
        assert_eq!(out.status, status, "{err:?}");
        let body: Value = serde_json::from_slice(&out.body).unwrap();
        assert_eq!(body, json!({"detail": err.message}));
    }
    let limited = KppError::new(ErrorKind::RateLimited, "Slow down.")
        .with_retry_after(std::time::Duration::from_millis(1500));
    let out = SystemOne::encode_error(&limited);
    assert_eq!(out.status, 429);
    assert_eq!(out.headers.get("retry-after"), Some("2"));
    assert_eq!(out.headers.get("retry-after-ms"), Some("1500"));
    let foreign = KppError::invalid_request("Bad field.")
        .with_param("messages[0]")
        .with_origin(Origin::new(DialectId::OpenAiChat));
    assert_eq!(SystemOne::encode_error(&foreign).status, 400);
}

#[test]
fn an_upload_round_trips_with_its_image_parts() {
    let (json, _) = request_fixture_in("requests/multipart", "door");
    let photo = base64::engine::general_purpose::STANDARD
        .decode(serde_json::from_slice::<Value>(&request_fixture("assets").0).unwrap()["assets"]["a3f1c2e8-found"]["data_base64"].as_str().unwrap())
        .unwrap();
    let form = FormWriter::write(&[
        FormField::Typed("request", "application/json", &json),
        FormField::File("door-photo", "door-camera.jpg", "image/jpeg", &photo),
    ]);
    let headers = HeaderList::new()
        .with("content-type", form.content_type())
        .with("TypeSafe-Danger-Zone-Images", "true");
    let body = bytes::Bytes::from(form.into_body());
    let checked = crate::limits::Checked::new(DialectId::SystemOne, body.clone()).unwrap();
    let Request::Decide(req) =
        dispatch::decode_checked_request(RequestKind::Decide, &checked, &headers).unwrap()
    else {
        panic!()
    };
    let asset = &req.assets["a3f1c2e8-door"];
    let Source::Base64(b) = &asset.source else {
        panic!()
    };
    assert_eq!(b.media_type, "image/jpeg");
    assert_eq!(b.data.as_ref(), photo.as_slice());
    let start = b.data.as_ptr() as usize - body.as_ptr() as usize;
    assert!(start < body.len(), "the image is a slice of the upload");

    let (enc, _) = encode(&req, SystemOneProfile::TYPESAFE).unwrap();
    let ct = enc.headers.get("content-type").unwrap().to_owned();
    assert!(ct.starts_with("multipart/form-data; boundary="), "{ct}");
    assert_eq!(enc.headers.get("typesafe-danger-zone-images"), Some("true"));
    let again = decode_with(
        &enc.body,
        &HeaderList::new()
            .with("content-type", ct)
            .with("typesafe-danger-zone-images", "true"),
    );
    assert_eq!(again, req);
    let boundary =
        crate::multipart::form_boundary(enc.headers.get("content-type").unwrap()).unwrap();
    let parts = crate::multipart::read_form(
        &enc.body,
        &boundary,
        crate::multipart::FormLimits::new(4, 1 << 20),
    )
    .unwrap();
    assert_eq!(parts[0].name, "request");
    assert_eq!(parts[0].content_type.as_deref(), Some("application/json"));
    assert_json_eq("door", &json, parts[0].body);
    assert_eq!(
        (parts[1].name.as_str(), parts[1].filename.as_deref()),
        ("door-photo", Some("door-camera.jpg"))
    );

    let (to_openrouter, _) = encode(&req, SystemOneProfile::OPENROUTER).unwrap();
    let sent = body_of(&to_openrouter);
    assert_eq!(sent["assets"]["a3f1c2e8-door"]["format"], "jpg");
    assert!(
        sent["assets"]["a3f1c2e8-door"]["data_base64"]
            .as_str()
            .unwrap()
            .starts_with("/9j/")
    );
}

fn request_fixture_in(folder: &str, name: &str) -> (Vec<u8>, HeaderList) {
    let body = std::fs::read(
        crate::test_support::fixtures_dir()
            .join(DIR)
            .join(folder)
            .join(format!("{name}.json")),
    )
    .unwrap();
    (body, HeaderList::new())
}

#[test]
fn uploads_follow_typesafes_multipart_rules() {
    let (json, _) = request_fixture_in("requests/multipart", "door");
    let photo = jpeg(300, 300);
    let decode_form = |fields: &[FormField<'_>]| {
        let form = FormWriter::write(fields);
        let h = HeaderList::new().with("content-type", form.content_type());
        SystemOne::decode_request(RequestKind::Decide, &form.into_body(), &h)
    };
    let refused = [
        vec![
            FormField::File("door-photo", "d.jpg", "image/jpeg", &photo),
            FormField::Typed("request", "application/json", &json),
        ],
        vec![FormField::Typed("request", "application/json", &json)],
        vec![
            FormField::Typed("request", "application/json", &json),
            FormField::File("door-photo", "d.jpg", "image/jpeg", &photo),
            FormField::File("spare", "s.jpg", "image/jpeg", &photo),
        ],
        vec![
            FormField::Typed("request", "application/json", &json),
            FormField::File("door-photo", "d.gif", "image/gif", &photo),
        ],
    ];
    for fields in refused {
        let err = decode_form(&fields).unwrap_err();
        assert_eq!(err.kind, ErrorKind::InvalidRequest, "{err:?}");
    }
}

#[test]
fn a_part_named_by_a_json_body_is_refused() {
    let (json, _) = request_fixture_in("requests/multipart", "door");
    let err =
        SystemOne::decode_request(RequestKind::Decide, &json, &HeaderList::new()).unwrap_err();
    assert_eq!(err.param.as_deref(), Some("assets.a3f1c2e8-door.part"));
    assert_eq!(SystemOne::encode_error(&err).status, 422);
}

#[test]
fn references_must_stand_in_strings_and_assets_must_be_used() {
    let image = png(300, 300);
    let ok = with_image(json!({"photo": "img-1"}), image.clone(), "image/png");
    ok.validate().unwrap();
    let mid_sentence = with_image(json!("Look at img-1 closely"), image.clone(), "image/png");
    mid_sentence.validate().unwrap();
    let only_key = with_image(json!({"img-1": "a photo"}), image.clone(), "image/png");
    let err = only_key.validate().unwrap_err();
    assert_eq!(err.param.as_deref(), Some("assets.img-1"));
    let mut in_criteria = with_image(json!({"photo": "none"}), image.clone(), "image/png");
    in_criteria.questions.push(Question::new(
        Some("gesture".into()),
        QuestionKind::Choice(Choice::new(vec![ChoiceOption::new(
            ChoiceValue::Text("thumbs_up".into()),
            Some(Description::Json(
                RawJson::from_string(r#"{"examples":["img-1"]}"#.into()).unwrap(),
            )),
        )])),
    ));
    in_criteria.validate().unwrap();
    let mut unused = with_image(json!({"photo": "img-1"}), image, "image/png");
    unused.assets.insert(
        "img-2".into(),
        Asset::new(Source::bytes("image/png", png(300, 300))),
    );
    assert_eq!(
        unused.validate().unwrap_err().param.as_deref(),
        Some("assets.img-2")
    );
    let (body, h) = request_fixture("assets");
    let mut v: Value = serde_json::from_slice(&body).unwrap();
    v["questions"]["same_dog"]["instructions"] = json!("Is the dog the same?");
    let req = decode_with(&json_body(&v), &h);
    let err = SystemOne::check(
        &Request::Decide(req),
        &EgressCtx::for_kind(RequestKind::Decide, StreamMode::Off),
    )
    .unwrap_err();
    assert_eq!(err.param.as_deref(), Some("assets.77b0d4a1-poster"));
}

#[test]
fn images_go_where_each_provider_takes_them() {
    let image = png(300, 300);
    let req = with_image(
        json!({"caption": "Door", "photo": "img-1"}),
        image.clone(),
        "image/png",
    );
    let (enc, _) = encode(&req, SystemOneProfile::PERPLEXITY).unwrap();
    let sent = body_of(&enc);
    assert_eq!(sent["state"]["caption"], "Door");
    assert_eq!(sent["state"]["photo"]["type"], "image_url");
    assert!(
        sent["state"]["photo"]["image_url"]["url"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,iVBOR")
    );
    assert!(sent.get("assets").is_none());
    assert_eq!(enc.path, "/v1/decisions");
    assert_eq!(enc.headers.get("typesafe-danger-zone-images"), None);
    let keys: Vec<&str> = sent["state"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys.len(), 2);

    let (enc, _) = encode(&req, SystemOneProfile::TYPESAFE).unwrap();
    assert_eq!(enc.headers.get("typesafe-danger-zone-images"), Some("true"));
    let sent = body_of(&enc);
    assert_eq!(sent["state"]["photo"], "img-1");
    assert_eq!(sent["assets"]["img-1"]["format"], "png");

    let in_text = with_image(json!("A photo: img-1"), image.clone(), "image/png");
    let err = encode(&in_text, SystemOneProfile::PERPLEXITY).unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("image reference"));
    encode(&in_text, SystemOneProfile::TYPESAFE).unwrap();

    let listed = with_image(
        json!(["Two photos:", "img-1", "img-2"]),
        image.clone(),
        "image/png",
    )
    .with_asset(
        "img-2",
        Asset::new(Source::bytes("image/jpeg", jpeg(300, 300))),
    );
    let mut cf = listed.clone();
    cf.model = "@cf/cloudflare/clef".into();
    let (enc, _) = encode(&cf, SystemOneProfile::CLOUDFLARE_WORKERS_AI).unwrap();
    let sent = body_of(&enc);
    assert_eq!(enc.path, "/run/@cf/cloudflare/clef");
    assert_eq!(sent["model"], "clef");
    assert_eq!(sent["state"], json!(["Two photos:"]));
    let images = sent["images"].as_array().unwrap();
    assert!(
        images[0]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,")
    );
    assert!(
        images[1]
            .as_str()
            .unwrap()
            .starts_with("data:image/jpeg;base64,")
    );
    let nested = with_image(json!({"photo": "img-1"}), image.clone(), "image/png");
    assert_eq!(
        encode(&nested, SystemOneProfile::CLOUDFLARE_WORKERS_AI)
            .unwrap_err()
            .unsupported_feature(),
        Some("images in the state")
    );

    let (enc, _) = encode(&listed, SystemOneProfile::BASETEN).unwrap();
    let sent = body_of(&enc);
    let content = &sent["state"]["messages"][0]["content"];
    assert_eq!(sent["state"]["messages"][0]["role"], "user");
    assert_eq!(content[0], json!({"type": "text", "text": "Two photos:"}));
    assert_eq!(content[1]["type"], "image_url");
    assert_eq!(content[2]["type"], "image_url");
    assert_eq!(
        encode(&nested, SystemOneProfile::BASETEN)
            .unwrap_err()
            .unsupported_feature(),
        Some("images in structured state")
    );
    let err = encode(&req, SystemOneProfile::FIREWORKS).unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("images"));
}

#[test]
fn state_parts_from_an_app_become_assets_and_go_back_as_parts() {
    let body = fixtures(DIR, "requests", "json")
        .into_iter()
        .find(|(n, _)| n == "providers/perplexity/object-photo")
        .unwrap()
        .1;
    let req = decode(&body);
    assert_eq!(req.assets.len(), 1);
    let (reference, asset) = req.assets.first().unwrap();
    assert_eq!(reference, "kaeso-image-0-ref");
    assert!(matches!(&asset.source, Source::Base64(b) if b.media_type == "image/jpeg"));
    let DecisionInput::Json(state) = &req.input else {
        panic!()
    };
    assert_eq!(
        state.get(),
        r#"{"caption":"Product photo from the listing","photo":"kaeso-image-0-ref"}"#
    );
    let (enc, _) = encode(&req, SystemOneProfile::TYPESAFE).unwrap();
    let sent = body_of(&enc);
    assert_eq!(sent["state"]["photo"], "kaeso-image-0-ref");
    assert_eq!(sent["assets"]["kaeso-image-0-ref"]["format"], "jpg");
    let (enc, _) = encode(&req, SystemOneProfile::OPENROUTER).unwrap();
    assert_json_eq("object-photo", &body, &enc.body);

    let mut mixed: Value = serde_json::from_slice(&request_fixture("assets").0).unwrap();
    mixed["state"]["extra"] = json!({"type": "image_url", "image_url": {"url": format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png(300, 300))), "detail": "low"}});
    let req = decode(&json_body(&mixed));
    assert_eq!(req.assets.len(), 3);
    let (enc, _) = encode(&req, SystemOneProfile::OPENROUTER).unwrap();
    let sent = body_of(&enc);
    assert_eq!(sent["state"]["extra"]["image_url"]["detail"], "low");
    assert_eq!(sent["assets"].as_object().unwrap().len(), 2);
}

#[test]
fn image_headers_refuse_what_typesafe_refuses() {
    let cases: [(Vec<u8>, &str, &str); 5] = [
        (webp(300, 300), "image/webp", "image format"),
        (png(200, 300), "image/png", "image size"),
        (jpeg(1600, 900), "image/jpeg", "image size"),
        (
            b"GIF89a\x2c\x01\x2c\x01\0\0".to_vec(),
            "image/gif",
            "image format",
        ),
        (b"not an image".to_vec(), "image/png", "image format"),
    ];
    for (bytes, media, feature) in cases {
        let req = with_image(json!({"photo": "img-1"}), bytes.clone(), media);
        let err = encode(&req, SystemOneProfile::TYPESAFE).unwrap_err();
        assert_eq!(
            err.unsupported_feature(),
            Some(feature),
            "{media}: {}",
            err.message
        );
        assert_eq!(err.param.as_deref(), Some("assets.img-1"));
        assert!(err.message.starts_with("TypeSafe takes"), "{}", err.message);
    }
    let mut apng = png(300, 300);
    apng.extend([0, 0, 0, 8]);
    apng.extend(b"acTL");
    apng.extend([0; 12]);
    let req = with_image(json!({"photo": "img-1"}), apng, "image/png");
    assert_eq!(
        encode(&req, SystemOneProfile::TYPESAFE)
            .unwrap_err()
            .unsupported_feature(),
        Some("animated image")
    );
    let at_limits = with_image(json!({"photo": "img-1"}), png(280, 1568), "image/png");
    encode(&at_limits, SystemOneProfile::TYPESAFE).unwrap();
    let webp_ok = with_image(json!({"photo": "img-1"}), webp(300, 300), "image/webp");
    encode(&webp_ok, SystemOneProfile::PERPLEXITY).unwrap();
    let url = DecideRequest::new("jev-latest", DecisionInput::Text("img-1".into()))
        .with_question(noul("seen", "Is a person shown?"))
        .with_asset(
            "img-1",
            Asset::new(Source::Url("https://example.com/door.png".into())),
        );
    assert_eq!(
        encode(&url, SystemOneProfile::TYPESAFE)
            .unwrap_err()
            .unsupported_feature(),
        Some("image URL")
    );
}

#[test]
fn a_typesafe_example_image_is_refused_before_sending() {
    let (body, h) = request_fixture("box");
    let err = encode(&decode_with(&body, &h), SystemOneProfile::TYPESAFE).unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("image size"));
    assert!(err.message.contains("16×16"), "{}", err.message);
}

#[test]
fn profiles_refuse_what_their_providers_refuse() {
    let (body, h) = request_fixture("box");
    let req = decode_with(&body, &h);
    for profile in [
        SystemOneProfile::PERPLEXITY,
        SystemOneProfile::BASETEN,
        SystemOneProfile::FIREWORKS,
    ] {
        assert_eq!(
            encode(&req, profile).unwrap_err().unsupported_feature(),
            Some("box question")
        );
    }
    let (body, _) = request_fixture("unknown-top-level-field");
    let req = decode(&body);
    let err = encode(&req, SystemOneProfile::PERPLEXITY).unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("x_future_field"));
    encode(&req, SystemOneProfile::OPENROUTER).unwrap();
    let many = (0..65).fold(
        DecideRequest::new("clef", DecisionInput::Text("x".into())),
        |r, i| r.with_question(noul(&format!("q{i}"), "?")),
    );
    assert_eq!(
        encode(&many, SystemOneProfile::CLOUDFLARE_WORKERS_AI)
            .unwrap_err()
            .unsupported_feature(),
        Some("questions")
    );
    let levels = Score::new(
        (0..11)
            .map(|i| Level::new(None, Some(Description::Text(i.to_string()))))
            .collect(),
    );
    let req = DecideRequest::new("jev-latest", DecisionInput::Text("x".into()))
        .with_question(Question::new(Some("s".into()), QuestionKind::Score(levels)));
    assert_eq!(
        encode(&req, SystemOneProfile::TYPESAFE)
            .unwrap_err()
            .unsupported_feature(),
        Some("levels")
    );
    encode(&req, SystemOneProfile::GENERIC).unwrap();
}

#[test]
fn names_a_provider_cant_take_become_positions_and_answers_map_back() {
    let req = DecideRequest::new("@cf/cloudflare/clef", DecisionInput::Text("Hi".into()))
        .with_question(noul("is it spam?", "Spam?"))
        .with_question(
            Question::new(
                Some("tone".into()),
                QuestionKind::Choice(Choice::new(vec![
                    ChoiceOption::new(ChoiceValue::Text("calm".into()), None),
                    ChoiceOption::new(ChoiceValue::Text("angry".into()), None),
                ])),
            )
            .with_instructions(kaeso_kpp::Instructions::Text("Tone?".into())),
        );
    let (enc, c) = encode(&req, SystemOneProfile::CLOUDFLARE_WORKERS_AI).unwrap();
    let sent = body_of(&enc);
    assert_eq!(
        sent["questions"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        ["q0", "q1"]
    );
    assert_eq!(c.adapted, ["names"]);
    assert_eq!(c.decision_keys().collect::<Vec<_>>(), ["q0", "q1"]);
    let reply = json!({
        "result": {
            "model": "clef",
            "answers": {
                "q1": {"type": "choice", "choice": "angry", "confidence": 0.4, "probabilities": {"calm": 0.3, "angry": 0.7}},
                "q0": {"type": "noul", "noul": 0.2}
            },
            "usage": {"input_tokens": 40, "output_tokens": 0}
        },
        "success": true,
        "errors": [],
        "messages": []
    });
    let Reply::Decide(r) =
        SystemOne::decode_response(200, &HeaderList::new(), &json_body(&reply), &c).unwrap()
    else {
        panic!()
    };
    assert_eq!(
        r.answers[0].value,
        AnswerValue::Predicate(PredicateAnswer::new(0.2))
    );
    assert_eq!(
        r.answers[1].value,
        AnswerValue::Choice(ChoiceAnswer::new(1, vec![Some(0.3), Some(0.7)]))
    );
    let out: Value = serde_json::from_slice(
        &SystemOne::encode_response(
            &Reply::Decide(r),
            &EncodeCtx::for_request(Request::Decide(req)),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(out["answers"]["is it spam?"]["noul"], 0.2);
    assert_eq!(out["answers"]["tone"]["choice"], "angry");
    assert_eq!(out.get("success"), None);

    let missing = json!({"model": "clef", "answers": {"q0": {"type": "noul", "noul": 0.2}}, "usage": {"input_tokens": 1, "output_tokens": 0}});
    let c = c.with_system_one_profile(SystemOneProfile::GENERIC);
    let err =
        SystemOne::decode_response(200, &HeaderList::new(), &json_body(&missing), &c).unwrap_err();
    assert!(err.message.contains("`q1`"), "{}", err.message);
    let extra = json!({"model": "m", "answers": {"q0": {"type": "noul", "noul": 0.2}, "q1": {"type": "noul", "noul": 0.1}, "zz": {"type": "noul", "noul": 0.1}}});
    assert!(SystemOne::decode_response(200, &HeaderList::new(), &json_body(&extra), &c).is_err());
    let wrong = json!({"model": "m", "answers": {"q0": {"type": "noul", "noul": 0.2}, "q1": {"type": "choice", "choice": "bored", "confidence": 1, "probabilities": {}}}});
    assert!(SystemOne::decode_response(200, &HeaderList::new(), &json_body(&wrong), &c).is_err());
    let fresh = EgressCtx::for_kind(RequestKind::Decide, StreamMode::Off);
    assert!(
        SystemOne::decode_response(200, &HeaderList::new(), &json_body(&missing), &fresh).is_err()
    );
}

#[test]
fn foreign_answers_get_typesafes_confidence_and_refusals_their_own_answer() {
    let (body, _) = request_fixture("instructions-forms");
    let req = decode(&body);
    let foreign = Extras::new(DialectId::OpenAiChat);
    let answer = |value: AnswerValue, confidence: Option<f64>| {
        let mut a = Answer::new(value).with_confidence(confidence);
        a.extras = foreign.clone();
        a
    };
    let mut reply = DecideResponse::new(
        "gpt-6-luna",
        vec![
            answer(AnswerValue::Predicate(PredicateAnswer::new(0.9)), Some(0.9)),
            answer(
                AnswerValue::Score(ScoreAnswer::new(
                    2.0,
                    vec![Some(0.0), Some(0.0), Some(0.5), Some(0.5)],
                )),
                Some(0.5),
            ),
            answer(
                AnswerValue::Choice(ChoiceAnswer::new(0, vec![Some(0.5), Some(0.3), Some(0.2)])),
                Some(0.5),
            ),
            answer(AnswerValue::Refusal(Refusal::new()), None),
            answer(
                AnswerValue::Score(ScoreAnswer::new(0.0, vec![Some(1.0)])),
                None,
            ),
        ],
    );
    reply.usage = kaeso_kpp::Usage::new(10, 0);
    let out: Value = serde_json::from_slice(
        &SystemOne::encode_response(
            &Reply::Decide(reply),
            &EncodeCtx::for_request(Request::Decide(req)),
        )
        .unwrap(),
    )
    .unwrap();
    let a = &out["answers"];
    assert_eq!(a["bug"], json!({"type": "noul", "noul": 0.9}));
    assert_eq!(a["area"]["choice"], "export");
    assert!((a["area"]["confidence"].as_f64().unwrap() - 0.25).abs() < 1e-9);
    assert!((a["urgency"]["confidence"].as_f64().unwrap() - 0.5).abs() < 1e-9);
    assert_eq!(
        a["urgency"]["legend"]["1"],
        json!(["Needs attention", "this week"])
    );
    assert_eq!(
        a["urgency"]["probabilities"],
        json!({"0": 0.0, "1": 0.0, "2": 0.5, "3": 0.5})
    );
    assert_eq!(a["one_option"], json!({"type": "refusal"}));
    assert_eq!(a["one_level"]["confidence"], 1.0);
    assert_eq!(
        out["usage"],
        json!({"input_tokens": 10, "output_tokens": 0})
    );
}

#[test]
fn boolean_and_labelled_options_are_written_as_system_one_takes_them() {
    let options = |values: Vec<ChoiceValue>| {
        QuestionKind::Choice(Choice::new(
            values
                .into_iter()
                .map(|v| ChoiceOption::new(v, None))
                .collect(),
        ))
    };
    let req = DecideRequest::new("jev-latest", DecisionInput::Text("x".into()))
        .with_question(Question::new(
            Some("b".into()),
            options(vec![
                ChoiceValue::Bool(true),
                ChoiceValue::Text("maybe".into()),
            ]),
        ))
        .with_question(Question::new(
            Some("s".into()),
            QuestionKind::Score(Score::new(vec![
                Level::new(
                    Some("low".into()),
                    Some(Description::Text("Can wait".into())),
                ),
                Level::new(Some("high".into()), None),
            ])),
        ));
    let (enc, c) = encode(&req, SystemOneProfile::TYPESAFE).unwrap();
    let sent = body_of(&enc);
    assert_eq!(
        sent["questions"]["b"]["criteria"],
        json!({"true": null, "maybe": null})
    );
    assert_eq!(
        sent["questions"]["s"]["criteria"],
        json!(["low: Can wait", "high"])
    );
    assert_eq!(c.adapted, ["booleans", "levels"]);
    let clash = DecideRequest::new("jev-latest", DecisionInput::Text("x".into())).with_question(
        Question::new(
            Some("b".into()),
            options(vec![
                ChoiceValue::Bool(true),
                ChoiceValue::Text("true".into()),
            ]),
        ),
    );
    assert_eq!(
        encode(&clash, SystemOneProfile::TYPESAFE)
            .unwrap_err()
            .unsupported_feature(),
        Some("repeated choice value")
    );
}

#[test]
fn one_user_message_becomes_the_state_and_a_conversation_is_refused() {
    let mut image = ImageBlock::new(Source::bytes("image/png", png(300, 300)));
    image.cache = Some(kaeso_kpp::CacheHint::default());
    let message = Message::new(
        Role::User,
        vec![
            Block::text("A photo:"),
            Block::Image(image),
            Block::Json(JsonBlock::new(
                RawJson::from_string(r#"{"seats":12}"#.into()).unwrap(),
            )),
        ],
    );
    let req = DecideRequest::new("jev-latest", DecisionInput::Messages(vec![message.clone()]))
        .with_question(noul("seen", "Is a person shown?"));
    let (enc, c) = encode(&req, SystemOneProfile::TYPESAFE).unwrap();
    let sent = body_of(&enc);
    assert_eq!(
        sent["state"],
        json!(["A photo:", "kaeso-image-0-ref", {"seats": 12}])
    );
    assert_eq!(sent["assets"]["kaeso-image-0-ref"]["type"], "image");
    assert_eq!(c.left_out, ["cache"]);
    let (enc, _) = encode(&req, SystemOneProfile::PERPLEXITY).unwrap();
    assert_eq!(body_of(&enc)["state"][1]["type"], "image_url");
    let conversation = DecideRequest::new(
        "jev-latest",
        DecisionInput::Messages(vec![
            message,
            Message::assistant("Ok."),
            Message::user("Now?"),
        ]),
    )
    .with_question(noul("seen", "?"));
    assert_eq!(
        encode(&conversation, SystemOneProfile::TYPESAFE)
            .unwrap_err()
            .unsupported_feature(),
        Some("conversation input")
    );
    let audio = DecideRequest::new(
        "jev-latest",
        DecisionInput::Messages(vec![Message::new(
            Role::User,
            vec![Block::audio(Source::bytes("audio/wav", vec![0u8; 8]))],
        )]),
    )
    .with_question(noul("seen", "?"));
    assert_eq!(
        encode(&audio, SystemOneProfile::TYPESAFE)
            .unwrap_err()
            .unsupported_feature(),
        Some("audio part")
    );
}

#[test]
fn foreign_fields_and_headers_follow_the_extras_rules() {
    let (body, h) = request_fixture("long-context");
    let mut req = decode_with(&body, &h);
    let (enc, _) = encode(&req, SystemOneProfile::TYPESAFE).unwrap();
    assert_eq!(
        enc.headers.get("typesafe-danger-zone-long-context"),
        Some("true")
    );
    crate::cross::check_foreign(
        &Request::Decide(req.clone()),
        &Origin::new(DialectId::OpenAiChat),
    )
    .unwrap();
    req.extras.fields.insert("x_future".into(), RawJson::null());
    let err =
        crate::cross::check_foreign(&Request::Decide(req), &Origin::new(DialectId::OpenAiChat))
            .unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("x_future"));
    let (body, _) = request_fixture("unknown-fields");
    let mut req = decode(&body);
    req.extras = Extras::new(DialectId::OpenAiChat);
    let mut foreign_question = req.clone();
    foreign_question.questions[0].extras =
        Extras::new(DialectId::OpenAiChat).with_field("weight", RawJson::null());
    let err = SystemOne::check(
        &Request::Decide(foreign_question),
        &EgressCtx::for_kind(RequestKind::Decide, StreamMode::Off),
    )
    .unwrap_err();
    assert_eq!(err.param.as_deref(), Some("questions[0]"));
}

#[test]
fn decision_requests_are_refused_by_every_other_codec_and_other_kinds_by_system_one() {
    let req = Request::Decide(
        DecideRequest::new("jev-latest", DecisionInput::Text("x".into()))
            .with_question(noul("a", "?")),
    );
    let reply = Reply::Decide(DecideResponse::new("m", vec![]));
    for d in DialectId::ALL {
        if matches!(
            d,
            DialectId::SystemOne | DialectId::OpenAiDecisions | DialectId::GeminiDecisions
        ) {
            continue;
        }
        let mut c = EgressCtx::new(&req);
        let errs = [
            dispatch::check(d, &req, &c).unwrap_err(),
            dispatch::encode_request(d, &req, &mut c).unwrap_err(),
            dispatch::decode_request(d, RequestKind::Decide, b"{}", &HeaderList::new())
                .unwrap_err(),
            dispatch::encode_response(d, &reply, &EncodeCtx::for_request(req.clone())).unwrap_err(),
        ];
        for err in errs {
            assert_eq!(err.unsupported_feature(), Some("decision request"), "{d}");
            assert!(
                err.message.contains("/v1/systemone"),
                "{d}: {}",
                err.message
            );
        }
        let checked = crate::limits::Checked::new(d, &b"{}"[..]);
        if let Ok(checked) = checked {
            let err =
                dispatch::decode_checked_request(RequestKind::Decide, &checked, &HeaderList::new())
                    .unwrap_err();
            assert_eq!(err.unsupported_feature(), Some("decision request"), "{d}");
        }
    }
    let chat = Request::Generate(GenerateRequest::new("m").with_message(Message::user("hi")));
    assert!(SystemOne::check(&chat, &EgressCtx::new(&chat)).is_err());
    assert!(SystemOne::decode_request(RequestKind::Generate, b"{}", &HeaderList::new()).is_err());
    let generate = Reply::Generate(kaeso_kpp::Response::new("r", "m", vec![]));
    assert!(SystemOne::encode_response(&generate, &EncodeCtx::for_request(chat)).is_err());
}

#[test]
fn json_parts_are_refused_by_every_chat_format() {
    let req = Request::Generate(GenerateRequest::new("m").with_message(Message::new(
        Role::User,
        vec![Block::json(RawJson::from_string("{}".into()).unwrap())],
    )));
    for d in [
        DialectId::AnthropicMessages,
        DialectId::OpenAiChat,
        DialectId::OpenAiResponses,
        DialectId::Gemini,
    ] {
        let err = dispatch::check(d, &req, &EgressCtx::new(&req)).unwrap_err();
        assert_eq!(
            err.unsupported_feature(),
            Some("json block"),
            "{d}: {}",
            err.message
        );
    }
}

#[test]
fn decision_requests_are_never_streamed() {
    let mut enc = SystemOne::stream_encoder(&EncodeCtx::new(RequestKind::Decide, StreamMode::Off));
    let err = enc
        .encode(&StreamEvent::ResponseStop(kaeso_kpp::ResponseStop::new()))
        .unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("streaming"));
    assert!(enc.finish().unwrap().is_empty());
    let mut dec =
        SystemOne::stream_decoder(&EgressCtx::for_kind(RequestKind::Decide, StreamMode::Off));
    assert!(matches!(
        dec.feed(b"data: {}\n\n").as_slice(),
        [StreamEvent::Error(_)]
    ));
    assert!(dec.finish().is_empty());
}

#[test]
fn decoding_refuses_what_system_one_refuses() {
    let cases = [
        (json!({"model": "m", "state": "x"}), Some("questions")),
        (
            json!({"model": "m", "state": "x", "questions": []}),
            Some("questions"),
        ),
        (
            json!({"model": "m", "state": "x", "questions": {"a": {"type": "extract"}}}),
            Some("questions.a.type"),
        ),
        (
            json!({"model": "m", "state": "x", "questions": {"a": {"type": "choice"}}}),
            Some("questions.a.criteria"),
        ),
        (
            json!({"model": "m", "state": "x", "questions": {"a": {"type": "score", "criteria": {"a": 1}}}}),
            Some("questions.a.criteria"),
        ),
        (
            json!({"model": "m", "state": "x", "questions": {"a": {"type": "noul", "criteria": "yes"}}}),
            Some("questions.a.criteria"),
        ),
        (
            json!({"model": "m", "state": "x", "questions": {"a": {"type": "noul"}}, "assets": {"r": {"type": "video", "format": "png", "data_base64": ""}}}),
            Some("assets.r.type"),
        ),
        (
            json!({"model": "m", "state": "x", "questions": {"a": {"type": "noul"}}, "assets": {"r": {"format": "webp", "data_base64": ""}}}),
            Some("assets.r.format"),
        ),
        (
            json!({"model": "m", "state": "x", "questions": {"a": {"type": "noul"}}, "assets": {"r": {"data_base64": "iVBO"}}}),
            Some("assets.r.format"),
        ),
        (
            json!({"model": "m", "state": "x", "questions": {"a": {"type": "noul"}}, "assets": {"r": {"type": "image"}}}),
            Some("assets.r"),
        ),
        (
            json!({"model": "m", "state": "x", "questions": {"a": {"type": "noul"}}, "assets": {"r": {"format": "png", "data_base64": "!!"}}}),
            Some("assets.r.data_base64"),
        ),
    ];
    for (body, param) in cases {
        let err =
            SystemOne::decode_request(RequestKind::Decide, &json_body(&body), &HeaderList::new())
                .unwrap_err();
        assert_eq!(err.kind, ErrorKind::InvalidRequest, "{body}");
        assert_eq!(err.param.as_deref(), param, "{body}: {}", err.message);
    }
    let box_with_criteria = json!({"model": "m", "state": "photo", "questions": {"b": {"type": "box", "criteria": ["x"]}}});
    let req = decode(&json_body(&box_with_criteria));
    let (enc, _) = encode(&req, SystemOneProfile::GENERIC).unwrap();
    assert_json_eq("box-criteria", &json_body(&box_with_criteria), &enc.body);
}

#[test]
fn request_limits_count_questions_options_and_assets() {
    let questions: serde_json::Map<String, Value> = (0..1001)
        .map(|i| (format!("q{i}"), json!({"type": "noul"})))
        .collect();
    let err = SystemOne::decode_request(
        RequestKind::Decide,
        &json_body(&json!({"model": "m", "state": "x", "questions": questions})),
        &HeaderList::new(),
    )
    .unwrap_err();
    assert_eq!(err.param.as_deref(), Some("questions"));
    let options: serde_json::Map<String, Value> =
        (0..256).map(|i| (format!("o{i}"), Value::Null)).collect();
    let both = json!({"model": "m", "state": "x", "questions": {"a": {"type": "choice", "criteria": options}, "b": {"type": "score", "criteria": vec!["l"; 255]}}});
    let err = SystemOne::decode_request(RequestKind::Decide, &json_body(&both), &HeaderList::new())
        .unwrap_err();
    assert!(err.message.contains("255"), "{}", err.message);
    let fine = json!({"model": "m", "state": "x", "questions": {"a": {"type": "score", "criteria": vec!["l"; 255]}, "b": {"type": "score", "criteria": vec!["l"; 255]}}});
    SystemOne::decode_request(RequestKind::Decide, &json_body(&fine), &HeaderList::new()).unwrap();
    let assets: serde_json::Map<String, Value> = (0..65)
        .map(|i| (format!("r{i}"), json!({"format": "png", "data_base64": ""})))
        .collect();
    let err = SystemOne::decode_request(RequestKind::Decide, &json_body(&json!({"model": "m", "state": "x", "questions": {"a": {"type": "noul"}}, "assets": assets})), &HeaderList::new()).unwrap_err();
    assert_eq!(err.param.as_deref(), Some("assets"));
    let shape = crate::limits::request_shape(DialectId::SystemOne, &json_body(&fine)).unwrap();
    assert_eq!((shape.questions, shape.assets), (2, 0));
}

#[test]
fn a_box_answer_without_confidence_keeps_its_null() {
    let req = DecideRequest::new("jev-latest", DecisionInput::Text("photo".into())).with_question(
        Question::new(
            Some("b".into()),
            QuestionKind::Box(kaeso_kpp::BoxQuestion::new()),
        ),
    );
    let mut reply = DecideResponse::new(
        "jev-1.14.0",
        vec![Answer::new(AnswerValue::Box(BoxAnswer::new(
            0.1, 0.9, 0.2, 0.8,
        )))],
    );
    reply.usage = kaeso_kpp::Usage::new(5, 1);
    let out: Value = serde_json::from_slice(
        &SystemOne::encode_response(
            &Reply::Decide(reply),
            &EncodeCtx::for_request(Request::Decide(req)),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        out["answers"]["b"],
        json!({"type": "box", "box": {"top": 0.1, "bottom": 0.9, "left": 0.2, "right": 0.8}, "confidence": null})
    );
    assert!(
        SystemOne::encode_response(
            &Reply::Decide(DecideResponse::new("m", vec![])),
            &EncodeCtx::new(RequestKind::Decide, StreamMode::Off)
        )
        .is_err()
    );
}

#[test]
fn typesafes_own_listing_is_read_too() {
    let listing = |name: &str| {
        std::fs::read(
            crate::test_support::fixtures_dir()
                .join(DIR)
                .join("providers/typesafe/models")
                .join(format!("{name}.json")),
        )
        .unwrap()
    };
    for name in ["list-doc", "list"] {
        let list = SystemOne::decode_models(200, &HeaderList::new(), &listing(name))
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(list.models.iter().any(|m| m.id == "jev-latest"), "{name}");
        let first = &list.models[0];
        assert!(first.extras.fields.contains_key("description"), "{name}");
        assert!(first.extras.fields.contains_key("release_date"), "{name}");
    }
    let full = listing("list");
    let ids: Vec<String> = SystemOne::decode_models(200, &HeaderList::new(), &full)
        .unwrap()
        .models
        .into_iter()
        .map(|m| m.id)
        .collect();
    assert_eq!(
        ids,
        ["jev-latest", "jev-preview", "jev-1.14.0", "jev-1.13.0"]
    );
    for bad in [
        &br#"{"models":[{"description":"no name"}]}"#[..],
        br#"{"models":3}"#,
    ] {
        assert_eq!(
            SystemOne::decode_models(200, &HeaderList::new(), bad)
                .unwrap_err()
                .kind,
            ErrorKind::ProviderError
        );
    }
    let refused = fixtures(DIR, "errors", "json")
        .into_iter()
        .find(|(n, _)| n == "providers/typesafe/recorded-missing-key-403")
        .unwrap()
        .1;
    let err = SystemOne::decode_models(403, &HeaderList::new(), &refused).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Authentication);
    assert_eq!(
        err.message,
        "Must supply an API key! Check your request and try again."
    );
}

#[test]
fn long_requests_ask_typesafe_for_long_context() {
    let long_context = |enc: &EncodedRequest| {
        enc.headers
            .iter()
            .filter(|(k, _)| k.eq_ignore_ascii_case("typesafe-danger-zone-long-context"))
            .count()
    };
    let adapted = |c: &EgressCtx| c.adapted.iter().any(|a| a == "long_context");
    let with_state = |bytes: usize, instructions: usize| {
        DecideRequest::new("jev-latest", DecisionInput::Text("x".repeat(bytes)))
            .with_question(noul("renews", &"y".repeat(instructions)))
    };
    // 64k tokens a request and 32k a question, at 4 bytes a token.
    let limits = SystemOneProfile::TYPESAFE.long_context.unwrap();
    assert_eq!((limits.request, limits.question), (65_536, 32_768));
    let short = with_state(1000, 10);
    let (enc, c) = encode(&short, SystemOneProfile::TYPESAFE).unwrap();
    assert_eq!((long_context(&enc), adapted(&c)), (0, false));
    // The state with its one question is beyond 32k tokens, though the request is within 64k.
    let one_question = with_state(140_000, 10);
    let (enc, c) = encode(&one_question, SystemOneProfile::TYPESAFE).unwrap();
    assert_eq!(
        enc.headers.get("typesafe-danger-zone-long-context"),
        Some("true")
    );
    assert!(adapted(&c));
    // Just within both: 32,768 tokens are 131,072 bytes, quotes included.
    let (enc, _) = encode(&with_state(131_000, 10), SystemOneProfile::TYPESAFE).unwrap();
    assert_eq!(long_context(&enc), 0);
    // Many short questions about a short state add up beyond 64k tokens.
    let mut many = with_state(10, 10);
    many.questions = (0..40)
        .map(|i| noul(&format!("q{i}"), &"z".repeat(7000)))
        .collect();
    let (enc, c) = encode(&many, SystemOneProfile::TYPESAFE).unwrap();
    assert_eq!((long_context(&enc), adapted(&c)), (1, true));
    // Only TypeSafe has the header.
    for profile in [
        SystemOneProfile::OPENROUTER,
        SystemOneProfile::PERPLEXITY,
        SystemOneProfile::BASETEN,
        SystemOneProfile::GENERIC,
    ] {
        let (enc, c) = encode(&one_question, profile).unwrap();
        assert_eq!(
            (long_context(&enc), adapted(&c)),
            (0, false),
            "{}",
            profile.name
        );
    }
    // An app that sent the header itself keeps it, once, and nothing is adapted.
    let (body, h) = request_fixture("long-context");
    let mut own = decode_with(&body, &h);
    own.input = DecisionInput::Text("x".repeat(300_000));
    let (enc, c) = encode(&own, SystemOneProfile::TYPESAFE).unwrap();
    assert_eq!((long_context(&enc), adapted(&c)), (1, false));
}

#[test]
fn listings_are_openais() {
    let list = SystemOne::decode_models(200, &HeaderList::new(), br#"{"object":"list","data":[{"id":"jev-latest","object":"model","created":1,"owned_by":"typesafe"}]}"#).unwrap();
    assert_eq!(list.models[0].id, "jev-latest");
    assert_eq!(
        SystemOne::encode_models_request(&kaeso_kpp::ModelsQuery::default())
            .unwrap()
            .path,
        "/v1/models"
    );
    assert!(
        SystemOne::decode_models(401, &HeaderList::new(), br#"{"detail":"Invalid API key."}"#)
            .is_err()
    );
}

fn decide(body: &Value) -> Result<Request, KppError> {
    SystemOne::decode_request(RequestKind::Decide, &json_body(body), &HeaderList::new())
}

#[test]
fn uploads_are_checked_part_by_part() {
    let (json, _) = request_fixture_in("requests/multipart", "door");
    let photo = jpeg(300, 300);
    let decode_form = |fields: &[FormField<'_>]| {
        let form = FormWriter::write(fields);
        let h = HeaderList::new().with("content-type", form.content_type());
        SystemOne::decode_request(RequestKind::Decide, &form.into_body(), &h)
    };
    let request = FormField::Typed("request", "application/json", &json);
    for fields in [
        vec![],
        vec![
            request,
            FormField::File("door-photo", "d.jpg", "image/jpeg", &photo),
            FormField::File("door-photo", "e.jpg", "image/jpeg", &photo),
        ],
        vec![
            request,
            FormField::Typed("request", "application/json", &json),
        ],
    ] {
        assert!(decode_form(&fields).is_err());
    }
    let mut twice: Value = serde_json::from_slice(&json).unwrap();
    twice["assets"]["second"] = json!({"type": "image", "part": "door-photo"});
    twice["state"]["other"] = json!("second");
    let twice = json_body(&twice);
    let err = decode_form(&[
        FormField::Typed("request", "application/json", &twice),
        FormField::File("door-photo", "d.jpg", "image/jpeg", &photo),
    ])
    .unwrap_err();
    assert_eq!(err.param.as_deref(), Some("assets.second.part"));
    let missing = decode_form(&[
        FormField::Typed("request", "application/json", &json),
        FormField::File("other", "d.jpg", "image/jpeg", &photo),
    ])
    .unwrap_err();
    assert_eq!(missing.param.as_deref(), Some("assets.a3f1c2e8-door.part"));

    let mut both: Value = serde_json::from_slice(&json).unwrap();
    both["assets"]["a3f1c2e8-door"]["data_base64"] = json!("ignored");
    let both = json_body(&both);
    let Request::Decide(req) = decode_form(&[
        FormField::Typed("request", "application/json", &both),
        FormField::File("door-photo", "d.bin", "application/octet-stream", &photo),
    ])
    .unwrap() else {
        panic!()
    };
    let asset = &req.assets["a3f1c2e8-door"];
    assert_eq!(asset.source.media_type(), Some("image/jpeg"));
    let (enc, _) = encode(&req, SystemOneProfile::TYPESAFE).unwrap();
    let ct = enc.headers.get("content-type").unwrap();
    let boundary = crate::multipart::form_boundary(ct).unwrap();
    let parts = crate::multipart::read_form(
        &enc.body,
        &boundary,
        crate::multipart::FormLimits::new(4, 1 << 20),
    )
    .unwrap();
    let sent: Value = serde_json::from_slice(parts[0].body).unwrap();
    assert_eq!(sent["assets"]["a3f1c2e8-door"]["data_base64"], "ignored");
    assert_eq!(
        parts[1].content_type.as_deref(),
        Some("application/octet-stream")
    );
}

#[test]
fn more_shapes_system_one_refuses() {
    for (body, param) in [
        (
            json!({"model": "m", "state": "x", "questions": {"a": {"type": "noul"}}, "assets": []}),
            "assets",
        ),
        (
            json!({"model": "m", "state": "x", "questions": {"a": {"type": "noul"}}, "assets": {"r": 5}}),
            "assets.r",
        ),
        (
            json!({"model": "m", "state": "x", "questions": {"a": 5}}),
            "questions.a",
        ),
        (
            json!({"model": "m", "state": "x", "questions": {"a": {"type": "choice", "criteria": ["x"]}}}),
            "questions.a.criteria",
        ),
        (
            json!({"model": "m", "state": "x", "questions": {"a": {"type": "score"}}}),
            "questions.a.criteria",
        ),
        (
            json!({"model": "m", "questions": {"a": {"type": "noul"}}}),
            "state",
        ),
    ] {
        let err = decide(&body).unwrap_err();
        assert_eq!(err.param.as_deref(), Some(param), "{body}: {}", err.message);
    }
    let chat = Request::Generate(GenerateRequest::new("m").with_message(Message::user("hi")));
    let mut c = EgressCtx::new(&chat);
    assert!(SystemOne::encode_request(&chat, &mut c).is_err());
}

#[test]
fn requests_with_hints_paths_and_many_options_or_images() {
    let mut req = DecideRequest::new("jev-latest", DecisionInput::Text("x".into())).with_question(
        noul("a", "?").with_instructions(kaeso_kpp::Instructions::Blocks(vec![Block::text(
            "Only text",
        )])),
    );
    req.cache = Some(kaeso_kpp::DecisionCache::new(Some("k".into())));
    req.user = Some("user-1".into());
    let (enc, c) = encode(&req, SystemOneProfile::TYPESAFE).unwrap();
    assert_eq!(body_of(&enc)["questions"]["a"]["instructions"], "Only text");
    assert_eq!(c.left_out, ["cache", "user"]);
    let request = Request::Decide(req.clone());
    let mut c = ctx(SystemOneProfile::GENERIC, &request).with_base_path("/decisions/v1/systemone/");
    assert_eq!(
        SystemOne::encode_request(&request, &mut c).unwrap().path,
        "/decisions/v1/systemone"
    );
    let mut c = ctx(SystemOneProfile::OPENROUTER, &request).with_api_root("/api/v1");
    assert_eq!(
        SystemOne::encode_request(&request, &mut c).unwrap().path,
        "/api/v1/systemone"
    );

    let options = (0..256)
        .map(|i| ChoiceOption::new(ChoiceValue::Text(format!("o{i}")), None))
        .collect();
    let wide = DecideRequest::new("jev-latest", DecisionInput::Text("x".into())).with_question(
        Question::new(Some("c".into()), QuestionKind::Choice(Choice::new(options))),
    );
    assert_eq!(
        encode(&wide, SystemOneProfile::TYPESAFE)
            .unwrap_err()
            .unsupported_feature(),
        Some("options")
    );

    let items: Vec<Value> = (0..9).map(|i| json!(format!("img-{i}"))).collect();
    let mut many = DecideRequest::new(
        "pplx-decider-latest",
        DecisionInput::Json(RawJson::from_serialize(&items).unwrap()),
    )
    .with_question(noul("a", "?"));
    for i in 0..9 {
        many.assets.insert(
            format!("img-{i}"),
            Asset::new(Source::bytes("image/png", png(300, 300))),
        );
    }
    assert_eq!(
        encode(&many, SystemOneProfile::PERPLEXITY)
            .unwrap_err()
            .unsupported_feature(),
        Some("images")
    );
    many.assets.truncate(8);
    many.input = DecisionInput::Json(RawJson::from_serialize(&items[..8]).unwrap());
    encode(&many, SystemOneProfile::PERPLEXITY).unwrap();

    let mut big = png(300, 300);
    big.resize(5 * 1024 * 1024, 0);
    let req = with_image(json!(["img-1"]), big, "image/png");
    assert_eq!(
        encode(&req, SystemOneProfile::CLOUDFLARE_WORKERS_AI)
            .unwrap_err()
            .unsupported_feature(),
        Some("image size")
    );
    let unread = with_image(json!(["img-1"]), b"????".to_vec(), "image/png");
    let err = encode(&unread, SystemOneProfile::BASETEN).unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("image format"));
}

#[test]
fn state_parts_keep_their_url_header_and_fields() {
    let png64 = base64::engine::general_purpose::STANDARD.encode(png(300, 300));
    let body = json!({
        "model": "m",
        "state": [
            {"type": "image_url", "image_url": {"url": format!("data:image/png;name=a.png;base64,{png64}"), "detail": "auto"}, "x_note": 1},
            {"type": "image_url", "image_url": {"url": "https://example.com/a.png"}},
            {"type": "image_url", "image_url": "not an object"}
        ],
        "questions": {"a": {"type": "noul"}}
    });
    let Request::Decide(req) = decide(&body).unwrap() else {
        panic!()
    };
    assert_eq!(req.assets.len(), 2);
    let request = Request::Decide(req.clone());
    let mut c = ctx(
        SystemOneProfile::GENERIC.with_media(MediaPlacement::State),
        &request,
    );
    let enc = SystemOne::encode_request(&request, &mut c).unwrap();
    assert_json_eq("state-parts", &json_body(&body), &enc.body);
    assert_eq!(
        encode(&req, SystemOneProfile::PERPLEXITY)
            .unwrap_err()
            .unsupported_feature(),
        Some("image URL")
    );
    let bad = json!({"model": "m", "state": {"type": "image_url", "image_url": {"url": "data:image/png;base64,!!"}}, "questions": {"a": {"type": "noul"}}});
    assert_eq!(decide(&bad).unwrap_err().param.as_deref(), Some("state"));
}

fn reply_ctx(req: &DecideRequest) -> EgressCtx {
    encode(req, SystemOneProfile::GENERIC).unwrap().1
}

#[test]
fn replies_system_one_cant_read() {
    let req = DecideRequest::new("m", DecisionInput::Text("x".into()))
        .with_question(noul("a", "?"))
        .with_question(Question::new(
            Some("c".into()),
            QuestionKind::Choice(Choice::new(vec![
                ChoiceOption::new(ChoiceValue::Text("x".into()), None),
                ChoiceOption::new(ChoiceValue::Text("y".into()), None),
            ])),
        ));
    let c = reply_ctx(&req);
    let read = |v: Value| SystemOne::decode_response(200, &HeaderList::new(), &json_body(&v), &c);
    let noul = json!({"type": "noul", "noul": 0.5});
    for bad in [
        json!({"model": "m", "answers": {"a": {"type": "noul", "noul": "high"}, "c": {"type": "choice", "choice": "x", "probabilities": {}}}}),
        json!({"model": "m", "answers": {"a": noul, "c": {"type": "choice", "choice": "x", "probabilities": {"z": 0.1}}}}),
        json!({"model": "m", "answers": {"a": noul, "c": {"type": "choice", "choice": "x", "probabilities": {"x": "much"}}}}),
        json!({"model": "m", "answers": {"a": noul, "c": {"type": "choice", "choice": "x", "probabilities": []}}}),
        json!({"model": "m", "answers": {"a": {"type": "box", "box": {"top": 0, "bottom": 1, "left": 0, "right": 1}}, "c": {"type": "refusal"}}}),
        json!({"model": "m", "answers": []}),
        json!({"model": "m", "answers": {"a": noul, "c": {"type": "refusal"}}, "usage": {"output_tokens": 1}}),
    ] {
        assert!(read(bad.clone()).is_err(), "{bad}");
    }
    let Reply::Decide(r) =
        read(json!({"model": "m", "answers": {"a": noul, "c": {"type": "refusal"}}})).unwrap()
    else {
        panic!()
    };
    assert_eq!(r.answers[1].value, AnswerValue::Refusal(Refusal::new()));
    let out: Value = serde_json::from_slice(
        &SystemOne::encode_response(
            &Reply::Decide(r.clone()),
            &EncodeCtx::for_request(Request::Decide(req.clone())),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        out,
        json!({"model": "m", "answers": {"a": {"type": "noul", "noul": 0.5}, "c": {"type": "refusal"}}})
    );
    let err = SystemOne::decode_response(
        429,
        &HeaderList::new().with("retry-after", "1"),
        br#"{"detail":"Slow down."}"#,
        &c,
    )
    .unwrap_err();
    assert_eq!(err.kind, ErrorKind::RateLimited);
    let other = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off);
    assert!(SystemOne::decode_response(200, &HeaderList::new(), b"{}", &other).is_err());
    let cf = c
        .clone()
        .with_system_one_profile(SystemOneProfile::CLOUDFLARE_WORKERS_AI);
    for body in [&b"[]"[..], b"{\"success\":true}", b"not json"] {
        assert!(SystemOne::decode_response(200, &HeaderList::new(), body, &cf).is_err());
    }

    let mut wrong = r.clone();
    wrong.answers.swap(0, 1);
    assert!(
        SystemOne::encode_response(
            &Reply::Decide(wrong),
            &EncodeCtx::for_request(Request::Decide(req.clone()))
        )
        .is_err()
    );
    let mut short = r.clone();
    short.answers.pop();
    assert!(
        SystemOne::encode_response(
            &Reply::Decide(short),
            &EncodeCtx::for_request(Request::Decide(req.clone()))
        )
        .is_err()
    );
    let mut far = r;
    far.answers[1] = Answer::new(AnswerValue::Choice(ChoiceAnswer::new(7, vec![])));
    assert!(
        SystemOne::encode_response(
            &Reply::Decide(far.clone()),
            &EncodeCtx::for_request(Request::Decide(req.clone()))
        )
        .is_err()
    );
    far.answers[1] = Answer::new(AnswerValue::Choice(ChoiceAnswer::new(
        0,
        vec![Some(f64::NAN), None],
    )));
    assert!(
        SystemOne::encode_response(
            &Reply::Decide(far),
            &EncodeCtx::for_request(Request::Decide(req))
        )
        .is_err()
    );
}

#[test]
fn legends_come_from_labels_where_a_level_has_no_description() {
    let req =
        DecideRequest::new("m", DecisionInput::Text("x".into())).with_question(Question::new(
            Some("s".into()),
            QuestionKind::Score(Score::new(vec![
                Level::new(Some("low".into()), None),
                Level::new(
                    None,
                    Some(Description::Json(
                        RawJson::from_string("{\"a\":1}".into()).unwrap(),
                    )),
                ),
                Level::default(),
            ])),
        ));
    let reply = DecideResponse::new(
        "m",
        vec![Answer::new(AnswerValue::Score(ScoreAnswer::new(
            1.0,
            vec![None, Some(1.0), None],
        )))],
    );
    let out: Value = serde_json::from_slice(
        &SystemOne::encode_response(
            &Reply::Decide(reply),
            &EncodeCtx::for_request(Request::Decide(req)),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        out["answers"]["s"]["legend"],
        json!({"0": "low", "1": {"a": 1}, "2": null})
    );
    assert_eq!(out["answers"]["s"]["probabilities"], json!({"1": 1.0}));
}

#[test]
fn provider_errors_in_every_shape() {
    let read = |status: u16, body: Value| {
        SystemOne::decode_error(status, &HeaderList::new(), &json_body(&body))
    };
    for (status, kind) in [
        (402, ErrorKind::Billing),
        (403, ErrorKind::PermissionDenied),
        (404, ErrorKind::NotFound),
        (504, ErrorKind::Timeout),
        (503, ErrorKind::Overloaded),
        (502, ErrorKind::ProviderError),
    ] {
        assert_eq!(read(status, json!({"detail": "x"})).kind, kind);
    }
    let err = read(
        422,
        json!({"detail": [{"loc": ["body", "questions", 0], "msg": "bad", "type": "x"}]}),
    );
    assert_eq!(err.param.as_deref(), Some("questions.0"));
    let err = read(
        422,
        json!({"detail": [{"loc": [], "msg": "bad", "type": "x"}]}),
    );
    assert_eq!((err.message.as_str(), err.param), ("bad", None));
    assert_eq!(read(422, json!({"detail": []})).message, "[]");
    assert_eq!(read(400, json!({"detail": 5})).message, "5");
    assert_eq!(
        read(400, json!({"detail": {"code": 1}})).message,
        r#"{"code":1}"#
    );
    assert!(
        read(400, json!({"detail": " "}))
            .message
            .contains("without a message")
    );
    for (kind, status) in [
        (ErrorKind::PermissionDenied, 403),
        (ErrorKind::NotFound, 404),
        (ErrorKind::Timeout, 504),
        (ErrorKind::Billing, 402),
        (ErrorKind::ContentFiltered, 400),
    ] {
        assert_eq!(
            SystemOne::encode_error(&KppError::new(kind, "x")).status,
            status
        );
    }
    let upstream = KppError::provider("x").with_status(502);
    assert_eq!(SystemOne::encode_error(&upstream).status, 502);
    assert_eq!(
        SystemOne::encode_error(&KppError::provider("x").with_status(529)).status,
        500
    );
    let odd = KppError::invalid_request("x").with_param("a[b");
    assert_eq!(
        serde_json::from_slice::<Value>(&SystemOne::encode_error(&odd).body).unwrap()["detail"][0]
            ["loc"],
        json!(["body", "a"])
    );
}

const CLOUDFLARE: &str = "providers/cloudflare-ai-gateway";
const CLOUDFLARE_ROOT: &str = "/client/v4/accounts/0123456789abcdef0123456789abcdef/ai/v1";

fn cloudflare_file(name: &str) -> Vec<u8> {
    std::fs::read(
        crate::test_support::fixtures_dir()
            .join(DIR)
            .join(CLOUDFLARE)
            .join(format!("{name}.json")),
    )
    .unwrap()
}

fn cloudflare_ctx(req: &Request) -> EgressCtx {
    ctx(SystemOneProfile::CLOUDFLARE_WORKERS_AI, req).with_api_root(CLOUDFLARE_ROOT)
}

#[test]
fn cloudflare_takes_images_in_a_list_before_the_state_beside_the_accounts_v1() {
    let req = Request::Decide(decode(&cloudflare_file("images/colour.app")));
    let mut c = cloudflare_ctx(&req);
    let enc = SystemOne::encode_request(&req, &mut c).unwrap();
    assert_json_eq(
        "images/colour.sent",
        &cloudflare_file("images/colour.sent"),
        &enc.body,
    );
    assert_eq!(
        enc.path,
        "/client/v4/accounts/0123456789abcdef0123456789abcdef/ai/run/@cf/cloudflare/clef"
    );
    assert!(c.adapted.is_empty(), "{:?}", c.adapted);
    assert_eq!(enc.headers.get("typesafe-danger-zone-images"), None);

    let reply = SystemOne::decode_response(
        200,
        &HeaderList::new(),
        &cloudflare_file("images/recorded-colour"),
        &c,
    )
    .unwrap();
    let Reply::Decide(r) = &reply else { panic!() };
    assert_eq!(r.model, "@cf/cloudflare/clef");
    assert_eq!((r.usage.input_tokens, r.usage.output_tokens), (218, 0));
    let Request::Decide(app) = req else { panic!() };
    let out: Value = serde_json::from_slice(
        &SystemOne::encode_response(&reply, &EncodeCtx::for_request(Request::Decide(app))).unwrap(),
    )
    .unwrap();
    assert_eq!(out["answers"]["colour"]["choice"], "red");
    assert_eq!(out["model"], "@cf/cloudflare/clef");
    assert_eq!(
        out.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["answers", "model", "usage"]
    );
}

#[test]
fn an_image_after_text_moves_before_the_state_on_cloudflare() {
    let req = with_image(json!(["A photo:", "img-1"]), png(300, 300), "image/png");
    let (enc, c) = encode(&req, SystemOneProfile::CLOUDFLARE_WORKERS_AI).unwrap();
    assert_eq!(body_of(&enc)["state"], json!(["A photo:"]));
    assert_eq!(body_of(&enc)["images"].as_array().unwrap().len(), 1);
    assert_eq!(c.adapted, ["image_order"]);
}

#[test]
fn cloudflare_refuses_what_clef_refuses_before_sending() {
    let cf = SystemOneProfile::CLOUDFLARE_WORKERS_AI;
    let one_option = DecideRequest::new("@cf/cloudflare/clef", DecisionInput::Text("x".into()))
        .with_question(
            Question::new(
                Some("c".into()),
                QuestionKind::Choice(Choice::new(vec![ChoiceOption::new(
                    ChoiceValue::Text("only".into()),
                    None,
                )])),
            )
            .with_instructions(kaeso_kpp::Instructions::Text("Which?".into())),
        );
    let err = encode(&one_option, cf).unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("options"));
    assert_eq!(err.param.as_deref(), Some("questions[0]"));
    assert!(
        err.message.contains("at least 2 options"),
        "{}",
        err.message
    );
    encode(&one_option, SystemOneProfile::TYPESAFE).unwrap();

    let one_level = DecideRequest::new("@cf/cloudflare/clef", DecisionInput::Text("x".into()))
        .with_question(
            Question::new(
                Some("s".into()),
                QuestionKind::Score(Score::new(vec![Level::new(
                    None,
                    Some(Description::Text("only".into())),
                )])),
            )
            .with_instructions(kaeso_kpp::Instructions::Text("How bad?".into())),
        );
    let err = encode(&one_level, cf).unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("levels"));
    assert!(err.message.contains("at least 2 levels"), "{}", err.message);

    for instructions in [None, Some(kaeso_kpp::Instructions::Text(String::new()))] {
        let mut q = noul("a", "?");
        q.instructions = instructions;
        let req = DecideRequest::new("@cf/cloudflare/clef", DecisionInput::Text("x".into()))
            .with_question(q);
        let err = encode(&req, cf).unwrap_err();
        assert_eq!(
            err.unsupported_feature(),
            Some("question without instructions")
        );
        assert_eq!(err.param.as_deref(), Some("questions[0].instructions"));
        encode(&req, SystemOneProfile::TYPESAFE).unwrap();
    }

    let (body, _) = request_fixture("unknown-top-level-field");
    let err = encode(&decode(&body), cf).unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("x_future_field"));

    let huge = with_image(json!(["img-1"]), png(5000, 4000), "image/png");
    let err = encode(&huge, cf).unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("image size"));
    assert!(err.message.contains("16 megapixels"), "{}", err.message);
    encode(&huge, SystemOneProfile::GENERIC).unwrap();

    let mut three = png(300, 300);
    three.resize(3 * 1024 * 1024, 0);
    let heavy = with_image(
        json!(["img-1", "img-2", "img-3"]),
        three.clone(),
        "image/png",
    )
    .with_asset(
        "img-2",
        Asset::new(Source::bytes("image/png", three.clone())),
    )
    .with_asset("img-3", Asset::new(Source::bytes("image/png", three)));
    let err = encode(&heavy, cf).unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("image size"));
    assert_eq!(err.param.as_deref(), Some("assets"));
    assert!(err.message.contains("8 MiB of images"), "{}", err.message);
}

#[test]
fn a_short_model_name_in_a_reply_gets_its_prefix_back() {
    let req = DecideRequest::new("@cf/cloudflare/clef-flash", DecisionInput::Text("x".into()))
        .with_question(noul("a", "?"));
    let (enc, c) = encode(&req, SystemOneProfile::CLOUDFLARE_WORKERS_AI).unwrap();
    assert_eq!(body_of(&enc)["model"], "clef-flash");
    assert_eq!(enc.path, "/run/@cf/cloudflare/clef-flash");
    let reply = |model: &str| json!({"result": {"model": model, "answers": {"a": {"type": "noul", "noul": 0.5}}, "usage": {"input_tokens": 3, "output_tokens": 0}}, "success": true, "errors": [], "messages": []});
    for (sent, read) in [
        ("clef-flash", "@cf/cloudflare/clef-flash"),
        ("@cf/cloudflare/clef-flash", "@cf/cloudflare/clef-flash"),
        ("", ""),
    ] {
        let Reply::Decide(r) =
            SystemOne::decode_response(200, &HeaderList::new(), &json_body(&reply(sent)), &c)
                .unwrap()
        else {
            panic!()
        };
        assert_eq!(r.model, read, "{sent}");
    }
}
