use kaeso_kpp::{
    Block, DialectId, Extras, GenerateRequest, KppError, Message, Origin, RawJson, Reply, Request,
    RequestKind, Response, StreamEvent,
};
use serde_json::Value;

use super::*;
use crate::test_support::{assert_json_eq, assert_sse_eq, encoded_error, fixtures, sidecar};

const DIR: &str = "openai_responses";
const R: DialectId = DialectId::OpenAiResponses;

const REFUSED_REQUESTS: &[(&str, &str)] = &[
    ("stateful-background", "background"),
    ("stateful-conversation", "conversation"),
    ("stateful-conversation-id-string", "conversation"),
    ("stateful-previous-response-id", "previous_response_id"),
];

const FAILED_RESPONSES: &[&str] = &["failed"];

fn raw(s: &str) -> RawJson {
    RawJson::from_string(s.to_owned()).unwrap()
}

fn fixture(folder: &str, name: &str, ext: &str) -> Vec<u8> {
    fixtures(DIR, folder, ext)
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap_or_else(|| panic!("no fixture {folder}/{name}"))
        .1
}

fn decode_req(kind: RequestKind, body: &[u8]) -> Request {
    OpenAiResponses::decode_request(kind, body, &HeaderList::new()).unwrap()
}

fn generate(name: &str) -> GenerateRequest {
    match decode_req(RequestKind::Generate, &fixture("requests", name, "json")) {
        Request::Generate(r) => r,
        other => panic!("{other:?}"),
    }
}

fn encode_req(req: &Request) -> (EncodedRequest, EgressCtx) {
    let mut ctx = EgressCtx::new(req).with_provider("openai");
    let enc = OpenAiResponses::encode_request(req, &mut ctx).unwrap();
    (enc, ctx)
}

fn body_of(req: GenerateRequest) -> Value {
    let (enc, _) = encode_req(&Request::Generate(req));
    serde_json::from_slice(&enc.body).unwrap()
}

fn gen_ctx() -> EgressCtx {
    EgressCtx::for_kind(RequestKind::Generate, kaeso_kpp::StreamMode::Off).with_provider("openai")
}

fn decode_resp(body: &[u8]) -> Response {
    match OpenAiResponses::decode_response(200, &HeaderList::new(), body, &gen_ctx()).unwrap() {
        Reply::Generate(r) => r,
        other => panic!("{other:?}"),
    }
}

fn feature(req: GenerateRequest) -> String {
    OpenAiResponses::check(&Request::Generate(req), &crate::test_support::egress())
        .unwrap_err()
        .unsupported_feature()
        .unwrap_or("<not unsupported>")
        .to_owned()
}

mod check;
mod foreign;
mod malformed;
mod replies;
mod requests;
mod stream_edges;
mod streams;
