use std::borrow::Cow;
use std::collections::HashMap;

use base64::Engine as _;
use base64::alphabet;
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
use bytes::Bytes;
use kaeso_kpp::Source;

const LENIENT: GeneralPurposeConfig = GeneralPurposeConfig::new()
    .with_decode_padding_mode(DecodePaddingMode::Indifferent)
    .with_decode_allow_trailing_bits(true);
const STANDARD: GeneralPurpose = GeneralPurpose::new(&alphabet::STANDARD, LENIENT);
const URL_SAFE: GeneralPurpose = GeneralPurpose::new(&alphabet::URL_SAFE, LENIENT);
const STANDARD_PADDED: GeneralPurpose = base64::engine::general_purpose::STANDARD;

pub const NOT_KEPT: &str = "not_kept";

const UNKNOWN_TYPE: &str = "application/octet-stream";

// A JPEG states its size after its metadata, which Exif limits to one 64 KiB segment; the
// count is a multiple of 4 so the cut never splits a base64 group.
const HEAD_CHARS: usize = 87_384;

const MEDIA_MIN_CHARS: usize = 16;
// Base64 this long beside a media type is a file whatever its key is called; shorter text
// there must sit under a key that names content or start like a known file.
const TYPED_ANY_KEY_CHARS: usize = 64;
const MEDIA_TYPE_MAX_CHARS: usize = 100;
const DATA_URL_HEADER_MAX: usize = 4096;
const KEY_MAX_CHARS: usize = 256;
// JSON inside a JSON string, such as a tool call's arguments, is read this many levels deep.
const NESTED_MAX: usize = 4;

/// What a request's log keeps of its media: nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Keep;

impl Keep {
    pub fn nothing() -> Self {
        Keep
    }

    // The reason a file of this size is left out; `None` would keep it.
    fn left_out(&mut self, _bytes: u64) -> Option<&'static str> {
        Some(NOT_KEPT)
    }
}

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
    let media_type = stated(media_type)
        .or(sniff(data).map(|s| s.0))
        .unwrap_or(UNKNOWN_TYPE);
    placeholder(media_type, data.len() as u64, dimensions(data), reason)
}

fn of_base64(media_type: Option<&str>, text: &str, reason: &str) -> String {
    let head = head(text);
    let media_type = stated(media_type)
        .or(sniff(&head).map(|s| s.0))
        .unwrap_or(UNKNOWN_TYPE);
    placeholder(media_type, decoded_len(text), dimensions(&head), reason)
}

fn base64_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'-' | b'_')
}

