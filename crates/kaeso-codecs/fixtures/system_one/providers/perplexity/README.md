# system_one / providers / perplexity (Perplexity)

Perplexity's Decisions API, `POST https://api.perplexity.ai/v1/decisions` with `Authorization: Bearer`: a System One body at another path. Source: https://docs.perplexity.ai/docs/decisions/quickstart, read on 2026-11-12. Images are `image_url` parts with PNG, JPEG or WebP data URLs anywhere in the state, at most 8; an unknown top-level field is a 400, so Kaeso refuses an app's own fields before sending.

No key was available; every file is spec-built. Perplexity isn't a built-in provider yet; the folder carries the format's state images.

| file | what it covers | provenance |
|---|---|---|
| `requests/review.json`, `responses/review.json` | the quickstart's three questions; a reply whose score `confidence` follows Perplexity's own formula, kept as sent | doc: quickstart steps 3 and 4, verbatim |
| `requests/square-color.json` | an `image_url` part as an item of an array state; options without descriptions | doc: quickstart ("Images in state", array), with a real 280 × 280 PNG |
| `requests/object-photo.json` | an `image_url` part as a value inside an object state | doc: quickstart ("Images in state", object), with a real 280 × 280 JPEG |
| `requests/image-state.json` | the image part itself as the state | doc: quickstart ("Images in state", only an image), with a real 300 × 300 PNG |
| `errors/invalid-model-400.json`, `.meta.json` | 400 for an unknown model in the OpenAI-style envelope | doc: quickstart ("Model names"), verbatim |
| `errors/noul-needs-criteria-400.json`, `.meta.json` | 400 with a string `code` | doc: quickstart ("Errors"), verbatim |
