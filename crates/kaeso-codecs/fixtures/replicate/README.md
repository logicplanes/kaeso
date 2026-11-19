# Fixtures: `replicate`

Wire bodies for Replicate's predictions (`https://api.replicate.com`, `Authorization: Bearer r8_…`): `POST /v1/models/{owner}/{name}/predictions` with `{"input": …}`, the prediction object it and `GET /v1/predictions/{id}` answer, and `GET /v1/collections/official`. No key was available: the finished predictions are the ones Replicate published on its model pages or are built from each model's schema; the errors were answered live without a valid key on 2026-11-14.

## Sources (read 2026-11-14)

- HTTP API reference and its OpenAPI document (`schemas_prediction_response`, `schemas_collection_response`): https://replicate.com/docs/reference/http
- Creating a prediction, with the `starting` reply: https://replicate.com/docs/topics/predictions/create-a-prediction
- Rate limits, with the 429 body: https://replicate.com/docs/topics/predictions/rate-limits
- Error codes: https://replicate.com/docs/reference/error-codes
- Each model's schema: `https://replicate.com/{owner}/{name}/api/schema`

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/images/*.json`, `requests/videos/*.json` | the input Kaeso sends each mapped model, with the model in `<name>.meta.json` | built from each model's schema, except the two below |
| `requests/videos/veo31-references-published.json`, `requests/videos/kling-published.json` | Veo 3.1 with reference images; Kling with its unused `guidance_scale` | the `input` of the predictions Replicate published on the models' pages |
| `predictions/images/flux-schnell-published.json`, `seedream-4.5-published.json`, `predictions/videos/veo-3.1-published.json`, `kling-published.json` | finished predictions with a list of links or one link | published on the models' pages; the page-only `_extras` object left out |
| `predictions/images/starting-doc.json` | a prediction still starting | the create-a-prediction page's reply, its webhook fields left out |
| `predictions/images/flux-pro-built.json`, `nano-banana-pro-built.json` | single-link outputs with a custom size and a resolution | built from the prediction schema and the models' `Output` |
| `predictions/*/starting-built.json`, `processing-built.json`, `failed-built.json`, `canceled-built.json`, `removed-built.json` | the other statuses, a failure with an `E####` code, output removed after an hour | built from the prediction schema; the wording of `error` isn't documented |
| `models/official-collection-built.json` | the official models collection | built from `schemas_collection_response` |
| `errors/recorded-*.json` | 401 without a token, with a made-up token, on the model and prediction paths | live |
| `errors/throttled-429-doc.json` | 429 | the rate-limits page's body |
