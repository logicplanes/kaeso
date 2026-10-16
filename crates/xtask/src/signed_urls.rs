use std::borrow::Cow;
use std::collections::BTreeSet;
use std::ops::Range;

const SCHEMES: [&[u8]; 4] = [b"https://", b"http://", b"https:\\/\\/", b"http:\\/\\/"];
const SIGNING_PARAMETERS: [&str; 5] = [
    "x-amz-credential",
    "x-amz-signature",
    "x-amz-security-token",
    "x-goog-credential",
    "x-goog-signature",
];

pub(crate) fn strip_signed_queries(text: &[u8]) -> Cow<'_, [u8]> {
    let mut kept = Vec::new();
    let mut copied = 0;
    let mut at = 0;
    while let Some(start) = next_url(text, at) {
        let end = url_end(text, start);
        if let Some(query) = signed_query(text, start..end) {
            kept.extend_from_slice(text.get(copied..query.start).unwrap_or_default());
            copied = query.end;
        }
        at = end;
    }
    if copied == 0 {
        return Cow::Borrowed(text);
    }
    kept.extend_from_slice(text.get(copied..).unwrap_or_default());
    Cow::Owned(kept)
}

fn next_url(text: &[u8], from: usize) -> Option<usize> {
    (from..text.len()).find_map(|at| {
        SCHEMES
            .iter()
            .find(|scheme| {
                text.get(at..at + scheme.len())
                    .is_some_and(|word| word.eq_ignore_ascii_case(scheme))
            })
            .map(|scheme| at + scheme.len())
    })
}

fn url_end(text: &[u8], mut at: usize) -> usize {
    while let Some((c, width)) = read(text, at) {
        if ends_url(c) {
            break;
        }
        at += width;
    }
    at
}

fn ends_url(c: char) -> bool {
    c.is_ascii_control() || matches!(c, ' ' | '"' | '\'' | '<' | '>' | '`' | ')' | '\\')
}

fn signed_query(text: &[u8], url: Range<usize>) -> Option<Range<usize>> {
    let query = query_range(text, url)?;
    let decoded = decode(text.get(query.start + 1..query.end)?);
    is_signed(&decoded).then_some(query)
}

fn query_range(text: &[u8], url: Range<usize>) -> Option<Range<usize>> {
    let mut at = url.start;
    let mut question = None;
    while at < url.end {
        let (c, width) = read(text, at)?;
        match (question, c) {
            (_, '#') => break,
            (None, '?') => question = Some(at),
            (Some(_), c) if !c.is_ascii() => break,
            _ => {}
        }
        at += width;
    }
    question.map(|start| start..at)
}

fn decode(text: &[u8]) -> String {
    let mut decoded = String::with_capacity(text.len());
    let mut at = 0;
    while let Some((c, width)) = read(text, at) {
        decoded.push(c);
        at += width;
    }
    decoded
}

fn read(text: &[u8], at: usize) -> Option<(char, usize)> {
    let rest = text.get(at..)?;
    let first = *rest.first()?;
    if first == b'\\' {
        return Some(json_escape(rest).unwrap_or(('\\', 1)));
    }
    if rest.starts_with(b"&amp;") {
        return Some(('&', 5));
    }
    Some((char::from(first), 1))
}

fn json_escape(rest: &[u8]) -> Option<(char, usize)> {
    match rest.get(1)? {
        b'/' => Some(('/', 2)),
        b'u' => {
            let hex = rest.get(2..6)?;
            if !hex.iter().all(u8::is_ascii_hexdigit) {
                return None;
            }
            let code = u32::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()?;
            Some((char::from_u32(code)?, 6))
        }
        _ => None,
    }
}

