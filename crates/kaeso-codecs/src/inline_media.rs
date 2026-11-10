use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;

use base64::Engine as _;
use kaeso_kpp::{Base64Source, Base64Text, KppError};
use serde::{Serialize, Serializer};

const WRITE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

// The request encoders build each part and message as JSON text and the body from those, so
// base64 written where a part is built would be copied once per layer. A deferring `Media`
// hands out a short marker in its place and `finish` writes the base64 into the body once, at
// its final size. The marker carries 64 random bits, so no text of a request can spell one.
#[derive(Debug)]
pub(crate) struct Media {
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

impl Media {
    pub(crate) fn deferred() -> Media {
        let random = RandomState::new().hash_one(0u8);
        Media {
            tag: Some(format!("kaeso-media-{random:016x}-")),
            items: Vec::new(),
        }
    }

    pub(crate) fn inline() -> Media {
        Media {
            tag: None,
            items: Vec::new(),
        }
    }

    fn defer(&mut self, prefix: String, source: &Base64Source) -> Option<String> {
        let tag = self.tag.as_ref()?;
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
        let prefixes = self
            .items
            .iter()
            .map(|i| escaped(&i.prefix))
            .collect::<Result<Vec<_>, _>>()?;
        let mut found = Vec::new();
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
            match index.filter(|i| *i < self.items.len()) {
                Some(index) if quoted => found.push((start, end, index)),
                _ => return Err(broken()),
            }
            at = end;
        }
        let mut size = body.len();
        for (start, end, index) in &found {
            let (prefix, item) = (prefixes.get(*index), self.items.get(*index));
            let (Some(prefix), Some(item)) = (prefix, item) else {
                return Err(broken());
            };
            size = size - (end - start) + prefix.len() + Base64Text::new(&item.source.data).len();
        }
        let mut out = Vec::with_capacity(size);
        let mut copied = 0;
        for (start, end, index) in found {
            let (Some(prefix), Some(item)) = (prefixes.get(index), self.items.get(index)) else {
                return Err(broken());
            };
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
mod tests {
    use super::*;

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
    fn a_marker_out_of_place_is_an_error_not_a_broken_body() {
        let mut media = Media::deferred();
        let marker = media.base64_string(&source(3));
        let nested =
            serde_json::to_vec(&serde_json::json!({ "args": format!("\"{marker}\"") })).unwrap();
        assert!(media.finish(nested).is_err());
        let mut media = Media::deferred();
        let marker = media.base64_string(&source(3));
        let unknown = format!("\"{marker}7\"").into_bytes();
        assert!(media.finish(unknown).is_err());
    }
}
