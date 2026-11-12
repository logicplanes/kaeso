//! `multipart/form-data` bodies (RFC 7578): a bounded reader and a writer, without I/O.

use bytes::Bytes;
use serde_json::{Map, Value};

/// How much one form may hold; a body over any limit is refused as a whole.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct FormLimits {
    /// The most parts one body may hold.
    pub parts: usize,
    /// The most bytes one part's body may hold.
    pub part_bytes: usize,
    /// The most bytes one part's headers may hold, the blank line after them included.
    pub header_bytes: usize,
}

impl FormLimits {
    /// Limits for a form of `parts` parts of at most `part_bytes` bytes each.
    pub const fn new(parts: usize, part_bytes: usize) -> Self {
        FormLimits {
            parts,
            part_bytes,
            header_bytes: 8 * 1024,
        }
    }
}

/// Why a body is not a `multipart/form-data` body Kaeso reads.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum FormError {
    /// The content type is not `multipart/form-data` or names no usable boundary.
    #[error("the content type must be `multipart/form-data` with a boundary")]
    ContentType,
    /// The body does not follow RFC 7578's framing.
    #[error("{0}")]
    Malformed(&'static str),
    /// The body holds more parts than the limits allow.
    #[error("the form has more than {0} parts")]
    TooManyParts(usize),
    /// A part is larger than the limits allow.
    #[error("the part `{name}` is larger than {limit} bytes")]
    PartTooLarge {
        /// The part's field name.
        name: String,
        /// The limit it is over.
        limit: usize,
    },
}

/// One part of a form, borrowing its body from the request body.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct FormPart<'a> {
    /// The field name from `Content-Disposition`.
    pub name: String,
    /// The file name from `Content-Disposition`, for a file field.
    pub filename: Option<String>,
    /// The part's `Content-Type`, as written.
    pub content_type: Option<String>,
    /// The part's body, byte for byte.
    pub body: &'a [u8],
}

impl FormPart<'_> {
    /// The body as text, for a field that isn't a file.
    pub fn text(&self) -> Option<&str> {
        std::str::from_utf8(self.body).ok()
    }
}

// RFC 2046 section 5.1.1: a boundary is 1 to 70 characters.
const MAX_BOUNDARY: usize = 70;

/// The boundary a `multipart/form-data` content type names.
pub fn form_boundary(content_type: &str) -> Result<String, FormError> {
    let mut params = content_type.split(';');
    let essence = params.next().unwrap_or_default().trim();
    if !essence.eq_ignore_ascii_case("multipart/form-data") {
        return Err(FormError::ContentType);
    }
    for param in params {
        let Some((key, value)) = param.split_once('=') else {
            continue;
        };
        if !key.trim().eq_ignore_ascii_case("boundary") {
            continue;
        }
        let value = value.trim();
        let value = match value.strip_prefix('"') {
            Some(rest) => unquote(rest).ok_or(FormError::ContentType)?.0,
            None => value.to_owned(),
        };
        if value.is_empty() || value.len() > MAX_BOUNDARY || value.ends_with(' ') {
            return Err(FormError::ContentType);
        }
        return Ok(value);
    }
    Err(FormError::ContentType)
}

/// Reads the parts of `body`, whose content type named `boundary`.
pub fn read_form<'a>(
    body: &'a [u8],
    boundary: &str,
    limits: FormLimits,
) -> Result<Vec<FormPart<'a>>, FormError> {
    let dash = format!("--{boundary}");
    let delimiter = format!("\r\n--{boundary}");
    let first = if body.starts_with(dash.as_bytes()) {
        0
    } else {
        find(body, delimiter.as_bytes())
            .map(|at| at + 2)
            .ok_or(FormError::Malformed("the body has no boundary line"))?
    };
    let mut rest = after(body, first + dash.len());
    let mut parts = Vec::new();
    loop {
        if rest.starts_with(b"--") {
            return Ok(parts);
        }
        rest = skip_padding(rest);
        rest = rest
            .strip_prefix(b"\r\n")
            .ok_or(FormError::Malformed("a boundary line does not end in CRLF"))?;
        if parts.len() == limits.parts {
            return Err(FormError::TooManyParts(limits.parts));
        }
        let window = rest.get(..limits.header_bytes).unwrap_or(rest);
        let (head, body) = match find(window, b"\r\n\r\n") {
            Some(at) => (window.get(..at).unwrap_or_default(), after(rest, at + 4)),
            None if rest.starts_with(b"\r\n") => (&b""[..], after(rest, 2)),
            None => return Err(FormError::Malformed("a part's headers do not end")),
        };
        let mut part = headers(head)?;
        let end = find(body, delimiter.as_bytes()).ok_or(FormError::Malformed(
            "the body ends before its closing boundary",
        ))?;
        if end > limits.part_bytes {
            return Err(FormError::PartTooLarge {
                name: part.name,
                limit: limits.part_bytes,
            });
        }
        part.body = body.get(..end).unwrap_or_default();
        parts.push(part);
        rest = after(body, end + delimiter.len());
    }
}

