use std::time::Duration;

use bytes::Bytes;
use futures::StreamExt as _;
use kaeso_codecs::{EgressCtx, HeaderList, dispatch};
use kaeso_core::cost::MediaUnits;
use kaeso_core::provider::AuthStyle;
use kaeso_core::providers::ProviderRef;
use kaeso_kpp::{
    AudioInput, ImageRequest, ImageResponse, ImageSize, RequestKind, Source, SpeechRequest,
    SpeechResponse, TranscriptionRequest, TranscriptionResponse,
};
use kaeso_kpp::{DialectId, KppError};
use kaeso_upstream::{Call, Guard, Outgoing, ResponseBody, Timeouts};
use url::Url;

use crate::state::AppState;

/// The request as the log keeps it: every field, each image replaced by its media type and
/// size, since request logs never hold media.
pub(super) fn logged_request(r: &ImageRequest) -> Bytes {
    let mut kept = r.clone();
    for input in kept.images.iter_mut().chain(kept.mask.as_mut()) {
        if let Source::Base64(b) = &input.source {
            let bytes = b.data.len() / 4 * 3;
            input.source = Source::text(b.media_type.clone(), format!("{bytes} bytes, not logged"));
        }
    }
    Bytes::from(serde_json::to_vec(&kept).unwrap_or_default())
}

fn without_bytes(input: &mut AudioInput) {
    if let Source::Base64(b) = &input.source {
        let bytes = b.data.len() / 4 * 3;
        input.source = Source::text(b.media_type.clone(), format!("{bytes} bytes, not logged"));
    }
}

/// A transcription as the log keeps it: every field, the audio replaced by its media type and
/// size.
pub(super) fn logged_transcription(r: &TranscriptionRequest) -> Bytes {
    let mut kept = r.clone();
    without_bytes(&mut kept.audio);
    Bytes::from(serde_json::to_vec(&kept).unwrap_or_default())
}

/// A request for audio made from audio, as the log keeps it: the source audio replaced by its
/// media type and size.
pub(super) fn logged_speech(r: &SpeechRequest) -> Bytes {
    let mut kept = r.clone();
    if let Some(audio) = kept.audio.as_mut() {
        without_bytes(audio);
    }
    Bytes::from(serde_json::to_vec(&kept).unwrap_or_default())
}

/// The characters a request asks to speak, and the length it asks a sound or music to last.
pub(super) fn units_of_speech(r: &SpeechRequest) -> MediaUnits {
    MediaUnits {
        characters: r.characters(),
        output_ms: r.duration.map_or(0, kaeso_core::assembler::seconds_to_ms),
        format: r.format.as_ref().map(|f| f.as_wire().to_owned()),
        ..MediaUnits::default()
    }
}

/// A transcription's units: the audio's length is known only from the reply.
pub(super) fn units_of_transcription(r: &TranscriptionRequest) -> MediaUnits {
    MediaUnits {
        format: Some(r.format_or_default().as_wire().to_owned()),
        ..MediaUnits::default()
    }
}

/// The characters the provider says it billed, when it says.
pub(super) fn add_speech(units: &mut MediaUnits, reply: &SpeechResponse) {
    if let Some(c) = reply.characters {
        units.characters = c;
    }
}

/// The length of the transcribed audio, which providers bill by the minute or hour.
pub(super) fn add_transcript(units: &mut MediaUnits, reply: &TranscriptionResponse) {
    if let Some(seconds) = reply.audio_seconds() {
        units.input_ms = kaeso_core::assembler::seconds_to_ms(seconds);
    }
}

/// Where a media request goes at its provider: from the root of the host, and with which auth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MediaEgress {
    pub(super) from_root: bool,
    pub(super) auth: Option<AuthStyle>,
}

