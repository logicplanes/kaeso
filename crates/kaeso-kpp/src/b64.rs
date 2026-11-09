use std::fmt;

use base64::Engine as _;
use base64::alphabet::STANDARD;
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
use bytes::Bytes;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

// Apps and providers write base64 with and without padding (RFC 4648 section 3.2 lets a
// specification leave it out); both decode to the same bytes, and KPP always writes it padded.
const READ: GeneralPurpose = GeneralPurpose::new(
    &STANDARD,
    GeneralPurposeConfig::new()
        .with_decode_padding_mode(DecodePaddingMode::Indifferent)
        .with_decode_allow_trailing_bits(true),
);
const WRITE: GeneralPurpose = base64::engine::general_purpose::STANDARD;

// A multiple of 3 bytes, so no chunk but the last is padded.
const CHUNK: usize = 3 * 1024;

pub(crate) fn decode(text: &[u8]) -> Result<Bytes, base64::DecodeError> {
    READ.decode(text).map(Bytes::from)
}

pub(crate) fn encoded_len(bytes: usize) -> usize {
    bytes.div_ceil(3).saturating_mul(4)
}

pub(crate) fn push(data: &[u8], out: &mut String) {
    out.reserve(encoded_len(data.len()));
    WRITE.encode_string(data, out);
}

/// Media bytes as standard, padded base64, written chunk by chunk.
///
/// It formats with [`fmt::Display`] and serializes as a JSON string through
/// [`Serializer::collect_str`], so a codec writes base64 straight into its output without
/// building the whole text first.
#[derive(Clone, Copy)]
pub struct Base64Text<'a>(pub(crate) &'a [u8]);

impl<'a> Base64Text<'a> {
    /// Bytes to write as base64.
    pub fn new(bytes: &'a [u8]) -> Self {
        Base64Text(bytes)
    }

    /// The length of the text in bytes.
    pub fn len(&self) -> usize {
        encoded_len(self.0.len())
    }

    /// Whether there are no bytes to write.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for Base64Text<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf = [0u8; CHUNK / 3 * 4];
        for chunk in self.0.chunks(CHUNK) {
            let n = WRITE
                .encode_slice(chunk, &mut buf)
                .map_err(|_| fmt::Error)?;
            let text = buf
                .get(..n)
                .and_then(|b| std::str::from_utf8(b).ok())
                .ok_or(fmt::Error)?;
            f.write_str(text)?;
        }
        Ok(())
    }
}

impl fmt::Debug for Base64Text<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Base64Text({} bytes)", self.0.len())
    }
}

impl Serialize for Base64Text<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

pub(crate) fn serialize<S: Serializer>(data: &Bytes, s: S) -> Result<S::Ok, S::Error> {
    Base64Text(data).serialize(s)
}

pub(crate) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Bytes, D::Error> {
    let text = std::borrow::Cow::<'de, str>::deserialize(d)?;
    decode(text.as_bytes()).map_err(|_| serde::de::Error::custom("`data` is not valid base64"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padding_is_optional_on_read_and_written_on_write() {
        assert_eq!(decode(b"AA==").unwrap(), Bytes::from_static(&[0]));
        assert_eq!(decode(b"AA").unwrap(), Bytes::from_static(&[0]));
        assert_eq!(Base64Text(&[0]).to_string(), "AA==");
        assert!(decode(b"A A=").is_err());
        assert!(decode(b"AA-_").is_err());
    }

    #[test]
    fn chunked_text_equals_one_shot_encoding() {
        for len in [0, 1, 2, 3, CHUNK - 1, CHUNK, CHUNK + 1, 3 * CHUNK + 2] {
            let data: Vec<u8> = (0..len).map(|i| (i * 7 % 251) as u8).collect();
            let text = Base64Text(&data);
            assert_eq!(text.to_string(), WRITE.encode(&data), "{len}");
            assert_eq!(text.len(), WRITE.encode(&data).len(), "{len}");
            assert_eq!(text.is_empty(), len == 0);
            assert_eq!(
                serde_json::to_string(&text).unwrap(),
                format!("\"{}\"", WRITE.encode(&data))
            );
            let mut pushed = String::from("x");
            push(&data, &mut pushed);
            assert_eq!(pushed, format!("x{}", WRITE.encode(&data)));
        }
        assert_eq!(format!("{:?}", Base64Text(&[1, 2])), "Base64Text(2 bytes)");
    }
}
