/// What a provider that speaks a form of OpenAI's Images API takes: where it serves
/// generations and edits, how it spells sizes, image inputs and the reply's delivery, and
/// which of OpenAI's fields it lacks. [`ImagesProfile::OPENAI`] is OpenAI itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ImagesProfile {
    /// The provider's name in error messages.
    pub provider: &'static str,
    /// The generations path, after the provider's base URL.
    pub generations_path: &'static str,
    pub edits: EditsForm,
    pub sizes: SizeForm,
    pub delivery: DeliveryForm,
    /// Whether an edit takes a mask.
    pub mask: bool,
    /// Whether generations stream partial images.
    pub streams: bool,
    /// Whether edits stream partial images.
    pub edits_stream: bool,
    /// The most images one request may ask for, when the provider documents a limit.
    pub max_count: Option<u32>,
    /// Whether the provider takes `n` at all; one that always makes one image doesn't.
    pub count: bool,
    pub user: UserField,
    /// The `quality` values the provider documents, or every value.
    pub qualities: Option<&'static [&'static str]>,
    /// The `output_format` values the provider documents, or every value.
    pub formats: Option<&'static [&'static str]>,
    /// Whether the reply's images arrive only as links.
    pub links_only: bool,
    /// Whether the provider ends a stream with `data: [DONE]` and names no events.
    pub data_only_stream: bool,
    /// Kaeso-typed fields from other providers' forms that this one takes:
    /// `aspect_ratio`, `resolution`, `seed` and `negative_prompt`.
    pub extensions: &'static [&'static str],
    /// OpenAI's fields the provider lacks, refused before sending.
    pub refused_fields: &'static [&'static str],
}

/// Where and how a provider takes the images of an edit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum EditsForm {
    /// OpenAI's edits endpoint: uploads as `multipart/form-data`, or JSON `images` of
    /// `{image_url}` / `{file_id}`.
    Endpoint { path: &'static str, file_ids: bool },
    /// An edits endpoint that takes only JSON: `image` (one) or `images` (several) of
    /// `{url}` objects, each a link or a data URL.
    UrlObjects { path: &'static str, max: u32 },
    /// Edits go to the generations endpoint, with the images in `field` as links or data URLs.
    OnGenerations {
        field: ImageField,
        max: u32,
        data_urls: bool,
    },
    /// The provider doesn't edit images.
    None,
}

/// The field that carries an edit's images on a generations request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ImageField {
    /// `image`: one string, or a list of strings.
    Image,
    /// `image_url`: one string.
    ImageUrl,
    /// `input_references`: a list of `{type: "image_url", image_url: {url}}`.
    InputReferences,
}

/// How a provider takes the output size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SizeForm {
    /// `size: "WIDTHxHEIGHT"` or `auto`.
    Size,
    /// `width` and `height` in pixels.
    WidthHeight,
    /// No pixel sizes: `aspect_ratio` and `resolution` only.
    AspectRatio,
}

/// How a provider spells the reply's delivery.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DeliveryForm {
    /// `response_format: "url" | "b64_json"`.
    ResponseFormat,
    /// `response_format: "url" | "base64"`.
    Base64Word,
    /// No such field: the provider always answers one way.
    Fixed,
}

/// Where a provider takes the end user's identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum UserField {
    /// `safety_identifier`, with the deprecated `user`.
    SafetyIdentifier,
    /// `user`.
    User,
    /// `user_id`.
    UserId,
    /// None: the identifier is left out.
    None,
}

impl ImagesProfile {
    /// OpenAI, from its OpenAPI document at commit `926a7b8`.
    pub const OPENAI: ImagesProfile = ImagesProfile {
        provider: "OpenAI",
        generations_path: "/v1/images/generations",
        edits: EditsForm::Endpoint {
            path: "/v1/images/edits",
            file_ids: true,
        },
        sizes: SizeForm::Size,
        delivery: DeliveryForm::ResponseFormat,
        mask: true,
        streams: true,
        edits_stream: true,
        max_count: Some(10),
        count: true,
        user: UserField::SafetyIdentifier,
        qualities: None,
        formats: None,
        links_only: false,
        data_only_stream: false,
        extensions: &[],
        refused_fields: &[],
    };

    /// Vercel's AI Gateway: OpenAI's endpoints; edits don't stream or take file ids
    /// (<https://vercel.com/docs/ai-gateway/capabilities/image-generation/openai>).
    pub const VERCEL: ImagesProfile = ImagesProfile {
        provider: "Vercel",
        edits: EditsForm::Endpoint {
            path: "/v1/images/edits",
            file_ids: false,
        },
        streams: false,
        edits_stream: false,
        max_count: None,
        user: UserField::User,
        ..ImagesProfile::OPENAI
    };

    /// SpaceX's Grok Imagine: aspect ratio and resolution instead of sizes, JSON edits of up
    /// to five images without a mask, and links by default
    /// (<https://docs.x.ai/developers/rest-api-reference/inference/images>).
    pub const XAI: ImagesProfile = ImagesProfile {
        provider: "SpaceX",
        generations_path: "/v1/images/generations",
        edits: EditsForm::UrlObjects {
            path: "/v1/images/edits",
            max: 5,
        },
        sizes: SizeForm::AspectRatio,
        delivery: DeliveryForm::ResponseFormat,
        mask: false,
        streams: false,
        edits_stream: false,
        max_count: Some(10),
        count: true,
        user: UserField::User,
        qualities: Some(&["low", "medium", "auto"]),
        formats: Some(&[]),
        links_only: false,
        data_only_stream: false,
        extensions: &["aspect_ratio", "resolution"],
        refused_fields: &[
            "background",
            "output_compression",
            "moderation",
            "style",
            "input_fidelity",
            "partial_images",
            "frame_count",
            "frame_duration_ms",
            "complexity",
            "svg",
            "markup",
        ],
    };

