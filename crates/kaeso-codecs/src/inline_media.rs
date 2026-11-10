use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use base64::Engine as _;
use kaeso_kpp::{Base64Source, Base64Text, KppError};
use serde::{Serialize, Serializer};

const WRITE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

// The request encoders build each part and message as JSON text and the body from those, so
// base64 written where a part is built would be copied once per layer. A deferring `Media`
// hands out a short marker in its place and `finish` writes the base64 into the body once, at
// its final size. Each marker must appear exactly once, as a whole JSON string, or the body is
// refused: text that spells a marker can make a request fail but never moves media.
#[derive(Debug)]
pub(crate) struct Media {
    deferring: bool,
    tag: Option<String>,
    items: Vec<Item>,
}

#[derive(Debug)]
struct Item {
    prefix: String,
    source: Base64Source,
}

#[derive(Clone, Debug)]
pub(crate) enum MediaText<'a> {
    Text(Base64Text<'a>),
    Marker(String),
}

impl Serialize for MediaText<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            MediaText::Text(t) => t.serialize(serializer),
            MediaText::Marker(m) => serializer.serialize_str(m),
        }
    }
}

// The seed is drawn once per process, from the system's randomness where std has it; the
// counter and an address make each request's tag differ even where it hasn't.
fn tag() -> String {
    static SEED: OnceLock<RandomState> = OnceLock::new();
    static COUNT: AtomicU64 = AtomicU64::new(0);
    let n = COUNT.fetch_add(1, Ordering::Relaxed);
    let at = std::ptr::addr_of!(n) as usize;
    let random = SEED.get_or_init(RandomState::new).hash_one((n, at));
    format!("kaeso-media-{random:016x}-")
}

impl Media {
    pub(crate) fn deferred() -> Media {
        #[cfg(test)]
        if tests::INLINE.with(std::cell::Cell::get) {
            return Media::inline();
        }
        Media {
            deferring: true,
            tag: None,
            items: Vec::new(),
        }
    }

    pub(crate) fn inline() -> Media {
        Media {
            deferring: false,
            tag: None,
            items: Vec::new(),
        }
    }

    fn defer(&mut self, prefix: String, source: &Base64Source) -> Option<String> {
        if !self.deferring {
            return None;
        }
        let tag = self.tag.get_or_insert_with(tag);
        let marker = format!("{tag}{}", self.items.len());
        self.items.push(Item {
            prefix,
            source: source.clone(),
        });
        Some(marker)
    }

    pub(crate) fn data_url(&mut self, source: &Base64Source, header: &str) -> String {
        let prefix = format!("{header},");
        self.defer(prefix, source)
            .unwrap_or_else(|| source.data_url_with(header))
    }

    pub(crate) fn base64_string(&mut self, source: &Base64Source) -> String {
        self.defer(String::new(), source)
            .unwrap_or_else(|| source.to_base64())
    }

