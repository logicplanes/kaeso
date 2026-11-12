# system_one / providers / openrouter (OpenRouter)

OpenRouter, provider id `openrouter`, base URL `https://openrouter.ai/api`, `POST /v1/systemone` with `Authorization: Bearer`, for apps that switch TypeSafe's SDKs to OpenRouter. Sources, read on 2026-11-12: the Jev guide https://openrouter.ai/docs/guides/community/jev, the Decisions API reference https://openrouter.ai/docs/api/api-reference/alphadecisions/submit-a-decisions-request (same body and reply), and the multimodal guide https://openrouter.ai/docs/guides/community/multimodal-decisions (images as `image_url` parts in the state or in `assets`, no uploads).

No key was available; every file is spec-built.

| file | what it covers | provenance |
|---|---|---|
| `requests/ticket.json`, `responses/ticket.json` | three question types; the reply's `id`, `provider` and `usage.cost`, which travel as Extras; probabilities written as integers (`0`) | doc: Decisions API reference, request and 200 examples, verbatim |
| `requests/listing-box.json`, `responses/listing-box.json` | a box question about an `image_url` part in an array state | doc: Jev guide ("Box questions"), with a real 300 × 300 PNG in place of the shortened base64; the reply's `model`, `usage` and `id` are built |
| `errors/payment-required-402.json`, `.meta.json` | 402 in OpenRouter's `error` envelope (KPP Billing) | doc: Decisions API reference, 402 example, verbatim |
