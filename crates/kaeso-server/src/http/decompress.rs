use std::io::Read;

use axum::http::{HeaderMap, header};
use bytes::Bytes;
use ruzstd::decoding::errors::{FrameDecoderError, ReadFrameHeaderError};
use ruzstd::decoding::{BlockDecodingStrategy, FrameDecoder};

// RFC 9659 §3: zstd content coding must not need a window above 8 MiB, and a decoder may refuse
// one; ruzstd reserves the declared window up front, so a larger one would be a cheap allocation.
const ZSTD_WINDOW: u64 = 8 << 20;
const CHUNK: usize = 64 << 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Identity,
    Gzip,
    Zstd,
}

impl Encoding {
    pub fn name(self) -> &'static str {
        match self {
            Encoding::Identity => "identity",
            Encoding::Gzip => "gzip",
            Encoding::Zstd => "zstd",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    Unsupported(String),
    Stacked(String),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Failed {
    TooLarge,
    Corrupt,
}

pub fn encoding(headers: &HeaderMap) -> Result<Encoding, Refused> {
    let mut codings = Vec::new();
    for value in headers.get_all(header::CONTENT_ENCODING) {
        let Ok(value) = value.to_str() else {
            return Err(Refused::Unsupported(
                String::from_utf8_lossy(value.as_bytes()).into_owned(),
            ));
        };
        codings.extend(
            value
                .split(',')
                .map(|c| c.trim().to_ascii_lowercase())
                .filter(|c| !c.is_empty() && c != "identity"),
        );
    }
    match codings.as_slice() {
        [] => Ok(Encoding::Identity),
        [one] => match one.as_str() {
            "gzip" | "x-gzip" => Ok(Encoding::Gzip),
            "zstd" => Ok(Encoding::Zstd),
            _ => Err(Refused::Unsupported(one.clone())),
        },
        many => Err(Refused::Stacked(many.join(", "))),
    }
}

pub fn decode(encoding: Encoding, raw: Bytes, limit: usize) -> Result<Bytes, Failed> {
    if encoding == Encoding::Identity || raw.is_empty() {
        return Ok(raw);
    }
    let mut out = Output::new(raw.len(), limit);
    match encoding {
        Encoding::Identity => {}
        Encoding::Gzip => gzip(&raw, &mut out)?,
        Encoding::Zstd => zstd(&raw, &mut out)?,
    }
    Ok(Bytes::from(out.buf))
}

// The buffer grows by doubling but never past the limit, so a body that decodes to more than
// the limit is refused before the limit is allocated, and one under it never holds it twice.
struct Output {
    buf: Vec<u8>,
    limit: usize,
}

impl Output {
    fn new(raw: usize, limit: usize) -> Self {
        Output {
            buf: Vec::with_capacity(raw.saturating_mul(4).min(limit)),
            limit,
        }
    }

    fn push(&mut self, data: &[u8]) -> Result<(), Failed> {
        let len = self.buf.len().saturating_add(data.len());
        if len > self.limit {
            return Err(Failed::TooLarge);
        }
        if len > self.buf.capacity() {
            let target = len
                .max(self.buf.capacity().saturating_mul(2))
                .min(self.limit);
            self.buf.reserve_exact(target - self.buf.len());
        }
        self.buf.extend_from_slice(data);
        Ok(())
    }
}

fn gzip(raw: &[u8], out: &mut Output) -> Result<(), Failed> {
    let mut decoder = flate2::read::MultiGzDecoder::new(raw);
    let mut chunk = vec![0; CHUNK];
    loop {
        let n = decoder.read(&mut chunk).map_err(|_| Failed::Corrupt)?;
        if n == 0 {
            return Ok(());
        }
        out.push(chunk.get(..n).unwrap_or_default())?;
    }
}

fn zstd(mut input: &[u8], out: &mut Output) -> Result<(), Failed> {
    let mut frame = FrameDecoder::new();
    frame.set_max_window_size(ZSTD_WINDOW);
    let mut chunk = vec![0; CHUNK];
    let mut frames = 0usize;
    while !input.is_empty() {
        match frame.init(&mut input) {
            Ok(()) => {}
            Err(FrameDecoderError::ReadFrameHeaderError(ReadFrameHeaderError::SkipFrame {
                length,
                ..
            })) => {
                input = usize::try_from(length)
                    .ok()
                    .and_then(|l| input.get(l..))
                    .ok_or(Failed::Corrupt)?;
                continue;
            }
            Err(_) => return Err(Failed::Corrupt),
        }
        frames += 1;
        loop {
            frame
                .decode_blocks(&mut input, BlockDecodingStrategy::UptoBlocks(1))
                .map_err(|_| Failed::Corrupt)?;
            while frame.can_collect() > 0 {
                let n = frame.read(&mut chunk).map_err(|_| Failed::Corrupt)?;
                if n == 0 {
                    break;
                }
                out.push(chunk.get(..n).unwrap_or_default())?;
            }
            if frame.is_finished() {
                break;
            }
        }
        if let (Some(sent), Some(computed)) = (
            frame.get_checksum_from_data(),
            frame.get_calculated_checksum(),
        ) && sent != computed
        {
            return Err(Failed::Corrupt);
        }
    }
    if frames == 0 {
        return Err(Failed::Corrupt);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use axum::http::HeaderValue;

    use super::*;

    fn headers(values: &[&'static str]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for v in values {
            h.append(header::CONTENT_ENCODING, HeaderValue::from_static(v));
        }
        h
    }

    fn gzipped(data: &[u8]) -> Vec<u8> {
        let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(data).unwrap();
        e.finish().unwrap()
    }

    fn zstded(data: &[u8]) -> Vec<u8> {
        ruzstd::encoding::compress_to_vec(data, ruzstd::encoding::CompressionLevel::Fastest)
    }

    #[test]
    fn one_known_coding_is_read_and_identity_or_none_is_left_alone() {
        assert_eq!(encoding(&headers(&[])), Ok(Encoding::Identity));
        assert_eq!(encoding(&headers(&["identity"])), Ok(Encoding::Identity));
        assert_eq!(encoding(&headers(&["gzip"])), Ok(Encoding::Gzip));
        assert_eq!(encoding(&headers(&["X-Gzip"])), Ok(Encoding::Gzip));
        assert_eq!(encoding(&headers(&[" zstd "])), Ok(Encoding::Zstd));
        assert_eq!(encoding(&headers(&["identity, zstd"])), Ok(Encoding::Zstd));
    }

    #[test]
    fn other_and_stacked_codings_are_refused() {
        assert_eq!(
            encoding(&headers(&["br"])),
            Err(Refused::Unsupported("br".into()))
        );
        assert_eq!(
            encoding(&headers(&["deflate"])),
            Err(Refused::Unsupported("deflate".into()))
        );
        assert_eq!(
            encoding(&headers(&["gzip, zstd"])),
            Err(Refused::Stacked("gzip, zstd".into()))
        );
        assert_eq!(
            encoding(&headers(&["gzip", "gzip"])),
            Err(Refused::Stacked("gzip, gzip".into()))
        );
    }

    #[test]
    fn gzip_and_zstd_bodies_decode_to_what_was_sent() {
        let body = br#"{"model":"gpt-4o","messages":[]}"#.repeat(100);
        for (encoding, raw) in [
            (Encoding::Gzip, gzipped(&body)),
            (Encoding::Zstd, zstded(&body)),
        ] {
            let decoded = decode(encoding, Bytes::from(raw), body.len()).unwrap();
            assert_eq!(decoded.as_ref(), body.as_slice(), "{encoding:?}");
        }
        let raw = Bytes::from_static(b"as sent");
        assert_eq!(decode(Encoding::Identity, raw.clone(), 1).unwrap(), raw);
    }

    #[test]
    fn concatenated_members_and_frames_and_skippable_frames_are_read() {
        let mut two = gzipped(b"ab");
        two.extend(gzipped(b"cd"));
        assert_eq!(
            decode(Encoding::Gzip, Bytes::from(two), 10)
                .unwrap()
                .as_ref(),
            b"abcd"
        );
        let mut frames = vec![0x50, 0x2a, 0x4d, 0x18, 2, 0, 0, 0, 9, 9];
        frames.extend(zstded(b"ab"));
        frames.extend(zstded(b"cd"));
        assert_eq!(
            decode(Encoding::Zstd, Bytes::from(frames), 10)
                .unwrap()
                .as_ref(),
            b"abcd"
        );
    }

    #[test]
    fn a_bomb_stops_at_the_limit_without_holding_more() {
        let limit = 1 << 20;
        let zeros = vec![0u8; 16 << 20];
        for (encoding, raw) in [
            (Encoding::Gzip, gzipped(&zeros)),
            (Encoding::Zstd, zstded(&zeros)),
        ] {
            assert!(raw.len() < 64 << 10, "{encoding:?}: {} bytes", raw.len());
            assert_eq!(
                decode(encoding, Bytes::from(raw), limit),
                Err(Failed::TooLarge)
            );
        }
        let mut out = Output::new(10, limit);
        for _ in 0..16 {
            out.push(&[0; 64 << 10]).unwrap();
        }
        assert_eq!(out.buf.capacity(), limit);
        assert_eq!(out.push(&[0]), Err(Failed::TooLarge));
    }

    #[test]
    fn a_body_exactly_at_the_limit_is_taken() {
        let body = vec![b'a'; 4096];
        let decoded = decode(Encoding::Zstd, Bytes::from(zstded(&body)), 4096).unwrap();
        assert_eq!(decoded.len(), 4096);
        assert_eq!(
            decode(Encoding::Gzip, Bytes::from(gzipped(&body)), 4095),
            Err(Failed::TooLarge)
        );
    }

    #[test]
    fn corrupt_truncated_and_frameless_bodies_are_refused() {
        let body = b"hello hello hello".repeat(20);
        for (encoding, raw) in [
            (Encoding::Gzip, gzipped(&body)),
            (Encoding::Zstd, zstded(&body)),
        ] {
            let cut = raw.get(..raw.len() - 3).unwrap().to_vec();
            assert_eq!(
                decode(encoding, Bytes::from(cut), 1 << 20),
                Err(Failed::Corrupt),
                "{encoding:?}"
            );
            assert_eq!(
                decode(encoding, Bytes::from_static(b"{\"plain\":true}"), 1 << 20),
                Err(Failed::Corrupt),
                "{encoding:?}"
            );
        }
        assert_eq!(
            decode(
                Encoding::Zstd,
                Bytes::from_static(&[0x50, 0x2a, 0x4d, 0x18, 0, 0, 0, 0]),
                10
            ),
            Err(Failed::Corrupt)
        );
    }

    #[test]
    fn a_zstd_frame_that_asks_for_a_huge_window_is_refused_before_it_is_allocated() {
        let mut frame = vec![0x28, 0xb5, 0x2f, 0xfd, 0x00];
        frame.push(0xd8);
        frame.extend([1, 0, 0]);
        assert_eq!(
            decode(Encoding::Zstd, Bytes::from(frame), 1 << 20),
            Err(Failed::Corrupt)
        );
    }
}
