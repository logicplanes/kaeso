// Chat Completions `input_audio.format` values and their media types: `wav` and `mp3` from
// `ChatCompletionRequestMessageContentPartAudio`, the others from OpenRouter's audio guide,
// spelled as Gemini's audio guide lists them.
const CHAT_AUDIO_FORMATS: &[(&str, &str)] = &[
    ("wav", "audio/wav"),
    ("mp3", "audio/mpeg"),
    ("aiff", "audio/aiff"),
    ("aac", "audio/aac"),
    ("ogg", "audio/ogg"),
    ("flac", "audio/flac"),
    ("m4a", "audio/m4a"),
];

const AUDIO_ALIASES: &[(&str, &str)] = &[
    ("audio/x-wav", "wav"),
    ("audio/wave", "wav"),
    ("audio/vnd.wave", "wav"),
    ("audio/mp3", "mp3"),
    ("audio/x-aiff", "aiff"),
    ("audio/x-flac", "flac"),
    ("audio/mp4", "m4a"),
    ("audio/x-m4a", "m4a"),
];

pub(crate) fn audio_media_type(format: &str) -> Option<&'static str> {
    CHAT_AUDIO_FORMATS
        .iter()
        .find(|(f, _)| *f == format)
        .map(|(_, m)| *m)
}

pub(crate) fn chat_audio_format(media_type: &str) -> Option<&'static str> {
    let essence = essence(media_type);
    CHAT_AUDIO_FORMATS
        .iter()
        .find(|(_, m)| *m == essence)
        .map(|(f, _)| *f)
        .or_else(|| {
            AUDIO_ALIASES
                .iter()
                .find(|(m, _)| *m == essence)
                .map(|(_, f)| *f)
        })
}

/// The type and subtype of a media type, lowercase, without its parameters.
pub(crate) fn essence(media_type: &str) -> String {
    media_type
        .split(';')
        .next()
        .unwrap_or(media_type)
        .trim()
        .to_ascii_lowercase()
}

pub(crate) fn is_audio(media_type: &str) -> bool {
    essence(media_type).starts_with("audio/")
}

pub(crate) fn is_video(media_type: &str) -> bool {
    essence(media_type).starts_with("video/")
}

// Gemini reads a public YouTube URL in `fileData.fileUri` without a MIME type (video
// understanding guide, "YouTube URLs").
pub(crate) fn is_youtube_url(url: &str) -> bool {
    let Some(rest) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
    else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    matches!(
        host.to_ascii_lowercase().as_str(),
        "youtube.com" | "www.youtube.com" | "m.youtube.com" | "youtu.be"
    )
}

// The built-in voices of `CreateChatCompletionRequest.audio.voice` at the pinned spec commit.
pub(crate) const OPENAI_VOICES: &[&str] = &[
    "alloy", "ash", "ballad", "coral", "echo", "fable", "nova", "onyx", "sage", "shimmer", "verse",
    "marin", "cedar",
];

// The 30 prebuilt voices of Gemini's speech generation guide.
pub(crate) const GEMINI_VOICES: &[&str] = &[
    "Zephyr",
    "Puck",
    "Charon",
    "Kore",
    "Fenrir",
    "Leda",
    "Orus",
    "Aoede",
    "Callirrhoe",
    "Autonoe",
    "Enceladus",
    "Iapetus",
    "Umbriel",
    "Algieba",
    "Despina",
    "Erinome",
    "Algenib",
    "Rasalgethi",
    "Laomedeia",
    "Achernar",
    "Alnilam",
    "Schedar",
    "Gacrux",
    "Pulcherrima",
    "Achird",
    "Zubenelgenubi",
    "Vindemiatrix",
    "Sadachbia",
    "Sadaltager",
    "Sulafat",
];

pub(crate) fn has_voice(voices: &[&str], voice: &str) -> bool {
    voices.iter().any(|v| v.eq_ignore_ascii_case(voice))
}

/// Whether a media type is 16-bit PCM at 24 kHz, mono, as Chat Completions' `pcm16` and
/// Gemini's `audio/l16; rate=24000; channels=1` and `audio/L16;codec=pcm;rate=24000` are.
pub(crate) fn is_pcm16(media_type: &str) -> bool {
    let mut parts = media_type.split(';').map(str::trim);
    if !parts
        .next()
        .is_some_and(|e| e.eq_ignore_ascii_case("audio/l16"))
    {
        return false;
    }
    let mut rate = false;
    for p in parts {
        let Some((k, v)) = p.split_once('=') else {
            return false;
        };
        match (k.trim().to_ascii_lowercase().as_str(), v.trim()) {
            ("rate", "24000") => rate = true,
            ("channels", "1") | ("codec", "pcm") => {}
            _ => return false,
        }
    }
    rate
}

