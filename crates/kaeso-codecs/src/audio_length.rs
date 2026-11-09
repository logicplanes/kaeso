//! How long an audio file lasts, read from its container's headers without decoding it.
//!
//! Providers that bill transcription or voice changing by the length of the audio don't always
//! say how long it was. [`audio_duration`] reads the length the file states about itself: WAV's
//! `fmt ` and `data` chunks, FLAC's `STREAMINFO`, an MP3's Xing, Info or VBRI frame or, for
//! constant bitrate, its first frame and its size, the granule position of an Ogg Opus or Vorbis
//! file's last page, MP4's `mvhd` or `mdhd`, and Matroska's or WebM's segment `Duration`. It
//! never decodes audio, reads at most [`MAX_SCAN`] bytes from the start and [`MAX_TAIL`] from
//! the end, and answers `None` when the file doesn't state its length or isn't one it knows.

use std::time::Duration;

/// The most bytes read from the start of a file while looking for its headers.
pub const MAX_SCAN: usize = 1 << 20;

/// The most bytes read from the end of an Ogg file while looking for its last page.
pub const MAX_TAIL: usize = 64 * 1024;

// Longer than any provider takes in one request (OpenAI 25 MB, ElevenLabs 3 GB of audio at
// most 10 hours, QwenCloud 12 hours); a stated length beyond it is a broken header.
const MAX_SECONDS: f64 = 24.0 * 3600.0;

/// The length `bytes` states, or `None` when it states none or is no format this module reads.
pub fn audio_duration(bytes: &[u8]) -> Option<Duration> {
    let seconds = if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WAVE") {
        wav(bytes)
    } else if bytes.starts_with(b"fLaC") {
        flac(bytes)
    } else if bytes.starts_with(b"OggS") {
        ogg(bytes)
    } else if bytes.get(4..8) == Some(b"ftyp") {
        mp4(bytes)
    } else if bytes.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        matroska(bytes)
    } else {
        mp3(bytes)
    }?;
    // Under a millisecond is no audio a provider bills, and rounds to no length at all.
    (seconds.is_finite() && seconds >= 0.001 && seconds <= MAX_SECONDS)
        .then(|| Duration::from_secs_f64(seconds))
}

fn u16_le(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        b.get(at..at.checked_add(2)?)?.try_into().ok()?,
    ))
}

fn u32_le(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        b.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn u64_le(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        b.get(at..at.checked_add(8)?)?.try_into().ok()?,
    ))
}

