use std::borrow::Cow;
use std::collections::HashMap;

use base64::Engine as _;
use base64::alphabet;
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
use bytes::Bytes;
use kaeso_kpp::Source;
use serde_json::Value;

const LENIENT: GeneralPurposeConfig = GeneralPurposeConfig::new()
    .with_decode_padding_mode(DecodePaddingMode::Indifferent)
    .with_decode_allow_trailing_bits(true);
const STANDARD: GeneralPurpose = GeneralPurpose::new(&alphabet::STANDARD, LENIENT);
const URL_SAFE: GeneralPurpose = GeneralPurpose::new(&alphabet::URL_SAFE, LENIENT);

pub const NOT_KEPT: &str = "not_kept";

const UNKNOWN_TYPE: &str = "application/octet-stream";

// A JPEG states its size after its metadata, which Exif limits to one 64 KiB segment; the
// count is a multiple of 4 so the cut never splits a base64 group.
const HEAD_CHARS: usize = 87_384;

const TYPED_MIN_CHARS: usize = 16;
const SNIFFED_MIN_CHARS: usize = 256;
const MEDIA_TYPE_MAX_CHARS: usize = 100;

fn placeholder(media_type: &str, bytes: u64, size: Option<(u32, u32)>, reason: &str) -> String {
    let mut media = serde_json::Map::new();
    media.insert("media_type".into(), media_type.into());
    media.insert("bytes".into(), bytes.into());
    if let Some((width, height)) = size {
        media.insert("width".into(), width.into());
        media.insert("height".into(), height.into());
    }
    media.insert("reason".into(), reason.into());
    serde_json::json!({ "kaeso_media": media }).to_string()
}

fn stated(media_type: Option<&str>) -> Option<&str> {
    media_type.filter(|m| !m.is_empty() && m.len() <= MEDIA_TYPE_MAX_CHARS)
}

fn of_bytes(media_type: Option<&str>, data: &[u8], reason: &str) -> String {
    let media_type = stated(media_type).or(sniff(data)).unwrap_or(UNKNOWN_TYPE);
    placeholder(media_type, data.len() as u64, dimensions(data), reason)
}

fn of_base64(media_type: Option<&str>, text: &str, reason: &str) -> String {
    let head = head(text);
    let media_type = stated(media_type).or(sniff(&head)).unwrap_or(UNKNOWN_TYPE);
    placeholder(media_type, decoded_len(text), dimensions(&head), reason)
}

fn is_base64(text: &str) -> bool {
    let body = text.trim_end_matches('=');
    body.len() % 4 != 1
        && text.len() - body.len() <= 2
        && body
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'-' | b'_'))
}

fn decoded_len(text: &str) -> u64 {
    (text.trim_end_matches('=').len() as u64).saturating_mul(3) / 4
}

fn head(text: &str) -> Vec<u8> {
    let cut = text.get(..HEAD_CHARS).unwrap_or(text);
    let engine = if cut.contains(['-', '_']) {
        URL_SAFE
    } else {
        STANDARD
    };
    engine.decode(cut).unwrap_or_default()
}

fn at<const N: usize>(data: &[u8], from: usize) -> Option<[u8; N]> {
    data.get(from..from.checked_add(N)?)?.try_into().ok()
}

fn sniff(data: &[u8]) -> Option<&'static str> {
    let brand = || at::<4>(data, 8);
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if data.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if data.starts_with(b"RIFF") {
        match &brand()? {
            b"WEBP" => Some("image/webp"),
            b"WAVE" => Some("audio/wav"),
            _ => None,
        }
    } else if data.starts_with(b"%PDF-") {
        Some("application/pdf")
    } else if data.starts_with(b"ID3") || matches!(data, [0xff, 0xfb | 0xf3 | 0xf2, ..]) {
        Some("audio/mpeg")
    } else if data.starts_with(b"OggS") {
        Some("audio/ogg")
    } else if data.starts_with(b"fLaC") {
        Some("audio/flac")
    } else if data.starts_with(b"\x1a\x45\xdf\xa3") {
        Some("video/webm")
    } else if at::<4>(data, 4).as_ref() == Some(b"ftyp") {
        Some(match &brand()? {
            b"M4A " => "audio/mp4",
            b"avif" => "image/avif",
            b"heic" | b"heix" | b"mif1" => "image/heic",
            b"qt  " => "video/quicktime",
            _ => "video/mp4",
        })
    } else {
        None
    }
}

