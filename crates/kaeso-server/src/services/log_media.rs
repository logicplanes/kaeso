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
pub const TOO_LARGE: &str = "too_large";

// Decoded bytes, so a request's kept media takes at most a third more as base64. Chosen to
// hold a few photos or a minute of speech while one request stays a small part of the log
// buffer.
pub const KEPT_PER_REQUEST: u64 = 10 << 20;

const UNKNOWN_TYPE: &str = "application/octet-stream";

// A JPEG states its size after its metadata, which Exif limits to one 64 KiB segment; the
// count is a multiple of 4 so the cut never splits a base64 group.
const HEAD_CHARS: usize = 87_384;

const MEDIA_MIN_CHARS: usize = 16;
// Under a key that names content, text this long beside a media type is a file; under any
// other key it must be this long. Shorter text must start like a known file, so a status
// word, an image tag or a digest beside a type stays.
const NAMED_MIN_CHARS: usize = 128;
const UNNAMED_MIN_CHARS: usize = 1024;
// A string this long that is nearly all base64 alphabet is a file even when it isn't valid
// base64: wrapped, cut off or with a stray character.
const BLOB_MIN_CHARS: usize = 4096;
const MEDIA_TYPE_MAX_CHARS: usize = 100;
const DATA_URL_HEADER_MAX: usize = 4096;
const KEY_MAX_CHARS: usize = 256;
// JSON inside a JSON string, such as a tool call's arguments, is read this many levels deep.
const NESTED_MAX: usize = 4;
// The codecs refuse deeper nesting (`kaeso_codecs::limits`); the search keeps a frame per
// level, so a body nested deeper is logged as one file instead of being searched.
const DEPTH_MAX: usize = 128;
// Inside text that is no longer read as JSON, a run of base64 alphabet this long is a file.
const RUN_MIN_CHARS: usize = 256;

/// What a request's log keeps of its media: nothing, or files in the order they appear until
/// the request's allowance is used up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Keep {
    left: Option<u64>,
}

impl Keep {
    pub fn nothing() -> Self {
        Keep { left: None }
    }

    pub fn media() -> Self {
        Keep {
            left: Some(KEPT_PER_REQUEST),
        }
    }

    fn left_out(&mut self, bytes: u64) -> Option<&'static str> {
        let Some(left) = self.left else {
            return Some(NOT_KEPT);
        };
        match left.checked_sub(bytes) {
            Some(rest) => {
                self.left = Some(rest);
                None
            }
            None => Some(TOO_LARGE),
        }
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

// A media type as RFC 6838 writes it, without parameters: a placeholder repeats only that,
// never text a request put where a type belongs.
fn stated(media_type: Option<&str>) -> Option<&str> {
    let name = media_type?.split(';').next()?.trim();
    let (kind, subtype) = name.split_once('/')?;
    let part = |p: &str| {
        (1..=MEDIA_TYPE_MAX_CHARS / 2).contains(&p.len())
            && p.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'+' | b'-' | b'_'))
    };
    (part(kind) && part(subtype)).then_some(name)
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

fn wraps(b: u8) -> bool {
    matches!(b, b' ' | b'\n' | b'\r' | b'\t')
}

const IN_BASE64: u8 = 0;
const WRAPS_BASE64: u8 = 1;
const NOT_BASE64: u8 = 2;

const fn base64_class(b: u8) -> u8 {
    if matches!(b, b' ' | b'\n' | b'\r' | b'\t') {
        WRAPS_BASE64
    } else if b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'-' | b'_' | b'=') {
        IN_BASE64
    } else {
        NOT_BASE64
    }
}

// A table read a block at a time takes a fifth of the time of testing each byte in turn; a
// large body's media is found sooner, and the request lets the body go sooner.
const BASE64_CLASSES: [u8; 256] = {
    let mut table = [NOT_BASE64; 256];
    let mut rest: &mut [u8] = &mut table;
    let mut b = 0u8;
    while let Some((slot, tail)) = rest.split_first_mut() {
        *slot = base64_class(b);
        rest = tail;
        b = b.wrapping_add(1);
    }
    table
};

