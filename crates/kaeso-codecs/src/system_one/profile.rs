/// Where a System One provider takes the images a request sends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MediaPlacement {
    /// The top-level `assets` map, each image written by its reference wherever it belongs in the
    /// state, the instructions and the criteria, as TypeSafe takes them.
    Assets,
    /// OpenAI-style `image_url` parts in the state, where the reference stood, as Perplexity takes
    /// them. A reference elsewhere, or inside a longer string, can't be sent.
    State,
    /// `image_url` parts where the app wrote parts and the `assets` map for the other images, as
    /// OpenRouter takes both.
    StateOrAssets,
    /// `image_url` parts in a user message under `state.messages`, as Baseten takes them.
    Conversation,
    /// A top-level `images` list of data URLs, as Cloudflare Workers AI, Liquid, vLLM and
    /// llama.cpp take them; each reference must be a whole item of a state array.
    Root,
    /// No images.
    None,
}

/// How a provider wraps its reply.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Envelope {
    /// The reply is the System One response.
    None,
    /// The response is the `result` of Cloudflare's API envelope.
    Result,
}

/// How a provider names the model in the body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ModelField {
    /// The model id as the request names it.
    AsSent,
    /// The id without this prefix, as Cloudflare's `clef` for `@cf/cloudflare/clef`.
    ShortName(&'static str),
}

/// Which question keys a provider takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum IdRule {
    /// Any string.
    Any,
    /// 1 to `max_len` ASCII letters, digits and the characters of `punctuation`.
    Word {
        max_len: usize,
        punctuation: &'static str,
    },
}

impl IdRule {
    pub fn allows(self, key: &str) -> bool {
        match self {
            IdRule::Any => true,
            IdRule::Word {
                max_len,
                punctuation,
            } => {
                !key.is_empty()
                    && key.len() <= max_len
                    && key
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || punctuation.contains(c))
            }
        }
    }
}

/// How a provider that speaks a form of TypeSafe's System One API takes decision requests: its
/// path, where it wants images, which question types and image formats it takes, and its limits.
/// A request the provider can't take is refused before it is sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct SystemOneProfile {
    /// The provider's name in refusals.
    pub name: &'static str,
    /// The request path; `{model}` stands for the model the body names. A `/v1/…` path goes
    /// under the context's API root.
    pub path: &'static str,
    pub media: MediaPlacement,
    pub envelope: Envelope,
    pub model_field: ModelField,
    /// Whether requests with images or box questions carry `TypeSafe-Danger-Zone-Images: true`.
    pub images_header: bool,
    /// Whether images an app uploaded as `multipart/form-data` parts are sent as parts again.
    pub multipart: bool,
    /// Whether the provider answers box questions.
    pub box_questions: bool,
    /// Whether the provider takes top-level fields it doesn't document, which an app's own fields
    /// are sent as.
    pub unknown_fields: bool,
    pub max_questions: Option<usize>,
    /// The most options one choice question may have.
    pub max_options: Option<usize>,
    /// The most levels one score question may have.
    pub max_levels: Option<usize>,
    /// The most images one request may show, counting each place an image is used.
    pub max_images: Option<usize>,
    /// The image media types the provider takes; empty for any.
    pub image_types: &'static [&'static str],
    /// The fewest and the most pixels an image's width and height may each have.
    pub image_px: Option<(u32, u32)>,
    /// The most bytes one image may have.
    pub image_bytes: Option<usize>,
    /// Whether the provider takes an animated image.
    pub animated_images: bool,
    /// Whether the provider fetches an image from an `https` URL, rather than only taking bytes.
    pub image_urls: bool,
    pub question_ids: IdRule,
}

const PNG_JPEG: &[&str] = &["image/png", "image/jpeg"];
const PNG_JPEG_WEBP: &[&str] = &["image/png", "image/jpeg", "image/webp"];
const MIB: usize = 1024 * 1024;

impl SystemOneProfile {
    /// TypeSafe (`api.typesafe.ai`): images as `assets`, uploaded parts kept, box questions, PNG
    /// and JPEG of 280 to 1568 pixels a side and at most 16 MiB, never resized
    /// (<https://docs.typesafe.ai/images#limits>).
    pub const TYPESAFE: SystemOneProfile = SystemOneProfile {
        name: "TypeSafe",
        path: "/v1/systemone",
        media: MediaPlacement::Assets,
        envelope: Envelope::None,
        model_field: ModelField::AsSent,
        images_header: true,
        multipart: true,
        box_questions: true,
        unknown_fields: true,
        max_questions: None,
        max_options: Some(255),
        max_levels: Some(10),
        max_images: None,
        image_types: PNG_JPEG,
        image_px: Some((280, 1568)),
        image_bytes: Some(16 * MIB),
        animated_images: false,
        image_urls: false,
        question_ids: IdRule::Any,
    };

    /// OpenRouter (`/api/v1/systemone`): image parts in the state or `assets` without uploads; box
    /// questions for the models whose provider answers them.
    pub const OPENROUTER: SystemOneProfile = SystemOneProfile {
        name: "OpenRouter",
        multipart: false,
        images_header: false,
        media: MediaPlacement::StateOrAssets,
        image_types: &[],
        image_px: None,
        image_bytes: None,
        animated_images: true,
        ..SystemOneProfile::TYPESAFE
    };