fn is_base64(text: &str) -> bool {
    let body = text.trim_end_matches('=');
    body.len() % 4 != 1 && text.len() - body.len() <= 2 && body.bytes().all(base64_byte)
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

fn starts_like_text(data: &[u8], marks: &[&str]) -> bool {
    let text = data.strip_prefix(b"\xef\xbb\xbf").unwrap_or(data);
    let text = text.trim_ascii_start();
    marks.iter().any(|m| {
        text.get(..m.len())
            .is_some_and(|start| start.eq_ignore_ascii_case(m.as_bytes()))
    })
}

// The media type a file's first bytes show, and whether they are distinctive enough to call
// text of unknown purpose a file: a signature of two bytes, such as an MP3 frame's or a
// bitmap's, also starts one in a few thousand random strings.
fn sniff(data: &[u8]) -> Option<(&'static str, bool)> {
    let brand = || at::<4>(data, 8);
    let strong = |media_type| Some((media_type, true));
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        strong("image/png")
    } else if data.starts_with(b"\xff\xd8\xff") {
        strong("image/jpeg")
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        strong("image/gif")
    } else if data.starts_with(b"RIFF") {
        match &brand()? {
            b"WEBP" => strong("image/webp"),
            b"WAVE" => strong("audio/wav"),
            b"AVI " => strong("video/x-msvideo"),
            _ => None,
        }
    } else if data.starts_with(b"II*\0") || data.starts_with(b"MM\0*") {
        strong("image/tiff")
    } else if data.starts_with(b"%PDF-") {
        strong("application/pdf")
    } else if data.starts_with(b"PK\x03\x04") {
        strong("application/zip")
    } else if data.starts_with(b"ID3") {
        strong("audio/mpeg")
    } else if data.starts_with(b"OggS") {
        strong("audio/ogg")
    } else if data.starts_with(b"fLaC") {
        strong("audio/flac")
    } else if data.starts_with(b"\x1a\x45\xdf\xa3") {
        strong("video/webm")
    } else if at::<4>(data, 4).as_ref() == Some(b"ftyp") {
        strong(match &brand()? {
            b"M4A " => "audio/mp4",
            b"avif" => "image/avif",
            b"heic" | b"heix" | b"mif1" => "image/heic",
            b"qt  " => "video/quicktime",
            _ => "video/mp4",
        })
    } else if starts_like_text(data, &["<svg", "<?xml"]) {
        strong("image/svg+xml")
    } else if starts_like_text(data, &["<!doctype html", "<html"]) {
        strong("text/html")
    } else if matches!(data, [0xff, 0xfb | 0xfa | 0xf3 | 0xf2 | 0xe3, ..]) {
        Some(("audio/mpeg", false))
    } else if matches!(data, [0xff, 0xf1 | 0xf9, ..]) {
        Some(("audio/aac", false))
    } else if data.starts_with(b"BM") {
        Some(("image/bmp", false))
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
    match sniff(data)?.0 {
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

struct DataUrl<'a> {
    media_type: &'a str,
    payload: &'a str,
    base64: bool,
}

// The codecs' reading of a `data:` URL (`kaeso_codecs::wire::parse_data_url`): the scheme and
// `base64` in any case, any parameters before it. Here leading space and a URL that is
// percent-encoded instead of base64 count too, since both still carry a file.
fn data_url(text: &str) -> Option<DataUrl<'_>> {
    let text = text.trim_start();
    if !text.get(..5)?.eq_ignore_ascii_case("data:") {
        return None;
    }
    let (header, payload) = text.split_once(',')?;
    if header.len() > DATA_URL_HEADER_MAX {
        return None;
    }
    let params = header.get(5..)?;
    let media_type = params.split(';').next().unwrap_or_default();
    let base64 = params
        .rsplit(';')
        .next()
        .is_some_and(|last| last.eq_ignore_ascii_case("base64"));
    let holds_file = if base64 {
        payload.len() >= 4 && is_base64(payload)
    } else {
        !media_type.is_empty() && payload.len() >= MEDIA_MIN_CHARS
    };
    holds_file.then_some(DataUrl {
        media_type,
        payload,
        base64,
    })
}

struct Span {
    start: usize,
    end: usize,
    media_type: (usize, usize),
    payload: usize,
}

// `data:…;base64,…` inside longer text, such as Markdown or a tool's answer: from the scheme
// to the end of the base64 that follows it.
fn data_url_spans(text: &str) -> Vec<Span> {
    let bytes = text.as_bytes();
    let mut spans = Vec::new();
    let mut from = 0;
    for (colon, _) in text.match_indices(':') {
        let Some(start) = colon.checked_sub(4).filter(|s| *s >= from) else {
            continue;
        };
        if !bytes
            .get(start..colon)
            .is_some_and(|s| s.eq_ignore_ascii_case(b"data"))
        {
            continue;
        }
        let header_end = bytes
            .iter()
            .enumerate()
            .skip(colon + 1)
            .take(DATA_URL_HEADER_MAX)
            .find(|(_, b)| {
                matches!(b, b',' | b'"' | b'\'' | b'<' | b'>') || b.is_ascii_whitespace()
            })
            .filter(|(_, b)| **b == b',')
            .map(|(i, _)| i);
        let Some(comma) = header_end else { continue };
        let Some(params) = text.get(colon + 1..comma) else {
            continue;
        };
        if !params
            .rsplit(';')
            .next()
            .is_some_and(|last| last.eq_ignore_ascii_case("base64"))
        {
            continue;
        }
        let payload = comma + 1;
        let run = bytes
            .iter()
            .skip(payload)
            .take_while(|b| base64_byte(**b) || **b == b'=')
            .count();
        if run < MEDIA_MIN_CHARS {
            continue;
        }
        let media_type = params.split(';').next().unwrap_or_default().len();
        spans.push(Span {
            start,
            end: payload + run,
            media_type: (colon + 1, colon + 1 + media_type),
            payload,
        });
        from = payload + run;
    }
    spans
}

fn swap_spans(text: &str, keep: &mut Keep) -> Option<String> {
    let mut out = String::new();
    let mut copied = 0;
    for span in data_url_spans(text) {
        let (Some(payload), Some(media_type)) = (
            text.get(span.payload..span.end),
            text.get(span.media_type.0..span.media_type.1),
        ) else {
            continue;
        };
        let bytes = decoded_len(payload);
        let Some(reason) = keep.left_out(bytes) else {
            continue;
        };
        let media_type = stated(Some(media_type))
            .or(sniff(&head(payload)).map(|s| s.0))
            .unwrap_or(UNKNOWN_TYPE);
        out.push_str(text.get(copied..span.start).unwrap_or_default());
        out.push_str(&format!(
            "[kaeso_media: {media_type}, {bytes} bytes, {reason}]"
        ));
        copied = span.end;
    }
    (copied > 0).then(|| {
        out.push_str(text.get(copied..).unwrap_or_default());
        out
    })
}

fn hex4(digits: Option<&str>) -> Option<u32> {
    u32::from_str_radix(digits.filter(|d| d.len() == 4)?, 16).ok()
}

// A JSON string's text, read as far as it can be: an escape JSON doesn't know stays as written
// and half of a surrogate pair becomes U+FFFD, so a string a strict parser refuses is still read.
fn unescaped(raw: &str) -> Cow<'_, str> {
    if !raw.contains('\\') {
        return Cow::Borrowed(raw);
    }
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(at) = rest.find('\\') {
        out.push_str(rest.get(..at).unwrap_or_default());
        let after = rest.get(at + 1..).unwrap_or_default();
        let mut chars = after.chars();
        let (c, used) = match chars.next() {
            Some('n') => ('\n', 1),
            Some('t') => ('\t', 1),
            Some('r') => ('\r', 1),
            Some('b') => ('\u{8}', 1),
            Some('f') => ('\u{c}', 1),
            Some('u') => match hex4(after.get(1..5)) {
                Some(high @ 0xd800..=0xdbff) => {
                    let low = after
                        .get(5..7)
                        .filter(|s| *s == "\\u")
                        .and_then(|_| hex4(after.get(7..11)))
                        .filter(|l| (0xdc00..=0xdfff).contains(l));
                    match low {
                        Some(low) => (
                            char::from_u32(0x10000 + ((high - 0xd800) << 10) + (low - 0xdc00))
                                .unwrap_or('\u{fffd}'),
                            11,
                        ),
                        None => ('\u{fffd}', 5),
                    }
                }
                Some(code) => (char::from_u32(code).unwrap_or('\u{fffd}'), 5),
                None => ('u', 1),
            },
            Some(other) => (other, other.len_utf8()),
            None => ('\\', 0),
        };
        out.push(c);
        rest = after.get(used..).unwrap_or_default();
    }
    out.push_str(rest);
    Cow::Owned(out)
}