// Base64 as apps really send it: also wrapped into lines, as `base64(1)`, Ruby's `encode64`
// and Python's `encodebytes` write it, or with a stray space. Returns it without the
// whitespace. Prose made of letters and spaces is not base64: it has far more spaces.
fn base64_of(text: &str) -> Option<Cow<'_, str>> {
    let mut spaces = 0usize;
    for block in text.as_bytes().chunks(4096) {
        let mut seen = IN_BASE64;
        for &b in block {
            let class = BASE64_CLASSES
                .get(usize::from(b))
                .copied()
                .unwrap_or(NOT_BASE64);
            seen |= class;
            spaces += usize::from(class & WRAPS_BASE64);
        }
        if seen & NOT_BASE64 != 0 {
            return None;
        }
    }
    if spaces.saturating_mul(20) > text.len() {
        return None;
    }
    let compact = if spaces == 0 {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(text.chars().filter(|c| !c.is_ascii_whitespace()).collect())
    };
    let body = compact.trim_end_matches('=');
    let whole = !body.contains('=')
        && compact.len() - body.len() <= 2
        && (body.len() % 4 != 1 || body.len() >= NAMED_MIN_CHARS);
    (whole && compact.len() >= MEDIA_MIN_CHARS).then_some(compact)
}

fn blob_like(text: &str) -> bool {
    if text.len() < BLOB_MIN_CHARS {
        return false;
    }
    let (mut spaces, mut other) = (0usize, 0usize);
    for b in text.bytes() {
        if wraps(b) {
            spaces += 1;
        } else if !base64_byte(b) && b != b'=' {
            other += 1;
        }
    }
    spaces.saturating_mul(20) <= text.len() && other.saturating_mul(33) <= text.len()
}

fn all_hex(text: &str) -> bool {
    text.bytes().all(|b| b.is_ascii_hexdigit())
}

fn decoded_len(text: &str) -> u64 {
    (text.trim_end_matches('=').len() as u64).saturating_mul(3) / 4
}

fn head(text: &str) -> Vec<u8> {
    let whole = text.trim_end_matches('=');
    let cut = whole
        .get(..whole.len().min(HEAD_CHARS) & !3)
        .unwrap_or(whole);
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

fn audio_format(format: &str) -> Cow<'static, str> {
    Cow::Borrowed(match format {
        "wav" => "audio/wav",
        "mp3" => "audio/mpeg",
        "flac" => "audio/flac",
        "opus" | "ogg" => "audio/ogg",
        "aac" => "audio/aac",
        "m4a" => "audio/mp4",
        "webm" => "audio/webm",
        "pcm16" | "pcm" => "audio/pcm",
        other if other.len() <= 16 && other.bytes().all(|b| b.is_ascii_alphanumeric()) => {
            return Cow::Owned(format!("audio/{other}"));
        }
        _ => "audio/basic",
    })
}

struct DataUrl<'a> {
    media_type: &'a str,
    payload: Cow<'a, str>,
    base64: bool,
}

// The codecs' reading of a `data:` URL (`kaeso_codecs::wire::parse_data_url`): the scheme and
// `base64` in any case, any parameters before it. Here leading space, wrapped or cut-off
// base64 and a URL that is percent-encoded instead of base64 count too, since each still
// carries a file.
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
    let payload = if base64 {
        match base64_of(payload) {
            Some(compact) => compact,
            None if payload.len() >= 4 && is_short_base64(payload) => Cow::Borrowed(payload),
            None if blob_like(payload) => Cow::Borrowed(payload),
            None => return None,
        }
    } else if !media_type.is_empty() && payload.len() >= MEDIA_MIN_CHARS {
        Cow::Borrowed(payload)
    } else {
        return None;
    };
    Some(DataUrl {
        media_type,
        payload,
        base64,
    })
}

fn is_short_base64(text: &str) -> bool {
    let body = text.trim_end_matches('=');
    body.len() % 4 != 1 && text.len() - body.len() <= 2 && body.bytes().all(base64_byte)
}

struct Span {
    start: usize,
    end: usize,
    media_type: (usize, usize),
    payload: usize,
    base64: bool,
}