    pub(crate) fn base64<'a>(&mut self, source: &'a Base64Source) -> MediaText<'a> {
        match self.defer(String::new(), source) {
            Some(marker) => MediaText::Marker(marker),
            None => MediaText::Text(source.base64()),
        }
    }

    pub(crate) fn finish(self, body: Vec<u8>) -> Result<Vec<u8>, KppError> {
        let Some(tag) = self.tag.filter(|_| !self.items.is_empty()) else {
            return Ok(body);
        };
        let tag = tag.as_bytes();
        let mut found = Vec::with_capacity(self.items.len());
        let mut seen = vec![false; self.items.len()];
        let mut at = 0;
        while let Some(offset) = find(body.get(at..).unwrap_or_default(), tag) {
            let start = at + offset;
            let digits = body
                .get(start + tag.len()..)
                .unwrap_or_default()
                .iter()
                .take_while(|b| b.is_ascii_digit())
                .count();
            let end = start + tag.len() + digits;
            let index = std::str::from_utf8(body.get(start + tag.len()..end).unwrap_or_default())
                .ok()
                .and_then(|d| d.parse::<usize>().ok());
            let quoted = start > 0
                && body.get(start - 1) == Some(&b'"')
                && body.get(end) == Some(&b'"')
                && body.get(start.wrapping_sub(2)) != Some(&b'\\');
            let first = index.and_then(|i| seen.get_mut(i)).filter(|s| !**s);
            match (index, first) {
                (Some(index), Some(seen)) if quoted => {
                    *seen = true;
                    found.push((start, end, index));
                }
                _ => return Err(broken()),
            }
            at = end;
        }
        if seen.contains(&false) {
            return Err(broken());
        }
        let mut written = Vec::with_capacity(found.len());
        let mut size = body.len();
        for (start, end, index) in found {
            let item = self.items.get(index).ok_or_else(broken)?;
            let prefix = escaped(&item.prefix)?;
            size = size - (end - start) + prefix.len() + Base64Text::new(&item.source.data).len();
            written.push((start, end, prefix, item));
        }
        let mut out = Vec::with_capacity(size);
        let mut copied = 0;
        for (start, end, prefix, item) in written {
            out.extend_from_slice(body.get(copied..start).unwrap_or_default());
            out.extend_from_slice(prefix.as_bytes());
            let from = out.len();
            out.resize(from + Base64Text::new(&item.source.data).len(), 0);
            let tail = out.get_mut(from..).ok_or_else(broken)?;
            WRITE
                .encode_slice(&item.source.data, tail)
                .map_err(|_| broken())?;
            copied = end;
        }
        out.extend_from_slice(body.get(copied..).unwrap_or_default());
        Ok(out)
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    let first = *needle.first()?;
    haystack
        .iter()
        .enumerate()
        .filter(|(_, b)| **b == first)
        .map(|(i, _)| i)
        .find(|&i| haystack.get(i..i + needle.len()) == Some(needle))
}

fn escaped(prefix: &str) -> Result<String, KppError> {
    let quoted = serde_json::to_string(prefix).map_err(|_| broken())?;
    Ok(quoted
        .strip_prefix('"')
        .and_then(|q| q.strip_suffix('"'))
        .unwrap_or_default()
        .to_owned())
}

