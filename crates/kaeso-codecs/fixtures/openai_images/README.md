# Fixtures: `openai_images`

Wire bodies for `POST /v1/images/generations`, `POST /v1/images/edits` and the providers that speak a form of OpenAI's Images API.

## Sources

- OpenAI OpenAPI spec at commit `926a7b8133226630bac025350e84b0ca5c31d45a` (2026-11-07): `CreateImageRequest`, `CreateImageEditRequest`, `EditImageBodyJsonParam`, `ImageRefParam`, `ImagesResponse`, `Image`, `ImageGenUsage`, `ImagesUsage`, `ImageGenPartialImageEvent`, `ImageGenCompletedEvent`, `ImageEditPartialImageEvent`, `ImageEditCompletedEvent`, `ErrorResponse`.
- openai-python 3.41.0 (`images.generate`, `images.edit`, `images.create_variation`), whose multipart field names (`image[]`, `svg[model]`) the upload fixture follows.
- No fixture here is recorded: no OpenAI or provider key was available. Every file is built from the spec (`schema:`) or a documented example (`doc:`). Images are a 1×1 PNG.

The layout and the provenance marks are in [Fixtures and recordings](https://docs.kaeso.app/development/kpp/fixtures). Each folder's `README.md` lists every file.

## Doubts

- The spec defines no error event for image streams; the codec writes OpenRouter's documented `{"type": "error", "error": {…}}`.
- Error messages in `errors/` are worded after the guide; confirm them by recording.