// Where base64 that starts at `from` ends: at the first byte outside its alphabet, or, for
// base64 wrapped into lines or broken by a stray space, after its last piece. A short piece
// counts only as the last line of wrapped base64, so the words after a file stay.
fn base64_run(bytes: &[u8], from: usize) -> usize {
    let piece = |at: usize| {
        bytes
            .iter()
            .skip(at)
            .take_while(|b| base64_byte(**b) || **b == b'=')
            .count()
    };
    let mut last = piece(from);
    let mut end = from + last;
    loop {
        let between = bytes
            .get(end..(end + 2).min(bytes.len()))
            .unwrap_or_default();
        let gap = between.iter().take_while(|b| wraps(**b)).count();
        let wrapped = between.iter().take(gap).all(|b| matches!(b, b'\n' | b'\r'));
        let next = piece(end + gap);
        let closes = bytes
            .get(end + gap + next)
            .is_none_or(|b| !b.is_ascii_whitespace() || matches!(b, b'\n' | b'\r'));
        if gap == 0 || next == 0 || !(next >= 60 || (wrapped && last >= 60 && closes)) {
            return end;
        }
        end += gap + next;
        last = next;
    }
}

// `data:` URLs inside longer text, such as Markdown or a tool's answer: from the scheme to the
// end of the file that follows it.
fn data_url_spans(text: &str) -> Vec<Span> {
    let bytes = text.as_bytes();
    let mut spans = Vec::new();
    let mut from = 0;
    // Bytes before this hold no comma that ends a header, so a flood of schemes is read once.
    let mut searched = 0;
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
        let limit = (colon + 1 + DATA_URL_HEADER_MAX).min(bytes.len());
        let mut at = searched.max(colon + 1);
        let mut comma = None;
        let mut quoted = false;
        while at < limit {
            match bytes.get(at) {
                Some(b'"') => {
                    quoted = !quoted;
                    at += 1;
                }
                Some(b',') if !quoted => {
                    comma = Some(at);
                    break;
                }
                Some(b) if !quoted && (b.is_ascii_whitespace() || matches!(b, b'<' | b'>')) => {
                    break;
                }
                _ => at += 1,
            }
        }
        searched = at;
        let Some(comma) = comma else { continue };
        let Some(params) = text.get(colon + 1..comma) else {
            continue;
        };
        let media_type = params.split(';').next().unwrap_or_default();
        let base64 = params
            .rsplit(';')
            .next()
            .is_some_and(|last| last.eq_ignore_ascii_case("base64"));
        let payload = comma + 1;
        let end = if base64 {
            base64_run(bytes, payload)
        } else if media_type.contains('/') {
            payload
                + bytes
                    .iter()
                    .skip(payload)
                    .take_while(|b| {
                        !b.is_ascii_whitespace() && !matches!(b, b'"' | b'\'' | b')' | b'<' | b'>')
                    })
                    .count()
        } else {
            payload
        };
        if end - payload < MEDIA_MIN_CHARS {
            continue;
        }
        spans.push(Span {
            start,
            end,
            media_type: (colon + 1, colon + 1 + media_type.len()),
            payload,
            base64,
        });
        from = end;
        searched = end;
    }
    spans
}

fn note(media_type: &str, bytes: u64, reason: &str) -> String {
    format!("[kaeso_media: {media_type}, {bytes} bytes, {reason}]")
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
        let bytes = if span.base64 {
            decoded_len(payload)
        } else {
            payload.len() as u64
        };
        let Some(reason) = keep.left_out(bytes) else {
            continue;
        };
        let media_type = stated(Some(media_type))
            .or(sniff(&head(payload)).map(|s| s.0))
            .unwrap_or(UNKNOWN_TYPE);
        out.push_str(text.get(copied..span.start).unwrap_or_default());
        out.push_str(&note(media_type, bytes, reason));
        copied = span.end;
    }
    (copied > 0).then(|| {
        out.push_str(text.get(copied..).unwrap_or_default());
        out
    })
}

// Text that is no longer read as JSON, because it sits too many strings deep: every long run
// of base64 alphabet in it is taken for a file.
fn swap_runs(text: &str, keep: &mut Keep) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = String::new();
    let mut copied = 0;
    let mut i = 0;
    while i < bytes.len() {
        let run = bytes
            .iter()
            .skip(i)
            .take_while(|b| base64_byte(**b) || **b == b'=')
            .count();
        if run >= RUN_MIN_CHARS
            && let Some(payload) = text.get(i..i + run)
            && let Some(reason) = keep.left_out(decoded_len(payload))
        {
            let media_type = sniff(&head(payload)).map_or(UNKNOWN_TYPE, |s| s.0);
            out.push_str(text.get(copied..i).unwrap_or_default());
            out.push_str(&note(media_type, decoded_len(payload), reason));
            copied = i + run;
        }
        i += run.max(1);
    }
    (copied > 0).then(|| {
        out.push_str(text.get(copied..).unwrap_or_default());
        out
    })
}

