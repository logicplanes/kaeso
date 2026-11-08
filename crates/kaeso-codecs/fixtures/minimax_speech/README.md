# Fixtures: `minimax_speech`

MiniMax's synchronous text-to-speech API, `POST /v1/t2a_v2` (T2A v2). Nothing here is recorded: no MiniMax key was available.

| file | what it covers | provenance |
|---|---|---|
| `requests/speech.json` | voice and audio settings, `pronunciation_dict`, `voice_modify`, `language_boost`, hex output | doc: https://platform.minimax.io/docs/api-reference/speech/t2a/api/openapi.json example "Request(Non-streaming)" (read 2026-11-08) |
| `responses/speech.json` | hex audio, `extra_info` with `usage_characters`, `base_resp` | doc: as above, the reply example; the hex is ten made-up MP3 bytes |
| `errors/invalid-params-200.json` | a failing `base_resp` with HTTP 200 | schema: `T2aV2Resp.base_resp` and the error code page https://platform.minimax.io/docs/api-reference/errorcode; the message is made up |