const TYPE_KEYS: [&str; 6] = [
    "media_type",
    "mime_type",
    "mimeType",
    "mediaType",
    "content_type",
    "contentType",
];
// Keys that hold a file's bytes in the formats Kaeso reads and in MCP: base64 under one of
// them is a file even without a type beside it.
const FILE_KEYS: [&str; 4] = ["b64_json", "file_data", "blob", "bytes"];
const CONTENT_KEYS: [&str; 5] = ["data", "result", "image", "audio", "video"];
// Base64 that is never a file, however long: what providers sign or encrypt.
const OPAQUE_KEYS: [&str; 5] = [
    "signature",
    "encrypted_content",
    "thoughtSignature",
    "thought_signature",
    "id",
];

#[derive(Clone, Copy)]
struct Token {
    start: usize,
    inner_end: usize,
    end: usize,
}

struct Candidate<'a> {
    token: Token,
    key: Option<Cow<'a, str>>,
    len: usize,
    starts_like_a_file: bool,
    certain: bool,
}

struct Frame<'a> {
    object: bool,
    under: Option<Cow<'a, str>>,
    key: Option<Cow<'a, str>>,
    declared: Option<Cow<'a, str>>,
    format: Option<Cow<'a, str>>,
    holds_text: bool,
    candidates: Vec<Candidate<'a>>,
}

