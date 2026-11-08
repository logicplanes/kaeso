# Fixtures: `minimax_images`

Wire bodies for MiniMax's `POST /v1/image_generation`, built from https://platform.minimax.io/docs/api-reference/image-generation-t2i and the error code reference, read 2026-11-08. None is recorded: no MiniMax key with image access was available.

## requests

| file | what it covers | provenance |
|---|---|---|
| `generate.json` | the reference's example with `aspect_ratio`, `n` and `prompt_optimizer` | doc: the reference's request example |
| `generate-size-seed.json` | `width`, `height`, `seed`, `response_format: base64` | schema: `ImageGenerationReq` |

## responses

| file | what it covers | provenance |
|---|---|---|
| `generate-urls.json` | the reference's reply with three links, `metadata` and `base_resp` | doc: the reference's response example (links filled in) |
| `generate-base64.json` | one base64 image | schema: `data.image_base64` |

## errors

| file | what it covers | provenance |
|---|---|---|
| `sensitive-prompt-200.json` | `1026` in `base_resp` with HTTP 200 | doc: error code reference |
| `invalid-key-200.json` | `2049` in `base_resp` with HTTP 200 | doc: error code reference |

`*.meta.json` give the HTTP status.