fn after(bytes: &[u8], at: usize) -> &[u8] {
    bytes.get(at..).unwrap_or_default()
}

// RFC 2046 lets transport padding (spaces and tabs) follow a boundary.
fn skip_padding(mut rest: &[u8]) -> &[u8] {
    while let [b' ' | b'\t', tail @ ..] = rest {
        rest = tail;
    }
    rest
}

fn headers(head: &[u8]) -> Result<FormPart<'static>, FormError> {
    let text = std::str::from_utf8(head)
        .map_err(|_| FormError::Malformed("a part's headers are not UTF-8"))?;
    let mut part = FormPart {
        name: String::new(),
        filename: None,
        content_type: None,
        body: &[],
    };
    let mut named = false;
    for line in text.split("\r\n") {
        let (key, value) = line
            .split_once(':')
            .ok_or(FormError::Malformed("a part's header has no colon"))?;
        let key = key.trim();
        if key.eq_ignore_ascii_case("content-disposition") {
            disposition(value, &mut part)?;
            named = true;
        } else if key.eq_ignore_ascii_case("content-type") {
            part.content_type = Some(value.trim().to_owned());
        }
    }
    if !named {
        return Err(FormError::Malformed(
            "a part has no `Content-Disposition: form-data` header",
        ));
    }
    Ok(part)
}

fn disposition(value: &str, part: &mut FormPart<'_>) -> Result<(), FormError> {
    let value = value.trim_start();
    let (kind, mut rest) = value.split_once(';').unwrap_or((value, ""));
    if !kind.trim().eq_ignore_ascii_case("form-data") {
        return Err(FormError::Malformed(
            "a part's `Content-Disposition` is not `form-data`",
        ));
    }
    let mut name = None;
    loop {
        rest = rest.trim_start_matches([' ', '\t', ';']);
        if rest.is_empty() {
            break;
        }
        let (key, after) = rest.split_once('=').ok_or(FormError::Malformed(
            "a `Content-Disposition` parameter has no value",
        ))?;
        let after = after.trim_start();
        let (value, tail) = match after.strip_prefix('"') {
            Some(quoted) => unquote(quoted).ok_or(FormError::Malformed(
                "a `Content-Disposition` parameter is not closed",
            ))?,
            None => {
                let end = after.find(';').unwrap_or(after.len());
                let (value, tail) = after.split_at(end);
                (value.trim().to_owned(), tail)
            }
        };
        match key.trim().to_ascii_lowercase().as_str() {
            "name" => name = Some(value),
            "filename" => part.filename = Some(value),
            _ => {}
        }
        rest = tail;
    }
    part.name = name.ok_or(FormError::Malformed("a part has no field name"))?;
    Ok(())
}

// A quoted string after its opening quote: the value and what follows the closing quote.
fn unquote(s: &str) -> Option<(String, &str)> {
    let mut out = String::new();
    let mut chars = s.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Some((out, s.get(i + 1..).unwrap_or_default())),
            '\\' => out.push(chars.next()?.1),
            c => out.push(c),
        }
    }
    None
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    let first = *needle.first()?;
    let mut at = 0;
    while let Some(window) = haystack.get(at..) {
        let i = window.iter().position(|b| *b == first)? + at;
        if haystack.get(i..)?.starts_with(needle) {
            return Some(i);
        }
        at = i + 1;
    }
    None
}

