/// How a provider of OpenAI's embeddings API takes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct EmbeddingsProfile {
    /// The provider's name, for the errors of the fields it doesn't take.
    pub provider: &'static str,
    /// Whether the provider takes `dimensions`. One that ignores it answers vectors of the
    /// model's full size, so the field is refused before anything is sent.
    pub dimensions: bool,
    /// The name the provider gives `dimensions`, such as Voyage AI's `output_dimension`.
    pub dimensions_field: &'static str,
    /// The only sizes the provider takes for `dimensions`; any size when empty.
    pub dimension_sizes: &'static [u32],
    /// Whether the provider takes `user`. One that refuses unknown fields gets it refused
    /// before anything is sent.
    pub user: bool,
    /// Whether the provider takes `encoding_format: "float"`. One that knows only `base64` gets
    /// no `encoding_format` for floats, which it sends by default.
    pub float_encoding: bool,
    /// Whether the provider takes inputs as token ids.
    pub token_inputs: bool,
}

impl EmbeddingsProfile {
    pub const OPENAI: EmbeddingsProfile = EmbeddingsProfile {
        provider: "OpenAI",
        dimensions: true,
        dimensions_field: "dimensions",
        dimension_sizes: &[],
        user: true,
        float_encoding: true,
        token_inputs: true,
    };

    /// Voyage AI's `/v1/embeddings` (<https://www.mongodb.com/docs/voyageai/api-reference/overview/>):
    /// `output_dimension` with 256, 512, 1024 or 2048 in place of `dimensions`, no `user`, an
    /// `encoding_format` of `base64` only and text inputs only. It refuses every field it
    /// doesn't know with a 400, so each of these is mapped or refused before sending.
    pub const VOYAGE: EmbeddingsProfile = EmbeddingsProfile::compatible("Voyage AI")
        .with_dimensions_as("output_dimension", &[256, 512, 1024, 2048])
        .without_user()
        .without_float_encoding()
        .without_token_inputs();

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

    /// The provider takes `dimensions` under `field`, and only the sizes `sizes` when there are
    /// any ([`EmbeddingsProfile::dimensions_field`], [`EmbeddingsProfile::dimension_sizes`]).
    #[must_use]
    pub const fn with_dimensions_as(mut self, field: &'static str, sizes: &'static [u32]) -> Self {
        self.dimensions_field = field;
        self.dimension_sizes = sizes;
        self
    }

    /// The provider doesn't take `user` ([`EmbeddingsProfile::user`]).
    #[must_use]
    pub const fn without_user(mut self) -> Self {
        self.user = false;
        self
    }

    /// The provider doesn't take `encoding_format: "float"`
    /// ([`EmbeddingsProfile::float_encoding`]).
    #[must_use]
    pub const fn without_float_encoding(mut self) -> Self {
        self.float_encoding = false;
        self
    }

    /// The provider takes text inputs only ([`EmbeddingsProfile::token_inputs`]).
    #[must_use]
    pub const fn without_token_inputs(mut self) -> Self {
        self.token_inputs = false;
        self
    }
}

impl Default for EmbeddingsProfile {
    fn default() -> Self {
        EmbeddingsProfile::OPENAI
    }
}
