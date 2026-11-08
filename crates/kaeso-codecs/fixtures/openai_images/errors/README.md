# `fixtures/openai_images/errors`

| file | what it covers | provenance |
|---|---|---|
| `moderation-blocked-400.json` | 400 `moderation_blocked` | doc: https://developers.openai.com/api/docs/guides/image-generation (moderation blocks return `error.code = "moderation_blocked"`) |
| `moderation-blocked-400.meta.json` | HTTP status 400 | as above |
| `rate-limit-429.json` | 429 `slow_down` | schema: openai-openapi@926a7b8 `components.responses.InferenceRateLimited` |
| `rate-limit-429.meta.json` | HTTP status 429 | as above |
| `variations-retired-410.json` | 410 from the retired variations endpoint | doc: openai-openapi@926a7b8 `/images/variations` 410 response ("The image variations endpoint is no longer available.") |
| `variations-retired-410.meta.json` | HTTP status 410 | as above |
