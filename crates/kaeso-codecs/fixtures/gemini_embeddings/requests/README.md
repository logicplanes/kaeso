# `fixtures/gemini_embeddings/requests`

| file | what it covers | provenance |
|---|---|---|
| `embed-single.json` | `embedContent` with the top-level `outputDimensionality` | schema: EmbedContentRequest (discovery v1beta revision 20261106) |
| `embed-config.json` | `embedContentConfig` with `taskType` and `outputDimensionality`, `model` in the body | schema: EmbedContentConfig (discovery v1beta revision 20261106) |
| `embed-task-title.json` | `taskType: RETRIEVAL_DOCUMENT` with `title`, `gemini-embedding-001` | doc: embeddings, task types |
| `batch.json` | `batchEmbedContents` with two requests | schema: BatchEmbedContentsRequest (discovery v1beta revision 20261106) |
| `sdk-batch-user-role.json` | `batchEmbedContents` as google-genai 2.32.0 sends `embed_content` for `gemini-embedding-2`: one content with two parts and `role: user` | sdk: google-genai 2.32.0, `models.embed_content(model="gemini-embedding-2", contents=[…, …])` |