fn be16(data: &[u8], from: usize) -> Option<u32> {
    at::<2>(data, from).map(|b| u32::from(u16::from_be_bytes(b)))
}

fn le16(data: &[u8], from: usize) -> Option<u32> {
    at::<2>(data, from).map(|b| u32::from(u16::from_le_bytes(b)))
}

fn le24(data: &[u8], from: usize) -> Option<u32> {
    at::<3>(data, from).map(|[a, b, c]| u32::from_le_bytes([a, b, c, 0]))
}

// JPEG (ITU-T T.81, B.1.1.3): segments follow each other until a start-of-frame marker, which
// holds the height and then the width.
fn jpeg_size(data: &[u8]) -> Option<(u32, u32)> {
    let mut i = 2;
    loop {
        if *data.get(i)? != 0xff {
            return None;
        }
        let marker = *data.get(i + 1)?;
        match marker {
            0xff => i += 1,
            0xd8 | 0x01 | 0xd0..=0xd7 => i += 2,
            0xc0..=0xcf if !matches!(marker, 0xc4 | 0xc8 | 0xcc) => {
                return Some((be16(data, i + 7)?, be16(data, i + 5)?));
            }
            _ => i = i.checked_add(2 + be16(data, i + 2)? as usize)?,
        }
    }
}

// WebP container specification: the first chunk is VP8X (extended), VP8 (lossy) or VP8L
// (lossless), each with its own place for the canvas size.
fn webp_size(data: &[u8]) -> Option<(u32, u32)> {
    match &at::<4>(data, 12)? {
        b"VP8X" => Some((le24(data, 24)? + 1, le24(data, 27)? + 1)),
        b"VP8 " => Some((le16(data, 26)? & 0x3fff, le16(data, 28)? & 0x3fff)),
        b"VP8L" => {
            let bits = u32::from_le_bytes(at::<4>(data, 21)?);
            Some(((bits & 0x3fff) + 1, ((bits >> 14) & 0x3fff) + 1))
        }
        _ => None,
    }
}

fn dimensions(data: &[u8]) -> Option<(u32, u32)> {
    match sniff(data)? {
        "image/png" => Some((
            u32::from_be_bytes(at::<4>(data, 16)?),
            u32::from_be_bytes(at::<4>(data, 20)?),
        )),
        "image/gif" => Some((le16(data, 6)?, le16(data, 8)?)),
        "image/jpeg" => jpeg_size(data),
        "image/webp" => webp_size(data),
        _ => None,
    }
}

fn audio_format(format: &str) -> Option<&'static str> {
    Some(match format {
        "wav" => "audio/wav",
        "mp3" => "audio/mpeg",
        "flac" => "audio/flac",
        "opus" | "ogg" => "audio/ogg",
        "aac" => "audio/aac",
        "m4a" => "audio/mp4",
        "webm" => "audio/webm",
        "pcm16" | "pcm" => "audio/pcm",
        _ => return None,
    })
}

fn data_url(text: &str) -> Option<(&str, &str)> {
    let rest = text.strip_prefix("data:")?;
    let (header, payload) = rest.split_once(',')?;
    let (media_type, encoding) = header.rsplit_once(';')?;
    (header.len() <= 2 * MEDIA_TYPE_MAX_CHARS && encoding.eq_ignore_ascii_case("base64"))
        .then(|| (media_type.split(';').next().unwrap_or(media_type), payload))
}

#[derive(Clone, Copy)]
enum Media<'a> {
    DataUrl,
    Typed(&'a str),
    Sniffed,
}

const TYPE_KEYS: [&str; 3] = ["media_type", "mime_type", "mimeType"];

