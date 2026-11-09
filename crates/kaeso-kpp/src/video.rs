use serde::{Deserialize, Serialize};

use crate::block::Source;
use crate::error::KppError;
use crate::extras::Extras;
use crate::image::ImageSize;
use crate::response::Usage;

/// What a video request asks for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum VideoOperation {
    /// A new video from a prompt and the images that guide it.
    #[default]
    Generate,
    /// A new video made by changing an earlier one with a new prompt.
    Remix,
}

impl VideoOperation {
    /// The operation's name in logs and messages: `generate` or `remix`.
    pub const fn as_str(self) -> &'static str {
        match self {
            VideoOperation::Generate => "generate",
            VideoOperation::Remix => "remix",
        }
    }
}

/// What an image given to a video request is for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum VideoImageRole {
    /// The video starts with this image.
    #[default]
    FirstFrame,
    /// The video ends with this image.
    LastFrame,
    /// The image guides what the video shows, without being one of its frames.
    Reference,
}

impl VideoImageRole {
    /// The role's name in messages: `first_frame`, `last_frame` or `reference`.
    pub const fn as_str(self) -> &'static str {
        match self {
            VideoImageRole::FirstFrame => "first_frame",
            VideoImageRole::LastFrame => "last_frame",
            VideoImageRole::Reference => "reference",
        }
    }
}

/// An image a video request sends, and what it is for.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VideoImage {
    pub source: Source,
    #[serde(default)]
    pub role: VideoImageRole,
    /// The file name an upload carried.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl VideoImage {
    pub fn new(source: Source, role: VideoImageRole) -> Self {
        VideoImage {
            source,
            role,
            filename: None,
            extras: Extras::default(),
        }
    }

    #[must_use]
    pub fn with_filename(mut self, filename: impl Into<String>) -> Self {
        self.filename = Some(filename.into());
        self
    }
}

/// A request that starts a video job: a generation or a remix.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VideoRequest {
    pub model: String,
    #[serde(default)]
    pub operation: VideoOperation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// The images that guide the video, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<VideoImage>,
    /// The provider's id of the earlier video a remix changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remix_of: Option<String>,
    /// The earlier video itself, for providers that take the video to change by link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_video: Option<Source>,
    /// How long the video lasts, in whole seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seconds: Option<u32>,
    /// The output size in pixels, `WIDTHxHEIGHT`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<ImageSize>,
    /// The output aspect ratio, such as `16:9`, for providers that take one instead of a size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aspect_ratio: Option<String>,
    /// The output resolution tier, such as `720p`, as the provider spells it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
    /// What the video should not show.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub negative_prompt: Option<String>,
    /// Whether the video has sound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<bool>,
    /// How many videos to make.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    /// A stable identifier of the app's end user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl VideoRequest {
    pub fn new(model: impl Into<String>, operation: VideoOperation) -> Self {
        VideoRequest {
            model: model.into(),
            operation,
            ..VideoRequest::default()
        }
    }

    #[must_use]
    pub fn with_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = Some(prompt.into());
        self
    }

    #[must_use]
    pub fn with_image(mut self, image: VideoImage) -> Self {
        self.images.push(image);
        self
    }

    /// The images given for `role`.
    pub fn images_for(&self, role: VideoImageRole) -> impl Iterator<Item = &VideoImage> {
        self.images.iter().filter(move |i| i.role == role)
    }

    /// Checks what every video request needs, whatever its format: a model, a prompt or an
    /// image to start from, at most one first and one last frame, and counts in range.
    pub fn validate(&self) -> Result<(), KppError> {
        let invalid = |message: &str, param: &str| {
            Err(KppError::invalid_request(message.to_owned()).with_param(param.to_owned()))
        };
        if self.model.is_empty() {
            return invalid("`model` must not be empty.", "model");
        }
        let prompt = self.prompt.as_deref().is_some_and(|p| !p.is_empty());
        match self.operation {
            VideoOperation::Generate if !prompt && self.images.is_empty() => {
                return invalid("`prompt` must not be empty.", "prompt");
            }
            VideoOperation::Remix if !prompt => {
                return invalid("`prompt` must not be empty.", "prompt");
            }
            VideoOperation::Remix if self.remix_of.is_none() && self.source_video.is_none() => {
                return invalid("A remix names the video it changes.", "video_id");
            }
            _ => {}
        }
        for role in [VideoImageRole::FirstFrame, VideoImageRole::LastFrame] {
            if self.images_for(role).count() > 1 {
                return invalid(
                    &format!("Send at most one image as the {} frame.", frame_word(role)),
                    "input_reference",
                );
            }
        }
        if self.images_for(VideoImageRole::LastFrame).next().is_some()
            && self.images_for(VideoImageRole::FirstFrame).next().is_none()
        {
            return invalid(
                "A last frame needs a first frame to start from.",
                "input_reference",
            );
        }
        if self.seconds == Some(0) {
            return invalid("`seconds` must be at least 1.", "seconds");
        }
        if self.count == Some(0) {
            return invalid("`n` must be at least 1.", "n");
        }
        Ok(())
    }

    /// What a model must be able to do to serve the request.
    pub fn required_capabilities(&self) -> crate::Capabilities {
        crate::Capabilities {
            images: !self.images.is_empty(),
            video_input: self.source_video.is_some(),
            video_output: true,
            ..crate::Capabilities::default()
        }
    }
}