fn u32_be(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(
        b.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn u64_be(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_be_bytes(
        b.get(at..at.checked_add(8)?)?.try_into().ok()?,
    ))
}

// RIFF/WAVE: the `fmt ` chunk's average bytes per second and the `data` chunk's size. A streamed
// file may state no size (0 or 0xFFFFFFFF); then the data runs to the end of the file.
fn wav(b: &[u8]) -> Option<f64> {
    let mut at = 12usize;
    let mut byte_rate = None;
    while at.saturating_add(8) <= b.len().min(MAX_SCAN) {
        let id = b.get(at..at.checked_add(4)?)?;
        let size = u32_le(b, at + 4)?;
        let body = at + 8;
        match id {
            b"fmt " => byte_rate = Some(u32_le(b, body + 8)?),
            b"data" => {
                let rate = f64::from(byte_rate.filter(|r| *r > 0)?);
                let left = b.len().saturating_sub(body);
                let stated = usize::try_from(size).unwrap_or(usize::MAX);
                let data = if size == 0 || size == u32::MAX {
                    left
                } else {
                    stated.min(left)
                };
                return Some(data as f64 / rate);
            }
            _ => {}
        }
        // RIFF chunks are padded to an even size.
        let padded = usize::try_from(size).ok()?.checked_add(size as usize & 1)?;
        at = body.checked_add(padded)?;
    }
    None
}

// FLAC: the first metadata block is STREAMINFO (RFC 9639 section 8.2); bytes 10..18 of its body
// hold the sample rate (20 bits), channels (3), bits per sample (5) and total samples (36).
fn flac(b: &[u8]) -> Option<f64> {
    if b.get(4)? & 0x7F != 0 {
        return None;
    }
    let packed = u64::from_be_bytes(b.get(18..26)?.try_into().ok()?);
    let rate = (packed >> 44) & 0xF_FFFF;
    let samples = packed & 0xF_FFFF_FFFF;
    (rate > 0 && samples > 0).then(|| samples as f64 / rate as f64)
}

// Ogg (RFC 3533): the codec is named by the first page's packet, and the length is the granule
// position of the last page of the same stream, found in the file's last bytes.
fn ogg(b: &[u8]) -> Option<f64> {
    let serial = u32_le(b, 14)?;
    let segments = usize::from(*b.get(26)?);
    let packet = b.get(27 + segments..)?;
    let (rate, skip) = if packet.starts_with(b"OpusHead") {
        // RFC 7845 section 4: granule positions count 48 kHz samples, less the pre-skip.
        (48_000u64, u64::from(u16_le(packet, 10)?))
    } else if packet.starts_with(b"\x01vorbis") {
        (u64::from(u32_le(packet, 12)?), 0)
    } else {
        return None;
    };
    let tail_start = b.len().saturating_sub(MAX_TAIL);
    let tail = b.get(tail_start..)?;
    let mut end = tail.len();
    while let Some(at) = tail.get(..end)?.windows(4).rposition(|w| w == b"OggS") {
        end = at;
        if tail.get(at + 4) != Some(&0) || u32_le(tail, at + 14) != Some(serial) {
            continue;
        }
        let granule = u64_le(tail, at + 6)?;
        if granule == u64::MAX {
            continue;
        }
        return (rate > 0 && granule > skip).then(|| (granule - skip) as f64 / rate as f64);
    }
    None
}

// ISO BMFF (MP4, M4A, MOV): the movie header's duration in its time scale (ISO/IEC 14496-12
// section 8.2.2), or the first media header's (8.4.2) when the movie header states none. The
// boxes are walked by their sizes, so the media data in between is never read.
fn mp4(b: &[u8]) -> Option<f64> {
    let moov = find_box(b, b"moov")?;
    let from_header = |name: &[u8; 4], within: &[u8]| -> Option<f64> {
        let h = find_box(within, name)?;
        let (scale, duration) = match h.first()? {
            0 => (u32_be(h, 12)?, u64::from(u32_be(h, 16)?)),
            1 => (u32_be(h, 20)?, u64_be(h, 24)?),
            _ => return None,
        };
        (scale > 0 && duration > 0 && duration != u64::from(u32::MAX) && duration != u64::MAX)
            .then(|| duration as f64 / f64::from(scale))
    };
    from_header(b"mvhd", moov).or_else(|| {
        let trak = find_box(moov, b"trak")?;
        let mdia = find_box(trak, b"mdia")?;
        from_header(b"mdhd", mdia)
    })
}

// The body of the first box called `name` among the boxes that fill `b`.
fn find_box<'a>(b: &'a [u8], name: &[u8; 4]) -> Option<&'a [u8]> {
    let mut at = 0usize;
    for _ in 0..4096 {
        if at >= b.len() {
            return None;
        }
        let size = u64::from(u32_be(b, at)?);
        let kind = b.get(at + 4..at + 8)?;
        let (header, size) = match size {
            0 => (8, (b.len() - at) as u64),
            1 => (16, u64_be(b, at + 8)?),
            n => (8, n),
        };
        let size = usize::try_from(size).ok()?;
        if size < header {
            return None;
        }
        let end = at.checked_add(size)?;
        if kind == name {
            return b.get(at + header..end.min(b.len()));
        }
        at = end;
    }
    None
}

