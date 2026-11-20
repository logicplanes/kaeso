# `fixtures/gemini/errors`

| file | what it covers | provenance |
|---|---|---|
| `recorded-error-invalid-argument.json` | 400 `INVALID_ARGUMENT` with a `BadRequest` field violation | RECORDED.md |
| `recorded-error-invalid-key.json` | 400 `INVALID_ARGUMENT` with the reason `API_KEY_INVALID` | RECORDED.md |
| `recorded-error-unknown-model.json` | 404 `NOT_FOUND` | RECORDED.md |
| `recorded-image-output.json` | 429 `RESOURCE_EXHAUSTED` with `QuotaFailure` and `RetryInfo`: no quota for the image model | RECORDED.md |
| `recorded-stream-thinking-high.json` | 503 `UNAVAILABLE`, the model under high demand | RECORDED.md |
| `image-generation-region.json` | 400 `FAILED_PRECONDITION`, image generation not available in the key's country | recorded from `gemini-3.5-flash` with `responseModalities: [TEXT, IMAGE]` on 2026-11-08 03:00 UTC; written again by hand after a later recording replaced the file |
| `file-permission-denied-published.json` | 403 `PERMISSION_DENIED` for a `fileData.fileUri` the key can't read: deleted, expired or another project's file | published: the body a developer posted on the [Google AI Developers Forum](https://discuss.ai.google.dev/t/code-403-message-you-do-not-have-permission-to-access-the-file-oxwwu82fr55g-or-it-may-not-exist-status-permission-denied-retrying/87233) on 2025-06-05, written out as JSON |
| `billing-prepay-depleted.json` | 402 with the status `RESOURCE_EXHAUSTED`: the billing account's prepaid credits are used up | answered live by Google AI Studio on 2026-11-20 to a `generateContent` request with a key whose prepaid balance was empty |