pub(super) fn egress_of(
    provider: &ProviderRef,
    kind: RequestKind,
    model: &str,
) -> Option<MediaEgress> {
    match kind {
        RequestKind::Video => provider.videos(model).map(|e| MediaEgress {
            from_root: e.from_root,
            auth: e.auth,
        }),
        RequestKind::Image => provider.images(model).map(|e| MediaEgress {
            from_root: e.from_root,
            auth: e.auth,
        }),
        RequestKind::Speech => provider.speech(model).map(|e| MediaEgress {
            from_root: e.from_root,
            auth: e.auth,
        }),
        RequestKind::Transcription => provider.transcriptions(model).map(|e| MediaEgress {
            from_root: e.from_root,
            auth: e.auth,
        }),
        _ => None,
    }
}

pub(super) fn units_of_request(r: &ImageRequest) -> MediaUnits {
    MediaUnits {
        input_images: r.images.len() as u64,
        output_images: 0,
        output_pixels: pixels(r.size.as_ref()),
        size: r.size.as_ref().map(|s| s.as_wire().to_owned()),
        quality: r.quality.as_ref().map(|q| q.as_wire().to_owned()),
        resolution: r.resolution.clone(),
        ..MediaUnits::default()
    }
}

/// Adds what the reply says it made: its images, and the size and quality the provider used
/// when it reports them.
pub(super) fn add_reply(units: &mut MediaUnits, reply: &ImageResponse) {
    units.output_images = reply.images.len() as u64;
    if let Some(tier) = &reply.settings.resolution {
        units.resolution = Some(tier.clone());
    }
    add_settings(
        units,
        reply.settings.size.as_ref(),
        reply.settings.quality.as_ref().map(|q| q.as_wire()),
    );
}

pub(super) fn add_streamed(units: &mut MediaUnits, images: u64) {
    units.output_images = images;
}

fn add_settings(units: &mut MediaUnits, size: Option<&ImageSize>, quality: Option<&str>) {
    if let Some(size) = size
        && *size != ImageSize::Auto
    {
        units.size = Some(size.as_wire().to_owned());
        units.output_pixels = pixels(Some(size)).or(units.output_pixels);
    }
    if let Some(q) = quality {
        units.quality = Some(q.to_owned());
    }
}

fn pixels(size: Option<&ImageSize>) -> Option<u64> {
    let (w, h) = size?.dimensions()?;
    u64::from(w).checked_mul(u64::from(h))
}

/// How the request that started a provider's task reached it, for asking how the task stands.
pub(super) struct Asked<'a> {
    pub(super) base: &'a Url,
    pub(super) auth: Option<AuthStyle>,
    pub(super) guard: Guard,
    pub(super) credential: &'a secrecy::SecretString,
    pub(super) timeouts: Timeouts,
    pub(super) max_body: usize,
}

// QwenCloud asks callers to check a text-to-image task every few seconds and makes a small image
// in a few (https://docs.qwencloud.com/api-reference/image-generation/wan-text-to-image-v2/query-result).
const TASK_WAITS: [Duration; 4] = [
    Duration::ZERO,
    Duration::from_secs(2),
    Duration::from_secs(3),
    Duration::from_secs(5),
];

