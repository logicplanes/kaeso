# Fixtures: `gemini_embeddings`

Wire bodies for `POST /v1beta/models/{model}:embedContent` and `:batchEmbedContents` on Google AI Studio.

The layout and the provenance marks are in [Fixtures and recordings](https://docs.kaeso.app/development/kpp/fixtures). The sources are those of [`gemini`](../gemini/README.md): the discovery document at revision `20261106` (`EmbedContentRequest`, `EmbedContentConfig`, `BatchEmbedContentsRequest`, `EmbedContentResponse`, `BatchEmbedContentsResponse`, `ContentEmbedding`, `EmbeddingUsageMetadata`) and the embeddings guide. Every reply was recorded on 2026-11-08 with `cargo xtask record gemini_embeddings`; `gemini-embedding-2` and `gemini-embedding-001` both return `usageMetadata`.
