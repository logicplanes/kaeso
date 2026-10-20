use kaeso_kpp::{ErrorKind, KppError, RawJson};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

pub const DEFAULT_MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct SseEvent {
    pub event: Option<String>,
    pub data: String,
    pub id: Option<String>,
    pub retry: Option<u64>,
    pub other: Vec<(String, String)>,
    pub has_data: bool,
}

impl SseEvent {
    pub fn new(event: Option<&str>, data: impl Into<String>) -> Self {
        SseEvent {
            event: event.map(str::to_owned),
            data: data.into(),
            has_data: true,
            ..SseEvent::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SseItem {
    Event(SseEvent),
    Comment(String),
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SseError {
    #[error("the provider's event stream contains a line that is not valid UTF-8")]
    InvalidUtf8,
    #[error("the provider sent a streamed event larger than Kaeso's limit of {limit} bytes")]
    #[non_exhaustive]
    TooLarge { limit: usize },
    #[error("the provider's event stream ended in the middle of an event")]
    Truncated,
}

impl From<SseError> for KppError {
    fn from(e: SseError) -> Self {
        KppError::new(ErrorKind::ProviderError, e.to_string())
    }
}

#[derive(Clone, Debug)]
pub struct SseParser {
    line: Vec<u8>,
    skip_lf: bool,
    started: bool,
    frame: SseEvent,
    has_fields: bool,
    data_lines: usize,
    frame_bytes: usize,
    max_bytes: usize,
}

impl Default for SseParser {
    fn default() -> Self {
        SseParser::new()
    }
}

impl SseParser {
    pub fn new() -> Self {
        SseParser::with_limit(DEFAULT_MAX_FRAME_BYTES)
    }

    pub fn with_limit(max_bytes: usize) -> Self {
        SseParser {
            line: Vec::new(),
            skip_lf: false,
            started: false,
            frame: SseEvent::default(),
            has_fields: false,
            data_lines: 0,
            frame_bytes: 0,
            max_bytes,
        }
    }

    pub fn feed(&mut self, chunk: &[u8]) -> Result<Vec<SseItem>, SseError> {
        let mut out = Vec::new();
        self.feed_into(chunk, &mut out)?;
        Ok(out)
    }

    pub fn feed_into(&mut self, chunk: &[u8], out: &mut Vec<SseItem>) -> Result<(), SseError> {
        let mut rest = chunk;
        if !self.started && !rest.is_empty() {
            let bom = [0xEF, 0xBB, 0xBF];
            let pending = self.line.len();
            let need = bom.len().saturating_sub(pending);
            let take = need.min(rest.len());
            self.line
                .extend_from_slice(rest.get(..take).unwrap_or_default());
            rest = rest.get(take..).unwrap_or_default();
            if self.line.len() < bom.len() && bom.starts_with(&self.line) {
                return Ok(());
            }
            self.started = true;
            if self.line == bom {
                self.line.clear();
            } else {
                let buffered = std::mem::take(&mut self.line);
                self.consume(&buffered, out)?;
            }
        }
        self.consume(rest, out)
    }

    pub fn finish(&mut self) -> Result<Vec<SseItem>, SseError> {
        let mut out = Vec::new();
        self.started = true;
        self.skip_lf = false;
        if !self.line.is_empty() {
            self.line.clear();
            self.dispatch(&mut Vec::new());
            return Err(SseError::Truncated);
        }
        self.dispatch(&mut out);
        Ok(out)
    }

    fn consume(&mut self, mut bytes: &[u8], out: &mut Vec<SseItem>) -> Result<(), SseError> {
        if self.skip_lf {
            if let Some((&b'\n', tail)) = bytes.split_first() {
                bytes = tail;
                self.skip_lf = false;
            } else if !bytes.is_empty() {
                self.skip_lf = false;
            }
        }
        while let Some(pos) = bytes.iter().position(|&b| b == b'\n' || b == b'\r') {
            let (head, tail) = bytes.split_at(pos);
            let Some((&terminator, after)) = tail.split_first() else {
                break;
            };
            let line = if self.line.is_empty() {
                std::borrow::Cow::Borrowed(head)
            } else {
                let mut joined = std::mem::take(&mut self.line);
                joined.extend_from_slice(head);
                std::borrow::Cow::Owned(joined)
            };
            self.check_len(line.len())?;
            self.process_line(&line, out)?;
            bytes = after;
            if terminator == b'\r' {
                match bytes.split_first() {
                    Some((&b'\n', tail)) => bytes = tail,
                    Some(_) => {}
                    None => self.skip_lf = true,
                }
            }
        }
        self.check_len(self.line.len().saturating_add(bytes.len()))?;
        self.line.extend_from_slice(bytes);
        Ok(())
    }

    fn check_len(&self, len: usize) -> Result<(), SseError> {
        if len > self.max_bytes {
            return Err(SseError::TooLarge {
                limit: self.max_bytes,
            });
        }
        Ok(())
    }

    fn process_line(&mut self, line: &[u8], out: &mut Vec<SseItem>) -> Result<(), SseError> {
        let line = std::str::from_utf8(line).map_err(|_| SseError::InvalidUtf8)?;
        if line.is_empty() {
            self.dispatch(out);
            return Ok(());
        }
        if let Some(comment) = line.strip_prefix(':') {
            let comment = comment.strip_prefix(' ').unwrap_or(comment);
            out.push(SseItem::Comment(comment.to_owned()));
            return Ok(());
        }
        let (name, value) = match line.split_once(':') {
            Some((name, value)) => (name, value.strip_prefix(' ').unwrap_or(value)),
            None => (line, ""),
        };
        self.has_fields = true;
        self.frame_bytes = self.frame_bytes.saturating_add(line.len());
        if self.frame_bytes > self.max_bytes {
            return Err(SseError::TooLarge {
                limit: self.max_bytes,
            });
        }
        match name {
            "data" => {
                if self.data_lines > 0 {
                    self.frame.data.push('\n');
                }
                self.frame.data.push_str(value);
                self.frame.has_data = true;
                self.data_lines += 1;
            }
            "event" => self.frame.event = Some(value.to_owned()),
            "id" if !value.contains('\0') => self.frame.id = Some(value.to_owned()),
            "retry" if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) => {
                match value.parse() {
                    Ok(ms) => self.frame.retry = Some(ms),
                    Err(_) => self.frame.other.push((name.to_owned(), value.to_owned())),
                }
            }
            _ => self.frame.other.push((name.to_owned(), value.to_owned())),
        }
        Ok(())
    }

    fn dispatch(&mut self, out: &mut Vec<SseItem>) {
        if self.has_fields {
            out.push(SseItem::Event(std::mem::take(&mut self.frame)));
        }
        self.has_fields = false;
        self.data_lines = 0;
        self.frame_bytes = 0;
    }
}

pub(crate) fn framing(ev: &SseEvent) -> Option<serde_json::Value> {
    if ev.id.is_none() && ev.retry.is_none() && ev.other.is_empty() {
        return None;
    }
    let mut map = serde_json::Map::new();
    if let Some(id) = &ev.id {
        map.insert("id".to_owned(), id.as_str().into());
    }
    if let Some(retry) = ev.retry {
        map.insert("retry".to_owned(), retry.into());
    }
    if !ev.other.is_empty() {
        let other = ev
            .other
            .iter()
            .map(|(k, v)| serde_json::Value::from(vec![k.as_str(), v.as_str()]))
            .collect();
        map.insert("other".to_owned(), serde_json::Value::Array(other));
    }
    Some(serde_json::Value::Object(map))
}

pub(crate) fn set_framing(ev: &mut SseEvent, framing: &serde_json::Value) {
    ev.id = framing
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    ev.retry = framing.get("retry").and_then(serde_json::Value::as_u64);
    ev.other = framing
        .get("other")
        .and_then(serde_json::Value::as_array)
        .map(|pairs| {
            pairs
                .iter()
                .filter_map(|pair| {
                    let name = pair.get(0)?.as_str()?;
                    let value = pair.get(1)?.as_str()?;
                    Some((name.to_owned(), value.to_owned()))
                })
                .collect()
        })
        .unwrap_or_default();
}

pub fn write_event(out: &mut Vec<u8>, event: Option<&str>, data: &str) {
    if let Some(event) = event {
        write_field(out, "event", event);
    }
    write_data(out, data);
    out.push(b'\n');
}

pub fn write_item(out: &mut Vec<u8>, item: &SseItem) {
    match item {
        SseItem::Comment(text) => {
            for line in lines(text) {
                out.extend_from_slice(b": ");
                out.extend_from_slice(line.as_bytes());
                out.push(b'\n');
            }
        }
        SseItem::Event(ev) => {
            if starts_with_bom(ev) {
                out.push(b'\n');
            }
            if let Some(event) = &ev.event {
                write_field(out, "event", event);
            }
            if let Some(id) = &ev.id {
                write_field(out, "id", id);
            }
            if let Some(retry) = ev.retry {
                write_field(out, "retry", &retry.to_string());
            }
            for (name, value) in &ev.other {
                let name: String = name
                    .chars()
                    .filter(|c| !matches!(c, '\r' | '\n' | ':'))
                    .collect();
                if !name.is_empty() {
                    write_field(out, &name, value);
                }
            }
            if ev.has_data {
                write_data(out, &ev.data);
            }
            out.push(b'\n');
        }
    }
}

fn starts_with_bom(ev: &SseEvent) -> bool {
    ev.event.is_none()
        && ev.id.is_none()
        && ev.retry.is_none()
        && ev
            .other
            .iter()
            .map(|(name, _)| name.trim_start_matches(['\r', '\n', ':']))
            .find(|name| !name.is_empty())
            .is_some_and(|name| name.starts_with('\u{feff}'))
}

fn write_field(out: &mut Vec<u8>, name: &str, value: &str) {
    out.extend_from_slice(name.as_bytes());
    out.extend_from_slice(b": ");
    out.extend(value.bytes().filter(|b| !matches!(b, b'\r' | b'\n')));
    out.push(b'\n');
}

fn lines(text: &str) -> impl Iterator<Item = &str> {
    let mut rest = Some(text);
    std::iter::from_fn(move || {
        let current = rest?;
        match current.find(['\r', '\n']) {
            Some(pos) => {
                let (line, tail) = current.split_at(pos);
                let skip = if tail.starts_with("\r\n") { 2 } else { 1 };
                rest = tail.get(skip..);
                Some(line)
            }
            None => {
                rest = None;
                Some(current)
            }
        }
    })
}

fn write_data(out: &mut Vec<u8>, data: &str) {
    for line in lines(data) {
        out.extend_from_slice(b"data: ");
        out.extend_from_slice(line.as_bytes());
        out.push(b'\n');
    }
}

#[derive(Default, Serialize, Deserialize)]
struct FrameRecord {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    comment: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    event: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    data: Option<Box<RawValue>>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    text: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    retry: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    other: Vec<(String, String)>,
}

pub(crate) enum WrittenFrame {
    Comment,
    Event(Option<Box<RawValue>>),
}

pub(crate) fn comment_extra(text: &str) -> Result<RawJson, serde_json::Error> {
    RawJson::from_serialize(&FrameRecord {
        comment: Some(text.to_owned()),
        ..FrameRecord::default()
    })
}

pub(crate) fn frame_extra(ev: &SseEvent) -> Result<RawJson, serde_json::Error> {
    let json = ev
        .has_data
        .then(|| serde_json::from_str::<Box<RawValue>>(&ev.data).ok())
        .flatten();
    let text = ev.has_data && json.is_none();
    let data = match json {
        Some(json) => Some(json),
        None if text => Some(serde_json::value::to_raw_value(&ev.data)?),
        None => None,
    };
    RawJson::from_serialize(&FrameRecord {
        comment: None,
        event: ev.event.clone(),
        data,
        text,
        id: ev.id.clone(),
        retry: ev.retry,
        other: ev.other.clone(),
    })
}

pub(crate) fn write_frame_extra(
    out: &mut Vec<u8>,
    raw: &RawValue,
) -> Result<WrittenFrame, serde_json::Error> {
    let f: FrameRecord = serde_json::from_str(raw.get())?;
    if let Some(comment) = f.comment {
        write_item(out, &SseItem::Comment(comment));
        out.push(b'\n');
        return Ok(WrittenFrame::Comment);
    }
    let (data, json) = match f.data {
        Some(d) if f.text => (Some(serde_json::from_str::<String>(d.get())?), None),
        Some(d) => (Some(d.get().to_owned()), Some(d)),
        None => (None, None),
    };
    let has_data = data.is_some();
    let mut ev = SseEvent::new(f.event.as_deref(), data.unwrap_or_default());
    ev.has_data = has_data;
    ev.id = f.id;
    ev.retry = f.retry;
    ev.other = f.other;
    write_item(out, &SseItem::Event(ev));
    Ok(WrittenFrame::Event(json))
}

pub fn parse_all(bytes: &[u8]) -> Result<Vec<SseItem>, SseError> {
    let mut parser = SseParser::new();
    let mut items = parser.feed(bytes)?;
    items.extend(parser.finish()?);
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn events(items: &[SseItem]) -> Vec<&SseEvent> {
        items
            .iter()
            .filter_map(|i| match i {
                SseItem::Event(e) => Some(e),
                SseItem::Comment(_) => None,
            })
            .collect()
    }

    #[test]
    fn parses_named_events_and_multiline_data() {
        let items = parse_all(
            b"event: message_start\ndata: {\"a\":1}\n\nevent: ping\ndata: {}\n\ndata: line1\ndata: line2\n\n",
        )
        .unwrap();
        let evs = events(&items);
        assert_eq!(evs.len(), 3);
        assert_eq!(evs[0].event.as_deref(), Some("message_start"));
        assert_eq!(evs[0].data, "{\"a\":1}");
        assert_eq!(evs[2].event, None);
        assert_eq!(evs[2].data, "line1\nline2");
    }

    #[test]
    fn a_field_whose_name_starts_with_a_bom_is_written_so_it_reads_back() {
        let input = b"\n\xEF\xBB\xBF{  \"model\":oss-120b\",\n  \"stream\": true\n}\n";
        let items = parse_all(input).unwrap();
        let mut written = Vec::new();
        for item in &items {
            write_item(&mut written, item);
        }
        assert_eq!(parse_all(&written).unwrap(), items);
        let SseItem::Event(ev) = &items[0] else {
            panic!("{items:?}")
        };
        assert!(ev.other[0].0.starts_with('\u{feff}'), "{ev:?}");
    }

    #[test]
    fn handles_crlf_cr_comments_bom_and_unknown_fields() {
        let items = parse_all(
            b"\xEF\xBB\xBF: keep-alive\r\nevent:x\rdata:no-space\r\nid: 7\nretry: 1500\nfoo: bar\n\r\n",
        )
        .unwrap();
        assert_eq!(items[0], SseItem::Comment("keep-alive".into()));
        let SseItem::Event(ev) = &items[1] else {
            panic!("expected an event")
        };
        assert_eq!(ev.event.as_deref(), Some("x"));
        assert_eq!(ev.data, "no-space");
        assert_eq!(ev.id.as_deref(), Some("7"));
        assert_eq!(ev.retry, Some(1500));
        assert_eq!(ev.other, vec![("foo".to_owned(), "bar".to_owned())]);
    }

    #[test]
    fn keeps_frames_without_data_and_dispatches_unterminated_frames() {
        let items = parse_all(b"event: lonely\n\ndata: [DONE]\n").unwrap();
        let evs = events(&items);
        assert_eq!(evs[0].event.as_deref(), Some("lonely"));
        assert_eq!(evs[0].data, "");
        assert!(!evs[0].has_data);
        assert_eq!(evs[1].data, "[DONE]");
        assert!(evs[1].has_data);
    }

    #[test]
    fn frames_without_data_lines_are_written_back_without_one() {
        let stream = b"retry: 3000\n\nid: 7\n\ndata:\n\nid\ndata\n\n";
        let items = parse_all(stream).unwrap();
        let evs = events(&items);
        assert_eq!(evs[0].retry, Some(3000));
        assert!(!evs[0].has_data);
        assert_eq!(evs[1].id.as_deref(), Some("7"));
        assert!(!evs[1].has_data);
        assert!(evs[2].has_data && evs[2].data.is_empty());
        assert_eq!(evs[3].id.as_deref(), Some(""));
        assert!(evs[3].has_data && evs[3].data.is_empty());
        let mut out = Vec::new();
        for item in &items {
            write_item(&mut out, item);
        }
        assert_eq!(
            String::from_utf8(out.clone()).unwrap(),
            "retry: 3000\n\nid: 7\n\ndata: \n\nid: \ndata: \n\n"
        );
        assert_eq!(parse_all(&out).unwrap(), items);
    }

    #[test]
    fn framing_survives_a_trip_through_json() {
        let items = parse_all(b"id: 4\nretry: 10\nx: y\nx: z\ndata: d\n\ndata: e\n\n").unwrap();
        let evs = events(&items);
        let framed = framing(evs[0]).unwrap();
        assert_eq!(
            framed,
            serde_json::json!({"id": "4", "retry": 10, "other": [["x", "y"], ["x", "z"]]})
        );
        let mut back = SseEvent::new(None, "d");
        set_framing(&mut back, &framed);
        assert_eq!(&back, evs[0]);
        assert_eq!(framing(evs[1]), None);
        let mut cleared = back.clone();
        set_framing(&mut cleared, &serde_json::json!({"other": [["x"], 1]}));
        assert_eq!(cleared, SseEvent::new(None, "d"));
    }

    #[test]
    fn id_and_retry_follow_the_spec_and_keep_what_it_ignores() {
        let items = parse_all(
            b"id: a\0b\nretry: 12x\nretry\nretry: 99999999999999999999\nid:  spaced\ndata: x\n\n",
        )
        .unwrap();
        let evs = events(&items);
        assert_eq!(evs[0].id.as_deref(), Some(" spaced"));
        assert_eq!(evs[0].retry, None);
        assert_eq!(
            evs[0].other,
            vec![
                ("id".to_owned(), "a\0b".to_owned()),
                ("retry".to_owned(), "12x".to_owned()),
                ("retry".to_owned(), String::new()),
                ("retry".to_owned(), "99999999999999999999".to_owned()),
            ]
        );
        let mut out = Vec::new();
        write_item(&mut out, &items[0]);
        assert_eq!(parse_all(&out).unwrap(), items);
    }

    #[test]
    fn feed_into_keeps_items_before_a_bad_line() {
        let mut p = SseParser::new();
        let mut out = Vec::new();
        assert!(p.feed_into(b"data: a\n\ndata: \xFF\n", &mut out).is_err());
        assert_eq!(out, vec![SseItem::Event(SseEvent::new(None, "a"))]);
    }

    #[test]
    fn a_stream_cut_mid_line_is_truncated() {
        assert_eq!(
            parse_all(b"data: {\"type\":\"cont").unwrap_err(),
            SseError::Truncated
        );
    }

    #[test]
    fn writer_cannot_be_tricked_into_extra_fields() {
        let mut out = Vec::new();
        write_event(&mut out, Some("x\nevent: y"), "a\rb\r\nc");
        let items = parse_all(&out).unwrap();
        let evs = events(&items);
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].event.as_deref(), Some("xevent: y"));
        assert_eq!(evs[0].data, "a\nb\nc");
    }

    #[test]
    fn blank_lines_between_frames_produce_nothing() {
        let items = parse_all(b"\n\n\ndata: a\n\n\n\n").unwrap();
        assert_eq!(items, vec![SseItem::Event(SseEvent::new(None, "a"))]);
    }

    #[test]
    fn crlf_split_across_chunks_is_one_line_ending() {
        let mut p = SseParser::new();
        let mut items = p.feed(b"data: a\r").unwrap();
        items.extend(p.feed(b"\n\r").unwrap());
        items.extend(p.feed(b"\ndata: b\n\n").unwrap());
        items.extend(p.finish().unwrap());
        assert_eq!(
            items,
            vec![
                SseItem::Event(SseEvent::new(None, "a")),
                SseItem::Event(SseEvent::new(None, "b"))
            ]
        );
    }

    #[test]
    fn rejects_oversized_lines_and_invalid_utf8() {
        let mut p = SseParser::with_limit(8);
        assert_eq!(
            p.feed(b"data: 0123456789").unwrap_err(),
            SseError::TooLarge { limit: 8 }
        );
        assert_eq!(
            parse_all(b"data: \xFF\n\n").unwrap_err(),
            SseError::InvalidUtf8
        );
        let mut p = SseParser::with_limit(12);
        p.feed(b"data: 12345\n").unwrap();
        p.feed(b": a comment\n").unwrap();
        assert!(p.feed(b"data: 12345\n").is_err(), "frame limit spans lines");
    }

    #[test]
    fn writer_output_parses_back() {
        let mut out = Vec::new();
        write_event(&mut out, Some("content_block_delta"), "{\"x\":1}");
        write_event(&mut out, None, "a\nb");
        write_item(&mut out, &SseItem::Comment("ping".into()));
        let ev = SseEvent {
            event: Some("e".into()),
            data: "d".into(),
            id: Some("1".into()),
            retry: Some(10),
            other: vec![("x".into(), "y".into())],
            has_data: true,
        };
        write_item(&mut out, &SseItem::Event(ev.clone()));
        assert_eq!(
            String::from_utf8(out.clone()).unwrap(),
            "event: content_block_delta\ndata: {\"x\":1}\n\ndata: a\ndata: b\n\n: ping\nevent: e\nid: 1\nretry: 10\nx: y\ndata: d\n\n"
        );
        let items = parse_all(&out).unwrap();
        assert_eq!(items.len(), 4);
        assert_eq!(items[3], SseItem::Event(ev));
    }

    fn arb_stream() -> impl Strategy<Value = Vec<u8>> {
        let line = prop_oneof![
            "[a-z{}\":, ]{0,12}".prop_map(|s| format!("data: {s}")),
            "[a-z{} ]{0,6}".prop_map(|s| format!("data:{s}")),
            Just("data".to_owned()),
            "[a-z_.]{1,10}".prop_map(|s| format!("event: {s}")),
            "[a-z0-9 ]{0,8}".prop_map(|s| format!("id: {s}")),
            "[0-9]{1,6}|[0-9a-z]{0,3}".prop_map(|s| format!("retry: {s}")),
            "[a-z]{1,6}: [a-z ]{0,6}",
            "[a-z ]{0,8}".prop_map(|s| format!(":{s}")),
            Just(String::new()),
            Just(String::new()),
            Just("data: é✓".to_owned()),
        ];
        let ending = prop_oneof![Just("\n"), Just("\r\n"), Just("\r")];
        proptest::collection::vec((line, ending), 0..30).prop_map(|lines| {
            let mut s = String::new();
            for (l, e) in lines {
                s.push_str(&l);
                s.push_str(e);
            }
            s.into_bytes()
        })
    }

    proptest! {
        #[test]
        fn arbitrary_chunk_splits_yield_the_same_items(
            stream in arb_stream(),
            cuts in proptest::collection::vec(any::<prop::sample::Index>(), 0..8),
            bom in any::<bool>(),
        ) {
            let mut stream = stream;
            if bom {
                let mut with_bom = vec![0xEF, 0xBB, 0xBF];
                with_bom.extend(stream);
                stream = with_bom;
            }
            let whole = parse_all(&stream).unwrap();
            let mut points: Vec<usize> = cuts.iter().map(|c| c.index(stream.len() + 1)).collect();
            points.sort_unstable();
            let mut parser = SseParser::new();
            let mut pieces = Vec::new();
            let mut last = 0;
            for p in points {
                pieces.extend(parser.feed(&stream[last..p]).unwrap());
                last = p;
            }
            pieces.extend(parser.feed(&stream[last..]).unwrap());
            pieces.extend(parser.finish().unwrap());
            prop_assert_eq!(&whole, &pieces);
            let mut written = Vec::new();
            for item in &whole {
                write_item(&mut written, item);
            }
            prop_assert_eq!(parse_all(&written).unwrap(), whole);
        }

        #[test]
        fn written_events_parse_back(
            name in proptest::option::of("[a-z_.]{1,12}"),
            data in "[a-z0-9{}\":,\\n ]{0,40}",
        ) {
            let mut out = Vec::new();
            write_event(&mut out, name.as_deref(), &data);
            let items = parse_all(&out).unwrap();
            prop_assert_eq!(items, vec![SseItem::Event(SseEvent::new(name.as_deref(), data))]);
        }
    }
}
