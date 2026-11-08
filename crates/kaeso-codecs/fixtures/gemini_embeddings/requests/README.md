# `fixtures/gemini_embeddings/requests`

| file | what it covers | provenance |
|---|---|---|
| `embed-single.json` | `embedContent` with the top-level `outputDimensionality` | schema: EmbedContentRequest (discovery v1beta revision 20261106) |
| `embed-config.json` | `embedContentConfig` with `taskType` and `outputDimensionality`, `model` in the body | schema: EmbedContentConfig (discovery v1beta revision 20261106) |
| `embed-task-title.json` | `taskType: RETRIEVAL_DOCUMENT` with `title`, `gemini-embedding-001` | doc: embeddings, task types |
| `batch.json` | `batchEmbedContents` with two requests | schema: BatchEmbedContentsRequest (discovery v1beta revision 20261106) |