    /// Together AI: width and height, `base64` for bytes, one edited image by link on the
    /// generations endpoint (<https://docs.together.ai/reference/post-images-generations>).
    pub const TOGETHER: ImagesProfile = ImagesProfile {
        provider: "Together AI",
        generations_path: "/v1/images/generations",
        edits: EditsForm::OnGenerations {
            field: ImageField::ImageUrl,
            max: 1,
            data_urls: false,
        },
        sizes: SizeForm::WidthHeight,
        delivery: DeliveryForm::Base64Word,
        mask: false,
        streams: false,
        edits_stream: false,
        max_count: Some(4),
        count: true,
        user: UserField::None,
        qualities: Some(&[]),
        formats: Some(&["jpeg", "png"]),
        links_only: false,
        data_only_stream: false,
        extensions: &["aspect_ratio", "seed", "negative_prompt"],
        refused_fields: &[
            "background",
            "output_compression",
            "moderation",
            "style",
            "input_fidelity",
            "partial_images",
            "frame_count",
            "frame_duration_ms",
            "complexity",
            "svg",
            "markup",
        ],
    };

    /// QwenCloud's OpenAI-compatible mode: generations only, edits through `image` on the
    /// same endpoint, links valid for 24 hours
    /// (<https://docs.qwencloud.com/api-reference/image-generation/qwen-text-to-image-openai>).
    pub const QWENCLOUD: ImagesProfile = ImagesProfile {
        provider: "QwenCloud",
        generations_path: "/v1/images/generations",
        edits: EditsForm::OnGenerations {
            field: ImageField::Image,
            max: 10,
            data_urls: true,
        },
        sizes: SizeForm::Size,
        delivery: DeliveryForm::Fixed,
        mask: false,
        streams: false,
        edits_stream: false,
        max_count: Some(6),
        count: true,
        user: UserField::None,
        qualities: Some(&[]),
        formats: Some(&[]),
        links_only: true,
        data_only_stream: false,
        extensions: &["seed", "negative_prompt"],
        refused_fields: &[
            "background",
            "output_compression",
            "moderation",
            "style",
            "input_fidelity",
            "partial_images",
            "frame_count",
            "frame_duration_ms",
            "complexity",
            "svg",
            "markup",
        ],
    };

    /// Z.ai's GLM-Image: one image per request, `hd` or `standard`, links valid for 30 days,
    /// served from the root of the host (<https://docs.z.ai/api-reference/image/generate-image>).
    pub const ZAI: ImagesProfile = ImagesProfile {
        provider: "Z.ai",
        generations_path: "/api/paas/v4/images/generations",
        edits: EditsForm::None,
        sizes: SizeForm::Size,
        delivery: DeliveryForm::Fixed,
        mask: false,
        streams: false,
        edits_stream: false,
        max_count: Some(1),
        count: false,
        user: UserField::UserId,
        qualities: Some(&["hd", "standard"]),
        formats: Some(&[]),
        links_only: true,
        data_only_stream: false,
        extensions: &[],
        refused_fields: &[
            "background",
            "output_compression",
            "moderation",
            "style",
            "input_fidelity",
            "partial_images",
            "frame_count",
            "frame_duration_ms",
            "complexity",
            "svg",
            "markup",
        ],
    };

    /// OpenRouter's image API at `/api/v1/images`: OpenAI's fields with aspect ratio,
    /// resolution and reference images, bytes only, and data-only streams
    /// (<https://openrouter.ai/docs/guides/overview/multimodal/image-generation>).
    pub const OPENROUTER: ImagesProfile = ImagesProfile {
        provider: "OpenRouter",
        generations_path: "/v1/images",
        edits: EditsForm::OnGenerations {
            field: ImageField::InputReferences,
            max: 16,
            data_urls: true,
        },
        sizes: SizeForm::Size,
        delivery: DeliveryForm::Fixed,
        mask: false,
        streams: true,
        edits_stream: true,
        max_count: Some(10),
        count: true,
        user: UserField::User,
        qualities: None,
        formats: None,
        links_only: false,
        data_only_stream: true,
        extensions: &["aspect_ratio", "resolution", "seed"],
        refused_fields: &["moderation", "style", "input_fidelity", "complexity", "svg"],
    };

    /// A provider that names itself `provider` and otherwise speaks OpenAI's Images API.
    pub const fn compatible(provider: &'static str) -> ImagesProfile {
        ImagesProfile {
            provider,
            ..ImagesProfile::OPENAI
        }
    }

    pub(crate) fn takes_quality(&self, value: &str) -> bool {
        self.qualities.is_none_or(|q| q.contains(&value))
    }

    pub(crate) fn takes_format(&self, value: &str) -> bool {
        self.formats.is_none_or(|f| f.contains(&value))
    }

    pub(crate) fn extends(&self, field: &str) -> bool {
        self.extensions.contains(&field)
    }

    pub(crate) fn refuses(&self, field: &str) -> bool {
        self.refused_fields.contains(&field)
    }

    pub(crate) fn edits_path(&self) -> Option<&'static str> {
        match self.edits {
            EditsForm::Endpoint { path, .. } | EditsForm::UrlObjects { path, .. } => Some(path),
            EditsForm::OnGenerations { .. } => Some(self.generations_path),
            EditsForm::None => None,
        }
    }
}
