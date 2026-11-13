# system_one / providers / cloudflare-ai-gateway (Cloudflare AI Gateway)

Cloudflare's Clef decision models on Workers AI, `POST https://api.cloudflare.com/client/v4/accounts/{account_id}/ai/run/@cf/cloudflare/{model}` with a Cloudflare API token as `Authorization: Bearer` and, when the key names one, the gateway in `cf-aig-gateway-id`. The body is a System One body that names the model by its short name (`clef`, `clef-flash`, `clef-omni`) and takes images in a top-level `images` list; the reply is the `result` of Cloudflare's envelope, `{"result", "success", "errors", "messages"}`. Kaeso's profile is `SystemOneProfile::CLOUDFLARE_WORKERS_AI`.

## Sources (read 2026-11-13)

| Source | Address |
|---|---|
| Model pages, with the input and output schemas | https://developers.cloudflare.com/workers-ai/models/clef/, `/clef-flash/`, `/clef-omni/` (`schema-input.json`, `schema-output.json`) |
| AI Gateway REST API (the model-in-path Workers AI endpoint and `cf-aig-gateway-id`) | https://developers.cloudflare.com/ai-gateway/usage/rest-api/#call-a-workers-ai-model |
| Workers AI errors | https://developers.cloudflare.com/workers-ai/platform/errors/ |
| Changelog | https://developers.cloudflare.com/changelog/post/2026-10-01-clef-workers-ai/, https://developers.cloudflare.com/changelog/post/2026-10-09-clef-omni-workers-ai/ |

Every `recorded-` file was recorded live on 2026-11-13 with curl, the request body sent as the file of the same name without `recorded-` (or the request named below), and is kept as Cloudflare answered it. No file holds the account id or the token.

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/support.json`, `responses/recorded-support.json` | the model page's example: a yes/no, a choice and a score question to `clef`; a yes/no answer without `confidence` | doc: Clef model page, verbatim; reply live |
| `requests/support-flash.json`, `responses/recorded-support-flash.json` | the same request to `clef-flash` | doc, with the model name changed; reply live |
| `requests/support-omni.json`, `responses/recorded-support-omni.json` | the same request to `clef-omni` | doc, with the model name changed; reply live |
| `images/colour.app.json` | a System One request as an app writes it: a PNG asset as the first item of a state array | built |
| `images/colour.sent.json` | what Kaeso sends for it: the image as a data URL in `images`, the state without it | built by the codec; sent live as it is |
| `images/recorded-colour.json` | Clef's answer to it | live |
| `errors/recorded-invalid-token-401.json` | 401, code 10000 `Authentication error`, for a request without a token | live |
| `errors/recorded-too-many-questions-422.json` | 422, code 3030: 65 questions | live |
| `errors/recorded-one-option-422.json` | 422, code 3030: a choice with one option | live |
| `errors/recorded-one-level-422.json` | 422, code 3030: a score with one level | live |
| `errors/recorded-no-instructions-400.json` | 400, code 5006: a question without instructions | live |
| `errors/recorded-too-many-images-422.json` | 422, code 3030: 5 images | live |
| `errors/recorded-unknown-field-422.json` | 422, code 3030: a top-level field the schema doesn't have | live |
| `errors/recorded-wrong-model-422.json` | 422, code 3030: `clef-flash` named in the body of a request to `@cf/cloudflare/clef` | live |
| `errors/recorded-unknown-gateway-400.json` | 400, code 2001: `cf-aig-gateway-id` naming a gateway the account doesn't have | live |

Each error has a `.meta.json` with its status.

## Where Cloudflare differs from TypeSafe

| Difference | How KPP carries it | Source |
|---|---|---|
| the path names the account and the model: `/client/v4/accounts/{account_id}/ai/run/@cf/cloudflare/{model}` | the key's account becomes the API root, and the profile's path `/run/@cf/cloudflare/{model}` goes beside its `/v1` | REST API page |
| the body names the model by its short name | `ModelField::ShortName("@cf/cloudflare/")`; the reply's `clef` reads as `@cf/cloudflare/clef` | model pages, live |
| the reply is the `result` of an envelope | `Envelope::Result`; the envelope's other fields stay in extras and never reach an app | live |
| 1 to 64 questions, each with instructions; 2 to 255 options, 2 to 10 levels; no box questions | refused before sending | input schema, live |
| images in a top-level `images` list, at most 4, PNG, JPEG or WebP, 4 MiB and 16 megapixels each, 8 MiB together | `MediaPlacement::Root`, refused beyond those limits | input schema, live |
| a field the schema doesn't have is a 422 | an app's own fields are refused before sending | live |
| errors in Cloudflare's envelope with `success: false` | read like an OpenAI error, with Cloudflare's message | live |

## Doubts

- OpenRouter's multimodal guide says Workers AI reads only the first ~2,000 tokens of a text state. A 10,350-token state with the asked-about fact at its end was answered correctly and billed as 10,350 input tokens, so the state is truncated only to fit the context, as the model pages say.
- Clef-omni's `audio` and `videos` lists have no place in KPP; Kaeso sends images only.