fn hex4(digits: Option<&str>) -> Option<u32> {
    let digits = digits.filter(|d| d.len() == 4 && all_hex(d))?;
    u32::from_str_radix(digits, 16).ok()
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
const CONTENT_KEYS: [&str; 6] = ["data", "result", "image", "audio", "video", "input_audio"];
// Objects that hold one file: long base64 in or under one of these keys is that file even
// when nothing states its type.
const MEDIA_KEYS: [&str; 7] = [
    "audio",
    "input_audio",
    "image",
    "video",
    "inline_data",
    "inlineData",
    "source",
];
// Text that is never a file, however long: what providers sign or encrypt, and what names or
// locates something.
const TEXT_KEYS: [&str; 12] = [
    "signature",
    "encrypted_content",
    "thoughtSignature",
    "thought_signature",
    "id",
    "text",
    "path",
    "name",
    "uri",
    "url",
    "title",
    "filename",
];
const OPAQUE_KINDS: [&str; 4] = ["thinking", "reasoning", "signature", "encrypted"];

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
    kind: Option<Cow<'a, str>>,
    format: Option<Cow<'a, str>>,
    holds_text: bool,
    opaque: bool,
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
            kind: None,
            format: None,
            opaque: false,
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
// type may follow the bytes. A text source holds text and a reasoning block what its provider
// signed, so in those only what starts like a file is one.
fn close(frame: Frame<'_>, found: &mut Vec<Finding>) {
    let declared = frame.declared.as_deref().or(frame.kind.as_deref());
    let in_media = frame
        .under
        .as_deref()
        .is_some_and(|under| MEDIA_KEYS.contains(&under));
    for c in frame.candidates {
        let key = c.key.as_deref().unwrap_or_default();
        let media_type = if c.certain {
            Some(declared.map(str::to_owned))
        } else if frame.holds_text || frame.opaque {
            None
        } else {
            let named = CONTENT_KEYS.contains(&key);
            let enough = if named {
                NAMED_MIN_CHARS
            } else {
                UNNAMED_MIN_CHARS
            };
            if declared.is_some() && (c.len >= enough || c.starts_like_a_file) {
                Some(declared.map(str::to_owned))
            } else if key == "data"
                && c.len >= NAMED_MIN_CHARS
                && let Some(format) = &frame.format
            {
                Some(Some(audio_format(format).into_owned()))
            } else if c.len >= UNNAMED_MIN_CHARS
                && (MEDIA_KEYS.contains(&key) || (key == "data" && in_media))
            {
                Some(None)
            } else {
                None
            }
        };
        if let Some(media_type) = media_type {
            found.push(Finding {
                token: c.token,
                kind: Kind::Base64(media_type),
            });
        }
    }
}

fn contains_mark(text: &str, mark: &str) -> bool {
    let Some(last) = mark.as_bytes().last().copied() else {
        return false;
    };
    text.as_bytes()
        .iter()
        .enumerate()
        .filter(|(_, b)| **b == last)
        .any(|(at, _)| {
            (at + 1)
                .checked_sub(mark.len())
                .and_then(|from| text.get(from..=at))
                .is_some_and(|found| found.eq_ignore_ascii_case(mark))
        })
}

fn note_type<'a>(frame: &mut Frame<'a>, key: &str, text: Cow<'a, str>) {
    if TYPE_KEYS.contains(&key) {
        frame.declared = Some(text);
    } else if key == "format" {
        frame.format = Some(text);
    } else if key == "type" || key == "kind" {
        if OPAQUE_KINDS.iter().any(|kind| text.contains(kind)) {
            frame.opaque = true;
        }
        if key == "type" && text == "text" {
            frame.holds_text = true;
        } else if key == "type" && text.contains('/') && !text.contains(' ') {
            frame.kind = Some(text);
        }
    }
}