/// The audio of several blocks or stream deltas as one base64 string. Pieces without padding
/// join as they are; padded pieces are decoded and encoded again, which changes no byte.
pub(crate) fn join_base64<'a>(pieces: impl IntoIterator<Item = &'a str>) -> Option<String> {
    use base64::Engine as _;
    let pieces: Vec<&str> = pieces.into_iter().collect();
    let padded = pieces
        .iter()
        .rev()
        .skip(1)
        .any(|p| p.trim_end().ends_with('='));
    if !padded {
        return Some(pieces.concat());
    }
    let engine = base64::engine::general_purpose::STANDARD;
    let mut bytes = Vec::new();
    for p in pieces {
        bytes.extend(engine.decode(p.trim()).ok()?);
    }
    Some(engine.encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_formats_map_both_ways_and_aliases_one_way() {
        for (format, media) in CHAT_AUDIO_FORMATS {
            assert_eq!(audio_media_type(format), Some(*media));
            assert_eq!(chat_audio_format(media), Some(*format));
        }
        assert_eq!(chat_audio_format("audio/MP3"), Some("mp3"));
        assert_eq!(chat_audio_format("audio/x-wav; codecs=1"), Some("wav"));
        assert_eq!(chat_audio_format("audio/webm"), None);
        assert_eq!(audio_media_type("pcm16"), None);
    }

    #[test]
    fn youtube_urls_are_told_apart_by_their_host() {
        assert!(is_youtube_url(
            "https://www.youtube.com/watch?v=9hE5-98ZeCg"
        ));
        assert!(is_youtube_url("https://youtu.be/9hE5-98ZeCg"));
        assert!(!is_youtube_url("https://example.com/youtube.com/x.mp4"));
        assert!(!is_youtube_url("files/abc"));
        assert!(is_video("video/mp4") && is_audio("Audio/L16; rate=24000"));
    }

    #[test]
    fn base64_pieces_join_without_and_with_padding() {
        assert_eq!(join_base64(["AAAA", "AQID"]).as_deref(), Some("AAAAAQID"));
        assert_eq!(join_base64(["AA==", "AQ=="]).as_deref(), Some("AAE="));
        assert_eq!(join_base64(["AA==", "%%"]), None);
        assert_eq!(join_base64(["AAAA", "AQ=="]).as_deref(), Some("AAAAAQ=="));
    }

    #[test]
    fn pcm16_is_24_khz_mono_l16_in_both_spellings() {
        assert!(is_pcm16("audio/l16; rate=24000; channels=1"));
        assert!(is_pcm16("audio/L16;codec=pcm;rate=24000"));
        assert!(!is_pcm16("audio/l16; rate=16000"));
        assert!(!is_pcm16("audio/l16"));
        assert!(!is_pcm16("audio/l16; rate=24000; channels=2"));
        assert!(!is_pcm16("audio/wav"));
        assert!(has_voice(GEMINI_VOICES, "kore") && !has_voice(OPENAI_VOICES, "Kore"));
    }
}

/// The media type of base64 image bytes, from their first bytes, for providers that send
/// images without naming the format.
pub(crate) fn sniff_image(data: &str) -> Option<&'static str> {
    use base64::Engine as _;
    let head = data.get(..16).unwrap_or(data);
    let bytes = base64::engine::general_purpose::STANDARD_NO_PAD
        .decode(head.trim_end_matches('='))
        .ok()?;
    let starts = |magic: &[u8]| bytes.starts_with(magic);
    Some(if starts(&[0x89, b'P', b'N', b'G']) {
        "image/png"
    } else if starts(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg"
    } else if starts(b"GIF8") {
        "image/gif"
    } else if starts(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        "image/webp"
    } else {
        return None;
    })
}

#[cfg(test)]
mod sniff_tests {
    use super::sniff_image;

    #[test]
    fn image_formats_are_told_by_their_first_bytes() {
        assert_eq!(
            sniff_image("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJ"),
            Some("image/png")
        );
        assert_eq!(sniff_image("/9j/4AAQSkZJRgABAQ"), Some("image/jpeg"));
        assert_eq!(sniff_image("R0lGODlhAQABAIAAAA"), Some("image/gif"));
        assert_eq!(sniff_image("UklGRiQAAABXRUJQVlA4"), Some("image/webp"));
        assert_eq!(sniff_image("AAAA"), None);
        assert_eq!(sniff_image("!!"), None);
    }
}