impl<'a> Frame<'a> {
    fn new(object: bool, under: Option<Cow<'a, str>>) -> Self {
        Frame {
            object,
            holds_text: under.as_deref() == Some("text"),
            under,
            key: None,
            declared: None,
            format: None,
            candidates: Vec::new(),
        }
    }

    fn current_key(&self) -> Option<Cow<'a, str>> {
        if self.object {
            self.key.clone()
        } else {
            self.under.clone()
        }
    }
}

enum Kind {
    DataUrl,
    Base64(Option<String>),
    Inner,
}

struct Finding {
    token: Token,
    kind: Kind,
}

// Media beside the type it is in is decided once the whole object has been read, since the
// type may follow the bytes. A text source holds text, not base64.
fn close<'a>(frame: Frame<'a>, parent: Option<&mut Frame<'a>>, found: &mut Vec<Finding>) {
    if !frame.object
        && let Some(parent) = parent
    {
        parent.candidates.extend(frame.candidates);
        return;
    }
    if frame.holds_text {
        return;
    }
    for c in frame.candidates {
        let key = c.key.as_deref().unwrap_or_default();
        let named = CONTENT_KEYS.contains(&key);
        let declared = frame.declared.as_deref();
        let typed =
            declared.is_some() && (c.len >= TYPED_ANY_KEY_CHARS || named || c.starts_like_a_file);
        let media_type = if c.certain || typed {
            Some(declared)
        } else if key == "data" {
            frame.format.as_deref().and_then(audio_format).map(Some)
        } else {
            None
        };
        if let Some(media_type) = media_type {
            found.push(Finding {
                token: c.token,
                kind: Kind::Base64(media_type.map(str::to_owned)),
            });
        }
    }
}

fn contains_base64_mark(raw: &str) -> bool {
    raw.match_indices(';').any(|(at, _)| {
        raw.get(at + 1..at + 8)
            .is_some_and(|next| next.eq_ignore_ascii_case("base64,"))
    })
}

fn value<'a>(raw: &'a str, token: Token, stack: &mut [Frame<'a>], found: &mut Vec<Finding>) {
    let key = stack.last().and_then(Frame::current_key);
    if raw.len() <= MEDIA_TYPE_MAX_CHARS
        && let (Some(frame), Some(key)) = (stack.last_mut().filter(|f| f.object), key.as_deref())
    {
        let text = || unescaped(raw);
        if TYPE_KEYS.contains(&key) {
            frame.declared = Some(text());
        } else if key == "format" {
            frame.format = Some(text());
        } else if key == "type" && raw == "text" {
            frame.holds_text = true;
        }
    }
    if raw.len() < MEDIA_MIN_CHARS {
        return;
    }
    let start = raw.trim_start();
    let could_be_base64 = raw
        .bytes()
        .all(|b| base64_byte(b) || matches!(b, b'=' | b'\\'));
    let opens_with_scheme = raw
        .as_bytes()
        .iter()
        .take(64)
        .position(|b| *b == b':')
        .and_then(|colon| raw.get(colon.checked_sub(4)?..colon))
        .is_some_and(|s| s.eq_ignore_ascii_case("data"));
    if opens_with_scheme && data_url(&unescaped(raw)).is_some() {
        found.push(Finding {
            token,
            kind: Kind::DataUrl,
        });
    } else if could_be_base64 {
        let text = unescaped(raw);
        if !is_base64(&text) {
            return;
        }
        let key_name = key.as_deref().unwrap_or_default();
        let sniffed = sniff(&head(text.get(..64).unwrap_or(&text)));
        let certain = sniffed.is_some_and(|s| s.1) || FILE_KEYS.contains(&key_name);
        match stack.last_mut() {
            Some(frame) if certain || !OPAQUE_KEYS.contains(&key_name) => {
                frame.candidates.push(Candidate {
                    token,
                    key,
                    len: text.len(),
                    starts_like_a_file: sniffed.is_some(),
                    certain,
                });
            }
            None if certain => found.push(Finding {
                token,
                kind: Kind::Base64(None),
            }),
            _ => {}
        }
    } else if start.starts_with(['{', '[']) || contains_base64_mark(raw) {
        found.push(Finding {
            token,
            kind: Kind::Inner,
        });
    }
}

