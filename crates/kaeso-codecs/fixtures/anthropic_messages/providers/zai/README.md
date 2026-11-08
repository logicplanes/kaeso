# Fixtures: Z.ai (`zai`) on `anthropic_messages`

Z.ai's Anthropic-compatible API, `POST https://api.z.ai/api/anthropic/v1/messages`, with `x-api-key` (Z.ai's own examples also send `Authorization: Bearer`; both reach the key check, see below) and `anthropic-version: 2023-06-01`. It serves Z.ai's GLM models.

## Sources (read 2026-11-08)

| Source | Address |
|---|---|
| Error codes, their HTTP statuses and messages | https://docs.z.ai/api-reference/api-code |
| The Anthropic endpoint's base URL | https://docs.z.ai/devpack/quick-start, https://docs.z.ai/devpack/notice/api-key-management |
| Thinking and effort on the Anthropic endpoint | https://docs.z.ai/devpack/latest-model, https://docs.z.ai/guides/capabilities/thinking-mode |
| Implicit prompt caching | https://docs.z.ai/guides/capabilities/cache |
| The China endpoint, `https://open.bigmodel.cn/api/anthropic` | https://docs.bigmodel.cn/cn/guide/develop/claude/introduction |

Z.ai documents no example reply, stream, token count or model listing for its Anthropic endpoint, and no list of the Anthropic fields it takes. No working key was available, so there is no recorded reply or stream: only errors. Nothing here holds a key or personal data.

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/invalid-api-key.json` | A plain request to `glm-4.6` | written for recording |
| `errors/recorded-invalid-api-key.json` | 401 for a key Z.ai doesn't accept: OpenAI's error envelope, with Z.ai's code `1000` as `type` and a Chinese message | live, `cargo xtask record`, `glm-4.6` |
| `errors/missing-api-key.json` | 401 for a request without a key, code `1001` | live, 2026-11-08 01:37 UTC, the same request sent with `curl` and no key |
| `errors/list-invalid-api-key.json` | `GET /v1/models` with a key Z.ai doesn't accept: HTTP 200 with Z.ai's platform envelope `{"code", "msg", "success": false}`. Z.ai answers `GET /v1/models/glm-4.6` and a made-up path the same way, so whether it serves a model listing is unknown | live, 2026-11-08 01:37 UTC, with `curl` |
| `errors/insufficient-balance.json` | 429 with code `1113`, an empty balance | doc: the error code table, in the shape of the recorded errors |
| `errors/unknown-model.json` | 400 with code `1211` | doc: as above |
| `errors/prompt-too-long.json` | 400 with code `1261` | doc: as above |
| `errors/sensitive-content.json` | 400 with code `1301` | doc: as above |
| `errors/overloaded.json` | 429 with code `1305` | doc: as above |
| `errors/plan-lacks-model.json` | 429 with code `1311`, with `glm-5.3` for the table's `${model_name}` | doc: as above |
| `errors/rate-limited.json` | 429 with code `1302` | doc: as above |

The `doc:` errors put the table's message and code into the shape Z.ai sent on 2026-11-08 (`{"error": {"message", "type"}}`), which the general API's own example writes as `{"error": {"code", "message"}}`. Each error's `meta.json` holds under `encoded` the Anthropic error an Anthropic app gets, and `encoded_status` its status where it differs from Z.ai's.

## Probes (2026-11-08, not stored)

- `x-api-key` and `Authorization: Bearer` with the same key both got the 401 `1000`; a request without a key got `1001`.
- `POST /v1/messages/count_tokens` got the same 401s as `/v1/messages`, while a made-up path got the HTTP 200 envelope, so Z.ai may serve token counts; it documents none, and no response could be seen without a working key.

## What KPP does with Z.ai's own fields

- The error decoder takes the kind from Z.ai's code where the status would mislead apps: `1113`, `1309` and `1314` are billing, `1211` not found, `1220`, `1311` and `1315` permission, `1261` a context that is too long, `1301` filtered content and `1305` overloaded; `1000`, `1001`, `1003` and `1005` are authentication. Other codes follow the status.
- The platform envelope reads as an error, also with HTTP 200, so a model check with a wrong key reports the refused key.
