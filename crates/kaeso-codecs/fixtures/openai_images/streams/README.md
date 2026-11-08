# `fixtures/openai_images/streams`

| file | what it covers | provenance |
|---|---|---|
| `generate.sse` | the reference's generation stream: one partial image, then the finished one with usage | doc: openai-openapi@926a7b8 `/images/generations` example "Streaming" (`b64_json` filled with a 1×1 PNG) |
| `edit-every-field.sse` | an edit stream with every field of `ImageEditPartialImageEvent` and `ImageEditCompletedEvent` | doc: openai-openapi@926a7b8 `ImageGenPartialImageEvent` and `ImageEditCompletedEvent` examples |
