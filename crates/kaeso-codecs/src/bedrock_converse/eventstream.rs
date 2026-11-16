//! AWS's binary event stream encoding (`application/vnd.amazon.eventstream`), which Bedrock's
//! `ConverseStream` answers in: messages of a 12-byte prelude (total length, headers length,
//! the prelude's CRC32), headers, a payload and the message's CRC32
//! (<https://smithy.io/2.0/aws/amazon-eventstream.html>).

/// One decoded message: its `:message-type`, `:event-type`, `:exception-type`,
/// `:error-code` and `:error-message` headers, and its payload.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Message {
    pub message_type: Option<String>,
    pub event_type: Option<String>,
    pub exception_type: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub payload: Vec<u8>,
}

/// Why the bytes are no event stream; the stream can't go on after it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameError(pub String);

const PRELUDE: usize = 12;
const MIN_MESSAGE: usize = PRELUDE + 4;
// Clients must not enforce the services' size limits, but a length beyond them is no message
// Bedrock sends: 24 MiB of payload and 128 KiB of headers (the specification's limits).
const MAX_MESSAGE: usize = MIN_MESSAGE + 128 * 1024 + 24 * 1024 * 1024;

/// Takes bytes as they arrive and hands out each whole message.
#[derive(Debug, Default)]
pub struct Framer {
    buf: Vec<u8>,
}

impl Framer {
    pub fn new() -> Self {
        Framer::default()
    }

    /// The whole messages `bytes` completes, in order, or the first malformed one.
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<Message>, FrameError> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        loop {
            let Some(prelude) = self.buf.get(..PRELUDE) else {
                return Ok(out);
            };
            let total = be32(prelude, 0) as usize;
            let headers = be32(prelude, 4) as usize;
            if crc32(prelude.get(..8).unwrap_or_default()) != be32(prelude, 8) {
                return Err(FrameError(
                    "a message's prelude checksum doesn't match".into(),
                ));
            }
            if !(MIN_MESSAGE..=MAX_MESSAGE).contains(&total) || headers > total - MIN_MESSAGE {
                return Err(FrameError(format!(
                    "a message says it is {total} bytes long with {headers} bytes of headers"
                )));
            }
            let Some(message) = self.buf.get(..total) else {
                return Ok(out);
            };
            let (body, crc) = message.split_at(total - 4);
            if crc32(body) != be32(crc, 0) {
                return Err(FrameError("a message's checksum doesn't match".into()));
            }
            let header_bytes = body.get(PRELUDE..PRELUDE + headers).unwrap_or_default();
            let mut m = read_headers(header_bytes)?;
            m.payload = body.get(PRELUDE + headers..).unwrap_or_default().to_vec();
            out.push(m);
            self.buf.drain(..total);
        }
    }

    /// Whether bytes of an unfinished message are left.
    pub fn has_partial(&self) -> bool {
        !self.buf.is_empty()
    }
}

fn be32(bytes: &[u8], at: usize) -> u32 {
    let mut b = [0_u8; 4];
    for (i, slot) in b.iter_mut().enumerate() {
        *slot = bytes.get(at + i).copied().unwrap_or_default();
    }
    u32::from_be_bytes(b)
}

fn read_headers(mut bytes: &[u8]) -> Result<Message, FrameError> {
    let bad = |what: &str| FrameError(format!("a message's headers are malformed: {what}"));
    let mut m = Message::default();
    while let Some((&name_len, rest)) = bytes.split_first() {
        let (name, rest) = split(rest, usize::from(name_len)).ok_or_else(|| bad("name"))?;
        let name = std::str::from_utf8(name).map_err(|_| bad("name"))?;
        let (&kind, rest) = rest.split_first().ok_or_else(|| bad("type"))?;
        let fixed = match kind {
            0 | 1 => Some(0),
            2 => Some(1),
            3 => Some(2),
            4 => Some(4),
            5 | 8 => Some(8),
            9 => Some(16),
            6 | 7 => None,
            _ => return Err(bad("type")),
        };
        let (value, rest) = match fixed {
            Some(n) => split(rest, n).ok_or_else(|| bad("value"))?,
            None => {
                let (len, rest) = split(rest, 2).ok_or_else(|| bad("value length"))?;
                let len = usize::from(u16::from_be_bytes([
                    len.first().copied().unwrap_or_default(),
                    len.get(1).copied().unwrap_or_default(),
                ]));
                split(rest, len).ok_or_else(|| bad("value"))?
            }
        };
        if kind == 7 {
            let text = std::str::from_utf8(value)
                .map_err(|_| bad("value"))?
                .to_owned();
            match name {
                ":message-type" => m.message_type = Some(text),
                ":event-type" => m.event_type = Some(text),
                ":exception-type" => m.exception_type = Some(text),
                ":error-code" => m.error_code = Some(text),
                ":error-message" => m.error_message = Some(text),
                _ => {}
            }
        }
        bytes = rest;
    }
    Ok(m)
}

fn split(bytes: &[u8], n: usize) -> Option<(&[u8], &[u8])> {
    (bytes.len() >= n).then(|| bytes.split_at(n))
}

