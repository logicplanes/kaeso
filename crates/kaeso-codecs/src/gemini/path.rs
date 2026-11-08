use std::borrow::Cow;
use std::fmt::Write as _;

use kaeso_kpp::{DialectId, KppError, RequestKind};

/// Where a Gemini-format provider serves its models, and the wire switches it needs.
///
/// Google AI Studio serves `POST /v1beta/models/{model}:generateContent`; a platform that
/// names the project and region in the path sets its own prefix with
/// [`with_models_path`](Self::with_models_path) for each request.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct GeminiProfile {
    /// The path the model ids follow, without a trailing slash, such as `/v1beta/models`.
    pub models_path: Cow<'static, str>,
    /// Whether every content of a request must name its `role`; a content that the app sent
    /// without one gets `user`.
    pub role_on_every_content: bool,
}

impl GeminiProfile {
    /// Google AI Studio, `https://generativelanguage.googleapis.com/v1beta/models`.
    pub const GOOGLE_AI_STUDIO: GeminiProfile = GeminiProfile {
        models_path: Cow::Borrowed("/v1beta/models"),
        role_on_every_content: false,
    };

    /// The profile with another models path, such as
    /// `/v1/projects/{project}/locations/{location}/publishers/google/models`.
    #[must_use]
    pub fn with_models_path(mut self, path: impl Into<Cow<'static, str>>) -> Self {
        let path: Cow<'static, str> = path.into();
        self.models_path = match path.strip_suffix('/') {
            Some(trimmed) => Cow::Owned(trimmed.to_owned()),
            None => path,
        };
        self
    }

    /// The profile with `role` written on every content.
    #[must_use]
    pub fn with_role_on_every_content(mut self, required: bool) -> Self {
        self.role_on_every_content = required;
        self
    }

    /// The path of `action` on `model`, such as
    /// `/v1beta/models/gemini-3.5-flash:streamGenerateContent?alt=sse`.
    ///
    /// The model id is one percent-encoded path segment. A leading `models/` is the resource
    /// name of the same model and is removed; `tunedModels/<id>` addresses a tuned model next
    /// to the models path. The ids `.` and `..` are refused, because URL parsers drop such
    /// segments even when they are percent-encoded.
    pub fn path(&self, model: &str, action: GeminiAction) -> Result<String, KppError> {
        let mut path = self.model_path(model)?;
        path.push(':');
        path.push_str(action.as_str());
        if action == GeminiAction::StreamGenerateContent {
            path.push_str("?alt=sse");
        }
        Ok(path)
    }

    /// The path of `GET` for one model, such as `/v1beta/models/gemini-3.5-flash`.
    pub fn model_path(&self, model: &str) -> Result<String, KppError> {
        let (base, id) = match model.strip_prefix(TUNED_PREFIX) {
            Some(id) => {
                let parent = self.models_path.strip_suffix("/models").ok_or_else(|| {
                    KppError::invalid_request(format!(
                        "The model `{model}` is a tuned model, which this provider doesn't serve; use a base model's id."
                    ))
                    .with_param("model")
                    .with_status(400)
                })?;
                (format!("{parent}/tunedModels"), id)
            }
            None => (
                self.models_path.clone().into_owned(),
                model.strip_prefix(MODELS_PREFIX).unwrap_or(model),
            ),
        };
        if id.is_empty() {
            return Err(KppError::invalid_request(
                "The request names no model; name the model to send it to.",
            )
            .with_param("model")
            .with_status(400));
        }
        // A URL parser removes a `.` or `..` path segment even when it is percent-encoded
        // (WHATWG URL, "single-dot" and "double-dot" segments), so no encoding keeps them.
        if matches!(id, "." | "..") {
            return Err(KppError::invalid_request(format!(
                "The model id `{model}` can't be part of a URL; use the model's real id."
            ))
            .with_param("model")
            .with_status(400));
        }
        let mut path = base;
        path.push('/');
        for b in id.bytes() {
            // `:` separates the model from the method in Gemini paths, so it is encoded too.
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b'@') {
                path.push(char::from(b));
            } else {
                let _ = write!(path, "%{b:02X}");
            }
        }
        Ok(path)
    }

    /// The path of the model listing, such as `/v1beta/models`.
    pub fn list_path(&self) -> &str {
        &self.models_path
    }
}

impl Default for GeminiProfile {
    fn default() -> Self {
        GeminiProfile::GOOGLE_AI_STUDIO
    }
}

const MODELS_PREFIX: &str = "models/";
const TUNED_PREFIX: &str = "tunedModels/";

/// The method a Gemini request calls on a model, the part of the path after the colon.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum GeminiAction {
    /// `generateContent`, one reply.
    GenerateContent,
    /// `streamGenerateContent`, a reply in server-sent events (`alt=sse`).
    StreamGenerateContent,
    /// `countTokens`.
    CountTokens,
    /// `embedContent`, one embedding.
    EmbedContent,
    /// `batchEmbedContents`, several embeddings.
    BatchEmbedContents,
}

impl GeminiAction {
    /// Every method, in the order of the variants.
    pub const ALL: [GeminiAction; 5] = [
        GeminiAction::GenerateContent,
        GeminiAction::StreamGenerateContent,
        GeminiAction::CountTokens,
        GeminiAction::EmbedContent,
        GeminiAction::BatchEmbedContents,
    ];