/// Asks the provider how the task an image reply named stands until its answer names no
/// pending task, within the request's deadline, and hands back that answer.
pub(super) async fn wait_for_task(
    state: &AppState,
    asked: &Asked<'_>,
    dialect: DialectId,
    ctx: &EgressCtx,
    task: &str,
    deadline: tokio::time::Instant,
) -> Result<(u16, HeaderList, Bytes), KppError> {
    for round in 0usize.. {
        let wait = TASK_WAITS
            .get(round)
            .or(TASK_WAITS.last())
            .copied()
            .unwrap_or_default();
        let at = tokio::time::Instant::now() + wait;
        if at >= deadline {
            return Err(super::pipeline::upstream_error(
                &kaeso_upstream::UpstreamError::Timeout("request"),
            ));
        }
        tokio::time::sleep_until(at).await;
        let req = dispatch::encode_task_poll(dialect, task, ctx)?;
        let resp = tokio::time::timeout_at(
            deadline,
            state.upstream.send(Call {
                base_url: asked.base,
                auth: asked.auth,
                guard: asked.guard,
                credential: asked.credential,
                request: Outgoing::from(req),
                stream: false,
                timeouts: asked.timeouts,
                max_body: asked.max_body,
            }),
        )
        .await
        .map_err(|_| {
            super::pipeline::upstream_error(&kaeso_upstream::UpstreamError::Timeout("request"))
        })?
        .map_err(|e| super::pipeline::upstream_error(&e))?;
        let body = match resp.body {
            ResponseBody::Full(b) => b,
            ResponseBody::Stream(mut s) => {
                let mut all = Vec::new();
                while let Some(chunk) = s.next().await {
                    all.extend_from_slice(&chunk.map_err(|e| super::pipeline::upstream_error(&e))?);
                }
                Bytes::from(all)
            }
        };
        if !(200..300).contains(&resp.status) {
            return Err(dispatch::decode_error(
                dialect,
                resp.status,
                &resp.headers,
                &body,
            ));
        }
        if dispatch::pending_task(dialect, ctx.kind, &body).is_none() {
            return Ok((resp.status, resp.headers, body));
        }
    }
    Err(KppError::provider(
        "The provider's task didn't finish in time; try again later.",
    ))
}

/// The base URL without its path, for image APIs served from the root of the provider's host.
pub(super) fn host_root(base: &Url) -> Url {
    let mut root = base.clone();
    root.set_path("/");
    root
}

#[cfg(test)]
mod tests {
    use kaeso_kpp::{GeneratedImage, ImageInput, ImageOperation, ImageQuality, ImageSettings};

    use super::*;

    #[test]
    fn logged_requests_keep_fields_and_drop_image_bytes() {
        let mut r = ImageRequest::new("gpt-image-2", ImageOperation::Edit)
            .with_prompt("hat")
            .with_image(ImageInput::new(Source::base64("image/png", "AAAAAAAA")))
            .with_image(ImageInput::new(Source::Url(
                "https://example.com/a.png".into(),
            )));
        r.mask = Some(ImageInput::new(Source::base64("image/png", "AAAA")));
        let logged = String::from_utf8(logged_request(&r).to_vec()).unwrap();
        assert!(!logged.contains("AAAA"), "{logged}");
        assert!(logged.contains("6 bytes, not logged") && logged.contains("3 bytes, not logged"));
        assert!(logged.contains("https://example.com/a.png") && logged.contains("\"hat\""));
    }

    #[test]
    fn units_come_from_the_request_and_what_the_reply_reports() {
        let mut r = ImageRequest::new("m", ImageOperation::Generate).with_prompt("fox");
        r.size = Some(ImageSize::pixels(1024, 768));
        r.quality = Some(ImageQuality::High);
        r.resolution = Some("2k".into());
        let mut units = units_of_request(&r);
        assert_eq!(units.output_pixels, Some(786_432));
        let mut reply =
            ImageResponse::new("m", vec![GeneratedImage::new(Source::Url("u".into())); 3]);
        let mut settings = ImageSettings::default();
        settings.size = Some(ImageSize::pixels(2048, 2048));
        settings.quality = Some(ImageQuality::Medium);
        reply.settings = settings;
        add_reply(&mut units, &reply);
        assert_eq!(units.output_images, 3);
        assert_eq!(units.output_pixels, Some(4_194_304));
        assert_eq!(units.quality.as_deref(), Some("medium"));
        assert_eq!(units.resolution.as_deref(), Some("2k"));
        add_streamed(&mut units, 1);
        assert_eq!(units.output_images, 1);
        assert_eq!(
            host_root(&Url::parse("https://api.z.ai/api/anthropic").unwrap()).as_str(),
            "https://api.z.ai/"
        );
    }
}
