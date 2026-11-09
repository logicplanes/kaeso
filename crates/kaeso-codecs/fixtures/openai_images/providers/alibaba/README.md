# openai_images / providers / alibaba

QwenCloud (`alibaba`), built from the provider's documentation on 2026-11-08; not recorded, since no key was available.

## responses

| file | what it covers | provenance |
|---|---|---|
| `generate.json` | a 24-hour link with the qwen-image-3.0 metering usage | doc: https://docs.qwencloud.com/api-reference/image-generation/qwen-text-to-image-openai, read 2026-11-08 |
| `generate-2k.json` | a 2048 by 2048 image billed at the 2K tier | spec-built: `generate.json` with the 2K tier's `output_image_type` and size from the same reference |