    /// Perplexity (`/v1/decisions`, with a System One body): image parts anywhere in the state,
    /// PNG, JPEG and WebP data URLs, at most 8 images, no box questions, and no top-level field it
    /// doesn't document (<https://docs.perplexity.ai/docs/decisions/quickstart>).
    pub const PERPLEXITY: SystemOneProfile = SystemOneProfile {
        name: "Perplexity",
        path: "/v1/decisions",
        media: MediaPlacement::State,
        images_header: false,
        multipart: false,
        box_questions: false,
        unknown_fields: false,
        max_images: Some(8),
        image_types: PNG_JPEG_WEBP,
        image_px: None,
        image_bytes: None,
        animated_images: true,
        ..SystemOneProfile::TYPESAFE
    };

    /// Baseten (`/v1/systemone`): images as parts of a user message under `state.messages`,
    /// PNG, JPEG and WebP, at most 16, no box questions
    /// (<https://docs.baseten.co/reference/inference-api/system-one>).
    pub const BASETEN: SystemOneProfile = SystemOneProfile {
        name: "Baseten",
        media: MediaPlacement::Conversation,
        images_header: false,
        multipart: false,
        box_questions: false,
        max_images: Some(16),
        image_types: PNG_JPEG_WEBP,
        image_px: None,
        image_bytes: None,
        animated_images: true,
        ..SystemOneProfile::TYPESAFE
    };

    /// Fireworks AI (`/inference/v1/systemone`): text only, no box questions.
    pub const FIREWORKS: SystemOneProfile = SystemOneProfile {
        name: "Fireworks AI",
        media: MediaPlacement::None,
        images_header: false,
        multipart: false,
        box_questions: false,
        ..SystemOneProfile::TYPESAFE
    };

    /// Cloudflare Workers AI's Clef models (`/ai/run/@cf/cloudflare/{model}`): the short model
    /// name in the body, at most 64 questions with ids of letters, digits, `_`, `.` and `-`, and at
    /// most 4 PNG, JPEG or WebP images of 4 MiB in a top-level `images` list; the reply is the
    /// `result` of Cloudflare's envelope (<https://developers.cloudflare.com/workers-ai/models/clef/>).
    pub const CLOUDFLARE_WORKERS_AI: SystemOneProfile = SystemOneProfile {
        name: "Cloudflare Workers AI",
        path: "/ai/run/@cf/cloudflare/{model}",
        media: MediaPlacement::Root,
        envelope: Envelope::Result,
        model_field: ModelField::ShortName("@cf/cloudflare/"),
        images_header: false,
        multipart: false,
        box_questions: false,
        unknown_fields: true,
        max_questions: Some(64),
        max_options: Some(255),
        max_levels: Some(10),
        max_images: Some(4),
        image_types: PNG_JPEG_WEBP,
        image_px: None,
        image_bytes: Some(4 * MIB),
        animated_images: true,
        image_urls: false,
        question_ids: IdRule::Word {
            max_len: 100,
            punctuation: "_.-",
        },
    };

    /// Any other server of the format, such as vLLM, llama.cpp or a custom provider: images as
    /// `assets`, every question type, image URLs where its placement can write them, and no limits
    /// Kaeso checks; its admin sets the path through
    /// [`crate::EgressCtx::with_base_path`] and the image placement with
    /// [`SystemOneProfile::with_media`].
    pub const GENERIC: SystemOneProfile = SystemOneProfile {
        name: "The provider",
        max_options: None,
        max_levels: None,
        images_header: false,
        multipart: false,
        image_types: &[],
        image_px: None,
        image_bytes: None,
        animated_images: true,
        image_urls: true,
        ..SystemOneProfile::TYPESAFE
    };

    /// The profile with images placed as `media` says.
    #[must_use]
    pub const fn with_media(mut self, media: MediaPlacement) -> Self {
        self.media = media;
        self
    }

    /// The profile sending requests to `path` instead.
    #[must_use]
    pub const fn with_path(mut self, path: &'static str) -> Self {
        self.path = path;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_rules_allow_what_their_providers_take() {
        assert!(IdRule::Any.allows(""));
        let cf = SystemOneProfile::CLOUDFLARE_WORKERS_AI.question_ids;
        assert!(cf.allows("is_bug.v2-a"));
        assert!(!cf.allows(""));
        assert!(!cf.allows("has space"));
        assert!(!cf.allows(&"x".repeat(101)));
        assert!(cf.allows(&"x".repeat(100)));
    }

    #[test]
    fn profiles_differ_where_their_providers_do() {
        let generic = SystemOneProfile::GENERIC
            .with_media(MediaPlacement::Root)
            .with_path("/decisions/v1/systemone");
        assert_eq!(generic.media, MediaPlacement::Root);
        assert_eq!(generic.path, "/decisions/v1/systemone");
        let flags = |p: SystemOneProfile| (p.images_header, p.unknown_fields, p.box_questions);
        assert_eq!(flags(SystemOneProfile::TYPESAFE), (true, true, true));
        assert_eq!(flags(SystemOneProfile::PERPLEXITY), (false, false, false));
        assert_eq!(SystemOneProfile::PERPLEXITY.path, "/v1/decisions");
        assert_eq!(SystemOneProfile::FIREWORKS.media, MediaPlacement::None);
        assert_eq!(SystemOneProfile::BASETEN.max_images, Some(16));
    }
}
