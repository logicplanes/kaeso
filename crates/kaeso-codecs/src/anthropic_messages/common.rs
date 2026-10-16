use indexmap::IndexMap;
use kaeso_kpp::{DialectId, Extras, KppError, RawJson, StopReason};
use serde::Serialize;
use serde_json::Value;
use serde_json::value::RawValue;

use super::wire::Raw;
use crate::wire::{Ext, nested_extra_for, verbatim_from_form};

pub(crate) const D: DialectId = DialectId::AnthropicMessages;

pub(crate) fn to_raw<T: Serialize + ?Sized>(value: &T) -> Result<Raw, KppError> {
    serde_json::value::to_raw_value(value).map_err(internal)
}

pub(crate) fn internal(e: impl std::fmt::Display) -> KppError {
    KppError::provider(format!(
        "Kaeso could not build an Anthropic Messages payload ({e}). This is a bug in Kaeso; please report it."
    ))
}

pub(crate) fn empty_object() -> Raw {
    RawJson::empty_object().into_inner()
}

pub(crate) fn is_empty_object(raw: &RawValue) -> bool {
    matches!(serde_json::from_str::<Value>(raw.get()), Ok(Value::Object(m)) if m.is_empty())
}

pub(crate) fn scoped(scope: &str, key: &str) -> String {
    if scope.is_empty() {
        key.to_owned()
    } else {
        format!("{scope}:{key}")
    }
}

pub(crate) fn nested_ext<T>(known: T, extras: &Extras, prefix: &str) -> Ext<T> {
    Ext::with(
        known,
        nested_extra_for(extras, D, prefix),
        verbatim_from_form(extras, D, prefix),
    )
}

pub(crate) fn record_spelling(
    extras: &mut Extras,
    scope: &str,
    field: &str,
    actual: Option<Value>,
) {
    let marker = match actual {
        None => serde_json::json!({ "absent": true }),
        Some(v) => serde_json::json!({ "value": v }),
    };
    extras
        .form
        .insert(scoped(scope, &format!("spelling:{field}")), marker);
}

pub(crate) fn spelling(extras: &Extras, scope: &str, field: &str) -> Option<Option<Value>> {
    let marker = extras.form_value(D, &scoped(scope, &format!("spelling:{field}")))?;
    if marker.get("absent").and_then(Value::as_bool) == Some(true) {
        return Some(None);
    }
    marker.get("value").map(|v| Some(v.clone()))
}

pub(crate) fn no_position(err: &impl std::fmt::Display) -> String {
    let text = err.to_string();
    match text.find(" at line ") {
        Some(i) => text.get(..i).unwrap_or(&text).to_owned(),
        None => text,
    }
}

pub(crate) fn request_kind_name(kind: kaeso_kpp::RequestKind) -> &'static str {
    match kind {
        kaeso_kpp::RequestKind::Generate => "message generation request",
        kaeso_kpp::RequestKind::CountTokens => "token counting request",
        kaeso_kpp::RequestKind::Embed => "embeddings request",
        _ => "request of this kind",
    }
}

pub(crate) fn is_data_url(url: &str) -> bool {
    url.get(..5)
        .is_some_and(|p| p.eq_ignore_ascii_case("data:"))
}

pub(crate) fn parse_base64_data_url(url: &str) -> Option<(String, String)> {
    if !is_data_url(url) {
        return None;
    }
    let (meta, data) = url.get(5..)?.split_once(',')?;
    let mut parts = meta.split(';');
    let media_type = parts.next()?.trim().to_ascii_lowercase();
    let base64 = parts
        .next_back()
        .is_some_and(|p| p.trim().eq_ignore_ascii_case("base64"));
    (base64 && !media_type.is_empty()).then(|| (media_type, data.to_owned()))
}

pub(crate) const STOP_REASONS: [&str; 7] = [
    "end_turn",
    "max_tokens",
    "stop_sequence",
    "tool_use",
    "pause_turn",
    "refusal",
    "model_context_window_exceeded",
];

pub(crate) fn stop_from_wire(reason: &str, sequence: Option<&str>) -> StopReason {
    match reason {
        "end_turn" => StopReason::EndTurn,
        "max_tokens" | "model_context_window_exceeded" => StopReason::MaxTokens,
        "stop_sequence" => StopReason::StopSequence(sequence.unwrap_or_default().to_owned()),
        "tool_use" => StopReason::ToolUse,
        "pause_turn" => StopReason::PauseTurn,
        "refusal" => StopReason::Refusal,
        other => StopReason::Other(other.to_owned()),
    }
}