/// A request body that uploaded files are taken from: shared when the caller holds it as
/// [`Bytes`], so each file is a slice of it, and copied file by file otherwise.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Upload<'a> {
    Shared(&'a Bytes),
    Borrowed(&'a [u8]),
}

impl<'a> Upload<'a> {
    pub(crate) fn bytes(self) -> &'a [u8] {
        match self {
            Upload::Shared(b) => b,
            Upload::Borrowed(b) => b,
        }
    }

    /// `part`, a slice of the body, as its own buffer.
    pub(crate) fn share(self, part: &[u8]) -> Bytes {
        let Upload::Shared(body) = self else {
            return Bytes::copy_from_slice(part);
        };
        let start = (part.as_ptr() as usize).checked_sub(body.as_ptr() as usize);
        match start {
            Some(start) if start.saturating_add(part.len()) <= body.len() => {
                body.slice(start..start + part.len())
            }
            _ => Bytes::copy_from_slice(part),
        }
    }
}

/// Writes a `multipart/form-data` body part by part.
#[derive(Clone, Debug)]
pub struct FormWriter {
    boundary: String,
    body: Vec<u8>,
}

/// One field to write: text, or a file with its name and media type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum FormField<'a> {
    /// A text field.
    Text(&'a str, &'a str),
    /// A file field: name, file name, media type and bytes.
    File(&'a str, &'a str, &'a str, &'a [u8]),
    /// A field with a media type and no file name, such as a JSON part: name, media type and
    /// bytes.
    Typed(&'a str, &'a str, &'a [u8]),
}

