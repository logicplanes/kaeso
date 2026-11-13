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
    /// The id without this prefix, as Cloudflare's `clef` for `@cf/cloudflare/clef`; a reply
    /// that names the short id gets the prefix back.
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

/// The context a provider gives a request unless it asks for long context, in tokens: the state
/// and every question together, and the state with any one question.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ContextLimits {
    pub request: u64,
    pub question: u64,
}

/// How a provider that speaks a form of TypeSafe's System One API takes decision requests: its
/// path, where it wants images, which question types and image formats it takes, and its limits.
/// A request the provider can't take is refused before it is sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct SystemOneProfile {
    /// The provider's name in refusals.
    pub name: &'static str,
    /// The request path; `{model}` stands for the model the body names. With an API root
    /// ([`crate::EgressCtx::api_root`]), a `/v1/…` path goes under it and any other path beside
    /// it, under the root without its `/v1`.
    pub path: &'static str,
    pub media: MediaPlacement,
    pub envelope: Envelope,
    pub model_field: ModelField,
    /// Whether requests with images or box questions carry `TypeSafe-Danger-Zone-Images: true`.
    pub images_header: bool,
    /// The context a request gets without `TypeSafe-Danger-Zone-Long-Context: true`; a request
    /// estimated beyond it is sent with that header, named in `adapted` as `long_context`.
    /// `None` for a provider without the header.
    pub long_context: Option<ContextLimits>,
    /// Whether images an app uploaded as `multipart/form-data` parts are sent as parts again.
    pub multipart: bool,
    /// Whether the provider answers box questions.
    pub box_questions: bool,
    /// Whether the provider takes top-level fields it doesn't document, which an app's own fields
    /// are sent as.
    pub unknown_fields: bool,
    pub max_questions: Option<usize>,
    /// The fewest options one choice question may have.
    pub min_options: usize,
    /// The most options one choice question may have.
    pub max_options: Option<usize>,
    /// The fewest levels one score question may have.
    pub min_levels: usize,
    /// The most levels one score question may have.
    pub max_levels: Option<usize>,
    /// Whether every question must have instructions.
    pub instructions_required: bool,
    /// The most images one request may show, counting each place an image is used.
    pub max_images: Option<usize>,
    /// The image media types the provider takes; empty for any.
    pub image_types: &'static [&'static str],
    /// The fewest and the most pixels an image's width and height may each have.
    pub image_px: Option<(u32, u32)>,
    /// The most bytes one image may have.
    pub image_bytes: Option<usize>,
    /// The most pixels one image may have, its width times its height.
    pub image_pixels: Option<u64>,
    /// The most bytes all images of one request may have together.
    pub images_bytes: Option<usize>,
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
    /// (<https://docs.typesafe.ai/images#limits>), and 64k tokens a request and 32k a question
    /// without the long-context header (<https://docs.typesafe.ai/models#context-length>).
    pub const TYPESAFE: SystemOneProfile = SystemOneProfile {
        name: "TypeSafe",
        path: "/v1/systemone",
        media: MediaPlacement::Assets,
        envelope: Envelope::None,
        model_field: ModelField::AsSent,
        images_header: true,
        long_context: Some(ContextLimits {
            request: 64 * 1024,
            question: 32 * 1024,
        }),
        multipart: true,
        box_questions: true,
        unknown_fields: true,
        max_questions: None,
        min_options: 1,
        max_options: Some(255),
        min_levels: 1,
        max_levels: Some(10),
        instructions_required: false,
        max_images: None,
        image_types: PNG_JPEG,
        image_px: Some((280, 1568)),
        image_bytes: Some(16 * MIB),
        image_pixels: None,
        images_bytes: None,
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
        long_context: None,
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
        long_context: None,
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
        long_context: None,
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
        long_context: None,
        multipart: false,
        box_questions: false,
        ..SystemOneProfile::TYPESAFE
    };

    /// Cloudflare Workers AI's Clef models (`/run/@cf/cloudflare/{model}` beside the account's
    /// `/v1`, which is `/client/v4/accounts/{account_id}/ai/run/@cf/cloudflare/{model}`): the
    /// short model name in the body, 1 to 64 questions with instructions and ids of letters,
    /// digits, `_`, `.` and `-`, 2 to 255 options and 2 to 10 levels, at most 4 PNG, JPEG or WebP
    /// images of 4 MiB and 16 megapixels in a top-level `images` list, 8 MiB together, and no
    /// field it doesn't document; the reply is the `result` of Cloudflare's envelope
    /// (<https://developers.cloudflare.com/workers-ai/models/clef/>, `schema-input.json`).
    pub const CLOUDFLARE_WORKERS_AI: SystemOneProfile = SystemOneProfile {
        name: "Cloudflare Workers AI",
        path: "/run/@cf/cloudflare/{model}",
        media: MediaPlacement::Root,
        envelope: Envelope::Result,
        model_field: ModelField::ShortName("@cf/cloudflare/"),
        images_header: false,
        long_context: None,
        multipart: false,
        box_questions: false,
        unknown_fields: false,
        max_questions: Some(64),
        min_options: 2,
        max_options: Some(255),
        min_levels: 2,
        max_levels: Some(10),
        instructions_required: true,
        max_images: Some(4),
        image_types: PNG_JPEG_WEBP,
        image_px: None,
        image_bytes: Some(4 * MIB),
        image_pixels: Some(16_000_000),
        images_bytes: Some(8 * MIB),
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
        long_context: None,
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
        let rules = |p: SystemOneProfile| {
            (
                p.min_options,
                p.min_levels,
                p.instructions_required,
                p.unknown_fields,
            )
        };
        assert_eq!(
            rules(SystemOneProfile::CLOUDFLARE_WORKERS_AI),
            (2, 2, true, false)
        );
        assert_eq!(rules(SystemOneProfile::GENERIC), (1, 1, false, true));
    }
}