// CRC-32 (IEEE 802.3, reflected, polynomial 0xEDB88320), as gzip computes it.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0_u32;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// One message in the encoding, for tests and the mock provider: string headers and a payload.
pub fn encode(headers: &[(&str, &str)], payload: &[u8]) -> Vec<u8> {
    let mut h = Vec::new();
    for (name, value) in headers {
        h.push(u8::try_from(name.len()).unwrap_or(u8::MAX));
        h.extend_from_slice(name.as_bytes());
        h.push(7);
        h.extend_from_slice(&u16::try_from(value.len()).unwrap_or(u16::MAX).to_be_bytes());
        h.extend_from_slice(value.as_bytes());
    }
    let total = u32::try_from(MIN_MESSAGE + h.len() + payload.len()).unwrap_or(u32::MAX);
    let mut out = Vec::with_capacity(total as usize);
    out.extend_from_slice(&total.to_be_bytes());
    out.extend_from_slice(&u32::try_from(h.len()).unwrap_or(u32::MAX).to_be_bytes());
    out.extend_from_slice(&crc32(&out).to_be_bytes());
    out.extend_from_slice(&h);
    out.extend_from_slice(payload);
    out.extend_from_slice(&crc32(&out).to_be_bytes());
    out
}

/// An event message: `:message-type: event`, its `:event-type` and a JSON payload.
pub fn event(event_type: &str, payload: &[u8]) -> Vec<u8> {
    encode(
        &[
            (":message-type", "event"),
            (":event-type", event_type),
            (":content-type", "application/json"),
        ],
        payload,
    )
}

/// A stream fixture as Bedrock sends it: each `{"event": …, "data": …}` or
/// `{"exception": …, "data": …}` of the JSON list as one message.
pub fn from_fixture(json: &[u8]) -> Result<Vec<u8>, serde_json::Error> {
    let entries: Vec<serde_json::Value> = serde_json::from_slice(json)?;
    let mut out = Vec::new();
    for e in entries {
        let data = e.get("data").map(ToString::to_string).unwrap_or_default();
        match e.get("exception").and_then(serde_json::Value::as_str) {
            Some(ty) => out.extend(encode(
                &[(":message-type", "exception"), (":exception-type", ty)],
                data.as_bytes(),
            )),
            None => out.extend(event(
                e.get("event")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default(),
                data.as_bytes(),
            )),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_is_gzips() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn messages_are_read_whole_whatever_the_chunks() {
        let mut bytes = event("messageStart", br#"{"role":"assistant"}"#);
        bytes.extend(encode(
            &[
                (":message-type", "exception"),
                (":exception-type", "throttlingException"),
            ],
            br#"{"message":"slow down"}"#,
        ));
        for chunk in [1, 3, 7, bytes.len()] {
            let mut f = Framer::new();
            let mut got = Vec::new();
            for c in bytes.chunks(chunk) {
                got.extend(f.feed(c).unwrap());
            }
            assert!(!f.has_partial());
            assert_eq!(got.len(), 2, "chunks of {chunk}");
            assert_eq!(got[0].event_type.as_deref(), Some("messageStart"));
            assert_eq!(got[0].payload, br#"{"role":"assistant"}"#);
            assert_eq!(got[1].message_type.as_deref(), Some("exception"));
            assert_eq!(
                got[1].exception_type.as_deref(),
                Some("throttlingException")
            );
        }
    }

    #[test]
    fn every_header_type_is_read_past_and_bad_frames_are_refused() {
        let mut h = Vec::new();
        for (kind, value) in [
            (0_u8, vec![]),
            (1, vec![]),
            (2, vec![1]),
            (3, vec![0, 1]),
            (4, vec![0; 4]),
            (5, vec![0; 8]),
            (6, vec![0, 2, 9, 9]),
            (8, vec![0; 8]),
            (9, vec![0; 16]),
        ] {
            h.push(1);
            h.push(b'x');
            h.push(kind);
            h.extend(value);
        }
        h.push(11);
        h.extend(b":event-type");
        h.extend([7, 0, 4]);
        h.extend(b"ping");
        let m = read_headers(&h).unwrap();
        assert_eq!(m.event_type.as_deref(), Some("ping"));
        assert!(read_headers(&[1, b'x', 42]).is_err());
        assert!(read_headers(&[5, b'x']).is_err());
        assert!(read_headers(&[1, b'x', 7, 0, 9, b'a']).is_err());
        assert!(read_headers(&[1, b'x', 7, 0]).is_err());
        assert!(read_headers(&[1, 0xff, 7, 0, 0]).is_err());
        assert!(read_headers(&[1, b'x', 7, 0, 1, 0xff]).is_err());
        assert!(read_headers(&[1, b'x']).is_err());

        let good = event("messageStop", b"{}");
        let mut corrupt = good.clone();
        let last = corrupt.len() - 5;
        corrupt[last] ^= 1;
        assert!(
            Framer::new()
                .feed(&corrupt)
                .unwrap_err()
                .0
                .contains("checksum")
        );
        let mut prelude = good.clone();
        prelude[9] ^= 1;
        assert!(
            Framer::new()
                .feed(&prelude)
                .unwrap_err()
                .0
                .contains("prelude")
        );
        let mut short = encode(&[], b"");
        short[3] = 4;
        let crc = crc32(&short[..8]).to_be_bytes();
        short[8..12].copy_from_slice(&crc);
        assert!(
            Framer::new()
                .feed(&short)
                .unwrap_err()
                .0
                .contains("4 bytes long")
        );
        let mut f = Framer::new();
        assert!(f.feed(&good[..5]).unwrap().is_empty());
        assert!(f.has_partial());
    }
}
