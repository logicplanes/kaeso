//! ElevenLabs' `output_format` values, `{codec}_{sample rate}[_{kbps}]`, from the
//! `output_format` query parameter of the text-to-speech operations.

use kaeso_kpp::AudioFormat;

/// Every value the non-streaming text-to-speech endpoints take; the streaming ones take the
/// same without `wav_*`.
pub(super) const OUTPUT_FORMATS: &[&str] = &[
    "alaw_8000",
    "mp3_22050_32",
    "mp3_24000_48",
    "mp3_44100_128",
    "mp3_44100_192",
    "mp3_44100_32",
    "mp3_44100_64",
    "mp3_44100_96",
    "opus_48000_128",
    "opus_48000_192",
    "opus_48000_32",
    "opus_48000_64",
    "opus_48000_96",
    "pcm_16000",
    "pcm_22050",
    "pcm_24000",
    "pcm_32000",
    "pcm_44100",
    "pcm_48000",
    "pcm_8000",
    "ulaw_8000",
    "wav_16000",
    "wav_22050",
    "wav_24000",
    "wav_32000",
    "wav_44100",
    "wav_48000",
    "wav_8000",
];

/// The format, sample rate and bit rate of an `output_format` value.
pub(super) fn parse(value: &str) -> Option<(AudioFormat, Option<u32>, Option<u32>)> {
    let mut parts = value.split('_');
    let codec = parts.next()?;
    let rate = parts.next().and_then(|r| r.parse().ok());
    let kbps = parts.next().and_then(|b| b.parse().ok());
    let format = match codec {
        "mp3" => AudioFormat::Mp3,
        "opus" => AudioFormat::Opus,
        "wav" => AudioFormat::Wav,
        "pcm" if rate == Some(24_000) => AudioFormat::Pcm16,
        "pcm" | "ulaw" | "alaw" => AudioFormat::Other(codec.to_owned()),
        _ => return None,
    };
    let rate = if format == AudioFormat::Pcm16 {
        None
    } else {
        rate
    };
    Some((format, rate, kbps))
}

/// The `output_format` value of a format, a sample rate and a bit rate, with ElevenLabs'
/// default rates for what is not named: free-tier rates, since 44.1 kHz PCM and WAV need a paid
/// plan.
pub(super) fn value(format: &AudioFormat, rate: Option<u32>, kbps: Option<u32>) -> Option<String> {
    let v = match format {
        AudioFormat::Mp3 => format!("mp3_{}_{}", rate.unwrap_or(44_100), kbps.unwrap_or(128)),
        AudioFormat::Opus => format!("opus_{}_{}", rate.unwrap_or(48_000), kbps.unwrap_or(64)),
        AudioFormat::Wav => format!("wav_{}", rate.unwrap_or(24_000)),
        AudioFormat::Pcm16 => format!("pcm_{}", rate.unwrap_or(24_000)),
        AudioFormat::Other(c) if c == "pcm" => format!("pcm_{}", rate?),
        AudioFormat::Other(c) if matches!(c.as_str(), "ulaw" | "mulaw") => {
            format!("ulaw_{}", rate.unwrap_or(8_000))
        }
        AudioFormat::Other(c) if c == "alaw" => format!("alaw_{}", rate.unwrap_or(8_000)),
        _ => return None,
    };
    OUTPUT_FORMATS.contains(&v.as_str()).then_some(v)
}

/// The media type ElevenLabs answers an `output_format` with.
pub(super) fn media_type(value: &str) -> &'static str {
    match value.split('_').next() {
        Some("mp3") => "audio/mpeg",
        Some("opus") => "audio/opus",
        Some("wav") => "audio/wav",
        Some("pcm") => "audio/pcm",
        Some("ulaw") => "audio/basic",
        Some("alaw") => "audio/alaw",
        _ => "audio/mpeg",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_parse_and_write_back() {
        assert_eq!(
            parse("mp3_44100_128"),
            Some((AudioFormat::Mp3, Some(44_100), Some(128)))
        );
        assert_eq!(parse("pcm_24000"), Some((AudioFormat::Pcm16, None, None)));
        assert_eq!(
            parse("pcm_16000"),
            Some((AudioFormat::Other("pcm".into()), Some(16_000), None))
        );
        assert_eq!(parse("flac_44100"), None);
        for v in OUTPUT_FORMATS {
            let (f, r, b) = parse(v).unwrap();
            assert_eq!(value(&f, r, b).as_deref(), Some(*v), "{v}");
        }
        assert_eq!(
            value(&AudioFormat::Mp3, None, None).as_deref(),
            Some("mp3_44100_128")
        );
        assert_eq!(
            value(&AudioFormat::Wav, None, None).as_deref(),
            Some("wav_24000")
        );
        assert_eq!(value(&AudioFormat::Aac, None, None), None);
        assert_eq!(value(&AudioFormat::Mp3, Some(48_000), None), None);
        assert_eq!(media_type("ulaw_8000"), "audio/basic");
    }
}