fn is_signed(query: &str) -> bool {
    let names: BTreeSet<String> = query
        .split('&')
        .filter_map(|pair| pair.split('=').next())
        .map(str::to_ascii_lowercase)
        .collect();
    let has = |name: &str| names.contains(name);
    SIGNING_PARAMETERS.iter().any(|name| has(name))
        || (has("googleaccessid") && has("signature"))
        || (has("sig") && (has("sv") || has("se")))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn strip(text: &str) -> String {
        String::from_utf8(strip_signed_queries(text.as_bytes()).into_owned()).unwrap()
    }

    #[test]
    fn aws_signature_v4_queries_are_removed() {
        for query in [
            "X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Credential=AKIAIOSFODNN7EXAMPLE%2F20260101%2Fus-east-1%2Fs3%2Faws4_request&X-Amz-Signature=abc123",
            "X-Amz-Signature=abc123",
            "X-Amz-Security-Token=FwoGZXIvYXdzEXAMPLE&response-content-type=application%2Fpdf",
            "x-amz-credential=AKIAIOSFODNN7EXAMPLE",
        ] {
            assert_eq!(
                strip(&format!(
                    r#"{{"url":"https://bucket.s3.amazonaws.com/a/b.pdf?{query}"}}"#
                )),
                r#"{"url":"https://bucket.s3.amazonaws.com/a/b.pdf"}"#,
                "{query}"
            );
        }
    }

    #[test]
    fn google_cloud_queries_are_removed() {
        for query in [
            "X-Goog-Algorithm=GOOG4-RSA-SHA256&X-Goog-Credential=signer%40example.iam.gserviceaccount.com&X-Goog-Signature=abc",
            "X-Goog-Signature=abc",
            "GoogleAccessId=signer%40example.iam.gserviceaccount.com&Expires=1700000000&Signature=abc%3D",
        ] {
            assert_eq!(
                strip(&format!(
                    "see https://storage.googleapis.com/bucket/report.pdf?{query} for details"
                )),
                "see https://storage.googleapis.com/bucket/report.pdf for details",
                "{query}"
            );
        }
        assert_eq!(
            strip("https://example.com/a?GoogleAccessId=someone"),
            "https://example.com/a?GoogleAccessId=someone"
        );
    }

    #[test]
    fn azure_shared_access_signatures_are_removed() {
        for query in [
            "sv=2024-11-04&ss=b&srt=o&sp=r&se=2026-01-01T00:00:00Z&sig=abc%3D",
            "sig=abc%3D&se=2026-01-01T00:00:00Z",
            "sp=r&sv=2024-11-04&sig=abc%3D",
        ] {
            assert_eq!(
                strip(&format!(
                    "[file](https://account.blob.core.windows.net/c/file.pdf?{query})"
                )),
                "[file](https://account.blob.core.windows.net/c/file.pdf)",
                "{query}"
            );
        }
        assert_eq!(
            strip("https://example.com/search?sig=figs&q=fruit"),
            "https://example.com/search?sig=figs&q=fruit"
        );
    }

    #[test]
    fn json_escaped_urls_are_recognised() {
        assert_eq!(
            strip(
                r#"{"url":"https:\/\/bucket.s3.amazonaws.com\/a.pdf?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Signature=abc","n":1}"#
            ),
            r#"{"url":"https:\/\/bucket.s3.amazonaws.com\/a.pdf","n":1}"#
        );
        assert_eq!(
            strip(r#""https://a.example.com/x?sv=1&sig=abc&se=2""#),
            r#""https://a.example.com/x""#
        );
        assert_eq!(
            strip(r#"<a href=\"https://a.example.com/x?sv=1&amp;sig=abc\">x</a>"#),
            r#"<a href=\"https://a.example.com/x\">x</a>"#
        );
    }

    #[test]
    fn the_fragment_and_everything_around_the_url_are_kept() {
        assert_eq!(
            strip("HTTPS://b.s3.amazonaws.com/k?X-Amz-Signature=abc#page=2\r\nnext"),
            "HTTPS://b.s3.amazonaws.com/k#page=2\r\nnext"
        );
        assert_eq!(
            strip("https://b.s3.amazonaws.com/Ocean\u{e1}rio?X-Amz-Signature=abc\u{3002}"),
            "https://b.s3.amazonaws.com/Ocean\u{e1}rio\u{3002}"
        );
        assert_eq!(
            strip("'https://b.example.com/k?X-Amz-Signature=a\\nb'"),
            "'https://b.example.com/k\\nb'"
        );
    }

    #[test]
    fn every_signed_url_among_many_is_stripped() {
        let stream = "event: done\r\n\
            data: {\"sources\":[{\"url\":\"https://oceanario.pt/en/?utm_source=openai\"},\
            {\"url\":\"https://b.s3.eu-west-1.amazonaws.com/k?X-Amz-Credential=AKIAIOSFODNN7EXAMPLE&X-Amz-Signature=1\"},\
            {\"url\":\"https://maps.apple.com/place?place-id=I5F8\"},\
            {\"url\":\"https://storage.googleapis.com/b/o?X-Goog-Signature=2\"}]}\r\n\r\n";
        let expected = "event: done\r\n\
            data: {\"sources\":[{\"url\":\"https://oceanario.pt/en/?utm_source=openai\"},\
            {\"url\":\"https://b.s3.eu-west-1.amazonaws.com/k\"},\
            {\"url\":\"https://maps.apple.com/place?place-id=I5F8\"},\
            {\"url\":\"https://storage.googleapis.com/b/o\"}]}\r\n\r\n";
        assert_eq!(strip(stream), expected);
    }

    #[test]
    fn text_without_signed_urls_is_borrowed_unchanged() {
        for text in [
            "",
            "no links at all",
            "https://",
            "http:\\/\\/example.com\\/a?b=c&d=e",
            "https://www.flytap.com/x.pdf?hash=7FB4&la=es-ES and https://example.com/?q=Signature",
            "https://example.com/a?Signature=x&Expires=1",
            "https://example.com/a#?X-Amz-Signature=x",
            "https://a.example.com/p\\u00",
            "https://a.example.com/p\\u12zz?X-Amz-Signature=x",
            "https://a.example.com/p\\uD800?X-Amz-Signature=x",
        ] {
            assert!(
                matches!(strip_signed_queries(text.as_bytes()), Cow::Borrowed(_)),
                "{text}"
            );
        }
        let bytes = b"data: \xff\xfe https://a.example.com/?X-Amz-Signature=1 \xff\n";
        assert_eq!(
            strip_signed_queries(bytes).as_ref(),
            b"data: \xff\xfe https://a.example.com/ \xff\n"
        );
    }

    #[test]
    fn no_fixture_carries_a_signed_url() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../kaeso-codecs/fixtures");
        let mut pending = vec![fixtures];
        let mut checked = 0;
        while let Some(path) = pending.pop() {
            if path.is_dir() {
                pending.extend(
                    std::fs::read_dir(&path)
                        .unwrap()
                        .map(|entry| entry.unwrap().path()),
                );
                continue;
            }
            let text = std::fs::read(&path).unwrap();
            assert!(
                matches!(strip_signed_queries(&text), Cow::Borrowed(_)),
                "{} holds a signed URL",
                path.display()
            );
            checked += 1;
        }
        assert!(checked > 100, "{checked}");
    }
}