// Media is found by its shape, never by a format's field names alone: a `data:` URL, base64
// next to the media type or audio format it is in, a `file_data` upload, or base64 that
// starts like a known image, audio, video or PDF file. A text source holds text, not base64.
fn find<'a>(value: &'a Value, under: Option<&str>, found: &mut HashMap<&'a str, Media<'a>>) {
    match value {
        Value::String(s) => {
            if data_url(s).is_some() {
                found.insert(s, Media::DataUrl);
            } else if s.len() >= SNIFFED_MIN_CHARS && is_base64(s) && sniff(&head(s)).is_some() {
                found.insert(s, Media::Sniffed);
            }
        }
        Value::Array(items) => {
            for item in items {
                find(item, under, found);
            }
        }
        Value::Object(map) => {
            let text = |k: &str| map.get(k).and_then(Value::as_str);
            let declared = TYPE_KEYS
                .iter()
                .find_map(|k| text(k))
                .or_else(|| text("format").and_then(audio_format));
            let holds_text = under == Some("text") || text("type") == Some("text");
            for (key, child) in map {
                let encoded = child
                    .as_str()
                    .filter(|s| s.len() >= TYPED_MIN_CHARS && is_base64(s));
                match (key.as_str(), encoded, declared) {
                    ("data", Some(s), Some(media_type)) if !holds_text => {
                        found.insert(s, Media::Typed(media_type));
                    }
                    ("file_data", Some(s), _) => {
                        found.insert(s, Media::Sniffed);
                    }
                    _ => find(child, Some(key), found),
                }
            }
        }
        _ => {}
    }
}

fn swap_strings(
    text: &str,
    shortest: usize,
    mut swap: impl FnMut(&str) -> Option<String>,
) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = String::new();
    let mut copied = 0;
    let mut i = 0;
    while let Some(&b) = bytes.get(i) {
        if b != b'"' {
            i += 1;
            continue;
        }
        let mut end = i + 1;
        let mut escaped = false;
        loop {
            match bytes.get(end)? {
                b'\\' => {
                    escaped = true;
                    end += 2;
                }
                b'"' => break,
                _ => end += 1,
            }
        }
        let raw = text.get(i + 1..end)?;
        if raw.len() >= shortest {
            let content = if escaped {
                serde_json::from_str::<String>(text.get(i..=end)?)
                    .ok()
                    .map(Cow::Owned)
            } else {
                Some(Cow::Borrowed(raw))
            };
            if let Some(new) = content.and_then(|c| swap(&c)) {
                out.push_str(text.get(copied..i)?);
                out.push_str(&new);
                copied = end + 1;
            }
        }
        i = end + 1;
    }
    (copied > 0).then(|| {
        out.push_str(text.get(copied..).unwrap_or_default());
        out
    })
}

fn may_hold_media(text: &str) -> bool {
    if ["base64", "\"data\"", "file_data"]
        .iter()
        .any(|mark| text.contains(mark))
    {
        return true;
    }
    let mut run = 0;
    text.bytes().any(|b| {
        run = if b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'-' | b'_') {
            run + 1
        } else {
            0
        };
        run >= SNIFFED_MIN_CHARS
    })
}

/// JSON as the log keeps it: each image, audio clip, video and file in it replaced by its
/// media type and size, with everything else byte for byte as it was. `None` when it holds
/// no media or isn't JSON.
pub fn without_media(text: &str) -> Option<String> {
    if !may_hold_media(text) {
        return None;
    }
    let value: Value = serde_json::from_str(text).ok()?;
    let mut found = HashMap::new();
    find(&value, None, &mut found);
    let shortest = found.keys().map(|s| s.len()).min()?;
    swap_strings(text, shortest, |content| {
        Some(match found.get(content)? {
            Media::DataUrl => {
                let (media_type, payload) = data_url(content)?;
                of_base64(Some(media_type), payload, NOT_KEPT)
            }
            Media::Typed(media_type) => of_base64(Some(media_type), content, NOT_KEPT),
            Media::Sniffed => of_base64(None, content, NOT_KEPT),
        })
    })
}

/// A request body as the log keeps it. A body that isn't text, such as an upload that could
/// not be read, is logged as its type and size alone.
pub fn body(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => without_media(text).unwrap_or_else(|| text.to_owned()),
        Err(_) => of_bytes(None, bytes, NOT_KEPT),
    }
}

/// The media of a decoded request or reply, taken out before it is written as JSON so the
/// bytes are never encoded only to be dropped.
#[derive(Debug, Default)]
pub struct LeftOut {
    marks: HashMap<String, String>,
}

impl LeftOut {
    pub fn take(&mut self, source: &mut Source) {
        let Source::Base64(media) = source else {
            return;
        };
        let mark = Bytes::from(format!("kaeso media left out {}", self.marks.len()));
        self.marks.insert(
            STANDARD_PADDED.encode(&mark),
            of_bytes(Some(&media.media_type), &media.data, NOT_KEPT),
        );
        *source = Source::bytes(media.media_type.clone(), mark);
    }

