# `fixtures/openai_images/requests`

| file | what it covers | provenance |
|---|---|---|
| `generate.json` | the reference's generation example | doc: openai-openapi@926a7b8 `/images/generations` example "Generate image" |
| `generate-every-field.json` | every field of `CreateImageRequest` a GPT image model takes, with streaming and both user fields | schema: openai-openapi@926a7b8 `CreateImageRequest` |
| `generate-stream-off.json` | an explicit `stream: false` | schema: openai-openapi@926a7b8 `CreateImageRequest.stream` |
| `generate-user.json` | the deprecated `user` alone | schema: openai-openapi@926a7b8 `CreateImageRequest.user` |
| `generate-gif.json` | GIF output with `frame_count` and `frame_duration_ms` | schema: openai-openapi@926a7b8 `frame_count`, `frame_duration_ms` |
| `generate-svg.json` | SVG output with the `svg` author object | schema: openai-openapi@926a7b8 `svg` |
| `generate-legacy-dall-e-3.json` | the retired DALL·E 3 fields `style`, `response_format`, `quality: hd` | schema: openai-openapi@926a7b8 `CreateImageRequest` (deprecated values) |
| `stream-generate.json` | the reference's streaming example | doc: openai-openapi@926a7b8 `/images/generations` example "Streaming" |
| `edit-json-url.json` | a JSON edit with an image link | doc: openai-openapi@926a7b8 `EditImageBodyJsonParam` example `json_with_url` |
| `edit-json-file-id.json` | a JSON edit with file ids and a mask | doc: openai-openapi@926a7b8 `EditImageBodyJsonParam` example `json_with_file_id` |
| `edit-json-data-url.json` | a data URL image, `markup`, `input_fidelity` and partial images | schema: openai-openapi@926a7b8 `EditImageBodyJsonParam`, `ImageRefParam` |
| `edit-upload.multipart` | a multipart upload of two images and a mask, with every scalar field, `svg[model]` and `frame_count` | schema: openai-openapi@926a7b8 `CreateImageEditRequest`; field names as openai-python writes them |
| `edit-upload.meta.json` | the upload's content type with its boundary | as above |
