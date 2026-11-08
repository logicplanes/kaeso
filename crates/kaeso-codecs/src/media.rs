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
}
