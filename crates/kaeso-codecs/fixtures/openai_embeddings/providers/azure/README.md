# openai_embeddings / providers / azure (Azure)

Azure, provider id `azure`, `POST {endpoint}/openai/v1/embeddings` of each key's resource with the key in `api-key`. Reference: Azure's [embeddings guide](https://learn.microsoft.com/en-us/azure/foundry/openai/how-to/embeddings) and its v1 OpenAPI document (`CreateEmbeddingRequest`), read on 2026-11-14.

No key was available. The guide's reply is abridged (`{"data":[{"embedding":[<vector-values>]}]}`), so no reply is built from it; only the request and the error are here.

| file | what it covers | provenance |
|---|---|---|
| `requests/quick-brown-fox-doc.json` | the guide's request to `text-embedding-3-small` | doc |
| `errors/recorded-invalid-api-key.json` | 401 with `code: "401"` as a string | live, with a made-up key |

The request body is OpenAI's: `input`, `model`, `dimensions`, `encoding_format`, `user`. `model` names a deployment, which Kaeso fills with the model's `upstream_model` when it has one.
