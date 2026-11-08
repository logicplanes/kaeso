use serde::{Deserialize, Serialize};

use crate::block::Source;
use crate::error::KppError;
use crate::extras::Extras;
use crate::request::StreamMode;
use crate::response::Usage;

/// What an image request asks for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ImageOperation {
    /// New images from a prompt.
    #[default]
    Generate,
    /// Images made from given images and a prompt.
    Edit,
    /// Variations of one image, without a prompt.
    Variation,
}

impl ImageOperation {
    /// The operation's name in logs and messages: `generate`, `edit` or `variation`.
    pub const fn as_str(self) -> &'static str {
        match self {
            ImageOperation::Generate => "generate",
            ImageOperation::Edit => "edit",
            ImageOperation::Variation => "variation",
        }
    }
}

/// A request for images: generation, edit or variation.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ImageRequest {
    pub model: String,
    #[serde(default)]
    pub operation: ImageOperation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// The images to edit or vary, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<ImageInput>,
    /// The mask for the first image: its transparent areas mark what to change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<ImageInput>,
    /// How many images to make.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    /// The output size, `auto` or `WIDTHxHEIGHT`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<ImageSize>,
    /// The output aspect ratio, such as `16:9`, for providers that take one instead of a size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aspect_ratio: Option<String>,
    /// The output resolution tier, such as `1K` or `2K`, as the provider spells it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<ImageQuality>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<ImageFormat>,
    /// The compression level of a `jpeg` or `webp` output, 0 to 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compression: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<ImageBackground>,
    /// Whether the reply carries the images' bytes or links to them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivery: Option<ImageDelivery>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
    /// What the images should not show.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub negative_prompt: Option<String>,
    /// A stable identifier of the app's end user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(default, skip_serializing_if = "StreamMode::is_off")]
    pub stream: StreamMode,
    /// How many partial images a streamed reply sends before the final one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partial_images: Option<u32>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl ImageRequest {
    pub fn new(model: impl Into<String>, operation: ImageOperation) -> Self {
        ImageRequest {
            model: model.into(),
            operation,
            ..ImageRequest::default()
        }
    }

    #[must_use]
    pub fn with_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = Some(prompt.into());
        self
    }

    #[must_use]
    pub fn with_image(mut self, image: ImageInput) -> Self {
        self.images.push(image);
        self
    }

    /// Checks what every image request needs, whatever its format: a model, a prompt to
    /// generate or edit with, images to edit or vary, and counts in range.
    pub fn validate(&self) -> Result<(), KppError> {
        let invalid = |message: &str, param: &str| {
            Err(KppError::invalid_request(message.to_owned()).with_param(param.to_owned()))
        };
        if self.model.is_empty() {
            return invalid("`model` must not be empty.", "model");
        }
        let prompt = self.prompt.as_deref().is_some_and(|p| !p.is_empty());
        match self.operation {
            ImageOperation::Generate | ImageOperation::Edit if !prompt => {
                return invalid("`prompt` must not be empty.", "prompt");
            }
            ImageOperation::Generate if !self.images.is_empty() || self.mask.is_some() => {
                return invalid(
                    "A generation request takes no images; send them to the edits endpoint.",
                    "image",
                );
            }
            ImageOperation::Edit | ImageOperation::Variation if self.images.is_empty() => {
                return invalid("Send at least one image.", "image");
            }
            ImageOperation::Variation if self.images.len() > 1 || self.mask.is_some() => {
                return invalid("A variation request takes exactly one image.", "image");
            }
            _ => {}
        }
        if self.count == Some(0) {
            return invalid("`n` must be at least 1.", "n");
        }
        if self.compression.is_some_and(|c| c > 100) {
            return invalid(
                "`output_compression` must be between 0 and 100.",
                "output_compression",
            );
        }
        Ok(())
    }

    /// What a model must be able to do to serve the request.
    pub fn required_capabilities(&self) -> crate::Capabilities {
        crate::Capabilities {
            images: !self.images.is_empty() || self.mask.is_some(),
            image_output: true,
            streaming: !self.stream.is_off(),
            ..crate::Capabilities::default()
        }
    }
}