// Reads JSON's strings without parsing it, so a body a parser refuses (cut off, nested too
// deep, with a lone surrogate) is searched like any other.
fn find(text: &str) -> Vec<Finding> {
    let bytes = text.as_bytes();
    let mut stack: Vec<Frame<'_>> = Vec::new();
    let mut found = Vec::new();
    let mut i = 0;
    while let Some(&b) = bytes.get(i) {
        match b {
            b'{' | b'[' => {
                let under = stack.last().and_then(Frame::current_key);
                stack.push(Frame::new(b == b'{', under));
                i += 1;
            }
            b'}' | b']' => {
                if let Some(frame) = stack.pop() {
                    close(frame, stack.last_mut(), &mut found);
                }
                i += 1;
            }
            b'"' => {
                let mut end = i + 1;
                let closed = loop {
                    match bytes.get(end) {
                        Some(b'\\') => end += 2,
                        Some(b'"') => break true,
                        Some(_) => end += 1,
                        None => break false,
                    }
                };
                let inner_end = end.min(bytes.len());
                let token = Token {
                    start: i,
                    inner_end,
                    end: if closed { inner_end + 1 } else { inner_end },
                };
                let raw = text.get(i + 1..inner_end).unwrap_or_default();
                let next = bytes
                    .iter()
                    .skip(token.end)
                    .find(|b| !b.is_ascii_whitespace());
                match stack.last_mut().filter(|f| f.object) {
                    Some(frame) if next == Some(&b':') => {
                        frame.key = (raw.len() <= KEY_MAX_CHARS).then(|| unescaped(raw));
                    }
                    _ => value(raw, token, &mut stack, &mut found),
                }
                i = token.end;
            }
            _ => i += 1,
        }
    }
    while let Some(frame) = stack.pop() {
        close(frame, stack.last_mut(), &mut found);
    }
    found.sort_by_key(|f| f.token.start);
    found
}

fn nested(text: &str, keep: &mut Keep, depth: usize) -> Option<String> {
    let json = text.trim_start().starts_with(['{', '[']) && depth < NESTED_MAX;
    json.then(|| strip(text, keep, depth + 1))
        .flatten()
        .or_else(|| swap_spans(text, keep))
}

fn strip(text: &str, keep: &mut Keep, depth: usize) -> Option<String> {
    let mut out = String::new();
    let mut copied = 0;
    for Finding { token, kind } in find(text) {
        let Some(raw) = text.get(token.start + 1..token.inner_end) else {
            continue;
        };
        let content = unescaped(raw);
        let swapped = match kind {
            Kind::DataUrl => data_url(&content).and_then(|url| {
                if url.base64 {
                    let reason = keep.left_out(decoded_len(url.payload))?;
                    Some(of_base64(Some(url.media_type), url.payload, reason))
                } else {
                    let bytes = url.payload.len() as u64;
                    let reason = keep.left_out(bytes)?;
                    let media_type = stated(Some(url.media_type)).unwrap_or(UNKNOWN_TYPE);
                    Some(placeholder(media_type, bytes, None, reason))
                }
            }),
            Kind::Base64(media_type) => keep
                .left_out(decoded_len(&content))
                .map(|reason| of_base64(media_type.as_deref(), &content, reason)),
            Kind::Inner => {
                nested(&content, keep, depth).and_then(|inner| serde_json::to_string(&inner).ok())
            }
        };
        if let Some(new) = swapped {
            out.push_str(text.get(copied..token.start).unwrap_or_default());
            out.push_str(&new);
            copied = token.end;
        }
    }
    (copied > 0).then(|| {
        out.push_str(text.get(copied..).unwrap_or_default());
        out
    })
}

