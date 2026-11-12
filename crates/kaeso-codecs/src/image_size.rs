//! What an image file is and how large, read from its header without decoding it.
//!
//! Some providers take only certain image formats and sizes and refuse the rest instead of
//! converting them: TypeSafe's System One API takes PNG and JPEG from 280 to 1568 pixels a side.
//! [`image_info`] reads the format and the size a file states about itself, PNG's `IHDR`, a JPEG's
//! first start-of-frame marker, WebP's first chunk and GIF's logical screen, from at most
//! [`MAX_SCAN`] bytes, so a request is refused before it is sent. It never decodes pixels and
//! answers `None` for any other file or a header it can't read.

/// The most bytes read from the start of a file while looking for its size.
pub const MAX_SCAN: usize = 64 * 1024;

/// An image format [`image_info`] reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ImageKind {
    Png,
    Jpeg,
    Webp,
    Gif,
}

impl ImageKind {
    /// The format's media type, such as `image/png`.
    pub const fn media_type(self) -> &'static str {
        match self {
            ImageKind::Png => "image/png",
            ImageKind::Jpeg => "image/jpeg",
            ImageKind::Webp => "image/webp",
            ImageKind::Gif => "image/gif",
        }
    }
}

/// What a file's header says about the image.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ImageInfo {
    pub kind: ImageKind,
    pub width: u32,
    pub height: u32,
    /// An animated PNG (`acTL` before the first `IDAT`), an animated WebP or a GIF with more than
    /// one frame announced before [`MAX_SCAN`]; `false` when the header doesn't say.
    pub animated: bool,
}

/// The format and size `bytes` states, or `None` when it is no format this module reads or its
/// header is broken.
pub fn image_info(bytes: &[u8]) -> Option<ImageInfo> {
    let b = bytes.get(..MAX_SCAN).unwrap_or(bytes);
    let (kind, (width, height), animated) = if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        let (size, animated) = png(b)?;
        (ImageKind::Png, size, animated)
    } else if b.starts_with(&[0xff, 0xd8, 0xff]) {
        (ImageKind::Jpeg, jpeg(b)?, false)
    } else if b.starts_with(b"RIFF") && b.get(8..12) == Some(b"WEBP") {
        let (size, animated) = webp(b)?;
        (ImageKind::Webp, size, animated)
    } else if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
        (ImageKind::Gif, (le16(b, 6)?, le16(b, 8)?), false)
    } else {
        return None;
    };
    (width > 0 && height > 0).then_some(ImageInfo {
        kind,
        width,
        height,
        animated,
    })
}

fn at<const N: usize>(b: &[u8], i: usize) -> Option<[u8; N]> {
    b.get(i..i.checked_add(N)?)?.try_into().ok()
}

fn be16(b: &[u8], i: usize) -> Option<u32> {
    at::<2>(b, i).map(|x| u32::from(u16::from_be_bytes(x)))
}

fn be32(b: &[u8], i: usize) -> Option<u32> {
    at::<4>(b, i).map(u32::from_be_bytes)
}

fn le16(b: &[u8], i: usize) -> Option<u32> {
    at::<2>(b, i).map(|x| u32::from(u16::from_le_bytes(x)))
}

fn le24(b: &[u8], i: usize) -> Option<u32> {
    let [a, c, d] = at::<3>(b, i)?;
    Some(u32::from_le_bytes([a, c, d, 0]))
}

// PNG (RFC 2083 / ISO 15948): `IHDR` is the first chunk; an animated PNG has an `acTL` chunk
// before the first `IDAT` (APNG specification).
fn png(b: &[u8]) -> Option<((u32, u32), bool)> {
    if b.get(12..16) != Some(b"IHDR") {
        return None;
    }
    let size = (be32(b, 16)?, be32(b, 20)?);
    let mut at_chunk = 8usize;
    let mut animated = false;
    while let (Some(len), Some(name)) = (be32(b, at_chunk), b.get(at_chunk + 4..at_chunk + 8)) {
        match name {
            b"acTL" => {
                animated = true;
                break;
            }
            b"IDAT" | b"IEND" => break,
            _ => {}
        }
        let Some(next) = usize::try_from(len)
            .ok()
            .and_then(|l| at_chunk.checked_add(12)?.checked_add(l))
        else {
            break;
        };
        at_chunk = next;
    }
    Some((size, animated))
}

// JPEG (ITU T.81, B.1.1): markers after SOI until the first start of frame, SOF0–SOF15 except
// DHT (C4), JPG (C8) and DAC (CC); its frame header holds the height, then the width.
fn jpeg(b: &[u8]) -> Option<(u32, u32)> {
    let mut i = 2usize;
    loop {
        if *b.get(i)? != 0xff {
            return None;
        }
        let marker = *b.get(i + 1)?;
        match marker {
            0xff => i += 1,
            0xd8 | 0x01 | 0xd0..=0xd7 => i += 2,
            0xc0..=0xcf if !matches!(marker, 0xc4 | 0xc8 | 0xcc) => {
                return Some((be16(b, i + 7)?, be16(b, i + 5)?));
            }
            _ => {
                i = i
                    .checked_add(2)?
                    .checked_add(usize::try_from(be16(b, i + 2)?).ok()?)?
            }
        }
    }
}