/// An image a request sends: inline bytes, a URL or a provider file id.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ImageInput {
    pub source: Source,
    /// The file name an upload carried.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl ImageInput {
    pub fn new(source: Source) -> Self {
        ImageInput {
            source,
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

macro_rules! wire_enum {
    ($(#[$doc:meta])* $name:ident { $($variant:ident = $wire:literal),* $(,)? }) => {
        $(#[$doc])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        #[non_exhaustive]
        pub enum $name {
            $($variant,)*
            /// A value KPP doesn't name, as written.
            Other(String),
        }

        impl $name {
            pub fn from_wire(s: &str) -> Self {
                match s {
                    $($wire => $name::$variant,)*
                    other => $name::Other(other.to_owned()),
                }
            }

            pub fn as_wire(&self) -> &str {
                match self {
                    $($name::$variant => $wire,)*
                    $name::Other(s) => s,
                }
            }
        }

        impl Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(self.as_wire())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let s = String::deserialize(d)?;
                Ok($name::from_wire(&s))
            }
        }
    };
}

wire_enum!(
    /// An image size: `auto`, or `WIDTHxHEIGHT` in pixels.
    ImageSize { Auto = "auto" }
);

impl ImageSize {
    /// A size of `width` by `height` pixels.
    pub fn pixels(width: u32, height: u32) -> Self {
        ImageSize::Other(format!("{width}x{height}"))
    }

    /// The width and height of a `WIDTHxHEIGHT` size.
    pub fn dimensions(&self) -> Option<(u32, u32)> {
        let ImageSize::Other(s) = self else {
            return None;
        };
        let (w, h) = s.split_once('x')?;
        let digits = |t: &str| !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit());
        if !digits(w) || !digits(h) {
            return None;
        }
        Some((w.parse().ok()?, h.parse().ok()?))
    }
}

wire_enum!(
    /// How much effort the provider spends on each image.
    ImageQuality {
        Auto = "auto",
        Low = "low",
        Medium = "medium",
        High = "high",
        XHigh = "xhigh",
        Max = "max",
        Standard = "standard",
        Hd = "hd",
    }
);

wire_enum!(
    /// The file format of generated images.
    ImageFormat {
        Png = "png",
        Jpeg = "jpeg",
        Webp = "webp",
        Gif = "gif",
        Svg = "svg",
    }
);

impl ImageFormat {
    /// The format's media type, such as `image/png`.
    pub fn media_type(&self) -> Option<&'static str> {
        Some(match self {
            ImageFormat::Png => "image/png",
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Webp => "image/webp",
            ImageFormat::Gif => "image/gif",
            ImageFormat::Svg => "image/svg+xml",
            ImageFormat::Other(_) => return None,
        })
    }

    /// The format of a media type, such as `image/jpeg`.
    pub fn from_media_type(media_type: &str) -> Option<Self> {
        let essence = media_type.split(';').next()?.trim().to_ascii_lowercase();
        Some(match essence.as_str() {
            "image/png" => ImageFormat::Png,
            "image/jpeg" | "image/jpg" => ImageFormat::Jpeg,
            "image/webp" => ImageFormat::Webp,
            "image/gif" => ImageFormat::Gif,
            "image/svg+xml" => ImageFormat::Svg,
            _ => return None,
        })
    }
}

wire_enum!(
    /// The background of generated images.
    ImageBackground {
        Auto = "auto",
        Transparent = "transparent",
        Opaque = "opaque",
    }
);

/// Whether a reply carries an image's bytes or a link to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ImageDelivery {
    /// A link the provider hosts for a while.
    Url,
    /// The bytes, base64-encoded.
    Base64,
}

/// The settings a provider reports it used, for all images of a reply.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ImageSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<ImageSize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<ImageQuality>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<ImageFormat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<ImageBackground>,
}

impl ImageSettings {
    pub fn is_empty(&self) -> bool {
        self == &ImageSettings::default()
    }
}

/// The reply to an image request.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ImageResponse {
    pub model: String,
    /// When the images were made, in Unix seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<u64>,
    pub images: Vec<GeneratedImage>,
    #[serde(default, skip_serializing_if = "ImageSettings::is_empty")]
    pub settings: ImageSettings,
    pub usage: Usage,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl ImageResponse {
    pub fn new(model: impl Into<String>, images: Vec<GeneratedImage>) -> Self {
        ImageResponse {
            model: model.into(),
            images,
            ..ImageResponse::default()
        }
    }
}

/// One image of a reply: its bytes or a link, and the prompt the provider actually used.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GeneratedImage {
    pub source: Source,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revised_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl GeneratedImage {
    pub fn new(source: Source) -> Self {
        GeneratedImage {
            source,
            revised_prompt: None,
            extras: Extras::default(),
        }
    }
}