    pub fn write<T: serde::Serialize>(&self, value: &T) -> Result<String, serde_json::Error> {
        let text = serde_json::to_string(value)?;
        let shortest = self.marks.keys().map(String::len).min().unwrap_or(0);
        if self.marks.is_empty() {
            return Ok(text);
        }
        Ok(
            swap_strings(&text, shortest, |content| self.marks.get(content).cloned())
                .unwrap_or(text),
        )
    }
}

const STANDARD_PADDED: GeneralPurpose = base64::engine::general_purpose::STANDARD;

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: [u8; 33] = [
        0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, b'I', b'H', b'D', b'R', 0, 0,
        3, 0x20, 0, 0, 2, 0x58, 8, 6, 0, 0, 0, 0, 0, 0, 0,
    ];

    fn b64(data: &[u8]) -> String {
        STANDARD_PADDED.encode(data)
    }

    fn stripped(text: &str) -> Value {
        serde_json::from_str(&without_media(text).unwrap()).unwrap()
    }

    fn png_left_out() -> Value {
        serde_json::json!({ "kaeso_media": {
            "media_type": "image/png", "bytes": 33, "width": 800, "height": 600, "reason": "not_kept"
        }})
    }

    #[test]
    fn every_way_an_app_sends_media_inline_is_left_out_with_its_type_and_size() {
        let png = b64(&PNG);
        let wav = b64(b"RIFF\0\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0");
        let pdf = b64(b"%PDF-1.7 a short document");
        let mp3 = b64(b"ID3\x04\0\0\0\0\0\0 a short sound");
        let chat = format!(
            r#"{{"messages":[{{"role":"user","content":[{{"type":"text","text":"What is this?"}},{{"type":"image_url","image_url":{{"url":"data:image/png;base64,{png}"}}}},{{"type":"input_audio","input_audio":{{"data":"{wav}","format":"wav"}}}},{{"type":"file","file":{{"filename":"a.pdf","file_data":"{pdf}"}}}}]}}]}}"#
        );
        let out = stripped(&chat);
        let parts = &out["messages"][0]["content"];
        assert_eq!(parts[0]["text"], "What is this?");
        assert_eq!(parts[1]["image_url"]["url"], png_left_out());
        assert_eq!(
            parts[2]["input_audio"]["data"]["kaeso_media"]["media_type"],
            "audio/wav"
        );
        assert_eq!(
            parts[3]["file"]["file_data"]["kaeso_media"],
            serde_json::json!({ "media_type": "application/pdf", "bytes": 25, "reason": "not_kept" })
        );
        assert_eq!(parts[3]["file"]["filename"], "a.pdf");

        let anthropic = format!(
            r#"{{"messages":[{{"role":"user","content":[{{"type":"image","source":{{"type":"base64","media_type":"image/png","data":"{png}"}}}}]}}]}}"#
        );
        assert_eq!(
            stripped(&anthropic)["messages"][0]["content"][0]["source"]["data"],
            png_left_out()
        );

        let gemini = format!(
            r#"{{"contents":[{{"parts":[{{"inlineData":{{"mimeType":"application/pdf","data":"{pdf}"}}}},{{"inline_data":{{"mime_type":"audio/mpeg","data":"{mp3}"}}}}]}}]}}"#
        );
        let parts = &stripped(&gemini)["contents"][0]["parts"];
        assert_eq!(
            parts[0]["inlineData"]["data"]["kaeso_media"]["media_type"],
            "application/pdf"
        );
        assert_eq!(parts[1]["inline_data"]["data"]["kaeso_media"]["bytes"], 24);

        let reply = format!(
            r#"{{"id":"r","blocks":[{{"image":{{"source":{{"base64":{{"media_type":"image/png","data":"{png}"}}}}}}}}]}}"#
        );
        assert_eq!(
            stripped(&reply)["blocks"][0]["image"]["source"]["base64"]["data"],
            png_left_out()
        );
    }

    #[test]
    fn media_without_a_stated_type_is_known_by_how_its_file_starts() {
        let mut jpeg = vec![
            0xff, 0xd8, 0xff, 0xe0, 0, 4, 0, 0, 0xff, 0xc0, 0, 17, 8, 1, 0x40, 2, 0x80,
        ];
        jpeg.resize(400, 0);
        let text = format!(r#"{{"blob":"{}"}}"#, b64(&jpeg));
        assert_eq!(
            stripped(&text)["blob"],
            serde_json::json!({ "kaeso_media": {
                "media_type": "image/jpeg", "bytes": 400, "width": 640, "height": 320, "reason": "not_kept"
            }})
        );
        for (start, media_type) in [
            (&b"GIF89a\x02\x00\x03\x00"[..], "image/gif"),
            (
                b"RIFF\0\0\0\0WEBPVP8X\0\0\0\0\0\0\0\0\x0f\0\0\x07\0\0",
                "image/webp",
            ),
            (b"RIFF\0\0\0\0WAVEfmt ", "audio/wav"),
            (b"%PDF-1.7", "application/pdf"),
            (b"ID3\x04", "audio/mpeg"),
            (b"OggS", "audio/ogg"),
            (b"fLaC", "audio/flac"),
            (b"\0\0\0\x18ftypmp42", "video/mp4"),
            (b"\0\0\0\x18ftypM4A ", "audio/mp4"),
            (b"\x1a\x45\xdf\xa3", "video/webm"),
        ] {
            assert_eq!(sniff(start), Some(media_type));
        }
        assert_eq!(dimensions(b"GIF89a\x02\x00\x03\x00"), Some((2, 3)));
        assert_eq!(
            dimensions(b"RIFF\0\0\0\0WEBPVP8X\0\0\0\0\0\0\0\0\x0f\0\0\x07\0\0"),
            Some((16, 8))
        );
    }

    #[test]
    fn text_that_only_looks_like_media_stays() {
        let signature = "c2lnbmF0dXJl".repeat(40);
        let kept = [
            r#"{"messages":[{"role":"user","content":"data: the plan, base64 or not"}]}"#.to_owned(),
            format!(r#"{{"content":[{{"type":"thinking","thinking":"hm","signature":"{signature}"}}]}}"#),
            r#"{"source":{"type":"text","media_type":"text/plain","data":"SGVsbG8gd29ybGQgYWdhaW4="}}"#.to_owned(),
            r#"{"blocks":[{"document":{"source":{"text":{"media_type":"text/plain","data":"SGVsbG8gd29ybGQgYWdhaW4="}}}}]}"#.to_owned(),
            r#"{"image_url":{"url":"https://example.com/base64/cat.png"}}"#.to_owned(),
            "not json, base64".to_owned(),
        ];
        for text in kept {
            assert_eq!(without_media(&text), None, "{text}");
        }
    }

    #[test]
    fn everything_but_the_media_stays_byte_for_byte() {
        let text = format!(
            r#"{{"z":1.50,"a":"café \"x\"","big":12345678901234567890123,"url":"data:image\/png;base64,{}"}}"#,
            b64(&PNG).replace('/', "\\/")
        );
        let out = without_media(&text).unwrap();
        assert!(
            out.starts_with(
                r#"{"z":1.50,"a":"café \"x\"","big":12345678901234567890123,"url":{"kaeso_media""#
            ),
            "{out}"
        );
    }

    #[test]
    fn a_body_that_is_not_text_is_logged_as_its_type_and_size() {
        let mut upload = PNG.to_vec();
        upload.extend([0xff, 0xfe]);
        let logged: Value = serde_json::from_str(&body(&upload)).unwrap();
        assert_eq!(logged["kaeso_media"]["media_type"], "image/png");
        assert_eq!(logged["kaeso_media"]["bytes"], 35);
        assert_eq!(body(b"plain words"), "plain words");
    }

    #[test]
    fn decoded_media_is_left_out_before_it_is_written() {
        let mut image = Source::bytes("image/png", PNG.to_vec());
        let mut link = Source::Url("https://example.com/a.png".into());
        let mut left_out = LeftOut::default();
        left_out.take(&mut image);
        left_out.take(&mut link);
        let written: Value =
            serde_json::from_str(&left_out.write(&vec![image, link]).unwrap()).unwrap();
        assert_eq!(written[0]["base64"]["data"], png_left_out());
        assert_eq!(written[1]["url"], "https://example.com/a.png");
        assert_eq!(LeftOut::default().write(&"plain").unwrap(), "\"plain\"");
    }
}