    /// The method's name in the path.
    pub const fn as_str(self) -> &'static str {
        match self {
            GeminiAction::GenerateContent => "generateContent",
            GeminiAction::StreamGenerateContent => "streamGenerateContent",
            GeminiAction::CountTokens => "countTokens",
            GeminiAction::EmbedContent => "embedContent",
            GeminiAction::BatchEmbedContents => "batchEmbedContents",
        }
    }

    /// The method a path names, `None` for one Kaeso doesn't serve.
    pub fn from_wire(name: &str) -> Option<GeminiAction> {
        GeminiAction::ALL.into_iter().find(|a| a.as_str() == name)
    }

    /// The kind of KPP request the method carries.
    pub const fn kind(self) -> RequestKind {
        match self {
            GeminiAction::GenerateContent | GeminiAction::StreamGenerateContent => {
                RequestKind::Generate
            }
            GeminiAction::CountTokens => RequestKind::CountTokens,
            GeminiAction::EmbedContent | GeminiAction::BatchEmbedContents => RequestKind::Embed,
        }
    }

    /// The dialect whose codec reads and writes the method's bodies.
    pub const fn dialect(self) -> DialectId {
        match self {
            GeminiAction::EmbedContent | GeminiAction::BatchEmbedContents => {
                DialectId::GeminiEmbeddings
            }
            _ => DialectId::Gemini,
        }
    }
}

/// Splits the last path segment of a Gemini request, such as
/// `gemini-3.5-flash:streamGenerateContent`, into the percent-decoded model id and the method.
/// `None` when the segment names no method Kaeso serves or its model id isn't valid
/// percent-encoded UTF-8.
pub fn split_model_action(segment: &str) -> Option<(String, GeminiAction)> {
    let (model, action) = segment.rsplit_once(':')?;
    let action = GeminiAction::from_wire(action)?;
    let model = percent_decode(model)?;
    (!model.is_empty()).then_some((model, action))
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while let Some(&b) = bytes.get(i) {
        if b == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_name_the_model_and_the_method() {
        let p = GeminiProfile::GOOGLE_AI_STUDIO;
        assert_eq!(
            p.path("gemini-3.5-flash", GeminiAction::GenerateContent)
                .unwrap(),
            "/v1beta/models/gemini-3.5-flash:generateContent"
        );
        assert_eq!(
            p.path("models/gemini-3.5-flash", GeminiAction::StreamGenerateContent)
                .unwrap(),
            "/v1beta/models/gemini-3.5-flash:streamGenerateContent?alt=sse"
        );
        assert_eq!(
            p.path("tunedModels/my-model-1", GeminiAction::CountTokens)
                .unwrap(),
            "/v1beta/tunedModels/my-model-1:countTokens"
        );
        assert_eq!(
            p.path("a/b:c d", GeminiAction::EmbedContent).unwrap(),
            "/v1beta/models/a%2Fb%3Ac%20d:embedContent"
        );
        assert_eq!(p.model_path("gemini-x").unwrap(), "/v1beta/models/gemini-x");
        assert_eq!(p.list_path(), "/v1beta/models");
        for bad in [".", "..", "models/..", "", "models/"] {
            let e = p.model_path(bad).unwrap_err();
            assert_eq!(
                (e.kind, e.param.as_deref(), e.status),
                (kaeso_kpp::ErrorKind::InvalidRequest, Some("model"), Some(400)),
                "{bad}"
            );
        }
        assert_eq!(GeminiProfile::default(), p);
    }

    #[test]
    fn another_platform_sets_its_own_prefix() {
        let p = GeminiProfile::GOOGLE_AI_STUDIO
            .with_models_path(format!(
                "/v1/projects/{}/locations/{}/publishers/google/models/",
                "p-1", "europe-west4"
            ))
            .with_role_on_every_content(true);
        assert!(p.role_on_every_content);
        assert_eq!(
            p.path("gemini-3.5-flash", GeminiAction::GenerateContent)
                .unwrap(),
            "/v1/projects/p-1/locations/europe-west4/publishers/google/models/gemini-3.5-flash:generateContent"
        );
        let q = GeminiProfile::GOOGLE_AI_STUDIO.with_models_path("/custom/endpoints");
        assert_eq!(q.list_path(), "/custom/endpoints");
        let e = q.model_path("tunedModels/x").unwrap_err();
        assert!(e.message.contains("tuned model"), "{}", e.message);
    }

    #[test]
    fn segments_split_into_model_and_method() {
        assert_eq!(
            split_model_action("gemini-3.5-flash:streamGenerateContent"),
            Some((
                "gemini-3.5-flash".to_owned(),
                GeminiAction::StreamGenerateContent
            ))
        );
        assert_eq!(
            split_model_action("a%3Ab:countTokens"),
            Some(("a:b".to_owned(), GeminiAction::CountTokens))
        );
        assert_eq!(split_model_action("gemini:predict"), None);
        assert_eq!(split_model_action(":generateContent"), None);
        assert_eq!(split_model_action("gemini"), None);
        assert_eq!(split_model_action("%zz:generateContent"), None);
        assert_eq!(split_model_action("%ff:generateContent"), None);
        for a in GeminiAction::ALL {
            assert_eq!(GeminiAction::from_wire(a.as_str()), Some(a));
        }
        assert_eq!(GeminiAction::EmbedContent.kind(), RequestKind::Embed);
        assert_eq!(
            GeminiAction::BatchEmbedContents.dialect(),
            DialectId::GeminiEmbeddings
        );
        assert_eq!(GeminiAction::CountTokens.dialect(), DialectId::Gemini);
        assert_eq!(GeminiAction::CountTokens.kind(), RequestKind::CountTokens);
    }
}