fn value<'a>(raw: &'a str, token: Token, stack: &mut [Frame<'a>], found: &mut Vec<Finding>) {
    let key = stack.last().and_then(Frame::current_key);
    let key_name = key.as_deref().unwrap_or_default();
    let named_type =
        TYPE_KEYS.contains(&key_name) || matches!(key_name, "format" | "type" | "kind");
    if named_type && let Some(frame) = stack.last_mut().filter(|f| f.object) {
        // A type too long to be one still says that the object holds a file.
        let text = if raw.len() <= DATA_URL_HEADER_MAX {
            unescaped(raw)
        } else {
            Cow::Borrowed("")
        };
        note_type(frame, key_name, text);
    }
    if raw.len() < MEDIA_MIN_CHARS {
        return;
    }
    let text = unescaped(raw);
    let finding = |kind| Finding { token, kind };
    if data_url(&text).is_some() {
        found.push(finding(Kind::DataUrl));
    } else if let Some(base64) = base64_of(&text) {
        let sniffed = sniff(&head(base64.get(..64).unwrap_or(&base64)));
        let plain = all_hex(&base64) || (base64.starts_with('/') && sniffed.is_none());
        let certain = sniffed.is_some_and(|s| s.1)
            || (FILE_KEYS.contains(&key_name) && base64.len() >= 32 && !plain);
        let object = stack.iter_mut().rev().find(|f| f.object);
        match object {
            Some(frame) if certain || !(plain || named_type || TEXT_KEYS.contains(&key_name)) => {
                frame.candidates.push(Candidate {
                    token,
                    len: base64.len(),
                    starts_like_a_file: sniffed.is_some(),
                    certain,
                    key,
                });
            }
            None if certain => found.push(finding(Kind::Base64(None))),
            _ => {}
        }
    } else if text.trim_start().starts_with(['{', '['])
        || contains_mark(&text, "base64,")
        || contains_mark(&text, "data:")
    {
        found.push(finding(Kind::Inner));
    } else if blob_like(&text) {
        found.push(finding(Kind::Base64(None)));
    }
}

struct TooDeep;

