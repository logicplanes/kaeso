use kaeso_kpp::{Extras, GenerateRequest, Role, StreamMode};
use serde_json::Value;

use super::DIALECT;
use super::input::{INSTRUCTIONS, SYSTEM_ITEMS};
use super::overrides::{CANON, SET, UNSET, form_set_insert};
use super::request::INPUT;

// SIWC "Preview limitations": the plan route refuses these fields, `previous_response_id`
// over HTTP included, and wants `stream: true` and `store: false` on every request.
const REFUSED: &[&str] = &[
    "background",
    "conversation",
    "max_output_tokens",
    "max_tool_calls",
    "metadata",
    "moderation",
    "multi_agent",
    "prompt",
    "prompt_cache_retention",
    "safety_identifier",
    "temperature",
    "top_logprobs",
    "top_p",
    "truncation",
    "user",
    "previous_response_id",
    "store",
    "stream",
];

const ROLE: &str = "role";

fn refused(path: &str) -> bool {
    REFUSED.contains(&path.split('.').next().unwrap_or(path))
}

fn forget(extras: &mut Extras, drop: impl Fn(&str) -> bool) {
    extras.fields.retain(|path, _| !drop(path));
    for map in [SET, CANON] {
        if let Some(Value::Object(entries)) = extras.form.get_mut(map) {
            entries.retain(|path, _| !drop(path));
        }
    }
    if let Some(Value::Array(paths)) = extras.form.get_mut(UNSET) {
        paths.retain(|p| !p.as_str().is_some_and(&drop));
    }
}

/// Rewrites a request for the Responses route that serves a ChatGPT plan through Sign in
/// with ChatGPT ([preview limitations](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations)).
///
/// It leaves out every field that route refuses, wherever the request keeps it (typed
/// fields, extras, and a Responses app's explicit defaults and nulls), asks for a stream
/// that OpenAI doesn't store, sends `input` as an array, and sends every system text as
/// `instructions` or a `developer` message, since the route refuses `system` items.
pub fn for_chatgpt_plan(req: &mut GenerateRequest) {
    req.sampling.max_output_tokens = None;
    req.sampling.temperature = None;
    req.sampling.top_p = None;
    req.user = None;
    req.stream = StreamMode::on(true);
    forget(&mut req.extras, refused);
    if req.extras.is_from(DIALECT) {
        req.extras
            .form
            .retain(|name, _| ![INPUT, SYSTEM_ITEMS, INSTRUCTIONS].contains(&name.as_str()));
        form_set_insert(&mut req.extras, "store".to_owned(), Value::Bool(false));
    }
    for message in req.messages.iter_mut().filter(|m| m.role == Role::System) {
        forget(&mut message.extras, |path| path == ROLE);
    }
}