impl FormWriter {
    /// A body of `fields` with a boundary that occurs in none of them.
    pub fn write(fields: &[FormField<'_>]) -> FormWriter {
        let boundary = boundary_for(fields);
        // The body is allocated once at its final size: growing it while a large file is
        // copied in would hold the file twice, and reserve twice its size, while it moves.
        let size = fields
            .iter()
            .map(|f| match f {
                FormField::Text(n, v) => FIELD_FRAMING + 3 * n.len() + v.len(),
                FormField::File(n, f, t, bytes) => {
                    FIELD_FRAMING + 3 * (n.len() + f.len()) + t.len() + bytes.len()
                }
                FormField::Typed(n, t, bytes) => {
                    FIELD_FRAMING + 3 * n.len() + t.len() + bytes.len()
                }
            })
            .fold(boundary.len() + 6, |sum, n| {
                sum.saturating_add(n).saturating_add(boundary.len())
            });
        let mut w = FormWriter {
            boundary,
            body: Vec::with_capacity(size),
        };
        for field in fields {
            w.field(*field);
        }
        w.body.extend_from_slice(b"--");
        w.body.extend_from_slice(w.boundary.as_bytes());
        w.body.extend_from_slice(b"--\r\n");
        w
    }

    /// The `Content-Type` header value for the body.
    pub fn content_type(&self) -> String {
        format!("multipart/form-data; boundary={}", self.boundary)
    }

    /// The body.
    pub fn into_body(self) -> Vec<u8> {
        self.body
    }

    fn field(&mut self, field: FormField<'_>) {
        let b = &mut self.body;
        b.extend_from_slice(b"--");
        b.extend_from_slice(self.boundary.as_bytes());
        b.extend_from_slice(b"\r\nContent-Disposition: form-data; name=\"");
        match field {
            FormField::Text(name, value) => {
                b.extend_from_slice(quoted(name).as_bytes());
                b.extend_from_slice(b"\"\r\n\r\n");
                b.extend_from_slice(value.as_bytes());
            }
            FormField::File(name, filename, media_type, bytes) => {
                b.extend_from_slice(quoted(name).as_bytes());
                b.extend_from_slice(b"\"; filename=\"");
                b.extend_from_slice(quoted(filename).as_bytes());
                b.extend_from_slice(b"\"\r\nContent-Type: ");
                b.extend_from_slice(header_safe(media_type).as_bytes());
                b.extend_from_slice(b"\r\n\r\n");
                b.extend_from_slice(bytes);
            }
            FormField::Typed(name, media_type, bytes) => {
                b.extend_from_slice(quoted(name).as_bytes());
                b.extend_from_slice(b"\"\r\nContent-Type: ");
                b.extend_from_slice(header_safe(media_type).as_bytes());
                b.extend_from_slice(b"\r\n\r\n");
                b.extend_from_slice(bytes);
            }
        }
        b.extend_from_slice(b"\r\n");
    }
}

// `--`, the `Content-Disposition` and `Content-Type` lines and the CRLFs around one field,
// without the boundary and the names.
const FIELD_FRAMING: usize = 128;

// RFC 7578 section 4.2: browsers percent-encode `"`, CR and LF in field and file names.
fn quoted(s: &str) -> String {
    s.replace('"', "%22")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

fn header_safe(s: &str) -> String {
    s.chars().filter(|c| !matches!(c, '\r' | '\n')).collect()
}

// Derived from the content instead of a random source, so that writing stays deterministic;
// a candidate found in any field is skipped.
fn boundary_for(fields: &[FormField<'_>]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for b in bytes {
            hash ^= u64::from(*b);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    };
    for f in fields {
        match f {
            FormField::Text(n, v) => {
                feed(n.as_bytes());
                feed(v.as_bytes());
            }
            FormField::File(n, f, t, bytes) => {
                feed(n.as_bytes());
                feed(f.as_bytes());
                feed(t.as_bytes());
                feed(bytes.get(..4096).unwrap_or(bytes));
            }
            FormField::Typed(n, t, bytes) => {
                feed(n.as_bytes());
                feed(t.as_bytes());
                feed(bytes.get(..4096).unwrap_or(bytes));
            }
        }
    }
    let mut n = hash;
    loop {
        let candidate = format!("kaeso-form-{n:016x}");
        let found = fields.iter().any(|f| {
            let hay: &[&[u8]] = match f {
                FormField::Text(a, b) => &[a.as_bytes(), b.as_bytes()],
                FormField::File(a, b, c, d) => &[a.as_bytes(), b.as_bytes(), c.as_bytes(), d],
                FormField::Typed(a, b, c) => &[a.as_bytes(), b.as_bytes(), c],
            };
            hay.iter().any(|h| find(h, candidate.as_bytes()).is_some())
        });
        if !found {
            return candidate;
        }
        n = n.wrapping_add(1);
    }
}

// Multipart names nest with brackets, as OpenAI's SDKs write `svg[model]` and
// `svg[reasoning][effort]`; JSON bodies carry the same fields as objects.
pub(crate) fn insert_nested(
    out: &mut Map<String, Value>,
    name: &str,
    value: &str,
    integers: &[&str],
) -> Result<(), String> {
    let (head, rest) = match name.find('[') {
        Some(i) => name.split_at(i),
        None => (name, ""),
    };
    let mut keys = Vec::new();
    let mut tail = rest;
    while let Some(after) = tail.strip_prefix('[') {
        let end = after
            .find(']')
            .ok_or_else(|| format!("the field name `{name}` is not closed"))?;
        let (key, more) = after.split_at(end);
        keys.push(key);
        tail = more.get(1..).unwrap_or_default();
    }
    if !tail.is_empty() || head.is_empty() {
        return Err(format!("the field name `{name}` cannot be read"));
    }
    let leaf = if keys.is_empty() && integers.contains(&head) {
        value
            .trim()
            .parse::<u64>()
            .map(Value::from)
            .unwrap_or_else(|_| Value::String(value.to_owned()))
    } else {
        Value::String(value.to_owned())
    };
    let mut slot = out.entry(head.to_owned()).or_insert(Value::Null);
    for key in keys {
        if key.is_empty() {
            if !slot.is_array() {
                *slot = Value::Array(Vec::new());
            }
            let Value::Array(items) = slot else {
                return Err(format!("the field name `{name}` cannot be read"));
            };
            items.push(Value::Null);
            slot = items
                .last_mut()
                .ok_or_else(|| format!("the field name `{name}` cannot be read"))?;
        } else {
            if !slot.is_object() {
                *slot = Value::Object(Map::new());
            }
            let Value::Object(map) = slot else {
                return Err(format!("the field name `{name}` cannot be read"));
            };
            slot = map.entry(key.to_owned()).or_insert(Value::Null);
        }
    }
    *slot = leaf;
    Ok(())
}

/// The form fields of a JSON value, nested names in brackets as OpenAI's SDKs write them.
pub(crate) fn flatten(key: &str, value: &Value, out: &mut Vec<(String, String)>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                flatten(&format!("{key}[{k}]"), v, out);
            }
        }
        Value::Array(items) => {
            for v in items {
                flatten(&format!("{key}[]"), v, out);
            }
        }
        Value::String(s) => out.push((key.to_owned(), s.clone())),
        Value::Null => {}
        other => out.push((key.to_owned(), other.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIMITS: FormLimits = FormLimits::new(8, 1024);

    #[test]
    fn reads_what_the_writer_writes() {
        let png = b"\x89PNG\r\n\x1a\n--kaeso\r\n";
        let w = FormWriter::write(&[
            FormField::Text("prompt", "a red \"fox\"\r\nline two"),
            FormField::File("image[]", "fox.png", "image/png", png),
            FormField::Text("n", "2"),
        ]);
        let boundary = form_boundary(&w.content_type()).unwrap();
        let body = w.into_body();
        let parts = read_form(&body, &boundary, LIMITS).unwrap();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0].name, "prompt");
        assert_eq!(parts[0].text(), Some("a red \"fox\"\r\nline two"));
        assert_eq!(parts[1].name, "image[]");
        assert_eq!(parts[1].filename.as_deref(), Some("fox.png"));
        assert_eq!(parts[1].content_type.as_deref(), Some("image/png"));
        assert_eq!(parts[1].body, png);
        assert_eq!(parts[2].text(), Some("2"));
    }