fn broken() -> KppError {
    KppError::provider(
        "Kaeso could not write a request's media into its body. This is a bug in Kaeso; please report it.",
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use std::cell::Cell;

    use kaeso_kpp::{DialectId, RequestKind};

    use super::*;
    use crate::{EgressCtx, HeaderList, dispatch};

    thread_local! {
        pub(crate) static INLINE: Cell<bool> = const { Cell::new(false) };
    }

    fn source(len: usize) -> Base64Source {
        Base64Source::new(
            "image/png",
            (0..len).map(|i| (i * 31 % 251) as u8).collect::<Vec<_>>(),
        )
    }

    #[test]
    fn deferred_media_is_written_where_inline_media_would_be() {
        let (image, audio) = (source(70_000), source(5));
        let body = |media: &mut Media| {
            let url = media.data_url(&image, "data:image/png;name=\"a\";base64");
            let data = media.base64(&audio);
            let again = media.base64_string(&audio);
            serde_json::to_vec(&serde_json::json!([
                {"url": url, "data": serde_json::to_value(&data).unwrap()},
                {"text": "a \"quoted\" string", "data": again},
            ]))
            .unwrap()
        };
        let mut inline = Media::inline();
        let expected = body(&mut inline);
        assert_eq!(inline.finish(expected.clone()).unwrap(), expected);
        let mut deferred = Media::deferred();
        let small = body(&mut deferred);
        assert!(small.len() < 300, "{}", small.len());
        let out = deferred.finish(small).unwrap();
        assert_eq!(out, expected);
        assert_eq!(out.capacity(), out.len());
    }

    #[test]
    fn a_request_without_media_draws_no_tag() {
        let media = Media::deferred();
        assert!(media.tag.is_none());
        let body = br#"{"text":"kaeso-media-0000000000000000-0"}"#.to_vec();
        assert_eq!(media.finish(body.clone()).unwrap(), body);
    }

    #[test]
    fn a_marker_out_of_place_twice_or_missing_is_an_error_not_a_broken_body() {
        let marker = |media: &mut Media| media.base64_string(&source(3));
        let mut media = Media::deferred();
        let m = marker(&mut media);
        let nested =
            serde_json::to_vec(&serde_json::json!({ "args": format!("\"{m}\"") })).unwrap();
        assert!(media.finish(nested).is_err());
        let mut media = Media::deferred();
        let m = marker(&mut media);
        assert!(media.finish(format!("\"{m}7\"").into_bytes()).is_err());
        let mut media = Media::deferred();
        let m = marker(&mut media);
        let twice = serde_json::to_vec(&serde_json::json!([m, m])).unwrap();
        assert!(
            media.finish(twice).is_err(),
            "a duplicated index is refused"
        );
        let mut media = Media::deferred();
        let _ = marker(&mut media);
        assert!(
            media.finish(b"[]".to_vec()).is_err(),
            "a lost file is refused"
        );
    }

    #[test]
    fn every_request_draws_its_own_tag() {
        let mut a = Media::deferred();
        let mut b = Media::deferred();
        assert_ne!(a.base64_string(&source(1)), b.base64_string(&source(1)));
    }

    fn image_request(from: DialectId, text: &str) -> Vec<u8> {
        let b64 = source(90_000).to_base64();
        let body = match from {
            DialectId::AnthropicMessages => serde_json::json!({
                "model": "claude-sonnet-4-5", "max_tokens": 64,
                "messages": [{"role": "user", "content": [
                    {"type": "text", "text": text},
                    {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": b64}},
                    {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": b64}},
                ]}, {"role": "assistant", "content": [
                    {"type": "tool_use", "id": "toolu_1", "name": "look", "input": {"note": text}},
                ]}, {"role": "user", "content": [
                    {"type": "tool_result", "tool_use_id": "toolu_1", "content": [
                        {"type": "text", "text": serde_json::json!({"seen": text}).to_string()},
                    ]},
                ]}],
            }),
            _ => serde_json::json!({
                "model": "gpt-4o",
                "messages": [{"role": "user", "content": [
                    {"type": "text", "text": text},
                    {"type": "image_url", "image_url": {"url": format!("data:image/png;base64,{b64}")}},
                    {"type": "image_url", "image_url": {"url": format!("data:image/png;base64,{b64}")}},
                ]}, {"role": "assistant", "tool_calls": [
                    {"id": "call_1", "type": "function", "function": {"name": "look", "arguments": serde_json::json!({"note": text}).to_string()}},
                ]}, {"role": "tool", "tool_call_id": "call_1", "content": serde_json::json!({"seen": text}).to_string()}],
            }),
        };
        serde_json::to_vec(&body).unwrap()
    }

    fn encoded(from: DialectId, to: DialectId, body: &[u8], inline: bool) -> Vec<u8> {
        let req = dispatch::decode_request(from, RequestKind::Generate, body, &HeaderList::new())
            .unwrap();
        INLINE.with(|i| i.set(inline));
        let out = dispatch::encode_request(to, &req, &mut EgressCtx::new(&req));
        INLINE.with(|i| i.set(false));
        out.unwrap().body
    }

    #[test]
    fn deferred_bodies_equal_inline_bodies_byte_for_byte_and_keep_marker_like_text() {
        let lookalike = "kaeso-media-0123456789abcdef-0";
        for (from, to) in [
            (DialectId::OpenAiChat, DialectId::OpenAiChat),
            (DialectId::OpenAiChat, DialectId::OpenAiResponses),
            (DialectId::AnthropicMessages, DialectId::AnthropicMessages),
            (DialectId::AnthropicMessages, DialectId::OpenAiResponses),
        ] {
            let body = image_request(from, lookalike);
            let deferred = encoded(from, to, &body, false);
            let inline = encoded(from, to, &body, true);
            assert!(deferred == inline, "{from:?} to {to:?}");
            let text = String::from_utf8(deferred).unwrap();
            assert_eq!(
                text.matches(lookalike).count(),
                3,
                "{from:?} to {to:?}: {text:.300}"
            );
        }
    }
}
