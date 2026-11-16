use std::borrow::Cow;
use std::collections::HashSet;

use kaeso_kpp::DialectId;
use kaeso_store::clickhouse::parts::{self, Hash};
use serde::Deserialize;
use serde_json::value::RawValue;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

// Smaller elements cost more as a hash and a row of their own than they would save.
pub(crate) const MIN_PART_BYTES: usize = 512;

/// The members of a request that hold its history, which an agent resends every turn.
pub(crate) fn history_keys(dialect: DialectId) -> &'static [&'static str] {
    match dialect {
        DialectId::AnthropicMessages | DialectId::OpenAiChat => &["system", "messages", "tools"],
        DialectId::OpenAiResponses => &["instructions", "input", "tools"],
        DialectId::Gemini => &["systemInstruction", "contents", "tools"],
        _ => &[],
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct Split {
    pub(crate) skeleton: String,
    pub(crate) placeholders: u32,
    /// Each distinct part once, in the order the body first names it.
    pub(crate) parts: Vec<(Hash, String)>,
}

pub(crate) fn hash(bytes: &str) -> Hash {
    let digest = Sha256::digest(bytes.as_bytes());
    let mut out = [0u8; 16];
    out.copy_from_slice(digest.get(..16).unwrap_or(&[0u8; 16]));
    out
}

struct Members<'a>(Vec<(Cow<'a, str>, &'a RawValue)>);

impl<'de> Deserialize<'de> for Members<'de> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = Members<'de>;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a JSON object")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Members<'de>, A::Error> {
                let mut out = Vec::new();
                while let Some((k, v)) = map.next_entry::<Cow<'de, str>, &'de RawValue>()? {
                    out.push((k, v));
                }
                Ok(Members(out))
            }
        }
        d.deserialize_map(V)
    }
}

// Where `inner`, a slice serde_json borrowed from `body`, starts in it.
fn offset_in(body: &str, inner: &str) -> Option<usize> {
    let at = (inner.as_ptr() as usize).checked_sub(body.as_ptr() as usize)?;
    (body.get(at..at + inner.len()) == Some(inner)).then_some(at)
}

/// The body with every history element of at least [`MIN_PART_BYTES`] cut out under the hash of
/// its exact bytes. `None` keeps the body whole: it isn't a JSON object, has no such element, or
/// already reads like a body with placeholders.
pub(crate) fn split(body: &str, keys: &[&str]) -> Option<Split> {
    if keys.is_empty() || body.len() < MIN_PART_BYTES || parts::mentions_placeholders(body) {
        return None;
    }
    let Members(members) = serde_json::from_str(body).ok()?;
    let mut cuts: Vec<&str> = Vec::new();
    for (key, value) in &members {
        if !keys.contains(&key.as_ref()) {
            continue;
        }
        if value.get().starts_with('[') {
            let elements: Vec<&RawValue> = serde_json::from_str(value.get()).ok()?;
            cuts.extend(
                elements
                    .iter()
                    .map(|e| e.get())
                    .filter(|e| e.len() >= MIN_PART_BYTES),
            );
        } else if value.get().len() >= MIN_PART_BYTES {
            cuts.push(value.get());
        }
    }
    if cuts.is_empty() {
        return None;
    }
    let mut placed: Vec<(usize, &str)> = Vec::with_capacity(cuts.len());
    for cut in cuts {
        placed.push((offset_in(body, cut)?, cut));
    }
    placed.sort_unstable_by_key(|(at, _)| *at);
    let mut skeleton = String::with_capacity(body.len() / 4);
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let mut from = 0;
    for (at, cut) in &placed {
        let h = hash(cut);
        skeleton.push_str(body.get(from..*at)?);
        skeleton.push_str(&parts::placeholder(&h));
        from = at + cut.len();
        if seen.insert(h) {
            out.push((h, (*cut).to_owned()));
        }
    }
    skeleton.push_str(body.get(from..)?);
    Some(Split {
        skeleton,
        placeholders: u32::try_from(placed.len()).unwrap_or(u32::MAX),
        parts: out,
    })
}

/// What a gateway last wrote of one part: a request that started within its span, or up to
/// [`parts::REWRITE_EVERY`] after it, and expires no later relies on that row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Written {
    pub first_at: OffsetDateTime,
    pub last_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
}

