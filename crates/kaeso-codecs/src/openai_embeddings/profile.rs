/// How a provider of OpenAI's embeddings API takes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct EmbeddingsProfile {
    /// The provider's name, for the errors of the fields it doesn't take.
    pub provider: &'static str,
    /// Whether the provider takes `dimensions`. One that ignores it answers vectors of the
    /// model's full size, so the field is refused before anything is sent.
    pub dimensions: bool,
}

impl EmbeddingsProfile {
    pub const OPENAI: EmbeddingsProfile = EmbeddingsProfile {
        provider: "OpenAI",
        dimensions: true,
    };

    /// OpenAI's embeddings API as another provider serves it.
    pub const fn compatible(provider: &'static str) -> Self {
        EmbeddingsProfile {
            provider,
            ..EmbeddingsProfile::OPENAI
        }
    }

    /// The provider doesn't take `dimensions` ([`EmbeddingsProfile::dimensions`]).
    #[must_use]
    pub const fn without_dimensions(mut self) -> Self {
        self.dimensions = false;
        self
    }
}

impl Default for EmbeddingsProfile {
    fn default() -> Self {
        EmbeddingsProfile::OPENAI
    }
}