pub(crate) fn stop_to_wire(stop: &StopReason, own: bool) -> (String, Option<String>) {
    let reason = match stop {
        StopReason::EndTurn => "end_turn",
        StopReason::MaxTokens => "max_tokens",
        StopReason::StopSequence(s) => return ("stop_sequence".to_owned(), Some(s.clone())),
        StopReason::ToolUse => "tool_use",
        StopReason::ContentFilter | StopReason::Refusal => "refusal",
        StopReason::PauseTurn => "pause_turn",
        StopReason::Other(s) if own || STOP_REASONS.contains(&s.as_str()) => s.as_str(),
        StopReason::Other(s) => match s.as_str() {
            "length" | "max_output_tokens" | "max_completion_tokens" => "max_tokens",
            "function_call" | "tool_calls" => "tool_use",
            "content_filter" => "refusal",
            _ => "end_turn",
        },
        _ => "end_turn",
    };
    (reason.to_owned(), None)
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Rewrites {
    pub(crate) ids: IndexMap<String, String>,
    pub(crate) names: IndexMap<String, String>,
    sent_ids: IndexMap<String, String>,
    sent_names: IndexMap<String, String>,
}

impl Rewrites {
    pub(crate) fn id(&mut self, original: &str) -> String {
        rewrite(
            original,
            valid_id,
            usize::MAX,
            &mut self.sent_ids,
            &mut self.ids,
        )
    }

    pub(crate) fn name(&mut self, original: &str) -> String {
        rewrite(
            original,
            valid_name,
            MAX_NAME_LEN,
            &mut self.sent_names,
            &mut self.names,
        )
    }
}

pub(crate) const MAX_NAME_LEN: usize = 128;

fn valid_chars(s: &str) -> bool {
    s.bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

pub(crate) fn valid_id(id: &str) -> bool {
    !id.is_empty() && valid_chars(id)
}

pub(crate) fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= MAX_NAME_LEN && valid_chars(name)
}

fn rewrite(
    original: &str,
    valid: fn(&str) -> bool,
    max_len: usize,
    forward: &mut IndexMap<String, String>,
    reverse: &mut IndexMap<String, String>,
) -> String {
    if let Some(sent) = forward.get(original) {
        return sent.clone();
    }
    if valid(original) {
        return original.to_owned();
    }
    let suffix = format!("_{:016x}", fnv1a(original.as_bytes()));
    let keep = max_len.saturating_sub(suffix.len());
    let mut sent: String = original
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .take(keep)
        .collect();
    sent.push_str(&suffix);
    let mut candidate = sent.clone();
    let mut n = 0u32;
    while reverse.get(&candidate).is_some_and(|o| o != original) {
        n = n.saturating_add(1);
        candidate = format!(
            "{}{n}",
            sent.get(..sent.len().saturating_sub(4)).unwrap_or(&sent)
        );
    }
    forward.insert(original.to_owned(), candidate.clone());
    reverse.insert(candidate.clone(), original.to_owned());
    candidate
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_values_are_kept_and_invalid_ones_rewritten_deterministically() {
        let mut r = Rewrites::default();
        assert_eq!(r.id("toolu_01ABC-x"), "toolu_01ABC-x");
        assert!(r.ids.is_empty());
        let sent = r.id("call.1:abc");
        assert!(valid_id(&sent), "{sent}");
        assert!(sent.starts_with("call_1_abc_"));
        assert_eq!(r.id("call.1:abc"), sent, "stable within a request");
        assert_eq!(Rewrites::default().id("call.1:abc"), sent, "deterministic");
        assert_eq!(r.ids.get(&sent).map(String::as_str), Some("call.1:abc"));
        assert_ne!(r.id("call_1:abc"), sent, "different originals differ");

        let long = "n".repeat(200);
        let name = r.name(&long);
        assert!(valid_name(&name) && name.len() == MAX_NAME_LEN, "{name}");
        assert_eq!(r.names.get(&name), Some(&long));
        let empty = r.name("");
        assert!(valid_name(&empty));
        assert!(valid_name(&r.name("ünïcode tool")));
    }

    #[test]
    fn base64_data_urls_parse() {
        assert_eq!(
            parse_base64_data_url("data:image/PNG;base64,AAAA"),
            Some(("image/png".into(), "AAAA".into()))
        );
        assert_eq!(
            parse_base64_data_url("DATA:application/pdf;name=x.pdf;base64,JVBE"),
            Some(("application/pdf".into(), "JVBE".into()))
        );
        assert_eq!(parse_base64_data_url("data:text/plain,hello"), None);
        assert_eq!(parse_base64_data_url("data:;base64,AAAA"), None);
        assert_eq!(parse_base64_data_url("https://x/y.png"), None);
        assert!(is_data_url("data:x") && !is_data_url("dat"));
    }

    #[test]
    fn stop_reasons_round_trip() {
        for s in [
            "end_turn",
            "max_tokens",
            "tool_use",
            "pause_turn",
            "refusal",
        ] {
            assert_eq!(
                stop_to_wire(&stop_from_wire(s, None), false),
                (s.to_owned(), None)
            );
        }
        assert_eq!(
            stop_from_wire("model_context_window_exceeded", None),
            StopReason::MaxTokens
        );
        let other = |s: &str| StopReason::Other(s.to_owned());
        assert_eq!(
            stop_to_wire(&other("future_reason"), true).0,
            "future_reason"
        );
        assert_eq!(stop_to_wire(&other("length"), false).0, "max_tokens");
        assert_eq!(stop_to_wire(&other("function_call"), false).0, "tool_use");
        assert_eq!(stop_to_wire(&other("weird"), false).0, "end_turn");
        assert_eq!(
            stop_from_wire("stop_sequence", Some("END")),
            StopReason::StopSequence("END".into())
        );
        assert_eq!(stop_to_wire(&StopReason::ContentFilter, false).0, "refusal");
    }
}