// Matroska and WebM (RFC 9559): the segment's Info element holds Duration as a float in units
// of TimestampScale nanoseconds (1 ms when absent). Recorders that write while they record,
// such as browsers' MediaRecorder, leave Duration out, and the answer is then `None`.
fn matroska(b: &[u8]) -> Option<f64> {
    const SEGMENT: u64 = 0x1853_8067;
    const INFO: u64 = 0x1549_A966;
    const TIMESTAMP_SCALE: u64 = 0x2A_D7B1;
    const DURATION: u64 = 0x4489;
    let b = b.get(..b.len().min(MAX_SCAN))?;
    let (_, header) = element(b, 0)?;
    let mut at = header.end;
    let segment = loop {
        let (id, e) = element(b, at)?;
        if id == SEGMENT {
            break e;
        }
        at = e.end;
    };
    let mut at = segment.start;
    let info = loop {
        let (id, e) = element(b, at)?;
        if id == INFO {
            break e;
        }
        if e.unknown {
            return None;
        }
        at = e.end;
    };
    let mut at = info.start;
    let mut scale = 1_000_000u64;
    let mut duration = None;
    while at < info.end.min(b.len()) {
        let (id, e) = element(b, at)?;
        let body = b.get(e.start..e.end)?;
        match id {
            TIMESTAMP_SCALE => scale = body.iter().fold(0u64, |n, &x| n << 8 | u64::from(x)),
            DURATION => {
                duration = match body.len() {
                    4 => Some(f64::from(f32::from_be_bytes(body.try_into().ok()?))),
                    8 => Some(f64::from_be_bytes(body.try_into().ok()?)),
                    _ => None,
                };
            }
            _ => {}
        }
        at = e.end;
    }
    Some(duration? * scale as f64 / 1e9)
}

struct Element {
    start: usize,
    end: usize,
    unknown: bool,
}

// One EBML element at `at`: its id and where its body starts and ends.
fn element(b: &[u8], at: usize) -> Option<(u64, Element)> {
    let (id, id_len) = vint(b, at, true)?;
    let (size, size_len) = vint(b, at + id_len, false)?;
    let start = at + id_len + size_len;
    let all_ones = (1u64 << (7 * size_len)) - 1;
    let unknown = size == all_ones;
    let end = if unknown {
        b.len()
    } else {
        start.checked_add(usize::try_from(size).ok()?)?
    };
    Some((
        id,
        Element {
            start,
            end,
            unknown,
        },
    ))
}