    #[test]
    fn reads_a_body_as_python_clients_send_it() {
        let body = b"preamble\r\n--abc\r\nContent-Disposition: form-data; name=\"model\"\r\n\r\ngpt-image-2\r\n--abc\r\ncontent-disposition: form-data; name=\"image\"; filename=\"a.png\"\r\ncontent-type: image/png\r\n\r\nPNG\r\n--abc--\r\nepilogue";
        let parts = read_form(body, "abc", LIMITS).unwrap();
        assert_eq!(parts[0].text(), Some("gpt-image-2"));
        assert_eq!(parts[1].body, b"PNG");
        assert_eq!(
            form_boundary("multipart/form-data; charset=utf-8; boundary=\"a b\"").unwrap(),
            "a b"
        );
    }

    #[test]
    fn refuses_what_breaks_the_framing_or_the_limits() {
        assert_eq!(
            form_boundary("application/json"),
            Err(FormError::ContentType)
        );
        assert_eq!(
            form_boundary("multipart/form-data"),
            Err(FormError::ContentType)
        );
        let long = format!("multipart/form-data; boundary={}", "x".repeat(71));
        assert_eq!(form_boundary(&long), Err(FormError::ContentType));
        let part = |n: usize| {
            format!("--b\r\nContent-Disposition: form-data; name=\"f{n}\"\r\n\r\nvalue\r\n")
        };
        let many: String = (0..9).map(part).collect::<String>() + "--b--";
        assert_eq!(
            read_form(many.as_bytes(), "b", LIMITS),
            Err(FormError::TooManyParts(8))
        );
        let big = format!(
            "--b\r\nContent-Disposition: form-data; name=\"image\"\r\n\r\n{}\r\n--b--",
            "x".repeat(1025)
        );
        assert!(matches!(
            read_form(big.as_bytes(), "b", LIMITS),
            Err(FormError::PartTooLarge { limit: 1024, .. })
        ));
        for bad in [
            "no boundary at all",
            "--b\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\nunclosed",
            "--b\r\nContent-Type: text/plain\r\n\r\nx\r\n--b--",
            "--b\r\nContent-Disposition: attachment; name=\"a\"\r\n\r\nx\r\n--b--",
            "--b\r\nContent-Disposition: form-data; name=\"a\r\n\r\nx\r\n--b--",
            "--b\r\nContent-Disposition: form-data\r\n\r\nx\r\n--b--",
            "--bjunk\r\n",
        ] {
            assert!(
                matches!(
                    read_form(bad.as_bytes(), "b", LIMITS),
                    Err(FormError::Malformed(_))
                ),
                "{bad}"
            );
        }
    }

    proptest::proptest! {
        #[test]
        fn any_fields_survive_a_round_trip(
            fields in proptest::collection::vec(("[a-z\\[\\]]{1,12}", proptest::collection::vec(proptest::num::u8::ANY, 0..200)), 0..6)
        ) {
            let written: Vec<FormField<'_>> = fields
                .iter()
                .map(|(n, b)| FormField::File(n, "f.bin", "application/octet-stream", b))
                .collect();
            let w = FormWriter::write(&written);
            let boundary = form_boundary(&w.content_type()).unwrap();
            let body = w.into_body();
            let parts = read_form(&body, &boundary, LIMITS).unwrap();
            proptest::prop_assert_eq!(parts.len(), fields.len());
            for (p, (n, b)) in parts.iter().zip(&fields) {
                proptest::prop_assert_eq!(&p.name, n);
                proptest::prop_assert_eq!(p.body, b.as_slice());
            }
        }

        #[test]
        fn never_panics_on_any_bytes(body in proptest::collection::vec(proptest::num::u8::ANY, 0..400)) {
            let _ = read_form(&body, "b", LIMITS);
        }
    }
}
