# `fixtures/openai_images/responses`

| file | what it covers | provenance |
|---|---|---|
| `generate.json` | the reference's reply, with `ImageGenUsage` | doc: openai-openapi@926a7b8 `/images/generations` example "Generate image" (`b64_json` filled with a 1×1 PNG) |
| `generate-every-field.json` | two images, `background`, `output_format`, `size`, `quality`, `output_tokens_details`, `safety_identifier` | schema: openai-openapi@926a7b8 `ImagesResponse`, `ImageGenUsage` |
| `generate-svg.json` | SVG output with `svg_usage` | schema: openai-openapi@926a7b8 `ImagesResponse.svg_usage`, `SvgUsage` |
| `legacy-dall-e-3-url.json` | a link with `revised_prompt` | schema: openai-openapi@926a7b8 `Image.url`, `Image.revised_prompt` |