// An EBML variable-length integer (RFC 8794 section 4): its length is the count of leading
// zero bits of the first byte plus one. An id keeps its length marker; a size drops it.
fn vint(b: &[u8], at: usize, keep_marker: bool) -> Option<(u64, usize)> {
    let first = *b.get(at)?;
    let len = usize::try_from(first.leading_zeros()).ok()? + 1;
    if len > 8 {
        return None;
    }
    let mut value = if keep_marker {
        u64::from(first)
    } else {
        u64::from(first) & ((1u64 << (8 - len)) - 1)
    };
    for i in 1..len {
        value = value << 8 | u64::from(*b.get(at + i)?);
    }
    Some((value, len))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Frame {
    version: u8,
    layer: u8,
    rate: u32,
    bitrate: u32,
    len: usize,
    mono: bool,
}

impl Frame {
    fn samples(self) -> u32 {
        match (self.layer, self.version) {
            (1, _) => 384,
            (2, _) | (3, 3) => 1152,
            _ => 576,
        }
    }
}

// An MPEG audio frame header (ISO/IEC 11172-3 and 13818-3): version, layer, bitrate and sample
// rate indices, and the frame's length in bytes.
fn frame(b: &[u8], at: usize) -> Option<Frame> {
    let h = u32_be(b, at)?;
    if h >> 21 != 0x7FF {
        return None;
    }
    let version = ((h >> 19) & 3) as u8;
    let layer = 4 - ((h >> 17) & 3) as u8;
    let bitrate_index = ((h >> 12) & 0xF) as usize;
    let rate_index = ((h >> 10) & 3) as usize;
    if version == 1 || layer == 4 || bitrate_index == 0 || bitrate_index == 15 || rate_index == 3 {
        return None;
    }
    const RATES: [[u32; 3]; 4] = [
        [11_025, 12_000, 8_000],
        [0, 0, 0],
        [22_050, 24_000, 16_000],
        [44_100, 48_000, 32_000],
    ];
    const KBPS_V1: [[u32; 15]; 3] = [
        [
            0, 32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448,
        ],
        [
            0, 32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384,
        ],
        [
            0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
        ],
    ];
    const KBPS_V2: [[u32; 15]; 3] = [
        [
            0, 32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256,
        ],
        [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160],
        [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160],
    ];
    let rate = *RATES.get(usize::from(version))?.get(rate_index)?;
    let table = if version == 3 { &KBPS_V1 } else { &KBPS_V2 };
    let kbps = *table.get(usize::from(layer) - 1)?.get(bitrate_index)?;
    let bitrate = kbps * 1000;
    let padding = (h >> 9) & 1;
    let len = match (layer, version) {
        (1, _) => (12 * bitrate / rate + padding) * 4,
        (3, 0 | 2) => 72 * bitrate / rate + padding,
        _ => 144 * bitrate / rate + padding,
    };
    Some(Frame {
        version,
        layer,
        rate,
        bitrate,
        len: usize::try_from(len).ok().filter(|l| *l > 4)?,
        mono: (h >> 6) & 3 == 3,
    })
}

// MP3: past an ID3v2 tag, the first frame. A Xing or Info frame (LAME's) or a VBRI frame
// (Fraunhofer's) states the number of frames; without one, the stream is taken as constant
// bitrate and its length follows from its size, but only when a second frame of the same kind
// follows the first, so arbitrary bytes that happen to look like a header aren't trusted.
fn mp3(b: &[u8]) -> Option<f64> {
    let mut at = 0usize;
    if b.starts_with(b"ID3") {
        let size = b
            .get(6..10)?
            .iter()
            .fold(0usize, |n, &x| n << 7 | usize::from(x & 0x7F));
        let footer = if b.get(5)? & 0x10 != 0 { 10 } else { 0 };
        at = 10 + size + footer;
    }
    let scan_end = b.len().min(at.saturating_add(MAX_SCAN));
    let first = loop {
        if at + 4 > scan_end {
            return None;
        }
        if let Some(f) = frame(b, at) {
            break f;
        }
        at += 1;
    };
    let side = match (first.version == 3, first.mono) {
        (true, false) => 32,
        (true, true) => 17,
        (false, false) => 17,
        (false, true) => 9,
    };
    let xing = at + 4 + side;
    if matches!(b.get(xing..xing + 4), Some(b"Xing" | b"Info")) {
        let flags = u32_be(b, xing + 4)?;
        if flags & 1 == 0 {
            return None;
        }
        let frames = u32_be(b, xing + 8)?;
        let samples = u64::from(frames) * u64::from(first.samples());
        return Some(
            samples.saturating_sub(lame_gap(b, xing, flags)) as f64 / f64::from(first.rate),
        );
    }
    let vbri = at + 4 + 32;
    if b.get(vbri..vbri + 4) == Some(b"VBRI") {
        let frames = u32_be(b, vbri + 14)?;
        return Some(f64::from(frames) * f64::from(first.samples()) / f64::from(first.rate));
    }
    let next = frame(b, at + first.len)?;
    if (next.version, next.layer, next.rate) != (first.version, first.layer, first.rate) {
        return None;
    }
    let mut end = b.len();
    if end >= 128 && b.get(end - 128..end - 125) == Some(b"TAG") {
        end -= 128;
    }
    Some((end - at) as f64 * 8.0 / f64::from(first.bitrate))
}

// LAME's tag after the Xing fields states the encoder delay and padding, 12 bits each, which
// the frames include but the audio doesn't (LAME's "Mp3 Info Tag rev 1" specification).
fn lame_gap(b: &[u8], xing: usize, flags: u32) -> u64 {
    let lame = xing
        + 8
        + [(1, 4), (2, 4), (4, 100), (8, 4)]
            .iter()
            .filter(|(bit, _)| flags & bit != 0)
            .map(|(_, len)| len)
            .sum::<usize>();
    if !matches!(b.get(lame..lame + 4), Some(b"LAME" | b"Lavf" | b"Lavc")) {
        return 0;
    }
    let Some(&[a, m, c]) = b.get(lame + 21..lame + 24) else {
        return 0;
    };
    let delay = u64::from(a) << 4 | u64::from(m >> 4);
    let padding = u64::from(m & 0x0F) << 8 | u64::from(c);
    delay + padding
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(b: &[u8]) -> Option<f64> {
        audio_duration(b).map(|d| (d.as_secs_f64() * 1000.0).round() / 1000.0)
    }

    fn wav_of(rate: u32, channels: u16, bits: u16, samples: u32, stated: Option<u32>) -> Vec<u8> {
        let block = channels * bits / 8;
        let data = samples * u32::from(block);
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + data).to_le_bytes());
        b.extend_from_slice(b"WAVE");
        b.extend_from_slice(b"LIST\x03\0\0\0abc\0");
        b.extend_from_slice(b"fmt \x10\0\0\0\x01\0");
        b.extend_from_slice(&channels.to_le_bytes());
        b.extend_from_slice(&rate.to_le_bytes());
        b.extend_from_slice(&(rate * u32::from(block)).to_le_bytes());
        b.extend_from_slice(&block.to_le_bytes());
        b.extend_from_slice(&bits.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&stated.unwrap_or(data).to_le_bytes());
        b.resize(b.len() + data as usize, 0);
        b
    }

    #[test]
    fn wav_length_is_its_data_over_its_byte_rate() {
        assert_eq!(secs(&wav_of(16_000, 1, 16, 24_000, None)), Some(1.5));
        assert_eq!(secs(&wav_of(48_000, 2, 16, 48_000, None)), Some(1.0));
        assert_eq!(secs(&wav_of(8_000, 1, 8, 4_000, Some(u32::MAX))), Some(0.5));
        assert_eq!(secs(&wav_of(8_000, 1, 8, 4_000, Some(0))), Some(0.5));
        let short = wav_of(8_000, 1, 8, 4_000, Some(1_000_000));
        assert_eq!(secs(&short), Some(0.5), "a size past the end of the file");
        let mut no_rate = wav_of(8_000, 1, 8, 4_000, None);
        no_rate.splice(40..44, [0, 0, 0, 0]);
        assert_eq!(secs(&no_rate), None);
        assert_eq!(secs(b"RIFF\0\0\0\0WAVE"), None);
    }

    #[test]
    fn flac_length_is_streaminfo_samples_over_rate() {
        let mut b = b"fLaC\x00\x00\x00\x22".to_vec();
        b.extend_from_slice(&[0; 10]);
        let packed: u64 = (44_100u64 << 44) | (1 << 41) | (15 << 36) | 88_200;
        b.extend_from_slice(&packed.to_be_bytes());
        b.extend_from_slice(&[0; 16]);
        assert_eq!(secs(&b), Some(2.0));
        let unknown = {
            let mut u = b.clone();
            u.splice(
                18..26,
                ((44_100u64 << 44) | (1 << 41) | (15 << 36)).to_be_bytes(),
            );
            u
        };
        assert_eq!(secs(&unknown), None, "zero total samples means unknown");
        b[4] = 0x04;
        assert_eq!(secs(&b), None, "the first block must be STREAMINFO");
    }

    fn ogg_page(serial: u32, granule: u64, packet: &[u8]) -> Vec<u8> {
        let mut p = b"OggS\0\x02".to_vec();
        p.extend_from_slice(&granule.to_le_bytes());
        p.extend_from_slice(&serial.to_le_bytes());
        p.extend_from_slice(&[0; 8]);
        p.push(1);
        p.push(u8::try_from(packet.len()).unwrap());
        p.extend_from_slice(packet);
        p
    }

    #[test]
    fn ogg_length_is_the_last_granule_of_the_stream() {
        let mut head = b"OpusHead\x01\x01".to_vec();
        head.extend_from_slice(&312u16.to_le_bytes());
        head.extend_from_slice(&48_000u32.to_le_bytes());
        head.extend_from_slice(&[0, 0, 0]);
        let mut opus = ogg_page(7, 0, &head);
        opus.extend(ogg_page(7, 48_000 + 312, b"audio"));
        opus.extend(ogg_page(9, 999_999, b"other stream"));
        assert_eq!(secs(&opus), Some(1.0));

        let mut vorbis_head = b"\x01vorbis\0\0\0\0\x01".to_vec();
        vorbis_head.extend_from_slice(&22_050u32.to_le_bytes());
        let mut vorbis = ogg_page(3, 0, &vorbis_head);
        vorbis.extend(ogg_page(3, 44_100, b"audio"));
        assert_eq!(secs(&vorbis), Some(2.0));

        let flac_in_ogg = ogg_page(3, 0, b"\x7fFLAC");
        assert_eq!(secs(&flac_in_ogg), None);
        let mut unfinished = ogg_page(7, 0, &head);
        unfinished.extend(ogg_page(7, u64::MAX, b"audio"));
        assert_eq!(secs(&unfinished), None);
    }

    fn mp4_box(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut b = u32::try_from(body.len() + 8)
            .unwrap()
            .to_be_bytes()
            .to_vec();
        b.extend_from_slice(kind);
        b.extend_from_slice(body);
        b
    }

    #[test]
    fn mp4_length_is_the_movie_or_media_header_duration() {
        let mut mvhd = vec![0u8; 4];
        mvhd.extend_from_slice(&[0; 8]);
        mvhd.extend_from_slice(&1_000u32.to_be_bytes());
        mvhd.extend_from_slice(&2_500u32.to_be_bytes());
        let mut file = mp4_box(b"ftyp", b"M4A \0\0\0\0");
        file.extend(mp4_box(b"mdat", &[0xAB; 64]));
        file.extend(mp4_box(b"moov", &mp4_box(b"mvhd", &mvhd)));
        assert_eq!(secs(&file), Some(2.5), "moov after mdat");

        let mut v1 = vec![1u8, 0, 0, 0];
        v1.extend_from_slice(&[0; 16]);
        v1.extend_from_slice(&48_000u32.to_be_bytes());
        v1.extend_from_slice(&96_000u64.to_be_bytes());
        let mdia = mp4_box(b"mdia", &mp4_box(b"mdhd", &v1));
        let mut file = mp4_box(b"ftyp", b"isom\0\0\0\0");
        file.extend(mp4_box(b"moov", &mp4_box(b"trak", &mdia)));
        assert_eq!(
            secs(&file),
            Some(2.0),
            "the media header when there's no movie header"
        );

        let mut bad = mp4_box(b"ftyp", b"isom\0\0\0\0");
        bad.extend_from_slice(&4u32.to_be_bytes());
        bad.extend_from_slice(b"moov");
        assert_eq!(secs(&bad), None);
    }

    fn ebml(id: &[u8], body: &[u8]) -> Vec<u8> {
        let mut b = id.to_vec();
        b.push(0x80 | u8::try_from(body.len()).unwrap());
        b.extend_from_slice(body);
        b
    }

    #[test]
    fn matroska_length_is_the_segment_duration_in_its_scale() {
        let mut info = ebml(&[0x2A, 0xD7, 0xB1], &[0x0F, 0x42, 0x40]);
        info.extend(ebml(&[0x44, 0x89], &1500.0f64.to_be_bytes()));
        let mut segment = ebml(&[0x11, 0x4D, 0x9B, 0x74], &[0xEC, 0x81, 0x00]);
        segment.extend(ebml(&[0x15, 0x49, 0xA9, 0x66], &info));
        let mut file = ebml(&[0x1A, 0x45, 0xDF, 0xA3], b"\x42\x82\x84webm");
        file.extend([
            0x18, 0x53, 0x80, 0x67, 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        ]);
        file.extend(&segment);
        assert_eq!(secs(&file), Some(1.5));

        let float = ebml(&[0x44, 0x89], &2000.0f32.to_be_bytes());
        let mut file = ebml(&[0x1A, 0x45, 0xDF, 0xA3], b"");
        file.extend(ebml(
            &[0x18, 0x53, 0x80, 0x67],
            &ebml(&[0x15, 0x49, 0xA9, 0x66], &float),
        ));
        assert_eq!(
            secs(&file),
            Some(2.0),
            "default scale of 1 ms, a 4-byte float"
        );

        let recorder = ebml(&[0x15, 0x49, 0xA9, 0x66], &ebml(&[0x2A, 0xD7, 0xB1], &[1]));
        let mut file = ebml(&[0x1A, 0x45, 0xDF, 0xA3], b"");
        file.extend(ebml(&[0x18, 0x53, 0x80, 0x67], &recorder));
        assert_eq!(
            secs(&file),
            None,
            "no Duration, as browsers' MediaRecorder writes"
        );

        let tiny = ebml(&[0x44, 0x89], &1e-9f64.to_be_bytes());
        let mut file = ebml(&[0x1A, 0x45, 0xDF, 0xA3], b"");
        file.extend(ebml(
            &[0x18, 0x53, 0x80, 0x67],
            &ebml(&[0x15, 0x49, 0xA9, 0x66], &tiny),
        ));
        assert_eq!(secs(&file), None, "under a millisecond is no length");
    }

    // MPEG-1 layer III, 128 kbit/s, 44.1 kHz, stereo: 417 bytes per frame without padding.
    const HEADER: [u8; 4] = [0xFF, 0xFB, 0x90, 0x00];

    fn cbr(frames: usize) -> Vec<u8> {
        let mut f = HEADER.to_vec();
        f.resize(417, 0);
        f.repeat(frames)
    }

    #[test]
    fn mp3_length_comes_from_xing_vbri_or_constant_bitrate() {
        let mut xing = HEADER.to_vec();
        xing.resize(4 + 32, 0);
        xing.extend_from_slice(b"Xing\0\0\0\x01");
        xing.extend_from_slice(&100u32.to_be_bytes());
        xing.resize(417, 0);
        let mut tagged = b"ID3\x04\0\0\0\0\0\x0a".to_vec();
        tagged.extend_from_slice(&[0; 10]);
        tagged.extend(&xing);
        assert_eq!(
            secs(&tagged),
            Some(2.612),
            "100 frames of 1152 samples at 44.1 kHz"
        );

        let mut vbri = HEADER.to_vec();
        vbri.resize(4 + 32, 0);
        vbri.extend_from_slice(b"VBRI\0\x01\0\0\0\0\0\0\0\0");
        vbri.extend_from_slice(&50u32.to_be_bytes());
        vbri.resize(417, 0);
        assert_eq!(secs(&vbri), Some(1.306));

        let mut plain = cbr(10);
        assert_eq!(secs(&plain), Some(0.261), "4170 bytes at 128 kbit/s");
        plain.extend_from_slice(b"TAG");
        plain.resize(plain.len() + 125, 0);
        assert_eq!(
            secs(&plain),
            Some(0.261),
            "an ID3v1 tag at the end isn't audio"
        );

        let mut no_frames = HEADER.to_vec();
        no_frames.resize(4 + 32, 0);
        no_frames.extend_from_slice(b"Info\0\0\0\x0e");
        no_frames.resize(417, 0);
        assert_eq!(secs(&no_frames), None);
        assert_eq!(
            secs(&cbr(1)),
            None,
            "one frame with nothing after it to confirm it"
        );
        assert_eq!(secs(&[0xFF, 0xFB, 0x90, 0x00, 1, 2, 3]), None);
        assert_eq!(secs(&[0u8; 2048]), None);
        assert_eq!(secs(b""), None);
    }

    #[test]
    fn mpeg2_layer3_frames_are_half_as_long() {
        let header = [0xFF, 0xF3, 0x64, 0xC0];
        let f = frame(&header, 0).unwrap();
        assert_eq!(
            (f.rate, f.bitrate, f.samples(), f.len),
            (24_000, 48_000, 576, 144)
        );
        assert!(f.mono);
        assert_eq!(
            frame(&[0xFF, 0xEB, 0x90, 0x00], 0),
            None,
            "the reserved version"
        );
        assert_eq!(
            frame(&[0xFF, 0xFB, 0xF0, 0x00], 0),
            None,
            "a bad bitrate index"
        );
    }

    #[test]
    fn a_box_size_that_runs_past_the_end_ends_the_search() {
        let mut file = mp4_box(b"ftyp", b"isom\0\0\x02\0isomiso2avc1mp41");
        file.extend_from_slice(&[0, 0, 3, 0x32]);
        file.extend_from_slice(b"moov");
        file.extend_from_slice(&[0, 0, 0, 1]);
        file.extend_from_slice(&[0xFF; 112]);
        assert_eq!(secs(&file), None);
        let mut huge = mp4_box(b"ftyp", b"isom\0\0\0\0");
        huge.extend_from_slice(&[0, 0, 0, 1]);
        huge.extend_from_slice(b"free");
        huge.extend_from_slice(&u64::MAX.to_be_bytes());
        assert_eq!(secs(&huge), None);
    }

    #[test]
    fn stated_lengths_past_a_day_are_broken_headers() {
        let mut mvhd = vec![0u8; 12];
        mvhd.extend_from_slice(&1u32.to_be_bytes());
        mvhd.extend_from_slice(&200_000u32.to_be_bytes());
        let mut file = mp4_box(b"ftyp", b"isom\0\0\0\0");
        file.extend(mp4_box(b"moov", &mp4_box(b"mvhd", &mvhd)));
        assert_eq!(secs(&file), None);
    }

    #[test]
    fn recorded_files_state_the_length_ffprobe_reads() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        for (file, seconds) in [
            ("openai_speech/responses/speech.mp3", 1.111),
            ("elevenlabs/responses/recorded-speech.mp3", 0.604),
            ("elevenlabs/responses/recorded-sound.mp3", 0.48),
            ("elevenlabs/responses/recorded-dialogue.mp3", 2.48),
            ("elevenlabs/responses/recorded-voice-change.mp3", 1.115),
            ("openai_videos/content/sample.mp4", 1.0),
        ] {
            let bytes = std::fs::read(dir.join(file)).unwrap();
            assert_eq!(secs(&bytes), Some(seconds), "{file}");
        }
        let pcm =
            std::fs::read(dir.join("elevenlabs/responses/recorded-speech-stream.pcm")).unwrap();
        assert_eq!(secs(&pcm), None, "raw PCM states no length");
    }

    proptest::proptest! {
        #[test]
        fn any_bytes_give_an_answer_without_panicking(bytes in proptest::collection::vec(proptest::num::u8::ANY, 0..2048)) {
            let _ = audio_duration(&bytes);
            for magic in [&b"RIFF\0\0\0\0WAVE"[..], b"fLaC", b"OggS", b"\0\0\0\x10ftyp", &[0x1A, 0x45, 0xDF, 0xA3], b"ID3", &HEADER] {
                let mut b = magic.to_vec();
                b.extend_from_slice(&bytes);
                let _ = audio_duration(&b);
            }
        }
    }
}