// Reads JSON's strings without parsing it, so a body a parser refuses (cut off, with a lone
// surrogate or a trailing comma) is searched like any other.
fn find(text: &str) -> Result<Vec<Finding>, TooDeep> {
    let bytes = text.as_bytes();
    let mut stack: Vec<Frame<'_>> = Vec::new();
    let mut found = Vec::new();
    let mut i = 0;
    while let Some(&b) = bytes.get(i) {
        match b {
            b'{' | b'[' => {
                if stack.len() >= DEPTH_MAX {
                    return Err(TooDeep);
                }
                let under = stack.last().and_then(Frame::current_key);
                stack.push(Frame::new(b == b'{', under));
                i += 1;
            }
            b'}' | b']' => {
                if let Some(frame) = stack.pop() {
                    close(frame, &mut found);
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
        close(frame, &mut found);
    }
    found.sort_by_key(|f| f.token.start);
    Ok(found)
}

fn nested(text: &str, keep: &mut Keep, depth: usize) -> Option<String> {
    let json = text.trim_start().starts_with(['{', '[']);
    if json && depth < NESTED_MAX {
        return strip(text, keep, depth + 1).or_else(|| swap_spans(text, keep));
    }
    let spans = swap_spans(text, keep);
    if !json {
        return spans;
    }
    swap_runs(spans.as_deref().unwrap_or(text), keep).or(spans)
}

fn strip(text: &str, keep: &mut Keep, depth: usize) -> Option<String> {
    let Ok(found) = find(text) else {
        let bytes = text.len() as u64;
        return keep
            .left_out(bytes)
            .map(|reason| placeholder("application/json", bytes, None, reason));
    };
    let mut out = String::new();
    let mut copied = 0;
    for Finding { token, kind } in found {
        let Some(raw) = text.get(token.start + 1..token.inner_end) else {
            continue;
        };
        let content = unescaped(raw);
        let swapped = match kind {
            Kind::DataUrl => data_url(&content).and_then(|url| {
                if url.base64 {
                    let reason = keep.left_out(decoded_len(&url.payload))?;
                    Some(of_base64(Some(url.media_type), &url.payload, reason))
                } else {
                    let bytes = url.payload.len() as u64;
                    let reason = keep.left_out(bytes)?;
                    let media_type = stated(Some(url.media_type)).unwrap_or(UNKNOWN_TYPE);
                    Some(placeholder(media_type, bytes, None, reason))
                }
            }),
            Kind::Base64(media_type) => {
                let base64 = base64_of(&content).unwrap_or(Cow::Borrowed(&content));
                keep.left_out(decoded_len(&base64))
                    .map(|reason| of_base64(media_type.as_deref(), &base64, reason))
            }
            Kind::Inner => match nested(&content, keep, depth) {
                Some(inner) => serde_json::to_string(&inner).ok(),
                None if blob_like(&content) => keep
                    .left_out(decoded_len(&content))
                    .map(|reason| of_base64(None, &content, reason)),
                None => None,
            },
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
/// not be read, is one file: kept as a `data:` URL, or logged as its type and size alone,
/// never as its text.
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

/// [`body`] of a body the log may take: JSON kept whole becomes the logged text without a copy
/// when nothing else holds its bytes.
pub fn owned_body(bytes: Bytes, keep: &mut Keep) -> String {
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return body(&bytes, keep);
    };
    let start = text.trim_start();
    if !(start.is_empty() || start.starts_with(['{', '[', '"'])) {
        return body(&bytes, keep);
    }
    if let Some(stripped) = without_media(text, keep) {
        return stripped;
    }
    String::from_utf8(Vec::from(bytes))
        .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

/// The media of a decoded request or reply, taken out before it is written as JSON so the
/// bytes are never encoded only to be dropped.
#[derive(Debug)]
pub struct LeftOut {
    // Each file taken out leaves a mark where it stood. The marks carry a random part, so
    // nothing an app sends can be one.
    nonce: uuid::Uuid,
    marks: HashMap<String, String>,
}

impl Default for LeftOut {
    fn default() -> Self {
        LeftOut {
            nonce: uuid::Uuid::now_v7(),
            marks: HashMap::new(),
        }
    }
}

impl LeftOut {
    /// Leaves `source` as it is when the request's allowance still holds it, and takes it
    /// out otherwise.
    pub fn fit(&mut self, source: &mut Source, keep: &mut Keep) {
        let Source::Base64(media) = source else {
            return;
        };
        if let Some(reason) = keep.left_out(media.data.len() as u64) {
            let bytes = media.data.len() as u64;
            self.take(source, bytes, reason);
        }
    }

    /// Takes `source` out as a file of `bytes` in all, of which `source` holds at least the
    /// start: its type and, for an image, its dimensions are read from that.
    pub fn take(&mut self, source: &mut Source, bytes: u64, reason: &str) {
        let Source::Base64(media) = source else {
            return;
        };
        let media_type = stated(Some(&media.media_type))
            .or(sniff(&media.data).map(|s| s.0))
            .unwrap_or(UNKNOWN_TYPE);
        let mark = Bytes::from(format!("kaeso media {} {}", self.nonce, self.marks.len()));
        self.marks.insert(
            STANDARD_PADDED.encode(&mark),
            placeholder(media_type, bytes, dimensions(&media.data), reason),
        );
        *source = Source::bytes(media.media_type.clone(), mark);
    }

    pub fn write<T: serde::Serialize>(&self, value: &T) -> Result<String, serde_json::Error> {
        let text = serde_json::to_string(value)?;
        if self.marks.is_empty() {
            return Ok(text);
        }
        let bytes = text.as_bytes();
        let mut out = String::with_capacity(text.len());
        let mut copied = 0;
        let mut i = 0;
        while let Some(&b) = bytes.get(i) {
            if b != b'"' {
                i += 1;
                continue;
            }
            let mut end = i + 1;
            while let Some(&b) = bytes.get(end) {
                match b {
                    b'\\' => end += 2,
                    b'"' => break,
                    _ => end += 1,
                }
            }
            if let Some(new) = text.get(i + 1..end).and_then(|s| self.marks.get(s)) {
                out.push_str(text.get(copied..i).unwrap_or_default());
                out.push_str(new);
                copied = end + 1;
            }
            i = end + 1;
        }
        out.push_str(text.get(copied..).unwrap_or_default());
        Ok(out)
    }
}

#[cfg(test)]
mod tests;
