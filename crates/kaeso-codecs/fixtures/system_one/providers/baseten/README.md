# system_one / providers / baseten (Baseten)

Baseten's Model APIs, provider id `baseten`, `POST https://inference.baseten.co/v1/systemone`. Sources, read on 2026-11-12: the System One reference https://docs.baseten.co/reference/inference-api/system-one and the guide https://docs.baseten.co/inference/model-apis/decision-models. Images go in a user message under `state.messages`, as `image_url` parts with PNG, JPEG or WebP data URLs, at most 16; `service_tier` is Baseten's own top-level field and travels as Extras.

No key was available; every file is spec-built.

| file | what it covers | provenance |
|---|---|---|
| `requests/screenshot.json`, `responses/screenshot.json` | a conversation state with a text and an image part; `service_tier` | doc: the guide's "Evaluate images" example, with a real 280 × 280 PNG; the reply is built from its printed probability and the reference's answer fields |
| `requests/route.json`, `responses/route.json` | three question types, text state | doc: the joey-1 changelog example (https://www.baseten.co/resources/changelog/joey-1-available-on-baseten/); the reply is built from the reference's answer fields with `output_tokens: 0` |
