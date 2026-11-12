/// How a provider that speaks OpenAI's Decisions API takes decision requests: its path, its
/// question limit and whether question instructions may hold images. A request the provider
/// can't take is refused before it is sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct DecisionsProfile {
    /// The provider's name in refusals.
    pub name: &'static str,
    /// The request path; a `/v1/…` path goes under the context's API root.
    pub path: &'static str,
    /// The most questions one request may ask.
    pub max_questions: Option<usize>,
    /// Whether question instructions may hold images, as well as text and JSON.
    pub instruction_images: bool,
}

impl DecisionsProfile {
    /// OpenAI (`api.openai.com`): at most 400 questions, images in the input and in question
    /// instructions (<https://developers.openai.com/api/reference/resources/decisions/methods/create>).
    pub const OPENAI: DecisionsProfile = DecisionsProfile {
        name: "OpenAI",
        path: "/v1/decisions",
        max_questions: Some(400),
        instruction_images: true,
    };

    /// Any other server of the format, such as a custom provider: no question limit Kaeso
    /// checks; its admin sets the path through [`crate::EgressCtx::with_base_path`].
    pub const GENERIC: DecisionsProfile = DecisionsProfile {
        name: "The provider",
        max_questions: None,
        ..DecisionsProfile::OPENAI
    };

    /// The profile sending requests to `path` instead.
    #[must_use]
    pub const fn with_path(mut self, path: &'static str) -> Self {
        self.path = path;
        self
    }

    /// The profile with images in question instructions allowed or refused.
    #[must_use]
    pub const fn with_instruction_images(mut self, allowed: bool) -> Self {
        self.instruction_images = allowed;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_differ_where_their_providers_do() {
        assert_eq!(DecisionsProfile::OPENAI.max_questions, Some(400));
        assert_eq!(DecisionsProfile::GENERIC.max_questions, None);
        let p = DecisionsProfile::GENERIC
            .with_path("/decide")
            .with_instruction_images(false);
        assert_eq!((p.path, p.instruction_images), ("/decide", false));
    }
}
