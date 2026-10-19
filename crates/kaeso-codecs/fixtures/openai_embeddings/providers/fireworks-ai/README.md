# `fixtures/openai_embeddings/providers/fireworks-ai`

Fireworks AI, provider id `fireworks-ai`, over its OpenAI-compatible embeddings API at `https://api.fireworks.ai/inference/v1/embeddings`, with `authorization: Bearer`.

Sources, read 2026-10-19:

- API reference: https://docs.fireworks.ai/api-reference/creates-an-embedding-vector-representing-the-input-text
- Guide: https://docs.fireworks.ai/guides/querying-embeddings-models

There is no live recording: no test key was used. Both files are request bodies copied from curl examples of the guide, re-indented and otherwise unchanged.

| file | what it covers | provenance |
|---|---|---|
| `requests/llm-hidden-state-normalize-doc.json` | `normalize: true` for an LLM's hidden-state embedding, model `fireworks/gpt-oss-120b`; `normalize` is kept as an extra | guide, curl under the LLM embeddings section |
| `requests/bge-small-string-array-doc.json` | an array input to `BAAI/bge-small-en-v1.5`, a model id without `accounts/` | guide, curl for the legacy models |

The documentation has no complete embeddings reply or error body: the reference's example is copied from OpenAI's and is not valid JSON, and the rendered examples are generated from the schema. So there are no response or error fixtures.