// WebP container specification: the first chunk is VP8X (extended, with an animation flag),
// VP8 (lossy) or VP8L (lossless), each with its own place for the canvas size.
fn webp(b: &[u8]) -> Option<((u32, u32), bool)> {
    match &at::<4>(b, 12)? {
        b"VP8X" => {
            let animated = b.get(20).is_some_and(|flags| flags & 0x02 != 0);
            Some(((le24(b, 24)? + 1, le24(b, 27)? + 1), animated))
        }
        b"VP8 " => Some(((le16(b, 26)? & 0x3fff, le16(b, 28)? & 0x3fff), false)),
        b"VP8L" => {
            let bits = u32::from_le_bytes(at::<4>(b, 21)?);
            Some((((bits & 0x3fff) + 1, ((bits >> 14) & 0x3fff) + 1), false))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_header(width: u32, height: u32) -> Vec<u8> {
        let mut b = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR".to_vec();
        b.extend(width.to_be_bytes());
        b.extend(height.to_be_bytes());
        b.extend([8, 2, 0, 0, 0, 0, 0, 0, 0]);
        b
    }

    fn jpeg_header(width: u16, height: u16) -> Vec<u8> {
        let mut b = vec![0xff, 0xd8, 0xff, 0xe0, 0x00, 0x04, 0x4a, 0x46];
        b.extend([0xff, 0xff, 0xc2, 0x00, 0x11, 0x08]);
        b.extend(height.to_be_bytes());
        b.extend(width.to_be_bytes());
        b.extend([3, 1, 0x22, 0]);
        b
    }

    #[test]
    fn reads_png_jpeg_webp_and_gif_sizes() {
        let png = image_info(&png_header(280, 1568)).unwrap();
        assert_eq!(
            (png.kind, png.width, png.height, png.animated),
            (ImageKind::Png, 280, 1568, false)
        );
        let jpeg = image_info(&jpeg_header(1600, 200)).unwrap();
        assert_eq!(
            (jpeg.kind, jpeg.width, jpeg.height),
            (ImageKind::Jpeg, 1600, 200)
        );
        let mut webp = b"RIFF\0\0\0\0WEBPVP8X\x0a\0\0\0\x02\0\0\0".to_vec();
        webp.extend([0x2b, 0x01, 0x00, 0x2b, 0x01, 0x00]);
        let info = image_info(&webp).unwrap();
        assert_eq!(
            (info.kind, info.width, info.height, info.animated),
            (ImageKind::Webp, 300, 300, true)
        );
        assert_eq!(info.kind.media_type(), "image/webp");
        let gif = image_info(b"GIF89a\x2c\x01\x18\x01\0\0").unwrap();
        assert_eq!(
            (gif.kind, gif.width, gif.height),
            (ImageKind::Gif, 300, 280)
        );
    }

    #[test]
    fn finds_animation_in_a_png_before_its_image_data() {
        let mut apng = png_header(300, 300);
        apng.extend([0, 0, 0, 8]);
        apng.extend(b"acTL");
        apng.extend([0; 12]);
        assert!(image_info(&apng).unwrap().animated);
        let mut still = png_header(300, 300);
        still.extend([0, 0, 0, 0]);
        still.extend(b"IDAT");
        still.extend([0; 4]);
        still.extend([0, 0, 0, 8]);
        still.extend(b"acTL");
        assert!(!image_info(&still).unwrap().animated);
    }

    #[test]
    fn real_files_read_like_their_headers() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/system_one/requests/assets.json");
        let body: serde_json::Value = serde_json::from_slice(&std::fs::read(dir).unwrap()).unwrap();
        use base64::Engine as _;
        let bytes = |key: &str| {
            base64::engine::general_purpose::STANDARD
                .decode(body["assets"][key]["data_base64"].as_str().unwrap())
                .unwrap()
        };
        let jpeg = image_info(&bytes("a3f1c2e8-found")).unwrap();
        assert_eq!(
            (jpeg.kind, jpeg.width, jpeg.height),
            (ImageKind::Jpeg, 280, 280)
        );
        let png = image_info(&bytes("77b0d4a1-poster")).unwrap();
        assert_eq!(
            (png.kind, png.width, png.height),
            (ImageKind::Png, 280, 280)
        );
    }

    #[test]
    fn anything_else_or_a_broken_header_reads_as_nothing() {
        for bytes in [
            &b""[..],
            b"not an image",
            b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dXXXX",
            &[0xff, 0xd8, 0xff, 0xe0, 0xff, 0xff],
            &[0xff, 0xd8, 0x00],
            b"RIFF\0\0\0\0WEBPVP8Z",
            b"GIF89a\0\0\x10\0",
        ] {
            assert_eq!(image_info(bytes), None, "{bytes:?}");
        }
        assert_eq!(image_info(&png_header(0, 10)), None);
    }

    #[test]
    fn a_long_jpeg_is_read_only_up_to_the_scan_limit() {
        let mut b = vec![0xff, 0xd8];
        while b.len() < MAX_SCAN + 10 {
            b.extend([0xff, 0xe1, 0x7f, 0xff]);
            b.extend(std::iter::repeat_n(0, 0x7ffd));
        }
        b.extend(jpeg_header(300, 300).get(2..).unwrap());
        assert_eq!(image_info(&b), None);
    }
}