/// JSON as the log keeps it: each image, audio clip, video and file that isn't kept replaced
/// by its media type and size, with everything else byte for byte as it was. `None` when
/// nothing is left out.
///
/// Media is found by its shape, never by a format's field names alone, and without parsing:
/// a `data:` URL, alone in a string or inside longer text; base64 next to the media type or
/// audio format it is in, or under a key that holds a file's bytes; base64 that starts like a
/// known file; and any of these inside JSON that is itself a string, such as a tool call's
/// arguments.
pub fn without_media(text: &str, keep: &mut Keep) -> Option<String> {
    strip(text, keep, 0)
}

fn whole_file(media_type: Option<&'static str>, bytes: &[u8], keep: &mut Keep) -> String {
    match keep.left_out(bytes.len() as u64) {
        Some(reason) => of_bytes(media_type, bytes, reason),
        None => serde_json::Value::String(format!(
            "data:{};base64,{}",
            media_type
                .or(sniff(bytes).map(|s| s.0))
                .unwrap_or(UNKNOWN_TYPE),
            STANDARD_PADDED.encode(bytes)
        ))
        .to_string(),
    }
}

/// A request body as the log keeps it. A body that isn't JSON, such as an upload that could
/// not be read, is one file, logged as its type and size alone, never as its text.
pub fn body(bytes: &[u8], keep: &mut Keep) -> String {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return whole_file(None, bytes, keep);
    };
    let start = text.trim_start();
    if start.is_empty() || start.starts_with(['{', '[', '"']) {
        without_media(text, keep).unwrap_or_else(|| text.to_owned())
    } else if start.starts_with("--") {
        whole_file(Some("multipart/form-data"), bytes, keep)
    } else {
        let sniffed = sniff(bytes).map(|s| s.0);
        whole_file(sniffed.or(Some("text/plain")), bytes, keep)
    }
}

/// The media of a decoded request or reply, taken out before it is written as JSON so the
/// bytes are never encoded only to be dropped.
#[derive(Debug, Default)]
pub struct LeftOut {
    marks: HashMap<String, String>,
}

impl LeftOut {
    /// Takes `source`'s bytes out, leaving its type and size.
    pub fn fit(&mut self, source: &mut Source, keep: &mut Keep) {
        let Source::Base64(media) = source else {
            return;
        };
        let Some(reason) = keep.left_out(media.data.len() as u64) else {
            return;
        };
        let mark = Bytes::from(format!("kaeso media left out {}", self.marks.len()));
        self.marks.insert(
            STANDARD_PADDED.encode(&mark),
            of_bytes(Some(&media.media_type), &media.data, reason),
        );
        *source = Source::bytes(media.media_type.clone(), mark);
    }

    pub fn write<T: serde::Serialize>(&self, value: &T) -> Result<String, serde_json::Error> {
        let text = serde_json::to_string(value)?;
        if self.marks.is_empty() {
            return Ok(text);
        }
        let mut out = String::with_capacity(text.len());
        let mut copied = 0;
        for (at, _) in text.match_indices('"') {
            if at < copied {
                continue;
            }
            let Some(end) = text.get(at + 1..).and_then(|rest| rest.find('"')) else {
                break;
            };
            let end = at + 1 + end;
            if let Some(new) = text.get(at + 1..end).and_then(|s| self.marks.get(s)) {
                out.push_str(text.get(copied..at).unwrap_or_default());
                out.push_str(new);
                copied = end + 1;
            }
        }
        out.push_str(text.get(copied..).unwrap_or_default());
        Ok(out)
    }
}

#[cfg(test)]
mod tests;