fn frame_word(role: VideoImageRole) -> &'static str {
    match role {
        VideoImageRole::LastFrame => "last",
        _ => "first",
    }
}

/// How far a video job has come.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum VideoStatus {
    /// Waiting for the provider to start.
    #[default]
    Queued,
    /// Being made.
    InProgress,
    /// Made; its outputs can be fetched.
    Completed,
    /// Ended without a video.
    Failed,
}

impl VideoStatus {
    /// The status as OpenAI's Videos API writes it: `queued`, `in_progress`, `completed` or
    /// `failed`.
    pub const fn as_str(self) -> &'static str {
        match self {
            VideoStatus::Queued => "queued",
            VideoStatus::InProgress => "in_progress",
            VideoStatus::Completed => "completed",
            VideoStatus::Failed => "failed",
        }
    }

    /// The status of `as_str`'s spelling.
    pub fn from_wire(s: &str) -> Option<VideoStatus> {
        Some(match s {
            "queued" => VideoStatus::Queued,
            "in_progress" => VideoStatus::InProgress,
            "completed" => VideoStatus::Completed,
            "failed" => VideoStatus::Failed,
            _ => return None,
        })
    }

    /// Whether the job has ended, made or not.
    pub const fn is_done(self) -> bool {
        matches!(self, VideoStatus::Completed | VideoStatus::Failed)
    }
}

/// Which file of a made video an output is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum VideoVariant {
    /// The video itself.
    #[default]
    Video,
    /// A still image of the video.
    Thumbnail,
    /// A grid of stills across the video.
    Spritesheet,
}

impl VideoVariant {
    /// The variant as OpenAI's `variant` query parameter spells it.
    pub const fn as_str(self) -> &'static str {
        match self {
            VideoVariant::Video => "video",
            VideoVariant::Thumbnail => "thumbnail",
            VideoVariant::Spritesheet => "spritesheet",
        }
    }

    /// The variant of `as_str`'s spelling.
    pub fn from_wire(s: &str) -> Option<VideoVariant> {
        Some(match s {
            "video" => VideoVariant::Video,
            "thumbnail" => VideoVariant::Thumbnail,
            "spritesheet" => VideoVariant::Spritesheet,
            _ => return None,
        })
    }
}

/// A file a finished video job made: where to fetch it, and its media type when known.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VideoOutput {
    #[serde(default)]
    pub variant: VideoVariant,
    /// A link, the bytes themselves, or a file id the provider resolves to a link.
    pub source: Source,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
}

impl VideoOutput {
    pub fn new(variant: VideoVariant, source: Source) -> Self {
        VideoOutput {
            variant,
            source,
            media_type: None,
        }
    }

    #[must_use]
    pub fn with_media_type(mut self, media_type: impl Into<String>) -> Self {
        self.media_type = Some(media_type.into());
        self
    }
}

/// Why a video job failed, as the provider says it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VideoError {
    /// The provider's code for the failure.
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub param: Option<String>,
    /// The provider's error type, when it names one apart from the code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

impl VideoError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        VideoError {
            code: code.into(),
            message: message.into(),
            param: None,
            kind: None,
        }
    }
}

/// A video job as the provider reports it: when it is started, and each time it is asked.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VideoJob {
    /// The job's id at the provider, which the provider is asked about later.
    pub id: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub status: VideoStatus,
    /// How far the job has come, 0 to 100, when the provider says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<u32>,
    /// When the job was created, in Unix seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
    /// When the job ended, in Unix seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<i64>,
    /// When the outputs stop being available, in Unix seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// How long the video lasts, in whole seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seconds: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<ImageSize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aspect_ratio: Option<String>,
    /// Whether the video has sound, when the provider says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<bool>,
    /// The id of the video this one remixes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remixed_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<VideoError>,
    /// The files a finished job made.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<VideoOutput>,
    /// The tokens the provider counted, for providers that bill video by the token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl VideoJob {
    pub fn new(id: impl Into<String>, status: VideoStatus) -> Self {
        VideoJob {
            id: id.into(),
            status,
            ..VideoJob::default()
        }
    }

    /// The first output of `variant`.
    pub fn output(&self, variant: VideoVariant) -> Option<&VideoOutput> {
        self.outputs.iter().find(|o| o.variant == variant)
    }
}

/// The order of a video listing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ListOrder {
    Asc,
    /// Newest first, OpenAI's default.
    #[default]
    Desc,
}