/// One event of a streamed image reply: a partial image, or a finished one with its usage.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ImageEvent {
    /// The partial image's place among the partial images, counted from 0; `None` for a
    /// finished image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partial: Option<u32>,
    pub image: GeneratedImage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<u64>,
    #[serde(default, skip_serializing_if = "ImageSettings::is_empty")]
    pub settings: ImageSettings,
    /// The usage, on a finished image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl ImageEvent {
    pub fn partial(index: u32, image: GeneratedImage) -> Self {
        ImageEvent {
            partial: Some(index),
            image,
            created: None,
            settings: ImageSettings::default(),
            usage: None,
            extras: Extras::default(),
        }
    }

    pub fn finished(image: GeneratedImage, usage: Option<Usage>) -> Self {
        ImageEvent {
            partial: None,
            image,
            created: None,
            settings: ImageSettings::default(),
            usage,
            extras: Extras::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_values_round_trip_and_keep_unknown_ones() {
        for s in [
            "auto", "low", "medium", "high", "xhigh", "max", "standard", "hd", "ultra",
        ] {
            assert_eq!(ImageQuality::from_wire(s).as_wire(), s);
        }
        for s in ["png", "jpeg", "webp", "gif", "svg", "avif"] {
            assert_eq!(ImageFormat::from_wire(s).as_wire(), s);
        }
        assert_eq!(
            serde_json::to_string(&ImageBackground::Transparent).unwrap(),
            "\"transparent\""
        );
        let back: ImageQuality = serde_json::from_str("\"ultra\"").unwrap();
        assert_eq!(back, ImageQuality::Other("ultra".into()));
    }

    #[test]
    fn sizes_tell_their_dimensions() {
        assert_eq!(
            ImageSize::pixels(1536, 1024).dimensions(),
            Some((1536, 1024))
        );
        assert_eq!(
            ImageSize::from_wire("1024x1024").dimensions(),
            Some((1024, 1024))
        );
        assert_eq!(ImageSize::Auto.dimensions(), None);
        for bad in ["1024", "x1024", "1024x", "1024*1024", "+1x2", "1Kx2K"] {
            assert_eq!(ImageSize::from_wire(bad).dimensions(), None, "{bad}");
        }
    }

    #[test]
    fn formats_know_their_media_types() {
        assert_eq!(ImageFormat::Jpeg.media_type(), Some("image/jpeg"));
        assert_eq!(
            ImageFormat::from_media_type("image/jpg"),
            Some(ImageFormat::Jpeg)
        );
        assert_eq!(
            ImageFormat::from_media_type("IMAGE/PNG; q=1"),
            Some(ImageFormat::Png)
        );
        assert_eq!(ImageFormat::from_media_type("image/avif"), None);
        assert_eq!(ImageFormat::Other("avif".into()).media_type(), None);
    }

    #[test]
    fn requests_need_what_their_operation_needs() {
        let ok = ImageRequest::new("m", ImageOperation::Generate).with_prompt("fox");
        ok.validate().unwrap();
        let param = |r: &ImageRequest| r.validate().unwrap_err().param.unwrap();
        assert_eq!(
            param(&ImageRequest::new("", ImageOperation::Generate)),
            "model"
        );
        assert_eq!(
            param(&ImageRequest::new("m", ImageOperation::Generate)),
            "prompt"
        );
        let png = || ImageInput::new(Source::base64("image/png", "AA=="));
        assert_eq!(param(&ok.clone().with_image(png())), "image");
        let edit = ImageRequest::new("m", ImageOperation::Edit).with_prompt("hat");
        assert_eq!(param(&edit), "image");
        edit.clone().with_image(png()).validate().unwrap();
        let vary = ImageRequest::new("m", ImageOperation::Variation).with_image(png());
        vary.validate().unwrap();
        assert_eq!(param(&vary.clone().with_image(png())), "image");
        let mut many = ok.clone();
        many.count = Some(0);
        assert_eq!(param(&many), "n");
        let mut squeezed = ok.clone();
        squeezed.compression = Some(101);
        assert_eq!(param(&squeezed), "output_compression");
        let caps = edit.with_image(png()).required_capabilities();
        assert!(caps.images && caps.image_output && !caps.streaming);
    }

    #[test]
    fn a_minimal_request_has_a_small_stable_form() {
        let req = ImageRequest::new("gpt-image-2", ImageOperation::Generate).with_prompt("a fox");
        let json = serde_json::to_string(&req).unwrap();
        assert_eq!(
            json,
            r#"{"model":"gpt-image-2","operation":"generate","prompt":"a fox"}"#
        );
        let back: ImageRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back, req);
    }
}