impl Written {
    pub(crate) fn covers(&self, started_at: OffsetDateTime, expires_at: OffsetDateTime) -> bool {
        let rewrite =
            time::Duration::try_from(parts::REWRITE_EVERY).unwrap_or(time::Duration::HOUR);
        started_at >= self.first_at
            && started_at <= self.last_at + rewrite
            && expires_at <= self.expires_at
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn long(n: usize) -> String {
        "x".repeat(n)
    }

    fn round_trip(body: &str, dialect: DialectId) -> Split {
        let s = split(body, history_keys(dialect)).unwrap();
        let parts: HashMap<Hash, String> = s.parts.iter().cloned().collect();
        assert_eq!(parts::splice(&s.skeleton, &parts), body);
        assert_eq!(parts::hashes(&s.skeleton).len(), s.placeholders as usize);
        s
    }

    #[test]
    fn each_dialect_cuts_its_history_and_keeps_the_rest_inline() {
        let big = long(600);
        let anthropic = format!(
            r#"{{"model":"claude","system":"{big}","messages":[{{"role":"user","content":"hi"}}, {{"role":"user","content":"{big}"}}],"tools":[{{"name":"t","description":"{big}"}}],"metadata":{{"note":"{big}"}}}}"#
        );
        let s = round_trip(&anthropic, DialectId::AnthropicMessages);
        assert_eq!(s.placeholders, 3, "system, one long message, one tool");
        assert!(
            s.skeleton.contains(r#"{"role":"user","content":"hi"}"#),
            "short ones stay"
        );
        assert!(s.skeleton.contains(&big), "metadata isn't history");
        assert_eq!(s.parts.len(), 3);

        let chat = format!(
            r#"{{"model":"gpt","messages":[{{"role":"system","content":"{big}"}},{{"role":"user","content":"{big}"}}]}}"#
        );
        assert_eq!(round_trip(&chat, DialectId::OpenAiChat).placeholders, 2);

        let responses = format!(
            r#"{{"model":"gpt","instructions":"{big}","input":[{{"type":"message","role":"user","content":"{big}"}},{{"type":"function_call_output","call_id":"c","output":"ok"}}],"tools":[]}}"#
        );
        assert_eq!(
            round_trip(&responses, DialectId::OpenAiResponses).placeholders,
            2
        );

        let gemini = format!(
            r#"{{"systemInstruction":{{"parts":[{{"text":"{big}"}}]}},"contents":[{{"role":"user","parts":[{{"text":"{big}"}}]}}],"generationConfig":{{}}}}"#
        );
        assert_eq!(round_trip(&gemini, DialectId::Gemini).placeholders, 2);
    }

    #[test]
    fn spacing_escapes_and_repeats_come_back_byte_for_byte() {
        let big = long(700);
        let body = format!(
            "{{ \"messages\" : [\n  {{\"role\": \"user\", \"content\": \"{big}\\u00e9\\n\"}} ,\n  {{\"role\": \"user\", \"content\": \"{big}\\u00e9\\n\"}}\n ] , \"m\\u0065ssages\": [] }}"
        );
        let s = round_trip(&body, DialectId::AnthropicMessages);
        assert_eq!(s.placeholders, 2);
        assert_eq!(s.parts.len(), 1, "the same bytes are one part");
    }

    #[test]
    fn bodies_without_long_history_stay_whole() {
        let big = long(600);
        let small = r#"{"messages":[{"role":"user","content":"hi"}]}"#;
        for (body, keys) in [
            (small.to_owned(), history_keys(DialectId::AnthropicMessages)),
            (
                format!(r#"{{"messages":[{{"content":"{}"}}]}}"#, long(400)),
                history_keys(DialectId::AnthropicMessages),
            ),
            (
                format!(r#"{{"messages":[{{"content":"{big}"}}]}}"#),
                history_keys(DialectId::OpenAiEmbeddings),
            ),
            (format!(r#"["{big}"]"#), history_keys(DialectId::OpenAiChat)),
            (
                format!(r#"{{"messages":[{{"content":"{big}"}}],"#),
                history_keys(DialectId::OpenAiChat),
            ),
            (
                format!(r#"{{"messages":[{{"content":"{big}","x":{{"$kaeso_part":"a"}}}}]}}"#),
                history_keys(DialectId::OpenAiChat),
            ),
            (
                format!(r#"{{"input":"{big}"}}"#),
                history_keys(DialectId::OpenAiChat),
            ),
        ] {
            assert_eq!(split(&body, keys), None, "{body:.80}");
        }
        let string_input = format!(r#"{{"input":"{big}"}}"#);
        assert_eq!(
            round_trip(&string_input, DialectId::OpenAiResponses).placeholders,
            1
        );
    }

    #[test]
    fn a_written_part_covers_requests_within_its_span_and_the_interval() {
        let t = OffsetDateTime::UNIX_EPOCH + time::Duration::days(10);
        let w = Written {
            first_at: t,
            last_at: t,
            expires_at: t + time::Duration::days(30),
        };
        let exp = |at: OffsetDateTime| at + time::Duration::days(30) - time::Duration::hours(2);
        assert!(w.covers(t, w.expires_at));
        assert!(w.covers(t + time::Duration::minutes(59), exp(t)));
        assert!(
            !w.covers(t + time::Duration::minutes(61), exp(t)),
            "rewrite after the interval"
        );
        assert!(
            !w.covers(t - time::Duration::seconds(1), exp(t)),
            "an earlier request widens the span"
        );
        assert!(
            !w.covers(t, w.expires_at + time::Duration::seconds(1)),
            "a longer retention moves the expiry"
        );
    }
}