/// What a video listing asks for: a page after an id, its length and its order.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VideoListQuery {
    /// The id of the last video of the previous page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    /// How many videos to list, 0 to 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default)]
    pub order: ListOrder,
}

impl VideoListQuery {
    /// The page length OpenAI's Videos API lists when the query names none.
    pub const DEFAULT_LIMIT: u32 = 20;
    /// The longest page OpenAI's Videos API lists.
    pub const MAX_LIMIT: u32 = 100;

    /// The page length to list: the query's, or the default.
    pub fn limit(&self) -> u32 {
        self.limit
            .unwrap_or(Self::DEFAULT_LIMIT)
            .min(Self::MAX_LIMIT)
    }
}

/// A page of a video listing.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VideoList {
    pub videos: Vec<VideoJob>,
    #[serde(default)]
    pub has_more: bool,
}

impl VideoList {
    pub fn new(videos: Vec<VideoJob>, has_more: bool) -> Self {
        VideoList { videos, has_more }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(role: VideoImageRole) -> VideoImage {
        VideoImage::new(Source::Url("https://example.com/a.png".into()), role)
    }

    #[test]
    fn requests_need_a_model_a_prompt_or_an_image_and_one_frame_each() {
        let ok = VideoRequest::new("m", VideoOperation::Generate).with_prompt("fox");
        assert!(ok.validate().is_ok());
        let param = |r: &VideoRequest| r.validate().unwrap_err().param;
        assert_eq!(
            param(&VideoRequest::new("", VideoOperation::Generate)).as_deref(),
            Some("model")
        );
        assert_eq!(
            param(&VideoRequest::new("m", VideoOperation::Generate)).as_deref(),
            Some("prompt")
        );
        let from_image = VideoRequest::new("m", VideoOperation::Generate)
            .with_image(image(VideoImageRole::FirstFrame));
        assert!(from_image.validate().is_ok());
        let two_first = from_image
            .clone()
            .with_image(image(VideoImageRole::FirstFrame));
        assert_eq!(param(&two_first).as_deref(), Some("input_reference"));
        let last_only = VideoRequest::new("m", VideoOperation::Generate)
            .with_prompt("fox")
            .with_image(image(VideoImageRole::LastFrame));
        assert_eq!(param(&last_only).as_deref(), Some("input_reference"));
        let mut seconds = ok.clone();
        seconds.seconds = Some(0);
        assert_eq!(param(&seconds).as_deref(), Some("seconds"));
        let mut count = ok.clone();
        count.count = Some(0);
        assert_eq!(param(&count).as_deref(), Some("n"));
    }

    #[test]
    fn a_remix_needs_a_prompt_and_its_video() {
        let mut r = VideoRequest::new("m", VideoOperation::Remix).with_prompt("in snow");
        assert_eq!(r.validate().unwrap_err().param.as_deref(), Some("video_id"));
        r.remix_of = Some("job".into());
        assert!(r.validate().is_ok());
        r.prompt = None;
        assert_eq!(r.validate().unwrap_err().param.as_deref(), Some("prompt"));
        let caps = VideoRequest::new("m", VideoOperation::Generate)
            .with_image(image(VideoImageRole::Reference))
            .required_capabilities();
        assert!(caps.video_output && caps.images && !caps.video_input);
    }

    #[test]
    fn wire_names_round_trip() {
        for s in [
            VideoStatus::Queued,
            VideoStatus::InProgress,
            VideoStatus::Completed,
            VideoStatus::Failed,
        ] {
            assert_eq!(VideoStatus::from_wire(s.as_str()), Some(s));
        }
        assert!(VideoStatus::Failed.is_done() && !VideoStatus::Queued.is_done());
        for v in [
            VideoVariant::Video,
            VideoVariant::Thumbnail,
            VideoVariant::Spritesheet,
        ] {
            assert_eq!(VideoVariant::from_wire(v.as_str()), Some(v));
        }
        assert_eq!(VideoVariant::from_wire("gif"), None);
        assert_eq!(VideoStatus::from_wire("done"), None);
        assert_eq!(VideoOperation::Remix.as_str(), "remix");
        assert_eq!(VideoImageRole::Reference.as_str(), "reference");
        let q = VideoListQuery {
            limit: Some(500),
            ..VideoListQuery::default()
        };
        assert_eq!(q.limit(), 100);
        assert_eq!(VideoListQuery::default().limit(), 20);
        let job = VideoJob::new("j", VideoStatus::Completed);
        assert!(job.output(VideoVariant::Video).is_none());
        let out = VideoOutput::new(VideoVariant::Video, Source::Url("u".into()))
            .with_media_type("video/mp4");
        assert_eq!(out.media_type.as_deref(), Some("video/mp4"));
        assert_eq!(VideoList::new(vec![job], true).videos.len(), 1);
    }
}
